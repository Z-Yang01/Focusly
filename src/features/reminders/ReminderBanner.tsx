/** 提醒触发横幅：完成 / 稍后提醒 / 关闭 */
import { BellRing, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ReminderFiredEvent } from "@/types";

export interface ReminderBannerProps {
  fired: ReminderFiredEvent;
  /** 完成提醒 → completeReminder */
  onDone: () => void;
  /** 稍后提醒（分钟数）→ snoozeReminder */
  onSnooze: (minutes: number) => void;
  /** 关闭提醒 → cancelReminder */
  onDismiss: () => void;
  onOpenNote?: () => void;
}

/** 距明天 09:00 的分钟数 */
function minutesUntilTomorrow9(): number {
  const t = new Date();
  t.setDate(t.getDate() + 1);
  t.setHours(9, 0, 0, 0);
  return Math.max(1, Math.round((t.getTime() - Date.now()) / 60000));
}

export function ReminderBanner({
  fired,
  onDone,
  onSnooze,
  onDismiss,
  onOpenNote,
}: ReminderBannerProps) {
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-amber-200 bg-amber-50 px-2 py-1.5 text-sm dark:border-amber-900 dark:bg-amber-950/60">
      <BellRing className="size-4 shrink-0 text-amber-500" />
      <button
        type="button"
        className="min-w-0 max-w-[45%] truncate text-left font-medium hover:underline"
        title={fired.noteTitle || "无标题"}
        onClick={onOpenNote}
      >
        {fired.noteTitle || "无标题"}
      </button>

      <div className="ml-auto flex flex-wrap items-center gap-1">
        <Button
          type="button"
          size="sm"
          className="h-6 px-2 text-xs"
          onClick={onDone}
        >
          完成
        </Button>
        <Button type="button" variant="outline" size="sm" className="h-6 px-2 text-xs" onClick={() => onSnooze(5)}>
          5 分钟
        </Button>
        <Button type="button" variant="outline" size="sm" className="h-6 px-2 text-xs" onClick={() => onSnooze(30)}>
          30 分钟
        </Button>
        <Button type="button" variant="outline" size="sm" className="h-6 px-2 text-xs" onClick={() => onSnooze(60)}>
          1 小时
        </Button>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-6 px-2 text-xs"
          onClick={() => onSnooze(minutesUntilTomorrow9())}
        >
          明天
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className="size-6 text-muted-foreground hover:text-foreground"
          title="关闭提醒"
          onClick={onDismiss}
        >
          <X className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}
