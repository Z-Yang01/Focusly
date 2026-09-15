/** Markdown 渲染：GFM 支持、任务清单可勾选、链接外开、本地图片 asset 协议渲染 */
import { Children, isValidElement, useState, type ComponentPropsWithoutRef } from "react";
import ReactMarkdown, { type Components, type ExtraProps } from "react-markdown";
import remarkGfm from "remark-gfm";
import { ImageOff } from "lucide-react";
import { openExternal } from "@/lib/api";
import { convertFileSrc } from "@/lib/tauri";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";
import type { Element } from "hast";

export interface MarkdownViewProps {
  content: string;
  /** 行号为 0-based（与 todo.ts 一致；remark 是 1-based，内部已减 1） */
  onToggleTodo?: (line: number, checked: boolean) => void;
  className?: string;
}

type AnchorProps = ComponentPropsWithoutRef<"a"> & ExtraProps;
type ImageProps = ComponentPropsWithoutRef<"img"> & ExtraProps;
type LiProps = ComponentPropsWithoutRef<"li"> & ExtraProps;

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

export function MarkdownView({ content, onToggleTodo, className }: MarkdownViewProps) {
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
      const disabled = Boolean(inputProps?.disabled);
      // li 的 position 从 "- [ ]" 源行开始；remark 是 1-based，转 0-based
      const line = node.position?.start.line;

      const inner = Children.toArray(children).filter(
        (c) => !(isValidElement(c) && (c.props as { type?: unknown }).type === "checkbox"),
      );

      return (
        <li className={cn("flex items-start gap-2", cls)} {...rest}>
          <span
            role={disabled ? undefined : "checkbox"}
            aria-checked={checked}
            aria-disabled={disabled}
            aria-label="切换待办状态"
            className={cn(
              "mt-0.5 inline-flex shrink-0",
              !disabled && "cursor-pointer",
            )}
            onClick={() => {
              if (disabled || line === undefined) return;
              onToggleTodo?.(line - 1, !checked);
            }}
          >
            <Checkbox checked={checked} disabled={disabled} className="pointer-events-none" />
          </span>
          <div className="min-w-0 flex-1">{inner}</div>
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
