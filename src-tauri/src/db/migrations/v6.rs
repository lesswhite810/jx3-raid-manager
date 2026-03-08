use rusqlite::params;
use crate::db::migration::error_to_string;
use rusqlite::Connection;
use serde_json::Value;

/// V6 迁移：
/// 1. 副本名称变更 - "弓月城" -> "会战弓月城"
/// 2. 创建团队副本角色可见性表 raid_role_visibility
/// 3. 将所有时间字段从 UTC 转换为本地时间
/// 4. 删除历史备份表 accounts_legacy, raids_legacy
///
/// 变更内容：
/// - 更新 raids 表中的 name 和 id 字段（合并操作）
/// - 更新 raid_bosses 表中的 raid_name 字段
/// - 更新 records 表中 data JSON 字段的 raidName 和 date（合并操作）
/// - 更新 favorite_raids 表中的 raid_name 字段
/// - 创建 raid_role_visibility 表（团队副本专用角色可见性配置）
/// - 转换所有时间字段从 UTC 到本地时间（按表合并操作）
/// - 删除历史备份表 accounts_legacy, raids_legacy
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V6 迁移：开始...");

    // 1. 创建 raid_role_visibility 表
    create_raid_role_visibility_table(conn)?;
    log::info!("V6 迁移：raid_role_visibility 表创建完成");

    // 2. 转换所有时间字段从 UTC 到本地时间（按表合并）
    convert_utc_to_local_time(conn)?;
    log::info!("V6 迁移：时间字段转换完成");

    // 3. 更新 raids 表（合并 name 和 id 操作）
    migrate_raids_table(conn)?;

    // 4. 更新 raid_bosses 表中的 raid_name 字段
    let bosses_updated = conn.execute(
        "UPDATE raid_bosses SET raid_name = '会战弓月城' WHERE raid_name = '弓月城'",
        [],
    ).map_err(error_to_string)?;

    log::info!("V6 迁移：已更新 {} 条 raid_bosses 记录", bosses_updated);

    // 5. 更新 records 表（合并 raidName 和 date 时间转换）
    migrate_records_data(conn)?;

    // 6. 更新 favorite_raids 表中的 raid_name 字段
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
        -- 团队副本角色可见性表 (V6+)
        -- 专门用于存储团队副本级别的角色禁用/启用配置，与 instance_types 分离
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

/// 更新 raids 表（合并 name 和 id 操作）
fn migrate_raids_table(conn: &Connection) -> Result<(), String> {
    // 一次性查询所有需要更新的 raids 记录
    let raids: Vec<(String, String)> = conn
        .prepare("SELECT id, name FROM raids WHERE name = '弓月城' OR id LIKE '%弓月城%'")
        .map_err(error_to_string)?
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    let mut name_updated = 0;
    let mut id_updated = 0;

    for (old_id, name) in raids {
        // 更新 name 字段
        if name == "弓月城" {
            conn.execute(
                "UPDATE raids SET name = '会战弓月城' WHERE id = ?1",
                params![&old_id],
            ).map_err(error_to_string)?;
            name_updated += 1;
        }

        // 更新 id 字段
        if old_id.contains("弓月城") {
            let new_id = old_id.replace("弓月城", "会战弓月城");
            conn.execute(
                "UPDATE raids SET id = ?1 WHERE id = ?2",
                params![&new_id, &old_id],
            ).map_err(error_to_string)?;
            id_updated += 1;
        }
    }

    log::info!("V6 迁移：raids 表更新完成 (name: {}, id: {})", name_updated, id_updated);
    Ok(())
}

/// 将所有时间字段从 UTC 转换为本地时间（按表合并）
fn convert_utc_to_local_time(conn: &Connection) -> Result<(), String> {
    log::info!("V6 迁移：开始转换时间字段...");

    // 定义每张表需要转换的字段（按表分组）
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
        ("records", &[]), // records 表在 migrate_records_data 中单独处理
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

/// 转换单个表的多个时间列（一次查询，多次更新）
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
        log::info!("V6 迁移：表 {} 不存在，跳过", table);
        return Ok(0);
    }

    // 构建查询所有需要转换的列
    let columns_str = columns.join(", ");
    let conditions: Vec<String> = columns.iter()
        .map(|c| format!("{} IS NOT NULL AND {} != ''", c, c))
        .collect();
    let where_clause = conditions.join(" OR ");

    let query = format!("SELECT rowid, {} FROM {} WHERE {}", columns_str, table, where_clause);

    // 一次性获取所有记录
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

    let mut converted_count = 0;

    // 为每个需要更新的记录执行更新
    for (rowid, values) in records {
        for (col_idx, value_opt) in values.iter().enumerate() {
            if let Some(utc_time) = value_opt {
                if let Some(local_time) = convert_utc_string_to_local(utc_time) {
                    let column = columns[col_idx];
                    let update_query = format!("UPDATE {} SET {} = ?1 WHERE rowid = ?2", table, column);
                    conn.execute(&update_query, params![&local_time, rowid])
                        .map_err(error_to_string)?;
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
    // 尝试解析 UTC 时间
    let utc_datetime = chrono::DateTime::parse_from_rfc3339(utc_str).ok()?;

    // 转换为本地时间
    let local_datetime = utc_datetime.with_timezone(&chrono::Local);

    // 返回本地时间的 RFC3339 格式
    Some(local_datetime.to_rfc3339())
}

/// 更新 records 表中 data JSON 字段（合并 raidName 和 date 时间转换）
fn migrate_records_data(conn: &Connection) -> Result<(), String> {
    // 一次性查询所有可能需要更新的记录（包含弓月城或有日期字段）
    let records: Vec<(String, String)> = conn
        .prepare("SELECT id, data FROM records WHERE data LIKE '%弓月城%' OR data LIKE '%+00:00%'")
        .map_err(error_to_string)?
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let data: String = row.get(1)?;
            Ok((id, data))
        })
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    let mut raid_name_updated = 0;
    let mut date_converted = 0;

    for (id, data) in records {
        // 解析 JSON
        if let Ok(mut json) = serde_json::from_str::<Value>(&data) {
            let mut needs_update = false;

            // 1. 检查并更新 raidName 字段
            if let Some(raid_name) = json.get("raidName").and_then(|v| v.as_str()) {
                if raid_name.contains("弓月城") {
                    let new_raid_name = raid_name.replace("弓月城", "会战弓月城");
                    json["raidName"] = serde_json::Value::String(new_raid_name);
                    needs_update = true;
                    raid_name_updated += 1;
                }
            }

            // 2. 检查 date 字段是否为 UTC 时间格式，需要转换
            if let Some(date_str) = json.get("date").and_then(|v| v.as_str()) {
                // 如果是 RFC3339 格式的 UTC 时间，转换为本地时间
                if let Some(local_time) = convert_utc_string_to_local(date_str) {
                    json["date"] = serde_json::Value::String(local_time);
                    needs_update = true;
                    date_converted += 1;
                }
            }

            // 只在需要更新时执行数据库更新
            if needs_update {
                if let Ok(new_data) = serde_json::to_string(&json) {
                    conn.execute(
                        "UPDATE records SET data = ?1 WHERE id = ?2",
                        params![&new_data, &id],
                    ).map_err(error_to_string)?;
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
