#!/usr/bin/env node
/**
 * Focusly 命令契约审计（静态分析，不运行应用）
 *
 * 防止 Tauri 命令漂移：
 *   1. src-tauri/src/lib.rs  generate_handler! 注册命令  ←→  前端 invoke 调用
 *   2. src/types/index.ts 关键类型字段 ←→ Rust db/models.rs serde camelCase 输出
 *   3. lib.rs mod 声明完整性（pomodoro/privacy/dnd/timeparse/daily/imagemgr/quickcapture）
 *
 * 输出报告到 stdout；P0 问题（前端调用未注册命令 / 类型不对齐 / mod 缺失）→ exit 1。
 * 未使用命令（注册但前端从未调用）仅信息性输出，不影响退出码。
 *
 * 纯函数均导出，供 scripts/contract-audit.test.ts 复用；
 * main 由 import.meta 守卫包裹，被测试导入时不会执行。
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

/** 仓库根目录（scripts/ 的上一级） */
export const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

/** lib.rs 必须声明的业务模块（AGENTS.md 架构关键模块） */
export const REQUIRED_MODULES = [
  "pomodoro",
  "privacy",
  "dnd",
  "timeparse",
  "daily",
  "imagemgr",
  "quickcapture",
];

/**
 * 关键类型契约：Rust models.rs struct ←→ src/types/index.ts interface。
 * 字段名一律为 serde(rename_all = "camelCase") 输出名。
 */
export const REQUIRED_TYPE_FIELDS = {
  Note: [
    "id",
    "title",
    "content",
    "contentFormat",
    "status",
    "isPinned",
    "isAlwaysOnTop",
    "showOnAllDesktops",
    "desktopPinState",
    "fullscreenBehavior",
    "deletedAt",
    "isPrivate",
    "locked",
    "readonly",
    "scale",
  ],
  TaskMeta: [
    "noteId",
    "taskKey",
    "lineText",
    "status",
    "estimatePomodoros",
    "completedPomodoros",
    "priority",
    "dueAt",
    "skipDate",
    "updatedAt",
  ],
  SearchHit: [
    "id",
    "title",
    "snippet",
    "tags",
    "todoTotal",
    "todoDone",
    "updatedAt",
    "isPinned",
    "isPrivate",
    "status",
  ],
};

/** 前端 invoke 入口文件：src/lib/api.ts + 各 feature 目录下的 api.ts */
export function invoke_source_files(rootDir = ROOT_DIR) {
  const files = [path.join(rootDir, "src", "lib", "api.ts")];
  const featuresDir = path.join(rootDir, "src", "features");
  if (fs.existsSync(featuresDir)) {
    for (const ent of fs.readdirSync(featuresDir, { withFileTypes: true })) {
      const f = path.join(featuresDir, ent.name, "api.ts");
      if (ent.isDirectory() && fs.existsSync(f)) files.push(f);
    }
  }
  return files;
}

/** 去掉整行 `//` 注释（避免把注释里的示例命令当作真实调用/注册） */
function strip_line_comments(source, commentToken = "//") {
  return source
    .split("\n")
    .filter((line) => !line.trimStart().startsWith(commentToken))
    .join("\n");
}

/**
 * 从 Rust 源码提取 generate_handler![...] 中注册的全部命令名。
 * 返回 Set<string>；无 generate_handler 块时返回空 Set。
 */
export function extract_commands_from_rs(source) {
  const block = source.match(/generate_handler!\s*\[([\s\S]*?)\]/);
  if (!block) return new Set();
  const names = new Set();
  for (const m of strip_line_comments(block[1]).matchAll(/commands::\w+::(\w+)/g)) {
    names.add(m[1]);
  }
  return names;
}

/**
 * 从 TS 源码提取全部 invoke 的命令名。
 * 覆盖 invoke("cmd") 与 invoke< Type >("cmd")（含嵌套泛型如 Record<string, string>）。
 * 返回 Set<string>。
 */
