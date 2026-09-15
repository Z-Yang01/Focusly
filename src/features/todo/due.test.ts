import { describe, expect, it } from "vitest";
import {
  isOverdue,
  parseTaskLine,
  setTaskDue,
  setTaskPriority,
  tasksDueBetween,
} from "./due";

describe("parseTaskLine", () => {
  it("解析裸文本上的到期日", () => {
    expect(parseTaskLine("买牛奶 ^2026-09-20")).toEqual({
      text: "买牛奶",
      due: "2026-09-20",
    });
  });

  it("解析完整任务行：复选框前缀 + 到期日 + 优先级", () => {
    expect(parseTaskLine("- [ ] 交报告 ^2026-09-20 !高")).toEqual({
      text: "交报告",
      due: "2026-09-20",
      priority: "高",
    });
    expect(parseTaskLine("* [x] 已完成 ^2026-09-01 !中").priority).toBe("中");
    expect(parseTaskLine("+ [ ] 写周报 !低").priority).toBe("低");
  });

  it("非法 ^日期 视为普通文本保留", () => {
    const r = parseTaskLine("买牛奶 ^2026-13-40");
    expect(r.due).toBeUndefined();
    expect(r.text).toContain("^2026-13-40");
  });

  it("多个 ^日期 取最后一个且全部摘除", () => {
    const r = parseTaskLine("任务 ^2026-09-01 中段 ^2026-10-05 尾部");
    expect(r.due).toBe("2026-10-05");
    expect(r.text).toBe("任务 中段 尾部");
  });

  it("无标记时只返回 text", () => {
    expect(parseTaskLine("普通任务")).toEqual({ text: "普通任务" });
  });
});

describe("setTaskDue", () => {
  it("追加到期日并保证文本后有空格", () => {
    expect(setTaskDue("买牛奶", "2026-09-20")).toBe("买牛奶 ^2026-09-20");
    expect(setTaskDue("买牛奶 ", "2026-09-20")).toBe("买牛奶 ^2026-09-20");
  });

  it("替换已有到期日", () => {
    expect(setTaskDue("交报告 ^2026-09-01", "2026-09-20")).toBe("交报告 ^2026-09-20");
  });

  it("传入空值移除到期日", () => {
    expect(setTaskDue("交报告 ^2026-09-01", undefined)).toBe("交报告");
  });

  it("保留复选框前缀与优先级标记", () => {
    expect(setTaskDue("- [ ] 交报告 ^2026-09-01 !高", "2026-09-20")).toBe(
      "- [ ] 交报告 !高 ^2026-09-20",
    );
  });

  it("非法日期串不追加", () => {
    expect(setTaskDue("买牛奶", "2026-9-9")).toBe("买牛奶");
  });
});

describe("setTaskPriority", () => {
  it("追加与替换优先级", () => {
    expect(setTaskPriority("买牛奶", "高")).toBe("买牛奶 !高");
    expect(setTaskPriority("买牛奶 !高", "低")).toBe("买牛奶 !低");
  });

  it("空值移除优先级，保留到期日", () => {
    expect(setTaskPriority("交报告 ^2026-09-20 !高", undefined)).toBe("交报告 ^2026-09-20");
  });
});

describe("tasksDueBetween / isOverdue", () => {
  const items = [
    { text: "逾期任务 ^2026-09-10", checked: false },
    { text: "今日任务 ^2026-09-15", checked: false },
    { text: "本周任务 ^2026-09-18", checked: false },
    { text: "已勾选 ^2026-09-15", checked: true },
    { text: "无日期任务", checked: false },
  ];

  it("闭区间过滤（含起止日），已勾选与无日期排除", () => {
    expect(tasksDueBetween(items, "2026-09-15", "2026-09-18").map((t) => t.text)).toEqual([
      "今日任务 ^2026-09-15",
      "本周任务 ^2026-09-18",
    ]);
    expect(tasksDueBetween(items, "2026-09-16", "2026-09-17")).toHaveLength(0);
  });

  it("isOverdue：到期早于今天且未勾选", () => {
    expect(isOverdue(items[0], "2026-09-15")).toBe(true);
    expect(isOverdue(items[1], "2026-09-15")).toBe(false);
    // 已勾选不算逾期
    expect(isOverdue(items[3], "2026-09-15")).toBe(false);
    // 无日期不算逾期
    expect(isOverdue(items[4], "2026-09-15")).toBe(false);
  });
});
