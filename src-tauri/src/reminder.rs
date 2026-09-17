//! 提醒调度器：tokio 事件驱动（无轮询）。
//! 依据最近的 pending 提醒睡眠，到点触发通知；插入/修改/提醒后由 `SchedulerHandle::wake()` 唤醒重算。

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

use crate::db::models::{Reminder, RepeatType};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// 过期兜底：错过超过一天的 pending 提醒直接取消，不轰炸用户。
const STALE_CUTOFF_HOURS: i64 = 24;

/// 睡眠步进上限（秒）。tokio 的 sleep_until 基于单调时钟，系统睡眠期间会停走，
/// 一次睡到目标可能在唤醒后大幅迟到。拆成最多 60 秒的步进，每次唤醒用
/// `Utc::now()`（墙钟）重查是否到期。
/// 注意：60s 心跳仅用于睡眠唤醒补偿和托盘 tooltip 刷新，调度判定（是否触发、
/// 触发哪些）仍以 UTC 绝对时间为准——到期后由 `fire_due` 重查数据库决定。
const HEARTBEAT_SECS: u64 = 60;

/// 全项目 remind_at 时间字符串的统一入口。
pub fn fmt(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// 全项目 remind_at 时间字符串的统一出口。
pub fn parse(s: &str) -> AppResult<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| AppError::Invalid(format!("时间格式无效（应为 RFC3339）: {s} ({e})")))
}

#[derive(Clone)]
pub struct SchedulerHandle(UnboundedSender<()>);

impl SchedulerHandle {
    pub fn wake(&self) {
        // 通道只在调度器 loop 退出（应用收尾）后才会发送失败；该告警说明调度器已不可用
        if self.0.send(()).is_err() {
            log::warn!("提醒调度器唤醒失败：接收端已关闭");
        }
    }
}

/// 创建调度器通道（handle 存入 AppState，loop task 在 manage 之后启动以避免竞态）。
pub fn channel() -> (SchedulerHandle, UnboundedReceiver<()>) {
    let (tx, rx) = unbounded_channel();
    (SchedulerHandle(tx), rx)
}

/// 一步式启动（manage 之后调用，或确认状态已就绪时使用）。
/// lib.rs 当前用 channel()+spawn_loop 以精确控制启动时序，此便捷入口保留备用。
#[allow(dead_code)]
pub fn spawn(app: AppHandle) -> SchedulerHandle {
    let (handle, rx) = channel();
    spawn_loop(app, rx);
    handle
}

pub fn spawn_loop(app: AppHandle, rx: UnboundedReceiver<()>) {
    // 启动时错过提醒汇总（静默失败）
    {
        let state = app.state::<AppState>();
        let summary = state
            .db
            .with(crate::db::missed::count_missed)
            .ok()
            .and_then(crate::dnd::missed_summary_text);
        if let Some(text) = summary {
            use tauri_plugin_notification::NotificationExt;
            if let Err(e) = app
                .notification()
                .builder()
                .title("Focusly")
                .body(&text)
                .show()
            {
                log::warn!("错过提醒汇总通知发送失败: {e}");
            }
        }
    }
    tauri::async_runtime::spawn(loop_task(app, rx));
}

async fn loop_task(app: AppHandle, mut rx: UnboundedReceiver<()>) {
    log::info!("提醒调度器已启动");
    loop {
        match next_pending_time(&app) {
            // 没有待触发提醒：挂起等待唤醒
            None => {
                if rx.recv().await.is_none() {
                    return; // 所有发送端已释放
                }
            }
            Some(t) => {
                // 步进睡眠（每段最多 HEARTBEAT_SECS）+ 唤醒后 UTC 墙钟重查，
                // 兼容系统睡眠/时钟跳变（详见 HEARTBEAT_SECS 注释）。
                loop {
                    let now = Utc::now();
                    if t <= now {
                        fire_due(&app);
                        break;
                    }
                    // t - now > 0（上面刚判过）；num_seconds 向零取整可能为 0，夹到至少 1s
                    let step = (t - now).num_seconds().clamp(1, HEARTBEAT_SECS as i64) as u64;
                    let deadline =
                        tokio::time::Instant::now() + std::time::Duration::from_secs(step);
                    tokio::select! {
                        _ = tokio::time::sleep_until(deadline) => {
                            // 是否真正到期由循环顶部的 UTC 重查判定；
                            // 未到期的纯心跳唤醒顺带刷新托盘 tooltip
                            //（番茄钟剩余时间随墙钟校准，见 pomodoro::refresh_tooltip_from_snapshot）。
                            if Utc::now() < t {
                                crate::pomodoro::refresh_tooltip_from_snapshot(&app);
                            }
                        }
                        _ = rx.recv() => break, // 被唤醒：重新计算最近时间
                    }
                }
            }
        }
    }
}

