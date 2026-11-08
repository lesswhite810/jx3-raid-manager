use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, UserAttentionType, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const RESTORE_MENU_ID: &str = "restore-main-window";
const EXIT_MENU_ID: &str = "exit-app";
static MINIMIZE_CHECK_PENDING: AtomicBool = AtomicBool::new(false);

/// 「最小化到托盘」检查的抑制截止时刻（进程启动后的毫秒数）。
///
/// 恢复动作本身会触发 WM_SIZE，进而产生 `WindowEvent::Resized`；这些事件可能在恢复
/// 返回之后才被派发，而此时 `IsIconic()` 仍可能短暂读到旧值。若排队的检查在抑制窗口内
/// 执行，就会把刚恢复的窗口再次 `hide()` 回托盘，表现为「点了恢复但窗口闪一下又没了」。
/// 采用「截止时刻」而不是布尔标志，是因为事件派发是异步的，布尔标志在函数返回时就会失效。
static RESTORE_SUPPRESS_UNTIL_MILLIS: AtomicU64 = AtomicU64::new(0);
static PROCESS_START: OnceLock<std::time::Instant> = OnceLock::new();

/// 抑制窗口长度：需覆盖恢复后的 WM_SIZE / Resized 派发与最小化状态提交延迟。
const RESTORE_SUPPRESS_MILLIS: u64 = 1200;

/// 抢前台焦点失败时临时置顶的持续时间。
///
/// 置顶仅用于把窗口带到游戏之上，到期必须撤销：常驻置顶会遮挡游戏客户区，
/// 导致试炼采集因 `client_area_is_unobscured()` 不成立而停止采帧。
const TEMPORARY_TOPMOST_MILLIS: u64 = 1200;

fn now_millis() -> u64 {
    PROCESS_START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

fn restore_suppression_active() -> bool {
    now_millis() < RESTORE_SUPPRESS_UNTIL_MILLIS.load(Ordering::Acquire)
}

fn begin_restore_suppression() {
    RESTORE_SUPPRESS_UNTIL_MILLIS.store(now_millis() + RESTORE_SUPPRESS_MILLIS, Ordering::Release);
}

/// 真实 HWND 状态，用于区分「tao 内部标志已更新」与「窗口真的显示出来了」。
#[derive(Debug, Clone, Copy)]
struct NativeWindowState {
    visible: bool,
    minimized: bool,
    foreground: bool,
}

impl NativeWindowState {
    fn describe(&self) -> String {
        format!(
            "native(visible={} minimized={} foreground={})",
            self.visible, self.minimized, self.foreground
        )
    }

    fn is_restored(&self) -> bool {
        self.visible && !self.minimized
    }
}

/// 将三个窗口状态查询结果（is_visible / is_minimized / is_focused）格式化为诊断字符串。
/// 这些 getter 读取的是 Win32 真实 HWND 状态（IsWindowVisible / IsIconic / GetForegroundWindow），
/// 而非 tao 内部 WindowFlags，用于区分「标志陈旧」与「窗口未真正显示」。
fn format_window_state<T: std::fmt::Debug>(visible: T, minimized: T, focused: T) -> String {
    format!(
        "visible={:?} minimized={:?} focused={:?}",
        visible, minimized, focused
    )
}

/// Win32 兜底恢复：同步完成「还原最小化 + 显示隐藏窗口 + 抢前台焦点」。
///
/// 之所以不只用 tauri/tao 的 `unminimize()` + `show()`：
/// - `show()` 底层是 `SW_SHOW`，对最小化窗口不会还原；
/// - tao 的 `set_visible` / `set_minimized` 都是通过 `execute_in_thread` 异步投递，
///   返回时窗口状态可能尚未改变，紧接着的检查会读到旧状态；
/// - tao 的 `set_focus()` 读内部 `WindowFlags` 而非真实 HWND，标志陈旧会整段 NO-OP。
#[cfg(windows)]
fn native_restore_and_focus(window: &tauri::WebviewWindow) -> Result<NativeWindowState, String> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow, ASFW_ANY, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE,
    };

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("读取窗口句柄失败: {}", error))?
        .0;
    if hwnd.is_null() {
        return Err("主窗口句柄为空".to_string());
    }
    // SAFETY: hwnd 取自当前进程持有的主窗口，调用只改变该窗口的显示、层级与前台状态，
    // 不涉及内存释放或跨进程数据传递；返回值仅用于判定成败，失败不会造成未定义行为。
    unsafe {
        // SW_RESTORE 同时覆盖「已最小化」与「被 SW_HIDE 隐藏」两种状态并激活窗口。
        ShowWindow(hwnd, SW_RESTORE);
        // 回到非置顶层级的最上方，避免被普通窗口压在下面。
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );
        // 游戏置顶时前台锁会拒绝 SetForegroundWindow；ASFW_ANY 让系统临时放行本进程。
        // 该授权只影响前台切换权限，失败时退回系统默认行为。
        AllowSetForegroundWindow(ASFW_ANY);
        SetForegroundWindow(hwnd);
        // 仍被前台锁拒绝时，短暂附加到当前前台线程的输入队列后重试。
        // 这是 SetForegroundWindow 在「另一进程持有前台」时的标准绕过方式；
        // 必须在同一函数内立即分离，避免长期改变输入队列行为。
        if GetForegroundWindow() != hwnd {
            let foreground = GetForegroundWindow();
            if !foreground.is_null() {
                let mut foreground_pid = 0u32;
                let foreground_thread = GetWindowThreadProcessId(foreground, &mut foreground_pid);
                let current_thread = GetCurrentThreadId();
                if foreground_thread != 0 && foreground_thread != current_thread {
                    if AttachThreadInput(current_thread, foreground_thread, 1) != 0 {
                        SetForegroundWindow(hwnd);
                        AttachThreadInput(current_thread, foreground_thread, 0);
                    }
                }
            }
        }
        Ok(NativeWindowState {
            visible: IsWindowVisible(hwnd) != 0,
            minimized: IsIconic(hwnd) != 0,
            foreground: GetForegroundWindow() == hwnd,
        })
    }
}

