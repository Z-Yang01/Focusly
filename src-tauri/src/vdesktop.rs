//! Windows 虚拟桌面固定：通过 ImmersiveShell 的 IVirtualDesktopPinnedApps COM 接口，
//! 与任务栏右键"在所有桌面上显示"是同一机制。
//!
//! 接口/CLSID 定义来自 MScholtes/VirtualDesktop 与 widavies/WinJump（已核实的 vtable 顺序）。
//! 实现采用手工 vtable 声明（windows 0.58 的 `#[interface]` 宏在 rustc 1.98 下
//! 因 `interface` 成为保留关键字而无法解析，手工声明确定性最高）。

/// CLSID_ImmersiveShell（Windows Shell 宿主服务）
pub const CLSID_IMMERSIVE_SHELL: &str = "c2f03a33-21f5-47fa-b4bb-156362a2f239";
/// CLSID_VirtualDesktopPinnedApps（固定服务）
pub const CLSID_VIRTUAL_DESKTOP_PINNED_APPS: &str = "b5a399e7-1c87-46b8-88e9-fc5747b171bd";
/// IVirtualDesktopPinnedApps 的 IID（vtable: IUnknown×3 + IsAppIdPinned, PinAppID,
/// UnpinAppID, IsWindowPinned, PinWindow, UnpinWindow）
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
    use std::ffi::c_void;
    use super::{CLSID_IMMERSIVE_SHELL, CLSID_VIRTUAL_DESKTOP_PINNED_APPS, IID_VIRTUAL_DESKTOP_PINNED_APPS};
    use windows::core::{GUID, Interface as _};
    use windows::Win32::Foundation::{BOOL, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_LOCAL_SERVER,
        COINIT_APARTMENTTHREADED, IServiceProvider,
    };

    /// combaseapi.h: RPC_E_CHANGED_MODE（0x80010106）——线程已按其他套间模型初始化。
    /// 不同 windows crate 版本导出位置不一致，直接本地定义。
    const RPC_E_CHANGED_MODE: windows::core::HRESULT = windows::core::HRESULT(0x8001_0106u32 as i32);

    fn guid(s: &str) -> GUID {
        GUID::from_u128(u128::from_str_radix(s.replace('-', "").as_str(), 16).expect("合法 GUID"))
    }

    /// IServiceProvider 自定义 vtable（IUnknown×3 + QueryService）。
    #[repr(C)]
    struct ServiceProviderVtbl {
        query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> windows::core::HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        query_service: unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            *const GUID,
            *mut *mut c_void,
        ) -> windows::core::HRESULT,
    }

    /// IVirtualDesktopPinnedApps vtable（IUnknown×3 + 六个方法，顺序不可变）。
    /// 未使用的 IsAppIdPinned/PinAppID/UnpinAppID 参数以裸指针占位（x64 指针宽度一致，布局等价）。
    #[repr(C)]
    struct PinnedAppsVtbl {
        query_interface: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> windows::core::HRESULT,
        add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
        is_app_id_pinned: unsafe extern "system" fn(*mut c_void, *const u16, *mut BOOL) -> windows::core::HRESULT,
        pin_app_id: unsafe extern "system" fn(*mut c_void, *const u16) -> windows::core::HRESULT,
        unpin_app_id: unsafe extern "system" fn(*mut c_void, *const u16) -> windows::core::HRESULT,
        is_window_pinned: unsafe extern "system" fn(*mut c_void, HWND, *mut BOOL) -> windows::core::HRESULT,
        pin_window: unsafe extern "system" fn(*mut c_void, HWND) -> windows::core::HRESULT,
        unpin_window: unsafe extern "system" fn(*mut c_void, HWND) -> windows::core::HRESULT,
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
        let shell_clsid = guid(CLSID_IMMERSIVE_SHELL);
        let pinned_clsid = guid(CLSID_VIRTUAL_DESKTOP_PINNED_APPS);
        let iid_pinned = guid(IID_VIRTUAL_DESKTOP_PINNED_APPS);

        let shell: IServiceProvider = CoCreateInstance(&shell_clsid, None, CLSCTX_LOCAL_SERVER)
            .map_err(|e| {
            crate::vdesktop::VdError::Unsupported(format!("ImmersiveShell 服务不可用: {e}"))
        })?;

        // 手工调用 IServiceProvider::QueryService（泛型版要求 windows-core Interface trait，
        // 我们的对象是手工 vtable，因此直接走 vtable 槽位）
        let sp_vtbl = &**(shell.as_raw() as *mut *mut ServiceProviderVtbl);
        let mut obj: *mut c_void = std::ptr::null_mut();
        let hr = (sp_vtbl.query_service)(shell.as_raw(), &pinned_clsid, &iid_pinned, &mut obj);
        if hr.is_err() || obj.is_null() {
            return Err(crate::vdesktop::VdError::Unsupported(format!(
                "虚拟桌面固定服务不可用: {hr:?}"
            )));
        }

        let vtbl = &**(obj as *mut *mut PinnedAppsVtbl);

        // 先查当前状态，避免重复调用
        let mut current = BOOL(0);
        let hr = (vtbl.is_window_pinned)(obj, hwnd, &mut current);
        if hr.is_err() {
            (vtbl.release)(obj);
            return Err(crate::vdesktop::VdError::Unsupported(format!(
                "查询固定状态失败: {hr:?}"
            )));
        }
        if current.as_bool() == pin {
            (vtbl.release)(obj);
            return Ok(());
        }

        let hr = if pin { (vtbl.pin_window)(obj, hwnd) } else { (vtbl.unpin_window)(obj, hwnd) };
        (vtbl.release)(obj);
        if hr.is_err() {
            return Err(crate::vdesktop::VdError::Failed(if pin {
                format!("固定窗口到所有桌面失败: {hr:?}")
            } else {
                format!("取消固定失败: {hr:?}")
            }));
        }
        Ok(())
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
