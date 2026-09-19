# Changelog

所有显著变更记录于此文件。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [Unreleased]

### 质量优化（Q1–Q4 + Q6）

#### 数据安全
- 备份改为 SQLite Online Backup API 原子快照（修复裸拷贝 WAL 不一致风险）
- 新增备份恢复演练测试（备份→破坏→还原→数据完整比对）
- 导入后自动重建 FTS5 索引（修复导入数据搜索不可见）
- 导出补齐 v2 全部字段（回收站/私密/锁定/只读/scale）
- 正文勾选状态自动同步 task_meta（双向真源统一）
- task_meta 永久删除时级联清理

#### 调度正确性
- 番茄钟 60s 心跳限幅（系统睡眠唤醒后调度漂移 ≤60s，立即按 UTC 重算）
- 全屏事件接线（FullscreenChanged 事件驱动，通知延迟从最坏 2s 轮询降至毫秒级）
- 退出时几何兜底（RunEvent::Exit 遍历全部便签窗口写库）
- 便签中途设为私密时清洗番茄运行快照与会话表残留

#### UI/UX
- 新增设计令牌中心 `src/design-tokens.ts`（spacing/radius/duration/easing/zIndex/breakpoint）
- 全局可访问性提升：14 文件 aria-label 补齐、键盘导航（方向键卡片网格/Escape 预览退出/Tab 任务清单）、焦点环统一、减少动效支持、对比度验证
- 骨架屏加载态（NoteWindow）
- 统一 EmptyState 组件（六视图差异化文案）
- NoteCard 悬停效果（Fluent 风格 shadow+border）
- 搜索框拆分为 SearchBar + SearchResults + SyntaxHints + RecentSearches 四组件
- 速记箱键盘增强（Ctrl+Tab 切换、aria 全覆盖）
- 迷你番茄窗 SVG 进度环 + 阶段切换颜色过渡
- 模板按钮归位顶栏
- 拖拽图片 overlay 虚线边框增强
- 标签过滤条内容区顶部显示
- 视图切换淡入动画

#### 可观测性
- 统一 toast 通知系统（48 处原生 alert 全量迁移，error/success/info 三型）
- 全局 ErrorBoundary（白屏兜底 + 重试入口）
- 日志 5MB 自动轮转
- 诊断包导出（环境 + DB 健康 + 日志尾部，零内容泄漏）
- 8 处静默失败补日志（FTS/几何/标签/快捷键）

#### 安全与维护
- vitest 2→4.1.11 升级（清除 critical/high dev 依赖漏洞）
- window/mod.rs 锁中毒容错统一（lock_ok 辅助，消除 7 处级联风险）
- 契约审计脚本化（86 命令逐条核对，`npm run contract-audit`）
- vite 分包（单 578KB → react_vendor/markdown/app 三块并行加载）

#### 已知问题
- 独占全屏（DirectX）无法覆盖——Windows 平台限制
- 私密空间为隔离脱敏模式，完整加密为后续版本
- 通知点击暂不直达便签窗口
- vitest 4.x `toBe` 不支持第二参数 message

### B1 批次与门禁欠账（2026-09-17 ~ 09-18）

#### Added
- 今日任务时间轴（迁移 v8）：独立于便签的每日计划视图——时间轴任务块、当前时间指示、拖拽调时（15 分钟吸附）、日期切换、今日进度
- 今日任务到点通知：独立 30s ticker，勿扰时段内挂起、时段结束后补发，私密任务通知脱敏
- 重复规则按日物化：每天/每周/工作日模板幂等生成当日实例（source_task_id 溯源）
- 任务↔便签互通：便签待办右键「加入今日计划」；今日任务一键转便签
- 搜索任务分区：is:task / is:today / due:today / status:todo|done|skipped token
- 番茄绑定今日任务：task_key="daily:<id>"，完成回写 completed_pomodoros
- 番茄控制条：管理器顶栏常驻 + 托盘番茄菜单（修复番茄钟"无处关闭"）
- 每任务专注时长覆盖（迁移 v9）：task_meta/daily_tasks.focus_min，NULL 跟随全局 25 分钟
- 便签图钉三态 pin_mode（迁移 v6）：normal / topmost / desktop
- 列表覆盖索引 idx_notes_alive_order（迁移 v7）：all 视图走出全表扫描
- 快速待办创建：管理器待办视图顶部输入栏，Enter 一键创建便签+待办
- 提示音系统：Web Audio 合成（番茄完成/提醒叮咚）+ 设置开关
- 多主题系统：浅/深/暖阳/森林/海洋/跟随系统 + 色卡预览选择器
- 数据目录跟随安装目录：安装/便携（data/ 或 portable.marker）自动检测，开发版回退 %APPDATA%
- 待办拖动排序（迁移 v10）：便签预览态拖拽待办，展示序写 `task_meta.sort_order`，正文真相源不变；
  带续行/嵌套的复杂任务项自动禁用拖拽；排序经页面重载持久化（CDP 实测）
