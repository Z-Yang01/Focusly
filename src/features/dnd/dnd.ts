/** 勿扰时段纯逻辑（与 Rust 侧 src-tauri/src/dnd.rs 规则严格一致，纯函数便于单测）：
 *  - "HH:MM" 严格两位（00-23 : 00-59），空串/非法 = 不合法
 *  - 启用 = 起止均合法且不相同（start==end 视为未启用，同 Rust parse_window）
 *  - start > end 即跨午夜窗口（如 22:00-07:00）
 *
 * 设置键名约定见 ./api.ts："dnd_start"/"dnd_end"；缺失/空 = 关闭勿扰
 * （读取端 fail-open，始终通知）。 */

/** 打开勿扰时的默认窗口（晚间 22:00 至次日 07:00） */
export const DEFAULT_START = "22:00";
export const DEFAULT_END = "07:00";

/** 保存成功提示文案（含跨午夜说明） */
export const SAVED_HINT = "勿扰时段已保存（跨午夜支持）";

const HM_RE = /^([01]\d|2[0-3]):([0-5]\d)$/;

/** "HH:MM"（严格两位、分钟/小时越界为假）是否合法 */
export function isValidHm(v: string): boolean {
  return HM_RE.test(v);
}

/** 勿扰是否启用：起止均合法且不相同（start==end 视为未启用） */
export function isDndEnabled(start: string, end: string): boolean {
  return isValidHm(start) && isValidHm(end) && start !== end;
}

/** 是否为跨午夜窗口（如 22:00-07:00）；未启用时恒为 false */
export function isCrossMidnight(start: string, end: string): boolean {
  return isDndEnabled(start, end) && start > end;
}
