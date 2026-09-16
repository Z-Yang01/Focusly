/** Markdown 渲染：GFM 支持、任务清单可勾选、链接外开、本地图片 asset 协议渲染。
 *  可选任务增强（taskMetaList/today/runningTaskKey/onTaskMenu）：经 mergeTaskMeta
 *  合并出 todo / done / skipped / running 四态渲染 + 任务行右键回调；
 *  props 缺省时行为与旧版完全一致。 */
import {
  Children,
  isValidElement,
  useMemo,
  useState,
  type ComponentPropsWithoutRef,
  type MouseEvent as ReactMouseEvent,
} from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import { ImageOff } from "lucide-react";
import { openExternal } from "@/lib/api";
import { convertFileSrc } from "@/lib/tauri";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";
import { assignTaskKeys } from "@/features/todo/taskKey";
import { parseTodos } from "@/features/todo/todo";
import {
  mergeTaskMeta,
  type EffectiveStatus,
  type MetaLike,
  type TaskDisplayState,
} from "@/features/todo/taskMeta";
import type { Element } from "hast";

export interface MarkdownViewProps {
  content: string;
  /** 行号为 0-based（与 todo.ts 一致；remark 是 1-based，内部已减 1） */
  onToggleTodo?: (line: number, checked: boolean) => void;
  className?: string;
  /** task_meta 列表（结构与 taskMeta.ts 的 MetaLike 一致；缺省 = 不启用任务增强） */
  taskMetaList?: MetaLike[];
  /** 本地日期 YYYY-MM-DD（skip 跨日复活判断），缺省取当天 */
  today?: string;
  /** 正在专注的任务 key（番茄运行态 → 🍅） */
  runningTaskKey?: string;
  /** 任务行右键 / 清除跳过时回调（任务菜单本体由 NoteWindow 总控实现） */
  onTaskMenu?: (e: { taskKey: string; lineText: string; status: EffectiveStatus }) => void;
}

type AnchorProps = ComponentPropsWithoutRef<"a"> & ExtraProps;
type ImageProps = ComponentPropsWithoutRef<"img"> & ExtraProps;
type LiProps = ComponentPropsWithoutRef<"li"> & ExtraProps;

