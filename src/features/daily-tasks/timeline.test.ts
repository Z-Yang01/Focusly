import { describe, expect, it } from "vitest";
import {
  addDays,
  currentLinePercent,
  hhmmToMinutes,
  lanePack,
  localDateKey,
  minutesToHhmm,
  timelineRange,
  weekdayLabel,
  yToSnappedMinutes,
} from "./timeline";

describe("hhmmToMinutes / minutesToHhmm", () => {
  it("解析合法 HH:MM", () => {
    expect(hhmmToMinutes("09:30")).toBe(570);
    expect(hhmmToMinutes("00:00")).toBe(0);
    expect(hhmmToMinutes("23:59")).toBe(1439);
  });
  it("非法输入返回 null", () => {
    expect(hhmmToMinutes(null)).toBeNull();
    expect(hhmmToMinutes("")).toBeNull();
    expect(hhmmToMinutes("9:30")).toBeNull();
    expect(hhmmToMinutes("24:00")).toBeNull();
    expect(hhmmToMinutes("12:60")).toBeNull();
  });
  it("往返转换与越界夹紧", () => {
    expect(minutesToHhmm(570)).toBe("09:30");
    expect(minutesToHhmm(-5)).toBe("00:00");
    expect(minutesToHhmm(1500)).toBe("23:59");
  });
});

describe("timelineRange", () => {
  it("无任务时用默认 09:00-21:00", () => {
    expect(timelineRange([], [])).toEqual({ startMin: 540, endMin: 1260 });
  });
  it("任务超出默认范围时外扩 30 分钟", () => {
    const r = timelineRange([7 * 60], [22 * 60]);
    expect(r.startMin).toBe(6 * 60 + 30);
    expect(r.endMin).toBe(22 * 60 + 30);
  });
  it("end <= start 时保证 1 小时", () => {
    const r = timelineRange([], [], 10 * 60, 9 * 60);
    expect(r.endMin - r.startMin).toBeGreaterThanOrEqual(60);
  });
});

describe("yToSnappedMinutes", () => {
  it("吸附到 15 分钟", () => {
    const range = { startMin: 9 * 60 };
    expect(yToSnappedMinutes(0, range, 60)).toBe(540);
    expect(yToSnappedMinutes(60, range, 60)).toBe(600);
    expect(yToSnappedMinutes(31, range, 60)).toBe(570); // 31px ≈ 571min → 570 吸附
    expect(yToSnappedMinutes(0, range, 60, 5)).toBe(540);
  });
});

describe("lanePack", () => {
  it("重叠任务进不同 lane，不重叠复用同 lane", () => {
    const lanes = lanePack([
      { id: "a", start: 540, end: 600 },
      { id: "b", start: 540, end: 660 }, // 与 a 重叠
      { id: "c", start: 600, end: 660 }, // a 结束后，复用 lane0
      { id: "d", start: 620, end: 700 }, // 与 b(540-660) 重叠 → 新 lane
    ]);
    expect(lanes.get("a")).toBe(0);
    expect(lanes.get("b")).toBe(1);
    expect(lanes.get("c")).toBe(0);
    expect(lanes.get("d")).toBe(2);
  });
  it("相接（前一结束=后一开始）不算重叠", () => {
    const lanes = lanePack([
      { id: "a", start: 540, end: 600 },
      { id: "b", start: 600, end: 660 },
    ]);
    expect(lanes.get("b")).toBe(0);
  });
});

describe("currentLinePercent", () => {
  const range = { startMin: 540, endMin: 1260 };
  it("范围内返回 0..1", () => {
    expect(currentLinePercent(540, range)).toBe(0);
    expect(currentLinePercent(900, range)).toBeCloseTo(0.5);
    expect(currentLinePercent(1260, range)).toBe(1);
  });
  it("范围外返回 null", () => {
    expect(currentLinePercent(100, range)).toBeNull();
    expect(currentLinePercent(1300, range)).toBeNull();
  });
});

describe("date helpers", () => {
  it("localDateKey/addDays/weekdayLabel", () => {
    expect(localDateKey(new Date(2026, 8, 18))).toBe("2026-09-18");
    expect(addDays("2026-09-18", 1)).toBe("2026-09-19");
    expect(addDays("2026-09-01", -1)).toBe("2026-08-31");
    expect(weekdayLabel("2026-09-18")).toBe("周五");
  });
});
