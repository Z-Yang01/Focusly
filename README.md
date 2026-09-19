# Focusly

> **快。轻。不会丢。不会忘。** —— Windows 11 桌面便签 / 待办 / 提醒工具

Focusly 是一个完全本地化的个人桌面便签软件：想到什么马上记下来，让它一直留在桌面上；需要完成的事情可以勾选，需要记住的事情可以提醒，需要长期保存的内容可以归档。

- 📝 **多便签桌面常驻**：每张便签是一个真实的原生窗口（无边框/圆角/可自由拖动缩放），独立置顶、独立全屏策略
- ⚡ **Markdown + 富文本编辑**：标题/粗斜体/删除线/列表/引用/代码块/链接/表格/分割线，`[编辑][预览]` 切换
- ☑️ **待办**：`- [ ] / - [x]` 即待办，预览态直接点击勾选，支持拖动排序（只改展示序存 task_meta，正文真相源不变）、统计与搜索
- 🖼 **图片**：拖拽 / 粘贴截图 / 文件选择三种入口，存本地目录，SQLite 只存元数据
- 🔗 **链接**：自动识别，系统浏览器打开，白名单 http/https
- 🔔 **定时提醒**：Rust 后台调度（事件驱动，无轮询），便签全部隐藏也照样弹 Windows 通知；仅一次/每天/每周/工作日/每月/每年；触发后可 完成 / 稍后(5m/30m/1h/明天) / 关闭
- ⏱ **番茄钟**：专注/短休/长休状态机（tokio 事件驱动），便签底栏/迷你悬浮窗/管理器顶栏控制条/托盘同步；绑定便签任务或今日任务，统计番茄数/专注时长/中断次数；每任务专注时长可单独覆盖（默认 25 分钟）
- 📅 **今日任务时间轴**：独立于便签的每日计划——时间轴任务块、拖拽调时（15 分钟吸附）、到点通知（勿扰内挂起、时段后补发）、重复规则按日物化（每天/每周/工作日/每月/每年，月/年按锚点日号 clamp 到月末）、任务↔便签互通、搜索 token（`is:task` / `due:today` 等）
- 🖥 **跨虚拟桌面**：`IVirtualDesktopPinnedApps` COM 与任务栏"在所有桌面显示"同源机制，不可用时明确提示"系统不支持"（不伪造成功）
- 📺 **全屏跟随**：`SetWinEventHook` 事件驱动检测前台全屏，每张便签独立策略：普通 / 始终置顶 / 全屏显示 / 全屏自动隐藏
- ⌨️ **全局快捷键**：显示/隐藏全部 `Ctrl+Shift+Space`、新建便签 `Ctrl+Shift+N`、聚焦搜索 `Ctrl+Shift+F`；可在设置中自定义、冲突检测、恢复默认
- 🔍 **全局搜索**：标题/正文/待办/标签，点击结果直接打开对应便签
- 🏷 **标签**：简单字符串标签（`#工作`），侧栏按标签过滤
- 🗂 **管理器主窗口**：全部便签 / 待办 / 已归档三视图 + 卡片网格 + 归档/恢复/永久删除
- 🧰 **系统托盘**：显示全部 / 隐藏全部 / 新建便签 / 打开管理器 / 设置 / 退出；关闭主窗口默认最小化到托盘（可改为退出）
- 💾 **数据安全**：SQLite WAL + 编辑 500ms 防抖自动保存 + 关窗/失焦强制落盘 + 启动自动备份轮转（保留 5 份）+ JSON 导出/导入
- 🌗 **多主题**：浅色 / 深色 / 暖阳 / 森林 / 海洋 / 跟随系统，色卡预览选择器，全窗口同步
- 🚀 **开机自启**、启动后最小化到托盘均可配置；启动默认只开管理器（不自动弹出全部便签），「启动后自动显示便签」可在设置开启

## 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | Tauri 2（tray / notification / global-shortcut / dialog / opener / autostart / single-instance / clipboard-manager 插件） |
| 后端 | Rust · rusqlite(SQLite WAL) · serde · chrono · tokio · image · windows(Win32/COM) |
| 前端 | React 18 · TypeScript · Vite 6 · Tailwind CSS v4 · shadcn/ui 风格组件 · Radix UI · Zustand · react-markdown + remark-gfm · Lucide |

禁止引入：Electron / Next.js / 任何服务端 / 登录 / 云同步 / PostgreSQL / Redis / Docker。

## 快速开始（开发）

