/** 番茄钟迷你窗：label="pomodoro-mini" 窗口的内容（路由由总控接线，此处直接渲染即可，
 *  显示由总控创建窗口时的 visible 控制）。无边框深色卡片，顶部可拖动。
 *  倒计时数字外圈为 SVG 进度环（progress = 1 - 剩余/阶段总时长）。 */
import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { pomodoroMiniHide } from "./api";
import { PHASE_META, PHASE_TOTAL_SEC } from "./format";
import { usePomodoro } from "./usePomodoro";

const BTN =
  "rounded-md px-2.5 py-1 text-xs font-medium text-zinc-300 transition-colors bg-zinc-800 hover:bg-zinc-700 disabled:opacity-50";

const RING_RADIUS = 36;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;
/** 阶段切换闪烁的底色回退时长（与 transition-colors duration-500 对齐） */
const FLASH_MS = 500;

export function MiniPomodoroWindow() {
  const {
    state,
    remaining,
    mmss,
    isRunning,
    available,
    start,
    pause,
    resume,
    skip,
    stop,
    addMinutes,
  } = usePomodoro();

  const phase = state?.phase ?? "focus";
  const meta = PHASE_META[phase] ?? PHASE_META.focus;
  const paused = !isRunning && !!state?.paused;
  const active = !!state && (!!state.endsAt || state.remainingSec > 0);
  const taskText = state?.taskText?.trim() ? state.taskText : "专注";

  // 阶段切换（focus↔break）时整卡短暂提亮一次，transition-colors 渐隐回退
  const [flash, setFlash] = useState(false);
  const phaseRef = useRef(phase);
  useEffect(() => {
    if (phaseRef.current === phase) return;
    phaseRef.current = phase;
    setFlash(true);
    const timer = setTimeout(() => setFlash(false), FLASH_MS);
    return () => clearTimeout(timer);
  }, [phase]);

  // 进度环：progress = 1 - 剩余/总时长；+5分钟可能超出总时长，夹到 [0,1]
  const totalSec = PHASE_TOTAL_SEC[phase] ?? PHASE_TOTAL_SEC.focus;
  const progress = totalSec > 0 ? Math.min(1, Math.max(0, 1 - remaining / totalSec)) : 0;

  const hide = () => {
    // 总控接线 pomodoro_mini_hide；未接线时静默失败
    void pomodoroMiniHide().catch(() => {});
  };

  return (
    <div
      className={cn(
        "flex h-screen select-none flex-col overflow-hidden rounded-xl border border-zinc-800 text-zinc-100 shadow-2xl transition-colors duration-500",
        flash ? "bg-zinc-800" : "bg-zinc-900",
      )}
    >
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

      {/* 主体：阶段文字 + 进度环包住倒计时数字 + 任务/暂停提示 */}
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1 px-4">
        <div className="text-xs text-zinc-400">
          {meta.emoji} {meta.label}
        </div>
        <div className="relative flex size-20 items-center justify-center">
          <svg aria-hidden className="absolute inset-0 size-full" viewBox="0 0 80 80">
            <circle
              cx="40"
              cy="40"
              r={RING_RADIUS}
              fill="none"
              stroke="currentColor"
              strokeWidth="4"
              className="text-zinc-700/70"
            />
            <circle
              cx="40"
              cy="40"
              r={RING_RADIUS}
              fill="none"
              stroke="currentColor"
              strokeWidth="4"
              strokeLinecap="round"
              strokeDasharray={RING_CIRCUMFERENCE}
              strokeDashoffset={RING_CIRCUMFERENCE * (1 - progress)}
              className="text-amber-500 transition-[stroke-dashoffset] duration-1000"
              transform="rotate(-90 40 40)"
            />
          </svg>
          <div
            role="timer"
            aria-live="polite"
            aria-label="番茄倒计时"
            className={cn(
              "text-2xl font-semibold tabular-nums tracking-tight",
              paused && "opacity-60",
            )}
          >
            {mmss}
          </div>
        </div>
        <div className="max-w-full truncate text-xs text-zinc-500" title={taskText}>
          {available ? (paused ? "⏸ 已暂停" : taskText) : "后端未就绪"}
        </div>
      </div>

      {/* 控制按钮：2×2 网格（暂停+跳过 / 停止++5分钟） */}
      <div className="grid shrink-0 grid-cols-2 gap-1.5 border-t border-zinc-800/80 px-3 py-2.5">
        {!active ? (
          <button
            type="button"
            className={cn(BTN, "col-span-2")}
            onClick={() => void start(null, null, null)}
          >
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
