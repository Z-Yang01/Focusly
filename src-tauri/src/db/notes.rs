use std::collections::HashMap;

use rusqlite::{params, Connection, Row};
use uuid::Uuid;

use super::models::{Note, NoteSummary};
use crate::error::{AppError, AppResult};

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// v2 起新功能（版本快照、软删除）统一秒级精度时间戳，便于同秒排序时用 rowid 决胜。
fn now_secs() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

pub(crate) fn row_to_note(r: &Row) -> rusqlite::Result<Note> {
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
        pin_mode: r.get("pin_mode")?,
    })
}

/// notes 表全列清单（单一事实源：todos_view 等本地投影统一引用，防止列漂移）。
pub(crate) const NOTE_COLS: &str =
    "id, title, content, content_format, status, is_pinned, is_always_on_top, show_on_all_desktops, \
     desktop_pin_state, fullscreen_behavior, x, y, width, height, monitor_id, created_at, updated_at, archived_at, \
     deleted_at, is_private, locked, readonly_flag, pin_mode, scale";

pub fn create(conn: &Connection, title: &str, content: &str) -> AppResult<Note> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    conn.execute(
        "INSERT INTO notes (id, title, content, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, content, ts, ts],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Note> {
    conn.query_row(
        &format!("SELECT {NOTE_COLS} FROM notes WHERE id = ?1"),
        params![id],
        row_to_note,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::Invalid(format!("便签不存在: {id}")),
        other => AppError::Db(other.to_string()),
    })
}

/// 便签存在性查询（DAO 契约补全；供未来导入去重/外键校验使用）
#[allow(dead_code)]
pub fn exists(conn: &Connection, id: &str) -> AppResult<bool> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM notes WHERE id=?1", params![id], |r| {
        r.get(0)
    })?;
    Ok(n > 0)
}

/// filter: all | active | archived | todo | trash
/// - active/archived/todo 均要求 deleted_at IS NULL（回收站内容不进入常规视图）
/// - trash: deleted_at IS NOT NULL（任意 status）
/// - all: 未删除的全部
pub fn list(conn: &Connection, filter: &str) -> AppResult<Vec<NoteSummary>> {
    let (where_clause, params_slice): (String, Vec<&dyn rusqlite::ToSql>) = match filter {
        "archived" => (
            "WHERE status = 'archived' AND deleted_at IS NULL".into(),
            vec![],
        ),
        "todo" => (
            "WHERE status = 'active' AND deleted_at IS NULL \
             AND (content LIKE '%- [ ]%' OR content LIKE '%- [x]%')"
                .into(),
            vec![],
        ),
        "trash" => ("WHERE deleted_at IS NOT NULL".into(), vec![]),
        "all" => ("WHERE deleted_at IS NULL".into(), vec![]),
        _ => (
            "WHERE status = 'active' AND deleted_at IS NULL".into(),
            vec![],
        ),
    };
    let sql = format!(
        "SELECT {NOTE_COLS} FROM notes {where_clause} \
         ORDER BY is_pinned DESC, updated_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let notes: Vec<Note> = stmt
        .query_map(params_slice.as_slice(), row_to_note)?
        .collect::<rusqlite::Result<_>>()?;
    // 回收站条目的正文供预览面板整文渲染，不截断；其余列表只消费摘要
    attach_tags_and_summary(conn, notes, filter != "trash")
}

/// 标签批量聚合 + 摘要组装：一条聚合查询替代逐便签 tags_for_note 的 N+1。
fn attach_tags_and_summary(
    conn: &Connection,
    notes: Vec<Note>,
    cap: bool,
) -> AppResult<Vec<NoteSummary>> {
    let ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
    let tag_map = tags_batch(conn, &ids)?;
    Ok(notes
        .into_iter()
        .map(|n| {
            let tags = tag_map.get(&n.id).cloned().unwrap_or_default();
            summary_with(n, tags, cap)
        })
        .collect())
}

/// 批量取标签：note_id -> 按名升序的标签列表（与 tags_for_note 同序）。
fn tags_batch(
    conn: &Connection,
    note_ids: &[String],
) -> AppResult<HashMap<String, Vec<String>>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    // 分批防 SQLite 变量数上限
    for chunk in note_ids.chunks(500) {
        let placeholders = vec!["?"; chunk.len()].join(",");
        let sql = format!(
            "SELECT nt.note_id, t.name FROM note_tags nt \
             JOIN tags t ON t.id = nt.tag_id \
             WHERE nt.note_id IN ({placeholders}) ORDER BY t.name ASC"
        );
        let mut stmt = conn.prepare(&sql)?;
        let binds: Vec<&dyn rusqlite::ToSql> =
            chunk.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(binds.as_slice(), |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (note_id, name) = row?;
            map.entry(note_id).or_default().push(name);
        }
    }
    Ok(map)
}

