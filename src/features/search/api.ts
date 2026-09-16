/** 搜索 v2 / 保存搜索 — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  searchNotesV2（search_notes_v2，{ query, includePrivate }）与 lib/api.ts 的
 *  searchNotes（search_notes 降级 LIKE 版）不同名不同义，两者并存。 */
export {
  deleteSavedSearch,
  listSavedSearches,
  saveSearch,
  searchNotesV2,
} from "@/lib/api";
export type { SavedSearch, SearchHit } from "@/types";
