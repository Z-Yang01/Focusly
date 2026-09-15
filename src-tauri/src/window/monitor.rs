//! 显示器工具：枚举显示器、识别窗口所在显示器、越界检测。
//! 使用 Win32 API，坐标全部为物理像素。

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFOEXW,
    MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

#[cfg(windows)]
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub device: String,
    pub rect: RECT,
}

#[cfg(windows)]
unsafe extern "system" fn enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = &mut *(lparam.0 as *mut Vec<MonitorInfo>);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmonitor, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).as_bool() {
        let device = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())],
        );
        monitors.push(MonitorInfo {
            device,
            rect: info.monitorInfo.rcMonitor,
        });
    }
    true.into()
}

#[cfg(windows)]
/// 枚举所有显示器（物理坐标）。
pub fn all_monitors() -> Vec<MonitorInfo> {
    let mut monitors: Vec<MonitorInfo> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize),
        );
    }
    monitors
}

#[cfg(windows)]
/// 窗口所在显示器的设备名（如 \\.\DISPLAY1）。
pub fn monitor_of_window(hwnd: HWND) -> Option<String> {
    let nearest = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    unsafe {
        GetMonitorInfoW(nearest, &mut info as *mut MONITORINFOEXW as *mut MONITORINFO).ok()?;
    }
    Some(String::from_utf16_lossy(
        &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())],
    ))
}

#[cfg(windows)]
/// 窗口矩形与任意显示器有足够相交（避免窗口"丢"在屏幕外）。
pub fn rect_visible_on_any_monitor(x: i32, y: i32, width: i32, height: i32) -> bool {
    const MIN_VISIBLE: i32 = 48;
    let (wx1, wy1, wx2, wy2) = (x, y, x + width, y + height);
    all_monitors().iter().any(|m| {
        let ix = wx2.min(m.rect.right) - wx1.max(m.rect.left);
        let iy = wy2.min(m.rect.bottom) - wy1.max(m.rect.top);
        ix >= MIN_VISIBLE && iy >= MIN_VISIBLE
    })
}

#[cfg(not(windows))]
pub fn all_monitors() -> Vec<()> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn rect_visible_on_any_monitor(_x: i32, _y: i32, _w: i32, _h: i32) -> bool {
    true
}

#[cfg(windows)]
/// 前台窗口是否处于全屏（矩形铺满所在显示器且非系统 UI）。
pub fn is_foreground_fullscreen() -> bool {
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWINDOWATTRIBUTE,
    };
    use windows::Win32::UI::Shell::SHQueryUserNotificationState;
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        // 快速通道：系统通知状态（全屏 D3D / 演示模式 / BUSY 均视为全屏占用）
        if let Ok(state) = SHQueryUserNotificationState() {
            use windows::Win32::UI::Shell::QUERY_USER_NOTIFICATION_STATE;
            if matches!(
                state,
                QUERY_USER_NOTIFICATION_STATE::QUNS_RUNNING_D3D_FULL_SCREEN
                    | QUERY_USER_NOTIFICATION_STATE::QUNS_PRESENTATION_MODE
            ) {
                return true;
            }
            // QUNS_BUSY 同时覆盖"普通应用铺满全屏"（如浏览器 F11），
            // 但也包含其他勿扰场景；配合矩形判定避免误判。
            let hwnd = GetForegroundWindow();
            if hwnd.is_invalid() {
                return false;
            }
            // 排除被 cloak 的窗口（其他虚拟桌面上的 UWP 等）
            let mut cloaked: u32 = 0;
            let hr = DwmGetWindowAttribute(
                hwnd,
                DWMWINDOWATTRIBUTE::DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut _,
                std::mem::size_of::<u32>() as u32,
            );
            if hr.is_ok() && cloaked != 0 {
                return false;
            }
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return false;
            }
            let monitor =
                MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO::default();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let m = info.rcMonitor;
                let is_full = rect.left <= m.left
                    && rect.top <= m.top
                    && rect.right >= m.right
                    && rect.bottom >= m.bottom;
                if is_full {
                    return true;
                }
            }
            if state == QUERY_USER_NOTIFICATION_STATE::QUNS_BUSY {
                // BUSY 且前台窗口未铺满：不视为全屏
                return false;
            }
            false
        } else {
            false
        }
    }
}

#[cfg(not(windows))]
pub fn is_foreground_fullscreen() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn fullscreen_probe_does_not_crash() {
        // 无法在 CI/自动化环境中可靠构造全屏前台窗口，只验证调用安全
        let _ = super::is_foreground_fullscreen();
        let monitors = super::all_monitors();
        // 有显示器的环境下至少能枚举到一个
        if !monitors.is_empty() {
            assert!(super::rect_visible_on_any_monitor(0, 0, 100, 100));
            assert!(!super::rect_visible_on_any_monitor(-20000, -20000, 100, 100));
        }
    }
}
