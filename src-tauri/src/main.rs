#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod equip_sync;
mod game_directory;
mod gkp_parser;
mod kungfu_data;
mod runtime_mode;
mod updater;

const PREPARE_INSTALL_DATA_FLAG: &str = "--prepare-install-data";

#[cfg(target_os = "windows")]
fn check_webview2() -> Result<String, String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    // 检查注册表中的 WebView2 版本
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}",
            "/v",
            "pv",
        ])
        .creation_flags(CREATE_NO_WINDOW)
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
                    .creation_flags(CREATE_NO_WINDOW)
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
    tauri_plugin_log::Builder::new()
        .level_for("reqwest", log::LevelFilter::Info)
        .level_for("h2", log::LevelFilter::Info)
        .level_for("hyper", log::LevelFilter::Info)
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Stdout,
        ))
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Webview,
        ))
        .target(tauri_plugin_log::Target::new(
            tauri_plugin_log::TargetKind::Folder {
                path: db::get_app_dir().unwrap_or(std::path::PathBuf::from(".")),
                file_name: Some("jx3-raid-manager.log".into()),
            },
        ))
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
        .max_file_size(10 * 1024 * 1024)
}

fn is_prepare_install_data_mode() -> bool {
    std::env::args()
        .skip(1)
        .any(|arg| arg == PREPARE_INSTALL_DATA_FLAG)
}

fn run_prepare_install_data() -> Result<(), String> {
    db::init_db().map(|_| ())
}

fn main() {
    if is_prepare_install_data_mode() {
        if let Err(error) = run_prepare_install_data() {
            eprintln!("准备安装版数据失败: {}", error);
            std::process::exit(1);
        }
        return;
    }

    // 设置 panic hook
    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {:?}", info);
    }));

    // 先初始化日志
    let log_plugin = init_logging().build();

    // 预先初始化数据库（确保表结构准备好）
    match db::init_db() {
        Ok(_) => log::info!("[INIT] 数据库预初始化完成"),
        Err(e) => log::error!("[INIT] 数据库预初始化失败: {}", e),
    }

    // 启动后台装备同步任务
    std::thread::spawn(|| {
        // 等待 2 秒让应用完全启动
        std::thread::sleep(std::time::Duration::from_secs(2));
        
        let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        rt.block_on(async {
            log::info!("[EquipSync] 开始后台装备同步...");
            match equip_sync::EquipSync::sync_if_needed().await {
                Ok(count) => {
                    if count > 0 {
                        log::info!("[EquipSync] 后台同步完成，共 {} 件装备", count);
                    } else {
                        log::info!("[EquipSync] 无需同步或已同步");
                    }
                }
                Err(e) => {
                    log::error!("[EquipSync] 后台同步失败: {}", e);
                }
            }
        });
    });

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
                log::error!(
                    "下载链接: https://developer.microsoft.com/zh-cn/microsoft-edge/webview2/"
                );
            }
        }
    }

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(log_plugin);

    let builder = if option_env!("TAURI_PUBLIC_KEY")
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        builder
    } else {
        builder.plugin(tauri_plugin_updater::Builder::new().build())
    };

    // MCP Bridge 插件仅在 debug 模式下启用
    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_mcp_bridge::init());

    builder
        .invoke_handler(tauri::generate_handler![
            gkp_parser::parse_binary_gkp,
            game_directory::scan_game_directory,
            game_directory::auto_parse_game_directory,
            game_directory::import_local_accounts,
            game_directory::analyze_roles,
            game_directory::validate_game_directory,
            game_directory::scan_jx3_clients,
            db::db_init,
            // 新的版本管理命令
            db::db_get_version_info,
            db::db_is_local_storage_migrated,
            db::db_set_local_storage_migrated,
            // 账号相关
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
            // 赛季相关
            db::db_get_game_versions,
            db::db_save_game_version,
            db::db_get_seasons,
            db::db_get_seasons_by_version,
            db::db_save_season,
            db::db_get_season_for_date,
            db::db_get_current_season,
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
            db::db_clear_equipments,
            equip_sync::equip_force_sync,
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
            db::db_delete_directory,
            // 副本收藏
            db::db_get_favorite_raids,
            db::db_add_favorite_raid,
            db::db_remove_favorite_raid,
            db::db_is_favorite_raid,
            // 角色可见性配置 (V5+)
            db::db_get_instance_types,
            db::db_get_all_role_visibility,
            db::db_save_role_visibility,
            // 团队副本角色可见性配置 (V6+)
            db::db_get_raid_role_visibility,
            db::db_save_raid_role_visibility,
            // 数据目录管理
            db::db_get_data_dir_info,
            db::db_set_custom_data_dir,
            db::db_reset_custom_data_dir,
            updater::updater_get_runtime_info,
            updater::updater_check,
            updater::updater_download_and_install,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
