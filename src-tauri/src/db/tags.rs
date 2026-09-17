use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::error::AppResult;

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn tags_for_note(conn: &Connection, note_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tags t \
         JOIN note_tags nt ON nt.tag_id = t.id \
         WHERE nt.note_id = ?1 ORDER BY t.name ASC",
    )?;
    let names = stmt
        .query_map(params![note_id], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names)
}

pub fn all_tags_with_counts(conn: &Connection) -> AppResult<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT t.name, COUNT(nt.note_id) AS cnt FROM tags t \
         LEFT JOIN note_tags nt ON nt.tag_id = t.id \
         JOIN notes n ON n.id = nt.note_id AND n.status = 'active' \
         GROUP BY t.id ORDER BY cnt DESC, t.name ASC",
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn attach_tag(conn: &Connection, note_id: &str, name: &str) -> AppResult<Vec<String>> {
    let name = name.trim().trim_start_matches('#').to_string();
    if name.is_empty() {
        return Err(crate::error::AppError::Invalid("标签名不能为空".into()));
    }
    if name.len() > 32 {
        return Err(crate::error::AppError::Invalid(
            "标签名过长（最多 32 字符）".into(),
        ));
    }
    let tag_id: String =
        match conn.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| {
            r.get(0)
        }) {
            Ok(id) => id,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let id = Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO tags (id, name, created_at) VALUES (?1, ?2, ?3)",
                    params![id, name, now()],
                )?;
                id
            }
            Err(e) => return Err(e.into()),
        };
    conn.execute(
        "INSERT OR IGNORE INTO note_tags (note_id, tag_id) VALUES (?1, ?2)",
        params![note_id, tag_id],
    )?;
    tags_for_note(conn, note_id)
}

pub fn detach_tag(conn: &Connection, note_id: &str, name: &str) -> AppResult<Vec<String>> {
    conn.execute(
        "DELETE FROM note_tags WHERE note_id=?1 AND tag_id=(SELECT id FROM tags WHERE name=?2)",
        params![note_id, name],
    )?;
    // 清理孤儿标签
    conn.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM note_tags)",
        [],
    )?;
    tags_for_note(conn, note_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_detach_cycle() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        let n = super::super::notes::create(&conn, "t", "c").unwrap();

        let tags = attach_tag(&conn, &n.id, "#工作").unwrap();
        assert_eq!(tags, vec!["工作"]);
        attach_tag(&conn, &n.id, "工作").unwrap(); // 幂等
        attach_tag(&conn, &n.id, "学习").unwrap();
        assert_eq!(tags_for_note(&conn, &n.id).unwrap().len(), 2);

        let all = all_tags_with_counts(&conn).unwrap();
        assert_eq!(all.len(), 2);

        detach_tag(&conn, &n.id, "工作").unwrap();
        assert_eq!(tags_for_note(&conn, &n.id).unwrap(), vec!["学习"]);

        // 便签删除后标签被清理
        super::super::notes::delete(&conn, &n.id).unwrap();
        let all = all_tags_with_counts(&conn).unwrap();
        assert!(all.is_empty(), "孤儿标签应被清理");
    }
}
