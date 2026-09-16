/** 版本历史面板：左侧版本列表，右侧只读预览 / 与当前内容 diff，可恢复 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { AlertTriangle, GitCompare, History, RefreshCw, RotateCcw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import { getNote } from "@/lib/api";
import { diffLines, diffStats, type DiffLine } from "@/lib/diff";
import { describeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";
import { MarkdownView } from "@/features/editor/MarkdownView";
import {
  listVersions,
  restoreVersion,
  type NoteVersion,
  type VersionSource,
} from "./api";

function errMsg(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

const SOURCE_LABEL: Record<VersionSource, string> = {
  auto: "自动",
  manual: "手动",
  "pre-restore": "恢复前",
};

function SourceBadge({ source }: { source: VersionSource }) {
  const isManual = source === "manual";
  const isPreRestore = source === "pre-restore";
  return (
    <Badge
      variant={isManual ? "secondary" : "outline"}
      className={cn(
        "shrink-0 px-1.5 py-0 text-[10px] font-normal",
        !isManual && "text-muted-foreground",
        isPreRestore && "border-amber-500/40 text-amber-600 dark:text-amber-400",
      )}
    >
      {SOURCE_LABEL[source] ?? String(source)}
    </Badge>
  );
}

/** diff 行渲染：remove 红底 / add 绿底 / same 灰字等宽 */
function DiffView({ diff }: { diff: DiffLine[] }) {
  if (diff.length > 0 && diff.every((d) => d.kind === "same")) {
    return <p className="text-xs text-muted-foreground">两个版本内容完全相同。</p>;
  }
  return (
    <div className="overflow-hidden rounded-md border font-mono text-xs leading-relaxed">
      {diff.map((line, idx) => (
        <div
          key={idx}
          className={cn(
            "flex gap-2 whitespace-pre-wrap px-2 py-px",
            line.kind === "add" && "bg-green-500/10 text-green-700 dark:text-green-400",
            line.kind === "remove" && "bg-red-500/10 text-red-700 dark:text-red-400",
            line.kind === "same" && "text-muted-foreground",
          )}
        >
          <span className="w-4 shrink-0 select-none text-right opacity-60">
            {line.kind === "add" ? "+" : line.kind === "remove" ? "-" : " "}
          </span>
          <span className="min-w-0 flex-1 break-all">{line.text || " "}</span>
        </div>
      ))}
    </div>
  );
}

export interface VersionHistoryPanelProps {
  noteId: string;
  open: boolean;
  onOpenChange: (v: boolean) => void;
  /** 恢复成功后的回调（父级可借此刷新便签内容 / 列表） */
  onRestored?: () => void;
}

type LoadState = "loading" | "ready" | "error";

