//! 便签业务编排（区别于 db::notes DAO）：跨 DAO、窗口与事件通知的完整流程。

use std::collections::HashSet;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::models::{Note, NoteDetail};
use crate::db::notes::NoteUpdate;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::window;

/// 新便签默认尺寸（与 window::DEFAULT_NOTE_W/H 保持一致）
const NEW_NOTE_W: i32 = 320;
const NEW_NOTE_H: i32 = 360;

fn emit_notes_changed(app: &AppHandle, note_id: &str) {
    let _ = app.emit("notes-changed", json!({ "noteId": note_id }));
}

/// 只建行并写入层叠几何，不打开窗口（create_note 命令用）。
pub fn create_record(app: &AppHandle) -> AppResult<Note> {
    let state = app.state::<AppState>();
    let active = state.db.with(|c| crate::db::notes::count(c, "active"))?;
    let (x, y) = window::cascade_position(app, active.max(0) as usize);
    let note = state.db.with(|c| crate::db::notes::create(c, "", ""))?;
    state.db.with(|c| {
        crate::db::notes::update_geometry(c, &note.id, x, y, NEW_NOTE_W, NEW_NOTE_H, None)
    })?;
    state.db.with(|c| crate::db::notes::get(c, &note.id))
}

/// 新建并打开便签（托盘/快捷键/管理器共用）。
pub fn create_and_open(app: &AppHandle) -> AppResult<Note> {
    let note = create_record(app)?;
    window::open_note_window(app, &note)?;
    emit_notes_changed(app, &note.id);
    log::info!("新建便签 {}", note.id);
    Ok(note)
}

/// 打开已有便签窗口（不存在则 Invalid）。
pub fn open_existing(app: &AppHandle, id: &str) -> AppResult<()> {
    let note = {
        let state = app.state::<AppState>();
        state.db.with(|c| crate::db::notes::get(c, id))?
    };
    window::open_note_window(app, &note)?;
    Ok(())
}

/// 便签详情：note + images + tags + 最近一条 pending 提醒。
pub fn get_detail(app: &AppHandle, id: &str) -> AppResult<NoteDetail> {
    let state = app.state::<AppState>();
    state.db.with(|c| -> AppResult<NoteDetail> {
        let note = crate::db::notes::get(c, id)?;
        let images = crate::db::images::list_for_note(c, id)?;
        let tags = crate::db::tags::tags_for_note(c, id)?;
        let next_reminder = crate::db::reminders::pending_for_note(c, id)?.into_iter().next();
        Ok(NoteDetail { note, images, tags, next_reminder })
    })
}

/// 更新标题/正文（touch 刷新 updated_at）。
/// 同步：版本快照（source=auto）+ FTS 索引。
pub fn update_content(app: &AppHandle, id: &str, title: &str, content: &str) -> AppResult<Note> {
    let state = app.state::<AppState>();
    let note = state.db.with(|c| -> AppResult<Note> {
        let note = crate::db::notes::update(
            c,
            &NoteUpdate {
                id: id.to_string(),
                title: Some(title.to_string()),
                content: Some(content.to_string()),
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: None,
                touch: true,
            },
        )?;
        // 内容变更：快照 + 重建 FTS 索引（失败不阻塞保存主流程）
        if let Err(e) = crate::db::notes::snapshot_version(c, id, "auto") {
            log::warn!("版本快照失败 {id}: {e}");
        }
        let tags = crate::db::tags::tags_for_note(c, id)?;
        if let Err(e) = crate::db::search::fts_sync(c, id, title, content, &tags.join(" ")) {
            log::warn!("FTS 同步失败 {id}: {e}");
        }
        Ok(note)
    })?;
    emit_notes_changed(app, id);
    Ok(note)
}