```bash
# 前置要求（Windows）
# 1) Node.js 18+        2) Rust stable (msvc)      3) VS Build Tools 2022（"使用 C++ 的桌面开发"工作负载，含 Windows SDK）
# 4) WebView2 运行时（Windows 11 自带）

npm install
npm run tauri dev      # 桌面开发模式（自动启动 Vite :1420 + cargo build）
```

其他命令：

```bash
npm test               # 前端单测（vitest：todo/editor/shortcut 纯函数）
cargo test             # Rust 单测（在 src-tauri/ 下执行）
npm run build          # 仅构建前端产物
npm run tauri build    # Windows 发布构建（NSIS 安装包）
npm run icon           # 重新生成全套应用图标（assets/icon.png → icons/*）
```

## 数据目录

安装/便携模式数据跟随 exe 目录（`<exe目录>/data/`，由安装器创建的 `data/` 或 `portable.marker` 触发，开发目录自动排除）；否则回退 `%APPDATA%/com.focusly.app/`：

```text
<数据目录>/
├── database.sqlite     # 全部数据（WAL 模式，synchronous=NORMAL）
├── images/<note_id>/   # 便签图片文件（SQLite 只存元数据）
├── backups/            # 启动自动备份（Online Backup API 原子快照，保留 5 份）
├── logs/focusly.log    # 运行日志（5MB 轮转）
└── errors/             # 结构化错误日志 YYYY-MM-DD-<category>.md（问题/原因/影响/解决方式）
```

架构与开发规范见 [AGENTS.md](./AGENTS.md)。

---

## 交付说明（v0.1.0）

### 1. 项目目录结构

见 [AGENTS.md](./AGENTS.md)「目录结构」章节。

### 2. 使用的技术栈

见上表。

### 3. 已实现功能

**P0（全部）**：创建便签 · 编辑便签 · 自动保存（防抖+关窗flush+崩溃保护） · 删除/归档/恢复 · 自由调整大小 · 拖动窗口 · 置顶 · 显示/隐藏全部 · SQLite（迁移框架） · Markdown 编辑/预览 · Todo 勾选 · 图片（拖/粘/选） · Windows 通知 · 基础快捷键 · System Tray

**P1（全部）**：多虚拟桌面显示（COM Pin，含显式降级） · 全屏跟随（4 种策略） · 自定义快捷键（冲突检测/恢复默认） · 搜索 · 标签 · 提醒重复（每天/每周/工作日） · JSON 导入导出 · 深色模式

**夜间批（11 个功能 agent 合并，已过前端契约审计）**：

- **数据层 v2**：回收站（软删除 `deleted_at`）/ 版本历史快照（auto/manual/pre-restore）/ 私密标记（`is_private`/`locked`/`readonly`/`scale` 列）/ FTS5 trigram 全文索引（建/改/删/恢复自动同步）/ 剪贴板历史表 / 保存的搜索表
- **回收站视图**：列表 / 单条恢复 / 批量恢复 / 单条永久删除 / 清空回收站（均有确认）
- **版本历史面板**：版本列表 + LCS 逐行差异对比 + 一键回滚（回滚前自动 pre-restore 快照）
- **FTS5 搜索 v2**：`tag:` / `is:pinned` / `is:private` / `has:image` / `has:todo` / `due:` 语法解析 + `<mark>` 高亮摘要 + 搜索栏 v2
- **命令面板**：`Ctrl+K` 全局命令入口
- **快速捕获窗**：独立 `quick-capture` 窗口速记 → 收件箱（只建行不开窗）
- **剪贴板历史**：条目列表 / 固定（pinned）/ 删除 / 清空，上限 100 条
- **中文时间解析**：自然语言提醒时间（"明天下午三点" 等），22 个解析用例
- **今日/逾期视图**：扫描 `^YYYY-MM-DD` 到期任务，逾期 / 今天 / 未来 7 天三分区（私密便签排除）
- **私密空间**：私密便签视图 / 隐私标志设置 / 通知脱敏（私密便签提醒不下发原标题与内容）/ 导出过滤 / 到期视图与 FTS 默认排除私密
- **窗口布局**：网格排列纯函数 + 布局预设保存 / 应用 / 删除（迁移 v4 `layout_presets`）
- **模板**：模板套用到新建便签
- **每日笔记**：`daily_get_or_create` 按本地日期幂等创建，管理器一键入口
- **图片管理**：SHA-256 重复图片检测 / 孤儿文件清理 / 缩略图生成
- **勿扰时段**：`dnd_start`/`dnd_end` 设置（跨午夜支持、fail-open），勿扰期间提醒推迟到时段结束；启动时汇总错过的提醒

