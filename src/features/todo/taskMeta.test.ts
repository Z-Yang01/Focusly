import { describe, expect, it } from "vitest";
import { effectiveStatus, isNoopMeta, mergeTaskMeta } from "./taskMeta";

const mkMeta = (over: Partial<Parameters<typeof mergeTaskMeta>[1][number]> & { taskKey: string }) => ({
  lineText: over.taskKey,
  status: "todo",
  skipDate: null,
  ...over,
});

const todos = [
  { taskKey: "aaaa-0", line: 0, text: "写周报", checked: false },
  { taskKey: "bbbb-0", line: 1, text: "买牛奶", checked: true },
  { taskKey: "cccc-0", line: 2, text: "休息", checked: false },
];

describe("mergeTaskMeta", () => {
  it("无 meta 全为 markdown 状态", () => {
    const r = mergeTaskMeta(todos, [], "2026-09-16");
    expect(r.tasks.map((t) => t.status)).toEqual(["todo", "done", "todo"]);
    expect(r.orphans).toHaveLength(0);
  });

  it("skipped 生效显示 ✖️", () => {
    const r = mergeTaskMeta(
      todos,
      [mkMeta({ taskKey: "aaaa-0", status: "skipped", skipDate: "2026-09-16" })],
      "2026-09-16",
    );
    expect(r.tasks[0].status).toBe("skipped");
  });

  it("跨日 skip 自动复活为 todo", () => {
    const r = mergeTaskMeta(
      todos,
      [mkMeta({ taskKey: "aaaa-0", status: "skipped", skipDate: "2026-09-15" })],
      "2026-09-16",
    );
    expect(r.tasks[0].status).toBe("todo");
  });

  it("markdown 勾选优先于 meta", () => {
    const r = mergeTaskMeta(
      todos,
      [mkMeta({ taskKey: "bbbb-0", status: "todo" })],
      "2026-09-16",
    );
    expect(r.tasks[1].status).toBe("done");
  });

  it("running 注入显示 🍅", () => {
    const r = mergeTaskMeta(todos, [], "2026-09-16", "cccc-0");
    expect(r.tasks[2].status).toBe("running");
  });

  it("孤儿 meta 检出（任务被删除/编辑）", () => {
    const r = mergeTaskMeta(
      todos,
      [mkMeta({ taskKey: "dead-0", status: "skipped", skipDate: null })],
      "2026-09-16",
    );
    expect(r.orphans).toHaveLength(1);
    expect(r.orphans[0].taskKey).toBe("dead-0");
  });

  it("done 的 meta 不被 running 覆盖", () => {
    const r = mergeTaskMeta(
      todos,
      [mkMeta({ taskKey: "bbbb-0", status: "done" })],
      "2026-09-16",
      "bbbb-0",
    );
    expect(r.tasks[1].status).toBe("done");
  });
});

describe("effectiveStatus / isNoopMeta", () => {
  it("无 meta = todo", () => {
    expect(effectiveStatus(undefined, "2026-09-16")).toBe("todo");
  });
  it("noop 判定", () => {
    expect(
      isNoopMeta({ taskKey: "k", lineText: "t", status: "todo", skipDate: null }),
    ).toBe(true);
    expect(
      isNoopMeta({ taskKey: "k", lineText: "t", status: "skipped", skipDate: "2026-09-16" }),
    ).toBe(false);
  });
});
