use rusqlite::{params, Connection};
use crate::db::migration::error_to_string;

/// V1 迁移：将 JSON blob 格式的 accounts 数据迁移到结构化表
///
/// 前置条件：
/// - 存在旧的 accounts 表（包含 id, data 列）
///
/// 迁移内容：
/// - 创建 accounts 结构化表
/// - 创建 roles 结构化表
/// - 将 JSON 数据拆分并迁移到新表
/// - 旧表重命名为 accounts_legacy 作为备份
pub fn migrate(conn: &Connection) -> Result<(), String> {
    let timestamp = chrono::Utc::now().to_rfc3339();

    log::info!("V1 迁移：开始执行账号数据迁移...");

    // Step 1: 检查是否存在旧的 accounts 表（有 data 列）
    let has_old_table: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if has_old_table == 0 {
        log::info!("V1 迁移：不存在旧 accounts 表，无需迁移");
        return Ok(());
    }

    // 检查是否有 data 列（说明是旧格式）
    let has_data_column: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('accounts') WHERE name='data'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if has_data_column == 0 {
        log::info!("V1 迁移：accounts 表不是 JSON blob 格式，无需迁移");
        return Ok(());
    }

    // Step 2: 读取旧数据
    let mut stmt = conn
        .prepare("SELECT id, data FROM accounts")
        .map_err(error_to_string)?;
    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(error_to_string)?;

    let mut accounts_data: Vec<(String, String)> = Vec::new();
    for row in rows {
        let (id, data) = row.map_err(error_to_string)?;
        accounts_data.push((id, data));
    }
    drop(stmt);

    log::info!("V1 迁移：读取到 {} 条旧账号数据", accounts_data.len());

    if accounts_data.is_empty() {
        log::info!("V1 迁移：旧表中无数据，跳过迁移");
        return Ok(());
    }

    // Step 3: 重命名旧表为 legacy
    conn.execute("ALTER TABLE accounts RENAME TO accounts_legacy;", [])
        .map_err(error_to_string)?;
    log::info!("V1 迁移：已将旧 accounts 表重命名为 accounts_legacy");

    // Step 4: 创建新的结构化表
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            account_name TEXT NOT NULL,
            account_type TEXT NOT NULL DEFAULT 'OWN',
            password TEXT,
            notes TEXT,
            hidden INTEGER DEFAULT 0,
            disabled INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS roles (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            name TEXT NOT NULL,
            server TEXT,
            region TEXT,
            sect TEXT,
            equipment_score INTEGER,
            disabled INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_roles_account_id ON roles(account_id);
        CREATE INDEX IF NOT EXISTS idx_accounts_name ON accounts(account_name);
    "#,
    )
    .map_err(error_to_string)?;

    // Step 5: 迁移数据
    let mut migrated_accounts = 0;
    let mut migrated_roles = 0;

    for (account_id, data_str) in accounts_data {
        let data: serde_json::Value = match serde_json::from_str(&data_str) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("V1 迁移：解析账号 {} JSON 失败: {}", account_id, e);
                continue;
            }
        };

        let account_name = data["accountName"]
            .as_str()
            .or(data["username"].as_str())
            .unwrap_or("")
            .to_string();
        let account_type = data["type"].as_str().unwrap_or("OWN").to_string();
        let password = data["password"].as_str().map(|s| s.to_string());
        let notes = data["notes"].as_str().map(|s| s.to_string());
        let hidden = data["hidden"].as_bool().unwrap_or(false) as i32;
        let disabled = data["disabled"].as_bool().unwrap_or(false) as i32;

        match conn.execute(
            "INSERT OR IGNORE INTO accounts (id, account_name, account_type, password, notes, hidden, disabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![account_id, account_name, account_type, password, notes, hidden, disabled, timestamp, timestamp],
        ) {
            Ok(_) => migrated_accounts += 1,
            Err(e) => {
                log::warn!("V1 迁移：插入账号 {} 失败: {}", account_id, e);
                continue;
            }
        }

        if let Some(roles_array) = data["roles"].as_array() {
            for role_json in roles_array {
                let role_id = role_json["id"]
                    .as_str()
                    .or(role_json["name"].as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let role_name = role_json["name"].as_str().unwrap_or("").to_string();
                let server = role_json["server"].as_str().map(|s| s.to_string());
                let region = role_json["region"].as_str().map(|s| s.to_string());
                let sect = role_json["sect"].as_str().map(|s| s.to_string());
                let role_disabled = role_json["disabled"].as_bool().unwrap_or(false) as i32;

                let equipment_score = role_json["equipmentScore"]
                    .as_i64()
                    .or(role_json["equipmentScore"].as_str().and_then(|s| s.parse().ok()));

                match conn.execute(
                    "INSERT OR IGNORE INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![role_id, &account_id, role_name, server, region, sect, equipment_score, role_disabled, timestamp, timestamp],
                ) {
                    Ok(_) => migrated_roles += 1,
                    Err(e) => log::warn!("V1 迁移：插入角色失败: {}", e),
                }
            }
        }
    }

    log::info!("V1 迁移完成：迁移 {} 个账号，{} 个角色", migrated_accounts, migrated_roles);
    Ok(())
}
