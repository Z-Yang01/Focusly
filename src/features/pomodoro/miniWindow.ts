/** 迷你番茄窗的前端创建/显示/隐藏
 *  位置策略：全程物理像素（onMoved 事件与 setPosition(PhysicalPosition) 同口径），
 *  恢复时按当前显示器边界校验，越界回退到屏幕右上区域；避免 DPI/插拔后跑位。 */
import { availableMonitors, PhysicalPosition } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { toast } from "@/stores/toast";

const LABEL = "pomodoro-mini";
const POS_KEY = "focusly.pomodoro.pos";
const WIDTH = 340;
/** 240 = 头部 37 + 主体（阶段文字 + 80px 进度环 + 任务行）~128 + 2×2 按钮网格 ~75 */
const HEIGHT = 240;

interface PhysPoint {
  x: number;
  y: number;
}

function loadStoredPos(): PhysPoint | null {
  try {
    const raw = localStorage.getItem(POS_KEY);
    if (!raw) return null;
    const p = JSON.parse(raw);
    if (typeof p?.x === "number" && typeof p?.y === "number") return p;
    return null;
  } catch {
    return null;
  }
}

function saveStoredPos(p: PhysPoint): void {
  try {
    localStorage.setItem(POS_KEY, JSON.stringify(p));
  } catch {
    /* 存储失败不影响功能 */
  }
}

/** 校验物理位置落在任一显示器内（至少 100/40px 可见），否则返回 null */
async function validateOnScreen(pos: PhysPoint): Promise<PhysPoint | null> {
  try {
    const monitors = await availableMonitors();
    const visible = monitors.some((m) => {
      const { x, y } = m.position;
      const { width, height } = m.size;
      return (
        pos.x + WIDTH > x + 100 &&
        pos.x < x + width - 100 &&
        pos.y + HEIGHT > y + 40 &&
        pos.y < y + height - 40
      );
    });
    return visible ? pos : null;
  } catch {
    return null;
  }
}

/** 无有效记忆位置时的默认位置：主屏右上区域 */
async function defaultPhysicalPos(): Promise<PhysPoint | null> {
  try {
    const monitors = await availableMonitors();
    if (monitors.length === 0) return null;
    const primary = monitors[0];
    return {
      x: primary.position.x + primary.size.width - WIDTH - 48,
      y: primary.position.y + 64,
    };
  } catch {
    return null;
  }
}

/** 打开或聚焦迷你番茄窗（已存在则显示+聚焦） */
export async function ensureMiniPomodoro(): Promise<void> {
  try {
    await ensureMiniPomodoroInner();
  } catch (err) {
    console.error("[pomodoro-mini] 创建/显示失败", err);
    toast.error(`迷你番茄窗打开失败: ${String(err)}`);
  }
}

async function ensureMiniPomodoroInner(): Promise<void> {
  const existing = await WebviewWindow.getByLabel(LABEL);
  if (existing) {
    await existing.show();
    await existing.setFocus();
    return;
  }

  const stored = loadStoredPos();
  const onScreen = stored ? await validateOnScreen(stored) : null;

  const win = new WebviewWindow(LABEL, {
    title: "番茄钟",
    url: "index.html",
    width: WIDTH,
    height: HEIGHT,
    decorations: false,
    transparent: true,
    alwaysOnTop: true,
    skipTaskbar: true,
    resizable: false,
    shadow: false,
  });
  await new Promise<void>((resolve, reject) => {
    win.once("tauri://created", () => resolve());
    win.once("tauri://error", (e) => reject(new Error(String(e.payload))));
  });

  // 物理像素定位（构造器 x/y 是逻辑像素，DPI≠100% 会漂移，故创建后用物理坐标覆盖）
  const target = onScreen ?? (await defaultPhysicalPos());
  if (target) {
    await win.setPosition(new PhysicalPosition(target.x, target.y));
  }

  // 记录移动（物理像素）
  win.onMoved((e: { payload: { x: number; y: number } }) =>
    saveStoredPos({ x: e.payload.x, y: e.payload.y }),
  );
}

/** 隐藏（不销毁）迷你番茄窗；窗口不存在时静默 */
export async function hideMiniPomodoro(): Promise<void> {
  const existing = await WebviewWindow.getByLabel(LABEL);
  if (existing) await existing.hide();
}
