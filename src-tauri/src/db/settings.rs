use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::AppResult;

pub const DEFAULTS: &[(&str, &str)] = &[
    ("theme", "system"),      // system | light | dark
    ("close_action", "tray"), // tray | quit
    ("autostart", "false"),
    ("start_minimized", "true"),
    ("launch_show_notes", "true"),
    // 番茄钟（v5 迁移默认值；读取端见 pomodoro.rs settings_from）
    ("pomo_focus_min", "25"),       // 专注时长（分钟）
    ("pomo_short_min", "5"),        // 短休时长
    ("pomo_long_min", "15"),        // 长休时长
    ("pomo_long_every", "4"),       // 每 N 个专注进入长休
    ("pomo_auto_next", "false"),    // 阶段结束自动开始下一阶段
    ("pomo_sound", "off"),          // 提示音（MVP 仅占位）
    ("pomo_force_remind", "false"), // 勿扰时段仍强制提醒
];

pub fn get_all(conn: &Connection) -> AppResult<HashMap<String, String>> {
    let mut map: HashMap<String, String> = DEFAULTS
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<(String, String)>>>()?;
    for (k, v) in rows {
        map.insert(k, v);
    }
    Ok(map)
}

pub fn get(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    let r = conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    );
    match r {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_overrides_default() {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        assert_eq!(get(&conn, "theme").unwrap().as_deref(), Some("system"));
        set(&conn, "theme", "dark").unwrap();
        assert_eq!(get(&conn, "theme").unwrap().as_deref(), Some("dark"));
        let all = get_all(&conn).unwrap();
        assert_eq!(all.get("theme").map(String::as_str), Some("dark"));
        assert_eq!(all.len(), DEFAULTS.len());
    }
}
