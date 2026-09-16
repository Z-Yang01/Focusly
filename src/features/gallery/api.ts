/** 图片管理 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  后端命令 image_find_duplicates / image_cleanup_orphans / image_make_thumbnails
 *  由总控接线（实现见 src-tauri/src/imagemgr.rs 头注释）；未就绪时调用方降级为
 *  "后端未就绪" 提示。 */
export {
  imageCleanupOrphans,
  imageFindDuplicates,
  imageMakeThumbnails,
} from "@/lib/api";
export type { DupEntry, DupGroup } from "@/types";
