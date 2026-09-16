/** 快速捕获 / 剪贴板 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  clipboard_* / quickcapture_* 命令注册前调用会 reject，调用方需优雅降级。 */
export {
  clipboardAdd,
  clipboardClear,
  clipboardList,
  clipboardPin,
  clipboardRemove,
  /** 只建行不开窗，返回空便签（= create_note，统一复用 lib/api.ts 的 createNote） */
  createNote as createNoteRecord,
  quickCaptureHide,
  quickCaptureReady,
  quickCaptureToggle,
  setNoteTags,
  /** 写入标题/正文（Rust 侧同步版本快照 + FTS + notes-changed 事件） */
  updateNoteContent,
} from "@/lib/api";
export type { ClipboardEntry } from "@/types";