/// 设置便签标志：pinned / always_on_top / all_desktops。
pub fn set_flag(app: &AppHandle, id: &str, flag: &str, value: bool) -> AppResult<Note> {
    let field = match flag {
        "pinned" => "pinned",
        "always_on_top" => "always_on_top",
        "all_desktops" => "all_desktops",
        other => return Err(AppError::Invalid(format!("未知便签标志: {other}"))),
    };
    let state = app.state::<AppState>();
    let note = state.db.with(|c| {
        crate::db::notes::update(
            c,
            &NoteUpdate {
                id: id.to_string(),
                title: None,
                content: None,
                is_pinned: (field == "pinned").then_some(value),
                is_always_on_top: (field == "always_on_top").then_some(value),
                show_on_all_desktops: (field == "all_desktops").then_some(value),
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: None,
                touch: true,
            },
        )
    })?;
    // all_desktops 的虚拟桌面固定在 apply_note_flags -> apply_desktop_pin 内完成
    window::apply_note_flags(app, &note);
    emit_notes_changed(app, id);
    Ok(note)
}

/// 设置全屏行为：normal / always_top / fullscreen_show / fullscreen_hide。
pub fn set_fullscreen_behavior(app: &AppHandle, id: &str, behavior: &str) -> AppResult<Note> {
    const VALID: &[&str] = &["normal", "always_top", "fullscreen_show", "fullscreen_hide"];
    if !VALID.contains(&behavior) {
        return Err(AppError::Invalid(format!(
            "未知全屏行为: {behavior}（可选 {VALID:?}）"
        )));
    }
    let state = app.state::<AppState>();
    let note = state.db.with(|c| {
        crate::db::notes::update(
            c,
            &NoteUpdate {
                id: id.to_string(),
                title: None,
                content: None,
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: Some(behavior.to_string()),
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: None,
                touch: false,
            },
        )
    })?;
    window::apply_note_flags(app, &note);
    emit_notes_changed(app, id);
    Ok(note)
}

/// 归档：先关窗口再落库。
pub fn archive(app: &AppHandle, id: &str) -> AppResult<Note> {
    window::close_note_window(app, id);
    let state = app.state::<AppState>();
    let note = state.db.with(|c| crate::db::notes::archive(c, id))?;
    log::info!("便签已归档 {id}");
    emit_notes_changed(app, id);
    Ok(note)
}

/// 从归档恢复（不自动开窗，由前端决定）。
pub fn restore(app: &AppHandle, id: &str) -> AppResult<Note> {
    let state = app.state::<AppState>();
    let note = state.db.with(|c| crate::db::notes::restore(c, id))?;
    log::info!("便签已恢复 {id}");
    emit_notes_changed(app, id);
    Ok(note)
}

/// 永久删除：关窗口 → 删行（级联图片/标签记录）→ 清理图片文件与目录。
pub fn delete_permanently(app: &AppHandle, id: &str) -> AppResult<()> {
    window::close_note_window(app, id);
    let state = app.state::<AppState>();
    let image_paths = state.db.with(|c| {
        crate::db::search::fts_remove(c, id);
        crate::db::notes::delete(c, id)
    })?;
    for path in &image_paths {
        crate::db::images::delete_file_best_effort(&state.paths.images, path);
    }
    crate::db::images::cleanup_note_dir(&state.paths.images, id);
    log::info!("便签已永久删除 {id}（清理 {} 个图片文件）", image_paths.len());
    emit_notes_changed(app, id);
    Ok(())
}

/// 移入回收站（软删除）：关窗口，内容保留，可恢复。
pub fn move_to_trash(app: &AppHandle, id: &str) -> AppResult<Note> {
    window::close_note_window(app, id);
    let state = app.state::<AppState>();
    let note = state.db.with(|c| crate::db::notes::soft_delete(c, id))?;
    log::info!("便签已移入回收站 {id}");
    emit_notes_changed(app, id);
    Ok(note)
}

/// 从回收站恢复。
pub fn restore_from_trash(app: &AppHandle, id: &str) -> AppResult<Note> {
    let state = app.state::<AppState>();
    let note = state.db.with(|c| crate::db::notes::restore_from_trash(c, id))?;
    log::info!("便签已从回收站恢复 {id}");
    emit_notes_changed(app, id);
    Ok(note)
}

/// 清空回收站：逐个永久删除。
pub fn empty_trash(app: &AppHandle) -> AppResult<usize> {
    let state = app.state::<AppState>();
    let ids: Vec<String> = state.db.with(|c| {
        let mut stmt = c.prepare("SELECT id FROM notes WHERE deleted_at IS NOT NULL")?;
        let ids = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(ids)
    })?;
    let n = ids.len();
    for id in ids {
        delete_permanently(app, &id)?;
    }
    log::info!("回收站已清空（{n} 条）");
    Ok(n)
}

