//! 系统托盘：菜单（显隐便签/新建/管理器/设置/退出）与左键单击行为。

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};

use crate::window;

/// 托盘图标 id（pomodoro.rs 经 app.tray_by_id 更新 tooltip）
pub const TRAY_ID: &str = "focusly-tray";

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_all = MenuItem::with_id(app, "show_all", "显示全部便签", true, None::<&str>)?;
    let hide_all = MenuItem::with_id(app, "hide_all", "隐藏全部便签", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let new_note = MenuItem::with_id(app, "new_note", "新建便签", true, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let open_manager = MenuItem::with_id(app, "open_manager", "打开管理器", true, None::<&str>)?;
    let open_settings = MenuItem::with_id(app, "open_settings", "设置", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_all,
            &hide_all,
            &sep1,
            &new_note,
            &sep2,
            &open_manager,
            &open_settings,
            &sep3,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID);
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder
        .menu(&menu)
        .tooltip("Focusly")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show_all" => {
                let _ = window::show_all_notes(app);
            }
            "hide_all" => {
                let _ = window::hide_all_notes(app);
            }
            "new_note" => {
                let _ = crate::notes::create_and_open(app);
            }
            "open_manager" => window::show_manager(app),
            "open_settings" => {
                window::show_manager(app);
                let _ = app.emit("open-settings", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                window::show_manager(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 更新托盘 tooltip（番茄钟运行状态："🍅 24:59 任务文本" / "⏸ 剩余 mm:ss"）。
/// 托盘不存在（创建失败/测试环境）或更新失败时仅记日志，静默降级。
pub fn set_tooltip(app: &AppHandle, text: &str) {
    match app.tray_by_id(TRAY_ID) {
        Some(tray) => {
            if let Err(e) = tray.set_tooltip(Some(text)) {
                log::warn!("托盘 tooltip 更新失败: {e}");
            }
        }
        None => log::debug!("托盘图标不存在（{TRAY_ID}），跳过 tooltip 更新"),
    }
}
