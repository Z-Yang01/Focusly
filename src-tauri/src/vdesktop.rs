//! Windows 虚拟桌面固定：通过 ImmersiveShell 的 IVirtualDesktopPinnedApps COM 接口，
//! 与任务栏右键"在所有桌面上显示"是同一机制。
//!
//! 接口/CLSID 定义来自 MScholtes/VirtualDesktop 与 widavies/WinJump（已核实的 vtable 顺序）。

/// CLSID_ImmersiveShell（Windows Shell 宿主服务）
pub const CLSID_IMMERSIVE_SHELL: &str = "c2f03a33-21f5-47fa-b4bb-156362a2f239";
/// CLSID_VirtualDesktopPinnedApps（固定服务）
pub const CLSID_VIRTUAL_DESKTOP_PINNED_APPS: &str = "b5a399e7-1c87-46b8-88e9-fc5747b171bd";
/// IVirtualDesktopPinnedApps 的 IID（vtable: IsAppIdPinned, PinAppID, UnpinAppID,
/// IsWindowPinned, PinWindow, UnpinWindow）
pub const IID_VIRTUAL_DESKTOP_PINNED_APPS: &str = "4ce81583-1e4c-4632-a621-07a53543148f";

#[derive(Debug, thiserror::Error)]
pub enum VdError {
    #[error("当前系统不支持该功能: {0}")]
    Unsupported(String),
    #[error("操作失败: {0}")]
    Failed(String),
}

#[cfg(not(windows))]
pub fn set_window_pinned(
    _app: &tauri::AppHandle,
    _win: &tauri::WebviewWindow,
    _pin: bool,
) -> Result<(), VdError> {
    Err(VdError::Unsupported("仅 Windows 支持".into()))
}

#[cfg(windows)]
mod imp {
    use windows::core::GUID;
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
        IServiceProvider,
    };

    /// combaseapi.h: RPC_E_CHANGED_MODE（0x80010106）——线程已按其他套间模型初始化。
    /// 不同 windows crate 版本导出位置不一致，直接本地定义。
    const RPC_E_CHANGED_MODE: windows::core::HRESULT = windows::core::HRESULT(0x8001_0106u32 as i32);

    /// COM 自定义接口。vtable 顺序 = IUnknown 三项 + 下面六个方法，顺序绝对不能变。
    #[windows::core::interface("4ce81583-1e4c-4632-a621-07a53543148f")]
    pub unsafe interface IVirtualDesktopPinnedApps: windows::core::IUnknown {
        pub fn isappidpinned(&self, appid: windows::core::PCWSTR) -> windows::core::Result<BOOL>;
        pub fn pinappid(&self, appid: windows::core::PCWSTR) -> windows::core::Result<()>;
        pub fn unpinappid(&self, appid: windows::core::PCWSTR) -> windows::core::Result<()>;
        pub fn iswindowpinned(&self, hwnd: HWND) -> windows::core::Result<BOOL>;
        pub fn pinwindow(&self, hwnd: HWND) -> windows::core::Result<()>;
        pub fn unpinwindow(&self, hwnd: HWND) -> windows::core::Result<()>;
    }

    pub unsafe fn pin_window(hwnd: HWND, pin: bool) -> Result<(), crate::vdesktop::VdError> {
        // CoInitializeEx：S_OK/S_FALSE 视为已初始化（需配对 CoUninitialize）；
        // RPC_E_CHANGED_MODE 表示线程已按其他套间模型初始化，直接复用但不反初始化。
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let needs_uninitialize = hr.is_ok();
        if hr.is_err() && hr != RPC_E_CHANGED_MODE {
            return Err(crate::vdesktop::VdError::Failed(format!(
                "CoInitializeEx 失败: {hr:?}"
            )));
        }

        let result = pin_with_service(hwnd, pin);
        if needs_uninitialize {
            CoUninitialize();
        }
        result
    }

    unsafe fn pin_with_service(hwnd: HWND, pin: bool) -> Result<(), crate::vdesktop::VdError> {
        let shell_clsid = GUID::from(crate::vdesktop::CLSID_IMMERSIVE_SHELL);
        let pinned_clsid = GUID::from(crate::vdesktop::CLSID_VIRTUAL_DESKTOP_PINNED_APPS);

        let shell: IServiceProvider = CoCreateInstance(&shell_clsid, None, CLSCTX_LOCAL_SERVER)
            .map_err(|e| {
                crate::vdesktop::VdError::Unsupported(format!("ImmersiveShell 服务不可用: {e}"))
            })?;

        let pinned: IVirtualDesktopPinnedApps = shell
            .QueryService::<IVirtualDesktopPinnedApps>(&pinned_clsid)
            .map_err(|e| {
                crate::vdesktop::VdError::Unsupported(format!("虚拟桌面固定服务不可用: {e}"))
            })?;

        // 先查当前状态，避免重复调用
        let current = pinned
            .iswindowpinned(hwnd)
            .map_err(|e| crate::vdesktop::VdError::Unsupported(format!("查询固定状态失败: {e}")))?
            .as_bool();
        if current == pin {
            return Ok(());
        }
        if pin {
            pinned.pinwindow(hwnd).map_err(|e| {
                crate::vdesktop::VdError::Failed(format!("固定窗口到所有桌面失败: {e}"))
            })
        } else {
            pinned.unpinwindow(hwnd).map_err(|e| {
                crate::vdesktop::VdError::Failed(format!("取消固定失败: {e}"))
            })
        }
    }
}

#[cfg(windows)]
pub fn set_window_pinned(
    app: &tauri::AppHandle,
    win: &tauri::WebviewWindow,
    pin: bool,
) -> Result<(), VdError> {
    let _ = app; // 预留：未来可能需要按窗口查询所属桌面
    let raw = win
        .hwnd()
        .map_err(|e| VdError::Failed(format!("无法获取窗口句柄: {e}")))?;
    // tauri 返回其内部 windows-core 的 HWND，取裸值转换为本 crate 的 HWND
    let hwnd = windows::Win32::Foundation::HWND(raw.0 as _);
    unsafe { imp::pin_window(hwnd, pin) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_guids_documented() {
        assert_eq!(CLSID_IMMERSIVE_SHELL, "c2f03a33-21f5-47fa-b4bb-156362a2f239");
        assert_eq!(
            CLSID_VIRTUAL_DESKTOP_PINNED_APPS,
            "b5a399e7-1c87-46b8-88e9-fc5747b171bd"
        );
        assert_eq!(
            IID_VIRTUAL_DESKTOP_PINNED_APPS,
            "4ce81583-1e4c-4632-a621-07a53543148f"
        );
    }
}
