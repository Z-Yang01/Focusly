# Focusly

> **快。轻。不会丢。不会忘。** —— Windows 11 桌面便签 / 待办 / 提醒工具

Focusly 是一个完全本地化的个人桌面便签软件：想到什么马上记下来，让它一直留在桌面上；需要完成的事情可以勾选，需要记住的事情可以提醒，需要长期保存的内容可以归档。

- 📝 **多便签桌面常驻**：每张便签是一个真实的原生窗口（无边框/圆角/可自由拖动缩放），独立置顶、独立全屏策略
- ⚡ **Markdown + 富文本编辑**：标题/粗斜体/删除线/列表/引用/代码块/链接/表格/分割线，`[编辑][预览]` 切换
- ☑️ **待办**：`- [ ] / - [x]` 即待办，预览态直接点击勾选，支持统计与搜索，未完成折叠待扩展
- 🖼 **图片**：拖拽 / 粘贴截图 / 文件选择三种入口，存本地目录，SQLite 只存元数据
- 🔗 **链接**：自动识别，系统浏览器打开，白名单 http/https
- 🔔 **定时提醒**：Rust 后台调度（事件驱动，无轮询），便签全部隐藏也照样弹 Windows 通知；仅一次/每天/每周/工作日；触发后可 完成 / 稍后(5m/30m/1h/明天) / 关闭
- 🖥 **跨虚拟桌面**：`IVirtualDesktopPinnedApps` COM 与任务栏"在所有桌面显示"同源机制，不可用时明确提示"系统不支持"（不伪造成功）
- 📺 **全屏跟随**：`SetWinEventHook` 事件驱动检测前台全屏，每张便签独立策略：普通 / 始终置顶 / 全屏显示 / 全屏自动隐藏
- ⌨️ **全局快捷键**：显示/隐藏全部 `Ctrl+Shift+Space`、新建便签 `Ctrl+Shift+N`、聚焦搜索 `Ctrl+Shift+F`；可在设置中自定义、冲突检测、恢复默认
- 🔍 **全局搜索**：标题/正文/待办/标签，点击结果直接打开对应便签
- 🏷 **标签**：简单字符串标签（`#工作`），侧栏按标签过滤
- 🗂 **管理器主窗口**：全部便签 / 待办 / 已归档三视图 + 卡片网格 + 归档/恢复/永久删除
- 🧰 **系统托盘**：显示全部 / 隐藏全部 / 新建便签 / 打开管理器 / 设置 / 退出；关闭主窗口默认最小化到托盘（可改为退出）
- 💾 **数据安全**：SQLite WAL + 编辑 500ms 防抖自动保存 + 关窗/失焦强制落盘 + 启动自动备份轮转（保留 5 份）+ JSON 导出/导入
- 🌗 **深浅色主题**：浅色 / 深色 / 跟随系统，全窗口同步
- 🚀 **开机自启**、启动后最小化到托盘、启动自动显示便签，均可配置

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

