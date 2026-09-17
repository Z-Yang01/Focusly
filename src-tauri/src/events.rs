//! 前端事件名统一常量（Rust → JS）。
//!
//! 与 `src/types/index.ts#EVENTS` / `POMODORO_EVENTS` 一一对应。
//! 约定：Rust 侧发送事件一律引用本模块常量，禁止在业务代码里
//! 硬编码事件名字符串（改名时只动这里与前端 types 两处）。

pub const NOTES_CHANGED: &str = "notes-changed";
pub const SETTINGS_CHANGED: &str = "settings-changed";
pub const REMINDER_FIRED: &str = "reminder-fired";
pub const SHORTCUT_ERROR: &str = "shortcut-error";
pub const NOTES_VISIBILITY: &str = "notes-visibility";
pub const FOCUS_SEARCH: &str = "focus-search";
pub const OPEN_SETTINGS: &str = "open-settings";
pub const POMODORO_STATE: &str = "pomodoro-state";
pub const POMODORO_FINISHED: &str = "pomodoro-finished";
/// 前端已约定（types/index.ts#POMODORO_EVENTS.taskMetaChanged）；
/// Rust 侧当前未发送，保留常量对齐契约。
#[allow(dead_code)]
pub const TASK_META_CHANGED: &str = "task-meta-changed";
