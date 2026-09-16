//! 前台全屏检测：WinEvent 钩子线程（EVENT_SYSTEM_FOREGROUND）+ 消费者线程。
//! 钩子回调只向通道发信号（回调内不允许做重活），
//! 真实的全屏探测与策略应用在消费者线程执行，且仅在状态变化时应用。

#[cfg(windows)]
mod imp {
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Mutex;

    use tauri::AppHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK, UnhookWinEvent};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, GetMessageW, TranslateMessage, MSG,
    };

    /// winuser.h: EVENT_SYSTEM_FOREGROUND —— 前台窗口切换事件
    const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
    /// winable.h: WINEVENT_OUTOFCONTEXT —— 回调在调用线程外投递（无需 DLL 注入）
    const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;

    /// 回调无法捕获环境，发送端放入全局静态。
    static WINEVENT_TX: Mutex<Option<Sender<()>>> = Mutex::new(None);

    pub fn spawn(app: AppHandle) {
        let (tx, rx) = channel::<()>();
        if let Ok(mut guard) = WINEVENT_TX.lock() {
            *guard = Some(tx);
        }

        // 钩子线程：注册 WinEvent 钩子并跑消息循环（钩子依赖本线程的消息泵）
        let _ = std::thread::Builder::new()
            .name("focusly-winevent".into())
            .spawn(hook_thread);

        // 消费者线程：检测全屏并应用策略
        let _ = std::thread::Builder::new()
            .name("focusly-fullscreen".into())
            .spawn(move || consumer_thread(rx, app));
    }

    unsafe extern "system" fn on_foreground(
        _hook: HWINEVENTHOOK,
        _event: u32,
        _hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _id_thread: u32,
        _dwms_event_time: u32,
    ) {
        if let Ok(guard) = WINEVENT_TX.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(());
            }
        }
    }

    fn hook_thread() {
        unsafe {
            let hook: HWINEVENTHOOK = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None, // 不绑定 DLL 模块
                Some(on_foreground),
                0, // 所有进程
                0, // 所有线程
                WINEVENT_OUTOFCONTEXT,
            );
            if hook.is_invalid() {
                log::error!("SetWinEventHook 注册失败，全屏自动显隐策略不可用");
                return;
            }
            log::info!("前台切换监听已启动（EVENT_SYSTEM_FOREGROUND）");

            // WinEvent 钩子（OUTOFCONTEXT）要求注册线程有消息循环
            let mut msg = MSG::default();
            loop {
                // GetMessageW 返回 BOOL：0=WM_QUIT，-1=出错，其余=收到消息
                let r = GetMessageW(&mut msg, None, 0, 0);
                if r.0 == 0 || r.0 == -1 {
                    break;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
            log::info!("前台切换监听已退出");
        }
    }

    fn consumer_thread(rx: Receiver<()>, app: AppHandle) {
        let mut last: Option<bool> = None;
        while rx.recv().is_ok() {
            let fullscreen = crate::window::monitor::is_foreground_fullscreen();
            if last == Some(fullscreen) {
                continue; // 状态未变化，跳过
            }
            last = Some(fullscreen);
            log::info!("前台全屏状态变化: fullscreen={fullscreen}");
            crate::window::apply_fullscreen_policy(&app, fullscreen);
            // 全屏变化 → 同步投递番茄钟调度器（FullscreenChanged：写提示位 + watch
            // 唤醒通知延迟等待，消除等待处对 2s 轮询的依赖；投递失败仅记日志，
            // 等待处保留 2s 兜底轮询，不致命）
            if let Err(e) = crate::pomodoro::send_cmd(
                &app,
                crate::pomodoro::PomodoroCmd::FullscreenChanged(fullscreen),
            ) {
                log::warn!("FullscreenChanged 投递失败: {e}");
            }
        }
    }
}

#[cfg(windows)]
pub fn spawn(app: tauri::AppHandle) {
    imp::spawn(app);
}

#[cfg(not(windows))]
pub fn spawn(_app: tauri::AppHandle) {}
