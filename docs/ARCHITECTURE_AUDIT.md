# Focusly Rust 侧架构审计报告（ARCHITECTURE_AUDIT）

> 审计日期：2026-09-17（生产级收敛优化轮）
> 范围：`src-tauri/` 全部 Rust 代码。前端不在本报告范围。
> 验证：`cargo test` 全绿（166 基线 + 新增回归测试），`rustfmt --check --edition 2021` 干净，编译 0 warning。

---

## 1. 命令层（commands/）

**注册命令总数：87**（`lib.rs` generate_handler，与 `src/lib/api.ts` 封装一一对应）

| 文件 | 命令数 | 职责 |
|---|---:|---|
| `commands/notes_cmd.rs` | 26 | 便签 CRUD / 回收站 / 版本 / 隐私 / FTS 搜索 / 到期视图 / 每日笔记 / 图钉 |
| `commands/system_cmd.rs` | 14 | 显隐 / 退出 / 导入导出 / 外链 / 诊断 |
| `commands/pomodoro_cmd.rs` | 13 | 番茄钟状态机命令 + 统计 + task_meta |
| `commands/settings_cmd.rs` | 8 | 设置 / 快捷键 / 保存的搜索 |
| `commands/quickcapture_cmd.rs` | 8 | 速记箱窗口 + 剪贴板历史 |
| `commands/layout_cmd.rs` | 5 | 窗口布局预设 / 排列 |
| `commands/reminders_cmd.rs` | 6 | 提醒 CRUD / 自然语言解析 |
| `commands/images_cmd.rs` | 4 | 图片添加（路径/粘贴）/ 删除 / 存在性 |
| `commands/imagemgr_cmd.rs` | 3 | 重复检测 / 孤儿清理 / 缩略图 |

收敛结论：命令层全部为薄转发（`AppHandle/State` → 服务层函数），审计期间发现并移除了
`system_cmd::export_diagnostics` 内嵌的 6 条统计 SQL（迁移至 `db::health`），命令层现已零 SQL。

## 2. 数据层（db/）DAO 函数清单

| 文件 | pub fn 数 | 说明 |
|---|---:|---|
| `db/notes.rs` | 20 | 便签 CRUD/视图/版本快照/回收站/标题定位（本次 +2：`list_trash_ids`、`find_id_by_title`） |
| `db/task_meta.rs` | 11 | 任务三态（含 `sync_task_meta_from_content` 纯函数同步） |
| `db/pomodoro_sessions.rs` | 8 | 会话 + 统计（本次 +1：`clear_task_text_for_note`） |
| `db/reminders.rs` | 9 | 提醒 CRUD/状态流转/重复计算 |
| `db/images.rs` | 9 | 图片元数据 + best-effort 文件清理（本次 +1：`list_all`） |
| `db/mod.rs` | 5 | 连接管理：`open` / `with` / `tx`(新) / `health`(新) / `in_memory`(测试) |
| `db/clipboard.rs` | 6 | 剪贴板历史（上限 100，pinned 不淘汰） |
| `db/search.rs` | 4 | FTS5 同步/搜索/全量重建 |
| `db/versions.rs` | 3 | 版本列表/读取/恢复（恢复内含 pre-restore 快照） |
| `db/settings.rs` | 3 | KV 设置（DEFAULTS 合并读取） |
| `db/shortcuts.rs` / `db/layouts.rs` / `db/saved_searches.rs` / `db/tags.rs` | 各 4 | 小型 KV/关联 DAO |
| `db/migrations.rs` | 1 | `run()`：PRAGMA user_version 顺序迁移（v1→v7） |
| `db/missed.rs` | 1 | 错过提醒计数 |
| `db/todos_view.rs` | 1 | 到期视图聚合（列清单已收敛复用 `db::notes`） |
| `db/models.rs` | 5 | 模型 impl（RepeatType/FullscreenBehavior parse 等，非 SQL） |

