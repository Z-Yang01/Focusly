/** 勿扰时段设置 Tauri invoke 封装（本地 api.ts，避免改动全局 src/lib/api.ts）。
 *  复用既有命令 commands::settings_cmd 的 get_all_settings / set_setting，
 *  因此无需后端新增注册即可工作。启用勿扰时两个键各写 "HH:MM"，
 *  关闭时写空串（Rust 侧读取端容错：缺失/空 = 始终通知，见 src-tauri/src/dnd.rs）。 */
import { invoke } from "@tauri-apps/api/core";

/** settings 表键：勿扰开始时刻（"HH:MM"，空 = 关闭勿扰） */
export const DND_START_KEY = "dnd_start";
/** settings 表键：勿扰结束时刻（"HH:MM"，空 = 关闭勿扰） */
export const DND_END_KEY = "dnd_end";

export const getAllSettings = () => invoke<Record<string, string>>("get_all_settings");
export const setSetting = (key: string, value: string) => invoke<void>("set_setting", { key, value });
