/** 命令面板命令注册表：CommandItem 类型 + 默认命令集 + 输入过滤 */
import { useMemo } from "react";
import {
  CalendarClock,
  CalendarDays,
  Eye,
  EyeOff,
  FileText,
  Lock,
  Monitor,
  Moon,
  PenLine,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Sun,
  Trash2,
  Zap,
  type LucideIcon,
} from "lucide-react";
import {
  hideAllNotes,
  newNote,
  openNoteWindow,
  pomodoroReportToNote,
  quickCaptureToggle,
  setSetting,
  showAllNotes,
  toggleAllNotes,
} from "@/lib/api";
import { addDays, localDateKey, weekStartOf } from "@/features/daily-tasks/timeline";
import { SETTINGS_KEYS } from "@/types";
import { toast } from "@/stores/toast";

/** window CustomEvent：打开设置（ManagerWindow 后续接线） */
export const FOCUSLY_OPEN_SETTINGS = "focusly:open-settings";
/** window CustomEvent：打开番茄统计弹窗（ManagerWindow 监听 setStatsOpen） */
export const FOCUSLY_OPEN_STATS = "focusly:open-stats";
/** window CustomEvent：导航到指定视图（today / trash / search） */
export const FOCUSLY_NAVIGATE = "focusly:navigate";

export interface NavigateDetail {
  view: "today" | "trash" | "search" | "plan" | "all" | "archived" | "private";
  /** view=search 时携带搜索词 */
  query?: string;
}

export function dispatchNavigate(detail: NavigateDetail): void {
  window.dispatchEvent(new CustomEvent<NavigateDetail>(FOCUSLY_NAVIGATE, { detail }));
}

export interface CommandItem {
  id: string;
  title: string;
  group: string;
  icon?: LucideIcon;
  /** 空格分隔的匹配关键词（含中文/英文别名） */
  keywords: string;
  run: () => void;
}

/** invoke 失败时不静默：console.error + toast.error（非 hook 上下文可用的命令式入口） */
function withErrorToast(label: string, action: () => Promise<unknown>): () => void {
  return () => {
    action().catch((err) => {
      console.error(`${label}失败`, err);
      toast.error(`${label}失败：${err instanceof Error ? err.message : String(err)}`);
    });
  };
}

