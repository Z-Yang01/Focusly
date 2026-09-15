//! 窗口布局预设（layout_presets 表，name 唯一，同名 UPSERT 覆盖 data）。

use rusqlite::{params, Connection, Row};
use uuid::Uuid;

use crate::db::models::LayoutPreset;
use crate::error::{AppError, AppResult};

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn row_to_preset(r: &Row) -> rusqlite::Result<LayoutPreset> {
    Ok(LayoutPreset {
        id: r.get("id")?,
        name: r.get("name")?,
        data: r.get("data")?,
        created_at: r.get("created_at")?,
    })
}

const COLS: &str = "id, name, data, created_at";

/// 保存布局预设：同名覆盖 data（UPSERT by name），返回保存后的记录。
pub fn save(conn: &Connection, name: &str, data_json: &str) -> AppResult<LayoutPreset> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Invalid("布局名称不能为空".into()));
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO layout_presets (id, name, data, created_at) VALUES (?1, ?2, ?3, ?4) \
         ON CONFLICT(name) DO UPDATE SET data = excluded.data",
        params![id, name, data_json, now()],
    )?;
    get_by_name(conn, name)
}

fn get_by_name(conn: &Connection, name: &str) -> AppResult<LayoutPreset> {
    conn.query_row(
        &format!("SELECT {COLS} FROM layout_presets WHERE name = ?1"),
        params![name],
        row_to_preset,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Invalid(format!("布局预设不存在: {name}"))
        }
        other => AppError::Db(other.to_string()),
    })
}

pub fn get(conn: &Connection, id: &str) -> AppResult<LayoutPreset> {
    conn.query_row(
        &format!("SELECT {COLS} FROM layout_presets WHERE id = ?1"),
        params![id],
        row_to_preset,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::Invalid(format!("布局预设不存在: {id}")),
        other => AppError::Db(other.to_string()),
    })
}

/// 全部布局预设，按名称排序。
pub fn list(conn: &Connection) -> AppResult<Vec<LayoutPreset>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM layout_presets ORDER BY name ASC"
    ))?;
    let list = stmt
        .query_map([], row_to_preset)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM layout_presets WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        crate::db::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn save_upserts_by_name() {
        let conn = setup();
        let data1 = r#"[["n1",0,0,300,400]]"#;
        let p1 = save(&conn, "工作布局", data1).unwrap();
        assert_eq!(p1.name, "工作布局");
        assert_eq!(p1.data, data1);

        // 同名保存覆盖 data，不新增行，保留原 id
        let data2 = r#"[["n1",10,20,300,400],["n2",320,20,300,400]]"#;
        let p2 = save(&conn, "工作布局", data2).unwrap();
        assert_eq!(p2.id, p1.id, "UPSERT 保留原 id");
        assert_eq!(p2.data, data2);
        assert_eq!(list(&conn).unwrap().len(), 1);

        save(&conn, "全屏写作", data1).unwrap();
        let all = list(&conn).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].name, "全屏写作", "按名称排序");
        assert_eq!(all[1].name, "工作布局");

        assert_eq!(get(&conn, &p1.id).unwrap().data, data2);
        assert_eq!(get_by_name(&conn, "工作布局").unwrap().id, p1.id);
    }

    #[test]
    fn save_rejects_blank_name() {
        let conn = setup();
        assert!(matches!(
            save(&conn, "   ", "[]"),
            Err(AppError::Invalid(_))
        ));
        assert!(matches!(save(&conn, "", "[]"), Err(AppError::Invalid(_))));
    }

    #[test]
    fn delete_removes_row() {
        let conn = setup();
        let p = save(&conn, "临时布局", "[]").unwrap();
        delete(&conn, &p.id).unwrap();
        assert!(list(&conn).unwrap().is_empty());
        assert!(matches!(get(&conn, &p.id), Err(AppError::Invalid(_))));
        delete(&conn, &p.id).unwrap(); // 幂等
    }
}