/// 设置私密标志：private / locked / readonly。
pub fn set_privacy_flag(app: &AppHandle, id: &str, flag: &str, value: bool) -> AppResult<Note> {
    let field = match flag {
        "private" => "is_private",
        "locked" => "locked",
        "readonly" => "readonly",
        other => return Err(AppError::Invalid(format!("未知隐私标志: {other}"))),
    };
    let state = app.state::<AppState>();
    let note = state.db.with(|c| {
        crate::db::notes::update(
            c,
            &NoteUpdate {
                id: id.to_string(),
                title: None,
                content: None,
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: (field == "is_private").then_some(value),
                locked: (field == "locked").then_some(value),
                readonly_flag: (field == "readonly").then_some(value),
                scale: None,
                touch: false,
            },
        )
    })?;
    log::info!("便签 {id} 隐私标志 {flag}={value}");
    emit_notes_changed(app, id);
    Ok(note)
}

/// 恢复到指定历史版本（restore_version 内部先做 pre-restore 快照）。
pub fn restore_note_version(app: &AppHandle, version_id: &str) -> AppResult<Note> {
    let state = app.state::<AppState>();
    let note = state.db.with(|c| -> AppResult<Note> {
        let note = crate::db::versions::restore_version(c, version_id)?;
        let tags = crate::db::tags::tags_for_note(c, &note.id)?;
        crate::db::search::fts_sync(c, &note.id, &note.title, &note.content, &tags.join(" "))?;
        Ok(note)
    })?;
    emit_notes_changed(app, &note.id);
    Ok(note)
}

/// FTS 全文搜索（排除回收站；私密便签默认排除）。
pub fn search_notes_v2(app: &AppHandle, keyword: &str, include_private: bool) -> AppResult<Vec<crate::db::models::SearchHit>> {
    let state = app.state::<AppState>();
    let hits = state.db.with(|c| crate::db::search::search(c, keyword, include_private))?;
    Ok(hits)
}

/// 重设便签标签：清空现有关联后附着新集合（去重），返回最终标签列表。
pub fn set_note_tags(app: &AppHandle, id: &str, tags: Vec<String>) -> AppResult<Vec<String>> {
    let final_tags = {
        let state = app.state::<AppState>();
        state.db.with(|c| -> AppResult<Vec<String>> {
            let existing = crate::db::tags::tags_for_note(c, id)?;
            for name in existing {
                let _ = crate::db::tags::detach_tag(c, id, &name);
            }
            let mut seen: HashSet<String> = HashSet::new();
            for tag in tags {
                let name = tag.trim().to_string();
                if name.is_empty() || !seen.insert(name.clone()) {
                    continue;
                }
                crate::db::tags::attach_tag(c, id, &name)?;
            }
            crate::db::tags::tags_for_note(c, id)
        })?
    };
    // 标签变化 → FTS 索引重同步（tags 列可搜）
    {
        let state = app.state::<AppState>();
        let _ = state.db.with(|c| -> AppResult<()> {
            let note = crate::db::notes::get(c, id)?;
            crate::db::search::fts_sync(c, id, &note.title, &note.content, &final_tags.join(" "))
        });
    }
    emit_notes_changed(app, id);
    Ok(final_tags)
}

/// 导出全部数据到 JSON 文件（隐私防线：默认排除私密便签，include_private=false）。
pub fn export_data(app: &AppHandle, path: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    let data = crate::export::build_export(&state.db, false)?;
    crate::export::write_to_file(&data, path)?;
    log::info!("数据已导出: {path}");
    Ok(())
}

/// 从 JSON 文件导入数据。
pub fn import_data(app: &AppHandle, path: &str) -> AppResult<crate::export::ImportSummary> {
    let state = app.state::<AppState>();
    let summary = crate::export::import_from_file(&state.db, path)?;
    let _ = app.emit("notes-changed", json!({}));
    log::info!("数据导入完成: {summary:?}");
    Ok(summary)
}