export function extract_invokes_from_ts(source) {
  const names = new Set();
  const re = /invoke(?:\s*<[^(]*?>)?\s*\(\s*"(\w+)"/g;
  for (const m of strip_line_comments(source).matchAll(re)) {
    names.add(m[1]);
  }
  return names;
}

/**
 * 交叉比对。
 * missing = 前端调用了但 Rust 未注册（P0，运行时必报 "command not found"）
 * unused  = Rust 注册了但前端从未调用（信息性，非错误）
 */
export function diff_commands(registered, invoked) {
  return {
    missing: [...invoked].filter((c) => !registered.has(c)).sort(),
    unused: [...registered].filter((c) => !invoked.has(c)).sort(),
  };
}

/** snake_case → camelCase */
export function snake_to_camel(s) {
  return s.replace(/_([a-zA-Z0-9])/g, (_, c) => c.toUpperCase());
}

/**
 * 从 Rust 源码提取 `pub struct <name> { ... }` 的字段集（camelCase 化）。
 * struct 不存在返回 null。
 */
export function extract_rust_struct_fields(source, structName) {
  const m = source.match(new RegExp(`pub struct ${structName}\\s*\\{([^}]*)\\}`));
  if (!m) return null;
  const fields = new Set();
  for (const f of m[1].matchAll(/pub\s+(?:r#)?(\w+)\s*:/g)) {
    fields.add(snake_to_camel(f[1]));
  }
  return fields;
}

/**
 * 从 TS 源码提取 `export interface <name> { ... }` 的字段集。
 * interface 不存在返回 null。
 */
export function extract_ts_interface_fields(source, interfaceName) {
  const m = source.match(new RegExp(`export interface ${interfaceName}\\s*\\{([^}]*)\\}`));
  if (!m) return null;
  const fields = new Set();
  for (const f of m[1].matchAll(/^\s*(?:readonly\s+)?(\w+)\s*\??\s*:/gm)) {
    fields.add(f[1]);
  }
  return fields;
}

/**
 * 校验关键类型字段与 Rust serde camelCase 输出双向对齐。
 * 返回问题列表（空数组 = 通过）：
 *   - "Note.xxx: models.rs ..." Rust struct 缺字段
 *   - "Note.xxx: src/types/index.ts ..." TS interface 缺字段
 */
export function check_type_fields(
  modelsSource,
  typesSource,
  required = REQUIRED_TYPE_FIELDS,
) {
  const issues = [];
  for (const [name, fields] of Object.entries(required)) {
    const rust = extract_rust_struct_fields(modelsSource, name);
    if (!rust) {
      issues.push(`${name}: db/models.rs 中未找到 pub struct ${name}`);
      continue;
    }
    const ts = extract_ts_interface_fields(typesSource, name);
    if (!ts) {
      issues.push(`${name}: src/types/index.ts 中未找到 export interface ${name}`);
      continue;
    }
    for (const f of fields) {
      if (!rust.has(f)) {
        issues.push(`${name}.${f}: Rust db/models.rs 的 ${name} 中不存在（camelCase 对齐失败）`);
      }
      if (!ts.has(f)) {
        issues.push(`${name}.${f}: src/types/index.ts 的 ${name} 缺少该字段（与 Rust serde camelCase 不对齐）`);
      }
    }
  }
  return issues;
}

/**
 * 校验 lib.rs 中必需的 `mod` 声明。
 * 返回缺失模块名列表（空数组 = 通过）。
 */
export function check_modules(libRsSource, required = REQUIRED_MODULES) {
  const declared = new Set();
  for (const m of libRsSource.matchAll(/(?:^|\n)\s*(?:pub\s+)?mod\s+(\w+)\s*;/g)) {
    declared.add(m[1]);
  }
  return [...required].filter((m) => !declared.has(m)).sort();
}

// ---------------------------------------------------------------------------
// main（仅直接执行时运行；被测试导入时不执行）
// ---------------------------------------------------------------------------

function read(relPath) {
  return fs.readFileSync(path.join(ROOT_DIR, relPath), "utf8");
}

export function run_audit(log = console.log) {
  const errors = [];
  const lines = [];
  const section = (t) => lines.push("", `== ${t} ==`);

  // ---- 1. 命令注册 vs 前端调用 ----
  let libRs;
  try {
    libRs = read(path.join("src-tauri", "src", "lib.rs"));
  } catch (e) {
    errors.push(`无法读取 src-tauri/src/lib.rs: ${e.message}`);
    libRs = "";
  }
  const registered = extract_commands_from_rs(libRs);

  const invoked = new Set();
  const invokeFiles = invoke_source_files();
  if (invokeFiles.length === 0) {
    errors.push("未找到任何前端 invoke 源文件（src/lib/api.ts 缺失？）");
  }
  for (const f of invokeFiles) {
    try {
      for (const c of extract_invokes_from_ts(fs.readFileSync(f, "utf8"))) invoked.add(c);
    } catch (e) {
      errors.push(`无法读取 ${path.relative(ROOT_DIR, f)}: ${e.message}`);
    }
  }

  const { missing, unused } = diff_commands(registered, invoked);

  section("命令契约（generate_handler ↔ invoke）");
  lines.push(`Rust 注册命令: ${registered.size}`);
  lines.push(`前端调用命令（去重）: ${invoked.size}`);
  for (const c of missing) {
    const msg = `[P0] 前端调用了未注册命令: ${c}（运行时必报 command not found）`;
    lines.push(`  ${msg}`);
    errors.push(msg);
  }
  if (missing.length === 0) lines.push("  缺失: 0 ✓");
  if (unused.length > 0) {
    lines.push(`  未使用命令（注册但前端未调用，信息性）: ${unused.join(", ")}`);
  } else {
    lines.push("  未使用命令: 0");
  }

  // ---- 2. 类型契约 ----
  section("类型契约（db/models.rs serde camelCase ↔ src/types/index.ts）");
  let modelsRs = "";
  let typesTs = "";
  try {
    modelsRs = read(path.join("src-tauri", "src", "db", "models.rs"));
  } catch (e) {
    errors.push(`无法读取 src-tauri/src/db/models.rs: ${e.message}`);
  }
  try {
    typesTs = read(path.join("src", "types", "index.ts"));
  } catch (e) {
    errors.push(`无法读取 src/types/index.ts: ${e.message}`);
  }
  const typeIssues = check_type_fields(modelsRs, typesTs);
  if (typeIssues.length === 0) {
    lines.push(`  Note / TaskMeta / SearchHit 字段对齐 ✓（${Object.values(REQUIRED_TYPE_FIELDS).flat().length} 个关键字段）`);
  } else {
    for (const t of typeIssues) {
      lines.push(`  [P0] ${t}`);
      errors.push(t);
    }
  }

  // ---- 3. mod 声明 ----
  section("lib.rs 模块声明");
  const missingMods = check_modules(libRs);
  if (missingMods.length === 0) {
    lines.push(`  ${REQUIRED_MODULES.join(" / ")} 全部已声明 ✓`);
  } else {
    const msg = `lib.rs 缺少 mod 声明: ${missingMods.join(", ")}`;
    lines.push(`  [P0] ${msg}`);
    errors.push(msg);
  }

  // ---- 汇总 ----
  section("结果");
  lines.push(errors.length === 0 ? "通过 ✓（契约无漂移）" : `失败 ✗（${errors.length} 个 P0 问题）`);
  for (const l of lines) log(l);
  return { ok: errors.length === 0, errors, lines };
}

const isMain = (() => {
  try {
    return import.meta.url === pathToFileURL(process.argv[1] ?? "").href;
  } catch {
    return false;
  }
})();

if (isMain) {
  process.exit(run_audit().ok ? 0 : 1);
}
