/** 全局快捷键录制控件 */
import { useEffect, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { Button } from "@/components/ui/button";
import {
  acceleratorToDisplay,
  isValidAccelerator,
  normalizeAccelerator,
} from "@/lib/shortcut";
import { cn } from "@/lib/utils";

export interface ShortcutRecorderProps {
  value: string;
  onChange: (v: string) => void;
  onError?: (msg: string) => void;
}

export function ShortcutRecorder({ value, onChange, onError }: ShortcutRecorderProps) {
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 录制态在根元素打标志：命令面板等全局捕获监听器据此放行，
  // 否则 Ctrl+K 这类组合永远被面板抢占、录制框收不到事件
  useEffect(() => {
    if (!recording) return;
    document.documentElement.setAttribute("data-shortcut-recording", "1");
    return () => document.documentElement.removeAttribute("data-shortcut-recording");
  }, [recording]);

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") {
      setRecording(false);
      setError(null);
      return;
    }
    const accelerator = normalizeAccelerator(e);
    if (!accelerator) return; // 正按着修饰键或未识别，继续等待
    if (!isValidAccelerator(accelerator)) {
      const msg = `“${acceleratorToDisplay(accelerator)}” 不是有效快捷键：需至少包含一个修饰键（Ctrl / Alt / Super）`;
      setError(msg);
      onError?.(msg);
      return;
    }
    setError(null);
    setRecording(false);
    onChange(accelerator);
  };

  if (recording) {
    return (
      <div className="flex flex-col gap-1">
        <div
          tabIndex={0}
          autoFocus
          role="textbox"
          aria-label="按下新的快捷键"
          className={cn(
            "flex h-8 min-w-40 items-center justify-center rounded-md border border-dashed border-primary bg-accent/50 px-2 text-xs text-muted-foreground outline-none",
            error && "border-destructive text-destructive",
          )}
          onKeyDown={handleKeyDown}
          onBlur={() => {
            setRecording(false);
            setError(null);
          }}
        >
          {error ?? "按下新快捷键，Esc 取消"}
        </div>
      </div>
    );
  }

  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      className="h-8 min-w-40 justify-between gap-2 font-mono text-xs"
      onClick={() => setRecording(true)}
    >
      <span>{value ? acceleratorToDisplay(value) : "未设置"}</span>
      <span className="font-sans text-[10px] text-muted-foreground">点击修改</span>
    </Button>
  );
}
