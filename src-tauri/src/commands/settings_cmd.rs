//! 设置与快捷键命令。

use std::collections::HashMap;

use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use crate::db::models::ShortcutEntry;
use crate::error::{AppError, AppResult};
use crate::state::AppState;

const SETTING_KEYS: &[&str] = &[
    "theme",
    "close_action",
    "autostart",
    "start_minimized",
    "launch_show_notes",
];

const SHORTCUT_ACTIONS: &[&str] = &["toggle_notes", "new_note", "focus_search"];

#[tauri::command]
pub fn get_all_settings(state: State<'_, AppState>) -> AppResult<HashMap<String, String>> {
    state.db.with(|c| crate::db::settings::get_all(c))
}

#[tauri::command]
pub fn set_setting(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> AppResult<()> {
    if !SETTING_KEYS.contains(&key.as_str()) {
        return Err(AppError::Invalid(format!("未知设置项: {key}")));
    }
    state.db.with(|c| crate::db::settings::set(c, &key, &value))?;

    if key == "autostart" {
        use tauri_plugin_autostart::ManagerExt;
        let result = if value == "true" {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(e) = result {
            log::error!("设置开机自启动失败: {e}");
            crate::filesystem::journal_error(
                &state.paths,
                "window",
                "开机自启动设置失败",
                &e.to_string(),
                "实际自启动状态可能与设置不一致",
                "检查系统启动项权限后重试",
            );
        }
    }

    let _ = app.emit("settings-changed", json!({"key": key, "value": value}));
    Ok(())
}

#[tauri::command]
pub fn get_shortcuts(state: State<'_, AppState>) -> AppResult<Vec<ShortcutEntry>> {
    state.db.with(|c| crate::db::shortcuts::list(c))
}

/// 校验并归一化组合键：小写；每段 ∈ {ctrl,shift,alt,super} 或单字符 / F1-F24；修饰键在前。
fn validate_accelerator(accelerator: &str) -> AppResult<String> {
    let trimmed = accelerator.trim();
    if trimmed.is_empty() {
        return Err(AppError::Invalid("快捷键不能为空".into()));
    }
    const MODIFIERS: &[&str] = &["ctrl", "shift", "alt", "super"];
    let mut has_key = false;
    let mut parts: Vec<String> = Vec::new();
    for raw in trimmed.split('+') {
        let part = raw.trim().to_lowercase();
        if part.is_empty() {
            return Err(AppError::Invalid(format!("快捷键格式无效: {accelerator}")));
        }
        if MODIFIERS.contains(&part.as_str()) {
            if has_key {
                return Err(AppError::Invalid(format!(
                    "快捷键格式无效（修饰键须在按键之前）: {accelerator}"
                )));
            }
            parts.push(part);
            continue;
        }
        let is_fkey = part
            .strip_prefix('f')
            .and_then(|digits| digits.parse::<u8>().ok())
            .map(|n| (1..=24).contains(&n))
            .unwrap_or(false);
        let valid = part.chars().count() == 1 || is_fkey;
        if !valid {
            return Err(AppError::Invalid(format!(
                "快捷键包含无法识别的按键: {part}"
            )));
        }
        has_key = true;
        parts.push(part);
    }
    if !has_key {
        return Err(AppError::Invalid(format!("快捷键缺少主按键: {accelerator}")));
    }
    Ok(parts.join("+"))
}

#[tauri::command]
pub fn set_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    accelerator: String,
) -> AppResult<()> {
    if !SHORTCUT_ACTIONS.contains(&action.as_str()) {
        return Err(AppError::Invalid(format!("未知快捷键动作: {action}")));
    }
    let normalized = validate_accelerator(&accelerator)?;

    // 冲突：其他动作已占用该组合键
    let entries = state.db.with(|c| crate::db::shortcuts::list(c))?;
    if let Some(other) = entries
        .iter()
        .find(|e| e.action != action && e.accelerator.eq_ignore_ascii_case(&normalized))
    {
        return Err(AppError::Shortcut(format!(
            "与 {} 快捷键冲突",
            other.action
        )));
    }

    state.db.with(|c| crate::db::shortcuts::set(c, &action, &normalized))?;
    // 重注册全部（失败条目内部已记录，不影响整体返回）
    crate::shortcut::register_all(&app)?;
    let _ = app.emit(
        "settings-changed",
        json!({"key": "shortcuts", "action": action, "value": normalized}),
    );
    Ok(())
}

#[tauri::command]
pub fn reset_shortcuts(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    state.db.with(|c| crate::db::shortcuts::reset_all(c))?;
    crate::shortcut::register_all(&app)?;
    let _ = app.emit(
        "settings-changed",
        json!({"key": "shortcuts", "value": "reset"}),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerator_validation() {
        assert_eq!(validate_accelerator("Ctrl+Shift+Space").unwrap(), "ctrl+shift+space");
        assert_eq!(validate_accelerator("ctrl+shift+n").unwrap(), "ctrl+shift+n");
        assert_eq!(validate_accelerator("Alt+F4").unwrap(), "alt+f4");
        assert_eq!(validate_accelerator("F5").unwrap(), "f5");
        assert_eq!(validate_accelerator("super+q").unwrap(), "super+q");
        assert_eq!(validate_accelerator("ctrl+alt+F12").unwrap(), "ctrl+alt+f12");

        // 非法输入
        assert!(validate_accelerator("").is_err());
        assert!(validate_accelerator("ctrl").is_err(), "缺少主按键");
        assert!(validate_accelerator("shift+ctrl+n").is_err(), "修饰键顺序错误");
        assert!(validate_accelerator("ctrl+ctrl+n").is_err(), "主按键后不得再出现修饰键");
        assert!(validate_accelerator("ctrl+foo").is_err(), "多字符非法按键");
        assert!(validate_accelerator("ctrl+f25").is_err(), "F 键范围 1-24");
        assert!(validate_accelerator("ctrl++n").is_err(), "空段非法");
    }
}
