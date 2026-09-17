/**
 * 契约审计纯函数单测 + 真实文件端到端快照断言。
 *
 * 端到端快照（注册命令数 / 未使用命令集）即是漂移防线：
 * 任何一侧增删命令都会让快照失败，强制人工确认双端同步。
 */
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";
import {
  ROOT_DIR,
  REQUIRED_MODULES,
  REQUIRED_TYPE_FIELDS,
  check_modules,
  check_type_fields,
  diff_commands,
  extract_commands_from_rs,
  extract_invokes_from_ts,
  extract_rust_struct_fields,
  extract_ts_interface_fields,
  invoke_source_files,
  snake_to_camel,
} from "./contract-audit.mjs";

const readReal = (rel) => readFileSync(path.join(ROOT_DIR, rel), "utf8");

// ---------------------------------------------------------------------------
// extract_commands_from_rs
// ---------------------------------------------------------------------------

describe("extract_commands_from_rs", () => {
  it("空文件返回空 Set", () => {
    expect(extract_commands_from_rs("")).toEqual(new Set());
  });

  it("无 generate_handler 块返回空 Set", () => {
    const src = "fn main() { println!(\"commands::notes_cmd::create_note\"); }";
    expect(extract_commands_from_rs(src)).toEqual(new Set());
  });

  it("标准 generate_handler 块：提取全部命令名，忽略注释与模块路径", () => {
    const src = `
      .invoke_handler(tauri::generate_handler![
          // 便签
          commands::notes_cmd::create_note,
          commands::notes_cmd::get_note,
          // 系统
          commands::system_cmd::quit_app,
      ])
      .build(tauri::generate_context!())
    `;
    expect(extract_commands_from_rs(src)).toEqual(
      new Set(["create_note", "get_note", "quit_app"]),
    );
  });

  it("注释掉的注册行不计入", () => {
    const src = `
      tauri::generate_handler![
          commands::a_cmd::real_one,
          // commands::a_cmd::commented_out,
      ]
    `;
    expect(extract_commands_from_rs(src)).toEqual(new Set(["real_one"]));
  });

  it("同名命令去重", () => {
    const src = `
      tauri::generate_handler![
          commands::x_cmd::foo,
          commands::y_cmd::foo,
      ]
    `;
    expect(extract_commands_from_rs(src)).toEqual(new Set(["foo"]));
  });
});

// ---------------------------------------------------------------------------
// extract_invokes_from_ts
// ---------------------------------------------------------------------------

describe("extract_invokes_from_ts", () => {
  it("空文件返回空 Set", () => {
    expect(extract_invokes_from_ts("")).toEqual(new Set());
  });

  it("提取 invoke< Type >(\"cmd\")", () => {
    const src = `export const createNote = () => invoke<Note>("create_note");`;
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["create_note"]));
  });

  it("提取无泛型 invoke(\"cmd\")", () => {
    const src = `export const f = () => invoke("plain_cmd");`;
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["plain_cmd"]));
  });

  it("嵌套泛型 Record<string, string> 也能提取", () => {
    const src = `export const g = () => invoke<Record<string, string>>("get_all_settings");`;
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["get_all_settings"]));
  });

  it("跨行调用与多空格均可提取", () => {
    const src = [
      "export const a = () =>",
      "  invoke<Note>(",
      '    "multi_line_cmd",',
      "    { noteId },",
      "  );",
      "export const b = () => invoke<   Note   >(  \"spaced_cmd\" );",
    ].join("\n");
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["multi_line_cmd", "spaced_cmd"]));
  });

  it("注释行与变量参数不提取", () => {
    const src = [
      '// invoke<Note>("commented_cmd");',
      "const dynamic = () => invoke(someVariable);",
      'const real = () => invoke<void>("real_cmd");',
    ].join("\n");
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["real_cmd"]));
  });

  it("同名命令去重（delete_note 被 deleteNote/purgeNote 双封装）", () => {
    const src = [
      'export const a = () => invoke<void>("delete_note", { noteId });',
      'export const b = () => invoke<void>("delete_note", { noteId });',
    ].join("\n");
    expect(extract_invokes_from_ts(src)).toEqual(new Set(["delete_note"]));
  });
});

// ---------------------------------------------------------------------------
// diff_commands
// ---------------------------------------------------------------------------

