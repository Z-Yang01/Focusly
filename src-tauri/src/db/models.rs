use serde::{Deserialize, Serialize};

pub const STATUS_ACTIVE: &str = "active";
pub const STATUS_ARCHIVED: &str = "archived";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
    pub content_format: String,
    pub status: String,
    pub is_pinned: bool,
    pub is_always_on_top: bool,
    pub show_on_all_desktops: bool,
    pub desktop_pin_state: String,
    pub fullscreen_behavior: String,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub monitor_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    #[serde(flatten)]
    pub note: Note,
    pub tags: Vec<String>,
    pub todo_total: i64,
    pub todo_done: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteImage {
    pub id: String,
    pub note_id: String,
    pub path: String,
    pub filename: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatType {
    Once,
    Daily,
    Weekly,
    Weekdays,
}

impl RepeatType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepeatType::Once => "once",
            RepeatType::Daily => "daily",
            RepeatType::Weekly => "weekly",
            RepeatType::Weekdays => "weekdays",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "daily" => RepeatType::Daily,
            "weekly" => RepeatType::Weekly,
            "weekdays" => RepeatType::Weekdays,
            _ => RepeatType::Once,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReminderStatus {
    Pending,
    Triggered,
    Dismissed,
    Done,
    Cancelled,
}

impl ReminderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReminderStatus::Pending => "pending",
            ReminderStatus::Triggered => "triggered",
            ReminderStatus::Dismissed => "dismissed",
            ReminderStatus::Done => "done",
            ReminderStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub note_id: String,
    pub remind_at: String,
    pub repeat_type: String,
    pub status: String,
    pub created_at: String,
    pub triggered_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutEntry {
    pub action: String,
    pub accelerator: String,
    pub enabled: bool,
}

/// 便签全屏行为。
/// normal: 普通；always_top: 始终置顶；
/// fullscreen_show: 全屏时保持置顶显示；fullscreen_hide: 全屏时自动隐藏。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FullscreenBehavior {
    Normal,
    AlwaysTop,
    FullscreenShow,
    FullscreenHide,
}

impl FullscreenBehavior {
    pub fn as_str(&self) -> &'static str {
        match self {
            FullscreenBehavior::Normal => "normal",
            FullscreenBehavior::AlwaysTop => "always_top",
            FullscreenBehavior::FullscreenShow => "fullscreen_show",
            FullscreenBehavior::FullscreenHide => "fullscreen_hide",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "always_top" => FullscreenBehavior::AlwaysTop,
            "fullscreen_show" => FullscreenBehavior::FullscreenShow,
            "fullscreen_hide" => FullscreenBehavior::FullscreenHide,
            _ => FullscreenBehavior::Normal,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDetail {
    #[serde(flatten)]
    pub note: Note,
    pub images: Vec<NoteImage>,
    pub tags: Vec<String>,
    pub next_reminder: Option<Reminder>,
}

/// 导出文件结构（JSON 导入导出共用）。
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportData {
    pub app: String,
    pub schema_version: i64,
    pub exported_at: String,
    pub notes: Vec<Note>,
    pub note_images: Vec<NoteImage>,
    pub reminders: Vec<Reminder>,
    pub tags: Vec<(String, String)>, // note_id, tag_name
    pub shortcuts: Vec<ShortcutEntry>,
    pub settings: Vec<(String, String)>,
}
