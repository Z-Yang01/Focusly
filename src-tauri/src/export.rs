//! 数据导出/导入（JSON）。SQL 仅限本文件内的 SELECT/UPSERT（硬性要求允许范围）。

use std::collections::HashSet;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;

use crate::db::models::{ExportData, NoteImage, Reminder, TaskMeta};
use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::reminder;

pub const APP_TAG: &str = "focusly";
pub const SCHEMA_VERSION: i64 = 1;

/// 导入结果摘要。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub imported_notes: usize,
    pub skipped: usize,
}

/// 构建导出数据：全部便签（不过滤归档状态）、图片记录、提醒、标签、快捷键、设置。
///
/// 隐私防线：`include_private=false`（默认）时排除私密便签（is_private=1），
/// 其图片/提醒/标签行一并过滤；`includes_private` 字段如实记录本次导出是否含私密内容。
pub fn build_export(db: &Db, include_private: bool) -> AppResult<ExportData> {
    db.with(|c| -> AppResult<ExportData> {
        // 私密过滤谓词（与 privacy::export_filter_note 同义；privacy 模块接线前先内联）
        let keep = |is_private: bool| include_private || !is_private;
        let notes: Vec<_> = crate::db::notes::list(c, "all")?
            .into_iter()
            .map(|s| s.note)
            .filter(|n| keep(n.is_private))
            .collect();

        let mut note_images: Vec<NoteImage> = Vec::new();
        {
            // LEFT JOIN 过滤被排除便签的图片行；孤儿行（无对应便签）保持原语义保留
            let mut stmt = c.prepare(
                "SELECT ni.id, ni.note_id, ni.path, ni.filename, ni.width, ni.height, ni.created_at \
                 FROM note_images ni \
                 LEFT JOIN notes n ON n.id = ni.note_id \
                 WHERE ?1 OR COALESCE(n.is_private, 0) = 0 \
                 ORDER BY ni.created_at ASC",
            )?;
            let rows = stmt.query_map(params![include_private], |r| {
                Ok(NoteImage {
                    id: r.get(0)?,
                    note_id: r.get(1)?,
                    path: r.get(2)?,
                    filename: r.get(3)?,
                    width: r.get(4)?,
                    height: r.get(5)?,
                    created_at: r.get(6)?,
                })
            })?;
            for row in rows {
                note_images.push(row?);
            }
        }

        let mut reminders: Vec<Reminder> = Vec::new();
        {
            // 同步过滤被排除便签的提醒行（避免通过提醒得知私密便签的存在）
            let mut stmt = c.prepare(
                "SELECT r.id, r.note_id, r.remind_at, r.repeat_type, r.status, r.created_at, r.triggered_at, r.anchor_at \
                 FROM reminders r \
                 LEFT JOIN notes n ON n.id = r.note_id \
                 WHERE ?1 OR COALESCE(n.is_private, 0) = 0 \
                 ORDER BY r.remind_at ASC",
            )?;
            let rows = stmt.query_map(params![include_private], |r| {
                Ok(Reminder {
                    id: r.get(0)?,
                    note_id: r.get(1)?,
                    remind_at: r.get(2)?,
                    repeat_type: r.get(3)?,
                    status: r.get(4)?,
                    created_at: r.get(5)?,
                    triggered_at: r.get(6)?,
                    anchor_at: r.get(7)?,
                })
            })?;
            for row in rows {
                reminders.push(row?);
            }
        }

        let mut tags: Vec<(String, String)> = Vec::new();
        {
            // 标签名可能泄露私密便签内容，同样过滤被排除便签的标签行
            let mut stmt = c.prepare(
                "SELECT nt.note_id, t.name FROM note_tags nt \
                 JOIN tags t ON t.id = nt.tag_id \
                 LEFT JOIN notes n ON n.id = nt.note_id \
                 WHERE ?1 OR COALESCE(n.is_private, 0) = 0 \
                 ORDER BY nt.note_id ASC, t.name ASC",
            )?;
            let rows = stmt.query_map(params![include_private], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                tags.push(row?);
            }
        }

        // 任务三态元数据：私密便签的行按 include_private 过滤（line_text 可能泄露内容）
        let mut task_meta: Vec<TaskMeta> = Vec::new();
        {
            let mut stmt = c.prepare(
                "SELECT tm.note_id, tm.task_key, tm.line_text, tm.status, tm.estimate_pomodoros, \
                 tm.completed_pomodoros, tm.priority, tm.due_at, tm.skip_date, tm.updated_at, \
                 tm.focus_min, tm.sort_order \
                 FROM task_meta tm \
                 LEFT JOIN notes n ON n.id = tm.note_id \
                 WHERE ?1 OR COALESCE(n.is_private, 0) = 0",
            )?;
            let rows = stmt.query_map(params![include_private], |r| {
                Ok(TaskMeta {
                    note_id: r.get(0)?,
                    task_key: r.get(1)?,
                    line_text: r.get(2)?,
                    status: r.get(3)?,
                    estimate_pomodoros: r.get(4)?,
                    completed_pomodoros: r.get(5)?,
                    priority: r.get(6)?,
                    due_at: r.get(7)?,
                    skip_date: r.get(8)?,
                    updated_at: r.get(9)?,
                    focus_min: r.get(10)?,
                    sort_order: r.get(11)?,
                })
            })?;
            for row in rows {
                task_meta.push(row?);
            }
        }

        let shortcuts = crate::db::shortcuts::list(c)?;
        let mut settings: Vec<(String, String)> =
            crate::db::settings::get_all(c)?.into_iter().collect();
        settings.sort();

        Ok(ExportData {
            app: APP_TAG.to_string(),
            schema_version: SCHEMA_VERSION,
            exported_at: reminder::fmt(chrono::Utc::now()),
            notes,
            note_images,
            reminders,
            tags,
            shortcuts,
            settings,
            task_meta,
            includes_private: include_private,
        })
    })
}

