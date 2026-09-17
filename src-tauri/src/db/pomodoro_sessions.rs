//! 番茄钟会话（pomodoro_sessions 表）DAO 与统计查询。
//!
//! 时间口径：started_at / ended_at 与 remind_at 一致，使用 `crate::reminder::fmt/parse`
//! 的 RFC3339 UTC 字符串（同格式定宽，SQL 字符串比较即时间比较）。
//! status: running | completed | interrupted；phase: focus | short_break | long_break。

use std::collections::HashMap;

use chrono::{Local, TimeZone, Utc};
use rusqlite::{params, Connection, Row};

use super::models::PomodoroSession;
use crate::error::AppResult;

const COLS: &str =
    "id, note_id, task_key, task_text_snapshot, phase, planned_sec, actual_sec, started_at, ended_at, status, interrupt_reason";

fn row_to_session(r: &Row) -> rusqlite::Result<PomodoroSession> {
    Ok(PomodoroSession {
        id: r.get("id")?,
        note_id: r.get("note_id")?,
        task_key: r.get("task_key")?,
        task_text_snapshot: r.get("task_text_snapshot")?,
        phase: r.get("phase")?,
        planned_sec: r.get("planned_sec")?,
        actual_sec: r.get("actual_sec")?,
        started_at: r.get("started_at")?,
        ended_at: r.get("ended_at")?,
        status: r.get("status")?,
        interrupt_reason: r.get("interrupt_reason")?,
    })
}

/// 写入一条运行中的会话（started_at = 现在）。
pub fn insert_running(
    conn: &Connection,
    id: &str,
    note_id: &str,
    task_key: &str,
    task_text: &str,
    phase: &str,
    planned_sec: i64,
) -> AppResult<()> {
    insert_running_at(
        conn,
        id,
        note_id,
        task_key,
        task_text,
        phase,
        planned_sec,
        &crate::reminder::fmt(Utc::now()),
    )
}

/// task_key 约定："daily:<uuid>" 前缀 = 今日任务绑定。
pub fn daily_task_id_of(task_key: &str) -> Option<&str> {
    task_key.strip_prefix("daily:").filter(|s| !s.is_empty())
}

/// 可控 started_at 的插入（测试与启动恢复复算用）。
pub fn insert_running_at(
    conn: &Connection,
    id: &str,
    note_id: &str,
    task_key: &str,
    task_text: &str,
    phase: &str,
    planned_sec: i64,
    started_at: &str,
) -> AppResult<()> {
    conn.execute(
        "INSERT INTO pomodoro_sessions \
             (id, note_id, task_key, task_text_snapshot, phase, planned_sec, actual_sec, started_at, ended_at, status, interrupt_reason) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, NULL, 'running', NULL)",
        params![id, note_id, task_key, task_text, phase, planned_sec, started_at],
    )?;
    Ok(())
}

/// 结束会话：status ∈ completed | interrupted，写实际秒与结束时间。
pub fn finish(
    conn: &Connection,
    id: &str,
    status: &str,
    actual_sec: i64,
    interrupt_reason: Option<&str>,
    ended_at: &str,
) -> AppResult<()> {
    conn.execute(
        "UPDATE pomodoro_sessions \
         SET status = ?2, actual_sec = ?3, interrupt_reason = ?4, ended_at = ?5 \
         WHERE id = ?1",
        params![id, status, actual_sec, interrupt_reason, ended_at],
    )?;
    Ok(())
}

/// 私密防线：清空某便签运行中会话的任务文本快照，返回清理行数。
pub fn clear_task_text_for_note(conn: &Connection, note_id: &str) -> AppResult<usize> {
    let n = conn.execute(
        "UPDATE pomodoro_sessions SET task_text_snapshot = '' \
         WHERE note_id = ?1 AND status = 'running'",
        params![note_id],
    )?;
    Ok(n)
}

