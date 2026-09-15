//! 快速捕获窗口（label="quick-capture"）与剪贴板历史服务。
//!
//! 窗口模式与便签一致：`visible(false)` 创建，前端渲染就绪后调
//! `quickcapture_ready` 再 show + set_focus（防白闪）。
//!
//! # 总控接线附录：本模块暴露的 8 个未来命令（注册进 lib.rs 的 invoke_handler）
//!
//! | 命令名                 | 参数（前端 camelCase）           | 返回                          |
//! |------------------------|----------------------------------|-------------------------------|
//! | `quickcapture_toggle`  | 无                               | `AppResult<()>`               |
//! | `quickcapture_hide`    | 无                               | `AppResult<()>`               |
//! | `quickcapture_ready`   | 无                               | `AppResult<()>`               |
//! | `clipboard_add`        | `content: String, kind: String`  | `AppResult<ClipboardEntry>`   |
//! | `clipboard_list`       | `limit: number`                  | `AppResult<Vec<ClipboardEntry>>` |
//! | `clipboard_remove`     | `id: String`                     | `AppResult<()>`               |
//! | `clipboard_clear`      | 无                               | `AppResult<usize>`（删除条数） |
//! | `clipboard_pin`        | `id: String`（切换固定）         | `AppResult<ClipboardEntry>`   |
//!
//! 注册形如（总控执行，本文件不含命令层代码）：
//! `quickcapture::toggle(&app)` / `quickcapture::hide(&app)` / `quickcapture::capture_ready(&app)`
//! / `quickcapture::add_entry(&app, &content, &kind)` / `quickcapture::list_entries(&app, limit)`
//! / `quickcapture::remove_entry(&app, &id)` / `quickcapture::clear_entries(&app)`
//! / `quickcapture::pin_entry(&app, &id)`。

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::db::clipboard;
use crate::db::models::ClipboardEntry;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// 快速捕获窗口 label（前端按此路由到 QuickCaptureWindow）
pub const LABEL: &str = "quick-capture";
/// 窗口尺寸：物理像素
pub const WIDTH: i32 = 440;
pub const HEIGHT: i32 = 260;
/// 距屏幕顶部的边距（物理像素）
const TOP_MARGIN: i32 = 48;

/// 顶部居中 x 坐标（纯函数，可单测）。
/// 窗口比屏幕宽时贴左边（不越出显示器）。
pub fn top_center_x(monitor_x: i32, monitor_w: i32, win_w: i32) -> i32 {
    monitor_x + (monitor_w - win_w).max(0) / 2
}

/// 切换快速捕获窗口：可见则隐藏；已存在但隐藏则 show + 聚焦；不存在则创建。
/// 新建后窗口保持不可见，等前端调 `quickcapture_ready` 再显示。
pub fn toggle(app: &AppHandle) -> AppResult<()> {
    if let Some(win) = app.get_webview_window(LABEL) {
        return if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
            Ok(())
        } else {
            capture_ready(app)
        };
    }
    create_window(app)
}

/// 创建窗口（不可见），定位到主屏顶部居中。
fn create_window(app: &AppHandle) -> AppResult<()> {
    let (x, y) = top_position(app);
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("快速捕获")
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .focused(true)
        .visible(false)
        .build()?;
    let _ = win.set_size(PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    log::info!("快速捕获窗口已创建 label={LABEL} pos=({x},{y})");
    Ok(())
}

/// 主屏顶部居中坐标；取不到主屏时回退 (0, 48)。
fn top_position(app: &AppHandle) -> (i32, i32) {
    if let Ok(Some(m)) = app.primary_monitor() {
        let pos = m.position();
        let size = m.size();
        return (
            top_center_x(pos.x, size.width as i32, WIDTH),
            pos.y + TOP_MARGIN,
        );
    }
    (0, TOP_MARGIN)
}

/// 前端渲染就绪：显示窗口并聚焦（对应 note_window_ready 机制）。
pub fn capture_ready(app: &AppHandle) -> AppResult<()> {
    let win = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::Window(format!("窗口不存在: {LABEL}")))?;
    let _ = win.show();
    let _ = win.set_focus();
    Ok(())
}

/// 隐藏快速捕获窗口（不销毁，保留草稿与滚动状态）；窗口不存在时为幂等成功。
pub fn hide(app: &AppHandle) -> AppResult<()> {
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    Ok(())
}

// ---------- 剪贴板历史服务（薄层调 db::clipboard，供命令层直接转发） ----------

/// 新增一条剪贴板记录（容量由 DAO 剪裁：100 条未固定上限）。
pub fn add_entry(app: &AppHandle, content: &str, kind: &str) -> AppResult<ClipboardEntry> {
    let state = app.state::<AppState>();
    state.db.with(|c| clipboard::add(c, content, kind))
}

/// 历史列表（pinned 优先，其余按时间倒序）。
pub fn list_entries(app: &AppHandle, limit: i64) -> AppResult<Vec<ClipboardEntry>> {
    let state = app.state::<AppState>();
    state.db.with(|c| clipboard::list(c, limit))
}

/// 删除单条记录。
pub fn remove_entry(app: &AppHandle, id: &str) -> AppResult<()> {
    let state = app.state::<AppState>();
    state.db.with(|c| clipboard::remove(c, id))
}

/// 清空全部历史，返回删除条数。
pub fn clear_entries(app: &AppHandle) -> AppResult<usize> {
    let state = app.state::<AppState>();
    state.db.with(clipboard::clear)
}

/// 切换固定状态，返回切换后的记录。
pub fn pin_entry(app: &AppHandle, id: &str) -> AppResult<ClipboardEntry> {
    let state = app.state::<AppState>();
    state.db.with(|c| clipboard::toggle_pin(c, id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_on_monitor() {
        assert_eq!(top_center_x(0, 1920, 440), 740);
        assert_eq!(top_center_x(2560, 1920, 440), 3300);
    }

    #[test]
    fn respects_monitor_offset_including_negative() {
        assert_eq!(top_center_x(100, 1000, 200), 500);
        assert_eq!(top_center_x(-1920, 1920, 440), -1180);
    }

    #[test]
    fn clamps_when_window_wider_than_monitor() {
        assert_eq!(top_center_x(0, 300, 440), 0, "窗口更宽时贴显示器左边");
        assert_eq!(top_center_x(50, 0, 100), 50);
    }
}
