/** 应用设置对话框：外观 / 行为 / 快捷键 / 数据 */
import { useEffect, useState } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  exportData,
  getAppInfo,
  getAllSettings,
  getShortcuts,
  importData,
  resetShortcuts,
  setSetting,
  setShortcut,
  revealDataDir,
} from "@/lib/api";
import { onShortcutError } from "@/lib/tauri";
import { useTheme } from "@/app/theme";
import { ShortcutRecorder } from "@/features/shortcuts/ShortcutRecorder";
import { SETTINGS_KEYS, type AppInfo, type ShortcutAction, type ShortcutEntry } from "@/types";

export interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (v: boolean) => void;
}

/**
 * 本地扩展动作：quick_capture（速记箱）由 DB 迁移 v4 引入，
 * types/index.ts 的 ShortcutAction 尚未包含；此处本地联合扩展，后端已支持 set/get。
 */
type ShortcutActionExt = ShortcutAction | "quick_capture";

type ShortcutMap = Record<ShortcutActionExt, string>;

const SHORTCUT_ACTIONS: { action: ShortcutActionExt; label: string }[] = [
  { action: "toggle_notes", label: "显示 / 隐藏全部便签" },
  { action: "new_note", label: "新建便签" },
  { action: "focus_search", label: "聚焦搜索" },
  { action: "quick_capture", label: "速记箱（快速捕获）" },
];

const SHORTCUT_ACTION_NAMES: Record<string, string> = {
  toggle_notes: "显示 / 隐藏全部便签",
  new_note: "新建便签",
  focus_search: "聚焦搜索",
  quick_capture: "速记箱（快速捕获）",
};