- 月/年重复规则：今日任务 repeat_rule 与提醒 repeat_type 同步扩展 monthly/yearly，
  月末/闰年按锚点日号 clamp 到当月最后一天（1-31 → 2-28；引擎从锚点年月重算，避免连续 clamp 漂移）
- P2 批次后命令契约 96 → 97（新增 task_meta_reorder）

### 番茄复盘批次（2026-09-19）

#### Added
- 专注复盘报表（StatsDialog 新增「复盘」tab）：按周/按月 + 上/下一期导航——逐日专注分钟条形图、
  合计卡（番茄数/时长/完成率/中断次数）、最专注任务 Top5（含今日/便签来源标注）、
  中断原因分布（manual/skip/task_done/switch_task/app_exit 归一 + 其他）、24 小时高发时段直方图
- 时间轴「实际专注块」：当日已完结 focus 会话渲染为右侧竖槽细条（中断灰色），悬停显示时刻/任务/时长
- 番茄会话 × 今日任务联动：任务标完成时若绑定番茄在跑，自动以 task_done 结束
  （StopIfTask 在状态机内原子判定，防 Start 竞态误杀新会话）
- 两个新命令：pomodoro_stats_report / pomodoro_sessions_by_date（契约 97 → 99）

#### Fixed
- 【P0】同文本重复待办拖动换位后勾选回写错行/meta 错绑：key 派生改为始终基于原始正文序，
  展示行经 displayLineByKey 映射，回写锚定原文行（缺失时 no-op 并留错误日志，绝不猜行号）
- 【隐私】中途转私后明文重现：便签设私现在同步清洗状态机内任务文本（此前 auto_next 续阶段
  会把明文重新写回会话表并广播事件）；今日任务转私密 → 脱敏其全部会话快照（含历史行），
  Rust 侧在会话写入单点（begin_phase）按便签/今日任务私密状态统一脱敏
- 【P1】月/年重复提醒跨次触发日号漂移（1-31 → 2-28 → 3-28…）：迁移 v11 新增 reminders.anchor_at，
  链式触发始终从原始锚点重算（月末 clamp 语义不变）；v11 前旧行锚点为 NULL 维持旧行为
- 【P1】复盘高发时段柱状图因 CSS 高度解析恒为空；时间轴会话块被任务块常态遮挡（右侧竖槽让位）；
  单元素池任务显示假拖拽把手；跨池拖动静默写库（现在拒绝）；快速连拖丢前一次结果（乐观更新）
- reorder 单事务化；stats_report 起止日期乱序/非法统一为空报表，聚合以 Rust 侧本地日复核为权威口径；
  DST 午夜切换时区的窗口边界取"当日最早存在时刻"（不再漏盖/越界）

### 计划增强批（2026-09-19，阶段 A）

#### Added
- 今日计划周概览：时间轴头部日/周切换——7 列只读概览（每天完成 x/y、跳过数、紧凑任务行，
  今天高亮），点列头跳回该日时间轴；按周前后导航。新命令 daily_task_list_range（一次拉 7 天）
- 批量顺延：周概览中「批量顺延」进入选择模式，勾选待办状态任务一键移到次日
  （重复模板不可顺延；顺延后到点通知标记复位）。新命令 daily_task_postpone_to（契约 101→103）

### 每周/每月专注日报（2026-09-19）

- 复盘 tab 新增「生成为便签」：把当前查看的周/月区间报表一键生成为 Markdown 便签
  （合计/每日明细表/最专注任务/中断原因/高发时段），可编辑、归档、搜索——报表变成可留存的便签