/// 列表/搜索传输瘦身：正文截断上限（字符）。卡片摘要只需 60 字符，
/// todo 计数已在截断前按全文统计；单便签详情（get_note）始终全文。
const LIST_CONTENT_CAP_CHARS: usize = 500;

/// 组装列表摘要：todo 计数用截断前的全文算，再按需截断正文。
/// cap=false（回收站预览）保留全文。
fn summary_with(mut note: Note, tags: Vec<String>, cap: bool) -> NoteSummary {
    let (todo_total, todo_done) = count_todos(&note.content);
    if cap && note.content.chars().count() > LIST_CONTENT_CAP_CHARS {
        note.content = note
            .content
            .chars()
            .take(LIST_CONTENT_CAP_CHARS)
            .collect();
    }
    NoteSummary {
        note,
        tags,
        todo_total,
        todo_done,
    }
}

pub fn to_summary(conn: &Connection, note: Note) -> AppResult<NoteSummary> {
    let tags = super::tags::tags_for_note(conn, &note.id)?;
    let (todo_total, todo_done) = count_todos(&note.content);
    Ok(NoteSummary {
        note,
        tags,
        todo_total,
        todo_done,
    })
}

/// 统计 markdown 任务列表（- [ ] / - [x]）。
pub fn count_todos(content: &str) -> (i64, i64) {
    let mut total = 0i64;
    let mut done = 0i64;
    for line in content.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("- [ ]") {
            if rest.starts_with(' ') || rest.is_empty() {
                total += 1;
            }
        } else if let Some(rest) = t.strip_prefix("- [x]") {
            if rest.starts_with(' ') || rest.is_empty() {
                total += 1;
                done += 1;
            }
        } else if let Some(rest) = t.strip_prefix("- [X]") {
            if rest.starts_with(' ') || rest.is_empty() {
                total += 1;
                done += 1;
            }
        }
    }
    (total, done)
}

/// 部分更新：None 字段不写库。`..Default::default()`（touch=false）配合结构体更新语法使用。
#[derive(Default)]
pub struct NoteUpdate {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_pinned: Option<bool>,
    pub is_always_on_top: Option<bool>,
    pub show_on_all_desktops: Option<bool>,
    pub desktop_pin_state: Option<String>,
    pub fullscreen_behavior: Option<String>,
    pub monitor_id: Option<String>,
    pub is_private: Option<bool>,
    pub locked: Option<bool>,
    /// 数据库列 readonly_flag（serde 输出名 readonly）
    pub readonly_flag: Option<bool>,
    pub scale: Option<f64>,
    pub pin_mode: Option<String>,
    /// 几何与隐藏类更新不刷新 updated_at，避免列表频繁重排
    pub touch: bool,
}

