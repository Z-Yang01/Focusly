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

**P0（全部）**：创建便签 · 编辑便签 · 自动保存（防抖+关窗flush+崩溃保护） · 删除/归档/恢复 · 自由调整大小 · 拖动窗口 · 置顶 · 显示/隐藏全部 · SQLite（7 表 + 迁移） · Markdown 编辑/预览 · Todo 勾选 · 图片（拖/粘/选） · Windows 通知 · 基础快捷键 · System Tray

**P1（全部）**：多虚拟桌面显示（COM Pin，含显式降级） · 全屏跟随（4 种策略） · 自定义快捷键（冲突检测/恢复默认） · 搜索 · 标签 · 提醒重复（每天/每周/工作日） · JSON 导入导出 · 深色模式

### 4. 尚未实现（P2 及以后）

- 历史版本记录 / 内容回滚
- 待办拖动排序、多级待办折叠
- 每日统计与复盘视图
- 提醒的月/年重复规则、通知点击直达便签窗口
- ZIP / Markdown 批量导出
- FTS5 全文索引（当前 LIKE 搜索，个人数据量足够）

### 5. 数据库 Schema

7 张表，见 `src-tauri/src/db/migrations.rs`（PRAGMA user_version 迁移）：

- `notes` — id, title, content, content_format, status(active/archived), is_pinned, is_always_on_top, show_on_all_desktops, desktop_pin_state, fullscreen_behavior, x, y, width, height, monitor_id, created_at, updated_at, archived_at
- `note_images` — id, note_id→CASCADE, path, filename, width, height, created_at
- `reminders` — id, note_id→CASCADE, remind_at, repeat_type(once/daily/weekly/weekdays), status(pending/triggered/dismissed/done/cancelled), created_at, triggered_at
- `tags` — id, name(UNIQUE), created_at；`note_tags` — note_id, tag_id（级联清理）
- `shortcuts` — action(PK), accelerator, enabled, updated_at
- `settings` — key(PK), value

### 6. Rust / React 架构

见 [AGENTS.md](./AGENTS.md)「职责划分」与「架构决策记录」。

### 7. 测试结果

| 项 | 结果 |
|---|---|
| `npx tsc --noEmit` | ✅ 0 错误 |
| `npx vitest run` | ✅ 3 个文件 41 个用例全部通过（todo 15 / editorActions 15 / shortcut 11） |
| Rust 单测（db CRUD / 迁移 / 提醒重复计算 / 备份轮转 / 错误日志） | 已编写，随 MSVC 工具链就绪执行 `cargo test` |
| 桌面端编译/运行 | ⏳ 等待 VS Build Tools 安装完成后验证 |
| 手动 E2E 三链路 | ⏳ 待桌面端可运行后执行 |

### 8. Windows 构建方法

```bash
npm run tauri build   # 产物：src-tauri/target/release/bundle/nsis/Focusly_0.1.0_x64-setup.exe
```

### 9. 已知问题 / 平台限制

- **独占全屏**（DirectX exclusive，多为全屏游戏）下任何窗口都无法覆盖，`fullscreen_show` 仅对无边框全屏（浏览器 F11、无边框视频）有效——Windows 平台机制限制。
- **虚拟桌面 Pin** 依赖未公开 COM 接口（Windows 10 1809+ / Windows 11 已验证的接口定义）；Windows 大版本更新可能使其失效，此时便签状态显示"系统不支持"，功能自动降级为"仅当前桌面"，不影响其他功能。
- **通知点击**暂不直达便签窗口（toast 激活需要额外的 COM activator），通知为提示型，用户从托盘/管理器打开便签。
- **粘贴图片**仅支持位图（截图/复制的图片）；从资源管理器复制的文件请用拖拽或"插入图片"。
- 导入 JSON 时图片文件不迁移（记录引用原路径），跨机器导入缺失图片会显示"图片缺失"占位。
