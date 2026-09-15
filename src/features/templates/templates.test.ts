import { describe, expect, it } from "vitest";
import {
  BUILTIN_TEMPLATES,
  CUSTOM_TEMPLATES_KEY,
  applyTemplate,
  defaultTemplateVars,
  deleteCustomTemplate,
  loadCustomTemplates,
  renderTemplateText,
  saveCustomTemplate,
  templatePreviewLines,
  type NoteTemplate,
  type TemplateStorage,
} from "./templates";

/** 内存 Storage 桩（vitest node 环境无 window.localStorage） */
function fakeStorage(initial: Record<string, string> = {}): TemplateStorage {
  const map = new Map(Object.entries(initial));
  return {
    getItem: (k) => (map.has(k) ? (map.get(k) as string) : null),
    setItem: (k, v) => {
      map.set(k, v);
    },
    removeItem: (k) => {
      map.delete(k);
    },
  };
}

const tpl: NoteTemplate = {
  id: "t1",
  name: "测试",
  content: "# {{date}}\n时间 {{time}}",
};

describe("BUILTIN_TEMPLATES", () => {
  it("至少 4 个模板，id 唯一且名称/内容非空", () => {
    expect(BUILTIN_TEMPLATES.length).toBeGreaterThanOrEqual(4);
    const ids = BUILTIN_TEMPLATES.map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const t of BUILTIN_TEMPLATES) {
      expect(t.name.trim()).not.toBe("");
      expect(t.content.trim()).not.toBe("");
    }
  });

  it("覆盖待办/会议/站会/日记四类场景，待办模板含任务行", () => {
    const names = BUILTIN_TEMPLATES.map((t) => t.name);
    expect(names).toEqual(expect.arrayContaining(["空白待办", "会议纪要", "每日站会", "日记"]));
    for (const t of BUILTIN_TEMPLATES) {
      if (t.name === "空白待办" || t.name === "会议纪要") {
        expect(t.content).toContain("- [ ] ");
      }
    }
  });
});

describe("defaultTemplateVars / renderTemplateText", () => {
  it("默认变量为本地 YYYY-MM-DD 与 HH:mm", () => {
    const vars = defaultTemplateVars(new Date(2026, 8, 15, 9, 5)); // 本地 2026-09-15 09:05
    expect(vars.date).toBe("2026-09-15");
    expect(vars.time).toBe("09:05");
  });

  it("替换 {{date}} 与 {{time}}", () => {
    const out = renderTemplateText(tpl.content, { date: "2026-09-15", time: "08:30" });
    expect(out).toBe("# 2026-09-15\n时间 08:30");
  });

  it("未传 vars 时使用当前日期/时间格式", () => {
    const out = renderTemplateText("d={{date}} t={{time}}");
    expect(out).toMatch(/^d=\d{4}-\d{2}-\d{2} t=\d{2}:\d{2}$/);
  });

  it("无对应值的占位保持原样，vars 可扩展自定义键", () => {
    const out = renderTemplateText("{{date}} {{who}} {{unknown}}", { who: "小明" });
    expect(out).toBe(`${defaultTemplateVars().date} 小明 {{unknown}}`);
  });
});

describe("applyTemplate", () => {
  it("空内容直接返回渲染后的模板", () => {
    expect(applyTemplate("", tpl, { date: "2026-01-01", time: "00:00" })).toBe(
      "# 2026-01-01\n时间 00:00",
    );
  });

  it("已有内容时保留原文并空一行追加", () => {
    const out = applyTemplate("原有笔记", tpl, { date: "2026-01-01", time: "00:00" });
    expect(out).toBe("原有笔记\n\n# 2026-01-01\n时间 00:00");
  });

  it("已有内容尾随换行被规整为单个空行分隔", () => {
    const out = applyTemplate("原文\n\n\n", tpl, { date: "2026-01-01", time: "00:00" });
    expect(out).toBe("原文\n\n# 2026-01-01\n时间 00:00");
  });
});

