# AGENTS.md — Focusly 开发指南

> 本文件面向 AI 代理与本仓库的开发者，描述项目结构、架构决策与开发规范。

## 项目概述

**Focusly** 是一个 Windows 11 优先的个人桌面便签 / 待办 / 提醒工具。

核心原则：**快。轻。不会丢。不会忘。**

- 快速记录 → 桌面常驻 → 避免遗忘 → 待办执行 → 定时提醒 → 完成/归档 → 随时恢复
- 完全本地化：无账号、无登录、无服务端、无云同步
- 技术栈：Tauri 2 + React 18 + TypeScript + Vite + SQLite(rusqlite) + Tailwind CSS v4 + shadcn/ui 风格组件 + Radix UI + Lucide + Zustand

## 目录结构

```text
Focusly/
├── index.html                  # 唯一 HTML 入口（所有窗口共用）
├── src/                        # React/TS：只做 UI 与交互
│   ├── app/                    # 入口路由（按窗口 label）、ThemeProvider
│   │   ├── App.tsx             #   label=manager → ManagerWindow；note-<id> → NoteWindow
│   │   └── theme.tsx           #   浅/深/跟随系统，class 策略，跨窗口同步
│   ├── features/
│   │   ├── notes/              # ManagerWindow（管理器）、NoteWindow（便签窗口）、NoteCard
│   │   ├── editor/             # MarkdownEditor/MarkdownView —— 全项目唯一 Markdown 渲染模块
│   │   ├── todo/               # 待办解析/勾选回写（纯函数 + 测试）
│   │   ├── reminders/          # 提醒设置 Popover、提醒触发 Banner
│   │   ├── shortcuts/          # 快捷键录制组件
│   │   ├── settings/           # 设置对话框（外观/行为/快捷键/数据）
│   │   └── search/             # 全局搜索栏
│   ├── components/ui/          # shadcn/ui 风格基础组件（Radix 封装）
│   ├── lib/
│   │   ├── api.ts              # ★ 唯一 invoke 封装层（所有 Tauri 命令调用）
│   │   ├── tauri.ts            # 事件监听、convertFileSrc、窗口角色
│   │   ├── format.ts           # 时间/摘要格式化
│   │   ├── shortcut.ts         # 快捷键归一化/校验（纯函数）
│   │   └── utils.ts            # cn()
│   ├── stores/ui.ts            # zustand UI 状态
│   └── types/index.ts          # ★ 类型契约（与 Rust serde camelCase 严格对应）
├── src-tauri/
│   └── src/
│       ├── lib.rs              # 插件注册、setup 顺序、命令表、窗口事件
│       ├── state.rs            # AppState（db/paths/scheduler/快捷键映射/全屏隐藏集）
│       ├── error.rs            # AppError 统一错误（序列化为 {kind,message}）
│       ├── db/                 # SQLite 层：连接 + migrations + DAO（SQL 只在这里）
│       │   ├── migrations.rs   #   PRAGMA user_version 迁移，7 张表
│       │   └── notes.rs / reminders.rs / images.rs / tags.rs / settings.rs / shortcuts.rs
│       ├── commands/           # #[tauri::command] 薄层（无业务逻辑）
│       ├── notes.rs            # 便签业务编排（DB + 窗口 + 事件）
│       ├── export.rs           # JSON 导出/导入（ExportData 结构 + 事务 upsert）
│       ├── window/             # 多窗口、几何持久化（去抖）、显示器越界校正
│       │   ├── monitor.rs      #   Win32 枚举显示器、全屏前台检测
│       │   └── foreground.rs   #   SetWinEventHook 事件驱动全屏跟随
│       ├── vdesktop.rs         # Windows 虚拟桌面 Pin（IVirtualDesktopPinnedApps COM）
│       ├── reminder.rs         # tokio sleep_until 事件驱动提醒调度器（无轮询）
│       ├── shortcut.rs         # 全局快捷键注册/冲突/重载
│       ├── tray.rs             # 系统托盘
│       ├── filesystem.rs       # 数据目录、启动备份轮转、error/*.md 错误日志
│       └── logger.rs           # 极简文件日志（logs/focusly.log）
├── scripts/gen-icon.mjs        # 零依赖 PNG 图标生成
└── vitest.config.ts
```

## 职责划分（强约束）

| React / TypeScript | Rust / Tauri |
|---|---|
| UI、页面、编辑器、Markdown、Todo、搜索、设置界面、交互 | SQLite、多窗口、窗口几何/置顶、托盘、全局快捷键、Windows 通知、后台提醒、文件系统/图片文件、生命周期 |

