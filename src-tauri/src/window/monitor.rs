//! 显示器工具：枚举显示器、识别窗口所在显示器、越界检测。
//! 使用 Win32 API，坐标全部为物理像素。

#[cfg(windows)]
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromWindow, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
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
    if GetMonitorInfoW(
        hmonitor,
        &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
    )
    .as_bool()
    {
        let device = String::from_utf16_lossy(
            &info.szDevice[..info
                .szDevice
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(info.szDevice.len())],
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
        // 返回值仅为 BOOL 表示枚举是否被回调中断（enum_proc 恒返回 true），无需检查
        let _ = EnumDisplayMonitors(
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
        if !GetMonitorInfoW(
            nearest,
            &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
        .as_bool()
        {
            return None;
        }
    }
    Some(String::from_utf16_lossy(
        &info.szDevice[..info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len())],
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

#[cfg(windows)]
/// 主显示器工作区矩形（物理像素，已扣除任务栏）；获取失败返回 None。
/// 网格排列等平铺场景必须用工作区而非整块显示器，否则便签会盖住任务栏。
pub fn primary_work_area_rect() -> Option<RECT> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{
        MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };
    unsafe {
        let hmonitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
            Some(info.rcWork)
        } else {
            None
        }
    }
}

/// 主显示器工作区尺寸（物理像素）。
pub fn primary_work_area_size() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        primary_work_area_rect().map(|r| (r.right - r.left, r.bottom - r.top))
    }
    #[cfg(not(windows))]
    {
        None
    }
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
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWINDOWATTRIBUTE};
    use windows::Win32::UI::Shell::SHQueryUserNotificationState;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        // 快速通道：系统通知状态（原始值：2=QUNS_BUSY, 3=QUNS_RUNNING_D3D_FULL_SCREEN, 4=QUNS_PRESENTATION_MODE）
        let Ok(state) = SHQueryUserNotificationState() else {
            return false;
        };
        if state.0 == 3 || state.0 == 4 {
            return true;
        }
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }
        // 排除本进程窗口：无边框最大化的管理器/便签矩形恰好铺满显示器，
        // 误判会导致"用户一切到管理器，全屏隐藏策略就触发"。
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return false;
        }
        // 排除被 cloak 的窗口（其他虚拟桌面上的 UWP 等）；DWMWA_CLOAKED = 14
        let mut cloaked: u32 = 0;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(14),
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
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO::default();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            let m = info.rcMonitor;
            if rect.left <= m.left
                && rect.top <= m.top
                && rect.right >= m.right
                && rect.bottom >= m.bottom
            {
                return true;
            }
        }
        // QUNS_BUSY(=2) 且前台窗口未铺满：不视为全屏
        false
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
            assert!(!super::rect_visible_on_any_monitor(
                -20000, -20000, 100, 100
            ));
        }
    }
}
