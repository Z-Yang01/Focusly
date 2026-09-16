import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  describeTime,
  formatLocal,
  fromDatetimeLocal,
  parseRfc3339,
  savedAtText,
  snippetOf,
  toDatetimeLocal,
} from "./format";

/** 固定“现在”：2026-09-16 10:00:00 本地时间（秒/毫秒为 0，远离分支边界） */
const NOW = new Date(2026, 8, 16, 10, 0, 0, 0);

/** 本地构造的 Date → RFC3339 UTC 串 */
const rfc3339 = (d: Date) => d.toISOString();

/** 断言 Date 的本地时间字段 [年, 月(0起), 日, 时, 分] */
const expectLocalYMDHM = (d: Date, parts: [number, number, number, number, number]) => {
  expect([
    d.getFullYear(),
    d.getMonth(),
    d.getDate(),
    d.getHours(),
    d.getMinutes(),
  ]).toEqual(parts);
};

describe("parseRfc3339", () => {
  it("合法 UTC ISO 串 → Date（时间戳与 Date.UTC 一致）", () => {
    const d = parseRfc3339("2026-09-16T02:00:00Z");
    expect(d).not.toBeNull();
    expect(d!.getTime()).toBe(Date.UTC(2026, 8, 16, 2, 0, 0));
  });

  it("非法串 → null", () => {
    expect(parseRfc3339("not-a-date")).toBeNull();
  });

  it("空串 / null / undefined → null", () => {
    expect(parseRfc3339("")).toBeNull();
    expect(parseRfc3339(null)).toBeNull();
    expect(parseRfc3339(undefined)).toBeNull();
  });
});

describe("toDatetimeLocal / fromDatetimeLocal", () => {
  it("toDatetimeLocal：本地字段 + 两位补零", () => {
    expect(toDatetimeLocal(new Date(2026, 8, 16, 9, 5))).toBe("2026-09-16T09:05");
    expect(toDatetimeLocal(new Date(2026, 0, 3, 7, 8))).toBe("2026-01-03T07:08");
  });

  it("fromDatetimeLocal：无时区后缀按本地时间解析", () => {
    const back = new Date(fromDatetimeLocal("2026-09-16T09:05")!);
    expectLocalYMDHM(back, [2026, 8, 16, 9, 5]);
  });

  it("roundtrip：toDatetimeLocal → fromDatetimeLocal → Date 还原本地字段", () => {
    const src = new Date(2026, 8, 16, 9, 5, 0, 0);
    const local = toDatetimeLocal(src);
    const iso = fromDatetimeLocal(local);
    expect(iso).toBe(src.toISOString());
    expectLocalYMDHM(new Date(iso!), [2026, 8, 16, 9, 5]);
  });

  it("fromDatetimeLocal：非法串 → null", () => {
    expect(fromDatetimeLocal("garbage")).toBeNull();
  });
});

describe("describeTime", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("非法输入 → 空串", () => {
    expect(describeTime(null)).toBe("");
    expect(describeTime(undefined)).toBe("");
    expect(describeTime("garbage")).toBe("");
  });

  it("未来 30 分钟 → “30 分钟后”", () => {
    expect(describeTime(rfc3339(new Date(2026, 8, 16, 10, 30)))).toBe("30 分钟后");
  });

  it("未来 59 分钟 → “59 分钟后”（分钟分支上界内）", () => {
    expect(describeTime(rfc3339(new Date(2026, 8, 16, 10, 59)))).toBe("59 分钟后");
  });

  it("未来 2 小时 → “2 小时后”", () => {
    expect(describeTime(rfc3339(new Date(2026, 8, 16, 12, 0)))).toBe("2 小时后");
  });

  it("未来 25 小时 → “1 天后”", () => {
    expect(describeTime(rfc3339(new Date(2026, 8, 17, 11, 0)))).toBe("1 天后");
  });

  it("过去 90 分钟 → 走 formatLocal 分支（同年省略年份）", () => {
    expect(describeTime(rfc3339(new Date(2026, 8, 16, 8, 30)))).toBe("09-16 08:30");
  });

  it("未来 30 天（>7 天）→ 走 formatLocal 分支", () => {
    expect(describeTime(rfc3339(new Date(2026, 9, 16, 10, 0)))).toBe("10-16 10:00");
  });
});

describe("formatLocal", () => {
  beforeEach(() => {
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(NOW);
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("同年 → “MM-DD HH:mm”", () => {
    expect(formatLocal(new Date(2026, 8, 16, 9, 5))).toBe("09-16 09:05");
  });

  it("跨年 → 前缀四位年份", () => {
    expect(formatLocal(new Date(2025, 8, 16, 9, 5))).toBe("2025-09-16 09:05");
  });
});

describe("savedAtText", () => {
  it("时分秒两位补零", () => {
    expect(savedAtText(new Date(2026, 8, 16, 9, 5, 3))).toBe("09:05:03");
  });
});

describe("snippetOf", () => {
  it("剥离标题/加粗/链接语法并压缩空白", () => {
    const md = "# 标题\n\n这是 **加粗** 与 [链接](https://example.com) 混排";
    expect(snippetOf(md)).toBe("标题 这是 加粗 与 链接 混排");
  });

  it("图片语法 → “ [图片]”占位", () => {
    expect(snippetOf("看这个 ![截图](a.png) 就懂了")).toBe("看这个 [图片] 就懂了");
  });

  it("围栏代码块整体剥离", () => {
    expect(snippetOf("前\n```\nconst a = 1;\n```\n后")).toBe("前 后");
  });

  it("行内标记（引用 / 列表 / 行内代码）剥为纯文本", () => {
    expect(snippetOf("> 引言\n- 项目\n- `code` 尾")).toBe("引言 项目 code 尾");
  });

  it("超过 max 截断并追加省略号（默认 80）", () => {
    expect(snippetOf("a".repeat(100))).toBe(`${"a".repeat(80)}…`);
    expect(snippetOf("你好世界", 2)).toBe("你好…");
  });

  it("短文本原样返回", () => {
    expect(snippetOf("纯文本")).toBe("纯文本");
  });
});
