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
    // v2: 回收站 / 便签锁（私密、锁定、只读、缩放）/ 版本快照 / 剪贴板历史 / 保存的搜索 / FTS5 全文索引
    // 注意：readonly 用 readonly_flag 列名（readonly 有成为 SQL 关键字的风险），serde 输出名仍为 readonly
    r#"
ALTER TABLE notes ADD COLUMN deleted_at TEXT;
ALTER TABLE notes ADD COLUMN is_private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN readonly_flag INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN scale REAL;
CREATE TABLE note_versions (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  content TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT 'auto',
  created_at TEXT NOT NULL
);
CREATE INDEX idx_versions_note ON note_versions(note_id, created_at DESC);
CREATE TABLE clipboard_history (
  id TEXT PRIMARY KEY,
  content TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'text',
  created_at TEXT NOT NULL,
  pinned INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE saved_searches (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  query TEXT NOT NULL,
  created_at TEXT NOT NULL
);
CREATE VIRTUAL TABLE notes_fts USING fts5(note_id UNINDEXED, title, body, tags, tokenize='trigram');
"#,
    // v3: 窗口布局预设（name 唯一，data 为窗口快照 JSON：[[noteId,x,y,w,h],...]）
    r#"
CREATE TABLE layout_presets (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  data TEXT NOT NULL,
  created_at TEXT NOT NULL
);
"#,
    // v4: 快速捕获（速记箱）全局快捷键默认绑定
    r#"
INSERT OR IGNORE INTO shortcuts (action, accelerator, enabled, updated_at) VALUES ('quick_capture', 'Ctrl+Shift+Q', 1, '');
"#,
    // v5: 番茄钟（会话统计 + 任务元数据）+ 番茄钟全局快捷键与默认设置
    r#"
CREATE TABLE pomodoro_sessions (
    id TEXT PRIMARY KEY,
    note_id TEXT,
    task_key TEXT,
    task_text_snapshot TEXT,
    phase TEXT NOT NULL,
    planned_sec INTEGER NOT NULL,
    actual_sec INTEGER NOT NULL DEFAULT 0,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    status TEXT NOT NULL,
    interrupt_reason TEXT
);
CREATE INDEX idx_pomo_time ON pomodoro_sessions(started_at DESC);
CREATE INDEX idx_pomo_task ON pomodoro_sessions(note_id, task_key);

CREATE TABLE task_meta (
    note_id TEXT NOT NULL,
    task_key TEXT NOT NULL,
    line_text TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'todo',
    estimate_pomodoros INTEGER NOT NULL DEFAULT 0,
    completed_pomodoros INTEGER NOT NULL DEFAULT 0,
    priority TEXT,
    due_at TEXT,
    skip_date TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (note_id, task_key)
);

INSERT OR IGNORE INTO shortcuts (action, accelerator, enabled, updated_at) VALUES ('pomodoro_toggle', 'Ctrl+Shift+P', 1, '');

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('pomo_focus_min', '25'),
    ('pomo_short_min', '5'),
    ('pomo_long_min', '15'),
    ('pomo_long_every', '4'),
    ('pomo_auto_next', 'false'),
    ('pomo_sound', 'off'),
    ('pomo_force_remind', 'false');
"#,
    // v6: 便签图钉三态（normal / topmost / desktop）
    r#"
    ALTER TABLE notes ADD COLUMN pin_mode TEXT NOT NULL DEFAULT 'normal';
    "#,
    // v7: 列表主路径覆盖索引。v1 的 idx_notes_status_updated 建于 deleted_at 列出现之前，
    // "all" 视图（WHERE deleted_at IS NULL ORDER BY is_pinned DESC, updated_at DESC）只能全表扫描+排序；
    // 部分索引与该 WHERE/ORDER BY 完全匹配，回收站行不进索引，体积可控。
    r#"
    CREATE INDEX idx_notes_alive_order ON notes(is_pinned DESC, updated_at DESC) WHERE deleted_at IS NULL;
    "#,
    // v8: 今日任务时间轴（独立于便签的每日任务）+ 番茄会话绑定。
    // id 沿用全库 TEXT uuid 主键约定（与其余表一致，避免 rowid 回填管道）。
    // source_task_id：重复模板 → 当日实例的溯源（实例自身 repeat_rule='none'）。
    // start_notified：到点通知去重（0=未通知；勿扰期间保持 0，时段结束后补发）。
    r#"
    CREATE TABLE daily_tasks (
        id TEXT PRIMARY KEY,
        date TEXT NOT NULL,
        start_time TEXT,
        end_time TEXT,
        title TEXT NOT NULL,
        note TEXT,
        estimate_pomodoros INTEGER NOT NULL DEFAULT 0,
        completed_pomodoros INTEGER NOT NULL DEFAULT 0,
        priority TEXT NOT NULL DEFAULT 'medium',
        status TEXT NOT NULL DEFAULT 'todo',
        tags TEXT,
        repeat_rule TEXT NOT NULL DEFAULT 'none',
        is_private INTEGER NOT NULL DEFAULT 0,
        start_notified INTEGER NOT NULL DEFAULT 0,
        source_task_id TEXT REFERENCES daily_tasks(id) ON DELETE CASCADE,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX idx_daily_tasks_date ON daily_tasks(date, start_time);
    ALTER TABLE pomodoro_sessions ADD COLUMN daily_task_id TEXT REFERENCES daily_tasks(id) ON DELETE SET NULL;
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
    use rusqlite::params;

    #[test]
    fn migrations_apply_cleanly_and_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        run(&conn).unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);

        // settings 默认值存在
        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(theme, "system");

        // shortcuts 默认值存在（v5 起含 quick_capture 与 pomodoro_toggle）
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM shortcuts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 5);
    }

    /// 模拟 v1 旧库：只跑第一条迁移并手工把 user_version 置为 1。
    fn setup_v1(conn: &Connection) {
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
    }

    #[test]
    fn v1_to_v2_upgrade_preserves_data() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v1(&conn);

        // 旧行数据
        conn.execute(
            "INSERT INTO notes (id, title, content, status, created_at, updated_at) \
             VALUES ('n1', '旧便签', '# 旧内容\n- [ ] 任务', 'active', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tags (id, name, created_at) VALUES ('t1', '工作', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO note_tags (note_id, tag_id) VALUES ('n1', 't1')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);

        // 旧数据完整
        let (title, content, status): (String, String, String) = conn
            .query_row(
                "SELECT title, content, status FROM notes WHERE id = 'n1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "旧便签");
        assert_eq!(content, "# 旧内容\n- [ ] 任务");
        assert_eq!(status, "active");
        let tags: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM note_tags WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tags, 1);

        // 新列默认值正确
        let (is_private, locked, readonly_flag, deleted_at, scale): (i64, i64, i64, Option<String>, Option<f64>) =
            conn.query_row(
                "SELECT is_private, locked, readonly_flag, deleted_at, scale FROM notes WHERE id = 'n1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!((is_private, locked, readonly_flag), (0, 0, 0));
        assert!(deleted_at.is_none());
        assert!(scale.is_none());

        // fts 表存在且为空
        let fts_tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'notes_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_tables, 1);
        let fts_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts_rows, 0);

        // 新表存在
        for table in ["note_versions", "clipboard_history", "saved_searches"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "缺少表 {table}");
        }
    }

    #[test]
    fn upgraded_v1_db_supports_active_list() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v1(&conn);
        conn.execute(
            "INSERT INTO notes (id, title, content, created_at, updated_at) \
             VALUES ('n1', '存活便签', '内容', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let list = crate::db::notes::list(&conn, "active").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].note.title, "存活便签");
        assert!(!list[0].note.is_private);
        assert!(!list[0].note.locked);
        assert!(!list[0].note.readonly);
        assert!(list[0].note.deleted_at.is_none());
        assert_eq!(crate::db::notes::list(&conn, "trash").unwrap().len(), 0);
    }

    /// 模拟 v2 旧库：跑前两条迁移并手工把 user_version 置为 2。
    fn setup_v2(conn: &Connection) {
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.execute_batch(MIGRATIONS[1]).unwrap();
        conn.pragma_update(None, "user_version", 2).unwrap();
    }

    #[test]
    fn v2_to_v3_upgrade_creates_layout_presets() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v2(&conn);

        // v2 时代的旧行数据
        conn.execute(
            "INSERT INTO notes (id, title, content, created_at, updated_at) \
             VALUES ('n1', '旧便签', '内容', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);

        // layout_presets 表存在
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'layout_presets'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);

        // 表结构可用：插入 + name 唯一约束生效
        conn.execute(
            "INSERT INTO layout_presets (id, name, data, created_at) VALUES ('p1', '工作', '[]', '')",
            [],
        )
        .unwrap();
        let dup = conn.execute(
            "INSERT INTO layout_presets (id, name, data, created_at) VALUES ('p2', '工作', '[]', '')",
            [],
        );
        assert!(dup.is_err(), "name 唯一约束应生效");

        // 旧数据完整
        let title: String = conn
            .query_row("SELECT title FROM notes WHERE id = 'n1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "旧便签");
    }

    /// 模拟 v3 旧库：跑前三条迁移并手工把 user_version 置为 3。
    fn setup_v3(conn: &Connection) {
        for sql in MIGRATIONS.iter().take(3) {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 3).unwrap();
    }

    #[test]
    fn v3_to_v4_upgrade_adds_quick_capture_shortcut() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v3(&conn);
        run(&conn).unwrap();
        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);
        let accel: String = conn
            .query_row(
                "SELECT accelerator FROM shortcuts WHERE action = 'quick_capture'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(accel, "Ctrl+Shift+Q");
    }

    /// 模拟 v4 旧库：跑前四条迁移并手工把 user_version 置为 4。
    fn setup_v4(conn: &Connection) {
        for sql in MIGRATIONS.iter().take(4) {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 4).unwrap();
    }

    #[test]
    fn v4_to_v5_upgrade_creates_pomodoro_tables_and_defaults() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v4(&conn);

        // v4 时代的旧行数据
        conn.execute(
            "INSERT INTO notes (id, title, content, created_at, updated_at) \
             VALUES ('n1', '旧便签', '内容', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();

        run(&conn).unwrap();

        let v: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, MIGRATIONS.len() as i64);

        // 新表存在
        for table in ["pomodoro_sessions", "task_meta"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "缺少表 {table}");
        }

        // 新索引存在
        for idx in ["idx_pomo_time", "idx_pomo_task"] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    params![idx],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "缺少索引 {idx}");
        }

        // settings 7 个新键
        for key in [
            "pomo_focus_min",
            "pomo_short_min",
            "pomo_long_min",
            "pomo_long_every",
            "pomo_auto_next",
            "pomo_sound",
            "pomo_force_remind",
        ] {
            let n: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM settings WHERE key = ?1",
                    params![key],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "缺少设置 {key}");
        }

        // shortcuts 新动作
        let accel: String = conn
            .query_row(
                "SELECT accelerator FROM shortcuts WHERE action = 'pomodoro_toggle'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(accel, "Ctrl+Shift+P");

        // 旧数据完整
        let title: String = conn
            .query_row("SELECT title FROM notes WHERE id = 'n1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "旧便签");
    }
}
