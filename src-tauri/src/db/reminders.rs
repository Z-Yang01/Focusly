use rusqlite::{params, Connection};
use uuid::Uuid;

use super::models::{Reminder, RepeatType};
use crate::error::{AppError, AppResult};

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn row_to_reminder(r: &Row) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: r.get("id")?,
        note_id: r.get("note_id")?,
        remind_at: r.get("remind_at")?,
        repeat_type: r.get("repeat_type")?,
        status: r.get("status")?,
        created_at: r.get("created_at")?,
        triggered_at: r.get("triggered_at")?,
    })
}

use rusqlite::Row;

pub fn create(
    conn: &Connection,
    note_id: &str,
    remind_at: &str,
    repeat: RepeatType,
) -> AppResult<Reminder> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO reminders (id, note_id, remind_at, repeat_type, status, created_at) \
         VALUES (?1, ?2, ?3, ?4, 'pending', ?5)",
        params![id, note_id, remind_at, repeat.as_str(), now()],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Reminder> {
    conn.query_row(
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at \
         FROM reminders WHERE id=?1",
        params![id],
        row_to_reminder,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::Invalid(format!("提醒不存在: {id}")),
        other => AppError::Db(other.to_string()),
    })
}

/// 某便签当前 pending 的提醒（按时间升序）。
pub fn pending_for_note(conn: &Connection, note_id: &str) -> AppResult<Vec<Reminder>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at \
         FROM reminders WHERE note_id=?1 AND status='pending' ORDER BY remind_at ASC",
    )?;
    let list = stmt
        .query_map(params![note_id], row_to_reminder)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

/// 所有 pending 提醒。
pub fn all_pending(conn: &Connection) -> AppResult<Vec<Reminder>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at \
         FROM reminders WHERE status='pending' ORDER BY remind_at ASC",
    )?;
    let list = stmt
        .query_map([], row_to_reminder)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn set_status(conn: &Connection, id: &str, status: &str, mark_triggered: bool) -> AppResult<()> {
    if mark_triggered {
        conn.execute(
            "UPDATE reminders SET status=?2, triggered_at=?3 WHERE id=?1",
            params![id, status, now()],
        )?;
    } else {
        conn.execute(
            "UPDATE reminders SET status=?2 WHERE id=?1",
            params![id, status],
        )?;
    }
    Ok(())
}

/// 推迟：更新触发时间并回到 pending。
pub fn snooze_to(conn: &Connection, id: &str, remind_at: &str) -> AppResult<Reminder> {
    conn.execute(
        "UPDATE reminders SET remind_at=?2, status='pending', triggered_at=NULL WHERE id=?1",
        params![id, remind_at],
    )?;
    get(conn, id)
}

/// 删除某便签的 pending 提醒（重新设置提醒前调用）。
pub fn cancel_pending_for_note(conn: &Connection, note_id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE reminders SET status='cancelled' WHERE note_id=?1 AND status='pending'",
        params![note_id],
    )?;
    Ok(())
}

pub fn list_for_note(conn: &Connection, note_id: &str, limit: i64) -> AppResult<Vec<Reminder>> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at \
         FROM reminders WHERE note_id=?1 ORDER BY remind_at DESC LIMIT ?2",
    )?;
    let list = stmt
        .query_map(params![note_id, limit], row_to_reminder)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

/// 计算重复提醒的下一次触发时间。
/// 纯函数，单测覆盖。
pub fn next_occurrence(
    repeat: RepeatType,
    remind_at: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{Datelike, Duration, TimeZone};
    match repeat {
        RepeatType::Once => None,
        RepeatType::Daily => {
            let mut next = remind_at;
            while next <= now {
                next += Duration::days(1);
            }
            Some(next)
        }
        RepeatType::Weekly => {
            let mut next = remind_at;
            while next <= now {
                next += Duration::weeks(1);
            }
            Some(next)
        }
        RepeatType::Weekdays => {
            let mut next = remind_at;
            // 从 remind_at 的时刻开始推进，跳过周末，直到大于 now
            loop {
                next += Duration::days(1);
                let wd = next.weekday();
                if wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun {
                    continue;
                }
                if next > now {
                    return Some(TimeZone::from_utc_datetime(&chrono::Utc, &next.naive_utc()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn create_and_status_flow() {
        let conn = setup();
        let notes = super::super::notes;
        let n = notes::create(&conn, "t", "c").unwrap();
        let r = create(&conn, &n.id, "2026-09-16T09:00:00Z", RepeatType::Once).unwrap();
        assert_eq!(r.status, "pending");
        assert_eq!(pending_for_note(&conn, &n.id).unwrap().len(), 1);

        set_status(&conn, &r.id, "done", false).unwrap();
        assert_eq!(get(&conn, &r.id).unwrap().status, "done");
        assert_eq!(pending_for_note(&conn, &n.id).unwrap().len(), 0);

        let r2 = snooze_to(&conn, &r.id, "2026-09-16T10:00:00Z").unwrap();
        assert_eq!(r2.status, "pending");
        assert!(r2.triggered_at.is_none(), "snooze 清除触发标记");
    }

    #[test]
    fn repeat_next_occurrence() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 9, 15, 12, 0, 0).unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 9, 10, 9, 0, 0).unwrap();

        assert_eq!(next_occurrence(RepeatType::Once, at, now), None);

        let daily = next_occurrence(RepeatType::Daily, at, now).unwrap();
        assert_eq!(daily, chrono::Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap());

        let weekly = next_occurrence(RepeatType::Weekly, at, now).unwrap();
        assert_eq!(weekly, chrono::Utc.with_ymd_and_hms(2026, 9, 17, 9, 0, 0).unwrap());

        // 2026-09-12 是周六：工作日提醒从周五(9/11)推进到周一(9/14) 09:00
        let fri = chrono::Utc.with_ymd_and_hms(2026, 9, 11, 9, 0, 0).unwrap();
        let wd = next_occurrence(RepeatType::Weekdays, fri, now).unwrap();
        assert_eq!(wd, chrono::Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap());
    }
}
