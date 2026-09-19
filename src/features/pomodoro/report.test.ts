import { describe, expect, it } from "vitest";
import { barPct, dayIndexInRange, reportRange, REASON_LABEL } from "./report";

describe("reportRange", () => {
  // 2026-09-16 是周三
  const now = new Date(2026, 8, 16, 12, 0, 0);

  it("本周：周一至周日", () => {
    const r = reportRange("week", 0, now);
    expect(r.start).toBe("2026-09-14");
    expect(r.end).toBe("2026-09-20");
    expect(r.label).toBe("09-14 ~ 09-20");
  });

  it("上周偏移 -1", () => {
    const r = reportRange("week", -1, now);
    expect(r.start).toBe("2026-09-07");
    expect(r.end).toBe("2026-09-13");
  });

  it("本月：自然月整月", () => {
    const r = reportRange("month", 0, now);
    expect(r.start).toBe("2026-09-01");
    expect(r.end).toBe("2026-09-30");
    expect(r.label).toBe("9 月");
  });

  it("上月偏移 -1（2 月闰年边界由 Date 自行处理）", () => {
    const r = reportRange("month", -1, now);
    expect(r.start).toBe("2026-08-01");
    expect(r.end).toBe("2026-08-31");
  });

  it("周日起点也归入本周（周一为一周起点）", () => {
    const sunday = new Date(2026, 8, 20, 8, 0, 0);
    const r = reportRange("week", 0, sunday);
    expect(r.start).toBe("2026-09-14");
    expect(r.end).toBe("2026-09-20");
  });
});

describe("dayIndexInRange", () => {
  const range = { start: "2026-09-14", end: "2026-09-20", label: "" };

  it("区间首尾与中间", () => {
    expect(dayIndexInRange("2026-09-14", range)).toBe(0);
    expect(dayIndexInRange("2026-09-16", range)).toBe(2);
    expect(dayIndexInRange("2026-09-20", range)).toBe(6);
  });

  it("区间外 / 非法日期为 -1", () => {
    expect(dayIndexInRange("2026-09-21", range)).toBe(-1);
    expect(dayIndexInRange("bad", range)).toBe(-1);
  });
});

describe("barPct", () => {
  it("常规比例", () => {
    expect(barPct(50, 100)).toBe(50);
    expect(barPct(1500, 3000)).toBe(50);
    expect(barPct(1, 3)).toBe(33.3);
  });

  it("边界：0 / 超出 / 非法 max", () => {
    expect(barPct(0, 100)).toBe(0);
    expect(barPct(120, 100)).toBe(120);
    expect(barPct(10, 0)).toBe(0);
    expect(barPct(10, Number.NaN)).toBe(0);
  });
});

describe("REASON_LABEL", () => {
  it("覆盖全部归一原因", () => {
    for (const r of ["manual", "skip", "task_done", "app_exit", "switch_task", "other"]) {
      expect(REASON_LABEL[r]).toBeTruthy();
    }
  });
});