pub fn update(conn: &Connection, u: &NoteUpdate) -> AppResult<Note> {
    let mut sets: Vec<&str> = Vec::new();
    let mut vals: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let mut bind = |set: &'static str, v: Box<dyn rusqlite::ToSql>| {
        sets.push(set);
        vals.push(v);
    };
    if let Some(ref v) = u.title {
        bind("title", Box::new(v.clone()));
    }
    if let Some(ref v) = u.content {
        bind("content", Box::new(v.clone()));
    }
    if let Some(v) = u.is_pinned {
        bind("is_pinned", Box::new(v as i64));
    }
    if let Some(v) = u.is_always_on_top {
        bind("is_always_on_top", Box::new(v as i64));
    }
    if let Some(v) = u.show_on_all_desktops {
        bind("show_on_all_desktops", Box::new(v as i64));
    }
    if let Some(ref v) = u.desktop_pin_state {
        bind("desktop_pin_state", Box::new(v.clone()));
    }
    if let Some(ref v) = u.fullscreen_behavior {
        bind("fullscreen_behavior", Box::new(v.clone()));
    }
    if let Some(ref v) = u.monitor_id {
        bind("monitor_id", Box::new(v.clone()));
    }
    if let Some(v) = u.is_private {
        bind("is_private", Box::new(v as i64));
    }
    if let Some(v) = u.locked {
        bind("locked", Box::new(v as i64));
    }
    if let Some(v) = u.readonly_flag {
        bind("readonly_flag", Box::new(v as i64));
    }
    if let Some(v) = u.scale {
        bind("scale", Box::new(v));
    }
    if let Some(ref v) = u.pin_mode {
        bind("pin_mode", Box::new(v.clone()));
    }
    if u.touch {
        bind("updated_at", Box::new(now()));
    }

    if sets.is_empty() {
        return get(conn, &u.id);
    }
    vals.push(Box::new(u.id.to_string()));
    let placeholders: Vec<String> = (1..=vals.len()).map(|i| format!("?{i}")).collect();
    let mut assigns = String::new();
    for (i, s) in sets.iter().enumerate() {
        if i > 0 {
            assigns.push_str(", ");
        }
        assigns.push_str(&format!("{s} = {}", placeholders[i]));
    }
    let where_ph = placeholders[vals.len() - 1].clone();
    let sql = format!("UPDATE notes SET {assigns} WHERE id = {where_ph}");
    let refs: Vec<&dyn rusqlite::ToSql> = vals.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, refs.as_slice())?;
    get(conn, &u.id)
}

pub fn update_geometry(
    conn: &Connection,
    id: &str,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    monitor_id: Option<&str>,
) -> AppResult<()> {
    conn.execute(
        "UPDATE notes SET x=?1, y=?2, width=?3, height=?4, monitor_id=?5 WHERE id=?6",
        params![x, y, width, height, monitor_id, id],
    )?;
    Ok(())
}

pub fn archive(conn: &Connection, id: &str) -> AppResult<Note> {
    let ts = now();
    conn.execute(
        "UPDATE notes SET status='archived', archived_at=?2, is_pinned=0 WHERE id=?1",
        params![id, ts],
    )?;
    get(conn, id)
}

pub fn restore(conn: &Connection, id: &str) -> AppResult<Note> {
    conn.execute(
        "UPDATE notes SET status='active', archived_at=NULL WHERE id=?1",
        params![id],
    )?;
    get(conn, id)
}

/// 移入回收站（软删除）：仅置 deleted_at，status 与数据保持不变。
pub fn soft_delete(conn: &Connection, id: &str) -> AppResult<Note> {
    conn.execute(
        "UPDATE notes SET deleted_at=?2 WHERE id=?1 AND deleted_at IS NULL",
        params![id, now_secs()],
    )?;
    get(conn, id)
}

/// 从回收站恢复（清空 deleted_at）。
pub fn restore_from_trash(conn: &Connection, id: &str) -> AppResult<Note> {
    conn.execute("UPDATE notes SET deleted_at=NULL WHERE id=?1", params![id])?;
    get(conn, id)
}

