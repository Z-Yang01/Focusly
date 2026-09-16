/** 便签底栏番茄条：Idle → 开始按钮；Running/Paused → 倒计时 + 控制按钮。
 *  状态来自全局 usePomodoro（多窗口同源）；后端未就绪时降级为提示文本。 */
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { StatePayload } from "./types";
import { usePomodoro } from "./usePomodoro";

export interface PomodoroBarProps {
  noteId: string;
  /** 当前选中任务（NoteWindow 传入；无绑定任务则不传，开始即纯专注） */
  taskKey?: string | null;
  /** 当前选中任务文本（私密便签时调用方也应脱敏/不传） */
  taskText?: string | null;
}

/** 是否存在可操作的会话（运行中或暂停中） */
function sessionActive(state: StatePayload | null): boolean {
  if (!state) return false;
  if (state.endsAt && !state.paused) return true;
  return state.paused && state.remainingSec > 0;
}

export function PomodoroBar({ noteId, taskKey, taskText }: PomodoroBarProps) {
  const { state, mmss, isRunning, available, start, pause, resume, skip, stop, addMinutes } =
    usePomodoro();

  if (!available) {
    return (
      <span className="select-none truncate text-muted-foreground/60" title="番茄钟后端未就绪">
        番茄钟后端未就绪
      </span>
    );
  }

  const active = sessionActive(state);

  if (!active) {
    const handleStart = () => {
      if (taskKey) {
        void start(noteId, taskKey, taskText ?? "");
      } else {
        void start(null, null, null);
      }
    };
    return (
      <Button
        type="button"
        variant="ghost"
        size="sm"
        className="h-6 gap-1 px-2 text-xs text-muted-foreground hover:text-foreground"
        onClick={handleStart}
      >
        🍅 开始专注
      </Button>
    );
  }

  const boundHere = state?.noteId === noteId;
  const taskLabel = boundHere ? state?.taskText : null;
  const paused = !isRunning && !!state?.paused;

  const ctrl =
    "h-6 gap-1 rounded-md px-1.5 text-xs text-muted-foreground hover:text-foreground";

  return (
    <div className="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
      <span
        className={cn("select-none font-medium tabular-nums text-foreground", paused && "opacity-70")}
        title={paused ? "已暂停" : "专注进行中"}
      >
        🍅 {mmss}
      </span>
      {paused && <span className="select-none">⏸</span>}
      {taskLabel && (
        <span className="max-w-[120px] truncate" title={taskLabel}>
          {taskLabel}
        </span>
      )}
      {boundHere && (state?.completedInCycle ?? 0) > 0 && (
        <span className="select-none tabular-nums" title="本轮已完成的番茄数">
          ×{state?.completedInCycle}
        </span>
      )}
      <Button type="button" variant="ghost" size="sm" className={ctrl} onClick={() => void (paused ? resume() : pause())}>
        {paused ? "继续" : "暂停"}
      </Button>
      <Button type="button" variant="ghost" size="sm" className={ctrl} onClick={() => void skip()}>
        跳过
      </Button>
      <Button type="button" variant="ghost" size="sm" className={ctrl} onClick={() => void stop("manual")}>
        停止
      </Button>
      <Button type="button" variant="ghost" size="sm" className={ctrl} onClick={() => void addMinutes(5)}>
        +5分钟
      </Button>
    </div>
  );
}