/** 本地日期键 YYYY-MM-DD（skip_date 比较用） */
function localTodayStr(): string {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** 从本地路径 / URL 中取文件名 */
function fileNameOf(src: string): string {
  const name = src.replace(/[\\/]+$/, "").split(/[\\/]/).pop() ?? src;
  try {
    return decodeURIComponent(name);
  } catch {
    return name;
  }
}

/** 图片：本地绝对路径经 convertFileSrc 转 asset URL；加载失败显示占位 */
function MarkdownImage({ src, alt }: { src: string; alt: string }) {
  const [failed, setFailed] = useState(false);

  if (!src || failed) {
    return (
      <span className="inline-flex items-center gap-1.5 rounded-md border border-dashed px-2 py-1 text-xs text-muted-foreground align-middle">
        <ImageOff className="size-3.5 shrink-0" />
        图片缺失：{src ? fileNameOf(src) : "未知"}
      </span>
    );
  }

  const url = /^https?:\/\//i.test(src) ? src : convertFileSrc(src);
  return (
    <img
      src={url}
      alt={alt}
      loading="lazy"
      onError={() => setFailed(true)}
      className="max-w-full rounded-md border align-middle"
    />
  );
}

export function MarkdownView({
  content,
  onToggleTodo,
  className,
  taskMetaList,
  today,
  runningTaskKey,
  onTaskMenu,
}: MarkdownViewProps) {
  /** 是否启用任务增强（缺省时完全走旧渲染路径） */
  const enhanced = taskMetaList !== undefined || runningTaskKey !== undefined;

  /** 0-based 行号 → 合并后的任务展示态 */
  const taskByLine = useMemo(() => {
    const empty = new Map<number, TaskDisplayState>();
    if (!enhanced) return empty;
    const todos = assignTaskKeys(parseTodos(content));
    const merged = mergeTaskMeta(todos, taskMetaList ?? [], today ?? localTodayStr(), runningTaskKey);
    return new Map(merged.tasks.map((t) => [t.line, t]));
  }, [enhanced, content, taskMetaList, today, runningTaskKey]);

  const components: Components = {
    a: (props: AnchorProps) => {
      const href = typeof props.href === "string" ? props.href : undefined;
      return (
        <a
          href={href}
          title={href}
          className="break-all text-blue-600 underline decoration-blue-400 underline-offset-2 hover:text-blue-700 dark:text-blue-400 dark:hover:text-blue-300"
          onClick={(e) => {
            if (!href || !/^https?:\/\//i.test(href)) return;
            e.preventDefault();
            void openExternal(href).catch((err) => {
              console.error("打开外部链接失败", err);
            });
          }}
        >
          {props.children}
        </a>
      );
    },
    img: (props: ImageProps) => (
      <MarkdownImage
        src={typeof props.src === "string" ? props.src : ""}
        alt={props.alt ?? ""}
      />
    ),
    li: (props: LiProps) => {
      const { node, className: cls, children, ...rest } = props;
      const isTask = typeof cls === "string" && cls.includes("task-list-item");

      if (!isTask || !node) {
        return (
          <li className={cls} {...rest}>
            {children}
          </li>
        );
      }

      // GFM 任务项：li 第一个子元素是 checkbox input，取其 checked / disabled
      const inputNode = node.children.find(
        (c): c is Element => c.type === "element" && c.tagName === "input",
      );
      const inputProps = inputNode?.properties as
        | { checked?: boolean; disabled?: boolean }
        | undefined;
      const checked = Boolean(inputProps?.checked);
      // remark-gfm 生成的任务复选框自带 disabled 属性，但 Focusly 的便签始终可交互，
      // 不能把它当作"用户禁用"处理（否则点击永远无效）
      const disabled = false;
      // li 的 position 从 "- [ ]" 源行开始；remark 是 1-based，转 0-based
      const line = node.position?.start.line;
      const line0 = line !== undefined ? line - 1 : undefined;

      const task = enhanced && line0 !== undefined ? taskByLine.get(line0) : undefined;
      // 合并态：running > done > skipped > todo（未启用增强时退化为 markdown 勾选态）
      const status: EffectiveStatus = task
        ? task.status
        : checked
          ? "done"
          : "todo";
      const isSkipped = status === "skipped";
      const isRunning = status === "running";

      const inner = Children.toArray(children).filter(
        (c) => !(isValidElement(c) && (c.props as { type?: unknown }).type === "checkbox"),
      );

      // 控制区：running → 🍅；skipped → ✖ 徽标（点击清除跳过）；其余 → Checkbox
      const control = isRunning ? (
        <span
          className="mt-0.5 inline-flex shrink-0 text-base leading-none"
          title="专注中"
        >
          🍅
        </span>
      ) : isSkipped ? (
        <span
          role="button"
          aria-label="清除跳过，恢复为待办"
          title="清除跳过（恢复为待办）"
          className={cn(
            "mt-0.5 inline-flex shrink-0 cursor-pointer items-center rounded border px-1",
            "text-[10px] leading-4 text-muted-foreground hover:bg-accent",
          )}
          onClick={() => {
            if (line0 === undefined || !task) return;
            onToggleTodo?.(line0, false); // 保持 markdown 源为未勾选
            onTaskMenu?.({ taskKey: task.taskKey, lineText: task.text, status: "todo" });
          }}
        >
          ✕
        </span>
      ) : (
        <span
          role={disabled ? undefined : "checkbox"}
          aria-checked={status === "done"}
          aria-disabled={disabled}
          aria-label="切换待办状态"
          className={cn("mt-0.5 inline-flex shrink-0", !disabled && "cursor-pointer")}
          onClick={() => {
            if (disabled || line0 === undefined) return;
            onToggleTodo?.(line0, status !== "done");
          }}
        >
          <Checkbox checked={status === "done"} disabled={disabled} className="pointer-events-none" />
        </span>
      );

      // 任务行右键 → 交由 NoteWindow 总控弹出任务菜单
      const handleContextMenu = task
        ? (e: ReactMouseEvent<HTMLLIElement>) => {
            e.preventDefault();
            onTaskMenu?.({ taskKey: task.taskKey, lineText: task.text, status: task.status });
          }
        : undefined;

      return (
        <li className={cn("flex items-start gap-2", cls)} {...rest} onContextMenu={handleContextMenu}>
          {control}
          <div className={cn("min-w-0 flex-1", isSkipped && "text-muted-foreground line-through")}>
            {inner}
          </div>
        </li>
      );
    },
  };

  return (
    <div className={cn("md-body text-sm leading-relaxed break-words", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={components}>
        {content}
      </ReactMarkdown>
    </div>
  );
}
