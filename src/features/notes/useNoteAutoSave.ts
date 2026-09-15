/** 便签自动保存：500ms 防抖、blur/卸载 flush、失败可见 */
import { useCallback, useEffect, useRef, useState } from "react";
import { updateNoteContent } from "@/lib/api";

export interface UseNoteAutoSaveOptions {
  noteId: string;
  title: string;
  content: string;
}

export interface NoteAutoSaveState {
  /** 当前内容与最近一次保存一致 */
  saved: boolean;
  saving: boolean;
  /** 保存失败原因（不静默） */
  error: string | null;
  lastSavedAt: Date | null;
  saveNow: () => Promise<void>;
}

export function useNoteAutoSave({
  noteId,
  title,
  content,
}: UseNoteAutoSaveOptions): NoteAutoSaveState {
  const [saved, setSaved] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null);

  // 最新值（渲染期同步），保存时取快照
  const latestRef = useRef({ title, content });
  latestRef.current = { title, content };
  // 基线：最近一次成功保存（或初始加载）的内容
  const baselineRef = useRef({ title, content });
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const inFlightRef = useRef(false);
  const rerunRef = useRef(false);

  const saveNowRef = useRef<() => Promise<void>>(async () => {});

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const isDirty = useCallback(
    () =>
      latestRef.current.title !== baselineRef.current.title ||
      latestRef.current.content !== baselineRef.current.content,
    [],
  );

  const saveNow = useCallback(async () => {
    clearTimer();
    if (inFlightRef.current) {
      // 上一次保存还在路上：结束后再补一次
      rerunRef.current = true;
      return;
    }
    const snapshot = { ...latestRef.current };
    if (
      snapshot.title === baselineRef.current.title &&
      snapshot.content === baselineRef.current.content
    ) {
      setSaved(true);
      return;
    }
    inFlightRef.current = true;
    setSaving(true);
    setError(null);
    try {
      await updateNoteContent(noteId, snapshot.title, snapshot.content);
      baselineRef.current = snapshot;
      setSaved(true);
      setLastSavedAt(new Date());
    } catch (err) {
      console.error("保存便签失败", err);
      setError(err instanceof Error ? err.message : String(err));
      setSaved(false);
    } finally {
      inFlightRef.current = false;
      setSaving(false);
      if (rerunRef.current) {
        rerunRef.current = false;
        if (isDirty()) {
          timerRef.current = setTimeout(() => void saveNowRef.current(), 200);
        }
      }
    }
  }, [noteId, clearTimer, isDirty]);

  saveNowRef.current = saveNow;

  // 防抖自动保存
  useEffect(() => {
    if (title === baselineRef.current.title && content === baselineRef.current.content) return;
    setSaved(false);
    clearTimer();
    timerRef.current = setTimeout(() => void saveNow(), 500);
    return clearTimer;
  }, [title, content, saveNow, clearTimer]);

  // 窗口失焦时 flush
  useEffect(() => {
    const onBlur = () => {
      if (isDirty()) void saveNowRef.current();
    };
    window.addEventListener("blur", onBlur);
    return () => window.removeEventListener("blur", onBlur);
  }, [isDirty]);

  // 卸载时若有未保存修改，直接保存
  useEffect(
    () => () => {
      if (
        latestRef.current.title !== baselineRef.current.title ||
        latestRef.current.content !== baselineRef.current.content
      ) {
        void saveNowRef.current();
      }
    },
    [],
  );

  return { saved, saving, error, lastSavedAt, saveNow };
}
