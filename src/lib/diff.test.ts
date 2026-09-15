import { describe, expect, it } from "vitest";
import { diffLines, diffStats } from "./diff";

describe("diffLines", () => {
  it("完全相同的文本全部为 same，行号一一对应", () => {
    const diff = diffLines("a\nb\nc", "a\nb\nc");
    expect(diff).toEqual([
      { kind: "same", text: "a", aLine: 0, bLine: 0 },
      { kind: "same", text: "b", aLine: 1, bLine: 1 },
      { kind: "same", text: "c", aLine: 2, bLine: 2 },
    ]);
  });

  it("纯新增：b 多出的行标记为 add", () => {
    const diff = diffLines("a\nc", "a\nb\nc");
    expect(diff).toEqual([
      { kind: "same", text: "a", aLine: 0, bLine: 0 },
      { kind: "add", text: "b", bLine: 1 },
      { kind: "same", text: "c", aLine: 1, bLine: 2 },
    ]);
  });

  it("纯删除：a 独有的行标记为 remove", () => {
    const diff = diffLines("a\nb\nc", "a\nc");
    expect(diff).toEqual([
      { kind: "same", text: "a", aLine: 0, bLine: 0 },
      { kind: "remove", text: "b", aLine: 1 },
      { kind: "same", text: "c", aLine: 2, bLine: 1 },
    ]);
  });

  it("修改一行 → remove + add，其余保持 same", () => {
    const diff = diffLines("line1\nold\nline3", "line1\nnew\nline3");
    expect(diff.map((d) => d.kind)).toEqual(["same", "remove", "add", "same"]);
    expect(diff[1]).toEqual({ kind: "remove", text: "old", aLine: 1 });
    expect(diff[2]).toEqual({ kind: "add", text: "new", bLine: 1 });
  });

  it("空串 vs 有内容：空串按一行空行处理", () => {
    expect(diffLines("", "hello")).toEqual([
      { kind: "remove", text: "", aLine: 0 },
      { kind: "add", text: "hello", bLine: 0 },
    ]);
  });

  it("有内容 vs 空串：全部删除并补一行空行 add", () => {
    const diff = diffLines("hello", "");
    expect(diff[0]).toEqual({ kind: "remove", text: "hello", aLine: 0 });
    expect(diffStats(diff)).toEqual({ added: 1, removed: 1 });
  });

  it("两个空串 → 单行 same 空行", () => {
    expect(diffLines("", "")).toEqual([{ kind: "same", text: "", aLine: 0, bLine: 0 }]);
  });

  it("中文内容正确 diff", () => {
    const diff = diffLines("第一天\n写代码", "第一天\n读代码");
    expect(diff).toEqual([
      { kind: "same", text: "第一天", aLine: 0, bLine: 0 },
      { kind: "remove", text: "写代码", aLine: 1 },
      { kind: "add", text: "读代码", bLine: 1 },
    ]);
  });

  it("结尾换行差异 → b 多一个空行 add", () => {
    expect(diffLines("a", "a\n")).toEqual([
      { kind: "same", text: "a", aLine: 0, bLine: 0 },
      { kind: "add", text: "", bLine: 1 },
    ]);
  });

  it("空行参与比较：删除中间空行", () => {
    const diff = diffLines("a\n\nb", "a\nb");
    expect(diff).toEqual([
      { kind: "same", text: "a", aLine: 0, bLine: 0 },
      { kind: "remove", text: "", aLine: 1 },
      { kind: "same", text: "b", aLine: 2, bLine: 1 },
    ]);
  });

  it("多行改动走 LCS 最长公共子序列", () => {
    const diff = diffLines("x\nkeep\ny", "keep\nz");
    expect(diff.map((d) => `${d.kind}:${d.text}`).join(",")).toBe(
      "remove:x,same:keep,remove:y,add:z",
    );
  });
});

describe("diffStats", () => {
  it("分别统计 add 与 remove 行数", () => {
    const diff = diffLines("a\nb", "a\nc\nd");
    expect(diffStats(diff)).toEqual({ added: 2, removed: 1 });
  });

  it("无差异时为 0", () => {
    expect(diffStats(diffLines("相同\n文本", "相同\n文本"))).toEqual({ added: 0, removed: 0 });
  });
});
