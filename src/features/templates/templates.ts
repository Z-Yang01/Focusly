/** 模板纯函数：内置模板常量、{{date}}/{{time}} 占位符渲染、自定义模板存取。
 *  无框架依赖；存储通过可选参数注入（默认 window.localStorage，非 DOM 环境回退内存实现，
 *  便于 vitest（node 环境）直接传入内存 Storage 测试）。 */

export interface NoteTemplate {
  id: string;
  name: string;
  /** 展示用 emoji / 图标字符，可选 */
  icon?: string;
  /** Markdown 正文，支持 {{date}} / {{time}} 及自定义 {{key}} 占位符 */
  content: string;
}

/** 自定义模板在 localStorage 的键 */
export const CUSTOM_TEMPLATES_KEY = "focusly.templates.custom";

// ---------- 内置模板 ----------

/** 内置高质量模板（只读约定：调用方不得修改） */
export const BUILTIN_TEMPLATES: NoteTemplate[] = [
  {
    id: "builtin-todo",
    name: "空白待办",
    icon: "✅",
    content: [
      "# {{date}} 待办",
      "",
      "## 今日待办",
      "",
      "- [ ] ",
      "- [ ] ",
      "- [ ] ",
      "",
      "## 完成",
      "",
      "- [x] ",
      "",
    ].join("\n"),
  },
  {
    id: "builtin-meeting",
    name: "会议纪要",
    icon: "📝",
    content: [
      "# 会议纪要",
      "",
      "- 日期：{{date}} {{time}}",
      "- 参会人：",
      "- 记录人：",
      "",
      "## 议题",
      "",
      "1. ",
      "",
      "## 决议",
      "",
      "- ",
      "",
      "## 待办清单",
      "",
      "- [ ] ",
      "",
    ].join("\n"),
  },
  {
    id: "builtin-standup",
    name: "每日站会",
    icon: "🌤️",
    content: [
      "# 每日站会 · {{date}}",
      "",
      "## 昨日完成",
      "",
      "- ",
      "",
      "## 今日计划",
      "",
      "- ",
      "",
      "## 阻塞",
      "",
      "- 无",
      "",
    ].join("\n"),
  },
  {
    id: "builtin-diary",
    name: "日记",
    icon: "📖",
    content: [
      "# 日记 · {{date}}",
      "",
      "## 心情",
      "",
      "😄 ",
      "",
      "## 记录",
      "",
      "今天",
      "",
      "## 感恩三件事",
      "",
      "1. ",
      "2. ",
      "3. ",
      "",
    ].join("\n"),
  },
];

// ---------- 占位符渲染 ----------

function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

/** 默认占位变量：本地日期 YYYY-MM-DD 与本地时间 HH:mm */
export function defaultTemplateVars(now: Date = new Date()): Record<string, string> {
  const date = `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}`;
  const time = `${pad2(now.getHours())}:${pad2(now.getMinutes())}`;
  return { date, time };
}

/** 渲染文本：{{date}}/{{time}} 及 vars 中的自定义键做占位替换；无对应值的占位保持原样 */
export function renderTemplateText(text: string, vars?: Record<string, string>): string {
  const merged = { ...defaultTemplateVars(), ...vars };
  return text.replace(/\{\{(\w+)\}\}/g, (raw: string, key: string) =>
    Object.prototype.hasOwnProperty.call(merged, key) ? merged[key] : raw,
  );
}

/** 应用模板：空内容直接返回渲染结果；已有内容则保留原文，空一行后追加渲染结果 */
export function applyTemplate(
  content: string,
  tpl: NoteTemplate,
  vars?: Record<string, string>,
): string {
  const rendered = renderTemplateText(tpl.content, vars);
  if (content.trim() === "") return rendered;
  return `${content.replace(/\n+$/, "")}\n\n${rendered}`;
}

/** 预览行：渲染占位符后取前 max 个非空行（选择器网格用） */
export function templatePreviewLines(content: string, max: number = 2): string[] {
  return renderTemplateText(content)
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "")
    .slice(0, max);
}

// ---------- 自定义模板存取（storage 可注入） ----------

/** 最小存储接口：window.localStorage 天然满足；测试注入内存实现 */
export interface TemplateStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

function memoryStorage(): TemplateStorage {
  const map = new Map<string, string>();
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

function resolveStorage(storage?: TemplateStorage): TemplateStorage {
  if (storage) return storage;
  if (typeof window !== "undefined" && window.localStorage) return window.localStorage;
  return memoryStorage();
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null;
}

/** 宽松校验：name 非空字符串 + content 为字符串才算合法；id 缺失时用 fallbackId */
function coerceTemplate(v: unknown, fallbackId: string): NoteTemplate | null {
  if (!isRecord(v)) return null;
  const { id, name, icon, content } = v;
  if (typeof name !== "string" || name.trim() === "") return null;
  if (typeof content !== "string") return null;
  return {
    id: typeof id === "string" && id.trim() !== "" ? id : fallbackId,
    name,
    ...(typeof icon === "string" && icon !== "" ? { icon } : {}),
    content,
  };
}

/** 读取自定义模板：键不存在 / 坏 JSON / 非数组 / 非法条目一律容错（跳过或返回空数组） */
export function loadCustomTemplates(storage?: TemplateStorage): NoteTemplate[] {
  const raw = resolveStorage(storage).getItem(CUSTOM_TEMPLATES_KEY);
  if (raw === null || raw.trim() === "") return [];
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(parsed)) return [];
  const list: NoteTemplate[] = [];
  parsed.forEach((item, i) => {
    const tpl = coerceTemplate(item, `custom-${Date.now().toString(36)}-${i}`);
    if (tpl) list.push(tpl);
  });
  return list;
}

function persist(list: NoteTemplate[], storage: TemplateStorage): void {
  storage.setItem(CUSTOM_TEMPLATES_KEY, JSON.stringify(list));
}

/** 保存自定义模板（upsert）：id 相同原位覆盖，否则追加；id 缺省自动生成且不与现有冲突。
 *  名称/内容为空时抛错。返回保存后的完整列表。 */
export function saveCustomTemplate(tpl: NoteTemplate, storage?: TemplateStorage): NoteTemplate[] {
  if (tpl.name.trim() === "") throw new Error("模板名称不能为空");
  if (tpl.content.trim() === "") throw new Error("模板内容不能为空");
  const store = resolveStorage(storage);
  const list = loadCustomTemplates(store);
  const next: NoteTemplate = { ...tpl, name: tpl.name.trim() };
  const idx = next.id !== "" ? list.findIndex((t) => t.id === next.id) : -1;
  if (idx >= 0) {
    list[idx] = next;
  } else {
    if (next.id === "") next.id = `custom-${Date.now().toString(36)}-${list.length}`;
    while (list.some((t) => t.id === next.id)) next.id = `${next.id}~`;
    list.push(next);
  }
  persist(list, store);
  return list;
}

/** 删除自定义模板：id 不存在时静默返回现有列表。返回删除后的完整列表 */
export function deleteCustomTemplate(id: string, storage?: TemplateStorage): NoteTemplate[] {
  const store = resolveStorage(storage);
  const list = loadCustomTemplates(store).filter((t) => t.id !== id);
  persist(list, store);
  return list;
}
