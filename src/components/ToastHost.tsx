/** 全局 Toast 展示层：固定右下角堆叠，在应用根部挂载一次。
 *  每个 Tauri 窗口是独立 JS 上下文，各自持有一个实例——谁触发谁看到。 */
import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useToastStore, type ToastItem, type ToastKind } from "@/stores/toast";
import { cn } from "@/lib/utils";

interface KindStyle {
  Icon: ComponentType<{ className?: string }>;
  iconClass: string;
  borderClass: string;
}

const kindStyles: Record<ToastKind, KindStyle> = {
  success: {
    Icon: CheckCircle2,
    iconClass: "text-emerald-600 dark:text-emerald-400",
    borderClass: "border-emerald-500/40",
  },
  error: {
    Icon: XCircle,
    iconClass: "text-destructive",
    borderClass: "border-destructive/50",
  },
  info: {
    Icon: Info,
    iconClass: "text-blue-500 dark:text-blue-400",
    borderClass: "border-blue-500/40",
  },
};

/** 单条 Toast：入场用 CSS transition 从右侧滑入（挂载后下一帧切换到终态） */
function ToastEntry({ item }: { item: ToastItem }) {
  const dismiss = useToastStore((s) => s.dismiss);
  const [entered, setEntered] = useState(false);

  useEffect(() => {
    const raf = requestAnimationFrame(() => setEntered(true));
    return () => cancelAnimationFrame(raf);
  }, []);

  const { Icon, iconClass, borderClass } = kindStyles[item.kind];

  return (
    <div
      role={item.kind === "error" ? "alert" : "status"}
      className={cn(
        "pointer-events-auto flex w-80 max-w-[calc(100vw-2rem)] items-start gap-2.5 rounded-lg border bg-popover py-2.5 pl-3.5 pr-2 text-popover-foreground shadow-lg transition-all duration-200 ease-out",
        borderClass,
        entered ? "translate-x-0 opacity-100" : "translate-x-4 opacity-0",
      )}
    >
      <Icon className={cn("mt-0.5 size-4 shrink-0", iconClass)} />
      <p className="min-w-0 flex-1 break-words text-sm leading-5">{item.message}</p>
      <button
        type="button"
        onClick={() => dismiss(item.id)}
        aria-label="关闭提示"
        className="rounded p-1 text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:outline-none"
      >
        <X className="size-3.5" />
      </button>
    </div>
  );
}

interface ToastHostProps {
  children?: ReactNode;
}

export function ToastHost({ children }: ToastHostProps) {
  const toasts = useToastStore((s) => s.toasts);
  return (
    <>
      {children}
      <div
        aria-live="polite"
        className="pointer-events-none fixed bottom-4 right-4 z-[100] flex flex-col items-end gap-2"
      >
        {toasts.map((t) => (
          <ToastEntry key={t.id} item={t} />
        ))}
      </div>
    </>
  );
}
