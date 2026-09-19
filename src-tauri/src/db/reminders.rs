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
        anchor_at: r.get("anchor_at")?,
    })
}

use rusqlite::Row;

pub fn create(
    conn: &Connection,
    note_id: &str,
    remind_at: &str,
    repeat: RepeatType,
) -> AppResult<Reminder> {
    create_with_anchor(conn, note_id, remind_at, repeat, remind_at)
}

/// 带原始锚点建行。月/年重复链式补建时必须透传首次的 anchor_at，
/// 否则月末 clamp 后日号会永久漂移（31→28→28）；anchor_at 为空串时按 NULL 落库。
pub fn create_with_anchor(
    conn: &Connection,
    note_id: &str,
    remind_at: &str,
    repeat: RepeatType,
    anchor_at: &str,
) -> AppResult<Reminder> {
    let id = Uuid::new_v4().to_string();
    let anchor: Option<&str> = if anchor_at.is_empty() {
        None
    } else {
        Some(anchor_at)
    };
    conn.execute(
        "INSERT INTO reminders (id, note_id, remind_at, repeat_type, status, created_at, anchor_at) \
         VALUES (?1, ?2, ?3, ?4, 'pending', ?5, ?6)",
        params![id, note_id, remind_at, repeat.as_str(), now(), anchor],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Reminder> {
    conn.query_row(
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at, anchor_at \
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
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at, anchor_at \
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
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at, anchor_at \
         FROM reminders WHERE status='pending' ORDER BY remind_at ASC",
    )?;
    let list = stmt
        .query_map([], row_to_reminder)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

pub fn set_status(
    conn: &Connection,
    id: &str,
    status: &str,
    mark_triggered: bool,
) -> AppResult<()> {
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
        "SELECT id, note_id, remind_at, repeat_type, status, created_at, triggered_at, anchor_at \
         FROM reminders WHERE note_id=?1 ORDER BY remind_at DESC LIMIT ?2",
    )?;
    let list = stmt
        .query_map(params![note_id, limit], row_to_reminder)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(list)
}

/// 计算重复提醒的下一次触发时间。
/// `anchor`：月/年重复的原始锚点（首次创建时的 remind_at）——必须从锚点重算，
/// 若从"刚触发的那次"推进，月末 clamp 后日号会永久漂移（31→28→28）。其余规则忽略 anchor。
/// 月/年推进在 UTC 时刻上做，跨夏令时后本地触发时刻可能漂移 ±1 小时——
/// 与 daily/weekly 的整日推进口径一致（全库既定取舍）。
/// 纯函数，单测覆盖。
pub fn next_occurrence(
    repeat: RepeatType,
    remind_at: chrono::DateTime<chrono::Utc>,
    anchor: chrono::DateTime<chrono::Utc>,
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
        // 月/年重复按锚点"日号"推进：目标月无该日 clamp 到当月最后一天
        // （如 1-31 → 2-28，闰年锚 2-29 → 平年 2-28）。
        // 不能从 remind_at 连续推进——clamp 后再推进会丢失原日号（31→28→28 漂移），
        // 必须始终从原始锚点 anchor 重算（anchor 由 reminders.anchor_at 持久化并跨轮透传）。
        RepeatType::Monthly => {
            if remind_at > now {
                return Some(remind_at);
            }
            next_by_months(anchor, 1, now)
        }
        RepeatType::Yearly => {
            if remind_at > now {
                return Some(remind_at);
            }
            next_by_months(anchor, 12, now)
        }
    }
}

/// 按锚点年月 + k×step_months 重算下一次触发（锚点日号对目标月 clamp 到月末），
/// 返回第一个严格大于 now 的候选。时刻（时/分/秒）保持锚点值。
/// 锚点本身尚未到期（remind_at > now）的情形由调用方语义覆盖：锚点行就是下一次触发。
fn next_by_months(
    anchor: chrono::DateTime<chrono::Utc>,
    step_months: u32,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<chrono::DateTime<chrono::Utc>> {
    use super::daily_tasks::days_in_month;
    use chrono::{Datelike, TimeZone, Timelike};
    let mut year = anchor.year();
    let mut month = anchor.month();
    loop {
        let total = year * 12 + (month as i32 - 1) + step_months as i32;
        year = total.div_euclid(12);
        month = total.rem_euclid(12) as u32 + 1;
        let day = anchor.day().min(days_in_month(year, month));
        let t = anchor.time();
        let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(
            t.hour(),
            t.minute(),
            t.second(),
        )?;
        let cand = TimeZone::from_utc_datetime(&chrono::Utc, &naive);
        if cand > now {
            return Some(cand);
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
        let n = super::super::notes::create(&conn, "t", "c").unwrap();
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

        assert_eq!(next_occurrence(RepeatType::Once, at, at, now), None);

        let daily = next_occurrence(RepeatType::Daily, at, at, now).unwrap();
        assert_eq!(
            daily,
            chrono::Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap()
        );

        let weekly = next_occurrence(RepeatType::Weekly, at, at, now).unwrap();
        assert_eq!(
            weekly,
            chrono::Utc.with_ymd_and_hms(2026, 9, 17, 9, 0, 0).unwrap()
        );

        // 2026-09-12 是周六：工作日提醒从周五(9/11)推进到周一(9/14) 09:00
        let fri = chrono::Utc.with_ymd_and_hms(2026, 9, 11, 9, 0, 0).unwrap();
        let wd = next_occurrence(RepeatType::Weekdays, fri, fri, now).unwrap();
        assert_eq!(
            wd,
            chrono::Utc.with_ymd_and_hms(2026, 9, 16, 9, 0, 0).unwrap()
        );
    }

    #[test]
    fn repeat_next_occurrence_monthly_yearly_clamp() {
        // 锚 1-31 09:00，已触发行 remind_at=2-28（clamp 后），now=3-01：
        // 必须从锚点重算 → 3-31（若从 2-28 连续推进会漂移成 3-28，正是本用例锁死的回归）
        let anchor = chrono::Utc.with_ymd_and_hms(2026, 1, 31, 9, 0, 0).unwrap();
        let at = chrono::Utc.with_ymd_and_hms(2026, 2, 28, 9, 0, 0).unwrap();
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 1, 0, 0, 0).unwrap();
        let m = next_occurrence(RepeatType::Monthly, at, anchor, now).unwrap();
        assert_eq!(
            m,
            chrono::Utc.with_ymd_and_hms(2026, 3, 31, 9, 0, 0).unwrap()
        );

        // 闰年锚 2-29，remind_at=2027-02-28（clamp 后），now=2027-03-01 → 2028-02-29
        let anchor2 = chrono::Utc.with_ymd_and_hms(2024, 2, 29, 8, 0, 0).unwrap();
        let at2 = chrono::Utc.with_ymd_and_hms(2027, 2, 28, 8, 0, 0).unwrap();
        let now2 = chrono::Utc.with_ymd_and_hms(2027, 3, 1, 0, 0, 0).unwrap();
        let y = next_occurrence(RepeatType::Yearly, at2, anchor2, now2).unwrap();
        assert_eq!(
            y,
            chrono::Utc.with_ymd_and_hms(2028, 2, 29, 8, 0, 0).unwrap()
        );

        // remind_at 尚未到期时原样返回（与 daily/weekly 行为一致）
        let at3 = chrono::Utc.with_ymd_and_hms(2026, 12, 25, 8, 0, 0).unwrap();
        let now3 = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(
            next_occurrence(RepeatType::Monthly, at3, at3, now3),
            Some(at3)
        );

        // 跨年 12 个月推进：年规则下 1-31 锚在 2027-01-31 触发后 → 2028-01-31
        let anchor4 = chrono::Utc.with_ymd_and_hms(2027, 1, 31, 7, 0, 0).unwrap();
        let now4 = chrono::Utc.with_ymd_and_hms(2027, 2, 1, 0, 0, 0).unwrap();
        let y4 = next_occurrence(RepeatType::Yearly, anchor4, anchor4, now4).unwrap();
        assert_eq!(
            y4,
            chrono::Utc.with_ymd_and_hms(2028, 1, 31, 7, 0, 0).unwrap()
        );
    }

    #[test]
    fn monthly_anchor_persists_across_chain() {
        // 端到端：create 写锚点 → fire_due 语义（create_with_anchor 透传锚点）→ 二轮后日号不漂移
        let conn = setup();
        let n = super::super::notes::create(&conn, "t", "c").unwrap();
        let r = create(&conn, &n.id, "2026-01-31T09:00:00Z", RepeatType::Monthly).unwrap();
        assert_eq!(r.anchor_at.as_deref(), Some("2026-01-31T09:00:00Z"));

        // 第一轮触发（1-31）：next 应为 2-28（clamp），锚点透传
        let now1 = chrono::Utc.with_ymd_and_hms(2026, 1, 31, 9, 0, 1).unwrap();
        let anchor = chrono::DateTime::parse_from_rfc3339(r.anchor_at.as_deref().unwrap())
            .unwrap()
            .with_timezone(&chrono::Utc);
        let at1 = chrono::DateTime::parse_from_rfc3339(&r.remind_at)
            .unwrap()
            .with_timezone(&chrono::Utc);
        let next1 = next_occurrence(RepeatType::Monthly, at1, anchor, now1).unwrap();
        assert_eq!(next1.format("%Y-%m-%d").to_string(), "2026-02-28");
        let r2 = create_with_anchor(
            &conn,
            &n.id,
            &crate::reminder::fmt(next1),
            RepeatType::Monthly,
            r.anchor_at.as_deref().unwrap_or(""),
        )
        .unwrap();
        assert_eq!(
            r2.anchor_at.as_deref(),
            Some("2026-01-31T09:00:00Z"),
            "锚点跨轮透传"
        );

        // 第二轮触发（2-28）：next 必须回到 31 号（3-31），而不是从 28 号漂移
        let now2 = chrono::Utc.with_ymd_and_hms(2026, 2, 28, 9, 0, 1).unwrap();
        let at2 = chrono::DateTime::parse_from_rfc3339(&r2.remind_at)
            .unwrap()
            .with_timezone(&chrono::Utc);
        let next2 = next_occurrence(RepeatType::Monthly, at2, anchor, now2).unwrap();
        assert_eq!(
            next2.format("%Y-%m-%d").to_string(),
            "2026-03-31",
            "锚点防漂移"
        );
    }
}
