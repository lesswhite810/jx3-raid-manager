use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub id: i32,
    pub name: String,
    pub game_version_id: i32,
    #[serde(default)]
    pub trial_equip_level_min: Option<i32>,
    #[serde(default)]
    pub trial_equip_level_max: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JX3EquipResponse {
    total: i32,
    per: i32,
    pages: i32,
    page: i32,
    list: Vec<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("HTTP请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("数据库错误: {0}")]
    Database(String),
    #[allow(dead_code)]
    #[error("赛季数据无效")]
    InvalidSeason,
}

pub struct EquipSync;

impl EquipSync {
    pub async fn sync_if_needed() -> Result<usize, SyncError> {
        // 单连接复用：原流程 4 次 init_db（get_current_season + is_season_synced
        // + save_equipments + mark_season_synced），合并为 1 次
        let conn = crate::db::init_db().map_err(|e: String| SyncError::Database(e))?;

        let season = match Self::get_current_season_with_conn(&conn)? {
            Some(s) => s,
            None => {
                info!("[EquipSync] 未获取到当前赛季信息，跳过装备同步");
                return Ok(0);
            }
        };

        let cache_key = format!("equip_cache_{}", season.name);
        if Self::is_season_synced_with_conn(&conn, &cache_key)? {
            info!("[EquipSync] 赛季「{}」已同步，跳过", season.name);
            return Ok(0);
        }

        let range = match Self::resolve_equip_range(
            season.trial_equip_level_min,
            season.trial_equip_level_max,
        ) {
            Ok(range) => range,
            Err(reason) => {
                warn!(
                    "[EquipSync] 赛季「{}」{}，跳过同步",
                    season.name, reason
                );
                return Ok(0);
            }
        };

        match range {
            Some((min, max)) => info!(
                "[EquipSync] 开始同步赛季「{}」装备，范围: {}-{}",
                season.name, min, max
            ),
            None => info!(
                "[EquipSync] 赛季「{}」未配置品级范围，同步全部无修装备",
                season.name
            ),
        }

        let start_time = std::time::Instant::now();

        let armor_items = Self::fetch_equipments("armor", range).await?;
        let trinket_items = Self::fetch_equipments("trinket", range).await?;
        let weapon_items = Self::fetch_equipments("weapon", range).await?;

        let total_items = armor_items.len() + trinket_items.len() + weapon_items.len();
        info!("[EquipSync] 获取到 {} 件装备（防具: {}, 饰品: {}, 武器: {}）", total_items, armor_items.len(), trinket_items.len(), weapon_items.len());

        if total_items == 0 {
            warn!("[EquipSync] 未获取到任何装备，保留现有装备数据并跳过写入");
            return Ok(0);
        }

        let all_items: Vec<serde_json::Value> = armor_items
            .iter()
            .chain(trinket_items.iter())
            .chain(weapon_items.iter())
            .cloned()
            .collect();
        if let Some((min, max)) = Self::derive_level_range(&all_items) {
            info!("[EquipSync] 实际同步品级范围: {}-{}", min, max);
        }

        Self::save_equipments_with_conn(&conn, &armor_items, &trinket_items, &weapon_items)?;
        Self::mark_season_synced_with_conn(&conn, &cache_key)?;

        let duration = start_time.elapsed();
        info!("[EquipSync] ✓ 赛季「{}」同步完成！共同步 {} 件装备，耗时 {:?}", season.name, total_items, duration);

        Ok(total_items)
    }

    fn get_current_season_with_conn(conn: &rusqlite::Connection) -> Result<Option<Season>, SyncError> {
        // 使用 Unix 时间戳进行比较
        let now_ts = chrono::Utc::now().timestamp();

        let result: Result<Season, rusqlite::Error> = conn.query_row(
            "SELECT id, name, version_id, trial_equip_level_min, trial_equip_level_max
             FROM seasons
             WHERE (start_date = 0 OR start_date <= ?) AND (end_date = 0 OR end_date >= ?)
             ORDER BY start_date DESC
             LIMIT 1",
            [now_ts, now_ts],
            |row| {
                Ok(Season {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    game_version_id: row.get(2)?,
                    trial_equip_level_min: row.get(3)?,
                    trial_equip_level_max: row.get(4)?,
                })
            },
        );

        match result {
            Ok(season) => {
                info!("[EquipSync] 找到当前赛季: {} (min={:?}, max={:?})",
                    season.name, season.trial_equip_level_min, season.trial_equip_level_max);
                Ok(Some(season))
            },
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                info!("[EquipSync] 未找到当前赛季，now_ts={}", now_ts);
                Ok(None)
            },
            Err(e) => Err(SyncError::Database(e.to_string())),
        }
    }

    /// 解析赛季试炼装备品级范围。
    ///
    /// - 未配置（或配置为 0）→ 不限范围，同步全部「无修」装备，范围由实际数据推导。
    /// - 已配置且合法 → 按该区间同步。低等级新赛季允许低于 20000 的品级。
    /// - 配置不完整或非法 → 返回错误，调用方不得继续。
    fn resolve_equip_range(
        min_level: Option<i32>,
        max_level: Option<i32>,
    ) -> Result<Option<(i32, i32)>, String> {
        let min = min_level.filter(|v| *v > 0);
        let max = max_level.filter(|v| *v > 0);

        match (min, max) {
            (None, None) => Ok(None),
            (Some(min), Some(max)) if max >= min => Ok(Some((min, max))),
            (Some(_), Some(_)) => {
                Err("试炼装备品级范围无效（最大值不得小于最小值）".to_string())
            }
            _ => Err("试炼装备品级范围配置不完整".to_string()),
        }
    }

    /// 从已抓取的装备数据推导实际品级区间，用于记录同步结果。
    fn derive_level_range(items: &[serde_json::Value]) -> Option<(i32, i32)> {
        let levels = items
            .iter()
            .filter_map(|item| item.get("Level").and_then(|v| v.as_i64()))
            .filter(|level| *level > 0)
            .collect::<Vec<_>>();

        if levels.is_empty() {
            return None;
        }

        let min = *levels.iter().min().unwrap_or(&0);
        let max = *levels.iter().max().unwrap_or(&0);
        Some((min as i32, max as i32))
    }

    /// 强制同步的删除前门禁：确认当前赛季存在且品级范围可用。
    ///
    /// 这里只清除同步标记，不删除装备数据；装备的删除与写入在
    /// `save_equipments_with_conn` 的同一事务内完成，抓取失败不会丢失已有数据。
    fn prepare_force_sync_with_conn(conn: &rusqlite::Connection) -> Result<(), String> {
        let season = Self::get_current_season_with_conn(conn)
            .map_err(|e| format!("读取当前赛季失败: {}", e))?
            .ok_or_else(|| "未找到当前赛季，无法同步装备".to_string())?;
        let range = Self::resolve_equip_range(
            season.trial_equip_level_min,
            season.trial_equip_level_max,
        )
        .map_err(|reason| format!("当前赛季「{}」{}", season.name, reason))?;

        match range {
            Some((min, max)) => info!(
                "[EquipSync] 强制同步赛季「{}」装备，范围: {}-{}",
                season.name, min, max
            ),
            None => info!(
                "[EquipSync] 赛季「{}」未配置品级范围，将同步全部无修装备",
                season.name
            ),
        }

        conn.execute("DELETE FROM cache WHERE key LIKE 'equip_cache_%'", [])
            .map_err(|e| format!("清除同步标记失败: {}", e))?;
        info!("[EquipSync] 已清除装备同步标记");

        Ok(())
    }

    fn is_season_synced_with_conn(conn: &rusqlite::Connection, cache_key: &str) -> Result<bool, SyncError> {
        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT value FROM cache WHERE key = ?",
            [cache_key],
            |row| row.get(0),
        );

        match result {
            Ok(_) => Ok(true),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(SyncError::Database(e.to_string())),
        }
    }

    fn mark_season_synced_with_conn(conn: &rusqlite::Connection, cache_key: &str) -> Result<(), SyncError> {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "INSERT OR REPLACE INTO cache (key, value, updated_at) VALUES (?, ?, ?)",
            [cache_key, "1", &timestamp],
        ).map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        Ok(())
    }

    async fn fetch_equipments(
        equip_type: &str,
        range: Option<(i32, i32)>,
    ) -> Result<Vec<serde_json::Value>, SyncError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let per = 100;
        let max_pages = 10;

        // 未配置范围时不传 min_level/max_level，由接口返回全部「无修」装备。
        let range_query = match range {
            Some((min, max)) => format!("&min_level={}&max_level={}", min, max),
            None => String::new(),
        };

        let build_url = |page: i32| {
            format!(
                "https://node.jx3box.com/equip/{}?client=std&page={}&per={}&name=无修{}",
                equip_type, page, per, range_query
            )
        };

        let type_name = match equip_type {
            "armor" => "防具",
            "trinket" => "饰品",
            "weapon" => "武器",
            _ => equip_type,
        };
        info!("[EquipSync] 获取 {} 第1页...", type_name);

        let first_response = client.get(build_url(1)).send().await?;
        let first_data: JX3EquipResponse = first_response.json().await?;

        let total_pages = first_data.pages.min(max_pages);
        info!("[EquipSync] {} 共 {} 页，将获取 {} 页", type_name, first_data.pages, total_pages);

        let mut all_items = first_data.list;

        if total_pages > 1 {
            let urls: Vec<String> = (2..=total_pages).map(build_url).collect();

            let results: Vec<Result<serde_json::Value, reqwest::Error>> = futures::future::join_all(
                urls.iter().map(|url| {
                    let client = client.clone();
                    async move {
                        let response = client.get(url).send().await?;
                        let data: JX3EquipResponse = response.json().await?;
                        Ok(serde_json::json!({ "list": data.list }))
                    }
                })
            ).await;

            for result in results {
                match result {
                    Ok(data) => {
                        if let Some(list) = data.get("list").and_then(|v| v.as_array()) {
                            info!("[EquipSync] {} 页: {} 件", type_name, list.len());
                            all_items.extend(list.clone());
                        }
                    }
                    Err(e) => {
                        warn!("[EquipSync] {} 页获取失败: {}", type_name, e);
                    }
                }
            }
        }

        info!("[EquipSync] {} 共获取 {} 件", type_name, all_items.len());

        Ok(all_items)
    }

    fn save_equipments_with_conn(conn: &rusqlite::Connection, armor_items: &[serde_json::Value], trinket_items: &[serde_json::Value], weapon_items: &[serde_json::Value]) -> Result<(), SyncError> {
        let tx = conn.unchecked_transaction()
            .map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        tx.execute("DELETE FROM equipments", [])
            .map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let mut stmt = tx.prepare(
            "INSERT INTO equipments (id, name, ui_id, icon_id, level, quality, bind_type, type_label,
             attribute_types, attributes, recommend, diamonds, data, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        ).map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        let all_items: Vec<&serde_json::Value> = armor_items.iter()
            .chain(trinket_items.iter())
            .chain(weapon_items.iter())
            .collect();

        for item in all_items {
            let id = item.get("ID").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_default();
            
            if id.contains('_') {
                continue;
            }
            
            let name = item.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let ui_id = item.get("UiID").and_then(|v| v.as_i64()).map(|v| v.to_string()).unwrap_or_default();
            let icon_id = item.get("_IconID")
                .and_then(|v| v.as_i64())
                .or_else(|| item.get("IconID").and_then(|v| v.as_i64()))
                .unwrap_or(0) as i32;
            let level = item.get("Level").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let quality = item.get("Quality").and_then(|v| v.as_i64()).unwrap_or(0).to_string();
            let bind_type = item.get("BindType").and_then(|v| v.as_i64()).unwrap_or(0) as i32;

            let sub_type = item.get("SubType").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let detail_type = item.get("DetailType").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            let type_label = match detail_type {
                6 => "暗器",
                _ => match sub_type {
                    1 => "武器",
                    2 => "上衣",
                    3 => "帽子",
                    4 => "项链",
                    5 => "戒指",
                    6 => "腰带",
                    7 => "腰坠",
                    8 => "下装",
                    9 => "鞋子",
                    10 => "护腕",
                    _ => "其他",
                },
            };

            let attr_types = item.get("_AttrType").map(|v| v.to_string()).unwrap_or_default();
            let mut all_magic: Vec<&serde_json::Value> = Vec::with_capacity(5);
            for i in 1..=5 {
                if let Some(m) = item.get(&format!("_Magic{}Type", i)) {
                    if !m.is_null() {
                        all_magic.push(m);
                    }
                }
            }
            let attrs = serde_json::to_string(&all_magic).unwrap_or_default();
            let recommend = item.get("_Attrs").map(|v| v.to_string()).unwrap_or_default();

            let mut all_diamonds: Vec<&serde_json::Value> = Vec::with_capacity(3);
            for i in 1..=3 {
                if let Some(d) = item.get(&format!("_DiamondAttributeID{}", i)) {
                    if !d.is_null() {
                        all_diamonds.push(d);
                    }
                }
            }
            let diamonds = serde_json::to_string(&all_diamonds).unwrap_or_default();
            let data = item.to_string();

            let _ = stmt.execute(rusqlite::params![
                &id, &name, &ui_id, icon_id, level,
                &quality, bind_type, type_label,
                &attr_types, &attrs, &recommend, &diamonds, &data, &timestamp,
            ]);
        }

        let total = armor_items.len() + trinket_items.len() + weapon_items.len();
        drop(stmt);
        tx.commit().map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        info!("[EquipSync] 已保存 {} 件装备到数据库", total);

        Ok(())
    }
}

