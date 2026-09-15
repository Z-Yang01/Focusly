/** 搜索 v2 / 保存搜索 —— 本 feature 专用 invoke 封装与类型（与 Rust serde camelCase 输出对应） */
import { invoke } from "@tauri-apps/api/core";

/** search_notes_v2 命中项；snippet 由后端生成，含 <mark>高亮</mark> */
export interface SearchHit {
  id: string;
  title: string;
  snippet: string;
  tags: string[];
  todoTotal: number;
  todoDone: number;
  /** RFC3339 (UTC) */
  updatedAt: string;
  isPinned: boolean;
  isPrivate: boolean;
  status: "active" | "archived";
}

/** 保存的搜索（save_search / list_saved_searches） */
export interface SavedSearch {
  id: string;
  name: string;
  query: string;
  /** RFC3339 (UTC) */
  createdAt: string;
}

/**
 * 全文搜索 v2。includePrivate 固定传 false（私密空间功能后续接入）。
 * 后端命令未接线时会 reject —— 调用方必须 catch 并降级（显示"后端未就绪"）。
 */
export const searchNotesV2 = (query: string, includePrivate = false) =>
  invoke<SearchHit[]>("search_notes_v2", { query, includePrivate });

export const listSavedSearches = () => invoke<SavedSearch[]>("list_saved_searches");

export const saveSearch = (name: string, query: string) =>
  invoke<void>("save_search", { name, query });

export const deleteSavedSearch = (id: string) => invoke<void>("delete_saved_search", { id });
