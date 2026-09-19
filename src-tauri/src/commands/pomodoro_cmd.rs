//! 番茄钟 / 任务元数据命令（薄层：转发 pomodoro.rs 与 db 层）。
//!
//! # 总控接线附录（lib.rs 的 generate_handler 追加以下 13 项）
//! `pomodoro_start, pomodoro_pause, pomodoro_resume, pomodoro_skip, pomodoro_stop,
//! pomodoro_add_minutes, pomodoro_state, pomodoro_complete_task,
//! pomodoro_stats_today, pomodoro_stats_range, task_meta_get, task_meta_list, task_meta_update`
//!
//! 前端参数为 camelCase（tauri 自动转换）：如 `pomodoro_start({ noteId, taskKey, taskText })`、
//! `pomodoro_stop({ reason })`、`pomodoro_add_minutes({ minutes })`、
//! `pomodoro_stats_range({ days })`、`task_meta_update({ noteId, taskKey, lineText, status, estimate, priority, dueAt, clearSkip })`。

use tauri::{AppHandle, Manager, State};

use crate::db::models::TaskMeta;
use crate::db::pomodoro_sessions::{DailyStat, StatsToday};
use crate::error::AppResult;
use crate::pomodoro::{self, PomoSnapshot, PomodoroCmd};
use crate::state::AppState;

/// 开始一个专注阶段。note_id/task_key 缺省 = 无绑定任务的通用专注；task_text 缺省 = "专注"。
/// 返回处理后的最新快照（前端 StatePayload 契约）。
#[tauri::command]
pub async fn pomodoro_start(
    app: AppHandle,
    note_id: Option<String>,
    task_key: Option<String>,
    task_text: Option<String>,
) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(
        &app,
        PomodoroCmd::Start {
            note_id: note_id.unwrap_or_default(),
            task_key: task_key.unwrap_or_default(),
            task_text: task_text.unwrap_or_else(|| "专注".into()),
        },
    )
    .await
}

#[tauri::command]
pub async fn pomodoro_pause(app: AppHandle) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(&app, PomodoroCmd::Pause).await
}

#[tauri::command]
pub async fn pomodoro_resume(app: AppHandle) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(&app, PomodoroCmd::Resume).await
}

/// 跳过当前阶段（interrupted + reason="skip"）。
#[tauri::command]
pub async fn pomodoro_skip(app: AppHandle) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(&app, PomodoroCmd::Skip).await
}

/// 停止番茄钟（interrupted + 自定义原因）。
#[tauri::command]
pub async fn pomodoro_stop(app: AppHandle, reason: Option<String>) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(
        &app,
        PomodoroCmd::Stop {
            reason: reason.unwrap_or_default(),
        },
    )
    .await
}

/// 运行中阶段延时 N 分钟（仅非暂停时生效，planned_sec 同加）。
#[tauri::command]
pub async fn pomodoro_add_minutes(app: AppHandle, minutes: i64) -> AppResult<PomoSnapshot> {
    pomodoro::send_sync(&app, PomodoroCmd::AddMinutes(minutes)).await
}

/// 当前番茄钟快照（Idle / Running 全量状态）。
#[tauri::command]
pub fn pomodoro_state(_app: AppHandle) -> AppResult<PomoSnapshot> {
    Ok(pomodoro::current_snapshot())
}

/// 完成任务：task_meta 置 done + 便签正文第一条匹配行勾选回写。
/// 不改变运行中阶段；返回当前快照（前端 StatePayload 契约）。
#[tauri::command]
pub fn pomodoro_complete_task(
    app: AppHandle,
    note_id: String,
    task_key: String,
) -> AppResult<PomoSnapshot> {
    pomodoro::complete_task(&app, &note_id, &task_key)?;
    Ok(pomodoro::current_snapshot())
}

/// 今日统计：{focusCount, focusSec, doneTasks, skippedTasks, interrupts}。
#[tauri::command]
pub fn pomodoro_stats_today(app: AppHandle) -> AppResult<StatsToday> {
    let state = app.state::<AppState>();
    let (start, now) = pomodoro::local_today_bounds();
    state
        .db
        .with(|c| crate::db::pomodoro_sessions::stats_today(c, &start, &now))
}

/// 近 N 天按本地日聚合：[{date, focusCount, focusSec}]（升序，窗口内零填充）。
#[tauri::command]
pub fn pomodoro_stats_range(app: AppHandle, days: i64) -> AppResult<Vec<DailyStat>> {
    let state = app.state::<AppState>();
    state
        .db
        .with(|c| crate::db::pomodoro_sessions::stats_daily(c, days))
}

