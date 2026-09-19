//! 今日任务 DAO（daily_tasks 表）。SQL 只出现在本文件。
//! 重复规则模型：repeat_rule != none 的行是模板；`materialize_recurring`
//! 按目标日期物化实例行（source_task_id 指向模板，实例 repeat_rule='none'），
//! 按 (source_task_id, date) 幂等去重。

use rusqlite::{params, Connection, Row};

use crate::db::models::{DailyTask, DailyTaskStats};
use crate::error::{AppError, AppResult};

pub const REPEAT_NONE: &str = "none";
pub const REPEAT_DAILY: &str = "daily";
pub const REPEAT_WEEKLY: &str = "weekly";
pub const REPEAT_WEEKDAY: &str = "weekday";
pub const REPEAT_MONTHLY: &str = "monthly";
pub const REPEAT_YEARLY: &str = "yearly";
pub const VALID_REPEATS: &[&str] = &[
    REPEAT_NONE,
    REPEAT_DAILY,
    REPEAT_WEEKLY,
    REPEAT_WEEKDAY,
    REPEAT_MONTHLY,
    REPEAT_YEARLY,
];
pub const VALID_PRIORITIES: &[&str] = &["high", "medium", "low"];
pub const VALID_STATUSES: &[&str] = &["todo", "done", "skipped"];

const COLS: &str = "id, date, start_time, end_time, title, note, estimate_pomodoros, \
completed_pomodoros, priority, status, tags, repeat_rule, is_private, start_notified, \
source_task_id, focus_min, created_at, updated_at";