**收敛与功能批（0.1.0 之后，2026-09-16 ~ 09-18）**：

- **P0 修复**：开窗命令主线程死锁（全部设置类操作失效的根因）；Tauri ACL 权限（allow-destroy / allow-set-position / allow-read-text）；网格平铺改用工作区 + 越界 clamp；新建便签默认尺寸按 DPI 缩放
- **B1 今日任务时间轴 + 番茄绑定**（迁移 v8）：TimelineView、拖拽调时、到点通知、重复物化、任务↔便签互通、统计与搜索集成（契约 87→96 命令）
- **番茄收尾**：管理器顶栏常驻番茄控制条 + 托盘番茄菜单；每任务专注时长覆盖（迁移 v9）
- **平台/数据**：数据目录跟随安装目录（便携/安装自动检测）；备份改 Online Backup API 原子快照 + 恢复演练
- **UX/主题**：R3 全量优化（工具栏智能收纳/预览沉浸/管理器密度）、六主题 + 色卡、提示音系统、快速待办创建栏、统一 toast + ErrorBoundary + 设计令牌

### 4. 尚未实现（P2 及以后）

- 多级待办折叠
- 复盘视图（周/月维度聚合；每日统计已落地：统计对话框任务区块 + 时间轴今日进度）
- 提醒的月/年重复规则、通知点击直达便签窗口
- ZIP / Markdown 批量导出
- 图片 OCR / 附件类文件管理

### 5. 数据库 Schema

见 `src-tauri/src/db/migrations.rs`（PRAGMA user_version 迁移，当前 v11）：

- v1（7 张表）：`notes`, `note_images`, `reminders`, `tags` + `note_tags`, `shortcuts`, `settings`
- v2：`notes` 补列 `deleted_at` / `is_private` / `locked` / `readonly_flag` / `scale`；新增 `note_versions`（版本快照）、`clipboard_history`（剪贴板历史）、`saved_searches`（保存的搜索）、`notes_fts`（FTS5 trigram 虚表，触发器外由业务层同步）
- v3：新增 `layout_presets`（窗口布局预设）
- v4：快速捕获全局快捷键默认绑定（`quick_capture` = Ctrl+Shift+Q）
- v5：番茄钟——`pomodoro_sessions`（会话）+ `task_meta`（便签任务元数据）+ 7 个 `pomo_*` 默认设置 + `pomodoro_toggle` 快捷键（Ctrl+Shift+P）
- v6：便签图钉三态 `pin_mode`（normal / topmost / desktop）
- v7：列表主路径覆盖索引 `idx_notes_alive_order`（all 视图走出全表扫描，回收站行不进索引）
- v8：今日任务时间轴——`daily_tasks` 表 + `pomodoro_sessions.daily_task_id` 外键
- v9：每任务专注时长覆盖——`task_meta.focus_min` / `daily_tasks.focus_min`（NULL = 跟随全局 `pomo_focus_min`）
- v10：待办拖动排序——`task_meta.sort_order`（1..n；NULL = 未排序按内容顺序兜底；只决定展示顺序，不回写正文）
- v11：月/年重复提醒的原始锚点——`reminders.anchor_at`（链式触发从锚点重算，防止月末 clamp 后日号永久漂移）

### 6. Rust / React 架构

见 [AGENTS.md](./AGENTS.md)「职责划分」与「架构决策记录」。

### 7. 测试结果（2026-09-19 门禁实测）

| 项 | 结果 |
|---|---|
| `npx tsc --noEmit` | ✅ 0 错误 |
| `npm test`（vitest） | ✅ 16 文件 275 用例全部通过 |
| `cargo test`（src-tauri/） | ✅ 207 用例全部通过（连跑 3 次稳定） |
| `cargo clippy --all-targets` | ✅ 0 告警（`cargo fmt --check` 同步 clean） |
| `npm run lint`（eslint） | ✅ 通过（--max-warnings 200） |
| `npm run contract-audit` | ✅ 99 个 Rust 命令与前端封装逐一对照，无漂移 |
| 桌面端 E2E | ✅ CDP 驱动真实 UI 验收：拖拽排序持久、复盘报表渲染、时间轴专注块、任务完成联动停番茄（数据库副本上执行） |
| 备份恢复演练 | ✅ 演练测试（备份→破坏→还原→比对）+ 真实备份目录 5 份轮转、`SQLite format 3` 头校验 |

> 2026-09-15 静态契约审计期的历史结果与 MSVC 工具链安装步骤已随门禁全绿失效，见 git 历史。