describe("diff_commands", () => {
  it("双空集合 → 无缺失无未使用", () => {
    expect(diff_commands(new Set(), new Set())).toEqual({ missing: [], unused: [] });
  });

  it("前端调用了未注册命令 → 出现在 missing（P0）", () => {
    const r = diff_commands(new Set(["a", "b"]), new Set(["b", "ghost_cmd"]));
    expect(r.missing).toEqual(["ghost_cmd"]);
    // "a" 已注册但未被任何前端调用，同时计入 unused（信息性）
    expect(r.unused).toEqual(["a"]);
  });

  it("注册但前端未调用 → 出现在 unused（信息性）", () => {
    const r = diff_commands(new Set(["a", "b"]), new Set(["b"]));
    expect(r.missing).toEqual([]);
    expect(r.unused).toEqual(["a"]);
  });

  it("双向同时存在时互不污染", () => {
    const r = diff_commands(new Set(["reg_only"]), new Set(["inv_only"]));
    expect(r.missing).toEqual(["inv_only"]);
    expect(r.unused).toEqual(["reg_only"]);
  });
});

// ---------------------------------------------------------------------------
// 结构体 / 接口字段提取 + 类型契约
// ---------------------------------------------------------------------------

describe("字段提取与类型契约", () => {
  const rustSample = `
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMeta {
    pub note_id: String,
    pub task_key: String,
    pub line_text: String,
    pub status: String,
    pub estimate_pomodoros: i64,
    pub completed_pomodoros: i64,
    pub priority: Option<String>,
    pub due_at: Option<String>,
    pub skip_date: Option<String>,
    pub updated_at: String,
}
  `;

  it("snake_to_camel", () => {
    expect(snake_to_camel("estimate_pomodoros")).toBe("estimatePomodoros");
    expect(snake_to_camel("readonly")).toBe("readonly");
    expect(snake_to_camel("id")).toBe("id");
  });

  it("extract_rust_struct_fields：snake_case 字段转 camelCase；同名前缀 struct 不误配", () => {
    const fields = extract_rust_struct_fields(rustSample, "TaskMeta");
    expect(fields).toEqual(
      new Set([
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
      ]),
    );
    // Note 不应匹配到 NoteSummary
    const src = "pub struct NoteSummary { pub note: Note, pub tags: Vec<String> }";
    expect(extract_rust_struct_fields(src, "NoteSummary")).toEqual(new Set(["note", "tags"]));
  });

  it("struct 不存在返回 null", () => {
    expect(extract_rust_struct_fields("pub struct Other {}", "TaskMeta")).toBeNull();
  });

  it("extract_ts_interface_fields：可选/readonly 字段均提取", () => {
    const src = `
export interface TaskMeta {
  readonly noteId: string;
  taskKey: string;
  estimatePomodoros?: number;
}
    `;
    expect(extract_ts_interface_fields(src, "TaskMeta")).toEqual(
      new Set(["noteId", "taskKey", "estimatePomodoros"]),
    );
    expect(extract_ts_interface_fields("export interface X {}", "TaskMeta")).toBeNull();
  });

  it("check_type_fields：双向对齐通过返回空数组", () => {
    const ts = `
export interface TaskMeta {
  noteId: string;
  taskKey: string;
  estimatePomodoros: number;
}
    `;
    expect(
      check_type_fields(rustSample, ts, { TaskMeta: ["noteId", "taskKey", "estimatePomodoros"] }),
    ).toEqual([]);
  });

  it("check_type_fields：TS 缺字段 → 报出对齐问题", () => {
    const ts = `
export interface TaskMeta {
  taskKey: string;
}
    `;
    const issues = check_type_fields(rustSample, ts, {
      TaskMeta: ["noteId", "taskKey", "updatedAt"],
    });
    expect(issues).toHaveLength(2);
    expect(issues[0]).toContain("TaskMeta.noteId");
    expect(issues[0]).toContain("src/types/index.ts");
    expect(issues[1]).toContain("TaskMeta.updatedAt");
  });

  it("check_type_fields：TS 字段在 Rust 端不存在（phantom 字段）也要报", () => {
    const ts = `
export interface TaskMeta {
  taskKey: string;
  phantomField: string;
}
    `;
    const issues = check_type_fields(rustSample, ts, { TaskMeta: ["taskKey", "phantomField"] });
    expect(issues).toHaveLength(1);
    expect(issues[0]).toContain("TaskMeta.phantomField");
    expect(issues[0]).toContain("db/models.rs");
  });

  it("check_type_fields：struct/interface 缺失直接报错", () => {
    const issues = check_type_fields("", "", { TaskMeta: ["taskKey"] });
    expect(issues).toHaveLength(1);
    expect(issues[0]).toContain("pub struct TaskMeta");
  });
});

// ---------------------------------------------------------------------------
// mod 声明
// ---------------------------------------------------------------------------

