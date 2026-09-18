//! 任务元数据（task_meta 表）DAO 与待办行回写纯函数。
//!
//! - `task_key`：任务行的稳定键（由前端 todo 模块对行文本派生）；
//! - `skip_date`：跳过当天的本地日期（"YYYY-MM-DD"），跨日由 [`effective_status`]
//!   判定自动复活为 todo（视图读取端调用，不回写库）；
//! - `mark_done_content_line`：把便签正文里第一条匹配的 `- [ ] ` 行勾选为 `[x]`，
//!   供 complete_task 命令回写正文（待办真相源 = 正文里的复选框）。

use chrono::Local;
use rusqlite::{params, Connection, Row};

use super::models::TaskMeta;
use crate::error::AppResult;

const COLS: &str = "note_id, task_key, line_text, status, estimate_pomodoros, completed_pomodoros, priority, due_at, skip_date, focus_min, updated_at";

fn row_to_meta(r: &Row) -> rusqlite::Result<TaskMeta> {
    Ok(TaskMeta {
        note_id: r.get("note_id")?,
        task_key: r.get("task_key")?,
        line_text: r.get("line_text")?,
        status: r.get("status")?,
        estimate_pomodoros: r.get("estimate_pomodoros")?,
        completed_pomodoros: r.get("completed_pomodoros")?,
        priority: r.get("priority")?,
        due_at: r.get("due_at")?,
        skip_date: r.get("skip_date")?,
        focus_min: r.get::<_, Option<i64>>("focus_min")?,
        updated_at: r.get("updated_at")?,
    })
}

fn now_str() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// 本地今天（"YYYY-MM-DD"），skip_date 的统一写入口径。
pub fn local_today() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

/// 部分更新载荷：None 字段保留原值（新行则用列默认）。
#[derive(Debug, Clone)]
pub struct TaskMetaUpsert {
    pub note_id: String,
    pub task_key: String,
    /// 最新任务行文本（总是覆盖，便于正文改动后同步）
    pub line_text: String,
    /// Some = 变更状态；None = 保留
    pub status: Option<String>,
    pub estimate: Option<i64>,
    pub priority: Option<String>,
    pub due_at: Option<String>,
    /// 本任务专注时长覆盖（分钟；None = 保留原值/默认）
    pub focus_min: Option<i64>,
    /// true = 状态重置为 todo 并清除 skip_date（手动"取消跳过"）
    pub clear_skip: bool,
}

/// 插入或部分更新一条任务元数据，返回更新后的行。
///
/// 状态语义：
/// - `clear_skip=true` → status='todo'、skip_date=NULL；
/// - `status='skipped'` → 自动写 skip_date=本地今天；
/// - `status='done'` → 同样清除 skip_date；
/// - 状态发生变化时刷新 updated_at（仅 estimate 等字段变更不刷新，避免污染统计窗口）。
pub fn upsert(conn: &Connection, u: &TaskMetaUpsert) -> AppResult<TaskMeta> {
    let existing = get(conn, &u.note_id, &u.task_key)?;

    let mut status = existing
        .as_ref()
        .map(|m| m.status.clone())
        .unwrap_or_else(|| "todo".to_string());
    let mut skip_date = existing.as_ref().and_then(|m| m.skip_date.clone());

    if u.clear_skip {
        status = "todo".to_string();
        skip_date = None;
    } else if let Some(s) = u.status.as_deref() {
        match s {
            "skipped" => {
                status = "skipped".to_string();
                skip_date = Some(local_today());
            }
            "done" => {
                status = "done".to_string();
                skip_date = None;
            }
            other => status = other.to_string(),
        }
    }

    let status_changed = match &existing {
        None => true,
        Some(m) => m.status != status || m.skip_date != skip_date,
    };
    let updated_at = if status_changed {
        now_str()
    } else {
        existing
            .as_ref()
            .map(|m| m.updated_at.clone())
            .unwrap_or_else(now_str)
    };

    let estimate = u
        .estimate
        .unwrap_or_else(|| existing.as_ref().map(|m| m.estimate_pomodoros).unwrap_or(0));
    let priority = u
        .priority
        .clone()
        .or_else(|| existing.as_ref().and_then(|m| m.priority.clone()));
    let due_at = u
        .due_at
        .clone()
        .or_else(|| existing.as_ref().and_then(|m| m.due_at.clone()));
    let completed = existing
        .as_ref()
        .map(|m| m.completed_pomodoros)
        .unwrap_or(0);
    let focus_min = u
        .focus_min
        .or_else(|| existing.as_ref().and_then(|m| m.focus_min));

    conn.execute(
        "INSERT INTO task_meta \
             (note_id, task_key, line_text, status, estimate_pomodoros, completed_pomodoros, priority, due_at, skip_date, focus_min, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
         ON CONFLICT(note_id, task_key) DO UPDATE SET \
             line_text = ?3, status = ?4, estimate_pomodoros = ?5, priority = ?7, due_at = ?8, skip_date = ?9, focus_min = ?10, updated_at = ?11",
        params![
            u.note_id,
            u.task_key,
            u.line_text,
            status,
            estimate,
            completed,
            priority,
            due_at,
            skip_date,
            focus_min,
            updated_at
        ],
    )?;

    get(conn, &u.note_id, &u.task_key)?
        .ok_or_else(|| crate::error::AppError::Db("task_meta upsert 后读取失败".into()))
}

