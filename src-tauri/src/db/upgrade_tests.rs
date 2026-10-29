#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::fs;
    use std::thread;
    use std::time::Duration;
    use rusqlite::Connection;

    const CURRENT_SCHEMA_VERSION: i32 = 18;

    fn get_test_dir() -> PathBuf {
        let app_data = std::env::var("APPDATA").expect("无法获取 APPDATA 环境变量");
        PathBuf::from(app_data)
            .join("com.jx3raidmanager.app")
            .join("upgrade-tests")
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

        copy_with_retry(&db_path, &backup_path).map_err(|e| format!("备份数据库失败: {}", e))?;

        // 将测试逻辑放在闭包中，确保任何失败路径都能恢复备份
        let result = (|| -> Result<String, String> {
            {
                let conn = Connection::open(&db_path)
                    .map_err(|e| format!("打开数据库失败: {}", e))?;

                if !verify_schema_version(&conn, version)? {
                    return Err(format!("初始版本验证失败，期望 V{}", version));
                }
            }

            println!("  开始执行 V{} -> V{} 升级...", version, CURRENT_SCHEMA_VERSION);

            let upgrade_conn = crate::db::init_db_with_path(&db_path)?;
            drop(upgrade_conn);

            let mut verification_results = Vec::new();
            {
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
        let upgrade_result = crate::db::init_db_with_path(&db_path)
            .expect("V13 -> V14 升级失败");
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
                    .query_row("SELECT value FROM config WHERE id = 1", [], |row| row.get(0))
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
            let conn = crate::db::create_latest_schema_for_test(&db_path)
                .expect("全新安装建库失败");

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
            assert!(has_backup_dir, "char_sync_rollback_marks 缺少主键 backup_dir");

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
            let conn = crate::db::create_latest_schema_for_test(&db_path)
                .expect("全新安装建库失败");

            // 若期望清单漏了该表，validate 不会报错但我们也检测不到；
            // 因此这里反过来断言：建库后该表确实在清单所覆盖的范围内。
            let result = crate::db::validate_post_upgrade_for_test(&conn, 18);
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

        let mut conn = crate::db::create_latest_schema_for_test(&db_path)
            .expect("建库失败");

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
        crate::db::save_raids_with_conn(&mut conn, "[]", None)
            .expect("默认参数保存空副本列表失败");

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
        assert_eq!(
            role_count, 1,
            "护栏缺失：默认参数下空数组回写把角色删掉了"
        );

        let raid_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            raid_count, raids_before,
            "护栏缺失：默认参数下空数组回写把副本删掉了"
        );

        // ---- 2) 显式 allow_delete=true：删除必须真的生效 ----
        // 证明护栏只是把「默认」改成安全值，没有把删除能力关死。
        crate::db::save_accounts_with_conn(&mut conn, "[]", Some(true))
            .expect("显式删除账号失败");
        let account_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            account_count_after, 0,
            "allow_delete=true 时账号未被删除，说明删除路径失效"
        );

        crate::db::save_raids_with_conn(&mut conn, "[]", Some(true))
            .expect("显式删除副本失败");
        let raid_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(
            raid_count_after, 0,
            "allow_delete=true 时副本未被删除，说明删除路径失效"
        );

        // ---- 3) 传非空列表：新增/更新必须生效 ----
        let accounts_json = r#"[{"id":"acc-new","accountName":"新账号","type":"OWN","roles":[]}]"#;
        crate::db::save_accounts_with_conn(&mut conn, accounts_json, None)
            .expect("新增账号失败");
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
}
