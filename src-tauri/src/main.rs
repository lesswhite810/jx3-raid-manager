#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gkp_parser;
mod db;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            gkp_parser::parse_binary_gkp,
            db::db_init,
            db::db_get_migration_status,
            db::db_check_migration_completed,
            db::db_set_migration_status,
            db::db_get_accounts,
            db::db_save_accounts,
            db::db_get_records,
            db::db_save_records,
            db::db_get_raids,
            db::db_save_raids,
            db::db_get_config,
            db::db_save_config,
            db::db_get_config_debug,
            db::db_reset_config,
            db::db_add_record,
            db::db_delete_record,
            db::db_get_records_by_raid,
            db::db_analyze_duplicates,
            db::db_deduplicate_accounts,
            db::db_deduplicate_raids,
            db::db_add_unique_constraint_raids,
            db::db_backup,
            db::db_restore,
            db::db_get_cache,
            db::db_save_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
