/** 勿扰时段纯逻辑单测（对应 src-tauri/src/dnd.rs 的同规则 Rust 单测） */
import { describe, expect, it } from "vitest";
import {
  DEFAULT_END,
  DEFAULT_START,
  DND_PRESETS,
  hmToMin,
  isCrossMidnight,
  isDndEnabled,
  isValidHm,
  minToHm,
  nudgeWindow,
  shiftHm,
  STEP_MIN,
} from "./dnd";

describe("isValidHm", () => {
  it("接受合法 HH:MM", () => {
    expect(isValidHm("00:00")).toBe(true);
    expect(isValidHm("07:00")).toBe(true);
    expect(isValidHm("22:30")).toBe(true);
    expect(isValidHm("23:59")).toBe(true);
  });

  it("拒绝空串与缺失", () => {
    expect(isValidHm("")).toBe(false);
  });

  it("拒绝非两位与越界", () => {
    expect(isValidHm("7:00")).toBe(false);
    expect(isValidHm("22:5")).toBe(false);
    expect(isValidHm("24:00")).toBe(false);
    expect(isValidHm("12:60")).toBe(false);
  });

  it("拒绝非数字与非冒号分隔", () => {
    expect(isValidHm("ab:cd")).toBe(false);
    expect(isValidHm("22-00")).toBe(false);
    expect(isValidHm("2200")).toBe(false);
  });
});

describe("isDndEnabled", () => {
  it("起止均合法且不同 = 启用", () => {
    expect(isDndEnabled("22:00", "07:00")).toBe(true);
    expect(isDndEnabled("13:00", "15:00")).toBe(true);
  });

  it("start==end 视为未启用（与 Rust parse_window 一致）", () => {
    expect(isDndEnabled("13:00", "13:00")).toBe(false);
    expect(isDndEnabled("00:00", "00:00")).toBe(false);
  });

  it("任一端非法/为空 = 未启用", () => {
    expect(isDndEnabled("", "")).toBe(false);
    expect(isDndEnabled("", "07:00")).toBe(false);
    expect(isDndEnabled("22:00", "")).toBe(false);
    expect(isDndEnabled("25:00", "07:00")).toBe(false);
  });
});

describe("isCrossMidnight", () => {
  it("start > end 即跨午夜", () => {
    expect(isCrossMidnight("22:00", "07:00")).toBe(true);
    expect(isCrossMidnight("23:59", "00:00")).toBe(true);
  });

  it("常规窗口与未启用均不算跨午夜", () => {
    expect(isCrossMidnight("13:00", "15:00")).toBe(false);
    expect(isCrossMidnight("13:00", "13:00")).toBe(false);
    expect(isCrossMidnight("", "")).toBe(false);
  });
});

describe("默认值", () => {
  it("默认窗口本身合法且跨午夜", () => {
    expect(isValidHm(DEFAULT_START)).toBe(true);
    expect(isValidHm(DEFAULT_END)).toBe(true);
    expect(isDndEnabled(DEFAULT_START, DEFAULT_END)).toBe(true);
    expect(isCrossMidnight(DEFAULT_START, DEFAULT_END)).toBe(true);
  });
});

describe("hmToMin / minToHm", () => {
  it("互转一致", () => {
    expect(hmToMin("00:00")).toBe(0);
    expect(hmToMin("22:30")).toBe(1350);
    expect(hmToMin("23:59")).toBe(1439);
    expect(hmToMin("7:00")).toBeNull();
    expect(minToHm(0)).toBe("00:00");
    expect(minToHm(1350)).toBe("22:30");
    expect(minToHm(1439)).toBe("23:59");
  });

  it("minToHm 对 24h 取模回绕（含负数）", () => {
    expect(minToHm(1440)).toBe("00:00");
    expect(minToHm(1455)).toBe("00:15");
    expect(minToHm(-15)).toBe("23:45");
    expect(minToHm(-1440)).toBe("00:00");
  });
});

describe("shiftHm（步进拨动）", () => {
  it("常规拨近/拨远", () => {
    expect(shiftHm("22:00", 15)).toBe("22:15");
    expect(shiftHm("22:00", -15)).toBe("21:45");
  });

  it("跨午夜回绕", () => {
    expect(shiftHm("23:59", 1)).toBe("00:00");
    expect(shiftHm("00:00", -1)).toBe("23:59");
  });

  it("非法起点原样返回（调用方先校验）", () => {
    expect(shiftHm("", 15)).toBe("");
    expect(shiftHm("7:00", 15)).toBe("7:00");
  });
});

describe("DND_PRESETS", () => {
  it("每个预设都是可启用的合法窗口", () => {
    for (const p of DND_PRESETS) {
      expect(isDndEnabled(p.start, p.end)).toBe(true);
    }
  });
});

describe("nudgeWindow（步进决策）", () => {
  it("合法拨动返回新窗口，另一侧不动", () => {
    const r = nudgeWindow({ start: "22:00", end: "07:00" }, "start", -STEP_MIN);
    expect(r).toEqual({ ok: true, window: { start: "21:45", end: "07:00" } });
  });

  it("跨午夜回绕", () => {
    const r = nudgeWindow({ start: "23:59", end: "08:00" }, "start", STEP_MIN);
    expect(r).toEqual({ ok: true, window: { start: "00:14", end: "08:00" } });
  });

  it("拨动后起止相同 → 拒绝并给出提示文案", () => {
    const bad = nudgeWindow({ start: "13:00", end: "13:15" }, "end", -STEP_MIN);
    expect(bad.ok).toBe(false);
    expect(bad.ok === false && bad.reason).toContain("起止不能相同");
    const bad2 = nudgeWindow({ start: "13:00", end: "13:15" }, "start", STEP_MIN);
    expect(bad2.ok).toBe(false);
  });

  it("非法输入（空串）→ 拒绝，不误开勿扰", () => {
    expect(nudgeWindow({ start: "", end: "" }, "start", STEP_MIN).ok).toBe(false);
  });
});
