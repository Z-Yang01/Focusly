/** 搜索语法解析（纯函数）：tag:/is:/has:/due: 结构化条件 + 自由文本，支持引号短语 */

/** is: 的合法值 */
export const SEARCH_IS_VALUES = ["todo", "archived", "private", "pinned"] as const;
/** has: 的合法值 */
export const SEARCH_HAS_VALUES = ["image", "reminder"] as const;
/** due: 的合法值 */
export const SEARCH_DUE_VALUES = ["today", "week"] as const;

export type SearchIsFilter = (typeof SEARCH_IS_VALUES)[number];
export type SearchHasFilter = (typeof SEARCH_HAS_VALUES)[number];
export type SearchDueFilter = (typeof SEARCH_DUE_VALUES)[number];

/** 解析结果：结构化条件已归位，其余 token 合并进 freeText */
export interface SearchQuery {
  /** 自由文本（引号已剥除、多余空白归一、单空格连接） */
  freeText: string;
  /** tag: 值（已剥 # 前缀，去重，保序） */
  tags: string[];
  /** is: 条件（去重，保序） */
  is: SearchIsFilter[];
  /** has: 条件（去重，保序） */
  has: SearchHasFilter[];
  /** due: 条件（多次出现取最后一个） */
  due?: SearchDueFilter;
  /** 原始输入（原样保留） */
  raw: string;
}

/** 结构化 token 形如 key:value，key 不区分大小写 */
const STRUCTURED_RE = /^(tag|is|has|due):(.*)$/i;
const WHITESPACE_RE = /\s/;

/**
 * 分词：token 间以空白分隔；引号内的空白不分割（引号字符保留在 token 里）。
 * 未闭合的引号容错：剩余内容作为同一个 token。
 */
export function tokenizeQuery(input: string): string[] {
  const tokens: string[] = [];
  let current = "";
  let inQuote = false;
  for (const ch of input) {
    if (ch === '"') {
      inQuote = !inQuote;
      current += ch;
    } else if (!inQuote && WHITESPACE_RE.test(ch)) {
      if (current) {
        tokens.push(current);
        current = "";
      }
    } else {
      current += ch;
    }
  }
  if (current) tokens.push(current);
  return tokens;
}

/** 剥除成对的首尾引号；引号不成对则原样返回 */
function stripQuotes(value: string): string {
  return value.length >= 2 && value.startsWith('"') && value.endsWith('"')
    ? value.slice(1, -1)
    : value;
}

/** tag 值：剥引号 + 剥 # 前缀 */
function parseTagValue(rawValue: string): string {
  return stripQuotes(rawValue).replace(/^#+/, "").trim();
}

function pushUnique<T>(list: T[], value: T): void {
  if (!list.includes(value)) list.push(value);
}

/**
 * 解析搜索输入。
 * 规则：
 * - `tag:#工作` / `tag:学习`（# 可省略）→ tags（AND 语义，由调用方解释）
 * - `is:todo|archived|private|pinned` → is
 * - `has:image|reminder` → has
 * - `due:today|week` → due
 * - 已知 key + 未知值（如 `is:done`）→ 整个 token 原样归入自由文本，避免静默丢词
 * - 其余 token（含引号短语）→ 自由文本；引号内结构化语法（如 `"tag:工作"`）不解析
 */
export function parseSearchQuery(input: string): SearchQuery {
  const tags: string[] = [];
  const is: SearchIsFilter[] = [];
  const has: SearchHasFilter[] = [];
  let due: SearchDueFilter | undefined;
  const freeTokens: string[] = [];

  for (const token of tokenizeQuery(input)) {
    const match = STRUCTURED_RE.exec(token);
    if (!match) {
      const text = stripQuotes(token);
      if (text) freeTokens.push(text);
      continue;
    }
    const key = match[1].toLowerCase();
    const value = stripQuotes(match[2]);
    if (key === "tag") {
      const tag = parseTagValue(match[2]);
      if (tag) pushUnique(tags, tag);
    } else if (key === "is") {
      const v = value.toLowerCase();
      if ((SEARCH_IS_VALUES as readonly string[]).includes(v)) {
        pushUnique(is, v as SearchIsFilter);
      } else {
        freeTokens.push(token);
      }
    } else if (key === "has") {
      const v = value.toLowerCase();
      if ((SEARCH_HAS_VALUES as readonly string[]).includes(v)) {
        pushUnique(has, v as SearchHasFilter);
      } else {
        freeTokens.push(token);
      }
    } else {
      const v = value.toLowerCase();
      if ((SEARCH_DUE_VALUES as readonly string[]).includes(v)) {
        due = v as SearchDueFilter;
      } else {
        freeTokens.push(token);
      }
    }
  }

  return {
    freeText: freeTokens.join(" "),
    tags,
    is,
    has,
    due,
    raw: input,
  };
}

/** 查询是否不含任何条件（空查询） */
export function isQueryEmpty(q: SearchQuery): boolean {
  return (
    q.freeText === "" && q.tags.length === 0 && q.is.length === 0 && q.has.length === 0 && !q.due
  );
}

/** 含空白则加引号，保证反向序列化后可被 parseSearchQuery 还原 */
function quoteIfNeeded(value: string): string {
  return WHITESPACE_RE.test(value) ? `"${value}"` : value;
}

/**
 * 反向序列化（保存搜索用）：结构化条件在前、自由文本在后。
 * formatQuery(parseSearchQuery(input)) 再 parse 回来，除 raw 外字段一致。
 */
export function formatQuery(q: SearchQuery): string {
  const parts: string[] = [];
  for (const tag of q.tags) parts.push(`tag:${quoteIfNeeded(`#${tag}`)}`);
  for (const v of q.is) parts.push(`is:${v}`);
  for (const v of q.has) parts.push(`has:${v}`);
  if (q.due) parts.push(`due:${q.due}`);
  if (q.freeText) parts.push(quoteIfNeeded(q.freeText));
  return parts.join(" ");
}
