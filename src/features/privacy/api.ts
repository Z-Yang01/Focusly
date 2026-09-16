/** 私密空间 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  后端命令 list_private_notes / set_note_privacy 由总控接线；未就绪时调用方降级为"后端未就绪"空态。 */
export { listPrivateNotes, setNotePrivacy } from "@/lib/api";
export type { PrivacyFlag, PrivateNote } from "@/types";