#[cfg(not(windows))]
fn native_restore_and_focus(_window: &tauri::WebviewWindow) -> Result<NativeWindowState, String> {
    Err("当前平台不支持 Win32 兜底恢复".to_string())
}

/// 调整主窗口的置顶层级；只在抢焦点失败时短暂使用，到期必须撤销。
#[cfg(windows)]
fn native_set_topmost(window: &tauri::WebviewWindow, topmost: bool) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    let hwnd = window
        .hwnd()
        .map_err(|error| format!("读取窗口句柄失败: {}", error))?
        .0;
    let insert_after = if topmost {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    // SAFETY: 同 native_restore_and_focus，只调整当前进程主窗口的置顶标志，
    // 不改变尺寸与位置，也不触碰其他进程的窗口。
    let changed = unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    };
    if changed == 0 {
        return Err("SetWindowPos 调整置顶层级失败".to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn native_set_topmost(_window: &tauri::WebviewWindow, _topmost: bool) -> Result<(), String> {
    Err("当前平台不支持 Win32 置顶操作".to_string())
}

/// 到期撤销临时置顶。使用一次性线程而不是轮询，失败只记录日志。
///
/// 撤销动作回到主线程执行：窗口可能在等待期间被销毁，跨线程直接操作句柄更脆弱。
fn schedule_topmost_release(window: tauri::WebviewWindow) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(TEMPORARY_TOPMOST_MILLIS));
        let release_window = window.clone();
        if let Err(error) =
            window.run_on_main_thread(move || match native_set_topmost(&release_window, false) {
                Ok(()) => log::info!("[恢复窗口] 已撤销临时置顶"),
                Err(error) => log::warn!("[恢复窗口] 撤销临时置顶失败: {}", error),
            })
        {
            log::warn!("[恢复窗口] 安排撤销临时置顶失败: {}", error);
        }
    });
}

pub fn initialize_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let restore = MenuItem::with_id(app, RESTORE_MENU_ID, "恢复主窗口", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, EXIT_MENU_ID, "退出应用", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&restore, &exit])?;
    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "缺少系统托盘所需的应用图标")
    })?;

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("剑网三副本管家")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            RESTORE_MENU_ID => {
                log::info!("[恢复窗口] 触发来源: 托盘菜单「恢复主窗口」");
                restore_main_window(app);
            }
            EXIT_MENU_ID => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                log::info!("[恢复窗口] 触发来源: 托盘左键点击");
                restore_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    log::info!("系统托盘已就绪");
    Ok(())
}

