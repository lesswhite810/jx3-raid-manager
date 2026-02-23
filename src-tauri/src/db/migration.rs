use rusqlite::{params, Connection};
use std::collections::HashSet;

// 直接引用同级 migrations 模块
use super::migrations;

/// 错误转换辅助函数
pub fn error_to_string(e: rusqlite::Error) -> String {
    format!("Database error: {}", e)
}

/// 执行指定版本的迁移脚本
///
/// 注意：此函数只负责执行数据迁移，不检查表结构
/// 调用方应该根据 schema_versions 表中的版本号来决定是否调用此函数
pub fn apply_migration(conn: &Connection, version: i32) -> Result<(), String> {
    match version {
        1 => migrations::v1::migrate(conn),
        2 => migrations::v2::migrate(conn),
        _ => Err(format!("未知的迁移版本: {}", version)),
    }
}

/// 初始化预制副本数据（从 static_raids.json 读取）
/// 使用 INSERT OR IGNORE，不会重复插入已有数据
pub fn init_static_raids(conn: &Connection) -> Result<(), String> {
    let static_json = include_str!("static_raids.json");
    let static_raids: Vec<serde_json::Value> = serde_json::from_str(static_json)
        .map_err(|e| format!("解析预制副本数据失败: {}", e))?;

    let mut inserted_count = 0;
    let mut boss_inserted_names: HashSet<String> = HashSet::new();
    let mut version_inserted_names: HashSet<String> = HashSet::new();

    for raid in static_raids {
        let name = raid["name"].as_str().unwrap_or_default();
        let version = raid["version"].as_str().unwrap_or_default();

        // 将版本名称入库 raid_versions
        if !version.is_empty() && !version_inserted_names.contains(version) {
            conn.execute(
                "INSERT OR IGNORE INTO raid_versions (name) VALUES (?)",
                params![version],
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
                        params![&id, name, difficulty, player_count, version, "", is_active],
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
                        params![boss_id, name, boss_name, boss_order],
                    )
                    .map_err(error_to_string)?;
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
