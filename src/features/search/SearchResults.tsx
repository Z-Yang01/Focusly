/** 搜索结果列表：结构化条件前端过滤 + 键盘导航辅助（moveActive）+ 高亮 snippet 渲染。
 *  snippet 来自后端 FTS5，仅 <mark> 被保留，其余 <>& 由 toSafeMarkHtml 转义。 */
import { Archive, Info, Loader2, Lock, Pin, Search, SquareCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { EmptyState } from "@/components/ui/empty-state";
import { formatLocal, parseRfc3339 } from "@/lib/format";
import type { SearchQuery } from "@/lib/query-parser";
import { cn } from "@/lib/utils";
import type { SearchHit } from "./api";

// ---------- snippet 安全渲染：保留 <mark>，转义其余 <>& ----------

const MARK_OPEN = String.fromCharCode(0);
const MARK_CLOSE = String.fromCharCode(1);

export function toSafeMarkHtml(snippet: string): string {
  // 占位符用控制字符（正常文本不会出现），避免误伤正文空格
  return snippet
    .replace(/<mark>/gi, MARK_OPEN)
    .replace(/<\/mark>/gi, MARK_CLOSE)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(new RegExp(MARK_OPEN, "g"), "<mark>")
    .replace(new RegExp(MARK_CLOSE, "g"), "</mark>");
}

// ---------- 结构化条件的前端过滤（MVP：SearchHit 已有字段；has:/due: 需后端支持） ----------

export function applyStructuredFilters(hits: SearchHit[], parsed: SearchQuery) {
  let out = hits;
  if (parsed.tags.length > 0) {
    out = out.filter((h) => parsed.tags.every((t) => h.tags.includes(t)));
  }
  for (const cond of parsed.is) {
    if (cond === "todo") out = out.filter((h) => h.todoTotal > 0);
    else if (cond === "archived") out = out.filter((h) => h.status === "archived");
    else if (cond === "private") out = out.filter((h) => h.isPrivate);
    else out = out.filter((h) => h.isPinned);
  }
  const pending: string[] = [];
  if (parsed.has.length > 0) pending.push(parsed.has.map((h) => `has:${h}`).join(" "));
  if (parsed.due) pending.push(`due:${parsed.due}`);
  return { filtered: out, pending };
}

// ---------- 键盘导航：列表内循环移动（ArrowUp/Down 共用） ----------

export function moveActive(current: number, length: number, delta: 1 | -1): number {
  if (length <= 0) return -1;
  const next = current + delta;
  if (next < 0) return length - 1;
  if (next >= length) return 0;
  return next;
}

function updatedAtText(hit: SearchHit): string {
  const d = parseRfc3339(hit.updatedAt);
  return d ? formatLocal(d) : "";
}

function chipsOf(parsed: SearchQuery): { key: string; text: string }[] {
  return [
    ...parsed.tags.map((t) => ({ key: `tag:${t}`, text: `#${t}` })),
    ...parsed.is.map((v) => ({ key: `is:${v}`, text: `is:${v}` })),
    ...parsed.has.map((v) => ({ key: `has:${v}`, text: `has:${v}` })),
    ...(parsed.due ? [{ key: "due", text: `due:${parsed.due}` }] : []),
  ];
}

export interface SearchResultsProps {
  /** 已经过 applyStructuredFilters 过滤的结果 */
  filtered: SearchHit[];
  parsed: SearchQuery;
  /** 需后端支持、暂未生效的结构化条件（如 has:/due:） */
  pending: string[];
  /** 键盘选中项（hover 经 onActiveChange 同步为同一状态 → 高亮一致） */
  active: number;
  /** 搜索请求在途 → 显示 Spinner */
  loading: boolean;
  /** 后端 search_notes_v2 不可用 */
  error: boolean;
  onActiveChange: (index: number) => void;
  onOpen: (hit: SearchHit) => void;
}

export function SearchResults({
  filtered,
  parsed,
  pending,
  active,
  loading,
  error,
  onActiveChange,
  onOpen,
}: SearchResultsProps) {
  const chips = chipsOf(parsed);
  return (
    <div className="max-h-96 overflow-y-auto p-1">
      {chips.length > 0 && (
        <div className="flex flex-wrap items-center gap-1 px-2 pb-1 pt-1.5">
          {chips.map((chip) => (
            <Badge key={chip.key} variant="secondary" className="px-1.5 py-0 text-[10px] font-normal">
              {chip.text}
            </Badge>
          ))}
          {pending.length > 0 && (
            <span className="ml-auto flex items-center gap-1 text-[10px] text-muted-foreground">
              <Info className="size-3" />
              {pending.join(" ")} 需后端支持，暂未过滤
            </span>
          )}
        </div>
      )}
      {error ? (
        <div className="px-3 py-6 text-center text-xs text-muted-foreground">
          搜索后端未就绪（search_notes_v2 不可用），语法补全与保存搜索仍可用
        </div>
      ) : loading ? (
        <div className="flex items-center justify-center gap-1.5 px-3 py-6 text-xs text-muted-foreground">
          <Loader2 className="size-3.5 animate-spin" />
          搜索中…
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={Search}
          title="未找到匹配结果"
          description="试试不同关键词或语法"
          className="px-3 py-6"
        />
      ) : (
        filtered.map((hit, i) => (
          <button
            key={hit.id}
            type="button"
            className={cn(
              "flex w-full items-start gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
              i === active && "bg-accent",
            )}
            onMouseDown={(e) => e.preventDefault()}
            onMouseEnter={() => onActiveChange(i)}
            onClick={() => onOpen(hit)}
          >
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1">
                {hit.isPinned && <Pin className="size-3 shrink-0 text-muted-foreground" />}
                <span className="truncate text-xs font-medium">{hit.title || "无标题"}</span>
                {hit.isPrivate && <Lock className="size-3 shrink-0 text-muted-foreground" />}
                {hit.status === "archived" && (
                  <Archive className="size-3 shrink-0 text-muted-foreground" />
                )}
              </div>
              <div
                className="truncate text-xs text-muted-foreground [&_mark]:bg-transparent [&_mark]:font-semibold [&_mark]:text-foreground"
                // snippet 来自后端，仅 <mark> 被保留，其余 <>& 已转义
                dangerouslySetInnerHTML={{ __html: toSafeMarkHtml(hit.snippet) }}
              />
            </div>
            <div className="flex shrink-0 flex-col items-end gap-1">
              <span className="text-[10px] text-muted-foreground">{updatedAtText(hit)}</span>
              <div className="flex items-center gap-1">
                {hit.todoTotal > 0 && (
                  <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
                    <SquareCheck className="size-3" />
                    {hit.todoDone}/{hit.todoTotal}
                  </span>
                )}
                {hit.tags.slice(0, 2).map((tag) => (
                  <Badge key={tag} variant="secondary" className="px-1 py-0 text-[10px]">
                    {tag}
                  </Badge>
                ))}
              </div>
            </div>
          </button>
        ))
      )}
    </div>
  );
}
