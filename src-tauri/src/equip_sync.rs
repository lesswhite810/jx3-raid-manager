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
        let season = match Self::get_current_season()? {
            Some(s) => s,
            None => {
                info!("[EquipSync] 未获取到当前赛季信息，跳过装备同步");
                return Ok(0);
            }
        };

        let cache_key = format!("equip_cache_{}", season.name);
        if Self::is_season_synced(&cache_key)? {
            info!("[EquipSync] 赛季「{}」已同步，跳过", season.name);
            return Ok(0);
        }

        let (min_level, max_level) = match (season.trial_equip_level_min, season.trial_equip_level_max) {
            (Some(min), Some(max)) if min >= 20000 && max >= min => (min, max),
            _ => {
                info!("[EquipSync] 赛季「{}」装分范围无效，跳过同步: min={:?}, max={:?}",
                    season.name, season.trial_equip_level_min, season.trial_equip_level_max);
                return Ok(0);
            }
        };

        info!("[EquipSync] 开始同步赛季「{}」装备，范围: {}-{}", season.name, min_level, max_level);

        let start_time = std::time::Instant::now();

        let armor_items = Self::fetch_equipments("armor", min_level, max_level).await?;
        let trinket_items = Self::fetch_equipments("trinket", min_level, max_level).await?;
        let weapon_items = Self::fetch_equipments("weapon", min_level, max_level).await?;

        let total_items = armor_items.len() + trinket_items.len() + weapon_items.len();
        info!("[EquipSync] 获取到 {} 件装备（防具: {}, 饰品: {}, 武器: {}）", total_items, armor_items.len(), trinket_items.len(), weapon_items.len());

        Self::save_equipments(&armor_items, &trinket_items, &weapon_items)?;
        Self::mark_season_synced(&cache_key)?;

        let duration = start_time.elapsed();
        info!("[EquipSync] ✓ 赛季「{}」同步完成！共同步 {} 件装备，耗时 {:?}", season.name, total_items, duration);

        Ok(total_items)
    }

    fn get_current_season() -> Result<Option<Season>, SyncError> {
        let conn = crate::db::init_db().map_err(|e: String| SyncError::Database(e))?;

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

    fn is_season_synced(cache_key: &str) -> Result<bool, SyncError> {
        let conn = crate::db::init_db().map_err(|e: String| SyncError::Database(e))?;

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

    fn mark_season_synced(cache_key: &str) -> Result<(), SyncError> {
        let conn = crate::db::init_db().map_err(|e: String| SyncError::Database(e))?;
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        conn.execute(
            "INSERT OR REPLACE INTO cache (key, value, updated_at) VALUES (?, ?, ?)",
            [cache_key, "1", &timestamp],
        ).map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;

        Ok(())
    }

    async fn fetch_equipments(equip_type: &str, min_level: i32, max_level: i32) -> Result<Vec<serde_json::Value>, SyncError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let per = 100;
        let max_pages = 10;

        let first_url = format!(
            "https://node.jx3box.com/equip/{}?client=std&page=1&per={}&min_level={}&max_level={}&name=无修",
            equip_type, per, min_level, max_level
        );

        let type_name = match equip_type {
            "armor" => "防具",
            "trinket" => "饰品",
            "weapon" => "武器",
            _ => equip_type,
        };
        info!("[EquipSync] 获取 {} 第1页...", type_name);

        let first_response = client.get(&first_url).send().await?;
        let first_data: JX3EquipResponse = first_response.json().await?;

        let total_pages = first_data.pages.min(max_pages);
        info!("[EquipSync] {} 共 {} 页，将获取 {} 页", type_name, first_data.pages, total_pages);

        let mut all_items = first_data.list;

        if total_pages > 1 {
            let mut urls: Vec<String> = Vec::new();
            for page in 2..=total_pages {
                let url = format!(
                    "https://node.jx3box.com/equip/{}?client=std&page={}&per={}&min_level={}&max_level={}&name=无修",
                    equip_type, page, per, min_level, max_level
                );
                urls.push(url);
            }

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

    fn save_equipments(armor_items: &[serde_json::Value], trinket_items: &[serde_json::Value], weapon_items: &[serde_json::Value]) -> Result<(), SyncError> {
        let conn = crate::db::init_db().map_err(|e: String| SyncError::Database(e))?;

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
    
    // 先清除缓存
    let conn = crate::db::init_db().map_err(|e| format!("数据库初始化失败: {}", e))?;
    conn.execute("DELETE FROM cache WHERE key LIKE 'equip_cache_%'", [])
        .map_err(|e| format!("清除缓存失败: {}", e))?;
    info!("[EquipSync] 已清除装备缓存");
    
    // 清除现有装备数据
    conn.execute("DELETE FROM equipments", [])
        .map_err(|e| format!("清除装备数据失败: {}", e))?;
    info!("[EquipSync] 已清除旧装备数据");
    
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
