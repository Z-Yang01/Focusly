/** 图片管理 invoke 封装 — 本地类型定义（不改动 src/lib/api.ts 与 src/types）。
 *  后端命令 image_find_duplicates / image_cleanup_orphans / image_make_thumbnails
 *  由总控接线（实现见 src-tauri/src/imagemgr.rs 头注释）；未就绪时调用方降级为
 *  "后端未就绪" 提示。 */
import { invoke } from "@tauri-apps/api/core";

/** 重复图片条目（与 Rust imagemgr::DupEntry serde camelCase 输出对应） */
export interface DupEntry {
  imageId: string;
  noteId: string;
  path: string;
  filename: string;
  size: number;
}

/** 一个重复组：同一 SHA-256 内容哈希对应 ≥2 个文件 */
export interface DupGroup {
  hash: string;
  entries: DupEntry[];
}

/** 按文件内容 SHA-256 找重复图片（仅返回 ≥2 条的组） */
export const imageFindDuplicates = () => invoke<DupGroup[]>("image_find_duplicates");

/** 清理 images/ 下未被数据库引用的孤儿文件，返回删除数量 */
export const imageCleanupOrphans = () => invoke<number>("image_cleanup_orphans");

/** 生成缩略图（thumbs/<imageId>.jpg，最长边 320，JPEG q80），返回生成数量 */
export const imageMakeThumbnails = () => invoke<number>("image_make_thumbnails");
