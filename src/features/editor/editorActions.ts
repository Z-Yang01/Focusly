/** 编辑器文本变换纯函数：加粗/斜体/删除线/行内码 共用 wrapSelection */

export interface EditorMutation {
  text: string;
  selStart: number;
  selEnd: number;
}

export interface SimpleMutation {
  text: string;
  selStart: number;
}

function clampPos(text: string, pos: number): number {
  return Math.max(0, Math.min(pos, text.length));
}

/**
 * 用成对 marker 包裹选区（或取消已存在的包裹）。
 * 无选区时插入一对 marker，光标落在两个 marker 中间。
 */
export function wrapSelection(
  text: string,
  selStart: number,
  selEnd: number,
  marker: string,
): EditorMutation {
  const s = clampPos(text, selStart);
  const e = clampPos(text, Math.max(selStart, selEnd));

  if (s === e) {
    return {
      text: text.slice(0, s) + marker + marker + text.slice(s),
      selStart: s + marker.length,
      selEnd: s + marker.length,
    };
  }

  const before = text.slice(s - marker.length, s);
  const after = text.slice(e, e + marker.length);
  if (before === marker && after === marker) {
    return {
      text: text.slice(0, s - marker.length) + text.slice(s, e) + text.slice(e + marker.length),
      selStart: s - marker.length,
      selEnd: e - marker.length,
    };
  }

  return {
    text: text.slice(0, s) + marker + text.slice(s, e) + marker + text.slice(e),
    selStart: s + marker.length,
    selEnd: e + marker.length,
  };
}

/**
 * 切换光标所在行的行首前缀（如 "- "、"- [ ] "、"> "）。
 * 已有该前缀则去掉（光标相应回退，但不越过行首），没有则添加。
 */
export function toggleLinePrefix(text: string, selStart: number, prefix: string): SimpleMutation {
  const pos = clampPos(text, selStart);
  const lineStart = text.lastIndexOf("\n", pos - 1) + 1;
  let lineEnd = text.indexOf("\n", lineStart);
  if (lineEnd === -1) lineEnd = text.length;

  const line = text.slice(lineStart, lineEnd);
  if (line.startsWith(prefix)) {
    return {
      text: text.slice(0, lineStart) + line.slice(prefix.length) + text.slice(lineEnd),
      selStart: Math.max(lineStart, pos - prefix.length),
    };
  }
  return {
    text: text.slice(0, lineStart) + prefix + line + text.slice(lineEnd),
    selStart: pos + prefix.length,
  };
}

/** 在光标处插入片段，光标移到片段之后 */
export function insertAtCursor(text: string, selStart: number, snippet: string): SimpleMutation {
  const pos = clampPos(text, selStart);
  return {
    text: text.slice(0, pos) + snippet + text.slice(pos),
    selStart: pos + snippet.length,
  };
}

/** 常用插入模板 */
export const SNIPPETS = {
  heading: "\n## ",
  divider: "\n---\n",
  table: "\n| 列一 | 列二 | 列三 |\n| --- | --- | --- |\n| 内容 | 内容 | 内容 |\n",
  codeBlock: "\n```\n\n```",
  link: "[标题](https://)",
} as const;
