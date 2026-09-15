/** 快速捕获的本地 invoke 封装（不经 lib/api.ts：该文件与命令注册由总控统一接线）。
 *  类型在本地定义，与 src/types 契约保持一致；clipboard_* / quickcapture_* 命令
 *  注册前调用会 reject，调用方需优雅降级。 */
import { invoke } from "@tauri-apps/api/core";

/** 与 Rust serde camelCase 输出严格对应的 Note（镜像 src/types#Note） */
export interface Note {
  id: string;
  title: string;
  content: string;
  contentFormat: string;
  status: string;
  isPinned: boolean;
  isAlwaysOnTop: boolean;
  showOnAllDesktops: boolean;
  desktopPinState: string;
  fullscreenBehavior: string;
  x: number | null;
  y: number | null;
  width: number | null;
  height: number | null;
  monitorId: string | null;
  deletedAt: string | null;
  isPrivate: boolean;
  locked: boolean;
  readonly: boolean;
  scale: number | null;
  createdAt: string;
  updatedAt: string;
  archivedAt: string | null;
}

/** 剪贴板历史条目（镜像 src/types#ClipboardEntry / Rust ClipboardEntry） */
export interface ClipboardEntry {
  id: string;
  content: string;
  /** text | image …（kind 列） */
  kind: string;
  /** RFC3339 UTC */
  createdAt: string;
  pinned: boolean;
}

// ---------- 快速捕获窗口（总控注册后可用） ----------
export const quickCaptureToggle = () => invoke<void>("quickcapture_toggle");
export const quickCaptureHide = () => invoke<void>("quickcapture_hide");
export const quickCaptureReady = () => invoke<void>("quickcapture_ready");

// ---------- 速记保存（复用已注册的便签命令） ----------
/** 只建行不开窗，返回空便签 */
export const createNoteRecord = () => invoke<Note>("create_note");
/** 写入标题/正文（Rust 侧同步版本快照 + FTS + notes-changed 事件） */
export const updateNoteContent = (noteId: string, title: string, content: string) =>
  invoke<Note>("update_note_content", { noteId, title, content });
/** 重设标签，返回最终标签列表 */
export const setNoteTags = (noteId: string, tags: string[]) =>
  invoke<string[]>("set_note_tags", { noteId, tags });

// ---------- 剪贴板历史（总控注册后可用） ----------
export const clipboardAdd = (content: string, kind: string) =>
  invoke<ClipboardEntry>("clipboard_add", { content, kind });
export const clipboardList = (limit: number) =>
  invoke<ClipboardEntry[]>("clipboard_list", { limit });
export const clipboardRemove = (id: string) => invoke<void>("clipboard_remove", { id });
/** 清空全部历史，返回删除条数 */
export const clipboardClear = () => invoke<number>("clipboard_clear");
/** 切换固定状态，返回切换后的条目 */
export const clipboardPin = (id: string) => invoke<ClipboardEntry>("clipboard_pin", { id });
