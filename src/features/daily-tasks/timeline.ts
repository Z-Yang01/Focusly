/** 今日时间轴纯函数：时间换算、轴范围、行道分箱、吸附。便于单测。 */

/** "HH:MM" → 当日分钟数；非法/空返回 null */
export function hhmmToMinutes(hhmm: string | null | undefined): number | null {
  if (!hhmm) return null;
  const m = /^(\d{2}):(\d{2})$/.exec(hhmm.trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (h > 23 || min > 59) return null;
  return h * 60 + min;
}

/** 当日分钟数 → "HH:MM"（越界取模夹紧到 00:00–23:59） */
export function minutesToHhmm(totalMin: number): string {
  const clamped = Math.max(0, Math.min(23 * 60 + 59, Math.round(totalMin)));
  const h = Math.floor(clamped / 60);
  const m = clamped % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}`;
}

/** 轴范围（分钟）。默认 09:00–21:00；任务超出时外扩并留 30 分钟边距。 */
export function timelineRange(
  startTimes: (number | null)[],
  endTimes: (number | null)[],
  fallbackStart = 9 * 60,
  fallbackEnd = 21 * 60,
): { startMin: number; endMin: number } {
  let startMin = fallbackStart;
  let endMin = fallbackEnd;
  for (const s of startTimes) {
    if (s !== null) startMin = Math.min(startMin, s);
  }
  for (const e of endTimes) {
    if (e !== null) endMin = Math.max(endMin, e);
  }
  if (startMin < fallbackStart) startMin = Math.max(0, startMin - 30);
  if (endMin > fallbackEnd) endMin = Math.min(24 * 60, endMin + 30);
  if (endMin <= startMin) endMin = startMin + 60;
  return { startMin, endMin };
}

/** 分钟 → 轴内纵向像素（pxPerHour 为每小时高度） */
export function minutesToY(min: number, range: { startMin: number }, pxPerHour: number): number {
  return ((min - range.startMin) / 60) * pxPerHour;
}

/** 纵向像素 → 分钟（相对轴起点），吸附 snapMin（默认 15 分钟） */
export function yToSnappedMinutes(
  y: number,
  range: { startMin: number },
  pxPerHour: number,
  snapMin = 15,
): number {
  const raw = range.startMin + (y / pxPerHour) * 60;
  return Math.max(0, Math.round(raw / snapMin) * snapMin);
}

/** 任务块行道分箱：贪心把重叠时间段放进不同 lane（互不重叠的同 lane）。
 *  输入须按 start 升序。返回 id → lane。无时间的任务不参与。 */
export function lanePack(
  items: { id: string; start: number; end: number }[],
): Map<string, number> {
  const laneEnds: number[] = [];
  const result = new Map<string, number>();
  for (const it of items) {
    let lane = laneEnds.findIndex((end) => it.start >= end);
    if (lane === -1) {
      lane = laneEnds.length;
      laneEnds.push(it.end);
    } else {
      laneEnds[lane] = it.end;
    }
    result.set(it.id, lane);
  }
  return result;
}

/** 当前时刻 → 轴内百分比（0..1）；不在范围内返回 null */
export function currentLinePercent(nowMin: number, range: { startMin: number; endMin: number }): number | null {
  if (nowMin < range.startMin || nowMin > range.endMin) return null;
  return (nowMin - range.startMin) / (range.endMin - range.startMin);
}

/** 本地日期 → "YYYY-MM-DD"（与 Rust 侧 daily_task::local_today 同构） */
export function localDateKey(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 日期偏移：YYYY-MM-DD ± n 天 */
export function addDays(dateKey: string, days: number): string {
  const d = new Date(`${dateKey}T00:00:00`);
  d.setDate(d.getDate() + days);
  return localDateKey(d);
}

/** 今天是周几的中文（日期切换显示用） */
export function weekdayLabel(dateKey: string): string {
  const d = new Date(`${dateKey}T00:00:00`);
  if (Number.isNaN(d.getTime())) return "";
  return ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][d.getDay()];
}

/** 搜索 token 解析：识别 is:task / is:today / due:today / status:todo|done|skipped。
 *  含任一任务 token 即返回过滤条件；否则 null（非任务搜索）。 */
export interface TaskQueryTokens {
  /** 去除 token 后的剩余文本（LIKE 用） */
  text: string;
  /** 出现 is:today / due:today 时 = todayKey，否则 null（不限日期） */
  date: string | null;
  /** status:xxx token，否则 null */
  status: string | null;
}
const TASK_TOKEN_RE = /\b(?:is:task|is:today|due:today|status:(?:todo|done|skipped))\b/;
const TASK_TOKEN_STRIP_RE = /\b(?:is:task|is:today|due:today|status:(?:todo|done|skipped))\b/g;
const HAS_TODAY_RE = /\b(?:is:today|due:today)\b/;
const STATUS_RE = /\bstatus:(todo|done|skipped)\b/;

export function parseTaskQuery(raw: string, todayKey: string): TaskQueryTokens | null {
  if (!TASK_TOKEN_RE.test(raw)) return null;
  const hasToday = HAS_TODAY_RE.test(raw);
  const statusMatch = STATUS_RE.exec(raw);
  const text = raw.replace(TASK_TOKEN_STRIP_RE, " ").replace(/\s+/g, " ").trim();
  return { text, date: hasToday ? todayKey : null, status: statusMatch ? statusMatch[1] : null };
}
