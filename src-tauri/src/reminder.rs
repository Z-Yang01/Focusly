//! 提醒调度器：tokio 事件驱动（无轮询）。
//! 依据最近的 pending 提醒睡眠，到点触发通知；插入/修改/提醒后由 `SchedulerHandle::wake()` 唤醒重算。

use std::time::SystemTime;

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
        let _ = self.0.send(());
    }
}

/// 创建调度器通道（handle 存入 AppState，loop task 在 manage 之后启动以避免竞态）。
pub fn channel() -> (SchedulerHandle, UnboundedReceiver<()>) {
    let (tx, rx) = unbounded_channel();
    (SchedulerHandle(tx), rx)
}

/// 一步式启动（manage 之后调用，或确认状态已就绪时使用）。
pub fn spawn(app: AppHandle) -> SchedulerHandle {
    let (handle, rx) = channel();
    spawn_loop(app, rx);
    handle
}

pub fn spawn_loop(app: AppHandle, mut rx: UnboundedReceiver<()>) {
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
                let now = Utc::now();
                if t <= now {
                    fire_due(&app);
                } else {
                    let deadline = tokio::time::Instant::from(SystemTime::from(t));
                    tokio::select! {
                        _ = tokio::time::sleep_until(deadline) => fire_due(&app),
                        _ = rx.recv() => {} // 被唤醒：重新计算最近时间
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
    let pending = match state.db.with(|c| crate::db::reminders::all_pending(c)) {
        Ok(list) => list,
        Err(e) => {
            log::error!("读取待触发提醒失败: {e}");
            return None;
        }
    };
    let mut min: Option<DateTime<Utc>> = None;
    for r in pending {
        match parse(&r.remind_at) {
            Ok(t) => min = Some(match min {
                Some(m) => m.min(t),
                None => t,
            }),
            Err(_) => log::error!("提醒 {} 的 remind_at 非法: {}", r.id, r.remind_at),
        }
    }
    min
}

/// 触发所有到点（remind_at <= now）的 pending 提醒。
fn fire_due(app: &AppHandle) {
    let state = app.state::<AppState>();
    let now = Utc::now();
    let cutoff = now - chrono::Duration::hours(STALE_CUTOFF_HOURS);

    let due: Vec<Reminder> = {
        let pending = match state.db.with(|c| crate::db::reminders::all_pending(c)) {
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

        // 便签标题与摘要（私密便签通知脱敏：不下发原文标题与内容）
        let (title, content, is_private) = match state
            .db
            .with(|c| crate::db::notes::get(c, &r.note_id))
        {
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
            "reminder-fired",
            json!({
                "reminderId": r.id,
                "noteId": r.note_id,
                "noteTitle": notice_title,
                "remindAt": r.remind_at,
            }),
        );

        // 状态流转：once → triggered；重复 → triggered + 新 pending 行
        let repeat = RepeatType::parse(&r.repeat_type);
        if let Err(e) = state.db.with(|c| {
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
    const MARKDOWN_CHARS: &[char] = &['#', '*', '`', '_', '~', '[', ']', '(', ')', '>', '!', '|', '-'];
    let cleaned: String = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.trim_start_matches(MARKDOWN_CHARS).trim())
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
