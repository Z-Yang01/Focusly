use rusqlite::{params, Connection};

use super::models::ShortcutEntry;
use crate::error::AppResult;

pub const DEFAULT_SHORTCUTS: &[(&str, &str)] = &[
    ("toggle_notes", "Ctrl+Shift+Space"),
    ("new_note", "Ctrl+Shift+N"),
    ("focus_search", "Ctrl+Shift+F"),
];

pub fn list(conn: &Connection) -> AppResult<Vec<ShortcutEntry>> {
    let mut stmt = conn.prepare(
        "SELECT action, accelerator, enabled FROM shortcuts ORDER BY action ASC",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(ShortcutEntry {
                action: r.get(0)?,
                accelerator: r.get(1)?,
                enabled: r.get::<_, i64>(2)? != 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn set(conn: &Connection, action: &str, accelerator: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO shortcuts (action, accelerator, enabled, updated_at) \
         VALUES (?1, ?2, 1, ?3) \
         ON CONFLICT(action) DO UPDATE SET accelerator = excluded.accelerator, updated_at = excluded.updated_at",
        params![action, accelerator, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}

pub fn set_enabled(conn: &Connection, action: &str, enabled: bool) -> AppResult<()> {
    conn.execute(
        "UPDATE shortcuts SET enabled = ?2 WHERE action = ?1",
        params![action, enabled as i64],
    )?;
    Ok(())
}

pub fn reset_all(conn: &Connection) -> AppResult<()> {
    for (action, accelerator) in DEFAULT_SHORTCUTS {
        set(conn, action, accelerator)?;
        set_enabled(conn, action, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_reset() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        set(&conn, "new_note", "Ctrl+Alt+M").unwrap();
        let entries = list(&conn).unwrap();
        let nn = entries.iter().find(|e| e.action == "new_note").unwrap();
        assert_eq!(nn.accelerator, "Ctrl+Alt+M");

        reset_all(&conn).unwrap();
        let entries = list(&conn).unwrap();
        let nn = entries.iter().find(|e| e.action == "new_note").unwrap();
        assert_eq!(nn.accelerator, "Ctrl+Shift+N");
        assert!(entries.len() >= 3);
    }
}
