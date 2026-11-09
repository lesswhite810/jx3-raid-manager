use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, UserAttentionType,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const RESTORE_MENU_ID: &str = "restore-main-window";
const EXIT_MENU_ID: &str = "exit-app";

/// 抢前台焦点失败时临时置顶的持续时间。
///
/// 置顶仅用于把窗口带到游戏之上，到期必须撤销：常驻置顶会遮挡游戏客户区，
/// 导致试炼采集因 `client_area_is_unobscured()` 不成立而停止采帧。
const TEMPORARY_TOPMOST_MILLIS: u64 = 1200;

/// 真实 HWND 状态，用于区分「tao 内部标志已更新」与「窗口真的显示出来了」。
#[derive(Debug, Clone, Copy)]
struct NativeWindowState {
    visible: bool,
    minimized: bool,
    maximized: bool,
    foreground: bool,
}

impl NativeWindowState {
    fn describe(&self) -> String {
        format!(
            "native(visible={} minimized={} maximized={} foreground={})",
            self.visible, self.minimized, self.maximized, self.foreground
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

/// 读取主窗口当前是否处于最大化。
///
/// 必须在调用 tao 的 `show()` 之前读取：一旦窗口被还原成普通尺寸，再读就只能看到
/// 「没最大化」，无法还原用户原本的窗口形态。
#[cfg(windows)]
fn native_is_maximized(window: &tauri::WebviewWindow) -> Option<bool> {
    use windows_sys::Win32::UI::WindowsAndMessaging::IsZoomed;

    let hwnd = window.hwnd().ok()?.0;
    if hwnd.is_null() {
        return None;
    }
    // SAFETY: hwnd 取自当前进程持有的主窗口，IsZoomed 只读取窗口样式位，不改变任何状态。
    Some(unsafe { IsZoomed(hwnd) } != 0)
}

#[cfg(not(windows))]
fn native_is_maximized(_window: &tauri::WebviewWindow) -> Option<bool> {
    None
}

/// Win32 兜底恢复：同步完成「还原最小化 + 显示隐藏窗口 + 抢前台焦点」。
///
/// 之所以不只用 tauri/tao 的 `unminimize()` + `show()`：
/// - `show()` 底层是 `SW_SHOW`，对最小化窗口不会还原；
/// - tao 的 `set_visible` / `set_minimized` 都是通过 `execute_in_thread` 异步投递，
///   返回时窗口状态可能尚未改变，紧接着的检查会读到旧状态；
/// - tao 的 `set_focus()` 读内部 `WindowFlags` 而非真实 HWND，标志陈旧会整段 NO-OP。
///
/// `prefer_maximized` 是调用方在显示窗口之前读到的最大化状态。Win32 语义里
/// `SW_RESTORE` 对「处于最大化」的窗口等于「还原」，所以这里不能一律用 `SW_RESTORE`：
/// 否则「最大化 → 收进托盘 → 托盘恢复」会让窗口掉回配置文件里的默认尺寸。
#[cfg(windows)]
fn native_restore_and_focus(
    window: &tauri::WebviewWindow,
    prefer_maximized: bool,
) -> Result<NativeWindowState, String> {
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AllowSetForegroundWindow, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, IsZoomed, SetForegroundWindow, SetWindowPos, ShowWindow, ASFW_ANY,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_RESTORE, SW_SHOWMAXIMIZED,
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
        // 同时覆盖「已最小化」「被 SW_HIDE 隐藏」「仍处于最大化」三种状态：
        // - SW_SHOWMAXIMIZED：显示并保持最大化，不会把最大化窗口还原成普通尺寸；
        // - SW_RESTORE：显示并还原到普通尺寸，仅在窗口原本不是最大化时使用。
        // IsZoomed 作为兜底，覆盖调用方读取状态与实际显示之间窗口又被换过形态的情况。
        let keep_maximized = prefer_maximized || IsZoomed(hwnd) != 0;
        ShowWindow(
            hwnd,
            if keep_maximized {
                SW_SHOWMAXIMIZED
            } else {
                SW_RESTORE
            },
        );
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
            maximized: IsZoomed(hwnd) != 0,
            foreground: GetForegroundWindow() == hwnd,
        })
    }
}

