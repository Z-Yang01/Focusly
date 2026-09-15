//! 错过提醒计数 DAO：统计"近 7 天内自动取消且从未触发"的提醒，
//! 供应用启动时用 `dnd::missed_summary_text` 生成一条错过汇总系统通知。
//!
//! 口径（与 reminder.rs 的过期兜底对应）：
//! - `status = 'cancelled'`：错过超过 24h 被 fire_due 自动取消的行
//!   （`triggered_at IS NULL` 排除先触发后取消的正常流转）；
//! - 近 7 天：`remind_at >= now - 7d`，避免翻旧账。
//! 两端都用 SQLite `datetime()` 规整成 UTC 秒级字符串再比较，
//! 对 RFC3339 格式差异（`+00:00` / `Z`、精度）鲁棒。

use rusqlite::Connection;

use crate::error::AppResult;

/// 近 7 天内自动取消且未触发的提醒条数。
pub fn count_missed(conn: &Connection) -> AppResult<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM reminders \
         WHERE status = 'cancelled' \
           AND triggered_at IS NULL \
           AND datetime(remind_at) >= datetime('now', '-7 days')",
        [],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// 测试与日志用：与 fmt（reminder::fmt）一致的 remind_at 字符串。
#[cfg(test)]
pub(crate) fn rfc3339(t: chrono::DateTime<chrono::Utc>) -> String {
    use chrono::SecondsFormat;
    t.to_rfc3339_opts(SecondsFormat::Secs, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use rusqlite::params;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        conn
    }

    fn add_reminder(
        conn: &Connection,
        remind_at: chrono::DateTime<Utc>,
        status: &str,
        mark_triggered: bool,
    ) -> String {
        let note = super::super::notes::create(conn, "错过汇总测试", "内容").unwrap();
        let r = super::super::reminders::create(
            conn,
            &note.id,
            &rfc3339(remind_at),
            super::super::models::RepeatType::Once,
        )
        .unwrap();
        super::super::reminders::set_status(conn, &r.id, status, mark_triggered).unwrap();
        r.id
    }

    #[test]
    fn empty_db_counts_zero() {
        let conn = setup();
        assert_eq!(count_missed(&conn).unwrap(), 0);
    }

    #[test]
    fn counts_recent_auto_cancelled() {
        let conn = setup();
        // 昨天到期 → 被自动取消、从未触发：计入
        add_reminder(&conn, Utc::now() - Duration::hours(25), "cancelled", false);
        assert_eq!(count_missed(&conn).unwrap(), 1);
    }

    #[test]
    fn ignores_older_than_seven_days() {
        let conn = setup();
        add_reminder(&conn, Utc::now() - Duration::days(8), "cancelled", false);
        assert_eq!(count_missed(&conn).unwrap(), 0, "7 天前的不算");

        add_reminder(&conn, Utc::now() - Duration::days(6), "cancelled", false);
        assert_eq!(count_missed(&conn).unwrap(), 1, "6 天前的算");
    }

    #[test]
    fn ignores_triggered_cancelled_and_other_statuses() {
        let conn = setup();
        // 先触发后取消（triggered_at 非空）：正常流转，不算错过
        add_reminder(&conn, Utc::now() - Duration::hours(25), "cancelled", true);
        // 其他状态不算
        add_reminder(&conn, Utc::now() - Duration::hours(25), "pending", false);
        add_reminder(&conn, Utc::now() - Duration::hours(25), "done", false);
        add_reminder(&conn, Utc::now() - Duration::hours(25), "triggered", false);
        assert_eq!(count_missed(&conn).unwrap(), 0);
    }

    #[test]
    fn boundary_at_seven_days_is_inclusive() {
        let conn = setup();
        // 比界内/界外各留 1 小时余量，避免测试运行耗时带来的抖动
        add_reminder(
            &conn,
            Utc::now() - Duration::days(7) - Duration::hours(1),
            "cancelled",
            false,
        );
        assert_eq!(count_missed(&conn).unwrap(), 0, "刚过 7 天边界：不算");

        add_reminder(
            &conn,
            Utc::now() - Duration::days(7) + Duration::hours(1),
            "cancelled",
            false,
        );
        assert_eq!(count_missed(&conn).unwrap(), 1, "7 天以内：算");
    }

    #[test]
    fn multiple_rows_all_counted() {
        let conn = setup();
        add_reminder(&conn, Utc::now() - Duration::hours(25), "cancelled", false);
        add_reminder(&conn, Utc::now() - Duration::hours(30), "cancelled", false);
        add_reminder(&conn, Utc::now() - Duration::days(2), "cancelled", false);
        add_reminder(&conn, Utc::now() - Duration::days(8), "cancelled", false);
        add_reminder(&conn, Utc::now() - Duration::hours(25), "cancelled", true);
        assert_eq!(count_missed(&conn).unwrap(), 3);
    }

    #[test]
    fn string_params_bind_correctly() {
        // 防御性检查：params![] 空参数 + datetime() 规整在真实 schema 上可用
        let conn = setup();
        let _ = add_reminder(&conn, Utc::now() - Duration::hours(25), "cancelled", false);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM reminders WHERE status = ?1",
                params!["cancelled"],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        assert_eq!(count_missed(&conn).unwrap(), 1);
    }
}
