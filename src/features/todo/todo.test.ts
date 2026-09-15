import { describe, expect, it } from "vitest";
import { appendTodo, parseTodos, todoStats, toggleTodoAtLine } from "./todo";

describe("parseTodos", () => {
  it("解析基础任务行", () => {
    const items = parseTodos("- [ ] 买牛奶\n- [x] 写周报");
    expect(items).toEqual([
      { line: 0, text: "买牛奶", checked: false, indent: 0 },
      { line: 1, text: "写周报", checked: true, indent: 0 },
    ]);
  });

  it("跳过非任务行，行号保持原文索引", () => {
    const items = parseTodos("第一段\n- [ ] 任务\n普通 - [ ] 伪装\n- [X] 大写勾选");
    expect(items).toEqual([
      { line: 1, text: "任务", checked: false, indent: 0 },
      { line: 3, text: "大写勾选", checked: true, indent: 0 },
    ]);
  });

  it("识别多级缩进", () => {
    const items = parseTodos("- [ ] 一级\n  - [x] 二级\n\t- [ ] 三级(tab)");
    expect(items.map((t) => t.indent)).toEqual([0, 2, 1]);
    expect(items[1].line).toBe(1);
    expect(items[2].line).toBe(2);
  });

  it("支持 * 和 + bullet 以及空文本", () => {
    const items = parseTodos("* [ ] 星号\n+ [x] 加号\n- [ ] ");
    expect(items).toHaveLength(3);
    expect(items[2].text).toBe("");
  });

  it("空文档返回空数组", () => {
    expect(parseTodos("")).toEqual([]);
  });
});

describe("todoStats", () => {
  it("统计总数与完成数", () => {
    expect(todoStats("- [ ] a\n- [x] b\n正文\n- [X] c")).toEqual({ total: 3, done: 2 });
  });

  it("无任务时为 0", () => {
    expect(todoStats("普通文本")).toEqual({ total: 0, done: 0 });
  });
});

describe("toggleTodoAtLine", () => {
  const doc = "- [ ] a\n- [x] b\n正文";

  it("精确切换指定行", () => {
    expect(toggleTodoAtLine(doc, 0, true)).toBe("- [x] a\n- [x] b\n正文");
    expect(toggleTodoAtLine(doc, 1, false)).toBe("- [ ] a\n- [ ] b\n正文");
  });

  it("保留缩进与行尾内容", () => {
    const indented = "  - [ ] 带缩进的任务";
    expect(toggleTodoAtLine(indented, 0, true)).toBe("  - [x] 带缩进的任务");
  });

  it("非任务行返回原文", () => {
    expect(toggleTodoAtLine(doc, 2, true)).toBe(doc);
  });

  it("行号越界返回原文", () => {
    expect(toggleTodoAtLine(doc, 99, true)).toBe(doc);
    expect(toggleTodoAtLine(doc, -1, true)).toBe(doc);
  });
});

describe("appendTodo", () => {
  it("空文档直接追加，无前导换行", () => {
    expect(appendTodo("", "新任务")).toBe("- [ ] 新任务");
  });

  it("有尾换行的文档直接拼接", () => {
    expect(appendTodo("已有内容\n", "新任务")).toBe("已有内容\n- [ ] 新任务");
  });

  it("无尾换行的文档先补换行", () => {
    expect(appendTodo("已有内容", "新任务")).toBe("已有内容\n- [ ] 新任务");
  });

  it("多个尾换行保持原样拼接", () => {
    expect(appendTodo("a\n\n", "新任务")).toBe("a\n\n- [ ] 新任务");
  });
});