/// 单条读取（不存在返回 None）。
pub fn get(conn: &Connection, note_id: &str, task_key: &str) -> AppResult<Option<TaskMeta>> {
    let r = conn.query_row(
        &format!("SELECT {COLS} FROM task_meta WHERE note_id = ?1 AND task_key = ?2"),
        params![note_id, task_key],
        row_to_meta,
    );
    match r {
        Ok(m) => Ok(Some(m)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// 某便签下全部任务元数据（updated_at DESC）。
pub fn list_for_note(conn: &Connection, note_id: &str) -> AppResult<Vec<TaskMeta>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM task_meta WHERE note_id = ?1 ORDER BY updated_at DESC"
    ))?;
    let rows = stmt
        .query_map(params![note_id], row_to_meta)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 番茄钟焦点完成：completed_pomodoros + 1（行不存在时静默忽略）。
pub fn incr_completed(conn: &Connection, note_id: &str, task_key: &str) -> AppResult<()> {
    conn.execute(
        "UPDATE task_meta SET completed_pomodoros = completed_pomodoros + 1 \
         WHERE note_id = ?1 AND task_key = ?2",
        params![note_id, task_key],
    )?;
    Ok(())
}

/// 生效状态（纯函数，不回写库）：skipped 且 skip_date < today → 复活为 'todo'；
/// 其余（含 skip_date 缺失的异常行、done、todo）原样返回。
#[allow(dead_code)]
pub fn effective_status(meta: &TaskMeta, today: &str) -> String {
    match (meta.status.as_str(), meta.skip_date.as_deref()) {
        ("skipped", Some(d)) if d < today => "todo".to_string(),
        _ => meta.status.clone(),
    }
}

/// 任务文本规范化：trim + 连续空白折叠为单空格（匹配口径与 `mark_done_content_line` 一致）。
pub fn normalize_task_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 在正文中找到第 1 个"去掉行首缩进后、复选框后的文本规范化后恰等于 line_text"
/// 的 `- [ ] ` 行，把该行的 `[ ]` 改为 `[x]`，返回新正文。
/// 找不到或目标为空 / 该行已是 done 时返回 None。
pub fn mark_done_content_line(content: &str, line_text: &str) -> Option<String> {
    let target = normalize_task_text(line_text);
    if target.is_empty() {
        return None;
    }
    let mut changed = false;
    let mut out: Vec<String> = Vec::with_capacity(content.lines().count());
    for line in content.lines() {
        let mut l = line.to_string();
        if !changed {
            let trimmed = line.trim_start();
            let indent_len = line.len() - trimmed.len();
            if let Some(rest) = trimmed.strip_prefix("- [ ]") {
                // 只匹配标准 "- [ ] "（后跟空格）形式
                if rest.starts_with(' ') && normalize_task_text(rest) == target {
                    l = format!(
                        "{}- [x]{}",
                        &line[..indent_len],
                        rest.replacen("[ ]", "[x]", 1)
                    );
                    changed = true;
                }
            }
        }
        out.push(l);
    }
    if changed {
        Some(out.join("\n"))
    } else {
        None
    }
}

// ---------- 任务身份算法（与前端 taskKey.ts 严格对齐） ----------

/// FNV-1a 32 位哈希（按 UTF-16 码元迭代，与 JS charCodeAt 完全一致），
/// 返回 8 位十六进制。金标准：fnv1a32("写周报") = "12943e83"。
pub fn fnv1a32(input: &str) -> String {
    let mut h: u32 = 0x811c_9dc5;
    for unit in input.encode_utf16() {
        h ^= unit as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

/// 从便签正文解析待办行：(行文本, 是否勾选)。匹配规则与前端 parseTodos 一致。
fn extract_task_lines(content: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    for line in content.split('\n') {
        let t = line.trim_start();
        let rest = match t
            .strip_prefix("- ")
            .or_else(|| t.strip_prefix("* "))
            .or_else(|| t.strip_prefix("+ "))
        {
            Some(r) => r,
            None => continue,
        };
        // rest 形如 "[ ] 文本" / "[x] 文本" / "[X] 文本"
        if !rest.starts_with('[') || rest.len() < 3 {
            continue;
        }
        let mark = rest.as_bytes()[1];
        let checked = mark == b'x' || mark == b'X';
        let body = &rest[3..];
        let text = body.strip_prefix(' ').unwrap_or(body).to_string();
        out.push((text, checked));
    }
    out
}

/// 内容变更后同步 task_meta（双向真源规则）：
/// - 勾选 `[x]` → meta.status = 'done'
/// - 取消勾选 `[ ]` 且 meta 原为 done → 回退 'todo'
/// - meta 为 skipped → 不受勾选影响（跳过是独立维度）
/// - 无 meta 的未勾选行不建行（避免空记录膨胀）
///
/// 返回更新的行数。
pub fn sync_task_meta_from_content(
    conn: &Connection,
    note_id: &str,
    content: &str,
) -> AppResult<usize> {
    let mut changed = 0usize;
    let mut seen: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for (raw_text, checked) in extract_task_lines(content) {
        let norm = normalize_task_text(&raw_text);
        let occurrence = seen.entry(norm.clone()).or_insert(0);
        let task_key = format!("{}-{}", fnv1a32(&norm), occurrence);
        *occurrence += 1;

        let existing = conn
            .query_row(
                "SELECT status FROM task_meta WHERE note_id = ?1 AND task_key = ?2",
                params![note_id, task_key],
                |r| r.get::<_, String>(0),
            )
            .ok();

        match existing.as_deref() {
            Some("skipped") => continue, // 跳过态不受勾选影响
            Some("done") if checked => continue,
            Some("done") if !checked => {
                conn.execute(
                    "UPDATE task_meta SET status = 'todo', updated_at = ?3                      WHERE note_id = ?1 AND task_key = ?2",
                    params![note_id, task_key, chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()],
                )?;
                changed += 1;
            }
            _ if checked => {
                conn.execute(
                    "INSERT INTO task_meta (note_id, task_key, line_text, status, updated_at)                      VALUES (?1, ?2, ?3, 'done', ?4)                      ON CONFLICT(note_id, task_key) DO UPDATE SET status = 'done', updated_at = ?4",
                    params![note_id, task_key, raw_text.trim(), chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()],
                )?;
                changed += 1;
            }
            _ => {}
        }
    }
    Ok(changed)
}

/// 删除某便签的全部任务元数据（便签永久删除时调用）。
pub fn delete_for_note(conn: &Connection, note_id: &str) -> AppResult<usize> {
    let n = conn.execute("DELETE FROM task_meta WHERE note_id = ?1", params![note_id])?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;

    #[test]
    fn fnv1a32_cross_language_golden() {
        // 与前端 taskKey.test.ts 的金标准一致（node 实测值）
        assert_eq!(fnv1a32(""), "811c9dc5");
        assert_eq!(fnv1a32("写周报"), "12943e83");
        assert_eq!(fnv1a32("窗口渲染正常"), "d294d34f");
    }

    #[test]
    fn sync_creates_done_and_reverts_unchecked() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        let note = crate::db::notes::create(&conn, "t", "").unwrap();
        let content = "# 实机验收\n- [ ] 任务甲\n- [x] 任务乙";
        sync_task_meta_from_content(&conn, &note.id, content).unwrap();
        // 甲：未勾选无 meta → 不建行
        // 乙：勾选 → 建 done
        let metas = list_for_note(&conn, &note.id).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].status, "done");
        assert_eq!(metas[0].line_text, "任务乙");

        // 取消勾选乙 → meta 回退 todo
        sync_task_meta_from_content(&conn, &note.id, "# 实机验收\n- [ ] 任务甲\n- [ ] 任务乙")
            .unwrap();
        let metas = list_for_note(&conn, &note.id).unwrap();
        let yi = metas.iter().find(|m| m.line_text == "任务乙").unwrap();
        assert_eq!(yi.status, "todo");
    }

    #[test]
    fn sync_preserves_skipped() {
        let conn = Connection::open_in_memory().unwrap();
        migrations::run(&conn).unwrap();
        let note = crate::db::notes::create(&conn, "t", "- [ ] 被跳过的任务").unwrap();
        upsert(
            &conn,
            &TaskMetaUpsert {
                note_id: note.id.clone(),
                focus_min: None,
                task_key: crate::db::task_meta::fnv1a32("被跳过的任务") + "-0",
                line_text: "被跳过的任务".into(),
                status: Some("skipped".into()),
                estimate: None,
                priority: None,
                due_at: None,
                clear_skip: false,
            },
        )
        .unwrap();
        sync_task_meta_from_content(&conn, &note.id, "- [ ] 被跳过的任务").unwrap();
        let metas = list_for_note(&conn, &note.id).unwrap();
        assert_eq!(metas[0].status, "skipped", "跳过态不受未勾选同步影响");
    }

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        conn
    }

    fn ups(note_id: &str, task_key: &str, line_text: &str) -> TaskMetaUpsert {
        TaskMetaUpsert {
            note_id: note_id.into(),
            focus_min: None,
            task_key: task_key.into(),
            line_text: line_text.into(),
            status: None,
            estimate: None,
            priority: None,
            due_at: None,
            clear_skip: false,
        }
    }

    #[test]
    fn upsert_inserts_with_defaults_and_is_idempotent() {
        let conn = setup();
        let m1 = upsert(&conn, &ups("n1", "k1", "- [ ] 买牛奶")).unwrap();
        assert_eq!(m1.status, "todo");
        assert_eq!(m1.estimate_pomodoros, 0);
        assert_eq!(m1.completed_pomodoros, 0);
        assert_eq!(m1.priority, None);
        assert_eq!(m1.skip_date, None);
        assert!(!m1.updated_at.is_empty());

        let m2 = upsert(&conn, &ups("n1", "k1", "- [ ] 买牛奶")).unwrap();
        assert_eq!(m2.updated_at, m1.updated_at, "无字段变更不刷新 updated_at");
        assert_eq!(
            list_for_note(&conn, "n1").unwrap().len(),
            1,
            "幂等：不产生新行"
        );
    }

    #[test]
    fn upsert_partial_update_keeps_other_fields() {
        let conn = setup();
        upsert(
            &conn,
            &TaskMetaUpsert {
                estimate: Some(2),
                priority: Some("high".into()),
                due_at: Some("2026-09-20".into()),
                ..ups("n1", "k1", "- [ ] 写周报")
            },
        )
        .unwrap();
        let m = upsert(&conn, &ups("n1", "k1", "- [ ] 写周报！")).unwrap();
        assert_eq!(m.estimate_pomodoros, 2, "None 字段保留原值");
        assert_eq!(m.priority.as_deref(), Some("high"));
        assert_eq!(m.due_at.as_deref(), Some("2026-09-20"));
        assert_eq!(m.line_text, "- [ ] 写周报！", "line_text 总是覆盖");
        assert_eq!(m.status, "todo");
    }

    #[test]
    fn skip_writes_today_and_clear_skip_resets() {
        let conn = setup();
        let m = upsert(
            &conn,
            &TaskMetaUpsert {
                status: Some("skipped".into()),
                ..ups("n1", "k1", "- [ ] 跳过我")
            },
        )
        .unwrap();
        assert_eq!(m.status, "skipped");
        assert_eq!(m.skip_date.as_deref(), Some(local_today().as_str()));

        let m2 = upsert(
            &conn,
            &TaskMetaUpsert {
                clear_skip: true,
                ..ups("n1", "k1", "- [ ] 跳过我")
            },
        )
        .unwrap();
        assert_eq!(m2.status, "todo", "clear_skip 复位为 todo");
        assert_eq!(m2.skip_date, None);
    }

    #[test]
    fn done_clears_skip_date() {
        let conn = setup();
        upsert(
            &conn,
            &TaskMetaUpsert {
                status: Some("skipped".into()),
                ..ups("n1", "k1", "- [ ] a")
            },
        )
        .unwrap();
        let m = upsert(
            &conn,
            &TaskMetaUpsert {
                status: Some("done".into()),
                ..ups("n1", "k1", "- [ ] a")
            },
        )
        .unwrap();
        assert_eq!(m.status, "done");
        assert_eq!(m.skip_date, None);
    }

    #[test]
    fn effective_status_revives_skipped_after_day_rolls() {
        let mut m = TaskMeta {
            note_id: "n1".into(),
            task_key: "k1".into(),
            line_text: "- [ ] a".into(),
            status: "skipped".into(),
            estimate_pomodoros: 0,
            completed_pomodoros: 0,
            priority: None,
            due_at: None,
            skip_date: Some("2026-09-14".into()),
            focus_min: None,
            updated_at: String::new(),
        };
        assert_eq!(effective_status(&m, "2026-09-15"), "todo", "跨日自动复活");
        assert_eq!(
            effective_status(&m, "2026-09-14"),
            "skipped",
            "当天保持跳过"
        );

        m.skip_date = None;
        assert_eq!(
            effective_status(&m, "2026-09-15"),
            "skipped",
            "无 skip_date 不复活（异常行保守处理）"
        );

        m.status = "done".into();
        assert_eq!(effective_status(&m, "2026-09-15"), "done", "done 不受影响");
        m.status = "todo".into();
        assert_eq!(effective_status(&m, "2026-09-15"), "todo");
    }

    #[test]
    fn list_for_note_orders_by_updated_at_desc() {
        let conn = setup();
        upsert(&conn, &ups("n1", "k1", "- [ ] a")).unwrap();
        upsert(&conn, &ups("n1", "k2", "- [ ] b")).unwrap();
        // updated_at 精度为秒：直接改时间戳保证顺序确定（避免同秒并列的偶发顺序）
        conn.execute(
            "UPDATE task_meta SET updated_at = '2026-09-16T12:00:00+00:00' WHERE task_key = 'k1'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE task_meta SET updated_at = '2026-09-16T13:00:00+00:00' WHERE task_key = 'k2'",
            [],
        )
        .unwrap();
        let list = list_for_note(&conn, "n1").unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].task_key, "k2", "更新的在前");
        assert_eq!(list[1].task_key, "k1");
        assert!(list_for_note(&conn, "不存在").unwrap().is_empty());
    }

    #[test]
    fn incr_completed_counts_focus_finishes() {
        let conn = setup();
        upsert(&conn, &ups("n1", "k1", "- [ ] a")).unwrap();
        incr_completed(&conn, "n1", "k1").unwrap();
        incr_completed(&conn, "n1", "k1").unwrap();
        incr_completed(&conn, "n1", "k2").unwrap(); // 不存在的行：静默
        assert_eq!(
            get(&conn, "n1", "k1").unwrap().unwrap().completed_pomodoros,
            2
        );
    }

    // ---------- mark_done_content_line（纯函数） ----------

    #[test]
    fn mark_done_basic_and_returns_new_content() {
        let out = mark_done_content_line("- [ ] 买牛奶\n- [ ] 写代码", "买牛奶").unwrap();
        assert_eq!(out, "- [x] 买牛奶\n- [ ] 写代码");
    }

    #[test]
    fn mark_done_preserves_indentation() {
        let out = mark_done_content_line("# 列表\n    - [ ] 缩进任务", "缩进任务").unwrap();
        assert_eq!(out, "# 列表\n    - [x] 缩进任务");
    }

    #[test]
    fn mark_done_takes_first_duplicate() {
        let out = mark_done_content_line("- [ ] 重复\n- [ ] 重复", "重复").unwrap();
        assert_eq!(out, "- [x] 重复\n- [ ] 重复", "只改第一处");
    }

    #[test]
    fn mark_done_normalizes_whitespace() {
        // 目标文本与行内文本的空白差异被规范化吸收
        let out = mark_done_content_line("- [ ]  买   牛奶 ", "买 牛奶").unwrap();
        assert_eq!(out, "- [x]  买   牛奶 ");
        let out2 = mark_done_content_line("- [ ] \t带\t制表符", "带 制表符").unwrap();
        assert!(out2.starts_with("- [x]"));
    }

    #[test]
    fn mark_done_multiline_document() {
        let content = "# 标题\n\n普通文字\n- [ ] 任务甲\n中间段落\n  - [ ] 任务乙\n- [x] 已完成";
        let out = mark_done_content_line(content, "任务乙").unwrap();
        assert!(out.contains("  - [x] 任务乙"));
        assert!(out.contains("- [ ] 任务甲"), "其他行不动");
        assert!(out.contains("- [x] 已完成"));
    }

    #[test]
    fn mark_done_not_found_returns_none() {
        assert!(mark_done_content_line("- [ ] A", "B").is_none());
        assert!(mark_done_content_line("", "A").is_none());
        assert!(
            mark_done_content_line("- [ ] A", "  ").is_none(),
            "空目标直接 None"
        );
    }

    #[test]
    fn mark_done_already_done_returns_none() {
        assert!(mark_done_content_line("- [x] 已完成的事", "已完成的事").is_none());
    }

    #[test]
    fn normalize_task_text_collapses_whitespace() {
        assert_eq!(normalize_task_text("  a   b\tc  "), "a b c");
        assert_eq!(normalize_task_text(""), "");
    }
}
