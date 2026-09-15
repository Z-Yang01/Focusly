//! 图片管理命令（薄层，转发 imagemgr 模块）。

use crate::error::AppResult;
use tauri::AppHandle;

#[tauri::command]
pub fn image_find_duplicates(app: AppHandle) -> AppResult<Vec<crate::imagemgr::DupGroup>> {
    crate::imagemgr::find_duplicates(&app)
}

#[tauri::command]
pub fn image_cleanup_orphans(app: AppHandle) -> AppResult<usize> {
    crate::imagemgr::cleanup_orphans(&app)
}

#[tauri::command]
pub fn image_make_thumbnails(app: AppHandle) -> AppResult<usize> {
    crate::imagemgr::make_thumbnails(&app)
}
