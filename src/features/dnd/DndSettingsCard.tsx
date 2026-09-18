/** 通知勿扰设置卡片（供总控嵌入设置页旁或管理器侧栏底部）：
 *  启用开关 + 起止 time input。启用时把 "HH:MM" 写入 settings 键 dnd_start/dnd_end，
 *  关闭时写空串（后端读取端容错：缺失/空 = 始终通知）。
 *  后端判定规则见 src-tauri/src/dnd.rs：start==end 视为未启用，支持跨午夜窗口；
 *  勿扰期间到点提醒由调度器推迟到时段结束后再通知（不丢失、不轰炸）。
 *  复用既有 set_setting / get_all_settings 命令（见 ./api.ts），命令异常时降级提示。 */
import { useEffect, useState } from "react";
import { BellOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { DND_END_KEY, DND_START_KEY, getAllSettings, setSetting } from "./api";
import {
  DEFAULT_END,
  DEFAULT_START,
  SAVED_HINT,
  isCrossMidnight,
  isDndEnabled,
  isValidHm,
} from "./dnd";

interface DndSettingsCardProps {
  className?: string;
}

type Status = { kind: "ok" | "error"; text: string } | null;

const INVALID_HINT = "起止时间需为合法 HH:MM，且开始与结束不能相同（相同 = 不启用）";

/** 命令不可用（等待接线/权限异常）时的降级提示 */
function degradeMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  if (/not found|未注册|unknown command/i.test(msg)) {
    return "设置命令尚未接入（等待后端注册）";
  }
  return msg;
}

export function DndSettingsCard({ className }: DndSettingsCardProps) {
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [enabled, setEnabled] = useState(false);
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [status, setStatus] = useState<Status>(null);

  // 挂载时拉取设置回显（键不存在 → 空串 = 关闭）
  useEffect(() => {
    let alive = true;
    getAllSettings()
      .then((s) => {
        if (!alive) return;
        const st = s[DND_START_KEY] ?? "";
        const en = s[DND_END_KEY] ?? "";
        setStart(st);
        setEnd(en);
        setEnabled(isDndEnabled(st, en));
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

  /** 依次写两个键；成功后同步本地输入并清脏标记 */
  const persist = async (nextStart: string, nextEnd: string): Promise<boolean> => {
    setBusy(true);
    try {
      await setSetting(DND_START_KEY, nextStart);
      await setSetting(DND_END_KEY, nextEnd);
      setStart(nextStart);
      setEnd(nextEnd);
      setDirty(false);
      return true;
    } catch (err) {
      console.error("保存勿扰设置失败", err);
      setStatus({ kind: "error", text: degradeMessage(err) });
      return false;
    } finally {
      setBusy(false);
    }
  };

  const handleEnabledChange = async (v: boolean) => {
    if (busy) return;
    if (!v) {
      const ok = await persist("", "");
      if (ok) {
        setEnabled(false);
        setStatus({ kind: "ok", text: "勿扰已关闭，提醒将正常通知" });
      }
      return;
    }
    // 输入为空/非法时回退到默认窗口（22:00-07:00）
    const s = isValidHm(start) ? start : DEFAULT_START;
    const e = isValidHm(end) ? end : DEFAULT_END;
    if (!isDndEnabled(s, e)) {
      setStatus({ kind: "error", text: INVALID_HINT });
      return;
    }
    const ok = await persist(s, e);
    if (ok) {
      setEnabled(true);
      setStatus({ kind: "ok", text: SAVED_HINT });
    }
  };

  const handleSave = async () => {
    if (busy) return;
    if (!isDndEnabled(start, end)) {
      setStatus({ kind: "error", text: INVALID_HINT });
      return;
    }
    const ok = await persist(start, end);
    if (ok) setStatus({ kind: "ok", text: SAVED_HINT });
  };

  return (
    <div className={cn("rounded-lg border bg-card p-3 text-sm", className)}>
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5">
          <BellOff className="size-3.5 text-muted-foreground" />
          <span className="font-medium">通知勿扰</span>
        </div>
        <Switch
          checked={enabled}
          disabled={busy}
          onCheckedChange={(v) => void handleEnabledChange(v)}
        />
      </div>
      <p className="mt-1 text-xs text-muted-foreground">
        勿扰期间到点提醒将推迟到时段结束后通知，不会丢失。
      </p>

      <div className="mt-2 grid grid-cols-2 gap-2">
        <div className="min-w-0 space-y-1">
          <Label htmlFor="dnd-start" className="text-xs text-muted-foreground">
            开始
          </Label>
          <Input
            id="dnd-start"
            type="time"
            value={start}
            disabled={busy}
            onChange={(e) => {
              setStart(e.target.value);
              setDirty(true);
            }}
            className="h-8 px-2 text-xs"
          />
        </div>
        <div className="min-w-0 space-y-1">
          <Label htmlFor="dnd-end" className="text-xs text-muted-foreground">
            结束
          </Label>
          <Input
            id="dnd-end"
            type="time"
            value={end}
            disabled={busy}
            onChange={(e) => {
              setEnd(e.target.value);
              setDirty(true);
            }}
            className="h-8 px-2 text-xs"
          />
        </div>
      </div>
      <Button
        type="button"
        size="sm"
        className="mt-2 h-8 w-full text-xs"
        disabled={busy || !enabled || !dirty}
        onClick={() => void handleSave()}
      >
        保存
      </Button>

      {enabled && isCrossMidnight(start, end) && (
        <p className="mt-1 text-xs text-muted-foreground">
          跨午夜时段：当日 {start} 至次日 {end}
        </p>
      )}
      {enabled && dirty && (
        <p className="mt-1 text-xs text-muted-foreground">有未保存的修改</p>
      )}
      {status && (
        <p
          className={cn(
            "mt-1.5 text-xs",
            status.kind === "ok" ? "text-muted-foreground" : "text-destructive",
          )}
        >
          {status.text}
        </p>
      )}
    </div>
  );
}