export function VersionHistoryPanel({
  noteId,
  open,
  onOpenChange,
  onRestored,
}: VersionHistoryPanelProps) {
  const [versions, setVersions] = useState<NoteVersion[]>([]);
  const [state, setState] = useState<LoadState>("loading");
  const [errorText, setErrorText] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [view, setView] = useState<"preview" | "diff">("preview");
  /** 当前已保存内容（diff 基准）；加载失败为 null，仅影响对比功能 */
  const [currentContent, setCurrentContent] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const reload = useCallback(async () => {
    setState("loading");
    setErrorText("");
    setSelectedId(null);
    setView("preview");
    try {
      const [list, current] = await Promise.all([
        listVersions(noteId),
        getNote(noteId)
          .then((n) => n.content)
          .catch(() => null),
      ]);
      setVersions(list);
      setCurrentContent(current);
      setSelectedId(list[0]?.id ?? null);
      setState("ready");
    } catch (err) {
      console.error("加载版本历史失败", err);
      setErrorText(errMsg(err));
      setState("error");
    }
  }, [noteId]);

  useEffect(() => {
    if (open) void reload();
  }, [open, reload]);

  const selected = versions.find((v) => v.id === selectedId) ?? null;

  const handleSelect = (id: string) => {
    setSelectedId(id);
    setView("preview");
  };

  const diff = useMemo(
    () => (selected && currentContent !== null ? diffLines(currentContent, selected.content) : []),
    [selected, currentContent],
  );
  const stats = useMemo(() => diffStats(diff), [diff]);

  const handleRestore = async () => {
    if (!selected || busy) return;
    if (!confirm(`确定恢复到此版本（${describeTime(selected.createdAt)}）？当前内容会先自动备份。`))
      return;
    setBusy(true);
    try {
      await restoreVersion(selected.id);
      toast.success("已恢复到此版本");
      onRestored?.();
      // 恢复后可能出现"恢复前"备份版本，重新拉取
      await reload();
    } catch (err) {
      console.error("恢复版本失败", err);
      toast.error(`恢复失败：${errMsg(err)}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader>
          <DialogTitle className="text-base">版本历史</DialogTitle>
          <DialogDescription>自动保存与恢复前的历史快照，可预览、对比或恢复</DialogDescription>
        </DialogHeader>

        {state === "loading" ? (
          <div className="flex h-64 items-center justify-center text-xs text-muted-foreground">
            加载中…
          </div>
        ) : state === "error" ? (
          <div className="flex h-64 flex-col items-center justify-center gap-2 text-muted-foreground">
            <AlertTriangle className="size-6 opacity-60" />
            <span className="text-sm">后端未就绪</span>
            <span className="max-w-sm truncate text-xs" title={errorText}>
              {errorText}
            </span>
            <Button variant="outline" size="sm" onClick={() => void reload()}>
              <RefreshCw />
              重试
            </Button>
          </div>
        ) : versions.length === 0 ? (
          <div className="flex h-64 flex-col items-center justify-center gap-2 text-muted-foreground">
            <History className="size-8 opacity-50" />
            <span className="text-sm">暂无历史版本</span>
          </div>
        ) : (
          <div className="flex h-[60vh] min-h-0 gap-4">
            {/* 左：版本列表 */}
            <aside className="flex w-52 shrink-0 flex-col border-r pr-2">
              <ScrollArea className="min-h-0 flex-1">
                <ul className="flex flex-col gap-1 pr-1">
                  {versions.map((v) => (
                    <li key={v.id}>
                      <button
                        type="button"
                        onClick={() => handleSelect(v.id)}
                        className={cn(
                          "w-full rounded-md px-2.5 py-2 text-left transition-colors",
                          v.id === selectedId
                            ? "bg-accent text-accent-foreground"
                            : "hover:bg-accent/60",
                        )}
                      >
                        <span className="flex items-center gap-1.5">
                          <SourceBadge source={v.source} />
                          <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
                            {describeTime(v.createdAt)}
                          </span>
                        </span>
                        <span className="mt-1 block truncate text-xs text-muted-foreground">
                          {v.title || "无标题"}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              </ScrollArea>
            </aside>

            {/* 右：预览 / diff */}
            <section className="flex min-w-0 flex-1 flex-col">
              {view === "diff" && (
                <div className="mb-2 flex items-center gap-2 text-xs">
                  <Badge variant="secondary" className="text-green-600 dark:text-green-400">
                    +{stats.added}
                  </Badge>
                  <Badge variant="secondary" className="text-red-600 dark:text-red-400">
                    -{stats.removed}
                  </Badge>
                  <span className="text-muted-foreground">当前内容 vs 选中版本</span>
                </div>
              )}
              <ScrollArea className="min-h-0 flex-1 rounded-lg border bg-card p-3">
                {!selected ? (
                  <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
                    选择左侧版本查看
                  </div>
                ) : view === "preview" ? (
                  <div>
                    <div className="mb-2 border-b pb-2 text-sm font-medium">
                      {selected.title || "无标题"}
                    </div>
                    <MarkdownView content={selected.content} />
                  </div>
                ) : currentContent === null ? (
                  <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
                    无法加载当前内容，暂不能对比
                  </div>
                ) : (
                  <DiffView diff={diff} />
                )}
              </ScrollArea>
              <div className="mt-3 flex items-center justify-end gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={!selected}
                  onClick={() => setView(view === "diff" ? "preview" : "diff")}
                >
                  <GitCompare />
                  {view === "diff" ? "退出对比" : "与此版本对比"}
                </Button>
                <Button size="sm" disabled={!selected || busy} onClick={() => void handleRestore()}>
                  <RotateCcw />
                  恢复此版本
                </Button>
              </div>
            </section>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
