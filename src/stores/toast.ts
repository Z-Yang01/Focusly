import { create } from "zustand";

export type ToastKind = "success" | "error" | "info";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

interface ToastState {
  toasts: ToastItem[];
  push: (kind: ToastKind, message: string, ttl?: number) => void;
  dismiss: (id: number) => void;
}

const DEFAULT_TTL = 4000;
const MAX_TOASTS = 5;

let nextId = 1;
/** 组件卸载时仍可能有待计时器，dismiss 时统一清理 */
const timers = new Map<number, ReturnType<typeof setTimeout>>();

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  push: (kind, message, ttl = DEFAULT_TTL) => {
    const id = nextId++;
    set((s) => ({
      toasts: [...s.toasts, { id, kind, message }].slice(-MAX_TOASTS),
    }));
    if (ttl > 0) {
      const timer = setTimeout(() => get().dismiss(id), ttl);
      timers.set(id, timer);
    }
  },
  dismiss: (id) => {
    const timer = timers.get(id);
    if (timer !== undefined) {
      clearTimeout(timer);
      timers.delete(id);
    }
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
  },
}));

/** 非 React 上下文可调用的命令式入口（谁调用谁窗口显示） */
export const toast = {
  success: (message: string) => useToastStore.getState().push("success", message),
  error: (message: string) => useToastStore.getState().push("error", message),
  info: (message: string) => useToastStore.getState().push("info", message),
};
