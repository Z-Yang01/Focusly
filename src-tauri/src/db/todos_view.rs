//! 待办到期视图：扫描 active 未删便签里的未完成任务行 `^YYYY-MM-DD` 到期标记，
//! 按本地"今天"分为 逾期 / 今天 / 未来7天 三个区。
//!
//! 任务行语法（与前端 src/features/todo/due.ts 保持一致）：
//!   `- [ ] 任务文本 ^2026-09-20`（行尾到期日，可有 `!高/!中/!低` 优先级标记）
//!
//! 注意：SQL 中 `AND deleted_at IS NULL AND is_private = 0` 依赖 v2 迁移新增的列
//! （软删除列 deleted_at、私密标记 is_private，见 migrations v2）；
//! 测试里带了补列垫片（见 tests::setup），v2 落地后垫片自动跳过。

use chrono::{Duration, Local, NaiveDate};
use rusqlite::{Connection, Row};
use serde::Serialize;

use super::models::{Note, NoteSummary};
use crate::error::AppResult;

/// 单条到期视图条目：NoteSummary 平铺 + 该分区内的到期任务数。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DueNoteSummary {
    #[serde(flatten)]
    pub summary: NoteSummary,
    /// 该条目在所属分区（逾期/今天/未来7天）内的到期任务数
    pub due_count: i64,
}

/// 到期视图：三段分区，每区内按 updated_at DESC。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoView {
    pub overdue: Vec<DueNoteSummary>,
    pub today: Vec<DueNoteSummary>,
    pub next7days: Vec<DueNoteSummary>,
}

// notes.rs 的 row_to_note/NOTE_COLS 是私有的，为了不动那个文件这里本地保留一份
// （列清单与 v2 迁移后的 notes 表一致：deleted_at/is_private/locked/readonly_flag/scale）。
const NOTE_COLS: &str =
    "id, title, content, content_format, status, is_pinned, is_always_on_top, show_on_all_desktops, \
     desktop_pin_state, fullscreen_behavior, x, y, width, height, monitor_id, created_at, updated_at, archived_at, \
     deleted_at, is_private, locked, readonly_flag, scale";

fn row_to_note(r: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get("id")?,
        title: r.get("title")?,
        content: r.get("content")?,
        content_format: r.get("content_format")?,
        status: r.get("status")?,
        is_pinned: r.get::<_, i64>("is_pinned")? != 0,
        is_always_on_top: r.get::<_, i64>("is_always_on_top")? != 0,
        show_on_all_desktops: r.get::<_, i64>("show_on_all_desktops")? != 0,
        desktop_pin_state: r.get("desktop_pin_state")?,
        fullscreen_behavior: r.get("fullscreen_behavior")?,
        x: r.get("x")?,
        y: r.get("y")?,
        width: r.get("width")?,
        height: r.get("height")?,
        monitor_id: r.get("monitor_id")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        archived_at: r.get("archived_at")?,
        deleted_at: r.get("deleted_at")?,
        is_private: r.get::<_, i64>("is_private")? != 0,
        locked: r.get::<_, i64>("locked")? != 0,
        readonly: r.get::<_, i64>("readonly_flag")? != 0,
        scale: r.get("scale")?,
    })
}

/// 提取文本中最后一个合法的 `^YYYY-MM-DD` 到期日。
/// 用 `str::get` 取子串，避免多字节字符上按字节切片 panic。
fn extract_due(text: &str) -> Option<NaiveDate> {
    let bytes = text.as_bytes();
    let mut found = None;
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'^' {
            continue;
        }
        if let Some(cand) = text.get(i + 1..i + 11) {
            if let Ok(d) = NaiveDate::parse_from_str(cand, "%Y-%m-%d") {
                found = Some(d); // 多个标记取最后一个（最近编辑的生效）
            }
        }
    }
    found
}

/// 一行是否为未完成任务行（`- [ ] ` 前缀），返回复选框后的文本。
fn unchecked_task_text(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix("- [ ]")?;
    if rest.is_empty() || rest.starts_with(' ') {
        Some(rest)
    } else {
        None
    }
}

/// 统计内容里落在 (today, bucket) 各分区的未完成任务数。
/// 返回 (overdue, today, next7days)。
fn count_due_buckets(content: &str, today: NaiveDate) -> (i64, i64, i64) {
    let horizon = today + Duration::days(7);
    let mut o = 0i64;
    let mut t = 0i64;
    let mut n = 0i64;
    for line in content.lines() {
        let Some(text) = unchecked_task_text(line) else {
            continue;
        };
        let Some(due) = extract_due(text) else {
            continue;
        };
        if due < today {
            o += 1;
        } else if due == today {
            t += 1;
        } else if due <= horizon {
            n += 1;
        }
    }
    (o, t, n)
}

