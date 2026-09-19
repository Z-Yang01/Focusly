/** Tauri 运行时工具：当前窗口、事件监听、资源 URL */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import type {
  PomodoroFinishedEvent,
  ReminderFiredEvent,
  SettingsChangedEvent,
  ShortcutErrorEvent,
  StatePayload,
  TaskMetaChangedEvent,
} from "@/types";
import { EVENTS, POMODORO_EVENTS } from "@/types";

/** 当前窗口角色 */
export type WindowRole =
  | { kind: "manager" }
  | { kind: "note"; noteId: string }
  | { kind: "quick-capture" }
  | { kind: "pomodoro-mini" };

export function getWindowRole(): WindowRole {
  const label = getCurrentWebviewWindow().label;
  if (label === "manager") return { kind: "manager" };
  if (label === "quick-capture") return { kind: "quick-capture" };
  if (label === "pomodoro-mini") return { kind: "pomodoro-mini" };
  const noteId = label.replace(/^note-/, "");
  return noteId ? { kind: "note", noteId } : { kind: "manager" };
}

export { convertFileSrc, listen };
export type { UnlistenFn };

export const onNotesChanged = (cb: () => void) => listen(EVENTS.notesChanged, cb);
export const onSettingsChanged = (cb: (e: SettingsChangedEvent) => void) =>
  listen<SettingsChangedEvent>(EVENTS.settingsChanged, (e) => cb(e.payload));
export const onReminderFired = (cb: (e: ReminderFiredEvent) => void) =>
  listen<ReminderFiredEvent>(EVENTS.reminderFired, (e) => cb(e.payload));
export const onShortcutError = (cb: (e: ShortcutErrorEvent) => void) =>
  listen<ShortcutErrorEvent>(EVENTS.shortcutError, (e) => cb(e.payload));
export const onNotesVisibility = (cb: (visible: boolean) => void) =>
  listen<boolean>(EVENTS.notesVisibility, (e) => cb(e.payload));
export const onFocusSearch = (cb: () => void) => listen(EVENTS.focusSearch, cb);
export const onOpenSettings = (cb: () => void) => listen(EVENTS.openSettings, cb);
export const onAppExitFlush = (cb: () => void) => listen(EVENTS.appExitFlush, cb);

// ---------- 番茄钟事件（原 features/pomodoro/api.ts 内监听，事件监听统一归本文件） ----------
export const onPomodoroState = (cb: (state: StatePayload) => void) =>
  listen<StatePayload>(POMODORO_EVENTS.state, (e) => cb(e.payload));
export const onPomodoroFinished = (cb: (e: PomodoroFinishedEvent) => void) =>
  listen<PomodoroFinishedEvent>(POMODORO_EVENTS.finished, (e) => cb(e.payload));
export const onTaskMetaChanged = (cb: (e: TaskMetaChangedEvent) => void) =>
  listen<TaskMetaChangedEvent>(POMODORO_EVENTS.taskMetaChanged, (e) => cb(e.payload));
