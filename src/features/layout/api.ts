/** 布局预设 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  后端命令由接线方注册（见 src-tauri/src/window/layout.rs 文件头），
 *  未注册时 invoke 会 reject，调用方需降级提示。 */
export {
  layoutApplyPreset,
  layoutArrangeGrid,
  layoutDeletePreset,
  layoutListPresets,
  layoutSavePreset,
} from "@/lib/api";
export type { LayoutPreset } from "@/types";
