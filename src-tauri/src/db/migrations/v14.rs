use rusqlite::{params, Connection};

pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V14 迁移开始 ==========");
    log::info!("V14 迁移：创建 app_config 表（应用配置 key-value 存储）");

    let now = chrono::Local::now().to_rfc3339();

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params!["game_directory", "", &now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params!["setup_completed", "false", &now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params!["account_ids", "[]", &now],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
        params!["last_scan_mingyi_at", "", &now],
    )
    .map_err(|e| e.to_string())?;

    log::info!("V14 迁移：app_config 表创建完成，默认值已插入");
    log::info!("========== V14 迁移完成 ==========");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    fn get_config_value(conn: &Connection, key: &str) -> String {
        conn.query_row(
            "SELECT value FROM app_config WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn test_v14_creates_app_config_table() {
        let conn = setup_test_db();

        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='app_config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_exists);
    }

    #[test]
    fn test_v14_inserts_default_values() {
        let conn = setup_test_db();

        assert_eq!(get_config_value(&conn, "game_directory"), "");
        assert_eq!(get_config_value(&conn, "setup_completed"), "false");
        assert_eq!(get_config_value(&conn, "account_ids"), "[]");
        assert_eq!(get_config_value(&conn, "last_scan_mingyi_at"), "");
    }

    #[test]
    fn test_v14_idempotent() {
        let conn = setup_test_db();

        // 修改一个值
        conn.execute(
            "UPDATE app_config SET value = 'E:\\Game\\JX3' WHERE key = 'game_directory'",
            [],
        )
        .unwrap();

        // 再次运行迁移
        migrate(&conn).unwrap();

        // 已存在的值不应被覆盖
        assert_eq!(get_config_value(&conn, "game_directory"), "E:\\Game\\JX3");
    }

    #[test]
    fn test_v14_all_expected_keys_exist() {
        let conn = setup_test_db();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_config", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 4);

        let keys: Vec<String> = {
            let mut stmt = conn.prepare("SELECT key FROM app_config").unwrap();
            let rows = stmt
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap();
            rows.map(|r| r.unwrap()).collect()
        };

        assert!(keys.contains(&"game_directory".to_string()));
        assert!(keys.contains(&"setup_completed".to_string()));
        assert!(keys.contains(&"account_ids".to_string()));
        assert!(keys.contains(&"last_scan_mingyi_at".to_string()));
    }
}
