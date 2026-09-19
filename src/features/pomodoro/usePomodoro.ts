/** 番茄钟全局状态：模块级 zustand store + 单例运行时（事件订阅 / 初始拉取 / 1s 推算）。
 *  多组件、多窗口（同一 webview 内）共享同一份状态；跨窗口靠 pomodoro-state 事件同步。
 *  剩余秒数的权威是 endsAt（本地 setInterval 每秒推算）；暂停时用 remainingSec 静态值。 */
import { useCallback, useEffect, useMemo } from "react";
import { toast } from "@/stores/toast";
import { create } from "zustand";
import {
  onPomodoroFinished,
  onPomodoroState,
  pomodoroAddMinutes,
  pomodoroCompleteTask,
  pomodoroPause,
  pomodoroResume,
  pomodoroSkip,
  pomodoroStart,
  pomodoroState,
  pomodoroStop,
} from "./api";
import { fmtMmss } from "./format";
import type { StatePayload } from "./types";
import { playPomodoroDone, playBreakDone } from "@/lib/sound";

/** 由 endsAt / remainingSec 推算剩余秒数 */
export function remainingOf(s: StatePayload): number {
  if (!s.paused && s.endsAt) {
    const end = Date.parse(s.endsAt);
    if (!Number.isNaN(end)) return Math.max(0, Math.round((end - Date.now()) / 1000));
  }
  return Math.max(0, Math.floor(s.remainingSec));
}

interface PomodoroStore {
  state: StatePayload | null;
  /** 每秒推算的剩余秒数（暂停/空闲时等于 state.remainingSec） */
  remaining: number;
  /** 后端命令可用性：任一命令失败即降级为 false（UI 显示"后端未就绪"） */
  available: boolean;
  apply: (s: StatePayload | null) => void;
  setAvailable: (v: boolean) => void;
}

export const usePomodoroStore = create<PomodoroStore>((set) => ({
  state: null,
  remaining: 0,
  available: true,
  apply: (s) => set({ state: s, remaining: s ? remainingOf(s) : 0 }),
  setAvailable: (v) => set({ available: v }),
}));

// ---------- 单例运行时（按订阅者计数启停） ----------

let refCount = 0;
let runtimeToken = 0;
let timerId: ReturnType<typeof setInterval> | null = null;
let unlistenState: (() => void) | null = null;

function tick(): void {
  const { state, remaining } = usePomodoroStore.getState();
  if (!state) return;
  const next = remainingOf(state);
  if (next !== remaining) usePomodoroStore.setState({ remaining: next });
}

async function ensureRuntime(): Promise<void> {
  refCount += 1;
  if (refCount > 1) return;
  const token = ++runtimeToken;

  // 1) 挂载即拉取当前状态（命令缺失 → 标记后端未就绪）
  void pomodoroState()
    .then((s) => {
      if (token !== runtimeToken) return;
      usePomodoroStore.getState().apply(s);
      usePomodoroStore.getState().setAvailable(true);
    })
    .catch((err) => {
      console.error("pomodoro_state 拉取失败（后端未就绪？）", err);
      if (token === runtimeToken) usePomodoroStore.getState().setAvailable(false);
    });

  // 2) 订阅事件：状态推送 + 完成时兜底重拉（保证多窗口同源）
  try {
    const offState = await onPomodoroState((s) => usePomodoroStore.getState().apply(s));
    const offFinished = await onPomodoroFinished((e) => {
      // 提示音：focus 结束→chime；break 结束→soft
      if (e.phase === "focus") playPomodoroDone();
      else playBreakDone();
      // 中断反馈：手动停止/跳过/意外退出导致 focus 会话中断时明确告知（本番茄未计入）
      if (
        e.phase === "focus" &&
        (e.reason === "manual" || e.reason === "skip" || e.reason === "app_exit")
      ) {
        toast.info("专注已中断，本番茄未计入统计");
      }
      void pomodoroState()
        .then((s) => {
          if (token === runtimeToken) usePomodoroStore.getState().apply(s);
        })
        .catch(() => {});
    });
    if (token !== runtimeToken) {
      offState();
      offFinished();
      return;
    }
    unlistenState = () => {
      offState();
      offFinished();
    };
  } catch (err) {
    console.error("番茄钟事件订阅失败", err);
    if (token === runtimeToken) usePomodoroStore.getState().setAvailable(false);
  }

  // 3) 1s 本地推算（权威仍是 endsAt / 事件推送）
  if (token === runtimeToken && timerId === null) {
    timerId = setInterval(tick, 1000);
  }
}

