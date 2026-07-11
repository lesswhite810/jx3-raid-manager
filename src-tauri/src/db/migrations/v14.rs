use rusqlite::{params, Connection};
use serde_json::Value;

/// V14 迁移：app_config + scan_records + raid_bosses 重构
///
/// 本迁移合并了原本独立的三个迁移（在 v2.1.41 之后引入，均未发布）：
///
/// 1. **app_config 表**：应用配置 key-value 存储，默认 game_directory /
///    setup_completed / last_scan_mingyi_at 三个键
///
/// 2. **scan_records 表**：自动扫描副本掉落待确认记录表
///    - status: 'pending'（待确认）/ 'rejected'（已驳回）
///    - drops: JSON 数组，原始物品名
///    - jcl_files: JSON 数组，关联 JCL 文件名
///    - 确认后通过事务迁移到 records 表
///
/// 3. **raid_bosses 表重构**：采用联合主键 (raid_name, boss_id)
///    - raid_name 列统一存 raids.name（副本基础名），同名副本天然共享 BOSS 记录
///    - 修正"阿史"→"阿史那承庆"（与 static_raids.json 对齐）
///    - 修复旧表 raid_name 列语义混乱（V9 写 raids.name，前端保存写 raids.id 且
///      ON CONFLICT 覆盖 V9 数据）导致的 BOSS 匹配失效问题
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V14 迁移开始 ==========");

    create_app_config_table(conn)?;
    create_scan_records_table(conn)?;
    rebuild_raid_bosses_table(conn)?;

    log::info!("========== V14 迁移完成 ==========");
    Ok(())
}

/// 1. 创建 app_config 表并插入默认配置
fn create_app_config_table(conn: &Connection) -> Result<(), String> {
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
        params!["last_scan_mingyi_at", "", &now],
    )
    .map_err(|e| e.to_string())?;

    // 从旧 config 表迁移 game_directory（V14 前存储为 config.game.gameDirectory）
    migrate_game_directory_from_old_config(conn, &now)?;

    log::info!("V14 迁移：app_config 表创建完成，默认值已插入");
    Ok(())
}

/// 从旧 config 表的 JSON blob 中读取 game.gameDirectory 并迁移到 app_config
fn migrate_game_directory_from_old_config(conn: &Connection, now: &str) -> Result<(), String> {
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
        return Ok(());
    };

    let parsed: Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let game_dir = parsed
        .get("game")
        .and_then(|g| g.get("gameDirectory"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if game_dir.is_empty() {
        return Ok(());
    }

    conn.execute(
        "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'game_directory' AND value = ''",
        params![game_dir, now],
    )
    .map_err(|e| e.to_string())?;

    log::info!("V14 迁移：从旧 config 表迁移 game_directory={}", game_dir);
    Ok(())
}

/// 2. 创建 scan_records 表，存储自动扫描的待确认记录
fn create_scan_records_table(conn: &Connection) -> Result<(), String> {
    log::info!("V14 迁移：创建 scan_records 表（自动扫描待确认记录）");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS scan_records (
            id TEXT PRIMARY KEY,
            data TEXT,
            raid_name TEXT,
            account_id TEXT,
            role_id TEXT,
            record_date INTEGER,
            record_type TEXT,
            drops TEXT,
            jcl_files TEXT,
            status TEXT DEFAULT 'pending',
            created_at TEXT,
            updated_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_scan_records_status ON scan_records(status);
        CREATE INDEX IF NOT EXISTS idx_scan_records_account ON scan_records(account_id);
        "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("V14 迁移：scan_records 表创建完成");
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
    }

    // ========== scan_records 表测试 ==========

    #[test]
    fn test_v14_creates_scan_records_table() {
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
        assert!(table_exists);
    }

    #[test]
    fn test_v14_scan_records_default_status_is_pending() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO scan_records (id, data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params!["test1", "{}", "2024-01-01T00:00:00+08:00", "2024-01-01T00:00:00+08:00"],
        )
        .unwrap();
        let status: String = conn
            .query_row("SELECT status FROM scan_records WHERE id = ?1", params!["test1"], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(status, "pending");
    }

    #[test]
    fn test_v14_scan_records_creates_indexes() {
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
        assert!(index_exists("idx_scan_records_status"));
        assert!(index_exists("idx_scan_records_account"));
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
            "#,
        )
        .unwrap();

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
