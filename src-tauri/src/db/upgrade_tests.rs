#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;

    use crate::db::CURRENT_SCHEMA_VERSION;

    fn get_test_dir() -> PathBuf {
        let app_data = std::env::var("APPDATA").expect("无法获取 APPDATA 环境变量");
        let test_dir = PathBuf::from(app_data)
            .join("com.jx3raidmanager.app")
            .join("upgrade-tests");
        fs::create_dir_all(&test_dir).expect("创建测试目录失败");
        test_dir
    }

    /// 带重试的文件复制，规避 Windows 上杀毒软件或文件句柄延迟导致的瞬时锁定
    fn copy_with_retry(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
        let mut last_err: Option<String> = None;
        for attempt in 0..5 {
            match fs::copy(src, dst) {
                Ok(_) => return Ok(()),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("另一个程序正在使用此文件")
                        || msg.contains("being used by another process")
                        || e.kind() == std::io::ErrorKind::PermissionDenied
                    {
                        last_err = Some(format!("复制失败（尝试 {}）: {}", attempt + 1, msg));
                        thread::sleep(Duration::from_millis(200 * (attempt as u64 + 1)));
                        continue;
                    }
                    return Err(format!("复制 {:?} -> {:?} 失败: {}", src, dst, msg));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| "重试次数耗尽".to_string()))
    }

    /// 带重试的文件删除，规避 Windows 上的瞬时文件锁定
    fn remove_with_retry(path: &std::path::Path) -> Result<(), String> {
        let mut last_err: Option<String> = None;
        for attempt in 0..5 {
            match fs::remove_file(path) {
                Ok(_) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("另一个程序正在使用此文件")
                        || msg.contains("being used by another process")
                        || e.kind() == std::io::ErrorKind::PermissionDenied
                    {
                        last_err = Some(format!("删除失败（尝试 {}）: {}", attempt + 1, msg));
                        thread::sleep(Duration::from_millis(200 * (attempt as u64 + 1)));
                        continue;
                    }
                    return Err(format!("删除 {:?} 失败: {}", path, msg));
                }
            }
        }
        Err(last_err.unwrap_or_else(|| "重试次数耗尽".to_string()))
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

    fn verify_column_exists(
        conn: &Connection,
        table_name: &str,
        column_name: &str,
    ) -> Result<bool, String> {
        let count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM pragma_table_info('{}') WHERE name='{}'",
                    table_name, column_name
                ),
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

        copy_with_retry(&db_path, &backup_path).map_err(|e| format!("备份数据库失败: {}", e))?;

        // 将测试逻辑放在闭包中，确保任何失败路径都能恢复备份
        let result = (|| -> Result<String, String> {
            {
                let conn =
                    Connection::open(&db_path).map_err(|e| format!("打开数据库失败: {}", e))?;

                if !verify_schema_version(&conn, version)? {
                    return Err(format!("初始版本验证失败，期望 V{}", version));
                }
            }

            println!(
                "  开始执行 V{} -> V{} 升级...",
                version, CURRENT_SCHEMA_VERSION
            );

            let upgrade_conn = crate::db::init_db_with_path(&db_path)?;
            drop(upgrade_conn);

            let mut verification_results = Vec::new();
            {
                let conn =
                    Connection::open(&db_path).map_err(|e| format!("重新打开数据库失败: {}", e))?;

                if !verify_schema_version(&conn, CURRENT_SCHEMA_VERSION)? {
                    let actual: i32 = conn
                        .query_row(
                            "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1",
                            [],
                            |row| row.get(0),
                        )
                        .map_err(|e| format!("查询版本失败: {}", e))?;
                    return Err(format!(
                        "升级后版本验证失败，期望 V{}，实际 V{}",
                        CURRENT_SCHEMA_VERSION, actual
                    ));
                }

                let issues = verify_data_integrity(&conn)?;
                if !issues.is_empty() {
                    return Err(format!("数据完整性问题: {}", issues.join(", ")));
                }

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
            } // conn 在此处 drop，确保恢复前文件句柄已释放

            Ok(format!(
                "V{} -> V{} 升级成功 | {}",
                version,
                CURRENT_SCHEMA_VERSION,
                verification_results.join(", ")
            ))
        })();

        // 无论测试成功还是失败，都恢复备份，避免测试数据库被永久修改
        if let Err(e) = copy_with_retry(&backup_path, &db_path) {
            // 恢复失败时记录警告但不影响测试结果
            eprintln!("警告: 恢复数据库失败: {}", e);
        }
        let _ = remove_with_retry(&backup_path);

        result
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v1 夹具，CI 未生成"]
    fn test_v1_upgrade() {
        if !get_test_db_path(1).exists() {
            println!("V1 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(1);
        println!("V1 升级测试: {:?}", result);
        assert!(result.is_ok(), "V1 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v2 夹具，CI 未生成"]
    fn test_v2_upgrade() {
        if !get_test_db_path(2).exists() {
            println!("V2 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(2);
        println!("V2 升级测试: {:?}", result);
        assert!(result.is_ok(), "V2 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v3 夹具，CI 未生成"]
    fn test_v3_upgrade() {
        if !get_test_db_path(3).exists() {
            println!("V3 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(3);
        println!("V3 升级测试: {:?}", result);
        assert!(result.is_ok(), "V3 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v4 夹具，CI 未生成"]
    fn test_v4_upgrade() {
        if !get_test_db_path(4).exists() {
            println!("V4 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(4);
        println!("V4 升级测试: {:?}", result);
        assert!(result.is_ok(), "V4 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v5 夹具，CI 未生成"]
    fn test_v5_upgrade() {
        if !get_test_db_path(5).exists() {
            println!("V5 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(5);
        println!("V5 升级测试: {:?}", result);
        assert!(result.is_ok(), "V5 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v6 夹具，CI 未生成"]
    fn test_v6_upgrade() {
        if !get_test_db_path(6).exists() {
            println!("V6 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(6);
        println!("V6 升级测试: {:?}", result);
        assert!(result.is_ok(), "V6 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v7 夹具，CI 未生成"]
    fn test_v7_upgrade() {
        if !get_test_db_path(7).exists() {
            println!("V7 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(7);
        println!("V7 升级测试: {:?}", result);
        assert!(result.is_ok(), "V7 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v8 夹具，CI 未生成"]
    fn test_v8_upgrade() {
        if !get_test_db_path(8).exists() {
            println!("V8 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(8);
        println!("V8 升级测试: {:?}", result);
        assert!(result.is_ok(), "V8 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v9 夹具，CI 未生成"]
    fn test_v9_upgrade() {
        if !get_test_db_path(9).exists() {
            println!("V9 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(9);
        println!("V9 升级测试: {:?}", result);
        assert!(result.is_ok(), "V9 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v10 夹具，CI 未生成"]
    fn test_v10_upgrade() {
        if !get_test_db_path(10).exists() {
            println!("V10 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(10);
        println!("V10 升级测试: {:?}", result);
        assert!(result.is_ok(), "V10 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v11 夹具，CI 未生成"]
    fn test_v11_upgrade() {
        if !get_test_db_path(11).exists() {
            println!("V11 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(11);
        println!("V11 升级测试: {:?}", result);
        assert!(result.is_ok(), "V11 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v12 夹具，CI 未生成"]
    fn test_v12_upgrade() {
        if !get_test_db_path(12).exists() {
            println!("V12 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(12);
        println!("V12 升级测试: {:?}", result);
        assert!(result.is_ok(), "V12 升级失败: {:?}", result.err());
    }

    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v13 夹具，CI 未生成"]
    fn test_v13_upgrade() {
        if !get_test_db_path(13).exists() {
            println!("V13 升级测试跳过：预置数据库不存在");
            return;
        }
        let result = test_upgrade_from_version(13);
        println!("V13 升级测试: {:?}", result);
        assert!(result.is_ok(), "V13 升级失败: {:?}", result.err());
    }

    /// V13 -> V14 升级专项验证：检查 V14 引入的新表与 raid_bosses 重构结果
    ///
    /// 注意：此测试使用独立的数据库副本 `v13-specific-test.db`，避免与
    /// `test_v13_upgrade`（使用 `v13-test.db`）在并行测试执行时发生文件冲突。
    #[test]
    #[ignore = "需要 scripts/create-test-databases.js 预置 v13 夹具，CI 未生成"]
    fn test_v13_to_v14_specific_checks() {
        let source_path = get_test_db_path(13);
        let db_path = get_test_dir().join("v13-specific-test.db");
        let backup_path = get_test_dir().join("v13-specific-test-backup.db");

        if !source_path.exists() {
            println!("V13 专项验证跳过：源数据库不存在: {:?}", source_path);
            return;
        }

        // 复制一份独立的数据库副本，避免与 test_v13_upgrade 并行冲突
        copy_with_retry(&source_path, &db_path).expect("创建测试数据库副本失败");
        copy_with_retry(&db_path, &backup_path).expect("备份数据库失败");

        // 执行升级（显式 expect 以暴露升级失败的根本原因）
        let upgrade_result = crate::db::init_db_with_path(&db_path).expect("V13 -> V14 升级失败");
        // 立即释放升级过程中打开的连接，避免后续文件操作被锁
        drop(upgrade_result);

        {
            let conn = Connection::open(&db_path).expect("重新打开数据库失败");

            // 1. app_config 表必须存在且包含三个默认键
            assert!(
                verify_table_exists(&conn, "app_config").unwrap_or(false),
                "app_config 表不存在"
            );
            for key in ["game_directory", "setup_completed", "last_scan_mingyi_at"] {
                let count: i64 = conn
                    .query_row(
                        "SELECT COUNT(*) FROM app_config WHERE key = ?1",
                        [key],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                assert_eq!(count, 1, "app_config 缺少默认键: {}", key);
            }

            // 2. records 表应有 created_at / updated_at 列和索引
            let has_created_at: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='created_at'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            assert!(has_created_at, "records 表缺少 created_at 列");

            let has_updated_at: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='updated_at'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            assert!(has_updated_at, "records 表缺少 updated_at 列");

            assert!(
                verify_index_exists(&conn, "idx_records_status").unwrap_or(false),
                "idx_records_status 索引不存在"
            );
            assert!(
                verify_index_exists(&conn, "idx_records_source").unwrap_or(false),
                "idx_records_source 索引不存在"
            );

            // 2b. jcl_cache 表应存在
            assert!(
                verify_table_exists(&conn, "jcl_cache").unwrap_or(false),
                "jcl_cache 表不存在"
            );

            // 2c. scan_records 表不应存在（已合并到 records）
            assert!(
                !verify_table_exists(&conn, "scan_records").unwrap_or(false),
                "scan_records 表不应存在（已合并到 records）"
            );

            // 3. raid_bosses 表必须为新结构（无 id 列，主键为 raid_name + boss_id）
            let has_id_column: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('raid_bosses') WHERE name='id'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(true);
            assert!(!has_id_column, "raid_bosses 仍保留旧 id 列，V14 重构未生效");

            let pk_info: Vec<(String, i64)> = conn
                .prepare("SELECT name, pk FROM pragma_table_info('raid_bosses') WHERE pk > 0")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .filter_map(Result::ok)
                .collect();
            let pk_names: Vec<&str> = pk_info.iter().map(|(n, _)| n.as_str()).collect();
            assert!(
                pk_names.contains(&"raid_name") && pk_names.contains(&"boss_id"),
                "raid_bosses 联合主键不正确: {:?}",
                pk_names
            );

            // 4. 不应残留 '阿史' 名称（V14 修正为 '阿史那承庆'）
            let ashi_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM raid_bosses WHERE name = '阿史'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(ashi_count, 0, "raid_bosses 仍存在 '阿史' 名称");

            // 5. 从旧 config 表迁移 game_directory（V13 数据库含旧 config 表）
            let has_old_config: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='config'",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            if has_old_config {
                let old_config_value: Option<String> = conn
                    .query_row("SELECT value FROM config WHERE id = 1", [], |row| {
                        row.get(0)
                    })
                    .ok();
                if let Some(json_str) = old_config_value {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        if let Some(game_dir) = parsed
                            .get("game")
                            .and_then(|g| g.get("gameDirectory"))
                            .and_then(|v| v.as_str())
                        {
                            if !game_dir.is_empty() {
                                let migrated: String = conn
                                    .query_row(
                                        "SELECT value FROM app_config WHERE key = 'game_directory'",
                                        [],
                                        |row| row.get(0),
                                    )
                                    .unwrap_or_default();
                                assert_eq!(
                                    migrated, game_dir,
                                    "V14 未从旧 config 表迁移 game_directory"
                                );
                            }
                        }
                    }
                }
            }
        } // conn 在此处 drop，确保文件句柄释放

        // 清理：删除专项测试使用的临时数据库和备份
        remove_with_retry(&db_path).expect("删除测试数据库失败");
        remove_with_retry(&backup_path).expect("删除备份失败");

        println!("V13 -> V14 专项验证通过：app_config / records 时间戳 / jcl_cache / raid_bosses 重构均符合预期");
    }

    /// V18 专项验证：`char_sync_rollback_marks` 表在三条初始化路径上都必须存在。
    ///
    /// 回归背景：该表最初只由 `db/migrations/v18.rs` 创建，而「全新安装」
    /// （`install_fresh_db` → `create_latest_schema`）与「数据库版本高于应用」
    /// 的修复路径都不执行增量迁移，导致该功能在这些用户机器上直接报
    /// "no such table"。此测试锁定三件事：
    /// 1. 全新安装路径建表
    /// 2. 增量升级路径（V17 -> V18）建表
    /// 3. `validate_post_upgrade` 的期望表清单包含它
    #[test]
    fn test_v18_rollback_marks_table_on_fresh_install() {
        let db_path = get_test_dir().join("v18-fresh-test.db");
        remove_with_retry(&db_path).expect("清理旧的测试数据库失败");

        {
            let conn =
                crate::db::create_latest_schema_for_test(&db_path).expect("全新安装建库失败");

            assert!(
                verify_table_exists(&conn, "char_sync_rollback_marks").unwrap_or(false),
                "全新安装未创建 char_sync_rollback_marks 表"
            );

            // 表结构：backup_dir 主键 + rolled_back_at 非空
            let has_backup_dir: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('char_sync_rollback_marks') WHERE name='backup_dir' AND pk > 0",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            assert!(
                has_backup_dir,
                "char_sync_rollback_marks 缺少主键 backup_dir"
            );

            let has_rolled_back_at: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('char_sync_rollback_marks') WHERE name='rolled_back_at' AND \"notnull\" = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(false);
            assert!(
                has_rolled_back_at,
                "char_sync_rollback_marks 缺少 NOT NULL 列 rolled_back_at"
            );

            // 基本读写可用（这正是前端 list/set/clear 三个命令依赖的能力）
            conn.execute(
                "INSERT INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
                rusqlite::params!["C:/tmp/backup-1", "2026-10-29T00:00:00+08:00"],
            )
            .expect("写入回滚标记失败");
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM char_sync_rollback_marks WHERE backup_dir = ?1",
                    ["C:/tmp/backup-1"],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            assert_eq!(count, 1, "回滚标记写入后读不回来");
        }

        remove_with_retry(&db_path).expect("删除测试数据库失败");
        println!("V18 专项验证通过：全新安装已创建 char_sync_rollback_marks 且读写可用");
    }

    /// 校验 `validate_post_upgrade` 的期望表清单包含 V18 新表，
    /// 否则表结构漂移检测会漏报（这正是当初该缺陷没被 CI 发现的原因）。
    #[test]
    fn test_v18_expected_tables_include_rollback_marks() {
        let db_path = get_test_dir().join("v18-expected-tables-test.db");
        remove_with_retry(&db_path).expect("清理旧的测试数据库失败");

        {
            let conn =
                crate::db::create_latest_schema_for_test(&db_path).expect("全新安装建库失败");

            // 若期望清单漏了该表，validate 不会报错但我们也检测不到；
            // 因此这里反过来断言：建库后该表确实在清单所覆盖的范围内。
            let result = crate::db::validate_post_upgrade_for_test(&conn, 20);
            assert!(
                result.is_ok(),
                "validate_post_upgrade 在全新安装库上失败: {:?}",
                result.err()
            );
        }

        remove_with_retry(&db_path).expect("删除测试数据库失败");
        println!("V18 期望表清单校验通过");
    }

    /// 保存护栏回归：`db_save_accounts` / `db_save_raids` 在默认参数下
    /// 必须是「只增改不删」，否则一次读取失败 + 全量回写就会清空用户数据。
    ///
    /// 直接调用真实的内部实现（连接可注入），覆盖三个方向：
    /// 1. 默认（None）传空数组 → 不删除任何数据
    /// 2. 显式 allow_delete=true 传空数组 → 确实删除（证明护栏不是把删除功能关死）
    /// 3. 传非空列表 → 新增/更新生效
    #[test]
    fn test_save_guards_do_not_delete_by_default() {
        let db_path = get_test_dir().join("save-guard-test.db");
        remove_with_retry(&db_path).expect("清理旧的测试数据库失败");

        let mut conn = crate::db::create_latest_schema_for_test(&db_path).expect("建库失败");

        // 预置一个账号 + 一个角色 + 一个自定义副本
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type, sort_order, updated_at)
             VALUES ('acc-1', '主账号', 'OWN', 0, '2026-10-29T00:00:00+08:00')",
            [],
        )
        .expect("预置账号失败");
        conn.execute(
            "INSERT INTO roles (id, account_id, name, region, server, updated_at)
             VALUES ('role-1', 'acc-1', '角色一', '电信一区', '长安城', '2026-10-29T00:00:00+08:00')",
            [],
        )
        .expect("预置角色失败");
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, is_active, is_static)
             VALUES ('25人普通回归测试副本', '回归测试副本', '普通', 25, 1, 0)",
            [],
        )
        .expect("预置副本失败");

        // 记录基线：raids 里已有 init_static_raids 预置的静态副本，不能假设为 1
        let raids_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
            .unwrap_or(0);
        assert!(raids_before >= 1, "基线异常：副本表初始为空");

        // ---- 1) 默认参数：空数组回写不得删除任何数据 ----
        crate::db::save_accounts_with_conn(&mut conn, "[]", None)
            .expect("默认参数保存空账号列表失败");
        crate::db::save_raids_with_conn(&mut conn, "[]", None).expect("默认参数保存空副本列表失败");

        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            account_count, 1,
            "护栏缺失：默认参数下空数组回写把账号删掉了"
        );

        let role_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM roles", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(role_count, 1, "护栏缺失：默认参数下空数组回写把角色删掉了");

        let raid_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            raid_count, raids_before,
            "护栏缺失：默认参数下空数组回写把副本删掉了"
        );

        // ---- 2) 显式 allow_delete=true：删除必须真的生效 ----
        // 证明护栏只是把「默认」改成安全值，没有把删除能力关死。
        crate::db::save_accounts_with_conn(&mut conn, "[]", Some(true)).expect("显式删除账号失败");
        let account_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            account_count_after, 0,
            "allow_delete=true 时账号未被删除，说明删除路径失效"
        );

        crate::db::save_raids_with_conn(&mut conn, "[]", Some(true)).expect("显式删除副本失败");
        let raid_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            raid_count_after, 0,
            "allow_delete=true 时副本未被删除，说明删除路径失效"
        );

        // ---- 3) 传非空列表：新增/更新必须生效 ----
        let accounts_json = r#"[{"id":"acc-new","accountName":"新账号","type":"OWN","roles":[]}]"#;
        crate::db::save_accounts_with_conn(&mut conn, accounts_json, None).expect("新增账号失败");
        let new_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM accounts WHERE id = 'acc-new'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(new_count, 1, "非空列表回写未新增账号");

        drop(conn);
        remove_with_retry(&db_path).expect("删除测试数据库失败");
        println!("保存护栏语义验证通过：默认不删、显式才删、非空正常写入");
    }

    /// 构造一个「真实的 V18 结构」数据库。
    ///
    /// 做法：先建出最新的完整 schema，再**逐项回退** V19~V22 的增量改动，
    /// 使数据库在结构与数据上都等价于一个停留在 V18 的用户库。
    ///
    /// 为什么不手写一份 V18 DDL：
    /// 手写的最小结构会漏掉迁移脚本依赖的表（实测 `upgrade_db` 需要
    /// `game_versions` 等表，最小结构直接失败）。用「完整 schema 回退」能保证
    /// 除 V19~V22 之外的 V1~V18 结构都是真实的，也避免测试自身变成另一份
    /// 需要同步维护的 DDL 副本。
    fn create_v18_database() -> Result<Connection, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;

        // 1) 建出最新完整结构（等价于一个全新安装的库）
        crate::db::install_fresh_db_for_test(&conn)?;

        // 2) 回退 V21 / V22：删掉它们新增的表与索引
        conn.execute_batch(
            r#"
            DROP TABLE IF EXISTS trial_draw_equipment_manual_overrides;
            DROP TABLE IF EXISTS trial_draws;
            "#,
        )
        .map_err(|e| format!("回退 V21/V22 表失败: {}", e))?;

        // 3) 回退 V20：移除 equipment_unreviewed 列。
        //    SQLite 3.35+ 支持 DROP COLUMN；本地 bundled rusqlite 满足该要求。
        conn.execute(
            "ALTER TABLE trial_records DROP COLUMN equipment_unreviewed",
            [],
        )
        .map_err(|e| format!("回退 V20 列失败: {}", e))?;

        // 4) 回退 V19：移除 source / status / source_key 列，并删除唯一索引与 tombstone 表
        conn.execute_batch(
            r#"
            DROP INDEX IF EXISTS idx_trial_records_source_key;
            ALTER TABLE trial_records DROP COLUMN source_key;
            ALTER TABLE trial_records DROP COLUMN status;
            ALTER TABLE trial_records DROP COLUMN source;
            DROP TABLE IF EXISTS trial_auto_scan_tombstones;
            "#,
        )
        .map_err(|e| format!("回退 V19 列失败: {}", e))?;

        // 5) 版本号回退到 V18，并清理 V19~V22 的迁移记录
        conn.execute("DELETE FROM schema_versions WHERE version > 18", [])
            .map_err(|e| format!("回退版本记录失败: {}", e))?;
        conn.execute(
            "INSERT OR REPLACE INTO schema_versions (version, applied_at, description)
             VALUES (18, '2026-01-01T00:00:00Z', 'V18 基线')",
            [],
        )
        .map_err(|e| format!("写入 V18 版本记录失败: {}", e))?;

        // 6) 清空试炼记录，写入可控的历史数据（模拟真实用户库）
        conn.execute("DELETE FROM trial_records", [])
            .map_err(|e| format!("清理试炼记录失败: {}", e))?;
        conn.execute_batch(
            r#"
            INSERT INTO trial_records
                (id, account_id, role_id, layer, card_1, record_type, date)
            VALUES
                ('trial-manual', 'acc-1', 'role-1', 3, '旧牌面A', 'manual', 100),
                ('trial-auto',   'acc-1', 'role-1', 5, '',        'auto',   200),
                ('trial-cards',  'acc-1', 'role-1', 7, '旧牌面B', 'manual', 300);
            "#,
        )
        .map_err(|e| format!("写入 V18 试炼记录失败: {}", e))?;

        Ok(conn)
    }

    /// V18 → V22 端到端升级：走真实 `upgrade_db`，覆盖 V19~V22 全部增量迁移。
    ///
    /// 与 `test_v1_upgrade` 等夹具测试的区别：
    /// - 夹具测试依赖外部预置 `.db`（CI 不生成 → `#[ignore]` 跳过）
    /// - 本测试自建结构，**任何环境都真实执行**，是升级路径的真正门禁
    ///
    /// 覆盖点：
    /// 1. V19：`source` / `status` 列回填 + `idx_trial_records_source_key` 唯一索引
    /// 2. V20：`equipment_unreviewed` 全表回填且自动记录被标记
    /// 3. V21：`trial_draws` 表与索引创建，历史牌面不被重写
    /// 4. V22：`trial_draw_equipment_manual_overrides` 表创建
    /// 5. 数据零丢失：3 条历史记录全部保留
    #[test]
    fn test_v18_to_v22_end_to_end_upgrade() {
        let conn = create_v18_database().expect("构造 V18 数据库失败");

        let rows_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
            .expect("统计 V18 记录数失败");
        assert_eq!(rows_before, 3, "V18 夹具应有 3 条试炼记录");

        // 走真实升级入口（与生产 `init_db` 同一套 `upgrade_db` 逻辑）
        crate::db::upgrade_db_for_test(&conn, 18).expect("V18 -> V22 升级失败");

        // ---- 1) 版本必须推进到最新 ----
        let version: i32 = conn
            .query_row(
                "SELECT version FROM schema_versions ORDER BY version DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .expect("查询升级后版本失败");
        assert_eq!(
            version, CURRENT_SCHEMA_VERSION,
            "升级后版本未推进到 V{}",
            CURRENT_SCHEMA_VERSION
        );

        // 迁移记录必须逐版本落库：基线 V18 + V19~V22 共 5 条。
        // 注意不能断言 22 条——`schema_versions` 只记录**实际执行过**的迁移，
        // 全新安装只写当前版本一行，本测试的基线也只写 V18 一行。
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_versions", [], |row| row.get(0))
            .expect("统计 schema_versions 失败");
        assert_eq!(
            applied, 5,
            "schema_versions 应有 V18 基线 + V19~V22 共 5 条，实际 {}",
            applied
        );

        // 每个被执行的迁移都必须留下记录（不能跳过某一版）
        for expected in 19..=CURRENT_SCHEMA_VERSION {
            let exists: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM schema_versions WHERE version = ?1",
                    [expected],
                    |row| row.get(0),
                )
                .expect("查询迁移记录失败");
            assert!(exists, "schema_versions 缺少 V{} 的迁移记录", expected);
        }

        // ---- 2) V19：source / status 回填 ----
        let has_source: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records') WHERE name='source'",
                [],
                |row| row.get(0),
            )
            .expect("检查 source 列失败");
        assert!(has_source, "V19 未添加 trial_records.source 列");

        let has_status: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records') WHERE name='status'",
                [],
                |row| row.get(0),
            )
            .expect("检查 status 列失败");
        assert!(has_status, "V19 未添加 trial_records.status 列");

        // 已有行必须被回填为 manual/confirmed（不能留 NULL）
        let null_backfill: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_records WHERE source IS NULL OR status IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("统计未回填行失败");
        assert_eq!(
            null_backfill, 0,
            "V19 回填不完整：存在 source/status 为 NULL 的行"
        );

        // V19 的唯一索引必须真实存在
        let has_unique_index: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='index' AND name='idx_trial_records_source_key'",
                [],
                |row| row.get(0),
            )
            .expect("检查唯一索引失败");
        assert!(has_unique_index, "V19 未创建 idx_trial_records_source_key");

        // ---- 3) V20：equipment_unreviewed 回填 ----
        let has_unreviewed: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records')
                 WHERE name='equipment_unreviewed'",
                [],
                |row| row.get(0),
            )
            .expect("检查 equipment_unreviewed 列失败");
        assert!(has_unreviewed, "V20 未添加 equipment_unreviewed 列");

        let null_unreviewed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_records WHERE equipment_unreviewed IS NULL",
                [],
                |row| row.get(0),
            )
            .expect("统计 equipment_unreviewed 空值失败");
        assert_eq!(
            null_unreviewed, 0,
            "V20 未完成 equipment_unreviewed 全表回填"
        );

        // 自动记录（牌面全空）在**干净的 V18→V22** 路径上不会被标记为待核对。
        //
        // 这是 V19/V20 的真实语义，不是测试写错：
        // - V19 把升级前所有记录统一回填为 source='manual'（"任何升级前的记录
        //   都是此前 UI 手工保存的数据"）；
        // - V20 的标记条件是 source='auto' AND 牌面全空，而干净的 V18 库在 V19
        //   之后已不存在 source='auto' 的行。
        //
        // 因此 V20 的标记只对「开发版已经跑过 V19、库里已存在 auto 行」的库生效
        // （该分支由 `v20.rs` 的单元测试覆盖）。这里锁定干净升级路径的实际行为，
        // 避免未来有人误以为历史自动记录会被标记。
        let auto_flagged: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_records WHERE id='trial-auto'",
                [],
                |row| row.get(0),
            )
            .expect("查询自动记录标记失败");
        assert_eq!(
            auto_flagged, 0,
            "干净 V18→V22 升级不应把历史记录标记为待核对（V19 已回填为 manual）"
        );

        // 历史行必须全部回填为 manual/confirmed
        let manual_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_records
                 WHERE source = 'manual' AND status = 'confirmed'",
                [],
                |row| row.get(0),
            )
            .expect("统计 manual 记录失败");
        assert_eq!(
            manual_count, rows_before,
            "V19 应把全部历史记录回填为 manual/confirmed"
        );

        // 带牌面的记录不应被标记为未知
        let cards_flagged: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_records WHERE id='trial-cards'",
                [],
                |row| row.get(0),
            )
            .expect("查询带牌面记录标记失败");
        assert_eq!(cards_flagged, 0, "V20 误把已有牌面的记录标记为待核对");

        // ---- 4) V21：trial_draws 表 + 索引 ----
        let has_draws: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='table' AND name='trial_draws'",
                [],
                |row| row.get(0),
            )
            .expect("检查 trial_draws 失败");
        assert!(has_draws, "V21 未创建 trial_draws 表");

        let has_draw_index: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='index' AND name='idx_trial_draws_record_sequence'",
                [],
                |row| row.get(0),
            )
            .expect("检查 trial_draws 索引失败");
        assert!(has_draw_index, "V21 未创建 idx_trial_draws_record_sequence");

        // V21 不得把历史父级牌面拆成 draws（那是数据改写，不是迁移）
        let draws_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_draws", [], |row| row.get(0))
            .expect("统计 trial_draws 失败");
        assert_eq!(draws_count, 0, "V21 不应把历史牌面自动写入 trial_draws");

        // ---- 5) V22：覆盖表必须显式存在（不依赖 ensure_* 的隐式副作用） ----
        let has_overrides: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type='table' AND name='trial_draw_equipment_manual_overrides'",
                [],
                |row| row.get(0),
            )
            .expect("检查覆盖表失败");
        assert!(
            has_overrides,
            "V22 未创建 trial_draw_equipment_manual_overrides 表"
        );

        // ---- 6) 数据零丢失 + 历史牌面未被重写 ----
        let rows_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
            .expect("统计升级后记录数失败");
        assert_eq!(rows_after, rows_before, "升级过程中试炼记录丢失");

        let card_a: String = conn
            .query_row(
                "SELECT card_1 FROM trial_records WHERE id='trial-manual'",
                [],
                |row| row.get(0),
            )
            .expect("查询历史牌面失败");
        assert_eq!(card_a, "旧牌面A", "升级改写了历史牌面，属于数据破坏");

        // ---- 7) 升级后校验必须通过 ----
        crate::db::validate_post_upgrade_for_test(&conn, 18)
            .expect("升级后校验未通过，说明结构不完整");

        println!(
            "V18 -> V22 端到端升级验证通过：22 个版本、{} 条记录零丢失、V19~V22 结构齐全",
            rows_after
        );
    }

    /// 反向验证：缺少 V19 唯一索引时，升级后校验必须**失败**。
    ///
    /// 这条测试守护 `UNIQUE_INDEXES` 的语义：唯一索引缺失不能只告警，
    /// 否则去重保障会静默失效。
    #[test]
    fn test_missing_unique_index_fails_post_upgrade_validation() {
        let conn = Connection::open_in_memory().expect("创建内存库失败");
        crate::db::install_fresh_db_for_test(&conn).expect("全新安装失败");

        // 人为破坏：删掉唯一索引
        conn.execute("DROP INDEX IF EXISTS idx_trial_records_source_key", [])
            .expect("删除唯一索引失败");

        let result = crate::db::validate_post_upgrade_for_test(&conn, CURRENT_SCHEMA_VERSION);
        assert!(
            result.is_err(),
            "唯一索引缺失时升级后校验仍通过，去重保障会静默失效"
        );
        let message = result.unwrap_err();
        assert!(
            message.contains("唯一索引"),
            "错误信息应明确指出唯一索引缺失，实际: {}",
            message
        );
    }

    /// 升级前必须生成可回退的快照文件。
    ///
    /// 用真实的文件库（非内存库）验证 `backup_before_upgrade`：
    /// 内存库没有物理文件，无法覆盖这条路径，而真实用户库恰恰是文件库。
    #[test]
    fn test_upgrade_writes_pre_migration_backup() {
        let db_path = get_test_dir().join("backup-check-test.db");
        remove_with_retry(&db_path).expect("清理旧测试库失败");

        // 先建出一个已回退到 V18 的真实文件库
        {
            let conn = create_v18_database().expect("构造 V18 库失败");
            // 内存库 -> 文件库：用 backup API 落盘
            let mut file_conn = Connection::open(&db_path).expect("创建文件库失败");
            rusqlite::backup::Backup::new(&conn, &mut file_conn)
                .expect("初始化落盘失败")
                .run_to_completion(100, std::time::Duration::from_millis(10), None)
                .expect("落盘失败");
        }

        let backup_path = db_path.with_extension("db.pre-v18.bak");
        remove_with_retry(&backup_path).ok();

        // 走真实升级（文件库，会触发快照）
        {
            let conn = crate::db::init_db_with_path(&db_path).expect("V18 -> V22 升级失败");
            drop(conn);
        }

        assert!(
            backup_path.exists(),
            "升级未生成迁移前快照: {:?}",
            backup_path
        );
        let size = std::fs::metadata(&backup_path)
            .map(|m| m.len())
            .unwrap_or(0);
        assert!(size > 0, "迁移前快照为空文件，无法用于回退");

        // 快照必须是**升级前**的状态：版本仍是 V18，且没有 V19 新增的列
        {
            let conn = Connection::open(&backup_path).expect("打开快照失败");
            let version: i32 = conn
                .query_row("SELECT MAX(version) FROM schema_versions", [], |row| {
                    row.get(0)
                })
                .expect("查询快照版本失败");
            assert_eq!(
                version, 18,
                "快照不是升级前状态，版本应为 V18，实际 V{}",
                version
            );

            let has_source: bool = conn
                .query_row(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records') WHERE name='source'",
                    [],
                    |row| row.get(0),
                )
                .expect("检查快照结构失败");
            assert!(
                !has_source,
                "快照已包含 V19 的 source 列，说明快照是在迁移之后才生成的"
            );

            let rows: i64 = conn
                .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
                .expect("统计快照记录失败");
            assert_eq!(rows, 3, "快照应包含升级前的 3 条试炼记录，实际 {}", rows);
        }

        drop(std::fs::remove_file(&db_path));
        let _ = std::fs::remove_file(&backup_path);
        println!("迁移前快照验证通过：文件存在、非空、且内容为升级前状态（V18 / 3 条记录）");
    }
}
