use rusqlite::{params, Connection};
use serde_json::Value;

/// V14 迁移：app_config 合并 + records 时间戳 + raid_bosses 重构
///
/// 本迁移合并了原本独立的三个迁移（在 v2.1.41 之后引入，均未发布）：
///
/// 1. **app_config 表（统一配置存储）**：
///    - 将旧 `config` 表的 JSON blob 迁移到 `app_config.config_json` 键
///    - 将旧 `migration_flags` 表的 `local_storage_migrated` 迁移到 `app_config`
///    - 默认 game_directory / setup_completed / last_scan_mingyi_at / config_json /
///      local_storage_migrated 五个键
///    - 迁移完成后删除 `config` 和 `migration_flags` 表，消除四表重叠问题
///
/// 2. **records 表增强**：添加 created_at / updated_at 时间戳列，
///    使 records 表能同时承载手工记录（status='confirmed'）和自动扫描待确认记录
///    （status='pending'/'scanning'，source='auto_scan'），消除 scan_records 冗余表。
///    同时创建 jcl_cache 表（JCL 解析缓存，跨会话复用）。
///
/// 3. **raid_bosses 表重构**：采用联合主键 (raid_name, boss_id)
///    - raid_name 列统一存 raids.name（副本基础名），同名副本天然共享 BOSS 记录
///    - 修正"阿史"→"阿史那承庆"（与 static_raids.json 对齐）
///    - 修复旧表 raid_name 列语义混乱（V9 写 raids.name，前端保存写 raids.id 且
///      ON CONFLICT 覆盖 V9 数据）导致的 BOSS 匹配失效问题
///
/// 4. **废弃 raid_versions 表**：与 game_versions 表完全冗余
///    - 两表 name 列和 id 值完全一致
///    - game_versions 有 sort_order 字段，功能等价且更完整
///    - 迁移完成后删除 raid_versions 表
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V14 迁移开始 ==========");

    create_app_config_table(conn)?;
    add_records_timestamps(conn)?;
    create_jcl_cache_table(conn)?;
    rebuild_raid_bosses_table(conn)?;
    drop_raid_versions_table(conn)?;

    log::info!("========== V14 迁移完成 ==========");
    Ok(())
}

/// 1. 创建 app_config 表并插入默认配置
///
/// 合并旧 config 表的 JSON blob 到 `app_config.config_json` 键，
/// 合并旧 migration_flags 表的 `local_storage_migrated` 到 `app_config`，
/// 然后删除 config 和 migration_flags 表，统一配置存储。
fn create_app_config_table(conn: &Connection) -> Result<(), String> {
    log::info!("V14 迁移：创建 app_config 表（统一配置 key-value 存储）");

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

    // 插入默认键值
    for (key, default_value) in [
        ("game_directory", ""),
        ("setup_completed", "false"),
        ("last_scan_mingyi_at", ""),
        ("config_json", ""),
        ("local_storage_migrated", "false"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, default_value, &now],
        )
        .map_err(|e| e.to_string())?;
    }

    // 从旧 config 表迁移 game_directory 和完整 JSON blob
    migrate_config_from_old_config(conn, &now)?;

    // 从旧 migration_flags 表迁移 local_storage_migrated
    migrate_migration_flags(conn, &now)?;

    log::info!("V14 迁移：app_config 表创建完成，配置已统一");
    Ok(())
}

