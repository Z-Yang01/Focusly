/** 回收站视图：已删除便签的恢复 / 永久删除 / 批量操作 / 只读预览 */
import { useCallback, useEffect, useState } from "react";
import { ArchiveRestore, RefreshCw, SquareCheck, Trash2 } from "lucide-react";
import { EmptyState } from "@/components/ui/empty-state";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { describeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";
import { MarkdownView } from "@/features/editor/MarkdownView";
import {
  emptyTrash,
  listDeletedNotes,
  purgeNote,
  restoreFromTrash,
  type TrashItem,
} from "./api";

function errMsg(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

export interface TrashViewProps {
  /** 由挂载点控制尺寸（父级需给定有界高度，列表内部滚动） */
  className?: string;
}

export function TrashView({ className }: TrashViewProps) {
  const [items, setItems] = useState<TrashItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const list = await listDeletedNotes();
      setItems(list);
      // 仅保留仍存在的选中项
      const ids = new Set(list.map((i) => i.id));
      setSelected((prev) => {
        const next = new Set<string>();
        prev.forEach((id) => {
          if (ids.has(id)) next.add(id);
        });
        return next;
      });
    } catch (err) {
      console.error("加载回收站失败", err);
      setLoadError(errMsg(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** 执行动作：成功后重新拉取，失败 toast 提示 */
  const run = useCallback(
    async (action: () => Promise<void>) => {
      if (busy) return;
      setBusy(true);
      try {
        await action();
        await refresh();
      } catch (err) {
        console.error("回收站操作失败", err);
        toast.error(`操作失败：${errMsg(err)}`);
      } finally {
        setBusy(false);
      }
    },
    [busy, refresh],
  );

  const toggleOne = (id: string, checked: boolean) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (checked) next.add(id);
      else next.delete(id);
      return next;
    });
  };

  const allSelected = items.length > 0 && selected.size === items.length;
  const toggleAll = () => {
    setSelected(allSelected ? new Set() : new Set(items.map((i) => i.id)));
  };

  const handlePurge = (item: TrashItem) => {
    if (!confirm(`确定永久删除「${item.title || "无标题"}」？此操作不可恢复。`)) return;
    void run(() => purgeNote(item.id));
  };

  const handleEmpty = () => {
    if (items.length === 0) return;
    if (!confirm(`确定清空回收站？共 ${items.length} 条便签将被永久删除。`)) return;
    if (!confirm("再次确认：清空后无法恢复，确定继续？")) return;
    void run(() => emptyTrash());
  };

  const handleRestoreSelected = () => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    void run(async () => {
      let failed = 0;
      for (const id of ids) {
        try {
          await restoreFromTrash(id);
        } catch {
          failed++;
        }
      }
      if (failed > 0) throw new Error(`${failed} 条恢复失败`);
    });
  };

  const handlePurgeSelected = () => {
    const ids = Array.from(selected);
    if (ids.length === 0) return;
    if (!confirm(`确定永久删除所选 ${ids.length} 条便签？此操作不可恢复。`)) return;
    void run(async () => {
      let failed = 0;
      for (const id of ids) {
        try {
          await purgeNote(id);
        } catch {
          failed++;
        }
      }
      if (failed > 0) throw new Error(`${failed} 条删除失败`);
    });
  };

  const preview = items.find((i) => i.id === previewId) ?? null;

  return (
    <div className={cn("flex h-full min-h-0 flex-col gap-3 text-sm", className)}>
      {/* 顶部：全选 / 批量 / 清空 / 刷新 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Checkbox
            checked={allSelected ? true : selected.size > 0 ? "indeterminate" : false}
            onCheckedChange={() => toggleAll()}
            disabled={items.length === 0}
            aria-label="全选"
          />
          <span className="text-xs text-muted-foreground">
            {selected.size > 0
              ? `已选 ${selected.size} / ${items.length} 项`
              : `共 ${items.length} 项`}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {selected.size > 0 && (
            <>
              <Button variant="outline" size="sm" disabled={busy} onClick={handleRestoreSelected}>
                <ArchiveRestore />
                恢复所选
              </Button>
              <Button variant="destructive" size="sm" disabled={busy} onClick={handlePurgeSelected}>
                <Trash2 />
                删除所选
              </Button>
            </>
          )}
          <Button
            variant="outline"
            size="sm"
            className="text-destructive hover:bg-destructive/10 hover:text-destructive"
            disabled={busy || items.length === 0}
            onClick={handleEmpty}
          >
            <Trash2 />
            清空回收站
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-8"
            disabled={loading}
            onClick={() => void refresh()}
            title="刷新"
            aria-label="刷新"
          >
            <RefreshCw className={cn(loading && "animate-spin")} />
          </Button>
        </div>
      </div>

      {/* 列表主体 */}
      {loading ? (
        <div className="flex flex-1 items-center justify-center rounded-lg border border-dashed text-xs text-muted-foreground">
          加载中…
        </div>
      ) : loadError ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed py-8 text-muted-foreground">
          <span className="text-sm">后端未就绪</span>
          <span className="max-w-md truncate text-xs" title={loadError}>
            {loadError}
          </span>
          <Button variant="outline" size="sm" onClick={() => void refresh()}>
            <RefreshCw />
            重试
          </Button>
        </div>
      ) : items.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center rounded-lg border border-dashed py-8">
          <EmptyState
            icon={Trash2}
            title="回收站是空的"
            description="删除的便签会出现在这里"
          />
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <ul className="flex flex-col gap-2 pr-1">
            {items.map((item) => (
              <li
                key={item.id}
                className={cn(
                  "flex items-center gap-3 rounded-lg border bg-card px-3 py-2.5 transition-shadow hover:shadow-sm",
                  selected.has(item.id) && "border-primary/40 bg-primary/5",
                )}
              >
                <Checkbox
                  checked={selected.has(item.id)}
                  onCheckedChange={(v) => toggleOne(item.id, v === true)}
                  onClick={(e) => e.stopPropagation()}
                  aria-label={`选择 ${item.title || "无标题"}`}
                />
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate text-left font-medium underline-offset-2 hover:underline"
                  title={item.title || "无标题"}
                  onClick={() => setPreviewId(item.id)}
                >
                  {item.title || "无标题"}
                </button>
                {item.todoTotal > 0 && (
                  <span className="flex shrink-0 items-center gap-1 text-xs text-muted-foreground">
                    <SquareCheck className="size-3.5" />
                    {item.todoDone}/{item.todoTotal}
                  </span>
                )}
                <span className="shrink-0 text-xs text-muted-foreground">
                  {describeTime(item.deletedAt)}
                </span>
                <div className="flex shrink-0 items-center gap-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={busy}
                    onClick={() => void run(() => restoreFromTrash(item.id))}
                  >
                    <ArchiveRestore />
                    恢复
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    className="text-destructive hover:bg-destructive/10 hover:text-destructive"
                    disabled={busy}
                    onClick={() => handlePurge(item)}
                  >
                    <Trash2 />
                    删除
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        </ScrollArea>
      )}

      {/* 只读预览 */}
      <Dialog
        open={preview !== null}
        onOpenChange={(v) => {
          if (!v) setPreviewId(null);
        }}
      >
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle className="truncate pr-6 text-base">
              {preview?.title || "无标题"}
            </DialogTitle>
            <DialogDescription>
              {preview ? `删除于 ${describeTime(preview.deletedAt)}` : ""}
              {preview && preview.todoTotal > 0
                ? ` · 待办 ${preview.todoDone}/${preview.todoTotal}`
                : ""}
            </DialogDescription>
          </DialogHeader>
          <ScrollArea className="max-h-[55vh] rounded-lg border bg-card p-4">
            <MarkdownView content={preview?.content ?? ""} />
          </ScrollArea>
        </DialogContent>
      </Dialog>
    </div>
  );
}
