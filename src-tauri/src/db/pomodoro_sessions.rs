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
#[allow(clippy::too_many_arguments)]
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

/// 私密防线（今日任务）：任务转私密时清空其全部会话的任务文本快照
/// （含历史 completed/interrupted 行——tooltip 与复盘报表都会读历史行），返回清理行数。
pub fn clear_task_text_for_daily(conn: &Connection, daily_id: &str) -> AppResult<usize> {
    let task_key = format!("daily:{daily_id}");
    let n = conn.execute(
        "UPDATE pomodoro_sessions SET task_text_snapshot = '' \
         WHERE task_key = ?1 AND task_text_snapshot != ''",
        params![task_key],
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

// ---------- 复盘报表（周/月窗口） ----------

/// 中断原因归一（报表口径）：已知字面量原样归组，历史自由字符串/缺失归 "other"。
pub fn normalize_reason(reason: Option<&str>) -> &'static str {
    match reason {
        Some("manual") => "manual",
        Some("skip") => "skip",
        Some("task_done") => "task_done",
        Some("app_exit") => "app_exit",
        Some("switch_task") => "switch_task",
        _ => "other",
    }
}

/// 本地日 → (UTC 起点, 次日 UTC 起点) 的 RFC3339（定宽字符串，SQL 比较即时间比较）。
/// 边界取该本地日"最早存在"的时刻：0 点歧义（DST 回拨出现两次 0 点）取第一次；
/// 0 点被跳过（DST 拨快）依次尝试 1:00、12:00——start/end 同一语义，
/// 窗口两端始终对齐本地日边界（实施午夜型 DST 切换的时区也不漏盖/越界）。
fn local_day_window_utc(date: &str) -> Option<(String, String)> {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?;
    let earliest_on = |d: chrono::NaiveDate| -> Option<chrono::DateTime<Utc>> {
        for h in [0u32, 1, 12] {
            match Local.from_local_datetime(&d.and_hms_opt(h, 0, 0)?) {
                chrono::LocalResult::Single(t) | chrono::LocalResult::Ambiguous(t, _) => {
                    return Some(t.with_timezone(&Utc));
                }
                chrono::LocalResult::None => continue,
            }
        }
        None
    };
    let start = earliest_on(d)?;
    let end = earliest_on(d + chrono::Duration::days(1))?;
    Some((crate::reminder::fmt(start), crate::reminder::fmt(end)))
}

/// RFC3339 → (本地日期 "YYYY-MM-DD", 本地小时 0..=23)。
fn local_date_hour(started_at: &str) -> Option<(String, u32)> {
    use chrono::Timelike;
    let t = chrono::DateTime::parse_from_rfc3339(started_at).ok()?;
    let l = t.with_timezone(&Local);
    Some((l.date_naive().format("%Y-%m-%d").to_string(), l.hour()))
}

/// 复盘报表里的任务专注条目。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFocus {
    /// 任务标签；私密便签快照已清空 → "私密任务"
    pub label: String,
    pub focus_sec: i64,
    pub session_count: i64,
    /// daily（今日任务）| note（便签任务）
    pub kind: String,
}

/// 中断原因分布条目。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasonCount {
    pub reason: String,
    pub count: i64,
}

/// focus 会话开始小时分布条目。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HourCount {
    pub hour: i64,
    pub count: i64,
}

/// 复盘报表（周/月窗口，start_date..end_date 本地日闭区间）。
/// 日期归属口径：会话按 started_at 的本地日归组（跨午夜归开始日）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusReport {
    /// 窗口内逐日零填充（升序）
    pub days: Vec<DailyStat>,
    /// focus 且 completed 的会话数 / 累计实际秒
    pub focus_count: i64,
    pub focus_sec: i64,
    /// interrupted 会话数（含全部阶段）
    pub interrupted_count: i64,
    /// focus 完成率 = completed ÷ (completed + interrupted)（focus 阶段内）；无样本为 None
    pub completion_rate: Option<f64>,
    /// 绑定任务按累计专注时长前 5（task_key 为空的未绑定会话不参与）
    pub top_tasks: Vec<TaskFocus>,
    /// 中断原因分布（归一后，次数降序）
    pub by_reason: Vec<ReasonCount>,
    /// focus 会话（含中断）开始小时分布 0..=23
    pub by_hour: Vec<HourCount>,
}

