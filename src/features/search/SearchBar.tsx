/** 常驻搜索框（容器组件）：状态管理 + 事件绑定 + 子面板组装。
 *  搜索语法解析 + search_notes_v2 + 结构化条件前端过滤
 *  + 最近/保存搜索（后端未就绪时降级本地）+ 语法补全面板。
 *  子组件：SearchResults（结果列表）/ SyntaxHints（语法补全）/ RecentSearches（最近·保存）。 */
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { BookmarkPlus, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getTags, openNoteWindow } from "@/lib/api";
import { parseSearchQuery } from "@/lib/query-parser";
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
import { RecentSearches, genId, loadLocalSaved, loadRecent, pushRecent, removeRecentItem, writeLocalSaved } from "./RecentSearches";
import { SearchResults, applyStructuredFilters, moveActive } from "./SearchResults";
import { localDateKey, parseTaskQuery } from "@/features/daily-tasks/timeline";
import { dailyTaskSearch } from "@/lib/api";
import type { DailyTask } from "@/types";
import { SyntaxHints, buildCompletion, trailingContext, type CompletionOption } from "./SyntaxHints";

export interface SearchBarHandle {
  focus: () => void;
}

export interface SearchBarProps {
  className?: string;
}

// ---------- 组件 ----------

export const SearchBar = forwardRef<SearchBarHandle, SearchBarProps>(function SearchBar(
  { className },
  ref,
) {
  const [q, setQ] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [taskHits, setTaskHits] = useState<DailyTask[]>([]);
  const [searchError, setSearchError] = useState(false);
  const [searching, setSearching] = useState(false);
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
      setTaskHits([]);
      setSearchError(false);
      setSearching(false);
      setActive(-1);
      return;
    }
    let alive = true;
    const timer = setTimeout(() => {
      setSearching(true);
      searchNotesV2(parsed.freeText, false)
        .then((list) => {
          if (!alive) return;
          setHits(list);
          setSearchError(false);
          setSearching(false);
          setActive(list.length > 0 ? 0 : -1);
        })
        .catch((err) => {
          console.error("search_notes_v2 不可用（后端未就绪）", err);
          if (!alive) return;
          setHits([]);
          setSearchError(true);
          setSearching(false);
          setActive(-1);
        });
    }, 250);
    return () => {
      alive = false;
      setSearching(false);
      clearTimeout(timer);
    };
  }, [parsed]);

  // 任务 token（is:task / is:today / status:… / due:today）→ 并行拉取今日任务
  useEffect(() => {
    const tokens = parseTaskQuery(parsed.raw, localDateKey(new Date()));
    if (!tokens) {
      setTaskHits([]);
      return;
    }
    let alive = true;
    dailyTaskSearch(tokens.text, tokens.status ?? undefined, tokens.date ?? undefined)
      .then((list) => {
        if (alive) setTaskHits(list);
      })
      .catch(() => {
        if (alive) setTaskHits([]);
      });
    return () => {
      alive = false;
    };
  }, [parsed.raw]);

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
    writeLocalSaved(list);
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
      writeLocalSaved(list);
      setSaved(list);
    }
  };

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    // Ctrl+Delete：清空全部搜索文本
    if (e.key === "Delete" && e.ctrlKey) {
      e.preventDefault();
      updateQ("");
      return;
    }
    if (e.key === "Escape") {
      // 有关键词 → 清空搜索；已为空 → 收起面板
      if (q.trim() !== "") updateQ("");
      else setDismissed(true);
      return;
    }
    if (mode === "completion" && completion && completion.options.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setCompletionActive((i) => moveActive(i, completion.options.length, 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setCompletionActive((i) => moveActive(i, completion.options.length, -1));
        return;
      }
      // Tab：接受当前高亮项（输入后默认高亮第一项）
      if (e.key === "Tab") {
        e.preventDefault();
        acceptCompletion(completion.options[completionIdx]);
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
        setActive((i) => moveActive(i, filtered.length, 1));
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => moveActive(i, filtered.length, -1));
      } else if (e.key === "Enter") {
        recordRecent();
        const idx = active >= 0 && active < filtered.length ? active : 0;
        if (filtered[idx]) openResult(filtered[idx]);
      }
    }
  };

  return (
    <div className={cn("relative", className)}>
      <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={inputRef}
        value={q}
        role="searchbox"
        aria-label="搜索便签"
        placeholder="搜索便签…（tag: is: has: due:）"
        className={cn(
          // 聚焦外框高亮：覆盖默认 ring 颜色为 primary/20
          "h-8 pl-8 text-xs focus-visible:ring-2 focus-visible:ring-primary/20",
          q.trim() !== "" && "pr-8",
        )}
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
            <RecentSearches
              recent={recent}
              saved={saved}
              savedSource={savedSource}
              onPick={updateQ}
              onRemoveRecent={(rq) => setRecent((list) => removeRecentItem(list, rq))}
              onDeleteSaved={handleDeleteSaved}
            />
          )}

          {/* 语法补全 */}
          {mode === "completion" && completion && (
            <SyntaxHints
              completion={completion}
              activeIndex={completionIdx}
              onHover={setCompletionActive}
              onAccept={acceptCompletion}
            />
          )}

          {/* 搜索结果 */}
          {mode === "results" && (
            <SearchResults
              filtered={filtered}
              tasks={taskHits}
              parsed={parsed}
              pending={pending}
              active={active}
              loading={searching}
              error={searchError}
              onActiveChange={setActive}
              onOpen={openResult}
            />
          )}
        </div>
      )}
    </div>
  );
});
