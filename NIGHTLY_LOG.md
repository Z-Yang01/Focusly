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

