/** 便签窗口主体：label=note-<id> 的窗口渲染此组件 */
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { readImage } from "@tauri-apps/plugin-clipboard-manager";
import {
  Ellipsis,
  Eye,
  Monitor,
  Pencil,
  Pin,
  SquareCheck,
  X,
} from "lucide-react";
import {
  addImage,
  addImageData,
  archiveNote,
  cancelReminder,
  closeNoteWindow,
  completeReminder,
  getNote,
  noteWindowReady,
  setNoteFlag,
  setNoteFullscreenBehavior,
  showManager,
  snoozeReminder,
  trashNote,
} from "@/lib/api";
import { onNotesChanged, onReminderFired, type UnlistenFn } from "@/lib/tauri";
import { savedAtText } from "@/lib/format";
import { todoStats } from "@/features/todo/todo";
import { MarkdownEditor } from "@/features/editor/MarkdownEditor";
import { ReminderPopover } from "@/features/reminders/ReminderPopover";
import { ReminderBanner } from "@/features/reminders/ReminderBanner";
import { useNoteAutoSave } from "./useNoteAutoSave";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
  ContextMenu,
  ContextMenuCheckboxItem,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuRadioGroup,
  ContextMenuRadioItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type {
  FullscreenBehavior,
  Note,
  NoteDetail,
  NoteFlag,
  ReminderFiredEvent,
} from "@/types";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";
import { usePomodoro } from "@/features/pomodoro/usePomodoro";
import { PomodoroBar } from "@/features/pomodoro/PomodoroBar";
import { ensureMiniPomodoro } from "@/features/pomodoro/miniWindow";
import { localDateKey } from "@/features/pomodoro/format";
import { taskMetaGet, taskMetaUpdate, pomodoroStart, pomodoroCompleteTask } from "@/features/pomodoro/api";
import type { TaskMeta } from "@/features/pomodoro/types";
import { VersionHistoryPanel } from "@/features/archive/VersionHistoryPanel";

export interface NoteWindowProps {
  noteId: string;
}

export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif"];

function isImagePath(p: string): boolean {
  const ext = p.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTENSIONS.includes(ext);
}

const FULLSCREEN_OPTIONS: { value: FullscreenBehavior; label: string }[] = [
  { value: "normal", label: "普通" },
  { value: "always_top", label: "始终置顶" },
  { value: "fullscreen_show", label: "全屏时显示" },
  { value: "fullscreen_hide", label: "全屏时自动隐藏" },
];

interface IconButtonProps {
  title: string;
  onClick: () => void;
  active?: boolean;
  children: ReactNode;
}

function IconButton({ title, onClick, active, children }: IconButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={cn("size-7 text-muted-foreground hover:text-foreground", active && "text-amber-500 hover:text-amber-500")}
          onClick={onClick}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom" className="text-xs">
        {title}
      </TooltipContent>
    </Tooltip>
  );
}