/// 版本快照钩子：把便签当前 title/content 插入 note_versions 一行，
/// 并清理该便签超过 50 条的旧版本（保留最近 50 条）。
/// 服务层在更新内容前调用（source: auto | manual | pre-restore …）。
pub fn snapshot_version(conn: &Connection, note_id: &str, source: &str) -> AppResult<()> {
    let (title, content): (String, String) = conn
        .query_row(
            "SELECT title, content FROM notes WHERE id = ?1",
            params![note_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::Invalid(format!("便签不存在: {note_id}"))
            }
            other => AppError::Db(other.to_string()),
        })?;
    let version_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO note_versions (id, note_id, title, content, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![version_id, note_id, title, content, source, now_secs()],
    )?;
    // 同秒创建的版本按 rowid（插入顺序）决胜，保留最新的 50 条
    conn.execute(
        "DELETE FROM note_versions WHERE note_id = ?1 AND id NOT IN (\
             SELECT id FROM note_versions WHERE note_id = ?1 \
             ORDER BY created_at DESC, rowid DESC LIMIT 50\
         )",
        params![note_id],
    )?;
    Ok(())
}

/// auto 快照最小间隔（秒）：窗口内的打字中间态不进版本历史（防 50 条配额被连续保存灌满）。
/// 手动快照与恢复前快照（source != 'auto'）不受限。
pub const AUTO_SNAPSHOT_MIN_INTERVAL_SECS: i64 = 120;

