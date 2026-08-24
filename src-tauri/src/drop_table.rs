use futures::future::join_all;
use log::{info, warn};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const JX3BOX_DROP_API_BASE: &str = "https://node.jx3box.com/fb/drop/v2";
const JX3BOX_ITEM_MERGED_API_BASE: &str = "https://node.jx3box.com/item_merged/id";
/// 魔盒交易行成交聚合接口（物品页"价格走势/成交记录"图表同款数据源）。
/// POST JSON: { item_id, server, aggregate_type: "hourly" | "daily" }，
/// 返回按时段聚合的真实成交均价与成交量数组（非挂单快照）。
const JX3BOX_AUCTION_API_BASE: &str = "https://next2.jx3box.com/api/auction/";
const ITEM_MERGED_PER_PAGE: usize = 50;

/// 一天的秒数
const DAY_SECS: i64 = 86_400;
/// 北京时间固定偏移（UTC+8），剑网 3 交易行数据以国内时区的自然日为准
const BEIJING_TZ_OFFSET_SECS: i64 = 8 * 3600;
/// 1 砖 = 10000 金 = 1 亿铜，价格接口中的封顶哨兵值，均价计算需剔除
const BRICK_SENTINEL_COPPER: i64 = 100_000_000;

/// 独立的 tokio runtime，供同步上下文（如 spawn_blocking 线程）调用 async HTTP 请求。
/// 使用 multi_thread 模式：扫描器在 std::thread::scope 的 4 个线程中并发调用
/// classify_drops_sync，current_thread runtime 的 I/O 驱动是线程局部的，
/// 跨线程 block_on 会导致 HTTP 请求永久挂起。
static SYNC_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn sync_runtime() -> &'static tokio::runtime::Runtime {
    SYNC_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("[DropTable] 创建同步 tokio runtime 失败")
    })
}

/// 全局 HTTP Client（复用连接池，避免每次请求重建连接）
static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("[DropTable] 创建全局 HTTP Client 失败")
    })
}

/// 材料价格内存缓存（key: "ItemType_ItemID|server"，value: (单价金, 缓存时间)）
/// TTL 1 小时，过期后重新查询。不持久化（材料价格每日变动，长期存储无意义）。
static MATERIAL_PRICE_CACHE: OnceLock<Mutex<HashMap<String, (i64, Instant)>>> = OnceLock::new();
const MATERIAL_PRICE_CACHE_TTL: Duration = Duration::from_secs(3600);

fn material_price_cache() -> &'static Mutex<HashMap<String, (i64, Instant)>> {
    MATERIAL_PRICE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// JX3Box 掉落表接口（drop/v2）返回的单条掉落记录
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DropItem {
    #[serde(rename = "MapID")]
    pub map_id: i64,
    #[serde(rename = "ItemType")]
    pub item_type: i32,
    #[serde(rename = "ItemID")]
    pub item_id: i64,
    #[serde(rename = "ItemExtID", default)]
    pub item_ext_id: i64,
    #[serde(rename = "ItemName")]
    pub item_name: String,
    #[serde(rename = "ItemQuality")]
    pub item_quality: i32,
    #[serde(rename = "ItemIconID")]
    pub item_icon_id: i32,
    #[serde(rename = "BossName")]
    pub boss_name: String,
    #[serde(rename = "ApplicableSchoolIDs", default)]
    pub applicable_school_ids: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "id", default)]
    pub id: i64,
}

/// drop/v2 接口响应（分页）
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct DropApiResponse {
    #[serde(default)]
    list: Vec<DropItem>,
}

/// JX3Box 物品详情接口（item_merged）返回的完整物品信息
///
/// 完整存储所有字段（含 JSON 字段），分类字段用于规则判断。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ItemMerged {
    /// 形如 "5_21511"
    pub id: String,
    #[serde(rename = "UiID", default)]
    pub ui_id: Option<i64>,
    #[serde(rename = "Source", default)]
    pub source: Option<String>,
    #[serde(rename = "SourceID", default)]
    pub source_id: Option<i64>,
    #[serde(rename = "Name", default)]
    pub name: String,
    #[serde(rename = "Desc", default)]
    pub desc: Option<String>,
    #[serde(rename = "Genre", default)]
    pub genre: Option<i64>,
    #[serde(rename = "SubType", default)]
    pub sub_type: Option<i64>,
    #[serde(rename = "DetailType", default)]
    pub detail_type: Option<i64>,
    #[serde(rename = "Price", default)]
    pub price: Option<i64>,
    #[serde(rename = "Level", default)]
    pub level: Option<i64>,
    #[serde(rename = "BindType", default)]
    pub bind_type: Option<i64>,
    #[serde(rename = "MaxDurability", default)]
    pub max_durability: Option<i64>,
    #[serde(rename = "AbradeRate", default)]
    pub abrade_rate: Option<i64>,
    #[serde(rename = "MaxExistTime", default)]
    pub max_exist_time: Option<i64>,
    #[serde(rename = "MaxExistAmount", default)]
    pub max_exist_amount: Option<i64>,
    #[serde(rename = "CanTrade", default)]
    pub can_trade: Option<bool>,
    #[serde(rename = "CanDestroy", default)]
    pub can_destroy: Option<bool>,
    #[serde(rename = "Quality", default)]
    pub quality: Option<i64>,
    #[serde(rename = "SkillID", default)]
    pub skill_id: Option<i64>,
    #[serde(rename = "SkillLevel", default)]
    pub skill_level: Option<i64>,
    #[serde(rename = "BelongSchool", default)]
    pub belong_school: Option<String>,
    #[serde(rename = "MagicKind", default)]
    pub magic_kind: Option<String>,
    #[serde(rename = "MagicType", default)]
    pub magic_type: Option<String>,
    #[serde(rename = "GetType", default)]
    pub get_type: Option<String>,
    #[serde(rename = "IconID", default)]
    pub icon_id: Option<i64>,
    #[serde(rename = "CanSetColor", default)]
    pub can_set_color: Option<bool>,
    #[serde(rename = "AucGenre", default)]
    pub auc_genre: Option<i64>,
    #[serde(rename = "AucSubType", default)]
    pub auc_sub_type: Option<i64>,
    #[serde(rename = "RequireCamp", default)]
    pub require_camp: Option<i64>,
    #[serde(rename = "MaxStrengthLevel", default)]
    pub max_strength_level: Option<i64>,
    #[serde(rename = "CanApart", default)]
    pub can_apart: Option<bool>,
    #[serde(rename = "CanExterior", default)]
    pub can_exterior: Option<serde_json::Value>,
    #[serde(rename = "CanChangeMagic", default)]
    pub can_change_magic: Option<bool>,
    #[serde(rename = "CanShared", default)]
    pub can_shared: Option<bool>,
    #[serde(rename = "RepairPriceRebate", default)]
    pub repair_price_rebate: Option<i64>,
    #[serde(rename = "CanStack", default)]
    pub can_stack: Option<bool>,
    #[serde(rename = "CanConsume", default)]
    pub can_consume: Option<bool>,
    #[serde(rename = "RequireLevel", default)]
    pub require_level: Option<i64>,
    #[serde(rename = "RequireGender", default)]
    pub require_gender: Option<i64>,
    #[serde(rename = "RequireHomelandLevel", default)]
    pub require_homeland_level: Option<i64>,
    #[serde(rename = "CanUseOnHorse", default)]
    pub can_use_on_horse: Option<bool>,
    #[serde(rename = "CanUseInFight", default)]
    pub can_use_in_fight: Option<bool>,
    #[serde(rename = "CanGoodCampUse", default)]
    pub can_good_camp_use: Option<bool>,
    #[serde(rename = "CanEvilCampUse", default)]
    pub can_evil_camp_use: Option<bool>,
    #[serde(rename = "CanNeutralCampUse", default)]
    pub can_neutral_camp_use: Option<bool>,
    #[serde(rename = "Diamonds", default)]
    pub diamonds: Option<serde_json::Value>,
    #[serde(rename = "Requires", default)]
    pub requires: Option<serde_json::Value>,
    #[serde(rename = "Recommend", default)]
    pub recommend: Option<serde_json::Value>,
    #[serde(rename = "RecommendXfs", default)]
    pub recommend_xfs: Option<serde_json::Value>,
    #[serde(rename = "TypeLabel", default)]
    pub type_label: Option<String>,
    #[serde(rename = "Appearance", default)]
    pub appearance: Option<String>,
    #[serde(rename = "AttributeTypes", default)]
    pub attribute_types: Option<serde_json::Value>,
    #[serde(rename = "CoolDown", default)]
    pub cool_down: Option<serde_json::Value>,
    #[serde(rename = "IsQuest", default)]
    pub is_quest: Option<bool>,
    #[serde(rename = "WuCaiHtml", default)]
    pub wu_cai_html: Option<String>,
    #[serde(rename = "Set", default)]
    pub set: Option<serde_json::Value>,
    #[serde(rename = "IsEquip", default)]
    pub is_equip: Option<bool>,
    #[serde(rename = "EquipUsage", default)]
    pub equip_usage: Option<i64>,
    #[serde(rename = "ImageUrl", default)]
    pub image_url: Option<String>,
    #[serde(rename = "GetSource", default)]
    pub get_source: Option<serde_json::Value>,
    #[serde(rename = "attributes", default)]
    pub attributes: Option<serde_json::Value>,
    #[serde(rename = "furniture_attributes", default)]
    pub furniture_attributes: Option<serde_json::Value>,
    #[allow(dead_code)]
    #[serde(rename = "idKey", default)]
    pub id_key: Option<i64>,
}