- 新命令 pomodoro_report_to_note（契约 100 → 101）；报表正文为 Rust 纯函数生成（单测覆盖，
  空段省略、零填充日不进明细表、私密任务沿用写入端脱敏）；生成后层叠定位并自动打开

### 功能优化批（2026-09-19）

#### Added
- 今日任务「推迟到明天」：编辑对话框一键以当前内容顺延一天并跳转查看（高频操作收口）
- 管理器视图记忆：重启后回到上次使用的视图
- 侧栏「今日计划」显示当日任务数角标

#### Fixed
- 任务菜单十余处操作静默失败（预计番茄数/专注时长/优先级/跳过等失败无任何提示）：
  统一收口 guardTask——成功刷新 meta，失败 toast；顺带修正任务菜单「加入今日计划」
  误传 task_meta_update 参数形状的旧写法
- 任务对话框 Esc/点遮罩静默丢弃填写内容：脏检测确认后关闭；提交失败不再无条件关窗
- 顶栏番茄控制条在窄窗口挤压搜索框至不可用：容器允许收缩，番茄条自适应图标态

### 启动行为修正（2026-09-19，用户反馈）

- 【迁移 v12】「启动后自动显示便签」默认反转为**关闭**：此前启动会把全部便签窗口一起弹到桌面
  （便签多时是惊吓面，且该默认从未被用户显式确认过）。现在启动只开管理器，
  便签经托盘「显示全部便签」/ Ctrl+Shift+Space 唤出；需要旧行为在设置里一键开回（开关保留）

### 布局与操作逻辑打磨批（2026-09-19，两 agent 审查驱动）

#### Added
- 番茄控制条窄容器自适应（<300px 收为图标态：暂停/停止，隐藏跳过/+5分钟）——320px 便签底栏
  运行番茄时不再溢出，提醒与保存状态保持可见（CDP 实测 overflow=false）
- 今日计划空态引导（EmptyState 指向「+ 添加任务」）；退出前广播 app-exit-flush 事件，
  各便签窗口立即落库防抖内容（退出不再丢最近 500ms 输入）
- 新命令 backup_now（导入前自动快照，契约 99 → 100）

#### Fixed
- 【P1】删除/回收便签不取消其待触发提醒——重复提醒对已删便签每天弹一次且永续再生（幽灵提醒）；
  现在删除/移入回收站时取消全部 pending 提醒并唤醒调度器
- 【P1】导入 JSON 无确认直接覆盖现有数据：现在先确认并列明后果，自动做导入前备份
- 【P1】待办视图 ScrollArea 被快速输入框顶出窗口下缘（约 46px 裁剪）：容器改 flex-col + flex-1 高度链
- 番茄 focus 会话被手动停止/跳过/意外退出中断时明确 toast 提示（本番茄未计入）；
  Dialog 滚动时右上角关闭按钮随内容滚走（改为内容层滚动、关闭钮常驻）；
  DropdownMenu/Popover 基类补 max-h（Radix available-height），小窗内长菜单不再被窗口边缘裁剪；
  时间轴短任务块 hover 提升至最上层；今日任务删除增加确认；NoteCard 菜单文案统一「移入回收站」；
  卡片长标签截断省略；便签骨架屏高度随窗口；速记箱清空剪贴板历史增加确认

#### Fixed
- 导入 JSON 时 task_meta 丢失 focus_min（导出含该字段但导入未回写，往返丢数据）
- 开窗命令主线程死锁：WebviewWindowBuilder 同步 build() 自死锁饿死全部 IPC——提醒/勿扰等设置类操作"无法设置"的根因
- Tauri ACL 权限：allow-destroy（便签无法关闭）/ allow-set-position（迷你窗定位）/ allow-read-text（剪贴板捕获）
- 网格平铺越界：改用工作区矩形 + 越界 clamp，不再遮挡任务栏、旧布局预设不再落屏外
- 高 DPI 新建便签过小：默认尺寸按主显示器 scale_factor 缩放（320×360 逻辑 px）
- 重复模板 focus_min 未传播到物化实例（v9 回归，单测抓出）
- 管理器窄侧栏（176px）溢出：布局工具条「保存布局」被顶出行外不可见、勿扰卡片时间输入被压至 35px——两组件拆行 + 弹性收缩重排（CDP 实测 aside 溢出 319→175 归零）

