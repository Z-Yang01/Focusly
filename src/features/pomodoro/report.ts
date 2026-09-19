/** 复盘报表纯函数：周/月区间生成、图表比例、中断原因文案（全部可单测） */

import { localDateKey } from "./format";

export type ReportMode = "week" | "month";

export interface DateRange {
  /** 本地日期 YYYY-MM-DD（闭区间） */
  start: string;
  end: string;
  /** 展示标签，如 "09-15 ~ 09-21" / "9 月" */
  label: string;
}

function addDays(d: Date, n: number): Date {
  const out = new Date(d);
  out.setDate(out.getDate() + n);
  return out;
}

function fmtShort(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 某偏移量的区间：offset 0 = 本周/本月，-1 = 上周/上月，依此类推。
 *  周为周一至周日；月为自然月。跨年时标签带年份区分。 */
export function reportRange(mode: ReportMode, offset: number, now = new Date()): DateRange {
  const yearOf = (d: Date) => (d.getFullYear() === now.getFullYear() ? "" : `${d.getFullYear()} 年 `);
  if (mode === "week") {
    const dow = (now.getDay() + 6) % 7; // 周一 = 0
    const monday = addDays(now, -dow + offset * 7);
    const sunday = addDays(monday, 6);
    const sameYear = monday.getFullYear() === sunday.getFullYear();
    const label = sameYear
      ? `${yearOf(monday)}${fmtShort(monday)} ~ ${fmtShort(sunday)}`
      : `${fmtShort(monday)} ~ ${fmtShort(sunday)}（跨年）`;
    return {
      start: localDateKey(monday),
      end: localDateKey(sunday),
      label,
    };
  }
  const first = new Date(now.getFullYear(), now.getMonth() + offset, 1);
  const last = new Date(first.getFullYear(), first.getMonth() + 1, 0);
  return {
    start: localDateKey(first),
    end: localDateKey(last),
    label: `${yearOf(first)}${first.getMonth() + 1} 月`,
  };
}

/** 某日期在区间内的序号（0-based）；不在区间内返回 -1 */
export function dayIndexInRange(date: string, range: DateRange): number {
  const toMs = (key: string) => {
    const [y, m, d] = key.split("-").map(Number);
    return Date.UTC(y, m - 1, d);
  };
  const ms = toMs(date);
  if (Number.isNaN(ms)) return -1;
  const a = toMs(range.start);
  const b = toMs(range.end);
  if (Number.isNaN(a) || Number.isNaN(b) || ms < a || ms > b) return -1;
  return Math.round((ms - a) / 86_400_000);
}

/** 条形高度百分比（0..100，一位小数）；max ≤ 0 或非法时为 0 */
export function barPct(value: number, max: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.round((Math.max(0, value) / max) * 1000) / 10;
}

/** 中断原因展示文案 */
export const REASON_LABEL: Record<string, string> = {
  manual: "手动停止",
  skip: "跳过阶段",
  task_done: "任务完成",
  app_exit: "应用退出",
  switch_task: "切换任务",
  other: "其他",
};
