import * as React from "react";

import { cn } from "@/lib/utils";

/** 骨架屏占位（shadcn 标准模式：pulse 动画 + muted 底色） */
function Skeleton({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn("animate-pulse rounded-md bg-muted", className)}
      {...props}
    />
  );
}

export { Skeleton };
