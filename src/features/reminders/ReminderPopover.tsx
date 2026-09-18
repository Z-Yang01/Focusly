/** 提醒设置气泡：快捷时间 / 自然语言时间 / 自定义时间 / 重复类型 / 清除 */
import { useState } from "react";
import { Bell, BellPlus, CalendarClock } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cancelReminder, listReminders, parseTimeNl, setReminder } from "@/lib/api";
import { describeTime, formatLocal, fromDatetimeLocal, parseRfc3339, toDatetimeLocal } from "@/lib/format";
import type { Note, Reminder, RepeatType } from "@/types";
import { cn } from "@/lib/utils";
import { toast } from "@/stores/toast";

export interface ReminderPopoverProps {
  /** NoteDetail 也满足该类型（nextReminder 可选） */
  note: Note & { nextReminder?: Reminder | null };
  onChanged?: () => void;
}

const REPEAT_LABEL: Record<RepeatType, string> = {
  once: "仅一次",
  daily: "每天",
  weekly: "每周",
  weekdays: "工作日",
  monthly: "每月",
  yearly: "每年",
};

function nextHour(): Date {
  const d = new Date();
  d.setHours(d.getHours() + 1, 0, 0, 0);
  return d;
}

function tomorrowAt9(): Date {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(9, 0, 0, 0);
  return d;
}

function nextMondayAt9(): Date {
  const d = new Date();
  const day = d.getDay(); // 0 周日 … 6 周六
  const ahead = ((8 - day) % 7) || 7; // 周一当天也算下周
  d.setDate(d.getDate() + ahead);
  d.setHours(9, 0, 0, 0);
  return d;
}

/** parse_time_nl 命令返回：at 为 RFC3339 UTC，repeat 为 RepeatType 字面量 */
interface NlParsed {
  at: string;
  repeat: string;
}

