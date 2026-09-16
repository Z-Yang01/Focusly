//! Focusly 应用组装：插件注册、setup 初始化顺序、窗口事件与命令表。

mod commands;
mod daily;
mod db;
mod dnd;
mod error;
mod export;
mod filesystem;
mod imagemgr;
mod logger;
mod notes;
mod pomodoro;
mod privacy;
mod quickcapture;
mod reminder;
mod shortcut;
mod state;
mod timeparse;
mod tray;
mod vdesktop;
mod window;

use tauri::Manager;

use state::AppState;

pub fn run() {
    tauri::Builder::default()
        // 单实例：必须最先注册，二次启动时唤起已有实例的管理器
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_manager(app);
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        // shortcut.to_string() 与注册串格式不一致，
                        // 用 mods/key 的稳定规范化键匹配（见 shortcut.rs）
                        if let Some(action) = shortcut::action_for_key(app, shortcut) {
                            shortcut::dispatch_action(app, &action);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            // 1. 数据目录与日志（便携模式：exe 旁有 portable.marker 时数据随程序走）
            let data_dir = filesystem::resolve_data_root(app.handle());
            let paths = filesystem::AppPaths::init(data_dir).expect("无法初始化应用数据目录");
            logger::init(&paths.logs);
            log::info!(
                "Focusly v{} 启动，数据目录 {}",
                env!("CARGO_PKG_VERSION"),
                paths.root.display()
            );

            // 2. 数据库：打开 → 迁移 → 启动备份
            let app_db = db::Db::open(&paths.db)?;
            app_db.with(db::migrations::run)?;
            if let Err(e) = filesystem::backup_database(&paths) {
                log::warn!("启动备份数据库失败: {e}");
            }

            // 3. 共享状态（先 manage，后续模块才能 app.state::<AppState>()）
            let (scheduler, scheduler_rx) = reminder::channel();
            let pomodoro = pomodoro::spawn(app.handle().clone());
            app.manage(AppState {
                db: app_db,
                paths,
                scheduler,
                pomodoro,
                shortcut_map: std::sync::Mutex::new(std::collections::HashMap::new()),
                fullscreen_hidden: std::sync::Mutex::new(std::collections::HashSet::new()),
                geometry_gens: std::sync::Mutex::new(std::collections::HashMap::new()),
            });
            // 番茄钟启动恢复：遗留 running 会话标记 interrupted，24h 内补发阶段结束通知
            pomodoro::recover(app.handle());

            // 4. 托盘（失败仅日志，不阻断启动）
            if let Err(e) = tray::create_tray(app.handle()) {
                log::error!("托盘图标创建失败: {e}");
            }

            // 5. 全局快捷键
            if let Err(e) = shortcut::register_all(app.handle()) {
                log::error!("全局快捷键注册失败: {e}");
            }

            // 6. 启动窗口（便签恢复 / 管理器是否显示）
            window::startup_windows(app.handle())?;

            // 7. 全屏检测 + 提醒调度器（依赖已 manage 的状态）
            window::foreground::spawn(app.handle().clone());
            reminder::spawn_loop(app.handle().clone(), scheduler_rx);

            Ok(())
        })
        .on_window_event(|window, event| {
            window::handle_window_event(window.app_handle(), window.label(), event);
        })
        .invoke_handler(tauri::generate_handler![
            // 便签
            commands::notes_cmd::create_note,
            commands::notes_cmd::new_note,
            commands::notes_cmd::open_note_window,
            commands::notes_cmd::get_note,
            commands::notes_cmd::list_notes,
            commands::notes_cmd::update_note_content,
            commands::notes_cmd::set_note_flag,
            commands::notes_cmd::set_note_fullscreen_behavior,
            commands::notes_cmd::archive_note,
            commands::notes_cmd::restore_note,
            commands::notes_cmd::delete_note,
            commands::notes_cmd::search_notes,
            commands::notes_cmd::set_note_tags,
            commands::notes_cmd::get_tags,
            // 图片
            commands::images_cmd::add_image,
            commands::images_cmd::add_image_data,
            commands::images_cmd::remove_image,
            commands::images_cmd::image_exists,
            // 提醒
            commands::reminders_cmd::set_reminder,
            commands::reminders_cmd::cancel_reminder,
            commands::reminders_cmd::complete_reminder,
            commands::reminders_cmd::snooze_reminder,
            commands::reminders_cmd::list_reminders,
            commands::reminders_cmd::parse_time_nl,
            // 设置
            commands::settings_cmd::get_all_settings,
            commands::settings_cmd::set_setting,
            commands::settings_cmd::get_shortcuts,
            commands::settings_cmd::set_shortcut,
            commands::settings_cmd::reset_shortcuts,
            // 系统
            commands::system_cmd::show_all_notes,
            commands::system_cmd::hide_all_notes,
            commands::system_cmd::toggle_all_notes,
            commands::system_cmd::show_manager,
            commands::system_cmd::hide_manager,
            commands::system_cmd::quit_app,
            commands::system_cmd::note_window_ready,
            commands::system_cmd::close_note_window,
            commands::system_cmd::export_data,
            commands::system_cmd::import_data,
            commands::system_cmd::open_external,
            commands::system_cmd::reveal_data_dir,
            commands::system_cmd::get_app_info,
            // 回收站 / 版本历史 / 私密 / FTS 搜索
            commands::notes_cmd::list_deleted_notes,
            commands::notes_cmd::trash_note,
            commands::notes_cmd::restore_from_trash,
            commands::notes_cmd::empty_trash,
            commands::notes_cmd::list_versions,
            commands::notes_cmd::restore_version,
            commands::notes_cmd::set_note_privacy,
            commands::notes_cmd::search_notes_v2,
            commands::notes_cmd::list_private_notes,
            // 保存的搜索
            commands::settings_cmd::save_search,
            commands::settings_cmd::list_saved_searches,
            commands::settings_cmd::delete_saved_search,
            // 快速捕获 / 剪贴板历史
            commands::quickcapture_cmd::quickcapture_toggle,
            commands::quickcapture_cmd::quickcapture_hide,
            commands::quickcapture_cmd::quickcapture_ready,
            commands::quickcapture_cmd::clipboard_add,
            commands::quickcapture_cmd::clipboard_list,
            commands::quickcapture_cmd::clipboard_remove,
            commands::quickcapture_cmd::clipboard_clear,
            commands::quickcapture_cmd::clipboard_pin,
            // 窗口布局
            commands::layout_cmd::layout_save_preset,
            commands::layout_cmd::layout_list_presets,
            commands::layout_cmd::layout_apply_preset,
            commands::layout_cmd::layout_delete_preset,
            commands::layout_cmd::layout_arrange_grid,
            // 今日/逾期视图（中文自然语言提醒解析 parse_time_nl 由提醒设置 UI 后续接入）
            commands::notes_cmd::get_due_view,
            // 图片管理
            commands::imagemgr_cmd::image_find_duplicates,
            commands::imagemgr_cmd::image_cleanup_orphans,
            commands::imagemgr_cmd::image_make_thumbnails,
            // 模板与每日笔记
            commands::notes_cmd::daily_get_or_create,
            // 番茄钟
            commands::pomodoro_cmd::pomodoro_start,
            commands::pomodoro_cmd::pomodoro_pause,
            commands::pomodoro_cmd::pomodoro_resume,
            commands::pomodoro_cmd::pomodoro_skip,
            commands::pomodoro_cmd::pomodoro_stop,
            commands::pomodoro_cmd::pomodoro_add_minutes,
            commands::pomodoro_cmd::pomodoro_state,
            commands::pomodoro_cmd::pomodoro_complete_task,
            commands::pomodoro_cmd::pomodoro_stats_today,
            commands::pomodoro_cmd::pomodoro_stats_range,
            commands::pomodoro_cmd::task_meta_get,
            commands::pomodoro_cmd::task_meta_list,
            commands::pomodoro_cmd::task_meta_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running focusly");
}
