/** Tauri API 薄封装 — 全项目唯一的 invoke 入口 */
import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  ClipboardEntry,
  DailyStat,
  DailyTask,
  DailyTaskStats,
  DupGroup,
  DueViewData,
  FullscreenBehavior,
  FocusReport,
  ImportSummary,
  LayoutPreset,
  Note,
  NoteDetail,
  NoteFilter,
  NoteFlag,
  NoteImage,
  NoteSummary,
  NoteVersion,
  NlParsed,
  PomodoroSession,
  PrivacyFlag,
  PrivateNote,
  RepeatType,
  Reminder,
  SavedSearch,
  SearchHit,
  ShortcutAction,
  ShortcutEntry,
  StatePayload,
  StatsToday,
  TagCount,
  TaskMeta,
  TrashItem,
} from "@/types";

// ---------- 便签 ----------
export const createNote = () => invoke<Note>("create_note");
export const newNote = () => invoke<Note>("new_note");
export const openNoteWindow = (noteId: string) => invoke<void>("open_note_window", { noteId });
export const getNote = (noteId: string) => invoke<NoteDetail>("get_note", { noteId });
export const listNotes = (filter: NoteFilter) => invoke<NoteSummary[]>("list_notes", { filter });
export const updateNoteContent = (noteId: string, title: string, content: string) =>
  invoke<Note>("update_note_content", { noteId, title, content });
export const setNoteFlag = (noteId: string, flag: NoteFlag, value: boolean) =>
  invoke<Note>("set_note_flag", { noteId, flag, value });
export const setNoteFullscreenBehavior = (noteId: string, behavior: FullscreenBehavior) =>
  invoke<Note>("set_note_fullscreen_behavior", { noteId, behavior });
export const archiveNote = (noteId: string) => invoke<Note>("archive_note", { noteId });
export const restoreNote = (noteId: string) => invoke<Note>("restore_note", { noteId });
export const deleteNote = (noteId: string) => invoke<void>("delete_note", { noteId });
/** 移入回收站（软删除，可恢复）；回收站 UI 见 features/archive */
export const trashNote = (noteId: string) => invoke<Note>("trash_note", { noteId });
export const searchNotes = (query: string) => invoke<NoteSummary[]>("search_notes", { query });
export const setNoteTags = (noteId: string, tags: string[]) =>
  invoke<string[]>("set_note_tags", { noteId, tags });
export const getTags = () => invoke<TagCount[]>("get_tags");

// ---------- 搜索 v2 / 保存的搜索 ----------
/** 全文搜索 v2（FTS5 trigram）；与 searchNotes（降级 LIKE 版）并存，签名不同名不同义 */
export const searchNotesV2 = (query: string, includePrivate = false) =>
  invoke<SearchHit[]>("search_notes_v2", { query, includePrivate });
export const listSavedSearches = () => invoke<SavedSearch[]>("list_saved_searches");
export const saveSearch = (name: string, query: string) =>
  invoke<void>("save_search", { name, query });
export const deleteSavedSearch = (id: string) => invoke<void>("delete_saved_search", { id });

// ---------- 回收站 / 版本历史 ----------
export const listDeletedNotes = () => invoke<TrashItem[]>("list_deleted_notes");
export const restoreFromTrash = (noteId: string) =>
  invoke<void>("restore_from_trash", { noteId });
/** 永久删除（调用方需先 confirm）。
 *  契约审计修复：Rust 端无 purge_note 命令，永久删除复用已注册的 delete_note
 *  （notes::delete_permanently：关窗 + FTS 清理 + 删除记录与图片文件）。 */
export const purgeNote = (noteId: string) => invoke<void>("delete_note", { noteId });
export const emptyTrash = () => invoke<void>("empty_trash");
export const listVersions = (noteId: string, limit = 50) =>
  invoke<NoteVersion[]>("list_versions", { noteId, limit });
/** 恢复到指定版本（后端自动做恢复前备份），返回恢复后的便签 */
export const restoreVersion = (versionId: string) =>
  invoke<Note>("restore_version", { versionId });

// ---------- 私密空间 ----------
/** 设置隐私标志（后端命令 set_note_privacy → notes::set_privacy_flag，flag ∈ private/locked/readonly） */
export const setPinMode = (noteId: string, mode: string) =>
  invoke<Note>("set_pin_mode", { noteId, mode });

export const setNotePrivacy = (noteId: string, flag: PrivacyFlag, value: boolean) =>
  invoke<Note>("set_note_privacy", { noteId, flag, value });
/** 私密便签列表（list_private_notes：active 未删 is_private=1 的 NoteSummary） */
export const listPrivateNotes = () => invoke<PrivateNote[]>("list_private_notes");

// ---------- 图片 / 图片管理 ----------
export const addImage = (noteId: string, path: string) =>
  invoke<NoteImage>("add_image", { noteId, path });
export const addImageData = (noteId: string, width: number, height: number, bytes: number[]) =>
  invoke<NoteImage>("add_image_data", { noteId, width, height, bytes });
