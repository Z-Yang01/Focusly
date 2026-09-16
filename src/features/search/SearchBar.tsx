/** 常驻搜索框：搜索语法解析 + search_notes_v2 + 结构化条件前端过滤
 *  + 最近/保存搜索（后端未就绪时降级本地）+ 语法补全面板 */
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import {
  AlarmClock,
  Archive,
  Bookmark,
  BookmarkPlus,
  CalendarDays,
  Clock,
  Image as ImageIcon,
  Info,
  Lock,
  Pin,
  Search,
  SquareCheck,
  Tag,
  X,
  type LucideIcon,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getTags, openNoteWindow } from "@/lib/api";
import { formatLocal, parseRfc3339 } from "@/lib/format";
import {
  parseSearchQuery,
  tokenizeQuery,
  SEARCH_DUE_VALUES,
  SEARCH_HAS_VALUES,
  SEARCH_IS_VALUES,
  type SearchDueFilter,
  type SearchHasFilter,
  type SearchIsFilter,
} from "@/lib/query-parser";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";
import {
  deleteSavedSearch as deleteSavedSearchRemote,
  listSavedSearches,
  saveSearch as saveSearchRemote,
  searchNotesV2,
  type SavedSearch,
  type SearchHit,
} from "./api";

export interface SearchBarHandle {
  focus: () => void;
}

export interface SearchBarProps {
  className?: string;
}

// ---------- localStorage（最近搜索 / 保存搜索降级存储） ----------

const RECENT_KEY = "focusly:recent-searches";
const LOCAL_SAVED_KEY = "focusly:saved-searches";
const RECENT_MAX = 8;

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

function loadRecent(): string[] {
  const list = readJson<unknown[]>(RECENT_KEY, []);
  return Array.isArray(list) ? list.filter((s): s is string => typeof s === "string") : [];
}

function pushRecent(list: string[], query: string): string[] {
  const next = [query, ...list.filter((q) => q !== query)].slice(0, RECENT_MAX);
  writeJson(RECENT_KEY, next);
  return next;
}

function removeRecentItem(list: string[], query: string): string[] {
  const next = list.filter((q) => q !== query);
  writeJson(RECENT_KEY, next);
  return next;
}

function loadLocalSaved(): SavedSearch[] {
  const list = readJson<SavedSearch[]>(LOCAL_SAVED_KEY, []);
  return Array.isArray(list) ? list : [];
}

function genId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

// ---------- snippet 安全渲染：保留 <mark>，转义其余 <>& ----------

const MARK_OPEN = String.fromCharCode(0);
const MARK_CLOSE = String.fromCharCode(1);

function toSafeMarkHtml(snippet: string): string {
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

// ---------- 语法补全 ----------

const IS_LABELS: Record<SearchIsFilter, string> = {
  todo: "含待办",
  archived: "已归档",
  private: "私密便签",
  pinned: "已置顶",
};
const HAS_LABELS: Record<SearchHasFilter, string> = { image: "含图片", reminder: "含提醒" };
const DUE_LABELS: Record<SearchDueFilter, string> = { today: "今天到期", week: "7 天内到期" };

const IS_ICONS: Record<SearchIsFilter, LucideIcon> = {
  todo: SquareCheck,
  archived: Archive,
  private: Lock,
  pinned: Pin,
};
const HAS_ICONS: Record<SearchHasFilter, LucideIcon> = { image: ImageIcon, reminder: AlarmClock };

interface CompletionOption {
  /** 接受后写入输入框的完整 token */
  token: string;
  label: string;
  hint: string;
  icon: LucideIcon;
}

interface CompletionState {
  key: "tag" | "is" | "has" | "due";
  /** 输入中的最后一个 token 是否已是完整合法值（此时 Enter 直接执行搜索而非补全） */
  complete: boolean;
  options: CompletionOption[];
}

/** 输入框末尾上下文：prefix 为可替换前缀，token 为末尾（未完形的）token */
function trailingContext(q: string): { prefix: string; token: string } {
  if (/\s$/.test(q)) return { prefix: q, token: "" };
  const tokens = tokenizeQuery(q);
  const token = tokens.length > 0 ? tokens[tokens.length - 1] : "";
  return { prefix: q.slice(0, q.length - token.length), token };
}

function buildCompletion(q: string, knownTags: string[]): CompletionState | null {
  const { token } = trailingContext(q);
  const match = /^(tag|is|has|due):(.*)$/i.exec(token);
  if (!match) return null;
  const key = match[1].toLowerCase() as CompletionState["key"];
  const typed = match[2].replace(/"/g, "");

  if (key === "tag") {
    const t = typed.replace(/^#+/, "").toLowerCase();
    const options: CompletionOption[] = knownTags
      .filter((name) => name.toLowerCase().startsWith(t))
      .slice(0, 8)
      .map((name) => ({
        token: `tag:${/\s/.test(name) ? `"#${name}"` : `#${name}`}`,
        label: `#${name}`,
        hint: "按标签过滤（多个 tag 为 AND）",
        icon: Tag,
      }));
    return {
      key,
      complete: t.length > 0 && knownTags.some((n) => n.toLowerCase() === t),
      options,
    };
  }

  if (key === "is") {
    const t = typed.toLowerCase();
    return {
      key,
      complete: (SEARCH_IS_VALUES as readonly string[]).includes(t),
      options: SEARCH_IS_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
        token: `is:${v}`,
        label: `is:${v}`,
        hint: IS_LABELS[v],
        icon: IS_ICONS[v],
      })),
    };
  }

  if (key === "has") {
    const t = typed.toLowerCase();
    return {
      key,
      complete: (SEARCH_HAS_VALUES as readonly string[]).includes(t),
      options: SEARCH_HAS_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
        token: `has:${v}`,
        label: `has:${v}`,
        hint: HAS_LABELS[v],
        icon: HAS_ICONS[v],
      })),
    };
  }

  const t = typed.toLowerCase();
  return {
    key,
    complete: (SEARCH_DUE_VALUES as readonly string[]).includes(t),
    options: SEARCH_DUE_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
      token: `due:${v}`,
      label: `due:${v}`,
      hint: DUE_LABELS[v],
      icon: CalendarDays,
    })),
  };
}

