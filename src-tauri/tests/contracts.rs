//! 跨层源码契约集成测试（Rust ↔ TS）。
//!
//! 这些契约靠字符串字面量在两端各自声明（AGENTS.md「事件/命令两端一一对应」），
//! 单语言单测无法覆盖，只能在对源码文本做解析比对。改任何一端命名，这里会红。

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("读取 {} 失败: {e}", p.display()))
}

/// 取一行中第一对双引号/单引号内的字面量（行需含至少一个引号对）。
fn quoted(line: &str, quote: char) -> Option<String> {
    let rest = line.split(quote).nth(1)?;
    rest.split(quote).next().map(str::to_owned)
}

/// events.rs：`pub const X: &str = "name";` → 事件名集合
fn rust_event_names() -> BTreeSet<String> {
    let src = read("src-tauri/src/events.rs");
    src.lines()
        .filter(|l| l.trim_start().starts_with("pub const"))
        .filter_map(|l| quoted(l, '"'))
        .collect()
}

/// types/index.ts：`export const NAME = { ... } as const;` 块内的值字面量集合
fn ts_event_values(const_name: &str) -> BTreeSet<String> {
    let src = read("src/types/index.ts");
    let start = src
        .find(&format!("export const {const_name} = {{"))
        .unwrap_or_else(|| panic!("types/index.ts 缺少 {const_name}"));
    let body = &src[start..];
    let end = body
        .find("} as const")
        .unwrap_or_else(|| panic!("{const_name} 块未闭合"));
    body[..end]
        .lines()
        .map(str::trim_start)
        // 只取 `key: "value",` 行；注释行以 /** 或 // 开头
        .filter(|l| {
            !l.starts_with('/') && !l.starts_with("export") && l.contains(": \"")
        })
        .filter_map(|l| quoted(l, '"'))
        .collect()
}

#[test]
fn rust_and_ts_event_name_sets_match() {
    let mut ts = ts_event_values("EVENTS");
    ts.extend(ts_event_values("POMODORO_EVENTS"));
    let rust = rust_event_names();
    let only_ts: Vec<&String> = ts.difference(&rust).collect();
    let only_rust: Vec<&String> = rust.difference(&ts).collect();
    assert!(
        only_ts.is_empty() && only_rust.is_empty(),
        "事件名两端不一致，仅 TS 有: {only_ts:?}，仅 Rust 有: {only_rust:?}",
    );
}

#[test]
fn every_shortcut_action_is_wired_end_to_end() {
    let src = read("src-tauri/src/commands/settings_cmd.rs");
    let start = src
        .find("const SHORTCUT_ACTIONS: &[&str] = &[")
        .expect("SHORTCUT_ACTIONS 未找到");
    let body = &src[start..];
    let end = body.find("];").expect("SHORTCUT_ACTIONS 未闭合");
    let actions: Vec<String> = body[..end]
        .lines()
        .filter_map(|l| l.trim_start().starts_with('"').then(|| quoted(l, '"')).flatten())
        .collect();
    assert!(actions.len() >= 6, "解析出的 action 数异常: {actions:?}");

    let dispatch = read("src-tauri/src/shortcut.rs");
    let dialog = read("src/features/settings/SettingsDialog.tsx");
    let migrations = read("src-tauri/src/db/migrations.rs");
    for a in &actions {
        assert!(
            dispatch.contains(&format!("\"{a}\" =>")),
            "动作 {a} 未在 shortcut.rs::dispatch_action 注册"
        );
        assert!(
            dialog.contains(&format!("action: \"{a}\"")),
            "动作 {a} 未在 SettingsDialog 快捷键录制列表出现"
        );
        assert!(
            migrations.contains(&format!("('{a}',")),
            "动作 {a} 无 migrations.rs 默认绑定行"
        );
    }
}

#[test]
fn qc_task_mode_contract() {
    // Rust 冷启动走 URL（?mode=task），前端 mode.ts 按同名键值解析；
    // 已存在的窗口走 qc-mode 事件（事件集对齐由 rust_and_ts_event_name_sets_match 覆盖）。
    let rust = read("src-tauri/src/quickcapture.rs");
    assert!(
        rust.contains("index.html?mode=task"),
        "quickcapture.rs 的初始任务 tab URL 变了，需同步前端 tabFromSearch"
    );
    let ts = read("src/features/quick-capture/mode.ts");
    assert!(
        ts.contains(r#".get("mode") === "task""#),
        "mode.ts 解析的 URL 参数与 Rust 端不再一致"
    );
}

#[test]
fn palette_navigate_views_are_handled_by_manager() {
    let palette = read("src/features/command-palette/useCommandPalette.ts");
    let manager = read("src/features/notes/ManagerWindow.tsx");
    let mut seen = BTreeSet::new();
    for chunk in palette.split("dispatchNavigate({ view: ") .skip(1) {
        let Some(view) = quoted(chunk, '"') else { continue };
        assert!(
            manager.contains(&format!("detail.view === \"{view}\"")),
            "命令面板导航到 {view}，但 ManagerWindow onNavigate 未处理"
        );
        seen.insert(view);
    }
    assert!(seen.len() >= 6, "解析出的 view 数异常: {seen:?}");
}

#[test]
fn pomo_dnd_setting_key_wired_everywhere() {
    // 专注勿扰设置键四处真相源：写入白名单 / 读取端默认 / 老库迁移默认 / 前端契约。
    // 任一处漏掉都会静默漂移（写失败 / 回显不对 / 默认不生效），只能源码比对拦截。
    for (rel, needle) in [
        ("src-tauri/src/commands/settings_cmd.rs", "\"pomo_dnd\""),
        ("src-tauri/src/db/settings.rs", "(\"pomo_dnd\", \"true\")"),
        ("src-tauri/src/db/migrations.rs", "('pomo_dnd', 'true')"),
        ("src/types/index.ts", "pomoDnd: \"pomo_dnd\""),
    ] {
        let src = read(rel);
        assert!(src.contains(needle), "{rel} 缺少 {needle}（专注勿扰键接线漂移）");
    }
    // 消费端：提醒调度器必须查询 focus_silence_until，否则设置项成死键
    let wiring = read("src-tauri/src/reminder.rs");
    assert!(
        wiring.contains("focus_silence_until("),
        "reminder.rs 不再消费 focus_silence_until——pomo_dnd 成了死设置"
    );
}
