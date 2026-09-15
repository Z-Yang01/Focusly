//! 窗口服务：便签多窗口创建/显隐/几何持久化、管理器窗口、全屏策略应用。

pub mod foreground;
pub mod monitor;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::db::models::{Note, FullscreenBehavior};
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

/// 为新便签计算层叠位置（主屏右上角开始）。
pub fn cascade_position(app: &AppHandle, existing: usize) -> (i32, i32) {
    let (mut x, mut y) = (100, 80);
    if let Ok(Some(m)) = app.primary_monitor() {
        let pos = m.position();
        let size = m.size();
        x = pos.x + size.width as i32 - DEFAULT_NOTE_W - 48 - (existing as i32 % 8) * CASCADE_STEP;
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

    let (mut x, mut y) = note.x.zip(note.y).unwrap_or_else(|| cascade_position(app, 0));
    let width = note.width.unwrap_or(DEFAULT_NOTE_W);
    let height = note.height.unwrap_or(DEFAULT_NOTE_H);
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

    let _ = win.set_size(PhysicalSize::new(width.max(120) as u32, height.max(100) as u32));
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
        let _ = app.state::<AppState>().db.with(|c| {
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
                    touch: false,
                },
            )
        });
    }
}

/// 便签窗口首次渲染完成：应用几何并显示（避免白闪）。
pub fn note_window_ready(app: &AppHandle, note_id: &str) -> AppResult<()> {
    let label = note_label(note_id);
    let win = app
        .get_webview_window(&label)
        .ok_or_else(|| AppError::Window(format!("窗口不存在: {label}")))?;
    let note = app.state::<AppState>().db.with(|c| crate::db::notes::get(c, note_id))?;
    let _ = win.set_always_on_top(note.is_always_on_top || note.fullscreen_behavior == "always_top");
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
    state.fullscreen_hidden.lock().unwrap().remove(&label);
}

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
        let notes = state.db.with(|c| crate::db::notes::all_active_with_windows(c))?;
        notes.iter().map(|n| note_label(&n.id)).collect()
    };
    {
        let mut hidden = app.state::<AppState>().fullscreen_hidden.lock().unwrap();
        for label in &labels {
            if let Some(win) = app.get_webview_window(label) {
                let _ = win.show();
                let _ = win.unminimize();
            }
            hidden.remove(label);
        }
    }
    let _ = app.emit("notes-visibility", true);
    Ok(())
}

/// 隐藏全部便签窗口（只隐藏窗口，不删内容）。
pub fn hide_all_notes(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let mut hidden = state.fullscreen_hidden.lock().unwrap();
    for (label, win) in app.webview_windows() {
        if note_id_from_label(&label).is_some() {
            let _ = win.hide();
            hidden.insert(label);
        }
    }
    let _ = app.emit("notes-visibility", false);
    Ok(())
}

pub fn toggle_all_notes(app: &AppHandle) -> AppResult<()> {
    let any_visible = app.webview_windows().iter().any(|(label, win)| {
        note_id_from_label(label).is_some()
            && win.is_visible().unwrap_or(false)
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
    let mut hidden = state.fullscreen_hidden.lock().unwrap();
    let Ok(notes) = state.db.with(|c| crate::db::notes::all_active_with_windows(c)) else {
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
                let _ = win.set_always_on_top(false);
                if !fullscreen && hidden.contains(&label) {
                    let _ = win.show();
                    hidden.remove(&label);
                }
            }
        }
    }
}

/// 立即保存窗口几何（关闭前兜底）。
pub fn save_geometry_now(app: &AppHandle, win: &tauri::WebviewWindow) {
    if note_id_from_label(win.label()).is_none() {
        return;
    }
    if let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) {
        let monitor = hwnd_of(win).and_then(monitor::monitor_of_window);
        let _ = app.state::<AppState>().db.with(|c| {
            crate::db::notes::update_geometry(
                c,
                note_id_from_label(win.label()).unwrap(),
                pos.x,
                pos.y,
                size.width as i32,
                size.height as i32,
                monitor.as_deref(),
            )
        });
    }
}

/// tauri 的 HWND（其内部 windows crate 版本可能与本 crate 不同）转换为本 crate 的 HWND。
#[cfg(windows)]
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
        let mut map = state.geometry_gens.lock().unwrap();
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
        let current = app
            .state::<AppState>()
            .geometry_gens
            .lock()
            .unwrap()
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
        let _ = win.set_always_on_top(note.is_always_on_top || note.fullscreen_behavior == "always_top");
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
        let notes = state.db.with(|c| crate::db::notes::all_active_with_windows(c))?;
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
pub fn hidden_by_fullscreen(state: &AppState) -> HashSet<String> {
    state.fullscreen_hidden.lock().unwrap().clone()
}

/// 类型别名映射（内部使用）
pub type GeometryMap = HashMap<String, u64>;
