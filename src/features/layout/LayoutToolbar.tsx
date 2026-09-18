/** 布局工具条（供总控窗口 ManagerWindow 挂载）：
 *  网格排列（cols 下拉 2/3/4/自适应）、保存当前布局为预设、预设列表（点击应用、×删除）。
 *  预设即快照，不另存"上次布局"。后端 layout_* 命令未注册时降级为提示。 */
import { useCallback, useEffect, useState } from "react";
import { X, Save } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  layoutApplyPreset,
  layoutArrangeGrid,
  layoutDeletePreset,
  layoutListPresets,
  layoutSavePreset,
  type LayoutPreset,
} from "./api";

const COL_OPTIONS = [
  { value: 0, label: "自适应" },
  { value: 2, label: "2 列" },
  { value: 3, label: "3 列" },
  { value: 4, label: "4 列" },
] as const;

/** 后端命令尚未注册（等待接线）时的降级提示 */
function degradeMessage(err: unknown): string {
  const msg = err instanceof Error ? err.message : String(err);
  if (/not found|未注册|unknown command/i.test(msg)) {
    return "布局命令尚未接入（等待后端注册 layout_* 命令）";
  }
  return msg;
}

export function LayoutToolbar() {
  const [presets, setPresets] = useState<LayoutPreset[]>([]);
  const [name, setName] = useState("");
  const [cols, setCols] = useState<number>(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [backendReady, setBackendReady] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const list = await layoutListPresets();
      setPresets(list);
      setBackendReady(true);
      setError(null);
    } catch (err) {
      setPresets([]);
      setBackendReady(false);
      setError(degradeMessage(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (action: () => Promise<unknown>) => {
    if (busy) return;
    setBusy(true);
    try {
      await action();
      setError(null);
      setBackendReady(true);
      await refresh();
    } catch (err) {
      setError(degradeMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleArrange = () =>
    run(() => layoutArrangeGrid(cols === 0 ? undefined : cols));

  const handleSave = () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setError("请先输入预设名称");
      return;
    }
    void run(async () => {
      await layoutSavePreset(trimmed);
      setName("");
    });
  };

  return (
    <div className="flex flex-col gap-1.5 rounded-md border p-2 text-xs">
      {/* 第一行：网格排列（select 弹性收缩，窄侧栏不溢出） */}
      <div className="flex items-center gap-1.5">
        <select
          aria-label="网格列数"
          className="h-8 min-w-0 flex-1 rounded-md border border-input bg-transparent px-1.5 text-xs"
          value={cols}
          onChange={(e) => setCols(Number(e.target.value))}
        >
          {COL_OPTIONS.map((o) => (
            <option key={o.value} value={o.value}>
              {o.label}
            </option>
          ))}
        </select>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0 text-xs"
          disabled={busy || !backendReady}
          title="将全部便签网格平铺到主显示器"
          onClick={() => void handleArrange()}
        >
          网格排列
        </Button>
      </div>

      {/* 第二行：保存预设 */}
      <div className="flex items-center gap-1.5">
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="预设名"
          className="h-8 min-w-0 flex-1 text-xs"
          onKeyDown={(e) => {
            if (e.key === "Enter") void handleSave();
          }}
        />
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="h-8 shrink-0 text-xs"
          disabled={busy || !backendReady}
          title="把当前所有便签窗口的位置和大小保存为预设"
          onClick={() => void handleSave()}
        >
          <Save className="size-3.5" />
          保存布局
        </Button>
      </div>

      {/* 第二行：预设列表（点击应用、× 删除） */}
      {presets.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {presets.map((p) => (
            <span
              key={p.id}
              className="inline-flex items-center overflow-hidden rounded-md border"
            >
              <button
                type="button"
                className="px-2 py-1 hover:bg-accent disabled:opacity-50"
                disabled={busy}
                title="应用此布局"
                onClick={() => void run(() => layoutApplyPreset(p.id))}
              >
                {p.name}
              </button>
              <button
                type="button"
                aria-label={`删除 ${p.name}`}
                className="px-1 py-1 text-muted-foreground hover:bg-destructive/10 hover:text-destructive disabled:opacity-50"
                disabled={busy}
                onClick={() => void run(() => layoutDeletePreset(p.id))}
              >
                <X className="size-3" />
              </button>
            </span>
          ))}
        </div>
      )}

      {/* 降级 / 错误提示 */}
      {error && <p className="text-xs text-muted-foreground">{error}</p>}
      {!backendReady && !error && (
        <p className="text-xs text-muted-foreground">布局命令尚未接入（等待后端注册 layout_* 命令）</p>
      )}
      {!error && backendReady && presets.length === 0 && (
        <p className="text-xs text-muted-foreground">还没有布局预设：摆好窗口后点「保存布局」。</p>
      )}
    </div>
  );
}
