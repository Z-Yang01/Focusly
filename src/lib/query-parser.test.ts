import { describe, expect, it } from "vitest";
import {
  formatQuery,
  isQueryEmpty,
  parseSearchQuery,
  tokenizeQuery,
  type SearchQuery,
} from "./query-parser";

/** 除 raw 外逐字段比较（formatQuery 不还原原始输入） */
function expectSameBody(a: SearchQuery, b: SearchQuery) {
  expect({
    freeText: b.freeText,
    tags: b.tags,
    is: b.is,
    has: b.has,
    due: b.due,
  }).toEqual({
    freeText: a.freeText,
    tags: a.tags,
    is: a.is,
    has: a.has,
    due: a.due,
  });
}

describe("tokenizeQuery", () => {
  it("空白分隔；引号内空白不分割", () => {
    expect(tokenizeQuery('会议 tag:#工作 "多 词"')).toEqual(["会议", "tag:#工作", '"多 词"']);
  });

  it("未闭合引号容错：剩余内容合并为一个 token", () => {
    expect(tokenizeQuery('会议 "记录 草稿')).toEqual(["会议", '"记录 草稿']);
  });
});

describe("parseSearchQuery", () => {
  it("空字符串：全空查询，raw 保留", () => {
    const q = parseSearchQuery("");
    expect(q).toEqual({ freeText: "", tags: [], is: [], has: [], due: undefined, raw: "" });
    expect(isQueryEmpty(q)).toBe(true);
  });

  it("纯空白输入等价空查询", () => {
    const q = parseSearchQuery("   \t  ");
    expect(isQueryEmpty(q)).toBe(true);
    expect(q.freeText).toBe("");
  });

  it("纯自由文本：多余空白归一为单空格", () => {
    const q = parseSearchQuery("会议  记录\t下周");
    expect(q.freeText).toBe("会议 记录 下周");
    expect(q.tags).toEqual([]);
    expect(q.raw).toBe("会议  记录\t下周");
  });

  it("tag 带 # 前缀：中文标签", () => {
    const q = parseSearchQuery("tag:#工作");
    expect(q.tags).toEqual(["工作"]);
    expect(q.freeText).toBe("");
  });

  it("tag 不带 # 前缀也可解析", () => {
    expect(parseSearchQuery("tag:学习").tags).toEqual(["学习"]);
  });

  it("多个 tag 去重保序；tag: 空值被忽略", () => {
    const q = parseSearchQuery("tag:#工作 tag:工作 tag:学习 tag:");
    expect(q.tags).toEqual(["工作", "学习"]);
  });

  it("tag 与自由文本混合", () => {
    const q = parseSearchQuery("开会 tag:#工作 重要");
    expect(q.tags).toEqual(["工作"]);
    expect(q.freeText).toBe("开会 重要");
  });

  it("引号短语作为自由文本（保留内部空格）", () => {
    const q = parseSearchQuery('"多 词 短语"');
    expect(q.freeText).toBe("多 词 短语");
  });

  it("引号短语与结构化条件混合", () => {
    const q = parseSearchQuery('"项目 例会" tag:#工作 is:todo');
    expect(q.freeText).toBe("项目 例会");
    expect(q.tags).toEqual(["工作"]);
    expect(q.is).toEqual(["todo"]);
  });

  it("is: 四个合法值；重复去重", () => {
    const q = parseSearchQuery("is:todo is:archived is:private is:pinned is:todo");
    expect(q.is).toEqual(["todo", "archived", "private", "pinned"]);
  });

  it("is: key/value 大小写归一", () => {
    const q = parseSearchQuery("IS:TODO Is:Pinned");
    expect(q.is).toEqual(["todo", "pinned"]);
  });

  it("is: 未知值 → 整个 token 原样归入自由文本", () => {
    const q = parseSearchQuery("is:todo is:done");
    expect(q.is).toEqual(["todo"]);
    expect(q.freeText).toBe("is:done");
  });

  it("has:image has:reminder", () => {
    const q = parseSearchQuery("has:image has:reminder has:image");
    expect(q.has).toEqual(["image", "reminder"]);
  });

  it("due:today / due:week；多次出现取最后一个；未知值归自由文本", () => {
    expect(parseSearchQuery("due:today").due).toBe("today");
    expect(parseSearchQuery("due:today due:week").due).toBe("week");
    const q = parseSearchQuery("due:month");
    expect(q.due).toBeUndefined();
    expect(q.freeText).toBe("due:month");
  });

  it("未知 key 的冒号 token 是自由文本", () => {
    const q = parseSearchQuery("url:https://example.com 说明");
    expect(q.freeText).toBe("url:https://example.com 说明");
  });

  it("引号内的结构化语法不解析", () => {
    const q = parseSearchQuery('"tag:工作"');
    expect(q.tags).toEqual([]);
    expect(q.freeText).toBe("tag:工作");
  });

  it("未闭合引号短语容错", () => {
    const q = parseSearchQuery('会议 "记录 草稿');
    expect(q.freeText).toBe('会议 "记录 草稿');
  });

  it("综合混合：中文 tag + is + has + due + 引号短语", () => {
    const q = parseSearchQuery('周报 tag:#工作 tag:周记 is:todo has:image due:week "每 周"');
    expect(q.tags).toEqual(["工作", "周记"]);
    expect(q.is).toEqual(["todo"]);
    expect(q.has).toEqual(["image"]);
    expect(q.due).toBe("week");
    expect(q.freeText).toBe("周报 每 周");
  });
});

describe("formatQuery", () => {
  it("空查询序列化为空串", () => {
    expect(formatQuery(parseSearchQuery(""))).toBe("");
  });

  it("结构化在前、自由文本在后；自由文本含空格时整体加引号", () => {
    const q = parseSearchQuery('开会 重要 tag:#工作 is:todo "多 词"');
    // 自由文本 token 已合并为单串（单空格连接），整体含空格故整体加引号
    expect(q.freeText).toBe("开会 重要 多 词");
    expect(formatQuery(q)).toBe('tag:#工作 is:todo "开会 重要 多 词"');
  });

  it("含空格的 tag 用引号包裹且保留 #", () => {
    const q = parseSearchQuery('tag:"工作 计划"');
    expect(formatQuery(q)).toBe('tag:"#工作 计划"');
  });

  it("round-trip：format → parse 后除 raw 外字段一致（复杂混合）", () => {
    const input = '周报 tag:#工作 tag:周记 is:todo is:pinned has:image has:reminder due:week "每 周"';
    const first = parseSearchQuery(input);
    expect(isQueryEmpty(first)).toBe(false);
    const second = parseSearchQuery(formatQuery(first));
    expectSameBody(first, second);
  });

  it("round-trip：纯自由文本与纯结构化查询", () => {
    const a = parseSearchQuery("会议  记录");
    expectSameBody(a, parseSearchQuery(formatQuery(a)));
    const b = parseSearchQuery("tag:#工作 is:archived due:today");
    expectSameBody(b, parseSearchQuery(formatQuery(b)));
  });

  it("round-trip：due 序列化与还原", () => {
    const q = parseSearchQuery("due:week");
    expect(formatQuery(q)).toBe("due:week");
    expectSameBody(q, parseSearchQuery(formatQuery(q)));
  });
});
