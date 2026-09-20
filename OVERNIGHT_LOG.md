# OVERNIGHT_LOG — Focusly 无人值守优化日志

> 每轮追加一条：时间、改动内容、原因、验证结果。（首条之前无历史日志，本文件于 2026-09-21 创建；
> 更早的夜间轮次记录见 NIGHTLY_LOG.md / NIGHTLY_PROGRESS.md。）

## 2026-09-21 02:06

- **改动**：`src/features/quick-capture/QuickCaptureWindow.tsx` 全局键处理 useEffect 的依赖数组
  补上 `tab`（`[hideWindow, save, saveTask]` → `[hideWindow, save, saveTask, tab]`），并加注释说明原因。
- **原因**：`onKey` 闭包直接读取 `tab` 来分流 Ctrl+Enter（任务页 → saveTask，其余 → save），
  但 `tab` 不在依赖里——页签切换本身不会重建监听器，处理器拿到的是过期页签。复现路径：
  在任务页输入标题后 Ctrl+Tab 切回速记页再按 Ctrl+Enter，会按旧的 `tab==="task"` 误存一条今日任务；
  反向场景（速记页有草稿 → 切到任务页立刻 Ctrl+Enter）会误触速记保存并关窗。这也是
  `npm run lint` 此前唯一的警告（react-hooks/exhaustive-deps），修 bug 同时清零警告。
- **验证**：`npm run -s test` 297/297 通过（约 1.4s，基线 294 + 前夜 R7 新增 3）；`npm run -s lint`
  0 警告（此前 1）；`npm run build`（tsc + vite）通过。组件级键处理无法在 node 环境单测覆盖
  （vitest 无 DOM/组件测试栈），行为修复以依赖数组语义为准。
- **备注**：当前分支为 `auto/nightly/focusly-20260920-0115`（非 main，沿用前夜工作分支，未做分支切换），
  本轮提交落在该分支，未 push。