describe("templatePreviewLines", () => {
  it("渲染占位符并取前 2 个非空行", () => {
    const rendered = renderTemplateText(tpl.content, { date: "2026-09-15", time: "08:00" });
    expect(templatePreviewLines(rendered)).toEqual(["# 2026-09-15", "时间 08:00"]);
  });

  it("默认 max=2，全空内容返回空数组", () => {
    expect(templatePreviewLines("\n \n")).toEqual([]);
    expect(templatePreviewLines("a\nb\nc\nd")).toEqual(["a", "b"]);
  });
});

describe("自定义模板存取（注入 storage）", () => {
  it("空存储读取返回空数组", () => {
    expect(loadCustomTemplates(fakeStorage())).toEqual([]);
  });

  it("坏 JSON 容错返回空数组", () => {
    expect(loadCustomTemplates(fakeStorage({ [CUSTOM_TEMPLATES_KEY]: "{oops" }))).toEqual([]);
  });

  it("非数组 JSON 容错返回空数组", () => {
    expect(loadCustomTemplates(fakeStorage({ [CUSTOM_TEMPLATES_KEY]: '{"a":1}' }))).toEqual([]);
  });

  it("加载时过滤非法条目、补齐缺失 id", () => {
    const raw = JSON.stringify([
      { name: "好模板", content: "内容" }, // 合法，id 缺失 → 自动补
      { name: "", content: "x" }, // 名称空 → 丢弃
      "not-an-object", // 非对象 → 丢弃
      { name: 42, content: "x" }, // 类型错误 → 丢弃
      { id: "c2", name: "第二个", content: "内容2", icon: "🎯" },
    ]);
    const list = loadCustomTemplates(fakeStorage({ [CUSTOM_TEMPLATES_KEY]: raw }));
    expect(list).toHaveLength(2);
    expect(list[0].id).toMatch(/^custom-/);
    expect(list[0].name).toBe("好模板");
    expect(list[1]).toEqual({ id: "c2", name: "第二个", content: "内容2", icon: "🎯" });
  });

  it("saveCustomTemplate 追加并持久化；id 缺省自动生成", () => {
    const store = fakeStorage();
    const list = saveCustomTemplate({ id: "", name: "我的模板", content: "A" }, store);
    expect(list).toHaveLength(1);
    expect(list[0].id).not.toBe("");
    expect(JSON.parse(store.getItem(CUSTOM_TEMPLATES_KEY) as string)).toEqual(list);
  });

  it("saveCustomTemplate 相同 id 原位覆盖，不产生重复", () => {
    const store = fakeStorage();
    saveCustomTemplate({ id: "c1", name: "A", content: "1" }, store);
    const list = saveCustomTemplate({ id: "c1", name: "A2", content: "2" }, store);
    expect(list).toHaveLength(1);
    expect(list[0].name).toBe("A2");
  });

  it("saveCustomTemplate 空名称/空内容抛错且不写入", () => {
    const store = fakeStorage();
    expect(() => saveCustomTemplate({ id: "", name: "  ", content: "x" }, store)).toThrow();
    expect(() => saveCustomTemplate({ id: "", name: "x", content: " " }, store)).toThrow();
    expect(loadCustomTemplates(store)).toEqual([]);
  });

  it("deleteCustomTemplate 删除指定 id 并持久化；不存在时静默", () => {
    const store = fakeStorage();
    saveCustomTemplate({ id: "c1", name: "A", content: "1" }, store);
    saveCustomTemplate({ id: "c2", name: "B", content: "2" }, store);
    const after = deleteCustomTemplate("c1", store);
    expect(after.map((t) => t.id)).toEqual(["c2"]);
    expect(JSON.parse(store.getItem(CUSTOM_TEMPLATES_KEY) as string)).toEqual(after);
    expect(deleteCustomTemplate("nope", store)).toHaveLength(1);
  });
});
