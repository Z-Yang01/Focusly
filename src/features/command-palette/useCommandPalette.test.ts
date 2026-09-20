import { describe, expect, it, vi } from "vitest";

/** lib/api 是 Tauri invoke 封装层，node 环境不可用；面板过滤为纯函数，桩掉即可 */
vi.mock("@/lib/api", () => ({
  hideAllNotes: vi.fn(),
  newNote: vi.fn(),
  openNoteWindow: vi.fn(),
  pomodoroReportToNote: vi.fn(),
  quickCaptureToggle: vi.fn(),
  setSetting: vi.fn(),
  showAllNotes: vi.fn(),
  toggleAllNotes: vi.fn(),
}));

import { filterCommands } from "./useCommandPalette";

const ids = (input: string) => filterCommands(input).map((c) => c.id);

describe("filterCommands", () => {
  it("空输入返回全部静态命令，且不含动态搜索项", () => {
    const all = filterCommands("");
    expect(all.length).toBeGreaterThanOrEqual(15);
    expect(all.some((c) => c.id === "search.current")).toBe(false);
  });

  it("非空输入时动态搜索命令居首", () => {
    expect(ids("任意输入")[0]).toBe("search.current");
  });

  it("拼音首字母可命中（速记箱/今日计划/回收站/已归档/私密空间）", () => {
    expect(ids("sjx")).toContain("capture.quick");
    expect(ids("jrjh")).toContain("nav.plan");
    expect(ids("hsz")).toContain("nav.trash");
    expect(ids("ygd")).toContain("nav.archived");
    expect(ids("smkj")).toContain("nav.private");
  });

  it("中文关键词命中新增命令", () => {
    expect(ids("番茄统计")).toContain("stats.pomodoro");
    expect(ids("周报")).toContain("report.week");
    expect(ids("快速捕获")).toContain("capture.quick");
  });

  it("多词按 AND 过滤", () => {
    const hit = ids("theme 浅色");
    expect(hit).toContain("theme.light");
    expect(hit).not.toContain("theme.dark");
  });

  it("大小写不敏感（title/keywords/group 统一小写匹配）", () => {
    const hit = ids("THEME 浅色");
    expect(hit).toContain("theme.light");
    expect(hit).not.toContain("theme.dark");
  });

  it("多空白与首尾空白不改变结果（trim + 按空白拆词）", () => {
    expect(ids("  theme    浅色  ")).toEqual(ids("theme 浅色"));
  });

  it("同一输入多次过滤结果稳定（纯函数，无动态顺序）", () => {
    expect(ids("速记")).toEqual(ids("速记"));
  });

  it("无法命中的输入只剩搜索命令", () => {
    expect(ids("zzqxqq")).toEqual(["search.current"]);
  });
});
