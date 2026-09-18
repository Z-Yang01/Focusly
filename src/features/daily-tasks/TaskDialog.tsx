/** 今日任务添加/编辑对话框（Radix Dialog 封装）。 */
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import type { DailyTask } from "@/types";

export interface TaskDialogValue {
  title: string;
  startTime: string;
  endTime: string;
  note: string;
  estimatePomodoros: number;
  /** 空串 = 跟随全局默认 */
  focusMin: string;
  priority: string;
  repeatRule: string;
  tags: string;
  isPrivate: boolean;
}

export function emptyValue(): TaskDialogValue {
  return {
    title: "",
    startTime: "",
    endTime: "",
    note: "",
    estimatePomodoros: 0,
    focusMin: "",
    priority: "medium",
    repeatRule: "none",
    tags: "",
    isPrivate: false,
  };
}

export function fromTask(t: DailyTask): TaskDialogValue {
  return {
    title: t.title,
    startTime: t.startTime ?? "",
    endTime: t.endTime ?? "",
    note: t.note ?? "",
    estimatePomodoros: t.estimatePomodoros,
    focusMin: t.focusMin != null ? String(t.focusMin) : "",
    priority: t.priority,
    repeatRule: t.repeatRule,
    tags: t.tags ?? "",
    isPrivate: t.isPrivate,
  };
}

interface TaskDialogProps {
  open: boolean;
  /** null = 新建；非 null = 编辑该任务 */
  editing: DailyTask | null;
  value: TaskDialogValue;
  onValueChange: (v: TaskDialogValue) => void;
  onCancel: () => void;
  onSubmit: () => void;
}

const PRIORITIES = [
  { value: "high", label: "高" },
  { value: "medium", label: "中" },
  { value: "low", label: "低" },
];
const REPEATS = [
  { value: "none", label: "不重复" },
  { value: "daily", label: "每天" },
  { value: "weekly", label: "每周" },
  { value: "weekday", label: "工作日" },
];

export function TaskDialog({ open, editing, value, onValueChange, onCancel, onSubmit }: TaskDialogProps) {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) setError(null);
  }, [open]);

  const set = (patch: Partial<TaskDialogValue>) => onValueChange({ ...value, ...patch });

  const submit = () => {
    if (!value.title.trim()) {
      setError("标题不能为空");
      return;
    }
    if (value.startTime && value.endTime && value.startTime >= value.endTime) {
      setError("结束时间需晚于开始时间");
      return;
    }
    setError(null);
    onSubmit();
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onCancel()}>
      <DialogContent className="w-80 max-w-[90vw] text-sm">
        <DialogHeader>
          <DialogTitle>{editing ? "编辑任务" : "添加任务"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-2.5">
          <div className="space-y-1">
            <Label htmlFor="dt-title" className="text-xs text-muted-foreground">标题</Label>
            <Input id="dt-title" value={value.title} autoFocus
              onChange={(e) => set({ title: e.target.value })} className="h-8 text-xs" />
          </div>
          <div className="flex gap-2">
            <div className="min-w-0 flex-1 space-y-1">
              <Label htmlFor="dt-start" className="text-xs text-muted-foreground">开始</Label>
              <Input id="dt-start" type="time" value={value.startTime}
                onChange={(e) => set({ startTime: e.target.value })} className="h-8 text-xs" />
            </div>
            <div className="min-w-0 flex-1 space-y-1">
              <Label htmlFor="dt-end" className="text-xs text-muted-foreground">结束</Label>
              <Input id="dt-end" type="time" value={value.endTime}
                onChange={(e) => set({ endTime: e.target.value })} className="h-8 text-xs" />
            </div>
          </div>
          <div className="flex gap-2">
            <div className="min-w-0 flex-1 space-y-1">
              <Label htmlFor="dt-pri" className="text-xs text-muted-foreground">优先级</Label>
              <select id="dt-pri" value={value.priority}
                onChange={(e) => set({ priority: e.target.value })}
                className="h-8 w-full rounded-md border border-input bg-transparent px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring">
                {PRIORITIES.map((p) => <option key={p.value} value={p.value}>{p.label}</option>)}
              </select>
            </div>
            <div className="min-w-0 flex-1 space-y-1">
              <Label htmlFor="dt-repeat" className="text-xs text-muted-foreground">重复</Label>
              <select id="dt-repeat" value={value.repeatRule}
                onChange={(e) => set({ repeatRule: e.target.value })}
                className="h-8 w-full rounded-md border border-input bg-transparent px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring">
                {REPEATS.map((r) => <option key={r.value} value={r.value}>{r.label}</option>)}
              </select>
            </div>
            <div className="w-20 space-y-1">
              <Label htmlFor="dt-est" className="text-xs text-muted-foreground">预计🍅</Label>
              <Input id="dt-est" type="number" min={0} max={99} value={value.estimatePomodoros}
                onChange={(e) => set({ estimatePomodoros: Math.max(0, Math.min(99, Number(e.target.value) || 0)) })}
                className="h-8 text-xs" />
            </div>
          </div>
          <div className="space-y-1">
            <Label htmlFor="dt-note" className="text-xs text-muted-foreground">备注</Label>
            <Input id="dt-note" value={value.note}
              onChange={(e) => set({ note: e.target.value })} className="h-8 text-xs" />
          </div>
          <div className="space-y-1">
            <Label htmlFor="dt-tags" className="text-xs text-muted-foreground">标签（逗号分隔）</Label>
            <Input id="dt-tags" value={value.tags}
              onChange={(e) => set({ tags: e.target.value })} className="h-8 text-xs" />
          </div>
          <div className="space-y-1">
            <Label htmlFor="dt-focus" className="text-xs text-muted-foreground">
              专注时长（分钟，留空 = 默认 25）
            </Label>
            <Input id="dt-focus" type="number" min={1} max={180} placeholder="25"
              value={value.focusMin}
              onChange={(e) => {
                const v = e.target.value;
                set({ focusMin: v === "" ? "" : String(Math.max(1, Math.min(180, Number(v) || 25))) });
              }}
              className="h-8 text-xs" />
          </div>
          <div className="flex items-center justify-between">
            <Label htmlFor="dt-private" className="text-xs text-muted-foreground">私密任务（通知脱敏）</Label>
            <Switch id="dt-private" checked={value.isPrivate}
              onCheckedChange={(v) => set({ isPrivate: v })} />
          </div>
          {error && <p className="text-xs text-destructive">{error}</p>}
        </div>
        <DialogFooter className="gap-2">
          <Button type="button" variant="outline" size="sm" className="h-7 text-xs" onClick={onCancel}>取消</Button>
          <Button type="button" size="sm" className="h-7 text-xs" onClick={submit}>
            {editing ? "保存" : "添加"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
