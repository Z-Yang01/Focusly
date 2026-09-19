import { describe, expect, it } from "vitest";
import { applyTaskOrder, mergeOrderedKeys, moveKey } from "./todoOrder";

describe("mergeOrderedKeys", () => {
  it("存储序在前，新 key 按内容顺序追加", () => {
    expect(mergeOrderedKeys(["a", "b", "c"], ["c", "a"])).toEqual(["c", "a", "b"]);
  });

  it("已删除的存储 key 被忽略", () => {
    expect(mergeOrderedKeys(["a", "b"], ["x", "b", "x2"])).toEqual(["b", "a"]);
  });

  it("无存储序时保持内容顺序", () => {
    expect(mergeOrderedKeys(["b", "a"], [])).toEqual(["b", "a"]);
  });

  it("重复存储 key 只保留一次", () => {
    expect(mergeOrderedKeys(["a", "b"], ["a", "a", "b"])).toEqual(["a", "b"]);
  });
});

describe("moveKey", () => {
  const keys = ["a", "b", "c", "d"];

  it("移动到目标之前", () => {
    expect(moveKey(keys, "d", "b", true)).toEqual(["a", "d", "b", "c"]);
  });

  it("移动到目标之后", () => {
    expect(moveKey(keys, "a", "c", false)).toEqual(["b", "c", "a", "d"]);
  });

  it("相邻交换（before）", () => {
    expect(moveKey(keys, "b", "a", true)).toEqual(["b", "a", "c", "d"]);
  });

  it("同位置 / 缺失 key 返回副本", () => {
    expect(moveKey(keys, "b", "b", true)).toEqual(keys);
    expect(moveKey(keys, "x", "a", true)).toEqual(keys);
    expect(moveKey(keys, "a", "x", true)).toEqual(keys);
    expect(keys).toEqual(["a", "b", "c", "d"]);
  });
});

