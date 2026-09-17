/** 快速待办输入：输入文本 → Enter 自动创建或追加待办 */
import { useState, type KeyboardEvent } from "react";
import { Plus } from "lucide-react";

interface QuickTodoInputProps {
  onSubmit: (text: string) => Promise<void>;
  placeholder?: string;
}

export function QuickTodoInput({ onSubmit, placeholder = "输入待办事项，Enter 快速创建…" }: QuickTodoInputProps) {
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);

  const handleSubmit = async () => {
    const trimmed = text.trim();
    if (!trimmed || saving) return;
    setSaving(true);
    try {
      await onSubmit(trimmed);
      setText("");
    } catch (err) {
      console.error("创建待办失败", err);
    } finally {
      setSaving(false);
    }
  };

  const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") void handleSubmit();
  };

  return (
    <div className="flex items-center gap-1.5 rounded-lg border border-primary/20 bg-primary/[0.04] px-2.5 py-1.5">
      <Plus className="size-3.5 shrink-0 text-primary" />
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={handleKey}
        placeholder={placeholder}
        className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/60"
        aria-label="快速创建待办"
      />
      {text.trim() && (
        <button
          type="button"
          onClick={() => void handleSubmit()}
          disabled={saving}
          className="shrink-0 rounded px-1.5 py-0.5 text-xs text-primary hover:bg-primary/10"
          aria-label="创建待办"
        >
          {saving ? "…" : "创建"}
        </button>
      )}
    </div>
  );
}
