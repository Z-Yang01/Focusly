/** Focusly 类型定义 — 与 Rust 侧 serde camelCase 输出严格对应 */

export type NoteStatus = "active" | "archived";
export type RepeatType = "once" | "daily" | "weekly" | "weekdays";
export type ReminderStatus = "pending" | "triggered" | "dismissed" | "done" | "cancelled";
export type FullscreenBehavior =
  | "normal"
  | "always_top"
  | "fullscreen_show"
  | "fullscreen_hide";
/** 虚拟桌面固定的真实状态（COM 不可用时明确报 unsupported，不伪造） */
export type DesktopPinState = "off" | "on" | "unsupported" | "failed";
export type NoteFilter = "all" | "active" | "archived" | "todo";
export type NoteFlag = "pinned" | "always_on_top" | "all_desktops";
export type ThemeMode = "system" | "light" | "dark" | "warm" | "forest" | "ocean";
export type ShortcutAction = "toggle_notes" | "new_note" | "focus_search";

export interface Note {
  id: string;
  title: string;
  content: string;
  contentFormat: string;
  status: NoteStatus;
  isPinned: boolean;
  isAlwaysOnTop: boolean;
  showOnAllDesktops: boolean;
  desktopPinState: DesktopPinState;
  fullscreenBehavior: FullscreenBehavior;
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
  monitorId: string | null;
  /** 回收站：非 null = 在回收站 */
  deletedAt: string | null;
  isPrivate: boolean;
  locked: boolean;
  readonly: boolean;
  /** 图钉三态: normal | topmost | desktop */
  pinMode: "normal" | "topmost" | "desktop";
  /** 恢复窗口时的显示器缩放 (1.0 / 1.25 / 1.5) */
  scale: number | null;
  createdAt: string;
  updatedAt: string;
  archivedAt: string | null;
}

/** FTS 全文搜索结果 */
export interface SearchHit {
  id: string;
  title: string;
  /** 含 <mark>高亮</mark> 的正文摘要 */
  snippet: string;
  tags: string[];
  todoTotal: number;
  todoDone: number;
  updatedAt: string;
  isPinned: boolean;
  isPrivate: boolean;
  status: NoteStatus;
}

/** 便签历史版本 */
export interface NoteVersion {
  id: string;
  noteId: string;
  title: string;
  content: string;
  /** auto | manual | pre-restore */
  source: VersionSource;
  createdAt: string;
}

/** 保存的搜索 */
export interface SavedSearch {
  id: string;
  name: string;
  query: string;
  createdAt: string;
}

/** 剪贴板历史条目 */
export interface ClipboardEntry {
  id: string;
  content: string;
  /** text | image_path */
  kind: string;
  createdAt: string;
  pinned: boolean;
}

export interface NoteSummary extends Note {
  tags: string[];
  todoTotal: number;
  todoDone: number;
}

export interface NoteImage {
  id: string;
  noteId: string;
  /** 本地绝对路径；前端用 convertFileSrc 转 asset:// URL */
  path: string;
  filename: string;
  width: number | null;
  height: number | null;
  createdAt: string;
}

export interface Reminder {
  id: string;
  noteId: string;
  /** RFC3339 (UTC) */
  remindAt: string;
  repeatType: RepeatType;
  status: ReminderStatus;
  createdAt: string;
  triggeredAt: string | null;
}

export interface NoteDetail extends Note {
  images: NoteImage[];
  tags: string[];
  nextReminder: Reminder | null;
}

export interface TagCount {
  name: string;
  count: number;
}

export interface ShortcutEntry {
  action: ShortcutAction;
  accelerator: string;
  enabled: boolean;
}

export interface AppInfo {
  dataDir: string;
  dbPath: string;
  version: string;
}

export interface ImportSummary {
  importedNotes: number;
  skipped: number;
}

/** 事件载荷 */
export interface ReminderFiredEvent {
  reminderId: string;
  noteId: string;
  noteTitle: string;
  remindAt: string;
}