#[cfg(not(windows))]
fn native_restore_and_focus(
    _window: &tauri::WebviewWindow,
    _prefer_maximized: bool,
) -> Result<NativeWindowState, String> {
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
/// 顺序固定为：记录最大化状态 → 同步 tao 状态 → Win32 兜底恢复并抢焦点 →
/// 抢不到焦点则临时置顶重试并请求用户注意 → 到期撤销置顶。
pub fn restore_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        // 冷启动尚未创建窗口时，由正常启动流程显示主窗口。
        log::warn!("[恢复窗口] 未找到主窗口，可能仍处于冷启动阶段，恢复跳过");
        return;
    };
    let state = || {
        format_window_state(
            window.is_visible(),
            window.is_minimized(),
            window.is_focused(),
        )
    };
    // 必须在 show() 之前读取：窗口一旦被还原成普通尺寸，就无法判断用户原本是否最大化。
    // 两个来源要取「或」而不是「先真后假」：窗口被隐藏、或被 hide 前的 unminimize 清掉
    // WS_MAXIMIZE 之后，IsZoomed 会返回 false，此时只有 tao 内部标志还留着最大化意图。
    let restore_maximized = native_is_maximized(&window).unwrap_or(false)
        || window.is_maximized().unwrap_or(false);
    log::info!(
        "[恢复窗口] 开始，恢复前状态: {} maximized={}",
        state(),
        restore_maximized
    );

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
    let mut native = match native_restore_and_focus(&window, restore_maximized) {
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
                match native_restore_and_focus(&window, restore_maximized) {
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

/// 最小化主窗口到任务栏（Tauri 命令）。
///
/// 最小化按钮的语义是「普通最小化」：窗口收进任务栏，进程与后台扫描继续运行。
/// 只有关闭按钮才会按用户选择的关闭行为决定是否隐藏到托盘。
#[tauri::command]
pub fn minimize_main_window(window: tauri::Window) -> Result<(), String> {
    window
        .minimize()
        .map_err(|error| format!("最小化失败: {}", error))?;
    log::info!(
        "[最小化] 窗口已最小化到任务栏，当前状态: {}",
        format_window_state(
            window.is_visible(),
            window.is_minimized(),
            window.is_focused()
        )
    );
    Ok(())
}

/// 隐藏主窗口到系统托盘（Tauri 命令）。
///
/// 仅在用户明确选择「最小化到托盘」时调用；托盘创建失败时保持窗口可见，
/// 否则隐藏后用户将没有任何恢复入口。
#[tauri::command]
pub fn hide_main_window(window: tauri::Window) -> Result<(), String> {
    if window.app_handle().tray_by_id(TRAY_ID).is_none() {
        log::warn!("[最小化到托盘] 系统托盘不可用，保持窗口在任务栏");
        return window
            .minimize()
            .map_err(|error| format!("最小化失败: {}", error));
    }
    // hide() 走 tao 的 VISIBLE 分支时，若窗口仍带着最小化位，tao 会刻意跳过
    // WS_VISIBLE 样式写入，只执行 ShowWindow(SW_HIDE)，HWND 会停在「隐藏 + 仍最小化」
    // 的组合态；这种状态恢复时 DWM 只能提交残帧，表现为「点托盘但窗口出不来画面」。
    // 仍可能走到这里的场景：窗口先被最小化到任务栏，用户再从任务栏缩略图关闭它。
    // 先清掉最小化位，让窗口回到正常的隐藏窗口状态；正常可见状态下这两步都是空操作。
    if let Err(error) = window.unminimize() {
        log::warn!("[最小化到托盘] 清除最小化状态失败: {}", error);
    }
    window
        .hide()
        .map_err(|error| format!("隐藏到托盘失败: {}", error))?;
    log::info!(
        "[最小化到托盘] 窗口已隐藏到托盘，当前状态: {}",
        format_window_state(
            window.is_visible(),
            window.is_minimized(),
            window.is_focused()
        )
    );
    Ok(())
}

/// 退出应用（Tauri 命令）。
///
/// 与托盘菜单「退出应用」共用同一条退出路径，确保清理逻辑一致。
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    log::info!("[退出] 收到退出请求，应用即将结束");
    app.exit(0);
}