**SQL 红线核验**：`grep execute(/prepare(/query_row(` 全库扫描，SQL 现仅存在于
`db/*`、`export.rs`（导入导出私有查询，AGENTS.md 允许）与测试模块内。
审计前散落在服务层/命令层的 6 处 SQL 已全部收敛进 DAO（见 §7）。

## 3. 事件发送点清单（Rust → JS）

事件名常量统一在 `src-tauri/src/events.rs`（与 `src/types/index.ts#EVENTS` 对应），共 9 个：

| 事件 | 发送点 | 触发场景 |
|---|---|---|
| `notes-changed` | `notes.rs::emit_notes_changed`（全部写路径）、`daily.rs`、`commands/reminders_cmd.rs` | 便签增删改/标签/导入后广播 |
| `settings-changed` | `commands/settings_cmd.rs` ×3 | 设置/快捷键/保存的搜索变更 |
| `reminder-fired` | `reminder.rs::fire_due` | 提醒触发 |
| `shortcut-error` | `shortcut.rs` ×3 | 快捷键解析失败/冲突/注册失败 |
| `notes-visibility` | `window/mod.rs` ×2 | 全部便签显隐切换 |
| `focus-search` | `shortcut.rs` | 全局快捷键聚焦搜索 |
| `open-settings` | `tray.rs` | 托盘菜单打开设置 |
| `pomodoro-state` | `pomodoro.rs::publish` + 私密脱敏重播 | 状态机每次变化全量广播 |
| `pomodoro-finished` | `pomodoro.rs` ×3 | 阶段自然结束 / 跳过 / 停止 |

所有 emit 均为 `let _ =`（UI 通知性质，失败不影响数据），符合分级策略。

## 4. 窗口生命周期

- **manager**：启动时按 `start_minimized` 决定显隐；关闭被拦截（`close_action`=tray 隐藏 / quit 退出）；单实例插件二次启动时 `show_manager` 唤起。
- **note-\<uuid\>**：每签一个真实窗口。一律 `visible:false` 创建 → 前端就绪调 `note_window_ready` → 应用几何 + show + focus（防白闪）。归档/删除/入回收站先 `close_note_window` 再落库。
- **quick-capture**：常驻隐藏窗口，`quickcapture_toggle` 显隐切换（不销毁，保留草稿）。
- **几何持久化**：Moved/Resized → `schedule_geometry_save`（800ms 去抖 + 代数裁决）→ `update_geometry`；`CloseRequested` 与 `RunEvent::Exit` 立即兜底保存；恢复时显示器相交校验，越界层叠拉回。
- **全屏跟随**：`SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` 钩子线程 + 消费者线程，仅状态变化时应用每签独立策略（normal/always_top/fullscreen_show/fullscreen_hide）。

## 5. 调度器生命周期

| 调度器 | 形态 | 启动 | 结束 |
|---|---|---|---|
| 提醒（reminder） | tokio task + `unbounded_channel` + `sleep_until` 精确等待 | `lib.rs` setup 尾部 `spawn_loop`（manage 之后，消除竞态） | 所有 `SchedulerHandle` drop（应用退出）后 `recv()` 返回 None 退出 |
| 番茄钟（pomodoro） | tokio task + mpsc + 60s 心跳（托盘刷新 + 睡眠误差限制） | `pomodoro::spawn` 在 manage **之前**拉起，handle 存入 AppState | 命令通道所有发送端 drop 后退出 |
| 全屏检测 | 两条 OS 线程（钩子线程跑消息泵 + 消费者线程） | setup 尾部 `foreground::spawn` | 进程退出（钩子随消息泵退出 Unhook） |
| 几何去抖 | 按需 tokio task（每次窗口事件一代，代数裁决弃旧） | 窗口 Moved/Resized | 800ms 后写库或被新一代取代 |

## 6. 全局状态与并发

`AppState` 字段：