/// 从旧 config 表迁移到 app_config：
/// - game_directory：从 config.game.gameDirectory 提取到独立键
/// - config_json：完整 JSON blob 迁移到 app_config.config_json 键
/// 迁移完成后删除旧 config 表
fn migrate_config_from_old_config(conn: &Connection, now: &str) -> Result<(), String> {
    let config_table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='config'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if !config_table_exists {
        return Ok(());
    }

    let old_value: Option<String> = conn
        .query_row("SELECT value FROM config WHERE id = 1", [], |row| row.get(0))
        .ok();

    let Some(json_str) = old_value else {
        // config 表存在但无数据，删除空表
        conn.execute("DROP TABLE IF EXISTS config", [])
            .map_err(|e| e.to_string())?;
        return Ok(());
    };

    // 迁移完整 JSON blob 到 config_json 键
    conn.execute(
        "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'config_json' AND value = ''",
        params![json_str, now],
    )
    .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：从旧 config 表迁移完整 JSON blob 到 app_config.config_json");

    // 从 JSON 中提取 game_directory 到独立键
    let parsed: Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => {
            // JSON 解析失败，仍然删除旧表
            conn.execute("DROP TABLE IF EXISTS config", [])
                .map_err(|e| e.to_string())?;
            return Ok(());
        }
    };

    let game_dir = parsed
        .get("game")
        .and_then(|g| g.get("gameDirectory"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !game_dir.is_empty() {
        conn.execute(
            "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'game_directory' AND value = ''",
            params![game_dir, now],
        )
        .map_err(|e| e.to_string())?;
        log::info!("V14 迁移：从旧 config 表迁移 game_directory={}", game_dir);
    }

    // 删除旧 config 表
    conn.execute("DROP TABLE IF EXISTS config", [])
        .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：已删除旧 config 表，配置统一到 app_config");

    Ok(())
}

/// 从旧 migration_flags 表迁移 local_storage_migrated 到 app_config
/// 迁移完成后删除旧 migration_flags 表
fn migrate_migration_flags(conn: &Connection, now: &str) -> Result<(), String> {
    let flags_table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='migration_flags'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if !flags_table_exists {
        return Ok(());
    }

    // 读取旧 migration_flags 中的 local_storage_migrated 值
    let old_value: Option<String> = conn
        .query_row(
            "SELECT value FROM migration_flags WHERE key = 'local_storage_migrated'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(val) = old_value {
        if val == "true" {
            conn.execute(
                "UPDATE app_config SET value = 'true', updated_at = ?1 WHERE key = 'local_storage_migrated'",
                params![now],
            )
            .map_err(|e| e.to_string())?;
            log::info!("V14 迁移：从旧 migration_flags 表迁移 local_storage_migrated=true");
        }
    }

    // 删除旧 migration_flags 表
    conn.execute("DROP TABLE IF EXISTS migration_flags", [])
        .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：已删除旧 migration_flags 表，标记统一到 app_config");

    Ok(())
}

/// 2. 为 records 表添加 created_at / updated_at 时间戳列及 source / status / drops / jcl_files 列
///
/// records 表通过 source (default 'manual') 和 status (default 'confirmed') 列
/// 区分手工记录和自动扫描记录。添加时间戳后可同时承载两种记录类型，
/// 无需独立的 scan_records 表。
/// 同时为 status 和 source 列创建索引，加速常用查询。
fn add_records_timestamps(conn: &Connection) -> Result<(), String> {
    log::info!("V14 迁移：为 records 表添加时间戳和扫描相关列");

    // 需要添加的列及其类型（如果不存在）
    let columns_to_add = [
        ("source", "TEXT DEFAULT 'manual'"),
        ("status", "TEXT DEFAULT 'confirmed'"),
        ("drops", "TEXT"),
        ("jcl_files", "TEXT"),
        ("created_at", "TEXT"),
        ("updated_at", "TEXT"),
    ];

    for (col_name, col_type) in &columns_to_add {
        let has_col: bool = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='{}'",
                    col_name
                ),
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if !has_col {
            conn.execute(
                &format!("ALTER TABLE records ADD COLUMN {} {}", col_name, col_type),
                [],
            )
            .map_err(|e| e.to_string())?;
            log::info!("V14 迁移：已添加 records.{} 列", col_name);
        }
    }

    // 为 status 和 source 列创建索引（加速 pending/confirmed 和 auto_scan/manual 过滤）
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_records_status ON records(status);
        CREATE INDEX IF NOT EXISTS idx_records_source ON records(source);
        "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("V14 迁移：records 表时间戳列和索引已就绪");
    Ok(())
}

