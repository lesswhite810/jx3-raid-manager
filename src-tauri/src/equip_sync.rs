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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EquipRange {
    min: Option<i32>,
    max: Option<i32>,
}

impl EquipRange {
    fn describe(self) -> String {
        match (self.min, self.max) {
            (None, None) => "未配置品级范围，同步全部无修装备".to_string(),
            (Some(min), None) => format!("最低品级: {}", min),
            (None, Some(max)) => format!("最高品级: {}", max),
            (Some(min), Some(max)) => format!("品级范围: {}-{}", min, max),
        }
    }

    fn includes(self, level: i64) -> bool {
        level > 0
            && self.min.map_or(true, |min| level >= i64::from(min))
            && self.max.map_or(true, |max| level <= i64::from(max))
    }
}

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
        // 缓存必须绑定实际生效的边界。这样旧的“未配置范围”缓存不会在
        // 静态配置补入最低品级后阻止重新同步。
        let cache_key = Self::cache_key(&season, range);
        if Self::is_season_synced_with_conn(&conn, &cache_key)? {
            info!("[EquipSync] 赛季「{}」当前品级边界已同步，跳过", season.name);
            return Ok(0);
        }

        info!(
            "[EquipSync] 开始同步赛季「{}」装备，{}",
            season.name,
            range.describe()
        );

        let start_time = std::time::Instant::now();

        // 三类接口均成功返回后才允许覆盖本地装备。任何抓取失败都会在保存前
        // 返回，已有装备和同步标记均保持不变。
        let fetched = Self::fetch_all_equipments(range).await;
        let total_items = Self::persist_fetch_result_with_conn(&conn, &cache_key, fetched)?;
        if total_items == 0 {
            return Ok(0);
        }

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
    /// - `None` 或 0 表示该端未配置；上下界可独立配置。
    /// - 两端均未配置时不限范围；配置的边界会同时用于请求和落库前过滤。
    /// - 负数及反向区间属于非法配置，调用方不得继续。
    fn resolve_equip_range(
        min_level: Option<i32>,
        max_level: Option<i32>,
    ) -> Result<EquipRange, String> {
        let normalize = |value: Option<i32>, boundary_name: &str| match value {
            None | Some(0) => Ok(None),
            Some(value) if value > 0 => Ok(Some(value)),
            Some(_) => Err(format!("试炼装备品级{}必须为正整数或 0", boundary_name)),
        };
        let min = normalize(min_level, "下限")?;
        let max = normalize(max_level, "上限")?;

        if let (Some(min), Some(max)) = (min, max) {
            if max < min {
                return Err("试炼装备品级范围无效（最大值不得小于最小值）".to_string());
            }
        }

        Ok(EquipRange { min, max })
    }

    fn cache_key(season: &Season, range: EquipRange) -> String {
        let min = range.min.map_or("none".to_string(), |value| value.to_string());
        let max = range.max.map_or("none".to_string(), |value| value.to_string());
        format!("equip_cache_v2_{}_min_{}_max_{}", season.id, min, max)
    }

    fn filter_equipments_by_range(
        items: Vec<serde_json::Value>,
        range: EquipRange,
    ) -> Vec<serde_json::Value> {
        if range.min.is_none() && range.max.is_none() {
            return items;
        }

        items
            .into_iter()
            .filter(|item| {
                let Some(level) = item.get("Level").and_then(|value| value.as_i64()) else {
                    return false;
                };
                range.includes(level)
            })
            .collect()
    }

    fn build_range_query(range: EquipRange) -> String {
        let mut query = String::new();
        if let Some(min) = range.min {
            query.push_str(&format!("&min_level={}", min));
        }
        if let Some(max) = range.max {
            query.push_str(&format!("&max_level={}", max));
        }
        query
    }

    async fn fetch_all_equipments(
        range: EquipRange,
    ) -> Result<(Vec<serde_json::Value>, Vec<serde_json::Value>, Vec<serde_json::Value>), SyncError> {
        let armor_items = Self::filter_equipments_by_range(
            Self::fetch_equipments("armor", range).await?,
            range,
        );
        let trinket_items = Self::filter_equipments_by_range(
            Self::fetch_equipments("trinket", range).await?,
            range,
        );
        let weapon_items = Self::filter_equipments_by_range(
            Self::fetch_equipments("weapon", range).await?,
            range,
        );
        Ok((armor_items, trinket_items, weapon_items))
    }

    fn persist_fetch_result_with_conn(
        conn: &rusqlite::Connection,
        cache_key: &str,
        fetched: Result<(Vec<serde_json::Value>, Vec<serde_json::Value>, Vec<serde_json::Value>), SyncError>,
    ) -> Result<usize, SyncError> {
        let (armor_items, trinket_items, weapon_items) = fetched?;
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

        Self::save_equipments_with_conn(conn, &armor_items, &trinket_items, &weapon_items)?;
        Self::mark_season_synced_with_conn(conn, cache_key)?;
        Ok(total_items)
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

        info!(
            "[EquipSync] 强制同步赛季「{}」装备，{}",
            season.name,
            range.describe()
        );

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
        range: EquipRange,
    ) -> Result<Vec<serde_json::Value>, SyncError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;

        let per = 100;
        let max_pages = 10;

        // 只传实际配置的边界，避免把未知上限伪造成固定值。
        let range_query = Self::build_range_query(range);

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

        let first_response = client.get(build_url(1)).send().await?.error_for_status()?;
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
                        let response = client.get(url).send().await?.error_for_status()?;
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
                    Err(e) => return Err(SyncError::Http(e)),
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

            stmt.execute(rusqlite::params![
                &id, &name, &ui_id, icon_id, level,
                &quality, bind_type, type_label,
                &attr_types, &attrs, &recommend, &diamonds, &data, &timestamp,
            ])
            .map_err(|e: rusqlite::Error| SyncError::Database(e.to_string()))?;
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

    fn setup_equipments_save_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().expect("应创建内存数据库");
        conn.execute_batch(
            r#"
            CREATE TABLE equipments (
                id TEXT PRIMARY KEY,
                name TEXT,
                ui_id TEXT,
                icon_id INTEGER,
                level INTEGER,
                quality TEXT,
                bind_type INTEGER,
                type_label TEXT,
                attribute_types TEXT,
                attributes TEXT,
                recommend TEXT,
                diamonds TEXT,
                data TEXT,
                updated_at TEXT
            );
            CREATE TABLE cache (
                key TEXT PRIMARY KEY,
                value TEXT,
                updated_at TEXT
            );
            INSERT INTO equipments (id, name) VALUES ('legacy', '旧装备');
            "#,
        )
        .expect("应创建装备保存测试表");
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
    fn force_sync_accepts_single_bound_configuration() {
        // 新赛季可只确认最低品级；最高品级未确定时不得阻断同步。
        for (min_level, max_level) in [(Some(900), None), (None, Some(900))] {
            let conn = setup_force_sync_preflight_db(min_level, max_level);
            EquipSync::prepare_force_sync_with_conn(&conn)
                .expect("单边范围应允许强制同步");

            let equipment_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
                .expect("应统计历史装备");
            assert_eq!(equipment_count, 1, "强制同步准备阶段不得删除历史装备");
        }
    }

    #[test]
    fn unconfigured_range_syncs_all_equipments_and_keeps_them() {
        // 新赛季未配置品级范围（0 或 NULL）时，按"同步全部无修装备"处理，
        // 而不是报错跳过；同步标记会被清除以触发重新抓取。
        for (min_level, max_level) in [(Some(0), Some(0)), (None, None)] {
            assert_eq!(
                EquipSync::resolve_equip_range(min_level, max_level),
                Ok(EquipRange { min: None, max: None }),
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
            Ok(EquipRange { min: Some(50), max: Some(100) }),
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

    #[test]
    fn min_only_range_requests_and_filters_from_900_without_inventing_max() {
        let range = EquipSync::resolve_equip_range(Some(900), None)
            .expect("最低品级配置应合法");
        assert_eq!(range, EquipRange { min: Some(900), max: None });
        assert_eq!(EquipSync::build_range_query(range), "&min_level=900");

        let filtered = EquipSync::filter_equipments_by_range(
            vec![
                serde_json::json!({ "ID": 1, "Level": 899 }),
                serde_json::json!({ "ID": 2, "Level": 900 }),
                serde_json::json!({ "ID": 3, "Level": 1200 }),
                serde_json::json!({ "ID": 4, "Level": 0 }),
            ],
            range,
        );
        assert_eq!(
            filtered.iter().map(|item| item["Level"].as_i64()).collect::<Vec<_>>(),
            vec![Some(900), Some(1200)],
            "即使服务端忽略 min_level，落库前也必须排除低于 900 的装备"
        );
    }

    #[test]
    fn max_only_range_is_supported_and_invalid_values_are_rejected() {
        let max_only = EquipSync::resolve_equip_range(None, Some(900))
            .expect("单独配置最高品级应合法");
        assert_eq!(max_only, EquipRange { min: None, max: Some(900) });
        assert_eq!(EquipSync::build_range_query(max_only), "&max_level=900");
        assert!(EquipSync::resolve_equip_range(Some(-1), None).is_err());
        assert!(EquipSync::resolve_equip_range(None, Some(-1)).is_err());
        assert!(EquipSync::resolve_equip_range(Some(901), Some(900)).is_err());
    }

    #[test]
    fn cache_key_changes_when_a_new_bound_is_configured() {
        let season = Season {
            id: 42,
            name: "苍生铸世".to_string(),
            game_version_id: 9,
            trial_equip_level_min: Some(900),
            trial_equip_level_max: None,
        };
        let unconfigured = EquipSync::cache_key(
            &season,
            EquipRange { min: None, max: None },
        );
        let minimum_900 = EquipSync::cache_key(
            &season,
            EquipRange { min: Some(900), max: None },
        );
        assert_ne!(unconfigured, minimum_900);
        assert_ne!(minimum_900, "equip_cache_苍生铸世");
    }

    #[test]
    fn empty_or_failed_fetch_keeps_existing_equipments_and_cache_unmarked() {
        for fetched in [
            Ok((Vec::new(), Vec::new(), Vec::new())),
            Err(SyncError::Database("模拟抓取失败".to_string())),
        ] {
            let conn = setup_force_sync_preflight_db(Some(900), None);
            let result = EquipSync::persist_fetch_result_with_conn(
                &conn,
                "equip_cache_v2_1_min_900_max_none",
                fetched,
            );

            let equipment_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
                .expect("应统计历史装备");
            let cache_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))
                .expect("应统计历史缓存");
            assert_eq!(equipment_count, 1, "空结果或失败不得删除历史装备");
            assert_eq!(cache_count, 1, "空结果或失败不得写入新同步标记");
            if let Ok(count) = result {
                assert_eq!(count, 0, "空结果不应视为同步成功");
            }
        }
    }

    #[test]
    fn failed_equipment_insert_rolls_back_delete_and_does_not_mark_cache() {
        let conn = setup_equipments_save_db();
        let duplicate_items = vec![
            serde_json::json!({ "ID": 100, "Name": "一号装备", "Level": 900 }),
            serde_json::json!({ "ID": 100, "Name": "重复装备", "Level": 901 }),
        ];
        let result = EquipSync::persist_fetch_result_with_conn(
            &conn,
            "equip_cache_v2_1_min_900_max_none",
            Ok((duplicate_items, Vec::new(), Vec::new())),
        );
        assert!(result.is_err(), "主键冲突必须传播，不能静默丢弃装备");

        let persisted_ids: Vec<String> = conn
            .prepare("SELECT id FROM equipments ORDER BY id")
            .expect("应准备装备查询")
            .query_map([], |row| row.get(0))
            .expect("应查询装备")
            .collect::<Result<Vec<_>, _>>()
            .expect("应读取装备");
        assert_eq!(persisted_ids, vec!["legacy".to_string()], "失败时应回滚删除操作");
        let cache_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM cache WHERE key = 'equip_cache_v2_1_min_900_max_none'",
                [],
                |row| row.get(0),
            )
            .expect("应查询新缓存键");
        assert_eq!(cache_count, 0, "保存失败不得标记同步成功");
    }
}
