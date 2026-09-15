/** 回收站与版本历史 invoke 封装 — 后端命令由 Rust 侧提供，失败时调用方优雅降级 */
import { invoke } from "@tauri-apps/api/core";
import type { Note } from "@/types";

/** 回收站条目（与 Rust serde camelCase 输出对应） */
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

/** 版本保存来源 */
export type VersionSource = "auto" | "manual" | "pre-restore";

/** 便签历史版本 */
export interface NoteVersion {
  id: string;
  noteId: string;
  title: string;
  content: string;
  source: VersionSource;
  /** RFC3339(UTC) 保存时间 */
  createdAt: string;
}

// ---------- 回收站 ----------
export const listDeletedNotes = () => invoke<TrashItem[]>("list_deleted_notes");
export const restoreFromTrash = (noteId: string) =>
  invoke<void>("restore_from_trash", { noteId });
/** 永久删除（调用方需先 confirm）。
 *  契约审计修复：Rust 端无 purge_note 命令，永久删除复用已注册的 delete_note
 *  （notes::delete_permanently：关窗 + FTS 清理 + 删除记录与图片文件）。 */
export const purgeNote = (noteId: string) => invoke<void>("delete_note", { noteId });
export const emptyTrash = () => invoke<void>("empty_trash");

// ---------- 版本历史 ----------
export const listVersions = (noteId: string, limit = 50) =>
  invoke<NoteVersion[]>("list_versions", { noteId, limit });
/** 恢复到指定版本（后端自动做恢复前备份），返回恢复后的便签 */
export const restoreVersion = (versionId: string) =>
  invoke<Note>("restore_version", { versionId });
