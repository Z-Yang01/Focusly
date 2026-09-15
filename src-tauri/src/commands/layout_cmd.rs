//! 窗口布局命令（薄层，转发 window::layout 服务）。

use crate::error::AppResult;
use tauri::AppHandle;

#[tauri::command]
pub fn layout_save_preset(app: AppHandle, name: String) -> AppResult<crate::db::models::LayoutPreset> {
    crate::window::layout::save_preset(&app, &name)
}

#[tauri::command]
pub fn layout_list_presets(app: AppHandle) -> AppResult<Vec<crate::db::models::LayoutPreset>> {
    crate::window::layout::list_presets(&app)
}

#[tauri::command]
pub fn layout_apply_preset(app: AppHandle, id: String) -> AppResult<usize> {
    crate::window::layout::apply_preset(&app, &id)
}

#[tauri::command]
pub fn layout_delete_preset(app: AppHandle, id: String) -> AppResult<()> {
    crate::window::layout::delete_preset(&app, &id)
}

#[tauri::command]
pub fn layout_arrange_grid(app: AppHandle, cols: Option<usize>) -> AppResult<usize> {
    crate::window::layout::arrange_grid(&app, cols)
}
