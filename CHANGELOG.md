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
