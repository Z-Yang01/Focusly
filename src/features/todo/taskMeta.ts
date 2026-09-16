/** task_meta 与 Markdown 待办的合并渲染逻辑（纯函数）
 *  - ✔️ done：以 Markdown 源文本 `- [x]` 为准（回写即真）
 *  - ✖️ skipped：仅存 task_meta（skip_date 跨日自动复活为 todo）
 *  - 🍅 running：由番茄运行时推导（runningTaskKey 匹配），不属于 meta
 *  - 任务行被编辑/删除：meta 无法匹配任何待办 → 孤儿（显示"原任务已删除"） */

export interface MetaLike {
  taskKey: string;
  lineText: string;
  status: string; // todo | done | skipped
  skipDate: string | null;
  priority?: string | null;
  dueAt?: string | null;
}

export interface TodoLike {
  taskKey: string;
  line: number;
  text: string;
  checked: boolean;
}

export type EffectiveStatus = "todo" | "done" | "skipped" | "running";

export interface TaskDisplayState {
  taskKey: string;
  line: number;
  text: string;
  /** 合并后的展示状态：running > done(markdown) > skipped(meta) > todo */
  status: EffectiveStatus;
  /** 命中的 meta（todo 且无 meta 时为 undefined） */
  meta?: MetaLike;
}

/** 生效状态：skipped 且 skip_date < today 自动复活为 todo */
export function effectiveStatus(meta: MetaLike | undefined, today: string): EffectiveStatus {
  if (!meta) return "todo";
  if (meta.status === "skipped") {
    return (meta.skipDate ?? "") < today ? "todo" : "skipped";
  }
  if (meta.status === "done") return "done";
  return "todo";
}

export interface MergeResult {
  tasks: TaskDisplayState[];
  /** 无法匹配任何待办行的 meta（任务被编辑/删除，历史保留） */
  orphans: MetaLike[];
}

/** 合并：markdown 待办 × task_meta × 番茄运行态 */
export function mergeTaskMeta(
  todos: TodoLike[],
  metaList: MetaLike[],
  today: string,
  runningTaskKey?: string,
): MergeResult {
  const byKey = new Map(metaList.map((m) => [m.taskKey, m]));
  const usedKeys = new Set<string>();

  const tasks: TaskDisplayState[] = todos.map((t) => {
    usedKeys.add(t.taskKey);
    const meta = byKey.get(t.taskKey);
    let status: EffectiveStatus = t.checked ? "done" : effectiveStatus(meta, today);
    if (runningTaskKey && runningTaskKey === t.taskKey && status !== "done") {
      status = "running";
    }
    return { taskKey: t.taskKey, line: t.line, text: t.text, status, meta };
  });

  const orphans = metaList.filter((m) => !usedKeys.has(m.taskKey));
  return { tasks, orphans };
}

/** meta 状态是否等价于"无 meta"（用于清理判断） */
export function isNoopMeta(meta: MetaLike): boolean {
  return meta.status === "todo" && !meta.priority && !meta.dueAt && !meta.skipDate;
}
