//! 窗口服务：便签多窗口创建/显隐/几何持久化、管理器窗口、全屏策略应用。

pub mod foreground;
pub mod layout;
pub mod monitor;

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
    WindowEvent,
};

use crate::db::models::{FullscreenBehavior, Note};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub const MANAGER_LABEL: &str = "manager";
pub const NOTE_LABEL_PREFIX: &str = "note-";
/// 几何去抖保存间隔
pub const GEOMETRY_DEBOUNCE: Duration = Duration::from_millis(800);
/// 新便签层叠偏移
const CASCADE_STEP: i32 = 36;
const DEFAULT_NOTE_W: i32 = 320;
const DEFAULT_NOTE_H: i32 = 360;

pub fn note_label(id: &str) -> String {
    format!("{NOTE_LABEL_PREFIX}{id}")
}

pub fn note_id_from_label(label: &str) -> Option<&str> {
    label.strip_prefix(NOTE_LABEL_PREFIX)
}

/// 新便签默认逻辑尺寸（CSS px）；落库/开窗时按主显示器 DPI 缩放为物理像素，
/// 避免高 DPI 屏上新建便签过小（320 物理 px 在 175% 缩放下只有 183 逻辑 px）。
pub fn default_note_size_physical(app: &AppHandle) -> (i32, i32) {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0);
    let (w, h) = (
        (DEFAULT_NOTE_W as f64 * scale).round() as i32,
        (DEFAULT_NOTE_H as f64 * scale).round() as i32,
    );
    // 不超过主显示器工作区（极小屏兜底）
    let (aw, ah) = monitor::primary_work_area_size().unwrap_or((i32::MAX, i32::MAX));
    (w.min(aw), h.min(ah))
}

/// 为新便签计算层叠位置（主屏右上角开始）。宽度按 DPI 缩放后参与计算。
pub fn cascade_position(app: &AppHandle, existing: usize) -> (i32, i32) {
    let (mut x, mut y) = (100, 80);
    if let Ok(Some(m)) = app.primary_monitor() {
        let pos = m.position();
        let size = m.size();
        let (dw, _) = default_note_size_physical(app);
        x = pos.x + size.width as i32 - dw - 48 - (existing as i32 % 8) * CASCADE_STEP;
        y = pos.y + 64 + (existing as i32 % 8) * CASCADE_STEP;
    }
    (x.max(0), y.max(0))
}

/// 打开（或聚焦）便签窗口。几何从 DB 恢复，越界时自动拉回。
pub fn open_note_window(app: &AppHandle, note: &Note) -> AppResult<tauri::WebviewWindow> {
    let label = note_label(&note.id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        return Ok(win);
    }

    let (mut x, mut y) = note
        .x
        .zip(note.y)
        .unwrap_or_else(|| cascade_position(app, 0));
    let (default_w, default_h) = default_note_size_physical(app);
    let width = note.width.unwrap_or(default_w);
    let height = note.height.unwrap_or(default_h);
    if !monitor::rect_visible_on_any_monitor(x, y, width, height) {
        let count = app.webview_windows().len();
        let (cx, cy) = cascade_position(app, count);
        x = cx;
        y = cy;
    }

    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title("便签")
        .decorations(false)
        .transparent(true)
        .resizable(true)
        .skip_taskbar(true)
        .shadow(false)
        .always_on_top(note.is_always_on_top || note.fullscreen_behavior == "always_top")
        .visible(false)
        .build()?;

    let _ = win.set_size(PhysicalSize::new(
        width.max(120) as u32,
        height.max(100) as u32,
    ));
    let _ = win.set_position(PhysicalPosition::new(x, y));

    if note.show_on_all_desktops {
        apply_desktop_pin(app, &win, note, true);
    }
    Ok(win)
}

