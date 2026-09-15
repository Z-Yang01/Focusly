/** 便签卡片（管理器网格） */
import { Archive, ArchiveRestore, EllipsisVertical, Pin, SquareCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { archiveNote, restoreNote, setNoteFlag, trashNote } from "@/lib/api";
import { describeTime, snippetOf } from "@/lib/format";
import type { NoteSummary } from "@/types";

export interface NoteCardProps {
  note: NoteSummary;
  onOpen: () => void;
}

export function NoteCard({ note, onOpen }: NoteCardProps) {
  const handleTogglePin = async () => {
    try {
      await setNoteFlag(note.id, "pinned", !note.isPinned);
    } catch (err) {
      console.error("设置置顶失败", err);
      alert(`操作失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleToggleArchive = async () => {
    try {
      if (note.status === "archived") {
        await restoreNote(note.id);
      } else {
        await archiveNote(note.id);
      }
    } catch (err) {
      console.error("归档/恢复失败", err);
      alert(`操作失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  const handleDelete = async () => {
    // 契约审计修复：删除改为移入回收站（trash_note），永久删除走回收站视图的 purge
    if (!confirm(`确定将「${note.title || "无标题"}」移入回收站？可在回收站中恢复。`)) return;
    try {
      await trashNote(note.id);
    } catch (err) {
      console.error("删除失败", err);
      alert(`删除失败：${err instanceof Error ? err.message : String(err)}`);
    }
  };

  return (
    <div
      className="group flex h-36 cursor-pointer flex-col rounded-lg border bg-card p-3 text-sm transition-shadow hover:shadow-md"
      onClick={onOpen}
    >
      <div className="flex items-start gap-1">
        {note.isPinned && <Pin className="mt-0.5 size-3.5 shrink-0 text-amber-500" />}
        <h3 className="min-w-0 flex-1 truncate font-medium" title={note.title || "无标题"}>
          {note.title || "无标题"}
        </h3>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              className="size-6 shrink-0 text-muted-foreground opacity-0 hover:text-foreground focus-visible:opacity-100 group-hover:opacity-100"
              onClick={(e) => e.stopPropagation()}
            >
              <EllipsisVertical className="size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="text-xs" onClick={(e) => e.stopPropagation()}>
            <DropdownMenuItem onSelect={onOpen}>打开</DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void handleTogglePin()}>
              {note.isPinned ? "取消置顶" : "置顶"}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void handleToggleArchive()}>
              {note.status === "archived" ? (
                <span className="flex items-center gap-1.5">
                  <ArchiveRestore className="size-3.5" />
                  恢复
                </span>
              ) : (
                <span className="flex items-center gap-1.5">
                  <Archive className="size-3.5" />
                  归档
                </span>
              )}
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              className="text-destructive focus:text-destructive"
              onSelect={() => void handleDelete()}
            >
              永久删除
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <p className="mt-1 line-clamp-2 min-h-0 flex-1 overflow-hidden text-xs text-muted-foreground">
        {snippetOf(note.content, 60) || "（空便签）"}
      </p>

      <div className="mt-2 flex items-center gap-1 overflow-hidden">
        {note.todoTotal > 0 && (
          <span className="flex shrink-0 items-center gap-1 text-xs text-muted-foreground">
            <SquareCheck className="size-3.5" />
            {note.todoDone}/{note.todoTotal}
          </span>
        )}
        {note.tags.slice(0, 3).map((tag) => (
          <Badge key={tag} variant="secondary" className="shrink-0 px-1.5 py-0 text-[10px]">
            {tag}
          </Badge>
        ))}
        {note.status === "archived" && (
          <Badge variant="outline" className="shrink-0 px-1.5 py-0 text-[10px]">
            已归档
          </Badge>
        )}
        <span className="ml-auto shrink-0 text-[10px] text-muted-foreground">
          {describeTime(note.updatedAt)}
        </span>
      </div>
    </div>
  );
}
