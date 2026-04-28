use crate::db::migration::error_to_string;
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// V10 迁移：引入赛季系统（批量插入优化版）
///
/// 优化点：
/// - 批量插入版本和赛季，单条 SQL 语句
/// - 事务包裹所有操作
/// - HashMap 预加载替代重复查询
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V10 迁移开始 ==========");
    log::info!("V10 迁移：引入赛季系统");

    // 注意：事务由调用方管理，不需要手动 BEGIN TRANSACTION

    if let Err(e) = do_migrate(conn) {
        return Err(e);
    }

    log::info!("========== V10 迁移完成 ==========");
    Ok(())
}

fn create_tables(conn: &Connection) -> Result<(), String> {
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
            trial_equip_level_min INTEGER DEFAULT 0,
            trial_equip_level_max INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (version_id) REFERENCES game_versions(id)
        );
        "#,
    )
    .map_err(error_to_string)?;

    Ok(())
}

fn do_migrate(conn: &Connection) -> Result<(), String> {
    create_tables(conn)?;

    let timestamp = chrono::Utc::now().to_rfc3339();

    let versions_json = include_str!("../static_versions.json");
    let seasons_json = include_str!("../static_seasons.json");
    let raids_json = include_str!("../static_raids.json");

    let versions: Vec<serde_json::Value> =
        serde_json::from_str(versions_json).map_err(|e| format!("解析预置版本数据失败: {}", e))?;
    let seasons: Vec<serde_json::Value> =
        serde_json::from_str(seasons_json).map_err(|e| format!("解析预置赛季数据失败: {}", e))?;
    let raids: Vec<serde_json::Value> =
        serde_json::from_str(raids_json).map_err(|e| format!("解析静态副本数据失败: {}", e))?;

    // 预加载已存在的版本
    let mut version_id_map: HashMap<String, i64> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT id, name FROM game_versions")
        .map_err(error_to_string)?;
    let iter = stmt
        .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
        .map_err(error_to_string)?;
    for r in iter.flatten() {
        version_id_map.insert(r.0, r.1);
    }

    // 预加载已存在的赛季
    let mut season_id_map: HashMap<String, i64> = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT id, name FROM seasons")
        .map_err(error_to_string)?;
    let iter = stmt
        .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
        .map_err(error_to_string)?;
    for r in iter.flatten() {
        season_id_map.insert(r.0, r.1);
    }

    // 收集待插入的版本
    let mut version_names: Vec<String> = Vec::new();
    let mut version_orders: Vec<i64> = Vec::new();
    for v in versions.iter() {
        let name = v["name"].as_str().unwrap_or_default();
        if !version_id_map.contains_key(name) {
            version_names.push(name.to_string());
            version_orders.push(v["sort_order"].as_i64().unwrap_or(0));
        }
    }

    // 批量插入版本
    if !version_names.is_empty() {
        let count = version_names.len();
        let placeholders: Vec<String> = (0..count).map(|_| "(?, ?, ?)".to_string()).collect();
        let sql = format!(
            "INSERT INTO game_versions (name, sort_order, created_at) VALUES {}",
            placeholders.join(", ")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        for (i, name) in version_names.iter().enumerate() {
            params_vec.push(Box::new(name.clone()));
            params_vec.push(Box::new(version_orders[i]));
            params_vec.push(Box::new(timestamp.clone()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(error_to_string)?;

        // 重新加载版本ID
        let mut stmt = conn
            .prepare("SELECT id, name FROM game_versions")
            .map_err(error_to_string)?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
            .map_err(error_to_string)?;
        version_id_map.clear();
        for r in iter.flatten() {
            version_id_map.insert(r.0, r.1);
        }
        log::info!("V10 迁移：批量插入 {} 个版本", count);
    }

    // 收集待插入的赛季
    let mut season_names: Vec<String> = Vec::new();
    let mut season_version_ids: Vec<i64> = Vec::new();
    let mut season_starts: Vec<i64> = Vec::new();
    let mut season_ends: Vec<i64> = Vec::new();
    let mut season_orders: Vec<i64> = Vec::new();
    let mut season_trial_mins: Vec<i64> = Vec::new();
    let mut season_trial_maxs: Vec<i64> = Vec::new();

    for s in seasons.iter() {
        let name = s["name"].as_str().unwrap_or_default();
        if !season_id_map.contains_key(name) {
            let version_name = s["version_name"].as_str().unwrap_or_default();
            if let Some(&version_id) = version_id_map.get(version_name) {
                season_names.push(name.to_string());
                season_version_ids.push(version_id);
                season_starts.push(s["start_date"].as_i64().unwrap_or(0));
                season_ends.push(s["end_date"].as_i64().unwrap_or(0));
                season_orders.push(s["sort_order"].as_i64().unwrap_or(0));
                season_trial_mins.push(s["trial_equip_level_min"].as_i64().unwrap_or(0));
                season_trial_maxs.push(s["trial_equip_level_max"].as_i64().unwrap_or(0));
            }
        }
    }

    // 批量插入赛季
    if !season_names.is_empty() {
        let count = season_names.len();
        let placeholders: Vec<String> = (0..count).map(|_| "(?, ?, ?, ?, ?, ?, ?, ?)".to_string()).collect();
        let sql = format!(
            "INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max, created_at) VALUES {}",
            placeholders.join(", ")
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        for i in 0..count {
            params_vec.push(Box::new(season_names[i].clone()));
            params_vec.push(Box::new(season_version_ids[i]));
            params_vec.push(Box::new(season_starts[i]));
            params_vec.push(Box::new(season_ends[i]));
            params_vec.push(Box::new(season_orders[i]));
            params_vec.push(Box::new(season_trial_mins[i]));
            params_vec.push(Box::new(season_trial_maxs[i]));
            params_vec.push(Box::new(timestamp.clone()));
        }
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())
            .map_err(error_to_string)?;

        // 重新加载赛季ID
        let mut stmt = conn
            .prepare("SELECT id, name FROM seasons")
            .map_err(error_to_string)?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
            .map_err(error_to_string)?;
        season_id_map.clear();
        for r in iter.flatten() {
            season_id_map.insert(r.0, r.1);
        }
        log::info!("V10 迁移：批量插入 {} 个赛季", count);
    }

    // 添加 season_id 列
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

    // 构建版本名到第一个赛季名的映射
    let mut version_first_season: HashMap<String, String> = HashMap::new();
    for s in seasons.iter() {
        let name = s["name"].as_str().unwrap_or_default();
        let version_name = s["version_name"].as_str().unwrap_or_default();
        version_first_season
            .entry(version_name.to_string())
            .or_insert_with(|| name.to_string());
    }

    // 更新副本的 season_id
    let mut update_count = 0;
    for raid in raids.iter() {
        let raid_name = raid["name"].as_str().unwrap_or_default();
        let season_name = raid["season"].as_str().unwrap_or_default();
        let version_name = raid["version"].as_str().unwrap_or_default();

        let target_season = if !season_name.is_empty() {
            Some(season_name.to_string())
        } else if !version_name.is_empty() {
            version_first_season.get(version_name).cloned()
        } else {
            None
        };

        if let Some(season) = target_season {
            if let Some(&season_id) = season_id_map.get(&season) {
                let rows = conn.execute(
                    "UPDATE raids SET season_id = ? WHERE name = ? AND season_id IS NULL",
                    params![season_id, raid_name],
                )
                .map_err(error_to_string)?;
                update_count += rows;
            }
        }
    }
    log::info!("V10 迁移：更新 {} 个副本配置的 season_id", update_count);

    // 创建索引
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_seasons_version_id ON seasons(version_id);
        CREATE INDEX IF NOT EXISTS idx_raids_season_id ON raids(season_id);
        "#,
    )
    .map_err(error_to_string)?;

    Ok(())
}