/// 最近的 pending 提醒时间。逐行解析（Rust 侧 chrono 比较），
/// 解析失败的行记日志并跳过。
fn next_pending_time(app: &AppHandle) -> Option<DateTime<Utc>> {
    let state = app.state::<AppState>();
    let pending = match state.db.with(crate::db::reminders::all_pending) {
        Ok(list) => list,
        Err(e) => {
            log::error!("读取待触发提醒失败: {e}");
            return None;
        }
    };
    let mut min: Option<DateTime<Utc>> = None;
    for r in pending {
        match parse(&r.remind_at) {
            Ok(t) => {
                min = Some(match min {
                    Some(m) => m.min(t),
                    None => t,
                })
            }
            Err(_) => log::error!("提醒 {} 的 remind_at 非法: {}", r.id, r.remind_at),
        }
    }
    min
}

/// 触发所有到点（remind_at <= now）的 pending 提醒。
///
/// 竞态不变量：
/// - 本函数只被 `loop_task`（单消费者）串行调用，不存在并发 fire_due，不会双发通知；
/// - 所有 DB 访问经 `AppState.db`（Mutex<Connection>）串行化，与命令层的
///   删除/snooze 天然互斥：snooze 改 `remind_at` 后 fire 仍会置 `triggered`
///   （一次性通知，snooze 目标被触发覆盖是预期语义）；fire 期间行被删除时
///   `set_status` 影响 0 行、`db::notes::get` 报错走兜底标题，均不 panic。
fn fire_due(app: &AppHandle) {
    let state = app.state::<AppState>();
    let now = Utc::now();
    let cutoff = now - chrono::Duration::hours(STALE_CUTOFF_HOURS);

    let due: Vec<Reminder> = {
        let pending = match state.db.with(crate::db::reminders::all_pending) {
            Ok(list) => list,
            Err(e) => {
                log::error!("fire_due 读取提醒失败: {e}");
                return;
            }
        };
        let mut due = Vec::new();
        for r in pending {
            match parse(&r.remind_at) {
                Ok(t) if t <= now => due.push(r),
                Ok(_) => {}
                Err(_) => log::error!("提醒 {} 的 remind_at 非法: {}", r.id, r.remind_at),
            }
        }
        due
    };
    if due.is_empty() {
        return;
    }

    // 勿扰时段：一次读取（空键 = 关闭勿扰，fail-open）
    let settings = state
        .db
        .with(crate::db::settings::get_all)
        .unwrap_or_default();
    let dnd_window = crate::dnd::parse_window(
        settings.get("dnd_start").map(String::as_str).unwrap_or(""),
        settings.get("dnd_end").map(String::as_str).unwrap_or(""),
    );
    let now_local = chrono::Local::now().time();

    for r in due {
        let Ok(t) = parse(&r.remind_at) else { continue };

        if t < cutoff {
            // 过期兜底：超一天的错过提醒不再补发
            if let Err(e) = state
                .db
                .with(|c| crate::db::reminders::set_status(c, &r.id, "cancelled", false))
            {
                log::error!("取消过期提醒 {} 失败: {e}", r.id);
            }
            crate::filesystem::journal_error(
                &state.paths,
                "notification",
                "提醒已过期超过 24 小时，已自动取消",
                &format!("remind_at={}（应用未运行或休眠期间错过）", r.remind_at),
                "用户不会收到该条提醒",
                "如仍需要，请重新设置提醒时间",
            );
            continue;
        }

        // 勿扰时段：不发通知、保持 pending，推迟到勿扰结束时刻
        if !crate::dnd::should_notify(&settings, now_local) {
            if let Some(w) = &dnd_window {
                let exit_utc = crate::dnd::next_exit_utc(now, w);
                if let Err(e) = state.db.with(|c| {
                    crate::db::reminders::snooze_to(c, &r.id, &crate::reminder::fmt(exit_utc))
                }) {
                    log::error!("勿扰推迟提醒 {} 失败: {e}", r.id);
                } else {
                    log::info!("提醒 {} 处于勿扰时段，推迟到 {}", r.id, exit_utc);
                }
            }
            continue;
        }

        // 便签标题与摘要（私密便签通知脱敏：不下发原文标题与内容）
        let (title, content, is_private) =
            match state.db.with(|c| crate::db::notes::get(c, &r.note_id)) {
                Ok(n) => (n.title, n.content, n.is_private),
                Err(e) => {
                    log::warn!("提醒 {} 关联的便签 {} 不存在: {e}", r.id, r.note_id);
                    ("提醒".to_string(), String::new(), false)
                }
            };
        let (notice_title, _) = crate::privacy::notification_text(&title, &content, is_private);
        let snippet = crate::privacy::mask_snippet(&markdown_snippet(&content, 80), is_private);
        if let Err(e) = app
            .notification()
            .builder()
            .title(format!("🔔 {notice_title}"))
            .body(snippet)
            .show()
        {
            log::error!("发送提醒通知失败: {e}");
            crate::filesystem::journal_error(
                &state.paths,
                "notification",
                "系统通知发送失败",
                &e.to_string(),
                "用户可能错过该提醒",
                "请检查系统通知权限/勿扰模式设置",
            );
        }

        let _ = app.emit(
            crate::events::REMINDER_FIRED,
            json!({
                "reminderId": r.id,
                "noteId": r.note_id,
                "noteTitle": notice_title,
                "remindAt": r.remind_at,
            }),
        );

        // 状态流转：once → triggered；重复 → triggered + 新 pending 行。
        // 事务边界：置 triggered 与补建下一轮 pending 同事务，
        // 防止"已触发但下一轮丢失/丢失旧行但未触发"的中间态。
        let repeat = RepeatType::parse(&r.repeat_type);
        if let Err(e) = state.db.tx(|c| {
            crate::db::reminders::set_status(c, &r.id, "triggered", true)?;
            if repeat != RepeatType::Once {
                if let Some(next) = crate::db::reminders::next_occurrence(repeat, t, Utc::now()) {
                    crate::db::reminders::create(c, &r.note_id, &fmt(next), repeat)?;
                }
            }
            Ok(())
        }) {
            log::error!("提醒 {} 状态流转失败: {e}", r.id);
            crate::filesystem::journal_error(
                &state.paths,
                "notification",
                "提醒状态流转失败",
                &e.to_string(),
                "提醒可能重复触发或丢失后续循环",
                "检查数据库日志并考虑重建该提醒",
            );
        }
    }
    // loop 下一轮会重算最近 pending 时间，无需 wake
}