// ---------- 结构化条件的前端过滤（MVP：SearchHit 已有字段；has:/due: 需后端支持） ----------

function applyStructuredFilters(hits: SearchHit[], parsed: ReturnType<typeof parseSearchQuery>) {
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

function updatedAtText(hit: SearchHit): string {
  const d = parseRfc3339(hit.updatedAt);
  return d ? formatLocal(d) : "";
}

// ---------- 组件 ----------

export const SearchBar = forwardRef<SearchBarHandle, SearchBarProps>(function SearchBar(
  { className },
  ref,
) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [searchError, setSearchError] = useState(false);
  const [active, setActive] = useState(-1);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [completionActive, setCompletionActive] = useState(0);
  const [recent, setRecent] = useState<string[]>([]);
  const [saved, setSaved] = useState<SavedSearch[]>([]);
  const [savedSource, setSavedSource] = useState<"remote" | "local">("remote");
  const [saving, setSaving] = useState(false);
  const [saveName, setSaveName] = useState("");
  const [knownTags, setKnownTags] = useState<string[]>([]);
  const tagsFetchedRef = useRef(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useImperativeHandle(ref, () => ({ focus: () => inputRef.current?.focus() }), []);

  const parsed = useMemo(() => parseSearchQuery(q), [q]);

  const completion = useMemo(() => buildCompletion(q, knownTags), [q, knownTags]);

  const { filtered, pending } = useMemo(
    () => applyStructuredFilters(hits, parsed),
    [hits, parsed],
  );

  // 面板模式：空输入 → 最近/保存；语法补全优先于结果
  const mode: "idle" | "completion" | "results" =
    q.trim() === ""
      ? "idle"
      : completion && !completion.complete && completion.options.length > 0
        ? "completion"
        : "results";
  const panelOpen = (focused || saving) && !dismissed;

  // 初始加载最近搜索 + 保存搜索（后端未就绪 → 降级 localStorage）
  useEffect(() => {
    setRecent(loadRecent());
    let alive = true;
    listSavedSearches()
      .then((list) => {
        if (!alive) return;
        setSavedSource("remote");
        setSaved(list);
      })
      .catch((err) => {
        console.error("list_saved_searches 不可用，降级为本地存储", err);
        if (!alive) return;
        setSavedSource("local");
        setSaved(loadLocalSaved());
      });
    return () => {
      alive = false;
    };
  }, []);

  // 防抖搜索：自由文本走 search_notes_v2，includePrivate 固定 false
  useEffect(() => {
    if (!parsed.raw.trim()) {
      setHits([]);
      setSearchError(false);
      setActive(-1);
      return;
    }
    let alive = true;
    const timer = setTimeout(() => {
      searchNotesV2(parsed.freeText, false)
        .then((list) => {
          if (!alive) return;
          setHits(list);
          setSearchError(false);
          setActive(list.length > 0 ? 0 : -1);
        })
        .catch((err) => {
          console.error("search_notes_v2 不可用（后端未就绪）", err);
          if (!alive) return;
          setHits([]);
          setSearchError(true);
          setActive(-1);
        });
    }, 250);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [parsed]);

  // tag: 补全首次展开时拉取标签列表
  useEffect(() => {
    if (completion?.key !== "tag" || tagsFetchedRef.current) return;
    tagsFetchedRef.current = true;
    getTags()
      .then((tags) => setKnownTags(tags.map((t) => t.name)))
      .catch((err) => {
        console.error("get_tags 不可用，标签补全为空", err);
        setKnownTags([]);
      });
  }, [completion?.key]);

  const updateQ = (value: string) => {
    setQ(value);
    setActive(0);
    setDismissed(false);
    setCompletionActive(0);
  };

  const recordRecent = () => {
    const query = parsed.raw.trim();
    if (query) setRecent((list) => pushRecent(list, query));
  };

  const openResult = (hit: SearchHit) => {
    recordRecent();
    openNoteWindow(hit.id).catch((err) => {
      console.error("打开便签失败", err);
      toast.error("打开便签失败，请重试");
    });
    setDismissed(true); // 保留关键词，仅收起面板
  };

  const completionOptions = completion?.options ?? [];
  const completionIdx = Math.min(completionActive, Math.max(0, completionOptions.length - 1));

  const acceptCompletion = (opt: CompletionOption) => {
    const { prefix } = trailingContext(q);
    updateQ(`${prefix}${opt.token} `);
    inputRef.current?.focus();
  };

  const confirmSave = async () => {
    const name = saveName.trim();
    const query = parsed.raw.trim();
    if (!name || !query) return;
    if (savedSource === "remote") {
      try {
        await saveSearchRemote(name, query);
        setSaved(await listSavedSearches());
        setSaving(false);
        setSaveName("");
        return;
      } catch (err) {
        console.error("save_search 不可用，降级为本地存储", err);
        setSavedSource("local");
      }
    }
    const list = [
      ...loadLocalSaved(),
      { id: genId(), name, query, createdAt: new Date().toISOString() },
    ];
    writeJson(LOCAL_SAVED_KEY, list);
    setSaved(list);
    setSaving(false);
    setSaveName("");
  };

  const handleDeleteSaved = (item: SavedSearch) => {
    if (savedSource === "remote") {
      deleteSavedSearchRemote(item.id)
        .then(() => setSaved((list) => list.filter((s) => s.id !== item.id)))
        .catch((err) => console.error("delete_saved_search 失败", err));
    } else {
      const list = loadLocalSaved().filter((s) => s.id !== item.id);
      writeJson(LOCAL_SAVED_KEY, list);
      setSaved(list);
    }
  };

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      setDismissed(true);
      return;
    }
    if (mode === "completion" && completion && completion.options.length > 0) {
      if (e.key === "ArrowDown" || e.key === "Tab") {
        e.preventDefault();
        setCompletionActive((completionIdx + 1) % completion.options.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setCompletionActive(
          (completionIdx - 1 + completion.options.length) % completion.options.length,
        );
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        acceptCompletion(completion.options[completionIdx]);
        return;
      }
    }
    if (mode === "results") {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => (filtered.length > 0 ? (i + 1) % filtered.length : -1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) =>
          filtered.length > 0 ? (i <= 0 ? filtered.length - 1 : i - 1) : -1,
        );
      } else if (e.key === "Enter") {
        recordRecent();
        const idx = active >= 0 && active < filtered.length ? active : 0;
        if (filtered[idx]) openResult(filtered[idx]);
      }
    }
  };

  const parsedChips: { key: string; text: string }[] = [
    ...parsed.tags.map((t) => ({ key: `tag:${t}`, text: `#${t}` })),
    ...parsed.is.map((v) => ({ key: `is:${v}`, text: `is:${v}` })),
    ...parsed.has.map((v) => ({ key: `has:${v}`, text: `has:${v}` })),
    ...(parsed.due ? [{ key: "due", text: `due:${parsed.due}` }] : []),
  ];

  return (
    <div className={cn("relative", className)}>
      <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={inputRef}
        value={q}
        role="searchbox"
        aria-label="搜索便签"
        placeholder="搜索便签…（tag: is: has: due:）"
        className={cn("h-8 pl-8 text-xs", q.trim() !== "" && "pr-8")}
        onChange={(e) => updateQ(e.target.value)}
        onFocus={() => {
          setFocused(true);
          setDismissed(false);
        }}
        onBlur={() => setFocused(false)}
        onKeyDown={handleKeyDown}
      />
      {q.trim() !== "" && (
        <button
          type="button"
          title="保存当前搜索"
          aria-label="保存当前搜索"
          className="absolute right-1.5 top-1/2 -translate-y-1/2 rounded p-1 text-muted-foreground hover:bg-accent hover:text-foreground"
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => {
            setSaveName(parsed.raw.trim());
            setSaving(true);
            setDismissed(false);
          }}
        >
          <BookmarkPlus className="size-3.5" />
        </button>
      )}

      {panelOpen && (
        <div className="absolute left-0 right-0 top-full z-50 mt-1 overflow-hidden rounded-md border bg-popover shadow-md">
          {/* 保存命名行 */}
          {saving && (
            <div className="flex items-center gap-1.5 border-b p-2">
              <Input
                autoFocus
                value={saveName}
                placeholder="保存的搜索名称…"
                className="h-7 text-xs"
                onChange={(e) => setSaveName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void confirmSave();
                  if (e.key === "Escape") setSaving(false);
                }}
              />
              <Button
                type="button"
                size="sm"
                className="h-7 px-2 text-xs"
                disabled={!saveName.trim()}
                onClick={() => void confirmSave()}
              >
                保存
              </Button>
              <Button
                type="button"
                size="sm"
                variant="ghost"
                className="h-7 px-2 text-xs"
                onClick={() => setSaving(false)}
              >
                取消
              </Button>
            </div>
          )}

          {/* 空输入：最近搜索 + 保存的搜索 */}
          {mode === "idle" && (
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
                        onClick={() => updateQ(rq)}
                      >
                        {rq}
                      </button>
                      <button
                        type="button"
                        title="删除该记录"
                        aria-label="删除该最近搜索记录"
                        className="rounded p-1 text-muted-foreground opacity-0 transition-opacity hover:text-foreground group-hover:opacity-100"
                        onMouseDown={(e) => e.preventDefault()}
                        onClick={() => setRecent((list) => removeRecentItem(list, rq))}
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
                  <p className="px-2 pb-1 text-[11px] text-amber-600">
                    后端未就绪，暂存于本机浏览器
                  </p>
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
                        onClick={() => updateQ(item.query)}
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
                        onClick={() => handleDeleteSaved(item)}
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
          )}

          {/* 语法补全 */}
          {mode === "completion" && completion && (
            <div className="max-h-80 overflow-y-auto p-1">
              <div className="px-2 pb-1 pt-1.5 text-[11px] text-muted-foreground">
                {completion.key}: 合法值
              </div>
              {completion.options.length === 0 ? (
                <div className="px-3 py-4 text-center text-xs text-muted-foreground">
                  暂无可补全项
                </div>
              ) : (
                completion.options.map((opt, i) => (
                  <button
                    key={opt.token}
                    type="button"
                    className={cn(
                      "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
                      i === completionIdx && "bg-accent",
                    )}
                    onMouseDown={(e) => e.preventDefault()}
                    onMouseEnter={() => setCompletionActive(i)}
                    onClick={() => acceptCompletion(opt)}
                  >
                    <opt.icon className="size-3.5 shrink-0 text-muted-foreground" />
                    <span className="shrink-0 text-xs font-medium">{opt.label}</span>
                    <span className="ml-auto truncate text-[11px] text-muted-foreground">
                      {opt.hint}
                    </span>
                  </button>
                ))
              )}
            </div>
          )}

          {/* 搜索结果 */}
          {mode === "results" && (
            <div className="max-h-96 overflow-y-auto p-1">
              {parsedChips.length > 0 && (
                <div className="flex flex-wrap items-center gap-1 px-2 pb-1 pt-1.5">
                  {parsedChips.map((chip) => (
                    <Badge
                      key={chip.key}
                      variant="secondary"
                      className="px-1.5 py-0 text-[10px] font-normal"
                    >
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
              {searchError ? (
                <div className="px-3 py-6 text-center text-xs text-muted-foreground">
                  搜索后端未就绪（search_notes_v2 不可用），语法补全与保存搜索仍可用
                </div>
              ) : filtered.length === 0 ? (
                <div className="px-3 py-6 text-center text-xs text-muted-foreground">
                  没有匹配的便签
                </div>
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
                    onMouseEnter={() => setActive(i)}
                    onClick={() => openResult(hit)}
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
                      <span className="text-[10px] text-muted-foreground">
                        {updatedAtText(hit)}
                      </span>
                      <div className="flex items-center gap-1">
                        {hit.todoTotal > 0 && (
                          <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
                            <SquareCheck className="size-3" />
                            {hit.todoDone}/{hit.todoTotal}
                          </span>
                        )}
                        {hit.tags.slice(0, 2).map((tag) => (
                          <Badge
                            key={tag}
                            variant="secondary"
                            className="px-1 py-0 text-[10px]"
                          >
                            {tag}
                          </Badge>
                        ))}
                      </div>
                    </div>
                  </button>
                ))
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
});