/// 2b. 创建 jcl_cache 表（JCL 解析缓存，跨会话复用）
///
/// 独立创建，语义上属于缓存层而非扫描记录层。
fn create_jcl_cache_table(conn: &Connection) -> Result<(), String> {
    log::info!("V14 迁移：创建 jcl_cache 表（JCL 解析缓存）");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS jcl_cache (
            file_path TEXT PRIMARY KEY,
            file_mtime INTEGER NOT NULL,
            boss_name TEXT,
            fight_start_ms INTEGER NOT NULL,
            fight_end_ms INTEGER NOT NULL,
            is_kill INTEGER NOT NULL,
            cached_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("V14 迁移：jcl_cache 表创建完成");
    Ok(())
}

/// 3. 重构 raid_bosses 表
///
/// 旧表主键为 boss_id（id 列），raid_name 列语义混乱——V9 迁移写入 raids.name
/// （如"阆风悬城"），前端 db_save_raids 写入 raids.id（如"25人普通阆风悬城"），
/// 且因 boss_id 相同触发 ON CONFLICT UPDATE，覆盖 V9 写入的共享数据。
///
/// 新表主键改为 (raid_name, boss_id) 联合主键，raid_name 列统一存 raids.name，
/// 同名副本天然共享同一组 BOSS 记录。
fn rebuild_raid_bosses_table(conn: &Connection) -> Result<(), String> {
    log::info!("V14 迁移：重构 raid_bosses 表结构（联合主键 raid_name+boss_id，raid_name 统一为 raids.name）");

    // 检查是否已完成迁移（raid_bosses_new 表已存在说明历史迁移未清理干净，
    // 或当前 raid_bosses 已经是新结构——通过检查是否有 id 列判断）
    let has_id_column: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('raid_bosses') WHERE name='id'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if !has_id_column {
        log::info!("V14 迁移：raid_bosses 已是新结构，跳过重构");
        return Ok(());
    }

    // 1. 创建新表（联合主键）
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS raid_bosses_new (
            raid_name TEXT NOT NULL,
            boss_id TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL,
            PRIMARY KEY (raid_name, boss_id)
        );
        "#,
    )
    .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：已创建 raid_bosses_new 表（联合主键 raid_name+boss_id）");

    // 2. 从旧表迁移数据，通过 LEFT JOIN raids 把 raid_name 统一为 raids.name
    //
    // 旧表 raid_name 列可能存：
    // - raids.name（如"阆风悬城"，V9 迁移写入）→ JOIN 不命中（r.id != raids.name），
    //   COALESCE 回退到 old.raid_name，结果仍是 raids.name
    // - raids.id（如"25人普通阆风悬城"，前端 db_save_raids 写入）→ JOIN 命中，
    //   COALESCE 取 r.name，结果为 raids.name
    //
    // INSERT OR IGNORE 按联合主键 (raid_name, boss_id) 去重，同一副本名下同一 boss_id 只保留一条。
    let migrated = conn
        .execute(
            r#"
            INSERT OR IGNORE INTO raid_bosses_new (raid_name, boss_id, name, boss_order)
            SELECT
                COALESCE(r.name, old.raid_name) AS raid_name,
                old.id AS boss_id,
                old.name,
                old.boss_order
            FROM raid_bosses old
            LEFT JOIN raids r ON r.id = old.raid_name
            "#,
            [],
        )
        .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：从旧表迁移 {} 条 BOSS 记录到新表", migrated);

    // 3. 修正"阿史"→"阿史那承庆"（与 static_raids.json 保持一致）
    let fixed = conn
        .execute(
            "UPDATE raid_bosses_new SET name = '阿史那承庆' WHERE name = '阿史'",
            [],
        )
        .map_err(|e| e.to_string())?;
    if fixed > 0 {
        log::info!("V14 迁移：修正 {} 条 '阿史'→'阿史那承庆'", fixed);
    }

    // 4. 删除旧表，重命名新表为 raid_bosses
    conn.execute_batch(
        r#"
        DROP TABLE raid_bosses;
        ALTER TABLE raid_bosses_new RENAME TO raid_bosses;
        "#,
    )
    .map_err(|e| e.to_string())?;
    log::info!("V14 迁移：已删除旧表并将 raid_bosses_new 重命名为 raid_bosses");

    Ok(())
}

