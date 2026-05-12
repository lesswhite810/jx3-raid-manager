#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::fs;
    use rusqlite::Connection;

    const CURRENT_SCHEMA_VERSION: i32 = 13;

    fn get_test_dir() -> PathBuf {
        let app_data = std::env::var("APPDATA").expect("无法获取 APPDATA 环境变量");
        PathBuf::from(app_data)
            .join("com.jx3raidmanager.app")
            .join("upgrade-tests")
    }

    fn get_test_db_path(version: i32) -> PathBuf {
        get_test_dir().join(format!("v{}-test.db", version))
    }

    fn get_backup_db_path(version: i32) -> PathBuf {
        get_test_dir().join(format!("v{}-test-backup.db", version))
    }

    fn verify_schema_version(conn: &Connection, expected: i32) -> Result<bool, String> {
        let actual: i32 = conn
            .query_row(
                "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("查询版本失败: {}", e))?;
        
        Ok(actual == expected)
    }

    fn verify_table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                [table_name],
                |row| row.get(0),
            )
            .map_err(|e| format!("查询表失败: {}", e))?;
        
        Ok(count > 0)
    }

    fn verify_column_exists(conn: &Connection, table_name: &str, column_name: &str) -> Result<bool, String> {
        let count: i64 = conn
            .query_row(
                &format!("SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='{}'", table_name, column_name),
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("查询列失败: {}", e))?;
        
        Ok(count > 0)
    }

    fn verify_index_exists(conn: &Connection, index_name: &str) -> Result<bool, String> {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?",
                [index_name],
                |row| row.get(0),
            )
            .map_err(|e| format!("查询索引失败: {}", e))?;
        
        Ok(count > 0)
    }

    fn verify_data_integrity(conn: &Connection) -> Result<Vec<String>, String> {
        let mut issues = Vec::new();
        
        if verify_table_exists(conn, "accounts")? {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
                .map_err(|e| format!("查询 accounts 失败: {}", e))?;
            if count == 0 {
                issues.push("accounts 表为空".to_string());
            }
        } else {
            issues.push("accounts 表不存在".to_string());
        }
        
        if verify_table_exists(conn, "roles")? {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM roles", [], |row| row.get(0))
                .map_err(|e| format!("查询 roles 失败: {}", e))?;
            if count == 0 {
                issues.push("roles 表为空".to_string());
            }
        } else {
            issues.push("roles 表不存在".to_string());
        }
        
        if !verify_table_exists(conn, "instance_types")? {
            issues.push("instance_types 表不存在".to_string());
        }
        
        if !verify_table_exists(conn, "game_versions")? {
            issues.push("game_versions 表不存在".to_string());
        }
        
        if !verify_table_exists(conn, "seasons")? {
            issues.push("seasons 表不存在".to_string());
        }
        
        Ok(issues)
    }

    fn test_upgrade_from_version(version: i32) -> Result<String, String> {
        let db_path = get_test_db_path(version);
        let backup_path = get_backup_db_path(version);
        
        if !db_path.exists() {
            return Err(format!("测试数据库不存在: {:?}", db_path));
        }
        
        fs::copy(&db_path, &backup_path)
            .map_err(|e| format!("备份数据库失败: {}", e))?;
        
        {
            let conn = Connection::open(&db_path)
                .map_err(|e| format!("打开数据库失败: {}", e))?;
            
            if !verify_schema_version(&conn, version)? {
                return Err(format!("初始版本验证失败，期望 V{}", version));
            }
        }
        
        println!("  开始执行 V{} -> V{} 升级...", version, CURRENT_SCHEMA_VERSION);
        
        crate::db::init_db_with_path(&db_path)?;
        
        let conn = Connection::open(&db_path)
            .map_err(|e| format!("重新打开数据库失败: {}", e))?;
        
        if !verify_schema_version(&conn, CURRENT_SCHEMA_VERSION)? {
            let actual: i32 = conn
                .query_row(
                    "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .map_err(|e| format!("查询版本失败: {}", e))?;
            return Err(format!("升级后版本验证失败，期望 V{}，实际 V{}", CURRENT_SCHEMA_VERSION, actual));
        }
        
        let issues = verify_data_integrity(&conn)?;
        if !issues.is_empty() {
            return Err(format!("数据完整性问题: {}", issues.join(", ")));
        }
        
        let mut verification_results = Vec::new();
        
        if verify_column_exists(&conn, "accounts", "sort_order")? {
            verification_results.push("accounts.sort_order 存在".to_string());
        } else {
            verification_results.push("accounts.sort_order 缺失".to_string());
        }
        
        if verify_column_exists(&conn, "roles", "martial")? {
            verification_results.push("roles.martial 存在".to_string());
        } else {
            verification_results.push("roles.martial 缺失".to_string());
        }
        
        if verify_column_exists(&conn, "raids", "season_id")? {
            verification_results.push("raids.season_id 存在".to_string());
        } else {
            verification_results.push("raids.season_id 缺失".to_string());
        }
        
        if verify_column_exists(&conn, "records", "raid_name")? {
            verification_results.push("records.raid_name 存在".to_string());
        } else {
            verification_results.push("records.raid_name 缺失".to_string());
        }
        
        if verify_index_exists(&conn, "idx_accounts_sort_order")? {
            verification_results.push("idx_accounts_sort_order 存在".to_string());
        }
        
        fs::copy(&backup_path, &db_path)
            .map_err(|e| format!("恢复数据库失败: {}", e))?;
        fs::remove_file(&backup_path)
            .map_err(|e| format!("删除备份失败: {}", e))?;
        
        Ok(format!("V{} -> V{} 升级成功 | {}", version, CURRENT_SCHEMA_VERSION, verification_results.join(", ")))
    }

    #[test]
    fn test_v1_upgrade() {
        let result = test_upgrade_from_version(1);
        println!("V1 升级测试: {:?}", result);
        assert!(result.is_ok(), "V1 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v2_upgrade() {
        let result = test_upgrade_from_version(2);
        println!("V2 升级测试: {:?}", result);
        assert!(result.is_ok(), "V2 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v3_upgrade() {
        let result = test_upgrade_from_version(3);
        println!("V3 升级测试: {:?}", result);
        assert!(result.is_ok(), "V3 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v4_upgrade() {
        let result = test_upgrade_from_version(4);
        println!("V4 升级测试: {:?}", result);
        assert!(result.is_ok(), "V4 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v5_upgrade() {
        let result = test_upgrade_from_version(5);
        println!("V5 升级测试: {:?}", result);
        assert!(result.is_ok(), "V5 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v6_upgrade() {
        let result = test_upgrade_from_version(6);
        println!("V6 升级测试: {:?}", result);
        assert!(result.is_ok(), "V6 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v7_upgrade() {
        let result = test_upgrade_from_version(7);
        println!("V7 升级测试: {:?}", result);
        assert!(result.is_ok(), "V7 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v8_upgrade() {
        let result = test_upgrade_from_version(8);
        println!("V8 升级测试: {:?}", result);
        assert!(result.is_ok(), "V8 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v9_upgrade() {
        let result = test_upgrade_from_version(9);
        println!("V9 升级测试: {:?}", result);
        assert!(result.is_ok(), "V9 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v10_upgrade() {
        let result = test_upgrade_from_version(10);
        println!("V10 升级测试: {:?}", result);
        assert!(result.is_ok(), "V10 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v11_upgrade() {
        let result = test_upgrade_from_version(11);
        println!("V11 升级测试: {:?}", result);
        assert!(result.is_ok(), "V11 升级失败: {:?}", result.err());
    }

    #[test]
    fn test_v12_upgrade() {
        let result = test_upgrade_from_version(12);
        println!("V12 升级测试: {:?}", result);
        assert!(result.is_ok(), "V12 升级失败: {:?}", result.err());
    }
}
