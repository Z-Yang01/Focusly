/** 图片管理对话框：重复图片 / 孤儿清理 / 缩略图 三个区块（Tabs）。
 *  - 重复图片：按文件内容 SHA-256 分组展示（文件名 / 便签 ID 前 8 位 / 大小），
 *    每项可"打开所在便签"；当前版本仅展示，不提供合并/删除操作。
 *  - 孤儿清理：confirm 前置后删除 images/ 下未被数据库引用的文件，结果"已清理 N 个文件"。
 *  - 缩略图：一键生成，结果"已生成 N 张"；缩略图存 thumbs/ 供后续列表加速。
 *  后端命令未接线时统一降级为"后端未就绪"提示（见 ./api.ts 头注释）。 */
import { useCallback, useEffect, useState } from "react";
import { Copy, ExternalLink, Images, Info, RefreshCw, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { openNoteWindow } from "@/lib/api";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";
import {
  imageCleanupOrphans,
  imageFindDuplicates,
  imageMakeThumbnails,
  type DupGroup,
} from "./api";

function errMsg(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** 后端未就绪 / 调用失败的统一降级态（带重试） */
function BackendNotReady({ error, onRetry }: { error: string; onRetry: () => void }) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed py-8 text-muted-foreground">
      <span className="text-sm">后端未就绪</span>
      <span className="max-w-md truncate text-xs" title={error}>
        {error}
      </span>
      <span className="max-w-md text-center text-xs opacity-70">
        image_* 命令待后端接线
      </span>
      <Button variant="outline" size="sm" onClick={onRetry}>
        <RefreshCw />
        重试
      </Button>
    </div>
  );
}

