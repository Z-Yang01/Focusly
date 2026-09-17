/** 番茄统计弹窗：今日（数字卡）/ 本周（28 天热力图 + 连续天数 + 合计）。
 *  后端未就绪时降级为提示 + 重试。 */
import { useCallback, useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { dailyTaskStats } from "@/lib/api";
import { pomodoroStatsRange, pomodoroStatsToday } from "./api";
import { computeStreak, heatTier, humanizeSec, localDateKey } from "./format";
import type { DailyStat, StatsToday } from "./types";
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
      <DialogContent className="max-w-md">
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
              <TabsTrigger value="week">本周</TabsTrigger>
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

            {/* 本周（近 28 天热力图） */}
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
          </Tabs>
        )}
      </DialogContent>
    </Dialog>
  );
}