/// 写出（pretty JSON）。
pub fn write_to_file(data: &ExportData, path: &str) -> AppResult<()> {
    let json = serde_json::to_string_pretty(data)
        .map_err(|e| AppError::Invalid(format!("导出数据序列化失败: {e}")))?;
    std::fs::write(Path::new(path), json)?;
    Ok(())
}

/// 从文件导入。app 字段必须为 focusly；事务内全部 upsert（保留原 id）；
/// 外键数据（images/reminders/tags）指向不存在的便签时跳过并计入 skipped。
pub fn import_from_file(db: &Db, path: &str) -> AppResult<ImportSummary> {
    let raw = std::fs::read_to_string(Path::new(path))?;
    let data: ExportData = serde_json::from_str(&raw)
        .map_err(|e| AppError::Invalid(format!("导入文件解析失败: {e}")))?;
    if data.app != APP_TAG {
        return Err(AppError::Invalid(format!(
            "不是 Focusly 导出文件（app={}）",
            data.app
        )));
    }

    db.with(|c| -> AppResult<ImportSummary> {
        let tx = c.unchecked_transaction()?;
        let mut imported_notes = 0usize;
        let mut skipped = 0usize;
        let mut note_ids: HashSet<String> = HashSet::new();

        for n in &data.notes {
            tx.execute(
                "INSERT OR REPLACE INTO notes \
                 (id, title, content, content_format, status, is_pinned, is_always_on_top, \
                  show_on_all_desktops, desktop_pin_state, fullscreen_behavior, x, y, width, height, \
                  monitor_id, created_at, updated_at, archived_at, deleted_at, is_private, locked, readonly_flag, scale, pin_mode) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)",
                params![
                    n.id,
                    n.title,
                    n.content,
                    n.content_format,
                    n.status,
                    n.is_pinned as i64,
                    n.is_always_on_top as i64,
                    n.show_on_all_desktops as i64,
                    n.desktop_pin_state,
                    n.fullscreen_behavior,
                    n.x,
                    n.y,
                    n.width,
                    n.height,
                    n.monitor_id,
                    n.created_at,
                    n.updated_at,
                    n.archived_at,
                    n.deleted_at,
                    n.is_private as i64,
                    n.locked as i64,
                    n.readonly as i64,
                    n.scale,
                    // 列为 NOT NULL DEFAULT 'normal'；旧导出文件缺省 pinMode 时按建表默认值落库
                    n.pin_mode.clone().unwrap_or_else(|| "normal".into()),
                ],
            )?;
            note_ids.insert(n.id.clone());
            imported_notes += 1;
        }

        for img in &data.note_images {
            if !note_ids.contains(&img.note_id) {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO note_images \
                 (id, note_id, path, filename, width, height, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![img.id, img.note_id, img.path, img.filename, img.width, img.height, img.created_at],
            )?;
        }

        for r in &data.reminders {
            if !note_ids.contains(&r.note_id) {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO reminders \
                 (id, note_id, remind_at, repeat_type, status, created_at, triggered_at, anchor_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![r.id, r.note_id, r.remind_at, r.repeat_type, r.status, r.created_at, r.triggered_at, r.anchor_at],
            )?;
        }

        for (note_id, tag_name) in &data.tags {
            if !note_ids.contains(note_id) {
                skipped += 1;
                continue;
            }
            let tag_id: String = match tx.query_row(
                "SELECT id FROM tags WHERE name = ?1",
                params![tag_name],
                |r| r.get(0),
            ) {
                Ok(id) => id,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    let id = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO tags (id, name, created_at) VALUES (?1, ?2, ?3)",
                        params![id, tag_name, reminder::fmt(chrono::Utc::now())],
                    )?;
                    id
                }
                Err(e) => return Err(e.into()),
            };
            tx.execute(
                "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
                params![note_id, tag_id],
            )?;
        }

        for s in &data.shortcuts {
            tx.execute(
                "INSERT OR REPLACE INTO shortcuts (action, accelerator, enabled, updated_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![s.action, s.accelerator, s.enabled as i64, reminder::fmt(chrono::Utc::now())],
            )?;
        }

        for (key, value) in &data.settings {
            tx.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }

        // 任务三态元数据：仅还原 note_ids 中存在的便签对应行（历史保留语义）
        for tm in &data.task_meta {
            if !note_ids.contains(&tm.note_id) {
                skipped += 1;
                continue;
            }
            tx.execute(
                "INSERT OR REPLACE INTO task_meta (note_id, task_key, line_text, status, estimate_pomodoros, completed_pomodoros, priority, due_at, skip_date, updated_at, focus_min, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    tm.note_id,
                    tm.task_key,
                    tm.line_text,
                    tm.status,
                    tm.estimate_pomodoros,
                    tm.completed_pomodoros,
                    tm.priority,
                    tm.due_at,
                    tm.skip_date,
                    tm.updated_at,
                    tm.focus_min,
                    tm.sort_order,
                ],
            )?;
        }

        // P0-2 修复：重建 FTS 索引，导入数据对全文搜索立即可见（事务内）
        crate::db::search::fts_rebuild_all(&tx)?;
        tx.commit()?;

        Ok(ImportSummary { imported_notes, skipped })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::RepeatType;
    use crate::db::notes::NoteUpdate;

    /// 建一条私密便签，并挂上图片行、提醒、标签（用于过滤测试）。
    fn create_private_note_with_attachments(db: &Db) -> String {
        let note = db
            .with(|c| crate::db::notes::create(c, "私密便签", "绝密内容"))
            .unwrap();
        db.with(|c| {
            crate::db::notes::update(
                c,
                &NoteUpdate {
                    id: note.id.clone(),
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
            )
        })
        .unwrap();
        db.with(|c| {
            c.execute(
                "INSERT INTO note_images (id, note_id, path, filename, width, height, created_at) \
                 VALUES ('img-p', ?1, 'p', 'f', 1, 1, '')",
                params![note.id],
            )?;
            crate::db::reminders::create(
                c,
                &note.id,
                "2026-10-01T09:00:00+00:00",
                RepeatType::Once,
            )?;
            crate::db::tags::attach_tag(c, &note.id, "绝密标签")
        })
        .unwrap();
        note.id
    }

    #[test]
    fn export_import_roundtrip() {
        let src = Db::in_memory().unwrap();
        let note = src
            .with(|c| crate::db::notes::create(c, "导出测试", "# 内容\n- [ ] 买牛奶"))
            .unwrap();
        src.with(|c| crate::db::tags::attach_tag(c, &note.id, "工作"))
            .unwrap();
        src.with(|c| {
            crate::db::reminders::create(
                c,
                &note.id,
                "2026-10-01T09:00:00+00:00",
                RepeatType::Daily,
            )
        })
        .unwrap();

        let data = build_export(&src, false).unwrap();
        assert_eq!(data.notes.len(), 1);
        assert_eq!(data.tags.len(), 1);
        assert_eq!(data.app, "focusly");
        assert!(!data.includes_private, "默认导出不含私密便签");

        // 写文件再读回，验证完整往返
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("export.json");
        write_to_file(&data, file.to_str().unwrap()).unwrap();

        let dst = Db::in_memory().unwrap();
        let summary = import_from_file(&dst, file.to_str().unwrap()).unwrap();
        assert_eq!(summary.imported_notes, 1);
        assert_eq!(summary.skipped, 0);

        let imported = dst.with(|c| crate::db::notes::get(c, &note.id)).unwrap();
        assert_eq!(imported.title, "导出测试");
        assert_eq!(imported.content, "# 内容\n- [ ] 买牛奶");

        let tags = dst
            .with(|c| crate::db::tags::tags_for_note(c, &note.id))
            .unwrap();
        assert_eq!(tags, vec!["工作".to_string()]);

        let pending = dst
            .with(|c| crate::db::reminders::pending_for_note(c, &note.id))
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].repeat_type, "daily");
    }

    #[test]
    fn export_excludes_private_by_default() {
        let src = Db::in_memory().unwrap();
        src.with(|c| crate::db::notes::create(c, "普通便签", "公开内容"))
            .unwrap();
        let private_id = create_private_note_with_attachments(&src);

        let data = build_export(&src, false).unwrap();
        assert!(!data.includes_private);
        assert_eq!(data.notes.len(), 1, "只应导出非私密便签");
        assert_eq!(data.notes[0].title, "普通便签");
        assert!(
            !serde_json::to_string(&data).unwrap().contains("绝密内容"),
            "导出 JSON 不得包含私密便签正文"
        );
        assert!(data.note_images.is_empty(), "私密便签的图片行应被过滤");
        assert!(data.reminders.is_empty(), "私密便签的提醒应被过滤");
        assert!(data.tags.is_empty(), "私密便签的标签行应被过滤");
        assert!(data.tags.iter().all(|(nid, _)| nid != &private_id));

        // 导出文件导入后也不应出现该私密便签
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("no-private.json");
        write_to_file(&data, file.to_str().unwrap()).unwrap();
        let dst = Db::in_memory().unwrap();
        let summary = import_from_file(&dst, file.to_str().unwrap()).unwrap();
        assert_eq!(summary.imported_notes, 1);
        assert!(dst.with(|c| crate::db::notes::get(c, &private_id)).is_err());
    }

    #[test]
    fn export_includes_private_when_opted_in() {
        let src = Db::in_memory().unwrap();
        src.with(|c| crate::db::notes::create(c, "普通便签", "公开内容"))
            .unwrap();
        create_private_note_with_attachments(&src);

        let data = build_export(&src, true).unwrap();
        assert!(data.includes_private, "应如实记录本次导出包含私密便签");
        assert_eq!(data.notes.len(), 2);
        assert!(data.notes.iter().any(|n| n.is_private));
        assert_eq!(data.note_images.len(), 1);
        assert_eq!(data.reminders.len(), 1);
        assert_eq!(data.tags.len(), 1);
    }

    #[test]
    fn import_rejects_foreign_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("other.json");
        std::fs::write(&file, r#"{"app":"someone-else"}"#).unwrap();
        let db = Db::in_memory().unwrap();
        let r = import_from_file(&db, file.to_str().unwrap());
        assert!(matches!(r, Err(AppError::Invalid(_))));
    }

    #[test]
    fn import_skips_orphan_fk_rows() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("orphans.json");
        std::fs::write(
            &file,
            r#"{
                "app": "focusly",
                "schemaVersion": 1,
                "exportedAt": "2026-01-01T00:00:00+00:00",
                "notes": [],
                "noteImages": [
                    {"id": "i1", "noteId": "ghost", "path": "p", "filename": "f", "width": 1, "height": 1, "createdAt": ""}
                ],
                "reminders": [
                    {"id": "r1", "noteId": "ghost", "remindAt": "2026-01-01T00:00:00+00:00", "repeatType": "once", "status": "pending", "createdAt": "", "triggeredAt": null}
                ],
                "tags": [["ghost", "孤立标签"]],
                "shortcuts": [],
                "settings": []
            }"#,
        )
        .unwrap();
        let db = Db::in_memory().unwrap();
        let summary = import_from_file(&db, file.to_str().unwrap()).unwrap();
        assert_eq!(summary.imported_notes, 0);
        assert_eq!(summary.skipped, 3, "指向不存在便签的外键数据应跳过");
        let tags: i64 = db
            .with(|c| Ok(c.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?))
            .unwrap();
        assert_eq!(tags, 0, "被跳过的标签不应入库");
    }
}