export function ReminderPopover({ note, onChanged }: ReminderPopoverProps) {
  const [open, setOpen] = useState(false);
  const [reminder, setReminderState] = useState<Reminder | null>(note.nextReminder ?? null);
  const [datetime, setDatetime] = useState("");
  const [repeat, setRepeat] = useState<RepeatType>("once");
  const [busy, setBusy] = useState(false);
  const [nlInput, setNlInput] = useState("");
  const [nlPreview, setNlPreview] = useState<NlParsed | null>(null);
  const [nlBusy, setNlBusy] = useState(false);

  const refresh = async () => {
    try {
      const list = await listReminders(note.id);
      const pending = list
        .filter((r) => r.status === "pending")
        .sort((a, b) => a.remindAt.localeCompare(b.remindAt));
      setReminderState(pending[0] ?? null);
    } catch (err) {
      console.error("获取提醒失败", err);
    }
  };

  const handleOpenChange = (v: boolean) => {
    setOpen(v);
    if (v) {
      setReminderState(note.nextReminder ?? null);
      void refresh();
    }
  };

  const applyAt = async (at: Date) => {
    setBusy(true);
    try {
      const rfc3339 = fromDatetimeLocal(toDatetimeLocal(at));
      if (!rfc3339) throw new Error("时间格式非法");
      const created = await setReminder(note.id, rfc3339, repeat);
      setReminderState(created);
      onChanged?.();
    } catch (err) {
      console.error("设置提醒失败", err);
      toast.error(`设置提醒失败：${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const handleClear = async () => {
    if (!reminder) return;
    setBusy(true);
    try {
      await cancelReminder(reminder.id);
      setReminderState(null);
      onChanged?.();
    } catch (err) {
      console.error("清除提醒失败", err);
      toast.error(`清除提醒失败：${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const handleCustom = async () => {
    const at = parseRfc3339(fromDatetimeLocal(datetime));
    if (!at) {
      toast.info("请选择有效的提醒时间");
      return;
    }
    await applyAt(at);
  };

  /** 解析自然语言时间（parse_time_nl 未注册/失败时优雅降级为提示） */
  const handleParseNl = async () => {
    const input = nlInput.trim();
    if (!input) return;
    setNlBusy(true);
    try {
      const parsed = await parseTimeNl(input);
      if (!parsed || !parsed.at) {
        setNlPreview(null);
        toast.info("无法识别，请换种说法");
        return;
      }
      setNlPreview(parsed);
    } catch (err) {
      console.error("自然语言时间解析失败", err);
      setNlPreview(null);
      toast.error(`解析失败：${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setNlBusy(false);
    }
  };

  /** 用解析结果设置提醒：at 已是 RFC3339 UTC，可直接走 setReminder */
  const applyNl = async () => {
    if (!nlPreview) return;
    setBusy(true);
    try {
      const created = await setReminder(note.id, nlPreview.at, nlPreview.repeat as RepeatType);
      setReminderState(created);
      setNlPreview(null);
      setNlInput("");
      onChanged?.();
    } catch (err) {
      console.error("设置提醒失败", err);
      toast.error(`设置提醒失败：${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setBusy(false);
    }
  };

  const trigger = (
    <Button
      type="button"
      variant="ghost"
      size="sm"
      className={cn("h-6 gap-1 px-1.5 text-xs", reminder && "text-amber-600 dark:text-amber-400")}
      title={reminder ? `提醒时间：${formatLocal(parseRfc3339(reminder.remindAt) ?? new Date())}` : "设置提醒"}
    >
      {reminder ? (
        <Bell className="size-3.5" />
      ) : (
        <BellPlus className="size-3.5 text-muted-foreground" />
      )}
      {reminder && <span>{describeTime(reminder.remindAt) || formatLocal(parseRfc3339(reminder.remindAt) ?? new Date())}</span>}
    </Button>
  );

  return (
    <Popover open={open} onOpenChange={handleOpenChange}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent align="center" className="w-72 text-sm">
        <div className="mb-2 flex items-center gap-1.5 text-xs text-muted-foreground">
          <CalendarClock className="size-3.5 shrink-0" />
          {reminder ? (
            <span>
              {formatLocal(parseRfc3339(reminder.remindAt) ?? new Date())} ·{" "}
              {REPEAT_LABEL[reminder.repeatType] ?? reminder.repeatType}
            </span>
          ) : (
            <span>暂无提醒</span>
          )}
        </div>

        <div className="grid grid-cols-3 gap-1.5">
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-1 text-xs"
            disabled={busy}
            onClick={() => void applyAt(nextHour())}
          >
            1 小时后
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-1 text-xs"
            disabled={busy}
            onClick={() => void applyAt(tomorrowAt9())}
          >
            明天 09:00
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 px-1 text-xs"
            disabled={busy}
            onClick={() => void applyAt(nextMondayAt9())}
          >
            下周一 09:00
          </Button>
        </div>

        <div className="mt-3 space-y-2">
          <div className="space-y-1">
            <Label htmlFor="reminder-nl" className="text-xs text-muted-foreground">
              自然语言时间
            </Label>
            <div className="flex gap-1.5">
              <Input
                id="reminder-nl"
                value={nlInput}
                onChange={(e) => {
                  setNlInput(e.target.value);
                  setNlPreview(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void handleParseNl();
                  }
                }}
                placeholder="明天下午3点 / 每周一10点 / 30分钟后"
                className="h-8 flex-1 text-xs"
              />
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-8 shrink-0 px-2 text-xs"
                disabled={nlBusy || !nlInput.trim()}
                onClick={() => void handleParseNl()}
              >
                解析
              </Button>
            </div>
            {nlPreview && (
              <div className="flex items-center justify-between gap-2 rounded-md bg-muted px-2 py-1.5">
                <span className="min-w-0 text-xs text-muted-foreground">
                  将提醒：{describeTime(nlPreview.at)}
                  （重复类型：{REPEAT_LABEL[nlPreview.repeat as RepeatType] ?? nlPreview.repeat}）
                </span>
                <Button
                  type="button"
                  size="sm"
                  className="h-6 shrink-0 px-2 text-xs"
                  disabled={busy}
                  onClick={() => void applyNl()}
                >
                  设为提醒
                </Button>
              </div>
            )}
          </div>
          <div className="space-y-1">
            <Label htmlFor="reminder-datetime" className="text-xs text-muted-foreground">
              自定义时间
            </Label>
            <Input
              id="reminder-datetime"
              type="datetime-local"
              value={datetime}
              onChange={(e) => setDatetime(e.target.value)}
              className="h-8 text-xs"
            />
          </div>
          <div className="space-y-1">
            <Label htmlFor="reminder-repeat" className="text-xs text-muted-foreground">
              重复
            </Label>
            <select
              id="reminder-repeat"
              value={repeat}
              onChange={(e) => setRepeat(e.target.value as RepeatType)}
              className="h-8 w-full rounded-md border border-input bg-transparent px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {(Object.keys(REPEAT_LABEL) as RepeatType[]).map((k) => (
                <option key={k} value={k}>
                  {REPEAT_LABEL[k]}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="mt-3 flex items-center justify-between gap-2">
          <Button
            type="button"
            size="sm"
            className="h-7 flex-1 text-xs"
            disabled={busy || !datetime}
            onClick={() => void handleCustom()}
          >
            设置提醒
          </Button>
          {reminder && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-7 text-xs"
              disabled={busy}
              onClick={() => void handleClear()}
            >
              清除提醒
            </Button>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
