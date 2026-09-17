//! Windows 桌面层钉住：通过 Progman/WorkerW 将便签窗口嵌入桌面层。
//!
//! 原理：
//! 1. 向 Progman 发送 0x052C 消息，触发 Windows 在桌面图标层之下创建一个 WorkerW
//! 2. 枚举顶级窗口找到这个 WorkerW（不含 SHELLDLL_DefView 的那个）
//! 3. SetParent(便签窗口, WorkerW) 将便签嵌入桌面层
//! 4. 设置 WS_EX_TOOLWINDOW 使其不占任务栏和 Alt+Tab
//!
//! 取消钉桌面时：SetParent(便签, NULL) 恢复顶级窗口，还原样式。

use windows::core::PCWSTR;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::error::{AppError, AppResult};

const SPAWN_WORKERW_MSG: u32 = 0x052C;
const WS_EX_TOOLWINDOW_VAL: u32 = 0x0000_0080;
use windows::Win32::UI::WindowsAndMessaging::WINDOW_LONG_PTR_INDEX;
const GWL_EXSTYLE_IDX: WINDOW_LONG_PTR_INDEX = WINDOW_LONG_PTR_INDEX(-20);

struct WorkerWCtx {
    target: Option<isize>,
    seen_defview: bool,
}

unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let ctx = &mut *(lparam.0 as *mut WorkerWCtx);
    let mut buf = [0u16; 256];
    let len = GetClassNameW(hwnd, &mut buf);
    let class = String::from_utf16_lossy(&buf[..len as usize]);

    if class == "WorkerW" {
        let defview = FindWindowExW(hwnd, HWND::default(), PCWSTR::null(), PCWSTR::null());
        let has_defview = defview.map(|h| !h.is_invalid()).unwrap_or(false);
        if has_defview {
            ctx.seen_defview = true;
        } else if ctx.seen_defview {
            ctx.target = Some(hwnd.0 as isize);
        }
    }
    true.into()
}

/// 查找桌面层 WorkerW。
unsafe fn find_workerw(progman: HWND) -> Option<HWND> {
    SendMessageTimeoutW(
        progman,
        SPAWN_WORKERW_MSG,
        WPARAM(0),
        LPARAM(0),
        SEND_MESSAGE_TIMEOUT_FLAGS(0),
        1000,
        None,
    );
    let mut ctx = WorkerWCtx {
        target: None,
        seen_defview: false,
    };
    EnumWindows(
        Some(enum_proc),
        LPARAM(&mut ctx as *mut WorkerWCtx as isize),
    );
    ctx.target.map(|p| HWND(p as *mut _))
}

pub unsafe fn get_progman() -> AppResult<HWND> {
    let cls: Vec<u16> = "Progman".encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = FindWindowW(PCWSTR::from_raw(cls.as_ptr()), None)
        .map_err(|e| AppError::Platform(format!("Progman not found: {e}")))?;
    if hwnd.is_invalid() {
        return Err(AppError::Platform("Progman handle invalid".into()));
    }
    Ok(hwnd)
}

pub unsafe fn pin_to_desktop(note: HWND) -> AppResult<()> {
    let progman = unsafe { get_progman()? };
    let workerw = find_workerw(progman)
        .ok_or_else(|| AppError::Platform("WorkerW not found (may need retry)".into()))?;
    if workerw.is_invalid() {
        return Err(AppError::Platform("WorkerW handle invalid".into()));
    }

    let ex = GetWindowLongW(note, GWL_EXSTYLE_IDX) as u32;
    SetWindowLongW(note, GWL_EXSTYLE_IDX, (ex | WS_EX_TOOLWINDOW_VAL) as i32);
    SetParent(note, workerw);

    let _ = SetWindowPos(
        note,
        HWND_BOTTOM,
        0,
        0,
        0,
        0,
        SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
    Ok(())
}

pub unsafe fn unpin_from_desktop(note: HWND) -> AppResult<()> {
    let parent = GetParent(note).unwrap_or_default();
    if parent.0 as isize == 0 {
        return Ok(());
    }

    let ex = GetWindowLongW(note, GWL_EXSTYLE_IDX) as u32;
    SetWindowLongW(note, GWL_EXSTYLE_IDX, (ex & !WS_EX_TOOLWINDOW_VAL) as i32);
    SetParent(note, HWND::default());
    let _ = SetWindowPos(
        note,
        HWND_TOP,
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED,
    );
    Ok(())
}

pub unsafe fn is_on_desktop(note: HWND) -> bool {
    let parent = GetParent(note).unwrap_or_default();
    if parent.0 as isize == 0 {
        return false;
    }
    let mut buf = [0u16; 256];
    let len = GetClassNameW(parent, &mut buf);
    let cls = String::from_utf16_lossy(&buf[..len as usize]);
    cls == "WorkerW"
}
