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
        15 => migrations::v15::migrate(conn),
        16 => migrations::v16::migrate(conn),
        17 => migrations::v17::migrate(conn),
        18 => migrations::v18::migrate(conn),
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
                        "INSERT OR IGNORE INTO raid_bosses (raid_name, boss_id, name, boss_order) VALUES (?, ?, ?, ?)",
                        params![name, boss_id, boss_name, boss_order],
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

    let inserted_new_version = !version_names.is_empty();
    if inserted_new_version {
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

        // 前端会缓存版本列表一天；新预置版本写入后必须让它重新按 sort_order 读取。
        conn.execute("DELETE FROM cache WHERE key = 'raid_versions_cache_v2'", [])
            .map_err(error_to_string)?;
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

    // 静态同步通常只插入缺失行。新赛季上线时，仅为仍保持预置起始日期、
    // 且结束日期为空或 0 的既有赛季补齐结束时间，不覆盖用户手工设置的日期。
    for s in seasons.iter() {
        let name = s["name"].as_str().unwrap_or_default();
        let version_name = s["version_name"].as_str().unwrap_or_default();
        let start_date = s["start_date"].as_i64().unwrap_or(0);
        let end_date = s["end_date"].as_i64().unwrap_or(0);

        if end_date == 0 || !season_id_map.contains_key(name) {
            continue;
        }

        if let Some(&version_id) = version_id_map.get(version_name) {
            conn.execute(
                "UPDATE seasons
                 SET end_date = ?
                 WHERE name = ?
                   AND version_id = ?
                   AND start_date = ?
                   AND (end_date IS NULL OR end_date = 0)",
                params![end_date, name, version_id, start_date],
            )
            .map_err(error_to_string)?;
        }
    }

    // 静态赛季的品级边界可在赛季上线后逐项确认。对已存在的预置赛季，
    // 仅回填 NULL/0 这两种历史占位值；任何用户已设置的正值及赛季日期
    // 都保持不变。这里基于 static_seasons 的通用映射，不按赛季名称分支。
    for s in seasons.iter() {
        let name = s["name"].as_str().unwrap_or_default();
        let version_name = s["version_name"].as_str().unwrap_or_default();
        let min = s["trial_equip_level_min"].as_i64().unwrap_or(0);
        let max = s["trial_equip_level_max"].as_i64().unwrap_or(0);

        if min <= 0 && max <= 0 {
            continue;
        }

        if let Some(&version_id) = version_id_map.get(version_name) {
            conn.execute(
                "UPDATE seasons
                 SET trial_equip_level_min = CASE
                         WHEN ? > 0 AND (trial_equip_level_min IS NULL OR trial_equip_level_min = 0)
                         THEN ? ELSE trial_equip_level_min END,
                     trial_equip_level_max = CASE
                         WHEN ? > 0 AND (trial_equip_level_max IS NULL OR trial_equip_level_max = 0)
                         THEN ? ELSE trial_equip_level_max END
                 WHERE name = ? AND version_id = ?",
                params![min, min, max, max, name, version_id],
            )
            .map_err(error_to_string)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DARK_SHADOW_START: i64 = 1_776_898_800;
    const SEASON_TRANSITION: i64 = 1_793_228_400;

    fn setup_v18_database(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE schema_versions (version INTEGER NOT NULL);
            INSERT INTO schema_versions (version) VALUES (18);

            CREATE TABLE game_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE seasons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                start_date INTEGER NOT NULL,
                end_date INTEGER,
                sort_order INTEGER NOT NULL DEFAULT 0,
                trial_equip_level_min INTEGER DEFAULT 0,
                trial_equip_level_max INTEGER DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE raids (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                difficulty TEXT NOT NULL DEFAULT '普通',
                player_count INTEGER NOT NULL DEFAULT 25,
                version TEXT,
                notes TEXT,
                is_active INTEGER DEFAULT 1,
                is_static INTEGER DEFAULT 0,
                season_id INTEGER
            );
            CREATE TABLE raid_bosses (
                raid_name TEXT NOT NULL,
                boss_id TEXT NOT NULL,
                name TEXT NOT NULL,
                boss_order INTEGER NOT NULL,
                PRIMARY KEY (raid_name, boss_id)
            );
            CREATE TABLE cache (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT
            );
            CREATE TABLE records (
                id TEXT PRIMARY KEY,
                data TEXT,
                raid_name TEXT
            );
            "#,
        )
        .expect("V18 数据库前置表应创建成功");
    }

    fn seed_existing_dark_shadow(
        conn: &Connection,
        start_date: i64,
        end_date: Option<i64>,
    ) -> i64 {
        conn.execute(
            "INSERT INTO game_versions (name, sort_order, created_at) VALUES ('丝路风雨', 8, 'old')",
            [],
        )
        .expect("应写入旧版本");
        let version_id: i64 = conn
            .query_row(
                "SELECT id FROM game_versions WHERE name = '丝路风雨'",
                [],
                |row| row.get(0),
            )
            .expect("应读取旧版本 ID");
        conn.execute(
            "INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, created_at)
             VALUES ('暗影千机', ?, ?, ?, 4, 'old')",
            params![version_id, start_date, end_date],
        )
        .expect("应写入旧赛季");
        version_id
    }

    fn season_name_for_timestamp(conn: &Connection, timestamp: i64) -> Option<String> {
        conn.query_row(
            "SELECT name FROM seasons
             WHERE start_date <= ?
               AND (end_date IS NULL OR end_date = 0 OR end_date > ?)
             ORDER BY sort_order DESC
             LIMIT 1",
            params![timestamp, timestamp],
            |row| row.get(0),
        )
        .ok()
    }

    #[test]
    fn static_sync_initializes_cangsheng_seed_in_empty_v18_database() {
        let conn = Connection::open_in_memory().expect("应创建内存数据库");
        setup_v18_database(&conn);

        init_static_raids(&conn).expect("空数据库静态同步应成功");

        let seeded_counts: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM game_versions WHERE name = '苍生铸世'),
                    (SELECT COUNT(*) FROM seasons WHERE name = '苍生铸世'),
                    (SELECT COUNT(*) FROM raids WHERE name = '洛阳之战'),
                    (SELECT COUNT(*) FROM raid_bosses WHERE raid_name = '洛阳之战')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("应统计空数据库预置结果");
        assert_eq!(seeded_counts, (1, 1, 3, 5));

        let equip_bounds: (i64, Option<i64>) = conn
            .query_row(
                "SELECT trial_equip_level_min, trial_equip_level_max
                 FROM seasons WHERE name = '苍生铸世'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("新数据库应写入当前赛季最低试炼品级");
        assert_eq!(equip_bounds, (900, Some(0)));
    }

    #[test]
    fn static_sync_backfills_only_placeholder_trial_bounds_idempotently() {
        let conn = Connection::open_in_memory().expect("应创建内存数据库");
        setup_v18_database(&conn);
        conn.execute(
            "INSERT INTO game_versions (name, sort_order, created_at) VALUES ('苍生铸世', 9, 'old')",
            [],
        )
        .expect("应写入当前版本");
        let version_id: i64 = conn
            .query_row(
                "SELECT id FROM game_versions WHERE name = '苍生铸世'",
                [],
                |row| row.get(0),
            )
            .expect("应读取当前版本 ID");
        conn.execute(
            "INSERT INTO seasons
             (name, version_id, start_date, end_date, sort_order,
              trial_equip_level_min, trial_equip_level_max, created_at)
             VALUES ('苍生铸世', ?, 123, 456, 5, NULL, NULL, 'old')",
            params![version_id],
        )
        .expect("应写入旧版 NULL 占位范围");

        init_static_raids(&conn).expect("静态同步应回填 NULL 占位范围");
        let backfilled: (i64, Option<i64>, i64, i64) = conn
            .query_row(
                "SELECT trial_equip_level_min, trial_equip_level_max, start_date, end_date
                 FROM seasons WHERE name = '苍生铸世' AND version_id = ?",
                params![version_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("应读取回填结果");
        assert_eq!(backfilled, (900, None, 123, 456));

        conn.execute(
            "UPDATE seasons SET trial_equip_level_min = 1000, trial_equip_level_max = 1200
             WHERE name = '苍生铸世' AND version_id = ?",
            params![version_id],
        )
        .expect("应模拟用户自定义正范围");
        init_static_raids(&conn).expect("重复同步应成功");

        let retained: (i64, i64, i64) = conn
            .query_row(
                "SELECT trial_equip_level_min, trial_equip_level_max,
                        (SELECT COUNT(*) FROM seasons WHERE name = '苍生铸世' AND version_id = ?)
                 FROM seasons WHERE name = '苍生铸世' AND version_id = ?",
                params![version_id, version_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("应读取重复同步后的范围");
        assert_eq!(retained, (1000, 1200, 1));
    }

    #[test]
    fn static_sync_adds_cangsheng_seed_to_existing_v18_database_idempotently() {
        let conn = Connection::open_in_memory().expect("应创建内存数据库");
        setup_v18_database(&conn);
        let silk_road_version_id = seed_existing_dark_shadow(&conn, DARK_SHADOW_START, Some(0));
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static)
             VALUES ('10人普通阆风悬城', '阆风悬城', '普通', 10, '丝路风雨', '', 0, 1)",
            [],
        )
        .expect("应写入用户禁用的旧预置副本");
        conn.execute(
            "INSERT INTO records (id, data, raid_name)
             VALUES ('legacy-record', '{\"amount\":100}', '阆风悬城')",
            [],
        )
        .expect("应写入历史副本记录");
        conn.execute(
            "INSERT INTO cache (key, value, updated_at) VALUES ('raid_versions_cache_v2', 'stale', 'old')",
            [],
        )
        .expect("应写入旧版本缓存");

        init_static_raids(&conn).expect("首次静态同步应成功");

        let version: (i64, i64) = conn
            .query_row(
                "SELECT id, sort_order FROM game_versions WHERE name = '苍生铸世'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("应插入苍生铸世版本");
        assert_eq!(version.1, 9);

        let season: (i64, i64, i64, i64, i64) = conn
            .query_row(
                "SELECT id, version_id, start_date, end_date, sort_order FROM seasons WHERE name = '苍生铸世'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("应插入苍生铸世赛季");
        assert_eq!(
            (season.1, season.2, season.3, season.4),
            (version.0, SEASON_TRANSITION, 0, 5)
        );

        let dark_shadow_end: i64 = conn
            .query_row(
                "SELECT end_date FROM seasons WHERE name = '暗影千机' AND version_id = ?",
                params![silk_road_version_id],
                |row| row.get(0),
            )
            .expect("应保留旧赛季");
        assert_eq!(dark_shadow_end, SEASON_TRANSITION);
        assert_eq!(
            season_name_for_timestamp(&conn, SEASON_TRANSITION - 1),
            Some("暗影千机".to_string())
        );
        assert_eq!(
            season_name_for_timestamp(&conn, SEASON_TRANSITION),
            Some("苍生铸世".to_string())
        );

        let configurations: Vec<(i64, String, i64, String, i64)> = conn
            .prepare(
                "SELECT player_count, difficulty, is_active, version, season_id
                 FROM raids
                 WHERE name = '洛阳之战'
                 ORDER BY player_count, CASE difficulty WHEN '普通' THEN 0 ELSE 1 END",
            )
            .expect("应准备洛阳之战配置查询")
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("应查询洛阳之战配置")
            .collect::<Result<Vec<_>, _>>()
            .expect("应读取洛阳之战配置");
        assert_eq!(
            configurations,
            vec![
                (10, "普通".to_string(), 1, "苍生铸世".to_string(), season.0),
                (25, "普通".to_string(), 1, "苍生铸世".to_string(), season.0),
                (25, "英雄".to_string(), 1, "苍生铸世".to_string(), season.0),
            ]
        );

        let bosses: Vec<(String, String, i64)> = conn
            .prepare(
                "SELECT boss_id, name, boss_order FROM raid_bosses
                 WHERE raid_name = '洛阳之战' ORDER BY boss_order",
            )
            .expect("应准备 BOSS 查询")
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("应查询 BOSS")
            .collect::<Result<Vec<_>, _>>()
            .expect("应读取 BOSS 列表");
        assert_eq!(
            bosses,
            vec![
                ("luoyangzhizhan_1".to_string(), "突利和顺".to_string(), 1),
                ("luoyangzhizhan_2".to_string(), "田承嗣".to_string(), 2),
                ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string(), 3),
                ("luoyangzhizhan_4".to_string(), "阿史那承庆".to_string(), 4),
                ("luoyangzhizhan_5".to_string(), "史朝义".to_string(), 5),
            ]
        );

        let static_raid: serde_json::Value = serde_json::from_str(include_str!("static_raids.json"))
            .expect("静态副本 JSON 应合法");
        assert_eq!(
            static_raid
                .as_array()
                .and_then(|raids| {
                    raids
                        .iter()
                        .find(|raid| raid["name"].as_str() == Some("洛阳之战"))
                })
                .and_then(|raid| raid["level"].as_i64()),
            Some(50)
        );

        let old_raid_active: i64 = conn
            .query_row(
                "SELECT is_active FROM raids WHERE id = '10人普通阆风悬城'",
                [],
                |row| row.get(0),
            )
            .expect("旧预置副本应保留");
        assert_eq!(old_raid_active, 0);
        let historical_record_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records WHERE id = 'legacy-record'", [], |row| {
                row.get(0)
            })
            .expect("历史记录应保留");
        assert_eq!(historical_record_count, 1);
        let invalidated_cache_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cache WHERE key = 'raid_versions_cache_v2'",
                [],
                |row| row.get(0),
            )
            .expect("应查询版本缓存");
        assert_eq!(invalidated_cache_count, 0);

        conn.execute(
            "INSERT INTO cache (key, value, updated_at) VALUES ('raid_versions_cache_v2', 'fresh', 'new')",
            [],
        )
        .expect("应写入刷新后的版本缓存");
        init_static_raids(&conn).expect("重复静态同步应成功");

        let duplicate_counts: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT
                    (SELECT COUNT(*) FROM game_versions WHERE name = '苍生铸世'),
                    (SELECT COUNT(*) FROM seasons WHERE name = '苍生铸世'),
                    (SELECT COUNT(*) FROM raids WHERE name = '洛阳之战'),
                    (SELECT COUNT(*) FROM raid_bosses WHERE raid_name = '洛阳之战')",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("应统计重复同步结果");
        assert_eq!(duplicate_counts, (1, 1, 3, 5));
        let refreshed_cache: String = conn
            .query_row(
                "SELECT value FROM cache WHERE key = 'raid_versions_cache_v2'",
                [],
                |row| row.get(0),
            )
            .expect("重复同步不应清除未新增版本时的缓存");
        assert_eq!(refreshed_cache, "fresh");
    }

    #[test]
    fn static_sync_does_not_overwrite_user_modified_dark_shadow_dates() {
        let explicit_end_conn = Connection::open_in_memory().expect("应创建内存数据库");
        setup_v18_database(&explicit_end_conn);
        let silk_road_version_id =
            seed_existing_dark_shadow(&explicit_end_conn, DARK_SHADOW_START, Some(1_790_000_000));

        init_static_raids(&explicit_end_conn).expect("静态同步应成功");
        let explicit_end: i64 = explicit_end_conn
            .query_row(
                "SELECT end_date FROM seasons WHERE name = '暗影千机' AND version_id = ?",
                params![silk_road_version_id],
                |row| row.get(0),
            )
            .expect("应读取用户设置的结束日期");
        assert_eq!(explicit_end, 1_790_000_000);

        let custom_start_conn = Connection::open_in_memory().expect("应创建内存数据库");
        setup_v18_database(&custom_start_conn);
        let custom_version_id = seed_existing_dark_shadow(&custom_start_conn, 1_776_898_801, Some(0));

        init_static_raids(&custom_start_conn).expect("静态同步应成功");
        let custom_dates: (i64, i64) = custom_start_conn
            .query_row(
                "SELECT start_date, end_date FROM seasons WHERE name = '暗影千机' AND version_id = ?",
                params![custom_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("应读取用户设置的起始日期");
        assert_eq!(custom_dates, (1_776_898_801, 0));
    }
}