#### Changed（内部质量）
- 生产级收敛：关键路径事务化、生产 unwrap 收敛、FTS5 注入防护、Reminder 时钟抽象、events.rs 统一事件常量
- 前端收敛：修复 StrictMode 事件监听泄漏、React Query staleTime 调优、NoteWindow 子组件拆分
- R3 UX：NoteWindow 工具栏智能收纳/预览沉浸/底栏精简、管理器密度提升、QuickCapture 紧凑化

### 快捕任务直达 × 番茄专注勿扰批（2026-09-19）

#### Added
- 【迁移 v13】快速捕获「新建今日任务」全局快捷键（默认 Ctrl+Shift+T）：速记箱新增「任务」tab
  （标题必填 + 可选开始时间，一键写入今日计划）；冷启动经 `index.html?mode=task` 直达任务 tab
  （新建窗口瞬间前端事件监听未注册，emit 必丢——用 URL 契约承载初始模式），已开窗则 `qc-mode` 事件热切换
- 【迁移 v14】番茄专注联动勿扰（设置「番茄专注期间通知勿扰」默认开）：专注运行区间内到点的提醒
  自动顺延（snooze）到本段结束——不弹窗、不丢失；暂停/休息/过期快照不抑制；勿扰指示 🔕 显示在控制条
- 勿扰设置卡片交互重做：三个预设时段一键套用 + ±15 分钟步进（跨午夜回绕），替代原生 time input；
  本地即时生效 + 400ms 防抖合并落盘，写写交叠串行化、失败回滚上次成功值
- 跨层契约集成测试（`src-tauri/tests/contracts.rs`，5 项）：Rust↔TS 事件名集合一致、
  每个快捷键 action 三处接线齐全、qc 任务模式 URL 契约、命令面板 view 在管理器有处理分支、
  `pomo_dnd` 四处真相源 + reminder 必消费 `focus_silence_until`（防死设置键）
- 命令面板扩充 6 条命令（呼出速记箱/今日计划/已归档/私密空间/番茄统计/生成专注周报便签），
  全部命令补拼音首字母检索；Ctrl+K 面板测试基建（mock api）
- 番茄控制条会话归属语义：归属便签与管理器顶栏渲染完整控制条，其他便签只显示弱化倒计时
  （整行按钮=点击接管会话，toast 提示原任务已停止）；编辑模式禁启动
- 命令面板 navigate 支持 all/archived/private 视图直跳

#### Changed（内部质量）
- SQLite 读侧调优：temp_store=MEMORY、mmap_size=256MB、cache_size=-64MB
- 启动备份加完整性校验：integrity_check + notes/reminders/settings/pomodoro_sessions 四表行数与源库比对，
  失败报错且坏备份文件保留不轮转；备份失败写结构化错误日志
- 退出路径 best-effort `PRAGMA optimize`；快捷键 action 白名单 `pub(crate)` 供迁移测试比对

#### Tests
- 前端 +17 用例（qc tab 契约 3、命令面板过滤 6、勿扰纯函数 8）；Rust 新增 focus_silence_until 矩阵、
  迁移默认快捷键白名单/加速器冲突拦截、备份验收

### 性能链路优化批（2026-09-19）

#### Changed（性能）
- notes-changed 事件风暴收敛：事件 payload 增加 `reason`（content/tags/structure，缺省按 structure）——
  自动保存（content）在管理器侧 800ms 尾沿防抖合并、不再失效标签查询（正文保存不动标签集合）；
  便签窗口对 content 事件整体跳过 refreshMeta（正文保存不改 flags/图片/标签/提醒，自身保存本地已是最新）。
  连续打字从「每 500ms 三列表全量 refetch + 每便签窗一次 get_note」降为「每秒最多一次列表刷新、便签窗零请求」
- 列表查询 N+1 消除：list/search/list_private 的标签获取改为一条批量聚合查询（500/批，与逐条查询同序同果）；
  正式列表正文在 Rust 侧截断到 500 字符传输（todo 计数按截断前全文统计，卡片摘要只需 60 字符；
  回收站列表保留全文——预览面板整文渲染；单便签详情始终全文）
