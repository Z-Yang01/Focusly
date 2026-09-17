/**
 * 提示音引擎：Web Audio API 纯本地合成，零外部文件、零依赖。
 * 三种音效：chime（番茄/提醒完成）、tick（阶段切换轻触）、ding（通用提示）。
 * 全部通过 OscillatorNode 合成，静音时零开销。
 */

let ctx: AudioContext | null = null;

/** 是否启用提示音（localStorage 持久化，默认开启） */
export function isSoundEnabled(): boolean {
  try { return localStorage.getItem("focusly.sound") !== "off"; } catch { return true; }
}
export function setSoundEnabled(on: boolean) {
  try { localStorage.setItem("focusly.sound", on ? "on" : "off"); } catch { /* noop */ }
}

function getCtx(): AudioContext {
  if (!ctx) ctx = new AudioContext();
  if (ctx.state === "suspended") void ctx.resume();
  return ctx;
}

/** 播放一个单音符 */
function tone(
  freq: number,
  start: number,
  dur: number,
  gainVal: number,
  type: OscillatorType = "sine",
) {
  const ac = getCtx();
  const osc = ac.createOscillator();
  const gain = ac.createGain();
  osc.type = type;
  osc.frequency.value = freq;
  gain.gain.setValueAtTime(0, ac.currentTime + start);
  gain.gain.linearRampToValueAtTime(gainVal, ac.currentTime + start + 0.01);
  gain.gain.exponentialRampToValueAtTime(0.001, ac.currentTime + start + dur);
  osc.connect(gain);
  gain.connect(ac.destination);
  osc.start(ac.currentTime + start);
  osc.stop(ac.currentTime + start + dur + 0.05);
}

/** 番茄完成：上行三音（C5→E5→G5） */
export function playPomodoroDone() {
  if (!isSoundEnabled()) return;
  tone(523.25, 0, 0.3, 0.15);
  tone(659.25, 0.15, 0.3, 0.15);
  tone(783.99, 0.3, 0.5, 0.18);
}

/** 休息结束：下行双音（G5→C5），柔和提示回归专注 */
export function playBreakDone() {
  if (!isSoundEnabled()) return;
  tone(783.99, 0, 0.25, 0.12);
  tone(523.25, 0.2, 0.4, 0.12);
}

/** 提醒到期：双音叮咚 */
export function playDing() {
  if (!isSoundEnabled()) return;
  tone(880, 0, 0.2, 0.14);
  tone(659.25, 0.15, 0.35, 0.14);
}

/** 通用轻触 */
export function playTick() {
  if (!isSoundEnabled()) return;
  tone(1200, 0, 0.06, 0.06, "triangle");
}
