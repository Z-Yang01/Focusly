# Nightly Log — Focusly（2026-09-20 夜）

## 启动
- 01:15 工作区干净，main @ ab6ca3d，创建分支 auto/nightly/focusly-20260920-0115
- 基线测试状态：vitest 294 通过（上一会话全绿验证），cargo test 215+5 通过

## Round 1（01:15–01:18）
- 改动：`src-tauri/src/notes.rs` empty_trash 批量化——单事务删全部回收站行（FTS/task_meta 同事务，
  个别失败跳过不阻塞），窗口关闭/提醒取消/番茄解绑/图片清理逐条 best-effort，全部完成后只发一次
  notes-changed（此前 N 条 = N 事务 + N 事件 + N 次前端失效级联）。返回值改为实际删除数。
- 测试：cargo test 215+5 通过，clippy 干净，vitest 294 通过（CI=1, --maxWorkers=2）
- 备注：CHANGELOG 条目待批次合并回 main 时统一补记（夜间小步不逐条写）。

## Round 2（02:00–02:04）
- 审计结论：P3（三列表按视图惰性拉取）放弃——侧栏「全部便签/待办/已归档」计数直接依赖三个
  全量列表（ManagerWindow.tsx:249-253），按视图 gate 会让计数变 0；替换为专用计数命令超出本轮预算。
- 修复：待办视图 SQL 过滤只认 `- [x]`，而 count_todos 与前端解析器均认大写 `- [X]`
  （GFM 合法写法）——仅含大写勾选的便签卡片显示待办徽标却不进待办视图。补 `LIKE '%- [X]%'`
  分支 + filters 测试扩展（db/notes.rs）。
- a11y：ReminderBanner 容器补 role="alert"（提醒横幅出现即被读屏播报）。
- 测试：cargo test 215+5 通过，clippy 干净，vitest 294 通过。


