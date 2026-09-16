//! 全文搜索：FTS5 trigram 虚拟表 notes_fts（列序 0=note_id, 1=title, 2=body, 3=tags）。
//! notes_fts 由应用层同步：服务层在便签创建/更新/删除/恢复时调用 fts_sync / fts_remove。
//! 搜索语法（tag:/is:/due: 等）由前端解析，本模块只接收解析后的自由文本 keyword。

use rusqlite::{params, Connection, Row};

use crate::db::models::SearchHit;
use crate::db::{notes, tags};
use crate::error::AppResult;

const SEARCH_LIMIT: i64 = 50;

/// 同步一条便签到 FTS 索引（先删后插，幂等）。
/// tags_joined 由调用方聚合（如 "工作 学习"），只影响 tags 列的可搜索性。
pub fn fts_sync(
    conn: &Connection,
    note_id: &str,
    title: &str,
    body: &str,
    tags_joined: &str,
) -> AppResult<()> {
    fts_remove(conn, note_id)?;
    conn.execute(
        "INSERT INTO notes_fts (note_id, title, body, tags) VALUES (?1, ?2, ?3, ?4)",
        params![note_id, title, body, tags_joined],
    )?;
    Ok(())
}

/// 从 FTS 索引移除一条便签。
pub fn fts_remove(conn: &Connection, note_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM notes_fts WHERE note_id = ?1", params![note_id])?;
    Ok(())
}

/// 搜索已解析的自由文本 keyword：
/// - 长度 >= 3 字符：FTS5 trigram（支持任意子串），按 rank 排序，snippet 高亮 body；
/// - 长度 < 3 字符：trigram 无法分词，降级为 title/content LIKE 查询，按 updated_at DESC，
///   snippet 手工截取 keyword 前后约 40 字符并包 <mark>。
/// 两者都排除回收站（deleted_at IS NOT NULL）与私密便签（include_private=false 时）。
pub fn search(conn: &Connection, keyword: &str, include_private: bool) -> AppResult<Vec<SearchHit>> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }
    if keyword.chars().count() >= 3 {
        search_fts(conn, keyword, include_private)
    } else {
        search_like(conn, keyword, include_private)
    }
}

/// 每条命中聚合 tags 与待办统计。
fn finish_hit(conn: &Connection, mut hit: SearchHit) -> AppResult<SearchHit> {
    hit.tags = tags::tags_for_note(conn, &hit.id)?;
    Ok(hit)
}

