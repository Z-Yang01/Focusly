import { describe, expect, it } from "vitest";
import { assignTaskKeys, fnv1a32, normalizeTaskText } from "./taskKey";

describe("normalizeTaskText", () => {
  it("trim + 合并连续空白", () => {
    expect(normalizeTaskText("  写周报   今天 \t完成 ")).toBe("写周报 今天 完成");
  });
  it("空串安全", () => {
    expect(normalizeTaskText("   ")).toBe("");
  });
});

describe("fnv1a32", () => {
  it("已知向量：空串 = 811c9dc5", () => {
    expect(fnv1a32("")).toBe("811c9dc5");
  });
  it("确定性且 8 位十六进制", () => {
    const a = fnv1a32("写周报");
    expect(a).toMatch(/^[0-9a-f]{8}$/);
    expect(fnv1a32("写周报")).toBe(a);
  });
  it("不同文本不同哈希（基本区分度）", () => {
    expect(fnv1a32("任务A")).not.toBe(fnv1a32("任务B"));
  });
});

describe("assignTaskKeys", () => {
  it("同文本任务按出现序区分，互不相同", () => {
    const todos = [{ text: "站会" }, { text: "其他" }, { text: "站会" }];
    const keys = assignTaskKeys(todos).map((t) => t.taskKey);
    const [hash0, occ0] = keys[0].split("-");
    const [hash2, occ2] = keys[2].split("-");
    expect(hash0).toBe(hash2, "同文本同哈希");
    expect(occ0).toBe("0");
    expect(occ2).toBe("1");
    expect(keys[1]).not.toBe(keys[0]);
  });
  it("其他行增删不影响既有 key（行内文本稳定）", () => {
    const before = assignTaskKeys([{ text: "A" }, { text: "B" }]).map((t) => t.taskKey);
    const after = assignTaskKeys([{ text: "新插入" }, { text: "A" }, { text: "B" }]).map(
      (t) => t.taskKey,
    );
    expect(after.slice(1)).toEqual(before);
  });
  it("文本规范化后等价的任务 key 一致", () => {
    const [a] = assignTaskKeys([{ text: "写   周报" }]);
    const [b] = assignTaskKeys([{ text: "写 周报" }]);
    expect(a.taskKey).toBe(b.taskKey);
  });
  it("保留输入字段", () => {
    const out = assignTaskKeys([{ text: "T", line: 3, checked: false, indent: 2 }]);
    expect(out[0]).toMatchObject({ text: "T", line: 3, checked: false, indent: 2 });
  });
});
