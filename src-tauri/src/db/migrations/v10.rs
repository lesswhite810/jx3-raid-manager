use crate::db::migration::error_to_string;
use rusqlite::{params, Connection};

/// V10 迁移：引入赛季系统
///
/// 迁移内容：
/// - 创建 game_versions 表（版本表）
/// - 创建 seasons 表（赛季表）
/// - 为 raids 表添加 season_id 外键
/// - 为 raid_records 表添加 season_id 外键
/// - 预置版本和赛季数据（从 static_seasons.json 读取）
/// - 建立索引优化查询性能
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V10 迁移开始 ==========");
    log::info!("V10 迁移：引入赛季系统");

    create_tables(conn)?;
    init_static_seasons(conn)?;
    add_season_id_to_raids(conn)?;
    add_season_id_to_records(conn)?;
    create_indexes(conn)?;

    log::info!("========== V10 迁移完成 ==========");
    Ok(())
}

fn create_tables(conn: &Connection) -> Result<(), String> {
    log::info!("V10 迁移：创建 game_versions 表");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS game_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS seasons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            version_id INTEGER NOT NULL,
            start_date INTEGER NOT NULL,
            end_date INTEGER,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (version_id) REFERENCES game_versions(id)
        );
        "#,
    )
    .map_err(error_to_string)?;

    log::info!("V10 迁移：表结构创建完成");
    Ok(())
}

fn init_static_seasons(conn: &Connection) -> Result<(), String> {
    log::info!("V10 迁移：初始化预置版本和赛季数据");

    let versions_json = include_str!("../static_versions.json");
    let seasons_json = include_str!("../static_seasons.json");

    let versions: Vec<serde_json::Value> =
        serde_json::from_str(versions_json).map_err(|e| format!("解析预置版本数据失败: {}", e))?;
    let seasons: Vec<serde_json::Value> =
        serde_json::from_str(seasons_json).map_err(|e| format!("解析预置赛季数据失败: {}", e))?;

    let timestamp = chrono::Utc::now().to_rfc3339();

    // 插入版本数据
    for version_data in versions.iter() {
        let name = version_data["name"].as_str().unwrap_or_default();
        let sort_order = version_data["sort_order"].as_i64().unwrap_or(0);

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM game_versions WHERE name = ?",
                params![name],
                |row| row.get(0),
            )
            .ok();

        if existing.is_none() {
            conn.execute(
                "INSERT INTO game_versions (name, sort_order, created_at) VALUES (?, ?, ?)",
                params![name, sort_order, &timestamp],
            )
            .map_err(error_to_string)?;
            log::info!("V10 迁移：插入版本 {} (sort_order: {})", name, sort_order);
        }
    }

    // 插入赛季数据
    for season_data in seasons.iter() {
        let season_name = season_data["name"].as_str().unwrap_or_default();
        let version_name = season_data["version_name"].as_str().unwrap_or_default();
        let start_date = season_data["start_date"].as_i64().unwrap_or(0);
        let end_date = season_data["end_date"].as_i64().unwrap_or(0);
        let sort_order = season_data["sort_order"].as_i64().unwrap_or(0);

        let version_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM game_versions WHERE name = ?",
                params![version_name],
                |row| row.get(0),
            )
            .ok();

        let Some(vid) = version_id else {
            log::warn!("V10 迁移：未找到版本 {}，跳过赛季 {}", version_name, season_name);
            continue;
        };

        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM seasons WHERE name = ?",
                params![season_name],
                |row| row.get(0),
            )
            .ok();

        if existing.is_none() {
            conn.execute(
                "INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![season_name, vid, start_date, end_date, sort_order, &timestamp],
            )
            .map_err(error_to_string)?;
            log::info!("V10 迁移：插入赛季 {} (version: {})", season_name, version_name);
        }
    }

    Ok(())
}

fn add_season_id_to_raids(conn: &Connection) -> Result<(), String> {
    log::info!("V10 迁移：为 raids 表添加 season_id 列");

    // 检查列是否存在
    let has_column: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('raids') WHERE name='season_id'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if has_column == 0 {
        conn.execute(
            "ALTER TABLE raids ADD COLUMN season_id INTEGER REFERENCES seasons(id)",
            [],
        )
        .map_err(error_to_string)?;
    }

    // 读取静态副本数据，获取 season 字段
    let static_json = include_str!("../static_raids.json");
    let static_raids: Vec<serde_json::Value> =
        serde_json::from_str(static_json).map_err(|e| format!("解析静态副本数据失败: {}", e))?;

    for raid in static_raids.iter() {
        let raid_name = raid["name"].as_str().unwrap_or_default();
        let season_name = raid["season"].as_str();
        let version_name = raid["version"].as_str();

        let season_to_use = if let Some(season) = season_name {
            // 直接使用 season 字段
            Some(season.to_string())
        } else if let Some(version) = version_name {
            // 查找版本对应的第一个赛季
            conn.query_row(
                "SELECT s.name FROM seasons s JOIN game_versions gv ON s.version_id = gv.id WHERE gv.name = ? ORDER BY s.sort_order LIMIT 1",
                params![version],
                |row| row.get::<_, String>(0),
            )
            .ok()
        } else {
            None
        };

        if let Some(season) = season_to_use {
            // 查找赛季 ID
            let season_id: Option<i64> = conn
                .query_row(
                    "SELECT s.id FROM seasons s WHERE s.name = ? LIMIT 1",
                    params![season],
                    |row| row.get(0),
                )
                .ok();

            if let Some(sid) = season_id {
                // 更新该副本下所有配置的 season_id
                conn.execute(
                    "UPDATE raids SET season_id = ? WHERE name = ? AND season_id IS NULL",
                    params![sid, raid_name],
                )
                .map_err(error_to_string)?;
            }
        }
    }

    log::info!("V10 迁移：raids.season_id 关联完成");
    Ok(())
}

fn add_season_id_to_records(conn: &Connection) -> Result<(), String> {
    log::info!("V10 迁移：为 raid_records 表添加 season_id 列");

    // 检查列是否存在
    let has_column: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('raid_records') WHERE name='season_id'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if has_column == 0 {
        conn.execute(
            "ALTER TABLE raid_records ADD COLUMN season_id INTEGER REFERENCES seasons(id)",
            [],
        )
        .map_err(error_to_string)?;

        log::info!("V10 迁移：raid_records.season_id 列已添加");
    }

    Ok(())
}

fn create_indexes(conn: &Connection) -> Result<(), String> {
    log::info!("V10 迁移：创建索引");

    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_seasons_version_id ON seasons(version_id);
        CREATE INDEX IF NOT EXISTS idx_raids_season_id ON raids(season_id);
        CREATE INDEX IF NOT EXISTS idx_raid_records_season_id ON raid_records(season_id);
        "#,
    )
    .map_err(error_to_string)?;

    log::info!("V10 迁移：索引创建完成");
    Ok(())
}