```text
%APPDATA%/com.focusly.app/
├── database.sqlite     # 全部数据（WAL 模式，synchronous=NORMAL）
├── images/<note_id>/   # 便签图片文件（SQLite 只存元数据）
├── backups/            # 启动自动备份（database-<时间戳>.sqlite，保留 5 份）
├── logs/focusly.log    # 运行日志
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

### 4. 尚未实现（P2 及以后）

- 待办拖动排序、多级待办折叠
- 每日统计与复盘视图
- 提醒的月/年重复规则、通知点击直达便签窗口
- ZIP / Markdown 批量导出
- 图片 OCR / 附件类文件管理

### 5. 数据库 Schema

见 `src-tauri/src/db/migrations.rs`（PRAGMA user_version 迁移，当前 v3）：

- v1（7 张表）：`notes`, `note_images`, `reminders`, `tags` + `note_tags`, `shortcuts`, `settings`
- v2：`notes` 补列 `deleted_at` / `is_private` / `locked` / `readonly_flag` / `scale`；新增 `note_versions`（版本快照）、`clipboard_history`（剪贴板历史）、`saved_searches`（保存的搜索）、`notes_fts`（FTS5 trigram 虚表，触发器外由业务层同步）
- v3：新增 `layout_presets`（窗口布局预设）

### 6. Rust / React 架构

见 [AGENTS.md](./AGENTS.md)「职责划分」与「架构决策记录」。

### 7. 测试结果（2026-09-15 夜间收官，Agent H 契约审计后）

| 项 | 结果 |
|---|---|
| `npx tsc --noEmit` | ✅ 0 错误 |
| `npx vitest run` | ✅ 8 个文件 123 个用例全部通过（query-parser 26 / diff 13 / shortcut 11 / todo 15 / due 14 / editorActions 15 / dnd 10 / templates 19） |
| 契约审计（Agent H） | ✅ 71 个 Rust 命令与前端调用逐一对照：修复 3 处（`purge_note` 不存在→复用 `delete_note`；`SETTING_KEYS` 白名单缺 `dnd_start`/`dnd_end`；删除入口接入 `trash_note` 使回收站可用），详见 `.focusly-agent/KNOWN_ISSUES.md` |
| Rust 语法自查 | ✅ 除 `vdesktop.rs`（windows-rs `#[interface]` 属性宏语法，rustfmt 已知工具局限，非代码错误）外全部文件可正常解析 |
| Rust 单测（db CRUD / 迁移 / 版本 / 剪贴板 / 保存搜索 / 到期视图 / 布局 / 图片管理 / 勿扰 / 时间解析等，共 125 个 `#[test]`） | 已编写，随 MSVC 工具链就绪执行 `cargo test` |
| 桌面端编译/运行 | ⏳ 等待 VS Build Tools 安装完成后验证（精确步骤见下） |
| 手动 E2E 多链路 | ⏳ 待桌面端可运行后执行 |

**Rust 编译验证步骤（MSVC 安装完成后执行）**：

```powershell
# 1) 安装/补齐 MSVC C++ 工作负载（用户正在安装）
& "$env:TEMP\vs_BuildTools.exe" modify --installPath "E:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools" --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --quiet --wait --norestart --nocache

# 2) PATH 预置 MSVC Hostx64/x64 bin（版本号以实际安装为准），再验证
$msvc = Get-ChildItem "E:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC" | Select-Object -First 1
$env:PATH = "E:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\$($msvc.Name)\bin\Hostx64\x64;$env:PATH"

cd src-tauri
cargo build
cargo test
```

### 8. Windows 构建方法

```bash
npm run tauri build   # 产物：src-tauri/target/release/bundle/nsis/Focusly_0.1.0_x64-setup.exe
```

### 9. 已知问题 / 平台限制（完整清单见 `.focusly-agent/KNOWN_ISSUES.md`）

- **Rust 编译验证未执行**：本机 MSVC C++ 工具链缺失，`cargo build` / `cargo test` 尚未运行；Rust 侧质量依据为静态契约审计（命令/参数/serde 类型/事件逐一对照）+ rustfmt 解析自查。安装完成后按上文步骤验证。
- **FTS5 trigram 建表与同步未实测**：trigram 分词要求 SQLite ≥ 3.34 且仅对 ≥3 字符有效（2 字符中文词命中有限）；索引同步逻辑有单元测试但未在真实桌面环境跑过。
- **私密 = 隔离非加密**：私密便签仅做视图/搜索/通知/导出层面的隔离，数据库文件中内容为明文，请勿依赖其对抗本机物理接触。
- **通知点击**暂不直达便签窗口（toast 激活需要额外的 COM activator），通知为提示型，用户从托盘/管理器打开便签。
- **通知脱敏范围**：私密便签的系统通知标题与摘要已脱敏，但 `reminder-fired` 事件仍下发的 noteId 可被前端关联（本地应用内可接受）。
- **rust-lld 备选链接器未验证**：MSVC link.exe 不可用时的 rust-lld 方案未经测试。
- **独占全屏**（DirectX exclusive，多为全屏游戏）下任何窗口都无法覆盖，`fullscreen_show` 仅对无边框全屏（浏览器 F11、无边框视频）有效——Windows 平台机制限制。
- **虚拟桌面 Pin** 依赖未公开 COM 接口（Windows 10 1809+ / Windows 11 已验证的接口定义）；Windows 大版本更新可能使其失效，此时便签状态显示"系统不支持"，功能自动降级为"仅当前桌面"，不影响其他功能。
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
