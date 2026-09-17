//! 便签命令（薄层）：业务在 notes.rs。

use tauri::{AppHandle, State};

use crate::db::models::{Note, NoteDetail, NoteSummary};
use crate::error::AppResult;
use crate::notes;
use crate::state::AppState;

/// 只建行不打开窗口（x/y 用层叠位置）。
#[tauri::command]
pub fn create_note(app: AppHandle) -> AppResult<Note> {
    notes::create_record(&app)
}

/// 新建并打开（托盘/快捷键/管理器共用）。
#[tauri::command]
pub fn new_note(app: AppHandle) -> AppResult<Note> {
    notes::create_and_open(&app)
}

/// 打开（或聚焦）便签窗口。
#[tauri::command]
pub fn open_note_window(app: AppHandle, note_id: String) -> AppResult<()> {
    notes::open_existing(&app, &note_id)
}

/// 便签详情：note + images + tags + 最近一条 pending 提醒。
#[tauri::command]
pub fn get_note(app: AppHandle, note_id: String) -> AppResult<NoteDetail> {
    notes::get_detail(&app, &note_id)
}

/// filter: all | active | archived | todo（默认 active）。
#[tauri::command]
pub fn list_notes(state: State<'_, AppState>, filter: String) -> AppResult<Vec<NoteSummary>> {
    state.db.with(|c| crate::db::notes::list(c, &filter))
}

#[tauri::command]
pub fn update_note_content(
    app: AppHandle,
    note_id: String,
    title: String,
    content: String,
) -> AppResult<Note> {
    notes::update_content(&app, &note_id, &title, &content)
}

/// flag: pinned | always_on_top | all_desktops。
#[tauri::command]
pub fn set_note_flag(
    app: AppHandle,
    note_id: String,
    flag: String,
    value: bool,
) -> AppResult<Note> {
    notes::set_flag(&app, &note_id, &flag, value)
}

/// behavior: normal | always_top | fullscreen_show | fullscreen_hide。
#[tauri::command]
pub fn set_note_fullscreen_behavior(
    app: AppHandle,
    note_id: String,
    behavior: String,
) -> AppResult<Note> {
    notes::set_fullscreen_behavior(&app, &note_id, &behavior)
}

#[tauri::command]
pub fn archive_note(app: AppHandle, note_id: String) -> AppResult<Note> {
    notes::archive(&app, &note_id)
}

#[tauri::command]
pub fn restore_note(app: AppHandle, note_id: String) -> AppResult<Note> {
    notes::restore(&app, &note_id)
}

#[tauri::command]
pub fn delete_note(app: AppHandle, note_id: String) -> AppResult<()> {
    notes::delete_permanently(&app, &note_id)
}

#[tauri::command]
pub fn search_notes(state: State<'_, AppState>, query: String) -> AppResult<Vec<NoteSummary>> {
    state.db.with(|c| crate::db::notes::search(c, &query))
}

/// 重设便签标签，返回最终标签列表。
#[tauri::command]
pub fn set_note_tags(app: AppHandle, note_id: String, tags: Vec<String>) -> AppResult<Vec<String>> {
    notes::set_note_tags(&app, &note_id, tags)
}

/// 全部标签及活跃便签计数。
/// 注意：返回对象数组（与前端 TagCount { name, count } 一致），而非元组数组。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

#[tauri::command]
pub fn get_tags(state: State<'_, AppState>) -> AppResult<Vec<TagCount>> {
    let pairs = state.db.with(crate::db::tags::all_tags_with_counts)?;
    Ok(pairs
        .into_iter()
        .map(|(name, count)| TagCount { name, count })
        .collect())
}

// ---------- 回收站 / 版本历史 / 私密 / FTS 搜索（批1新增） ----------

#[tauri::command]
pub fn list_deleted_notes(state: State<'_, AppState>) -> AppResult<Vec<NoteSummary>> {
    state.db.with(|c| crate::db::notes::list(c, "trash"))
}

#[tauri::command]
pub fn trash_note(app: AppHandle, note_id: String) -> AppResult<Note> {
    notes::move_to_trash(&app, &note_id)
}

#[tauri::command]
pub fn restore_from_trash(app: AppHandle, note_id: String) -> AppResult<Note> {
    notes::restore_from_trash(&app, &note_id)
}

#[tauri::command]
pub fn empty_trash(app: AppHandle) -> AppResult<usize> {
    notes::empty_trash(&app)
}

#[tauri::command]
pub fn list_versions(
    state: State<'_, AppState>,
    note_id: String,
    limit: Option<i64>,
) -> AppResult<Vec<crate::db::models::NoteVersion>> {
    state
        .db
        .with(|c| crate::db::versions::list_versions(c, &note_id, limit.unwrap_or(50)))
}

#[tauri::command]
pub fn restore_version(app: AppHandle, version_id: String) -> AppResult<Note> {
    notes::restore_note_version(&app, &version_id)
}

#[tauri::command]
pub fn set_note_privacy(
    app: AppHandle,
    note_id: String,
    flag: String,
    value: bool,
) -> AppResult<Note> {
    notes::set_privacy_flag(&app, &note_id, &flag, value)
}

#[tauri::command]
pub fn search_notes_v2(
    state: State<'_, AppState>,
    query: String,
    include_private: Option<bool>,
) -> AppResult<Vec<crate::db::models::SearchHit>> {
    // 只读查询，直接走 db 层（无需 AppHandle 发事件）
    state
        .db
        .with(|c| crate::db::search::search(c, &query, include_private.unwrap_or(false)))
}

#[tauri::command]
pub fn list_private_notes(state: State<'_, AppState>) -> AppResult<Vec<NoteSummary>> {
    state.db.with(crate::db::notes::list_private)
}

#[tauri::command]
pub fn get_due_view(state: State<'_, AppState>) -> AppResult<crate::db::todos_view::TodoView> {
    state.db.with(crate::db::todos_view::get_due_view)
}

// ---------- 图钉三态（normal / topmost / desktop） ----------

#[tauri::command]
pub fn set_pin_mode(app: AppHandle, note_id: String, mode: String) -> AppResult<Note> {
    notes::set_pin_mode(&app, &note_id, &mode)
}

// ---------- 每日笔记（批3） ----------

#[tauri::command]
pub fn daily_get_or_create(app: AppHandle) -> AppResult<Note> {
    crate::daily::get_or_create_daily(&app)
}
