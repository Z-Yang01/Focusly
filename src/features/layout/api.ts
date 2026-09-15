/** 布局预设 Tauri invoke 封装（本地 api.ts，避免改动全局 src/lib/api.ts）。
 *  后端命令由接线方注册（见 src-tauri/src/window/layout.rs 文件头），
 *  未注册时 invoke 会 reject，调用方需降级提示。 */
import { invoke } from "@tauri-apps/api/core";

/** 与 Rust LayoutPreset（serde camelCase）对应 */
export interface LayoutPreset {
  id: string;
  name: string;
  /** 布局快照 JSON：[[noteId,x,y,w,h],...] */
  data: string;
  createdAt: string;
}

export const layoutSavePreset = (name: string) =>
  invoke<LayoutPreset>("layout_save_preset", { name });
export const layoutListPresets = () => invoke<LayoutPreset[]>("layout_list_presets");
export const layoutApplyPreset = (id: string) => invoke<number>("layout_apply_preset", { id });
export const layoutDeletePreset = (id: string) => invoke<void>("layout_delete_preset", { id });
/** cols 传 undefined/null 表示自适应（后端 Option<usize> = None） */
export const layoutArrangeGrid = (cols?: number) =>
  invoke<number>("layout_arrange_grid", { cols: cols ?? null });