| 字段 | 类型 | 锁策略 | 备注 |
|---|---|---|---|
| `db` | `Db(Mutex<Connection>)` | 互斥锁，`with`/`tx` 单入口 | `busy_timeout` 3s；WAL + synchronous=NORMAL |
| `paths` | `AppPaths` | 只读 | 启动时初始化 |
| `scheduler` | `SchedulerHandle` | 无锁（mpsc 发送端） | wake 唤醒重算 |
| `pomodoro` | `PomodoroHandle` | 无锁（mpsc 发送端） | send 失败显式报“调度器已停止” |
| `shortcut_map` | `Mutex<HashMap>` | 锁内整体替换 | 触发回调只读 |
| `fullscreen_hidden` | `Mutex<HashSet<String>>` | `lock_ok`（锁中毒恢复） | 记录被全屏策略隐藏的 label |
| `geometry_gens` | `Mutex<HashMap<String,u64>>` | 同上 | 去抖代数，仅最新代可写库 |

模块级静态（进程生命周期，设计使然）：`pomodoro::SNAPSHOT`（OnceLock\<Mutex\> 运行快照）、
`pomodoro::FULLSCREEN_WATCH`/`FULLSCREEN_HINT`（通知延迟主路径）、
`window::foreground::WINEVENT_TX`（钩子回调发送端）。

**潜在竞态（已核验，均为良性）**：

1. **几何写库 vs 窗口销毁**：去抖任务醒来时窗口可能已关闭 → `get_webview_window` 返回 None 直接放弃，无悬垂写。
2. **番茄钟快照 vs 会话表**：`SNAPSHOT` 是内存投影，DB 会话行写失败有 `log::error` 留痕；快照仅显示用，阶段判定以 `ends_at` 绝对时间为准。
3. **`empty_trash` 逐条删除**：读取 id 列表后循环 `delete_permanently`，期间 UI 并发操作最多导致后续条目报“不存在”并中止——单用户桌面场景可接受，未改动语义。
4. **`show_all_notes` 持锁窗口操作**：`fullscreen_hidden` 锁内调用 `win.show()`；均为快速非阻塞调用，无死锁路径（锁内不取 DB 锁）。
5. **Db::tx 与 with 不可重入**：`tx` 闭包内再取连接会死锁（Mutex 无重入）。已在 `db/mod.rs` 文档注释中声明红线；现有调用点均已核验为纯 DAO 调用。

**潜在泄漏（已核验，均为有界）**：

- 几何去抖 task：窗口拖动高频产生，但每个 800ms 内自灭，仅暂存 AppHandle 克隆。
- 番茄钟 `spawn_notice`：全屏等待硬顶 4 小时，自灭。
- quick-capture 窗口：隐藏不销毁，属功能设计（保留草稿）。
- 静态 OnceLock：进程级缓存，量级恒定。

## 7. 本次修复的问题（收敛轮改动清单）

### 7.1 事务边界（Task 1）

统一入口：`Db::tx()`（`db/mod.rs` 新增）——`unchecked_transaction()` 包裹，闭包 `Err` 即回滚、`Ok` 提交。

| 位置 | 修复 |
|---|---|
| `notes.rs::update_content` | 更新正文 + 版本快照 + FTS 同步 + task_meta 同步，原先各自 autocommit，中途崩溃会留下“正文已变而派生数据未同步”的撕裂状态 → 现整体一个事务。快照/FTS/task_meta 保持“失败仅告警不阻塞保存”既有语义 |
| `notes.rs::restore_note_version` | pre-restore 快照 + 写回 + FTS 同步 → 事务化（防恢复半途而废） |
| `notes.rs::create_record` | 建行 + 层叠几何 → 事务化（防有行无几何） |
| `notes.rs::delete_permanently` | FTS 清理 + task_meta 清理 + 删行 → 事务化（删行失败时前两者回滚） |
| `notes.rs::set_note_tags` | 标签重写循环、FTS 重同步 → 各自事务化 |
| `daily.rs::create_today` | 建行 + 几何 + 标签 → 事务化 |
| `reminder.rs::fire_due` | 置 triggered + 补建下一轮 pending → 事务化（防“已触发但下一轮丢失”） |
| `export.rs::import_from_file` | **核验已是事务**（unchecked_transaction + fts_rebuild_all + commit），失败自动回滚，无需改动 |

