use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, WindowEvent,
};

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_ID: &str = "main-tray";
const RESTORE_MENU_ID: &str = "restore-main-window";
const EXIT_MENU_ID: &str = "exit-app";
static MINIMIZE_CHECK_PENDING: AtomicBool = AtomicBool::new(false);

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
            RESTORE_MENU_ID => restore_main_window(app),
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
                restore_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    log::info!("系统托盘已就绪");
    Ok(())
}

/// 托盘菜单、托盘点击与重复启动共用同一恢复路径。
pub fn restore_main_window(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        // 冷启动尚未创建窗口时，由正常启动流程显示主窗口。
        return;
    };
    if let Err(error) = window.unminimize() {
        log::warn!("恢复已有窗口失败: {}", error);
    }
    if let Err(error) = window.show() {
        log::warn!("显示已有窗口失败: {}", error);
    }
    if let Err(error) = window.set_focus() {
        log::warn!("聚焦已有窗口失败: {}", error);
    }
}

#[tauri::command]
pub fn minimize_to_tray(window: tauri::Window) -> Result<(), String> {
    if window.label() == MAIN_WINDOW_LABEL && window.app_handle().tray_by_id(TRAY_ID).is_some() {
        match window.hide() {
            Ok(()) => return Ok(()),
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
    match window.is_minimized() {
        Ok(true) => {
            if let Err(error) = window.hide() {
                log::error!("最小化到托盘失败，窗口保留在任务栏: {}", error);
            } else {
                log::info!("主窗口已最小化到托盘");
            }
        }
        Ok(false) => {}
        Err(error) => log::warn!("读取窗口最小化状态失败: {}", error),
    }
}

pub fn handle_window_event(window: &tauri::Window, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW_LABEL
        || !matches!(event, WindowEvent::Resized(_))
        || window.app_handle().tray_by_id(TRAY_ID).is_none()
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