/// 距该便签最近一条 auto 快照是否仍在节流窗口内（true = 应跳过本次快照）。
/// 无任何 auto 快照时返回 false（首个快照必落）。
pub fn auto_snapshot_throttled(conn: &Connection, note_id: &str) -> AppResult<bool> {
    let within: Option<bool> = conn
        .query_row(
            "SELECT (strftime('%s','now') - strftime('%s', created_at)) < ?1 \
             FROM note_versions WHERE note_id = ?2 AND source = 'auto' \
             ORDER BY created_at DESC, rowid DESC LIMIT 1",
            params![AUTO_SNAPSHOT_MIN_INTERVAL_SECS, note_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    Ok(within.unwrap_or(false))
}

/// 永久删除。返回该便签的图片文件路径列表（调用方负责删除物理文件）。
pub fn delete(conn: &Connection, id: &str) -> AppResult<Vec<String>> {
    let paths: Vec<String> = {
        let mut stmt = conn.prepare("SELECT path FROM note_images WHERE note_id=?1")?;
        let rows = stmt.query_map(params![id], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<Vec<String>>>()?
    };
    conn.execute("DELETE FROM notes WHERE id=?1", params![id])?;
    Ok(paths)
}

pub fn search(conn: &Connection, query: &str) -> AppResult<Vec<NoteSummary>> {
    // 与 db::search::search_like 同口径：% _ \ 都转义，防止通配符注入
    let q = format!(
        "%{}%",
        query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let sql = format!(
        "SELECT {NOTE_COLS} FROM notes \
         WHERE status='active' AND deleted_at IS NULL \
         AND (title LIKE ?1 ESCAPE '\\' OR content LIKE ?1 ESCAPE '\\') \
         ORDER BY is_pinned DESC, updated_at DESC LIMIT 50"
    );
    let mut stmt = conn.prepare(&sql)?;
    let notes: Vec<Note> = stmt
        .query_map(params![q], row_to_note)?
        .collect::<rusqlite::Result<_>>()?;
    attach_tags_and_summary(conn, notes, true)
}

/// 开窗恢复专用投影：与 NOTE_COLS 同列，仅 content 以空串占位——
/// 启动恢复/托盘显示只消费 id/几何/标志位，正文由 NoteWindow 打开后自行 get_note 加载。
/// NOTE_COLS 增删列时此处同步。
const NOTE_COLS_WINDOW: &str =
    "id, title, '' AS content, content_format, status, is_pinned, is_always_on_top, \
     show_on_all_desktops, desktop_pin_state, fullscreen_behavior, x, y, width, height, \
     monitor_id, created_at, updated_at, archived_at, deleted_at, is_private, locked, \
     readonly_flag, pin_mode, scale";

pub fn all_active_with_windows(conn: &Connection) -> AppResult<Vec<Note>> {
    let sql = format!(
        "SELECT {NOTE_COLS_WINDOW} FROM notes WHERE status='active' AND deleted_at IS NULL ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let notes = stmt
        .query_map([], row_to_note)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(notes)
}

/// 私密便签列表（活跃、未删除）。内容仍返回，但前端只展示标题。
pub fn list_private(conn: &Connection) -> AppResult<Vec<NoteSummary>> {
    let sql = format!(
        "SELECT {NOTE_COLS} FROM notes \
         WHERE status='active' AND deleted_at IS NULL AND is_private=1 \
         ORDER BY updated_at DESC"
    );
    let mut stmt = conn.prepare(&sql)?;
    let notes: Vec<Note> = stmt
        .query_map([], row_to_note)?
        .collect::<rusqlite::Result<_>>()?;
    attach_tags_and_summary(conn, notes, true)
}

pub fn count(conn: &Connection, status: &str) -> AppResult<i64> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE status=?1",
        params![status],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// 回收站全部便签 id（deleted_at IS NOT NULL），供服务层逐条永久删除。
pub fn list_trash_ids(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id FROM notes WHERE deleted_at IS NOT NULL")?;
    let ids = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

/// 按 title 精确匹配找 active 未删除便签的 id（每日笔记定位用），取最近更新的一条。
pub fn find_id_by_title(conn: &Connection, title: &str) -> AppResult<Option<String>> {
    let row = conn.query_row(
        "SELECT id FROM notes \
         WHERE title = ?1 AND status = 'active' AND deleted_at IS NULL \
         ORDER BY updated_at DESC LIMIT 1",
        params![title],
        |r| r.get::<_, String>(0),
    );
    match row {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // 与 Db::open 一致：级联删除语义依赖外键约束
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        super::super::migrations::run(&conn).unwrap();
        conn
    }

    #[test]
    fn crud_roundtrip() {
        let conn = setup();
        let n = create(&conn, "标题", "# 内容\n- [ ] 任务").unwrap();
        assert_eq!(n.title, "标题");
        assert_eq!(n.status, "active");

        update_geometry(&conn, &n.id, 100, 200, 300, 400, Some("MON1")).unwrap();
        let n2 = get(&conn, &n.id).unwrap();
        assert_eq!(n2.x, Some(100));
        assert_eq!(n2.monitor_id.as_deref(), Some("MON1"));

        update(
            &conn,
            &NoteUpdate {
                id: n.id.clone(),
                title: Some("新标题".into()),
                content: None,
                is_pinned: Some(true),
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                pin_mode: None,
                is_private: Some(true),
                locked: Some(true),
                readonly_flag: Some(true),
                scale: Some(1.25),
                touch: true,
            },
        )
        .unwrap();
        let n3 = get(&conn, &n.id).unwrap();
        assert_eq!(n3.title, "新标题");
        assert!(n3.is_pinned);
        assert!(n3.is_private);
        assert!(n3.locked);
        assert!(n3.readonly);
        assert_eq!(n3.scale, Some(1.25));
        assert_eq!(n3.x, Some(100), "几何不应被内容更新覆盖");

        let a = archive(&conn, &n.id).unwrap();
        assert_eq!(a.status, "archived");
        let r = restore(&conn, &n.id).unwrap();
        assert_eq!(r.status, "active");
        assert!(r.archived_at.is_none());
    }

    #[test]
    fn trash_flow() {
        let conn = setup();
        let a = create(&conn, "A", "c").unwrap();
        create(&conn, "B", "c").unwrap();

        let deleted = soft_delete(&conn, &a.id).unwrap();
        assert!(deleted.deleted_at.is_some());
        assert_eq!(deleted.status, "active", "软删除不改 status");

        assert_eq!(list(&conn, "active").unwrap().len(), 1);
        assert_eq!(list(&conn, "all").unwrap().len(), 1, "回收站不进 all");
        assert_eq!(list(&conn, "todo").unwrap().len(), 0);
        assert_eq!(list(&conn, "trash").unwrap().len(), 1);

        // 回收站里归档过的便签也不进 archived 视图
        archive(&conn, &a.id).unwrap();
        assert_eq!(list(&conn, "archived").unwrap().len(), 0);
        restore_from_trash(&conn, &a.id).unwrap();
        restore(&conn, &a.id).unwrap();
        assert_eq!(list(&conn, "active").unwrap().len(), 2);
        assert_eq!(list(&conn, "trash").unwrap().len(), 0);
    }

    #[test]
    fn snapshot_and_permanent_delete() {
        let conn = setup();
        let n = create(&conn, "v1", "内容1").unwrap();
        snapshot_version(&conn, &n.id, "auto").unwrap();
        update(
            &conn,
            &NoteUpdate {
                id: n.id.clone(),
                title: Some("v2".into()),
                content: Some("内容2".into()),
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: None,
                pin_mode: None,
                touch: true,
            },
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM note_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // 快照存在时软删除不影响数据；永久删除级联清空版本
        soft_delete(&conn, &n.id).unwrap();
        let paths = delete(&conn, &n.id).unwrap();
        assert!(paths.is_empty());
        let versions: i64 = conn
            .query_row("SELECT COUNT(*) FROM note_versions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(versions, 0, "永久删除应级联清理版本快照");

        // 不存在的便签快照报 Invalid
        let r = snapshot_version(&conn, "missing", "auto");
        assert!(matches!(r, Err(AppError::Invalid(_))));
    }

    #[test]
    fn delete_cascades_images_and_tags() {
        let conn = setup();
        let n = create(&conn, "t", "c").unwrap();
        conn.execute(
            "INSERT INTO note_images (id, note_id, path, filename, created_at) VALUES ('i1', ?1, 'p', 'f', '')",
            params![n.id],
        )
        .unwrap();
        super::super::tags::attach_tag(&conn, &n.id, "工作").unwrap();
        let paths = delete(&conn, &n.id).unwrap();
        assert_eq!(paths, vec!["p".to_string()]);
        assert!(!exists(&conn, &n.id).unwrap());
        let imgs: i64 = conn
            .query_row("SELECT COUNT(*) FROM note_images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(imgs, 0);
        let nts: i64 = conn
            .query_row("SELECT COUNT(*) FROM note_tags", [], |r| r.get(0))
            .unwrap();
        assert_eq!(nts, 0);
    }

    #[test]
    fn search_finds_title_and_content() {
        let conn = setup();
        create(&conn, "MediaFlow 开发计划", "...").unwrap();
        create(&conn, "学习", "去研究 MediaFlow RAG").unwrap();
        create(&conn, "无关", "别的").unwrap();
        let hits = search(&conn, "MediaFlow").unwrap();
        assert_eq!(hits.len(), 2);
        let hits = search(&conn, "RAG").unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn todo_count() {
        assert_eq!(count_todos("- [ ] a\n- [x] b\n正文\n- [X] c"), (3, 2));
        assert_eq!(count_todos("没有任务"), (0, 0));
    }

    #[test]
    fn auto_snapshot_throttle_window() {
        let conn = setup();
        let n = create(&conn, "t", "c").unwrap();
        // 无任何快照：不节流（首个快照必落）
        assert!(!auto_snapshot_throttled(&conn, &n.id).unwrap());
        // 刚落一条 auto 快照：窗口内节流
        snapshot_version(&conn, &n.id, "auto").unwrap();
        assert!(auto_snapshot_throttled(&conn, &n.id).unwrap());
        // 窗口外（回拨到 121s 前）：不再节流
        let stale = (chrono::Utc::now() - chrono::Duration::seconds(121))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
        conn.execute(
            "UPDATE note_versions SET created_at = ?2 WHERE note_id = ?1",
            params![n.id, stale],
        )
        .unwrap();
        assert!(!auto_snapshot_throttled(&conn, &n.id).unwrap());
        // 仅有 manual 快照：不节流（手动/恢复前快照不受窗口约束）
        let n2 = create(&conn, "t2", "c2").unwrap();
        snapshot_version(&conn, &n2.id, "manual").unwrap();
        assert!(!auto_snapshot_throttled(&conn, &n2.id).unwrap());
    }

    #[test]
    fn list_caps_content_counts_full_trash_keeps_full() {
        let conn = setup();
        // 构造 >500 字符正文，唯一的 todo 位于截断点之后（独立成行才被计数）
        let tail = "\n- [ ] 尾部任务";
        let long = format!("{}{tail}", "字".repeat(600));
        let n = create(&conn, "长文", &long).unwrap();
        super::super::tags::attach_tag(&conn, &n.id, "工作").unwrap();

        let active = list(&conn, "active").unwrap();
        assert_eq!(active.len(), 1);
        let s = &active[0];
        assert_eq!(
            s.note.content.chars().count(),
            500,
            "列表正文应截断到 500 字符"
        );
        assert_eq!(
            (s.todo_total, s.todo_done),
            (1, 0),
            "计数必须按截断前全文统计"
        );
        assert_eq!(s.tags, vec!["工作".to_string()], "批量聚合与逐条查询同结果");

        // 单便签详情始终全文
        assert_eq!(
            get(&conn, &n.id).unwrap().content.chars().count(),
            600 + tail.chars().count()
        );

        // 回收站列表保留全文（预览面板整文渲染）
        soft_delete(&conn, &n.id).unwrap();
        let trash = list(&conn, "trash").unwrap();
        assert!(
            trash[0].note.content.chars().count() > 500,
            "回收站不截断"
        );
    }

    #[test]
    fn filters() {
        let conn = setup();
        let a = create(&conn, "A", "- [ ] 待办").unwrap();
        create(&conn, "B", "纯文本").unwrap();
        archive(&conn, &a.id).unwrap();
        assert_eq!(list(&conn, "active").unwrap().len(), 1);
        assert_eq!(list(&conn, "archived").unwrap().len(), 1);
        assert_eq!(list(&conn, "todo").unwrap().len(), 0, "归档的不算待办视图");
        assert_eq!(list(&conn, "all").unwrap().len(), 2);
    }

    /// 回归：pin_mode 不依赖 scale，单独更新也必须落库（set_pin_mode 命令路径）。
    #[test]
    fn update_persists_pin_mode_without_scale() {
        let conn = setup();
        let n = create(&conn, "图钉", "内容").unwrap();
        update(
            &conn,
            &NoteUpdate {
                id: n.id.clone(),
                title: None,
                content: None,
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: None,
                pin_mode: Some("desktop".into()),
                touch: false,
            },
        )
        .unwrap();
        assert_eq!(
            get(&conn, &n.id).unwrap().pin_mode.as_deref(),
            Some("desktop")
        );

        // 与 scale 同更时两者都生效
        update(
            &conn,
            &NoteUpdate {
                id: n.id.clone(),
                title: None,
                content: None,
                is_pinned: None,
                is_always_on_top: None,
                show_on_all_desktops: None,
                desktop_pin_state: None,
                fullscreen_behavior: None,
                monitor_id: None,
                is_private: None,
                locked: None,
                readonly_flag: None,
                scale: Some(1.25),
                pin_mode: Some("topmost".into()),
                touch: false,
            },
        )
        .unwrap();
        let n2 = get(&conn, &n.id).unwrap();
        assert_eq!(n2.pin_mode.as_deref(), Some("topmost"));
        assert_eq!(n2.scale, Some(1.25));
    }

    #[test]
    fn trash_ids_and_title_lookup() {
        let conn = setup();
        let a = create(&conn, "每日笔记 2026-09-17", "内容").unwrap();
        create(&conn, "别的", "x").unwrap();

        assert_eq!(
            find_id_by_title(&conn, "每日笔记 2026-09-17").unwrap(),
            Some(a.id.clone())
        );
        assert_eq!(find_id_by_title(&conn, "不存在").unwrap(), None);

        soft_delete(&conn, &a.id).unwrap();
        assert_eq!(list_trash_ids(&conn).unwrap(), vec![a.id.clone()]);
        // 软删除后不再参与标题定位
        assert_eq!(
            find_id_by_title(&conn, "每日笔记 2026-09-17").unwrap(),
            None
        );
    }
}
