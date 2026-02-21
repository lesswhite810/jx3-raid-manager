use rusqlite::{params, Connection, Error as SqliteError};

/// Current database schema version
pub const CURRENT_SCHEMA_VERSION: i32 = 2;
pub const SCHEMA_VERSION_TABLE: &str = "schema_versions";

pub fn error_to_string(e: SqliteError) -> String {
    format!("Database error: {}", e)
}

/// Initialize the schema versions table
pub fn init_schema_versions(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {} (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT
        );
    "#,
        SCHEMA_VERSION_TABLE
    ))
    .map_err(error_to_string)
}

/// Get the current schema version
pub fn get_current_version(conn: &Connection) -> Result<i32, String> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
            [SCHEMA_VERSION_TABLE],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if table_exists == 0 {
        return Ok(0);
    }

    let version: i32 = conn
        .query_row(
            &format!(
                "SELECT version FROM {} ORDER BY version DESC LIMIT 1",
                SCHEMA_VERSION_TABLE
            ),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(version)
}

use crate::db::migrations::{v1, v2};

/// Apply migrations up to current version
pub fn apply_migrations(conn: &Connection) -> Result<(), String> {
    let mut current_version = get_current_version(conn)?;

    while current_version < CURRENT_SCHEMA_VERSION {
        let next_version = current_version + 1;
        match next_version {
            1 => {
                v1::apply_v1_migration(conn)?;
                mark_version_applied(
                    conn,
                    1,
                    "Split accounts table into accounts and roles tables",
                )?;
            }
            2 => {
                v2::apply_v2_migration(conn)?;
                mark_version_applied(
                    conn,
                    2,
                    "Upgrade raids table to structured columns with raid_bosses",
                )?;
            }
            _ => {
                return Err(format!("Unknown migration version: {}", next_version));
            }
        }
        current_version = next_version;
        log::info!("Successfully migrated database to version {}", current_version);
    }

    Ok(())
}

/// Mark a version as applied
pub fn mark_version_applied(
    conn: &Connection,
    version: i32,
    description: &str,
) -> Result<(), String> {
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        &format!(
            "INSERT OR REPLACE INTO {} (version, applied_at, description) VALUES (?, ?, ?)",
            SCHEMA_VERSION_TABLE
        ),
        params![version, timestamp, description],
    )
    .map_err(error_to_string)?;
    Ok(())
}

/// Check if migration is needed
pub fn is_migration_needed(conn: &Connection) -> Result<bool, String> {
    let old_accounts_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts' AND sql LIKE '%data TEXT%'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    Ok(old_accounts_exists > 0)
}


/// 初始化预制副本数据（从 static_raids.json 读取）
/// 该方法会在空库初次加载或者新增加预制数据时注入缺失的默认副本
/// raid_bosses 按副本名称关联，每个副本只存一份 boss 数据
pub fn init_static_raids(conn: &Connection) -> Result<(), String> {
    let static_json = include_str!("static_raids.json");
    let static_raids: Vec<serde_json::Value> = serde_json::from_str(static_json)
        .map_err(|e| format!("解析预制副本数据失败: {}", e))?;

    let mut inserted_count = 0;
    let mut boss_inserted_names = std::collections::HashSet::new();

    for raid in static_raids {
        let name = raid["name"].as_str().unwrap_or_default();
        let version = raid["version"].as_str().unwrap_or_default();

        // 插入各难度/人数配置
        if let Some(configs) = raid["configurations"].as_array() {
            for config in configs {
                let player_count = config["playerCount"].as_i64().unwrap_or(25);
                let difficulty = config["difficulty"].as_str().unwrap_or("普通");
                let is_active = if config["isActive"].as_bool().unwrap_or(true) { 1 } else { 0 };
                let id = format!("{}人{}{}", player_count, difficulty, name);

                let changes = conn.execute(
                    "INSERT OR IGNORE INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
                    params![&id, name, difficulty, player_count, version, "", is_active],
                ).map_err(error_to_string)?;

                inserted_count += changes;
            }
        }

        // boss 按副本名称只写一次，所有难度配置共享
        if !boss_inserted_names.contains(name) {
            if let Some(bosses) = raid["bosses"].as_array() {
                for boss in bosses {
                    let boss_id = boss["id"].as_str().unwrap_or_default();
                    let boss_name = boss["name"].as_str().unwrap_or_default();
                    let boss_order = boss["order"].as_i64().unwrap_or(0);

                    conn.execute(
                        "INSERT OR IGNORE INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
                        params![boss_id, name, boss_name, boss_order],
                    ).map_err(error_to_string)?;
                }
                boss_inserted_names.insert(name.to_string());
            }
        }
    }

    if inserted_count > 0 {
        log::info!("成功注入 {} 条预制副本数据", inserted_count);
    }

    Ok(())
}
