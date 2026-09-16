/** Markdown 编辑器：格式工具栏 + 快捷键 + 图片粘贴转发 + 预览切换 */
import { useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
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
  Quote,
  Strikethrough,
  Table,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
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
  taskMetaList?: { taskKey: string; lineText: string; status: string; skipDate: string | null }[];
  today?: string;
  runningTaskKey?: string;
  onTaskMenu?: (e: { taskKey: string; lineText: string; status: "todo" | "done" | "skipped" | "running" }) => void;
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
  /** 番茄任务元数据（可选） */
  taskPassthrough?: TaskMetaPassthrough;
}

interface Mutation {
  text: string;
  selStart: number;
  selEnd?: number;
}

type TextFn = (text: string, start: number, end: number) => Mutation;

export function MarkdownEditor({
  value,
  onChange,
  preview,
  onTogglePreview: _onTogglePreview,
  onImagePaths: _onImagePaths,
  onPasteImage,
  taskPassthrough,
}: MarkdownEditorProps) {
  const taRef = useRef<HTMLTextAreaElement | null>(null);
  const [dragActive, setDragActive] = useState(false);

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

  const tools: { title: string; icon: LucideIcon; run: () => void }[] = [
    { title: "加粗 (Ctrl+B)", icon: Bold, run: () => wrap("**") },
    { title: "斜体 (Ctrl+I)", icon: Italic, run: () => wrap("*") },
    { title: "删除线", icon: Strikethrough, run: () => wrap("~~") },
    { title: "行内代码", icon: Code, run: () => wrap("`") },
    { title: "二级标题", icon: Heading2, run: () => insert(SNIPPETS.heading) },
    { title: "无序列表", icon: List, run: () => togglePrefix("- ") },
    { title: "待办", icon: ListTodo, run: () => togglePrefix("- [ ] ") },
    { title: "引用", icon: Quote, run: () => togglePrefix("> ") },
    { title: "代码块", icon: FileCode, run: () => insert(SNIPPETS.codeBlock) },
    { title: "链接 (Ctrl+K)", icon: Link2, run: insertLink },
    { title: "表格", icon: Table, run: () => insert(SNIPPETS.table) },
    { title: "分割线", icon: Minus, run: () => insert(SNIPPETS.divider) },
  ];

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
            {tools.map((tool) => (
              <Tooltip key={tool.title}>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
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
          </div>
        )}

        {preview ? (
          <MarkdownView
            content={value}
            onToggleTodo={handleToggleTodo}
            taskMetaList={taskPassthrough?.taskMetaList}
            today={taskPassthrough?.today}
            runningTaskKey={taskPassthrough?.runningTaskKey}
            onTaskMenu={taskPassthrough?.onTaskMenu}
            className="min-h-0 flex-1 overflow-y-auto p-3"
          />
        ) : (
          <textarea
            ref={taRef}
            value={value}
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={handleKeyDown}
            className="min-h-0 flex-1 resize-none bg-transparent p-3 text-sm leading-relaxed outline-none placeholder:text-muted-foreground"
            placeholder="记录点什么…"
            spellCheck={false}
          />
        )}
      </div>
    </TooltipProvider>
  );
}
