/** 番茄钟 Tauri 命令封装 — 番茄钟域内唯一的 invoke 入口。
 *  命令由 Rust 侧实现；命令缺失（后端未接线）时 invoke 会 reject，由调用方降级。 */
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DailyStat,
  PomodoroFinishedEvent,
  StatePayload,
  StatsToday,
  TaskMeta,
  TaskMetaChangedEvent,
} from "./types";
import { POMODORO_EVENTS } from "./types";

// ---------- 番茄钟控制（全部返回 StatePayload） ----------

/** 开始专注；noteId/taskKey/taskText 为 null 表示不绑定任务的纯专注 */
export const pomodoroStart = (
  noteId: string | null,
  taskKey: string | null,
  taskText: string | null,
) => invoke<StatePayload>("pomodoro_start", { noteId, taskKey, taskText });

export const pomodoroPause = () => invoke<StatePayload>("pomodoro_pause");
export const pomodoroResume = () => invoke<StatePayload>("pomodoro_resume");
export const pomodoroSkip = () => invoke<StatePayload>("pomodoro_skip");
/** reason 例："manual" | "completed" */
export const pomodoroStop = (reason: string) =>
  invoke<StatePayload>("pomodoro_stop", { reason });
export const pomodoroAddMinutes = (minutes: number) =>
  invoke<StatePayload>("pomodoro_add_minutes", { minutes });
/** 完成绑定任务：回写正文 `- [x]` + 记 meta + 结束当前专注 */
export const pomodoroCompleteTask = (noteId: string, taskKey: string, lineText: string) =>
  invoke<StatePayload>("pomodoro_complete_task", { noteId, taskKey, lineText });

/** 当前状态；空闲时为 null */
export const pomodoroState = async (): Promise<StatePayload | null> => {
  const snap = await invoke<StatePayload>("pomodoro_state");
  return snap.running ? snap : null;
};

// ---------- 统计 ----------

export const pomodoroStatsToday = () => invoke<StatsToday>("pomodoro_stats_today");
export const pomodoroStatsRange = (days: number) =>
  invoke<DailyStat[]>("pomodoro_stats_range", { days });

// ---------- 任务元数据 ----------

export const taskMetaGet = (noteId: string) => invoke<TaskMeta[]>("task_meta_list", { noteId });

export interface TaskMetaUpdateArgs {
  noteId: string;
  taskKey: string;
  lineText: string;
  status?: "todo" | "done" | "skipped";
  estimate?: number | null;
  priority?: string | null; // high | medium | low
  /** RFC3339 UTC */
  dueAt?: string | null;
  /** 清除跳过标记（恢复为 todo） */
  clearSkip?: boolean;
}

export const taskMetaUpdate = (args: TaskMetaUpdateArgs) =>
  invoke<TaskMeta>("task_meta_update", { ...args });

// ---------- 迷你窗（窗口创建/隐藏由总控接线） ----------

/** 隐藏迷你窗；后端未接线时调用方需静默失败 */
export const pomodoroMiniHide = () => invoke<void>("pomodoro_mini_hide");

// ---------- 事件监听（Rust → JS） ----------

export const onPomodoroState = (cb: (state: StatePayload) => void) =>
  listen<StatePayload>(POMODORO_EVENTS.state, (e) => cb(e.payload));

export const onPomodoroFinished = (cb: (e: PomodoroFinishedEvent) => void) =>
  listen<PomodoroFinishedEvent>(POMODORO_EVENTS.finished, (e) => cb(e.payload));

export const onTaskMetaChanged = (cb: (e: TaskMetaChangedEvent) => void) =>
  listen<TaskMetaChangedEvent>(POMODORO_EVENTS.taskMetaChanged, (e) => cb(e.payload));

export type { UnlistenFn };
