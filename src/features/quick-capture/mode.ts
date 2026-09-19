/** 快速捕获窗口的 tab 契约（与 Rust src-tauri/src/quickcapture.rs 的 URL/事件约定对应）。
 *  冷启动新建窗口时前端还没注册事件监听，Rust 把初始 tab 编进 URL（index.html?mode=task）。 */

export type Tab = "write" | "task" | "history";

/** 从窗口 URL 的 search 串解析初始 tab；无/未知 mode 一律回落速记页 */
export function tabFromSearch(search: string): Tab {
  return new URLSearchParams(search).get("mode") === "task" ? "task" : "write";
}
