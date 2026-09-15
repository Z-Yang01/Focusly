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
export type ThemeMode = "system" | "light" | "dark";
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
  createdAt: string;
  updatedAt: string;
  archivedAt: string | null;
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
