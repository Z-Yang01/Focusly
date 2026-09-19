//! 今日任务命令（薄层）：CRUD / 状态 / 拖拽调时 / 转便签 / 统计 / 搜索。
//! 前端参数 camelCase（tauri 自动转换），如 `daily_task_create({ date, title, ... })`。

use tauri::{AppHandle, Manager, State};

use crate::daily_task;
use crate::db::daily_tasks;
use crate::db::models::{DailyTask, DailyTaskStats, Note};
use crate::error::AppResult;
use crate::pomodoro;
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
    app: AppHandle,
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
    let task = state.db.with(|c| {
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
    })?;
    // 私密防线：任务转私密 → 脱敏其全部番茄会话文本快照（含运行中与历史行）
    if task.is_private {
        crate::pomodoro::scrub_daily_task_text(&app, &id);
    }
    Ok(task)
}

/// 按日列表：先物化当日到期重复实例，再返回（模板行不出现）。
#[tauri::command]
pub fn daily_task_list(state: State<'_, AppState>, date: String) -> AppResult<Vec<DailyTask>> {
    state.db.with(|c| -> AppResult<Vec<DailyTask>> {
        daily_tasks::materialize_recurring(c, &date)?;
        daily_tasks::list_by_date(c, &date)
    })
}

/// 设置今日任务状态。置 done 时若该任务绑定的番茄仍在运行，
/// 自动以 reason="task_done" 结束（防"事毕钟走"）；判定与停止在状态机内原子完成
/// （StopIfTask），避免"读快照 → 发停止"窗口内 Start 换任务导致误杀新会话。
#[tauri::command]
pub async fn daily_task_set_status(
    app: AppHandle,
    id: String,
    status: String,
) -> AppResult<DailyTask> {
    let task = {
        let state = app.state::<AppState>();
        state
            .db
            .with(|c| daily_tasks::set_status(c, &id, &status))?
    };
    if status == "done" {
        pomodoro::send_sync(
            &app,
            pomodoro::PomodoroCmd::StopIfTask {
                task_key: format!("daily:{id}"),
                reason: "task_done".into(),
            },
        )
        .await?;
    }
    Ok(task)
}

/// 日期范围实例（周概览用）：先对 end 物化到期重复实例，再返回闭区间内全部实例。
#[tauri::command]
pub fn daily_task_list_range(
    state: State<'_, AppState>,
    start_date: String,
    end_date: String,
) -> AppResult<Vec<DailyTask>> {
    state.db.with(|c| -> AppResult<Vec<DailyTask>> {
        daily_tasks::materialize_recurring(c, &end_date)?;
        daily_tasks::list_by_range(c, &start_date, &end_date)
    })
}

/// 批量顺延：把选中任务移动到目标日期（重复模板被忽略）。返回实际移动数。
#[tauri::command]
pub fn daily_task_postpone_to(
    state: State<'_, AppState>,
    ids: Vec<String>,
    date: String,
) -> AppResult<usize> {
    state.db.with(|c| daily_tasks::postpone_to(c, &ids, &date))
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