export const removeImage = (imageId: string) => invoke<void>("remove_image", { imageId });
export const imageExists = (path: string) => invoke<boolean>("image_exists", { path });
/** 按文件内容 SHA-256 找重复图片（仅返回 ≥2 条的组） */
export const imageFindDuplicates = () => invoke<DupGroup[]>("image_find_duplicates");
/** 清理 images/ 下未被数据库引用的孤儿文件，返回删除数量 */
export const imageCleanupOrphans = () => invoke<number>("image_cleanup_orphans");
/** 生成缩略图（thumbs/<imageId>.jpg，最长边 320，JPEG q80），返回生成数量 */
export const imageMakeThumbnails = () => invoke<number>("image_make_thumbnails");

// ---------- 提醒 ----------
export const setReminder = (noteId: string, remindAt: string, repeatType: RepeatType) =>
  invoke<Reminder>("set_reminder", { noteId, remindAt, repeatType });
export const cancelReminder = (reminderId: string) => invoke<void>("cancel_reminder", { reminderId });
export const completeReminder = (reminderId: string) =>
  invoke<void>("complete_reminder", { reminderId });
export const snoozeReminder = (reminderId: string, remindAt: string) =>
  invoke<Reminder>("snooze_reminder", { reminderId, remindAt });
export const listReminders = (noteId: string, limit?: number) =>
  invoke<Reminder[]>("list_reminders", { noteId, limit: limit ?? 20 });
/** 中文自然语言时间解析（"明天下午3点"等）；无法识别返回 null */
export const parseTimeNl = (input: string) =>
  invoke<NlParsed | null>("parse_time_nl", { input });

// ---------- 待办视图 / 每日笔记 ----------
/** 逾期 / 今天 / 未来7天 三段聚合视图 */
export const getDueView = () => invoke<DueViewData>("get_due_view");
/** 找到/创建当天便签并打开窗口 */
export const getDailyNote = () => invoke<Note>("daily_get_or_create");

// ---------- 番茄钟 ----------
/** 开始专注；noteId/taskKey/taskText 为 null 表示不绑定任务的纯专注 */
export const pomodoroStart = (
  noteId: string | null,
  taskKey: string | null,
  taskText: string | null,
) => invoke<StatePayload>("pomodoro_start", { noteId, taskKey, taskText });

export const pomodoroPause = () => invoke<StatePayload>("pomodoro_pause");
export const pomodoroResume = () => invoke<StatePayload>("pomodoro_resume");
export const pomodoroSkip = () => invoke<StatePayload>("pomodoro_skip");
/** reason 例："manual" | "completed" */
export const pomodoroStop = (reason: string) =>
  invoke<StatePayload>("pomodoro_stop", { reason });
export const pomodoroAddMinutes = (minutes: number) =>
  invoke<StatePayload>("pomodoro_add_minutes", { minutes });
/** 完成绑定任务：回写正文 `- [x]` + 记 meta + 结束当前专注 */
export const pomodoroCompleteTask = (noteId: string, taskKey: string, lineText: string) =>
  invoke<StatePayload>("pomodoro_complete_task", { noteId, taskKey, lineText });

/** 当前状态；空闲时为 null */
export const pomodoroState = async (): Promise<StatePayload | null> => {
  const snap = await invoke<StatePayload>("pomodoro_state");
  return snap.running ? snap : null;
};

export const pomodoroStatsToday = () => invoke<StatsToday>("pomodoro_stats_today");
export const pomodoroStatsRange = (days: number) =>
  invoke<DailyStat[]>("pomodoro_stats_range", { days });
/** 复盘报表（周/月窗口，本地日闭区间 YYYY-MM-DD） */
export const pomodoroStatsReport = (startDate: string, endDate: string) =>
  invoke<FocusReport>("pomodoro_stats_report", { startDate, endDate });
/** 某本地日的实际专注会话（时间轴"实际专注块"） */
export const pomodoroSessionsByDate = (date: string) =>
  invoke<PomodoroSession[]>("pomodoro_sessions_by_date", { date });

export const taskMetaGet = (noteId: string) => invoke<TaskMeta[]>("task_meta_list", { noteId });

export interface TaskMetaUpdateArgs {
  noteId: string;
  taskKey: string;
  lineText: string;
  status?: "todo" | "done" | "skipped";
  estimate?: number | null;
  priority?: string | null; // high | medium | low
  /** RFC3339 UTC */
  dueAt?: string | null;
  /** 本任务专注时长覆盖（分钟；null = 恢复全局默认） */
  focusMin?: number | null;
  /** 清除跳过标记（恢复为 todo） */
  clearSkip?: boolean;
}

export const taskMetaUpdate = (args: TaskMetaUpdateArgs) =>
  invoke<TaskMeta>("task_meta_update", { ...args });

/** 待办拖动排序：keys 顺序即展示顺序（只写 task_meta.sort_order，不动正文） */
export const taskMetaReorder = (noteId: string, keys: string[]) =>
  invoke<number>("task_meta_reorder", { noteId, keys });

