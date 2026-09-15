/** 时间与文本格式化工具 */

/** RFC3339(UTC) → Date；非法返回 null */
export function parseRfc3339(s: string | null | undefined): Date | null {
  if (!s) return null;
  const d = new Date(s);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Date → 本地 datetime-local input 值 (YYYY-MM-DDTHH:mm) */
export function toDatetimeLocal(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** datetime-local 值 → RFC3339 UTC（给 Rust 侧存储） */
export function fromDatetimeLocal(v: string): string | null {
  const d = new Date(v);
  if (Number.isNaN(d.getTime())) return null;
  return d.toISOString();
}

/** 相对时间描述：3 分钟后 / 昨天 / 9月16日 09:00 */
export function describeTime(s: string | null | undefined): string {
  const d = parseRfc3339(s);
  if (!d) return "";
  const diffMs = d.getTime() - Date.now();
  const diffMin = Math.round(diffMs / 60000);
  if (diffMin > 0 && diffMin < 60) return `${diffMin} 分钟后`;
  if (diffMin >= 60 && diffMin < 24 * 60) {
    const h = Math.floor(diffMin / 60);
    return `${h} 小时后`;
  }
  if (diffMin >= 24 * 60 && diffMin < 7 * 24 * 60) {
    const days = Math.round(diffMin / (24 * 60));
    return `${days} 天后`;
  }
  return formatLocal(d);
}

/** 本地时间 "09-16 09:00" */
export function formatLocal(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const sameYear = d.getFullYear() === new Date().getFullYear();
  const md = `${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const hm = `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  return sameYear ? `${md} ${hm}` : `${d.getFullYear()}-${md} ${hm}`;
}

/** 自动保存状态文案 */
export function savedAtText(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** markdown → 纯文本摘要（用于卡片预览 / 通知 body） */
export function snippetOf(content: string, max = 80): string {
  const text = content
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "[图片]")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/[*_~`>#-]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return text.length > max ? `${text.slice(0, max)}…` : text;
}
