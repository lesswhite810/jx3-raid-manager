use rusqlite::params;
use crate::db::migration::error_to_string;
use rusqlite::Connection;
use serde_json::Value;

/// V6 迁移：副本名称变更 - "弓月城" -> "会战弓月城"
///
/// 变更内容：
/// - 更新 raids 表中的 name 字段
/// - 更新 raids 表中的 id 字段（包含"弓月城"的部分）
/// - 更新 raid_bosses 表中的 raid_name 字段
/// - 更新 records 表中 data JSON 字段的 raidName
/// - 更新 favorite_raids 表中的 raid_name 字段
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V6 迁移：副本名称变更 '弓月城' -> '会战弓月城'...");

    // 1. 更新 raids 表中的 name 字段
    let raids_updated = conn.execute(
        "UPDATE raids SET name = '会战弓月城' WHERE name = '弓月城'",
        [],
    ).map_err(error_to_string)?;

    log::info!("V6 迁移：已更新 {} 条 raids 记录", raids_updated);

    // 2. 更新 raids 表中的 id 字段（例如 "25人普通弓月城" -> "25人普通会战弓月城"）
    let raid_ids: Vec<String> = conn
        .prepare("SELECT id FROM raids WHERE id LIKE '%弓月城%'")
        .map_err(error_to_string)?
        .query_map([], |row| row.get(0))
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    for old_id in raid_ids {
        let new_id = old_id.replace("弓月城", "会战弓月城");
        conn.execute(
            "UPDATE raids SET id = ?1 WHERE id = ?2",
            params![&new_id, &old_id],
        ).map_err(error_to_string)?;
    }

    log::info!("V6 迁移：raids 表 id 字段更新完成");

    // 3. 更新 raid_bosses 表中的 raid_name 字段
    let bosses_updated = conn.execute(
        "UPDATE raid_bosses SET raid_name = '会战弓月城' WHERE raid_name = '弓月城'",
        [],
    ).map_err(error_to_string)?;

    log::info!("V6 迁移：已更新 {} 条 raid_bosses 记录", bosses_updated);

    // 4. 更新 records 表中 data JSON 字段的 raidName
    migrate_records_data(conn)?;

    // 5. 更新 favorite_raids 表中的 raid_name 字段
    let favorite_updated = conn.execute(
        "UPDATE favorite_raids SET raid_name = '会战弓月城' WHERE raid_name = '弓月城'",
        [],
    ).map_err(error_to_string)?;

    log::info!("V6 迁移：已更新 {} 条 favorite_raids 记录", favorite_updated);

    log::info!("V6 迁移：副本名称变更完成");
    Ok(())
}

/// 更新 records 表中 data JSON 字段的 raidName
fn migrate_records_data(conn: &Connection) -> Result<(), String> {
    // 查询 data 字段包含"弓月城"的记录
    let records: Vec<(String, String)> = conn
        .prepare("SELECT id, data FROM records WHERE data LIKE '%弓月城%'")
        .map_err(error_to_string)?
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let data: String = row.get(1)?;
            Ok((id, data))
        })
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    let mut updated_count = 0;
    for (id, data) in records {
        // 解析 JSON
        if let Ok(mut json) = serde_json::from_str::<Value>(&data) {
            // 检查并更新 raidName 字段
            if let Some(raid_name) = json.get("raidName").and_then(|v| v.as_str()) {
                if raid_name.contains("弓月城") {
                    let new_raid_name = raid_name.replace("弓月城", "会战弓月城");
                    json["raidName"] = serde_json::Value::String(new_raid_name);

                    // 序列化回字符串并更新
                    if let Ok(new_data) = serde_json::to_string(&json) {
                        conn.execute(
                            "UPDATE records SET data = ?1 WHERE id = ?2",
                            params![&new_data, &id],
                        ).map_err(error_to_string)?;
                        updated_count += 1;
                    }
                }
            }
        }
    }

    log::info!("V6 迁移：已更新 {} 条 records 记录", updated_count);
    Ok(())
}
