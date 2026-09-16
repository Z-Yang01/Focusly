/** 回收站与版本历史 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  失败时调用方优雅降级。 */
export {
  /** 永久删除（调用方需先 confirm）。
   *  契约审计修复：Rust 端无 purge_note 命令，永久删除复用已注册的 delete_note
   *  （notes::delete_permanently：关窗 + FTS 清理 + 删除记录与图片文件）。 */
  purgeNote,
  emptyTrash,
  listDeletedNotes,
  listVersions,
  restoreFromTrash,
  restoreVersion,
} from "@/lib/api";
export type { NoteVersion, TrashItem, VersionSource } from "@/types";
