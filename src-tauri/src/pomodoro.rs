//! 番茄钟：tokio 事件驱动状态机（与 reminder.rs 同模式：mpsc + sleep_until + select!，无轮询）。
//!
//! # 运行时形态
//! - `PomodoroHandle`（命令发送端）存入 `AppState.pomodoro`，命令层/快捷键经它投递；
//! - `loop_task` 持有状态机（Idle / Running），仅 Running 且非暂停时睡到 `ends_at`；
//! - 运行中快照（供 `pomodoro_state` 命令与前端事件）存在模块级 OnceLock<Mutex>，
//!   不强塞 AppState（AppState 只持通道句柄）。
//!
//! # 事件契约（Rust → JS）
//! - `pomodoro-state`：`{running, phase, endsAt, paused, remainingSec, noteId, taskKey, taskText, completedInCycle}`
//!   phase ∈ idle | focus | short_break | long_break，每次状态变化全量广播；
//! - `pomodoro-finished`：`{phase, nextPhase, noteId, taskKey, taskText}`
//!   nextPhase 为 null 表示回 Idle（无自动续接）。
//!
//! # 隐私与提醒防线
//! - 私密便签（is_private=1）：Start 时 task_text 全程置空（不入库快照、不进事件/通知/托盘），
//!   阶段结束通知前再复查一次（防会话中途切换隐私标志）；
//! - 勿扰：pomo_force_remind=true 跳过勿扰判定；否则复用 crate::dnd，推迟到勿扰结束再发；
//! - 全屏：通知延迟等待主路径事件驱动——window/foreground.rs 检测到全屏变化即投递
//!   `FullscreenChanged`（写 FULLSCREEN_HINT 提示位 + FULLSCREEN_WATCH 唤醒等待方）；
//!   2s 轮询实时探测仅作兜底（硬顶 4 小时）。等待在独立任务里，事件循环不受阻塞。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::watch;

use crate::db::pomodoro_sessions;
use crate::error::AppResult;
use crate::state::AppState;

/// 前端事件名（约定见 src/types/index.ts#EVENTS；字符串统一定义在 crate::events）
pub use crate::events::{POMODORO_FINISHED as FINISHED_EVENT, POMODORO_STATE as STATE_EVENT};

/// 全屏时通知重查间隔（秒）——事件驱动等待的超时兜底周期
const FULLSCREEN_POLL_SECS: u64 = 2;
/// 全屏等待硬顶：4 小时（防通知永久滞留）
const MAX_FULLSCREEN_WAIT_SECS: u64 = 4 * 60 * 60;
/// 启动恢复补发窗口（与 reminder 的错过兜底一致）
const RECOVER_CUTOFF_HOURS: i64 = 24;

// ---------- 阶段 ----------

/// 番茄钟阶段。as_str/parse 与 DB phase 列字面量一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Focus,
    ShortBreak,
    LongBreak,
}

impl Phase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Phase::Focus => "focus",
            Phase::ShortBreak => "short_break",
            Phase::LongBreak => "long_break",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "short_break" => Phase::ShortBreak,
            "long_break" => Phase::LongBreak,
            "focus" => Phase::Focus,
            _ => Phase::Focus,
        }
    }
}

// ---------- 命令通道 ----------

#[derive(Debug, Clone)]
pub enum PomodoroCmd {
    Start {
        note_id: String,
        task_key: String,
        task_text: String,
    },
    Pause,
    Resume,
    Skip,
    Stop {
        reason: String,
    },
    /// 仅当当前运行会话绑定指定 task_key 时才停止：
    /// "任务完成 → 停番茄"联动用，判定与停止在状态机内原子完成，防 Start 竞态误杀新会话。
    StopIfTask {
        task_key: String,
        reason: String,
    },
    /// 私密翻转后的机内文本清洗：运行会话匹配便签/今日任务则清空 task_text 并重播。
    /// DB 快照清理由调用方配合 DAO 完成（scrub_private_text / scrub_daily_task_text）。
    ScrubText {
        note_id: String,
        task_key: String,
    },
    AddMinutes(i64),
    /// 状态机内保留：命令层当前直调 pomodoro::complete_task，不经通道
    #[allow(dead_code)]
    CompleteTask,
    /// 全屏状态变化（window/foreground.rs 检测线程投递）：写提示位 + watch 唤醒通知延迟等待
    FullscreenChanged(bool),
    /// 立即重发布一次状态事件
    #[allow(dead_code)]
    ForceWake,
    /// 托盘/全局快捷键的统一切换：Idle→Start（无绑定任务），Running→Pause/Resume
    Toggle,
}

/// 命令信封：`ack` 非空时状态机处理完该命令后回发最新快照（写命令同步返回，见 send_sync）。
struct CmdEnvelope {
    cmd: PomodoroCmd,
    ack: Option<tokio::sync::oneshot::Sender<PomoSnapshot>>,
}

#[derive(Clone)]
pub struct PomodoroHandle(UnboundedSender<CmdEnvelope>);

impl PomodoroHandle {
    /// 投递后立即返回（内部生产者：全屏跟随、托盘/快捷键切换）。
    pub fn send(&self, cmd: PomodoroCmd) -> AppResult<()> {
        self.0
            .send(CmdEnvelope { cmd, ack: None })
            .map_err(|_| crate::error::AppError::Platform("番茄钟调度器已停止".into()))
    }

    /// 投递并等待状态机处理完成，返回处理后的最新快照。
    /// 这是写命令的契约返回值（前端 StatePayload）：保证响应时快照已含本次命令效果。
    pub async fn send_sync(&self, cmd: PomodoroCmd) -> AppResult<PomoSnapshot> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0
            .send(CmdEnvelope { cmd, ack: Some(tx) })
            .map_err(|_| crate::error::AppError::Platform("番茄钟调度器已停止".into()))?;
        rx.await
            .map_err(|_| crate::error::AppError::Platform("番茄钟调度器已停止".into()))
    }
}

/// 一步式启动：创建通道并拉起状态机循环（lib.rs 在 manage AppState 之前调用，
/// 返回的 handle 存入 AppState.pomodoro）。
pub fn spawn(app: AppHandle) -> PomodoroHandle {
    let (tx, rx) = unbounded_channel::<CmdEnvelope>();
    tauri::async_runtime::spawn(loop_task(app, rx));
    PomodoroHandle(tx)
}

/// 命令层薄封装：经 AppState 里的 handle 投递命令（投递即返回，不等处理）。
pub fn send_cmd(app: &AppHandle, cmd: PomodoroCmd) -> AppResult<()> {
    app.state::<AppState>().pomodoro.send(cmd)
}

/// 命令层薄封装（同步投递）：等状态机处理完该命令，返回处理后的最新快照。
/// 供 pomodoro_* 写命令作为契约返回值（前端 StatePayload，见 types/index.ts）。
pub async fn send_sync(app: &AppHandle, cmd: PomodoroCmd) -> AppResult<PomoSnapshot> {
    let handle = app.state::<AppState>().pomodoro.clone();
    handle.send_sync(cmd).await
}

