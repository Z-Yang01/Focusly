//! 便签版本快照（note_versions 表）。写入入口是 db::notes::snapshot_version，
//! 本模块负责查询与恢复。每张便签最多保留 50 条（写入时清理）。

use rusqlite::{params, Connection, Row};

use crate::db::models::{Note, NoteVersion};
use crate::db::notes;
use crate::error::{AppError, AppResult};

/// 单张便签的版本保留上限（与 notes::snapshot_version 的清理逻辑一致）
pub const MAX_VERSIONS_PER_NOTE: i64 = 50;

fn row_to_version(r: &Row) -> rusqlite::Result<NoteVersion> {
    Ok(NoteVersion {
        id: r.get("id")?,
        note_id: r.get("note_id")?,
        title: r.get("title")?,
        content: r.get("content")?,
        source: r.get("source")?,
        created_at: r.get("created_at")?,
    })
}

const VERSION_COLS: &str = "id, note_id, title, content, source, created_at";

/// 某便签的版本列表，created_at DESC（同秒按插入顺序倒序）。
pub fn list_versions(conn: &Connection, note_id: &str, limit: i64) -> AppResult<Vec<NoteVersion>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {VERSION_COLS} FROM note_versions WHERE note_id = ?1 \
         ORDER BY created_at DESC, rowid DESC LIMIT ?2"
    ))?;
    let list = stmt
        .query_map(params![note_id, limit], row_to_version)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn get_version(conn: &Connection, version_id: &str) -> AppResult<NoteVersion> {
    conn.query_row(
        &format!("SELECT {VERSION_COLS} FROM note_versions WHERE id = ?1"),
        params![version_id],
        row_to_version,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid(format!("版本不存在: {version_id}"))
        }
        other => AppError::Db(other.to_string()),
    })
}

/// 恢复版本：先把便签当前内容快照为 pre-restore，再把版本 title/content 写回便签。
pub fn restore_version(conn: &Connection, version_id: &str) -> AppResult<Note> {
    let v = get_version(conn, version_id)?;
    notes::snapshot_version(conn, &v.note_id, "pre-restore")?;
    notes::update(
        conn,
        &notes::NoteUpdate {
            id: v.note_id.clone(),
            title: Some(v.title.clone()),
            content: Some(v.content.clone()),
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
    notes::get(conn, &v.note_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // 与 Db::open 一致：级联删除语义依赖外键约束
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        crate::db::migrations::run(&conn).unwrap();
        conn
    }

    fn upd(id: &str, title: &str) -> notes::NoteUpdate {
        notes::NoteUpdate {
            id: id.to_string(),
            title: Some(title.to_string()),
            content: Some(format!("{title} 内容")),
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
        }
    }

    #[test]
    fn snapshot_modify_snapshot_ordering_and_restore() {
        let conn = setup();
        let n = notes::create(&conn, "v0 标题", "v0 内容").unwrap();

        notes::snapshot_version(&conn, &n.id, "auto").unwrap();
        notes::update(&conn, &upd(&n.id, "v1 标题")).unwrap();
        notes::snapshot_version(&conn, &n.id, "auto").unwrap();

        let versions = list_versions(&conn, &n.id, 50).unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].title, "v1 标题", "最新版本在前");
        assert_eq!(versions[1].title, "v0 标题");
        assert_eq!(versions[1].content, "v0 内容");
        assert_eq!(versions[0].source, "auto");

        // 恢复到 v0：先 pre-restore 快照当前（v1），再写回 v0
        let restored = restore_version(&conn, &versions[1].id).unwrap();
        assert_eq!(restored.title, "v0 标题");
        assert_eq!(restored.content, "v0 内容");

        let versions = list_versions(&conn, &n.id, 50).unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].source, "pre-restore");
        assert_eq!(versions[0].title, "v1 标题", "pre-restore 记录恢复前的状态");

        // 恢复其它便签的版本应报错（版本不存在）
        assert!(matches!(
            get_version(&conn, "missing-id"),
            Err(AppError::Invalid(_))
        ));
    }

    #[test]
    fn snapshot_keeps_max_50() {
        let conn = setup();
        let n = notes::create(&conn, "初版", "初版内容").unwrap();
        for i in 0..60 {
            notes::update(&conn, &upd(&n.id, &format!("标题{i}"))).unwrap();
            notes::snapshot_version(&conn, &n.id, "auto").unwrap();
        }
        let all = list_versions(&conn, &n.id, 1000).unwrap();
        assert_eq!(all.len(), MAX_VERSIONS_PER_NOTE as usize, "最多保留 50 条");
        assert_eq!(all[0].title, "标题59", "最新在前");
        assert_eq!(all[49].title, "标题10", "最旧保留的是第 10 次快照");
    }

    #[test]
    fn list_versions_is_scoped_to_note() {
        let conn = setup();
        let a = notes::create(&conn, "A", "a").unwrap();
        let b = notes::create(&conn, "B", "b").unwrap();
        notes::snapshot_version(&conn, &a.id, "auto").unwrap();
        notes::snapshot_version(&conn, &b.id, "manual").unwrap();

        let of_a = list_versions(&conn, &a.id, 10).unwrap();
        assert_eq!(of_a.len(), 1);
        assert_eq!(of_a[0].note_id, a.id);

        // limit 生效
        notes::snapshot_version(&conn, &a.id, "auto").unwrap();
        assert_eq!(list_versions(&conn, &a.id, 1).unwrap().len(), 1);
    }
}