/// 应用虚拟桌面固定（COM），失败时记录真实状态，不伪造成功。
fn apply_desktop_pin(app: &AppHandle, win: &tauri::WebviewWindow, note: &Note, pin: bool) {
    let state = pin && note.desktop_pin_state != "unsupported";
    let result = crate::vdesktop::set_window_pinned(app, win, state);
    let new_state = match result {
        Ok(()) => {
            if state {
                "on"
            } else {
                "off"
            }
        }
        Err(crate::vdesktop::VdError::Unsupported(msg)) => {
            log::warn!("虚拟桌面固定不可用: {msg}");
            "unsupported"
        }
        Err(crate::vdesktop::VdError::Failed(msg)) => {
            log::error!("虚拟桌面固定失败: {msg}");
            "failed"
        }
    };
    if new_state != note.desktop_pin_state {
        // 状态落库失败会留痕：下次重建窗口/重启时按旧状态重试（而非静默丢状态）
        if let Err(e) = app.state::<AppState>().db.with(|c| {
            crate::db::notes::update(
                c,
                &crate::db::notes::NoteUpdate {
                    id: note.id.clone(),
                    title: None,
                    content: None,
                    is_pinned: None,
                    is_always_on_top: None,
                    show_on_all_desktops: None,
                    desktop_pin_state: Some(new_state.into()),
                    fullscreen_behavior: None,
                    monitor_id: None,
                    is_private: None,
                    locked: None,
                    readonly_flag: None,
                    scale: None,
                    touch: false,
                    pin_mode: None,
                },
            )
        }) {
            log::error!("便签 {} 的桌面固定状态 {new_state} 落库失败: {e}", note.id);
        }
    }
}

/// 便签窗口首次渲染完成：应用几何并显示（避免白闪）。
pub fn note_window_ready(app: &AppHandle, note_id: &str) -> AppResult<()> {
    let label = note_label(note_id);
    let win = app
        .get_webview_window(&label)
        .ok_or_else(|| AppError::Window(format!("窗口不存在: {label}")))?;
    let note = app
        .state::<AppState>()
        .db
        .with(|c| crate::db::notes::get(c, note_id))?;
    let _ =
        win.set_always_on_top(note.is_always_on_top || note.fullscreen_behavior == "always_top");
    if let (Some(x), Some(y)) = (note.x, note.y) {
        let size = win.outer_size().unwrap_or(PhysicalSize::new(320, 360));
        if monitor::rect_visible_on_any_monitor(x, y, size.width as i32, size.height as i32) {
            let _ = win.set_position(PhysicalPosition::new(x, y));
        }
    }
    let _ = win.show();
    let _ = win.set_focus();
    Ok(())
}

pub fn close_note_window(app: &AppHandle, note_id: &str) {
    let label = note_label(note_id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.close();
    }
    let state = app.state::<AppState>();
    lock_ok(&state.fullscreen_hidden).remove(&label);
}

