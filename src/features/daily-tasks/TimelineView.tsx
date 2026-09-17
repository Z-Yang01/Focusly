/** 今日计划时间轴视图：左侧时间刻度 + 绝对定位任务块 + 当前时间线。
 *  交互：拖拽块垂直移动（15 分钟吸附，时长不变）、完成/跳过、番茄绑定启动、
 *  编辑、删除、转便签。纯几何逻辑在 ./timeline.ts。 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Copy, Pencil, Play, SkipForward, Square, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { toast } from "@/stores/toast";
import { onPomodoroFinished } from "@/lib/tauri";
import {
  dailyTaskCreate,
  dailyTaskDelete,
  dailyTaskList,
  dailyTaskSetStatus,
  dailyTaskSetTime,
  dailyTaskStats,
  dailyTaskToNote,
  dailyTaskUpdate,
  openNoteWindow,
  pomodoroStart,
  pomodoroState,
  pomodoroStop,
} from "@/lib/api";
import type { DailyTask, DailyTaskStats } from "@/types";
import { TaskDialog, emptyValue, fromTask, type TaskDialogValue } from "./TaskDialog";
import {
  addDays,
  currentLinePercent,
  hhmmToMinutes,
  lanePack,
  localDateKey,
  minutesToY,
  timelineRange,
  weekdayLabel,
} from "./timeline";

const PX_PER_HOUR = 56;
const SNAP_MIN = 15;
const HOUR_LABELS = Array.from({ length: 24 }, (_, i) => i);

interface Props {
  className?: string;
}

export function TimelineView({ className }: Props) {
  const queryClient = useQueryClient();
  const today = localDateKey(new Date());
  const [date, setDate] = useState(today);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editing, setEditing] = useState<DailyTask | null>(null);
  const [dialogValue, setDialogValue] = useState<TaskDialogValue>(emptyValue());
  const [nowMin, setNowMin] = useState(() => {
    const d = new Date();
    return d.getHours() * 60 + d.getMinutes();
  });
  const dragRef = useRef<{ id: string; startY: number; startMin: number; endMin: number | null } | null>(null);
  const [dragPreview, setDragPreview] = useState<{ id: string; startMin: number; endMin: number | null } | null>(null);

  const listQuery = useQuery({
    queryKey: ["dailyTasks", date],
    queryFn: async () => {
      const tasks = await dailyTaskList(date);
      const stats: DailyTaskStats = await dailyTaskStats(date);
      return { tasks, stats };
    },
  });

  const tasks = useMemo(() => listQuery.data?.tasks ?? [], [listQuery.data]);
  const stats = listQuery.data?.stats;

  const refresh = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: ["dailyTasks"] });
  }, [queryClient]);

  // 当前时间线每 30 秒刷新
  useEffect(() => {
    const timer = setInterval(() => {
      const d = new Date();
      setNowMin(d.getHours() * 60 + d.getMinutes());
    }, 30000);
    return () => clearInterval(timer);
  }, []);

  // 番茄完成/状态变化 → 刷新番茄数
  useEffect(() => {
    const off = onPomodoroFinished(() => refresh());
    return () => {
      void off.then((f) => f());
    };
  }, [refresh]);

  const range = useMemo(
    () =>
      timelineRange(
        tasks.map((t) => hhmmToMinutes(t.startTime)),
        tasks.map((t) => hhmmToMinutes(t.endTime)),
      ),
    [tasks],
  );

  const timed = useMemo(
    () =>
      tasks
        .filter((t) => hhmmToMinutes(t.startTime) !== null)
        .sort((a, b) => (hhmmToMinutes(a.startTime) ?? 0) - (hhmmToMinutes(b.startTime) ?? 0)),
    [tasks],
  );
  const unscheduled = tasks.filter((t) => hhmmToMinutes(t.startTime) === null);
  const lanes = useMemo(
    () =>
      lanePack(
        timed.map((t) => ({
          id: t.id,
          start: hhmmToMinutes(t.startTime) ?? 0,
          end: Math.max(hhmmToMinutes(t.endTime) ?? (hhmmToMinutes(t.startTime) ?? 0) + 30, (hhmmToMinutes(t.startTime) ?? 0) + 15),
        })),
      ),
    [timed],
  );
  const laneCount = useMemo(
    () => timed.reduce((max, t) => Math.max(max, (lanes.get(t.id) ?? 0) + 1), 1),
    [timed, lanes],
  );

  const act = async (fn: () => Promise<unknown>, okMsg?: string) => {
    try {
      await fn();
      refresh();
      if (okMsg) toast.success(okMsg);
    } catch (err) {
      console.error("今日任务操作失败", err);
      toast.error(`操作失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const startPomodoro = async (t: DailyTask) => {
    const st = await pomodoroState();
    if (st?.running) {
      await pomodoroStop("switch-to-daily-task");
    }
    await pomodoroStart("", `daily:${t.id}`, t.isPrivate ? "🔒 私密任务" : t.title);
    refresh();
    toast.success(`番茄已绑定：${t.isPrivate ? "私密任务" : t.title}`);
  };

  const toNote = async (t: DailyTask) => {
    await act(async () => {
      const note = await dailyTaskToNote(t.id);
      await openNoteWindow(note.id);
    }, "已转为便签");
  };

  const openCreate = () => {
    setEditing(null);
    setDialogValue({ ...emptyValue(), startTime: "", endTime: "" });
    setDialogOpen(true);
  };
  const openEdit = (t: DailyTask) => {
    setEditing(t);
    setDialogValue(fromTask(t));
    setDialogOpen(true);
  };
  const submitDialog = async () => {
    const input = {
      date,
      startTime: dialogValue.startTime || null,
      endTime: dialogValue.endTime || null,
      title: dialogValue.title.trim(),
      note: dialogValue.note.trim() || null,
      estimatePomodoros: dialogValue.estimatePomodoros,
      priority: dialogValue.priority,
      repeatRule: dialogValue.repeatRule,
      tags: dialogValue.tags.trim() || null,
      isPrivate: dialogValue.isPrivate,
    };
    await act(async () => {
      if (editing) {
        await dailyTaskUpdate(editing.id, input);
      } else {
        await dailyTaskCreate(input);
      }
    }, editing ? "已保存" : "已添加");
    setDialogOpen(false);
  };

  // ---- 拖拽移动时间块 ----
  const onBlockPointerDown = (e: React.PointerEvent, t: DailyTask) => {
    if ((e.target as HTMLElement).closest("button")) return; // 按钮不触发拖拽
    const startMin = hhmmToMinutes(t.startTime);
    if (startMin === null) return;
    const endMin = hhmmToMinutes(t.endTime);
    dragRef.current = { id: t.id, startY: e.clientY, startMin, endMin };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onBlockPointerMove = (e: React.PointerEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    const dy = e.clientY - drag.startY;
    const deltaMin = Math.round(dy / PX_PER_HOUR * 60 / SNAP_MIN) * SNAP_MIN;
    const newStart = Math.max(0, drag.startMin + deltaMin);
    const newEnd = drag.endMin !== null ? newStart + (drag.endMin - drag.startMin) : null;
    setDragPreview({ id: drag.id, startMin: newStart, endMin: newEnd });
  };
  const onBlockPointerUp = async () => {
    const drag = dragRef.current;
    dragRef.current = null;
    if (!drag || !dragPreview || dragPreview.id !== drag.id) {
      setDragPreview(null);
      return;
    }
    const { startMin, endMin } = dragPreview;
    setDragPreview(null);
    if (startMin === drag.startMin && endMin === drag.endMin) return;
    const t = tasks.find((x) => x.id === drag.id);
    if (!t) return;
    await act(() =>
      dailyTaskSetTime(t.id, minutesLabel(startMin), endMin !== null ? minutesLabel(endMin) : null),
    );
  };

  const nowPercent = date === today ? currentLinePercent(nowMin, range) : null;

  return (
    <div className={`flex h-full min-h-0 flex-col ${className ?? ""}`}>
      {/* 头部：日期切换 + 进度 + 添加 */}
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b px-3 py-2">
        <div className="flex items-center gap-1">
          <Button variant="ghost" size="icon" className="size-6 text-xs" title="前一天"
            onClick={() => setDate(addDays(date, -1))}>‹</Button>
          <input type="date" value={date} max="9999-12-31"
            onChange={(e) => e.target.value && setDate(e.target.value)}
            className="h-7 rounded-md border border-input bg-transparent px-2 text-xs" />
          <Button variant="ghost" size="icon" className="size-6 text-xs" title="后一天"
            onClick={() => setDate(addDays(date, 1))}>›</Button>
          {date !== today && (
            <Button variant="outline" size="sm" className="h-7 px-2 text-xs" onClick={() => setDate(today)}>
              今天
            </Button>
          )}
          <span className="text-xs text-muted-foreground">{weekdayLabel(date)}</span>
        </div>
        <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          <span>已完成 {stats?.done ?? 0}/{stats?.total ?? 0}</span>
          <span>·</span>
          <span>🍅 {stats?.completedPomodoros ?? 0}/{stats?.estimatePomodoros ?? 0}</span>
          <span>·</span>
          <span>专注 {stats?.plannedFocusMinutes ?? 0} 分钟</span>
          <Button size="sm" className="h-7 px-2 text-xs" onClick={openCreate}>+ 添加任务</Button>
        </div>
      </div>

      {/* 时间轴主体 */}
      <div className="flex min-h-0 flex-1">
        <div className="w-12 shrink-0 select-none pt-6 text-right text-[10px] text-muted-foreground">
          {HOUR_LABELS.filter((h) => {
            const min = h * 60;
            return min >= range.startMin - 59 && min <= range.endMin;
          }).map((h) => (
            <div key={h} style={{ height: PX_PER_HOUR }} className="relative">
              <span className="absolute right-1 -top-1.5">{String(h).padStart(2, "0")}:00</span>
            </div>
          ))}
        </div>
        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <div className="relative" style={{ height: ((range.endMin - range.startMin) / 60) * PX_PER_HOUR + 24 }}>
            {/* 小时刻度线 */}
            {HOUR_LABELS.map((h) => {
              const min = h * 60;
              if (min < range.startMin || min > range.endMin) return null;
              return (
                <div key={h} className="pointer-events-none absolute inset-x-0 border-t border-border/40"
                  style={{ top: minutesToY(min, range, PX_PER_HOUR) + 24 }} />
              );
            })}
            {/* 当前时间线 */}
            {nowPercent !== null && (
              <div className="pointer-events-none absolute inset-x-0 z-10" style={{ top: `calc(${nowPercent * 100}% + 12px)` }}>
                <div className="border-t-2 border-destructive/70" />
                <div className="absolute -left-1 -top-1 size-2 rounded-full bg-destructive" />
              </div>
            )}
            {/* 任务块 */}
            {timed.map((t) => {
              const startMin = hhmmToMinutes(t.startTime) ?? 0;
              const endMin = hhmmToMinutes(t.endTime);
              const durMin = Math.max(15, (endMin ?? startMin + 30) - startMin);
              const previewing = dragPreview?.id === t.id;
              const effStart = previewing ? dragPreview!.startMin : startMin;
              const lane = lanes.get(t.id) ?? 0;
              const laneW = 100 / laneCount;
              return (
                <div
                  key={t.id}
                  data-daily-task={t.id}
                  onPointerDown={(e) => onBlockPointerDown(e, t)}
                  onPointerMove={onBlockPointerMove}
                  onPointerUp={() => void onBlockPointerUp()}
                  className={`group absolute cursor-grab select-none rounded-md border bg-card/95 p-1.5 text-xs shadow-sm transition-shadow hover:shadow-md active:cursor-grabbing ${
                    t.status === "done" ? "opacity-60" : ""}`}
                  style={{
                    top: minutesToY(effStart, range, PX_PER_HOUR) + 24,
                    height: (durMin / 60) * PX_PER_HOUR - 4,
                    left: `calc(${lane * laneW}% + 4px)`,
                    width: `calc(${laneW}% - 8px)`,
                    borderLeftWidth: 3,
                    borderLeftColor: t.priority === "high" ? "#ef4444" : t.priority === "low" ? "#38bdf8" : "#f59e0b",
                    touchAction: "none",
                  }}
                >
                  <div className={`flex h-full min-h-0 flex-col justify-between gap-1 ${t.status === "skipped" ? "line-through opacity-50" : ""}`}>
                    <div className="min-w-0">
                      <div className="truncate font-medium">
                        {t.isPrivate ? "🔒 私密任务" : t.title}
                        {t.repeatRule !== "none" && <span className="ml-1 text-[10px] text-muted-foreground">↻</span>}
                      </div>
                      <div className="text-[10px] text-muted-foreground">
                        {t.startTime ?? ""}{t.endTime ? `–${t.endTime}` : ""}
                        {t.estimatePomodoros > 0 && (
                          <span className="ml-1">🍅 {t.completedPomodoros}/{t.estimatePomodoros}</span>
                        )}
                      </div>
                    </div>
                    <div className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                      {t.status !== "done" && (
                        <IconBtn title={t.status === "skipped" ? "恢复" : "完成"} onClick={() => void act(() => dailyTaskSetStatus(t.id, t.status === "todo" ? "done" : "todo"))}>
                          <Check className="size-3" />
                        </IconBtn>
                      )}
                      {t.status === "todo" && (
                        <IconBtn title="跳过" onClick={() => void act(() => dailyTaskSetStatus(t.id, "skipped"))}>
                          <SkipForward className="size-3" />
                        </IconBtn>
                      )}
                      {t.status === "todo" && (
                        <PomoBtn task={t} onStart={(task) => void startPomodoro(task)} />
                      )}
                      <IconBtn title="编辑" onClick={() => openEdit(t)}><Pencil className="size-3" /></IconBtn>
                      <IconBtn title="转为便签" onClick={() => void toNote(t)}><Copy className="size-3" /></IconBtn>
                      <IconBtn title="删除" onClick={() => void act(() => dailyTaskDelete(t.id), "已删除")}>
                        <Trash2 className="size-3" />
                      </IconBtn>
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* 未排时区 */}
      {unscheduled.length > 0 && (
        <div className="shrink-0 border-t px-3 py-2">
          <div className="mb-1 text-xs text-muted-foreground">未排时</div>
          <div className="flex flex-wrap gap-1.5">
            {unscheduled.map((t) => (
              <div key={t.id}
                className={`group flex items-center gap-1 rounded-full border bg-card px-2 py-0.5 text-xs ${t.status === "skipped" ? "line-through opacity-50" : ""}`}>
                <span className="max-w-40 truncate">{t.isPrivate ? "🔒 私密任务" : t.title}</span>
                {t.estimatePomodoros > 0 && <span className="text-[10px] text-muted-foreground">🍅{t.completedPomodoros}/{t.estimatePomodoros}</span>}
                <span className="flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                  <IconBtn title="完成" onClick={() => void act(() => dailyTaskSetStatus(t.id, t.status === "todo" ? "done" : "todo"))}>
                    <Check className="size-3" />
                  </IconBtn>
                  <IconBtn title="编辑" onClick={() => openEdit(t)}><Pencil className="size-3" /></IconBtn>
                  <IconBtn title="删除" onClick={() => void act(() => dailyTaskDelete(t.id), "已删除")}>
                    <Trash2 className="size-3" />
                  </IconBtn>
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      <TaskDialog
        open={dialogOpen}
        editing={editing}
        value={dialogValue}
        onValueChange={setDialogValue}
        onCancel={() => setDialogOpen(false)}
        onSubmit={() => void submitDialog()}
      />
    </div>
  );
}

function minutesLabel(min: number): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}`;
}

function IconBtn({ title, onClick, children }: { title: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <button type="button" title={title} aria-label={title}
      className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
      onClick={(e) => { e.stopPropagation(); onClick(); }}>
      {children}
    </button>
  );
}

/** 番茄启动按钮：运行中且绑定了同一任务 → 显示停止 */
function PomoBtn({ task, onStart }: { task: DailyTask; onStart: (t: DailyTask) => void }) {
  const [running, setRunning] = useState(false);
  const [sameTask, setSameTask] = useState(false);
  useEffect(() => {
    let alive = true;
    void pomodoroState().then((st) => {
      if (!alive) return;
      setRunning(Boolean(st?.running));
      setSameTask(st?.taskKey === `daily:${task.id}`);
    });
    return () => { alive = false; };
  }, [task.id]);
  if (running && sameTask) {
    return (
      <IconBtn title="停止番茄" onClick={() => void pomodoroStop("manual")}>
        <Square className="size-3" />
      </IconBtn>
    );
  }
  return (
    <IconBtn title="启动番茄" onClick={() => onStart(task)}>
      <Play className="size-3" />
    </IconBtn>
  );
}
