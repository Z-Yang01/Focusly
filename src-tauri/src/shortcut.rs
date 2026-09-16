//! 全局快捷键：从 DB 读配置注册到 global-shortcut 插件，并把触发分发到动作。

use std::collections::HashMap;

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::window;

/// DB 侧的 "Ctrl+Shift+N" 归一化为 tauri/global-hotkey 的小写形式 "ctrl+shift+n"。
pub fn accelerator_to_tauri(s: &str) -> String {
    s.trim().to_lowercase()
}

/// 匹配键：global-hotkey 的 Display 输出（如 "control+shift+Space"）与注册串
/// （如 "ctrl+shift+space"）不一致，不能用 to_string() 匹配。
/// 这里用 Shortcut 的 mods/key 的 Debug 形式做稳定的规范化键，
/// 注册时与回调触发时对同一 Shortcut 必然得到同一字符串。
fn shortcut_key(sc: &Shortcut) -> String {
    format!("{:?}|{:?}", sc.mods, sc.key)
}

/// 快捷键动作分发（供全局快捷键回调、托盘、前端复用）。
pub fn dispatch_action(app: &AppHandle, action: &str) {
    log::info!("快捷键动作触发: {action}");
    match action {
        "toggle_notes" => {
            let _ = window::toggle_all_notes(app);
        }
        "new_note" => {
            if let Err(e) = crate::notes::create_and_open(app) {
                log::error!("快捷键新建便签失败: {e}");
            }
        }
        "focus_search" => {
            window::show_manager(app);
            let _ = app.emit("focus-search", ());
        }
        "quick_capture" => {
            let _ = crate::quickcapture::toggle(app);
        }
        "pomodoro_toggle" => {
            let _ = crate::pomodoro::toggle_via_cmd(app);
        }
        other => log::warn!("未知的快捷键动作: {other}"),
    }
}

/// 读取 shortcuts 表并注册全部启用的快捷键。
/// 单条失败只记录并广播 shortcut-error，不阻断其他条目，也不返回 Err。
pub fn register_all(app: &AppHandle) -> AppResult<()> {
    let entries = {
        let state = app.state::<AppState>();
        state.db.with(|c| crate::db::shortcuts::list(c))?
    };

    app.global_shortcut().unregister_all().ok();

    let mut map: HashMap<String, String> = HashMap::new();
    for entry in &entries {
        if !entry.enabled {
            continue;
        }
        let accel = entry.accelerator.trim().to_string();
        if accel.is_empty() {
            continue;
        }
        let key = accelerator_to_tauri(&accel);

        let shortcut: Shortcut = match key.parse() {
            Ok(sc) => sc,
            Err(err) => {
                let msg = format!("快捷键格式无法解析: {accel}");
                log::warn!("{msg}: {err}");
                let _ = app.emit("shortcut-error", json!({"action": entry.action, "message": msg}));
                report_shortcut_failure(app, &msg, &err.to_string(), "该快捷键不可用");
                continue;
            }
        };
        let canonical = shortcut_key(&shortcut);

        // 冲突检测：同一组合键对应多个动作时，跳过后来者
        if let Some(prev) = map.insert(canonical.clone(), entry.action.clone()) {
            let msg = format!("与 {prev} 快捷键冲突");
            log::warn!("快捷键 {accel}: {msg}");
            let _ = app.emit("shortcut-error", json!({"action": entry.action, "message": msg}));
            report_shortcut_failure(
                app,
                &msg,
                &format!("动作 {} 与 {prev} 使用相同组合键 {accel}", entry.action),
                "后注册的快捷键不生效",
            );
            continue;
        }

        if let Err(err) = app.global_shortcut().register(key.as_str()) {
            let msg = format!("快捷键注册失败: {accel}");
            log::error!("{msg}: {err}");
            let _ = app.emit("shortcut-error", json!({"action": entry.action, "message": msg}));
            report_shortcut_failure(app, &msg, &err.to_string(), "组合键可能被其他应用占用，请在设置中更换");
            map.remove(&canonical);
            continue;
        }
        log::info!("已注册全局快捷键 {key} -> {}", entry.action);
    }

    let state = app.state::<AppState>();
    let mut guard = state
        .shortcut_map
        .lock()
        .map_err(|_| AppError::Platform("快捷键表锁被占用".into()))?;
    *guard = map;
    Ok(())
}

/// 查找某组合键当前绑定的动作（规范化键匹配，供快捷键回调使用）。
pub fn action_for_key(app: &AppHandle, sc: &Shortcut) -> Option<String> {
    let state = app.state::<AppState>();
    let map = state.shortcut_map.lock().ok()?;
    map.get(&shortcut_key(sc)).cloned()
}

fn report_shortcut_failure(app: &AppHandle, problem: &str, cause: &str, impact: &str) {
    let state = app.state::<AppState>();
    crate::filesystem::journal_error(
        &state.paths,
        "shortcut",
        problem,
        cause,
        impact,
        "在设置中修改快捷键组合后重试",
    );
}
