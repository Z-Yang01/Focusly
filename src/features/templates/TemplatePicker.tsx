/** 模板选择器：内置 + 自定义模板网格（名称 + 前 2 行预览），点击回调 onPick(tpl)。
 *  自定义模板存 localStorage（focusly.templates.custom，见 ./templates）；
 *  「管理」模式下可删除自定义模板；底部 actions 插槽留给宿主（如「新建模板」按钮，不强制）。
 *  供总控挂到 ManagerWindow 新建区（与 DailyNoteButton 并列）。 */
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { LayoutTemplate, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  BUILTIN_TEMPLATES,
  deleteCustomTemplate,
  loadCustomTemplates,
  templatePreviewLines,
  type NoteTemplate,
} from "./templates";

export interface TemplatePickerProps {
  /** 选中模板回调（原始模板；{{date}}/{{time}} 渲染由宿主用 applyTemplate 决定时机） */
  onPick: (tpl: NoteTemplate) => void;
  /** 自定义触发器（如宿主自己的按钮）；缺省用内置「模板」按钮 */
  children?: ReactNode;
  /** 底部自定义操作区（如宿主的「新建模板」按钮），缺省时显示占位符提示 */
  actions?: ReactNode;
  /** 触发按钮文案（仅在未传 children 时生效） */
  label?: string;
  className?: string;
}

export function TemplatePicker({
  onPick,
  children,
  actions,
  label = "模板",
  className,
}: TemplatePickerProps) {
  const [open, setOpen] = useState(false);
  const [managing, setManaging] = useState(false);
  const [customs, setCustoms] = useState<NoteTemplate[]>([]);

  // 每次打开时刷新自定义模板（localStorage 可能在别处被修改）
  useEffect(() => {
    if (open) {
      setCustoms(loadCustomTemplates());
      setManaging(false);
    }
  }, [open]);

  const all = useMemo(() => [...BUILTIN_TEMPLATES, ...customs], [customs]);
  const builtinIds = useMemo(() => new Set(BUILTIN_TEMPLATES.map((t) => t.id)), []);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        {children ?? (
          <Button type="button" variant="outline" size="sm" className={className}>
            <LayoutTemplate className="size-3.5" />
            {label}
          </Button>
        )}
      </PopoverTrigger>
      <PopoverContent align="start" className="w-80 p-3">
        <div className="mb-2 flex items-center justify-between">
          <span className="text-xs font-medium text-muted-foreground">选择模板</span>
          {customs.length > 0 && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-xs"
              onClick={() => setManaging((m) => !m)}
            >
              {managing ? "完成" : "管理"}
            </Button>
          )}
        </div>

        <div className="grid max-h-72 grid-cols-2 gap-2 overflow-y-auto">
          {all.map((tpl) => {
            const isCustom = !builtinIds.has(tpl.id);
            const preview = templatePreviewLines(tpl.content);
            return (
              <div key={tpl.id} className="relative">
                <button
                  type="button"
                  onClick={() => {
                    onPick(tpl);
                    setOpen(false);
                  }}
                  className="flex h-full w-full flex-col items-start gap-1 rounded-md border p-2 text-left transition-colors hover:bg-accent hover:text-accent-foreground"
                >
                  <span className="flex max-w-full items-center gap-1 text-xs font-medium">
                    {tpl.icon && (
                      <span aria-hidden className="shrink-0">
                        {tpl.icon}
                      </span>
                    )}
                    <span className="truncate">{tpl.name}</span>
                  </span>
                  <span className="w-full space-y-0.5 text-[11px] leading-snug text-muted-foreground">
                    {preview.map((line, i) => (
                      <span key={i} className="block truncate">
                        {line}
                      </span>
                    ))}
                  </span>
                </button>
                {isCustom && managing && (
                  <Button
                    type="button"
                    variant="destructive"
                    size="icon"
                    className="absolute -right-1 -top-1 size-5 rounded-full"
                    title={`删除模板「${tpl.name}」`}
                    onClick={() => setCustoms(deleteCustomTemplate(tpl.id))}
                  >
                    <Trash2 className="size-3" />
                  </Button>
                )}
              </div>
            );
          })}
        </div>

        <div className="mt-2 flex items-center justify-between border-t pt-2">
          {actions ?? (
            <span className="text-[11px] text-muted-foreground">
              模板支持 {"{{date}}"} / {"{{time}}"} 占位符
            </span>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
