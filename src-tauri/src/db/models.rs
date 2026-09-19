use serde::{Deserialize, Serialize};

/// 便签状态字面量契约（SQL 暂用字符串字面量，统一引用时收敛到这里）
#[allow(dead_code)]
pub const STATUS_ACTIVE: &str = "active";
#[allow(dead_code)]
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
    /// 以下 v2 字段允许旧导出 JSON 缺省（默认值与建表默认一致），序列化输出不受影响
    #[serde(default)]
    /// 回收站：软删除时间（NULL 表示未删除）
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub is_private: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    /// 数据库列为 readonly_flag（readonly 有 SQL 关键字风险），serde 输出名仍为 readonly
    pub readonly: bool,
    #[serde(default)]
    pub scale: Option<f64>,
    /// 图钉三态: normal | topmost | desktop
    #[serde(default)]
    pub pin_mode: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepeatType {
    Once,
    Daily,
    Weekly,
    Weekdays,
    Monthly,
    Yearly,
}

impl RepeatType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepeatType::Once => "once",
            RepeatType::Daily => "daily",
            RepeatType::Weekly => "weekly",
            RepeatType::Weekdays => "weekdays",
            RepeatType::Monthly => "monthly",
            RepeatType::Yearly => "yearly",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "daily" => RepeatType::Daily,
            "weekly" => RepeatType::Weekly,
            "weekdays" => RepeatType::Weekdays,
            "monthly" => RepeatType::Monthly,
            "yearly" => RepeatType::Yearly,
            _ => RepeatType::Once,
        }
    }
}

/// 提醒状态类型契约：reminders.status 列暂以 String 直读直写，
/// 此枚举为后续把状态收敛为强类型的预留表示（与列字面量一一对应）。
#[allow(dead_code)]
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
    #[allow(dead_code)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub note_id: String,
    pub remind_at: String,
    pub repeat_type: String,
    pub status: String,
    pub created_at: String,
    pub triggered_at: Option<String>,
    /// 月/年重复的原始锚点（v11；NULL = v11 前旧行，退化为以 remind_at 为锚）。
    /// 链式补建下一轮时透传本值，防止月末 clamp 后日号永久漂移（31→28→28）。
    #[serde(default)]
    pub anchor_at: Option<String>,
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
    // 与 parse 互为反函数；序列化走 serde rename_all，as_str 供日志/DB 字面量场景预留
    #[allow(dead_code)]
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
    /// 本次导出是否包含私密便签；旧导出文件缺省按 false 处理（导入不受影响）
    #[serde(default)]
    pub includes_private: bool,
    /// 任务三态元数据（新导出格式；旧文件缺省为空，导入不受影响）
    #[serde(default)]
    pub task_meta: Vec<TaskMeta>,
}

/// 全文搜索命中项（snippet 内含 <mark> 高亮标记）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub todo_total: i64,
    pub todo_done: i64,
    pub updated_at: String,
    pub is_pinned: bool,
    pub is_private: bool,
    pub status: String,
}

/// 便签版本快照（note_versions 表）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteVersion {
    pub id: String,
    pub note_id: String,
    pub title: String,
    pub content: String,
    /// auto | manual | pre-restore
    pub source: String,
    pub created_at: String,
}

/// 保存的搜索（saved_searches 表，按 name 唯一）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    pub id: String,
    pub name: String,
    pub query: String,
    pub created_at: String,
}

/// 剪贴板历史条目（clipboard_history 表）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    /// text | image …（kind 列）
    pub kind: String,
    pub created_at: String,
    pub pinned: bool,
}

/// 窗口布局预设（layout_presets 表，name 唯一；data 为窗口快照 JSON：[[noteId,x,y,w,h],...]）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPreset {
    pub id: String,
    pub name: String,
    pub data: String,
    pub created_at: String,
}

/// 番茄钟会话（pomodoro_sessions 表）。status: running | completed | interrupted。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroSession {
    pub id: String,
    pub note_id: Option<String>,
    pub task_key: Option<String>,
    pub task_text_snapshot: Option<String>,
    pub phase: String,
    pub planned_sec: i64,
    pub actual_sec: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub interrupt_reason: Option<String>,
}

/// 任务元数据（task_meta 表，主键 note_id+task_key）。
/// status: todo | done | skipped；skip_date 记录跳过日期，跨日自动复活为 todo。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMeta {
    pub note_id: String,
    pub task_key: String,
    pub line_text: String,
    pub status: String,
    pub estimate_pomodoros: i64,
    pub completed_pomodoros: i64,
    pub priority: Option<String>,
    pub due_at: Option<String>,
    pub skip_date: Option<String>,
    /// 本任务专注时长覆盖（分钟；None = 全局设置）
    pub focus_min: Option<i64>,
    /// 拖动展示顺序（1..n；None = 未排序，按内容顺序兜底）。只影响展示，不回写正文。
    #[serde(default)]
    pub sort_order: Option<i64>,
    pub updated_at: String,
}

/// 今日任务（daily_tasks 表）：独立于便签的时间轴任务。
/// status: todo | done | skipped；repeat_rule: none | daily | weekly | weekday。
/// repeat_rule != none 的行是"模板"：由 `materialize_recurring` 按日物化为
/// source_task_id 指向模板、repeat_rule='none' 的实例行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTask {
    pub id: String,
    /// 本地日期 YYYY-MM-DD
    pub date: String,
    /// 本地时刻 HH:MM（可空 = 无固定时间，排在"全天/未排时"区）
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub title: String,
    pub note: Option<String>,
    pub estimate_pomodoros: i64,
    pub completed_pomodoros: i64,
    /// high | medium | low
    pub priority: String,
    /// todo | done | skipped
    pub status: String,
    /// 逗号分隔标签（轻量存储，不做关系表）
    pub tags: Option<String>,
    pub repeat_rule: String,
    pub is_private: bool,
    pub start_notified: bool,
    /// 重复模板实例的来源模板 id
    pub source_task_id: Option<String>,
    /// 本任务专注时长覆盖（分钟；None = 全局设置）
    pub focus_min: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

/// 今日任务统计（时间轴头部与番茄统计页共用）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyTaskStats {
    pub total: i64,
    pub done: i64,
    pub skipped: i64,
    pub estimate_pomodoros: i64,
    pub completed_pomodoros: i64,
    /// 已完成任务的计划专注时长（分钟；按 start/end 差值累计，缺 end 按 0）
    pub planned_focus_minutes: i64,
}
