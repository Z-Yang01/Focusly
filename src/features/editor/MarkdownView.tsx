/** Markdown 渲染：GFM 支持、任务清单可勾选、链接外开、本地图片 asset 协议渲染。
 *  可选任务增强（taskMetaList/today/runningTaskKey/onTaskMenu）：经 mergeTaskMeta
 *  合并出 todo / done / skipped / running 四态渲染 + 任务行右键回调；
 *  可选待办拖拽排序（onTaskDrop）：展示层按 task_meta.sort_order 重排（todoOrder 纯函数，
 *  不修改存储正文），勾选回写经 key→原始行映射仍命中原正文行。
 *  props 缺省时行为与旧版完全一致。 */
import {
  Children,
  isValidElement,
  useMemo,
  useState,
  type ComponentPropsWithoutRef,
  type DragEvent as ReactDragEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import { GripVertical, ImageOff } from "lucide-react";
import { openExternal } from "@/lib/api";
import { convertFileSrc } from "@/lib/tauri";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";
import { assignTaskKeys } from "@/features/todo/taskKey";
import { parseTodos } from "@/features/todo/todo";
import { applyTaskOrder } from "@/features/todo/todoOrder";
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
  /** 待办拖拽排序落位回调（持久化由 NoteWindow 总控写 task_meta.sort_order） */
  onTaskDrop?: (e: { dragKey: string; targetKey: string; before: boolean }) => void;
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
  onTaskDrop,
}: MarkdownViewProps) {
  /** 是否启用任务增强（缺省时完全走旧渲染路径） */
  const enhanced = taskMetaList !== undefined || runningTaskKey !== undefined;

  /** 拖拽排序的展示层重排（未启用 onTaskDrop 时为 undefined → 渲染原内容） */
  const storedKeys = useMemo(
    () =>
      (taskMetaList ?? [])
        .filter((m) => m.sortOrder != null)
        .sort((a, b) => (a.sortOrder ?? 0) - (b.sortOrder ?? 0))
        .map((m) => m.taskKey),
    [taskMetaList],
  );
  const orderInfo = useMemo(
    () => (onTaskDrop ? applyTaskOrder(content, storedKeys) : undefined),
    [content, storedKeys, onTaskDrop],
  );
  const displayContent = orderInfo?.content ?? content;

  const [dragKey, setDragKey] = useState<string | null>(null);
  const [dropHint, setDropHint] = useState<{ key: string; before: boolean } | null>(null);

  /** 展示行号 → (合并任务态, 原始正文行号)。
   *  key 分配必须基于原始 content（taskKey 的同文本出现序号按原序派生，与 task_meta
   *  的绑定一致）；对重排后的 displayContent 重新派生会让同名任务错绑 meta / 错行回写。
   *  展示行 = displayLineByKey[key]（重排只整行换位，不丢行），回写行 = 原文行号。 */
  const taskInfoByLine = useMemo(() => {
    const empty = new Map<number, { task: TaskDisplayState; originalLine: number }>();
    if (!enhanced) return empty;
    const todos = assignTaskKeys(parseTodos(content));
    const merged = mergeTaskMeta(todos, taskMetaList ?? [], today ?? localTodayStr(), runningTaskKey);
    for (const t of merged.tasks) {
      const displayLine = orderInfo?.displayLineByKey.get(t.taskKey) ?? t.line;
      empty.set(displayLine, { task: t, originalLine: t.line });
    }
    return empty;
  }, [enhanced, content, taskMetaList, today, runningTaskKey, orderInfo]);

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

      const taskInfo = enhanced && line0 !== undefined ? taskInfoByLine.get(line0) : undefined;
      const task = taskInfo?.task;
      // 合并态：running > done > skipped > todo（未启用增强时退化为 markdown 勾选态）
      const status: EffectiveStatus = task
        ? task.status
        : checked
          ? "done"
          : "todo";
      const isSkipped = status === "skipped";
      const isRunning = status === "running";

      /** 勾选回写只认原始正文行号（taskKey → 原文行精确映射）；
       *  映射缺失时宁可 no-op 也绝不把展示行号当原文行写——正文真相源红线。 */
      const toggleOriginal = (checkedNext: boolean) => {
        if (!taskInfo) {
          console.error("待办回写失败：任务行映射缺失", { line0, taskKey: task?.taskKey });
          return;
        }
        onToggleTodo?.(taskInfo.originalLine, checkedNext);
      };

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
          tabIndex={0}
          aria-label="清除跳过，恢复为待办"
          title="清除跳过（恢复为待办）"
          className={cn(
            "mt-0.5 inline-flex shrink-0 cursor-pointer items-center rounded border px-1",
            "text-[10px] leading-4 text-muted-foreground hover:bg-accent",
          )}
          onClick={() => {
            if (line0 === undefined || !task) return;
            toggleOriginal(false); // 保持 markdown 源为未勾选
            onTaskMenu?.({ taskKey: task.taskKey, lineText: task.text, status: "todo" });
          }}
          onKeyDown={(e) => {
            if (e.key !== "Enter" && e.key !== " ") return;
            e.preventDefault();
            if (line0 === undefined || !task) return;
            toggleOriginal(false); // 保持 markdown 源为未勾选
            onTaskMenu?.({ taskKey: task.taskKey, lineText: task.text, status: "todo" });
          }}
        >
          ✕
        </span>
      ) : (
        <span
          role={disabled ? undefined : "checkbox"}
          tabIndex={disabled ? undefined : 0}
          aria-checked={status === "done"}
          aria-disabled={disabled}
          aria-label="切换待办状态"
          className={cn("mt-0.5 inline-flex shrink-0", !disabled && "cursor-pointer")}
          onClick={() => {
            if (disabled || line0 === undefined) return;
            toggleOriginal(status !== "done");
          }}
          onKeyDown={(e) => {
            if (disabled || line0 === undefined) return;
            if (e.key !== "Enter" && e.key !== " ") return;
            e.preventDefault();
            toggleOriginal(status !== "done");
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

      // 拖拽排序：仅"单行简单项"可拖；拖拽手柄发起，li 承接 dragover/drop
      const canDrag =
        !!onTaskDrop && !!task && orderInfo?.reorderableKeys.has(task.taskKey) === true;
      const hint = dropHint && task && dropHint.key === task.taskKey ? dropHint : null;

      const handleDragOver = (e: ReactDragEvent<HTMLLIElement>) => {
        if (!dragKey || !task || dragKey === task.taskKey) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        const rect = e.currentTarget.getBoundingClientRect();
        const before = e.clientY < rect.top + rect.height / 2;
        setDropHint((prev) =>
          prev?.key === task.taskKey && prev.before === before ? prev : { key: task.taskKey, before },
        );
      };

      const handleDrop = (e: ReactDragEvent<HTMLLIElement>) => {
        e.preventDefault();
        if (dragKey && task && dragKey !== task.taskKey) {
          // 直接按指针位置重算 before——不依赖 dropHint（dragleave 间隙可能已被清空）
          const rect = e.currentTarget.getBoundingClientRect();
          const before = e.clientY < rect.top + rect.height / 2;
          onTaskDrop?.({ dragKey, targetKey: task.taskKey, before });
        }
        setDragKey(null);
        setDropHint(null);
      };

      // 仅在真正离开 li（relatedTarget 不在内部）时清提示，避免跨子元素移动闪烁
      const handleDragLeave = (e: ReactDragEvent<HTMLLIElement>) => {
        if (!dragKey) return;
        const next = e.relatedTarget as Node | null;
        if (next && e.currentTarget.contains(next)) return;
        setDropHint(null);
      };

      return (
        <li
          className={cn(
            "group/task flex items-start gap-2",
            cls,
            hint && (hint.before ? "border-t-2 border-t-primary" : "border-b-2 border-b-primary"),
          )}
          {...rest}
          onContextMenu={handleContextMenu}
          onDragOver={canDrag || (dragKey && !!task) ? handleDragOver : undefined}
          onDrop={dragKey && !!task ? handleDrop : undefined}
          onDragLeave={dragKey ? handleDragLeave : undefined}
        >
          {canDrag && task && (
            <span
              draggable
              title="拖动排序"
              aria-label={`拖动排序：${task.text}`}
              className="mt-0.5 inline-flex shrink-0 cursor-grab text-muted-foreground/40 opacity-0 transition-opacity group-hover/task:opacity-100 active:cursor-grabbing hover:text-muted-foreground"
              onDragStart={(e) => {
                e.dataTransfer.effectAllowed = "move";
                e.dataTransfer.setData("text/plain", task.taskKey);
                setDragKey(task.taskKey);
              }}
              onDragEnd={() => {
                setDragKey(null);
                setDropHint(null);
              }}
            >
              <GripVertical className="size-3.5" />
            </span>
          )}
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
        {displayContent}
      </ReactMarkdown>
    </div>
  );
}
