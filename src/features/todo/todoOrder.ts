/** 待办拖动排序的展示层纯函数。
 *  原则：待办真相源 = 正文复选框，本模块绝不修改存储正文——
 *  只为 MarkdownView 生成"展示用正文"（任务文本在任务行槽位间换位，行数与结构不变）。
 *  key 稳定性约束：taskKey 按"原始正文序"派生（taskKey.ts 的同文本出现序号依赖解析顺序），
 *  因此消费方必须对原始 content 做 assignTaskKeys，再用本模块的 displayLineByKey /
 *  originalLineByKey 对齐展示行与原文行——禁止对重排后的文本重新派生 key。
 *  可拖性约束：只有"单行简单项"可拖（带续行/嵌套子列表的任务项不可拖）；
 *  池 = 紧邻的任务行（隔空行/隔非任务行即分池）；单元素池不可拖（无从换位），
 *  跨池拖动一律拒绝（displayLineByKey 消费方以 poolByKey 校验同池）。 */

import { assignTaskKeys } from "./taskKey";
import { parseTodos } from "./todo";
import type { TodoLike } from "./taskMeta";

/** 任务行：允许 - * + 三种标记 + 复选框 */
const TASK_LINE_RE = /^\s*[-*+] \[[ xX]\]/;
/** 任意列表行（含非任务项） */
const LIST_LINE_RE = /^\s*[-*+] /;

const indentOf = (s: string) => s.length - s.trimStart().length;

export interface TaskOrderInfo {
  /** 展示用正文（未启用重排或无可重排块时与输入相同） */
  content: string;
  /** taskKey → 原始正文行号（0-based）。勾选回写用 */
  originalLineByKey: Map<string, number>;
  /** taskKey → 展示行号（0-based）。展示层定位任务态用 */
  displayLineByKey: Map<string, number>;
  /** taskKey → 池序号。跨池拖动校验用（同池才可落位） */
  poolByKey: Map<string, number>;
  /** 展示顺序的全部任务 key（按池顺序拼接） */
  displayKeyOrder: string[];
  /** 可参与拖拽的 key（所在池 ≥2 个任务且自身为单行简单项） */
  reorderableKeys: Set<string>;
}

/** 从存储序（task_meta.sort_order 升序的 keys）+ 内容序合成展示顺序：
 *  存储序中仍存在的 key 按存储顺序在前；已删除的 key 忽略；
 *  新增/未排序的 key 按内容顺序追加在后。 */
export function mergeOrderedKeys(contentKeys: string[], storedKeys: string[]): string[] {
  const present = new Set(contentKeys);
  const seen = new Set<string>();
  const out: string[] = [];
  for (const k of storedKeys) {
    if (present.has(k) && !seen.has(k)) {
      out.push(k);
      seen.add(k);
    }
  }
  for (const k of contentKeys) {
    if (!seen.has(k)) {
      out.push(k);
      seen.add(k);
    }
  }
  return out;
}

/** 把 keys 中 dragKey 移动到 targetKey 的前/后，返回新数组（原数组不变）。
 *  找不到任一 key 时返回原数组副本。 */
export function moveKey(
  keys: string[],
  dragKey: string,
  targetKey: string,
  before: boolean,
): string[] {
  const dragIdx = keys.indexOf(dragKey);
  const targetIdx = keys.indexOf(targetKey);
  if (dragIdx < 0 || targetIdx < 0 || dragIdx === targetIdx) return [...keys];
  const out = keys.filter((k) => k !== dragKey);
  let insert = out.indexOf(targetKey);
  if (!before) insert += 1;
  out.splice(insert, 0, dragKey);
  return out;
}

interface TaskSlot {
  /** 原始行号 */
  line: number;
  key: string;
  /** 该任务项是否单行（下一行是 EOF/空行/同级任务或列表行；缩进更深 = 续行或子项 → 不简单） */
  simple: boolean;
}

function analyzeSlots(lines: string[]): TaskSlot[] {
  const slots: TaskSlot[] = [];
  const todos = assignTaskKeys(parseTodos(lines.join("\n")));
  const byLine = new Map<number, TodoLike>(todos.map((t) => [t.line, t]));
  for (let i = 0; i < lines.length; i++) {
    if (!TASK_LINE_RE.test(lines[i])) continue;
    const next = lines[i + 1];
    const base = indentOf(lines[i]);
    const sibling =
      next !== undefined &&
      next.trim() !== "" &&
      (TASK_LINE_RE.test(next) || LIST_LINE_RE.test(next)) &&
      indentOf(next) <= base;
    const simple = next === undefined || next.trim() === "" || sibling;
    const todo = byLine.get(i);
    if (todo) slots.push({ line: i, key: todo.taskKey, simple });
  }
  return slots;
}

/** 计算展示用正文与各映射。
 *  storedKeys = task_meta.sort_order 升序的 key 列表（未排序传 []）。 */
export function applyTaskOrder(content: string, storedKeys: string[]): TaskOrderInfo {
  const lines = content.split("\n");
  const slots = analyzeSlots(lines);
  const originalLineByKey = new Map(slots.map((s) => [s.key, s.line]));

  // 紧邻任务行构成池；隔空行/非任务行分池
  const pools: TaskSlot[][] = [];
  let current: TaskSlot[] = [];
  for (const slot of slots) {
    if (current.length === 0 || slot.line === current[current.length - 1].line + 1) {
      current.push(slot);
    } else {
      pools.push(current);
      current = [slot];
    }
  }
  if (current.length > 0) pools.push(current);

  const reorderableKeys = new Set<string>();
  const poolByKey = new Map<string, number>();
  const displayLineByKey = new Map<string, number>();
  const out = [...lines];
  const displayKeyOrder: string[] = [];

  pools.forEach((pool, poolIdx) => {
    const contentKeys = pool.map((s) => s.key);
    // 单元素池无从换位：保持内容序，不标记可拖
    const display = pool.length > 1 ? mergeOrderedKeys(contentKeys, storedKeys) : contentKeys;
    if (pool.length > 1) {
      const changed = display.some((k, j) => k !== contentKeys[j]);
      if (changed) {
        const textByKey = new Map(pool.map((s) => [s.key, lines[s.line]]));
        for (let j = 0; j < pool.length; j++) {
          out[pool[j].line] = textByKey.get(display[j]) ?? lines[pool[j].line];
        }
      }
      for (const k of display) {
        reorderableKeys.add(k);
      }
    }
    for (let j = 0; j < pool.length; j++) {
      const k = display[j];
      displayKeyOrder.push(k);
      poolByKey.set(k, poolIdx);
      displayLineByKey.set(k, pool[j].line);
    }
  });

  return {
    content: out.join("\n"),
    originalLineByKey,
    displayLineByKey,
    poolByKey,
    displayKeyOrder,
    reorderableKeys,
  };
}
