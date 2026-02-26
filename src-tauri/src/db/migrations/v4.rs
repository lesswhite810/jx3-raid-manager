use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V4 迁移：创建副本收藏表
///
/// 变更内容：
/// - 创建 favorite_raids 表，存储收藏的副本名称
/// - 清空 raids 表并重新插入静态数据（确保 rowid 顺序正确）
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V4 迁移：创建副本收藏表...");

    // 1. 创建收藏表
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS favorite_raids (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            raid_name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(error_to_string)?;

    // 2. 清空 raids 表并重新插入静态数据（确保 rowid 顺序正确）
    conn.execute("DELETE FROM raids", [])
        .map_err(error_to_string)?;
    log::info!("V4 迁移：已清空 raids 表");

    // 从 static_raids.json 读取并插入数据
    let static_json = include_str!("../static_raids.json");
    let static_raids: Vec<serde_json::Value> =
        serde_json::from_str(static_json).map_err(|e| format!("解析预制副本数据失败: {}", e))?;

    let mut inserted_count = 0;
    for raid in static_raids.iter() {
        let name = raid["name"].as_str().unwrap_or_default();
        let version = raid["version"].as_str().unwrap_or_default();

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

                conn.execute(
                    "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, 1)",
                    rusqlite::params![&id, name, difficulty, player_count, version, "", is_active],
                )
                .map_err(error_to_string)?;

                inserted_count += 1;
            }
        }
    }

    log::info!("V4 迁移：重新插入 {} 条 raids 数据", inserted_count);
    log::info!("V4 迁移：favorite_raids 表创建完成");
    Ok(())
}