/// 显示、取消最小化并聚焦指定便签窗口。
/// 通知点击聚焦便签接线时复用（见 AGENTS.md 已知平台限制：MVP 通知为提示型）。
#[allow(dead_code)]
pub fn focus_note_window(app: &AppHandle, note_id: &str) {
    if let Some(win) = app.get_webview_window(&note_label(note_id)) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn show_manager(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MANAGER_LABEL) {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

pub fn hide_manager(app: &AppHandle) {
    if let Some(win) = app.get_webview_window(MANAGER_LABEL) {
        let _ = win.hide();
    }
}

/// 显示全部便签窗口。
pub fn show_all_notes(app: &AppHandle) -> AppResult<()> {
    let labels: Vec<String> = {
        let state = app.state::<AppState>();
        let notes = state.db.with(crate::db::notes::all_active_with_windows)?;
        notes.iter().map(|n| note_label(&n.id)).collect()
    };
    {
        let app_state = app.state::<AppState>();
        let mut hidden = lock_ok(&app_state.fullscreen_hidden);
        for label in &labels {
            if let Some(win) = app.get_webview_window(label) {
                let _ = win.show();
                let _ = win.unminimize();
            }
            hidden.remove(label);
        }
    }
    let _ = app.emit(crate::events::NOTES_VISIBILITY, true);
    Ok(())
}

/// 隐藏全部便签窗口（只隐藏窗口，不删内容）。
pub fn hide_all_notes(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut hidden = lock_ok(&state.fullscreen_hidden);
    for (label, win) in app.webview_windows() {
        if note_id_from_label(&label).is_some() {
            let _ = win.hide();
            hidden.insert(label);
        }
    }
    let _ = app.emit(crate::events::NOTES_VISIBILITY, false);
    Ok(())
}

pub fn toggle_all_notes(app: &AppHandle) -> AppResult<()> {
    let any_visible = app.webview_windows().iter().any(|(label, win)| {
        note_id_from_label(label).is_some() && win.is_visible().unwrap_or(false)
    });
    if any_visible {
        hide_all_notes(app)
    } else {
        show_all_notes(app)
    }
}

/// 前台全屏状态变化时应用每张便签的全屏策略。
pub fn apply_fullscreen_policy(app: &AppHandle, fullscreen: bool) {
    let state = app.state::<AppState>();
    let mut hidden = lock_ok(&state.fullscreen_hidden);
    let Ok(notes) = state.db.with(crate::db::notes::all_active_with_windows) else {
        return;
    };
    for note in notes {
        let label = note_label(&note.id);
        let Some(win) = app.get_webview_window(&label) else {
            continue;
        };
        let behavior = FullscreenBehavior::parse(&note.fullscreen_behavior);
        match behavior {
            FullscreenBehavior::FullscreenHide => {
                if fullscreen && win.is_visible().unwrap_or(false) {
                    let _ = win.hide();
                    hidden.insert(label);
                } else if !fullscreen && hidden.contains(&label) {
                    let _ = win.show();
                    hidden.remove(&label);
                }
            }
            FullscreenBehavior::FullscreenShow => {
                let _ = win.set_always_on_top(true);
            }
            FullscreenBehavior::AlwaysTop => {
                let _ = win.set_always_on_top(true);
            }
            FullscreenBehavior::Normal => {
                // 不能无条件取消置顶：is_always_on_top 是用户显式设置的钉住状态，
                // 全屏策略切换时必须保留，否则每次前台切换都会把置顶便签打回普通层。
                let _ = win.set_always_on_top(note.is_always_on_top);
                if !fullscreen && hidden.contains(&label) {
                    let _ = win.show();
                    hidden.remove(&label);
                }
            }
        }
    }
}

/// 立即保存窗口几何（关闭前兜底）。落库失败仅告警：几何丢失只影响下次打开位置。
pub fn save_geometry_now(app: &AppHandle, win: &tauri::WebviewWindow) {
    let Some(note_id) = note_id_from_label(win.label()) else {
        return;
    };
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
        let monitor = hwnd_of(win).and_then(monitor::monitor_of_window);
        if let Err(e) = app.state::<AppState>().db.with(|c| {
            crate::db::notes::update_geometry(
                c,
                note_id,
                pos.x,
                pos.y,
                size.width as i32,
                size.height as i32,
                monitor.as_deref(),
            )
        }) {
            log::error!("便签 {note_id} 几何落库失败: {e}");
        }
    }
}

/// 锁中毒容错：即使持锁线程 panic 也能恢复（锁内操作均为简单集合读写，安全）。
fn lock_ok<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// tauri 的 HWND（其内部 windows crate 版本可能与本 crate 不同）转换为本 crate 的 HWND。
/// 当前 hwnd_of 直接按值转换（两版裸指针/宽度一致）；若未来依赖版本布局分叉，
/// 跨版本 HWND 都必须经此函数转换，故保留。
#[cfg(windows)]
#[allow(dead_code)]
fn to_win32_hwnd(h: windows::Win32::Foundation::HWND) -> windows::Win32::Foundation::HWND {
    h
}

/// 从 tauri 窗口取 Win32 HWND。
#[cfg(windows)]
fn hwnd_of(win: &tauri::WebviewWindow) -> Option<windows::Win32::Foundation::HWND> {
    // tauri 返回的是它依赖的 windows-core HWND，裸指针/宽度一致，直接按值转换
    let raw = win.hwnd().ok()?;
    Some(windows::Win32::Foundation::HWND(raw.0 as _))
}

/// 处理窗口事件：几何去抖持久化 + 管理器关闭行为。
pub fn handle_window_event(app: &AppHandle, label: &str, event: &WindowEvent) {
    match event {
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
            if note_id_from_label(label).is_some() {
                schedule_geometry_save(app, label);
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            if note_id_from_label(label).is_some() {
                // 便签窗口：只立即保存几何，放行关闭
                if let Some(win) = app.get_webview_window(label) {
                    save_geometry_now(app, &win);
                }
            } else {
                // 管理器：拦截关闭，按设置隐藏到托盘或退出
                api.prevent_close();
                let close_action = app
                    .state::<AppState>()
                    .db
                    .with(|c| crate::db::settings::get(c, "close_action"))
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "tray".into());
                if close_action == "quit" {
                    log::info!("close_action=quit，退出应用");
                    app.exit(0);
                } else if let Some(win) = app.get_webview_window(label) {
                    let _ = win.hide();
                }
            }
        }
        _ => {}
    }
}

