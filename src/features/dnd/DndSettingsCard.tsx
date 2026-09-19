/** 通知勿扰设置卡片（嵌入管理器侧栏底部，见 ManagerWindow）：
 *  启用开关 + 时段选择。选择方式 = 预设时段一键套用 + 起止 ±15min 步进，
 *  不用原生 time input（时钟弹窗点选繁琐）。
 *  交互模型：本地即时生效（连点不丢），落盘做 400ms 防抖合并——连点五次
 *  只写一次 settings；写失败回滚到上次成功的值并给出错误提示。
 *  启用时把 "HH:MM" 写入 settings 键 dnd_start/dnd_end，关闭写空串
 *  （后端读取端容错：缺失/空 = 始终通知）。
 *  后端判定规则见 src-tauri/src/dnd.rs：start==end 视为未启用，支持跨午夜窗口；
 *  勿扰期间到点提醒由调度器推迟到时段结束后再通知（不丢失、不轰炸）。
 *  纯逻辑（回绕/预设/校验）在 ./dnd.ts，有单测。 */
import { useEffect, useRef, useState } from "react";
import { BellOff, Minus, Plus } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { DND_END_KEY, DND_START_KEY, getAllSettings, setSetting } from "./api";
import {
  DEFAULT_END,
  DEFAULT_START,
  DND_PRESETS,
  SAVED_HINT,
  STEP_MIN,
  isCrossMidnight,
  isDndEnabled,
  isValidHm,
  nudgeWindow,
  type DndTimeWindow,
} from "./dnd";

interface DndSettingsCardProps {
  className?: string;
}

type Status = { kind: "ok" | "error"; text: string } | null;
type TimeWindow = DndTimeWindow;

/** 连点合并窗口：停手 400ms 后才真正写库 */
const DEBOUNCE_MS = 400;
/** 「已保存」提示停留时长（避免每次步进都留一行噪声） */
const OK_HINT_MS = 2500;

/** 命令不可用（等待接线/权限异常）时的降级提示 */
function degradeMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  if (/not found|未注册|unknown command/i.test(msg)) {
    return "设置命令尚未接入（等待后端注册）";
  }
  return msg;
}

/** 依次写两个键（两键互不依赖，失败即整体报错由调用方回滚展示） */
async function writeKeys(w: TimeWindow): Promise<void> {
  await setSetting(DND_START_KEY, w.start);
  await setSetting(DND_END_KEY, w.end);
}

const sameTimeWindow = (a: TimeWindow, b: TimeWindow) => a.start === b.start && a.end === b.end;

