//! 剪贴板历史（clipboard_history 表）。
//! 最多保留 100 条未固定记录，超出时淘汰最旧的非 pinned 条目；pinned 永不淘汰。

use rusqlite::{params, Connection, Row};
use uuid::Uuid;

use crate::db::models::ClipboardEntry;
use crate::error::{AppError, AppResult};

/// 未固定记录的保留上限
pub const MAX_ITEMS: i64 = 100;

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn row_to_entry(r: &Row) -> rusqlite::Result<ClipboardEntry> {
    Ok(ClipboardEntry {
        id: r.get("id")?,
        content: r.get("content")?,
        kind: r.get("kind")?,
        created_at: r.get("created_at")?,
        pinned: r.get::<_, i64>("pinned")? != 0,
    })
}

const ENTRY_COLS: &str = "id, content, kind, created_at, pinned";

/// 新增一条剪贴板记录并执行容量清理。
pub fn add(conn: &Connection, content: &str, kind: &str) -> AppResult<ClipboardEntry> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO clipboard_history (id, content, kind, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![id, content, kind, now()],
    )?;
    prune(conn)?;
    get(conn, &id)
}

/// 只保留最近 MAX_ITEMS 条未固定记录（同秒按 rowid 插入顺序决胜）。
fn prune(conn: &Connection) -> AppResult<()> {
    conn.execute(
        "DELETE FROM clipboard_history WHERE pinned = 0 AND id NOT IN (\
             SELECT id FROM clipboard_history WHERE pinned = 0 \
             ORDER BY created_at DESC, rowid DESC LIMIT ?1\
         )",
        params![MAX_ITEMS],
    )?;
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<ClipboardEntry> {
    conn.query_row(
        &format!("SELECT {ENTRY_COLS} FROM clipboard_history WHERE id = ?1"),
        params![id],
        row_to_entry,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid(format!("剪贴板记录不存在: {id}"))
        }
        other => AppError::Db(other.to_string()),
    })
}

/// 历史列表（pinned 优先，其余按时间倒序），limit 限制返回条数。
pub fn list(conn: &Connection, limit: i64) -> AppResult<Vec<ClipboardEntry>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {ENTRY_COLS} FROM clipboard_history \
         ORDER BY pinned DESC, created_at DESC, rowid DESC LIMIT ?1"
    ))?;
    let list = stmt
        .query_map(params![limit], row_to_entry)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn remove(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM clipboard_history WHERE id = ?1", params![id])?;
    Ok(())
}

/// 清空全部历史（含 pinned），返回删除的条数。
pub fn clear(conn: &Connection) -> AppResult<usize> {
    Ok(conn.execute("DELETE FROM clipboard_history", [])?)
}

/// 切换固定状态，返回切换后的记录。
pub fn toggle_pin(conn: &Connection, id: &str) -> AppResult<ClipboardEntry> {
    conn.execute(
        "UPDATE clipboard_history SET pinned = 1 - pinned WHERE id = ?1",
        params![id],
    )?;
    get(conn, id)
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

    #[test]
    fn add_prunes_oldest_unpinned() {
        let conn = setup();
        for i in 0..(MAX_ITEMS + 5) {
            add(&conn, &format!("内容{i}"), "text").unwrap();
        }
        let all = list(&conn, 1000).unwrap();
        assert_eq!(all.len(), MAX_ITEMS as usize, "只保留最近 100 条未固定记录");
        assert_eq!(all[0].content, format!("内容{}", MAX_ITEMS + 4), "最新在前");
        assert_eq!(all[99].content, "内容5", "最旧的 5 条被淘汰");
    }

    #[test]
    fn pinned_survives_pruning() {
        let conn = setup();
        let keep = add(&conn, "固定我", "text").unwrap();
        let pinned = toggle_pin(&conn, &keep.id).unwrap();
        assert!(pinned.pinned);

        for i in 0..(MAX_ITEMS + 10) {
            add(&conn, &format!("再塞{i}"), "text").unwrap();
        }
        let all = list(&conn, 1000).unwrap();
        assert_eq!(
            all.len(),
            (MAX_ITEMS + 1) as usize,
            "100 条未固定 + 1 条固定"
        );
        assert_eq!(all[0].id, keep.id, "pinned 排最前");
        assert!(all.iter().any(|e| e.id == keep.id), "pinned 不参与淘汰");
        assert_eq!(all[1].content, format!("再塞{}", MAX_ITEMS + 9));

        // 再 toggle 回来
        let unpinned = toggle_pin(&conn, &keep.id).unwrap();
        assert!(!unpinned.pinned);
    }

    #[test]
    fn remove_clear_and_limit() {
        let conn = setup();
        for i in 0..10 {
            add(&conn, &format!("c{i}"), "text").unwrap();
        }
        assert_eq!(list(&conn, 3).unwrap().len(), 3, "limit 生效");

        let first = list(&conn, 1).unwrap().remove(0);
        remove(&conn, &first.id).unwrap();
        assert_eq!(list(&conn, 1000).unwrap().len(), 9);

        let removed = clear(&conn).unwrap();
        assert_eq!(removed, 9);
        assert!(list(&conn, 1000).unwrap().is_empty());

        assert!(matches!(get(&conn, &first.id), Err(AppError::Invalid(_))));
    }

    #[test]
    fn kind_defaults_to_text_column() {
        let conn = setup();
        let e = add(&conn, "图片数据", "image").unwrap();
        assert_eq!(e.kind, "image");
        assert!(!e.pinned);
    }
}
