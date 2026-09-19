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

/** "HH:MM" → 当日分钟数；非法 → null */
export function hmToMin(v: string): number | null {
  if (!isValidHm(v)) return null;
  const [h, m] = v.split(":").map(Number);
  return h * 60 + m;
}

/** 分钟数 → "HH:MM"（对 24h 取模回绕，支持负数） */
export function minToHm(total: number): string {
  const t = ((total % 1440) + 1440) % 1440;
  return `${String(Math.floor(t / 60)).padStart(2, "0")}:${String(t % 60).padStart(2, "0")}`;
}

/** 把时间拨近 deltaMin 分钟（跨午夜回绕）；起点非法时原样返回 */
export function shiftHm(v: string, deltaMin: number): string {
  const base = hmToMin(v);
  return base === null ? v : minToHm(base + deltaMin);
}

/** 步进粒度（分钟）：点一次 ±15min */
export const STEP_MIN = 15;

/** 勿扰时段窗口（均为 "HH:MM" 字符串；空串 = 关闭） */
export interface DndTimeWindow {
  start: string;
  end: string;
}

/** 步进一格的决策结果：合法窗口，或拒绝原因（直接作错误提示展示） */
export type NudgeResult =
  | { ok: true; window: DndTimeWindow }
  | { ok: false; reason: string };

/** 把指定侧拨近 deltaMin（跨午夜回绕）；拨动后仍需满足启用规则，
 *  起止将相同则拒绝（防一步把勿扰「拨没」）。 */
export function nudgeWindow(
  cur: DndTimeWindow,
  which: "start" | "end",
  deltaMin: number,
): NudgeResult {
  const next: DndTimeWindow = {
    start: which === "start" ? shiftHm(cur.start, deltaMin) : cur.start,
    end: which === "end" ? shiftHm(cur.end, deltaMin) : cur.end,
  };
  if (!isDndEnabled(next.start, next.end)) {
    return { ok: false, reason: "起止不能相同（相同 = 不启用），已停止拨动" };
  }
  return { ok: true, window: next };
}

/** 预设时段（一键套用，替代手输时间）；均满足 isDndEnabled */
export const DND_PRESETS: { label: string; start: string; end: string }[] = [
  { label: "睡前 22:00-07:00", start: "22:00", end: "07:00" },
  { label: "整夜 23:00-06:00", start: "23:00", end: "06:00" },
  { label: "午休 12:00-14:00", start: "12:00", end: "14:00" },
];