/// 最近一条运行中的会话（应用重启恢复用）。
pub fn get_running(conn: &Connection) -> AppResult<Option<PomodoroSession>> {
    let r = conn.query_row(
        &format!(
            "SELECT {COLS} FROM pomodoro_sessions WHERE status = 'running' \
             ORDER BY started_at DESC LIMIT 1"
        ),
        [],
        row_to_session,
    );
    match r {
        Ok(s) => Ok(Some(s)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 今日统计。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsToday {
    pub focus_count: i64,
    pub focus_sec: i64,
    pub done_tasks: i64,
    pub skipped_tasks: i64,
    pub interrupts: i64,
}

/// 今日统计：
/// - focus_count / focus_sec：completed 且 phase=focus 且 started_at 落在窗口内；
/// - interrupts：interrupted 且 started_at 落在窗口内；
/// - done_tasks / skipped_tasks：task_meta 的状态变更（updated_at 落在窗口内，
///   任务可能当天完成但当天没有对应番茄钟会话）。
pub fn stats_today(conn: &Connection, today_start: &str, now: &str) -> AppResult<StatsToday> {
    let (focus_count, focus_sec) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(actual_sec), 0) FROM pomodoro_sessions \
         WHERE status = 'completed' AND phase = 'focus' \
           AND started_at >= ?1 AND started_at <= ?2",
        params![today_start, now],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    let interrupts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pomodoro_sessions \
         WHERE status = 'interrupted' AND started_at >= ?1 AND started_at <= ?2",
        params![today_start, now],
        |r| r.get(0),
    )?;
    let done_tasks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_meta \
         WHERE status = 'done' AND updated_at >= ?1 AND updated_at <= ?2",
        params![today_start, now],
        |r| r.get(0),
    )?;
    let skipped_tasks: i64 = conn.query_row(
        "SELECT COUNT(*) FROM task_meta \
         WHERE status = 'skipped' AND updated_at >= ?1 AND updated_at <= ?2",
        params![today_start, now],
        |r| r.get(0),
    )?;
    Ok(StatsToday {
        focus_count,
        focus_sec,
        done_tasks,
        skipped_tasks,
        interrupts,
    })
}

/// 单日聚合结果（date = 本地 "YYYY-MM-DD"）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyStat {
    pub date: String,
    pub focus_count: i64,
    pub focus_sec: i64,
}

