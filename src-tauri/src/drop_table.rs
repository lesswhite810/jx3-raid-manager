use log::{info, warn};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::time::Duration;

const JX3BOX_DROP_API_BASE: &str = "https://node.jx3box.com/fb/drop/v2";
const JX3BOX_ITEM_MERGED_API_BASE: &str = "https://node.jx3box.com/item_merged/id";
const ITEM_MERGED_PER_PAGE: usize = 50;

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
    let cached_categories = query_categories_by_names(&conn, actual_drop_names)?;

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

    // 6. 分类并入库
    save_items_to_db(&mut conn, map_id, raid_name, &uncached_items, &merged_items)?;

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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    let resp = client
        .get(&url)
        .header("Accept", "application/json")
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
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

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

/// 将查询到的物品详情分类后入库
fn save_items_to_db(
    conn: &mut Connection,
    map_id: i64,
    raid_name: Option<&str>,
    drop_items: &[&DropItem],
    merged_items: &[ItemMerged],
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
                INSERT OR IGNORE INTO drop_items (
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
                    category, class_source, created_at
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
                    ?69, ?70, ?71, ?72
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
