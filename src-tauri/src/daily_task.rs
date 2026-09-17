//! 今日任务服务：到点通知 ticker、任务转便签编排。
//! 通知复用 Windows 通知 + 勿扰（时段内挂起不标记，结束后补发）+ 私密脱敏。

use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::db::daily_tasks;
use crate::error::AppResult;
use crate::state::AppState;

/// 通知扫描节拍（秒）。分钟粒度任务 + 唤醒补偿，30s 足够。
const NOTIFY_TICK_SECS: u64 = 30;

static NOTIFIER_STARTED: AtomicBool = AtomicBool::new(false);

/// 启动到点通知扫描（lib.rs setup 调用一次；重复调用为无害 no-op）。
pub fn spawn_notifier(app: AppHandle) {
    if NOTIFIER_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(NOTIFY_TICK_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(e) = scan_and_notify(&app) {
                log::error!("今日任务到点通知扫描失败: {e}");
            }
        }
    });
}

/// 单次扫描：物化当日重复实例 → 找到点未通知任务 → 逐条通知并标记。
/// 勿扰时段内不通知也不标记（留给后续 tick 在勿扰结束后补发，不丢失）。
pub fn scan_and_notify(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let today = local_today();
    let hhmm = Local::now().format("%H:%M").to_string();

    let due: Vec<crate::db::models::DailyTask> = state
        .db
        .with(|c| -> AppResult<Vec<crate::db::models::DailyTask>> {
            daily_tasks::materialize_recurring(c, &today)?;
            daily_tasks::due_for_notification(c, &today, &hhmm)
        })?;
    if due.is_empty() {
        return Ok(());
    }

    let settings = state
        .db
        .with(|c| crate::db::settings::get_all(c))
        .unwrap_or_default();
    let now_local = Local::now().time();
    if !crate::dnd::should_notify(&settings, now_local) {
        // 勿扰内：不标记，等时段结束后补发（不丢失）
        return Ok(());
    }

    let mut notified: Vec<String> = Vec::new();
    for t in due {
        let (title, body) = if t.is_private {
            ("⏰ 今日任务".to_string(), "🔒 私密任务内容已隐藏".to_string())
        } else {
            match &t.start_time {
                Some(s) => (
                    "⏰ 今日任务".to_string(),
                    format!("{s} · {}到了", t.title),
                ),
                None => ("⏰ 今日任务".to_string(), t.title.clone()),
            }
        };
        if let Err(e) = app.notification().builder().title(title).body(body).show() {
            log::error!("今日任务通知发送失败 {}: {e}", t.id);
            continue;
        }
        let _ = app.emit(
            crate::events::NOTES_CHANGED,
            serde_json::json!({ "dailyTaskStarted": t.id }),
        );
        log::info!("今日任务到点通知: {}", t.id);
        notified.push(t.id);
    }
    if !notified.is_empty() {
        state.db.with(|c| daily_tasks::mark_notified(c, &notified))?;
    }
    Ok(())
}

/// 任务转便签：标题→便签标题，备注→正文，生成一条 "- [ ] 任务" 待办。
/// 私密任务转出的便签同为私密（私密便签在本机私密视图可见，
/// 脱敏只作用于通知/搜索/导出链路，见 privacy.rs）。
pub fn to_note(app: &AppHandle, task_id: &str) -> AppResult<crate::db::models::Note> {
    let state = app.state::<AppState>();
    let task = state.db.with(|c| daily_tasks::get(c, task_id))?;
    let mut content = format!("- [ ] {}", task.title);
    if let Some(note) = &task.note {
        if !note.trim().is_empty() {
            content.push_str("\n\n");
            content.push_str(note);
        }
    }
    let note = state.db.tx(|c| {
        let n = crate::db::notes::create(c, &task.title, &content)?;
        if task.is_private {
            crate::db::notes::update(
                c,
                &crate::db::notes::NoteUpdate {
                    id: n.id.clone(),
                    title: None,
                    content: None,
                    is_pinned: None,
                    is_always_on_top: None,
                    show_on_all_desktops: None,
                    desktop_pin_state: None,
                    fullscreen_behavior: None,
                    monitor_id: None,
                    is_private: Some(true),
                    locked: None,
                    readonly_flag: None,
                    scale: None,
                    touch: false,
                    pin_mode: None,
                },
            )?;
        }
        crate::db::notes::get(c, &n.id)
    })?;
    let (x, y) = crate::window::cascade_position(app, 0);
    let (w, h) = crate::window::default_note_size_physical(app);
    state.db.with(|c| {
        crate::db::notes::update_geometry(c, &note.id, x, y, w, h, None)
    })?;
    log::info!("今日任务已转为便签 {} → {}", task_id, note.id);
    Ok(note)
}

pub fn local_today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_today_format() {
        let d = local_today();
        assert_eq!(d.len(), 10);
        assert!(chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").is_ok());
    }
}