/// 待办到期视图。只看 active 且未软删除的便签；同一便签可出现在多个分区
/// （各区独立去重），区向量按 updated_at DESC（SQL 顺序透传）。
pub fn get_due_view(conn: &Connection) -> AppResult<TodoView> {
    // 依赖 v2 迁移的 deleted_at 列（软删除）；私密便签不出现在到期视图
    let sql = format!(
        "SELECT {NOTE_COLS} FROM notes \
         WHERE status = 'active' AND deleted_at IS NULL AND is_private = 0 \
         ORDER BY updated_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let notes = stmt.query_map([], row_to_note)?.collect::<rusqlite::Result<Vec<_>>>()?;

    let today = Local::now().date_naive();
    let mut view = TodoView::default();
    for note in notes {
        let (o, t, n) = count_due_buckets(&note.content, today);
        if o == 0 && t == 0 && n == 0 {
            continue;
        }
        let summary = super::notes::to_summary(conn, note)?;
        if o > 0 {
            view.overdue.push(DueNoteSummary { summary: summary.clone(), due_count: o });
        }
        if t > 0 {
            view.today.push(DueNoteSummary { summary: summary.clone(), due_count: t });
        }
        if n > 0 {
            view.next7days.push(DueNoteSummary { summary, due_count: n });
        }
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::super::notes;
    use super::*;
    use rusqlite::params;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        super::super::migrations::run(&conn).unwrap();
        // 测试垫片：v2 迁移（软删除列）落地前手工补 deleted_at，落地后此 ALTER 跳过
        let has_col: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('notes') WHERE name = 'deleted_at'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        if has_col == 0 {
            conn.execute("ALTER TABLE notes ADD COLUMN deleted_at TEXT", params![])
                .unwrap();
        }
        conn
    }

    fn offset_date(days: i64) -> String {
        let today = Local::now().date_naive();
        (today + Duration::days(days)).format("%Y-%m-%d").to_string()
    }

    #[test]
    fn extract_due_takes_last_valid() {
        let d = extract_due("买牛奶 ^2026-09-20").unwrap();
        assert_eq!(d.to_string(), "2026-09-20");
        // 多个取最后一个；非法日期被忽略
        let d2 = extract_due("a ^2026-09-01 b ^2026-10-05").unwrap();
        assert_eq!(d2.to_string(), "2026-10-05");
        assert!(extract_due("非法 ^2026-13-40").is_none());
        // ^ 后紧跟多字节字符：不得 panic
        assert!(extract_due("^中文").is_none());
        assert!(extract_due("没有标记").is_none());
    }

    #[test]
    fn buckets_by_local_today() {
        let conn = setup();
        notes::create(&conn, "逾期便签", &format!("- [ ] 交房租 ^{} !高", offset_date(-1))).unwrap();
        notes::create(&conn, "今日便签", &format!("- [ ] 站会 ^{}", offset_date(0))).unwrap();
        notes::create(&conn, "本周便签", &format!("- [ ] 复盘 ^{}", offset_date(3))).unwrap();
        notes::create(&conn, "无日期便签", "- [ ] 没有到期日").unwrap();
        notes::create(
            &conn,
            "混合便签",
            &format!(
                "- [ ] 今天的事 ^{}\n- [x] 已完成不算 ^{}\n- [ ] 六天后的事 ^{}",
                offset_date(0),
                offset_date(-5),
                offset_date(6)
            ),
        )
        .unwrap();

        let v = get_due_view(&conn).unwrap();
        assert_eq!(v.overdue.len(), 1);
        assert_eq!(v.overdue[0].summary.note.title, "逾期便签");
        assert_eq!(v.overdue[0].due_count, 1);

        // 今天：今日便签 + 混合便签（可同时出现在多个分区）
        assert_eq!(v.today.len(), 2);
        assert_eq!(v.next7days.len(), 2);

        let mixed_today = v
            .today
            .iter()
            .find(|e| e.summary.note.title == "混合便签")
            .unwrap();
        assert_eq!(mixed_today.due_count, 1, "已完成任务不计入");
        let mixed_next = v
            .next7days
            .iter()
            .find(|e| e.summary.note.title == "混合便签")
            .unwrap();
        assert_eq!(mixed_next.due_count, 1);

        // 无日期便签不出现在任何分区
        assert!(v
            .overdue
            .iter()
            .chain(v.today.iter())
            .chain(v.next7days.iter())
            .all(|e| e.summary.note.title != "无日期便签"));
    }

    #[test]
    fn excludes_archived_and_soft_deleted() {
        let conn = setup();
        let a = notes::create(&conn, "已归档", &format!("- [ ] x ^{}", offset_date(-1))).unwrap();
        notes::archive(&conn, &a.id).unwrap();
        let b = notes::create(&conn, "已删除", &format!("- [ ] y ^{}", offset_date(-1))).unwrap();
        conn.execute("UPDATE notes SET deleted_at = ?1 WHERE id = ?2", params!["2026-01-01T00:00:00Z", b.id])
            .unwrap();

        let v = get_due_view(&conn).unwrap();
        assert!(v.overdue.is_empty());
        assert!(v.today.is_empty());
        assert!(v.next7days.is_empty());
    }

    #[test]
    fn excludes_private_notes() {
        let conn = setup();
        notes::create(&conn, "私密便签", &format!("- [ ] x ^{}", offset_date(-1))).unwrap();
        conn.execute("UPDATE notes SET is_private = 1 WHERE title = '私密便签'", params![])
            .unwrap();
        let v = get_due_view(&conn).unwrap();
        assert!(v.overdue.is_empty(), "私密便签不应出现在到期视图");
    }

    #[test]
    fn horizon_is_seven_days() {
        let conn = setup();
        notes::create(&conn, "第8天", &format!("- [ ] 太远 ^{}", offset_date(8))).unwrap();
        let v = get_due_view(&conn).unwrap();
        assert!(v.next7days.is_empty(), "第 8 天不在未来7天分区");
    }
}