/// 私密防线：便签切换为私密时即时脱敏——清空运行快照中的任务文本（并重播事件），
/// 同时清空会话表 running 行的任务文本快照（SQL 在 DAO `clear_task_text_for_note`）。
pub fn scrub_private_text(app: &AppHandle, note_id: &str) {
    // 1) 运行快照：匹配便签则清空任务文本并重播事件
    if let Ok(mut g) = snapshot_cell().lock() {
        if g.note_id == note_id && !g.task_text.is_empty() {
            g.task_text = String::new();
            let _ = app.emit(
                STATE_EVENT,
                serde_json::to_value(&*g).unwrap_or_else(|_| json!({})),
            );
        }
    }
    // 2) 会话表：running 状态的任务文本快照清空（SQL 收敛进 DAO；失败必须留痕——
    //    这是私密防线的一环，静默失败会在会话表残留私密任务文本）
    use tauri::Manager;
    let state = app.state::<AppState>();
    if let Err(e) = state
        .db
        .with(|c| pomodoro_sessions::clear_task_text_for_note(c, note_id))
    {
        log::error!("私密脱敏：清理便签 {note_id} 的会话任务文本失败: {e}");
    }
    // 3) 状态机：清空运行会话持有的明文（不清则 Pause/Resume/auto_next 会把明文
    //    重新广播进事件、重新写入续阶段会话行）
    if let Err(e) = send_cmd(
        app,
        PomodoroCmd::ScrubText {
            note_id: note_id.to_string(),
            task_key: String::new(),
        },
    ) {
        log::error!("私密脱敏：便签 {note_id} 状态机文本清洗失败: {e}");
    }
}

/// 私密防线（今日任务）：任务转私密时即时脱敏——
/// 运行快照清文本（重播）、会话表该任务全部会话清文本快照、状态机内文本清空。
pub fn scrub_daily_task_text(app: &AppHandle, daily_id: &str) {
    let task_key = format!("daily:{daily_id}");
    if let Ok(mut g) = snapshot_cell().lock() {
        if g.task_key == task_key && !g.task_text.is_empty() {
            g.task_text = String::new();
            let _ = app.emit(
                STATE_EVENT,
                serde_json::to_value(&*g).unwrap_or_else(|_| json!({})),
            );
        }
    }
    use tauri::Manager;
    let state = app.state::<AppState>();
    if let Err(e) = state
        .db
        .with(|c| pomodoro_sessions::clear_task_text_for_daily(c, daily_id))
    {
        log::error!("私密脱敏：清理今日任务 {daily_id} 的会话任务文本失败: {e}");
    }
    if let Err(e) = send_cmd(
        app,
        PomodoroCmd::ScrubText {
            note_id: String::new(),
            task_key,
        },
    ) {
        log::error!("私密脱敏：今日任务 {daily_id} 状态机文本清洗失败: {e}");
    }
}

pub fn toggle_via_cmd(app: &AppHandle) -> AppResult<()> {
    send_cmd(app, PomodoroCmd::Toggle)
}

// ---------- 运行中快照（模块级共享，供 pomodoro_state 命令读取） ----------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomoSnapshot {
    pub running: bool,
    pub phase: String,
    pub ends_at: Option<String>,
    pub paused: bool,
    pub remaining_sec: i64,
    pub note_id: String,
    pub task_key: String,
    pub task_text: String,
    pub completed_in_cycle: u32,
}

impl PomoSnapshot {
    pub fn idle() -> Self {
        PomoSnapshot {
            running: false,
            phase: "idle".into(),
            ends_at: None,
            paused: false,
            remaining_sec: 0,
            note_id: String::new(),
            task_key: String::new(),
            task_text: String::new(),
            completed_in_cycle: 0,
        }
    }
}

static SNAPSHOT: OnceLock<Mutex<PomoSnapshot>> = OnceLock::new();

fn snapshot_cell() -> &'static Mutex<PomoSnapshot> {
    SNAPSHOT.get_or_init(|| Mutex::new(PomoSnapshot::idle()))
}

/// 当前番茄钟快照（pomodoro_state 命令；调度器未启动时返回 Idle）。
pub fn current_snapshot() -> PomoSnapshot {
    snapshot_cell()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| PomoSnapshot::idle())
}

/// 从运行中快照刷新托盘 tooltip（reminder.rs 的 60s 心跳复用）：
/// 系统睡眠唤醒后仅显示用的时间可能失真，按墙钟重算剩余。
/// 非 Running / ends_at 解析失败时不动托盘，避免覆盖 Idle 的 "Focusly" 静态文案；
/// 纯显示用途，阶段结束判定仍由番茄钟自身事件循环按 UTC 绝对时间执行。
pub fn refresh_tooltip_from_snapshot(app: &AppHandle) {
    let snap = current_snapshot();
    if !snap.running {
        return;
    }
    let Some(raw) = snap.ends_at.as_deref() else {
        return;
    };
    let Ok(ends_at) = crate::reminder::parse(raw) else {
        return;
    };
    let now = Utc::now();
    let pause = if snap.paused {
        Some((now, snap.remaining_sec))
    } else {
        None
    };
    let text = tooltip_text_for(
        Phase::parse(&snap.phase),
        ends_at,
        pause,
        &snap.task_text,
        now,
    );
    crate::tray::set_tooltip(app, &text);
}

// ---------- 设置（settings 表 → 强类型） ----------

#[derive(Debug, Clone, PartialEq)]
pub struct PomoSettings {
    pub focus_min: i64,
    pub short_min: i64,
    pub long_min: i64,
    pub long_every: u32,
    pub auto_next: bool,
    pub force_remind: bool,
    /// 提示音（MVP 占位：仅 off；保留字段供后续接线）
    #[allow(dead_code)]
    pub sound: String,
}

/// 从 settings 读 i64：非法/缺失回退 default，并夹到 [min, 10_000]。
pub fn parse_setting_i64(map: &HashMap<String, String>, key: &str, default: i64, min: i64) -> i64 {
    map.get(key)
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(min, 10_000))
        .unwrap_or(default)
}

pub fn parse_setting_bool(map: &HashMap<String, String>, key: &str, default: bool) -> bool {
    match map.get(key).map(String::as_str) {
        Some("true") => true,
        Some("false") => false,
        _ => default,
    }
}

/// settings map → 番茄钟设置（键名见 migrations v5 默认值）。
pub fn settings_from(map: &HashMap<String, String>) -> PomoSettings {
    PomoSettings {
        focus_min: parse_setting_i64(map, "pomo_focus_min", 25, 1),
        short_min: parse_setting_i64(map, "pomo_short_min", 5, 1),
        long_min: parse_setting_i64(map, "pomo_long_min", 15, 1),
        long_every: parse_setting_i64(map, "pomo_long_every", 4, 1).max(1) as u32,
        auto_next: parse_setting_bool(map, "pomo_auto_next", false),
        force_remind: parse_setting_bool(map, "pomo_force_remind", false),
        sound: map
            .get("pomo_sound")
            .cloned()
            .unwrap_or_else(|| "off".into()),
    }
}

impl PomoSettings {
    /// 阶段时长（秒）。
    pub fn phase_duration(&self, phase: Phase) -> i64 {
        let min = match phase {
            Phase::Focus => self.focus_min,
            Phase::ShortBreak => self.short_min,
            Phase::LongBreak => self.long_min,
        };
        min.max(1) * 60
    }
}

