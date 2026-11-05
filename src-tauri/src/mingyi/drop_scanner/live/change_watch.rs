//! 轻量监听活跃账号目录中的文件名变化。
//!
//! 这里只关心创建、改名和删除的目录项，不监听 LAST_WRITE：战斗日志持续写入时
//! 不会把每一次内容追加放大成全目录枚举。收到信号后，调用方再做既有的少量
//! 文件元数据检查和必要扫描。

#[cfg(not(windows))]
use std::path::Path;

#[cfg(all(test, windows))]
mod filesystem_tests {
    use super::ChangeWatch;
    use std::time::{Duration, Instant};

    #[test]
    fn native_watch_observes_a_new_file_without_blocking() {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp")
            .join(format!("trial-notify-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut watch = ChangeWatch::new(&directory).expect("Windows directory notification");
        assert!(!watch.changed());
        let file = directory.join("new.jcl");
        std::fs::write(&file, b"new log").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut changed = watch.changed();
        while !changed && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            changed = watch.changed();
        }
        assert!(changed && watch.is_valid());
        drop(watch);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}

#[cfg(not(target_os = "windows"))]
pub(super) struct ChangeWatch;

#[cfg(not(target_os = "windows"))]
impl ChangeWatch {
    pub(super) fn new(_path: &Path) -> Option<Self> {
        None
    }

    pub(super) fn changed(&mut self) -> bool {
        false
    }

    pub(super) fn is_valid(&self) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;

    use windows_sys::Win32::Foundation::{
        HANDLE, INVALID_HANDLE_VALUE, WAIT_EVENT, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FindCloseChangeNotification, FindFirstChangeNotificationW, FindNextChangeNotification,
        FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME,
    };
    use windows_sys::Win32::System::Threading::WaitForSingleObject;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum WaitState {
        Pending,
        Signaled,
        Invalid,
    }

    fn wait_state(value: WAIT_EVENT) -> WaitState {
        match value {
            WAIT_TIMEOUT => WaitState::Pending,
            WAIT_OBJECT_0 => WaitState::Signaled,
            WAIT_FAILED => WaitState::Invalid,
            _ => WaitState::Invalid,
        }
    }

    /// Win32 FindFirst/Next 变更通知句柄。调用方始终在同一 worker 线程中使用，
    /// 不需要跨线程传递，也不实现 Send。
    pub(crate) struct ChangeWatch {
        handle: HANDLE,
        valid: bool,
    }

    impl ChangeWatch {
        pub(crate) fn new(path: &Path) -> Option<Self> {
            // 官方 API 要求完整目录路径；相对路径直接让调用方回退到低频枚举。
            if !path.is_absolute() {
                return None;
            }
            let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
            wide_path.push(0);
            // SAFETY: 路径以 NUL 终止、非递归、只监听文件和目录名变化。
            let handle = unsafe {
                FindFirstChangeNotificationW(
                    wide_path.as_ptr(),
                    0,
                    FILE_NOTIFY_CHANGE_FILE_NAME | FILE_NOTIFY_CHANGE_DIR_NAME,
                )
            };
            (handle != INVALID_HANDLE_VALUE).then_some(Self {
                handle,
                valid: true,
            })
        }

        /// 零超时轮询一次。收到信号后必须重新 arm；重新 arm 失败时仍报告本次
        /// 变化，并将监听器标为失效，让调用方改走低频枚举而不是忙循环。
        pub(crate) fn changed(&mut self) -> bool {
            if !self.valid {
                return false;
            }
            match wait_state(unsafe { WaitForSingleObject(self.handle, 0) }) {
                WaitState::Pending => false,
                WaitState::Signaled => {
                    // SAFETY: handle 只由本实例持有，尚未关闭。
                    if unsafe { FindNextChangeNotification(self.handle) } == 0 {
                        self.valid = false;
                    }
                    true
                }
                WaitState::Invalid => {
                    self.valid = false;
                    false
                }
            }
        }

        /// false 表示通知句柄初始化失败或之后失效；调用方应恢复原有低频枚举。
        pub(crate) fn is_valid(&self) -> bool {
            self.valid
        }
    }

    impl Drop for ChangeWatch {
        fn drop(&mut self) {
            // SAFETY: 本对象唯一持有由 FindFirstChangeNotificationW 成功返回的句柄。
            // 即使 FindNext 失败，官方仍要求用 FindClose 结束该通知对象。
            unsafe {
                FindCloseChangeNotification(self.handle);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{wait_state, WaitState};
        use windows_sys::Win32::Foundation::{WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT};

        #[test]
        fn zero_timeout_wait_classifies_signal_without_spinning() {
            assert_eq!(wait_state(WAIT_TIMEOUT), WaitState::Pending);
            assert_eq!(wait_state(WAIT_OBJECT_0), WaitState::Signaled);
            assert_eq!(wait_state(WAIT_FAILED), WaitState::Invalid);
            assert_eq!(wait_state(123), WaitState::Invalid);
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) use platform::ChangeWatch;