export interface SettingsChangedEvent {
  key: string;
  value: string;
}

export interface ShortcutErrorEvent {
  action: string;
  message: string;
}

/** 事件名常量（Rust 侧 emit 同名） */
export const EVENTS = {
  notesChanged: "notes-changed",
  settingsChanged: "settings-changed",
  reminderFired: "reminder-fired",
  shortcutError: "shortcut-error",
  notesVisibility: "notes-visibility",
  focusSearch: "focus-search",
  openSettings: "open-settings",
} as const;

/** 设置键 */
export const SETTINGS_KEYS = {
  theme: "theme",
  closeAction: "close_action",
  autostart: "autostart",
  startMinimized: "start_minimized",
  launchShowNotes: "launch_show_notes",
} as const;

// ============================================================================
// 以下为 feature api 收敛（features/*/api.ts → lib/api.ts）迁入的类型契约，
// 均与 Rust 侧 serde camelCase 输出严格对应。
// ============================================================================

/** 版本保存来源（Rust notes 版本历史 source 列） */
export type VersionSource = "auto" | "manual" | "pre-restore";

/** 回收站条目（list_deleted_notes 返回） */
export interface TrashItem {
  id: string;
  title: string;
  content: string;
  /** RFC3339(UTC) 删除时间 */
  deletedAt: string;
  /** 归档时间；从未归档为 null */
  archivedAt: string | null;
  /** 最后修改时间 RFC3339(UTC) */
  updatedAt: string;
  todoTotal: number;
  todoDone: number;
}

/** 重复图片条目（Rust imagemgr::DupEntry serde camelCase 输出） */
export interface DupEntry {
  imageId: string;
  noteId: string;
  path: string;
  filename: string;
  size: number;
}

/** 一个重复组：同一 SHA-256 内容哈希对应 ≥2 个文件 */
export interface DupGroup {
  hash: string;
  entries: DupEntry[];
}

/** 布局预设（Rust LayoutPreset serde camelCase） */
export interface LayoutPreset {
  id: string;
  name: string;
  /** 布局快照 JSON：[[noteId,x,y,w,h],...] */
  data: string;
  createdAt: string;
}

/** 隐私标志（与 Rust notes::set_privacy_flag 的 flag 取值一致） */
export type PrivacyFlag = "private" | "locked" | "readonly";

/**
 * 私密便签列表项（list_private_notes 返回 NoteSummary 摊平 JSON）。
 * 列表 UI 仅渲染标题/徽标/更新时间，content 字段即使返回也一律不展示。
 */
export interface PrivateNote {
  id: string;
  title: string;
  isPrivate: boolean;
  locked: boolean;
  readonly: boolean;
  /** RFC3339(UTC) 最后修改时间 */
  updatedAt: string;
  /** 后端可能返回，本视图不渲染 */
  content?: string;
  todoTotal?: number;
  todoDone?: number;
}

/** parse_time_nl 命令返回：at 为 RFC3339 UTC，repeat 为 RepeatType 字面量 */
export interface NlParsed {
  at: string;
  repeat: string;
}

/** 到期视图中的单条便签（Rust TodoView/DueNoteSummary camelCase 输出） */
export interface DueNote {
  id: string;
  title: string;
  updatedAt: string;
  /** 该分区内（逾期/今天/未来7天）的到期任务数 */
  dueCount: number;
}

/** get_due_view 返回：逾期 / 今天 / 未来7天 三段分区 */
export interface DueViewData {
  overdue: DueNote[];
  today: DueNote[];
  next7days: DueNote[];
}

// ---------- 番茄钟（原 features/pomodoro/types.ts 契约） ----------

export type PomodoroPhase = "focus" | "short_break" | "long_break";