// ---------- 纯函数（状态机判定） ----------

/// 秒 → "mm:ss"（分钟可超两位，如 60:00；负数按 0）。
pub fn fmt_mmss(total_sec: i64) -> String {
    let s = total_sec.max(0);
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// 完成 completed_focus_in_cycle 个焦点（含本次）后的休息类型；
/// long_every <= 0 视为 1 防御性处理（不 panic）。
pub fn break_after(completed_focus_in_cycle: u32, long_every: u32) -> Phase {
    if long_every > 0 && completed_focus_in_cycle.is_multiple_of(long_every) {
        Phase::LongBreak
    } else {
        Phase::ShortBreak
    }
}

/// 某阶段结束后的下一阶段：焦点→休息（按完成计数决定长/短），休息→焦点。
/// completed_focus_in_cycle 为"本周期内已完成的焦点数（不含刚结束的这次）"。
pub fn next_phase(current: Phase, completed_focus_in_cycle: u32, long_every: u32) -> Phase {
    match current {
        Phase::Focus => break_after(completed_focus_in_cycle + 1, long_every),
        _ => Phase::Focus,
    }
}

/// 阶段结束后是否应自动续接下一阶段（纯判定）。
pub fn auto_next_decision(
    auto_next: bool,
    finished: Phase,
    completed_focus_in_cycle: u32,
    long_every: u32,
) -> Option<Phase> {
    if auto_next {
        Some(next_phase(finished, completed_focus_in_cycle, long_every))
    } else {
        None
    }
}

/// 已运行秒数：未暂停 = planned - 剩余；暂停中 = planned - 暂停时剩余；
/// 结果夹到 [0, planned]（系统休眠导致的越界不会写出负数/超额）。
pub fn elapsed_sec(
    planned_sec: i64,
    ends_at: DateTime<Utc>,
    pause: &Option<(DateTime<Utc>, i64)>,
    now: DateTime<Utc>,
) -> i64 {
    let remaining = match pause {
        Some((_, rem)) => *rem,
        None => (ends_at - now).num_seconds().max(0),
    };
    (planned_sec - remaining).clamp(0, planned_sec.max(0))
}

/// 阶段结束通知文案（未脱敏原文；脱敏由 privacy::notification_text 统一处理）。
pub fn notice_text(phase: Phase, task_text: &str) -> (String, String) {
    match phase {
        Phase::Focus => {
            let t = truncate_chars(task_text.trim(), 60);
            let body = if t.is_empty() {
                "专注完成，休息一下吧".to_string()
            } else {
                format!("完成：{t}")
            };
            ("🍅 专注完成".to_string(), body)
        }
        _ => ("☕ 休息结束".to_string(), "开始下一个专注".to_string()),
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.push('…');
    }
    out
}

// ---------- 状态机 ----------

struct RunningState {
    session_id: String,
    note_id: String,
    task_key: String,
    task_text: String,
    phase: Phase,
    planned_sec: i64,
    ends_at: DateTime<Utc>,
    /// Some((暂停时刻, 剩余秒)) = 暂停中
    pause: Option<(DateTime<Utc>, i64)>,
    completed_in_cycle: u32,
}

enum Machine {
    Idle,
    Running(Box<RunningState>),
}

/// 全屏提示位（FullscreenChanged 命令写入；通知延迟判定时与实时探测取或）。
static FULLSCREEN_HINT: AtomicBool = AtomicBool::new(false);

/// 全屏状态 watch：FullscreenChanged 命令写入，是通知延迟等待的主路径——
/// 等待方收到事件立即唤醒复查（window/foreground.rs 变化即投递），
/// 替代旧的纯 2s 轮询（轮询保留为兜底）。
static FULLSCREEN_WATCH: OnceLock<watch::Sender<bool>> = OnceLock::new();

/// 订阅全屏状态变化（spawn_notice 的等待任务用）。首次订阅时惰性初始化通道。
fn fullscreen_watch() -> watch::Receiver<bool> {
    FULLSCREEN_WATCH
        .get_or_init(|| watch::channel(false).0)
        .subscribe()
}

fn snapshot_of(st: &Machine, now: DateTime<Utc>) -> PomoSnapshot {
    match st {
        Machine::Idle => PomoSnapshot::idle(),
        Machine::Running(r) => {
            let (paused, remaining) = match &r.pause {
                Some((_, rem)) => (true, *rem),
                None => (false, (r.ends_at - now).num_seconds().max(0)),
            };
            PomoSnapshot {
                running: true,
                phase: r.phase.as_str().into(),
                ends_at: Some(crate::reminder::fmt(r.ends_at)),
                paused,
                remaining_sec: remaining,
                note_id: r.note_id.clone(),
                task_key: r.task_key.clone(),
                task_text: r.task_text.clone(),
                completed_in_cycle: r.completed_in_cycle,
            }
        }
    }
}

/// 托盘 tooltip 文案（纯函数）：运行中 → "🍅/☕ mm:ss 任务"；暂停 → "⏸ 剩余 mm:ss"。
pub fn tooltip_text_for(
    phase: Phase,
    ends_at: DateTime<Utc>,
    pause: Option<(DateTime<Utc>, i64)>,
    task_text: &str,
    now: DateTime<Utc>,
) -> String {
    let remaining = match &pause {
        Some((_, s)) => *s,
        None => (ends_at - now).num_seconds().max(0),
    };
    if pause.is_some() {
        format!("⏸ 剩余 {}", fmt_mmss(remaining))
    } else {
        let icon = if phase == Phase::Focus { "🍅" } else { "☕" };
        let t = truncate_chars(task_text.trim(), 16);
        if t.is_empty() {
            format!("{icon} {}", fmt_mmss(remaining))
        } else {
            format!("{icon} {} {t}", fmt_mmss(remaining))
        }
    }
}

fn tooltip_text(st: &Machine, now: DateTime<Utc>) -> String {
    match st {
        Machine::Idle => "Focusly".to_string(),
        Machine::Running(r) => tooltip_text_for(r.phase, r.ends_at, r.pause, &r.task_text, now),
    }
}

fn publish(app: &AppHandle, st: &Machine) {
    let snap = snapshot_of(st, Utc::now());
    if let Ok(mut g) = snapshot_cell().lock() {
        *g = snap.clone();
    }
    let _ = app.emit(
        STATE_EVENT,
        serde_json::to_value(&snap).unwrap_or_else(|_| json!({})),
    );
}

fn update_tooltip(app: &AppHandle, st: &Machine) {
    crate::tray::set_tooltip(app, &tooltip_text(st, Utc::now()));
}

fn read_settings(app: &AppHandle) -> PomoSettings {
    let map = {
        let state = app.state::<AppState>();
        state
            .db
            .with(crate::db::settings::get_all)
            .unwrap_or_default()
    };
    settings_from(&map)
}

/// 便签是否私密（读 notes.is_private；note_id 为空/便签不存在 → 非私密）。
fn note_is_private(app: &AppHandle, note_id: &str) -> bool {
    if note_id.is_empty() {
        return false;
    }
    let state = app.state::<AppState>();
    state
        .db
        .with(|c| crate::db::notes::get(c, note_id))
        .map(|n| n.is_private)
        .unwrap_or(false)
}

/// 今日任务绑定（task_key="daily:<id>"）是否私密；非 daily 绑定恒 false。
fn daily_task_is_private(app: &AppHandle, task_key: &str) -> bool {
    let Some(id) = pomodoro_sessions::daily_task_id_of(task_key) else {
        return false;
    };
    let state = app.state::<AppState>();
    state
        .db
        .with(|c| crate::db::daily_tasks::get(c, id))
        .map(|t| t.is_private)
        .unwrap_or(false)
}

/// 任务文本入链前的统一脱敏：私密便签 / 私密今日任务 → 空串。
/// 在 begin_phase 单点生效（落库快照 + 状态机 + 事件 + 通知全部继承），
/// 同时覆盖 Start 与 auto_next 续阶段——中途转私后新阶段不会再把明文写回。
fn effective_task_text(app: &AppHandle, note_id: &str, task_key: &str, text: &str) -> String {
    if note_is_private(app, note_id) || daily_task_is_private(app, task_key) {
        String::new()
    } else {
        text.to_string()
    }
}

async fn loop_task(app: AppHandle, mut rx: UnboundedReceiver<CmdEnvelope>) {
    log::info!("番茄钟调度器已启动");
    let mut st = Machine::Idle;
    publish(&app, &st);
    loop {
        // 仅 Running 且非暂停时睡到阶段结束；其余情况挂起等命令
        let deadline = match &st {
            Machine::Running(r) if r.pause.is_none() => Some(r.ends_at),
            _ => None,
        };
        let waiter = async {
            match deadline {
                Some(t) => {
                    // 心跳上限 60s：既驱动托盘 tooltip 刷新，也把系统睡眠/
                    // 时钟跳变后的调度误差限制在 60s 内（唤醒后立即按 UTC 重算剩余）
                    loop {
                        let remaining = (t - Utc::now()).num_seconds().max(0) as u64;
                        let step = remaining.min(60);
                        if step == 0 {
                            return;
                        }
                        tokio::time::sleep_until(
                            tokio::time::Instant::now() + Duration::from_secs(step),
                        )
                        .await;
                        if Utc::now() >= t {
                            return;
                        }
                        // 心跳：刷新托盘 tooltip（仅显示用途，阶段判定仍以绝对时间为准）
                        crate::tray::set_tooltip(&app, &tooltip_text(&st, Utc::now()));
                    }
                }
                None => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            env = rx.recv() => match env {
                Some(env) => {
                    handle_cmd(&app, &mut st, env.cmd);
                    // 写命令同步回执：此刻 SNAPSHOT 已含本次命令效果
                    //（状态变化分支内部都会 publish；无状态变化分支返回当前快照同样正确）
                    if let Some(ack) = env.ack {
                        let _ = ack.send(current_snapshot());
                    }
                }
                None => return, // 所有发送端已释放
            },
            _ = waiter => {
                if matches!(st, Machine::Running(ref r) if r.pause.is_none()) {
                    phase_end(&app, &mut st);
                }
            }
        }
    }
}

fn handle_cmd(app: &AppHandle, st: &mut Machine, cmd: PomodoroCmd) {
    match cmd {
        PomodoroCmd::Start {
            note_id,
            task_key,
            task_text,
        } => {
            // 已有运行中会话：先按切换任务中断
            if matches!(st, Machine::Running(_)) {
                stop_running(app, st, "switch_task");
            }
            // 私密脱敏在 begin_phase 单点生效（覆盖 Start / auto_next 两条入口）
            let settings = read_settings(app);
            begin_phase(
                app,
                st,
                Phase::Focus,
                note_id,
                task_key,
                task_text,
                0,
                &settings,
            );
        }
        PomodoroCmd::Pause => {
            if let Machine::Running(r) = st {
                if r.pause.is_none() {
                    let now = Utc::now();
                    let remaining = (r.ends_at - now).num_seconds().max(0);
                    r.pause = Some((now, remaining));
                    publish(app, st);
                    update_tooltip(app, st);
                }
            }
        }
        PomodoroCmd::Resume => {
            if let Machine::Running(r) = st {
                if let Some((_, remaining)) = r.pause.take() {
                    r.ends_at = Utc::now() + chrono::Duration::seconds(remaining);
                    publish(app, st);
                    update_tooltip(app, st);
                }
            }
        }
        PomodoroCmd::Skip => {
            if matches!(st, Machine::Running(_)) {
                finish_interrupted_and_advance(app, st, "skip");
            }
        }
        PomodoroCmd::Stop { reason } => {
            if matches!(st, Machine::Running(_)) {
                stop_running(app, st, &reason);
            }
        }
        PomodoroCmd::StopIfTask { task_key, reason } => {
            if let Machine::Running(r) = st {
                if r.task_key == task_key {
                    stop_running(app, st, &reason);
                }
            }
        }
        PomodoroCmd::ScrubText { note_id, task_key } => {
            if let Machine::Running(r) = st {
                let hit = (!note_id.is_empty() && r.note_id == note_id)
                    || (!task_key.is_empty() && r.task_key == task_key);
                if hit && !r.task_text.is_empty() {
                    r.task_text.clear();
                    publish(app, st);
                    update_tooltip(app, st);
                }
            }
        }
        PomodoroCmd::AddMinutes(mins) => {
            if let Machine::Running(r) = st {
                if r.pause.is_none() && mins > 0 {
                    // 边界防御：钳到 10_000 分钟（约 7 天，与设置解析的 10_000 上限同源），
                    // 防 ends_at/planned_sec 算术溢出 panic（chrono 加法溢出会 panic）
                    let mins = mins.min(10_000);
                    r.ends_at += chrono::Duration::minutes(mins);
                    r.planned_sec += mins * 60;
                    publish(app, st);
                    update_tooltip(app, st);
                }
            }
        }
        PomodoroCmd::CompleteTask => {
            if let Machine::Running(r) = st {
                let (nid, tk) = (r.note_id.clone(), r.task_key.clone());
                if !nid.is_empty() && !tk.is_empty() {
                    if let Err(e) = complete_task(app, &nid, &tk) {
                        log::warn!("番茄钟 CompleteTask 失败: {e}");
                    }
                }
            }
        }
        PomodoroCmd::FullscreenChanged(v) => {
            FULLSCREEN_HINT.store(v, Ordering::Relaxed);
            // 事件唤醒通知延迟等待（主路径）。通道由等待方首次订阅时惰性初始化，
            // 尚无订阅者时跳过——条件里的提示位 + 实时探测 + 2s 兜底轮询保证不丢唤醒。
            if let Some(tx) = FULLSCREEN_WATCH.get() {
                let _ = tx.send(v);
            }
        }
        PomodoroCmd::ForceWake => publish(app, st),
        PomodoroCmd::Toggle => toggle_cmd(app, st),
    }
}

fn toggle_cmd(app: &AppHandle, st: &mut Machine) {
    match st {
        Machine::Idle => handle_cmd(
            app,
            st,
            PomodoroCmd::Start {
                note_id: String::new(),
                task_key: String::new(),
                task_text: "专注".into(),
            },
        ),
        Machine::Running(_) => {
            let paused = matches!(st, Machine::Running(r) if r.pause.is_some());
            let next = if paused {
                PomodoroCmd::Resume
            } else {
                PomodoroCmd::Pause
            };
            handle_cmd(app, st, next);
        }
    }
}

/// 阶段计划时长：Focus 且绑定任务时用任务的 focus_min 覆盖（1..=180 分钟夹紧），
/// 未覆盖/休息阶段用全局设置。这样"每个待办可自定义专注时长，默认跟随全局 25 分钟"。
fn planned_for(
    app: &AppHandle,
    phase: Phase,
    note_id: &str,
    task_key: &str,
    settings: &PomoSettings,
) -> i64 {
    if phase != Phase::Focus || task_key.is_empty() {
        return settings.phase_duration(phase);
    }
    let state = app.state::<AppState>();
    let override_min = if let Some(did) = pomodoro_sessions::daily_task_id_of(task_key) {
        state
            .db
            .with(|c| crate::db::daily_tasks::get(c, did))
            .ok()
            .and_then(|t| t.focus_min)
    } else if !note_id.is_empty() {
        state
            .db
            .with(|c| crate::db::task_meta::get(c, note_id, task_key))
            .ok()
            .flatten()
            .and_then(|m| m.focus_min)
    } else {
        None
    };
    override_min
        .map(|m| m.clamp(1, 180).max(1) * 60)
        .unwrap_or_else(|| settings.phase_duration(phase))
}

/// 开始一个阶段：写 running 会话、更新状态机、广播、托盘。
#[allow(clippy::too_many_arguments)]
fn begin_phase(
    app: &AppHandle,
    st: &mut Machine,
    phase: Phase,
    note_id: String,
    task_key: String,
    task_text: String,
    completed_in_cycle: u32,
    settings: &PomoSettings,
) {
    // 私密脱敏单点：落库快照 / 状态机 / 事件 / 通知均从这里继承已脱敏文本
    let task_text = effective_task_text(app, &note_id, &task_key, &task_text);
    let planned = planned_for(app, phase, &note_id, &task_key, settings);
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now();
    let ends_at = now + chrono::Duration::seconds(planned);
    {
        let state = app.state::<AppState>();
        let r = state.db.with(|c| {
            pomodoro_sessions::insert_running(
                c,
                &id,
                &note_id,
                &task_key,
                &task_text,
                phase.as_str(),
                planned,
            )
        });
        if let Err(e) = r {
            log::error!("番茄钟会话写入失败: {e}");
        }
    }
    *st = Machine::Running(Box::new(RunningState {
        session_id: id,
        note_id,
        task_key,
        task_text,
        phase,
        planned_sec: planned,
        ends_at,
        pause: None,
        completed_in_cycle,
    }));
    log::info!("番茄钟阶段开始: {} {}s", phase.as_str(), planned);
    publish(app, st);
    update_tooltip(app, st);
}

/// 阶段自然到点：finish(completed) → 通知（脱敏+勿扰+全屏延迟，独立任务）→ auto_next 推进。
fn phase_end(app: &AppHandle, st: &mut Machine) {
    let settings = read_settings(app);
    let (finished, r) = match st {
        Machine::Running(r) => (r.phase, r),
        Machine::Idle => return,
    };
    let now = Utc::now();
    {
        let state = app.state::<AppState>();
        if let Err(e) = state.db.with(|c| {
            pomodoro_sessions::finish(
                c,
                &r.session_id,
                "completed",
                r.planned_sec,
                None,
                &crate::reminder::fmt(now),
            )
        }) {
            log::error!("番茄钟会话完结失败: {e}");
        }
    }

    let is_private = note_is_private(app, &r.note_id);
    // 焦点阶段自然完成：本周期完成数 +1（skip 不加，见 finish_interrupted_and_advance）
    let completed_before = r.completed_in_cycle;
    if finished == Phase::Focus {
        r.completed_in_cycle += 1;
        let state = app.state::<AppState>();
        // 今日任务绑定（task_key = "daily:<id>"）：只回写任务完成番茄数
        match pomodoro_sessions::daily_task_id_of(&r.task_key) {
            Some(daily_id) => {
                if let Err(e) = state
                    .db
                    .with(|c| crate::db::daily_tasks::incr_completed(c, daily_id))
                {
                    log::warn!("今日任务 completed_pomodoros 累加失败: {e}");
                }
            }
            None => {
                if !r.note_id.is_empty() && !r.task_key.is_empty() {
                    if let Err(e) = state
                        .db
                        .with(|c| crate::db::task_meta::incr_completed(c, &r.note_id, &r.task_key))
                    {
                        log::warn!("番茄钟 completed_pomodoros 累加失败: {e}");
                    }
                }
            }
        }
    }

    // next_phase 内部对 Focus 做 +1：传入增量前的计数，结果等价于按"含本次"的新计数选长/短休
    let next = next_phase(finished, completed_before, settings.long_every);
    let _ = app.emit(
        FINISHED_EVENT,
        json!({
            "phase": finished.as_str(),
            "nextPhase": next.as_str(),
            "noteId": r.note_id,
            "taskKey": r.task_key,
            // 私密便签 taskText 下发空串
            "taskText": if is_private { "" } else { r.task_text.as_str() },
        }),
    );

    // 通知在独立任务里做全屏/勿扰延迟，不阻塞状态机推进
    let (title, body) = notice_text(finished, &r.task_text);
    spawn_notice(
        app.clone(),
        title,
        body,
        r.note_id.clone(),
        settings.force_remind,
        true,
    );

    let completed = r.completed_in_cycle;
    let (note_id, task_key, task_text) =
        (r.note_id.clone(), r.task_key.clone(), r.task_text.clone());
    match auto_next_decision(
        settings.auto_next,
        finished,
        completed_before,
        settings.long_every,
    ) {
        Some(next_phase_to_start) => {
            begin_phase(
                app,
                st,
                next_phase_to_start,
                note_id,
                task_key,
                task_text,
                completed,
                &settings,
            );
        }
        None => {
            *st = Machine::Idle;
            publish(app, st);
            crate::tray::set_tooltip(app, "Focusly");
        }
    }
}

/// Skip：finish(interrupted, "skip")，按已运行计 actual_sec；不补发通知、不累计完成数。
fn finish_interrupted_and_advance(app: &AppHandle, st: &mut Machine, reason: &str) {
    let settings = read_settings(app);
    let (finished, r) = match st {
        Machine::Running(r) => (r.phase, r),
        Machine::Idle => return,
    };
    let now = Utc::now();
    let actual = elapsed_sec(r.planned_sec, r.ends_at, &r.pause, now);
    {
        let state = app.state::<AppState>();
        if let Err(e) = state.db.with(|c| {
            pomodoro_sessions::finish(
                c,
                &r.session_id,
                "interrupted",
                actual,
                Some(reason),
                &crate::reminder::fmt(now),
            )
        }) {
            log::error!("番茄钟会话中断落库失败: {e}");
        }
    }
    let next = next_phase(finished, r.completed_in_cycle, settings.long_every);
    let is_private = note_is_private(app, &r.note_id);
    let _ = app.emit(
        FINISHED_EVENT,
        json!({
            "phase": finished.as_str(),
            "nextPhase": next.as_str(),
            "noteId": r.note_id,
            "taskKey": r.task_key,
            "taskText": if is_private { "" } else { r.task_text.as_str() },
        }),
    );
    log::info!(
        "番茄钟阶段跳过: {} reason={reason} actual={actual}s",
        finished.as_str()
    );

    let completed = r.completed_in_cycle;
    let (note_id, task_key, task_text) =
        (r.note_id.clone(), r.task_key.clone(), r.task_text.clone());
    match auto_next_decision(settings.auto_next, finished, completed, settings.long_every) {
        Some(next_phase_to_start) => {
            begin_phase(
                app,
                st,
                next_phase_to_start,
                note_id,
                task_key,
                task_text,
                completed,
                &settings,
            );
        }
        None => {
            *st = Machine::Idle;
            publish(app, st);
            crate::tray::set_tooltip(app, "Focusly");
        }
    }
}

/// Stop：finish(interrupted, reason) 并回 Idle（不推进、不通知）。
fn stop_running(app: &AppHandle, st: &mut Machine, reason: &str) {
    let r = match st {
        Machine::Running(r) => r,
        Machine::Idle => return,
    };
    let now = Utc::now();
    let actual = elapsed_sec(r.planned_sec, r.ends_at, &r.pause, now);
    let is_private = note_is_private(app, &r.note_id);
    {
        let state = app.state::<AppState>();
        if let Err(e) = state.db.with(|c| {
            pomodoro_sessions::finish(
                c,
                &r.session_id,
                "interrupted",
                actual,
                Some(reason),
                &crate::reminder::fmt(now),
            )
        }) {
            log::error!("番茄钟会话停止落库失败: {e}");
        }
    }
    let _ = app.emit(
        FINISHED_EVENT,
        json!({
            "phase": r.phase.as_str(),
            "nextPhase": serde_json::Value::Null,
            "noteId": r.note_id,
            "taskKey": r.task_key,
            "taskText": if is_private { "" } else { r.task_text.as_str() },
        }),
    );
    log::info!("番茄钟已停止: reason={reason} actual={actual}s");
    *st = Machine::Idle;
    publish(app, st);
    crate::tray::set_tooltip(app, "Focusly");
}

// ---------- 通知（全屏延迟 + 勿扰 + 隐私脱敏） ----------

/// 独立任务：全屏时等待（主路径事件驱动 + 2s 兜底轮询，硬顶 4 小时）→
/// 勿扰时推迟到窗口结束 → 复查私密标志脱敏 → 发系统通知。
fn spawn_notice(
    app: AppHandle,
    title: String,
    body: String,
    note_id: String,
    force_remind: bool,
    wait_fullscreen: bool,
) {
    tauri::async_runtime::spawn(async move {
        if wait_fullscreen {
            // 主路径：FullscreenChanged 事件（源自 window/foreground.rs 检测线程，
            // 经命令通道 → FULLSCREEN_WATCH）即时唤醒复查，通知延迟从最坏 ~2s 降到毫秒级；
            // 兜底：2s 超时醒来照常复查（事件丢失/启动即全屏时仍能前进），
            // 4 小时硬顶防通知永久滞留。
            let mut rx = fullscreen_watch();
            let wait_started = std::time::Instant::now();
            while (FULLSCREEN_HINT.load(Ordering::Relaxed)
                || crate::window::monitor::is_foreground_fullscreen())
                && wait_started.elapsed() < Duration::from_secs(MAX_FULLSCREEN_WAIT_SECS)
            {
                let _ =
                    tokio::time::timeout(Duration::from_secs(FULLSCREEN_POLL_SECS), rx.changed())
                        .await;
            }
        }
        if !force_remind {
            let settings = {
                let state = app.state::<AppState>();
                state
                    .db
                    .with(crate::db::settings::get_all)
                    .unwrap_or_default()
            };
            let now_local = chrono::Local::now().time();
            if !crate::dnd::should_notify(&settings, now_local) {
                if let Some(w) = crate::dnd::parse_window(
                    settings.get("dnd_start").map(String::as_str).unwrap_or(""),
                    settings.get("dnd_end").map(String::as_str).unwrap_or(""),
                ) {
                    let exit = crate::dnd::next_exit_utc(Utc::now(), &w);
                    let dur = (exit - Utc::now())
                        .to_std()
                        .unwrap_or(Duration::from_secs(1));
                    log::info!("番茄钟通知处于勿扰时段，推迟到 {exit}");
                    tokio::time::sleep(dur).await;
                }
            }
        }
        let is_private = note_is_private(&app, &note_id);
        let (title, body) = crate::privacy::notification_text(&title, &body, is_private);
        if let Err(e) = app
            .notification()
            .builder()
            .title(&title)
            .body(&body)
            .show()
        {
            log::error!("番茄钟通知发送失败: {e}");
        }
    });
}

// ---------- 启动恢复 ----------

/// 应用启动时调用（lib.rs manage 之后）：把上次运行遗留的 running 会话
/// 标记为 interrupted（reason="app_exit"）；其绝对结束时间（started_at+planned_sec）
/// 在 24h 内则补一条阶段结束通知（脱敏+勿扰，超 24h 忽略——错过汇总已覆盖）。
pub fn recover(app: &AppHandle) {
    let state = app.state::<AppState>();
    let running = state.db.with(pomodoro_sessions::get_running).ok().flatten();
    let Some(s) = running else { return };
    let now = Utc::now();
    let started = crate::reminder::parse(&s.started_at).unwrap_or(now);
    let actual = (now - started).num_seconds().clamp(0, s.planned_sec.max(0));
    if let Err(e) = state.db.with(|c| {
        pomodoro_sessions::finish(
            c,
            &s.id,
            "interrupted",
            actual,
            Some("app_exit"),
            &crate::reminder::fmt(now),
        )
    }) {
        log::error!("番茄钟启动恢复：遗留会话 {} 标记失败: {e}", s.id);
        return;
    }
    log::info!(
        "番茄钟启动恢复：遗留会话 {} 已标记 interrupted（actual={actual}s）",
        s.id
    );

    let ends_at = started + chrono::Duration::seconds(s.planned_sec);
    if now - ends_at <= chrono::Duration::hours(RECOVER_CUTOFF_HOURS) {
        let settings = read_settings(app);
        let (title, body) = notice_text(
            Phase::parse(&s.phase),
            s.task_text_snapshot.as_deref().unwrap_or(""),
        );
        spawn_notice(
            app.clone(),
            title,
            body,
            s.note_id.clone().unwrap_or_default(),
            settings.force_remind,
            false,
        );
    }
}

// ---------- 完成任务（命令层与 CompleteTask 命令共用） ----------

/// 完成任务：task_meta 置 done + 便签正文第一条匹配行勾选回写（标题不变）。
pub fn complete_task(
    app: &AppHandle,
    note_id: &str,
    task_key: &str,
) -> AppResult<crate::db::models::TaskMeta> {
    let state = app.state::<AppState>();
    let meta = state
        .db
        .with(|c| crate::db::task_meta::get(c, note_id, task_key))?
        .ok_or_else(|| {
            crate::error::AppError::Invalid(format!("任务元数据不存在: {note_id}/{task_key}"))
        })?;
    // 正文回写：读 content → 勾选 → update_content（title 传回原值）
    let note = state.db.with(|c| crate::db::notes::get(c, note_id))?;
    if let Some(new_content) =
        crate::db::task_meta::mark_done_content_line(&note.content, &meta.line_text)
    {
        crate::notes::update_content(app, note_id, &note.title, &new_content)?;
    }
    let meta = state.db.with(|c| {
        crate::db::task_meta::upsert(
            c,
            &crate::db::task_meta::TaskMetaUpsert {
                note_id: note_id.to_string(),
                task_key: task_key.to_string(),
                line_text: meta.line_text.clone(),
                status: Some("done".into()),
                estimate: None,
                priority: None,
                due_at: None,
                clear_skip: false,
                focus_min: None,
            },
        )
    })?;
    Ok(meta)
}

// ---------- 统计窗口边界 ----------

/// 本地"今天"的 [起点, 现在]（RFC3339 UTC，stats_today 参数口径）。
pub fn local_today_bounds() -> (String, String) {
    let now = Utc::now();
    let today = chrono::Local::now().date_naive();
    let midnight = today
        .and_hms_opt(0, 0, 0)
        .unwrap_or_else(|| today.and_hms_opt(12, 0, 0).unwrap());
    let start_utc = chrono::Local
        .from_local_datetime(&midnight)
        .single()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or(now - chrono::Duration::hours(24));
    (crate::reminder::fmt(start_utc), crate::reminder::fmt(now))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
    }

    fn settings(mins: (i64, i64, i64), long_every: u32, auto: bool) -> PomoSettings {
        PomoSettings {
            focus_min: mins.0,
            short_min: mins.1,
            long_min: mins.2,
            long_every,
            auto_next: auto,
            force_remind: false,
            sound: "off".into(),
        }
    }

    // ---------- Phase ----------

    #[test]
    fn phase_as_str_parse_roundtrip() {
        for p in [Phase::Focus, Phase::ShortBreak, Phase::LongBreak] {
            assert_eq!(Phase::parse(p.as_str()), p);
        }
        assert_eq!(Phase::Focus.as_str(), "focus");
        assert_eq!(Phase::ShortBreak.as_str(), "short_break");
        assert_eq!(Phase::LongBreak.as_str(), "long_break");
        assert_eq!(Phase::parse("垃圾"), Phase::Focus, "未知值回退 focus");
    }

    #[test]
    fn break_after_long_every_boundary() {
        assert_eq!(break_after(1, 4), Phase::ShortBreak);
        assert_eq!(break_after(3, 4), Phase::ShortBreak);
        assert_eq!(break_after(4, 4), Phase::LongBreak, "每 4 个焦点进长休");
        assert_eq!(break_after(8, 4), Phase::LongBreak);
        assert_eq!(break_after(7, 4), Phase::ShortBreak);
        assert_eq!(
            break_after(1, 1),
            Phase::LongBreak,
            "long_every=1 每次都长休"
        );
        assert_eq!(
            break_after(5, 0),
            Phase::ShortBreak,
            "long_every=0 防御性回退短休"
        );
    }

    #[test]
    fn next_phase_focus_goes_to_break_break_returns_focus() {
        // completed_focus_in_cycle 为"不含本次"的已完成数：本次完成后共 3 个 → 短休
        assert_eq!(next_phase(Phase::Focus, 2, 4), Phase::ShortBreak);
        // 本次完成后共 4 个 → 长休
        assert_eq!(next_phase(Phase::Focus, 3, 4), Phase::LongBreak);
        assert_eq!(next_phase(Phase::Focus, 7, 4), Phase::LongBreak);
        assert_eq!(next_phase(Phase::ShortBreak, 0, 4), Phase::Focus);
        assert_eq!(next_phase(Phase::LongBreak, 4, 4), Phase::Focus);
    }

    #[test]
    fn auto_next_decision_respects_setting() {
        assert_eq!(
            auto_next_decision(true, Phase::Focus, 0, 4),
            Some(Phase::ShortBreak)
        );
        assert_eq!(
            auto_next_decision(true, Phase::ShortBreak, 3, 4),
            Some(Phase::Focus)
        );
        assert_eq!(auto_next_decision(false, Phase::Focus, 0, 4), None);
    }

    // ---------- fmt_mmss ----------

    #[test]
    fn fmt_mmss_basic_and_clamping() {
        assert_eq!(fmt_mmss(0), "00:00");
        assert_eq!(fmt_mmss(65), "01:05");
        assert_eq!(fmt_mmss(1500), "25:00");
        assert_eq!(fmt_mmss(3600), "60:00");
        assert_eq!(fmt_mmss(-5), "00:00", "负数按 0");
    }

    // ---------- 设置解析 ----------

    #[test]
    fn parse_setting_i64_fallback_and_clamp() {
        let mut m: HashMap<String, String> = HashMap::new();
        assert_eq!(parse_setting_i64(&m, "k", 25, 1), 25, "缺失回退默认");
        m.insert("k".into(), "abc".into());
        assert_eq!(parse_setting_i64(&m, "k", 25, 1), 25, "非法回退默认");
        m.insert("k".into(), "50".into());
        assert_eq!(parse_setting_i64(&m, "k", 25, 1), 50);
        m.insert("k".into(), "0".into());
        assert_eq!(parse_setting_i64(&m, "k", 25, 1), 1, "低于下限被夹住");
        m.insert("k".into(), "-9".into());
        assert_eq!(parse_setting_i64(&m, "k", 25, 1), 1);
    }

    #[test]
    fn parse_setting_bool_and_settings_from_map() {
        let mut m: HashMap<String, String> = HashMap::new();
        let s = settings_from(&m);
        assert_eq!(s, settings((25, 5, 15), 4, false), "全部默认值");
        assert_eq!(s.phase_duration(Phase::Focus), 1500);
        assert_eq!(s.phase_duration(Phase::ShortBreak), 300);
        assert_eq!(s.phase_duration(Phase::LongBreak), 900);

        m.insert("pomo_focus_min".into(), "30".into());
        m.insert("pomo_short_min".into(), "7".into());
        m.insert("pomo_long_min".into(), "20".into());
        m.insert("pomo_long_every".into(), "3".into());
        m.insert("pomo_auto_next".into(), "true".into());
        m.insert("pomo_force_remind".into(), "true".into());
        m.insert("pomo_sound".into(), "chime".into());
        let s = settings_from(&m);
        assert_eq!(s.focus_min, 30);
        assert_eq!(s.short_min, 7);
        assert_eq!(s.long_min, 20);
        assert_eq!(s.long_every, 3);
        assert!(s.auto_next);
        assert!(s.force_remind);
        assert_eq!(s.sound, "chime");
        assert_eq!(s.phase_duration(Phase::Focus), 1800);
    }

    // ---------- elapsed_sec ----------

    #[test]
    fn elapsed_sec_running_paused_and_clamped() {
        let start = at(2026, 9, 16, 10, 0, 0);
        let ends = start + chrono::Duration::seconds(1500);
        // 未暂停、跑了 10 分钟
        assert_eq!(
            elapsed_sec(1500, ends, &None, start + chrono::Duration::seconds(600)),
            600
        );
        // 未暂停、已越过 ends_at（系统休眠）：夹到 planned
        assert_eq!(
            elapsed_sec(1500, ends, &None, ends + chrono::Duration::seconds(500)),
            1500
        );
        // 暂停中：用暂停时刻的剩余
        let pause = Some((start + chrono::Duration::seconds(600), 900));
        assert_eq!(
            elapsed_sec(1500, ends, &pause, start + chrono::Duration::seconds(1200)),
            600
        );
        // 时间倒流防御：不为负
        assert_eq!(
            elapsed_sec(1500, ends, &None, start - chrono::Duration::seconds(600)),
            0
        );
    }

    // ---------- 通知与托盘文案 ----------

    #[test]
    fn notice_text_uses_task_text_and_break_copy() {
        let (t, b) = notice_text(Phase::Focus, "写周报");
        assert_eq!(t, "🍅 专注完成");
        assert_eq!(b, "完成：写周报");
        let (t, b) = notice_text(Phase::Focus, "   ");
        assert_eq!(t, "🍅 专注完成");
        assert_eq!(b, "专注完成，休息一下吧", "无任务文本回退通用文案");
        assert!(b.contains("专注完成"));
        let (t, b) = notice_text(Phase::ShortBreak, "无所谓");
        assert_eq!(t, "☕ 休息结束");
        assert_eq!(b, "开始下一个专注");
        let (t, _) = notice_text(Phase::LongBreak, "");
        assert_eq!(t, "☕ 休息结束");
    }

    #[test]
    fn notice_text_truncates_long_task() {
        let long = "字".repeat(100);
        let (_, b) = notice_text(Phase::Focus, &long);
        // "完成：" 3 字 + 60 字任务 + 省略号
        assert_eq!(b.chars().count(), 64);
        assert!(b.ends_with('…'));
    }

    #[test]
    fn tooltip_text_variants() {
        let start = at(2026, 9, 16, 10, 0, 0);
        let ends = start + chrono::Duration::seconds(1500);
        assert_eq!(
            tooltip_text_for(
                Phase::Focus,
                ends,
                None,
                "买牛奶",
                start + chrono::Duration::seconds(60)
            ),
            "🍅 24:00 买牛奶"
        );
        assert_eq!(
            tooltip_text_for(
                Phase::ShortBreak,
                start + chrono::Duration::seconds(300),
                None,
                "",
                start
            ),
            "☕ 05:00",
            "无任务文本省略尾部"
        );
        // 暂停态不看挂钟，用暂停时刻的剩余
        assert_eq!(
            tooltip_text_for(
                Phase::Focus,
                ends,
                Some((start, 1499)),
                "买牛奶",
                start + chrono::Duration::seconds(300)
            ),
            "⏸ 剩余 24:59"
        );
        assert_eq!(tooltip_text(&Machine::Idle, start), "Focusly");
    }

    // ---------- 快照 ----------

    #[test]
    fn snapshot_idle_and_running_shapes() {
        let idle = snapshot_of(&Machine::Idle, Utc::now());
        assert!(!idle.running);
        assert_eq!(idle.phase, "idle");
        assert_eq!(idle.ends_at, None);
        assert_eq!(idle.remaining_sec, 0);

        let start = at(2026, 9, 16, 10, 0, 0);
        let running = Machine::Running(Box::new(RunningState {
            session_id: "s".into(),
            note_id: "n1".into(),
            task_key: "k1".into(),
            task_text: "任务".into(),
            phase: Phase::LongBreak,
            planned_sec: 900,
            ends_at: start + chrono::Duration::seconds(900),
            pause: None,
            completed_in_cycle: 4,
        }));
        let snap = snapshot_of(&running, start + chrono::Duration::seconds(60));
        assert!(snap.running);
        assert_eq!(snap.phase, "long_break");
        assert!(snap.ends_at.is_some());
        assert!(!snap.paused);
        assert_eq!(snap.remaining_sec, 840);
        assert_eq!(snap.completed_in_cycle, 4);
    }

    // ---------- 统计窗口边界 ----------

    #[test]
    fn local_today_bounds_is_ordered_rfc3339() {
        let (start, now) = local_today_bounds();
        let s = crate::reminder::parse(&start).unwrap();
        let n = crate::reminder::parse(&now).unwrap();
        assert!(s <= n);
        assert!(n - s <= chrono::Duration::hours(25));
    }

    // ---------- 边界补缺（可靠性审查追加） ----------

    #[test]
    fn elapsed_after_add_minutes_still_clamped() {
        // AddMinutes 语义：planned_sec 与 ends_at 同加。延时后（含休眠越界）仍按新 planned 夹紧
        let start = at(2026, 9, 16, 10, 0, 0);
        let planned0 = 1500i64;
        let ends0 = start + chrono::Duration::seconds(planned0);
        // +10 分钟：planned/ends 同步增长
        let planned1 = planned0 + 600;
        let ends1 = ends0 + chrono::Duration::seconds(600);
        // 新 planned 下正常进行中：actual = 实际运行秒数（不超新 planned）
        assert_eq!(
            elapsed_sec(
                planned1,
                ends1,
                &None,
                start + chrono::Duration::seconds(600)
            ),
            600
        );
        // 休眠越过加时后的 ends_at：夹到新 planned，不超额
        assert_eq!(
            elapsed_sec(
                planned1,
                ends1,
                &None,
                ends1 + chrono::Duration::seconds(3600)
            ),
            planned1
        );
        // 时钟倒流：不为负
        assert_eq!(elapsed_sec(planned1, ends1, &None, start), 0);
    }

    #[test]
    fn skip_while_paused_actual_uses_pause_remaining() {
        // Skip 在暂停态：actual_sec 用暂停时刻的剩余（不吃挂钟），夹在 [0, planned]
        let start = at(2026, 9, 16, 10, 0, 0);
        let ends = start + chrono::Duration::seconds(1500);
        // 跑 10 分钟后暂停（剩余 900s），暂停期间休眠 2 小时再 Skip：actual 仍 = 600
        let pause = Some((start + chrono::Duration::seconds(600), 900i64));
        let after_sleep = start + chrono::Duration::seconds(600 + 7200);
        assert_eq!(elapsed_sec(1500, ends, &pause, after_sleep), 600);
        // 极端：暂停发生在 ends_at 之后（剩余夹 0）→ actual = planned
        let pause_zero = Some((ends + chrono::Duration::seconds(60), 0i64));
        assert_eq!(elapsed_sec(1500, ends, &pause_zero, after_sleep), 1500);
    }

    #[test]
    fn snapshot_remaining_nonnegative_when_ends_at_passed() {
        // ends_at 已过（心跳唤醒间隙读取快照）：remaining 夹 0，不出负数
        let start = at(2026, 9, 16, 10, 0, 0);
        let running = Machine::Running(Box::new(RunningState {
            session_id: "s".into(),
            note_id: "n1".into(),
            task_key: "k1".into(),
            task_text: String::new(),
            phase: Phase::Focus,
            planned_sec: 1500,
            ends_at: start + chrono::Duration::seconds(1500),
            pause: None,
            completed_in_cycle: 0,
        }));
        let snap = snapshot_of(&running, start + chrono::Duration::seconds(1500 + 90));
        assert_eq!(snap.remaining_sec, 0);
    }
}
