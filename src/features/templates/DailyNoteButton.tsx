/** 每日笔记按钮：点击 invoke Rust `daily_get_or_create`（无参 -> Note）。
 *  后端负责找到/创建当天便签并打开窗口，因此成功保持静默；失败 alert 提示。
 *  命令注册前调用会 reject（走 alert 分支），供总控与 TemplatePicker 一起挂 ManagerWindow 新建区。 */
import { useState, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CalendarCheck2 } from "lucide-react";
import { Button, type ButtonProps } from "@/components/ui/button";

/** `daily_get_or_create` 返回的 Note（镜像 src/types#Note，仅声明常用字段，多余字段透传） */
export interface DailyNoteResult {
  id: string;
  title: string;
  content: string;
}

export interface DailyNoteButtonProps extends ButtonProps {
  /** 打开/创建成功后的可选回调（窗口已由后端打开，这里只通知宿主） */
  onReady?: (note: DailyNoteResult) => void;
}

export function DailyNoteButton({ onReady, onClick, ...props }: DailyNoteButtonProps) {
  const [busy, setBusy] = useState(false);

  const handleClick = (e: MouseEvent<HTMLButtonElement>) => {
    onClick?.(e);
    if (e.defaultPrevented) return;
    setBusy(true);
    invoke<DailyNoteResult>("daily_get_or_create")
      .then((note) => onReady?.(note))
      .catch((err: unknown) => {
        const msg = err instanceof Error ? err.message : String(err);
        alert(`打开每日笔记失败：${msg}`);
      })
      .finally(() => setBusy(false));
  };

  return (
    <Button type="button" variant="outline" size="sm" {...props} disabled={busy || props.disabled} onClick={handleClick}>
      <CalendarCheck2 className="size-3.5" />
      每日笔记
    </Button>
  );
}
