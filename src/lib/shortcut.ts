/** 快捷键加速器工具：KeyboardEvent ↔ "Ctrl+Shift+N" ↔ Tauri 全局快捷键字符串 */

export interface AcceleratorKeyEvent {
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  metaKey: boolean;
  key: string;
}

/** 仅由修饰键组成的按键（此时无法构成有效快捷键） */
const MODIFIER_KEY_NAMES = new Set(["control", "shift", "alt", "meta", "altgraph"]);

/** 修饰键段 → 显示名（Super 统一表示 Win 键） */
const MODIFIER_DISPLAY: Record<string, string> = {
  ctrl: "Ctrl",
  control: "Ctrl",
  shift: "Shift",
  alt: "Alt",
  super: "Super",
  meta: "Super",
  cmd: "Super",
  command: "Super",
};

/** 具名主键 → 显示名（大小写不敏感） */
const NAMED_KEY_DISPLAY: Record<string, string> = {
  space: "Space",
  enter: "Enter",
  tab: "Tab",
  escape: "Esc",
  esc: "Esc",
};

/** 主键归一：字母大写、F1-F24 大写、空格/具名键 → 标准名，其余原样 */
function normalizeMainKey(raw: string): string {
  if (/^[a-z]$/i.test(raw)) return raw.toUpperCase();
  if (/^[0-9]$/.test(raw)) return raw;
  if (/^f([1-9]|1\d|2[0-4])$/i.test(raw)) return raw.toUpperCase();
  if (raw === " ") return "Space";
  return NAMED_KEY_DISPLAY[raw.toLowerCase()] ?? raw;
}

/** KeyboardEvent → "Ctrl+Shift+N" 形式；按下的就是修饰键（或无按键）时返回 null */
export function normalizeAccelerator(e: AcceleratorKeyEvent): string | null {
  if (!e.key) return null;
  if (MODIFIER_KEY_NAMES.has(e.key.toLowerCase())) return null;
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  if (e.metaKey) parts.push("Super");
  parts.push(normalizeMainKey(e.key));
  return parts.join("+");
}

/** 加速器字符串 → 人类可读显示（大小写归一） */
export function acceleratorToDisplay(s: string): string {
  return s
    .split("+")
    .map((part) => {
      const t = part.trim();
      if (!t) return t;
      const mod = MODIFIER_DISPLAY[t.toLowerCase()];
      if (mod) return mod;
      return normalizeMainKey(t);
    })
    .filter((t) => t.length > 0)
    .join("+");
}

/** 加速器字符串 → Tauri 全局快捷键格式（全小写） */
export function acceleratorToTauri(s: string): string {
  return s
    .split("+")
    .map((p) => p.trim().toLowerCase())
    .filter((p) => p.length > 0)
    .join("+");
}

/** 校验：按 "+" 拆段；修饰键可选但至少一个；主键必填且不能是修饰键 */
export function isValidAccelerator(s: string): boolean {
  const raw = s.split("+");
  if (raw.some((p) => p.trim() === "")) return false;
  let modifiers = 0;
  let mains = 0;
  for (const part of raw) {
    const t = part.trim().toLowerCase();
    if (MODIFIER_DISPLAY[t] !== undefined) {
      modifiers += 1;
    } else {
      mains += 1;
    }
  }
  return modifiers >= 1 && mains === 1;
}