function shortcutsToMap(list: ShortcutEntry[]): ShortcutMap {
  const map: ShortcutMap = {
    toggle_notes: "",
    new_note: "",
    focus_search: "",
    quick_capture: "",
  };
  for (const entry of list) {
    if (entry.action in map) map[entry.action] = entry.accelerator;
  }
  return map;
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme } = useTheme();
  const [loading, setLoading] = useState(false);
  const [settings, setSettings] = useState<Record<string, string>>({});
  const [shortcuts, setShortcuts] = useState<ShortcutMap>({
    toggle_notes: "",
    new_note: "",
    focus_search: "",
    quick_capture: "",
  });
  const [staged, setStaged] = useState<Partial<ShortcutMap>>({});
  const [info, setInfo] = useState<AppInfo | null>(null);

  // 快捷键注册失败（Rust 侧事件）→ 弹窗提示
  useEffect(() => {
    const off = onShortcutError((e) => {
      const name = SHORTCUT_ACTION_NAMES[e.action] ?? e.action;
      alert(`快捷键 "${name}" 注册失败：${e.message}`);
    });
    return () => {
      void off.then((f) => f());
    };
  }, []);

  // 每次打开时拉取最新设置
  useEffect(() => {
    if (!open) return;
    let alive = true;
    setLoading(true);
    Promise.all([getAllSettings(), getShortcuts(), getAppInfo()])
      .then(([s, list, appInfo]) => {
        if (!alive) return;
        setSettings(s);
        setShortcuts(shortcutsToMap(list));
        setStaged({});
        setInfo(appInfo);
      })
      .catch((err) => {
        console.error("加载设置失败", err);
        alert(`加载设置失败：${err instanceof Error ? err.message : String(err)}`);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [open]);

  const writeSetting = async (key: string, value: string) => {
    const prev = settings[key];
    setSettings((s) => ({ ...s, [key]: value }));
    try {
      await setSetting(key, value);
    } catch (err) {
      console.error("保存设置失败", err);
      alert(`保存设置失败：${err instanceof Error ? err.message : String(err)}`);
      setSettings((s) => ({ ...s, [key]: prev ?? "" }));
    }
  };

  const boolValue = (key: string) => settings[key] === "true";

  const currentShortcut = (action: ShortcutActionExt) => staged[action] ?? shortcuts[action];
  const hasStaged = Object.keys(staged).length > 0;

  const saveShortcuts = async () => {
    const entries = Object.entries(staged).filter(
      ([action, acc]) => acc !== undefined && acc !== shortcuts[action as ShortcutActionExt],
    ) as [ShortcutActionExt, string][];
    if (entries.length === 0) {
      setStaged({});
      return;
    }
    try {
      for (const [action, accelerator] of entries) {
        // lib/api.ts 的 setShortcut 形参仍是 ShortcutAction（types 禁改），此处断言透传
        await setShortcut(action as ShortcutAction, accelerator);
      }
      const list = await getShortcuts();
      setShortcuts(shortcutsToMap(list));
      setStaged({});
    } catch (err) {
      console.error("保存快捷键失败", err);
      alert(`保存快捷键失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleResetShortcuts = async () => {
    try {
      await resetShortcuts();
      const list = await getShortcuts();
      setShortcuts(shortcutsToMap(list));
      setStaged({});
    } catch (err) {
      console.error("恢复默认快捷键失败", err);
      alert(`恢复默认快捷键失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleExport = async () => {
    try {
      const path = await saveDialog({
        title: "导出数据",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      if (!path) return;
      await exportData(path);
      alert(`已导出到 ${path}`);
    } catch (err) {
      console.error("导出失败", err);
      alert(`导出失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleImport = async () => {
    try {
      const picked = await openDialog({
        title: "导入数据",
        filters: [{ name: "JSON", extensions: ["json"] }],
      });
      const path = Array.isArray(picked) ? picked[0] : picked;
      if (!path) return;
      const summary = await importData(path);
      alert(`导入完成：新增 ${summary.importedNotes} 条，跳过 ${summary.skipped} 条`);
    } catch (err) {
      console.error("导入失败", err);
      alert(`导入失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleReveal = async () => {
    try {
      await revealDataDir();
    } catch (err) {
      console.error("打开数据目录失败", err);
      alert(`打开数据目录失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  // 导出诊断包：环境/数据库健康/日志尾部（不含便签内容，不含私密数据）
  const handleExportDiagnostics = async () => {
    try {
      const path = await saveDialog({
        title: "导出诊断包",
        filters: [{ name: "诊断文本", extensions: ["txt"] }],
      });
      if (!path) return;
      await invoke("export_diagnostics", { path });
      alert(`诊断包已导出：${path}
（仅含环境信息、数据库健康摘要与日志尾部，不含便签内容）`);
    } catch (err) {
      console.error("导出诊断包失败", err);
      alert(`导出诊断包失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>设置</DialogTitle>
          <DialogDescription className="text-xs">
            {loading ? "正在加载设置…" : "更改即时生效"}
          </DialogDescription>
        </DialogHeader>

        <Tabs defaultValue="appearance" className="gap-3">
          <TabsList className="grid w-full grid-cols-4">
            <TabsTrigger value="appearance" className="text-xs">外观</TabsTrigger>
            <TabsTrigger value="behavior" className="text-xs">行为</TabsTrigger>
            <TabsTrigger value="shortcuts" className="text-xs">快捷键</TabsTrigger>
            <TabsTrigger value="data" className="text-xs">数据</TabsTrigger>
          </TabsList>

          {/* 外观 */}
          <TabsContent value="appearance" className="space-y-3">
            <div className="space-y-2">
              <Label className="text-xs text-muted-foreground">主题</Label>
              <RadioGroup
                value={theme}
                onValueChange={(v) => setTheme(v as "system" | "light" | "dark")}
                className="flex gap-4"
              >
                <div className="flex items-center gap-1.5">
                  <RadioGroupItem value="system" id="theme-system" />
                  <Label htmlFor="theme-system" className="text-sm font-normal">跟随系统</Label>
                </div>
                <div className="flex items-center gap-1.5">
                  <RadioGroupItem value="light" id="theme-light" />
                  <Label htmlFor="theme-light" className="text-sm font-normal">浅色</Label>
                </div>
                <div className="flex items-center gap-1.5">
                  <RadioGroupItem value="dark" id="theme-dark" />
                  <Label htmlFor="theme-dark" className="text-sm font-normal">深色</Label>
                </div>
              </RadioGroup>
            </div>
          </TabsContent>

          {/* 行为 */}
          <TabsContent value="behavior" className="space-y-3">
            <div className="flex items-center justify-between">
              <Label htmlFor="autostart" className="text-sm font-normal">开机自动启动</Label>
              <Switch
                id="autostart"
                disabled={loading}
                checked={boolValue(SETTINGS_KEYS.autostart)}
                onCheckedChange={(v) => void writeSetting(SETTINGS_KEYS.autostart, String(v))}
              />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="start-minimized" className="text-sm font-normal">
                启动后最小化到托盘
              </Label>
              <Switch
                id="start-minimized"
                disabled={loading}
                checked={boolValue(SETTINGS_KEYS.startMinimized)}
                onCheckedChange={(v) => void writeSetting(SETTINGS_KEYS.startMinimized, String(v))}
              />
            </div>
            <div className="flex items-center justify-between">
              <Label htmlFor="launch-show-notes" className="text-sm font-normal">
                启动后自动显示便签
              </Label>
              <Switch
                id="launch-show-notes"
                disabled={loading}
                checked={boolValue(SETTINGS_KEYS.launchShowNotes)}
                onCheckedChange={(v) =>
                  void writeSetting(SETTINGS_KEYS.launchShowNotes, String(v))
                }
              />
            </div>
            <div className="space-y-2 pt-1">
              <Label className="text-xs text-muted-foreground">关闭主窗口时</Label>
              <RadioGroup
                value={settings[SETTINGS_KEYS.closeAction] || "tray"}
                onValueChange={(v) => void writeSetting(SETTINGS_KEYS.closeAction, v)}
                className="flex gap-4"
              >
                <div className="flex items-center gap-1.5">
                  <RadioGroupItem value="tray" id="close-tray" />
                  <Label htmlFor="close-tray" className="text-sm font-normal">最小化到托盘</Label>
                </div>
                <div className="flex items-center gap-1.5">
                  <RadioGroupItem value="quit" id="close-quit" />
                  <Label htmlFor="close-quit" className="text-sm font-normal">退出程序</Label>
                </div>
              </RadioGroup>
            </div>
          </TabsContent>

          {/* 快捷键 */}
          <TabsContent value="shortcuts" className="space-y-3">
            <div className="space-y-2">
              {SHORTCUT_ACTIONS.map(({ action, label }) => (
                <div key={action} className="flex items-center justify-between gap-2">
                  <Label className="text-sm font-normal">{label}</Label>
                  <ShortcutRecorder
                    value={currentShortcut(action)}
                    onChange={(v) => setStaged((s) => ({ ...s, [action]: v }))}
                  />
                </div>
              ))}
            </div>
            <div className="flex items-center gap-2">
              <Button
                type="button"
                size="sm"
                className="h-7 text-xs"
                disabled={!hasStaged}
                onClick={() => void saveShortcuts()}
              >
                保存
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => void handleResetShortcuts()}
              >
                恢复默认
              </Button>
              {hasStaged && (
                <span className="text-xs text-muted-foreground">有未保存的修改</span>
              )}
            </div>
          </TabsContent>

          {/* 数据 */}
          <TabsContent value="data" className="space-y-3">
            <div className="flex gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 flex-1 text-xs"
                onClick={() => void handleExport()}
              >
                导出 JSON
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 flex-1 text-xs"
                onClick={() => void handleImport()}
              >
                导入 JSON
              </Button>
            </div>
            <div className="space-y-1">
              <Label className="text-xs text-muted-foreground">数据目录</Label>
              <code className="block break-all rounded bg-muted px-2 py-1 text-xs text-muted-foreground">
                {info?.dataDir ?? "加载中…"}
              </code>
            </div>
            <div className="flex items-center justify-between">
              <span className="text-xs text-muted-foreground">
                当前版本：{info?.version ?? "…"}
              </span>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-7 text-xs"
                onClick={() => void handleReveal()}
              >
                打开数据目录
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                onClick={() => void handleExportDiagnostics()}
              >
                导出诊断包
              </Button>
            </div>
          </TabsContent>
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
