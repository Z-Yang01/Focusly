pub mod images;
pub mod migrations;
pub mod models;
pub mod notes;
pub mod reminders;
pub mod settings;
pub mod shortcuts;
pub mod tags;

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

    /// 仅用于测试：直接拿连接跑迁移。
    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        Db(Mutex::new(conn)).with(|c| migrations::run(c))
    }
}