/** 所有 pomodoro_* 写命令的统一返回；noteId/taskKey/taskText 为 null 表示未绑定任务的专注 */
export interface StatePayload {
  /** P 的快照恒返回全量；running=false 表示空闲 */
  running?: boolean;
  phase: PomodoroPhase;
  /** RFC3339 UTC；null = 无进行中的阶段（暂停 / 空闲） */
  endsAt: string | null;
  paused: boolean;
  /** 暂停 / 空闲时的静态剩余秒数（运行中权威值是 endsAt） */
  remainingSec: number;
  noteId: string | null;
  taskKey: string | null;
  /** 私密便签的任务此处为 ""（脱敏） */
  taskText: string | null;
  /** 本轮周期内已完成的番茄数 */
  completedInCycle: number;
}

/** pomodoro-finished 事件载荷 */
export interface PomodoroFinishedEvent {
  phase: PomodoroPhase;
  nextPhase: PomodoroPhase;
  noteId: string | null;
  taskKey: string | null;
  /** 私密便签任务时为 "" */
  taskText: string | null;
}

/** task-meta-changed 事件载荷 */
export interface TaskMetaChangedEvent {
  noteId: string;
}

/** task_meta 行（与 Rust db/models.rs TaskMeta 的 serde camelCase 输出严格对应；
 *  结构与 taskMeta.ts 的 MetaLike 兼容，可直接作为 MetaLike[] 传入 mergeTaskMeta） */
export interface TaskMeta {
  noteId: string;
  taskKey: string;
  lineText: string;
  /** todo | done | skipped */
  status: string;
  estimatePomodoros: number;
  completedPomodoros: number;
  /** high | medium | low */
  priority: string | null;
  /** RFC3339 UTC */
  dueAt: string | null;
  /** 跳过日期 YYYY-MM-DD；跨日自动复活为 todo */
  skipDate: string | null;
  /** 本任务专注时长覆盖（分钟；null = 全局默认 25） */
  focusMin: number | null;
  /** RFC3339 UTC */
  updatedAt: string;
}

/** pomodoro_stats_today 返回 */
export interface StatsToday {
  focusCount: number;
  focusSec: number;
  doneTasks: number;
  skippedTasks: number;
  interrupts: number;
}

/** pomodoro_stats_range(days) 返回的单日统计 */
export interface DailyStat {
  /** 本地日期 YYYY-MM-DD */
  date: string;
  focusCount: number;
  focusSec: number;
}

/** 番茄钟事件名（Rust 侧 emit 同名） */
export const POMODORO_EVENTS = {
  state: "pomodoro-state",
  finished: "pomodoro-finished",
  taskMetaChanged: "task-meta-changed",
} as const;

/** 今日任务（daily_tasks 表，与 Rust db/models.rs DailyTask serde camelCase 输出对应）。
 *  status: todo | done | skipped；repeatRule: none | daily | weekly | weekday；
 *  repeatRule != none 的行是模板，不会出现在日视图（list 先物化实例）。 */
export interface DailyTask {
  id: string;
  /** 本地日期 YYYY-MM-DD */
  date: string;
  /** 本地时刻 HH:MM；null = 未排时（排在日视图末尾"未排时"区） */
  startTime: string | null;
  endTime: string | null;
  title: string;
  note: string | null;
  estimatePomodoros: number;
  completedPomodoros: number;
  /** high | medium | low */
  priority: string;
  /** todo | done | skipped */
  status: string;
  tags: string | null;
  repeatRule: string;
  isPrivate: boolean;
  startNotified: boolean;
  sourceTaskId: string | null;
  /** 本任务专注时长覆盖（分钟；null = 全局默认 25） */
  focusMin: number | null;
  createdAt: string;
  updatedAt: string;
}

/** daily_task_stats 返回（时间轴头部 + 统计页共用） */
export interface DailyTaskStats {
  total: number;
  done: number;
  skipped: number;
  estimatePomodoros: number;
  completedPomodoros: number;
  /** 已完成任务的计划专注时长（分钟） */
  plannedFocusMinutes: number;
}
