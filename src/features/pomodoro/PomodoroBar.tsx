/** 便签底栏番茄条：Idle → 开始按钮；Running/Paused → 倒计时 + 控制按钮。
 *  状态来自全局 usePomodoro（多窗口同源）；后端未就绪时降级为提示文本。
 *  会话归属：番茄钟是单一会话（Rust 单状态机），noteId 标记归属。
 *  归属便签与管理器顶栏（noteId=""，全局控制面）渲染完整控制条；
 *  其他便签只显示弱化倒计时 +「在此开始」（点击 = 将会话切换到本便签），
 *  避免"每张便签都启动了自己的番茄钟"的错觉。
 *  窄容器自适应：实测宽度 <300px 时收为 icon-only（320px 默认便签底栏
 *  与顶栏固定项共存时不溢出，提醒/保存状态保持可见）。 */
import { useEffect, useRef, useState } from "react";
import { BellOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { getAllSettings } from "@/lib/api";
import { onSettingsChanged } from "@/lib/tauri";
import { toast } from "@/stores/toast";
import { cn } from "@/lib/utils";
import type { StatePayload } from "./types";
import { usePomodoro } from "./usePomodoro";

export interface PomodoroBarProps {
  noteId: string;
  /** 当前选中任务（NoteWindow 传入；无绑定任务则不传，开始即纯专注） */
  taskKey?: string | null;
  /** 当前选中任务文本（私密便签时调用方也应脱敏/不传） */
  taskText?: string | null;
  /** 便签处于编辑模式时禁用启动/接管入口（已有会话的暂停/继续/停止不受限；
   *  管理器顶栏与迷你窗不传 = 不受限）。任务菜单入口本就只在预览态渲染。 */
  startBlocked?: boolean;
}

/** 是否存在可操作的会话（运行中或暂停中） */
function sessionActive(state: StatePayload | null): boolean {
  if (!state) return false;
  if (state.endsAt && !state.paused) return true;
  return state.paused && state.remainingSec > 0;
}

/** 实测容器宽度：<300px 视为窄态（320px 便签底栏 / 拖窄的顶栏） */
function useCompact(): {
  ref: React.RefObject<HTMLDivElement>;
  compact: boolean;
} {
  const ref = useRef<HTMLDivElement>(null);
  const [compact, setCompact] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setCompact(entry.contentRect.width < 300);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return { ref, compact };
}

/** 专注勿扰开关（pomo_dnd，缺键 = 开，同 Rust parse_setting_bool 默认）：
 *  挂载读一次 + settings-changed 增量同步；仅用于展示 🔕 指示，读失败静默按开处理不影响控制。 */
function usePomoDnd(): boolean {
  const [on, setOn] = useState(true);
  useEffect(() => {
    let alive = true;
    getAllSettings()
      .then((s) => {
        if (alive) setOn(s.pomo_dnd !== "false");
      })
      .catch(() => {
        /* 指示器降级，不做噪声 */
      });
    const off = onSettingsChanged((e) => {
      if (e.key === "pomo_dnd") setOn(e.value !== "false");
    });
    return () => {
      alive = false;
      void off.then((f) => f());
    };
  }, []);
  return on;
}

export function PomodoroBar({ noteId, taskKey, taskText, startBlocked = false }: PomodoroBarProps) {
  const { state, mmss, isRunning, available, start, pause, resume, skip, stop, addMinutes } =
    usePomodoro();
  const { ref, compact } = useCompact();
  const pomoDnd = usePomoDnd();

  if (!available) {
    return (
      <div ref={ref} className="flex min-w-0 items-center">
        <span className="select-none truncate text-muted-foreground/60" title="番茄钟后端未就绪">
          番茄钟后端未就绪
        </span>
      </div>
    );
  }

  const active = sessionActive(state);
  // 管理器顶栏（noteId=""）是全局控制面；便签只在会话归属自己时可操作
  const controllable = noteId === "" || state?.noteId === noteId;
  // 开始一律绑定本便签（含无任务的纯专注）；管理器保持 ""（未绑定的全局会话）
  const handleStart = () => {
    void start(noteId, taskKey ?? null, taskKey ? (taskText ?? "") : null);
  };

  if (!active) {
    return (
      <div ref={ref} className="flex min-w-0 items-center">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-6 shrink-0 gap-1 px-2 text-xs text-muted-foreground hover:text-foreground"
          disabled={startBlocked}
          title={startBlocked ? "编辑中——切到预览模式才能开始番茄钟" : compact ? "开始专注" : undefined}
          onClick={handleStart}
        >
          🍅{compact ? "" : " 开始专注"}
        </Button>
      </div>
    );
  }

  if (!controllable) {
    // 整行即按钮：弱化倒计时 + 归属说明，点击 = 会话切换到本便签（Rust switch_task 语义）。
    // 私密便签的会话 taskText 已被后端脱敏为 ""，此处回退不带名称。
    const owned = !!state?.noteId;
    const who = owned ? "其他便签" : "管理器/迷你窗";
    const hint = owned ? "会话在其他便签专注中" : "全局专注会话进行中";
    const prevText = state?.taskText?.trim();
    return (
      <div ref={ref} className="flex min-w-0 items-center">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="h-6 min-w-0 gap-1 px-1.5 text-xs font-normal text-muted-foreground/70 hover:text-foreground"
          disabled={startBlocked}
          title={
            startBlocked
              ? `该会话属于${who}；编辑中不可接管，切到预览模式后可点击切换`
              : `该会话属于${who}（可在管理器顶栏或迷你窗操作）；点击切换为本便签的会话`
          }
          onClick={() => {
            void start(noteId, taskKey ?? null, taskKey ? (taskText ?? "") : null).then((s) => {
              if (s) toast.info(`专注已切换到本便签${prevText ? `（原「${prevText}」已停止）` : ""}`);
            });
          }}
        >
          <span className="shrink-0 select-none tabular-nums">
            🍅 {mmss}
            {state?.paused ? " ⏸" : ""}
          </span>
          {!compact && <span className="min-w-0 select-none truncate">{hint}</span>}
        </Button>
      </div>
    );
  }

  const taskLabel = state?.taskText;
  const paused = !isRunning && !!state?.paused;

  const ctrl =
    "h-6 shrink-0 gap-1 rounded-md px-1.5 text-xs text-muted-foreground hover:text-foreground";
  const ctrlTitle = (label: string) => (compact ? label : undefined);

  return (
    <div ref={ref} className="flex min-w-0 items-center gap-1 text-xs text-muted-foreground">
      <span
        className={cn("shrink-0 select-none font-medium tabular-nums text-foreground", paused && "opacity-70")}
        title={paused ? "已暂停" : "专注进行中"}
      >
        🍅 {mmss}
      </span>
      {paused && <span className="select-none">⏸</span>}
      {!compact && pomoDnd && state?.phase === "focus" && !paused && (
        <span title="专注勿扰已开启：到点提醒将推迟到本段专注结束，不会丢失" className="shrink-0">
          <BellOff className="size-3.5 text-muted-foreground/70" aria-label="专注勿扰已开启" />
        </span>
      )}
      {!compact && taskLabel && (
        <span className="max-w-[120px] truncate" title={taskLabel}>
          {taskLabel}
        </span>
      )}
      {!compact && (state?.completedInCycle ?? 0) > 0 && (
        <span className="select-none tabular-nums" title="本轮已完成的番茄数">
          ×{state?.completedInCycle}
        </span>
      )}
      <Button type="button" variant="ghost" size="sm" className={ctrl} title={ctrlTitle(paused ? "继续" : "暂停")} onClick={() => void (paused ? resume() : pause())}>
        {paused ? (compact ? "▶" : "继续") : compact ? "⏸" : "暂停"}
      </Button>
      {!compact && (
        <Button type="button" variant="ghost" size="sm" className={ctrl} title="跳过本番茄，进入休息" onClick={() => void skip()}>
          跳过
        </Button>
      )}
      <Button type="button" variant="ghost" size="sm" className={ctrl} title={ctrlTitle("停止")} onClick={() => void stop("manual")}>
        {compact ? "■" : "停止"}
      </Button>
      {!compact && (
        <Button type="button" variant="ghost" size="sm" className={ctrl} title="专注时长顺延 5 分钟" onClick={() => void addMinutes(5)}>
          +5分钟
        </Button>
      )}
    </div>
  );
}