const DEFAULT_COMMANDS: CommandItem[] = [
  {
    id: "note.new",
    title: "新建便签",
    group: "便签",
    icon: Plus,
    keywords: "new create 新建 创建 便签 note xjbj xjb",
    run: withErrorToast("新建便签", newNote),
  },
  {
    id: "notes.show-all",
    title: "显示全部便签",
    group: "便签",
    icon: Eye,
    keywords: "show 显示 全部 可见 visible xsqb",
    run: withErrorToast("显示全部便签", showAllNotes),
  },
  {
    id: "notes.hide-all",
    title: "隐藏全部便签",
    group: "便签",
    icon: EyeOff,
    keywords: "hide 隐藏 全部 隐身 ycqb",
    run: withErrorToast("隐藏全部便签", hideAllNotes),
  },
  {
    id: "notes.toggle-all",
    title: "切换全部便签可见性",
    group: "便签",
    icon: RefreshCw,
    keywords: "toggle 切换 可见性 显示 隐藏 qhbj",
    run: withErrorToast("切换便签可见性", toggleAllNotes),
  },
  {
    id: "capture.quick",
    title: "呼出速记箱",
    group: "便签",
    icon: PenLine,
    keywords: "capture quick 速记 快速捕获 呼出 sjx kjbh",
    run: withErrorToast("呼出速记箱", quickCaptureToggle),
  },
  {
    id: "nav.plan",
    title: "打开今日计划",
    group: "导航",
    icon: CalendarClock,
    keywords: "plan today plan 今日计划 时间轴 任务 jrjh sjz",
    run: () => dispatchNavigate({ view: "plan" }),
  },
  {
    id: "nav.today",
    title: "打开今日视图",
    group: "导航",
    icon: CalendarDays,
    keywords: "today 今日 视图 待办 due jr st",
    run: () => dispatchNavigate({ view: "today" }),
  },
  {
    id: "nav.trash",
    title: "打开回收站",
    group: "导航",
    icon: Trash2,
    keywords: "trash 回收站 已删除 废纸篓 hsz",
    run: () => dispatchNavigate({ view: "trash" }),
  },
  {
    id: "nav.archived",
    title: "打开已归档",
    group: "导航",
    icon: FileText,
    keywords: "archive archived 已归档 归档 ygd",
    run: () => dispatchNavigate({ view: "archived" }),
  },
  {
    id: "nav.private",
    title: "打开私密空间",
    group: "导航",
    icon: Lock,
    keywords: "private secret 私密 空间 smkj sm",
    run: () => dispatchNavigate({ view: "private" }),
  },
  {
    id: "stats.pomodoro",
    title: "打开番茄统计",
    group: "工具",
    icon: Zap,
    keywords: "stats pomodoro 番茄 统计 复盘 fq tj",
    run: () => window.dispatchEvent(new CustomEvent(FOCUSLY_OPEN_STATS)),
  },
  {
    id: "report.week",
    title: "生成本周专注周报便签",
    group: "工具",
    icon: FileText,
    keywords: "report week 日报 周报 生成 专注 zzb zbb sc",
    run: withErrorToast("生成本周专注周报", () => {
      const start = weekStartOf(localDateKey(new Date()));
      return pomodoroReportToNote(start, addDays(start, 6), `专注周报 ${start.slice(5)} ~ ${addDays(start, 6).slice(5)}`).then((n) => openNoteWindow(n.id));
    }),
  },
  {
    id: "theme.light",
    title: "外观：浅色主题",
    group: "外观",
    icon: Sun,
    keywords: "theme light 浅色 亮色 白色 qs ys",
    run: withErrorToast("切换浅色主题", () => setSetting(SETTINGS_KEYS.theme, "light")),
  },
  {
    id: "theme.dark",
    title: "外观：深色主题",
    group: "外观",
    icon: Moon,
    keywords: "theme dark 深色 暗色 夜间 ss",
    run: withErrorToast("切换深色主题", () => setSetting(SETTINGS_KEYS.theme, "dark")),
  },
  {
    id: "theme.system",
    title: "外观：跟随系统主题",
    group: "外观",
    icon: Monitor,
    keywords: "theme system 跟随 系统 自动 auto gsxt",
    run: withErrorToast("切换主题", () => setSetting(SETTINGS_KEYS.theme, "system")),
  },
  {
    id: "app.open-settings",
    title: "打开设置",
    group: "应用",
    icon: Settings,
    keywords: "settings 设置 偏好 preference options sz szx",
    run: () => window.dispatchEvent(new CustomEvent(FOCUSLY_OPEN_SETTINGS)),
  },
];

/** 首项动态命令："搜索: <当前输入>" —— 执行搜索跳转（输入为空时不出现） */
function searchCommand(input: string): CommandItem {
  const query = input.trim();
  return {
    id: "search.current",
    title: `搜索: ${query}`,
    group: "搜索",
    icon: Search,
    keywords: `search 搜索 查找 ${query}`,
    run: () => dispatchNavigate({ view: "search", query }),
  };
}

/** 过滤：输入按空白拆词，每个词都需命中 title/keywords/group（大小写不敏感） */
export function filterCommands(input: string): CommandItem[] {
  const query = input.trim();
  const items = query ? [searchCommand(input), ...DEFAULT_COMMANDS] : [...DEFAULT_COMMANDS];
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (terms.length === 0) return items;
  return items.filter((item) => {
    const hay = `${item.title} ${item.keywords} ${item.group}`.toLowerCase();
    return terms.every((t) => hay.includes(t));
  });
}

/** 命令面板数据源：返回按输入过滤后的命令（搜索命令始终居首） */
export function useCommandPalette(input: string): CommandItem[] {
  return useMemo(() => filterCommands(input), [input]);
}