/// 内容摘要：去 markdown 符号、折叠空白、截断到 max 字符。
fn markdown_snippet(content: &str, max_chars: usize) -> String {
    const MARKDOWN_CHARS: &[char] = &[
        '#', '*', '`', '_', '~', '[', ']', '(', ')', '>', '!', '|', '-',
    ];
    let cleaned: String = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            // 全文剥离 markdown 符号（而非仅行首），保证摘要干净
            line.chars()
                .filter(|c| !MARKDOWN_CHARS.contains(c))
                .collect::<String>()
        })
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: String = cleaned.chars().take(max_chars).collect();
    if cleaned.chars().count() > max_chars {
        out.push('…');
    }
    if out.is_empty() {
        out.push_str("(无内容)");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn fmt_parse_roundtrip() {
        let t = chrono::Utc.with_ymd_and_hms(2026, 9, 15, 8, 30, 5).unwrap();
        let s = fmt(t);
        assert_eq!(s, "2026-09-15T08:30:05+00:00");
        assert_eq!(parse(&s).unwrap(), t);
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse("不是时间").is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn snippet_strips_markdown() {
        let s = markdown_snippet("# 标题\n- [ ] 买牛奶\n\n正文 **加粗**", 80);
        assert!(!s.contains('#'));
        assert!(!s.contains('*'));
        assert!(s.contains("买牛奶"));
        let long = "x".repeat(200);
        let t = markdown_snippet(&long, 80);
        assert_eq!(t.chars().count(), 81); // 80 + '…'
    }
}
