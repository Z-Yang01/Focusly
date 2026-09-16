/** 任务身份算法：task_key = {fnv1a32(规范化文本) 8位hex}-{同文本出现序号}
 *  规范化 = trim + 连续空白合并为单空格。
 *  行号变化不影响绑定；同文本多任务按出现序区分。
 *  task_meta 表主键为 (note_id, task_key)，故 key 本身不含 note_id。 */

export function normalizeTaskText(text: string): string {
  return text.trim().replace(/\s+/g, " ");
}

/** FNV-1a 32 位哈希，返回 8 位十六进制（前端唯一实现；Rust 侧不计算 key） */
export function fnv1a32(str: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < str.length; i += 1) {
    h ^= str.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, "0");
}

interface TaskTextInput {
  text: string;
}

/** 为整张便签的待办分配稳定 task_key（ occurrence = 同规范化文本之前的出现次数） */
export function assignTaskKeys<T extends TaskTextInput>(todos: T[]): (T & { taskKey: string })[] {
  const seen = new Map<string, number>();
  return todos.map((t) => {
    const norm = normalizeTaskText(t.text);
    const occurrence = seen.get(norm) ?? 0;
    seen.set(norm, occurrence + 1);
    return { ...t, taskKey: `${fnv1a32(norm)}-${occurrence}` };
  });
}
