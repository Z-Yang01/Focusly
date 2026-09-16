/** 番茄钟本地类型契约 — 与 Rust 侧 serde camelCase 输出严格对应。
 *  后端命令由独立模块实现；未接线时前端优雅降级为"后端未就绪"。
 *  （src/types/index.ts 为冻结契约文件，番茄钟事件常量在此本地定义） */

export type PomodoroPhase = "focus" | "short_break" | "long_break";

/** 所有 pomodoro_* 写命令的统一返回；noteId/taskKey/taskText 为 null 表示未绑定任务的专注 */
export interface StatePayload {
  phase: PomodoroPhase;
  /** RFC3339 UTC；null = 无进行中的阶段（暂停 / 空闲） */
  endsAt: string | null;
  paused: boolean;
  /** 暂停 / 空闲时的静态剩余秒数（运行中权威值是 endsAt） */
  remainingSec: number;
  noteId: string | null;
  taskKey: string | null;
  /** 私密便签的任务此处为 ""（脱敏） */
  taskText: string | null;
  /** 本轮周期内已完成的番茄数 */
  completedInCycle: number;
}

/** pomodoro-finished 事件载荷 */
export interface PomodoroFinishedEvent {
  phase: PomodoroPhase;
  nextPhase: PomodoroPhase;
  noteId: string | null;
  taskKey: string | null;
  /** 私密便签任务时为 "" */
  taskText: string | null;
}

/** task-meta-changed 事件载荷 */
export interface TaskMetaChangedEvent {
  noteId: string;
}

/** task_meta 行（结构与 taskMeta.ts 的 MetaLike 兼容，可直接作为 MetaLike[] 传入 mergeTaskMeta） */
export interface TaskMeta {
  taskKey: string;
  lineText: string;
  /** todo | done | skipped */
  status: string;
  /** 跳过日期 YYYY-MM-DD；跨日自动复活为 todo */
  skipDate: string | null;
  estimate?: number | null;
  priority?: number | null;
  /** RFC3339 UTC */
  dueAt?: string | null;
}

/** pomodoro_stats_today 返回 */
export interface StatsToday {
  focusCount: number;
  focusSec: number;
  doneTasks: number;
  skippedTasks: number;
  interrupts: number;
}

/** pomodoro_stats_range(days) 返回的单日统计 */
export interface DailyStat {
  /** 本地日期 YYYY-MM-DD */
  date: string;
  focusCount: number;
  focusSec: number;
}

/** 番茄钟事件名（Rust 侧 emit 同名） */
export const POMODORO_EVENTS = {
  state: "pomodoro-state",
  finished: "pomodoro-finished",
  taskMetaChanged: "task-meta-changed",
} as const;
