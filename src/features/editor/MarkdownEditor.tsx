/** Markdown 编辑器：格式工具栏 + 快捷键 + 图片粘贴转发 + 预览切换 */
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  Bold,
  Code,
  FileCode,
  Heading2,
  Italic,
  Link2,
  List,
  ListTodo,
  Minus,
  MoreHorizontal,
  Quote,
  Strikethrough,
  Table,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { insertAtCursor, SNIPPETS, toggleLinePrefix, wrapSelection } from "./editorActions";
import { MarkdownView } from "./MarkdownView";
import { toggleTodoAtLine } from "@/features/todo/todo";
import { cn } from "@/lib/utils";

/** 番茄/任务元数据透传（缺省 = 完全旧行为） */
export interface TaskMetaPassthrough {
  taskMetaList?: {
    taskKey: string;
    lineText: string;
    status: string;
    skipDate: string | null;
    sortOrder?: number | null;
  }[];
  today?: string;
  runningTaskKey?: string;
  onTaskMenu?: (e: { taskKey: string; lineText: string; status: "todo" | "done" | "skipped" | "running" }) => void;
  /** 待办拖拽排序落位（持久化写 task_meta.sort_order） */
  onTaskDrop?: (e: { dragKey: string; targetKey: string; before: boolean }) => void;
}

export interface MarkdownEditorProps {
  value: string;
  onChange: (v: string) => void;
  preview: boolean;
  onTogglePreview: () => void;
  /** 图片路径由父组件（tauri onDragDropEvent）提供时经此转发 */
  onImagePaths: (paths: string[]) => void;
  /** Ctrl+V 检测到剪贴板图片时调用（父组件读剪贴板并入库） */
  onPasteImage: () => void;
  noteId: string;
  /** 内容与标题均为空时显示多行新手引导 placeholder */
  emptyGuide?: boolean;
  /** 番茄任务元数据（可选） */
  taskPassthrough?: TaskMetaPassthrough;
}

interface Mutation {
  text: string;
  selStart: number;
  selEnd?: number;
}

type TextFn = (text: string, start: number, end: number) => Mutation;

interface Tool {
  id: string;
  title: string;
  icon: LucideIcon;
  run: () => void;
}

/** 常驻显示的 6 个高频工具：B / I / 删除线 / H2 / 列表 / 待办 */
const PRIMARY_TOOL_IDS = ["bold", "italic", "strike", "heading", "list", "todo"];
/** 窄窗口（<300px）只保留 3 个：B / I / 待办，其余全部收进 More 菜单 */
const NARROW_TOOL_IDS = ["bold", "italic", "todo"];

