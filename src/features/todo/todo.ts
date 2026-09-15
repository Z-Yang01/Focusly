/** GFM 任务清单（- [ ] / - [x]）的纯函数解析与编辑工具 */

export interface TodoItem {
  /** 0-based 行号（与文本 split("\n") 索引一致） */
  line: number;
  /** 复选框之后的文本 */
  text: string;
  checked: boolean;
  /** 行首空白字符数（缩进层级） */
  indent: number;
}

/** 识别 GFM 任务列表行：行首可有空白，- / * / + 均为合法 bullet，[x]/[X] 为已勾选 */
const TODO_LINE_RE = /^([ \t]*)[-*+] \[([xX ])\] (.*)$/;

/** 解析全部待办行 */
export function parseTodos(content: string): TodoItem[] {
  const items: TodoItem[] = [];
  const lines = content.split("\n");
  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(TODO_LINE_RE);
    if (!m) continue;
    items.push({
      line: i,
      text: m[3],
      checked: m[2] !== " ",
      indent: m[1].length,
    });
  }
  return items;
}

export interface TodoStats {
  total: number;
  done: number;
}

/** 统计待办数量 */
export function todoStats(content: string): TodoStats {
  let total = 0;
  let done = 0;
  for (const item of parseTodos(content)) {
    total += 1;
    if (item.checked) done += 1;
  }
  return { total, done };
}

/** 勾选/取消勾选指定行；行号越界或该行不是任务行时返回原文 */
export function toggleTodoAtLine(content: string, line: number, checked: boolean): string {
  const lines = content.split("\n");
  if (line < 0 || line >= lines.length) return content;
  const m = lines[line].match(/^([ \t]*)([-*+] \[)([xX ])(\].*)$/);
  if (!m) return content;
  lines[line] = `${m[1]}${m[2]}${checked ? "x" : " "}${m[4]}`;
  return lines.join("\n");
}

/** 在文档末尾追加一条未完成任务；处理空文档与结尾换行 */
export function appendTodo(content: string, text: string): string {
  const item = `- [ ] ${text}`;
  if (content === "") return item;
  return content.endsWith("\n") ? content + item : `${content}\n${item}`;
}
