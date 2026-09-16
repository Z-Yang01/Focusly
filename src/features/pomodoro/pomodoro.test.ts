/** 番茄钟纯函数测试：fmtMmss / humanizeSec / computeStreak / heatTier / localDateKey */
import { describe, expect, it } from "vitest";
import {
  computeStreak,
  fmtMmss,
  heatTier,
  humanizeSec,
  localDateKey,
} from "./format";

describe("fmtMmss", () => {
  it("59 → 00:59", () => {
    expect(fmtMmss(59)).toBe("00:59");
  });

  it("3600 → 1:00:00", () => {
    expect(fmtMmss(3600)).toBe("1:00:00");
  });

  it("负数 → 00:00", () => {
    expect(fmtMmss(-5)).toBe("00:00");
  });

  it("0 → 00:00", () => {
    expect(fmtMmss(0)).toBe("00:00");
  });

  it("3599 → 59:59（临界不进位到小时）", () => {
    expect(fmtMmss(3599)).toBe("59:59");
  });

  it("3661 → 1:01:01", () => {
    expect(fmtMmss(3661)).toBe("1:01:01");
  });

  it("1500 → 25:00", () => {
    expect(fmtMmss(1500)).toBe("25:00");
  });

  it("7325 → 2:02:05（小时不补零）", () => {
    expect(fmtMmss(7325)).toBe("2:02:05");
  });

  it("小数向下取整", () => {
    expect(fmtMmss(59.9)).toBe("00:59");
  });
});

describe("humanizeSec", () => {
  it("3725 → 1 小时 2 分钟", () => {
    expect(humanizeSec(3725)).toBe("1 小时 2 分钟");
  });

  it("3600 → 1 小时（整点不带分钟）", () => {
    expect(humanizeSec(3600)).toBe("1 小时");
  });

  it("7500 → 2 小时 5 分钟", () => {
    expect(humanizeSec(7500)).toBe("2 小时 5 分钟");
  });

  it("0 → 0 分钟", () => {
    expect(humanizeSec(0)).toBe("0 分钟");
  });

  it("59 → 0 分钟（不足 1 分钟）", () => {
    expect(humanizeSec(59)).toBe("0 分钟");
  });

  it("61 → 1 分钟", () => {
    expect(humanizeSec(61)).toBe("1 分钟");
  });

  it("负数 → 0 分钟", () => {
    expect(humanizeSec(-100)).toBe("0 分钟");
  });
});

describe("computeStreak", () => {
  const today = "2026-09-16";
  const d = (offset: number): string => {
    const [y, m, day] = today.split("-").map(Number);
    const dt = new Date(Date.UTC(y, m - 1, day));
    dt.setUTCDate(dt.getUTCDate() + offset);
    return dt.toISOString().slice(0, 10);
  };

  it("空列表 → 0", () => {
    expect(computeStreak([], today)).toBe(0);
  });

  it("仅今天 → 1", () => {
    expect(computeStreak([today], today)).toBe(1);
  });

  it("今天 + 昨天 → 2", () => {
    expect(computeStreak([today, d(-1)], today)).toBe(2);
  });

  it("仅昨天 → 1（锚定昨天，今天可延续）", () => {
    expect(computeStreak([d(-1)], today)).toBe(1);
  });

  it("最近一次是前天 → 0（断档）", () => {
    expect(computeStreak([d(-2)], today)).toBe(0);
  });

  it("今天 + 前 3 天连续 → 4", () => {
    expect(computeStreak([today, d(-1), d(-2), d(-3)], today)).toBe(4);
  });

  it("中间断档只算后段", () => {
    expect(computeStreak([today, d(-1), d(-3), d(-4)], today)).toBe(2);
  });

  it("重复日期去重", () => {
    expect(computeStreak([today, today, d(-1), d(-1)], today)).toBe(2);
  });

  it("乱序输入不受影响", () => {
    expect(computeStreak([d(-2), today, d(-1)], today)).toBe(3);
  });

  it("忽略非法日期串", () => {
    expect(computeStreak(["bad-date", today], today)).toBe(1);
  });

  it("忽略 2025-02-31 之类的假日期", () => {
    expect(computeStreak(["2025-02-31", today], today)).toBe(1);
  });

  it("today 缺省用本地当天", () => {
    const now = localDateKey(new Date());
    expect(computeStreak([now])).toBe(1);
  });
});

describe("heatTier", () => {
  it("0 → 档 0", () => {
    expect(heatTier(0)).toBe(0);
  });

  it("1-3 → 档 1", () => {
    expect(heatTier(1)).toBe(1);
    expect(heatTier(3)).toBe(1);
  });

  it("4-7 → 档 2", () => {
    expect(heatTier(4)).toBe(2);
    expect(heatTier(7)).toBe(2);
  });

  it("≥8 → 档 3", () => {
    expect(heatTier(8)).toBe(3);
    expect(heatTier(50)).toBe(3);
  });

  it("负数 → 档 0", () => {
    expect(heatTier(-2)).toBe(0);
  });
});

describe("localDateKey", () => {
  it("按本地时区生成 YYYY-MM-DD", () => {
    const d = new Date(2026, 8, 16, 23, 59);
    expect(localDateKey(d)).toBe("2026-09-16");
  });

  it("单月单位数补零", () => {
    expect(localDateKey(new Date(2026, 0, 5))).toBe("2026-01-05");
  });
});