fn search_fts(conn: &Connection, keyword: &str, include_private: bool) -> AppResult<Vec<SearchHit>> {
    // 双引号包成短语查询，转义内部引号，避免关键词中的 FTS5 语法字符（AND/OR/NEAR/* 等）改变语义
    let phrase = format!("\"{}\"", keyword.replace('"', "\"\""));
    let sql = format!(
        "SELECT n.id, n.title, \
               snippet(notes_fts, 2, '<mark>', '</mark>', '…', 14) AS snip, \
               n.content, n.updated_at, n.is_pinned, n.is_private, n.status \
        FROM notes n \
        JOIN notes_fts ON notes_fts.note_id = n.id \
        WHERE notes_fts MATCH ?1 \
          AND n.deleted_at IS NULL \
          AND (?2 = 1 OR n.is_private = 0) \
        ORDER BY rank \
        LIMIT {SEARCH_LIMIT}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let hits: Vec<crate::db::models::SearchHit> = stmt
        .query_map(params![phrase, include_private], row_to_hit)?
        .collect::<rusqlite::Result<_>>()?;
    hits.into_iter().map(|h| finish_hit(conn, h)).collect()
}

fn search_like(conn: &Connection, keyword: &str, include_private: bool) -> AppResult<Vec<SearchHit>> {
    let pattern = format!("%{}%", keyword.replace('%', "\\%").replace('_', "\\_"));
    let sql = format!(
        "SELECT n.id, n.title, n.content AS snip, \
               n.content, n.updated_at, n.is_pinned, n.is_private, n.status \
        FROM notes n \
        WHERE (n.title LIKE ?1 ESCAPE '\\' OR n.content LIKE ?1 ESCAPE '\\') \
          AND n.deleted_at IS NULL \
          AND (?2 = 1 OR n.is_private = 0) \
        ORDER BY n.updated_at DESC \
        LIMIT {SEARCH_LIMIT}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let hits: Vec<crate::db::models::SearchHit> = stmt
        .query_map(params![pattern, include_private], |r| {
            let mut hit = row_to_hit(r)?;
            let content: String = r.get("content")?;
            hit.snippet = manual_snippet(&hit.title, &content, keyword);
            Ok(hit)
        })?
        .collect::<rusqlite::Result<_>>()?;
    hits.into_iter().map(|h| finish_hit(conn, h)).collect()
}

fn row_to_hit(r: &Row) -> rusqlite::Result<SearchHit> {
    let content: String = r.get("content")?;
    let (todo_total, todo_done) = notes::count_todos(&content);
    Ok(SearchHit {
        id: r.get("id")?,
        title: r.get("title")?,
        snippet: r.get("snip")?,
        tags: Vec::new(),
        todo_total,
        todo_done,
        updated_at: r.get("updated_at")?,
        is_pinned: r.get::<_, i64>("is_pinned")? != 0,
        is_private: r.get::<_, i64>("is_private")? != 0,
        status: r.get("status")?,
    })
}

/// 短词降级路径的手工高亮摘要：优先在正文找 keyword，找不到退回标题，
/// 都没有则返回正文开头截断。命中处前后各取约 40 字符，命中区包 <mark>。
fn manual_snippet(title: &str, body: &str, keyword: &str) -> String {
    if let Some(s) = highlight_around(body, keyword) {
        return s;
    }
    if let Some(s) = highlight_around(title, keyword) {
        return s;
    }
    take_chars(body, 80)
}

/// 在 s 中大小写不敏感地找 keyword，返回 "…前文<mark>命中</mark>后文…"；未命中返回 None。
fn highlight_around(s: &str, keyword: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let kw_lower = keyword.to_lowercase();
    let pos = match lower.find(&kw_lower) {
        Some(p) => p,
        None => return None,
    };
    // lower 与 s 的字节偏移在常见中英文场景一致；极端 Unicode 大小写转换变长时向前找合法边界
    let mut pos = pos.min(s.len());
    while pos > 0 && !s.is_char_boundary(pos) {
        pos -= 1;
    }
    let before_chars = s[..pos].chars().count();
    let start = s
        .char_indices()
        .nth(before_chars.saturating_sub(40))
        .map(|(i, _)| i)
        .unwrap_or(0);
    let kw_chars = keyword.chars().count();
    // 命中区结束位置与摘要结束位置（按字符数换算回字节偏移）
    let char_at = |n: usize| {
        s[start..]
            .char_indices()
            .nth(n)
            .map(|(i, _)| start + i)
            .unwrap_or(s.len())
    };
    let match_end = char_at(before_chars + kw_chars);
    let end = char_at(before_chars + kw_chars + 40);
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(&s[start..pos]);
    out.push_str("<mark>");
    out.push_str(&s[pos..match_end]);
    out.push_str("</mark>");
    out.push_str(&s[match_end..end]);
    if end < s.len() {
        out.push('…');
    }
    Some(out)
}

/// 取前 n 个字符，超出补省略号。
fn take_chars(s: &str, n: usize) -> String {
    let cut = s
        .char_indices()
        .nth(n)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    if cut < s.len() {
        format!("{}…", &s[..cut])
    } else {
        s.to_string()
    }
}


/// 全量重建 FTS 索引：导入数据、批量修复后调用，保证搜索可见性（P0-2）。
/// 只索引未进入回收站的便签；归档与私密过滤由查询侧负责。
pub fn fts_rebuild_all(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM notes_fts", [])?;
    let mut stmt = conn
        .prepare("SELECT id, title, content FROM notes WHERE deleted_at IS NULL")?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut count = 0usize;
    for (id, title, content) in rows {
        let tags = crate::db::tags::tags_for_note(conn, &id)?;
        fts_sync(conn, &id, &title, &content, &tags.join(" "))?;
        count += 1;
    }
    log::info!("FTS 索引已全量重建（{count} 条）");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // 与 Db::open 一致：级联删除语义依赖外键约束
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        crate::db::migrations::run(&conn).unwrap();
        conn
    }

    fn seed(conn: &Connection, title: &str, body: &str) -> crate::db::models::Note {
        let n = notes::create(conn, title, body).unwrap();
        fts_sync(conn, &n.id, title, body, "").unwrap();
        n
    }

    #[test]
    fn fts_table_created_with_trigram() {
        let conn = setup();
        let sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'notes_fts'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(sql.contains("trigram"), "notes_fts 应使用 trigram 分词器: {sql}");
    }

    #[test]
    fn fts_finds_keyword_substring_and_highlights() {
        let conn = setup();
        let a = seed(&conn, "MediaFlow 开发计划", "第一步：搭建 MediaFlow 的 RAG 管道");
        let b = seed(&conn, "学习", "去研究 MediaFlow RAG");
        seed(&conn, "无关", "别的什么都不要写");
        crate::db::tags::attach_tag(&conn, &a.id, "工作").unwrap();

        let hits = search(&conn, "MediaFlow", false).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|h| h.snippet.contains("<mark>")), "FTS 路径应有高亮");
        let ha = hits.iter().find(|h| h.id == a.id).unwrap();
        assert_eq!(ha.tags, vec!["工作"]);
        assert_eq!(ha.title, "MediaFlow 开发计划");
        assert_eq!(ha.status, "active");
        assert!(!ha.is_private);
        let hb = hits.iter().find(|h| h.id == b.id).unwrap();
        assert_eq!(hb.todo_total, 0);

        // trigram 子串匹配（不必整词）
        let hits = search(&conn, "开发计划", false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a.id);

        // content 内的子串
        let hits = search(&conn, "RAG 管道", false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, a.id);

        // 空关键词不查库
        assert!(search(&conn, "   ", false).unwrap().is_empty());
    }

    #[test]
    fn short_keyword_falls_back_to_like() {
        let conn = setup();
        seed(&conn, "MediaFlow 开发计划", "开始开发吧");
        seed(&conn, "无关", "别的");

        // "开发" 只有 2 字符，trigram 无法分词 → LIKE 降级
        let hits = search(&conn, "开发", false).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "MediaFlow 开发计划");
        assert!(
            hits[0].snippet.contains("<mark>开发</mark>"),
            "LIKE 路径也应高亮: {}",
            hits[0].snippet
        );

        // 正文未命中时退回标题高亮
        seed(&conn, "计划本", "纯文本");
        let hits = search(&conn, "计划", false).unwrap();
        let by_title = hits.iter().find(|h| h.title == "计划本").unwrap();
        assert!(by_title.snippet.contains("<mark>计划</mark>"));

        // 1 字符同样降级
        let hits = search(&conn, "吧", false).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn private_and_deleted_excluded() {
        let conn = setup();
        let a = seed(&conn, "MediaFlow 笔记", "MediaFlow 正文内容足够长以支持 trigram");
        let b = seed(&conn, "私密 MediaFlow", "私密正文 MediaFlow 的内容");
        conn.execute("UPDATE notes SET is_private = 1 WHERE id = ?1", params![b.id])
            .unwrap();

        assert_eq!(search(&conn, "MediaFlow", false).unwrap().len(), 1);
        assert_eq!(search(&conn, "MediaFlow", true).unwrap().len(), 2);

        let short = search(&conn, "笔记", false).unwrap();
        assert_eq!(short.len(), 1, "短词路径同样排除私密");

        notes::soft_delete(&conn, &a.id).unwrap();
        assert_eq!(
            search(&conn, "MediaFlow", true).unwrap().len(),
            1,
            "回收站便签不参与搜索"
        );

        // 索引同步删除后同样查不到（幂等）
        fts_remove(&conn, &a.id).unwrap();
        assert_eq!(search(&conn, "MediaFlow", true).unwrap().len(), 1);

        // 重新同步后恢复（仍被 deleted 过滤）
        fts_sync(&conn, &a.id, "MediaFlow 笔记", "MediaFlow 正文内容足够长以支持 trigram", "").unwrap();
        assert_eq!(search(&conn, "MediaFlow", true).unwrap().len(), 1);
    }

    #[test]
    fn fts_sync_is_idempotent_and_tags_searchable() {
        let conn = setup();
        let a = seed(&conn, "标题A", "正文A");
        fts_sync(&conn, &a.id, "标题A", "正文A", "工作流").unwrap();
        fts_sync(&conn, &a.id, "标题A", "正文A", "工作流 学习记录").unwrap();

        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes_fts WHERE note_id = ?1", params![a.id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rows, 1, "重复同步不应产生重复索引行");

        // tags 列可经 FTS 路径搜索（>=3 字符才走 FTS）
        let hits = search(&conn, "工作流", false).unwrap();
        assert_eq!(hits.len(), 1, "tags 列可搜索");
        assert_eq!(hits[0].id, a.id);
        assert_eq!(hits[0].tags, Vec::<String>::new(), "tags 聚合来自 note_tags 而非索引");
    }
}
