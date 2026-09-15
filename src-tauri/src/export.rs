//! 数据导出/导入（JSON）。SQL 仅限本文件内的 SELECT/UPSERT（硬性要求允许范围）。

use std::collections::HashSet;
use std::path::Path;

use rusqlite::params;
use serde::Serialize;

use crate::db::models::{ExportData, NoteImage, Reminder};
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

/// 构建导出数据：全部便签（不过滤状态）、图片记录、提醒、标签、快捷键、设置。
pub fn build_export(db: &Db) -> AppResult<ExportData> {
    db.with(|c| -> AppResult<ExportData> {
        let notes: Vec<_> = crate::db::notes::list(c, "all")?.into_iter().map(|s| s.note).collect();

        let mut note_images: Vec<NoteImage> = Vec::new();
        {
            let mut stmt = c.prepare(
                "SELECT id, note_id, path, filename, width, height, created_at \
                 FROM note_images ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], |r| {
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
            let mut stmt = c.prepare(
                "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at \
                 FROM reminders ORDER BY remind_at ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(Reminder {
                    id: r.get(0)?,
                    note_id: r.get(1)?,
                    remind_at: r.get(2)?,
                    repeat_type: r.get(3)?,
                    status: r.get(4)?,
                    created_at: r.get(5)?,
                    triggered_at: r.get(6)?,
                })
            })?;
            for row in rows {
                reminders.push(row?);
            }
        }

        let mut tags: Vec<(String, String)> = Vec::new();
        {
            let mut stmt = c.prepare(
                "SELECT nt.note_id, t.name FROM note_tags nt \
                 JOIN tags t ON t.id = nt.tag_id ORDER BY nt.note_id ASC, t.name ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                tags.push(row?);
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
                  monitor_id, created_at, updated_at, archived_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
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
                 (id, note_id, remind_at, repeat_type, status, created_at, triggered_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![r.id, r.note_id, r.remind_at, r.repeat_type, r.status, r.created_at, r.triggered_at],
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

        tx.commit()?;
        Ok(ImportSummary { imported_notes, skipped })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::RepeatType;

    #[test]
    fn export_import_roundtrip() {
        let src = Db::in_memory().unwrap();
        let note = src
            .with(|c| crate::db::notes::create(c, "导出测试", "# 内容\n- [ ] 买牛奶"))
            .unwrap();
        src.with(|c| crate::db::tags::attach_tag(c, &note.id, "工作")).unwrap();
        src.with(|c| {
            crate::db::reminders::create(c, &note.id, "2026-10-01T09:00:00+00:00", RepeatType::Daily)
        })
        .unwrap();

        let data = build_export(&src).unwrap();
        assert_eq!(data.notes.len(), 1);
        assert_eq!(data.tags.len(), 1);
        assert_eq!(data.app, "focusly");

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
            .with(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(tags, 0, "被跳过的标签不应入库");
    }
}