- 启动开窗查询瘦身：all_active_with_windows 投影不再取正文（开窗只消费 id/几何/标志位，
  NoteWindow 打开后自行 get_note 加载全文）
- 自动保存写放大治理：auto 版本快照 120s 节流——节流窗口内的打字中间态不再写 note_versions
  （50 条配额不再被连续保存灌满；手动/恢复前快照不受限；查询失败退回照常快照），
  单次自动保存 SQL 从 ~8 条降至 ~5 条
- FTS 同步同值守卫：title/body/tags 三元组与现有索引一致时跳过删插（省整行 trigram 重分词）
- NoteCard memo 化 + 管理器 onOpen 改稳定回调（useCallback），列表 refetch 未变卡片不再重渲染
- 搜索栏任务 token 查询补 250ms 防抖（与主搜索一致，is:task 等前缀输入不再逐键打库）

#### Tests
- Rust 新增：auto_snapshot_throttle_window（节流窗口矩阵）、
  list_caps_content_counts_full_trash_keeps_full（截断/全文计数/trash 不截断/详情全文）

### 启动与加载优化批（2026-09-19）

#### Changed（性能）
- 启动备份移出关键路径：备份（Online Backup 复制 + integrity_check + 行数校验）移入后台线程，
  独立连接与 WAL 主连接并发安全；失败链路（warn 日志 + error/*.md 错误账本 + 坏文件不轮转）不变。
  管理器/便签窗口出现不再等待备份 IO
- 按窗口角色代码分割（React.lazy）：manager / note / quick-capture / pomodoro-mini 四窗口各自独立 chunk，
  按需加载——迷你番茄窗 JS 从 ~740KB（全量 bundle）降至 ~237KB（-68%），速记箱 ~247KB；
  Markdown 渲染器与管理器代码不再被非目标窗口下载解析。便签/速记窗本就 visible:false 等前端就绪再显示，
  chunk 加载延迟不产生白闪（Suspense fallback 为主题底色空壳）
- 启动开窗查询不再加载正文（随性能链路批 all_active_with_windows 瘦身生效）

## [0.1.0] - 2026-09-16

### 初始功能

#### 便签
- 多便签桌面常驻（原生窗口，无边框/圆角/可拖缩放）
- Markdown 编辑/预览（工具栏 + 快捷键 + 语法高亮）
- 图片（拖拽/粘贴截图/文件选择，本地存储）
- 自动保存（500ms 防抖 + 关窗 flush + WAL 崩溃保护）
- 置顶 / 归档 / 回收站 / 永久删除
- 位置/大小/显示器记忆 + 越界自动拉回

#### 提醒
- 自然语言时间解析（中文："明天下午3点"等）
- 重复规则（每天/每周/工作日）
- 勿扰时段（跨午夜）+ 错过补发
- Windows 通知 + 私密脱敏

#### 番茄钟
- 专注/短休/长休状态机，全局唯一会话
- 便签底栏/迷你悬浮窗/托盘三处同步倒计时
- 绑定便签任务，统计番茄数/专注时长/中断次数

#### 快速捕获
- 全局快捷键呼出速记箱
- 剪贴板历史（100 条，pinned 不淘汰）

#### 搜索
- FTS5 trigram 全文索引（中文子串可搜）
- 搜索语法（tag:/is:/due:/has:）+ 高亮 + 保存搜索

#### 窗口管理
- 跨虚拟桌面显示（COM PinWindow）
- 全屏跟随（SetWinEventHook 事件驱动）
- 窗口布局预设（网格排列 + 物理像素记忆）

#### 系统集成
- 系统托盘（六项菜单）
- 全局快捷键（可自定义 + 冲突检测）
- 深色/浅色/跟随系统主题
- 开机自启 / 便携模式
- JSON 导出/导入（默认排除私密便签）
- 诊断包导出
- 命令面板（Ctrl+K）

#### 数据
- SQLite WAL（10 张表，6 次迁移）
- 启动原子备份 ×5 轮转
- FTS5 trigram 全文索引
- 结构化错误日志（errors/*.md）
- 运行日志（logs/focusly.log，5MB 轮转）