### 7.2 unwrap/expect 审查（Task 2）

生产代码现存 3 处，全部评估为安全保留：

1. `lib.rs::AppPaths::init(...).expect` —— 数据目录不可用时应用无法运行，fail-fast 且消息明确（logger 尚未初始化，无处可去）。
2. `lib.rs` builder `.expect("error while building focusly")` —— Tauri 标准启动模式。
3. `vdesktop.rs::guid(...).expect("合法 GUID")` —— 入参为三个编译期常量 GUID 字符串，格式固定。
4. `pomodoro.rs` / `db/pomodoro_sessions.rs` 的 `and_hms_opt(12,0,0).unwrap()` 回退 —— 12:00:00 恒为合法 NaiveTime，仅作 DST 悬空时段兜底，不会 panic。

修复 1 处：`window/mod.rs::save_geometry_now` 的 `note_id_from_label(...).unwrap()`（前置 is_none 检查保护）→ 改为 `let-else` 结构化收窄，保护关系由编译器保证而非人眼。

### 7.3 `let _ =` 静默失败分级（Task 3）

**升级为显式日志（高风险：DB/隐私/线程/可见性）**：

- `window/mod.rs::apply_desktop_pin` —— desktop_pin_state 落库失败原先完全无声 → `log::error`。
- `window/mod.rs::save_geometry_now` —— 几何落库失败 → `log::error`。
- `pomodoro.rs::scrub_private_text` —— 私密便签会话文本清理失败（隐私防线一环）→ `log::error`。
- `reminder.rs::SchedulerHandle::wake` —— 调度器死亡告警 → `log::warn`。
- `window/foreground.rs::spawn` —— 两条检测线程创建失败（全屏功能整体失效且无线索）→ `log::error`。
- `tray.rs` 菜单 show_all/hide_all/new_note —— 用户点击无反馈时日志是唯一线索 → `log::error`。
- `desktop_pin.rs` —— `EnumWindows`/`SetParent`/`SetWindowPos` 失败 → `log::warn`（此前结果直接丢弃）。

**保留 `let _ =`（低风险，失败无数据后果）**：全部 `app.emit`（通知性质）、窗口 show/hide/set_position/set_focus、`logger.rs`/`db/images.rs` best-effort 文件清理（函数名即契约）、`TranslateMessage`/`UnhookWinEvent`、`let _ = app;` 占位参数。

### 7.4 FTS5/搜索审计（Task 4）

