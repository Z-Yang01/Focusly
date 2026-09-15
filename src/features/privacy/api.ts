/** 私密空间 invoke 封装 — 本地类型定义（不改动 src/lib/api.ts 与 src/types）。
 *  后端命令 list_private_notes / set_note_privacy 由总控接线；未就绪时调用方降级为"后端未就绪"空态。 */
import { invoke } from "@tauri-apps/api/core";
import type { Note } from "@/types";

/** 隐私标志（与 Rust notes::set_privacy_flag 的 flag 取值一致） */
export type PrivacyFlag = "private" | "locked" | "readonly";

/**
 * 私密便签列表项。后端 list_private_notes 返回 NoteSummary 摊平 JSON
 * （active、未删除、is_private=1）。列表 UI 仅渲染标题/徽标/更新时间，
 * content 字段即使返回也一律不展示（不显示内容摘要）。
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

/** 设置隐私标志（后端命令 set_note_privacy → notes::set_privacy_flag，flag ∈ private/locked/readonly） */
export const setNotePrivacy = (noteId: string, flag: PrivacyFlag, value: boolean) =>
  invoke<Note>("set_note_privacy", { noteId, flag, value });

/** 私密便签列表（后端命令 list_private_notes：SELECT active 未删 is_private=1 的 NoteSummary） */
export const listPrivateNotes = () => invoke<PrivateNote[]>("list_private_notes");