/// 4. 废弃 raid_versions 表
///
/// raid_versions (id, name) 与 game_versions (id, name, sort_order) 完全冗余：
/// - 两表 name 列和 id 值完全一致
/// - game_versions 有 sort_order 字段，功能等价且更完整
/// 迁移完成后删除 raid_versions 表，排序改用 game_versions.sort_order
fn drop_raid_versions_table(conn: &Connection) -> Result<(), String> {
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='raid_versions'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if exists {
        conn.execute("DROP TABLE IF EXISTS raid_versions", [])
            .map_err(|e| e.to_string())?;
        log::info!("V14 迁移：已删除冗余的 raid_versions 表，版本排序改用 game_versions.sort_order");
    } else {
        log::info!("V14 迁移：raid_versions 表不存在，跳过");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== app_config 测试 ==========

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
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

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
    fn test_v14_app_config_default_values() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        assert_eq!(get_config_value(&conn, "game_directory"), "");
        assert_eq!(get_config_value(&conn, "setup_completed"), "false");
        assert_eq!(get_config_value(&conn, "last_scan_mingyi_at"), "");
    }

    #[test]
    fn test_v14_app_config_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        // 修改一个值
        conn.execute(
            "UPDATE app_config SET value = 'E:\\Game\\JX3' WHERE key = 'game_directory'",
            [],
        )
        .unwrap();

        // 再次运行迁移，已存在的值不应被覆盖
        migrate(&conn).unwrap();
        assert_eq!(get_config_value(&conn, "game_directory"), "E:\\Game\\JX3");
    }

    #[test]
    fn test_v14_migrates_game_directory_from_old_config() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 创建旧 config 表并写入 gameDirectory
        conn.execute_batch(
            r#"
            CREATE TABLE config (id INTEGER PRIMARY KEY, value TEXT);
            INSERT INTO config (id, value) VALUES (1, '{"game":{"gameDirectory":"E:\\Old\\Game\\Dir","autoDetectEnabled":false},"ai":{},"trial":{}}');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert_eq!(
            get_config_value(&conn, "game_directory"),
            "E:\\Old\\Game\\Dir"
        );
        // config_json 应包含完整 JSON blob
        let config_json = get_config_value(&conn, "config_json");
        assert!(config_json.contains("autoDetectEnabled"));
        assert!(config_json.contains("gameDirectory"));
        // 旧 config 表应已删除
        let config_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!config_exists, "config 表应已删除");
    }

    #[test]
    fn test_v14_skips_migration_when_old_config_empty() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 旧 config 表存在但 gameDirectory 为空
        conn.execute_batch(
            r#"
            CREATE TABLE config (id INTEGER PRIMARY KEY, value TEXT);
            INSERT INTO config (id, value) VALUES (1, '{"game":{"gameDirectory":"","autoDetectEnabled":false},"ai":{},"trial":{}}');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();
        assert_eq!(get_config_value(&conn, "game_directory"), "");
    }

    #[test]
    fn test_v14_skips_migration_when_no_old_config_table() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        // 不创建 config 表

        migrate(&conn).unwrap();
        assert_eq!(get_config_value(&conn, "game_directory"), "");
        assert_eq!(get_config_value(&conn, "config_json"), "");
        assert_eq!(get_config_value(&conn, "local_storage_migrated"), "false");
    }

    #[test]
    fn test_v14_does_not_overwrite_existing_game_directory() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        // 用户已设置 game_directory
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "UPDATE app_config SET value = 'E:\\User\\Set\\Dir', updated_at = ?1 WHERE key = 'game_directory'",
            params![now],
        )
        .unwrap();

        // 创建旧 config 表（有不同值），再次运行迁移
        conn.execute_batch(
            r#"
            CREATE TABLE config (id INTEGER PRIMARY KEY, value TEXT);
            INSERT INTO config (id, value) VALUES (1, '{"game":{"gameDirectory":"E:\\Old\\Dir","autoDetectEnabled":false},"ai":{},"trial":{}}');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        // 用户设置的值应保留，不被旧值覆盖
        assert_eq!(
            get_config_value(&conn, "game_directory"),
            "E:\\User\\Set\\Dir"
        );
        // config_json 应已迁移（第二次运行时 config 表存在）
        let config_json = get_config_value(&conn, "config_json");
        assert!(config_json.contains("gameDirectory"), "config_json 应包含旧 config 数据");
        // 旧 config 表应已删除
        let config_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='config'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!config_exists, "config 表应已删除");
    }

    // ========== records 时间戳和索引测试 ==========

    #[test]
    fn test_v14_adds_records_timestamp_columns() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        let has_created_at: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='created_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_created_at, "records 表应有 created_at 列");

        let has_updated_at: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='updated_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_updated_at, "records 表应有 updated_at 列");
    }

    #[test]
    fn test_v14_creates_records_indexes() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        let index_exists = |index_name: &str| -> bool {
            conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name=?1",
                params![index_name],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert!(index_exists("idx_records_status"), "应有 idx_records_status 索引");
        assert!(index_exists("idx_records_source"), "应有 idx_records_source 索引");
    }

    #[test]
    fn test_v14_creates_jcl_cache_table() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='jcl_cache'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(table_exists, "jcl_cache 表应存在");
    }

    #[test]
    fn test_v14_does_not_create_scan_records() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='scan_records'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!table_exists, "scan_records 表不应存在");
    }

    // ========== raid_bosses 重构测试 ==========

    #[test]
    fn test_v14_raid_bosses_unifies_raid_name_from_raids_id() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        // 旧表 raid_name 存的是 raids.id（前端保存路径写入）
        conn.execute(
            "INSERT INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["langfengxuancheng_1", "25人英雄阆风悬城", "笑妆娘", 1],
        )
        .unwrap();

        migrate(&conn).unwrap();

        // 迁移后 raid_name 应归一化为 raids.name="阆风悬城"
        let raid_name: String = conn
            .query_row(
                "SELECT raid_name FROM raid_bosses WHERE boss_id = ?",
                params!["langfengxuancheng_1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raid_name, "阆风悬城");
    }

    #[test]
    fn test_v14_raid_bosses_preserves_raid_name_already_correct() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        // 旧表 raid_name 存的已经是 raids.name（V9 迁移写入）
        conn.execute(
            "INSERT INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["langfengxuancheng_1", "阆风悬城", "笑妆娘", 1],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let raid_name: String = conn
            .query_row(
                "SELECT raid_name FROM raid_bosses WHERE boss_id = ?",
                params!["langfengxuancheng_1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raid_name, "阆风悬城");
    }

    #[test]
    fn test_v14_raid_bosses_dedupes_conflicting_keys() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        // 模拟旧表同时存在 V9 写入和前端保存写入（boss_id 相同）
        conn.execute(
            "INSERT INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["langfengxuancheng_1", "阆风悬城", "笑妆娘", 1],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["langfengxuancheng_1", "25人英雄阆风悬城", "笑妆娘", 1],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM raid_bosses WHERE boss_id = ?",
                params!["langfengxuancheng_1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        let raid_name: String = conn
            .query_row(
                "SELECT raid_name FROM raid_bosses WHERE boss_id = ?",
                params!["langfengxuancheng_1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raid_name, "阆风悬城");
    }

    #[test]
    fn test_v14_raid_bosses_fixes_ashi_name() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        conn.execute(
            "INSERT INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["langfengxuancheng_4", "阆风悬城", "阿史", 4],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let name: String = conn
            .query_row(
                "SELECT name FROM raid_bosses WHERE boss_id = ?",
                params!["langfengxuancheng_4"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "阿史那承庆");
    }

    #[test]
    fn test_v14_raid_bosses_composite_primary_key() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        conn.execute(
            "INSERT INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["boss_1", "阆风悬城", "笑妆娘", 1],
        )
        .unwrap();

        migrate(&conn).unwrap();

        // 新表允许不同 raid_name 复用同一 boss_id
        conn.execute(
            "INSERT INTO raid_bosses (raid_name, boss_id, name, boss_order) VALUES (?, ?, ?, ?)",
            params!["其它副本", "boss_1", "其它BOSS", 1],
        )
        .unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM raid_bosses WHERE boss_id = ?",
                params!["boss_1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_v14_migrates_migration_flags() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 创建旧 migration_flags 表
        conn.execute_batch(
            r#"
            CREATE TABLE migration_flags (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT
            );
            INSERT INTO migration_flags (key, value, updated_at) VALUES ('local_storage_migrated', 'true', '2024-01-01T00:00:00+08:00');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        // local_storage_migrated 应迁移到 app_config
        assert_eq!(get_config_value(&conn, "local_storage_migrated"), "true");
        // 旧 migration_flags 表应已删除
        let flags_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='migration_flags'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!flags_exists, "migration_flags 表应已删除");
    }

    #[test]
    fn test_v14_migration_flags_not_true_stays_false() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 创建旧 migration_flags 表，但值为 false
        conn.execute_batch(
            r#"
            CREATE TABLE migration_flags (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT
            );
            INSERT INTO migration_flags (key, value, updated_at) VALUES ('local_storage_migrated', 'false', '2024-01-01T00:00:00+08:00');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        // 未迁移的应保持默认值 false
        assert_eq!(get_config_value(&conn, "local_storage_migrated"), "false");
    }

    #[test]
    fn test_v14_drops_raid_versions_table() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 迁移前 raid_versions 应存在
        let rv_exists_before: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='raid_versions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(rv_exists_before, "迁移前 raid_versions 表应存在");

        // game_versions 应保留
        let gv_exists_before: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='game_versions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(gv_exists_before, "game_versions 表应存在");

        migrate(&conn).unwrap();

        // 迁移后 raid_versions 应已删除
        let rv_exists_after: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='raid_versions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!rv_exists_after, "raid_versions 表应已删除");

        // game_versions 应仍存在
        let gv_exists_after: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='game_versions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(gv_exists_after, "game_versions 表应保留");
    }

    /// 预创建 records + raids + 旧 raid_bosses 表，模拟 V14 执行前的数据库状态
    fn setup_prerequisites(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE records (
                id TEXT PRIMARY KEY,
                data TEXT,
                raid_name TEXT,
                account_id TEXT,
                role_id TEXT,
                record_date INTEGER,
                record_type TEXT
            );
            CREATE TABLE raids (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                difficulty TEXT,
                player_count INTEGER,
                version TEXT,
                notes TEXT,
                is_active INTEGER,
                is_static INTEGER
            );
            CREATE TABLE raid_bosses (
                id TEXT PRIMARY KEY,
                raid_name TEXT NOT NULL,
                name TEXT NOT NULL,
                boss_order INTEGER NOT NULL
            );
            CREATE TABLE game_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE raid_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL
            );
            "#,
        )
        .unwrap();

        // 插入 game_versions 和 raid_versions（两表数据完全一致）
        for (name, sort_order) in [("丝路风雨", 8), ("横刀断浪", 7)] {
            conn.execute(
                "INSERT INTO game_versions (name, sort_order, created_at) VALUES (?, ?, '2024-01-01')",
                params![name, sort_order],
            )
            .unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO raid_versions (name) VALUES (?)",
                params![name],
            )
            .unwrap();
        }

        // 插入 raids：3 个难度共享 name="阆风悬城"
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params!["10人普通阆风悬城", "阆风悬城", "普通", 10, "丝路风雨", "", 1, 1],
        ).unwrap();
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params!["25人普通阆风悬城", "阆风悬城", "普通", 25, "丝路风雨", "", 1, 1],
        ).unwrap();
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params!["25人英雄阆风悬城", "阆风悬城", "英雄", 25, "丝路风雨", "", 1, 1],
        ).unwrap();
    }
}
