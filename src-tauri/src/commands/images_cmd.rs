//! 图片命令（薄层）：文件与 RGBA 数据入库，物理文件在数据目录 images/ 下。

use std::path::Path;

use tauri::State;

use crate::db::models::NoteImage;
use crate::error::AppResult;
use crate::state::AppState;

/// 从文件添加图片（校验扩展名，拷贝进 images/<note_id>/）。
#[tauri::command]
pub fn add_image(state: State<'_, AppState>, note_id: String, path: String) -> AppResult<NoteImage> {
    state.db.with(|c| {
        crate::db::images::add_file(c, &state.paths.images, &note_id, Path::new(&path))
    })
}

/// 剪贴板粘贴的 RGBA 原始字节，编码 PNG 后入库。
#[tauri::command]
pub fn add_image_data(
    state: State<'_, AppState>,
    note_id: String,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> AppResult<NoteImage> {
    state.db.with(|c| {
        crate::db::images::add_rgba(c, &state.paths.images, &note_id, &bytes, width, height)
    })
}

/// 删除图片记录及物理文件（仅限应用数据目录内）。
#[tauri::command]
pub fn remove_image(state: State<'_, AppState>, image_id: String) -> AppResult<()> {
    state
        .db
        .with(|c| crate::db::images::remove(c, &state.paths.images, &image_id))
}

/// 检测图片路径是否仍存在（前端加载失败兜底）。
#[tauri::command]
pub fn image_exists(path: String) -> bool {
    crate::db::images::path_exists(&path)
}