fn schedule_geometry_save(app: &AppHandle, label: &str) {
    let gen = {
        let state = app.state::<AppState>();
        let mut map = lock_ok(&state.geometry_gens);
        let g = map.entry(label.to_string()).or_insert(0);
        *g += 1;
        *g
    };
    let app = app.clone();
    let label = label.to_string();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(GEOMETRY_DEBOUNCE).await;
        let Some(win) = app.get_webview_window(&label) else {
            return;
        };
        // 期间又有新事件 → 本次放弃
        let current = lock_ok(&app.state::<AppState>().geometry_gens)
            .get(&label)
            .copied()
            .unwrap_or(0);
        if current != gen {
            return;
        }
        save_geometry_now(&app, &win);
    });
}

/// 重新应用所有便签窗口的置顶状态（用于切换 fullscreen_behavior / always_on_top）。
pub fn apply_note_flags(app: &AppHandle, note: &Note) {
    let label = note_label(&note.id);
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win
            .set_always_on_top(note.is_always_on_top || note.fullscreen_behavior == "always_top");
    }
    if note.show_on_all_desktops || note.desktop_pin_state == "on" {
        if let Some(win) = app.get_webview_window(&label) {
            apply_desktop_pin(app, &win, note, note.show_on_all_desktops);
        }
    }
}

/// 启动时：根据设置决定显示哪些窗口。
pub fn startup_windows(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let launch_show_notes = state
        .db
        .with(|c| crate::db::settings::get(c, "launch_show_notes"))?
        .unwrap_or_else(|| "true".into())
        == "true";
    let start_minimized = state
        .db
        .with(|c| crate::db::settings::get(c, "start_minimized"))?
        .unwrap_or_else(|| "true".into())
        == "true";

    if launch_show_notes {
        let notes = state.db.with(crate::db::notes::all_active_with_windows)?;
        for note in notes {
            if let Err(e) = open_note_window(app, &note) {
                log::error!("启动打开便签失败 {}: {e}", note.id);
            }
        }
    }
    if !start_minimized {
        show_manager(app);
    }
    Ok(())
}

/// 供外部模块查询：当前被全屏策略隐藏的窗口集合（调试用）。
#[allow(dead_code)]
pub fn hidden_by_fullscreen(state: &AppState) -> HashSet<String> {
    lock_ok(&state.fullscreen_hidden).clone()
}

/// 窗口几何持久化的 id→(packed x,y,w,h) 映射类型（geometry 存取接口契约）。
#[allow(dead_code)]
pub type GeometryMap = HashMap<String, u64>;