### 8. Windows 构建方法

```bash
npm run tauri build   # 产物：src-tauri/target/release/bundle/nsis/Focusly_0.1.0_x64-setup.exe
```

### 9. 已知问题 / 平台限制（完整清单见 `.focusly-agent/KNOWN_ISSUES.md`）

- **FTS5 trigram 字符数限制**：trigram 分词要求 SQLite ≥ 3.34 且仅对 ≥3 字符有效（2 字符中文词命中有限，自动降级 LIKE）；索引同步有单元测试，全文搜索路径已随 E2E 在真实 UI 验收。
- **私密 = 隔离非加密**：私密便签仅做视图/搜索/通知/导出层面的隔离，数据库文件中内容为明文，请勿依赖其对抗本机物理接触。
- **通知点击**暂不直达便签窗口，通知为提示型——评估结论（2026-09-18）：官方 tauri-plugin-notification 在 Windows 端不暴露 toast 点击/激活事件（上游 plugins-workspace #2150 未实现）；自实现需绕过插件直用 WinRT toast `on_activated`，依赖开始菜单快捷方式/AUMID 注册（便携/免安装场景不可靠），且需重接提醒/今日任务/番茄共 4 处通知调用，成本收益不匹配，暂不做。替代路径：提醒触发后应用内 Banner 可直接 完成/稍后/关闭；托盘「显示全部」/ 管理器 / `Ctrl+K` 均可直达便签。
- **通知脱敏范围**：私密便签的系统通知标题与摘要已脱敏，但 `reminder-fired` 事件仍下发的 noteId 可被前端关联（本地应用内可接受）。
- **独占全屏**（DirectX exclusive，多为全屏游戏）下任何窗口都无法覆盖，`fullscreen_show` 仅对无边框全屏（浏览器 F11、无边框视频）有效——Windows 平台机制限制。规避：将目标应用切换为「无边框窗口 / 窗口化全屏」显示模式，便签即可覆盖。
- **虚拟桌面 Pin** 依赖未公开 COM 接口（`IVirtualDesktopPinnedApps`，Windows 10 1809+ / Windows 11 已验证；手工 vtable 实现见 `vdesktop.rs`）。Windows 大版本更新可能使其失效，降级链路已核实：`CoCreateInstance`/`QueryService`/状态查询任一失败 → `VdError::Unsupported` → `desktop_pin_state="unsupported"` 落库 + 警告日志（`window/mod.rs` `apply_desktop_pin`），便签 UI 显示「系统不支持」，功能降级为"仅当前桌面"——不崩溃、不伪造成功、不影响其他功能；若固定操作本身失败则记 `failed`，下次窗口重建/重启时按落库状态重试。
- **粘贴图片**仅支持位图（截图/复制的图片）；从资源管理器复制的文件请用拖拽或"插入图片"。
- 导入 JSON 时图片文件不迁移（记录引用原路径），跨机器导入缺失图片会显示"图片缺失"占位。

### 10. 回滚方案

夜间开发共 11 个功能 commit + 总控接线，逐一可独立回滚（`git log --oneline` 对照）：

```bash
# 查看夜间 commit 列表
git log --oneline deef5c9..HEAD

# 单功能回滚（示例：回收站/版本历史视图）
git revert 1258963

# 总控接线 commit 与功能 commit 分开提交过，回滚功能 commit 后
# 若仍有残留接线，一并 revert 对应的 "总控接线" commit 即可
# 全部夜间改动回滚到基线：
git reset --hard deef5c9   # 仅当确定放弃全部夜间成果时使用（会丢弃工作区改动）
```

| 功能 | commit |
|---|---|
| 数据层 v2（回收站/版本/私密/FTS/剪贴板/保存搜索） | `699fe73` |
| 搜索语法 + 命令面板 | `7d1dffa` |
| 回收站视图 + 版本历史面板 | `1258963` |
| 快速捕获 + 剪贴板历史 | `a837a3a` |
| 窗口布局 | `3d3cc68` |
| 中文时间解析 + 逾期视图 + 私密防线 | `e31ff82` |
| 模板 + 每日笔记 + 图片管理 | `e5a8e3e` |
| 勿扰时段 + 错过汇总 | `4f8cdf5` |

契约审计修复（未提交时位于工作区）：`src/features/archive/api.ts`、`src/lib/api.ts`、`src/features/notes/NoteCard.tsx`、`src/features/notes/NoteWindow.tsx`、`src-tauri/src/commands/settings_cmd.rs`。
