use rusqlite::params;
use crate::db::migration::error_to_string;
use rusqlite::Connection;
use serde_json::Value;

/// V6 迁移：
/// 1. 副本名称变更 - "弓月城" -> "会战弓月城"
/// 2. 创建团队副本角色可见性表 raid_role_visibility
/// 3. 将所有时间字段从 UTC 转换为本地时间
/// 4. 删除历史备份表 accounts_legacy, raids_legacy
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V6 迁移：开始...");

    // 1. 创建 raid_role_visibility 表
    create_raid_role_visibility_table(conn)?;
    log::info!("V6 迁移：raid_role_visibility 表创建完成");

    // 2. 转换所有时间字段从 UTC 到本地时间（批量处理）
    convert_utc_to_local_time(conn)?;
    log::info!("V6 迁移：时间字段转换完成");

    // 3. 更新 raids 表
    migrate_raids_table(conn)?;

    // 4. 更新 raid_bosses 表
    let bosses_updated = conn.execute(
        "UPDATE raid_bosses SET raid_name = '会战弓月城' WHERE raid_name = '弓月城'",
        [],
    ).map_err(error_to_string)?;
    log::info!("V6 迁移：已更新 {} 条 raid_bosses 记录", bosses_updated);

    // 5. 更新 records 表
    migrate_records_data(conn)?;

    // 6. 更新 favorite_raids 表
    let favorite_updated = conn.execute(
        "UPDATE favorite_raids SET raid_name = '会战弓月城' WHERE raid_name = '弓月城'",
        [],
    ).map_err(error_to_string)?;
    log::info!("V6 迁移：已更新 {} 条 favorite_raids 记录", favorite_updated);

    // 7. 删除历史备份表
    drop_legacy_tables(conn)?;
    log::info!("V6 迁移：历史备份表删除完成");

    log::info!("V6 迁移：完成");
    Ok(())
}

/// 创建 raid_role_visibility 表
fn create_raid_role_visibility_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS raid_role_visibility (
            id TEXT PRIMARY KEY,
            role_id TEXT NOT NULL,
            raid_key TEXT NOT NULL,
            visible INTEGER DEFAULT 1,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
            UNIQUE(role_id, raid_key)
        );
        CREATE INDEX IF NOT EXISTS idx_rrv_role_id ON raid_role_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_rrv_raid_key ON raid_role_visibility(raid_key);
        "#,
    )
    .map_err(error_to_string)?;

    Ok(())
}

/// 更新 raids 表
fn migrate_raids_table(conn: &Connection) -> Result<(), String> {
    let raids: Vec<(String, String)> = conn
        .prepare("SELECT id, name FROM raids WHERE name = '弓月城' OR id LIKE '%弓月城%'")
        .map_err(error_to_string)?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    let mut name_updated = 0;
    let mut id_updated = 0;

    for (old_id, name) in raids {
        if name == "弓月城" {
            let new_id = old_id.replace("弓月城", "会战弓月城");
            let new_name = name.replace("弓月城", "会战弓月城");
            conn.execute(
                "UPDATE raids SET id = ?1, name = ?3 WHERE id = ?2",
                params![&new_id, &old_id, &new_name],
            ).map_err(error_to_string)?;
            name_updated += 1;
            id_updated += 1;
        }
    }

    log::info!("V6 迁移：raids 表更新完成 (name: {}, id: {})", name_updated, id_updated);
    Ok(())
}

/// 将所有时间字段从 UTC 转换为本地时间
fn convert_utc_to_local_time(conn: &Connection) -> Result<(), String> {
    log::info!("V6 迁移：开始转换时间字段...");

    let table_columns: [(&str, &[&str]); 13] = [
        ("schema_versions", &["applied_at"]),
        ("migration_flags", &["updated_at"]),
        ("cache", &["updated_at"]),
        ("daily_records", &["updated_at"]),
        ("baizhan_records", &["updated_at"]),
        ("trial_records", &["updated_at"]),
        ("equipments", &["updated_at"]),
        ("accounts", &["created_at", "updated_at"]),
        ("roles", &["created_at", "updated_at"]),
        ("favorite_raids", &["created_at"]),
        ("role_instance_visibility", &["created_at", "updated_at"]),
        ("records", &[]),
        ("raid_role_visibility", &["created_at", "updated_at"]),
    ];

    let mut total_converted = 0;

    for (table, columns) in &table_columns {
        if columns.is_empty() {
            continue;
        }
        let converted = convert_table_time_columns(conn, table, columns)?;
        total_converted += converted;
    }

    log::info!("V6 迁移：共转换 {} 条时间记录", total_converted);
    Ok(())
}