#[tauri::command]
pub async fn equip_force_sync() -> Result<String, String> {
    info!("[EquipSync] 手动触发装备同步...");

    let conn = crate::db::init_db().map_err(|e| format!("数据库初始化失败: {}", e))?;

    // 验证失败时不会触及任何缓存或装备；验证通过后才会执行清理。
    EquipSync::prepare_force_sync_with_conn(&conn)?;
    
    match EquipSync::sync_if_needed().await {
        Ok(count) => {
            if count > 0 {
                Ok(format!("同步完成，共同步 {} 件装备", count))
            } else {
                Ok("无需同步或已同步".to_string())
            }
        }
        Err(e) => Err(format!("同步失败: {:?}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_force_sync_preflight_db(
        min_level: Option<i32>,
        max_level: Option<i32>,
    ) -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("应创建内存数据库");
        conn.execute_batch(
            r#"
            CREATE TABLE seasons (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                start_date INTEGER NOT NULL,
                end_date INTEGER,
                trial_equip_level_min INTEGER,
                trial_equip_level_max INTEGER
            );
            CREATE TABLE equipments (id TEXT PRIMARY KEY, name TEXT);
            CREATE TABLE cache (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT
            );
            "#,
        )
        .expect("应创建强制同步前置表");
        conn.execute(
            "INSERT INTO seasons
             (id, name, version_id, start_date, end_date, trial_equip_level_min, trial_equip_level_max)
             VALUES (1, '待配置赛季', 1, 0, 0, ?, ?)",
            rusqlite::params![min_level, max_level],
        )
        .expect("应写入当前赛季");
        conn.execute("INSERT INTO equipments (id, name) VALUES ('legacy', '旧装备')", [])
            .expect("应写入历史装备");
        conn.execute(
            "INSERT INTO cache (key, value, updated_at)
             VALUES ('equip_cache_旧赛季', '1', 'old')",
            [],
        )
        .expect("应写入历史同步标记");
        conn
    }

    #[test]
    fn force_sync_rejects_invalid_range_without_touching_equipments() {
        // 反向区间属于明确非法配置，必须在清理前拒绝。
        let conn = setup_force_sync_preflight_db(Some(200), Some(199));

        let error = EquipSync::prepare_force_sync_with_conn(&conn)
            .expect_err("反向范围应在强制同步清理前被拒绝");
        assert!(error.contains("当前赛季"));

        let equipment_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .expect("应统计历史装备");
        let cache_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
            .expect("应统计历史缓存");
        assert_eq!(equipment_count, 1, "非法范围不得删除历史装备");
        assert_eq!(cache_count, 1, "非法范围不得删除同步标记");
    }

    #[test]
    fn force_sync_rejects_partial_range_configuration() {
        // 只配了最小值或最大值，属于配置不完整，不能当作"未配置"放行。
        for (min_level, max_level) in [(Some(100), None), (None, Some(100))] {
            let conn = setup_force_sync_preflight_db(min_level, max_level);
            let error = EquipSync::prepare_force_sync_with_conn(&conn)
                .expect_err("不完整的范围配置应被拒绝");
            assert!(error.contains("不完整"), "错误信息应指明配置不完整: {}", error);

            let equipment_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
                .expect("应统计历史装备");
            assert_eq!(equipment_count, 1, "配置不完整不得删除历史装备");
        }
    }

    #[test]
    fn unconfigured_range_syncs_all_equipments_and_keeps_them() {
        // 新赛季未配置品级范围（0 或 NULL）时，按"同步全部无修装备"处理，
        // 而不是报错跳过；同步标记会被清除以触发重新抓取。
        for (min_level, max_level) in [(Some(0), Some(0)), (None, None)] {
            assert_eq!(
                EquipSync::resolve_equip_range(min_level, max_level),
                Ok(None),
                "未配置范围应解析为不限范围"
            );

            let conn = setup_force_sync_preflight_db(min_level, max_level);
            EquipSync::prepare_force_sync_with_conn(&conn)
                .expect("未配置范围应允许强制同步");

            let equipment_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
                .expect("应统计装备");
            assert_eq!(equipment_count, 1, "未配置范围不得删除已有装备");

            let cache_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
                .expect("应统计同步标记");
            assert_eq!(cache_count, 0, "未配置范围应清除同步标记以触发重新抓取");
        }
    }

    #[test]
    fn equip_range_accepts_low_level_future_season() {
        assert_eq!(
            EquipSync::resolve_equip_range(Some(50), Some(100)),
            Ok(Some((50, 100))),
            "等级压缩后的低品级范围应被接受"
        );

        let conn = setup_force_sync_preflight_db(Some(50), Some(100));
        EquipSync::prepare_force_sync_with_conn(&conn)
            .expect("低等级合法范围应允许强制同步");
        let equipment_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .expect("应统计装备");
        assert_eq!(equipment_count, 1, "仅清理同步标记，不删除已有装备");
    }

    #[test]
    fn derive_level_range_reflects_actual_equipment_levels() {
        let items = vec![
            serde_json::json!({ "Level": 19800 }),
            serde_json::json!({ "Level": 41400 }),
            serde_json::json!({ "Level": 25500 }),
        ];
        assert_eq!(EquipSync::derive_level_range(&items), Some((19800, 41400)));

        assert_eq!(EquipSync::derive_level_range(&[]), None);
        let no_level = vec![serde_json::json!({ "Name": "无修冠" })];
        assert_eq!(EquipSync::derive_level_range(&no_level), None);
    }
}
