use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::db;

/// 应用配置（key-value 存储的封装视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub game_directory: Option<String>,
    pub setup_completed: bool,
    pub last_scan_mingyi_at: Option<String>,
    pub account_ids: Vec<String>,
}

const KEY_GAME_DIRECTORY: &str = "game_directory";
const KEY_SETUP_COMPLETED: &str = "setup_completed";
const KEY_ACCOUNT_IDS: &str = "account_ids";
const KEY_LAST_SCAN_MINGYI_AT: &str = "last_scan_mingyi_at";

/// 从 app_config 表读取指定 key 的值
fn read_value(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();
    Ok(value)
}

/// 更新指定 key 的值（UPSERT 语义）
fn upsert_value(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, &now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 将字符串值转为 Option<String>，空字符串视为 None
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

/// 读取完整应用配置
pub fn get_app_config_internal() -> Result<AppConfig, String> {
    let conn = db::init_db()?;

    let game_directory = non_empty(read_value(&conn, KEY_GAME_DIRECTORY)?);
    let setup_completed_str = read_value(&conn, KEY_SETUP_COMPLETED)?.unwrap_or_default();
    let setup_completed = setup_completed_str.eq_ignore_ascii_case("true");
    let last_scan_mingyi_at = non_empty(read_value(&conn, KEY_LAST_SCAN_MINGYI_AT)?);

    let account_ids_str = read_value(&conn, KEY_ACCOUNT_IDS)?.unwrap_or_else(|| "[]".to_string());
    let account_ids: Vec<String> = serde_json::from_str(&account_ids_str)
        .unwrap_or_default();

    Ok(AppConfig {
        game_directory,
        setup_completed,
        last_scan_mingyi_at,
        account_ids,
    })
}

/// 获取应用配置（Tauri 命令）
#[tauri::command]
pub fn get_app_config() -> Result<AppConfig, String> {
    get_app_config_internal()
}

/// 设置游戏目录（Tauri 命令）
#[tauri::command]
pub fn set_game_directory(path: String) -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_GAME_DIRECTORY, &path)?;
    log::info!("[AppConfig] 游戏目录已更新: {}", path);
    Ok(())
}

/// 设置账号 ID 列表（Tauri 命令）
#[tauri::command]
pub fn set_account_ids(account_ids: Vec<String>) -> Result<(), String> {
    let conn = db::init_db()?;
    let value = serde_json::to_string(&account_ids).map_err(|e| e.to_string())?;
    upsert_value(&conn, KEY_ACCOUNT_IDS, &value)?;
    log::info!("[AppConfig] 账号 ID 列表已更新，共 {} 个账号", account_ids.len());
    Ok(())
}

/// 标记引导流程完成（Tauri 命令）
#[tauri::command]
pub fn complete_setup() -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_SETUP_COMPLETED, "true")?;
    log::info!("[AppConfig] 启动引导已完成");
    Ok(())
}

/// 重置配置，回到未引导状态（Tauri 命令）
#[tauri::command]
pub fn reset_setup() -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_GAME_DIRECTORY, "")?;
    upsert_value(&conn, KEY_SETUP_COMPLETED, "false")?;
    upsert_value(&conn, KEY_ACCOUNT_IDS, "[]")?;
    upsert_value(&conn, KEY_LAST_SCAN_MINGYI_AT, "")?;
    log::info!("[AppConfig] 应用配置已重置，将回到引导界面");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在临时文件中创建测试数据库
    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        let now = chrono::Local::now().to_rfc3339();
        for key in [
            KEY_GAME_DIRECTORY,
            KEY_SETUP_COMPLETED,
            KEY_ACCOUNT_IDS,
            KEY_LAST_SCAN_MINGYI_AT,
        ] {
            let default_value = match key {
                KEY_SETUP_COMPLETED => "false",
                KEY_ACCOUNT_IDS => "[]",
                _ => "",
            };
            conn.execute(
                "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![key, default_value, &now],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn test_read_write_roundtrip() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_GAME_DIRECTORY, "E:\\Game\\JX3").unwrap();
        let value = read_value(&conn, KEY_GAME_DIRECTORY).unwrap();
        assert_eq!(value, Some("E:\\Game\\JX3".to_string()));
    }

    #[test]
    fn test_non_empty_filters_empty_string() {
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(non_empty(Some("value".to_string())), Some("value".to_string()));
        assert_eq!(non_empty(None), None);
    }

    #[test]
    fn test_account_ids_serialization() {
        let ids = vec!["acc1".to_string(), "acc2".to_string()];
        let serialized = serde_json::to_string(&ids).unwrap();
        assert_eq!(serialized, r#"["acc1","acc2"]"#);
        let deserialized: Vec<String> = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, ids);
    }
}
