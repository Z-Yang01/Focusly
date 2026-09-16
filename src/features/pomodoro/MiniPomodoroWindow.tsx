/** 番茄钟迷你窗：label="pomodoro-mini" 窗口的内容（路由由总控接线，此处直接渲染即可，
 *  显示由总控创建窗口时的 visible 控制）。无边框深色卡片，顶部可拖动。 */
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { pomodoroMiniHide } from "./api";
import { PHASE_META } from "./format";
import { usePomodoro } from "./usePomodoro";

const BTN =
  "rounded-md px-2.5 py-1 text-xs font-medium text-zinc-300 transition-colors bg-zinc-800 hover:bg-zinc-700 disabled:opacity-50";

export function MiniPomodoroWindow() {
  const { state, mmss, isRunning, available, start, pause, resume, skip, stop, addMinutes } =
    usePomodoro();

  const phase = state?.phase ?? "focus";
  const meta = PHASE_META[phase] ?? PHASE_META.focus;
  const paused = !isRunning && !!state?.paused;
  const active = !!state && (!!state.endsAt || state.remainingSec > 0);
  const taskText = state?.taskText?.trim() ? state.taskText : "专注";

  const hide = () => {
    // 总控接线 pomodoro_mini_hide；未接线时静默失败
    void pomodoroMiniHide().catch(() => {});
  };

  return (
    <div className="flex h-screen select-none flex-col overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900 text-zinc-100 shadow-2xl">
      {/* 顶部拖动区 + 关闭 */}
      <div
        data-tauri-drag-region
        className="relative flex h-9 shrink-0 items-center justify-center border-b border-zinc-800/80"
      >
        <span className="text-xs text-zinc-500" data-tauri-drag-region>
          Focusly 番茄钟
        </span>
        <button
          type="button"
          aria-label="关闭迷你窗"
          title="关闭"
          onClick={hide}
          className="absolute right-1 flex size-6 items-center justify-center rounded-md text-zinc-500 transition-colors hover:bg-zinc-800 hover:text-zinc-200"
        >
          <X className="size-3.5" />
        </button>
      </div>

      {/* 主体 */}
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1.5 px-4">
        <div className="text-sm text-zinc-400">
          {meta.emoji} {meta.label}
        </div>
        <div
          role="timer"
          aria-live="polite"
          aria-label="番茄倒计时"
          className={cn(
            "text-4xl font-semibold tabular-nums tracking-tight",
            paused && "opacity-60",
          )}
        >
          {mmss}
        </div>
        <div className="max-w-full truncate text-xs text-zinc-500" title={taskText}>
          {available ? taskText : "后端未就绪"}
        </div>
        {paused && <div className="text-xs text-amber-400">⏸ 已暂停</div>}
      </div>

      {/* 按钮行 */}
      <div className="flex shrink-0 items-center justify-center gap-1.5 border-t border-zinc-800/80 px-3 py-2.5">
        {!active ? (
          <button type="button" className={BTN} onClick={() => void start(null, null, null)}>
            开始
          </button>
        ) : (
          <>
            <button
              type="button"
              className={BTN}
              onClick={() => void (paused ? resume() : pause())}
            >
              {paused ? "继续" : "暂停"}
            </button>
            <button type="button" className={BTN} onClick={() => void skip()}>
              跳过
            </button>
            <button
              type="button"
              className="rounded-md px-2.5 py-1 text-xs font-medium text-red-400 transition-colors hover:bg-red-500/10"
              onClick={() => void stop("manual")}
            >
              停止
            </button>
            <button type="button" className={BTN} onClick={() => void addMinutes(5)}>
              +5分钟
            </button>
          </>
        )}
      </div>
    </div>
  );
}