/// item_merged 接口响应（分页）
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ItemMergedResponse {
    #[serde(default)]
    total: i64,
    #[serde(default)]
    per: i64,
    #[serde(default)]
    pages: i64,
    #[serde(default)]
    page: i64,
    #[serde(default)]
    list: Vec<ItemMerged>,
}

#[derive(Debug, thiserror::Error)]
pub enum DropTableError {
    #[error("HTTP请求失败: {0}")]
    Http(#[from] reqwest::Error),
    #[error("数据库错误: {0}")]
    Database(String),
    #[error("JSON解析失败: {0}")]
    Json(#[from] serde_json::Error),
    #[error("无效的 MapID: {0}")]
    InvalidMapId(i64),
}

/// 主流程：根据 JCL 实际掉落 + 副本可能掉落，查询物品分类并入库
///
/// 流程：
/// 1. 调用 drop/v2 获取副本可能掉落（每次实时调，轻量）
/// 2. 实际掉落 ∩ 副本可能掉落 = 交集物品
/// 3. 查 drop_items 表，过滤已缓存的物品
/// 4. 仅对未命中物品调用 item_merged 批量查询
/// 5. 纯字段分类后入库
/// 6. 返回所有实际掉落物品的分类映射
pub fn classify_drops_sync(
    map_id: i64,
    raid_name: Option<&str>,
    actual_drop_names: &[String],
) -> Result<std::collections::HashMap<String, String>, DropTableError> {
    sync_runtime().handle().block_on(async {
        classify_drops(map_id, raid_name, actual_drop_names).await
    })
}

/// async 版本
///
/// 优化流程：先按物品名称查 drop_items 缓存，全命中则完全跳过 drop/v2 和 item_merged。
/// 仅当有未命中物品时，才调 drop/v2 获取 ItemType/ItemID（item_merged 接口依赖此 ID）。
pub async fn classify_drops(
    map_id: i64,
    raid_name: Option<&str>,
    actual_drop_names: &[String],
) -> Result<std::collections::HashMap<String, String>, DropTableError> {
    if map_id <= 0 {
        return Err(DropTableError::InvalidMapId(map_id));
    }

    if actual_drop_names.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let mut conn = crate::db::init_db().map_err(|e| DropTableError::Database(e))?;

    // 1. 先按物品名称查 drop_items 缓存
    let mut cached_categories = query_categories_by_names(&conn, actual_drop_names)?;

    // 1.5 赛季一致性检查：装备价格随游戏版本更新变化（新赛季应用版本会录入新赛季时间），
    // 缓存行属于旧赛季或无赛季标记时，删除后走重新拉取流程，保证估价使用当前版本数据。
    // 材料价格由交易行动态查询，不受赛季影响，无需失效。
    let stale_equipment = find_stale_equipment_names(&conn, actual_drop_names);
    if !stale_equipment.is_empty() {
        for name in &stale_equipment {
            cached_categories.remove(name);
            conn.execute(
                "DELETE FROM drop_items WHERE item_name = ?1",
                params![name],
            )
            .map_err(|e| DropTableError::Database(e.to_string()))?;
        }
        info!(
            "[DropTable] 检测到 {} 条旧赛季装备缓存已失效，删除并重新获取: {:?}",
            stale_equipment.len(),
            stale_equipment
        );
    }

    info!(
        "[DropTable] 实际掉落 {} 条，缓存命中 {} 条",
        actual_drop_names.len(),
        cached_categories.len()
    );

    // 2. 全部命中缓存，无需调 API
    if cached_categories.len() == actual_drop_names.len() {
        info!("[DropTable] 全部命中缓存，跳过 drop/v2 和 item_merged");
        return Ok(cached_categories);
    }

    // 3. 有未命中物品，需调 drop/v2 获取 ItemType/ItemID（item_merged 依赖此 ID）
    let uncached_names: Vec<&str> = actual_drop_names
        .iter()
        .filter(|n| !cached_categories.contains_key(*n))
        .map(|s| s.as_str())
        .collect();

    let possible_drops = fetch_drop_table_from_api(map_id).await?;
    info!(
        "[DropTable] MapID={} 副本可能掉落 {} 条（用于获取未命中物品的 ItemType/ItemID）",
        map_id,
        possible_drops.len()
    );

    // 4. 从可能掉落中找到未命中物品的 DropItem
    let uncached_set: std::collections::HashSet<&str> = uncached_names.iter().copied().collect();
    let uncached_items: Vec<&DropItem> = possible_drops
        .iter()
        .filter(|d| uncached_set.contains(d.item_name.as_str()))
        .collect();

    info!(
        "[DropTable] 未命中 {} 条，在副本可能掉落中匹配到 {} 条",
        uncached_names.len(),
        uncached_items.len()
    );

    if uncached_items.is_empty() {
        // 未命中物品不在副本可能掉落中，直接返回已缓存的分类
        return Ok(cached_categories);
    }

    // 5. 批量查询未命中物品的详情
    let item_keys: Vec<String> = uncached_items
        .iter()
        .map(|d| format!("{}_{}", d.item_type, d.item_id))
        .collect();
    let merged_items = fetch_item_merged_batch(&item_keys).await?;
    info!(
        "[DropTable] item_merged 批量查询返回 {} 条",
        merged_items.len()
    );

    // 6. 分类并入库（记录入库时的当前赛季，供下个赛季的缓存失效检查比对）
    let current_season_id = crate::db::query_current_season_id(&conn);
    save_items_to_db(
        &mut conn,
        map_id,
        raid_name,
        &uncached_items,
        &merged_items,
        current_season_id,
    )?;

    // 7. 查询新入库物品的分类
    let new_names: Vec<String> = uncached_items
        .iter()
        .map(|d| d.item_name.clone())
        .collect();
    let new_categories = query_categories_by_names(&conn, &new_names)?;

    // 8. 合并缓存分类和新分类
    let mut all_categories = cached_categories;
    all_categories.extend(new_categories);

    Ok(all_categories)
}

/// 请求 JX3Box 掉落表 API（无缓存，每次实时调用）
///
/// 接口：GET https://node.jx3box.com/fb/drop/v2/{map_id}?client=std
/// 注意：drop/v2 接口一次性返回全部数据，无分页。
async fn fetch_drop_table_from_api(map_id: i64) -> Result<Vec<DropItem>, DropTableError> {
    let url = format!("{}?client=std", slash_join(JX3BOX_DROP_API_BASE, map_id));
    let resp = http_client()
        .get(&url)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(15))
        .send()
        .await?;

    if !resp.status().is_success() {
        warn!(
            "[DropTable] MapID={} 掉落接口返回非 2xx: {}",
            map_id,
            resp.status()
        );
        return Err(DropTableError::Database(format!(
            "JX3Box 掉落接口返回状态码: {}",
            resp.status()
        )));
    }

    let drops: Vec<DropItem> = resp.json().await?;
    info!(
        "[DropTable] MapID={} 掉落接口返回 {} 条记录",
        map_id,
        drops.len()
    );
    Ok(drops)
}

/// 批量请求 JX3Box item_merged 接口（分页，per=50）
///
/// 接口：GET https://node.jx3box.com/item_merged/id/{ids}?per=50&page=N
/// ids 为逗号分隔的 ItemType_ItemID 列表
async fn fetch_item_merged_batch(
    item_keys: &[String],
) -> Result<Vec<ItemMerged>, DropTableError> {
    let client = http_client();

    // 将 item_keys 合并为逗号分隔的字符串
    let ids_param = item_keys.join(",");
    let mut all_items: Vec<ItemMerged> = Vec::new();
    let mut page = 1;
    let mut total_pages = 1;

    while page <= total_pages {
        let url = format!(
            "{}/{}?per={}&page={}",
            JX3BOX_ITEM_MERGED_API_BASE,
            ids_param,
            ITEM_MERGED_PER_PAGE,
            page
        );

        let resp = client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;

        if !resp.status().is_success() {
            warn!(
                "[DropTable] item_merged 接口返回非 2xx: {} (page={})",
                resp.status(),
                page
            );
            break;
        }

        // 先获取文本体用于调试反序列化错误
        let resp_text = resp.text().await?;
        let body: ItemMergedResponse = match serde_json::from_str(&resp_text) {
            Ok(b) => b,
            Err(e) => {
                // 截取前 500 字符用于调试
                let preview: String = resp_text.chars().take(500).collect();
                warn!(
                    "[DropTable] item_merged 反序列化失败 (page={}): {} | 响应预览: {}",
                    page, e, preview
                );
                return Err(DropTableError::Json(e));
            }
        };
        total_pages = body.pages.max(1);
        let received = body.list.len();
        info!(
            "[DropTable] item_merged 第 {}/{} 页，返回 {} 条",
            page, total_pages, received
        );

        all_items.extend(body.list);

        if received == 0 {
            break;
        }
        page += 1;
    }

    Ok(all_items)
}

/// 查询 drop_items 表中已缓存的物品（按 item_type + item_id 过滤）
///
/// 返回 (item_type, item_id) -> item_name 的映射
#[allow(dead_code)]
fn query_cached_items(
    conn: &Connection,
    items: &[&DropItem],
) -> Result<std::collections::HashMap<(i32, i64), String>, DropTableError> {
    let mut cached = std::collections::HashMap::new();
    if items.is_empty() {
        return Ok(cached);
    }

    // SQLite 不支持元组 IN，改为对每个 (item_type, item_id) 单独查询
    // 物品数量通常 <= 20，性能可接受
    for d in items {
        let name: Option<String> = conn
            .query_row(
                "SELECT item_name FROM drop_items WHERE item_type = ?1 AND item_id = ?2",
                params![d.item_type, d.item_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(n) = name {
            cached.insert((d.item_type, d.item_id), n);
        }
    }

    Ok(cached)
}

/// 按物品名称批量查询分类
fn query_categories_by_names(
    conn: &Connection,
    names: &[String],
) -> Result<std::collections::HashMap<String, String>, DropTableError> {
    let mut result = std::collections::HashMap::new();
    if names.is_empty() {
        return Ok(result);
    }

    // 对每个名称单独查询（数量通常 <= 20，性能可接受）
    for name in names {
        let category: Option<String> = conn
            .query_row(
                "SELECT category FROM drop_items WHERE item_name = ?1",
                params![name],
                |row| row.get(0),
            )
            .ok();
        if let Some(cat) = category {
            result.insert(name.clone(), cat);
        }
    }

    Ok(result)
}

/// 找出与当前赛季不一致、需要失效重建的装备名称。
///
/// 规则：
/// - 仅检查 category='equipment' 的行（材料价格由交易行动态查询，不受赛季影响）；
/// - 行的 season_id 与当前赛季不一致（含 NULL，即旧版本入库的无标记行）视为过期；
/// - 当前未配置任何生效赛季时跳过失效检查，避免误清缓存。
fn find_stale_equipment_names(conn: &Connection, names: &[String]) -> Vec<String> {
    let Some(current_season_id) = crate::db::query_current_season_id(conn) else {
        return Vec::new();
    };

    let mut stale = Vec::new();
    for name in names {
        let row: Option<(Option<String>, Option<i64>)> = conn
            .query_row(
                "SELECT category, season_id FROM drop_items WHERE item_name = ?1",
                params![name],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if let Some((category, season_id)) = row {
            if category.as_deref() == Some("equipment") && season_id != Some(current_season_id) {
                stale.push(name.clone());
            }
        }
    }
    stale
}

/// 将查询到的物品详情分类后入库
fn save_items_to_db(
    conn: &mut Connection,
    map_id: i64,
    raid_name: Option<&str>,
    drop_items: &[&DropItem],
    merged_items: &[ItemMerged],
    season_id: Option<i64>,
) -> Result<(), DropTableError> {
    let now = chrono::Local::now().to_rfc3339();
    let merged_map: std::collections::HashMap<&str, &ItemMerged> = merged_items
        .iter()
        .map(|m| (m.id.as_str(), m))
        .collect();

    let tx = conn.transaction().map_err(|e| DropTableError::Database(e.to_string()))?;

    {
        let mut stmt = tx
            .prepare(
                r#"
                INSERT OR REPLACE INTO drop_items (
                    map_id, item_type, item_id, item_ext_id, boss_name, applicable_school_ids,
                    ui_id, source, source_id, item_name, description, genre, sub_type, detail_type,
                    price, level, bind_type, max_durability, abrade_rate, max_exist_time, max_exist_amount,
                    can_trade, can_destroy, quality, skill_id, skill_level, belong_school, magic_kind,
                    magic_type, get_type, icon_id, can_set_color, auc_genre, auc_sub_type, require_camp,
                    max_strength_level, can_apart, can_exterior, can_change_magic, can_shared,
                    repair_price_rebate, can_stack, can_consume, require_level, require_gender,
                    require_homeland_level, can_use_on_horse, can_use_in_fight, can_good_camp_use,
                    can_evil_camp_use, can_neutral_camp_use, type_label, appearance, cool_down,
                    is_quest, wu_cai_html, is_equip, equip_usage, image_url, id_key,
                    diamonds, requires, recommend, recommend_xfs, attribute_types, set_info,
                    get_source, attributes, furniture_attributes,
                    category, class_source, season_id, created_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6,
                    ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18, ?19, ?20, ?21,
                    ?22, ?23, ?24, ?25, ?26, ?27, ?28,
                    ?29, ?30, ?31, ?32, ?33, ?34, ?35,
                    ?36, ?37, ?38, ?39, ?40,
                    ?41, ?42, ?43, ?44, ?45,
                    ?46, ?47, ?48, ?49,
                    ?50, ?51, ?52, ?53,
                    ?54, ?55, ?56, ?57, ?58, ?59,
                    ?60, ?61, ?62, ?63, ?64, ?65,
                    ?66, ?67, ?68,
                    ?69, ?70, ?71, ?72, ?73
                )
                "#,
            )
            .map_err(|e| DropTableError::Database(e.to_string()))?;

        for d in drop_items {
            let item_key = format!("{}_{}", d.item_type, d.item_id);
            let merged = merged_map.get(item_key.as_str());

            let (category, class_source) = match merged {
                Some(m) => (classify_item(m), "api".to_string()),
                None => ("unknown".to_string(), "api_miss".to_string()),
            };

            let m = merged.cloned();

            // JSON 字段序列化
            let diamonds_json = m.as_ref().and_then(|x| x.diamonds.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let requires_json = m.as_ref().and_then(|x| x.requires.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let recommend_json = m.as_ref().and_then(|x| x.recommend.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let recommend_xfs_json = m.as_ref().and_then(|x| x.recommend_xfs.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let attribute_types_json = m.as_ref().and_then(|x| x.attribute_types.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let set_info_json = m.as_ref().and_then(|x| x.set.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let get_source_json = m.as_ref().and_then(|x| x.get_source.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let attributes_json = m.as_ref().and_then(|x| x.attributes.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());
            let furniture_attributes_json = m.as_ref().and_then(|x| x.furniture_attributes.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default());

            stmt.execute(params![
                map_id,
                d.item_type,
                d.item_id,
                d.item_ext_id,
                &d.boss_name,
                &d.applicable_school_ids,
                // item_merged 简单字段
                m.as_ref().and_then(|x| x.ui_id),
                m.as_ref().and_then(|x| x.source.clone()),
                m.as_ref().and_then(|x| x.source_id),
                d.item_name,
                m.as_ref().and_then(|x| x.desc.clone()),
                m.as_ref().and_then(|x| x.genre),
                m.as_ref().and_then(|x| x.sub_type),
                m.as_ref().and_then(|x| x.detail_type),
                m.as_ref().and_then(|x| x.price),
                m.as_ref().and_then(|x| x.level),
                m.as_ref().and_then(|x| x.bind_type),
                m.as_ref().and_then(|x| x.max_durability),
                m.as_ref().and_then(|x| x.abrade_rate),
                m.as_ref().and_then(|x| x.max_exist_time),
                m.as_ref().and_then(|x| x.max_exist_amount),
                m.as_ref().and_then(|x| x.can_trade),
                m.as_ref().and_then(|x| x.can_destroy),
                m.as_ref().and_then(|x| x.quality),
                m.as_ref().and_then(|x| x.skill_id),
                m.as_ref().and_then(|x| x.skill_level),
                m.as_ref().and_then(|x| x.belong_school.clone()),
                m.as_ref().and_then(|x| x.magic_kind.clone()),
                m.as_ref().and_then(|x| x.magic_type.clone()),
                m.as_ref().and_then(|x| x.get_type.clone()),
                m.as_ref().and_then(|x| x.icon_id),
                m.as_ref().and_then(|x| x.can_set_color),
                m.as_ref().and_then(|x| x.auc_genre),
                m.as_ref().and_then(|x| x.auc_sub_type),
                m.as_ref().and_then(|x| x.require_camp),
                m.as_ref().and_then(|x| x.max_strength_level),
                m.as_ref().and_then(|x| x.can_apart),
                m.as_ref()
                    .and_then(|x| x.can_exterior.as_ref())
                    .map(|v| match v {
                        serde_json::Value::Bool(b) => *b,
                        serde_json::Value::String(_) => true, // 字符串表示有门派限制但可作为外观
                        _ => false,
                    }),
                m.as_ref().and_then(|x| x.can_change_magic),
                m.as_ref().and_then(|x| x.can_shared),
                m.as_ref().and_then(|x| x.repair_price_rebate),
                m.as_ref().and_then(|x| x.can_stack),
                m.as_ref().and_then(|x| x.can_consume),
                m.as_ref().and_then(|x| x.require_level),
                m.as_ref().and_then(|x| x.require_gender),
                m.as_ref().and_then(|x| x.require_homeland_level),
                m.as_ref().and_then(|x| x.can_use_on_horse),
                m.as_ref().and_then(|x| x.can_use_in_fight),
                m.as_ref().and_then(|x| x.can_good_camp_use),
                m.as_ref().and_then(|x| x.can_evil_camp_use),
                m.as_ref().and_then(|x| x.can_neutral_camp_use),
                m.as_ref().and_then(|x| x.type_label.clone()),
                m.as_ref().and_then(|x| x.appearance.clone()),
                m.as_ref().and_then(|x| x.cool_down.as_ref()).map(|v| serde_json::to_string(v).unwrap_or_default()),
                m.as_ref().and_then(|x| x.is_quest),
                m.as_ref().and_then(|x| x.wu_cai_html.clone()),
                m.as_ref().and_then(|x| x.is_equip),
                m.as_ref().and_then(|x| x.equip_usage),
                m.as_ref().and_then(|x| x.image_url.clone()),
                m.as_ref().and_then(|x| x.id_key),
                // JSON 字段
                diamonds_json,
                requires_json,
                recommend_json,
                recommend_xfs_json,
                attribute_types_json,
                set_info_json,
                get_source_json,
                attributes_json,
                furniture_attributes_json,
                // 管理字段
                category,
                class_source,
                season_id,
                now,
            ])
            .map_err(|e| DropTableError::Database(e.to_string()))?;
        }
    }

    tx.commit().map_err(|e| DropTableError::Database(e.to_string()))?;
    info!(
        "[DropTable] drop_items 入库 {} 条 (raid={})",
        drop_items.len(),
        raid_name.unwrap_or("")
    );
    Ok(())
}

fn slash_join(base: &str, id: i64) -> String {
    if base.ends_with('/') {
        format!("{}{}", base, id)
    } else {
        format!("{}/{}", base, id)
    }
}

// ============== JX3Box 交易行价格查询 ==============

/// 计算给定时刻所在北京自然日的零点 Unix 时间戳（秒）
fn beijing_day_start(now_secs: i64) -> i64 {
    ((now_secs + BEIJING_TZ_OFFSET_SECS) / DAY_SECS) * DAY_SECS - BEIJING_TZ_OFFSET_SECS
}

/// 铜 → 金，四舍五入到整数金
fn copper_to_gold(copper: i64) -> i64 {
    (copper + 5000) / 10000
}

/// 计算指定时间窗口 [start, end) 内的成交量加权平均单价（铜）。
///
/// 过滤规则：
/// - created 不在窗口内的记录；
/// - unit_price <= 0 或 n_count <= 0 的无效记录；
/// - unit_price >= 1 亿铜（1 砖封顶哨兵值）与洗钱性质的异常高价。
///
/// 加权公式：Σ(单价 × 成交量) / Σ成交量，四舍五入。窗口内无有效成交时返回 None。
fn window_weighted_avg_copper(prices: &[PriceRecord], start: i64, end: i64) -> Option<i64> {
    let mut total_copper: i64 = 0;
    let mut total_count: i64 = 0;
    for p in prices {
        if p.created < start || p.created >= end {
            continue;
        }
        if p.unit_price <= 0 || p.unit_price >= BRICK_SENTINEL_COPPER || p.n_count <= 0 {
            continue;
        }
        total_copper += p.unit_price * p.n_count;
        total_count += p.n_count;
    }
    if total_count == 0 {
        return None;
    }
    Some((total_copper + total_count / 2) / total_count)
}

/// 交易行成交聚合接口（POST /api/auction/）单条聚合点。
///
/// 接口字段 timestamp / price / sample 通过 serde rename 映射到内部命名；
/// aggregate_type = "hourly" 时每个自然小时一个点：price 为该时段成交均价，
/// sample 为该时段成交量，created 为时段起始时间戳。
#[derive(Debug, Deserialize)]
struct PriceRecord {
    /// 聚合点时间戳（秒），接口字段 timestamp
    #[serde(default, rename = "timestamp")]
    created: i64,
    /// 该时段成交量，接口字段 sample
    #[serde(default, rename = "sample")]
    n_count: i64,
    /// 服务器名
    #[serde(default)]
    #[allow(dead_code)]
    server: String,
    /// 该时段成交均价（铜，需 ÷10000 转金），接口字段 price
    #[serde(default, rename = "price")]
    unit_price: i64,
}

/// 查询物品的交易行价格（前一天成交均价，铜 → 金）
///
/// 接口：POST https://next2.jx3box.com/api/auction/
/// body: { "item_id": "{ItemType}_{ItemID}", "server": ..., "aggregate_type": "hourly" }
///
/// 这是魔盒物品页"成交记录/价格走势"图表的同款数据源——按时段聚合的**真实成交**
/// （均价 + 成交量），而非交易行挂单快照。赛季消耗品（如上品茶饼·兑）价格随赛季
/// 推进持续变化，全历史中位数会严重虚高（实测 555 金 vs 近期真实成交约 297 金），
/// 因此采用分层取价，对齐"近期真实市场价"口径：
///
/// 1. 前一天（北京时间自然日）成交量加权均价；
/// 2. 无前一天成交时，回退近 7 天（含今日）成交量加权均价；
/// 3. 仍无数据时，回退全量成交中位数（对离群值稳健的兜底）。
///
/// 无有效成交数据时返回 None（调用方降级为用户手填）。
async fn fetch_item_price(item_key: &str, server: &str) -> Result<Option<i64>, DropTableError> {
    let body = serde_json::json!({
        "item_id": item_key,
        "server": server,
        "aggregate_type": "hourly",
    });

    let resp = http_client()
        .post(JX3BOX_AUCTION_API_BASE)
        .json(&body)
        .header("Accept", "application/json")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        warn!(
            "[DropTable] 成交聚合接口返回非 2xx: {} (item_key={})",
            resp.status(),
            item_key
        );
        return Ok(None);
    }

    // 响应顶层直接是聚合点数组
    let prices: Vec<PriceRecord> = resp.json().await?;

    let now_secs = chrono::Utc::now().timestamp();
    let today_start = beijing_day_start(now_secs);

    // 层级 1：前一天（北京时间自然日）成交量加权均价
    if let Some(avg) = window_weighted_avg_copper(&prices, today_start - DAY_SECS, today_start) {
        let gold = copper_to_gold(avg);
        info!(
            "[DropTable] 物品 {} 前一天成交均价: {} 金 (server={}, {} 个聚合点)",
            item_key, gold, server, prices.len()
        );
        return Ok(Some(gold));
    }

    // 层级 2：近 7 天（含今日）成交量加权均价。
    // hourly 聚合覆盖近约 30 天，但低频物品前一天可能整日无成交，此时以近一周
    // 成交量加权的均价更接近真实市场价；仍无成交则落入中位数兜底。
    if let Some(avg) = window_weighted_avg_copper(
        &prices,
        today_start - 6 * DAY_SECS,
        today_start + DAY_SECS,
    ) {
        let gold = copper_to_gold(avg);
        info!(
            "[DropTable] 物品 {} 前一天无成交，取近 7 天(含今日)成交均价: {} 金 (server={}, {} 个聚合点)",
            item_key, gold, server, prices.len()
        );
        return Ok(Some(gold));
    }

    // 层级 3：全量中位数兜底（长周期无成交的物品，中位数对离群值稳健）
    match median_price_gold(&prices) {
        Some(gold) => {
            info!(
                "[DropTable] 物品 {} 近期无成交，回退全量中位价: {} 金 (server={}, {} 个聚合点)",
                item_key, gold, server, prices.len()
            );
            Ok(Some(gold))
        }
        None => {
            info!(
                "[DropTable] 物品 {} 无有效成交数据 (server={})",
                item_key, server
            );
            Ok(None)
        }
    }
}

/// 计算成交聚合点的中位单价（金），并四舍五入到整数金。
///
/// 聚合点里可能混有异常高价（如封顶哨兵 1 亿铜、恶意抬价产生的孤立高点）。
/// 算术/加权平均会被这些离群值严重拉高，中位数对离群值天然稳健，
/// 能反映市场主流成交价，适合作为长周期无成交物品的最后兜底。
///
/// 仅 unit_price > 0 且 n_count > 0 的记录参与；无有效数据时返回 None。
fn median_price_gold(prices: &[PriceRecord]) -> Option<i64> {
    let mut valid: Vec<i64> = prices
        .iter()
        .filter(|p| p.unit_price > 0 && p.n_count > 0)
        .map(|p| p.unit_price)
        .collect();

    if valid.is_empty() {
        return None;
    }

    valid.sort_unstable();
    let mid = valid.len() / 2;
    let median = if valid.len() % 2 == 0 {
        (valid[mid - 1] + valid[mid]) / 2
    } else {
        valid[mid]
    };

    // 铜 → 金，四舍五入
    Some((median + 5000) / 10000)
}

/// 同步版本：批量查询材料交易行价格
///
/// 返回 (item_name -> 单价金) 映射。查询失败的物品不会出现在返回值中。
pub fn fetch_material_prices_sync(
    conn: &Connection,
    item_names: &[String],
    server: &str,
) -> HashMap<String, i64> {
    if item_names.is_empty() || server.is_empty() {
        return HashMap::new();
    }

    // 1. 从 drop_items 表查 item_type + item_id，拼成 "ItemType_ItemID"
    let mut name_to_key: HashMap<String, String> = HashMap::new();
    for name in item_names {
        if let Ok((item_type, item_id)) = conn.query_row(
            "SELECT item_type, item_id FROM drop_items WHERE item_name = ?1 LIMIT 1",
            params![name],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, i64>(1)?)),
        ) {
            name_to_key.insert(name.clone(), format!("{}_{}", item_type, item_id));
        }
    }

    if name_to_key.is_empty() {
        info!("[DropTable] 散件材料均未在 drop_items 表中找到 item_key，跳过价格查询");
        return HashMap::new();
    }

    // 2. 先查内存缓存（TTL 1 小时），未命中的物品才发起网络请求
    let cache = material_price_cache();
    let cache_read_at = Instant::now();
    let mut result: HashMap<String, i64> = HashMap::new();
    let mut to_fetch: Vec<(String, String)> = Vec::new(); // (name, item_key)

    {
        let cache_guard = cache.lock().unwrap();
        for (name, key) in &name_to_key {
            let cache_key = format!("{}|{}", key, server);
            match cache_guard.get(&cache_key) {
                Some((price, cached_at))
                    if cache_read_at.duration_since(*cached_at) < MATERIAL_PRICE_CACHE_TTL =>
                {
                    result.insert(name.clone(), *price);
                }
                _ => {
                    to_fetch.push((name.clone(), key.clone()));
                }
            }
        }
    }

    // 3. 并发查询未命中的物品（材料种类少，通常 <= 7，全部并发无压力）
    if !to_fetch.is_empty() {
        let fetched = sync_runtime().handle().block_on(async {
            let futures = to_fetch.into_iter().map(|(name, key)| {
                let server = server.to_string();
                async move {
                    let price = fetch_item_price(&key, &server).await;
                    (name, price)
                }
            });
            join_all(futures).await
        });

        let mut cache_guard = cache.lock().unwrap();
        for (name, price_result) in fetched {
            match price_result {
                Ok(Some(gold)) => {
                    if let Some(key) = name_to_key.get(&name) {
                        cache_guard.insert(format!("{}|{}", key, server), (gold, Instant::now()));
                    }
                    result.insert(name, gold);
                }
                Ok(None) => {
                    info!("[DropTable] 材料 {} 无交易行数据，降级为手填", name);
                }
                Err(e) => {
                    warn!("[DropTable] 材料 {} 价格查询失败: {}，降级为手填", name, e);
                }
            }
        }
    }

    result
}

/// 纯字段分类函数（无名称前缀依赖）
///
/// 分类规则（基于 4518 个物品分析得出）：
/// - API 查不到 → unknown
/// - Source=armor/weapon → equipment
/// - Source=homeland → furniture
/// - Source=trinket → 按 GetType/AucGenre/SubType 细分
/// - Source=other → 按 AucGenre/SubType/IsEquip/CanConsume 细分
pub fn classify_item(item: &ItemMerged) -> String {
    let source = item.source.as_deref().unwrap_or("");

    // Source 为空时（API 查不到字段），视为 unknown
    if source.is_empty() {
        return "unknown".to_string();
    }

    match source {
        "armor" | "weapon" => "equipment".to_string(),
        "homeland" => "furniture".to_string(),
        "trinket" => classify_trinket(item),
        "other" => classify_other(item),
        _ => "unknown".to_string(),
    }
}

/// trinket 分类规则
fn classify_trinket(item: &ItemMerged) -> String {
    let get_type = item.get_type.as_deref().unwrap_or("");
    let auc_genre = item.auc_genre.unwrap_or(-1);
    let sub_type = item.sub_type.unwrap_or(-1);

    // GetType 含"宠物" → 宠物
    if get_type.contains("宠物") {
        return "pet".to_string();
    }

    // AucGenre=5 → 坐骑
    if auc_genre == 5 {
        return "mount".to_string();
    }

    // AucGenre=4 且 SubType∈{11,14} → 挂件
    if auc_genre == 4 && (sub_type == 11 || sub_type == 14) {
        return "pendant".to_string();
    }

    // AucGenre=22 → 挂件
    if auc_genre == 22 {
        return "pendant".to_string();
    }

    // SubType=11 → 宠物
    if sub_type == 11 {
        return "pet".to_string();
    }

    // SubType∈{14,30} → 挂件
    if sub_type == 14 || sub_type == 30 {
        return "pendant".to_string();
    }

    // SubType=23 → 马具
    if sub_type == 23 {
        return "maju".to_string();
    }

    // SubType=17 → 外观
    if sub_type == 17 {
        return "appearance".to_string();
    }

    // 其余 trinket → 装备
    "equipment".to_string()
}

/// other 分类规则
fn classify_other(item: &ItemMerged) -> String {
    let auc_genre = item.auc_genre.unwrap_or(-1);
    let auc_sub_type = item.auc_sub_type.unwrap_or(-1);
    let sub_type = item.sub_type.unwrap_or(-1);
    let quality = item.quality.unwrap_or(-1);
    let is_equip = item.is_equip.unwrap_or(false);
    let can_consume = item.can_consume.unwrap_or(false);

    // AucGenre=20, AucSubType=2
    if auc_genre == 20 && auc_sub_type == 2 {
        if sub_type == 0 {
            if quality == 5 {
                return "xuanjing".to_string();
            } else {
                return "material".to_string();
            }
        }
        if sub_type == -1 {
            // SubType=null → 装备宝箱
            return "equipment".to_string();
        }
    }

    // AucGenre=24
    if auc_genre == 24 {
        if sub_type == 2 {
            return "secret_book".to_string();
        }
        if sub_type == 3 || sub_type == 4 {
            return "material".to_string();
        }
    }

    // AucGenre=15 → 材料
    if auc_genre == 15 {
        return "material".to_string();
    }
    // AucGenre=13 → 材料
    if auc_genre == 13 {
        return "material".to_string();
    }

    // AucGenre=10 → 材料
    if auc_genre == 10 {
        return "material".to_string();
    }

    // AucGenre=6 → 材料
    if auc_genre == 6 {
        return "material".to_string();
    }

    // AucGenre=1, IsEquip=true → 装备
    if auc_genre == 1 && is_equip {
        return "equipment".to_string();
    }

    // AucGenre=22 → 材料
    if auc_genre == 22 {
        return "material".to_string();
    }

    // AucGenre=0, SubType=2 → 外观
    if auc_genre == 0 && sub_type == 2 {
        return "appearance".to_string();
    }

    // IsEquip=false, CanConsume=true → 材料（附魔等）
    if !is_equip && can_consume {
        return "material".to_string();
    }

    // 其余 → unknown
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slash_join() {
        assert_eq!(slash_join("https://a.com/b", 795), "https://a.com/b/795");
        assert_eq!(slash_join("https://a.com/b/", 795), "https://a.com/b/795");
    }

    /// 构造一条价格成交记录（用于中位数单测）
    fn price_record(unit_price: i64, n_count: i64) -> PriceRecord {
        PriceRecord {
            created: 0,
            n_count,
            server: "test".to_string(),
            unit_price,
        }
    }

    #[test]
    fn test_median_price_gold_empty() {
        assert_eq!(median_price_gold(&[]), None);
    }

    #[test]
    fn test_median_price_gold_invalid_filtered() {
        // unit_price 或 n_count <= 0 的记录不参与
        let prices = [price_record(0, 10), price_record(5000, 0)];
        assert_eq!(median_price_gold(&prices), None);
    }

    #[test]
    fn test_median_price_gold_single() {
        // 单条 10 金 → 中位数 10 金
        let prices = [price_record(100000, 5)];
        assert_eq!(median_price_gold(&prices), Some(10));
    }

    #[test]
    fn test_median_price_gold_odd() {
        // 奇数条：10 金、30 金、20 金 → 排序后中位数 20 金
        let prices = [
            price_record(100000, 1),
            price_record(300000, 1),
            price_record(200000, 1),
        ];
        assert_eq!(median_price_gold(&prices), Some(20));
    }

    #[test]
    fn test_median_price_gold_even() {
        // 偶数条：10 金、20 金 → 中位数 (10+20)/2 = 15 金
        let prices = [price_record(100000, 1), price_record(200000, 1)];
        assert_eq!(median_price_gold(&prices), Some(15));
    }

    #[test]
    fn test_median_price_gold_outlier_robust() {
        // 绝代天骄玛瑙真实成交数据：混有封顶哨兵（1 亿铜）与次级异常价（800 金/100 金），
        // 中位数应落在正常价位约 3.2 金，不受离群值影响（加权平均会被拉到 45 金）。
        let prices = [
            price_record(100_000_000, 85),
            price_record(8_002_300, 5),
            price_record(8_002_100, 15),
            price_record(1_002_300, 1000),
            price_record(1_000_000, 105),
            price_record(32_300, 36),
            price_record(32_299, 11),
            price_record(32_000, 13),
            price_record(31_900, 1),
            price_record(31_200, 30),
            price_record(31_199, 947),
            price_record(31_000, 8),
            price_record(22_357, 718),
            price_record(22_200, 67),
            price_record(100_000_000, 85),
        ];
        assert_eq!(median_price_gold(&prices), Some(3));
    }

    /// 构造一条带成交时间戳的价格记录（用于窗口均价单测）
    fn price_record_at(created: i64, unit_price: i64, n_count: i64) -> PriceRecord {
        PriceRecord {
            created,
            n_count,
            server: "test".to_string(),
            unit_price,
        }
    }

    #[test]
    fn test_beijing_day_start() {
        // 2024-01-01 00:00 UTC = 北京时间 08:00，当天零点应为 2024-01-01 00:00 +08
        assert_eq!(beijing_day_start(1704067200), 1704038400);
        // 2024-01-01 15:30 UTC = 北京时间 23:30，仍在同一天
        assert_eq!(beijing_day_start(1704123000), 1704038400);
        // 2024-01-01 16:00 UTC = 北京时间次日 00:00，进入下一天
        assert_eq!(beijing_day_start(1704124800), 1704124800);
    }

    #[test]
    fn test_window_weighted_avg_copper_weighted() {
        // 成交量加权：10 金 ×1 件、20 金 ×3 件 → (10 + 60) / 4 = 17.5 金 ≈ 175000 铜，
        // 四舍五入为 175000 铜（17.5 金）
        let prices = [
            price_record_at(1000, 100_000, 1),
            price_record_at(1000, 200_000, 3),
        ];
        assert_eq!(window_weighted_avg_copper(&prices, 0, 2000), Some(175_000));
    }

    #[test]
    fn test_window_weighted_avg_copper_window_boundary() {
        // 窗口 [start, end)：created == start 参与统计，created == end 不参与
        let prices = [
            price_record_at(1000, 100_000, 1),
            price_record_at(2000, 900_000, 1),
        ];
        assert_eq!(
            window_weighted_avg_copper(&prices, 1000, 2000),
            Some(100_000)
        );
    }

    #[test]
    fn test_window_weighted_avg_copper_filters_invalid_and_sentinel() {
        // 剔除：窗口外、零值无效记录、1 砖封顶哨兵（1 亿铜）
        let prices = [
            price_record_at(500, 100_000, 10),      // 窗口外
            price_record_at(1000, 0, 10),           // 单价非法
            price_record_at(1000, 100_000, 0),      // 数量非法
            price_record_at(1500, BRICK_SENTINEL_COPPER, 85), // 哨兵值
            price_record_at(1800, 50_000, 2),
            price_record_at(1900, 70_000, 2),
        ];
        // 仅后两条参与：(50_000×2 + 70_000×2) / 4 = 60000 铜
        assert_eq!(window_weighted_avg_copper(&prices, 1000, 2000), Some(60_000));
    }

    #[test]
    fn test_window_weighted_avg_copper_empty() {
        assert_eq!(window_weighted_avg_copper(&[], 0, 1000), None);
        let prices = [price_record_at(5000, 100_000, 1)];
        assert_eq!(window_weighted_avg_copper(&prices, 0, 1000), None);
    }

    #[test]
    fn test_window_weighted_avg_copper_tier2_includes_today() {
        // 层级 2 窗口为"近 7 天（含今日）"：[today-6天, today+1天)，
        // 今日成交必须参与统计，7 天前的记录不参与
        let now = 1_787_580_180;
        let today_start = beijing_day_start(now);
        let prices = [
            price_record_at(today_start + 100, 300_000, 2), // 今日成交
            price_record_at(today_start - 6 * DAY_SECS, 100_000, 1), // 窗口起点，含
            price_record_at(today_start - 7 * DAY_SECS, 900_000, 1), // 窗口外
        ];
        // (300000×2 + 100000×1) / 3 = 233333 铜
        assert_eq!(
            window_weighted_avg_copper(
                &prices,
                today_start - 6 * DAY_SECS,
                today_start + DAY_SECS,
            ),
            Some(233_333)
        );
    }

    /// 构建含 seasons / drop_items 最小表结构的内存库（赛季失效单测用）
    fn setup_season_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("打开内存库失败");
        conn.execute_batch(
            r#"
            CREATE TABLE seasons (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                version_id INTEGER NOT NULL,
                start_date INTEGER NOT NULL,
                end_date INTEGER,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );
            CREATE TABLE drop_items (
                item_name TEXT PRIMARY KEY,
                category TEXT,
                season_id INTEGER
            );
            "#,
        )
        .expect("创建测试表失败");
        conn
    }

    #[test]
    fn test_find_stale_equipment_names() {
        let conn = setup_season_test_db();
        // 配置一个覆盖当前时间的赛季（start=0, end=NULL）
        conn.execute(
            "INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, created_at) VALUES ('当前赛季', 1, 0, NULL, 0, '')",
            [],
        )
        .expect("插入赛季失败");
        let current_id: i64 = crate::db::query_current_season_id(&conn).expect("应有生效赛季");

        let insert = |name: &str, category: &str, season_id: Option<i64>| {
            conn.execute(
                "INSERT INTO drop_items (item_name, category, season_id) VALUES (?1, ?2, ?3)",
                params![name, category, season_id],
            )
            .expect("插入物品失败");
        };
        insert("旧赛季装备", "equipment", Some(current_id - 1));
        insert("无标记装备", "equipment", None);
        insert("新赛季装备", "equipment", Some(current_id));
        insert("上赛季材料", "material", Some(current_id - 1));
        insert("无分类装备", "", Some(current_id - 1));

        let names = vec![
            "旧赛季装备".to_string(),
            "无标记装备".to_string(),
            "新赛季装备".to_string(),
            "上赛季材料".to_string(),
            "未入库装备".to_string(),
            "无分类装备".to_string(),
        ];
        assert_eq!(
            find_stale_equipment_names(&conn, &names),
            vec!["旧赛季装备".to_string(), "无标记装备".to_string()]
        );
    }

    #[test]
    fn test_find_stale_equipment_names_no_season_configured() {
        let conn = setup_season_test_db();
        // 未配置任何赛季时跳过失效检查，避免误清缓存
        assert_eq!(crate::db::query_current_season_id(&conn), None);
        conn.execute(
            "INSERT INTO drop_items (item_name, category, season_id) VALUES ('旧装备', 'equipment', 42)",
            [],
        )
        .expect("插入物品失败");
        assert_eq!(
            find_stale_equipment_names(&conn, &["旧装备".to_string()]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn test_drop_item_deserialize() {
        let json = r#"{
            "MapID": 795,
            "ItemType": 7,
            "ItemID": 109369,
            "ItemExtID": 0,
            "ItemName": "鹤芝护腰·悟",
            "ItemQuality": 4,
            "ItemIconID": 27411,
            "BossName": "唐醉",
            "ApplicableSchoolIDs": "2|3|4|5|8|9|10|19|23|24|38|39",
            "id": 11753
        }"#;
        let item: DropItem = serde_json::from_str(json).unwrap();
        assert_eq!(item.map_id, 795);
        assert_eq!(item.item_name, "鹤芝护腰·悟");
        assert_eq!(item.boss_name, "唐醉");
    }

    #[test]
    fn test_classify_armor() {
        let item = ItemMerged {
            id: "7_1".to_string(),
            source: Some("armor".to_string()),
            name: "流漓腰带".to_string(),
            ui_id: None, source_id: None, desc: None, genre: None,
            sub_type: Some(6), detail_type: None, price: None, level: None,
            bind_type: None, max_durability: None, abrade_rate: None,
            max_exist_time: None, max_exist_amount: None,
            can_trade: None, can_destroy: None, quality: Some(4),
            skill_id: None, skill_level: None,
            belong_school: None, magic_kind: None, magic_type: None,
            get_type: None, icon_id: None, can_set_color: None,
            auc_genre: Some(3), auc_sub_type: Some(3), require_camp: None,
            max_strength_level: None, can_apart: None, can_exterior: None,
            can_change_magic: None, can_shared: None, repair_price_rebate: None,
            can_stack: None, can_consume: None, require_level: None,
            require_gender: None, require_homeland_level: None,
            can_use_on_horse: None, can_use_in_fight: None,
            can_good_camp_use: None, can_evil_camp_use: None, can_neutral_camp_use: None,
            diamonds: None, requires: None, recommend: None, recommend_xfs: None,
            type_label: Some("腰带".to_string()), appearance: None,
            attribute_types: None, cool_down: None, is_quest: None,
            wu_cai_html: None, set: None, is_equip: Some(true),
            equip_usage: None, image_url: None, get_source: None,
            attributes: None, furniture_attributes: None, id_key: None,
        };
        assert_eq!(classify_item(&item), "equipment");
    }

    #[test]
    fn test_classify_xuanjing() {
        let item = ItemMerged {
            id: "5_1".to_string(),
            source: Some("other".to_string()),
            name: "昆玉玄晶".to_string(),
            ui_id: None, source_id: None, desc: None, genre: None,
            sub_type: Some(0), detail_type: None, price: None, level: None,
            bind_type: None, max_durability: None, abrade_rate: None,
            max_exist_time: None, max_exist_amount: None,
            can_trade: None, can_destroy: None, quality: Some(5),
            skill_id: None, skill_level: None,
            belong_school: None, magic_kind: None, magic_type: None,
            get_type: None, icon_id: None, can_set_color: None,
            auc_genre: Some(20), auc_sub_type: Some(2), require_camp: None,
            max_strength_level: None, can_apart: None, can_exterior: None,
            can_change_magic: None, can_shared: None, repair_price_rebate: None,
            can_stack: None, can_consume: None, require_level: None,
            require_gender: None, require_homeland_level: None,
            can_use_on_horse: None, can_use_in_fight: None,
            can_good_camp_use: None, can_evil_camp_use: None, can_neutral_camp_use: None,
            diamonds: None, requires: None, recommend: None, recommend_xfs: None,
            type_label: None, appearance: None,
            attribute_types: None, cool_down: None, is_quest: None,
            wu_cai_html: None, set: None, is_equip: Some(false),
            equip_usage: None, image_url: None, get_source: None,
            attributes: None, furniture_attributes: None, id_key: None,
        };
        assert_eq!(classify_item(&item), "xuanjing");
    }

    /// 端到端验证：缓存优先策略
    ///
    /// 验证流程：
    /// 1. 清空 drop_items 表
    /// 2. 第一次调用 classify_drops_sync → 应调 API，填充 drop_items
    /// 3. 第二次调用 classify_drops_sync → 应全命中缓存，drop_items 数量不变
    /// 4. 两次返回的分类结果应一致
    ///
    /// 运行方式：cargo test test_e2e_cache_first -- --ignored --nocapture
    #[test]
    #[ignore] // 需要网络访问 jx3box API，且操作生产数据库
    fn test_e2e_cache_first_optimization() {
        // 使用 25人普通阆风悬城 (MapID=795) 的真实掉落物品（来自 drop/v2 API）
        let map_id: i64 = 795;
        let raid_name = "25人普通阆风悬城";
        let drop_names = vec![
            "五行石（六级）".to_string(),       // ItemType=5, ItemID=24428
            "维峰丹".to_string(),               // ItemType=5, ItemID=44430
            "流漓腰带".to_string(),             // ItemType=7, ItemID=109335
            "《易筋经·秘卷》".to_string(),      // ItemType=5, ItemID=45798
            "肆级五彩石".to_string(),           // ItemType=5, ItemID=21511
        ];

        // 0. 初始化数据库并清空 drop_items 表
        let conn = crate::db::init_db().expect("init_db 失败");
        conn.execute("DELETE FROM drop_items", [])
            .expect("清空 drop_items 失败");

        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM drop_items", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(count_before, 0, "清空后 drop_items 应为 0 条");
        drop(conn); // 释放连接，让 classify_drops_sync 能获取写锁

        println!("========== 第一次调用（缓存为空，应调 API） ==========");
        let result1 = classify_drops_sync(map_id, Some(raid_name), &drop_names)
            .expect("第一次 classify_drops_sync 失败");
        println!("第一次返回分类: {:?}", result1);

        // 验证第一次调用后 drop_items 有数据
        let conn = crate::db::init_db().expect("init_db 失败");
        let count_after_first: i64 = conn
            .query_row("SELECT COUNT(*) FROM drop_items", [], |row| row.get(0))
            .unwrap_or(0);
        println!("第一次调用后 drop_items 条数: {}", count_after_first);
        assert!(
            count_after_first > 0,
            "第一次调用后 drop_items 应有数据（从 API 获取）"
        );
        drop(conn);

        println!("========== 第二次调用（应全命中缓存，跳过 API） ==========");
        let result2 = classify_drops_sync(map_id, Some(raid_name), &drop_names)
            .expect("第二次 classify_drops_sync 失败");
        println!("第二次返回分类: {:?}", result2);

        // 验证第二次调用后 drop_items 数量不变（没有新 API 调用）
        let conn = crate::db::init_db().expect("init_db 失败");
        let count_after_second: i64 = conn
            .query_row("SELECT COUNT(*) FROM drop_items", [], |row| row.get(0))
            .unwrap_or(0);
        println!("第二次调用后 drop_items 条数: {}", count_after_second);
        assert_eq!(
            count_after_first, count_after_second,
            "第二次调用后 drop_items 数量应不变（全命中缓存，无新 API 调用）"
        );
        drop(conn);

        // 验证两次返回的分类结果一致
        assert_eq!(
            result1, result2,
            "两次调用的分类结果应完全一致"
        );

        // 验证每个输入物品都有分类结果
        for name in &drop_names {
            assert!(
                result2.contains_key(name),
                "物品 '{}' 应在分类结果中",
                name
            );
        }

        println!("========== 端到端验证通过 ==========");
        println!("  - 第一次调用: {} 条物品从 API 获取并缓存", count_after_first);
        println!("  - 第二次调用: {} 条物品全命中缓存，0 次 API 调用", count_after_second);
        println!("  - 两次分类结果一致: ✓");
    }

    /// 验证：部分缓存命中时，仅查询未命中物品
    ///
    /// 预置 2 个物品到 drop_items，传入 5 个物品（2 缓存 + 3 未缓存），
    /// 验证 drop_items 新增量 = 3（仅未命中物品调 API）
    #[test]
    #[ignore]
    fn test_e2e_partial_cache_hit() {
        let map_id: i64 = 795;
        let raid_name = "25人普通阆风悬城";

        // 预置缓存的物品（模拟之前已分类的物品）
        let cached_items = vec![
            ("五行石（六级）".to_string(), "material".to_string()),
            ("维峰丹".to_string(), "material".to_string()),
        ];

        // 未缓存的物品（真实存在于 drop/v2 API 中）
        let uncached_items = vec![
            "流漓腰带".to_string(),
            "《易筋经·秘卷》".to_string(),
            "肆级五彩石".to_string(),
        ];

        let all_items: Vec<String> = cached_items
            .iter()
            .map(|(n, _)| n.clone())
            .chain(uncached_items.iter().cloned())
            .collect();

        // 初始化并清空 drop_items
        let conn = crate::db::init_db().expect("init_db 失败");
        conn.execute("DELETE FROM drop_items", [])
            .expect("清空失败");

        // 手动插入缓存物品
        let now = chrono::Local::now().to_rfc3339();
        for (name, category) in &cached_items {
            conn.execute(
                "INSERT OR REPLACE INTO drop_items (item_type, item_id, item_name, category, class_source, created_at)
                 VALUES (0, 0, ?1, ?2, 'test', ?3)",
                params![name, category, now],
            )
            .expect("插入缓存物品失败");
        }

        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM drop_items", [], |row| row.get(0))
            .unwrap_or(0);
        assert_eq!(count_before, 2, "预置缓存后应有 2 条");
        drop(conn);

        println!("========== 部分缓存命中测试 ==========");
        println!("  缓存: {} 条, 未缓存: {} 条, 总计: {} 条",
            cached_items.len(), uncached_items.len(), all_items.len());

        let result = classify_drops_sync(map_id, Some(raid_name), &all_items)
            .expect("classify_drops_sync 失败");

        // 验证 drop_items 新增量
        let conn = crate::db::init_db().expect("init_db 失败");
        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM drop_items", [], |row| row.get(0))
            .unwrap_or(0);
        println!("  调用后 drop_items: {} 条（新增 {} 条）", count_after, count_after - count_before);
        drop(conn);

        // 验证所有物品都有分类
        for name in &all_items {
            assert!(
                result.contains_key(name),
                "物品 '{}' 应在分类结果中",
                name
            );
        }

        // 验证缓存物品的分类未被覆盖
        let empty = String::new();
        for (name, expected_cat) in &cached_items {
            let actual = result.get(name).unwrap_or(&empty);
            assert_eq!(
                actual, expected_cat,
                "缓存物品 '{}' 的分类应保持为 '{}'",
                name, expected_cat
            );
        }

        println!("========== 部分缓存命中测试通过 ==========");
    }
}