describe("applyTaskOrder", () => {
  it("无存储序时内容原样（identity）", () => {
    const content = "- [ ] a\n- [ ] b\n";
    const r = applyTaskOrder(content, []);
    expect(r.content).toBe(content);
    expect(r.displayKeyOrder.length).toBe(2);
  });

  it("按存储序换位：文本在任务行槽位间交换，行数不变", () => {
    const content = "- [ ] 甲\n- [ ] 乙\n- [ ] 丙";
    const keys = applyTaskOrder(content, []).displayKeyOrder; // [甲, 乙, 丙] 内容序
    const r = applyTaskOrder(content, [keys[2], keys[0], keys[1]]);
    const lines = r.content.split("\n");
    expect(lines[0]).toContain("丙");
    expect(lines[1]).toContain("甲");
    expect(lines[2]).toContain("乙");
    expect(r.content.split("\n").length).toBe(3);
  });

  it("keyToOriginalLine 指向原始行号，勾选回写映射正确", () => {
    const content = "前言\n- [ ] 甲\n- [ ] 乙";
    const r0 = applyTaskOrder(content, []);
    const keyJia = r0.displayKeyOrder[0];
    const r = applyTaskOrder(content, [r0.displayKeyOrder[1], keyJia]);
    // 展示正文第 1 行（0-based）现在是"乙"，但 key→原始行映射：乙=2，甲=1
    expect(r.originalLineByKey.get(keyJia)).toBe(1);
    const keyYi = r0.displayKeyOrder[1];
    expect(r.originalLineByKey.get(keyYi)).toBe(2);
  });

  it("非任务行不参与重排；空行分隔的池独立重排", () => {
    const content = "# 标题\n- [ ] 甲\n\n段落文字\n- [ ] 乙\n- [ ] 丙";
    const keys = applyTaskOrder(content, []).displayKeyOrder; // [甲, 乙, 丙]
    const r = applyTaskOrder(content, [keys[2], keys[1], keys[0]]);
    const lines = r.content.split("\n");
    expect(lines[0]).toBe("# 标题");
    // 甲所在池只有自身（空行+段落分隔），无法被移动，保持原位
    expect(lines[1]).toContain("甲");
    expect(lines[2]).toBe("");
    expect(lines[3]).toBe("段落文字");
    // 乙丙池内按存储序交换
    expect(lines[4]).toContain("丙");
    expect(lines[5]).toContain("乙");
  });

  it("带续行/嵌套的任务项不可拖，同池的简单项仍可重排", () => {
    const content = "- [ ] 甲\n  - 子项\n- [ ] 乙\n- [ ] 丙";
    const r0 = applyTaskOrder(content, []);
    const keys = r0.displayKeyOrder; // [甲, 乙, 丙]
    expect(r0.reorderableKeys.has(keys[0])).toBe(false); // 甲带嵌套子项
    expect(r0.reorderableKeys.has(keys[1])).toBe(true);
    expect(r0.reorderableKeys.has(keys[2])).toBe(true);
    // 存储序把甲放最后：甲不可拖保持原位，乙丙在自身槽位不变（相对序未变）
    const r = applyTaskOrder(content, [keys[1], keys[2], keys[0]]);
    const lines = r.content.split("\n");
    expect(lines[0]).toContain("甲");
    expect(lines[1]).toContain("子项");
    expect(lines[2]).toContain("乙");
    expect(lines[3]).toContain("丙");
    expect(r.content.split("\n").length).toBe(4);
  });

  it("勾选项 [x] 同样参与重排", () => {
    const content = "- [ ] 甲\n- [x] 乙";
    const keys = applyTaskOrder(content, []).displayKeyOrder;
    const r = applyTaskOrder(content, [keys[1], keys[0]]);
    expect(r.content.split("\n")[0]).toContain("乙");
    expect(r.content.split("\n")[0]).toContain("[x]");
    expect(r.content.split("\n")[1]).toContain("甲");
  });

  it("存储序只含部分 key 时，其余按内容顺序追加", () => {
    const content = "- [ ] 甲\n- [ ] 乙\n- [ ] 丙";
    const keys = applyTaskOrder(content, []).displayKeyOrder;
    const r = applyTaskOrder(content, [keys[2]]);
    expect(r.content.split("\n")[0]).toContain("丙");
    expect(r.content.split("\n")[1]).toContain("甲");
    expect(r.content.split("\n")[2]).toContain("乙");
  });

  it("重复文本任务：key 按原序派生且拖动后保持稳定（回写不错行）", () => {
    const content = "- [ ] 买牛奶\n- [ ] 买牛奶\n- [ ] 别的";
    const r0 = applyTaskOrder(content, []);
    // 同文本 key 出现序号不同
    expect(new Set(r0.displayKeyOrder).size).toBe(3);
    const [k1, k2, k3] = r0.displayKeyOrder;
    // 把第 1 个"买牛奶"拖到最后：key → 展示行映射随之移动
    const r = applyTaskOrder(content, [k2, k3, k1]);
    expect(r.displayLineByKey.get(k1)).toBe(2);
    expect(r.displayLineByKey.get(k2)).toBe(0);
    // 原文行映射永不因拖动改变（回写锚点）
    expect(r.originalLineByKey.get(k1)).toBe(0);
    expect(r.originalLineByKey.get(k2)).toBe(1);
    // 展示正文行 0 现在是第二个"买牛奶"（k2），其回写行是 1
    expect(r.content.split("\n")[0]).toContain("买牛奶");
  });

  it("单元素池不可拖（reorderableKeys 不含），跨池拖动可被 poolByKey 拒绝", () => {
    const content = "# 标题\n- [ ] 甲\n\n段落\n- [ ] 乙\n- [ ] 丙";
    const r = applyTaskOrder(content, []);
    expect(r.reorderableKeys.size).toBe(2); // 乙丙池可拖；甲单元素池不可
    const poolJia = r.poolByKey.get(r.displayKeyOrder[0]);
    const poolYi = r.poolByKey.get(r.displayKeyOrder[1]);
    expect(poolJia).not.toBe(poolYi);
    expect(r.poolByKey.get(r.displayKeyOrder[2])).toBe(poolYi);
  });
});
