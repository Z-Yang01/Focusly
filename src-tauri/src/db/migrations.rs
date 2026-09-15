/// 数据库迁移。SQL 只允许出现在这里，禁止散落在业务代码。
/// 使用 PRAGMA user_version 追踪版本。
pub const MIGRATIONS: &[&str] = &[
    // v1: 初始 schema
    r#"
    CREATE TABLE notes (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL DEFAULT '',
        content TEXT NOT NULL DEFAULT '',
        content_format TEXT NOT NULL DEFAULT 'markdown',
        status TEXT NOT NULL DEFAULT 'active',
        is_pinned INTEGER NOT NULL DEFAULT 0,
        is_always_on_top INTEGER NOT NULL DEFAULT 0,
        show_on_all_desktops INTEGER NOT NULL DEFAULT 0,
        desktop_pin_state TEXT NOT NULL DEFAULT 'off',
        fullscreen_behavior TEXT NOT NULL DEFAULT 'normal',
        x INTEGER,
        y INTEGER,
        width INTEGER,
        height INTEGER,
        monitor_id TEXT,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        archived_at TEXT
    );
    CREATE INDEX idx_notes_status_updated ON notes(status, is_pinned DESC, updated_at DESC);

    CREATE TABLE note_images (
        id TEXT PRIMARY KEY,
        note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        path TEXT NOT NULL,
        filename TEXT NOT NULL,
        width INTEGER,
        height INTEGER,
        created_at TEXT NOT NULL
    );
    CREATE INDEX idx_note_images_note ON note_images(note_id);

    CREATE TABLE reminders (
        id TEXT PRIMARY KEY,
        note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        remind_at TEXT NOT NULL,
        repeat_type TEXT NOT NULL DEFAULT 'once',
        status TEXT NOT NULL DEFAULT 'pending',
        created_at TEXT NOT NULL,
        triggered_at TEXT
    );
    CREATE INDEX idx_reminders_pending ON reminders(status, remind_at);

    CREATE TABLE tags (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL UNIQUE,
        created_at TEXT NOT NULL
    );

    CREATE TABLE note_tags (
        note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
        tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (note_id, tag_id)
    );
    CREATE INDEX idx_note_tags_tag ON note_tags(tag_id);

    CREATE TABLE shortcuts (
        action TEXT PRIMARY KEY,
        accelerator TEXT NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    INSERT OR IGNORE INTO settings (key, value) VALUES
        ('theme', 'system'),
        ('close_action', 'tray'),
        ('autostart', 'false'),
        ('start_minimized', 'true'),
        ('launch_show_notes', 'true');

    INSERT OR IGNORE INTO shortcuts (action, accelerator, enabled, updated_at) VALUES
        ('toggle_notes', 'Ctrl+Shift+Space', 1, ''),
        ('new_note', 'Ctrl+Shift+N', 1, ''),
        ('focus_search', 'Ctrl+Shift+F', 1, '');
    "#,
];

use rusqlite::Connection;

use crate::error::AppResult;

pub fn run(conn: &Connection) -> AppResult<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate() {
        let version = (i + 1) as i64;
        if version <= current {
            continue;
        }
        conn.execute_batch(sql)?;
        conn.pragma_update(None, "user_version", version)?;
        log::info!("数据库迁移完成: v{version}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_cleanly_and_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);

        // settings 默认值存在
        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(theme, "system");

        // shortcuts 默认值存在
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shortcuts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }
}