/// 转换单个表的多个时间列（使用预编译语句批量更新）
fn convert_table_time_columns(conn: &Connection, table: &str, columns: &[&str]) -> Result<usize, String> {
    // 检查表是否存在
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
            params![table],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !table_exists {
        return Ok(0);
    }

    // 构建查询
    let columns_str = columns.join(", ");
    let conditions: Vec<String> = columns.iter()
        .map(|c| format!("{} IS NOT NULL AND {} != ''", c, c))
        .collect();
    let where_clause = conditions.join(" OR ");
    let query = format!("SELECT rowid, {} FROM {} WHERE {}", columns_str, table, where_clause);

    // 获取所有记录
    let records: Vec<(i64, Vec<Option<String>>)> = conn
        .prepare(&query)
        .map_err(error_to_string)?
        .query_map([], |row| {
            let rowid: i64 = row.get(0)?;
            let values: Vec<Option<String>> = (0..columns.len())
                .map(|i| row.get(i + 1).ok())
                .collect();
            Ok((rowid, values))
        })
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    if records.is_empty() {
        return Ok(0);
    }

    let mut converted_count = 0;

    // 为每个列预编译 UPDATE 语句
    for (col_idx, &column) in columns.iter().enumerate() {
        let update_query = format!("UPDATE {} SET {} = ?1 WHERE rowid = ?2", table, column);
        let mut stmt = conn.prepare_cached(&update_query).map_err(error_to_string)?;

        for (rowid, values) in &records {
            if let Some(utc_time) = &values[col_idx] {
                if let Some(local_time) = convert_utc_string_to_local(utc_time) {
                    stmt.execute(params![&local_time, rowid]).map_err(error_to_string)?;
                    converted_count += 1;
                }
            }
        }
    }

    if converted_count > 0 {
        log::info!("V6 迁移：表 {} 转换了 {} 条时间记录", table, converted_count);
    }

    Ok(converted_count)
}

/// 将 UTC 时间字符串转换为本地时间字符串
fn convert_utc_string_to_local(utc_str: &str) -> Option<String> {
    let utc_datetime = chrono::DateTime::parse_from_rfc3339(utc_str).ok()?;
    let local_datetime = utc_datetime.with_timezone(&chrono::Local);
    Some(local_datetime.to_rfc3339())
}

/// 更新 records 表中 data JSON 字段
fn migrate_records_data(conn: &Connection) -> Result<(), String> {
    let records: Vec<(String, String)> = conn
        .prepare("SELECT id, data FROM records WHERE data LIKE '%弓月城%' OR data LIKE '%+00:00%'")
        .map_err(error_to_string)?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    let mut raid_name_updated = 0;
    let mut date_converted = 0;

    // 使用预编译语句
    let mut stmt = conn.prepare_cached("UPDATE records SET data = ?1 WHERE id = ?2").map_err(error_to_string)?;

    for (id, data) in records {
        if let Ok(mut json) = serde_json::from_str::<Value>(&data) {
            let mut needs_update = false;

            if let Some(raid_name) = json.get("raidName").and_then(|v| v.as_str()) {
                if raid_name.contains("弓月城") {
                    json["raidName"] = serde_json::Value::String(raid_name.replace("弓月城", "会战弓月城"));
                    needs_update = true;
                    raid_name_updated += 1;
                }
            }

            if let Some(date_str) = json.get("date").and_then(|v| v.as_str()) {
                if let Some(local_time) = convert_utc_string_to_local(date_str) {
                    json["date"] = serde_json::Value::String(local_time);
                    needs_update = true;
                    date_converted += 1;
                }
            }

            if needs_update {
                if let Ok(new_data) = serde_json::to_string(&json) {
                    stmt.execute(params![&new_data, &id]).map_err(error_to_string)?;
                }
            }
        }
    }

    log::info!("V6 迁移：records 表更新完成 (raidName: {}, date转换: {})", raid_name_updated, date_converted);
    Ok(())
}

/// 删除历史备份表
fn drop_legacy_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DROP TABLE IF EXISTS accounts_legacy;
         DROP TABLE IF EXISTS raids_legacy;",
    )
    .map_err(error_to_string)?;

    log::info!("V6 迁移：已删除历史备份表");
    Ok(())
}