/// 复盘报表（周/月窗口；start_date..end_date 本地日闭区间 YYYY-MM-DD）：
/// 逐日聚合 + 合计/完成率 + Top 任务 + 中断原因分布 + 时段分布。
#[tauri::command]
pub fn pomodoro_stats_report(
    app: AppHandle,
    start_date: String,
    end_date: String,
) -> AppResult<crate::db::pomodoro_sessions::FocusReport> {
    let state = app.state::<AppState>();
    state
        .db
        .with(|c| crate::db::pomodoro_sessions::stats_report(c, &start_date, &end_date))
}

/// 复盘报表生成为便签（每周/每月日报）：Markdown 正文，返回新便签（前端决定打开）。
/// 内容源自 stats_report 聚合，私密任务在写入端已脱敏，无泄漏面。
#[tauri::command]
pub fn pomodoro_report_to_note(
    app: AppHandle,
    start_date: String,
    end_date: String,
    label: String,
) -> AppResult<crate::db::models::Note> {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let report = state
        .db
        .with(|c| crate::db::pomodoro_sessions::stats_report(c, &start_date, &end_date))?;
    let content = crate::db::pomodoro_sessions::report_markdown(&label, &report);
    let title = if label.starts_with("专注") {
        label.clone()
    } else {
        format!("专注 {label}")
    };
    let note = state
        .db
        .with(|c| crate::db::notes::create(c, &title, &content))?;
    // 层叠几何：与 daily_task_to_note 同款，避免盖在已有便签上
    let (x, y) = crate::window::cascade_position(&app, 0);
    let (w, h) = crate::window::default_note_size_physical(&app);
    state
        .db
        .with(|c| crate::db::notes::update_geometry(c, &note.id, x, y, w, h, None))?;
    log::info!("报表已生成便签 [{label}] → {}", note.id);
    Ok(note)
}

/// 某本地日的实际专注会话（时间轴"实际专注块"），开始时间升序。
#[tauri::command]
pub fn pomodoro_sessions_by_date(
    app: AppHandle,
    date: String,
) -> AppResult<Vec<crate::db::models::PomodoroSession>> {
    let state = app.state::<AppState>();
    state
        .db
        .with(|c| crate::db::pomodoro_sessions::sessions_by_date(c, &date))
}

/// 单条任务元数据（不存在返回 null）。
#[tauri::command]
pub fn task_meta_get(
    state: State<'_, AppState>,
    note_id: String,
    task_key: String,
) -> AppResult<Option<TaskMeta>> {
    state
        .db
        .with(|c| crate::db::task_meta::get(c, &note_id, &task_key))
}

/// 某便签下全部任务元数据（updated_at DESC）。
#[tauri::command]
pub fn task_meta_list(state: State<'_, AppState>, note_id: String) -> AppResult<Vec<TaskMeta>> {
    state
        .db
        .with(|c| crate::db::task_meta::list_for_note(c, &note_id))
}

/// 插入/部分更新任务元数据。None 字段保留原值；
/// status="skipped" 自动写 skip_date=本地今天；clearSkip=true 复位 todo 并清除 skip_date。
// 参数即前端 IPC 契约，保持平铺不收敛为 struct。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn task_meta_update(
    state: State<'_, AppState>,
    note_id: String,
    task_key: String,
    line_text: String,
    status: Option<String>,
    estimate: Option<i64>,
    priority: Option<String>,
    due_at: Option<String>,
    focus_min: Option<i64>,
    clear_skip: Option<bool>,
) -> AppResult<TaskMeta> {
    state.db.with(|c| {
        crate::db::task_meta::upsert(
            c,
            &crate::db::task_meta::TaskMetaUpsert {
                note_id,
                task_key,
                line_text,
                status,
                estimate,
                priority,
                due_at,
                focus_min,
                clear_skip: clear_skip.unwrap_or(false),
            },
        )
    })
}

/// 待办拖动排序：keys 顺序即展示顺序（1..n）。只写 task_meta.sort_order，不动正文。
#[tauri::command]
pub fn task_meta_reorder(
    state: State<'_, AppState>,
    note_id: String,
    keys: Vec<String>,
) -> AppResult<usize> {
    state
        .db
        .with(|c| crate::db::task_meta::reorder(c, &note_id, &keys))
}
