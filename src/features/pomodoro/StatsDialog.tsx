/** 番茄统计弹窗：今日（数字卡）/ 本周（28 天热力图）/ 复盘（周·月区间报表）。
 *  后端未就绪时降级为提示 + 重试。 */
import { useCallback, useEffect, useMemo, useState } from "react";
import { ChevronLeft, ChevronRight, RefreshCw } from "lucide-react";
import { dailyTaskStats } from "@/lib/api";
import { pomodoroStatsRange, pomodoroStatsReport, pomodoroStatsToday } from "./api";
import { computeStreak, heatTier, humanizeSec, localDateKey } from "./format";
import { barPct, REASON_LABEL, reportRange, type ReportMode } from "./report";
import type { DailyStat, FocusReport, StatsToday } from "./types";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

export interface StatsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

const HEAT_CLASS = ["bg-muted", "bg-primary/25", "bg-primary/55", "bg-primary"] as const;

const RANGE_DAYS = 28;

/** 生成最近 28 天（旧 → 新）的本地日期键 */
function lastDays(n: number): string[] {
  const out: string[] = [];
  const now = new Date();
  for (let i = n - 1; i >= 0; i -= 1) {
    const d = new Date(now);
    d.setDate(d.getDate() - i);
    out.push(localDateKey(d));
  }
  return out;
}

interface StatCardProps {
  label: string;
  value: string;
  className?: string;
}

function StatCard({ label, value, className }: StatCardProps) {
  return (
    <div className={cn("rounded-lg border p-3", className)}>
      <div className="text-lg font-semibold tabular-nums">{value}</div>
      <div className="mt-0.5 text-xs text-muted-foreground">{label}</div>
    </div>
  );
}

