/** 任务行到期语法（^YYYY-MM-DD）与优先级标记（!高/!中/!低）的纯函数工具。
 *
 *  与 features/todo/todo.ts（GFM 任务行解析，只读）配套：
 *  todo.ts 的 TodoItem.text 即"复选框之后的文本"，本模块既接受裸文本，
 *  也接受带 `- [ ] / * [x]` 前缀的完整任务行（前缀会被原样保留）。
 *  约定：到期/优先级标记放在任务文本行尾，如 `- [ ] 交报告 ^2026-09-20 !高`。
 */

export type TaskPriority = "高" | "中" | "低";

export interface ParsedTaskLine {
  /** 去掉复选框前缀、去掉到期/优先级标记后的任务文本 */
  text: string;
  /** 到期日 YYYY-MM-DD（行内多个合法标记时取最后一个） */
  due?: string;
  priority?: TaskPriority;
}

/** GFM 任务行前缀：可选缩进 + 减号/星号/加号 + [x]/[X]/[ ] + 空格 */
const CHECKBOX_PREFIX_RE = /^[ \t]*[-*+] \[[xX ]\] /;

/** ^YYYY-MM-DD 标记（全局，用于逐个校验与摘除） */
const DUE_TOKEN_RE = /\^(\d{4})-(\d{2})-(\d{2})/g;

/** 独立的 !高/!中/!低 标记（前后须为空白或行边界） */
const PRIORITY_TOKEN_RE = /(?:^|\s)!(高|中|低)(?=\s|$)/g;

/** 校验 YYYY-MM-DD 是否为真实存在的日历日期 */
export function isValidDueDate(s: string): boolean {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  if (!m) return false;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const d = Number(m[3]);
  if (mo < 1 || mo > 12 || d < 1 || d > 31) return false;
  const dt = new Date(Date.UTC(y, mo - 1, d));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === mo - 1 && dt.getUTCDate() === d;
}

/** 从后往前摘除标记区间；两侧都有空格时吃掉一个，避免留下双空格 */
function removeRanges(body: string, ranges: { start: number; end: number }[]): string {
  let out = body;
  for (let i = ranges.length - 1; i >= 0; i -= 1) {
    const head = out.slice(0, ranges[i].start);
    let tail = out.slice(ranges[i].end);
    if (head.endsWith(" ") && tail.startsWith(" ")) tail = tail.slice(1);
    out = head + tail;
  }
  return out;
}

/** 摘除文本中所有合法的 ^日期 标记，返回 [剩余文本, 最后一个合法日期] */
function stripDueTokens(body: string): [string, string | undefined] {
  let due: string | undefined;
  const valid: { start: number; end: number }[] = [];
  for (const m of body.matchAll(DUE_TOKEN_RE)) {
    const cand = `${m[1]}-${m[2]}-${m[3]}`;
    if (isValidDueDate(cand)) {
      due = cand;
      valid.push({ start: m.index ?? 0, end: (m.index ?? 0) + m[0].length });
    }
  }
  return [removeRanges(body, valid), due];
}

/** 摘除文本中所有 !高/!中/!低 标记，返回 [剩余文本, 最后一个优先级] */
function stripPriorityTokens(body: string): [string, TaskPriority | undefined] {
  let priority: TaskPriority | undefined;
  const valid: { start: number; end: number }[] = [];
  for (const m of body.matchAll(PRIORITY_TOKEN_RE)) {
    priority = m[1] as TaskPriority;
    valid.push({ start: m.index ?? 0, end: (m.index ?? 0) + m[0].length });
  }
  return [removeRanges(body, valid), priority];
}

/** 解析任务行（完整行或复选框后的文本均可） */
export function parseTaskLine(line: string): ParsedTaskLine {
  const prefix = line.match(CHECKBOX_PREFIX_RE)?.[0] ?? "";
  const body = line.slice(prefix.length);
  const [afterDue, due] = stripDueTokens(body);
  const [afterPriority, priority] = stripPriorityTokens(afterDue);
  const text = afterPriority.trim();
  return due || priority ? { text, due, priority } : { text };
}

/** 拆出复选框前缀与文本体 */
function splitPrefix(line: string): [string, string] {
  const prefix = line.match(CHECKBOX_PREFIX_RE)?.[0] ?? "";
  return [prefix, line.slice(prefix.length)];
}

/** 设置/移除任务行的到期日（行尾 ^YYYY-MM-DD；due 为空表示移除）。
 *  非法日期串直接忽略并移除旧标记。保留复选框前缀与优先级标记。 */
export function setTaskDue(line: string, due?: string): string {
  const [prefix, body] = splitPrefix(line);
  const [stripped] = stripDueTokens(body);
  if (!due || !isValidDueDate(due)) return prefix + stripped.trimEnd();
  return `${prefix}${stripped.trimEnd()} ^${due}`;
}

/** 设置/移除任务行的优先级标记（行尾 !高/!中/!低；p 为空表示移除）。
 *  保留复选框前缀与到期标记。 */
export function setTaskPriority(line: string, p?: TaskPriority): string {
  const [prefix, body] = splitPrefix(line);
  const [stripped] = stripPriorityTokens(body);
  if (!p) return prefix + stripped.trimEnd();
  return `${prefix}${stripped.trimEnd()} !${p}`;
}

/** 过滤：到期日落在 [start, end]（YYYY-MM-DD，闭区间，字符串比较）内的未勾选任务 */
export function tasksDueBetween<T extends { text: string; checked: boolean }>(
  todoItems: T[],
  start: string,
  end: string,
): T[] {
  return todoItems.filter((item) => {
    if (item.checked) return false;
    const { due } = parseTaskLine(item.text);
    return due !== undefined && due >= start && due <= end;
  });
}

/** 是否逾期：未勾选且有到期日，且到期日早于 todayStr（YYYY-MM-DD） */
export function isOverdue(task: { text: string; checked: boolean }, todayStr: string): boolean {
  if (task.checked) return false;
  const { due } = parseTaskLine(task.text);
  return due !== undefined && due < todayStr;
}
