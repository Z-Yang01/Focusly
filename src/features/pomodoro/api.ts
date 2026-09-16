/** 番茄钟 API — 薄 re-export 层（实现统一收敛在 src/lib/api.ts）。
 *  命令由 Rust 侧实现；命令缺失（后端未接线）时 invoke 会 reject，由调用方降级。
 *  事件监听统一由 src/lib/tauri.ts 提供（Rust → JS 事件属于 lib/tauri.ts 职责）。 */
export {
  pomodoroAddMinutes,
  pomodoroCompleteTask,
  pomodoroMiniHide,
  pomodoroPause,
  pomodoroResume,
  pomodoroSkip,
  pomodoroStart,
  pomodoroState,
  pomodoroStatsRange,
  pomodoroStatsToday,
  pomodoroStop,
  taskMetaGet,
  taskMetaUpdate,
  type TaskMetaUpdateArgs,
} from "@/lib/api";
export {
  onPomodoroFinished,
  onPomodoroState,
  onTaskMetaChanged,
  type UnlistenFn,
} from "@/lib/tauri";
export type {
  DailyStat,
  PomodoroFinishedEvent,
  PomodoroPhase,
  StatePayload,
  StatsToday,
  TaskMeta,
  TaskMetaChangedEvent,
} from "@/types";