describe("check_modules", () => {
  it("全部声明 → 空数组", () => {
    const src = [...REQUIRED_MODULES].map((m) => `mod ${m};`).join("\n");
    expect(check_modules(src)).toEqual([]);
  });

  it("缺失模块被报出；pub mod 也算已声明", () => {
    const src = `mod pomodoro;\npub mod privacy;\nmod other;`;
    const missing = check_modules(src);
    expect(missing).toContain("dnd");
    expect(missing).not.toContain("pomodoro");
    expect(missing).not.toContain("privacy");
  });
});

// ---------------------------------------------------------------------------
// 端到端：真实文件 + 快照
// ---------------------------------------------------------------------------

describe("端到端（真实仓库文件）", () => {
  const libRs = readReal(path.join("src-tauri", "src", "lib.rs"));
  const modelsRs = readReal(path.join("src-tauri", "src", "db", "models.rs"));
  const typesTs = readReal(path.join("src", "types", "index.ts"));

  const registered = extract_commands_from_rs(libRs);
  const invoked = new Set(
    invoke_source_files().flatMap((f) => [
      ...extract_invokes_from_ts(readFileSync(f, "utf8")),
    ]),
  );
  const { missing, unused } = diff_commands(registered, invoked);

  it("前端 invoke 源文件覆盖 lib/api.ts 与各 feature api.ts", () => {
    const rel = invoke_source_files().map((f) => path.relative(ROOT_DIR, f));
    expect(rel).toContain(path.join("src", "lib", "api.ts"));
    expect(rel).toContain(path.join("src", "features", "pomodoro", "api.ts"));
    expect(rel.length).toBeGreaterThanOrEqual(8);
  });

  it("[快照] Rust 注册命令总数（增删命令须双端同步后更新此快照）", () => {
    expect(registered.size).toBe(96);
    expect(registered).toContain("create_note");
    expect(registered).toContain("task_meta_update");
  });

  it("[快照] 前端调用命令总数", () => {
    expect(invoked.size).toBe(95);
  });

  it("P0：前端调用 ⊆ Rust 注册（无缺失）", () => {
    expect(missing).toEqual([]);
  });

  it("[快照] 未使用命令清单（注册但前端未调用，仅为信息性）", () => {
    expect(unused).toEqual(["task_meta_get"]);
  });

  it("P0：关键类型字段与 Rust serde camelCase 对齐", () => {
    expect(check_type_fields(modelsRs, typesTs)).toEqual([]);
  });

  it("P0：lib.rs 必需 mod 全部声明", () => {
    expect(check_modules(libRs)).toEqual([]);
  });

  it("快照数据自洽：REQUIRED_TYPE_FIELDS 覆盖的 struct/interface 双端都存在", () => {
    for (const name of Object.keys(REQUIRED_TYPE_FIELDS)) {
      expect(extract_rust_struct_fields(modelsRs, name)).not.toBeNull();
      expect(extract_ts_interface_fields(typesTs, name)).not.toBeNull();
    }
  });
});

// ---------------------------------------------------------------------------
// capabilities 权限契约（真实文件）——JS 侧窗口 API 的 ACL 漂移防线
// ---------------------------------------------------------------------------

describe("capabilities 权限契约（真实仓库文件）", () => {
  const caps = JSON.parse(readReal(path.join("src-tauri", "capabilities", "default.json")));

  it("P0：便签窗口 onCloseRequested 依赖 destroy() 收尾，必须授予 core:window:allow-destroy", () => {
    // Tauri 内部对注册了 tauri://close-requested JS 监听器的窗口一律 prevent_close
    // （tauri manager/window.rs），由 @tauri-apps/api 的 onCloseRequested 包装器
    // 调用 destroy() 完成关闭。缺权限时该 invoke 被 ACL 拒绝且无人兜底，
    // 表现为关闭/归档/删除后便签窗口永不销毁（用户视角"点击无反应"）。
    expect(caps.permissions).toContain("core:window:allow-destroy");
  });

  it("迷你番茄窗物理定位与速记箱剪贴板捕获所需权限", () => {
    // miniWindow.ts: win.setPosition(new PhysicalPosition(...))
    expect(caps.permissions).toContain("core:window:allow-set-position");
    // QuickCaptureWindow.tsx: readText()（clipboard-manager 默认集为空，需显式授予）
    expect(caps.permissions).toContain("clipboard-manager:allow-read-text");
  });

  it("capability 覆盖全部窗口（含动态创建的便签/速记/迷你番茄窗）", () => {
    expect(caps.windows).toContain("*");
  });
});
