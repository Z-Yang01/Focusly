/** 番茄钟纯函数：倒计时/时长格式化、连续天数、热力图分档（全部可单测） */

/** 秒 → mm:ss；≥1 小时 → h:mm:ss（小时不补零）；负数/非法 → 00:00 */
export function fmtMmss(totalSec: number): string {
  const s = Number.isFinite(totalSec) ? Math.max(0, Math.floor(totalSec)) : 0;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

/** 秒 → "X 小时 Y 分钟"（整点只显示小时；不足 1 小时只显示分钟；负数/非法 → "0 分钟"） */
export function humanizeSec(totalSec: number): string {
  const s = Number.isFinite(totalSec) ? Math.max(0, Math.floor(totalSec)) : 0;
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0 && m > 0) return `${h} 小时 ${m} 分钟`;
  if (h > 0) return `${h} 小时`;
  return `${m} 分钟`;
}

/** Date → 本地日期键 YYYY-MM-DD */
export function localDateKey(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

const DATE_KEY_RE = /^\d{4}-\d{2}-\d{2}$/;

function dateKeyToUtcMs(key: string): number {
  if (!DATE_KEY_RE.test(key)) return Number.NaN;
  const [y, m, d] = key.split("-").map(Number);
  const ms = Date.UTC(y, m - 1, d);
  // 校验回写一致（拒绝 2025-02-31 之类）
  return Number.isNaN(ms) || new Date(ms).toISOString().slice(0, 10) !== key
    ? Number.NaN
    : ms;
}

const DAY_MS = 86_400_000;

/** 连续专注天数：从 today（缺省本地当天）或昨天起向前数连续出现过的日期数。
 *  今天的记录尚未产生时，允许锚定在昨天（当天补上会延续）。 */
export function computeStreak(dates: string[], today: string = localDateKey(new Date())): number {
  const todayMs = dateKeyToUtcMs(today);
  if (Number.isNaN(todayMs)) return 0;

  const active = new Set<number>();
  for (const d of dates) {
    const ms = dateKeyToUtcMs(d);
    if (!Number.isNaN(ms)) active.add(ms);
  }
  if (active.size === 0) return 0;

  const anchor = active.has(todayMs)
    ? todayMs
    : active.has(todayMs - DAY_MS)
      ? todayMs - DAY_MS
      : Number.NaN;
  if (Number.isNaN(anchor)) return 0;

  let streak = 0;
  let cur = anchor;
  while (active.has(cur)) {
    streak += 1;
    cur -= DAY_MS;
  }
  return streak;
}

/** 热力图颜色档位：0 → 0；1-3 → 1；4-7 → 2；≥8 → 3（颜色由浅到深） */
export function heatTier(focusCount: number): 0 | 1 | 2 | 3 {
  if (!Number.isFinite(focusCount) || focusCount <= 0) return 0;
  if (focusCount < 4) return 1;
  if (focusCount < 8) return 2;
  return 3;
}

/** 阶段展示文案 */
export const PHASE_META: Record<string, { emoji: string; label: string }> = {
  focus: { emoji: "🍅", label: "专注" },
  short_break: { emoji: "☕", label: "短休" },
  long_break: { emoji: "🌴", label: "长休" },
};

/** 各阶段默认总时长（秒）：StatePayload 未携带总时长，进度环按此口径推算；
 *  与后端默认节奏一致（专注 25 分钟 / 短休 5 分钟 / 长休 15 分钟）。 */
export const PHASE_TOTAL_SEC: Record<string, number> = {
  focus: 25 * 60,
  short_break: 5 * 60,
  long_break: 15 * 60,
};
