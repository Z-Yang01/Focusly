/** 入口路由：按窗口 label 决定渲染管理器还是便签窗口。
 *  窗口 label 约定：manager | note-<uuid> */
import { useState } from "react";
import { ThemeProvider } from "@/app/theme";
import { getWindowRole } from "@/lib/tauri";
import { ManagerWindow } from "@/features/notes/ManagerWindow";
import { NoteWindow } from "@/features/notes/NoteWindow";

export default function App() {
  const [role] = useState(getWindowRole);

  return (
    <ThemeProvider>
      {role.kind === "manager" ? (
        <ManagerWindow />
      ) : (
        <NoteWindow noteId={role.noteId} />
      )}
    </ThemeProvider>
  );
}