function releaseRuntime(): void {
  refCount = Math.max(0, refCount - 1);
  if (refCount > 0) return;
  runtimeToken += 1;
  unlistenState?.();
  unlistenState = null;
  if (timerId !== null) {
    clearInterval(timerId);
    timerId = null;
  }
}

// ---------- 命令执行：失败统一降级 ----------

async function runStateCommand(p: Promise<StatePayload>): Promise<StatePayload | null> {
  try {
    const s = await p;
    usePomodoroStore.getState().apply(s);
    usePomodoroStore.getState().setAvailable(true);
    return s;
  } catch (err) {
    console.error("番茄钟命令失败（后端未就绪？）", err);
    usePomodoroStore.getState().setAvailable(false);
    return null;
  }
}

export interface UsePomodoroReturn {
  state: StatePayload | null;
  /** 每秒更新的剩余秒数 */
  remaining: number;
  /** mm:ss（超 1 小时 h:mm:ss） */
  mmss: string;
  /** 有结束时间的运行中阶段 */
  isRunning: boolean;
  /** 后端是否就绪（false → UI 显示"后端未就绪"降级） */
  available: boolean;
  start: (
    noteId?: string | null,
    taskKey?: string | null,
    taskText?: string | null,
  ) => Promise<StatePayload | null>;
  pause: () => Promise<StatePayload | null>;
  resume: () => Promise<StatePayload | null>;
  skip: () => Promise<StatePayload | null>;
  stop: (reason?: string) => Promise<StatePayload | null>;
  addMinutes: (minutes: number) => Promise<StatePayload | null>;
  completeTask: (
    noteId: string,
    taskKey: string,
    lineText: string,
  ) => Promise<StatePayload | null>;
}

/** 全局单例 hook：任意组件调用均读写同一份模块级 store */
export function usePomodoro(): UsePomodoroReturn {
  const state = usePomodoroStore((s) => s.state);
  const remaining = usePomodoroStore((s) => s.remaining);
  const available = usePomodoroStore((s) => s.available);

  useEffect(() => {
    void ensureRuntime();
    return () => {
      releaseRuntime();
    };
  }, []);

  const start = useCallback(
    (noteId: string | null = null, taskKey: string | null = null, taskText: string | null = null) =>
      runStateCommand(pomodoroStart(noteId, taskKey, taskText)),
    [],
  );
  const pause = useCallback(() => runStateCommand(pomodoroPause()), []);
  const resume = useCallback(() => runStateCommand(pomodoroResume()), []);
  const skip = useCallback(() => runStateCommand(pomodoroSkip()), []);
  const stop = useCallback(
    (reason: string = "manual") => runStateCommand(pomodoroStop(reason)),
    [],
  );
  const addMinutes = useCallback(
    (minutes: number) => runStateCommand(pomodoroAddMinutes(minutes)),
    [],
  );
  const completeTask = useCallback(
    (noteId: string, taskKey: string, lineText: string) =>
      runStateCommand(pomodoroCompleteTask(noteId, taskKey, lineText)),
    [],
  );

  return useMemo(
    () => ({
      state,
      remaining,
      mmss: fmtMmss(remaining),
      isRunning: !!state && !state.paused && !!state.endsAt,
      available,
      start,
      pause,
      resume,
      skip,
      stop,
      addMinutes,
      completeTask,
    }),
    [state, remaining, available, start, pause, resume, skip, stop, addMinutes, completeTask],
  );
}
