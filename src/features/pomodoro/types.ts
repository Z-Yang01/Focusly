/** 番茄钟类型契约 — 薄 re-export 层（类型统一收敛在 src/types/index.ts，serde camelCase）。
 *  保留本文件以维持 feature 内 `./types` 导入路径不变。 */

export {
  POMODORO_EVENTS,
  type DailyStat,
  type PomodoroFinishedEvent,
  type PomodoroPhase,
  type StatePayload,
  type StatsToday,
  type TaskMeta,
  type TaskMetaChangedEvent,
} from "@/types";
