import { describe, expect, it } from "vitest";
import { insertAtCursor, SNIPPETS, toggleLinePrefix, wrapSelection } from "./editorActions";

describe("wrapSelection", () => {
  it("有选区时包裹", () => {
    expect(wrapSelection("hello world", 0, 5, "**")).toEqual({
      text: "**hello** world",
      selStart: 2,
      selEnd: 7,
    });
  });

  it("已包裹时取消包裹", () => {
    expect(wrapSelection("**hello** world", 2, 7, "**")).toEqual({
      text: "hello world",
      selStart: 0,
      selEnd: 5,
    });
  });

  it("无选区时插入成对标记，光标居中", () => {
    expect(wrapSelection("abc", 1, 1, "**")).toEqual({
      text: "a****bc",
      selStart: 3,
      selEnd: 3,
    });
  });

  it("各 marker 共用：斜体/删除线/行内码", () => {
    expect(wrapSelection("x", 0, 1, "*")).toEqual({ text: "*x*", selStart: 1, selEnd: 2 });
    expect(wrapSelection("x", 0, 1, "~~")).toEqual({ text: "~~x~~", selStart: 2, selEnd: 3 });
    expect(wrapSelection("x", 0, 1, "`")).toEqual({ text: "`x`", selStart: 1, selEnd: 2 });
  });

  it("选区位置越界时收敛到文本范围", () => {
    expect(wrapSelection("ab", 0, 99, "**")).toEqual({ text: "**ab**", selStart: 2, selEnd: 4 });
  });

  it("中间被包裹但选区含边界时不误判取消", () => {
    // 选区从标记外开始：before 不等于标记，应整体再包裹而不是取消
    expect(wrapSelection("**hello**", 0, 7, "**")).toEqual({
      text: "****hello****",
      selStart: 2,
      selEnd: 9,
    });
  });
});

describe("toggleLinePrefix", () => {
  it("无前缀时添加，光标后移", () => {
    expect(toggleLinePrefix("hello", 2, "- ")).toEqual({ text: "- hello", selStart: 4 });
  });

  it("有前缀时移除，光标回退且不越过行首", () => {
    expect(toggleLinePrefix("- hello", 4, "- ")).toEqual({ text: "hello", selStart: 2 });
    expect(toggleLinePrefix("- hello", 1, "- ")).toEqual({ text: "hello", selStart: 0 });
  });

  it("处理待办与引用前缀", () => {
    expect(toggleLinePrefix("任务", 0, "- [ ] ")).toEqual({ text: "- [ ] 任务", selStart: 6 });
    expect(toggleLinePrefix("> 引用", 3, "> ")).toEqual({ text: "引用", selStart: 1 });
  });

  it("只影响光标所在行（多行文本）", () => {
    const doc = "第一行\n第二行\n第三行";
    expect(toggleLinePrefix(doc, 5, "- ")).toEqual({
      text: "第一行\n- 第二行\n第三行",
      selStart: 7,
    });
  });

  it("空行也可添加前缀", () => {
    expect(toggleLinePrefix("a\n\nb", 2, "> ")).toEqual({ text: "a\n> \nb", selStart: 4 });
  });
});

describe("insertAtCursor", () => {
  it("在光标处插入并移动光标", () => {
    expect(insertAtCursor("ab", 1, "XY")).toEqual({ text: "aXYb", selStart: 3 });
  });

  it("位置越界收敛到文本末尾", () => {
    expect(insertAtCursor("ab", 99, "!")).toEqual({ text: "ab!", selStart: 3 });
  });

  it("空文档插入", () => {
    expect(insertAtCursor("", 0, SNIPPETS.heading)).toEqual({ text: SNIPPETS.heading, selStart: 4 });
  });
});

describe("SNIPPETS", () => {
  it("包含全部模板常量", () => {
    expect(SNIPPETS.heading).toBe("\n## ");
    expect(SNIPPETS.divider).toBe("\n---\n");
    expect(SNIPPETS.table).toContain("| --- |");
    expect(SNIPPETS.codeBlock).toContain("```");
    expect(SNIPPETS.link).toBe("[标题](https://)");
  });
});