- `search_fts`：关键词已用双引号短语包裹 + `""` 转义，经 `?1` 绑定 —— FTS5 语法字符（AND/OR/NOT/NEAR/*/^{）无法注入改变语义。**核验通过，新增回归测试** `fts_keyword_with_syntax_chars_is_literal`（含引号/星号/插字符关键词）。
- 空关键词：trim 后为空直接返回空集（不查库）。核验通过。
- 超长关键词：SQLite 参数绑定无长度风险，snippet 截断保护；未加人为上限（不改变行为）。
- 中文/emoji：trigram 分词对 unicode 生效，≥3 字符走 FTS、<3 字符降级 LIKE，均有测试覆盖。
- **修复**：LIKE 降级路径与 `db::notes::search` 未转义转义符 `\` 本身——含 `\` 的关键词会吞掉后续字符语义（如搜 `\` 实际变成搜 `%`）。现按 `\` → `\\`、`%` → `\%`、`_` → `\_` 顺序转义，新增测试 `fts_handles_cjk_emoji_and_like_escapes`。

### 7.5 查询与索引（Task 5）

- `db::notes::list()`：显式列清单（非 SELECT *），`content` 为 `count_todos` 统计所必需——**核验无冗余列**。
- **新增 v7 迁移**：`idx_notes_alive_order ON notes(is_pinned DESC, updated_at DESC) WHERE deleted_at IS NULL` 部分索引。v1 的 `idx_notes_status_updated` 建于 `deleted_at` 列出现之前，"all" 视图（`WHERE deleted_at IS NULL ORDER BY is_pinned DESC, updated_at DESC`）此前只能全表扫描 + 排序；部分索引与该 WHERE/ORDER BY 完全匹配且回收站行不入索引。迁移只增不改，符合红线。

### 7.6 架构红线收敛（SQL 出层清理）

| 原位置 | 去向 |
|---|---|
| `notes.rs::empty_trash` 内联 SELECT | → `db::notes::list_trash_ids` |
| `daily.rs::find_today` 内联 SELECT | → `db::notes::find_id_by_title` |
| `pomodoro.rs::scrub_private_text` 内联 UPDATE | → `db::pomodoro_sessions::clear_task_text_for_note` |
| `commands/system_cmd.rs::export_diagnostics` 内联 6 条统计 SQL | → `db::health` + `DbHealth` 结构（命令层只做 JSON 格式化） |
| `imagemgr.rs::load_rows` 内联 SELECT | → `db::images::list_all`（复用 NoteImage 行映射） |
| `db/todos_view.rs` 本地 `NOTE_COLS`/`row_to_note` 副本 | → 复用 `db::notes` 公开版本（消除列清单双源漂移） |

### 7.7 审计中发现的潜伏缺陷（已修复，属数据一致性而非行为变更）

1. **`set_pin_mode` 从不落库**：`db::notes::update()` 中 `pin_mode` 绑定错误地嵌套在 `if let Some(scale)` 之内——`scale` 为 None 时 `pin_mode` 被静默丢弃。图钉模式的窗口效果生效但选择不持久化，重启后回显 normal。已提升为独立绑定，并加回归测试 `update_persists_pin_mode_without_scale`。
2. **导出/导入丢失 pin_mode**：`export.rs::import_from_file` 的 INSERT 列清单停留在 v5（无 `pin_mode` 列，v6 建列后未同步），`INSERT OR REPLACE` 会把图钉模式重置为 'normal'。已补列；旧导出 JSON 无 pinMode 字段时按建表默认值落库（`#[serde(default)]` 兼容）。
3. **到期视图 pin_mode 恒为空串**：`db/todos_view.rs` 本地列清单缺少 v6 的 `pin_mode` 列，靠 `unwrap_or_default()` 掩盖。随列清单收敛一并修复。
4. **错位的 `#[allow(dead_code)]`**：`notes.rs` 中该属性写在了 `set_pin_mode`（在用）之前，而真正保留未接线的 `search_notes_v2` 反而无属性。已归位。
5. **编译告警清零**：基线 7 个 warning（3 个未用 Result + 4 个死代码）全部消除，死代码保留项补注 `#[allow(dead_code)]` 并说明保留理由。
6. **`pomodoro.rs::scrub_private_text` 文档注释错位**：继承自相邻函数（"全局快捷键 pomodoro_toggle…"），已改写为函数实际职责说明。

## 8. 遗留观察项（不改动，供后续决策）

1. `notes.rs::set_pin_mode` 持久化了 pin_mode，但启动路径（`startup_windows`/`open_note_window`）尚不根据 pin_mode 重放桌面嵌入——`desktop_pin_state` 才是窗口恢复的事实源。两列语义相近，未来可考虑合并。
2. `db/versions.rs::MAX_VERSIONS_PER_NOTE` 与 `snapshot_version` 清理 SQL 中的字面量 `LIMIT 50` 仍是双源（DAO 内部注释已声明约束）。
3. `quickcapture.rs`/`daily.rs` 文件头部的"总控接线附录"注释描述的是历史接线过程，接线完成后可精简。
4. `Db::tx` 与 `Db::with` 的不可重入约束靠文档约定，未来若出现嵌套需求需先改为每请求连接或引入 `parking_lot::ReentrantMutex` 类方案（当前无此需求，不引入依赖）。

## 9. 验证结果

```
cargo test   → 185 passed; 0 failed（166 基线全保留 + 新增回归测试 + 同仓并行轮次新增测试）
rustfmt --check --edition 2021（全部改动文件）→ 干净
cargo build（warning）→ 0
```
