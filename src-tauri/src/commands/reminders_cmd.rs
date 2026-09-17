//! 提醒命令（薄层）：时间统一经 reminder::parse/fmt，改库后唤醒调度器。

use tauri::{AppHandle, Emitter, State};

use crate::db::models::{Reminder, RepeatType};
use crate::error::AppResult;
use crate::reminder;
use crate::state::AppState;

/// 为便签设置提醒（替换该便签现有 pending 提醒）。
/// 允许过去时间：已过期则调度器下一轮立即触发。
#[tauri::command]
pub fn set_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    note_id: String,
    remind_at: String,
    repeat_type: String,
) -> AppResult<Reminder> {
    let t = reminder::parse(&remind_at)?;
    let repeat = RepeatType::parse(&repeat_type);
    let r = state.db.with(|c| {
        crate::db::reminders::cancel_pending_for_note(c, &note_id)?;
        crate::db::reminders::create(c, &note_id, &reminder::fmt(t), repeat)
    })?;
    state.scheduler.wake();
    let _ = app.emit("notes-changed", serde_json::json!({"noteId": note_id}));
    log::info!("已设置提醒 {}（{}，{}）", r.id, r.remind_at, r.repeat_type);
    Ok(r)
}

#[tauri::command]
pub fn cancel_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    reminder_id: String,
) -> AppResult<()> {
    state
        .db
        .with(|c| crate::db::reminders::set_status(c, &reminder_id, "cancelled", false))?;
    state.scheduler.wake();
    let _ = app; // 预留：如需通知前端可在此 emit
    Ok(())
}

#[tauri::command]
pub fn complete_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    reminder_id: String,
) -> AppResult<()> {
    state
        .db
        .with(|c| crate::db::reminders::set_status(c, &reminder_id, "done", false))?;
    state.scheduler.wake();
    let _ = app; // 预留：如需通知前端可在此 emit
    Ok(())
}

/// 推迟到新时间并回到 pending。
#[tauri::command]
pub fn snooze_reminder(
    app: AppHandle,
    state: State<'_, AppState>,
    reminder_id: String,
    remind_at: String,
) -> AppResult<Reminder> {
    let t = reminder::parse(&remind_at)?;
    let r = state
        .db
        .with(|c| crate::db::reminders::snooze_to(c, &reminder_id, &reminder::fmt(t)))?;
    state.scheduler.wake();
    let _ = app;
    Ok(r)
}

#[tauri::command]
pub fn list_reminders(
    state: State<'_, AppState>,
    note_id: String,
    limit: Option<i64>,
) -> AppResult<Vec<Reminder>> {
    state
        .db
        .with(|c| crate::db::reminders::list_for_note(c, &note_id, limit.unwrap_or(50)))
}

// ---------- 自然语言时间解析（批3接线） ----------

/// 解析中文自然语言时间（"明天下午3点"/"每周一10点"/"工作日9点"/"30分钟后"…）。
/// 返回 None 表示无法解析；前端据此提示。
#[tauri::command]
pub fn parse_time_nl(input: String) -> AppResult<Option<serde_json::Value>> {
    let now = chrono::Utc::now();
    Ok(crate::timeparse::parse_time_nl(&input, now).map(|p| {
        serde_json::json!({
            "at": crate::reminder::fmt(p.at),
            "repeat": p.repeat.as_str(),
        })
    }))
}
