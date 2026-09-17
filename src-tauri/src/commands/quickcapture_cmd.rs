//! 快速捕获 / 剪贴板历史命令（薄层，转发 quickcapture 模块）。

use crate::error::AppResult;
use tauri::AppHandle;

#[tauri::command(async)] // 内部会创建速记箱窗口，必须避开主线程（见 notes_cmd 同步死锁注释）
pub fn quickcapture_toggle(app: AppHandle) -> AppResult<()> {
    crate::quickcapture::toggle(&app)
}

#[tauri::command]
pub fn quickcapture_hide(app: AppHandle) -> AppResult<()> {
    crate::quickcapture::hide(&app)
}

#[tauri::command]
pub fn quickcapture_ready(app: AppHandle) -> AppResult<()> {
    crate::quickcapture::capture_ready(&app)
}

#[tauri::command]
pub fn clipboard_add(
    app: AppHandle,
    content: String,
    kind: String,
) -> AppResult<crate::db::models::ClipboardEntry> {
    crate::quickcapture::add_entry(&app, &content, &kind)
}

#[tauri::command]
pub fn clipboard_list(
    app: AppHandle,
    limit: Option<i64>,
) -> AppResult<Vec<crate::db::models::ClipboardEntry>> {
    crate::quickcapture::list_entries(&app, limit.unwrap_or(50))
}

#[tauri::command]
pub fn clipboard_remove(app: AppHandle, id: String) -> AppResult<()> {
    crate::quickcapture::remove_entry(&app, &id)
}

#[tauri::command]
pub fn clipboard_clear(app: AppHandle) -> AppResult<usize> {
    crate::quickcapture::clear_entries(&app)
}

#[tauri::command]
pub fn clipboard_pin(app: AppHandle, id: String) -> AppResult<crate::db::models::ClipboardEntry> {
    crate::quickcapture::pin_entry(&app, &id)
}
