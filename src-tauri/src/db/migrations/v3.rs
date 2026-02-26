use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V3 迁移：更新试炼之地和百战记录的表结构
///
/// 变更内容：
/// 1. trial_records 表：添加 role_name, server, record_type 字段，date 从 TEXT 改为 INTEGER
/// 2. baizhan_records 表：date 从 TEXT 改为 INTEGER
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V3 迁移：更新试炼之地和百战记录表结构...");

    // ========== 1. 更新 trial_records 表 ==========

    // 检查 trial_records 表是否存在
    let trial_table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='trial_records'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if trial_table_exists == 0 {
        // 表不存在，直接创建最新结构的表
        log::info!("V3 迁移：trial_records 表不存在，创建新表...");
        conn.execute_batch(
            r#"
            CREATE TABLE trial_records (
                id TEXT PRIMARY KEY,
                account_id TEXT,
                role_id TEXT,
                role_name TEXT DEFAULT '',
                server TEXT DEFAULT '',
                layer INTEGER,
                bosses TEXT,
                card_1 TEXT,
                card_2 TEXT,
                card_3 TEXT,
                card_4 TEXT,
                card_5 TEXT,
                flipped_index INTEGER,
                record_type TEXT DEFAULT 'trial',
                date INTEGER NOT NULL,
                notes TEXT,
                updated_at TEXT
            );
            "#,
        )
        .map_err(error_to_string)?;
        log::info!("V3 迁移：trial_records 表创建完成");
    } else {
        // 表存在，检查并添加缺失字段
        // 检查 role_name 字段是否存在
        let role_name_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('trial_records') WHERE name='role_name'",
                [],
                |row| row.get(0),
            )
            .map_err(error_to_string)?;

        if role_name_exists == 0 {
            conn.execute(
                "ALTER TABLE trial_records ADD COLUMN role_name TEXT DEFAULT ''",
                [],
            )
            .map_err(error_to_string)?;
            log::info!("V3 迁移：已添加 trial_records.role_name 字段");
        }

        // 检查 server 字段是否存在
        let server_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('trial_records') WHERE name='server'",
                [],
                |row| row.get(0),
            )
            .map_err(error_to_string)?;

        if server_exists == 0 {
            conn.execute(
                "ALTER TABLE trial_records ADD COLUMN server TEXT DEFAULT ''",
                [],
            )
            .map_err(error_to_string)?;
            log::info!("V3 迁移：已添加 trial_records.server 字段");
        }

        // 检查 record_type 字段是否存在
        let record_type_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('trial_records') WHERE name='record_type'",
                [],
                |row| row.get(0),
            )
            .map_err(error_to_string)?;

        if record_type_exists == 0 {
            conn.execute(
                "ALTER TABLE trial_records ADD COLUMN record_type TEXT DEFAULT 'trial'",
                [],
            )
            .map_err(error_to_string)?;
            log::info!("V3 迁移：已添加 trial_records.record_type 字段");
        }

        // 处理 date 字段：从 TEXT 转换为 INTEGER
        // SQLite 不支持直接 ALTER TABLE 修改列类型，需要创建新表
        let date_type: Result<String, _> = conn.query_row(
            "SELECT type FROM pragma_table_info('trial_records') WHERE name='date'",
            [],
            |row| row.get(0),
        );

        if let Ok(dt) = date_type {
            if dt == "TEXT" {
                log::info!("V3 迁移：转换 trial_records.date 从 TEXT 到 INTEGER...");

                // 创建临时表
                conn.execute_batch(
                    r#"
                    CREATE TABLE trial_records_new (
                        id TEXT PRIMARY KEY,
                        account_id TEXT,
                        role_id TEXT,
                        role_name TEXT DEFAULT '',
                        server TEXT DEFAULT '',
                        layer INTEGER,
                        bosses TEXT,
                        card_1 TEXT,
                        card_2 TEXT,
                        card_3 TEXT,
                        card_4 TEXT,
                        card_5 TEXT,
                        flipped_index INTEGER,
                        record_type TEXT DEFAULT 'trial',
                        date INTEGER NOT NULL,
                        notes TEXT,
                        updated_at TEXT
                    );

                    -- 复制数据，尝试转换 date
                    INSERT INTO trial_records_new (id, account_id, role_id, role_name, server, layer, bosses, card_1, card_2, card_3, card_4, card_5, flipped_index, record_type, date, notes, updated_at)
                    SELECT id, account_id, role_id, IFNULL(role_name, ''), IFNULL(server, ''), layer, bosses, card_1, card_2, card_3, card_4, card_5, flipped_index, IFNULL(record_type, 'trial'),
                           CAST(date AS INTEGER),
                           notes, updated_at
                    FROM trial_records;

                    DROP TABLE trial_records;
                    ALTER TABLE trial_records_new RENAME TO trial_records;
                    "#,
                )
                .map_err(error_to_string)?;
                log::info!("V3 迁移：trial_records.date 转换完成");
            }
        }
    }

    // ========== 2. 更新 baizhan_records 表 ==========

    // 检查 baizhan_records 表是否存在
    let baizhan_table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='baizhan_records'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if baizhan_table_exists == 0 {
        // 表不存在，直接创建最新结构的表
        log::info!("V3 迁移：baizhan_records 表不存在，创建新表...");
        conn.execute_batch(
            r#"
            CREATE TABLE baizhan_records (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                role_id TEXT NOT NULL,
                role_name TEXT,
                server TEXT,
                date INTEGER NOT NULL,
                gold_income INTEGER DEFAULT 0,
                gold_expense INTEGER DEFAULT 0,
                notes TEXT,
                record_type TEXT DEFAULT 'baizhan',
                updated_at TEXT
            );
            "#,
        )
        .map_err(error_to_string)?;
        log::info!("V3 迁移：baizhan_records 表创建完成");
    } else {
        // 表存在，先检查并添加缺失的字段

        // 检查 record_type 字段是否存在
        let record_type_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('baizhan_records') WHERE name='record_type'",
                [],
                |row| row.get(0),
            )
            .map_err(error_to_string)?;

        if record_type_exists == 0 {
            conn.execute(
                "ALTER TABLE baizhan_records ADD COLUMN record_type TEXT DEFAULT 'baizhan'",
                [],
            )
            .map_err(error_to_string)?;
            log::info!("V3 迁移：已添加 baizhan_records.record_type 字段");
        }

        // 处理 date 字段：从 TEXT 转换为 INTEGER
        let bz_date_type: Result<String, _> = conn.query_row(
            "SELECT type FROM pragma_table_info('baizhan_records') WHERE name='date'",
            [],
            |row| row.get(0),
        );

        if let Ok(date_type) = bz_date_type {
            if date_type == "TEXT" {
                log::info!("V3 迁移：转换 baizhan_records.date 从 TEXT 到 INTEGER...");

                conn.execute_batch(
                    r#"
                    CREATE TABLE baizhan_records_new (
                        id TEXT PRIMARY KEY,
                        account_id TEXT NOT NULL,
                        role_id TEXT NOT NULL,
                        role_name TEXT,
                        server TEXT,
                        date INTEGER NOT NULL,
                        gold_income INTEGER DEFAULT 0,
                        gold_expense INTEGER DEFAULT 0,
                        notes TEXT,
                        record_type TEXT DEFAULT 'baizhan',
                        updated_at TEXT
                    );

                    INSERT INTO baizhan_records_new (id, account_id, role_id, role_name, server, date, gold_income, gold_expense, notes, record_type, updated_at)
                    SELECT id, account_id, role_id, role_name, server,
                           CAST(date AS INTEGER),
                           gold_income, gold_expense, notes, IFNULL(record_type, 'baizhan'), updated_at
                    FROM baizhan_records;

                    DROP TABLE baizhan_records;
                    ALTER TABLE baizhan_records_new RENAME TO baizhan_records;
                    "#
                ).map_err(error_to_string)?;
                log::info!("V3 迁移：baizhan_records.date 转换完成");
            }
        }
    }

    log::info!("V3 迁移：所有表结构更新完成");
    Ok(())
}
