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

## Round 3（02:45–02:48）
- 改动：搜索命中标签 N+1 消除——R1 的私有 tags_batch 提升为 `db::tags::for_notes`（pub，同序同果），
  db/search.rs 的 finish_hit（逐条查）改为 finish_hits（单条聚合查询，500/批）；
  FTS 与 LIKE 降级两条路径共用；db/notes.rs 列表路径改调共用函数（净 -29 行重复）。
  搜索是每次按键（250ms 防抖）触发的热路径，最多 50 命中此前 = 50 次标签查询。
- 测试：cargo test 215+5 通过，clippy 0 警告（清理了移动后残留的 unused import），vitest 294 通过。
  既有搜索测试断言 tags 结果与排序（fts_finds_keyword_substring_and_highlights）覆盖了等价性。

## 终止（2026-09-20 11:50）

- **触发停止规则：本地时间 11:50 >= 08:10，不再开新轮；按 08:55 硬停条款，最终日志只写不提交。**
- 原因：会话在 R3 后的休眠段（02:49→03:30）被中断，恢复时已过当日全部轮次窗口。
- 本文件与 NIGHTLY_PROGRESS.md 的终止段留在工作区未提交（规则禁止 08:55 后 git 操作）；
  后续可由人工并入或随下一批提交。

## Round 4（11:55–12:05，用户指示继续后的日间续轮）
- 审计闭环（R2 遗留线索）：前端 parseTodos 接受 `*`/`+` bullet，Rust 四处仅认 `-`——
  extract_task_lines（勾选同步）是唯一对齐点。对齐方向选「Rust 向前端看齐」：
  收紧前端会破坏已用 `* [ ]` 书写的存量便签，且 remark-gfm 预览本就渲染三类 bullet 为复选框。
- 改动：count_todos（卡片徽标）、list("todo") SQL 过滤（9 个 LIKE 分支）、
  todos_view::unchecked_task_text（今日/逾期/未来7天）、task_meta::mark_done_content_line
  （完成回写，bullet 原样保留）四处放宽到 `-`/`*`/`+`；AGENTS.md 待办真相源契约行同步。
- 测试：cargo 217 通过（新增 star_and_plus_bullets_count_as_tasks、
  mark_done_preserves_star_and_plus_bullets，扩展 todo_count/filters），clippy 0，vitest 294，build 通过。
- 备注：R1~R3 提交 + 本轮提交均在 auto/nightly/focusly-20260920-0115 分支；
  两份日志文件随本轮一并入库。

## Round 5（12:10–12:15）
- a11y 扫描结论：25/25 icon 按钮均有 aria-label/title（3 处疑似缺口实为扫描窗口截断），
  `<img>` 均有 alt，toast 已有 aria-live + error role=alert——前端 a11y 基线良好，无需改动。
- 改动：shortcut.rs（此前零测试）补 2 用例——迁移六条默认加速器经 accelerator_to_tauri
  归一化后必须 parse 成功为 Shortcut（注册期该失败只是静默 shortcut-error，测试期拦截格式漂移）；
  归一化 trim/小写/幂等。CHANGELOG.md 补记夜间系列 R1–R4 条目（合并回 main 前的文档收尾）。
- 测试：cargo 219 通过，clippy 0（修正测试模块位置触发的 items-after-test-module），
  vitest 294，build 通过。

## Round 6（12:20–12:25）
- 修复备忘清单标注「风险高、暂不做」的版本恢复编辑器不刷新问题——找到不碰事件链路的安全做法：
  - useNoteAutoSave 新增 `adoptExternal(title, content)`：清除防抖、把 latest/baseline 基线
    同时推到外部内容、复位 saved 标记（调用方先 adopt 再 setTitle/setContent，防抖 effect
    比对基线后判定无脏，不会把恢复内容再存一遍）。
  - NoteWindow 版本恢复回调从 refreshMeta（只刷元数据）改为 reloadAfterRestore：
    getNote 整篇重载 + adopt + setTitle/setContent。失败可见（toast + 错误日志）。
  - 语义确认：恢复确认框承诺「当前内容会先自动备份」，故放弃本地未保存修改符合预期语义。
  - 事件路径零改动——notes-changed content 事件仍被跳过，打字风暴优化不受影响。
- 测试：vitest 294、build、cargo 219、clippy 0 全绿。







