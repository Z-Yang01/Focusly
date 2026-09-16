/** 搜索语法补全：trailingContext / buildCompletion 状态机（纯函数，可单测）
 *  + SyntaxHints 补全面板（tag:/is:/has:/due: 合法值提示）。 */
import {
  AlarmClock,
  Archive,
  CalendarDays,
  Image as ImageIcon,
  Lock,
  Pin,
  SquareCheck,
  Tag,
  type LucideIcon,
} from "lucide-react";
import {
  SEARCH_DUE_VALUES,
  SEARCH_HAS_VALUES,
  SEARCH_IS_VALUES,
  tokenizeQuery,
  type SearchDueFilter,
  type SearchHasFilter,
  type SearchIsFilter,
} from "@/lib/query-parser";
import { cn } from "@/lib/utils";

const IS_LABELS: Record<SearchIsFilter, string> = {
  todo: "含待办",
  archived: "已归档",
  private: "私密便签",
  pinned: "已置顶",
};
const HAS_LABELS: Record<SearchHasFilter, string> = { image: "含图片", reminder: "含提醒" };
const DUE_LABELS: Record<SearchDueFilter, string> = { today: "今天到期", week: "7 天内到期" };

const IS_ICONS: Record<SearchIsFilter, LucideIcon> = {
  todo: SquareCheck,
  archived: Archive,
  private: Lock,
  pinned: Pin,
};
const HAS_ICONS: Record<SearchHasFilter, LucideIcon> = { image: ImageIcon, reminder: AlarmClock };

export interface CompletionOption {
  /** 接受后写入输入框的完整 token */
  token: string;
  label: string;
  hint: string;
  icon: LucideIcon;
}

export interface CompletionState {
  key: "tag" | "is" | "has" | "due";
  /** 输入中的最后一个 token 是否已是完整合法值（此时 Enter 直接执行搜索而非补全） */
  complete: boolean;
  options: CompletionOption[];
}

/** 输入框末尾上下文：prefix 为可替换前缀，token 为末尾（未完形的）token */
export function trailingContext(q: string): { prefix: string; token: string } {
  if (/\s$/.test(q)) return { prefix: q, token: "" };
  const tokens = tokenizeQuery(q);
  const token = tokens.length > 0 ? tokens[tokens.length - 1] : "";
  return { prefix: q.slice(0, q.length - token.length), token };
}

export function buildCompletion(q: string, knownTags: string[]): CompletionState | null {
  const { token } = trailingContext(q);
  const match = /^(tag|is|has|due):(.*)$/i.exec(token);
  if (!match) return null;
  const key = match[1].toLowerCase() as CompletionState["key"];
  const typed = match[2].replace(/"/g, "");

  if (key === "tag") {
    const t = typed.replace(/^#+/, "").toLowerCase();
    const options: CompletionOption[] = knownTags
      .filter((name) => name.toLowerCase().startsWith(t))
      .slice(0, 8)
      .map((name) => ({
        token: `tag:${/\s/.test(name) ? `"#${name}"` : `#${name}`}`,
        label: `#${name}`,
        hint: "按标签过滤（多个 tag 为 AND）",
        icon: Tag,
      }));
    return {
      key,
      complete: t.length > 0 && knownTags.some((n) => n.toLowerCase() === t),
      options,
    };
  }

  if (key === "is") {
    const t = typed.toLowerCase();
    return {
      key,
      complete: (SEARCH_IS_VALUES as readonly string[]).includes(t),
      options: SEARCH_IS_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
        token: `is:${v}`,
        label: `is:${v}`,
        hint: IS_LABELS[v],
        icon: IS_ICONS[v],
      })),
    };
  }

  if (key === "has") {
    const t = typed.toLowerCase();
    return {
      key,
      complete: (SEARCH_HAS_VALUES as readonly string[]).includes(t),
      options: SEARCH_HAS_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
        token: `has:${v}`,
        label: `has:${v}`,
        hint: HAS_LABELS[v],
        icon: HAS_ICONS[v],
      })),
    };
  }

  const t = typed.toLowerCase();
  return {
    key,
    complete: (SEARCH_DUE_VALUES as readonly string[]).includes(t),
    options: SEARCH_DUE_VALUES.filter((v) => v.startsWith(t)).map((v) => ({
      token: `due:${v}`,
      label: `due:${v}`,
      hint: DUE_LABELS[v],
      icon: CalendarDays,
    })),
  };
}

export interface SyntaxHintsProps {
  completion: CompletionState;
  /** 当前高亮项（键盘 / hover 同步） */
  activeIndex: number;
  onHover: (index: number) => void;
  onAccept: (option: CompletionOption) => void;
}

/** 语法补全面板：候选项 token 用等宽字体，与正文输入区分 */
export function SyntaxHints({ completion, activeIndex, onHover, onAccept }: SyntaxHintsProps) {
  return (
    <div className="max-h-80 overflow-y-auto p-1">
      <div className="px-2 pb-1 pt-1.5 text-[11px] text-muted-foreground">
        {completion.key}: 合法值
      </div>
      {completion.options.length === 0 ? (
        <div className="px-3 py-4 text-center text-xs text-muted-foreground">暂无可补全项</div>
      ) : (
        completion.options.map((opt, i) => (
          <button
            key={opt.token}
            type="button"
            className={cn(
              "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
              i === activeIndex && "bg-accent",
            )}
            onMouseDown={(e) => e.preventDefault()}
            onMouseEnter={() => onHover(i)}
            onClick={() => onAccept(opt)}
          >
            <opt.icon className="size-3.5 shrink-0 text-muted-foreground" />
            <span className="shrink-0 font-mono text-xs font-medium">{opt.label}</span>
            <span className="ml-auto truncate text-[11px] text-muted-foreground">{opt.hint}</span>
          </button>
        ))
      )}
    </div>
  );
}
