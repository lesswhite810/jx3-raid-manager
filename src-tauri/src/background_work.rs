//! 本机识别工作使用较低线程优先级，不调整游戏或主 UI 线程的优先级。

pub(crate) fn lower_current_thread_priority() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        // SAFETY: 仅设置当前后台线程；伪句柄无需关闭，失败不影响功能正确性。
        if unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) } == 0 {
            log::debug!("后台识别线程优先级设置失败，继续使用系统默认优先级");
        }
    }
}
