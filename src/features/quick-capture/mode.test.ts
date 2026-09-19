import { describe, expect, it } from "vitest";
import { tabFromSearch } from "./mode";

describe("tabFromSearch（冷启动初始 tab 契约）", () => {
  it("Rust 新建窗口 URL ?mode=task → 任务页", () => {
    expect(tabFromSearch("?mode=task")).toBe("task");
  });

  it("多参数查询串中仍能解析", () => {
    expect(tabFromSearch("?a=1&mode=task&b=2")).toBe("task");
  });

  it("无参数 / 其他 mode / 未知值一律回落速记页", () => {
    expect(tabFromSearch("")).toBe("write");
    expect(tabFromSearch("?mode=write")).toBe("write");
    expect(tabFromSearch("?mode=taskx")).toBe("write");
    expect(tabFromSearch("?mood=task")).toBe("write");
  });
});
