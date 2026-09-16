/** 快速捕获窗口主体（label="quick-capture"）：速记入「收件箱」+ 剪贴板历史侧栏。
 *  窗口由 Rust 以 visible(false) 创建，本组件挂载后调 quickcapture_ready 再显示（防白闪）。
 *  剪贴板捕获只读本机剪贴板并写入本地 SQLite，无任何网络行为。 */
import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { Check, ClipboardList, PenLine, Pin, PinOff, Trash2, X, type LucideIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { duration, easing, spacing } from "@/design-tokens";
import { describeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import {
  clipboardAdd,
  clipboardClear,
  clipboardList,
  clipboardPin,
  clipboardRemove,
  createNoteRecord,
  quickCaptureHide,
  quickCaptureReady,
  setNoteTags,
  updateNoteContent,
  type ClipboardEntry,
} from "./api";

/** 速记统一打上收件箱标签 */
const INBOX_TAG = "收件箱";
const HISTORY_LIMIT = 20;
const PREVIEW_MAX = 60;

type Tab = "write" | "history";

/** 历史条目预览：压平空白并截断到前 60 字 */
function previewOf(text: string, max = PREVIEW_MAX): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > max ? `${flat.slice(0, max)}…` : flat;
}

const TABS: { key: Tab; label: string; icon: LucideIcon }[] = [
  { key: "write", label: "速记", icon: PenLine },
  { key: "history", label: "剪贴板", icon: ClipboardList },
];

/** 页签切换器：滑动指示条（transform 过渡）标记当前页 */
function TabSwitcher({ tab, onChange }: { tab: Tab; onChange: (t: Tab) => void }) {
  const idx = Math.max(
    0,
    TABS.findIndex((t) => t.key === tab),
  );
  return (
    <div
      role="tablist"
      aria-label="快速捕获页签"
      className="relative flex items-center rounded-md bg-muted/60 p-0.5"
    >
      <span
        aria-hidden
        className="absolute inset-y-0.5 left-0.5 rounded bg-background shadow-sm transition-transform"
        style={{
          width: `calc((100% - ${spacing.xs}px) / ${TABS.length})`,
          transform: `translateX(${idx * 100}%)`,
          transitionDuration: `${duration.normal}ms`,
          transitionTimingFunction: easing.standard,
        }}
      />
      {TABS.map((t) => (
        <button
          key={t.key}
          type="button"
          role="tab"
          aria-selected={tab === t.key}
          aria-label={t.label}
          onClick={() => onChange(t.key)}
          className={cn(
            "relative z-10 inline-flex h-6 flex-1 items-center justify-center gap-1 rounded-md px-2 text-xs font-medium transition-colors",
            tab === t.key ? "text-foreground" : "text-muted-foreground hover:text-foreground",
          )}
        >
          <t.icon className="size-3.5" />
          {t.label}
        </button>
      ))}
    </div>
  );
}

function EmptyHint({ text }: { text: string }) {
  return (
    <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
      {text}
    </div>
  );
}

export function QuickCaptureWindow() {
  const [tab, setTab] = useState<Tab>("write");
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  /** clipboard_* 命令是否可用（后端未接线时历史区优雅降级） */
  const [backendOk, setBackendOk] = useState(true);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState(false);
  /** 保存成功后的短暂"已保存"反馈（duration.normal 后隐藏窗口） */
  const [savedFlash, setSavedFlash] = useState(false);

  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const readyRef = useRef(false);
  /** 上次入库的剪贴板文本：相同内容不重复入库 */
  const lastClipRef = useRef<string | null>(null);

  const refreshEntries = useCallback(async () => {
    try {
      setEntries(await clipboardList(HISTORY_LIMIT));
      setBackendOk(true);
    } catch (err) {
      console.warn("clipboard_list 不可用（命令未注册？）", err);
      setBackendOk(false);
    }
  }, []);

  /** 剪贴板写入捕获：窗口聚焦时读当前剪贴板文本，与上次不同才入库（失败静默） */
  const captureClipboard = useCallback(async () => {
    try {
      const text = await readText();
      if (!text || text === lastClipRef.current) return;
      lastClipRef.current = text;
      await clipboardAdd(text, "text");
      void refreshEntries();
    } catch {
      // 剪贴板无文本（如图片）或命令未注册：静默
    }
  }, [refreshEntries]);

  const hideWindow = useCallback(async () => {
    try {
      await quickCaptureHide();
    } catch {
      // quickcapture_hide 未注册时兜底：直接隐藏窗口（草稿保留，下次显示可继续）
      try {
        await getCurrentWebviewWindow().hide();
      } catch (err) {
        console.error("隐藏快速捕获窗口失败", err);
      }
    }
  }, []);

  // 初始：渲染就绪后通知 Rust 显示窗口（命令未注册则直接显示兜底），并做首次捕获与拉历史
  useEffect(() => {
    if (!readyRef.current) {
      readyRef.current = true;
      quickCaptureReady().catch(async (err) => {
        console.warn("quickcapture_ready 不可用，降级为直接显示", err);
        const win = getCurrentWebviewWindow();
        try {
          await win.show();
          await win.setFocus();
        } catch (showErr) {
          console.error("显示快速捕获窗口失败", showErr);
        }
      });
    }
    void captureClipboard();
    void refreshEntries();
  }, [captureClipboard, refreshEntries]);

  // 窗口聚焦：捕获剪贴板 + 把焦点还给正文输入框 + 复位保存反馈
  useEffect(() => {
    const un = getCurrentWebviewWindow().onFocusChanged(({ payload: focused }) => {
      if (!focused) return;
      setSavedFlash(false);
      void captureClipboard();
      if (tab === "write") textareaRef.current?.focus();
    });
    return () => {
      void un.then((f) => f());
    };
  }, [captureClipboard, tab]);

  // 切回速记页（含首次挂载）时聚焦正文
  useEffect(() => {
    if (tab === "write") textareaRef.current?.focus();
  }, [tab]);

  /** 保存 = 建行 → 写标题/正文 → 打「收件箱」标签 → "已保存"闪现 → 隐藏。空内容不保存直接关闭。 */
  const save = useCallback(async () => {
    if (saving) return;
    if (!content.trim()) {
      await hideWindow();
      return;
    }
    setSaving(true);
    setSaveError(false);
    try {
      const note = await createNoteRecord();
      await updateNoteContent(note.id, title.trim(), content);
      await setNoteTags(note.id, [INBOX_TAG]);
      setTitle("");
      setContent("");
      setSaving(false);
      // 短暂成功反馈后再隐藏（savedFlash 状态驱动底部"✓ 已保存"）
      setSavedFlash(true);
      await new Promise((resolve) => setTimeout(resolve, duration.normal));
      await hideWindow();
    } catch (err) {
      console.error("速记保存失败", err);
      setSaveError(true);
    } finally {
      setSaving(false);
      setSavedFlash(false);
    }
  }, [content, title, saving, hideWindow]);

  // 全局键：Esc 关闭（草稿保留）；Ctrl+Enter 保存并关闭；Ctrl+Tab 切换页签
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        void hideWindow();
      } else if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        void save();
      } else if (e.key === "Tab" && e.ctrlKey) {
        e.preventDefault();
        setTab((t) => (t === "write" ? "history" : "write"));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hideWindow, save]);

  /** 点击历史条目 = 追加到速记正文并切回速记页 */
  const appendToDraft = (entry: ClipboardEntry) => {
    setContent((c) => (c.trim() ? `${c.replace(/\n+$/, "")}\n${entry.content}` : entry.content));
    setSaveError(false);
    setTab("write");
  };

  const togglePin = async (id: string) => {
    try {
      const updated = await clipboardPin(id);
      setEntries((list) => list.map((e) => (e.id === id ? updated : e)));
    } catch (err) {
      console.warn("clipboard_pin 失败", err);
    }
  };

  const removeEntry = async (id: string) => {
    try {
      await clipboardRemove(id);
      setEntries((list) => list.filter((e) => e.id !== id));
    } catch (err) {
      console.warn("clipboard_remove 失败", err);
    }
  };

  const clearAll = async () => {
    try {
      await clipboardClear();
      setEntries([]);
    } catch (err) {
      console.warn("clipboard_clear 失败", err);
    }
  };

  return (
    // 与便签窗口一致的容器：rounded-xl + border + bg-background + shadow-xl
    <div className="flex h-screen flex-col overflow-hidden rounded-xl border bg-background text-sm shadow-xl">
      {/* 头部：页签切换（滑动指示条）+ 关闭 */}
      <div data-tauri-drag-region className="flex h-9 shrink-0 items-center gap-1.5 border-b px-2">
        <TabSwitcher tab={tab} onChange={setTab} />
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          className="size-6 text-muted-foreground hover:text-foreground"
          title="关闭（Esc）"
          aria-label="关闭速记箱"
          onClick={() => void hideWindow()}
        >
          <X className="size-4" />
        </Button>
      </div>

      {tab === "write" ? (
        <>
          <Input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="标题（可选）"
            className="h-9 shrink-0 rounded-none border-x-0 border-t-0 px-3 text-sm shadow-none focus-visible:ring-0"
          />
          <Textarea
            ref={textareaRef}
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              setSaveError(false);
            }}
            autoFocus
            placeholder="记点什么…（保存后进入「收件箱」标签）"
            className="min-h-[80px] flex-1 resize-none rounded-none border-0 px-3 py-2 shadow-none focus-visible:ring-0"
          />
        </>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col px-2 py-1.5">
          <div className="flex shrink-0 items-center justify-between pb-1 text-xs text-muted-foreground">
            <span>点击条目填入速记</span>
            <Button
              variant="ghost"
              size="sm"
              className="h-5 px-1.5 text-xs text-muted-foreground hover:text-foreground"
              disabled={entries.length === 0}
              aria-label="清空剪贴板历史"
              onClick={() => void clearAll()}
            >
              清空
            </Button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {!backendOk ? (
              <EmptyHint text="后端未就绪（剪贴板命令未注册）" />
            ) : entries.length === 0 ? (
              <EmptyHint text="暂无剪贴板历史" />
            ) : (
              <ul className="space-y-0.5">
                {entries.map((entry) => (
                  <li
                    key={entry.id}
                    className="group flex items-center gap-0.5 rounded-md px-1 py-0.5 hover:bg-accent"
                  >
                    <button
                      type="button"
                      className="min-w-0 flex-1 rounded text-left outline-none"
                      title="点击填入速记"
                      aria-label="将此记录填入速记"
                      onClick={() => appendToDraft(entry)}
                    >
                      <span className="block truncate">
                        {entry.pinned && (
                          <Pin className="mr-1 inline size-3 align-[-1px] text-amber-500" />
                        )}
                        {previewOf(entry.content)}
                      </span>
                      <span className="block text-xs text-muted-foreground">
                        {describeTime(entry.createdAt)}
                      </span>
                    </button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-6 shrink-0 text-muted-foreground hover:text-foreground"
                      title={entry.pinned ? "取消固定" : "固定"}
                      aria-label={entry.pinned ? "取消固定该记录" : "固定该记录"}
                      onClick={() => void togglePin(entry.id)}
                    >
                      {entry.pinned ? <PinOff className="size-3.5" /> : <Pin className="size-3.5" />}
                    </Button>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="size-6 shrink-0 text-muted-foreground hover:text-destructive"
                      title="删除"
                      aria-label="删除该剪贴板记录"
                      onClick={() => void removeEntry(entry.id)}
                    >
                      <Trash2 className="size-3.5" />
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      )}

      {/* 底部提示条 */}
      <div className="flex h-7 shrink-0 items-center justify-between border-t px-2.5 text-xs text-muted-foreground">
        {saving ? (
          <span>保存中…</span>
        ) : saveError ? (
          <span className="text-destructive">保存失败，请重试</span>
        ) : savedFlash ? (
          <span className="flex animate-fade-in items-center gap-1 text-emerald-600 dark:text-emerald-400">
            <Check className="size-3.5" />
            已保存
          </span>
        ) : (
          <span />
        )}
        <span>Ctrl+Enter 保存并关闭 · Esc 关闭 · Ctrl+Tab 切换</span>
      </div>
    </div>
  );
}