/// 近 N 天按本地日聚合（Rust 侧解析聚合，SQL 只做窗口过滤——可测性优先）。
pub fn stats_daily(conn: &Connection, days: i64) -> AppResult<Vec<DailyStat>> {
    if days <= 0 {
        return Ok(Vec::new());
    }
    let today = Local::now().date_naive();
    let start_local = today - chrono::Duration::days(days - 1);
    let cutoff = Local
        .from_local_datetime(
            &start_local
                .and_hms_opt(0, 0, 0)
                .unwrap_or_else(|| start_local.and_hms_opt(12, 0, 0).unwrap()),
        )
        .single()
        .map(|t| t.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(days));

    let mut stmt = conn.prepare(
        "SELECT started_at, phase, status, actual_sec FROM pomodoro_sessions \
         WHERE started_at >= ?1",
    )?;
    let rows = stmt
        .query_map(params![crate::reminder::fmt(cutoff)], |r| {
            let started: String = r.get(0)?;
            let phase: String = r.get(1)?;
            let status: String = r.get(2)?;
            let actual: i64 = r.get(3)?;
            let local_date = chrono::DateTime::parse_from_rfc3339(&started)
                .map(|t| {
                    t.with_timezone(&Local)
                        .date_naive()
                        .format("%Y-%m-%d")
                        .to_string()
                })
                .unwrap_or_default();
            Ok((
                local_date,
                phase == "focus" && status == "completed",
                actual,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let today_str = today.format("%Y-%m-%d").to_string();
    Ok(fold_daily(&rows, &today_str, days))
}

/// 纯函数：把 (本地日期, 是否有效焦点, 实际秒) 行聚合进以 today 结尾的 N 天窗口，
/// 窗口内逐日零填充、按日期升序返回；窗口外/非法日期行忽略。
pub fn fold_daily(rows: &[(String, bool, i64)], today: &str, days: i64) -> Vec<DailyStat> {
    if days <= 0 {
        return Vec::new();
    }
    let Ok(today_d) = chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return Vec::new();
    };
    let start = today_d - chrono::Duration::days(days - 1);
    let mut map: HashMap<String, (i64, i64)> = HashMap::new();
    for (date, is_focus, actual) in rows {
        let Ok(d) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") else {
            continue;
        };
        if d < start || d > today_d {
            continue;
        }
        if *is_focus {
            let e = map.entry(date.clone()).or_insert((0, 0));
            e.0 += 1;
            e.1 += actual;
        }
    }
    (0..days)
        .map(|i| {
            let key = (start + chrono::Duration::days(i))
                .format("%Y-%m-%d")
                .to_string();
            let (c, s) = map.get(&key).copied().unwrap_or((0, 0));
            DailyStat {
                date: key,
                focus_count: c,
                focus_sec: s,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn insert_and_get_running() {
        let conn = setup();
        assert!(get_running(&conn).unwrap().is_none(), "空表无运行会话");
        insert_running(&conn, "s1", "n1", "k1", "买牛奶", "focus", 1500).unwrap();
        let s = get_running(&conn).unwrap().expect("应能取到运行会话");
        assert_eq!(s.id, "s1");
        assert_eq!(s.note_id.as_deref(), Some("n1"));
        assert_eq!(s.task_text_snapshot.as_deref(), Some("买牛奶"));
        assert_eq!(s.phase, "focus");
        assert_eq!(s.planned_sec, 1500);
        assert_eq!(s.actual_sec, 0);
        assert_eq!(s.status, "running");
        assert_eq!(s.ended_at, None);
    }

    #[test]
    fn finish_sets_completed_fields() {
        let conn = setup();
        insert_running(&conn, "s1", "n1", "k1", "任务", "focus", 1500).unwrap();
        finish(
            &conn,
            "s1",
            "completed",
            1500,
            None,
            "2026-09-16T10:00:00+00:00",
        )
        .unwrap();
        let running = get_running(&conn).unwrap();
        assert!(running.is_none(), "结束后不再是 running");

        // 直接查行验证字段
        let (status, actual, ended, reason): (String, i64, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT status, actual_sec, ended_at, interrupt_reason FROM pomodoro_sessions WHERE id='s1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(status, "completed");
        assert_eq!(actual, 1500);
        assert_eq!(ended.as_deref(), Some("2026-09-16T10:00:00+00:00"));
        assert_eq!(reason, None);

        // interrupted + reason
        insert_running(&conn, "s2", "n1", "k1", "任务", "focus", 1500).unwrap();
        finish(
            &conn,
            "s2",
            "interrupted",
            300,
            Some("skip"),
            "2026-09-16T10:05:00+00:00",
        )
        .unwrap();
        let reason: Option<String> = conn
            .query_row(
                "SELECT interrupt_reason FROM pomodoro_sessions WHERE id='s2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reason.as_deref(), Some("skip"));
    }

    #[test]
    fn stats_today_counts_focus_and_interrupts_in_window() {
        let conn = setup();
        // 窗口内：2 个完成 focus + 1 个中断 + 1 个完成 break（不计数）
        insert_running_at(
            &conn,
            "a",
            "n1",
            "k1",
            "t",
            "focus",
            1500,
            "2026-09-16T01:00:00+00:00",
        )
        .unwrap();
        finish(
            &conn,
            "a",
            "completed",
            1500,
            None,
            "2026-09-16T01:25:00+00:00",
        )
        .unwrap();
        insert_running_at(
            &conn,
            "b",
            "n1",
            "k1",
            "t",
            "focus",
            1500,
            "2026-09-16T02:00:00+00:00",
        )
        .unwrap();
        finish(
            &conn,
            "b",
            "completed",
            1200,
            None,
            "2026-09-16T02:20:00+00:00",
        )
        .unwrap();
        insert_running_at(
            &conn,
            "c",
            "n1",
            "k1",
            "t",
            "focus",
            1500,
            "2026-09-16T03:00:00+00:00",
        )
        .unwrap();
        finish(
            &conn,
            "c",
            "interrupted",
            300,
            Some("skip"),
            "2026-09-16T03:05:00+00:00",
        )
        .unwrap();
        insert_running_at(
            &conn,
            "d",
            "n1",
            "k1",
            "t",
            "short_break",
            300,
            "2026-09-16T04:00:00+00:00",
        )
        .unwrap();
        finish(
            &conn,
            "d",
            "completed",
            300,
            None,
            "2026-09-16T04:05:00+00:00",
        )
        .unwrap();
        // 窗口外（前一天）
        insert_running_at(
            &conn,
            "e",
            "n1",
            "k1",
            "t",
            "focus",
            1500,
            "2026-09-15T23:00:00+00:00",
        )
        .unwrap();
        finish(
            &conn,
            "e",
            "completed",
            1500,
            None,
            "2026-09-15T23:25:00+00:00",
        )
        .unwrap();

        let st = stats_today(
            &conn,
            "2026-09-16T00:00:00+00:00",
            "2026-09-16T12:00:00+00:00",
        )
        .unwrap();
        assert_eq!(st.focus_count, 2, "只计 completed focus");
        assert_eq!(st.focus_sec, 1500 + 1200);
        assert_eq!(st.interrupts, 1, "break 的 completed 不进 interrupts");
    }

    #[test]
    fn stats_today_counts_task_meta_status_changes() {
        let conn = setup();
        let done = super::super::task_meta::TaskMetaUpsert {
            note_id: "n1".into(),
            task_key: "k1".into(),
            line_text: "- [ ] a".into(),
            status: Some("done".into()),
            estimate: None,
            priority: None,
            due_at: None,
            clear_skip: false,
        };
        let skipped = super::super::task_meta::TaskMetaUpsert {
            note_id: "n1".into(),
            task_key: "k2".into(),
            line_text: "- [ ] b".into(),
            status: Some("skipped".into()),
            ..done.clone()
        };
        super::super::task_meta::upsert(&conn, &done).unwrap();
        super::super::task_meta::upsert(&conn, &skipped).unwrap();

        // 窗口用昨天→现在：task_meta 的 updated_at 是真实"现在"
        let now = crate::reminder::fmt(Utc::now());
        let start = crate::reminder::fmt(Utc::now() - chrono::Duration::hours(2));
        let st = stats_today(&conn, &start, &now).unwrap();
        assert_eq!(st.done_tasks, 1);
        assert_eq!(st.skipped_tasks, 1);

        // 完全过期的窗口：不计入
        let st2 = stats_today(
            &conn,
            "2020-01-01T00:00:00+00:00",
            "2020-01-02T00:00:00+00:00",
        )
        .unwrap();
        assert_eq!(st2.done_tasks, 0);
        assert_eq!(st2.skipped_tasks, 0);
    }

    #[test]
    fn fold_daily_zero_fills_window_and_ignores_out_of_range() {
        let rows = vec![
            ("2026-09-14".to_string(), true, 1500),
            ("2026-09-14".to_string(), true, 1000),
            ("2026-09-15".to_string(), true, 500),
            ("2026-09-16".to_string(), false, 9999), // 非 focus 忽略
            ("2026-09-10".to_string(), true, 12345), // 窗口外忽略
            ("垃圾".to_string(), true, 100),         // 非法日期忽略
        ];
        let out = fold_daily(&rows, "2026-09-16", 3);
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].date, "2026-09-14");
        assert_eq!(out[0].focus_count, 2);
        assert_eq!(out[0].focus_sec, 2500);
        assert_eq!(out[1].date, "2026-09-15");
        assert_eq!(out[1].focus_count, 1);
        assert_eq!(out[2].date, "2026-09-16");
        assert_eq!(out[2].focus_count, 0, "零填充");
        assert_eq!(out[2].focus_sec, 0);

        assert!(fold_daily(&rows, "2026-09-16", 0).is_empty());
        assert!(fold_daily(&rows, "bad-date", 3).is_empty());
        // 升序
        let out7 = fold_daily(&rows, "2026-09-16", 7);
        assert_eq!(out7.len(), 7);
        assert_eq!(out7[0].date, "2026-09-10");
        assert_eq!(out7.last().unwrap().date, "2026-09-16");
    }

    #[test]
    fn stats_daily_today_integration() {
        let conn = setup();
        insert_running(&conn, "now1", "n1", "k1", "t", "focus", 1500).unwrap();
        finish(
            &conn,
            "now1",
            "completed",
            1500,
            None,
            &crate::reminder::fmt(Utc::now()),
        )
        .unwrap();
        let out = stats_daily(&conn, 7).unwrap();
        assert_eq!(out.len(), 7);
        let today = out.last().unwrap();
        assert_eq!(today.focus_count, 1, "今天的会话计入最后一天");
        assert_eq!(today.focus_sec, 1500);
        assert!(out[..6].iter().all(|d| d.focus_count == 0), "其余天零填充");
    }
}
