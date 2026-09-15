/** 管理器主窗口（label=manager） */
import { useEffect, useMemo, useRef, useState } from "react";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  Archive,
  Eye,
  EyeOff,
  Inbox,
  Plus,
  Settings,
  SquareCheck,
  StickyNote,
  Tag as TagIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { newNote, listNotes, getTags, openNoteWindow, toggleAllNotes } from "@/lib/api";
import { onFocusSearch, onNotesChanged, onNotesVisibility, onOpenSettings } from "@/lib/tauri";
import { useUiStore } from "@/stores/ui";
import { SearchBar, type SearchBarHandle } from "@/features/search/SearchBar";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { NoteCard } from "./NoteCard";
import { cn } from "@/lib/utils";

type ViewKey = "all" | "todo" | "archived";

function ManagerContent() {
  const queryClient = useQueryClient();
  const notesVisible = useUiStore((s) => s.notesVisible);
  const setNotesVisible = useUiStore((s) => s.setNotesVisible);

  const [view, setView] = useState<ViewKey>("all");
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const searchRef = useRef<SearchBarHandle | null>(null);

  const activeQuery = useQuery({ queryKey: ["notes", "active"], queryFn: () => listNotes("active") });
  const archivedQuery = useQuery({
    queryKey: ["notes", "archived"],
    queryFn: () => listNotes("archived"),
  });
  const todoQuery = useQuery({ queryKey: ["notes", "todo"], queryFn: () => listNotes("todo") });
  const tagsQuery = useQuery({ queryKey: ["tags"], queryFn: getTags });

  // 全局事件：数据失效 / 可见性同步 / 聚焦搜索 / 打开设置
  useEffect(() => {
    const offs = [
      onNotesChanged(() => {
        void queryClient.invalidateQueries({ queryKey: ["notes"] });
        void queryClient.invalidateQueries({ queryKey: ["tags"] });
      }),
      onNotesVisibility((v) => setNotesVisible(v)),
      onFocusSearch(() => searchRef.current?.focus()),
      onOpenSettings(() => setSettingsOpen(true)),
    ];
    return () => {
      offs.forEach((p) => void p.then((f) => f()));
    };
  }, [queryClient, setNotesVisible]);

  const notes = useMemo(() => {
    const base =
      view === "all" ? activeQuery.data : view === "todo" ? todoQuery.data : archivedQuery.data;
    const list = base ?? [];
    if (!selectedTag) return list;
    return list.filter((n) => n.tags.includes(selectedTag));
  }, [view, selectedTag, activeQuery.data, todoQuery.data, archivedQuery.data]);

  const handleCreate = async () => {
    try {
      await newNote();
    } catch (err) {
      console.error("新建便签失败", err);
      alert(`新建便签失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleToggleNotes = async () => {
    try {
      await toggleAllNotes();
    } catch (err) {
      console.error("切换便签可见性失败", err);
      alert(`操作失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleOpenNote = (id: string) => {
    openNoteWindow(id).catch((err) => {
      console.error("打开便签失败", err);
      alert("打开便签失败，请重试");
    });
  };

  const views: { key: ViewKey; label: string; icon: typeof StickyNote; count: number }[] = [
    { key: "all", label: "全部便签", icon: StickyNote, count: activeQuery.data?.length ?? 0 },
    { key: "todo", label: "待办", icon: SquareCheck, count: todoQuery.data?.length ?? 0 },
    { key: "archived", label: "已归档", icon: Archive, count: archivedQuery.data?.length ?? 0 },
  ];

  const isLoading =
    activeQuery.isLoading || archivedQuery.isLoading || todoQuery.isLoading || tagsQuery.isLoading;

  return (
    <div className="flex h-screen flex-col bg-background text-sm">
      {/* 顶栏 */}
      <header className="flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <span className="shrink-0 text-sm font-semibold tracking-wide">Focusly</span>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-8 shrink-0 text-muted-foreground hover:text-foreground"
          title={notesVisible ? "隐藏全部便签" : "显示全部便签"}
          onClick={() => void handleToggleNotes()}
        >
          {notesVisible ? <Eye className="size-4" /> : <EyeOff className="size-4" />}
        </Button>
        <SearchBar ref={searchRef} className="mx-auto min-w-0 max-w-md flex-1" />
        <Button type="button" size="sm" className="h-8 shrink-0 text-xs" onClick={() => void handleCreate()}>
          <Plus className="size-3.5" />
          新建便签
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-8 shrink-0 text-muted-foreground hover:text-foreground"
          title="设置"
          onClick={() => setSettingsOpen(true)}
        >
          <Settings className="size-4" />
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* 侧栏 */}
        <aside className="flex w-52 shrink-0 flex-col gap-0.5 overflow-y-auto border-r p-2">
          {views.map((v) => (
            <button
              key={v.key}
              type="button"
              className={cn(
                "flex items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-accent",
                view === v.key && "bg-accent font-medium",
              )}
              onClick={() => setView(v.key)}
            >
              <v.icon className="size-4 shrink-0 text-muted-foreground" />
              {v.label}
              <span className="ml-auto text-xs text-muted-foreground">{v.count}</span>
            </button>
          ))}

          <div className="mt-3 flex items-center gap-1.5 px-2 text-xs text-muted-foreground">
            <TagIcon className="size-3" />
            标签
          </div>
          {(tagsQuery.data ?? []).map((tag) => (
            <button
              key={tag.name}
              type="button"
              className={cn(
                "flex items-center gap-2 rounded-md px-2 py-1 text-left text-xs hover:bg-accent",
                selectedTag === tag.name && "bg-accent font-medium",
              )}
              onClick={() => setSelectedTag(selectedTag === tag.name ? null : tag.name)}
            >
              <span className="min-w-0 flex-1 truncate">{tag.name}</span>
              <span className="shrink-0 text-muted-foreground">{tag.count}</span>
            </button>
          ))}
          {selectedTag && (
            <button
              type="button"
              className="mt-1 rounded-md px-2 py-1 text-left text-xs text-muted-foreground hover:bg-accent"
              onClick={() => setSelectedTag(null)}
            >
              清除标签过滤
            </button>
          )}
        </aside>

        {/* 内容区 */}
        <main className="min-w-0 flex-1 p-3">
          <ScrollArea className="h-full">
            {isLoading ? (
              <div className="flex h-40 items-center justify-center text-xs text-muted-foreground">
                加载中…
              </div>
            ) : notes.length === 0 ? (
              <div className="flex h-full min-h-40 flex-col items-center justify-center gap-2 text-muted-foreground">
                <Inbox className="size-8 opacity-50" />
                <p className="text-xs">
                  {selectedTag || view !== "all"
                    ? "没有匹配的便签"
                    : "还没有便签，点击右上角「新建便签」创建一个吧"}
                </p>
              </div>
            ) : (
              <div className="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(230px,1fr))]">
                {notes.map((note) => (
                  <NoteCard
                    key={note.id}
                    note={note}
                    onOpen={() => handleOpenNote(note.id)}
                  />
                ))}
              </div>
            )}
          </ScrollArea>
        </main>
      </div>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}

export function ManagerWindow() {
  // 管理器窗口独立的 React Query 实例（每个 Tauri 窗口一个 JS 上下文）
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: false } },
      }),
  );
  return (
    <QueryClientProvider client={client}>
      <ManagerContent />
    </QueryClientProvider>
  );
}
