//! 每日笔记：按本地日期定位（或创建）当天专属便签的服务模块。
//!
//! 与 notes.rs 服务层模式一致（跨 DAO + 窗口 + 事件），但独立成文件：
//! 查找用本文件内的只读 SQL（title 精确匹配），创建复用 `db::notes::create` +
//! `update_geometry`（层叠位置）+ `db::tags::attach_tag`（打标签）。
//!
//! # 总控接线附录：未来命令（本文件不含命令层代码）
//!
//! | 命令名                | 参数 | 返回               |
//! |-----------------------|------|--------------------|
//! | `daily_get_or_create` | 无   | `AppResult<Note>`  |
//!
//! 总控执行：
//! 1. lib.rs 添加 `mod daily;`；
//! 2. 命令层薄封装并注册进 invoke_handler：
//!    `#[tauri::command] fn daily_get_or_create(app: tauri::AppHandle) -> Result<Note, AppError> { daily::get_or_create_daily(&app) }`。
//!
//! 前端入口：src/features/templates/DailyNoteButton.tsx（成功静默，后端开窗）。

use chrono::{DateTime, Utc};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::models::Note;
use crate::error::AppResult;
use crate::state::AppState;

/// 每日笔记固定标签
pub const DAILY_TAG: &str = "每日笔记";
/// 每日笔记默认尺寸（略高于普通便签，正文含待办区）
const DAILY_NOTE_W: i32 = 320;
const DAILY_NOTE_H: i32 = 420;

/// 本地日期「YYYY-MM-DD」（Local 时区，纯函数便于单测）。
pub fn daily_date(now: DateTime<Utc>) -> String {
    now.with_timezone(&chrono::Local)
        .format("%Y-%m-%d")
        .to_string()
}

/// 每日笔记标题：「每日笔记 YYYY-MM-DD」（本地时区）。
/// 标题格式是本模块的对外契约（find_today 按它精确匹配），暂仅单测直接引用，
/// `get_or_create_daily` 内联了同义 format!，接线/重构时统一收敛到此函数。
#[allow(dead_code)]
pub fn daily_title(now: DateTime<Utc>) -> String {
    format!("每日笔记 {}", daily_date(now))
}

/// 当天新建时的初始正文：日期标题 + 待办区 + 记录区。
pub fn daily_content(date: &str) -> String {
    format!("# 每日笔记 {date}\n\n## 今日待办\n\n- [ ] \n\n## 记录\n")
}

/// 找今天已存在的每日笔记（title 精确匹配 + active + 未删除，取最近更新的一条）。
/// SQL 收敛进 DAO（db::notes::find_id_by_title），本层只做"id → 完整行"编排。
fn find_today(conn: &rusqlite::Connection, title: &str) -> AppResult<Option<Note>> {
    match crate::db::notes::find_id_by_title(conn, title)? {
        Some(id) => Ok(Some(crate::db::notes::get(conn, &id)?)),
        None => Ok(None),
    }
}

/// 创建今天的每日笔记：层叠几何 + 初始正文 + 「每日笔记」标签 + FTS 同步。
/// 事务边界：建行/几何/标签同事务；FTS 保持"失败仅告警不阻塞创建"语义。
fn create_today(app: &AppHandle, date: &str) -> AppResult<Note> {
    let title = format!("每日笔记 {date}");
    let content = daily_content(date);
    let state = app.state::<AppState>();
    let active = state.db.with(|c| crate::db::notes::count(c, "active"))?;
    let (x, y) = crate::window::cascade_position(app, active.max(0) as usize);
    state.db.tx(|c| -> AppResult<Note> {
        let note = crate::db::notes::create(c, &title, &content)?;
        crate::db::notes::update_geometry(c, &note.id, x, y, DAILY_NOTE_W, DAILY_NOTE_H, None)?;
        crate::db::tags::attach_tag(c, &note.id, DAILY_TAG)?;
        // 标签进 FTS tags 列（失败不阻塞创建主流程，同 notes.rs 策略）
        if let Err(e) = crate::db::search::fts_sync(c, &note.id, &title, &content, DAILY_TAG) {
            log::warn!("每日笔记 FTS 同步失败 {}: {e}", note.id);
        }
        crate::db::notes::get(c, &note.id)
    })
}

/// 找到今天的每日笔记则直接开窗；没有则创建（打标签 + 层叠位置）再开窗。
/// 对应未来命令 `daily_get_or_create`（无参 -> Note）。
pub fn get_or_create_daily(app: &AppHandle) -> AppResult<Note> {
    let date = daily_date(Utc::now());
    let title = format!("每日笔记 {date}");
    let state = app.state::<AppState>();
    let existing = state.db.with(|c| find_today(c, &title))?;
    let (note, created) = match existing {
        Some(note) => (note, false),
        None => (create_today(app, &date)?, true),
    };
    crate::window::open_note_window(app, &note)?;
    if created {
        let _ = app.emit(crate::events::NOTES_CHANGED, json!({ "noteId": note.id }));
        log::info!("已创建每日笔记 {}", note.id);
    }
    Ok(note)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    /// 用「本地时区的某天某刻」反推 UTC 瞬间，使断言与运行机器时区无关。
    fn local_instant(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> DateTime<Utc> {
        chrono::Local
            .with_ymd_and_hms(y, m, d, hh, mm, 0)
            .earliest()
            .expect("有效的本地时间")
            .with_timezone(&Utc)
    }

    #[test]
    fn title_uses_local_date() {
        assert_eq!(
            daily_title(local_instant(2026, 9, 15, 12, 0)),
            "每日笔记 2026-09-15"
        );
        assert_eq!(
            daily_title(local_instant(2026, 1, 1, 12, 0)),
            "每日笔记 2026-01-01"
        );
    }

    #[test]
    fn local_midnight_maps_regardless_of_utc_offset() {
        // 本地 00:30 换算成 UTC 后瞬间可能落在前一天，标题仍应为本地当天日期
        assert_eq!(
            daily_title(local_instant(2026, 9, 15, 0, 30)),
            "每日笔记 2026-09-15"
        );
        // 本地 23:30 的 UTC 瞬间可能已在次日，标题仍为本地当天日期
        assert_eq!(
            daily_title(local_instant(2026, 9, 15, 23, 30)),
            "每日笔记 2026-09-15"
        );
    }

    #[test]
    fn content_has_date_heading_and_sections() {
        let c = daily_content("2026-09-15");
        assert!(c.starts_with("# 每日笔记 2026-09-15\n"));
        assert!(c.contains("## 今日待办"));
        assert!(c.contains("- [ ] "));
        assert!(c.contains("## 记录"));
    }
}
