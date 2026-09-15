/** 私密空间视图：私密便签列表（仅标题 + 🔒 徽标 + 更新时间，不显示内容摘要）。
 *  解锁查看为两步确认（按住 1.5s 防误触 → 展示全文），后端未就绪时降级为空态。
 *  隐私边界（诚实标注）：当前为隐私隔离模式，不是加密；完整加密（Windows Hello/DPAPI）为后续版本。 */
import { useCallback, useEffect, useRef, useState } from "react";
import { ExternalLink, Info, Lock, LockOpen, RefreshCw, ShieldX } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getNote, openNoteWindow } from "@/lib/api";
import { describeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import type { NoteDetail } from "@/types";
import { MarkdownView } from "@/features/editor/MarkdownView";
import { listPrivateNotes, setNotePrivacy, type PrivateNote } from "./api";

function errMsg(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** 按住确认按钮：持续按住 HOLD_MS 后触发 onConfirmed，中途松开进度归零（纯 UI 防误触） */
const HOLD_MS = 1500;

function HoldConfirmButton({
  onConfirmed,
  disabled,
}: {
  onConfirmed: () => void;
  disabled?: boolean;
}) {
  const [progress, setProgress] = useState(0);
  const timerRef = useRef<number | null>(null);
  const startRef = useRef(0);

  const stop = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const start = useCallback(() => {
    if (disabled) return;
    stop();
    startRef.current = Date.now();
    timerRef.current = window.setInterval(() => {
      const p = Math.min(1, (Date.now() - startRef.current) / HOLD_MS);
      setProgress(p);
      if (p >= 1) {
        stop();
        setProgress(0);
        onConfirmed();
      }
    }, 50);
  }, [disabled, onConfirmed, stop]);

  const cancel = useCallback(() => {
    stop();
    setProgress(0);
  }, [stop]);

  // 卸载时清理定时器
  useEffect(() => stop, [stop]);

  return (
    <Button
      type="button"
      className="relative w-full select-none overflow-hidden"
      disabled={disabled}
      onPointerDown={start}
      onPointerUp={cancel}
      onPointerLeave={cancel}
      onPointerCancel={cancel}
      onContextMenu={(e) => e.preventDefault()}
    >
      <span
        aria-hidden
        className="absolute inset-y-0 left-0 bg-primary-foreground/20 transition-none"
        style={{ width: `${progress * 100}%` }}
      />
      <LockOpen className="relative" />
      <span className="relative">按住 1.5 秒确认本人操作</span>
    </Button>
  );
}

export interface PrivateSpaceViewProps {
  /** 由挂载点控制尺寸（父级需给定有界高度，列表内部滚动） */
  className?: string;
}

export function PrivateSpaceView({ className }: PrivateSpaceViewProps) {
  const [items, setItems] = useState<PrivateNote[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 解锁弹窗：两步确认（1: 按住确认 → 2: 查看全文）
  const [unlockId, setUnlockId] = useState<string | null>(null);
  const [step, setStep] = useState<1 | 2>(1);
  const [detail, setDetail] = useState<NoteDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setItems(await listPrivateNotes());
    } catch (err) {
      console.error("加载私密便签失败", err);
      setLoadError(errMsg(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openUnlock = (id: string) => {
    setUnlockId(id);
    setStep(1);
    setDetail(null);
    setDetailError(null);
  };

  const closeUnlock = () => {
    setUnlockId(null);
    setDetail(null);
    setDetailError(null);
    setDetailLoading(false);
  };

  /** 第一步按住完成后加载全文（内容只有在确认后才拉取/展示） */
  const loadDetail = useCallback(async (id: string) => {
    setStep(2);
    setDetailLoading(true);
    setDetailError(null);
    try {
      setDetail(await getNote(id));
    } catch (err) {
      console.error("读取私密便签失败", err);
      setDetailError(errMsg(err));
    } finally {
      setDetailLoading(false);
    }
  }, []);

  /** 执行动作：成功后刷新列表，失败 alert */
  const run = useCallback(
    async (action: () => Promise<void>) => {
      if (busy) return;
      setBusy(true);
      try {
        await action();
        await refresh();
      } catch (err) {
        console.error("私密空间操作失败", err);
        alert(`操作失败：${errMsg(err)}`);
      } finally {
        setBusy(false);
      }
    },
    [busy, refresh],
  );

  const handleOpenInWindow = () => {
    if (!unlockId) return;
    void run(async () => {
      await openNoteWindow(unlockId);
      closeUnlock();
    });
  };

  const handleUnmarkPrivate = () => {
    if (!unlockId) return;
    void run(async () => {
      await setNotePrivacy(unlockId, "private", false);
      closeUnlock();
    });
  };

  const unlockTitle = items.find((i) => i.id === unlockId)?.title ?? "";

  return (
    <div className={cn("flex h-full min-h-0 flex-col gap-3 text-sm", className)}>
      {/* 顶部说明条（诚实标注当前能力边界） */}
      <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-xs text-muted-foreground">
        <Info className="mt-0.5 size-4 shrink-0 text-amber-600" />
        <span>
          私密便签不进入普通搜索、通知只显示占位、默认不导出。完整加密（Windows
          Hello/DPAPI）为后续版本，当前为隐私隔离模式。
        </span>
      </div>

      {/* 顶部：计数 / 刷新 */}
      <div className="flex flex-wrap items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground">共 {items.length} 条私密便签</span>
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

      {/* 列表主体：仅标题 + 🔒 徽标 + 更新时间（不显示内容摘要） */}
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
          <span className="max-w-md text-center text-xs opacity-70">
            list_private_notes 命令待后端接线
          </span>
          <Button variant="outline" size="sm" onClick={() => void refresh()}>
            <RefreshCw />
            重试
          </Button>
        </div>
      ) : items.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed py-8 text-muted-foreground">
          <Lock className="size-8 opacity-50" />
          <span className="text-sm">暂无私密便签</span>
          <span className="max-w-sm text-center text-xs opacity-70">
            在便签上将其设为私密后，会出现在这里
          </span>
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1">
          <ul className="flex flex-col gap-2 pr-1">
            {items.map((item) => (
              <li
                key={item.id}
                className="flex items-center gap-3 rounded-lg border bg-card px-3 py-2.5 transition-shadow hover:shadow-sm"
              >
                <Badge variant="secondary" className="shrink-0 gap-1" aria-label="私密">
                  <Lock className="size-3" />
                  私密
                </Badge>
                <span
                  className="min-w-0 flex-1 truncate font-medium"
                  title={item.title || "无标题"}
                >
                  {item.title || "无标题"}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {describeTime(item.updatedAt)}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={busy}
                  onClick={() => openUnlock(item.id)}
                >
                  <LockOpen />
                  解锁查看
                </Button>
              </li>
            ))}
          </ul>
        </ScrollArea>
      )}

      {/* 两步确认解锁弹窗 */}
      <Dialog
        open={unlockId !== null}
        onOpenChange={(v) => {
          if (!v) closeUnlock();
        }}
      >
        <DialogContent className="max-w-2xl">
          {step === 1 ? (
            <>
              <DialogHeader>
                <DialogTitle className="truncate pr-6 text-base">
                  {unlockTitle || "无标题"}
                </DialogTitle>
                <DialogDescription>
                  为防误触，请按住按钮确认是本人操作后查看内容。
                </DialogDescription>
              </DialogHeader>
              <p className="text-xs text-muted-foreground">
                当前为隐私隔离模式：此确认仅防误触，不构成身份验证或加密。
              </p>
              <DialogFooter>
                <HoldConfirmButton disabled={busy} onConfirmed={() => void loadDetail(unlockId!)} />
              </DialogFooter>
            </>
          ) : (
            <>
              <DialogHeader>
                <DialogTitle className="truncate pr-6 text-base">
                  {unlockTitle || "无标题"}
                </DialogTitle>
                <DialogDescription>已确认本人操作，内容仅本次弹窗展示</DialogDescription>
              </DialogHeader>
              {detailLoading ? (
                <div className="flex items-center justify-center rounded-lg border border-dashed py-10 text-xs text-muted-foreground">
                  加载内容…
                </div>
              ) : detailError ? (
                <div className="flex flex-col items-center gap-2 rounded-lg border border-dashed py-8 text-muted-foreground">
                  <ShieldX className="size-6 opacity-60" />
                  <span className="max-w-md truncate text-xs" title={detailError}>
                    {detailError}
                  </span>
                  <Button variant="outline" size="sm" onClick={() => void loadDetail(unlockId!)}>
                    <RefreshCw />
                    重试
                  </Button>
                </div>
              ) : (
                <ScrollArea className="max-h-[55vh] rounded-lg border bg-card p-4">
                  <MarkdownView content={detail?.content ?? ""} />
                </ScrollArea>
              )}
              <DialogFooter className="gap-2">
                <Button variant="outline" disabled={busy} onClick={handleOpenInWindow}>
                  <ExternalLink />
                  在窗口中打开
                </Button>
                <Button variant="outline" disabled={busy} onClick={handleUnmarkPrivate}>
                  <LockOpen />
                  移出私密
                </Button>
              </DialogFooter>
            </>
          )}
        </DialogContent>
      </Dialog>
    </div>
  );
}
