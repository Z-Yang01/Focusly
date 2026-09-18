//! 今日任务命令（薄层）：CRUD / 状态 / 拖拽调时 / 转便签 / 统计 / 搜索。
//! 前端参数 camelCase（tauri 自动转换），如 `daily_task_create({ date, title, ... })`。

use tauri::{AppHandle, State};

use crate::daily_task;
use crate::db::daily_tasks;
use crate::db::models::{DailyTask, DailyTaskStats, Note};
use crate::error::AppResult;
use crate::state::AppState;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn daily_task_create(
    state: State<'_, AppState>,
    date: String,
    start_time: Option<String>,
    end_time: Option<String>,
    title: String,
    note: Option<String>,
    estimate_pomodoros: Option<i64>,
    priority: Option<String>,
    repeat_rule: Option<String>,
    tags: Option<String>,
    is_private: Option<bool>,
    focus_min: Option<i64>,
) -> AppResult<DailyTask> {
    state.db.with(|c| {
        daily_tasks::create(
            c,
            &date,
            start_time.as_deref(),
            end_time.as_deref(),
            &title,
            note.as_deref(),
            estimate_pomodoros.unwrap_or(0),
            priority.as_deref().unwrap_or("medium"),
            repeat_rule.as_deref().unwrap_or("none"),
            tags.as_deref(),
            is_private.unwrap_or(false),
            focus_min,
        )
    })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn daily_task_update(
    state: State<'_, AppState>,
    id: String,
    date: String,
    start_time: Option<String>,
    end_time: Option<String>,
    title: String,
    note: Option<String>,
    estimate_pomodoros: Option<i64>,
    priority: Option<String>,
    repeat_rule: Option<String>,
    tags: Option<String>,
    is_private: Option<bool>,
    focus_min: Option<i64>,
) -> AppResult<DailyTask> {
    state.db.with(|c| {
        daily_tasks::update(
            c,
            &id,
            &date,
            start_time.as_deref(),
            end_time.as_deref(),
            &title,
            note.as_deref(),
            estimate_pomodoros.unwrap_or(0),
            priority.as_deref().unwrap_or("medium"),
            repeat_rule.as_deref().unwrap_or("none"),
            tags.as_deref(),
            is_private.unwrap_or(false),
            focus_min,
        )
    })
}

/// 按日列表：先物化当日到期重复实例，再返回（模板行不出现）。
#[tauri::command]
pub fn daily_task_list(state: State<'_, AppState>, date: String) -> AppResult<Vec<DailyTask>> {
    state.db.with(|c| -> AppResult<Vec<DailyTask>> {
        daily_tasks::materialize_recurring(c, &date)?;
        daily_tasks::list_by_date(c, &date)
    })
}

#[tauri::command]
pub fn daily_task_set_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> AppResult<DailyTask> {
    state.db.with(|c| daily_tasks::set_status(c, &id, &status))
}

/// 拖拽调整时间块（15 分钟吸附由前端完成，后端只校验格式）。
#[tauri::command]
pub fn daily_task_set_time(
    state: State<'_, AppState>,
    id: String,
    start_time: Option<String>,
    end_time: Option<String>,
) -> AppResult<DailyTask> {
    state
        .db
        .with(|c| daily_tasks::set_time(c, &id, start_time.as_deref(), end_time.as_deref()))
}

#[tauri::command]
pub fn daily_task_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.with(|c| daily_tasks::delete(c, &id))
}

#[tauri::command]
pub fn daily_task_stats(state: State<'_, AppState>, date: String) -> AppResult<DailyTaskStats> {
    state.db.with(|c| daily_tasks::stats(c, &date))
}

/// 任务转便签：返回新建便签（前端决定是否打开窗口）。
#[tauri::command(async)] // 内部会分配新窗口几何并可能被前端直接打开
pub fn daily_task_to_note(app: AppHandle, id: String) -> AppResult<Note> {
    daily_task::to_note(&app, &id)
}

/// 轻量搜索：LIKE title/note/tags + 可选 status/date 过滤。
#[tauri::command]
pub fn daily_task_search(
    state: State<'_, AppState>,
    query: String,
    status: Option<String>,
    date: Option<String>,
) -> AppResult<Vec<DailyTask>> {
    state
        .db
        .with(|c| daily_tasks::search(c, &query, status.as_deref(), date.as_deref()))
}
