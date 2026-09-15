//! 系统/窗口命令（薄层）：显隐窗口、退出、导入导出、外部链接、应用信息。

use serde_json::json;
use tauri::{AppHandle, State};

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
