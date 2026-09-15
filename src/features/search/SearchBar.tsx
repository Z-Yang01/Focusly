/** 常驻搜索框：250ms 防抖搜索 + 键盘导航结果下拉 */
import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { Search, SquareCheck } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { openNoteWindow, searchNotes } from "@/lib/api";
import { snippetOf } from "@/lib/format";
import type { NoteSummary } from "@/types";
import { cn } from "@/lib/utils";

export interface SearchBarHandle {
  focus: () => void;
}

export interface SearchBarProps {
  className?: string;
}

export const SearchBar = forwardRef<SearchBarHandle, SearchBarProps>(function SearchBar(
  { className },
  ref,
) {
  const [q, setQ] = useState("");
  const [results, setResults] = useState<NoteSummary[]>([]);
  const [active, setActive] = useState(-1);
  const [focused, setFocused] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useImperativeHandle(ref, () => ({ focus: () => inputRef.current?.focus() }), []);

  // 防抖搜索
  useEffect(() => {
    const query = q.trim();
    if (!query) {
      setResults([]);
      setActive(-1);
      return;
    }
    let alive = true;
    const timer = setTimeout(() => {
      searchNotes(query)
        .then((list) => {
          if (!alive) return;
          setResults(list);
          setActive(list.length > 0 ? 0 : -1);
        })
        .catch((err) => {
          console.error("搜索便签失败", err);
          if (alive) setResults([]);
        });
    }, 250);
    return () => {
      alive = false;
      clearTimeout(timer);
    };
  }, [q]);

  const closePanel = () => {
    setResults([]);
    setActive(-1);
  };

  const openResult = (note: NoteSummary) => {
    openNoteWindow(note.id).catch((err) => {
      console.error("打开便签失败", err);
      alert("打开便签失败，请重试");
    });
    // 保留关键词，仅收起结果面板
    closePanel();
  };

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Escape") {
      closePanel();
      return;
    }
    if (results.length === 0) return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive((i) => (i + 1) % results.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive((i) => (i <= 0 ? results.length - 1 : i - 1));
    } else if (e.key === "Enter") {
      const idx = active >= 0 && active < results.length ? active : 0;
      openResult(results[idx]);
    }
  };

  return (
    <div className={cn("relative", className)}>
      <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        ref={inputRef}
        value={q}
        placeholder="搜索便签…"
        className="h-8 pl-8 text-xs"
        onChange={(e) => setQ(e.target.value)}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        onKeyDown={handleKeyDown}
      />
      {focused && results.length > 0 && (
        <div className="absolute left-0 right-0 top-full z-50 mt-1 max-h-80 overflow-y-auto rounded-md border bg-popover p-1 shadow-md">
          {results.map((note, i) => (
            <button
              key={note.id}
              type="button"
              className={cn(
                "flex w-full items-start gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
                i === active && "bg-accent",
              )}
              // 防止 mousedown 先触发输入框 blur 导致点击丢失
              onMouseDown={(e) => e.preventDefault()}
              onMouseEnter={() => setActive(i)}
              onClick={() => openResult(note)}
            >
              <div className="min-w-0 flex-1">
                <div className="truncate text-xs font-medium">
                  {note.title || "无标题"}
                </div>
                <div className="truncate text-xs text-muted-foreground">
                  {snippetOf(note.content, 40)}
                </div>
              </div>
              <div className="flex shrink-0 flex-wrap items-center gap-1">
                {note.todoTotal > 0 && (
                  <span className="flex items-center gap-0.5 text-[10px] text-muted-foreground">
                    <SquareCheck className="size-3" />
                    {note.todoDone}/{note.todoTotal}
                  </span>
                )}
                {note.tags.slice(0, 3).map((tag) => (
                  <Badge key={tag} variant="secondary" className="px-1 py-0 text-[10px]">
                    {tag}
                  </Badge>
                ))}
              </div>
            </button>
          ))}
        </div>
      )}
    </div>
  );
});