export function StatsDialog({ open, onOpenChange }: StatsDialogProps) {
  const [today, setToday] = useState<StatsToday | null>(null);
  const [taskStats, setTaskStats] = useState<import("@/types").DailyTaskStats | null>(null);
  const [days, setDays] = useState<DailyStat[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [failed, setFailed] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);

  // 复盘报表状态
  const [mode, setMode] = useState<ReportMode>("week");
  const [offset, setOffset] = useState(0);
  const [report, setReport] = useState<FocusReport | null>(null);
  const range = useMemo(() => reportRange(mode, offset), [mode, offset]);

  useEffect(() => {
    if (!open) return;
    let alive = true;
    setLoading(true);
    setFailed(false);
    void (async () => {
      try {
        const [t, r, dt] = await Promise.all([
          pomodoroStatsToday(),
          pomodoroStatsRange(RANGE_DAYS),
          dailyTaskStats(localDateKey(new Date())).catch(() => null),
        ]);
        setTaskStats(dt);
        if (!alive) return;
        setToday(t);
        setDays(r);
      } catch (err) {
        console.error("番茄统计拉取失败（后端未就绪？）", err);
        if (alive) setFailed(true);
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [open, reloadKey]);

  const retry = useCallback(() => setReloadKey((k) => k + 1), []);

  // 复盘报表：区间变化即拉取（失败降级为空态 + 可重试，不阻塞其他 tab）
  const [reportLoading, setReportLoading] = useState(false);
  useEffect(() => {
    if (!open) return;
    let alive = true;
    setReportLoading(true);
    void pomodoroStatsReport(range.start, range.end)
      .then((r) => {
        if (alive) setReport(r);
      })
      .catch((err) => {
        console.error("复盘报表拉取失败", err);
        if (alive) setReport(null);
      })
      .finally(() => {
        if (alive) setReportLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [open, range.start, range.end, reloadKey]);

  const maxDaySec = Math.max(1, ...(report?.days ?? []).map((d) => d.focusSec));
  const topMax = Math.max(1, ...(report?.topTasks ?? []).map((t) => t.focusSec));
  const rateText =
    report?.completionRate != null ? `${Math.round(report.completionRate * 100)}%` : "—";
  const topHour = (report?.byHour ?? []).reduce(
    (best, h) => (h.count > best.count ? h : best),
    { hour: -1, count: 0 },
  );

  const byDate = new Map((days ?? []).map((d) => [d.date, d]));
  const cells = lastDays(RANGE_DAYS).map((date) => ({
    date,
    focusCount: byDate.get(date)?.focusCount ?? 0,
  }));
  const streak = computeStreak(cells.filter((c) => c.focusCount > 0).map((c) => c.date));
  const totalCount = cells.reduce((acc, c) => acc + c.focusCount, 0);
  const totalSec = (days ?? []).reduce((acc, d) => acc + d.focusSec, 0);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>番茄统计</DialogTitle>
          <DialogDescription className="text-xs">
            统计只计数量，不含私密便签的标题与内容
          </DialogDescription>
        </DialogHeader>

        {failed ? (
          <div className="flex flex-col items-center gap-3 py-8 text-sm text-muted-foreground">
            <span>番茄钟后端未就绪，统计数据暂不可用</span>
            <Button type="button" variant="outline" size="sm" onClick={retry}>
              <RefreshCw className="size-3.5" />
              重试
            </Button>
          </div>
        ) : loading ? (
          <div className="py-10 text-center text-sm text-muted-foreground">加载中…</div>
        ) : (
          <Tabs defaultValue="today">
            <TabsList>
              <TabsTrigger value="today">今日</TabsTrigger>
              <TabsTrigger value="week">热力</TabsTrigger>
              <TabsTrigger value="review">复盘</TabsTrigger>
            </TabsList>

            {/* 今日 */}
            <TabsContent value="today" className="mt-3">
              <div className="grid grid-cols-2 gap-2.5">
                <StatCard label="番茄数 🍅" value={String(today?.focusCount ?? 0)} />
                <StatCard label="专注时长" value={humanizeSec(today?.focusSec ?? 0)} />
                <StatCard label="完成 ✔️" value={String(today?.doneTasks ?? 0)} />
                <StatCard label="跳过 ✖️" value={String(today?.skippedTasks ?? 0)} />
              </div>
              <div className="mt-2.5 rounded-lg border px-3 py-2 text-xs text-muted-foreground">
                中断次数<span className="ml-2 font-medium tabular-nums text-foreground">{today?.interrupts ?? 0}</span>
              </div>
              <div className="mt-2.5 rounded-lg border px-3 py-2">
                <div className="mb-1 text-xs font-medium">今日计划任务</div>
                {taskStats ? (
                  <div className="grid grid-cols-2 gap-x-3 gap-y-1 text-xs text-muted-foreground">
                    <span>完成 {taskStats.done}/{taskStats.total} · 跳过 {taskStats.skipped}</span>
                    <span>🍅 {taskStats.completedPomodoros}/{taskStats.estimatePomodoros}</span>
                    <span className="col-span-2">已完成任务的计划专注时长 {taskStats.plannedFocusMinutes} 分钟</span>
                  </div>
                ) : (
                  <div className="text-xs text-muted-foreground">暂无任务数据</div>
                )}
              </div>
            </TabsContent>

            {/* 热力（近 28 天） */}
            <TabsContent value="week" className="mt-3">
              <div className="grid grid-cols-7 gap-1.5">
                {cells.map((c) => (
                  <div
                    key={c.date}
                    title={`${c.date} · ${c.focusCount} 个番茄`}
                    className={cn("aspect-square rounded-sm", HEAT_CLASS[heatTier(c.focusCount)])}
                  />
                ))}
              </div>
              <div className="mt-3 grid grid-cols-2 gap-2.5">
                <StatCard label="连续专注天数" value={`${streak} 天`} />
                <StatCard label="28 天合计" value={`${totalCount} 🍅 · ${humanizeSec(totalSec)}`} />
              </div>
            </TabsContent>

            {/* 复盘（周/月区间报表） */}
            <TabsContent value="review" className="mt-3">
              <div className="mb-2 flex items-center gap-1.5">
                <div className="flex overflow-hidden rounded-md border text-xs">
                  {(["week", "month"] as const).map((m) => (
                    <button
                      key={m}
                      type="button"
                      className={cn(
                        "px-2 py-1 transition-colors",
                        mode === m ? "bg-accent font-medium" : "text-muted-foreground hover:bg-accent/60",
                      )}
                      onClick={() => setMode(m)}
                    >
                      {m === "week" ? "按周" : "按月"}
                    </button>
                  ))}
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  aria-label="上一期"
                  disabled={offset <= -23}
                  onClick={() => setOffset((o) => o - 1)}
                >
                  <ChevronLeft className="size-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="size-6"
                  aria-label="下一期"
                  disabled={offset >= 0}
                  onClick={() => setOffset((o) => Math.min(0, o + 1))}
                >
                  <ChevronRight className="size-3.5" />
                </Button>
                <span className="ml-auto text-xs tabular-nums text-muted-foreground">
                  {reportLoading ? "加载中…" : range.label}
                </span>
              </div>

              {/* 逐日专注分钟条形图 */}
              <div className="flex h-20 items-end gap-0.5 rounded-lg border p-2">
                {(report?.days ?? []).map((d) => (
                  <div
                    key={d.date}
                    title={`${d.date} · ${d.focusCount} 🍅 · ${humanizeSec(d.focusSec)}`}
                    className="flex h-full flex-1 flex-col justify-end"
                  >
                    {d.focusSec > 0 && (
                      <div
                        className="rounded-sm bg-primary/70"
                        style={{ height: `${barPct(d.focusSec, maxDaySec)}%` }}
                      />
                    )}
                  </div>
                ))}
                {report && report.days.length === 0 && (
                  <div className="w-full text-center text-xs text-muted-foreground">暂无数据</div>
                )}
              </div>

              <div className="mt-2.5 grid grid-cols-2 gap-2.5">
                <StatCard label="番茄数 🍅" value={String(report?.focusCount ?? 0)} />
                <StatCard label="专注时长" value={humanizeSec(report?.focusSec ?? 0)} />
                <StatCard label="专注完成率" value={rateText} />
                <StatCard label="中断次数" value={String(report?.interruptedCount ?? 0)} />
              </div>

              {/* Top 任务 */}
              <div className="mt-2.5 rounded-lg border px-3 py-2">
                <div className="mb-1.5 text-xs font-medium">最专注任务 Top5</div>
                {(report?.topTasks.length ?? 0) === 0 ? (
                  <div className="text-xs text-muted-foreground">本窗口没有绑定任务的专注记录</div>
                ) : (
                  <div className="space-y-1.5">
                    {report?.topTasks.map((t) => (
                      <div key={`${t.kind}:${t.label}`} className="text-xs">
                        <div className="flex items-center justify-between gap-2">
                          <span className="min-w-0 truncate" title={t.label}>
                            {t.label}
                            <span className="ml-1 text-[10px] text-muted-foreground">
                              {t.kind === "daily" ? "· 今日" : "· 便签"}
                            </span>
                          </span>
                          <span className="shrink-0 tabular-nums text-muted-foreground">
                            {humanizeSec(t.focusSec)} · {t.sessionCount} 次
                          </span>
                        </div>
                        <div className="mt-0.5 h-1 rounded-sm bg-muted">
                          <div
                            className="h-full rounded-sm bg-primary/60"
                            style={{ width: `${barPct(t.focusSec, topMax)}%` }}
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* 中断原因 + 高发时段 */}
              <div className="mt-2.5 grid grid-cols-2 gap-2.5">
                <div className="rounded-lg border px-3 py-2">
                  <div className="mb-1 text-xs font-medium">中断原因</div>
                  {(report?.byReason.length ?? 0) === 0 ? (
                    <div className="text-xs text-muted-foreground">无中断，保持 👍</div>
                  ) : (
                    <div className="space-y-0.5 text-xs text-muted-foreground">
                      {report?.byReason.map((r) => (
                        <div key={r.reason} className="flex justify-between gap-2">
                          <span>{REASON_LABEL[r.reason] ?? r.reason}</span>
                          <span className="tabular-nums">{r.count}</span>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
                <div className="rounded-lg border px-3 py-2">
                  <div className="mb-1 text-xs font-medium">高发时段</div>
                  {topHour.count > 0 ? (
                    <div className="text-xs text-muted-foreground">
                      最常在 <span className="font-medium tabular-nums text-foreground">{String(topHour.hour).padStart(2, "0")}:00</span>{" "}
                      开始专注（{topHour.count} 次）
                    </div>
                  ) : (
                    <div className="text-xs text-muted-foreground">暂无专注记录</div>
                  )}
                  <div className="mt-1.5 flex h-8 items-end gap-px">
                    {(report?.byHour ?? []).map((h) => (
                      <div
                        key={h.hour}
                        title={`${String(h.hour).padStart(2, "0")}:00 · ${h.count} 次`}
                        className="flex h-full flex-1 flex-col justify-end"
                      >
                        {h.count > 0 && (
                          <div
                            className="rounded-sm bg-primary/50"
                            style={{ height: `${barPct(h.count, topHour.count)}%` }}
                          />
                        )}
                      </div>
                    ))}
                  </div>
                  <div className="mt-0.5 flex justify-between text-[10px] text-muted-foreground">
                    <span>0</span>
                    <span>12</span>
                    <span>23</span>
                  </div>
                </div>
              </div>
            </TabsContent>
          </Tabs>
        )}
      </DialogContent>
    </Dialog>
  );
}
