/** 到期视图：逾期 / 今天 / 未来7天 三段分区（供总控挂载到管理器）。
 *
 *  数据来自 Rust 命令 `get_due_view`（src-tauri/src/db/todos_view.rs），
 *  命令本身由总控注册到 invoke_handler —— 本组件只做展示。
 *  feature 内本地 api 函数与类型（不动 lib/api.ts）。
 */
import { useCallback, useEffect, useState } from "react";
import { AlertCircle, Inbox, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { getDueView } from "@/lib/api";
import { cn } from "@/lib/utils";

// ---------- 本地类型与 api（与 Rust TodoView/DueNoteSummary camelCase 输出对应） ----------

interface DueNote {
  id: string;
  title: string;
  updatedAt: string;
  /** 该分区内（逾期/今天/未来7天）的到期任务数 */
  dueCount: number;
}

interface DueViewData {
  overdue: DueNote[];
  today: DueNote[];
  next7days: DueNote[];
}

function fetchDueView(): Promise<DueViewData> {
  return getDueView();
}

// ---------- 视图 ----------

interface SectionConfig {
  key: keyof DueViewData;
  label: string;
  /** 圆点颜色：逾期红 / 今天蓝 / 未来7天灰 */
  dot: string;
  empty: string;
}

const SECTIONS: SectionConfig[] = [
  { key: "overdue", label: "逾期", dot: "bg-red-500", empty: "没有逾期任务" },
  { key: "today", label: "今天", dot: "bg-blue-500", empty: "今天没有到期任务" },
  { key: "next7days", label: "未来 7 天", dot: "bg-muted-foreground/40", empty: "未来 7 天没有到期任务" },
];

export interface TodayOverdueViewProps {
  onOpenNote: (id: string) => void;
}

export function TodayOverdueView({ onOpenNote }: TodayOverdueViewProps) {
  const [data, setData] = useState<DueViewData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await fetchDueView());
    } catch (err) {
      console.error("获取到期视图失败", err);
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const isEmpty =
    !loading && !error && data !== null &&
    data.overdue.length === 0 && data.today.length === 0 && data.next7days.length === 0;

  return (
    <div className="flex h-full flex-col gap-3 p-3">
      {/* 工具条 */}
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">到期视图</h2>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7"
          disabled={loading}
          onClick={() => void load()}
          title="刷新"
        >
          <RefreshCw className={cn("size-3.5", loading && "animate-spin")} />
        </Button>
      </div>

      {error && (
        <div className="flex flex-col items-center gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-4 text-sm text-destructive">
          <AlertCircle className="size-4" />
          <span>加载失败：{error}</span>
          <Button type="button" variant="outline" size="sm" onClick={() => void load()}>
            重试
          </Button>
        </div>
      )}

      {loading && !error && (
        <p className="py-6 text-center text-sm text-muted-foreground">加载中…</p>
      )}

      {isEmpty && (
        <div className="flex flex-col items-center gap-2 py-8 text-muted-foreground">
          <Inbox className="size-6" />
          <p className="text-sm">近期没有到期的任务，去便签里用「任务 ^2026-09-20」标记到期日吧</p>
        </div>
      )}

      {!loading && !error && data !== null && !isEmpty && (
        <div className="flex flex-col gap-4 overflow-y-auto">
          {SECTIONS.map((section) => {
            const items = data[section.key];
            return (
              <section key={section.key}>
                <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                  <span className={cn("size-1.5 rounded-full", section.dot)} />
                  {section.label}
                  <Badge variant="secondary" className="ml-auto px-1.5 py-0 text-[10px]">
                    {items.length}
                  </Badge>
                </h3>
                {items.length === 0 ? (
                  <p className="px-1 py-1 text-xs text-muted-foreground/70">{section.empty}</p>
                ) : (
                  <ul className="space-y-1">
                    {items.map((note) => (
                      <li key={`${section.key}-${note.id}`}>
                        <button
                          type="button"
                          onClick={() => onOpenNote(note.id)}
                          className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors hover:bg-accent hover:text-accent-foreground"
                        >
                          <span className="min-w-0 flex-1 truncate">{note.title || "无标题"}</span>
                          <Badge
                            variant={section.key === "overdue" ? "destructive" : "secondary"}
                            className="shrink-0 px-1.5 py-0 text-[10px]"
                          >
                            {note.dueCount} 项到期
                          </Badge>
                        </button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}
