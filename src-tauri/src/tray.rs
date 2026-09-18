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
    let pomo_toggle =
        MenuItem::with_id(app, "pomo_toggle", "番茄钟 开始/暂停", true, None::<&str>)?;
    let pomo_stop = MenuItem::with_id(app, "pomo_stop", "番茄钟 停止", true, None::<&str>)?;
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
            &pomo_toggle,
            &pomo_stop,
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
            // 托盘动作失败必须留痕：用户点了菜单却无反馈时，日志是唯一线索
            "show_all" => {
                if let Err(e) = window::show_all_notes(app) {
                    log::error!("托盘显示全部便签失败: {e}");
                }
            }
            "hide_all" => {
                if let Err(e) = window::hide_all_notes(app) {
                    log::error!("托盘隐藏全部便签失败: {e}");
                }
            }
            "new_note" => {
                // 开窗必须避开主线程（同步死锁，见 notes_cmd 注释），丢到异步运行时执行
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = crate::notes::create_and_open(&app) {
                        log::error!("托盘新建便签失败: {e}");
                    }
                });
            }
            // 番茄控制经 mpsc 发送到调度器，非阻塞，主线程安全
            "pomo_toggle" => {
                if let Err(e) = crate::pomodoro::toggle_via_cmd(app) {
                    log::error!("托盘番茄开关失败: {e}");
                }
            }
            "pomo_stop" => {
                if let Err(e) = crate::pomodoro::send_cmd(
                    app,
                    crate::pomodoro::PomodoroCmd::Stop {
                        reason: "tray".into(),
                    },
                ) {
                    log::error!("托盘番茄停止失败: {e}");
                }
            }
            "open_manager" => window::show_manager(app),
            "open_settings" => {
                window::show_manager(app);
                let _ = app.emit(crate::events::OPEN_SETTINGS, ());
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
