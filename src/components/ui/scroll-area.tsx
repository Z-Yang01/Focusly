import * as React from "react";

import { cn } from "@/lib/utils";

/** 轻量 ScrollArea：不依赖 Radix，就是一个 overflow-y-auto 的 div，
 *  尺寸/内边距等通过 className 透传，滚动条样式由全局细滚动条提供。 */
const ScrollArea = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, children, ...props }, ref) => (
    <div ref={ref} className={cn("overflow-y-auto", className)} {...props}>
      {children}
    </div>
  ),
);
ScrollArea.displayName = "ScrollArea";

export { ScrollArea };
