pub mod clipboard;
pub mod images;
pub mod layouts;
pub mod migrations;
pub mod missed;
pub mod models;
pub mod notes;
pub mod pomodoro_sessions;
pub mod reminders;
pub mod saved_searches;
pub mod search;
pub mod settings;
pub mod shortcuts;
pub mod tags;
pub mod task_meta;
pub mod todos_view;
pub mod versions;

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use crate::error::{AppError, AppResult};

/// SQLite 连接持有者。所有数据库访问都通过 `Db::with`，
/// 业务代码不允许散落 SQL（SQL 只存在于 migrations 与 DAO 文件）。
pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)
            .map_err(|e| AppError::Db(format!("无法打开数据库 {}: {e}", path.display())))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_millis(3000))?;
        Ok(Db(Mutex::new(conn)))
    }

    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let conn = self
            .0
            .lock()
            .map_err(|_| AppError::Db("数据库连接被占用（锁中毒）".into()))?;
        f(&conn)
    }

    /// 在单个事务中执行多步写操作：`f` 返回 `Err`（含 `?` 提前返回）时整体回滚，
    /// `Ok` 时提交。`f` 内不得再经 `self.with/tx` 嵌套取连接（Mutex 不可重入，会死锁）。
    pub fn tx<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let conn = self
            .0
            .lock()
            .map_err(|_| AppError::Db("数据库连接被占用（锁中毒）".into()))?;
        let tx = conn.unchecked_transaction()?;
        // f 出错时先行返回，tx drop 触发回滚
        let out = f(&conn)?;
        tx.commit()?;
        Ok(out)
    }

    /// 仅用于测试：直接拿连接跑迁移。
    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Db(Mutex::new(conn));
        db.with(|c| migrations::run(c))?;
        Ok(db)
    }
}

/// 诊断用数据库健康摘要（export_diagnostics 用；不含任何便签内容数据）。
pub struct DbHealth {
    pub integrity: String,
    pub notes_active: i64,
    pub notes_archived: i64,
    pub notes_trashed: i64,
    pub sessions_completed: i64,
    pub reminders_pending: i64,
}

/// 数据库健康摘要（只读；单项查询失败按既有诊断口径降级，不整体报错）。
pub fn health(conn: &Connection) -> AppResult<DbHealth> {
    let count = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(-1) };
    Ok(DbHealth {
        integrity: conn
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap_or_else(|_| "query_failed".into()),
        notes_active: count(
            "SELECT COUNT(*) FROM notes WHERE status='active' AND deleted_at IS NULL",
        ),
        notes_archived: count("SELECT COUNT(*) FROM notes WHERE status='archived'"),
        notes_trashed: count("SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL"),
        sessions_completed: count(
            "SELECT COUNT(*) FROM pomodoro_sessions WHERE status='completed'",
        ),
        reminders_pending: count("SELECT COUNT(*) FROM reminders WHERE status='pending'"),
    })
}
