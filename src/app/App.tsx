/** 入口路由：按窗口 label 决定渲染管理器还是便签窗口。
 *  窗口 label 约定：manager | note-<uuid>
 *  各窗口组件 React.lazy 动态加载：迷你番茄/速记箱窗不再携带管理器与 Markdown 代码，
 *  每个窗口只下载自己角色需要的 chunk。便签/速记窗本就 visible:false 等就绪再显示，
 *  chunk 加载延迟不产生白闪；管理器 fallback 为主题底色空壳。 */
import { lazy, Suspense, useState } from "react";
import { ThemeProvider } from "@/app/theme";
import { getWindowRole } from "@/lib/tauri";

const ManagerWindow = lazy(() =>
  import("@/features/notes/ManagerWindow").then((m) => ({ default: m.ManagerWindow })),
);
const NoteWindow = lazy(() =>
  import("@/features/notes/NoteWindow").then((m) => ({ default: m.NoteWindow })),
);
const QuickCaptureWindow = lazy(() =>
  import("@/features/quick-capture/QuickCaptureWindow").then((m) => ({
    default: m.QuickCaptureWindow,
  })),
);
const MiniPomodoroWindow = lazy(() =>
  import("@/features/pomodoro/MiniPomodoroWindow").then((m) => ({
    default: m.MiniPomodoroWindow,
  })),
);

function WindowFallback() {
  return <div className="h-screen w-screen bg-background" aria-hidden="true" />;
}

export default function App() {
  const [role] = useState(getWindowRole);

  return (
    <ThemeProvider>
      <Suspense fallback={<WindowFallback />}>
        {role.kind === "manager" ? (
          <ManagerWindow />
        ) : role.kind === "quick-capture" ? (
          <QuickCaptureWindow />
        ) : role.kind === "pomodoro-mini" ? (
          <MiniPomodoroWindow />
        ) : (
          <NoteWindow noteId={role.noteId} />
        )}
      </Suspense>
    </ThemeProvider>
  );
}