/** 区块一：重复图片（仅展示 + 打开所在便签，无合并操作） */
function DuplicatesTab() {
  const [groups, setGroups] = useState<DupGroup[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setGroups(await imageFindDuplicates());
    } catch (err) {
      console.error("重复图片检测失败", err);
      setError(errMsg(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openNote = (noteId: string) => {
    openNoteWindow(noteId).catch((err) => {
      console.error("打开便签失败", err);
      toast.error(`打开便签失败：${errMsg(err)}`);
    });
  };

  if (error) return <BackendNotReady error={error} onRetry={() => void refresh()} />;

  if (loading || groups === null) {
    return (
      <div className="flex items-center justify-center rounded-lg border border-dashed py-10 text-xs text-muted-foreground">
        正在按内容哈希扫描…
      </div>
    );
  }

  if (groups.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-1 rounded-lg border border-dashed py-10 text-muted-foreground">
        <Copy className="size-8 opacity-50" />
        <span className="text-sm">未发现重复图片</span>
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-muted-foreground">
        <Info className="mt-0.5 size-4 shrink-0 text-amber-600" />
        <span>
          按文件内容（SHA-256）分组，仅列出内容完全相同且 ≥2 个的组。当前版本仅展示，
          不提供合并/删除操作；同一组内可保留一份、打开所在便签后手动处理。
        </span>
      </div>
      <ScrollArea className="max-h-[46vh] rounded-lg border">
        <ul className="flex flex-col divide-y">
          {groups.map((group) => (
            <li key={group.hash} className="flex flex-col">
              <div className="flex items-center gap-2 bg-muted/40 px-3 py-1.5 text-xs text-muted-foreground">
                <Copy className="size-3 shrink-0" />
                <span className="font-mono" title={group.hash}>
                  {group.hash.slice(0, 12)}…
                </span>
                <span className="ml-auto shrink-0">{group.entries.length} 个文件相同</span>
              </div>
              <ul className="flex flex-col divide-y">
                {group.entries.map((entry) => (
                  <li
                    key={entry.imageId}
                    className="flex items-center gap-3 px-3 py-2 text-sm"
                  >
                    <span
                      className="min-w-0 flex-1 truncate font-medium"
                      title={entry.path}
                    >
                      {entry.filename}
                    </span>
                    <span
                      className="shrink-0 font-mono text-xs text-muted-foreground"
                      title={`便签 ${entry.noteId}`}
                    >
                      {entry.noteId.slice(0, 8)}
                    </span>
                    <span className="w-16 shrink-0 text-right text-xs text-muted-foreground">
                      {formatSize(entry.size)}
                    </span>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => openNote(entry.noteId)}
                      title="打开所在便签"
                    >
                      <ExternalLink />
                      打开所在便签
                    </Button>
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      </ScrollArea>
    </div>
  );
}

/** 区块二：孤儿清理（confirm 前置 → 执行 → "已清理 N 个文件"） */
function OrphansTab() {
  const [busy, setBusy] = useState(false);
  const [cleaned, setCleaned] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async () => {
    const ok = window.confirm(
      "将删除 images/ 目录下所有未被便签引用的图片文件（不涉及数据库记录与便签本身）。确定清理？",
    );
    if (!ok) return;
    setBusy(true);
    setError(null);
    try {
      setCleaned(await imageCleanupOrphans());
    } catch (err) {
      console.error("孤儿清理失败", err);
      setError(errMsg(err));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-muted-foreground">
        <Info className="mt-0.5 size-4 shrink-0 text-amber-600" />
        <span>
          孤儿文件指 images/ 目录下不再被任何便签图片记录引用的残留文件
          （删除图片中途失败、数据库回滚等可能产生）。仅清理应用数据目录内的文件。
        </span>
      </div>

      {error ? (
        <BackendNotReady error={error} onRetry={() => void run()} />
      ) : cleaned !== null ? (
        <div className="rounded-lg border bg-card px-3 py-2 text-sm">
          已清理 {cleaned} 个文件
        </div>
      ) : null}

      <Button onClick={() => void run()} disabled={busy} className="w-full">
        {busy ? <RefreshCw className="animate-spin" /> : <Trash2 />}
        {busy ? "清理中…" : "扫描并清理孤儿文件"}
      </Button>
    </div>
  );
}

/** 区块三：缩略图（一键生成 → "已生成 N 张"；存 thumbs/ 供后续列表加速） */
function ThumbsTab() {
  const [busy, setBusy] = useState(false);
  const [generated, setGenerated] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setGenerated(await imageMakeThumbnails());
    } catch (err) {
      console.error("缩略图生成失败", err);
      setError(errMsg(err));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-muted-foreground">
        <Info className="mt-0.5 size-4 shrink-0 text-amber-600" />
        <span>
          为全部便签图片生成缩略图（最长边 320，JPEG 质量 80）。缩略图存数据目录
          thumbs/ 下（以图片 ID 命名），供后续图片列表加速加载；已存在的自动跳过。
        </span>
      </div>

      {error ? (
        <BackendNotReady error={error} onRetry={() => void run()} />
      ) : generated !== null ? (
        <div className="rounded-lg border bg-card px-3 py-2 text-sm">
          已生成 {generated} 张
        </div>
      ) : null}

      <Button onClick={() => void run()} disabled={busy} className="w-full">
        {busy ? <RefreshCw className="animate-spin" /> : <Images />}
        {busy ? "生成中…" : "生成缩略图"}
      </Button>
    </div>
  );
}

export interface ImageManagerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  className?: string;
}

/** 图片管理对话框（受控组件；挂载点由总控接线，例如管理器工具栏按钮） */
export function ImageManagerDialog({ open, onOpenChange, className }: ImageManagerDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={cn("max-w-2xl", className)}>
        <DialogHeader>
          <DialogTitle>图片管理</DialogTitle>
          <DialogDescription>重复图片检测、孤儿文件清理与缩略图生成</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="duplicates" className="flex min-h-0 flex-col gap-2">
          <TabsList className="grid w-full grid-cols-3">
            <TabsTrigger value="duplicates">重复图片</TabsTrigger>
            <TabsTrigger value="orphans">孤儿清理</TabsTrigger>
            <TabsTrigger value="thumbnails">缩略图</TabsTrigger>
          </TabsList>
          <TabsContent value="duplicates" className="mt-0 flex min-h-0 flex-col">
            <DuplicatesTab />
          </TabsContent>
          <TabsContent value="orphans" className="mt-0">
            <OrphansTab />
          </TabsContent>
          <TabsContent value="thumbnails" className="mt-0">
            <ThumbsTab />
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