/** 单侧边界：−15 / 时间 / +15 步进（即点即改，落盘由父级防抖合并） */
function BoundaryStepper({
  label,
  value,
  onNudge,
}: {
  label: string;
  value: string;
  onNudge: (deltaMin: number) => void;
}) {
  return (
    <div className="flex min-w-0 flex-1 flex-col items-center gap-0.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <div className="flex items-center gap-0.5">
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7 text-muted-foreground hover:text-foreground"
          title={`提前 ${STEP_MIN} 分钟`}
          aria-label={`${label}提前${STEP_MIN}分钟`}
          onClick={() => onNudge(-STEP_MIN)}
        >
          <Minus className="size-3.5" />
        </Button>
        <span className="w-[5.2ch] select-none text-center text-base font-medium tabular-nums">
          {value}
        </span>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-7 text-muted-foreground hover:text-foreground"
          title={`推迟 ${STEP_MIN} 分钟`}
          aria-label={`${label}推迟${STEP_MIN}分钟`}
          onClick={() => onNudge(STEP_MIN)}
        >
          <Plus className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

export function DndSettingsCard({ className }: DndSettingsCardProps) {
  // start/end = 所见即所得的本地值；written = 已确认落盘的值；pending = 待写
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [enabled, setEnabled] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<Status>(null);

  const written = useRef<TimeWindow>({ start: "", end: "" });
  const pending = useRef<TimeWindow | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // 同一时刻只允许一次写库：两键必须成对落地，禁止两次写交叠（否则会混出混搭时段）
  const writing = useRef(false);
  const rerun = useRef(false);
  const disposed = useRef(false);
  // 用户在回显完成前已动手（理论上毫秒级竞态）→ 回显结果不得覆盖本地/已写值
  const touched = useRef(false);

  // 挂载时拉取设置回显（键不存在 → 空串 = 关闭）
  useEffect(() => {
    let alive = true;
    getAllSettings()
      .then((s) => {
        if (!alive || touched.current) return;
        const w = { start: s[DND_START_KEY] ?? "", end: s[DND_END_KEY] ?? "" };
        written.current = w;
        setStart(w.start);
        setEnd(w.end);
        setEnabled(isDndEnabled(w.start, w.end));
      })
      .catch((err) => {
        if (!alive) return;
        console.error("加载勿扰设置失败", err);
        setStatus({ kind: "error", text: degradeMessage(err) });
      });
    return () => {
      alive = false;
    };
  }, []);

  // 卸载时把未落盘的待写值补发出去（不等防抖，不丢改动），并终止重试链
  useEffect(
    () => () => {
      disposed.current = true;
      if (timer.current) clearTimeout(timer.current);
      const p = pending.current;
      if (p && !sameTimeWindow(p, written.current)) {
        void writeKeys(p).catch((err) => console.error("保存勿扰设置失败", err));
      }
    },
    [],
  );

  /** 立即结算 pending；写入进行中则打标记，完成后自动补一轮（连点不丢） */
  const flush = () => {
    if (timer.current) {
      clearTimeout(timer.current);
      timer.current = null;
    }
    if (writing.current) {
      rerun.current = true;
      return;
    }
    writePending();
  };

  /** 消费 pending 并落盘：失败回滚展示上次成功值 */
  const writePending = () => {
    const p = pending.current;
    pending.current = null;
    if (!p || sameTimeWindow(p, written.current)) return;
    writing.current = true;
    setSaving(true);
    void writeKeys(p)
      .then(() => {
        written.current = p;
        setStatus({ kind: "ok", text: SAVED_HINT });
      })
      .catch((err) => {
        console.error("保存勿扰设置失败", err);
        setStart(written.current.start);
        setEnd(written.current.end);
        // 回滚后按上次成功值重推导开关（如「启用失败」→ 回到关闭态，不留空时段 UI）
        setEnabled(isDndEnabled(written.current.start, written.current.end));
        setStatus({ kind: "error", text: degradeMessage(err) });
      })
      .finally(() => {
        writing.current = false;
        if (disposed.current) return;
        setSaving(false);
        if (rerun.current) {
          rerun.current = false;
          writePending();
        }
      });
  };

  /** 本地即时生效 + 防抖落盘（immediate = 显式动作，跳过合并窗口） */
  const apply = (w: TimeWindow, immediate = false) => {
    touched.current = true;
    setStart(w.start);
    setEnd(w.end);
    pending.current = w;
    if (timer.current) clearTimeout(timer.current);
    if (immediate) {
      flush();
      return;
    }
    timer.current = setTimeout(flush, DEBOUNCE_MS);
  };

  // 「已保存」自动消失；错误提示保留到下一次操作
  useEffect(() => {
    if (status?.kind !== "ok") return;
    const t = setTimeout(() => setStatus(null), OK_HINT_MS);
    return () => clearTimeout(t);
  }, [status]);

  const handleEnabledChange = (v: boolean) => {
    if (!v) {
      setEnabled(false);
      apply({ start: "", end: "" }, true);
      setStatus({ kind: "ok", text: "勿扰已关闭，提醒将正常通知" });
      return;
    }
    // 回显缺失/非法时用默认窗口（22:00-07:00）
    const w = {
      start: isValidHm(start) ? start : DEFAULT_START,
      end: isValidHm(end) ? end : DEFAULT_END,
    };
    if (!isDndEnabled(w.start, w.end)) {
      setStatus({ kind: "error", text: "起止不能相同（相同 = 不启用）" });
      return;
    }
    setEnabled(true);
    apply(w, true);
  };

  /** 步进拨动：合法性判定下沉 nudgeWindow（有单测），合法即排队落盘 */
  const nudge = (which: "start" | "end", deltaMin: number) => {
    if (!enabled) return;
    const r = nudgeWindow({ start, end }, which, deltaMin);
    if (!r.ok) {
      setStatus({ kind: "error", text: r.reason });
      return;
    }
    setStatus(null);
    apply(r.window);
  };

  const activePreset = DND_PRESETS.find((p) => p.start === start && p.end === end);
  const applyPreset = (p: { start: string; end: string }) => {
    if (!enabled || (p.start === start && p.end === end)) return;
    setStatus(null);
    apply({ start: p.start, end: p.end }, true);
  };

  const hint = saving
    ? { kind: "ok" as const, text: "保存中…" }
    : status;

  return (
    <div className={cn("rounded-lg border bg-card p-3 text-sm", className)}>
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5">
          <BellOff className="size-3.5 text-muted-foreground" />
          <span className="font-medium">通知勿扰</span>
        </div>
        <Switch checked={enabled} onCheckedChange={handleEnabledChange} />
      </div>
      <p className="mt-1 text-xs text-muted-foreground">
        勿扰期间到点提醒将推迟到时段结束后通知，不会丢失。
      </p>

      {enabled && (
        <>
          <div className="mt-2 flex items-end gap-1">
            <BoundaryStepper label="开始" value={start} onNudge={(d) => nudge("start", d)} />
            <span className="select-none pb-1.5 text-muted-foreground">→</span>
            <BoundaryStepper label="结束" value={end} onNudge={(d) => nudge("end", d)} />
          </div>
          <div className="mt-1.5 flex flex-wrap gap-1">
            {DND_PRESETS.map((p) => (
              <Button
                key={p.label}
                type="button"
                size="sm"
                variant={activePreset?.label === p.label ? "secondary" : "ghost"}
                className="h-6 px-2 text-xs text-muted-foreground hover:text-foreground"
                onClick={() => applyPreset(p)}
              >
                {p.label}
              </Button>
            ))}
          </div>
          {isCrossMidnight(start, end) && (
            <p className="mt-1 text-xs text-muted-foreground">
              跨午夜时段：当日 {start} 至次日 {end}
            </p>
          )}
        </>
      )}

      {hint && (
        <p
          className={cn(
            "mt-1.5 text-xs",
            hint.kind === "ok" ? "text-muted-foreground" : "text-destructive",
          )}
        >
          {hint.text}
        </p>
      )}
    </div>
  );
}