/** 迷你番茄窗关闭占位。契约审计修复：Rust 端从未注册 pomodoro_mini_hide 命令，
 *  原 invoke 每次都会 reject（调用方 .catch 静默吞掉），迷你窗实际自行随状态关闭。
 *  features/pomodoro/api.ts 转发层依赖此导出名，故保留为显式 no-op，不再调用未注册命令。 */
export const pomodoroMiniHide = async (): Promise<void> => {};

// ---------- 快速捕获 / 剪贴板 ----------
export const quickCaptureToggle = () => invoke<void>("quickcapture_toggle");
export const quickCaptureHide = () => invoke<void>("quickcapture_hide");
export const quickCaptureReady = () => invoke<void>("quickcapture_ready");

export const clipboardAdd = (content: string, kind: string) =>
  invoke<ClipboardEntry>("clipboard_add", { content, kind });
export const clipboardList = (limit: number) =>
  invoke<ClipboardEntry[]>("clipboard_list", { limit });
export const clipboardRemove = (id: string) => invoke<void>("clipboard_remove", { id });
/** 清空全部历史，返回删除条数 */
export const clipboardClear = () => invoke<number>("clipboard_clear");
/** 切换固定状态，返回切换后的条目 */
export const clipboardPin = (id: string) => invoke<ClipboardEntry>("clipboard_pin", { id });

// ---------- 布局预设 ----------
export const layoutSavePreset = (name: string) =>
  invoke<LayoutPreset>("layout_save_preset", { name });
export const layoutListPresets = () => invoke<LayoutPreset[]>("layout_list_presets");
export const layoutApplyPreset = (id: string) => invoke<number>("layout_apply_preset", { id });
export const layoutDeletePreset = (id: string) => invoke<void>("layout_delete_preset", { id });
/** cols 传 undefined/null 表示自适应（后端 Option<usize> = None） */
export const layoutArrangeGrid = (cols?: number) =>
  invoke<number>("layout_arrange_grid", { cols: cols ?? null });

// ---------- 设置 / 快捷键 ----------
export const getAllSettings = () => invoke<Record<string, string>>("get_all_settings");
export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });
export const getShortcuts = () => invoke<ShortcutEntry[]>("get_shortcuts");
export const setShortcut = (action: ShortcutAction, accelerator: string) =>
  invoke<void>("set_shortcut", { action, accelerator });
export const resetShortcuts = () => invoke<void>("reset_shortcuts");

// ---------- 数据 / 诊断 ----------
export const exportData = (path: string) => invoke<void>("export_data", { path });
export const importData = (path: string) => invoke<ImportSummary>("import_data", { path });
export const getAppInfo = () => invoke<AppInfo>("get_app_info");
/** 导出诊断包：环境/数据库健康/日志尾部（不含便签内容，不含私密数据） */
export const exportDiagnostics = (path: string) =>
  invoke<void>("export_diagnostics", { path });

// ---------- 窗口 / 系统 ----------
export const showAllNotes = () => invoke<void>("show_all_notes");
export const hideAllNotes = () => invoke<void>("hide_all_notes");
export const toggleAllNotes = () => invoke<void>("toggle_all_notes");
export const showManager = () => invoke<void>("show_manager");
export const hideManager = () => invoke<void>("hide_manager");
export const quitApp = () => invoke<void>("quit_app");
export const noteWindowReady = (noteId: string) => invoke<void>("note_window_ready", { noteId });
export const closeNoteWindow = (noteId: string) => invoke<void>("close_note_window", { noteId });
export const openExternal = (url: string) => invoke<void>("open_external", { url });
export const revealDataDir = () => invoke<void>("reveal_data_dir");

// ---------- 今日任务时间轴 ----------
export interface DailyTaskInput {
  date: string;
  startTime: string | null;
  endTime: string | null;
  title: string;
  note: string | null;
  estimatePomodoros: number;
  priority: string;
  repeatRule: string;
  tags: string | null;
  isPrivate: boolean;
  /** 本任务专注时长（分钟；null = 全局默认 25） */
  focusMin: number | null;
}
export const dailyTaskCreate = (input: DailyTaskInput) =>
  invoke<DailyTask>("daily_task_create", { ...input });
export const dailyTaskUpdate = (id: string, input: DailyTaskInput) =>
  invoke<DailyTask>("daily_task_update", { id, ...input });
export const dailyTaskList = (date: string) => invoke<DailyTask[]>("daily_task_list", { date });
export const dailyTaskSetStatus = (id: string, status: string) =>
  invoke<DailyTask>("daily_task_set_status", { id, status });
export const dailyTaskSetTime = (id: string, startTime: string | null, endTime: string | null) =>
  invoke<DailyTask>("daily_task_set_time", { id, startTime, endTime });
export const dailyTaskDelete = (id: string) => invoke<void>("daily_task_delete", { id });
export const dailyTaskStats = (date: string) =>
  invoke<DailyTaskStats>("daily_task_stats", { date });
/** 任务转便签：返回新建便签（不自动开窗，由调用方决定） */
export const dailyTaskToNote = (id: string) => invoke<Note>("daily_task_to_note", { id });
export const dailyTaskSearch = (query: string, status?: string, date?: string) =>
  invoke<DailyTask[]>("daily_task_search", { query, status, date });
