/** 管理器主窗口（label=manager） */
import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  QueryClient,
  QueryClientProvider,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import {
  Archive,
  BarChart3,
  CalendarClock,
  Images,
  Inbox,
  Lock,
  Plus,
  Settings,
  SquareCheck,
  StickyNote,
  Tag as TagIcon,
  Trash2,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { EmptyState } from "@/components/ui/empty-state";
import {
  newNote,
  listNotes,
  getTags,
  openNoteWindow,
  quickCaptureToggle,
  createNote,
  updateNoteContent,
} from "@/lib/api";
import {
  onFocusSearch,
  onNotesChanged,
  onOpenSettings,
} from "@/lib/tauri";
import { SearchBar, type SearchBarHandle } from "@/features/search/SearchBar";
import { SettingsDialog } from "@/features/settings/SettingsDialog";
import { NoteCard } from "./NoteCard";
import { TrashView } from "@/features/archive/TrashView";
import { TodayOverdueView } from "@/features/views/DueViews";
import { PrivateSpaceView } from "@/features/privacy/PrivateSpaceView";
import { LayoutToolbar } from "@/features/layout/LayoutToolbar";
import { CommandPaletteHost } from "@/features/command-palette/CommandPalette";
import { FOCUSLY_NAVIGATE, FOCUSLY_OPEN_SETTINGS, type NavigateDetail } from "@/features/command-palette/useCommandPalette";
import { TemplatePicker } from "@/features/templates/TemplatePicker";
import { applyTemplate, type NoteTemplate } from "@/features/templates/templates";
import { DailyNoteButton } from "@/features/templates/DailyNoteButton";
import { ImageManagerDialog } from "@/features/gallery/ImageManagerDialog";
import { StatsDialog } from "@/features/pomodoro/StatsDialog";
import { DndSettingsCard } from "@/features/dnd/DndSettingsCard";
import { TimelineView } from "@/features/daily-tasks/TimelineView";
import { PomodoroBar } from "@/features/pomodoro/PomodoroBar";
import { cn } from "@/lib/utils";
import { QuickTodoInput } from "@/features/todo/QuickTodoInput";
import { toast } from "@/stores/toast";

type ViewKey = "all" | "todo" | "plan" | "archived" | "today" | "trash" | "private";

function ManagerContent() {
  const queryClient = useQueryClient();

  const [view, setView] = useState<ViewKey>("all");
  const [selectedTag, setSelectedTag] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [galleryOpen, setGalleryOpen] = useState(false);
  const [statsOpen, setStatsOpen] = useState(false);
  const searchRef = useRef<SearchBarHandle | null>(null);

  const activeQuery = useQuery({ queryKey: ["notes", "active"], queryFn: () => listNotes("active") });
  const archivedQuery = useQuery({
    queryKey: ["notes", "archived"],
    queryFn: () => listNotes("archived"),
  });
  const todoQuery = useQuery({ queryKey: ["notes", "todo"], queryFn: () => listNotes("todo") });
  const tagsQuery = useQuery({ queryKey: ["tags"], queryFn: getTags });

  // 全局事件：数据失效 / 可见性同步 / 聚焦搜索 / 打开设置（Tauri 事件 + 命令面板 CustomEvent）
  useEffect(() => {
    const onNavigate = (e: Event) => {
      const detail = (e as CustomEvent<NavigateDetail>).detail;
      if (detail.view === "today") setView("today");
      else if (detail.view === "plan") setView("plan");
      else if (detail.view === "trash") setView("trash");
      else if (detail.view === "search" && detail.query) {
        setView("all");
        searchRef.current?.focus();
      }
    };
    const onOpenSettingsEvent = () => setSettingsOpen(true);
    const offs = [
      onNotesChanged(() => {
        void queryClient.invalidateQueries({ queryKey: ["notes"] });
        void queryClient.invalidateQueries({ queryKey: ["tags"] });
      }),
      onFocusSearch(() => searchRef.current?.focus()),
      onOpenSettings(() => setSettingsOpen(true)),
    ];
    window.addEventListener(FOCUSLY_NAVIGATE, onNavigate);
    window.addEventListener(FOCUSLY_OPEN_SETTINGS, onOpenSettingsEvent);
    return () => {
      offs.forEach((p) => void p.then((f) => f()));
      window.removeEventListener(FOCUSLY_NAVIGATE, onNavigate);
      window.removeEventListener(FOCUSLY_OPEN_SETTINGS, onOpenSettingsEvent);
    };
  }, [queryClient]);

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
      toast.error(`新建便签失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleCreateFromTemplate = async (tpl: NoteTemplate) => {
    try {
      const note = await createNote();
      await updateNoteContent(note.id, tpl.name, applyTemplate("", tpl));
      await openNoteWindow(note.id);
    } catch (err) {
      console.error("从模板创建失败", err);
      toast.error(`从模板创建失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleQuickCapture = async () => {
    try {
      await quickCaptureToggle();
    } catch (err) {
      console.error("打开速记箱失败", err);
      toast.error(`速记箱暂不可用：${err instanceof Error ? err.message : String(err)}`);
    }
  };

    // 快速创建待办：新建一张便签，内容即该待办
  const handleQuickTodo = async (text: string) => {
    try {
      const note = await createNote();
      await updateNoteContent(note.id, "", `- [ ] ${text}`);
      await openNoteWindow(note.id);
      toast.success("待办已创建");
    } catch (err) {
      toast.error(`创建待办失败: ${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleOpenNote = (id: string) => {
    openNoteWindow(id).catch((err) => {
      console.error("打开便签失败", err);
      toast.error("打开便签失败，请重试");
    });
  };

  // 键盘网格导航：卡片（data-note-card）聚焦后，方向键在卡片间移动（列数按当前网格实测）
  const handleGridKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (
      e.key !== "ArrowLeft" &&
      e.key !== "ArrowRight" &&
      e.key !== "ArrowUp" &&
      e.key !== "ArrowDown"
    ) {
      return;
    }
    const cards = Array.from(
      e.currentTarget.querySelectorAll<HTMLElement>("[data-note-card]"),
    );
    const current = cards.indexOf(document.activeElement as HTMLElement);
    if (cards.length === 0 || current === -1) return;
    e.preventDefault();
    const gap = parseFloat(getComputedStyle(e.currentTarget).columnGap) || 0;
    const columns = Math.max(
      1,
      Math.round((e.currentTarget.clientWidth + gap) / (cards[0].offsetWidth + gap)),
    );
    const last = cards.length - 1;
    let next = current;
    if (e.key === "ArrowLeft") next = Math.max(0, current - 1);
    else if (e.key === "ArrowRight") next = Math.min(last, current + 1);
    else if (e.key === "ArrowUp") next = Math.max(0, current - columns);
    else next = Math.min(last, current + columns);
    if (next !== current) cards[next].focus();
  };

  const views: { key: ViewKey; label: string; icon: typeof StickyNote; count?: number }[] = [
    { key: "all", label: "全部便签", icon: StickyNote, count: activeQuery.data?.length ?? 0 },
    { key: "todo", label: "待办", icon: SquareCheck, count: todoQuery.data?.length ?? 0 },
    { key: "plan", label: "今日计划", icon: CalendarClock },
    { key: "today", label: "今日 / 逾期", icon: CalendarClock },
    { key: "archived", label: "已归档", icon: Archive, count: archivedQuery.data?.length ?? 0 },
    { key: "trash", label: "回收站", icon: Trash2 },
    { key: "private", label: "私密空间", icon: Lock },
  ];

  const isNotesGrid = view === "all" || view === "todo" || view === "archived";
  const isLoading =
    isNotesGrid &&
    (activeQuery.isLoading || archivedQuery.isLoading || todoQuery.isLoading || tagsQuery.isLoading);

  // 网格视图统一空状态：图标 + 主文案 + 副文案（B3）
  const gridEmpty: { icon: LucideIcon; title: string; description: string } = (() => {
    if (selectedTag) {
      return {
        icon: Inbox,
        title: "没有匹配的便签",
        description: `试试其他标签，或清除「${selectedTag}」过滤`,
      };
    }
    if (view === "todo") {
      return {
        icon: SquareCheck,
        title: "暂无待办",
        description: "在便签中使用 - [ ] 语法创建待办事项",
      };
    }
    if (view === "archived") {
      return {
        icon: Archive,
        title: "没有已归档便签",
        description: "归档的便签会出现在这里",
      };
    }
    return {
      icon: StickyNote,
      title: "还没有便签",
      description: "点击右上角创建你的第一张便签",
    };
  })();

  return (
    <div className="flex h-screen flex-col bg-background text-sm">
      {/* 顶栏：高频操作（速记/模板/每日/新建）在左，低频（统计/图片，size-7 缩小）与设置在分隔线右侧 */}
      <header className="flex h-12 shrink-0 items-center gap-2 border-b px-3">
        <span className="flex shrink-0 items-center gap-1.5 text-sm font-semibold tracking-wide">
          <span className="inline-block size-2 rounded-full bg-pomodoro shadow-sm shadow-pomodoro/30" />
          Focusly
        </span>
        <SearchBar ref={searchRef} className="min-w-0 max-w-md flex-1" />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0 text-xs"
          title="速记箱（快速捕获）"
          onClick={() => void handleQuickCapture()}
        >
          <Zap className="size-3.5" />
          速记
        </Button>
        <TemplatePicker onPick={(tpl) => void handleCreateFromTemplate(tpl)}>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-8 shrink-0 text-xs"
            title="从模板新建"
          >
            模板
          </Button>
        </TemplatePicker>
        <DailyNoteButton />
        <Button type="button" size="sm" className="h-8 shrink-0 text-xs" onClick={() => void handleCreate()}>
          <Plus className="size-3.5" />
          新建
        </Button>
        <div aria-hidden className="mx-0.5 h-5 w-px shrink-0 bg-border" />
        {/* 常驻番茄控制：任意窗口状态都可开始/暂停/停止（修复"番茄没地方关"） */}
        <div className="flex shrink-0 items-center rounded-md border px-1 py-0.5">
          <PomodoroBar noteId="" />
        </div>
        <div aria-hidden className="mx-0.5 h-5 w-px shrink-0 bg-border" />
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7 shrink-0 text-muted-foreground hover:text-foreground"
          title="番茄统计"
          aria-label="番茄统计"
          onClick={() => setStatsOpen(true)}
        >
          <BarChart3 className="size-3.5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7 shrink-0 text-muted-foreground hover:text-foreground"
          title="图片管理（重复/孤儿/缩略图）"
          aria-label="图片管理"
          onClick={() => setGalleryOpen(true)}
        >
          <Images className="size-3.5" />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-8 shrink-0 text-muted-foreground hover:text-foreground"
          title="设置"
          aria-label="设置"
          onClick={() => setSettingsOpen(true)}
        >
          <Settings className="size-4" />
        </Button>
      </header>

      <div className="flex min-h-0 flex-1">
        {/* 侧栏 */}
        <aside className="flex w-44 shrink-0 flex-col gap-0.5 overflow-y-auto border-r bg-gradient-to-b from-primary/[0.03] to-transparent p-2">
          {views.map((v) => (
            <button
              key={v.key}
              type="button"
              aria-current={view === v.key ? "page" : undefined}
              className={cn(
                "flex items-center gap-2 rounded-md px-2 py-1 text-left hover:bg-accent",
                view === v.key && "bg-accent font-medium",
              )}
              onClick={() => setView(v.key)}
            >
              <v.icon className="size-4 shrink-0 text-muted-foreground" />
              {v.label}
              {typeof v.count === "number" && (
                <span className="ml-auto text-xs text-muted-foreground">{v.count}</span>
              )}
            </button>
          ))}

          <div className="mt-2 flex items-center gap-1.5 px-2 text-xs text-muted-foreground">
            <TagIcon className="size-3" />
            标签
          </div>
          {(tagsQuery.data ?? []).map((tag) => (
            <button
              key={tag.name}
              type="button"
              aria-pressed={selectedTag === tag.name}
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

          {/* 布局工具条（网格排列 / 布局预设） */}
          <div className="mt-3 border-t pt-2">
            <LayoutToolbar />
          </div>
          {/* 通知勿扰时段 */}
          <div className="mt-2 border-t pt-2">
            <DndSettingsCard />
          </div>
        </aside>

        {/* 内容区 */}
        <main className="flex min-w-0 flex-1 flex-col p-3">
          {/* 当前标签过滤条（B4：内容区顶部展示 + 快捷清除） */}
          {selectedTag && (
            <div className="mb-2 flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
              <TagIcon className="size-3" />
              <span>
                标签过滤：<span className="font-medium text-foreground">{selectedTag}</span>
              </span>
              <button
                type="button"
                className="ml-1 inline-flex items-center gap-0.5 rounded px-1 py-0.5 transition-colors hover:bg-accent hover:text-foreground"
                onClick={() => setSelectedTag(null)}
              >
                <X className="size-3" />
                清除过滤
              </button>
            </div>
          )}
          {/* key=view 触发重挂载 + 淡入动画（B1） */}
          <div key={view} className="min-h-0 flex-1 animate-fade-in">
            {view === "todo" && (
            <div className="mb-3">
              <QuickTodoInput onSubmit={handleQuickTodo} />
            </div>
          )}
          {view === "plan" ? (
              <TimelineView className="h-full" />
            ) : view === "today" ? (
              <TodayOverdueView onOpenNote={handleOpenNote} />
            ) : view === "trash" ? (
              <div className="h-full">
                <TrashView />
              </div>
            ) : view === "private" ? (
              <div className="h-full">
                <PrivateSpaceView />
              </div>
            ) : (
              <ScrollArea className="h-full">
                {isLoading ? (
                  <div className="flex h-40 items-center justify-center text-xs text-muted-foreground">
                    加载中…
                  </div>
                ) : notes.length === 0 ? (
                  <EmptyState {...gridEmpty} className="h-full min-h-40" />
                ) : (
                  <div
                    className="grid gap-2.5 [grid-template-columns:repeat(auto-fill,minmax(240px,1fr))]"
                    onKeyDown={handleGridKeyDown}
                  >
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
            )}
          </div>
        </main>
      </div>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <ImageManagerDialog open={galleryOpen} onOpenChange={setGalleryOpen} />
      <StatsDialog open={statsOpen} onOpenChange={setStatsOpen} />
      <CommandPaletteHost />
    </div>
  );
}

export function ManagerWindow() {
  // 管理器窗口独立的 React Query 实例（每个 Tauri 窗口一个 JS 上下文）
  // staleTime 30s：数据变更由 notes-changed 事件驱动 invalidateQueries，
  // 无需依赖默认的挂载/聚焦即 refetch（refetchOnWindowFocus 已关闭）
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: { retry: 1, refetchOnWindowFocus: false, staleTime: 30_000 },
        },
      }),
  );
  return (
    <QueryClientProvider client={client}>
      <ManagerContent />
    </QueryClientProvider>
  );
}