export function MarkdownEditor({
  value,
  onChange,
  preview,
  onTogglePreview,
  onImagePaths: _onImagePaths,
  onPasteImage,
  emptyGuide = false,
  taskPassthrough,
}: MarkdownEditorProps) {
  const taRef = useRef<HTMLTextAreaElement | null>(null);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [dragActive, setDragActive] = useState(false);
  /** 编辑区宽度 <300px 时收窄工具栏（ResizeObserver 实测容器，而非窗口） */
  const [narrow, setNarrow] = useState(false);

  useEffect(() => {
    const el = rootRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setNarrow(entry.contentRect.width < 300);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const applyMutation = (mut: Mutation) => {
    onChange(mut.text);
    const ta = taRef.current;
    if (!ta) return;
    const { selStart, selEnd } = mut;
    requestAnimationFrame(() => {
      ta.focus();
      ta.setSelectionRange(selStart, selEnd ?? selStart);
    });
  };

  const withTextarea = (fn: TextFn) => {
    const ta = taRef.current;
    if (!ta) return;
    const start = ta.selectionStart ?? 0;
    const end = ta.selectionEnd ?? start;
    applyMutation(fn(value, start, end));
  };

  const insert = (snippet: string) => {
    withTextarea((text, start) => insertAtCursor(text, start, snippet));
  };

  const togglePrefix = (prefix: string) => {
    withTextarea((text, start) => toggleLinePrefix(text, start, prefix));
  };

  const wrap = (marker: string) => {
    withTextarea((text, start, end) => wrapSelection(text, start, end, marker));
  };

  const insertLink = () => {
    const ta = taRef.current;
    if (!ta) return;
    const start = ta.selectionStart ?? 0;
    const end = ta.selectionEnd ?? start;
    const selected = value.slice(start, end);
    if (!selected) {
      insert(SNIPPETS.link);
      return;
    }
    const prefix = `[${selected}](`;
    const url = "https://";
    applyMutation({
      text: value.slice(0, start) + prefix + url + ")" + value.slice(end),
      selStart: start + prefix.length,
      selEnd: start + prefix.length + url.length,
    });
  };

  const tools: Tool[] = [
    { id: "bold", title: "加粗 (Ctrl+B)", icon: Bold, run: () => wrap("**") },
    { id: "italic", title: "斜体 (Ctrl+I)", icon: Italic, run: () => wrap("*") },
    { id: "strike", title: "删除线", icon: Strikethrough, run: () => wrap("~~") },
    { id: "code", title: "行内代码", icon: Code, run: () => wrap("`") },
    { id: "heading", title: "二级标题", icon: Heading2, run: () => insert(SNIPPETS.heading) },
    { id: "list", title: "无序列表", icon: List, run: () => togglePrefix("- ") },
    { id: "todo", title: "待办", icon: ListTodo, run: () => togglePrefix("- [ ] ") },
    { id: "quote", title: "引用", icon: Quote, run: () => togglePrefix("> ") },
    { id: "codeBlock", title: "代码块", icon: FileCode, run: () => insert(SNIPPETS.codeBlock) },
    { id: "link", title: "链接 (Ctrl+K)", icon: Link2, run: insertLink },
    { id: "table", title: "表格", icon: Table, run: () => insert(SNIPPETS.table) },
    { id: "divider", title: "分割线", icon: Minus, run: () => insert(SNIPPETS.divider) },
  ];

  const shownIds = narrow ? NARROW_TOOL_IDS : PRIMARY_TOOL_IDS;
  const shownTools = tools.filter((t) => shownIds.includes(t.id));
  const moreTools = tools.filter((t) => !shownIds.includes(t.id));

  const handleKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    const key = e.key.toLowerCase();
    if (key === "b") {
      e.preventDefault();
      wrap("**");
    } else if (key === "i") {
      e.preventDefault();
      wrap("*");
    } else if (key === "k") {
      e.preventDefault();
      insertLink();
    } else if (key === "v") {
      // 剪贴板含图片且无文本 → 转交父组件读剪贴板入库
      const data = (
        e.nativeEvent as KeyboardEvent & { clipboardData?: DataTransfer | null }
      ).clipboardData;
      const files = data?.files;
      const hasText = data?.types?.includes("text/plain") ?? false;
      const hasImage = files
        ? Array.from(files).some((f) => f.type.startsWith("image/"))
        : false;
      if (hasImage && !hasText) {
        e.preventDefault();
        onPasteImage();
      }
    }
  };

  const handleToggleTodo = (line: number, checked: boolean) => {
    onChange(toggleTodoAtLine(value, line, checked));
  };

  return (
    <TooltipProvider delayDuration={300}>
      <div
        ref={rootRef}
        className={cn(
          "flex h-full min-h-0 flex-col rounded-md transition-shadow",
          dragActive && "ring-2 ring-primary ring-inset",
        )}
        onDragOver={(e) => {
          e.preventDefault();
          setDragActive(true);
        }}
        onDragLeave={() => setDragActive(false)}
        onDrop={(e) => {
          // 路径由父组件通过 tauri onDragDropEvent 提供，这里仅阻断默认行为与高亮
          e.preventDefault();
          setDragActive(false);
        }}
      >
        {!preview && (
          <div className="flex flex-wrap items-center gap-0.5 border-b px-1.5 py-1">
            {shownTools.map((tool) => (
              <Tooltip key={tool.id}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label={tool.title}
                    className="size-7 text-muted-foreground hover:text-foreground"
                    // 防止点击工具栏时丢失 textarea 选区
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={tool.run}
                  >
                    <tool.icon className="size-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs">
                  {tool.title}
                </TooltipContent>
              </Tooltip>
            ))}
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label="更多格式"
                      className="size-7 text-muted-foreground hover:text-foreground"
                      onMouseDown={(e) => e.preventDefault()}
                    >
                      <MoreHorizontal className="size-4" />
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs">
                  更多格式
                </TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="start" className="text-sm">
                {moreTools.map((tool) => (
                  <DropdownMenuItem key={tool.id} onSelect={() => tool.run()}>
                    <tool.icon className="size-4 text-muted-foreground" />
                    {tool.title}
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
          </div>
        )}

        {preview ? (
          /* 预览容器：Escape 切回编辑模式（仅预览态生效；焦点在容器内时触发）；px-1 增加沉浸式左右留白 */
          <div
            className="flex min-h-0 flex-1 flex-col px-1"
            onKeyDown={(e) => {
              if (e.key === "Escape") {
                e.preventDefault();
                onTogglePreview();
              }
            }}
          >
            <MarkdownView
              content={value}
              onToggleTodo={handleToggleTodo}
              taskMetaList={taskPassthrough?.taskMetaList}
              today={taskPassthrough?.today}
              runningTaskKey={taskPassthrough?.runningTaskKey}
              onTaskMenu={taskPassthrough?.onTaskMenu}
              onTaskDrop={taskPassthrough?.onTaskDrop}
              className="min-h-0 flex-1 overflow-y-auto p-3"
            />
          </div>
        ) : (
          <textarea
            ref={taRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            className="min-h-0 flex-1 resize-none bg-transparent p-3 text-sm leading-relaxed outline-none placeholder:text-muted-foreground"
            placeholder={emptyGuide ? "记录灵感、待办、笔记…\n支持 Markdown 语法" : "记录点什么…"}
            spellCheck={false}
          />
        )}
      </div>
    </TooltipProvider>
  );
}
