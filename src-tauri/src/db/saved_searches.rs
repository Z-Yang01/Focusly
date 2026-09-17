//! 保存的搜索（saved_searches 表，name 唯一，同名 UPSERT 覆盖 query）。

use rusqlite::{params, Connection, Row};
use uuid::Uuid;

use crate::db::models::SavedSearch;
use crate::error::{AppError, AppResult};

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn row_to_saved(r: &Row) -> rusqlite::Result<SavedSearch> {
    Ok(SavedSearch {
        id: r.get("id")?,
        name: r.get("name")?,
        query: r.get("query")?,
        created_at: r.get("created_at")?,
    })
}

const COLS: &str = "id, name, query, created_at";

/// 保存搜索：同名覆盖 query（UPSERT by name），返回保存后的记录。
pub fn save(conn: &Connection, name: &str, query: &str) -> AppResult<SavedSearch> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("搜索名称不能为空".into()));
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO saved_searches (id, name, query, created_at) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(name) DO UPDATE SET query = excluded.query",
        params![id, name, query, now()],
    )?;
    get_by_name(conn, name)
}

fn get_by_name(conn: &Connection, name: &str) -> AppResult<SavedSearch> {
    conn.query_row(
        &format!("SELECT {COLS} FROM saved_searches WHERE name = ?1"),
        params![name],
        row_to_saved,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid(format!("保存的搜索不存在: {name}"))
        }
        other => AppError::Db(other.to_string()),
    })
}

/// 按 id 取单条（与 get_by_name 对偶的 DAO 契约；供未来按 id 定位/编辑使用）
#[allow(dead_code)]
pub fn get(conn: &Connection, id: &str) -> AppResult<SavedSearch> {
    conn.query_row(
        &format!("SELECT {COLS} FROM saved_searches WHERE id = ?1"),
        params![id],
        row_to_saved,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid(format!("保存的搜索不存在: {id}"))
        }
        other => AppError::Db(other.to_string()),
    })
}

/// 全部保存的搜索，按名称排序。
pub fn list(conn: &Connection) -> AppResult<Vec<SavedSearch>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM saved_searches ORDER BY name ASC"
    ))?;
    let list = stmt
        .query_map([], row_to_saved)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM saved_searches WHERE id = ?1", params![id])?;
    Ok(())
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
    fn save_upserts_by_name() {
        let conn = setup();
        let s1 = save(&conn, "工作搜索", "tag:工作").unwrap();
        assert_eq!(s1.name, "工作搜索");
        assert_eq!(s1.query, "tag:工作");

        // 同名保存覆盖 query，不新增行，保留原 id
        let s2 = save(&conn, "工作搜索", "tag:工作 is:todo").unwrap();
        assert_eq!(s2.id, s1.id, "UPSERT 保留原 id");
        assert_eq!(s2.query, "tag:工作 is:todo");
        assert_eq!(list(&conn).unwrap().len(), 1);

        save(&conn, "MediaFlow", "MediaFlow").unwrap();
        let all = list(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "MediaFlow", "按名称排序");

        assert_eq!(get(&conn, &s1.id).unwrap().query, "tag:工作 is:todo");
    }

    #[test]
    fn save_rejects_blank_name() {
        let conn = setup();
        assert!(matches!(save(&conn, "   ", "q"), Err(AppError::Invalid(_))));
    }

    #[test]
    fn delete_removes_row() {
        let conn = setup();
        let s = save(&conn, "临时", "q").unwrap();
        delete(&conn, &s.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        assert!(matches!(get(&conn, &s.id), Err(AppError::Invalid(_))));
        delete(&conn, &s.id).unwrap(); // 幂等
    }
}
