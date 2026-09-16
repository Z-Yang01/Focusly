//! 系统/窗口命令（薄层）：显隐窗口、退出、导入导出、外部链接、应用信息。

use serde_json::json;
use tauri::{AppHandle, State};
use tauri::Manager;

use crate::error::{AppError, AppResult};
use crate::export::ImportSummary;
use crate::notes;
use crate::state::AppState;
use crate::window;

#[tauri::command]
pub fn show_all_notes(app: AppHandle) -> AppResult<()> {
    window::show_all_notes(&app)
}

#[tauri::command]
pub fn hide_all_notes(app: AppHandle) -> AppResult<()> {
    window::hide_all_notes(&app)
}

#[tauri::command]
pub fn toggle_all_notes(app: AppHandle) -> AppResult<()> {
    window::toggle_all_notes(&app)
}

#[tauri::command]
pub fn show_manager(app: AppHandle) {
    window::show_manager(&app);
}

#[tauri::command]
pub fn hide_manager(app: AppHandle) {
    window::hide_manager(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    log::info!("用户请求退出应用");
    app.exit(0);
}

/// 便签窗口首次渲染完成：应用几何并显示。
#[tauri::command]
pub fn note_window_ready(app: AppHandle, note_id: String) -> AppResult<()> {
    window::note_window_ready(&app, &note_id)
}

#[tauri::command]
pub fn close_note_window(app: AppHandle, note_id: String) {
    window::close_note_window(&app, &note_id);
}

#[tauri::command]
pub fn export_data(app: AppHandle, path: String) -> AppResult<()> {
    notes::export_data(&app, &path)
}

#[tauri::command]
pub fn import_data(app: AppHandle, path: String) -> AppResult<ImportSummary> {
    notes::import_data(&app, &path)
}

/// 打开外部链接（仅 http/https 白名单）。
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> AppResult<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(AppError::Invalid("仅允许 http/https 链接".into()));
    }
    use tauri_plugin_opener::OpenerExt;
    if let Err(e) = app.opener().open_url(url.clone(), None::<&str>) {
        let state = app.state::<AppState>();
        crate::filesystem::journal_error(
            &state.paths,
            "window",
            "外部链接打开失败",
            &e.to_string(),
            &format!("链接 {url} 未能打开"),
            "请检查系统默认浏览器设置",
        );
        return Err(AppError::Window(format!("打开链接失败: {e}")));
    }
    Ok(())
}

/// 在资源管理器中打开应用数据目录。
#[tauri::command]
pub fn reveal_data_dir(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let root = state.paths.root.to_string_lossy().to_string();
    app.opener()
        .open_path(root, None::<&str>)
        .map_err(|e| AppError::Window(format!("打开数据目录失败: {e}")))?;
    Ok(())
}

/// 数据目录、数据库路径与版本号。
#[tauri::command]
pub fn get_app_info(state: State<'_, AppState>) -> serde_json::Value {
    json!({
        "dataDir": state.paths.root.display().to_string(),
        "dbPath": state.paths.db.display().to_string(),
        "version": env!("CARGO_PKG_VERSION"),
    })
}


/// 导出诊断包：版本/系统信息/数据库健康摘要/日志副本（不含便签内容、不含私密数据）。
/// 写入用户通过 dialog save 选择的路径（前端负责选路径）。
#[tauri::command]
pub fn export_diagnostics(app: AppHandle, path: String) -> AppResult<()> {
    use serde_json::json;

    let state = app.state::<AppState>();
    let mut lines: Vec<String> = Vec::new();

    // 基本环境
    lines.push(format!("app_version={}", env!("CARGO_PKG_VERSION")));
    lines.push(format!(
        "os={} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    lines.push(format!("generated_at={}", chrono::Local::now().to_rfc3339()));

    // 数据库健康摘要（只统计数量，不输出任何便签内容）
    let health = state.db.with(|c| -> AppResult<serde_json::Value> {
        let integrity = c
            .query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
            .unwrap_or_else(|_| "query_failed".into());
        let count = |sql: &str| -> i64 {
            c.query_row(sql, [], |r| r.get(0)).unwrap_or(-1)
        };
        Ok(json!({
            "integrity": integrity,
            "notes_active": count("SELECT COUNT(*) FROM notes WHERE status='active' AND deleted_at IS NULL"),
            "notes_archived": count("SELECT COUNT(*) FROM notes WHERE status='archived'"),
            "notes_trashed": count("SELECT COUNT(*) FROM notes WHERE deleted_at IS NOT NULL"),
            "sessions_completed": count("SELECT COUNT(*) FROM pomodoro_sessions WHERE status='completed'"),
            "reminders_pending": count("SELECT COUNT(*) FROM reminders WHERE status='pending'"),
        }))
    })?;
    lines.push(format!("db_health={}", health));

    // 日志尾部（最多 200 行；日志本身不含便签正文）
    let log_path = state.paths.logs.join("focusly.log");
    if let Ok(content) = std::fs::read_to_string(&log_path) {
        let tail: Vec<&str> = content.lines().rev().take(200).collect();
        lines.push("--- log_tail ---".into());
        for l in tail.into_iter().rev() {
            lines.push(l.to_string());
        }
    }

    std::fs::write(&path, lines.join("
"))
        .map_err(|e| crate::error::AppError::Io(format!("写入诊断包失败: {e}")))?;
    log::info!("诊断包已导出: {path}");
    Ok(())
}
