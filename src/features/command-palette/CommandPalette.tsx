/** Ctrl+K 命令面板：受控组件 + 自带监听的 Host（供 ManagerWindow 挂载） */
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { CornerDownLeft, Search } from "lucide-react";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { useCommandPalette, type CommandItem } from "./useCommandPalette";

export interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="inline-flex min-w-4 items-center justify-center rounded border bg-muted px-1 font-sans text-[10px] leading-4">
      {children}
    </kbd>
  );
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const [input, setInput] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);

  const items = useCommandPalette(input);
  const safeActive = items.length > 0 ? Math.min(active, items.length - 1) : -1;

  // 打开时重置输入与高亮，并聚焦输入框（等 Radix portal 渲染完成）
  useEffect(() => {
    if (!open) return;
    setInput("");
    setActive(0);
    const timer = setTimeout(() => inputRef.current?.focus(), 0);
    return () => clearTimeout(timer);
  }, [open]);

  // 激活项保持在可视区内
  useEffect(() => {
    listRef.current
      ?.querySelector<HTMLElement>('[data-active="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [safeActive, items]);

  const runItem = (item: CommandItem) => {
    onOpenChange(false);
    item.run();
  };

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      if (items.length > 0) setActive((safeActive + 1) % items.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (items.length > 0) setActive((safeActive - 1 + items.length) % items.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      const item = items[safeActive];
      if (item) runItem(item);
    }
    // Esc 由 Radix Dialog 统一处理（onOpenChange(false)）
  };

  // 按组分区（保持命令注册顺序）
  const groups = useMemo(() => {
    const out: { name: string; entries: { item: CommandItem; index: number }[] }[] = [];
    const byName = new Map<string, { name: string; entries: { item: CommandItem; index: number }[] }>();
    items.forEach((item, index) => {
      let group = byName.get(item.group);
      if (!group) {
        group = { name: item.group, entries: [] };
        byName.set(item.group, group);
        out.push(group);
      }
      group.entries.push({ item, index });
    });
    return out;
  }, [items]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="top-[12%] max-w-xl translate-y-0 gap-0 overflow-hidden rounded-xl border-border/60 p-0 shadow-2xl">
        <DialogTitle className="sr-only">命令面板</DialogTitle>

        {/* 顶部输入 */}
        <div className="flex items-center gap-2.5 border-b px-4 pr-12">
          <Search className="size-4 shrink-0 text-muted-foreground" />
          <input
            ref={inputRef}
            value={input}
            placeholder="输入命令或搜索…"
            spellCheck={false}
            className="h-12 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
            onChange={(e) => {
              setInput(e.target.value);
              setActive(0);
            }}
            onKeyDown={handleKeyDown}
          />
        </div>

        {/* 分组列表 */}
        <div ref={listRef} className="max-h-80 overflow-y-auto p-1.5" role="listbox">
          {items.length === 0 ? (
            <div className="px-3 py-10 text-center text-xs text-muted-foreground">
              没有匹配的命令
            </div>
          ) : (
            groups.map((group) => (
              <div key={group.name} className="px-1 py-0.5">
                <div className="px-2 py-1 text-[11px] font-medium text-muted-foreground">
                  {group.name}
                </div>
                {group.entries.map(({ item, index }) => {
                  const isActive = index === safeActive;
                  return (
                    <button
                      key={item.id}
                      type="button"
                      role="option"
                      aria-selected={isActive}
                      data-active={isActive}
                      className={cn(
                        "flex w-full items-center gap-2.5 rounded-md px-2 py-2 text-left text-sm outline-none",
                        isActive
                          ? "bg-accent text-accent-foreground"
                          : "text-foreground hover:bg-accent/60",
                      )}
                      onMouseMove={() => setActive(index)}
                      onClick={() => runItem(item)}
                    >
                      {item.icon && (
                        <item.icon className="size-4 shrink-0 text-muted-foreground" />
                      )}
                      <span className="min-w-0 flex-1 truncate">{item.title}</span>
                      {isActive && (
                        <CornerDownLeft className="size-3.5 shrink-0 text-muted-foreground" />
                      )}
                    </button>
                  );
                })}
              </div>
            ))
          )}
        </div>

        {/* 底部快捷键提示条 */}
        <div className="flex items-center gap-3 border-t bg-muted/40 px-4 py-2 text-[11px] text-muted-foreground">
          <span className="flex items-center gap-1">
            <Kbd>↑</Kbd>
            <Kbd>↓</Kbd> 选择
          </span>
          <span className="flex items-center gap-1">
            <Kbd>Enter</Kbd> 执行
          </span>
          <span className="flex items-center gap-1">
            <Kbd>Esc</Kbd> 关闭
          </span>
          <span className="ml-auto">{items.length} 项</span>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** App 级挂载接口：自带 Ctrl+K 监听（capture 阶段）与开关状态。
 *  总控只需在 ManagerWindow 里放 <CommandPaletteHost /> 即可。 */
export function CommandPaletteHost() {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      // 快捷键录制中不抢占（用户可能正在录制 Ctrl+K 本身）
      if (document.documentElement.hasAttribute("data-shortcut-recording")) return;
      if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        e.stopPropagation();
        setOpen((v) => !v);
      }
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);

  return <CommandPalette open={open} onOpenChange={setOpen} />;
}
