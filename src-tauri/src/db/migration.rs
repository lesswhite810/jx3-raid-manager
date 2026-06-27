use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};

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
        3 => migrations::v3::migrate(conn),
        4 => migrations::v4::migrate(conn),
        5 => migrations::v5::migrate(conn),
        6 => migrations::v6::migrate(conn),
        7 => migrations::v7::migrate(conn),
        8 => migrations::v8::migrate(conn),
        9 => migrations::v9::migrate(conn),
        10 => migrations::v10::migrate(conn),
        11 => migrations::v11::migrate(conn),
        12 => migrations::v12::migrate(conn),
        13 => migrations::v13::migrate(conn),
        14 => migrations::v14::migrate(conn),
        _ => Err(format!("未知的迁移版本: {}", version)),
    }
}

/// 初始化预制副本数据（从 static_raids.json 读取）
/// 使用 INSERT OR IGNORE，不会重复插入已有数据
pub fn init_static_raids(conn: &Connection) -> Result<(), String> {
    init_static_versions_and_seasons(conn)?;

    let static_json = include_str!("static_raids.json");
    let static_raids: Vec<serde_json::Value> =
        serde_json::from_str(static_json).map_err(|e| format!("解析预制副本数据失败: {}", e))?;

    let mut season_id_map: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name FROM seasons")
            .map_err(error_to_string)?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
            .map_err(error_to_string)?;
        for r in iter.flatten() {
            season_id_map.insert(r.0, r.1);
        }
    }

    let mut version_first_season: HashMap<String, String> = HashMap::new();
    let mut season_to_version: HashMap<String, String> = HashMap::new();
    let seasons_json = include_str!("static_seasons.json");
    let seasons: Vec<serde_json::Value> =
        serde_json::from_str(seasons_json).map_err(|e| format!("解析预置赛季数据失败: {}", e))?;
    for s in seasons.iter() {
        let name = s["name"].as_str().unwrap_or_default();
        let version_name = s["version_name"].as_str().unwrap_or_default();
        version_first_season
            .entry(version_name.to_string())
            .or_insert_with(|| name.to_string());
        season_to_version.insert(name.to_string(), version_name.to_string());
    }

    let mut inserted_count = 0;
    let mut boss_inserted_names: HashSet<String> = HashSet::new();
    let mut version_inserted_names: HashSet<String> = HashSet::new();

    for raid in static_raids.iter() {
        let name = raid["name"].as_str().unwrap_or_default();
        let version = raid["version"].as_str().unwrap_or_default();
        let season_name = raid["season"].as_str().unwrap_or_default();

        let target_season = if !season_name.is_empty() {
            Some(season_name.to_string())
        } else if !version.is_empty() {
            version_first_season.get(version).cloned()
        } else {
            None
        };

        let season_id: Option<i64> = target_season.as_ref().and_then(|s| season_id_map.get(s)).copied();

        let effective_version = if !version.is_empty() {
            version.to_string()
        } else if !season_name.is_empty() {
            season_to_version.get(season_name).cloned().unwrap_or_default()
        } else {
            String::new()
        };

        if !effective_version.is_empty() && !version_inserted_names.contains(&effective_version) {
            conn.execute(
                "INSERT OR IGNORE INTO raid_versions (name) VALUES (?)",
                params![&effective_version],
            )
            .map_err(error_to_string)?;
            version_inserted_names.insert(effective_version.clone());
        }

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
                        "INSERT OR IGNORE INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static, season_id) VALUES (?, ?, ?, ?, ?, ?, ?, 1, ?)",
                        params![&id, name, difficulty, player_count, &effective_version, "", is_active, season_id],
                    )
                    .map_err(error_to_string)?;

                inserted_count += changes;
            }
        }

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

fn init_static_versions_and_seasons(conn: &Connection) -> Result<(), String> {
    let timestamp = chrono::Utc::now().to_rfc3339();

    let versions_json = include_str!("static_versions.json");
    let versions: Vec<serde_json::Value> =
        serde_json::from_str(versions_json).map_err(|e| format!("解析预置版本数据失败: {}", e))?;

    let mut version_id_map: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name FROM game_versions")
            .map_err(error_to_string)?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
            .map_err(error_to_string)?;
        for r in iter.flatten() {
            version_id_map.insert(r.0, r.1);
        }
    }

    let mut version_names: Vec<String> = Vec::new();
    let mut version_orders: Vec<i64> = Vec::new();
    for v in versions.iter() {
        let name = v["name"].as_str().unwrap_or_default();
        if !version_id_map.contains_key(name) {
            version_names.push(name.to_string());
            version_orders.push(v["sort_order"].as_i64().unwrap_or(0));
        }
    }

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
        log::info!("初始化：批量插入 {} 个版本", count);
    }

    let seasons_json = include_str!("static_seasons.json");
    let seasons: Vec<serde_json::Value> =
        serde_json::from_str(seasons_json).map_err(|e| format!("解析预置赛季数据失败: {}", e))?;

    let mut season_id_map: HashMap<String, i64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, name FROM seasons")
            .map_err(error_to_string)?;
        let iter = stmt
            .query_map([], |row| Ok((row.get::<_, String>(1)?, row.get::<_, i64>(0)?)))
            .map_err(error_to_string)?;
        for r in iter.flatten() {
            season_id_map.insert(r.0, r.1);
        }
    }

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

        log::info!("初始化：批量插入 {} 个赛季", count);
    }

    Ok(())
}