export function NoteWindow({ noteId }: NoteWindowProps) {
  const [detail, setDetail] = useState<NoteDetail | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [loadState, setLoadState] = useState<"loading" | "ready" | "error">("loading");
  const [preview, setPreview] = useState(() => {
    try {
      return localStorage.getItem(`focusly-preview-${noteId}`) === "1";
    } catch {
      return false;
    }
  });
  const [banner, setBanner] = useState<ReminderFiredEvent | null>(null);
  const [versionOpen, setVersionOpen] = useState(false);
  const pomo = usePomodoro();
  const [taskMetaList, setTaskMetaList] = useState<TaskMeta[]>([]);
  const [taskMenu, setTaskMenu] = useState<{
    taskKey: string;
    lineText: string;
    status: "todo" | "done" | "skipped" | "running";
  } | null>(null);
  const today = localDateKey(new Date());
  const runningOnThisNote = pomo.state?.noteId === noteId ? pomo.state.taskKey : null;
  const pomoOnThisNote = pomo.state?.noteId === noteId && pomo.isRunning;

  const refreshTaskMeta = useCallback(async () => {
    try {
      setTaskMetaList(await taskMetaGet(noteId));
    } catch (err) {
      console.error("读取任务元数据失败", err);
    }
  }, [noteId]);

  useEffect(() => {
    void refreshTaskMeta();
  }, [refreshTaskMeta]);
  const [dragOver, setDragOver] = useState(false);

  const readyCalledRef = useRef(false);
  const closingRef = useRef(false);
  const autosave = useNoteAutoSave({ noteId, title, content });
  const saveNow = autosave.saveNow;

  // 初始加载：数据就绪后才通知 Rust 显示窗口（避免白闪）
  useEffect(() => {
    let alive = true;
    void (async () => {
      try {
        const d = await getNote(noteId);
        if (!alive) return;
        setDetail(d);
        setTitle(d.title);
        setContent(d.content);
        setLoadState("ready");
      } catch (err) {
        console.error("加载便签失败", err);
        if (alive) setLoadState("error");
      } finally {
        if (alive && !readyCalledRef.current) {
          readyCalledRef.current = true;
          noteWindowReady(noteId).catch((err) =>
            console.error("note_window_ready 调用失败", err),
          );
        }
      }
    })();
    return () => {
      alive = false;
    };
  }, [noteId]);

  const refreshMeta = useCallback(() => {
    getNote(noteId)
      .then((d) => setDetail(d))
      .catch((err) => console.error("刷新便签失败", err));
  }, [noteId]);

  // 便签变更：仅刷新 flags / images / tags / nextReminder，不覆盖正在编辑的标题与正文
  useEffect(() => {
    const off = onNotesChanged(() => refreshMeta());
    return () => {
      void off.then((f) => f());
    };
  }, [refreshMeta]);

  // 提醒触发：显示横幅
  useEffect(() => {
    const off: Promise<UnlistenFn> = onReminderFired((e) => {
      if (e.noteId === noteId) setBanner(e);
    });
    return () => {
      void off.then((f) => f());
    };
  }, [noteId]);

  const mergeNote = useCallback((n: Note) => {
    setDetail((d) =>
      d ? { ...d, ...n, images: d.images, tags: d.tags, nextReminder: d.nextReminder } : d,
    );
  }, []);

  const toggleFlag = async (flag: NoteFlag) => {
    if (!detail) return;
    const current =
      flag === "pinned"
        ? detail.isPinned
        : flag === "always_on_top"
          ? detail.isAlwaysOnTop
          : detail.showOnAllDesktops;
    try {
      const n = await setNoteFlag(noteId, flag, !current);
      mergeNote(n);
    } catch (err) {
      console.error("设置便签标记失败", err);
      toast.error(`设置失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const changeFullscreen = async (behavior: FullscreenBehavior) => {
    try {
      const n = await setNoteFullscreenBehavior(noteId, behavior);
      mergeNote(n);
    } catch (err) {
      console.error("设置全屏行为失败", err);
      toast.error(`设置失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  // 图片 → 数据库 + markdown 追加
  const insertImageMarkdown = useCallback(
    (paths: string[]) => {
      const images = paths.filter(isImagePath);
      if (images.length === 0) return;
      void (async () => {
        const lines: string[] = [];
        for (const p of images) {
          try {
            const img = await addImage(noteId, p);
            lines.push(`![${img.filename}](${img.path})`);
          } catch (err) {
            console.error("添加图片失败", err);
            toast.error(`添加图片失败：${p}`);
          }
        }
        if (lines.length === 0) return;
        setContent((c) => {
          const base = c === "" ? "" : `${c.replace(/\n+$/, "")}\n\n`;
          return `${base}${lines.join("\n")}\n`;
        });
      })();
    },
    [noteId],
  );

  // 粘贴剪贴板图片 → RGBA 字节入库
  const handlePasteImage = useCallback(async () => {
    try {
      const image = await readImage();
      const [bytes, size] = await Promise.all([image.rgba(), image.size()]);
      const saved = await addImageData(noteId, size.width, size.height, Array.from(bytes));
      setContent((c) => {
        const base = c === "" ? "" : `${c.replace(/\n+$/, "")}\n\n`;
        return `${base}![${saved.filename}](${saved.path})\n`;
      });
    } catch (err) {
      console.error("粘贴图片失败", err);
      toast.error("粘贴图片失败：剪贴板中没有图片，或图片写入失败");
    }
  }, [noteId]);

  // 文件对话框插入图片
  const handleInsertImage = async () => {
    try {
      const picked = await openDialog({
        title: "插入图片",
        multiple: true,
        filters: [{ name: "图片", extensions: IMAGE_EXTENSIONS }],
      });
      const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
      if (paths.length > 0) insertImageMarkdown(paths);
    } catch (err) {
      console.error("选择图片失败", err);
      toast.error(`选择图片失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  // 拖放：路径来自 tauri onDragDropEvent
  useEffect(() => {
    const un: Promise<UnlistenFn> = getCurrentWebview().onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        setDragOver(true);
      } else if (payload.type === "leave") {
        setDragOver(false);
      } else if (payload.type === "drop") {
        setDragOver(false);
        insertImageMarkdown(payload.paths);
      }
    });
    return () => {
      void un.then((f) => f());
    };
  }, [insertImageMarkdown]);

  // 关闭流程：先 flush 保存，再让 Rust 关窗
  const handleClose = useCallback(async () => {
    if (closingRef.current) return;
    closingRef.current = true;
    try {
      await saveNow();
    } catch (err) {
      console.error("关闭前保存失败", err);
      toast.error("关闭前自动保存失败，请重新打开便签检查内容");
    }
    try {
      await closeNoteWindow(noteId);
    } catch (err) {
      console.error("关闭便签窗口失败", err);
      closingRef.current = false;
      try {
        await getCurrentWebviewWindow().close();
      } catch (closeErr) {
        console.error("窗口 close 失败", closeErr);
      }
    }
  }, [noteId, saveNow]);

  // 拦截窗口关闭按钮（含系统关闭），先保存
  useEffect(() => {
    const un: Promise<UnlistenFn> = getCurrentWebviewWindow().onCloseRequested(async (event) => {
      if (closingRef.current) return; // 关闭流程已启动，放行
      event.preventDefault();
      await handleClose();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [handleClose]);

  // 横幅按钮
  const handleBannerDone = async () => {
    if (!banner) return;
    try {
      await completeReminder(banner.reminderId);
    } catch (err) {
      console.error("完成提醒失败", err);
      toast.error(`完成提醒失败：${err instanceof Error ? err.message : String(err)}`);
    }
    setBanner(null);
    refreshMeta();
  };

  const handleBannerSnooze = async (minutes: number) => {
    if (!banner) return;
    try {
      await snoozeReminder(banner.reminderId, new Date(Date.now() + minutes * 60000).toISOString());
    } catch (err) {
      console.error("稍后提醒失败", err);
      toast.error(`稍后提醒失败：${err instanceof Error ? err.message : String(err)}`);
    }
    setBanner(null);
    refreshMeta();
  };

  const handleBannerDismiss = async () => {
    if (!banner) return;
    try {
      await cancelReminder(banner.reminderId);
    } catch (err) {
      console.error("关闭提醒失败", err);
      toast.error(`关闭提醒失败：${err instanceof Error ? err.message : String(err)}`);
    }
    setBanner(null);
    refreshMeta();
  };

  const handleArchive = async () => {
    try {
      await archiveNote(noteId);
      await closeNoteWindow(noteId);
    } catch (err) {
      console.error("归档失败", err);
      toast.error(`归档失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleDelete = async () => {
    // 契约审计修复：删除改为移入回收站（trash_note），永久删除走回收站视图的 purge
    if (!confirm("确定将该便签移入回收站？可在回收站中恢复。")) return;
    try {
      await trashNote(noteId);
      await closeNoteWindow(noteId);
    } catch (err) {
      console.error("删除失败", err);
      toast.error(`删除失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const togglePreview = () => {
    setPreview((p) => {
      const next = !p;
      try {
        localStorage.setItem(`focusly-preview-${noteId}`, next ? "1" : "0");
      } catch (err) {
        console.error("保存预览偏好失败", err);
      }
      return next;
    });
  };

  if (loadState === "loading") {
    return <div className="h-screen bg-background" />;
  }

  if (loadState === "error" || !detail) {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-3 bg-background text-sm">
        <p className="text-muted-foreground">便签加载失败</p>
        <Button size="sm" onClick={() => window.location.reload()}>
          重试
        </Button>
      </div>
    );
  }

  const stats = todoStats(content);
  const desktopUnsupported = detail.desktopPinState === "unsupported";

  const menuItems = (
    <>
      <DropdownMenuCheckboxItem
        checked={detail.isPinned}
        onCheckedChange={() => void toggleFlag("pinned")}
      >
        置顶
      </DropdownMenuCheckboxItem>
      <DropdownMenuCheckboxItem
        checked={detail.showOnAllDesktops}
        disabled={desktopUnsupported}
        onCheckedChange={() => void toggleFlag("all_desktops")}
      >
        所有桌面显示{desktopUnsupported ? "（系统不支持）" : ""}
      </DropdownMenuCheckboxItem>
      <DropdownMenuSub>
        <DropdownMenuSubTrigger>全屏行为</DropdownMenuSubTrigger>
        <DropdownMenuSubContent>
          <DropdownMenuRadioGroup
            value={detail.fullscreenBehavior}
            onValueChange={(v) => void changeFullscreen(v as FullscreenBehavior)}
          >
            {FULLSCREEN_OPTIONS.map((opt) => (
              <DropdownMenuRadioItem key={opt.value} value={opt.value}>
                {opt.label}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuSubContent>
      </DropdownMenuSub>
      <DropdownMenuSeparator />
      <DropdownMenuItem onSelect={() => void handleInsertImage()}>插入图片</DropdownMenuItem>
      <DropdownMenuItem
        onSelect={() =>
          void showManager().catch((err) => {
            console.error("打开管理器失败", err);
            toast.error("打开管理器失败");
          })
        }
      >
        在管理器中显示
      </DropdownMenuItem>
      <DropdownMenuItem onSelect={() => setVersionOpen(true)}>版本历史</DropdownMenuItem>
      <DropdownMenuItem onSelect={() => void handleArchive()}>归档</DropdownMenuItem>
      <DropdownMenuSeparator />
      <DropdownMenuItem className="text-destructive focus:text-destructive" onSelect={() => void handleDelete()}>
        删除
      </DropdownMenuItem>
    </>
  );

  return (
    <TooltipProvider delayDuration={300}>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <div className="flex h-screen flex-col overflow-hidden rounded-xl border bg-background text-sm shadow-xl">
            {/* 标题栏 */}
            <div data-tauri-drag-region className="flex h-10 shrink-0 items-center gap-0.5 border-b px-1.5">
              <IconButton
                title={detail.isPinned ? "取消置顶" : "置顶"}
                onClick={() => void toggleFlag("pinned")}
                active={detail.isPinned}
              >
                <Pin className="size-4" />
              </IconButton>
              {detail.showOnAllDesktops && (
                <span
                  className="inline-flex size-7 items-center justify-center"
                  title={desktopUnsupported ? "在所有桌面显示（系统不支持）" : "在所有桌面显示"}
                >
                  <Monitor className={cn("size-4 text-muted-foreground", desktopUnsupported && "opacity-50")} />
                </span>
              )}
              <Input
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                onBlur={() => void saveNow()}
                placeholder="无标题"
                className="h-8 flex-1 border-none bg-transparent px-2 text-sm shadow-none focus-visible:ring-0"
              />
              {pomoOnThisNote && (
                <span title={`番茄进行中 ${pomo.mmss}`}>🍅</span>
              )}
              <IconButton
                title={preview ? "切换到编辑" : "切换到预览"}
                onClick={togglePreview}
              >
                {preview ? <Pencil className="size-4" /> : <Eye className="size-4" />}
              </IconButton>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="size-7 text-muted-foreground hover:text-foreground"
                  >
                    <Ellipsis className="size-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" className="text-sm">
                  {menuItems}
                </DropdownMenuContent>
              </DropdownMenu>
              <IconButton title="关闭" onClick={() => void handleClose()}>
                <X className="size-4" />
              </IconButton>
            </div>

            {/* 提醒横幅 */}
            {banner && (
              <ReminderBanner
                fired={banner}
                onDone={() => void handleBannerDone()}
                onSnooze={(m) => void handleBannerSnooze(m)}
                onDismiss={() => void handleBannerDismiss()}
                onOpenNote={() =>
                  void showManager().catch((err) => {
                    console.error("打开管理器失败", err);
                    toast.error("打开管理器失败");
                  })
                }
              />
            )}

            {/* 正文 */}
            <div className="relative min-h-0 flex-1">
              <MarkdownEditor
                value={content}
                onChange={setContent}
                preview={preview}
                onTogglePreview={togglePreview}
                onImagePaths={insertImageMarkdown}
                onPasteImage={() => void handlePasteImage()}
                noteId={noteId}
                taskPassthrough={{
                  taskMetaList,
                  today,
                  runningTaskKey: runningOnThisNote ?? undefined,
                  onTaskMenu: (e) =>
                    setTaskMenu({ taskKey: e.taskKey, lineText: e.lineText, status: e.status }),
                }}
              />
              {dragOver && (
                <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center bg-background/70">
                  <div className="rounded-lg border-2 border-dashed border-primary px-6 py-4 text-xs text-muted-foreground">
                    松开鼠标插入图片
                  </div>
                </div>
              )}
            </div>

            {/* 底部栏 */}
            <div className="flex h-8 shrink-0 items-center gap-2 border-t px-2 text-xs text-muted-foreground">
              <span className="flex items-center gap-1">
                {stats.total > 0 && (
                  <>
                    <SquareCheck className="size-3.5" />
                    {stats.done}/{stats.total}
                  </>
                )}
              </span>
              <PomodoroBar noteId={noteId} />
              <button
                type="button"
                title="迷你番茄窗"
                className="text-muted-foreground hover:text-foreground"
                onClick={() => void ensureMiniPomodoro()}
              >
                🖥
              </button>
              <div className="flex flex-1 justify-center">
                <ReminderPopover note={detail} onChanged={refreshMeta} />
              </div>
              <span className={cn(autosave.error && "text-destructive")}>
                {autosave.saving
                  ? "保存中…"
                  : autosave.error
                    ? "保存失败"
                    : autosave.lastSavedAt
                      ? `已保存 ${savedAtText(autosave.lastSavedAt)}`
                      : ""}
              </span>
            </div>
          </div>
        </ContextMenuTrigger>

        {/* 右键菜单 */}
        <ContextMenuContent className="text-sm">
          <ContextMenuCheckboxItem
            checked={detail.isPinned}
            onCheckedChange={() => void toggleFlag("pinned")}
          >
            置顶
          </ContextMenuCheckboxItem>
          <ContextMenuCheckboxItem
            checked={detail.showOnAllDesktops}
            disabled={desktopUnsupported}
            onCheckedChange={() => void toggleFlag("all_desktops")}
          >
            所有桌面显示{desktopUnsupported ? "（系统不支持）" : ""}
          </ContextMenuCheckboxItem>
          <ContextMenuSub>
            <ContextMenuSubTrigger>全屏行为</ContextMenuSubTrigger>
            <ContextMenuSubContent>
              <ContextMenuRadioGroup
                value={detail.fullscreenBehavior}
                onValueChange={(v) => void changeFullscreen(v as FullscreenBehavior)}
              >
                {FULLSCREEN_OPTIONS.map((opt) => (
                  <ContextMenuRadioItem key={opt.value} value={opt.value}>
                    {opt.label}
                  </ContextMenuRadioItem>
                ))}
              </ContextMenuRadioGroup>
            </ContextMenuSubContent>
          </ContextMenuSub>
          <ContextMenuSeparator />
          <ContextMenuItem
            onSelect={() =>
              void showManager().catch((err) => {
                console.error("打开管理器失败", err);
                toast.error("打开管理器失败");
              })
            }
          >
            在管理器中显示
          </ContextMenuItem>
          <ContextMenuItem onSelect={() => void handleArchive()}>归档</ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem
            className="text-destructive focus:text-destructive"
            onSelect={() => void handleDelete()}
          >
            删除
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      <VersionHistoryPanel
        noteId={noteId}
        open={versionOpen}
        onOpenChange={setVersionOpen}
        onRestored={refreshMeta}
      />
      <Dialog open={taskMenu !== null} onOpenChange={(v) => !v && setTaskMenu(null)}>
        <DialogContent className="max-w-xs gap-2 p-3 text-sm">
          <DialogHeader>
            <DialogTitle className="text-sm">任务：{taskMenu?.lineText}</DialogTitle>
          </DialogHeader>
          <div className="flex flex-col gap-1">
            {[
              {
                label: "🍅 设为当前番茄任务",
                act: () =>
                  taskMenu && void pomodoroStart(noteId, taskMenu.taskKey, taskMenu.lineText),
              },
              {
                label: "✔️ 完成任务",
                act: () =>
                  taskMenu &&
                  void pomodoroCompleteTask(noteId, taskMenu.taskKey, taskMenu.lineText),
              },
              ...[1, 2, 3, 5, 8].map((n) => ({
                label: `🍅 预计番茄数：${n}`,
                act: () =>
                  taskMenu &&
                  void taskMetaUpdate({
                    noteId,
                    taskKey: taskMenu.taskKey,
                    lineText: taskMenu.lineText,
                    estimate: n,
                  }),
              })),
              ...[
                { label: "⬆ 优先级：高", v: "high" },
                { label: "➖ 优先级：中", v: "medium" },
                { label: "⬇ 优先级：低", v: "low" },
              ].map((o) => ({
                label: o.label,
                act: () =>
                  taskMenu &&
                  void taskMetaUpdate({
                    noteId,
                    taskKey: taskMenu.taskKey,
                    lineText: taskMenu.lineText,
                    priority: o.v,
                  }),
              })),
              {
                label: "✖️ 跳过今天",
                act: () =>
                  taskMenu &&
                  void taskMetaUpdate({
                    noteId,
                    taskKey: taskMenu.taskKey,
                    lineText: taskMenu.lineText,
                    status: "skipped",
                  }),
              },
              {
                label: "↺ 清除跳过状态",
                act: () =>
                  taskMenu &&
                  void taskMetaUpdate({
                    noteId,
                    taskKey: taskMenu.taskKey,
                    lineText: taskMenu.lineText,
                    clearSkip: true,
                  }),
              },
            ].map((item) => (
              <Button
                key={item.label}
                type="button"
                variant="ghost"
                size="sm"
                className="justify-start"
                onClick={() => {
                  item.act();
                  void refreshTaskMeta();
                  setTaskMenu(null);
                }}
              >
                {item.label}
              </Button>
            ))}
          </div>
        </DialogContent>
      </Dialog>
    </TooltipProvider>
  );
}
