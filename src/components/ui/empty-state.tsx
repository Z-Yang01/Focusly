import type { LucideIcon } from "lucide-react";

import { cn } from "@/lib/utils";

export interface EmptyStateProps {
  /** 主图标（视图语义图标） */
  icon: LucideIcon;
  /** 主文案：一句话说明当前为空 */
  title: string;
  /** 副文案：引导用户下一步做什么 */
  description?: string;
  className?: string;
}

/** 统一空状态：图标 + 主文案 + 副文案（各视图共用，保证视觉一致） */
export function EmptyState({ icon: Icon, title, description, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center gap-2 text-muted-foreground",
        className,
      )}
    >
      <Icon className="size-8 opacity-50" />
      <p className="text-sm">{title}</p>
      {description && (
        <p className="max-w-xs text-center text-xs opacity-70">{description}</p>
      )}
    </div>
  );
}