/// 聚合复盘报表。SQL 只做窗口过滤，聚合在 Rust 侧完成（可测性优先，与 stats_daily 同思路）。
fn empty_report() -> FocusReport {
    FocusReport {
        days: Vec::new(),
        focus_count: 0,
        focus_sec: 0,
        interrupted_count: 0,
        completion_rate: None,
        top_tasks: Vec::new(),
        by_reason: Vec::new(),
        by_hour: Vec::new(),
    }
}

/// 聚合复盘报表。SQL 只做窗口过滤，聚合在 Rust 侧完成（可测性优先，与 stats_daily 同思路）。
/// 起止日期可乱序（内部交换）；非法日期返回空报表。
pub fn stats_report(conn: &Connection, start_date: &str, end_date: &str) -> AppResult<FocusReport> {
    let Ok(sd0) = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d") else {
        return Ok(empty_report());
    };
    let Ok(ed0) = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d") else {
        return Ok(empty_report());
    };
    let (sd, ed) = if ed0 < sd0 { (ed0, sd0) } else { (sd0, ed0) };
    let sd_str = sd.format("%Y-%m-%d").to_string();
    let ed_str = ed.format("%Y-%m-%d").to_string();
    let Some((win_start, _)) = local_day_window_utc(&sd_str) else {
        return Ok(empty_report());
    };
    // 窗口上界 = end_date 次日 0 点
    let next_day = (ed + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    let Some((win_end, _)) = local_day_window_utc(&next_day) else {
        return Ok(empty_report());
    };

    let mut stmt = conn.prepare(
        "SELECT started_at, phase, status, actual_sec, task_key, task_text_snapshot, interrupt_reason \
         FROM pomodoro_sessions WHERE started_at >= ?1 AND started_at < ?2",
    )?;
    let rows = stmt
        .query_map(params![win_start, win_end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // 逐日零填充键
    let day_count = (ed - sd).num_days() as usize + 1;
    let day_keys: Vec<String> = (0..day_count)
        .map(|i| {
            (sd + chrono::Duration::days(i as i64))
                .format("%Y-%m-%d")
                .to_string()
        })
        .collect();
    let mut day_map: HashMap<String, (i64, i64)> = HashMap::new();

    let mut focus_count = 0i64;
    let mut focus_sec = 0i64;
    let mut interrupted_count = 0i64;
    let mut focus_interrupted = 0i64;
    // task_key → (label, focus_sec, session_count, kind)
    let mut tasks: HashMap<String, (String, i64, i64, &'static str)> = HashMap::new();
    let mut reasons: HashMap<String, i64> = HashMap::new();
    let mut hours = [0i64; 24];

    for (started, phase, status, actual, task_key, snapshot, reason) in &rows {
        let Some((date, hour)) = local_date_hour(started) else {
            continue;
        };
        // Rust 侧本地日复核是权威口径：SQL 窗口只是粗过滤（DST 边界可能略宽），
        // 越界行一律跳过——保证合计/分布/逐日三者自洽。
        if date.as_str() < sd_str.as_str() || date.as_str() > ed_str.as_str() {
            continue;
        }
        let in_window = true;

        if status == "interrupted" {
            interrupted_count += 1;
            let key = normalize_reason(reason.as_deref()).to_string();
            *reasons.entry(key).or_insert(0) += 1;
        }
        if phase != "focus" {
            continue;
        }
        if in_window && (hour as usize) < hours.len() {
            hours[hour as usize] += 1;
        }
        // 任务归并：completed + interrupted 都计入（中断掉的时间也是花在任务上的时间）；
        // 未绑定任务的会话不参与 Top
        if !task_key.is_empty() && matches!(status.as_str(), "completed" | "interrupted") {
            let kind = if daily_task_id_of(task_key).is_some() {
                "daily"
            } else {
                "note"
            };
            let label = if snapshot.trim().is_empty() {
                "私密任务"
            } else {
                snapshot.as_str()
            };
            let e = tasks
                .entry(task_key.clone())
                .or_insert_with(|| (label.to_string(), 0, 0, kind));
            e.1 += actual;
            e.2 += 1;
        }
        match status.as_str() {
            "completed" => {
                focus_count += 1;
                focus_sec += actual;
                if in_window {
                    let e = day_map.entry(date.clone()).or_insert((0, 0));
                    e.0 += 1;
                    e.1 += actual;
                }
            }
            "interrupted" => focus_interrupted += 1,
            _ => {}
        }
    }

    let days = day_keys
        .into_iter()
        .map(|date| {
            let (c, s) = day_map.get(&date).copied().unwrap_or((0, 0));
            DailyStat {
                date,
                focus_count: c,
                focus_sec: s,
            }
        })
        .collect();

    let mut top_tasks: Vec<TaskFocus> = tasks
        .into_iter()
        .map(|(_, (label, sec, count, kind))| TaskFocus {
            label,
            focus_sec: sec,
            session_count: count,
            kind: kind.to_string(),
        })
        .collect();
    top_tasks.sort_by(|a, b| b.focus_sec.cmp(&a.focus_sec).then(a.label.cmp(&b.label)));
    top_tasks.truncate(5);

    let mut by_reason: Vec<ReasonCount> = reasons
        .into_iter()
        .map(|(reason, count)| ReasonCount { reason, count })
        .collect();
    by_reason.sort_by(|a, b| b.count.cmp(&a.count).then(a.reason.cmp(&b.reason)));

    let by_hour = hours
        .iter()
        .enumerate()
        .map(|(h, c)| HourCount {
            hour: h as i64,
            count: *c,
        })
        .collect();

    let completion_rate = if focus_count + focus_interrupted > 0 {
        Some(focus_count as f64 / (focus_count + focus_interrupted) as f64)
    } else {
        None
    };

    Ok(FocusReport {
        days,
        focus_count,
        focus_sec,
        interrupted_count,
        completion_rate,
        top_tasks,
        by_reason,
        by_hour,
    })
}

/// 某本地日的 focus 会话（时间轴"实际专注块"用），按开始时间升序。
/// running 会话不返回（进行中的专注由托盘/控制条/迷你窗呈现）；
/// 私密便签会话的 task_text_snapshot 已在写入端清空，此处仅透传。
pub fn sessions_by_date(conn: &Connection, date: &str) -> AppResult<Vec<PomodoroSession>> {
    let Some((start, end)) = local_day_window_utc(date) else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM pomodoro_sessions \
         WHERE phase = 'focus' AND status IN ('completed', 'interrupted') \
           AND started_at >= ?1 AND started_at < ?2 \
         ORDER BY started_at ASC"
    ))?;
    let rows = stmt
        .query_map(params![start, end], row_to_session)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
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
            focus_min: None,
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

    fn snapshot_of_session(conn: &Connection, id: &str) -> Option<String> {
        conn.query_row(
            "SELECT task_text_snapshot FROM pomodoro_sessions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .ok()
    }

    #[test]
    fn normalize_reason_groups_unknown() {
        assert_eq!(normalize_reason(Some("manual")), "manual");
        assert_eq!(normalize_reason(Some("skip")), "skip");
        assert_eq!(normalize_reason(Some("task_done")), "task_done");
        assert_eq!(normalize_reason(Some("app_exit")), "app_exit");
        assert_eq!(normalize_reason(Some("switch_task")), "switch_task");
        assert_eq!(normalize_reason(Some("任意历史字符串")), "other");
        assert_eq!(normalize_reason(None), "other");
    }

    #[test]
    fn clear_task_text_for_daily_scrubs_all_statuses() {
        let conn = setup();
        let at = |d: u32, h: u32, m: u32| {
            crate::reminder::fmt(Local.with_ymd_and_hms(2026, 9, d, h, m, 0).unwrap().into())
        };
        // running + completed 各一条明文快照，另一任务不受影响
        insert_running_at(
            &conn,
            "s1",
            "",
            "daily:d1",
            "机密标题",
            "focus",
            600,
            &at(18, 9, 0),
        )
        .unwrap();
        finish(&conn, "s1", "completed", 600, None, &at(18, 9, 10)).unwrap();
        insert_running_at(
            &conn,
            "s2",
            "",
            "daily:d1",
            "机密标题",
            "focus",
            600,
            &at(18, 10, 0),
        )
        .unwrap();
        insert_running_at(
            &conn,
            "s3",
            "",
            "daily:d2",
            "别的任务",
            "focus",
            600,
            &at(18, 11, 0),
        )
        .unwrap();
        let n = clear_task_text_for_daily(&conn, "d1").unwrap();
        assert_eq!(n, 2, "running + completed 都清理");
        assert_eq!(snapshot_of_session(&conn, "s1"), Some(String::new()));
        assert_eq!(snapshot_of_session(&conn, "s2"), Some(String::new()));
        assert_eq!(
            snapshot_of_session(&conn, "s3"),
            Some("别的任务".to_string()),
            "其他任务不受影响"
        );
    }

    #[test]
    fn sessions_by_date_returns_focus_of_that_local_day() {
        let conn = setup();
        let at = |d: u32, h: u32, m: u32| {
            crate::reminder::fmt(Local.with_ymd_and_hms(2026, 9, d, h, m, 0).unwrap().into())
        };
        // 18 日 focus completed → 返回
        insert_running_at(
            &conn,
            "s1",
            "n1",
            "daily:d1",
            "任务A",
            "focus",
            1500,
            &at(18, 9, 0),
        )
        .unwrap();
        finish(&conn, "s1", "completed", 1400, None, &at(18, 9, 25)).unwrap();
        // 18 日 short_break → 不返回
        insert_running_at(
            &conn,
            "s2",
            "n1",
            "",
            "",
            "short_break",
            300,
            &at(18, 9, 30),
        )
        .unwrap();
        finish(&conn, "s2", "completed", 300, None, &at(18, 9, 35)).unwrap();
        // 19 日 focus → 不在 18 日窗口
        insert_running_at(
            &conn,
            "s3",
            "n1",
            "k",
            "任务B",
            "focus",
            1500,
            &at(19, 9, 0),
        )
        .unwrap();
        // 18 日 running → 不返回
        insert_running_at(&conn, "s4", "n1", "k2", "R", "focus", 1500, &at(18, 20, 0)).unwrap();

        let list = sessions_by_date(&conn, "2026-09-18").unwrap();
        assert_eq!(list.len(), 1, "只含 18 日已完结 focus");
        assert_eq!(list[0].id, "s1");
        assert_eq!(sessions_by_date(&conn, "bad-date").unwrap().len(), 0);
    }

    #[test]
    fn stats_report_aggregates_window() {
        let conn = setup();
        let at = |d: u32, h: u32, m: u32| {
            crate::reminder::fmt(Local.with_ymd_and_hms(2026, 9, d, h, m, 0).unwrap().into())
        };
        // 18 日：daily 绑定 completed ×2（其一中断）+ note 绑定 completed + 中断 break（未知原因）
        insert_running_at(
            &conn,
            "a",
            "n1",
            "daily:d1",
            "日报",
            "focus",
            1500,
            &at(18, 9, 0),
        )
        .unwrap();
        finish(&conn, "a", "completed", 1500, None, &at(18, 9, 25)).unwrap();
        insert_running_at(
            &conn,
            "b",
            "n2",
            "k2",
            "便签任务",
            "focus",
            1500,
            &at(18, 10, 0),
        )
        .unwrap();
        finish(&conn, "b", "completed", 600, None, &at(18, 10, 15)).unwrap();
        insert_running_at(
            &conn,
            "c",
            "n1",
            "daily:d1",
            "日报",
            "focus",
            1500,
            &at(18, 11, 0),
        )
        .unwrap();
        finish(
            &conn,
            "c",
            "interrupted",
            300,
            Some("manual"),
            &at(18, 11, 10),
        )
        .unwrap();
        insert_running_at(&conn, "e", "n1", "", "", "short_break", 300, &at(18, 15, 0)).unwrap();
        finish(&conn, "e", "interrupted", 100, Some("xyz"), &at(18, 15, 5)).unwrap();
        // 19 日：私密切空快照 + 未绑定 task_key 的 completed
        insert_running_at(&conn, "d", "n3", "k9", "", "focus", 600, &at(19, 8, 0)).unwrap();
        finish(&conn, "d", "completed", 600, None, &at(19, 8, 10)).unwrap();
        insert_running_at(
            &conn,
            "f",
            "n3",
            "",
            "自由专注",
            "focus",
            600,
            &at(19, 21, 0),
        )
        .unwrap();
        finish(&conn, "f", "completed", 1200, None, &at(19, 21, 20)).unwrap();

        let r = stats_report(&conn, "2026-09-14", "2026-09-20").unwrap();
        assert_eq!(r.days.len(), 7);
        let d18 = r.days.iter().find(|d| d.date == "2026-09-18").unwrap();
        assert_eq!(d18.focus_count, 2);
        assert_eq!(d18.focus_sec, 2100);
        let d19 = r.days.iter().find(|d| d.date == "2026-09-19").unwrap();
        assert_eq!(d19.focus_sec, 1800);
        assert_eq!(r.focus_count, 4);
        assert_eq!(r.focus_sec, 3900);
        assert_eq!(r.interrupted_count, 2, "manual + xyz（break 也计入中断数）");
        assert_eq!(r.completion_rate, Some(0.8), "focus 完成 4 / (4+1)");

        // Top 任务：未绑定（f）不参与；私密切空快照归并为「私密任务」
        assert_eq!(r.top_tasks.len(), 3);
        assert_eq!(r.top_tasks[0].label, "日报");
        assert_eq!(r.top_tasks[0].kind, "daily");
        assert_eq!(r.top_tasks[0].focus_sec, 1800);
        assert_eq!(r.top_tasks[0].session_count, 2);
        assert!(r
            .top_tasks
            .iter()
            .any(|t| t.label == "私密任务" && t.kind == "note"));

        // 中断原因：manual 1 + other 1（xyz 归 other）；completed 上的 None 不进分布
        assert_eq!(r.by_reason.len(), 2);
        assert!(r
            .by_reason
            .iter()
            .any(|x| x.reason == "manual" && x.count == 1));
        assert!(r
            .by_reason
            .iter()
            .any(|x| x.reason == "other" && x.count == 1));

        // 时段分布只计 focus（9/10/11/8/21 各 1）
        assert_eq!(r.by_hour.iter().filter(|h| h.count > 0).count(), 5);

        // 空窗口：全零且完成率 None
        let r2 = stats_report(&conn, "2026-01-01", "2026-01-07").unwrap();
        assert_eq!(r2.focus_count, 0);
        assert_eq!(r2.completion_rate, None);
        assert_eq!(r2.days.len(), 7);
        // 乱序窗口自动交换
        let r3 = stats_report(&conn, "2026-09-20", "2026-09-14").unwrap();
        assert_eq!(r3.focus_count, 4);
        // 非法日期返回空报表不 panic
        let r4 = stats_report(&conn, "bad", "2026-09-20").unwrap();
        assert_eq!(r4.days.len(), 0);
    }
}
