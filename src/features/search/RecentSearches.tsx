/** 最近搜索 + 保存的搜索面板（idle 模式）。
 *  保存搜索后端未就绪时降级 localStorage；存储读写均为容错式（失败静默降级）。 */
import { Bookmark, Clock, X } from "lucide-react";
import type { SavedSearch } from "./api";

const RECENT_KEY = "focusly:recent-searches";
const LOCAL_SAVED_KEY = "focusly:saved-searches";
export const RECENT_MAX = 8;

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeJson(key: string, value: unknown): void {
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* 存储不可用时静默降级 */
  }
}

export function loadRecent(): string[] {
  const list = readJson<unknown[]>(RECENT_KEY, []);
  return Array.isArray(list) ? list.filter((s): s is string => typeof s === "string") : [];
}

export function pushRecent(list: string[], query: string): string[] {
  const next = [query, ...list.filter((q) => q !== query)].slice(0, RECENT_MAX);
  writeJson(RECENT_KEY, next);
  return next;
}

export function removeRecentItem(list: string[], query: string): string[] {
  const next = list.filter((q) => q !== query);
  writeJson(RECENT_KEY, next);
  return next;
}

export function loadLocalSaved(): SavedSearch[] {
  const list = readJson<SavedSearch[]>(LOCAL_SAVED_KEY, []);
  return Array.isArray(list) ? list : [];
}

export function writeLocalSaved(list: SavedSearch[]): void {
  writeJson(LOCAL_SAVED_KEY, list);
}

export function genId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

export interface RecentSearchesProps {
  recent: string[];
  saved: SavedSearch[];
  /** remote = 后端已接管；local = 降级本机存储（面板顶部给出提示） */
  savedSource: "remote" | "local";
  /** 选中某条（最近/保存）→ 回填输入框 */
  onPick: (query: string) => void;
  onRemoveRecent: (query: string) => void;
  onDeleteSaved: (item: SavedSearch) => void;
}

export function RecentSearches({
  recent,
  saved,
  savedSource,
  onPick,
  onRemoveRecent,
  onDeleteSaved,
}: RecentSearchesProps) {
  return (
    <div className="max-h-96 overflow-y-auto p-1">
      {recent.length > 0 && (
        <section>
          <div className="flex items-center gap-1.5 px-2 pb-1 pt-1.5 text-[11px] text-muted-foreground">
            <Clock className="size-3" />
            最近搜索
          </div>
          {recent.map((rq) => (
            <div key={rq} className="group flex items-center">
              <button
                type="button"
                className="min-w-0 flex-1 truncate rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onPick(rq)}
              >
                {rq}
              </button>
              <button
                type="button"
                title="删除该记录"
                aria-label="删除该最近搜索记录"
                className="rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onRemoveRecent(rq)}
              >
                <X className="size-3" />
              </button>
            </div>
          ))}
        </section>
      )}
      <section>
        <div className="flex items-center gap-1.5 px-2 pb-1 pt-1.5 text-[11px] text-muted-foreground">
          <Bookmark className="size-3" />
          保存的搜索
        </div>
        {savedSource === "local" && (
          <p className="px-2 pb-1 text-[11px] text-amber-600">后端未就绪，暂存于本机浏览器</p>
        )}
        {saved.length === 0 ? (
          <p className="px-2 py-1 text-[11px] text-muted-foreground">
            暂无。输入搜索后点击右侧书签图标即可保存。
          </p>
        ) : (
          saved.map((item) => (
            <div key={item.id} className="group flex items-center">
              <button
                type="button"
                className="min-w-0 flex-1 truncate rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onPick(item.query)}
              >
                <span className="font-medium">{item.name}</span>
                <span className="ml-2 text-[11px] text-muted-foreground">{item.query}</span>
              </button>
              <button
                type="button"
                title="删除该保存"
                aria-label={`删除保存的搜索「${item.name}」`}
                className="rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100"
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => onDeleteSaved(item)}
              >
                <X className="size-3" />
              </button>
            </div>
          ))
        )}
      </section>
      <div className="border-t px-2 py-1.5 text-[11px] text-muted-foreground">
        语法：<code>tag:#标签</code> · <code>is:todo</code> · <code>has:image</code> ·{" "}
        <code>due:today</code>，空格分隔，引号支持短语
      </div>
    </div>
  );
}
