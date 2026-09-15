import { create } from "zustand";
import type { ThemeMode } from "@/types";

interface UiState {
  /** 全部便签当前是否可见（来自 Rust 的 notes-visibility 事件） */
  notesVisible: boolean;
  theme: ThemeMode;
  setNotesVisible: (v: boolean) => void;
  setTheme: (t: ThemeMode) => void;
}

export const useUiStore = create<UiState>((set) => ({
  notesVisible: true,
  theme: "system",
  setNotesVisible: (v) => set({ notesVisible: v }),
  setTheme: (t) => set({ theme: t }),
}));
