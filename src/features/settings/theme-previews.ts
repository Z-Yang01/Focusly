/** 主题预览数据：设置页色卡展示用 */
import { Sun, Moon, Flame, Trees, Waves, MonitorCog, type LucideIcon } from "lucide-react";

export interface ThemePreview {
  value: string;
  label: string;
  icon: LucideIcon;
  /** Tailwind 类名组合：迷你色卡背景 */
  preview: string;
  /** 主色 CSS（用于圆点展示） */
  accent: string;
}

export const THEME_PREVIEWS: ThemePreview[] = [
  {
    value: "system",
    label: "跟随系统",
    icon: MonitorCog,
    preview: "bg-gradient-to-r from-white to-zinc-900",
    accent: "hsl(240 5% 10%)",
  },
  {
    value: "light",
    label: "浅色",
    icon: Sun,
    preview: "bg-gradient-to-r from-amber-50 to-orange-100",
    accent: "hsl(16 80% 52%)",
  },
  {
    value: "dark",
    label: "深色",
    icon: Moon,
    preview: "bg-gradient-to-r from-zinc-900 to-zinc-800",
    accent: "hsl(16 85% 60%)",
  },
  {
    value: "warm",
    label: "暖阳",
    icon: Flame,
    preview: "bg-gradient-to-r from-amber-100 to-orange-200",
    accent: "hsl(25 85% 48%)",
  },
  {
    value: "forest",
    label: "森林",
    icon: Trees,
    preview: "bg-gradient-to-r from-emerald-50 to-teal-100",
    accent: "hsl(155 60% 36%)",
  },
  {
    value: "ocean",
    label: "海洋",
    icon: Waves,
    preview: "bg-gradient-to-r from-sky-50 to-cyan-100",
    accent: "hsl(200 80% 38%)",
  },
];