fn row_to_task(r: &Row) -> rusqlite::Result<DailyTask> {
    Ok(DailyTask {
        id: r.get("id")?,
        date: r.get("date")?,
        start_time: r.get("start_time")?,
        end_time: r.get("end_time")?,
        title: r.get("title")?,
        note: r.get("note")?,
        estimate_pomodoros: r.get("estimate_pomodoros")?,
        completed_pomodoros: r.get("completed_pomodoros")?,
        priority: r.get("priority")?,
        status: r.get("status")?,
        tags: r.get("tags")?,
        repeat_rule: r.get("repeat_rule")?,
        is_private: r.get::<_, i64>("is_private")? != 0,
        start_notified: r.get::<_, i64>("start_notified")? != 0,
        source_task_id: r.get("source_task_id")?,
        focus_min: r.get("focus_min")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[allow(clippy::too_many_arguments)]
pub fn create(
    conn: &Connection,
    date: &str,
    start_time: Option<&str>,
    end_time: Option<&str>,
    title: &str,
    note: Option<&str>,
    estimate: i64,
    priority: &str,
    repeat_rule: &str,
    tags: Option<&str>,
    is_private: bool,
    focus_min: Option<i64>,
) -> AppResult<DailyTask> {
    if title.trim().is_empty() {
        return Err(AppError::Invalid("任务标题不能为空".into()));
    }
    if !VALID_PRIORITIES.contains(&priority) {
        return Err(AppError::Invalid(format!("未知优先级: {priority}")));
    }
    if !VALID_REPEATS.contains(&repeat_rule) {
        return Err(AppError::Invalid(format!("未知重复规则: {repeat_rule}")));
    }
    validate_hm_opt(start_time, "start_time")?;
    validate_hm_opt(end_time, "end_time")?;
    validate_date(date)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = now_iso();
    conn.execute(
        "INSERT INTO daily_tasks (id, date, start_time, end_time, title, note, \
         estimate_pomodoros, completed_pomodoros, priority, status, tags, repeat_rule, \
         is_private, start_notified, source_task_id, focus_min, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, 'todo', ?9, ?10, ?11, 0, NULL, ?12, ?13, ?13)",
        params![
            id,
            date,
            start_time,
            end_time,
            title.trim(),
            note,
            estimate.clamp(0, 99),
            priority,
            tags,
            repeat_rule,
            is_private as i64,
            focus_min,
            now
        ],
    )?;
    get(conn, &id)
}

/// 更新（全字段覆盖，None 字段保留原值由调用方组装——此处简化为显式全量更新）。
#[allow(clippy::too_many_arguments)]
pub fn update(
    conn: &Connection,
    id: &str,
    date: &str,
    start_time: Option<&str>,
    end_time: Option<&str>,
    title: &str,
    note: Option<&str>,
    estimate: i64,
    priority: &str,
    repeat_rule: &str,
    tags: Option<&str>,
    is_private: bool,
    focus_min: Option<i64>,
) -> AppResult<DailyTask> {
    if title.trim().is_empty() {
        return Err(AppError::Invalid("任务标题不能为空".into()));
    }
    if !VALID_PRIORITIES.contains(&priority) {
        return Err(AppError::Invalid(format!("未知优先级: {priority}")));
    }
    validate_hm_opt(start_time, "start_time")?;
    validate_hm_opt(end_time, "end_time")?;
    validate_date(date)?;
    let n = conn.execute(
        "UPDATE daily_tasks SET date=?2, start_time=?3, end_time=?4, title=?5, note=?6, \
         estimate_pomodoros=?7, priority=?8, repeat_rule=?9, tags=?10, is_private=?11, \
         focus_min=?12, updated_at=?13 WHERE id=?1",
        params![
            id,
            date,
            start_time,
            end_time,
            title.trim(),
            note,
            estimate.clamp(0, 99),
            priority,
            repeat_rule,
            tags,
            is_private as i64,
            focus_min,
            now_iso()
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("任务不存在: {id}")));
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    let n = conn.execute("DELETE FROM daily_tasks WHERE id = ?1", params![id])?;
    if n == 0 {
        return Err(AppError::NotFound(format!("任务不存在: {id}")));
    }
    Ok(())
}

pub fn get(conn: &Connection, id: &str) -> AppResult<DailyTask> {
    let sql = format!("SELECT {COLS} FROM daily_tasks WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_task)
        .map_err(|_| AppError::NotFound(format!("任务不存在: {id}")))
}

/// 按日列表：模板行不出现在日视图；排序 start_time 升序（NULL 最后），同刻按创建时间。
pub fn list_by_date(conn: &Connection, date: &str) -> AppResult<Vec<DailyTask>> {
    let sql = format!(
        "SELECT {COLS} FROM daily_tasks \
         WHERE date = ?1 AND repeat_rule = 'none' \
         ORDER BY (start_time IS NULL), start_time, created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![date], row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 日期范围（闭区间）内的全部实例（repeat_rule='none'），按日期+开始时间升序。
/// 周概览用：一次查询取 7 天，调用方（命令层）需先对 end 物化重复实例。
pub fn list_by_range(
    conn: &Connection,
    start_date: &str,
    end_date: &str,
) -> AppResult<Vec<DailyTask>> {
    let sql = format!(
        "SELECT {COLS} FROM daily_tasks          WHERE date BETWEEN ?1 AND ?2 AND repeat_rule = 'none'          ORDER BY date, (start_time IS NULL), start_time, created_at"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![start_date, end_date], row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 批量顺延：把 ids 指定的任务移动到目标日期（单事务）。
/// 重复模板（repeat_rule != 'none'）不允许顺延——改模板日期等于改锚点，语义不同；
/// 命令层只应传未完成实例，此处再做一道防线。返回实际移动的行数。
pub fn postpone_to(conn: &Connection, ids: &[String], to_date: &str) -> AppResult<usize> {
    chrono::NaiveDate::parse_from_str(to_date, "%Y-%m-%d")
        .map_err(|_| AppError::Invalid(format!("目标日期不合法: {to_date}")))?;
    let tx = conn.unchecked_transaction()?;
    let mut n = 0;
    for id in ids {
        n += tx.execute(
            "UPDATE daily_tasks SET date = ?2, start_notified = 0, updated_at = ?3              WHERE id = ?1 AND repeat_rule = 'none'",
            params![id, to_date, now_iso()],
        )?;
    }
    tx.commit()?;
    Ok(n)
}

/// 状态切换：todo | done | skipped。
pub fn set_status(conn: &Connection, id: &str, status: &str) -> AppResult<DailyTask> {
    if !VALID_STATUSES.contains(&status) {
        return Err(AppError::Invalid(format!("未知任务状态: {status}")));
    }
    let n = conn.execute(
        "UPDATE daily_tasks SET status = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, status, now_iso()],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("任务不存在: {id}")));
    }
    get(conn, id)
}

/// 专注番茄完成：completed_pomodoros +1（封顶 99）。
pub fn incr_completed(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE daily_tasks SET completed_pomodoros = MIN(completed_pomodoros + 1, 99), \
         updated_at = ?2 WHERE id = ?1",
        params![id, now_iso()],
    )?;
    Ok(())
}

/// 拖拽/编辑调时间：只改 start/end。
pub fn set_time(
    conn: &Connection,
    id: &str,
    start_time: Option<&str>,
    end_time: Option<&str>,
) -> AppResult<DailyTask> {
    validate_hm_opt(start_time, "start_time")?;
    validate_hm_opt(end_time, "end_time")?;
    let n = conn.execute(
        "UPDATE daily_tasks SET start_time = ?2, end_time = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, start_time, end_time, now_iso()],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("任务不存在: {id}")));
    }
    get(conn, id)
}

// ---------- 重复规则物化 ----------

/// 目标日期是否匹配模板的重复规则。
/// daily：目标日 >= 锚点日；weekly：目标日 >= 锚点日且 weekday 相同；
/// weekday：目标日 >= 锚点日且为周一至周五。其余 false。
pub fn repeat_matches(anchor_date: &str, target_date: &str, rule: &str) -> bool {
    use chrono::Datelike;
    let Ok(a) = chrono::NaiveDate::parse_from_str(anchor_date, "%Y-%m-%d") else {
        return false;
    };
    let Ok(t) = chrono::NaiveDate::parse_from_str(target_date, "%Y-%m-%d") else {
        return false;
    };
    if t < a {
        return false;
    }
    match rule {
        REPEAT_DAILY => true,
        REPEAT_WEEKLY => a.weekday() == t.weekday(),
        REPEAT_WEEKDAY => !matches!(t.weekday(), chrono::Weekday::Sat | chrono::Weekday::Sun),
        // 月/年重复按锚点"日号"匹配，目标月无该日（如 1-31 → 2 月）时 clamp 到当月最后一天
        REPEAT_MONTHLY => t.day() == a.day().min(days_in_month(t.year(), t.month())),
        REPEAT_YEARLY => {
            t.month() == a.month() && t.day() == a.day().min(days_in_month(t.year(), t.month()))
        }
        _ => false,
    }
}

/// 当月天数（year/month 的月末日期号）。供 reminders 月/年重复复用。
pub(crate) fn days_in_month(year: i32, month: u32) -> u32 {
    use chrono::Datelike;
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    chrono::NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(31)
}

/// 为目标日期物化到期重复模板（幂等）。返回新建实例数。
pub fn materialize_recurring(conn: &Connection, date: &str) -> AppResult<usize> {
    let templates: Vec<DailyTask> = {
        let sql =
            format!("SELECT {COLS} FROM daily_tasks WHERE repeat_rule != 'none' AND date <= ?1");
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![date], row_to_task)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    let mut created = 0;
    for t in templates {
        if !repeat_matches(&t.date, date, &t.repeat_rule) {
            continue;
        }
        // 幂等：该模板当日实例已存在则跳过（含用户手动删除后不再重建的语义由
        // 删除即物理删除保证——删实例不影响模板，次日仍生成）
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM daily_tasks WHERE source_task_id = ?1 AND date = ?2",
            params![t.id, date],
            |r| r.get(0),
        )?;
        if exists > 0 {
            continue;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = now_iso();
        conn.execute(
            "INSERT INTO daily_tasks (id, date, start_time, end_time, title, note, \
             estimate_pomodoros, completed_pomodoros, priority, status, tags, repeat_rule, \
             is_private, start_notified, source_task_id, focus_min, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, 'todo', ?9, 'none', ?10, 0, ?11, ?12, ?13, ?13)",
            params![
                id,
                date,
                t.start_time,
                t.end_time,
                t.title,
                t.note,
                t.estimate_pomodoros,
                t.priority,
                t.tags,
                t.is_private as i64,
                t.id,
                t.focus_min,
                now
            ],
        )?;
        created += 1;
    }
    Ok(created)
}

// ---------- 到点通知 ----------

/// 到点未通知的任务（date=今日，start_time <= hhmm，status=todo，start_notified=0）。
pub fn due_for_notification(
    conn: &Connection,
    date: &str,
    hhmm: &str,
) -> AppResult<Vec<DailyTask>> {
    let sql = format!(
        "SELECT {COLS} FROM daily_tasks \
         WHERE date = ?1 AND status = 'todo' AND start_notified = 0 \
         AND start_time IS NOT NULL AND start_time <= ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![date, hhmm], row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 批量标记已通知。
pub fn mark_notified(conn: &Connection, ids: &[String]) -> AppResult<()> {
    for id in ids {
        conn.execute(
            "UPDATE daily_tasks SET start_notified = 1, updated_at = ?2 WHERE id = ?1",
            params![id, now_iso()],
        )?;
    }
    Ok(())
}

// ---------- 统计 ----------

pub fn stats(conn: &Connection, date: &str) -> AppResult<DailyTaskStats> {
    conn.query_row(
        "SELECT COUNT(*), \
         SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END), \
         SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END), \
         COALESCE(SUM(estimate_pomodoros), 0), \
         COALESCE(SUM(completed_pomodoros), 0), \
         COALESCE(SUM(CASE WHEN status = 'done' AND start_time IS NOT NULL AND end_time IS NOT NULL \
           THEN (julianday('2000-01-01 ' || end_time) - julianday('2000-01-01 ' || start_time)) * 1440 ELSE 0 END), 0) \
         FROM daily_tasks WHERE date = ?1 AND repeat_rule = 'none'",
        params![date],
        |r| {
            Ok(DailyTaskStats {
                total: r.get(0)?,
                done: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                skipped: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                estimate_pomodoros: r.get(3)?,
                completed_pomodoros: r.get(4)?,
                planned_focus_minutes: r.get::<_, f64>(5)?.round() as i64,
            })
        },
    )
    .map_err(AppError::from)
}

// ---------- 搜索 ----------

/// 轻量搜索：LIKE 匹配 title/note/tags + 可选状态过滤。供搜索面板"任务"分区。
pub fn search(
    conn: &Connection,
    query: &str,
    status: Option<&str>,
    date: Option<&str>,
) -> AppResult<Vec<DailyTask>> {
    let like = format!("%{}%", query.replace(['%', '_'], ""));
    let sql = format!(
        "SELECT {COLS} FROM daily_tasks \
         WHERE (title LIKE ?1 OR IFNULL(note,'') LIKE ?1 OR IFNULL(tags,'') LIKE ?1) \
         AND (?2 IS NULL OR status = ?2) AND (?3 IS NULL OR date = ?3) \
         ORDER BY date DESC, start_time LIMIT 50"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![like, status, date], row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

// ---------- 校验 ----------

fn validate_date(date: &str) -> AppResult<()> {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|_| AppError::Invalid(format!("日期格式非法（需 YYYY-MM-DD）: {date}")))?;
    Ok(())
}

fn validate_hm_opt(v: Option<&str>, field: &str) -> AppResult<()> {
    let Some(v) = v else { return Ok(()) };
    let (h, m) = v
        .split_once(':')
        .ok_or_else(|| AppError::Invalid(format!("{field} 需为 HH:MM: {v}")))?;
    let ok = h.len() == 2
        && m.len() == 2
        && h.bytes().all(|b| b.is_ascii_digit())
        && m.bytes().all(|b| b.is_ascii_digit());
    if !ok {
        return Err(AppError::Invalid(format!("{field} 需为 HH:MM: {v}")));
    }
    let h: u8 = h
        .parse()
        .map_err(|_| AppError::Invalid(format!("{field} 小时越界: {v}")))?;
    let m: u8 = m
        .parse()
        .map_err(|_| AppError::Invalid(format!("{field} 分钟越界: {v}")))?;
    if h > 23 || m > 59 {
        return Err(AppError::Invalid(format!("{field} 越界: {v}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn create_and_get_round_trip() {
        let conn = setup();
        let t = create(
            &conn,
            "2026-09-18",
            Some("09:00"),
            Some("10:00"),
            "写周报",
            Some("按模板"),
            2,
            "high",
            REPEAT_NONE,
            Some("工作"),
            false,
            None,
        )
        .unwrap();
        assert_eq!(t.title, "写周报");
        assert_eq!(t.estimate_pomodoros, 2);
        assert_eq!(t.priority, "high");
        assert_eq!(t.status, "todo");
        assert!(t.source_task_id.is_none());
        assert_eq!(list_by_date(&conn, "2026-09-18").unwrap().len(), 1);
    }

    #[test]
    fn create_rejects_invalid_input() {
        let conn = setup();
        assert!(create(
            &conn,
            "2026-09-18",
            None,
            None,
            "  ",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None
        )
        .is_err());
        assert!(create(
            &conn,
            "2026-09-18",
            None,
            None,
            "t",
            None,
            0,
            "urgent",
            REPEAT_NONE,
            None,
            false,
            None
        )
        .is_err());
        assert!(create(
            &conn,
            "2026-09-18",
            Some("9:00"),
            None,
            "t",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None
        )
        .is_err());
        assert!(create(
            &conn,
            "2026-09-18",
            Some("25:00"),
            None,
            "t",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None
        )
        .is_err());
        assert!(create(
            &conn,
            "09-18",
            None,
            None,
            "t",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None
        )
        .is_err());
        assert!(create(
            &conn,
            "2026-09-18",
            None,
            None,
            "t",
            None,
            0,
            "medium",
            // monthly/yearly 已是合法规则；用真正未定义的字面量验证拒绝路径
            "monthlys",
            None,
            false,
            None
        )
        .is_err());
    }

    #[test]
    fn update_set_status_and_delete() {
        let conn = setup();
        let t = create(
            &conn,
            "2026-09-18",
            None,
            None,
            "任务",
            None,
            1,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        let u = update(
            &conn,
            &t.id,
            "2026-09-18",
            Some("10:00"),
            Some("11:00"),
            "改名",
            None,
            3,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(u.title, "改名");
        assert_eq!(u.estimate_pomodoros, 3);
        assert_eq!(u.start_time.as_deref(), Some("10:00"));
        let d = set_status(&conn, &t.id, "done").unwrap();
        assert_eq!(d.status, "done");
        assert!(set_status(&conn, &t.id, "running").is_err());
        delete(&conn, &t.id).unwrap();
        assert!(get(&conn, &t.id).is_err());
        assert!(delete(&conn, &t.id).is_err());
    }

    #[test]
    fn list_orders_by_start_time_nulls_last() {
        let conn = setup();
        create(
            &conn,
            "2026-09-18",
            Some("13:00"),
            None,
            "午后",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            None,
            None,
            "无时间",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            Some("09:00"),
            None,
            "清晨",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-19",
            Some("08:00"),
            None,
            "明天",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        let list = list_by_date(&conn, "2026-09-18").unwrap();
        let titles: Vec<&str> = list.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["清晨", "午后", "无时间"]);
    }

    #[test]
    fn repeat_matches_rules() {
        // daily：锚点起每天；之前不匹配
        assert!(repeat_matches("2026-09-18", "2026-09-19", REPEAT_DAILY));
        assert!(!repeat_matches("2026-09-18", "2026-09-17", REPEAT_DAILY));
        // weekly：同 weekday
        assert!(repeat_matches("2026-09-18", "2026-09-25", REPEAT_WEEKLY)); // 都是周五
        assert!(!repeat_matches("2026-09-18", "2026-09-19", REPEAT_WEEKLY));
        // weekday：周六周日不匹配
        assert!(repeat_matches("2026-09-18", "2026-09-21", REPEAT_WEEKDAY)); // 周一
        assert!(!repeat_matches("2026-09-18", "2026-09-19", REPEAT_WEEKDAY)); // 周六
        assert!(!repeat_matches("2026-09-18", "2026-09-20", REPEAT_WEEKDAY)); // 周日
        assert!(!repeat_matches("bad", "2026-09-18", REPEAT_DAILY));
    }

    #[test]
    fn repeat_matches_monthly_yearly_clamp() {
        // 每月 31 日：2 月 clamp 到 28（2026 平年）
        assert!(repeat_matches("2026-01-31", "2026-02-28", REPEAT_MONTHLY));
        assert!(!repeat_matches("2026-01-31", "2026-02-27", REPEAT_MONTHLY));
        // 平常月份按日号精确匹配
        assert!(repeat_matches("2026-01-31", "2026-03-31", REPEAT_MONTHLY));
        assert!(!repeat_matches("2026-01-31", "2026-03-30", REPEAT_MONTHLY));
        assert!(repeat_matches("2026-01-15", "2026-06-15", REPEAT_MONTHLY));
        assert!(!repeat_matches("2026-01-15", "2026-06-16", REPEAT_MONTHLY));
        // 每年 2-29：平年 clamp 到 2-28，闰年 2-29
        assert!(repeat_matches("2024-02-29", "2027-02-28", REPEAT_YEARLY));
        assert!(repeat_matches("2024-02-29", "2028-02-29", REPEAT_YEARLY));
        assert!(!repeat_matches("2024-02-29", "2027-03-01", REPEAT_YEARLY));
        // 锚点之前 / 非法日期不匹配
        assert!(!repeat_matches("2026-05-10", "2026-05-09", REPEAT_MONTHLY));
        assert!(!repeat_matches("bad", "2026-02-28", REPEAT_MONTHLY));
    }

    #[test]
    fn materialize_recurring_is_idempotent_and_skips_templates_in_day_view() {
        let conn = setup();
        create(
            &conn,
            "2026-09-17",
            Some("09:00"),
            None,
            "每日站会",
            None,
            1,
            "medium",
            REPEAT_DAILY,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-11",
            Some("14:00"),
            None,
            "周会",
            None,
            2,
            "high",
            REPEAT_WEEKLY,
            None,
            false,
            None,
        )
        .unwrap();
        let n1 = materialize_recurring(&conn, "2026-09-18").unwrap();
        assert_eq!(n1, 2, "daily 每天 + weekly 周五 9-18 生成");
        // 幂等
        assert_eq!(materialize_recurring(&conn, "2026-09-18").unwrap(), 0);
        // 实例出现在日视图，模板不出现在锚点日之前的视图
        let list = list_by_date(&conn, "2026-09-18").unwrap();
        assert_eq!(list.len(), 2);
        assert!(list
            .iter()
            .all(|t| t.source_task_id.is_some() && t.repeat_rule == REPEAT_NONE));
        // 用户删实例后同日不再自动重建（已删除 = 不存在,但幂等键也查不到 → 会重建。
        // 语义取舍：删除模板才不再生成；删除实例后重物化会重建，可接受并在此固化）
        let first = &list[0];
        delete(&conn, &first.id).unwrap();
        assert_eq!(materialize_recurring(&conn, "2026-09-18").unwrap(), 1);
    }

    #[test]
    fn weekly_template_anchor_friday_generates_only_fridays() {
        let conn = setup();
        create(
            &conn,
            "2026-09-18",
            Some("10:00"),
            None,
            "周五例会",
            None,
            0,
            "medium",
            REPEAT_WEEKLY,
            None,
            false,
            None,
        )
        .unwrap();
        // 2026-09-19 是周六
        assert_eq!(materialize_recurring(&conn, "2026-09-19").unwrap(), 0);
        // 2026-09-25 是周五
        assert_eq!(materialize_recurring(&conn, "2026-09-25").unwrap(), 1);
    }

    #[test]
    fn notification_scan_and_mark() {
        let conn = setup();
        create(
            &conn,
            "2026-09-18",
            Some("10:30"),
            None,
            "写代码",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            Some("15:00"),
            None,
            "开会",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        // 10:30 时刻只到"写代码"
        let due = due_for_notification(&conn, "2026-09-18", "10:30").unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].title, "写代码");
        mark_notified(&conn, &[due[0].id.clone()]).unwrap();
        assert!(due_for_notification(&conn, "2026-09-18", "10:30")
            .unwrap()
            .is_empty());
        // 15:00 时刻两条都过线，但已通知的不再出现
        let due2 = due_for_notification(&conn, "2026-09-18", "15:00").unwrap();
        assert_eq!(due2.len(), 1);
        assert_eq!(due2[0].title, "开会");
        // 已完成的不通知
        let t3 = create(
            &conn,
            "2026-09-18",
            Some("08:00"),
            None,
            "早读",
            None,
            0,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        set_status(&conn, &t3.id, "done").unwrap();
        let due3 = due_for_notification(&conn, "2026-09-18", "23:59").unwrap();
        assert!(due3.iter().all(|t| t.title != "早读"));
    }

    #[test]
    fn stats_counts_done_skipped_and_minutes() {
        let conn = setup();
        let a = create(
            &conn,
            "2026-09-18",
            Some("09:00"),
            Some("10:00"),
            "A",
            None,
            2,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            Some("10:00"),
            Some("11:30"),
            "B",
            None,
            1,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            None,
            None,
            "C",
            None,
            0,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        set_status(&conn, &a.id, "done").unwrap();
        incr_completed(&conn, &a.id).unwrap();
        let s = stats(&conn, "2026-09-18").unwrap();
        assert_eq!(s.total, 3);
        assert_eq!(s.done, 1);
        assert_eq!(s.estimate_pomodoros, 3);
        assert_eq!(s.completed_pomodoros, 1);
        assert_eq!(s.planned_focus_minutes, 60, "只计已完成任务 A 的 60 分钟");
    }

    #[test]
    fn set_time_and_incr_completed() {
        let conn = setup();
        let t = create(
            &conn,
            "2026-09-18",
            Some("09:00"),
            Some("10:00"),
            "A",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        let u = set_time(&conn, &t.id, Some("11:00"), Some("12:30")).unwrap();
        assert_eq!(u.start_time.as_deref(), Some("11:00"));
        assert!(set_time(&conn, &t.id, Some("9:00"), None).is_err());
        incr_completed(&conn, &t.id).unwrap();
        assert_eq!(get(&conn, &t.id).unwrap().completed_pomodoros, 1);
    }

    #[test]
    fn focus_min_roundtrip_and_recurring_propagation() {
        let conn = setup();
        // 自定义专注时长落库
        let t = create(
            &conn,
            "2026-09-18",
            None,
            None,
            "长任务",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            Some(45),
        )
        .unwrap();
        assert_eq!(t.focus_min, Some(45));
        // 更新改时长 / 清除回默认
        let u = update(
            &conn,
            &t.id,
            "2026-09-18",
            None,
            None,
            "长任务",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            Some(15),
        )
        .unwrap();
        assert_eq!(u.focus_min, Some(15));
        let c = create(
            &conn,
            "2026-09-18",
            None,
            None,
            "普通",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        assert_eq!(c.focus_min, None);
        // 重复模板的 focus_min 传播到实例
        create(
            &conn,
            "2026-09-17",
            Some("09:00"),
            None,
            "每日长专注",
            None,
            0,
            "medium",
            REPEAT_DAILY,
            None,
            false,
            Some(50),
        )
        .unwrap();
        materialize_recurring(&conn, "2026-09-18").unwrap();
        let list = list_by_date(&conn, "2026-09-18").unwrap();
        let inst = list.iter().find(|x| x.title == "每日长专注").unwrap();
        assert_eq!(inst.focus_min, Some(50), "模板 focus_min 传播到实例");
    }

    #[test]
    fn list_by_range_covers_days_in_order() {
        let conn = setup();
        create(
            &conn,
            "2026-09-15",
            Some("09:00"),
            None,
            "周一",
            None,
            0,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-17",
            None,
            None,
            "周三",
            None,
            0,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        // 范围外 + 重复模板不入结果
        create(
            &conn,
            "2026-09-20",
            None,
            None,
            "范围外",
            None,
            0,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-15",
            Some("08:00"),
            None,
            "模板",
            None,
            0,
            "medium",
            REPEAT_DAILY,
            None,
            false,
            None,
        )
        .unwrap();
        let list = list_by_range(&conn, "2026-09-14", "2026-09-19").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title, "周一");
        assert_eq!(list[1].title, "周三");
    }

    #[test]
    fn postpone_moves_instances_skips_templates_and_resets_notify() {
        let conn = setup();
        let a = create(
            &conn,
            "2026-09-18",
            Some("09:00"),
            None,
            "甲",
            None,
            1,
            "medium",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-18",
            Some("10:00"),
            None,
            "模板",
            None,
            0,
            "high",
            REPEAT_DAILY,
            None,
            false,
            None,
        )
        .unwrap();
        // 标记已通知，顺延后应复位（新的一天需要重新到点通知）
        mark_notified(&conn, &[a.id.clone()]).unwrap();
        let n = postpone_to(&conn, &[a.id.clone(), "nonexistent".into()], "2026-09-19").unwrap();
        assert_eq!(n, 1, "模板与不存在的 id 被忽略");
        let moved = get(&conn, &a.id).unwrap();
        assert_eq!(moved.date, "2026-09-19");
        assert!(!moved.start_notified, "顺延后通知标记复位");
        // 非法目标日期报错
        assert!(postpone_to(&conn, &[a.id.clone()], "bad").is_err());
    }

    #[test]
    fn search_filters() {
        let conn = setup();
        create(
            &conn,
            "2026-09-18",
            None,
            None,
            "写周报",
            None,
            0,
            "medium",
            REPEAT_NONE,
            Some("工作"),
            false,
            None,
        )
        .unwrap();
        create(
            &conn,
            "2026-09-19",
            None,
            None,
            "买菜",
            None,
            0,
            "low",
            REPEAT_NONE,
            None,
            false,
            None,
        )
        .unwrap();
        let all = search(&conn, "", None, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(search(&conn, "周报", None, None).unwrap().len(), 1);
        assert_eq!(search(&conn, "", Some("todo"), None).unwrap().len(), 2);
        assert_eq!(
            search(&conn, "", None, Some("2026-09-18")).unwrap().len(),
            1
        );
    }
}