/// 托盘菜单、托盘点击与重复启动共用同一恢复路径。
///
/// 顺序固定为：抑制最小化检查 → 同步 tao 状态 → Win32 兜底恢复并抢焦点 →
/// 抢不到焦点则临时置顶重试并请求用户注意 → 到期撤销置顶。
pub fn restore_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        // 冷启动尚未创建窗口时，由正常启动流程显示主窗口。
        log::warn!("[恢复窗口] 未找到主窗口，可能仍处于冷启动阶段，恢复跳过");
        return;
    };
    begin_restore_suppression();
    let state = || {
        format_window_state(
            window.is_visible(),
            window.is_minimized(),
            window.is_focused(),
        )
    };
    log::info!("[恢复窗口] 开始，恢复前状态: {}", state());

    // 1. 同步 tao 内部标志，避免后续 tao 调用因标志陈旧而 NO-OP。
    //    这两步是异步投递的，结果不足以作为判定依据，真正的判定交给第 2 步。
    if let Err(error) = window.unminimize() {
        log::warn!("恢复已有窗口失败: {}", error);
    }
    if let Err(error) = window.show() {
        log::warn!("显示已有窗口失败: {}", error);
    }
    log::info!("[恢复窗口] tao 状态同步完成，当前状态: {}", state());

    // 2. Win32 兜底：同步还原最小化、显示隐藏窗口并抢前台焦点。
    let mut native = match native_restore_and_focus(&window) {
        Ok(result) => {
            log::info!("[恢复窗口] Win32 兜底完成，{}", result.describe());
            Some(result)
        }
        Err(error) => {
            log::warn!("[恢复窗口] Win32 兜底恢复失败: {}", error);
            None
        }
    };

    // 3. 前台焦点仍抢不到时短暂置顶重试，并让任务栏闪烁提示用户。
    let foreground_acquired = native.map(|result| result.foreground).unwrap_or(false)
        || window.is_focused().unwrap_or(false);
    if !foreground_acquired {
        log::warn!("[恢复窗口] 未能取得前台焦点，尝试临时置顶并请求用户注意");
        match native_set_topmost(&window, true) {
            Ok(()) => {
                schedule_topmost_release(window.clone());
                match native_restore_and_focus(&window) {
                    Ok(result) => {
                        log::info!("[恢复窗口] 置顶后重试抢焦点，{}", result.describe());
                        native = Some(result);
                    }
                    Err(error) => log::warn!("[恢复窗口] 置顶后重试抢焦点失败: {}", error),
                }
            }
            Err(error) => log::warn!("[恢复窗口] 临时置顶失败: {}", error),
        }
        if let Err(error) = window.request_user_attention(Some(UserAttentionType::Critical)) {
            log::warn!("[恢复窗口] 请求用户注意失败: {}", error);
        }
    }

    // 4. 用真实 HWND 状态判定结果，避免「日志显示成功但窗口没出来」。
    match native {
        Some(result) if result.is_restored() && result.foreground => {
            log::info!("[恢复窗口] 恢复成功，{}，{}", result.describe(), state());
        }
        Some(result) if result.is_restored() => {
            log::warn!(
                "[恢复窗口] 窗口已显示但未取得前台焦点，{}，{}",
                result.describe(),
                state()
            );
        }
        Some(result) => {
            log::warn!(
                "[恢复窗口] 恢复后窗口仍不可见或仍处于最小化，{}，{}",
                result.describe(),
                state()
            );
        }
        None => log::warn!("[恢复窗口] 结束，Win32 兜底未执行，{}", state()),
    }
}

#[tauri::command]
pub fn minimize_to_tray(window: tauri::Window) -> Result<(), String> {
    if window.label() == MAIN_WINDOW_LABEL && window.app_handle().tray_by_id(TRAY_ID).is_some() {
        match window.hide() {
            Ok(()) => {
                log::info!(
                    "[最小化到托盘] 命令触发 hide 成功，当前状态: {}",
                    format_window_state(
                        window.is_visible(),
                        window.is_minimized(),
                        window.is_focused()
                    )
                );
                return Ok(());
            }
            Err(error) => log::warn!("隐藏到托盘失败，将使用任务栏最小化: {}", error),
        }
    }
    window
        .minimize()
        .map_err(|error| format!("最小化失败: {}", error))
}

fn hide_minimized_window(window: &tauri::Window) {
    // 托盘创建失败时保留任务栏入口，避免隐藏后无法恢复。
    if window.app_handle().tray_by_id(TRAY_ID).is_none() {
        return;
    }
    // 恢复流程进行中（含恢复后短暂延迟）时读到的最小化状态可能是尚未提交的旧值，必须跳过，
    // 否则会把刚刚恢复显示的窗口立刻再次隐藏回托盘。
    if restore_suppression_active() {
        log::info!(
            "[最小化检查] 处于恢复抑制窗口内，跳过隐藏，当前状态: {}",
            format_window_state(
                window.is_visible(),
                window.is_minimized(),
                window.is_focused()
            )
        );
        return;
    }
    match window.is_minimized() {
        Ok(true) => {
            if let Err(error) = window.hide() {
                log::error!("最小化到托盘失败，窗口保留在任务栏: {}", error);
            } else {
                log::info!(
                    "主窗口已最小化到托盘，当前状态: {}",
                    format_window_state(
                        window.is_visible(),
                        window.is_minimized(),
                        window.is_focused()
                    )
                );
            }
        }
        Ok(false) => {
            log::info!(
                "[最小化检查] 窗口未处于最小化状态，跳过隐藏，当前状态: {}",
                format_window_state(
                    window.is_visible(),
                    window.is_minimized(),
                    window.is_focused()
                )
            );
        }
        Err(error) => log::warn!("读取窗口最小化状态失败: {}", error),
    }
}

pub fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL
        || !matches!(event, WindowEvent::Resized(_))
        || window.app_handle().tray_by_id(TRAY_ID).is_none()
        || restore_suppression_active()
        || MINIMIZE_CHECK_PENDING.swap(true, Ordering::AcqRel)
    {
        return;
    }

    // WM_SIZE 回调可能早于 Windows 提交最小化状态。从工作线程排队回主循环，
    // 在当前原生消息结束后检查；连续缩放仅保留一次待执行检查，不启动轮询。
    let dispatch_window = window.clone();
    tauri::async_runtime::spawn(async move {
        let check_window = dispatch_window.clone();
        if let Err(error) = dispatch_window.run_on_main_thread(move || {
            hide_minimized_window(&check_window);
            MINIMIZE_CHECK_PENDING.store(false, Ordering::Release);
        }) {
            MINIMIZE_CHECK_PENDING.store(false, Ordering::Release);
            log::warn!("安排窗口最小化检查失败: {}", error);
        }
    });
}
