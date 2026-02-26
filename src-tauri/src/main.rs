#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod gkp_parser;

use tauri_plugin_log::{LogTarget, RotationStrategy};

#[cfg(target_os = "windows")]
fn check_webview2() -> Result<String, String> {
    use std::process::Command;

    // 检查注册表中的 WebView2 版本
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            "/v",
            "pv",
        ])
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // 解析版本号
                let version = stdout.trim().split_whitespace().last().unwrap_or("unknown");
                Ok(format!("WebView2 已安装，版本: {}", version))
            } else {
                // 尝试查询用户级安装
                let output2 = Command::new("reg")
                    .args([
                        "query",
                        r"HKCU\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
                        "/v",
                        "pv",
                    ])
                    .output();

                if let Ok(output2) = output2 {
                    if output2.status.success() {
                        let stdout = String::from_utf8_lossy(&output2.stdout);
                        let version = stdout.trim().split_whitespace().last().unwrap_or("unknown");
                        return Ok(format!("WebView2 已安装（用户级），版本: {}", version));
                    }
                }
                Err("WebView2 未安装".to_string())
            }
        }
        Err(e) => Err(format!("检查 WebView2 失败: {}", e)),
    }
}

fn init_logging() -> tauri_plugin_log::Builder {
    tauri_plugin_log::Builder::default()
        .level_for("reqwest", log::LevelFilter::Info)
        .level_for("h2", log::LevelFilter::Info)
        .level_for("hyper", log::LevelFilter::Info)
        .targets([
            LogTarget::Stdout,
            LogTarget::Webview,
            LogTarget::Folder(db::get_app_dir().unwrap_or(std::path::PathBuf::from("."))),
        ])
        .rotation_strategy(RotationStrategy::KeepOne)
        .max_file_size(10 * 1024 * 1024)
}

fn main() {
    // 设置 panic hook
    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {:?}", info);
    }));

    // 先初始化日志
    let log_plugin = init_logging().build();

    // 检查 WebView2 并记录日志
    #[cfg(target_os = "windows")]
    {
        match check_webview2() {
            Ok(msg) => {
                log::info!("{}", msg);
                log::info!("WebView2 检查通过，应用将正常启动");
            }
            Err(msg) => {
                log::error!("{}", msg);
                log::error!("请安装 WebView2 运行时后重新启动应用");
                log::error!("下载链接: https://developer.microsoft.com/zh-cn/microsoft-edge/webview2/");
            }
        }
    }

    tauri::Builder::default()
        .plugin(log_plugin)
        .invoke_handler(tauri::generate_handler![
            gkp_parser::parse_binary_gkp,
            db::db_init,
            // 新的版本管理命令
            db::db_get_version_info,
            db::db_is_local_storage_migrated,
            db::db_set_local_storage_migrated,
            // 账号相关
            db::db_get_accounts,
            db::db_save_accounts,
            db::db_get_accounts_structured,
            db::db_get_accounts_with_roles,
            db::db_get_roles_by_account,
            db::db_get_all_roles,
            db::db_save_account_structured,
            db::db_save_role_structured,
            db::db_delete_account_structured,
            db::db_delete_role_structured,
            // 记录相关
            db::db_get_records,
            db::db_save_records,
            db::db_add_record,
            db::db_delete_record,
            db::db_get_records_by_raid,
            // 副本相关
            db::db_get_raids,
            db::db_save_raids,
            db::db_get_raid_versions,
            // 配置相关
            db::db_get_config,
            db::db_save_config,
            db::db_get_config_debug,
            db::db_reset_config,
            // 缓存相关
            db::db_get_cache,
            db::db_save_cache,
            // 装备相关
            db::db_save_equipments,
            db::db_get_equipments,
            // 试炼记录
            db::db_add_trial_record,
            db::db_get_trial_records,
            db::db_delete_trial_record,
            // 百战记录
            db::db_add_baizhan_record,
            db::db_get_baizhan_records,
            db::db_delete_baizhan_record,
            db::db_update_baizhan_record,
            // 备份恢复
            db::db_backup,
            db::db_restore,
            // 副本收藏
            db::db_get_favorite_raids,
            db::db_add_favorite_raid,
            db::db_remove_favorite_raid,
            db::db_is_favorite_raid,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
