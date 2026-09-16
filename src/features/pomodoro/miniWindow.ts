/** 迷你番茄窗的前端创建/显示/隐藏（前端建窗，位置记忆存 localStorage） */
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";

const LABEL = "pomodoro-mini";
const POS_KEY = "focusly.pomodoro.pos";

/** 打开或聚焦迷你番茄窗（已存在则显示+聚焦） */
export async function ensureMiniPomodoro(): Promise<void> {
  const existing = await WebviewWindow.getByLabel(LABEL);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }
  let pos: { x: number; y: number } | null = null;
  try {
    pos = JSON.parse(localStorage.getItem(POS_KEY) ?? "null");
  } catch {
    pos = null;
  }
  const win = new WebviewWindow(LABEL, {
    title: "番茄钟",
    url: "index.html",
    width: 340,
    height: 170,
    x: pos?.x,
    y: pos?.y,
    decorations: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: false,
    shadow: false,
  });
  win.once("tauri://created", () => {
    log("迷你番茄窗已创建");
  });
  win.onMoved((e) => {
    try {
      localStorage.setItem(POS_KEY, JSON.stringify({ x: e.payload.x, y: e.payload.y }));
    } catch {
      /* 存储失败不影响功能 */
    }
  });
}

/** 隐藏（不销毁）迷你番茄窗；窗口不存在时静默 */
export async function hideMiniPomodoro(): Promise<void> {
  const existing = await WebviewWindow.getByLabel(LABEL);
  if (existing) await existing.hide();
}

function log(msg: string) {
  console.log(`[pomodoro-mini] ${msg}`);
}
