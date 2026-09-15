/** Tauri API 薄封装 — 全项目唯一的 invoke 入口 */
import { invoke } from "@tauri-apps/api/core";
import type {
  AppInfo,
  ImportSummary,
  Note,
  NoteDetail,
  NoteFilter,
  NoteFlag,
  NoteImage,
  NoteSummary,
  RepeatType,
  Reminder,
  ShortcutAction,
  ShortcutEntry,
  TagCount,
  FullscreenBehavior,
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
export const searchNotes = (query: string) => invoke<NoteSummary[]>("search_notes", { query });
export const setNoteTags = (noteId: string, tags: string[]) =>
  invoke<string[]>("set_note_tags", { noteId, tags });
export const getTags = () => invoke<TagCount[]>("get_tags");

// ---------- 图片 ----------
export const addImage = (noteId: string, path: string) =>
  invoke<NoteImage>("add_image", { noteId, path });
export const addImageData = (noteId: string, width: number, height: number, bytes: number[]) =>
  invoke<NoteImage>("add_image_data", { noteId, width, height, bytes });
export const removeImage = (imageId: string) => invoke<void>("remove_image", { imageId });
export const imageExists = (path: string) => invoke<boolean>("image_exists", { path });

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

// ---------- 设置 / 快捷键 ----------
export const getAllSettings = () => invoke<Record<string, string>>("get_all_settings");
export const setSetting = (key: string, value: string) =>
  invoke<void>("set_setting", { key, value });
export const getShortcuts = () => invoke<ShortcutEntry[]>("get_shortcuts");
export const setShortcut = (action: ShortcutAction, accelerator: string) =>
  invoke<void>("set_shortcut", { action, accelerator });
export const resetShortcuts = () => invoke<void>("reset_shortcuts");

// ---------- 数据 ----------
export const exportData = (path: string) => invoke<void>("export_data", { path });
export const importData = (path: string) => invoke<ImportSummary>("import_data", { path });
export const getAppInfo = () => invoke<AppInfo>("get_app_info");

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
