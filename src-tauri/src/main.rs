#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod gkp_parser;

use tauri_plugin_log::{LogTarget, RotationStrategy,fern};

fn main() {
    // 设置 panic hook
    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic occurred: {:?}", info);
    }));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                // 过滤 reqwest 的 DEBUG 日志，只显示 INFO 及以上
                .level_for("reqwest", log::LevelFilter::Info)
                .level_for("h2", log::LevelFilter::Info)
                .level_for("hyper", log::LevelFilter::Info)
                .targets([
                    LogTarget::Stdout,
                    LogTarget::Webview,
                    // Log to custom folder: ~/.jx3-raid-manager/logs/
                    LogTarget::Folder(db::get_app_dir().unwrap_or(std::path::PathBuf::from("."))),
                ])
                .rotation_strategy(RotationStrategy::KeepOne)
                .max_file_size(10 * 1024 * 1024) // 10MB
                .build(),
        )
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
