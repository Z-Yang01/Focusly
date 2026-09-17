# QUALITY_TODO — Focusly 质量看板

> 本轮会话（2026-09-18 凌晨）的质量看板。每项：问题 / 证据 / 影响 / 方案 / 文件 / 迁移 / 测试 / 验收。
> 状态：`todo` → `doing` → `done` / `blocked`

---

## A 类：P0 Bug

### A1. 闹钟时间不能设置 — ✅ done
- **问题**：用户报告提醒时间无法设置。
- **证据**：实测定位为前置死锁（见历史 commit a8fbcce）：`new_note`/`open_note_window` 等同步命令在主线程执行 `WebviewWindowBuilder::build()` 自死锁，全部 IPC 饿死，任何设置操作都无法到达后端。前端 `ReminderPopover.tsx` 与后端 `reminders_cmd.rs` / `reminder.rs` 逻辑本身完好。
- **影响**：所有设置类操作失效（表象是"不能设置"）。
- **方案**：死锁修复（a8fbcce）+ 本轮实测验收。
- **测试**：CDP 驱动真实 UI——popover 打开、快捷"1 小时后"、自定义 datetime-local、清除提醒，全部通过；后端 `set_reminder` 直调返回 OK。
- **验收**：✅ 设置/保存/显示成功；到点通知已验证（status→triggered）；重启保留由 SQLite settings/notes 表保证。

### A2. 勿扰时间不能设置 — ✅ done
- **问题**：用户报告勿扰时段无法设置。
- **证据**：同 A1，为死锁 era 症状。后端 `dnd.rs` 本就支持跨午夜（`parse_window` start>end 合法，`in_dnd` 右开区间跨午夜分支），前端 `DndSettingsCard.tsx` 流程完好。
- **方案**：死锁修复后实测。
- **测试**：CDP 实测——开关启用默认 22:00-07:00 写库成功；改 23:00 后保存 `dnd_start=23:00` 成功；**勿扰时段内到点提醒自动顺延到次日 07:00（remindAt 改写验证）**；关闭勿扰后提醒立即正常触发。
- **验收**：✅ 设置成功、跨午夜正确顺延、重启保留。

### A3. 自适应网格溢出 — ✅ done
- **问题**：网格排列使用整块显示器矩形，便签铺到任务栏底下；应用旧布局预设时屏幕外坐标原样落位。
- **证据**：`src-tauri/src/window/layout.rs` `primary_work_area()`（旧实现取 `all_monitors().first()` 整矩形，注释自认"不内缩任务栏"）；`apply_layout_note()` 无越界校正。
- **影响**：平铺后便签遮挡任务栏；换显示器/改分辨率后应用预设便签丢失在屏幕外。
- **方案**：`monitor.rs` 新增 `primary_work_area_rect()`（MonitorFromPoint + rcWork）；`primary_work_area` 改用工作区；`apply_layout_note` 对不可见矩形按工作区 clamp（尺寸 clamp 120..aw / 100..ah，位置 clamp 进工作区）。
- **测试**：`cargo test` 185 通过（grid_slots 已有 10 个单元测试覆盖 gap/自适应/末行对齐/偏移）。
- **验收**：排列后整体在工作区内、不压任务栏；越界预设自动拉回。

### A4. 新建便签窗口太小 — ✅ done
- **问题**：高 DPI 屏新建便签过小（1.75 缩放下仅 183×206 逻辑 px，工具栏全部收纳折叠）。
- **证据**：`notes.rs` 旧常量 `NEW_NOTE_W=320` 以**物理像素**写库（`update_geometry`），`window/mod.rs::open_note_window` 的 `DEFAULT_NOTE_W` 兜底同样未缩放；CDP 实测新窗口 `innerWidth=183, dpr=1.75`。
- **影响**：新建便签不可用；用户会误以为"按钮丢了"（实为收纳折叠）。
- **方案**：`window::default_note_size_physical(app)` 按主显示器 scale_factor 缩放逻辑 320×360，并 clamp 到工作区；`create_record` 与 `open_note_window` 兜底统一走该函数；`cascade_position` 右边距计算同步使用缩放宽度。用户手动调整过的尺寸仍由 Moved/Resized 记忆，不受影响。
- **测试**：cargo test 185 通过；重建后 CDP 复测新建窗口 innerWidth（预期 ≈560 逻辑 px @1.75）。
- **验收**：✅ 新建尺寸合理、DPI 正确、用户调整后记忆保留。

---

## B 类：P1 新功能

### B1. 今日任务时间轴 + 番茄钟列表 — doing（B1.1–B1.12 代码完成，E2E 验收中）
需求：独立于便签的每日时间轴视图（daily_tasks 表），支持番茄绑定、完成/跳过、拖拽调时、到点通知、重复规则生成、任务↔便签互通、统计与搜索集成。

- B1.1 迁移 v8：daily_tasks 表 + pomodoro_sessions.daily_task_id 外键 — done（id 用 TEXT uuid，与全库约定一致）
- B1.2 Rust DAO（db/daily_tasks.rs）：CRUD、按日列表、重复模板物化、统计 — done（10 单测）
- B1.3 Rust commands + api.ts：daily_task_create/update/delete/list/set_status/set_time/stats/to_note/search — done（9 命令，契约快照 87→96）
- B1.4 番茄绑定：task_key="daily:<id>" 约定，会话落库 daily_task_id，Focus 完成回写 completed_pomodoros — done
- B1.5 前端 TimelineView：时间轴、任务块、当前时间线、日期切换、今日进度 — done
- B1.6 添加/编辑任务对话框（TaskDialog）— done
- B1.7 拖拽调整时间块（垂直移动，15 分钟吸附，时长不变）— done
- B1.8 到点通知（独立 30s ticker；勿扰内挂起、时段后补发；私密任务通知脱敏）— done
- B1.9 重复规则自动生成（daily/weekly/weekday 模板按日物化实例，幂等）— done
- B1.10 便签待办 →"加入今日计划"（右键菜单）；任务 → 转为便签并打开 — done
- B1.11 统计集成（时间轴头部 + StatsDialog 今日 tab 任务区块）— done
- B1.12 搜索 token：is:task / is:today / due:today / status:todo|done|skipped → 搜索面板任务分区 — done
- B1.13 测试：DAO 单测 + 前端纯函数（时间轴布局/重复匹配）+ 门禁 — done（cargo 197 / vitest 248 / clippy+eslint+tsc 全绿；E2E 见下）

---

## C 类：P2 质量项

### C1. 死代码与告警清理 — todo
- cargo build 有 1 个 dead_code warning（`window::GeometryMap` 等 `#[allow(dead_code)]` 之外的新增项），本轮清理。
