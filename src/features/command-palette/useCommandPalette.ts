/** 命令面板命令注册表：CommandItem 类型 + 默认命令集 + 输入过滤 */
import { useMemo } from "react";
import {
  CalendarDays,
  Eye,
  EyeOff,
  Monitor,
  Moon,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Sun,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import { hideAllNotes, newNote, setSetting, showAllNotes, toggleAllNotes } from "@/lib/api";
import { SETTINGS_KEYS } from "@/types";

/** window CustomEvent：打开设置（ManagerWindow 后续接线） */
export const FOCUSLY_OPEN_SETTINGS = "focusly:open-settings";
/** window CustomEvent：导航到指定视图（today / trash / search） */
export const FOCUSLY_NAVIGATE = "focusly:navigate";

export interface NavigateDetail {
  view: "today" | "trash" | "search";
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

/** invoke 失败时不静默：与项目既有风格一致，console.error + alert */
function alerting(label: string, action: () => Promise<unknown>): () => void {
  return () => {
    action().catch((err) => {
      console.error(`${label}失败`, err);
      alert(`${label}失败：${err instanceof Error ? err.message : String(err)}`);
    });
  };
}

const DEFAULT_COMMANDS: CommandItem[] = [
  {
    id: "note.new",
    title: "新建便签",
    group: "便签",
    icon: Plus,
    keywords: "new create 新建 创建 便签 note",
    run: alerting("新建便签", newNote),
  },
  {
    id: "notes.show-all",
    title: "显示全部便签",
    group: "便签",
    icon: Eye,
    keywords: "show 显示 全部 可见 visible",
    run: alerting("显示全部便签", showAllNotes),
  },
  {
    id: "notes.hide-all",
    title: "隐藏全部便签",
    group: "便签",
    icon: EyeOff,
    keywords: "hide 隐藏 全部 隐身",
    run: alerting("隐藏全部便签", hideAllNotes),
  },
  {
    id: "notes.toggle-all",
    title: "切换全部便签可见性",
    group: "便签",
    icon: RefreshCw,
    keywords: "toggle 切换 可见性 显示 隐藏",
    run: alerting("切换便签可见性", toggleAllNotes),
  },
  {
    id: "nav.today",
    title: "打开今日视图",
    group: "导航",
    icon: CalendarDays,
    keywords: "today 今日 视图 待办 due",
    run: () => dispatchNavigate({ view: "today" }),
  },
  {
    id: "nav.trash",
    title: "打开回收站",
    group: "导航",
    icon: Trash2,
    keywords: "trash 回收站 已删除 废纸篓",
    run: () => dispatchNavigate({ view: "trash" }),
  },
  {
    id: "theme.light",
    title: "外观：浅色主题",
    group: "外观",
    icon: Sun,
    keywords: "theme light 浅色 亮色 白天",
    run: alerting("切换浅色主题", () => setSetting(SETTINGS_KEYS.theme, "light")),
  },
  {
    id: "theme.dark",
    title: "外观：深色主题",
    group: "外观",
    icon: Moon,
    keywords: "theme dark 深色 暗色 夜间",
    run: alerting("切换深色主题", () => setSetting(SETTINGS_KEYS.theme, "dark")),
  },
  {
    id: "theme.system",
    title: "外观：跟随系统主题",
    group: "外观",
    icon: Monitor,
    keywords: "theme system 跟随 系统 自动 auto",
    run: alerting("切换主题", () => setSetting(SETTINGS_KEYS.theme, "system")),
  },
  {
    id: "app.open-settings",
    title: "打开设置",
    group: "应用",
    icon: Settings,
    keywords: "settings 设置 偏好 preference options",
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
