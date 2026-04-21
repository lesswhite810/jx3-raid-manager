use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V9 迁移：
/// - 初始化预制副本数据
/// - 确保新增的预制副本（如阆风悬城）被添加到数据库
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V9 迁移开始 ==========");
    log::info!("V9 迁移：初始化预制副本数据");

    init_static_raids(conn)?;

    log::info!("========== V9 迁移完成 ==========");
    Ok(())
}

/// 初始化预制副本数据（从 static_raids.json 读取）
/// 使用 INSERT OR IGNORE，不会重复插入已有数据
fn init_static_raids(conn: &Connection) -> Result<(), String> {
    let static_json = include_str!("../static_raids.json");
    let static_raids: Vec<serde_json::Value> =
        serde_json::from_str(static_json).map_err(|e| format!("解析预制副本数据失败: {}", e))?;

    let mut inserted_count = 0;
    let mut boss_inserted_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut version_inserted_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for raid in static_raids.iter() {
        let name = raid["name"].as_str().unwrap_or_default();
        let version = raid["version"].as_str().unwrap_or_default();

        // 将版本名称入库 raid_versions
        if !version.is_empty() && !version_inserted_names.contains(version) {
            conn.execute(
                "INSERT OR IGNORE INTO raid_versions (name) VALUES (?)",
                rusqlite::params![version],
            )
            .map_err(error_to_string)?;
            version_inserted_names.insert(version.to_string());
        }

        // 插入各难度/人数配置
        if let Some(configs) = raid["configurations"].as_array() {
            for config in configs {
                let player_count = config["playerCount"].as_i64().unwrap_or(25);
                let difficulty = config["difficulty"].as_str().unwrap_or("普通");
                let is_active = if config["isActive"].as_bool().unwrap_or(true) {
                    1
                } else {
                    0
                };
                let id = format!("{}人{}{}", player_count, difficulty, name);

                let changes = conn
                    .execute(
                        "INSERT OR IGNORE INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
                        rusqlite::params![&id, name, difficulty, player_count, version, "", is_active],
                    )
                    .map_err(error_to_string)?;

                inserted_count += changes;
            }
        }

        // boss 按副本名称只写一次
        if !boss_inserted_names.contains(name) {
            if let Some(bosses) = raid["bosses"].as_array() {
                for boss in bosses {
                    let boss_id = boss["id"].as_str().unwrap_or_default();
                    let boss_name = boss["name"].as_str().unwrap_or_default();
                    let boss_order = boss["order"].as_i64().unwrap_or(0);

                    conn.execute(
                        "INSERT OR IGNORE INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
                        rusqlite::params![boss_id, name, boss_name, boss_order],
                    )
                    .map_err(error_to_string)?;
                }
                boss_inserted_names.insert(name.to_string());
            }
        }
    }

    if inserted_count > 0 {
        log::info!("V9 迁移：成功注入 {} 条预制副本数据", inserted_count);
    } else {
        log::info!("V9 迁移：预制副本数据已是最新，无需更新");
    }

    Ok(())
}