- **UI 不直接碰 SQLite / Windows API**。前端只能经 `lib/api.ts` → Rust command。
- **SQL 只出现在 `db/` 与 `reminder.rs`/`export.rs` 的私有查询**。
- 命令层（commands/）必须薄，业务在 `notes.rs`/`window/` 等服务模块。

## 关键契约

- **命令**：见 `src/lib/api.ts`（函数名 = 语义，Rust 端 snake_case，参数自动驼峰转换）。
- **事件**（Rust → JS）：`notes-changed` / `settings-changed` / `reminder-fired` / `shortcut-error` / `notes-visibility` / `focus-search` / `open-settings`，常量在 `src/types/index.ts#EVENTS`。
- **窗口 label**：`manager` | `note-<uuid>`。新窗口一律 `visible:false` 创建，前端就绪后调 `note_window_ready` 再显示（防白闪）。
- **时间**：`remind_at` 全程 RFC3339 UTC（`to_rfc3339_opts(Secs, false)`），Rust 侧统一经 `reminder::fmt/parse`。
- **待办真相源**：正文里的 `- [ ] / - [x]`，可被搜索/统计/导入导出，不存 HTML。

## 架构决策记录

1. **每张便签 = 一个真实原生窗口**（Tauri WebviewWindow，无边框+透明+圆角+skip taskbar）。
2. **提醒在 Rust 层**：tokio 任务持有最近到期时间，`sleep_until` 精确等待 + mpsc 唤醒重排；主窗口/便签全隐藏照常触发 Windows 通知。启动时补发 24h 内过期提醒，超 24h 自动取消（防轰炸）。
3. **自动保存**：前端 500ms debounce → `update_note_content`；窗口关闭/失焦 flush；SQLite WAL + synchronous=NORMAL；启动自动备份轮转（保留 5 份）。
4. **窗口几何**：Rust 侧监听 Moved/Resized，800ms 去抖写 DB；恢复时校验与显示器相交，越界层叠拉回；记录显示器设备名。
5. **虚拟桌面**：`IVirtualDesktopPinnedApps::PinWindow` COM（与任务栏"在所有桌面显示"同源）。接口不可用 → 显式 `unsupported` 状态并在 UI 标注，**不伪造成功**。
6. **全屏跟随**：`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 事件驱动 + `SHQueryUserNotificationState` + 前台窗口矩形==显示器矩形双重判定；每张便签独立策略（normal / always_top / fullscreen_show / fullscreen_hide）。
7. **图片**：存 `images/<note_id>/<uuid>.<ext>`，SQLite 只存元数据；支持拖拽(路径)/粘贴(RGBA→PNG)/选择文件；永久删除便签时级联清理，归档保留。
8. **链接**：`open_external` 白名单 http/https，其余拒绝。
9. **错误**：AppError 统一 {kind,message} 序列化；关键失败写 `errors/YYYY-MM-DD-<category>.md`（问题/原因/影响/解决方式）+ logs/focusly.log；禁止静默失败。

## 开发命令

```bash
npm install          # 前端依赖
npm run dev          # 仅前端（Vite, :1420）
npm run tauri dev    # 桌面开发（自动起 Vite + cargo build）
npm run tauri build  # Windows 构建产物（NSIS 安装包）
npm test             # vitest 单测（前端纯函数）
cargo test           # Rust 测试（src-tauri/ 下）
npm run icon         # 重新生成应用图标
```

## 环境要求（Windows）

- Node 18+、Rust stable-msvc、VS Build Tools 2022 (C++ 工作负载 + Windows SDK)、WebView2 运行时（Win11 自带）。

## 修改代码的红线

1. 不引入 Electron/Next.js/云服务/登录系统/PostgreSQL/Redis/Docker。
2. 不把业务逻辑写进 Rust 命令层，不把 SQL 写进前端。
3. 不新增 components/ui 之外的 UI 框架；不引入 `@tauri-apps/plugin-*` 之外的原生桥。
4. 迁移只增不改：新表/新列追加新 migration（MIGRATIONS 数组末尾 push）。
5. 所有新命令：Rust `commands/` 注册 → `lib.rs` generate_handler → `lib/api.ts` 封装 → 类型进 `types/index.ts`。

## 已知平台限制

- 独占全屏（D3D exclusive）下任何应用无法覆盖，fullscreen_show 仅对无边框全屏（浏览器 F11、无边框视频）有效。
- 虚拟桌面 Pin 依赖未公开 COM 接口（Windows 10 1809+ / Windows 11 验证可用）；大版本更新可能失效，此时状态为 `unsupported`，功能自动降级为"仅当前桌面"。
- Windows 通知点击聚焦便签依赖 toast 激活 COM 回调，MVP 阶段通知为提示型（用户从托盘/管理器打开便签）。
