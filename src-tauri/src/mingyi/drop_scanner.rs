use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Datelike;

use crate::app_config;
use crate::db;
use crate::game_directory::MINGYI_ACCOUNTS_BASE_PATH;
use crate::mingyi::active_detector::parse_info_jx3dat;

// === 正则表达式预编译 ===

// parse_jcl_filename 用
static JCL_FILENAME_RE: LazyLock<regex::Regex> = LazyLock::new(||
    regex::Regex::new(r"^(\d{4})-(\d{2})-(\d{2})-(\d{2})-(\d{2})-(\d{2})-(.+?)\((\d+)\)-(.*?)\((\d+)\)\.jcl$").unwrap()
);

// parse_money_from_msg 用
static GOLDB_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r#"text="(\d+)"[^>]*name="Text_GoldB""#).unwrap());
static GOLD_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r#"text="(\d+)"[^>]*name="Text_Gold""#).unwrap());
static SILVER_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r#"text="(\d+)"[^>]*name="Text_Silver""#).unwrap());
static COPPER_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r#"text="(\d+)"[^>]*name="Text_Copper""#).unwrap());

// type=21 的 result 表中，13/14 分别是对目标的有效伤害/有效治疗。
// 这里只解析原始数值，不从文本、成就或副本进度推断击杀。
static DAMAGE_EFF_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\[13\]\s*=\s*(-?\d+)").unwrap());
static HEAL_EFF_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\[14\]\s*=\s*(-?\d+)").unwrap());

// parse_expense_amount 用
static BRICK_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(\d+)金砖").unwrap());
static EXPENSE_GOLD_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"(\d+)金").unwrap());

// extract_drops_from_chatlog 用
static ITEM_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"分配者将：\[([^\]]+)\]分配给").unwrap());
static SALARY_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"每人底薪：(\d+)金").unwrap());
static EXPENSE_MSG_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\[([^\]]+)\]花费\[([^\]]+)\]购买了\[([^\]]+)\]").unwrap());
static PURCHASED_ITEM_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"购买了\[([^\]]+)\]").unwrap());
/// "记录给了"格式：[分配者]将[物品名]以[金额]记录给了[接收者]
/// 当团长手动分配物品时使用此格式，有时不生成"花费购买了"消息
static ALLOCATE_TO_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"\[([^\]]+)\]将\[([^\]]+)\]以\[([^\]]+)\]记录给了\[([^\]]+)\]").unwrap());
/// 本地角色入包日志：`你获得：[物品名]。` / `你获得：[物品名] × 17。`
/// 仅匹配 MSG_ITEM 中以"你获得："开头的行（本地视角），其他玩家的"XX获得："不匹配。
/// ×N 后缀为真实数量，缺失时视为 1。
static GAIN_QTY_RE: LazyLock<regex::Regex> = LazyLock::new(|| regex::Regex::new(r"^你获得：\[([^\]]+)\](?:\s*×\s*(\d+))?").unwrap());

/// 散件材料白名单（硬编码 7 种副本材料）
/// 这些材料无人竞拍时通常由散件老板统一接收
const SCRAPS_MATERIAL_WHITELIST: [&str; 7] = [
    "上品茶饼·兑", "维峰丹", "玛瑙", "猫眼石",
    "叁级五彩石", "五行石（六级）", "肆级五彩石",
];

/// 当前角色购买物品的信息（同物品多次购买累加）
#[derive(Debug, Clone, Default)]
struct PurchaseInfo {
    /// 购买数量：由购买消息前后 2 秒内的"你获得：[物品] × N。"到账事件配对求和；
    /// 无到账事件时按 1 笔计。可堆叠材料一次交易可到账多个，故为数量而非次数。
    count: u32,
    /// 累计购买价（用于判断 0 金购买=散件）
    total_price: i64,
}

/// 散件清单项（写入 RaidRecord JSON 的 scrapsItems）
///
/// 序列化为 camelCase（unitPrice / priceSource），与前端 types.ts 的 ScrapsItem 对齐。
/// v2.1.53 曾因缺少 rename_all 输出 snake_case 键，导致前端读不到单价与来源（已修复）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ScrapsItem {
    name: String,
    count: u32,
    /// 单价（金）。None 表示需用户在前端手填
    unit_price: Option<i64>,
    /// 该物品的累计实际购买价（金），来自"花费[..]购买了"消息；0 金购买（免费散件）为 0
    total_price: i64,
    /// "material" | "equipment"
    category: String,
    /// "jx3box" | "npc" | "manual"
    price_source: String,
}

/// 铜 → 金换算（四舍五入），与 drop_table::median_price_gold 口径一致
fn copper_to_gold(copper: i64) -> i64 {
    (copper + 5000) / 10000
}

/// 查询 drop_items 表中已缓存物品的单价（装备估价用）
///
/// 返回 (item_name -> price) 的映射，仅包含已缓存的物品。
/// 调用方对未命中的物品保持 unit_price = None（前端手填）。
fn query_prices_by_names(conn: &Connection, names: &[String]) -> HashMap<String, i64> {
    let mut result = HashMap::new();
    for name in names {
        if let Ok(price) = conn.query_row(
            "SELECT price FROM drop_items WHERE item_name = ?1 AND price IS NOT NULL",
            params![name],
            |row| row.get::<_, i64>(0),
        ) {
            result.insert(name.clone(), price);
        }
    }
    result
}

/// 计算散件清单 + 估价 + 散件支出
///
/// 输入：
/// - purchased_items：当前角色购买的物品映射（来自 chatlog 解析）
/// - item_categories：物品名 -> 分类（来自 classify_drops_sync）
/// - role_server：角色所在服务器（用于 JX3Box 交易行价格查询）
///
/// 规则：
/// - 散件材料：物品名 ∈ 白名单
/// - 散件装备：category == "equipment" && total_price == 0
///
/// 估价：
/// - 散件材料：调用 JX3Box 交易行成交聚合接口获取最新市场价（真实成交），
///   失败时降级为手填。
/// - 散件装备：直接使用 drop_items.Price（NPC 卖价，单位为铜需换算为金，
///   v2.1.53 曾直接当金使用导致估价虚高上万倍），无记录时手填。
///   装备不查询交易行市场价。
///
/// 散件支出（scraps_expense）：
/// - 仅统计白名单材料的实际购买花费合计（来自"花费[..]购买了"消息的金额）。
/// - 0 金装备为免费获取，不计支出；其他装备/小铁等购买属于非散件支出，计入
///   goldExpense（总支出）但不计入散件支出。
///
/// 返回：散件清单（已按名称排序）、scrapsValue（仅含已获取单价的物品）、scrapsExpense。
fn compute_scraps_items(
    conn: &Connection,
    purchased_items: &HashMap<String, PurchaseInfo>,
    item_categories: &HashMap<String, String>,
    role_server: &str,
) -> (Vec<ScrapsItem>, i64, i64) {
    if purchased_items.is_empty() {
        return (Vec::new(), 0, 0);
    }

    // 收集所有购买的物品名，用于批量查 drop_items.Price（装备 NPC 卖价）
    let purchased_names: Vec<String> = purchased_items.keys().cloned().collect();
    let price_map = query_prices_by_names(conn, &purchased_names);

    // 仅白名单材料批量查 JX3Box 交易行市场价（装备直接用 NPC 卖价，不查市场价）
    let market_names: Vec<String> = purchased_items
        .keys()
        .filter(|name| SCRAPS_MATERIAL_WHITELIST.contains(&name.as_str()))
        .cloned()
        .collect();
    let jx3box_price_map = crate::drop_table::fetch_market_prices_sync(
        conn,
        &market_names,
        role_server,
    );

    let mut items: Vec<ScrapsItem> = Vec::new();
    let mut total_value: i64 = 0;
    let mut scraps_expense: i64 = 0;

    for (name, info) in purchased_items {
        let category = item_categories.get(name).map(|s| s.as_str()).unwrap_or("");
        let is_material = SCRAPS_MATERIAL_WHITELIST.contains(&name.as_str());
        let is_zero_price_equipment = category == "equipment" && info.total_price == 0;

        if !is_material && !is_zero_price_equipment {
            continue;
        }

        let (unit_price, price_source) = if is_material {
            // 散件材料：优先使用 JX3Box 交易行价格
            match jx3box_price_map.get(name).copied() {
                Some(p) => (Some(p), "jx3box".to_string()),
                None => (None, "manual".to_string()),
            }
        } else {
            // 散件装备：直接使用 NPC 卖价（铜→金），无记录则手填
            match price_map.get(name).copied() {
                Some(p) => (Some(copper_to_gold(p)), "npc".to_string()),
                None => (None, "manual".to_string()),
            }
        };

        if let Some(p) = unit_price {
            total_value += info.count as i64 * p;
        }
        if is_material {
            scraps_expense += info.total_price;
        }

        items.push(ScrapsItem {
            name: name.clone(),
            count: info.count,
            unit_price,
            total_price: info.total_price,
            category: if is_material { "material".to_string() } else { "equipment".to_string() },
            price_source,
        });
    }

    // 按名称排序，保证输出稳定
    items.sort_by(|a, b| a.name.cmp(&b.name));

    (items, total_value, scraps_expense)
}

/// JCL 文件名解析结果
#[derive(Debug, Clone)]
struct JclFileInfo {
    /// 文件名（不含路径）
    file_name: String,
    /// 时间戳（毫秒）
    timestamp: i64,
    /// JCL 副本名（如 "25人英雄阆风悬城"）
    raid_display_name: String,
    /// 副本 ID（与 JX3Box 掉落接口的 MapID 同源，如阆风悬城=795）
    raid_id: i64,
    /// BOSS 名
    boss_name: String,
    /// BOSS 模板 ID（来自文件名，用于 JCL 内容中 NPC_INFO 的 dwTemplateID 匹配）
    boss_id: i64,
}

/// JCL 文件内容分析结果
///
/// 通过解析 JCL 文件内容提取以下信息：
/// - 真实 BOSS 名（当文件名 BOSS 不在 raid_bosses 配置中时，从内容搜索）
/// - 战斗开始/结束时间（来自 FIGHT_TIME 事件，比文件名时间更精确）
/// - BOSS 击杀状态（基于同一运行时实体的参战加 type=28/HP=0 终局死亡，或专属宝箱判定）
#[derive(Debug, Clone)]
struct JclAnalysis {
    /// 真实 BOSS 名（从 JCL 内容解析，当文件名 BOSS 不在配置中时使用）
    boss_name: Option<String>,
    /// 战斗开始时间（毫秒，Unix timestamp）
    /// 来自首个 FIGHT_TIME 事件（bFighting=true）的时间戳
    fight_start_ms: i64,
    /// 战斗结束时间（毫秒，Unix timestamp）
    /// 来自最后 FIGHT_TIME 事件（bFighting=false）的时间戳
    fight_end_ms: i64,
    /// 是否按当前解析规则确认击杀；false 也可能是证据不足，并不等于确认拉脱。
    /// 宝箱与首领战斗事件参与判定，成就和奖励消息不参与。
    is_kill: bool,
}

/// 外部 10 金只能补强这一类已经有完整战斗证据的候选。
///
/// 它刻意不写入 JclAnalysis / SQLite 缓存：`is_kill` 始终只代表 JCL 内的强证据，
/// 本结构只在单次扫描的聚类阶段消耗。
#[derive(Debug, Clone, PartialEq, Eq)]
struct DamageCompletionCandidate {
    boss_name: String,
    fight_end_ms: i64,
}

/// 单个运行时 NPC 的击杀证据。
///
/// 仅接受同一 dwID 的参战和终局死亡（死亡通知或 HP=0）。若其后再次进入战斗或
/// 报告正血量，则前面的死亡证据已不再是终局证据。
#[derive(Default)]
struct RuntimeEntityEvidence {
    participated: bool,
    terminal_death: bool,
}

impl RuntimeEntityEvidence {
    fn observe_fight_hint(&mut self, in_fight: bool, hp: f64) {
        if in_fight {
            self.participated = true;
            self.terminal_death = false;
        } else if hp > 0.0 && self.terminal_death {
            // 终局死亡后同一 dwID 又以满血脱战，说明此前死亡不再能代表终局；
            // 后续死亡必须先有新的 bFight=true。普通脱战本身不推断新的战斗轮次。
            self.participated = false;
            self.terminal_death = false;
        }
        if hp == 0.0 {
            self.terminal_death = true;
        }
    }

    fn observe_death_notify(&mut self) {
        self.terminal_death = true;
    }

    fn is_confirmed_kill(&self) -> bool {
        self.participated && self.terminal_death
    }
}

/// chatlog 预加载记录（单条 ChatLog 行的内存表示）
///
/// 用于 scan_raid_drops_with_raids 入口处一次性加载所有 chatlog 文件中的相关记录，
/// 避免后续每个副本实例重复打开 SQLite 连接和全表扫描。
#[derive(Clone, Debug)]
struct ChatlogRecord {
    /// 消息类型：MSG_ITEM / MSG_MONEY / MSG_ROOM / MSG_WHISPER
    msg_type: String,
    /// text 字段（消息文本）
    text: String,
    /// msg 字段（XML 格式的金额信息）
    msg: String,
    /// 时间戳（秒级）
    time_sec: i64,
}

/// 副本实例（聚类后的一场副本）
#[derive(Debug)]
struct RaidInstance {
    /// 账号 UID
    account_id: String,
    /// JCL 副本名（如 "25人英雄阆风悬城"）
    raid_display_name: String,
    /// 副本 MapID（与 JX3Box 掉落接口同源，如阆风悬城=795）
    /// 取自 JCL 文件名的副本 ID，用于查询理论掉落表
    map_id: i64,
    /// 副本开始时间：第一个成功击杀 JCL 的文件名时间（毫秒）
    start_time: i64,
    /// 副本结束时间：最后一个成功击杀 JCL 的战斗结束时间（毫秒）
    #[allow(dead_code)]
    end_time: i64,
    /// 最后一个 JCL（含拉托）的文件名时间（毫秒），用于确定聊天记录结束范围
    last_jcl_time: i64,
    /// 已击杀 BOSS 名列表（按 raid_bosses 顺序）
    bosses_killed: Vec<String>,
    /// 成功击杀的 JCL 文件名列表（仅通关的）
    jcl_files: Vec<String>,
    /// 每个成功 JCL 对应的 BOSS 名（与 jcl_files 一一对应）
    jcl_boss_names: Vec<String>,
    /// 成功击杀 BOSS 数（=通关 JCL 数）
    boss_kill_count: u32,
    /// 聊天记录分析的起始时间（毫秒）
    /// 取首个成功 JCL 的战斗开始时间（fight_start_ms），比文件名时间更精确
    first_gold_time: i64,
}

/// 从 app_config 获取游戏目录（自动补全为 bin/zhcn_hd 运行目录）
fn get_game_directory() -> Result<String, String> {
    let config = app_config::get_app_config_internal()?;
    let raw = config
        .game_directory
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "未配置游戏目录".to_string())?;
    // 补全路径（如 E:\Game\SeasunGame → E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd）
    let resolved = crate::game_directory::resolve_game_runtime_directory(&raw);
    if resolved != raw {
        log::info!("[DropScanner] 游戏目录补全: {} → {}", raw, resolved);
    }
    Ok(resolved)
}

/// 角色在数据库中的 UUID 身份
struct DbRoleIdentity {
    /// roles.id（UUID）
    role_id: String,
    /// accounts.id（UUID）
    account_id: String,
    /// 角色名（用于匹配 chatlog 支出消息中的买家）
    role_name: String,
    /// 服务器（用于扫描记录 JSON 的 server 字段）
    server: String,
    /// 大区（用于扫描记录 JSON 的 region 字段）
    region: String,
}

/// 从茗伊账号目录解析 info.jx3dat 获取角色身份，
/// 然后查询 roles 表映射到数据库 UUID。
///
/// info.jx3dat 提供数字 uid + role_name + server，
/// 但 records 表的 account_id/role_id 是 UUID 格式。
/// 必须通过 role_name + server 查询 roles 表得到 UUID。
///
/// 返回 None 表示角色未在数据库中登记，调用方应跳过此账号。
fn resolve_db_role_identity(
    conn: &Connection,
    account_dir: &Path,
    game_path: &Path,
) -> Result<Option<DbRoleIdentity>, String> {
    let info_path = account_dir.join("info.jx3dat");
    let identity = match parse_info_jx3dat(&info_path, Some(game_path)) {
        Some(id) => id,
        None => {
            log::warn!(
                "[DropScanner] 无法解析 info.jx3dat: {}",
                info_path.display()
            );
            return Ok(None);
        }
    };

    // 通过 role_name + server 查询 roles 表
    // 游戏角色名不允许包含空格，但茗伊 info.jx3dat 解析时可能引入空格
    // （如"少年 白了发" vs 数据库"少年白了发"），因此查询时去除空格后再比较
    let result: Result<(String, String), rusqlite::Error> = conn.query_row(
        "SELECT r.id, r.account_id FROM roles r \
         WHERE REPLACE(REPLACE(r.name, ' ', ''), '\u{3000}', '') = REPLACE(REPLACE(?1, ' ', ''), '\u{3000}', '') \
         AND r.server = ?2 LIMIT 1",
        params![identity.role_name, identity.server],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );

    match result {
        Ok((role_id, account_id)) => {
            log::info!(
                "[DropScanner] 角色身份映射: uid={}, name={}, server={} -> role_id={}, account_id={}",
                identity.uid,
                identity.role_name,
                identity.server,
                role_id,
                account_id
            );

            // 修复孤儿记录：role_id 匹配但 account_id 不一致的记录
            // 场景：账号被删除后重新添加（手动/导入），account_id 变了但 role_id 不变
            let orphan_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM records WHERE role_id = ?1 AND account_id != ?2",
                    params![&role_id, &account_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);

            if orphan_count > 0 {
                // 更新表列
                let updated = conn.execute(
                    "UPDATE records SET account_id = ?1 WHERE role_id = ?2 AND account_id != ?1",
                    params![&account_id, &role_id],
                ).map_err(|e| format!("更新孤儿记录 account_id 失败: {}", e))?;
                // 同步更新 JSON data 中的 accountId 字段（前端从 data 读取）
                // 过滤条件改用 account_id 列（已存在），避免 json_extract 全表扫描
                conn.execute(
                    "UPDATE records SET data = json_set(data, '$.accountId', ?1) \
                     WHERE role_id = ?2 AND account_id != ?1",
                    params![&account_id, &role_id],
                ).map_err(|e| format!("更新孤儿记录 JSON accountId 失败: {}", e))?;
                log::info!(
                    "[DropScanner] 修复 {} 条孤儿记录: role_id={} -> account_id={} (rows affected: {})",
                    orphan_count,
                    role_id,
                    account_id,
                    updated
                );
            }

            Ok(Some(DbRoleIdentity {
                role_id,
                account_id,
                role_name: identity.role_name.clone(),
                server: identity.server.clone(),
                region: identity.region.clone(),
            }))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            log::warn!(
                "[DropScanner] 角色未在数据库中登记，跳过: uid={}, name={}, server={}",
                identity.uid,
                identity.role_name,
                identity.server
            );
            Ok(None)
        }
        Err(e) => Err(format!("查询 roles 表失败: {}", e)),
    }
}

/// 解析 JCL 文件名
///
/// 格式：YYYY-MM-DD-HH-MM-SS-副本名(副本ID)-BOSS名(BOSS ID).jcl
/// 从 JCL 文件名提取日期前缀（YYYY-MM-DD）
/// 文件名格式：YYYY-MM-DD-HH-MM-SS-副本名(副本ID)-BOSS名(BOSS ID).jcl
/// 仅做轻量字符串提取，不做 regex/chrono 解析，用于快速范围过滤
fn extract_date_prefix(file_name: &str) -> Option<String> {
    // 文件名至少需要 10 个字符来包含 YYYY-MM-DD
    if file_name.len() < 10 {
        return None;
    }
    let prefix = &file_name[..10];
    // 验证格式：YYYY-MM-DD
    let bytes = prefix.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    // 验证年月日均为数字
    for i in 0..10 {
        if i == 4 || i == 7 {
            continue;
        }
        if !bytes[i].is_ascii_digit() {
            return None;
        }
    }
    Some(prefix.to_string())
}

/// 将 Unix 毫秒时间戳转换为本地时区（UTC+8）的日期字符串 YYYY-MM-DD
/// 用于与 JCL 文件名前缀做字符串比较
fn timestamp_to_local_date_string(ms: i64) -> String {
    // 转换为 UTC+8 的秒级时间戳
    let local_secs = (ms / 1000) + 8 * 3600;
    let days = local_secs.div_euclid(86400);
    // 1970-01-01 起的天数转日期
    let (year, month, day) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", year, month, day)
}

/// 将 1970-01-01 起的天数转换为年月日
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // 使用 chrono 做转换，确保正确性
    let date = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        + chrono::Duration::days(days);
    (date.year(), date.month(), date.day())
}

fn parse_jcl_filename(file_name: &str) -> Option<JclFileInfo> {
    // 先尝试 UTF-8 解析，如果不是则可能是 GBK 编码的 bytes
    // 但由于 Rust 的 OsString 在 Windows 上可能是 WTF-8/UTF-8，
    // 我们直接用正则解析字符串形式
    let caps = JCL_FILENAME_RE.captures(file_name)?;

    let year: i32 = caps[1].parse().ok()?;
    let month: u32 = caps[2].parse().ok()?;
    let day: u32 = caps[3].parse().ok()?;
    let hour: u32 = caps[4].parse().ok()?;
    let min: u32 = caps[5].parse().ok()?;
    let sec: u32 = caps[6].parse().ok()?;

    // 构建时间戳（毫秒）
    // JCL 文件名为本地时间（UTC+8），将其视为 UTC+8 时间后转换为 Unix 毫秒时间戳
    let dt = chrono::NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(year, month, day)?,
        chrono::NaiveTime::from_hms_opt(hour, min, sec)?,
    );
    // dt.and_utc() 把 dt 当作 UTC，得到的 timestamp_millis 比"UTC+8 时间对应的 Unix 时间戳"多 8 小时
    // 因此需要减去 8*3600*1000 毫秒，得到正确的 Unix 时间戳
    let timestamp = dt.and_utc().timestamp_millis() - 8 * 3600 * 1000;

    Some(JclFileInfo {
        file_name: file_name.to_string(),
        timestamp,
        raid_display_name: caps[7].to_string(),
        raid_id: caps[8].parse().unwrap_or(0),
        boss_name: caps[9].to_string(),
        boss_id: caps[10].parse().unwrap_or(0),
    })
}

/// 扫描指定账号目录下的 JCL 文件并解析
///
/// `since_ms`：只扫描 mtime >= since_ms 的 JCL 文件。
/// 通常传入 JX3 进程启动时间，只扫描本次会话产生的 JCL。
/// 传入 0 表示不过滤（扫描全部，仅用于测试）。
///
/// `start_ms`：副本时间下限（基于 JCL 文件名解析的副本开始时间）。
/// 用于"扫描本周"等按时间范围扫描的场景，传入 0 表示不过滤。
///
/// `end_ms`：副本时间上限（基于 JCL 文件名解析的副本开始时间，开区间）。
/// 用于"扫描本周"等按时间范围扫描的场景，传入 0 表示不过滤。
fn scan_jcl_files(
    account_dir: &PathBuf,
    since_ms: i64,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<JclFileInfo>, String> {
    let combat_logs_dir = account_dir.join("userdata").join("combat_logs");

    if !combat_logs_dir.exists() {
        return Ok(Vec::new());
    }

    // 快速跳过：检查 combat_logs 目录的 mtime。
    // 目录 mtime 在新文件创建时更新。若 mtime 早于 since_ms，
    // 说明本次会话无新 JCL 产生，直接返回空列表，避免遍历数千个历史文件。
    if since_ms > 0 {
        if let Ok(dir_meta) = std::fs::metadata(&combat_logs_dir) {
            if let Ok(dir_mtime) = dir_meta.modified() {
                let dir_mtime_ms = dir_mtime
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                if dir_mtime_ms < since_ms {
                    log::info!(
                        "[DropScanner] combat_logs 目录 mtime 早于进程启动时间，快速跳过 (dir_mtime_ms={}, since_ms={})",
                        dir_mtime_ms, since_ms
                    );
                    return Ok(Vec::new());
                }
            }
        }
    }

    let mut jcl_files: Vec<JclFileInfo> = Vec::new();
    let mut skipped_old = 0u32;
    let mut failed_parse = 0u32;
    let mut skipped_out_of_range = 0u32;

    let entries = std::fs::read_dir(&combat_logs_dir).map_err(|e| {
        format!("读取 combat_logs 目录失败: {} - {}", combat_logs_dir.display(), e)
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jcl") {
            continue;
        }

        // 先用文件 mtime 预过滤，避免对数千个历史文件解析文件名
        // mtime 早于 since_ms 的文件直接跳过，不调用 parse_jcl_filename
        if since_ms > 0 {
            if let Ok(meta) = entry.metadata() {
                if let Ok(mtime) = meta.modified() {
                    let mtime_ms = mtime
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0);
                    if mtime_ms < since_ms {
                        skipped_old += 1;
                        continue;
                    }
                }
            }
        }

        // 获取文件名，尝试 GBK 解码
        let file_name_os = entry.file_name();
        let file_name_bytes = file_name_os.to_string_lossy().to_string();

        // 快速日期前缀过滤：JCL 文件名格式为 YYYY-MM-DD-HH-MM-SS-...
        // 在调用完整 regex+chrono 解析前，先用文件名前10字符做日期字符串比较
        // 可跳过绝大部分范围外文件，避免 regex 匹配和 NaiveDateTime 构造
        if start_ms > 0 || end_ms > 0 {
            if let Some(date_str) = extract_date_prefix(&file_name_bytes) {
                // date_str 格式为 YYYY-MM-DD，可直接与边界日期字符串比较
                // start_ms 对应的日期字符串（本地时区）
                if start_ms > 0 {
                    let start_date = timestamp_to_local_date_string(start_ms);
                    if date_str.as_str() < start_date.as_str() {
                        skipped_out_of_range += 1;
                        continue;
                    }
                }
                if end_ms > 0 {
                    let end_date = timestamp_to_local_date_string(end_ms);
                    // end_ms 是开区间，日期等于 end_date 的需要保留（交给完整解析判断时间）
                    if date_str.as_str() > end_date.as_str() {
                        skipped_out_of_range += 1;
                        continue;
                    }
                }
            }
        }

        // 尝试解析文件名（Windows 上 file_name 应该已经是 UTF-8/WTF-8）
        // 如果解析失败，可能是 GBK 编码问题，尝试从原始字节解码
        let parsed = parse_jcl_filename(&file_name_bytes).or_else(|| {
            // 尝试从 OsStr 的字节进行 GBK 解码
            // 注意：在 Windows 上，OsStr 内部使用 WTF-8，非 ASCII 字符可能被错误编码
            // 这里我们尝试直接使用 to_string_lossy 的结果
            None
        });

        match parsed {
            Some(info) => {
                // 按副本时间（来自文件名）做范围过滤
                // start_ms/end_ms 都基于副本开始时间，非文件 mtime
                if start_ms > 0 && info.timestamp < start_ms {
                    skipped_out_of_range += 1;
                    continue;
                }
                if end_ms > 0 && info.timestamp >= end_ms {
                    skipped_out_of_range += 1;
                    continue;
                }
                jcl_files.push(info);
            }
            None => {
                failed_parse += 1;
                log::warn!(
                    "[DropScanner] 无法解析 JCL 文件名: {}",
                    file_name_bytes
                );
            }
        }
    }

    // 按时间戳排序
    jcl_files.sort_by_key(|f| f.timestamp);

    log::info!(
        "[DropScanner] JCL 扫描完成: 本次会话 {} 个, 跳过历史 {} 个, 跳过范围外 {} 个, 解析失败 {} 个",
        jcl_files.len(),
        skipped_old,
        skipped_out_of_range,
        failed_parse
    );

    Ok(jcl_files)
}

/// 查找指定账号在 `after_ms` 时间戳之后的第一个 JCL 文件时间戳
///
/// 用于历史记录复核：判断某副本实例之后是否产生了新 JCL（副本切换信号）。
/// 仅遍历目录并解析文件名，不读取文件内容。
///
/// - `account_dir`：茗伊账号目录（如 `.../{uid}@zhcn_hd`）
/// - `after_ms`：JCL 时间戳下限（毫秒），查找 timestamp > after_ms 的最小时间戳
///
/// 返回 `Ok(None)` 表示无后续 JCL 或目录不存在。
fn find_next_jcl_time_after(account_dir: &Path, after_ms: i64) -> Result<Option<i64>, String> {
    let combat_logs_dir = account_dir.join("userdata").join("combat_logs");
    if !combat_logs_dir.exists() {
        return Ok(None);
    }

    let mut next: Option<i64> = None;
    let entries = std::fs::read_dir(&combat_logs_dir).map_err(|e| {
        format!("读取 combat_logs 目录失败: {} - {}", combat_logs_dir.display(), e)
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("jcl") {
            continue;
        }
        let file_name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if let Some(jcl) = parse_jcl_filename(file_name) {
            if jcl.timestamp > after_ms {
                next = Some(next.map_or(jcl.timestamp, |n| n.min(jcl.timestamp)));
            }
        }
    }
    Ok(next)
}

/// 解析副本实例的"下一条 JCL"时间戳（副本切换信号）
///
/// 取值优先级：
/// 1. 本次扫描区间内的 JCL 时间戳（严格晚于 `last_jcl_time`）。批量路径的区间覆盖整个
///    CD 窗口，可以完整观察到后续副本产生的 JCL，所以优先采用扫描结果；
/// 2. 区间内找不到时，回退到调用方已知的 `known_next_jcl_time`。复核路径的扫描上界
///    `scan_end_ms` 就是它，而 `scan_jcl_files` 的上界是开区间
///    （`timestamp >= end_ms` 即跳过），该 JCL 必然不出现在扫描结果中，必须由调用方提供；
///    否则"副本已切换"信号会在被消费前被自己截断（条件 6 恒不成立、副本一直停在 scanning，
///    同时 chatlog 窗口回退到 now_ms 而吞掉后一副本的拍卖/结算事件）。
///
/// 两种来源都要求 `> last_jcl_time`，保证该信号只作用于时间上确实位于其之前的实例，
/// 避免误判同一次扫描产出的其他实例。
///
/// `scanned_timestamps` 需按升序排列（调用处已排序），这样返回的是最小的可用切换点。
fn resolve_next_jcl_time(
    scanned_timestamps: &[i64],
    last_jcl_time: i64,
    known_next_jcl_time: Option<i64>,
) -> Option<i64> {
    scanned_timestamps
        .iter()
        .find(|&&t| t > last_jcl_time)
        .copied()
        .or_else(|| known_next_jcl_time.filter(|&t| t > last_jcl_time))
}

/// 将 JCL 文件列表聚类为副本实例
///
/// 聚类规则（基于 JCL 内容分析的方案）：
/// 1. 按时间戳升序排序所有 JCL
/// 2. 按 JCL 顺序遍历，连续相同 raid_display_name 的 JCL 归为同一组
///    （raid_display_name 变化即开始新组，不再使用30分钟间隔聚类）
/// 3. 对每组使用 JclAnalysis.is_kill 筛选已确认击杀的 JCL
/// 4. 构建 RaidInstance（只含通关 JCL）
/// 副本会话时间间隔阈值（毫秒）
///
/// 当两个相邻 JCL 的时间间隔超过此阈值时，即使副本名相同也视为不同的副本会话。
/// 一次副本会话通常 30~90 分钟，相邻 JCL 间隔通常 < 30 分钟（含拉托重打）。
/// 2 小时阈值可稳定区分同副本的多次独立会话（含跨天、跨周场景）。
const RAID_SESSION_GAP_MS: i64 = 2 * 60 * 60 * 1000;

/// 检查指定时间范围内 chatlog 是否有"每人底薪"记录
/// 用于 2 小时阈值分割决策和中间工资检查
#[allow(dead_code)]
fn has_salary_between(
    chatlog_files: &[PathBuf],
    start_ms: i64,
    end_ms: i64,
    chatlog_range_cache: &HashMap<PathBuf, Option<(i64, i64)>>,
) -> bool {
    let start_sec = start_ms / 1000;
    let end_sec = end_ms / 1000;
    for path in chatlog_files {
        if !chatlog_file_covers_range_cached(path, start_ms, end_ms, chatlog_range_cache) {
            continue;
        }
        if let Ok(conn) = Connection::open(path) {
            let has = conn.query_row(
                "SELECT COUNT(*) FROM ChatLog WHERE time >= ?1 AND time <= ?2 AND (type = 'MSG_ROOM' OR type = 'MSG_WHISPER') AND text LIKE '%每人底薪%'",
                params![start_sec, end_sec],
                |row| row.get::<_, i64>(0),
            ).unwrap_or(0) > 0;
            if has { return true; }
        }
    }
    false
}

/// 在预加载的 chatlog 记录中检查指定时间范围是否有"每人底薪"记录
/// 用于 cluster_raid_instances，避免重复打开 SQLite 连接
fn has_salary_in_records(records: &[ChatlogRecord], start_ms: i64, end_ms: i64) -> bool {
    let start_sec = start_ms / 1000;
    let end_sec = end_ms / 1000;
    // 二分查找时间范围起点
    let start_idx = records.partition_point(|r| r.time_sec < start_sec);
    records[start_idx..]
        .iter()
        .take_while(|r| r.time_sec <= end_sec)
        .any(|r| {
            (r.msg_type == "MSG_ROOM" || r.msg_type == "MSG_WHISPER")
                && r.text.contains("每人底薪")
        })
}

fn cluster_raid_instances(
    account_id: &str,
    jcl_files: Vec<JclFileInfo>,
    jcl_analyses: &HashMap<String, JclAnalysis>,
    preloaded_records: &[ChatlogRecord],
) -> Vec<RaidInstance> {
    // 1. 按时间戳排序
    let mut sorted_files = jcl_files;
    sorted_files.sort_by_key(|f| f.timestamp);

    let mut instances: Vec<RaidInstance> = Vec::new();
    let mut current_group: Vec<JclFileInfo> = Vec::new();
    let mut current_raid_name: Option<String> = None;

    for jcl in sorted_files {
        // 判定是否需要开始新组：
        // 1) 副本名变化，或
        // 2) 与当前组最后一个 JCL 的时间间隔超过会话阈值（防止跨天/跨周的相同副本被合并）
        let name_changed = current_raid_name.as_deref() != Some(&jcl.raid_display_name);
        let time_gap_exceeded = current_group
            .last()
            .map(|last: &JclFileInfo| jcl.timestamp - last.timestamp > RAID_SESSION_GAP_MS)
            .unwrap_or(false);

        // 2小时阈值智能分组：
        // - 副本名变化 → 必定分割
        // - 时间间隔 > 2h 且前组有 boss 击杀 → 分割（无论是否有工资）
        // - 时间间隔 > 2h 但前组无击杀 → 不分割（避免对无已确认击杀的组做无谓分割）
        // - 时间间隔 <= 2h 但中间有工资记录 → 强制分割（发工资说明上个副本已结算）
        let should_split = if name_changed {
            true
        } else if time_gap_exceeded {
            // 2小时阈值增强：前一组必须有 boss 击杀才分割
            let prev_has_kill = current_group.iter().any(|jcl| {
                jcl_analyses.get(&jcl.file_name)
                    .map(|a| a.is_kill)
                    .unwrap_or(false)
            });
            if !prev_has_kill {
                log::info!(
                    "[DropScanner] 副本 {} 时间间隔 > {}h 但前组无击杀，不分割",
                    jcl.raid_display_name,
                    RAID_SESSION_GAP_MS / 3600000,
                );
                false
            } else {
                // 检查中间是否发过工资
                let gap_start = current_group.last().unwrap().timestamp;
                let gap_end = jcl.timestamp;
                let has_salary = has_salary_in_records(preloaded_records, gap_start, gap_end);
                if has_salary {
                    log::info!(
                        "[DropScanner] 副本 {} 时间间隔 > {}h 且中间有工资记录，分割",
                        jcl.raid_display_name,
                        RAID_SESSION_GAP_MS / 3600000,
                    );
                }
                true // 前组有击杀就分割（无论是否有工资，超过2h都应分割）
            }
        } else {
            // 时间间隔未超过2小时，但检查中间是否发过工资
            if !current_group.is_empty() {
                let gap_start = current_group.last().unwrap().timestamp;
                let gap_end = jcl.timestamp;
                // 只有前组有击杀时才做工资检查（避免对无已确认击杀的组做无谓查询）
                let prev_has_kill = current_group.iter().any(|jcl| {
                    jcl_analyses.get(&jcl.file_name)
                        .map(|a| a.is_kill)
                        .unwrap_or(false)
                });
                if prev_has_kill && has_salary_in_records(preloaded_records, gap_start, gap_end) {
                    log::info!(
                        "[DropScanner] 副本 {} 中间有工资记录，强制分割为两个副本",
                        jcl.raid_display_name,
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };

        if should_split {
            if !current_group.is_empty() {
                if let Some(instance) = build_raid_instance(account_id, &current_group, jcl_analyses) {
                    instances.push(instance);
                }
                current_group.clear();
            }
            current_raid_name = Some(jcl.raid_display_name.clone());
        }
        current_group.push(jcl);
    }
    // 处理最后一组
    if !current_group.is_empty() {
        if let Some(instance) = build_raid_instance(account_id, &current_group, jcl_analyses) {
            instances.push(instance);
        }
    }

    // 按开始时间排序
    instances.sort_by_key(|i| i.start_time);
    instances
}

/// 获取 JCL 的有效 BOSS 名（优先使用 JclAnalysis 的内容搜索结果，回退到文件名 BOSS）
fn get_effective_boss_name(jcl: &JclFileInfo, jcl_analyses: &HashMap<String, JclAnalysis>) -> String {
    jcl_analyses
        .get(&jcl.file_name)
        .and_then(|a| a.boss_name.clone())
        .unwrap_or_else(|| jcl.boss_name.clone())
}

/// 从一组同副本名的 JCL 构建一个 RaidInstance
///
/// 通关判定只使用专属宝箱，或同一运行时实体的参战加终局死亡（type=28/HP=0）证据。
/// 每个连续相同 BOSS 的子组只取最后一份已确认击杀的 JCL，不根据名称差异或文件顺序补记击杀。
/// 若整组没有可计入的击杀（包括证据不足），返回 None 跳过。
fn build_raid_instance(
    account_id: &str,
    group: &[JclFileInfo],
    jcl_analyses: &HashMap<String, JclAnalysis>,
) -> Option<RaidInstance> {
    // 1. 按连续相同 BOSS 名分组
    let mut boss_groups: Vec<Vec<&JclFileInfo>> = Vec::new();
    let mut current_boss_name: Option<String> = None;
    for jcl in group {
        let boss_name = get_effective_boss_name(jcl, jcl_analyses);
        if current_boss_name.as_deref() != Some(&boss_name) {
            boss_groups.push(Vec::new());
            current_boss_name = Some(boss_name);
        }
        boss_groups.last_mut().unwrap().push(jcl);
    }

    // 2. 对每个 BOSS 子组，确定通关 JCL
    //    每个 BOSS 子组只取 1 个击杀 JCL（避免同一 BOSS 多次拉托+击杀导致重复计数）
    let mut success_jcls: Vec<&JclFileInfo> = Vec::new();
    for boss_group in &boss_groups {
        // 先尝试 JCL 内部信号（is_kill=true），取最后一个明确击杀的 JCL
        // 一个 BOSS 子组内可能有多次尝试或重复日志，按最后一份已确认击杀的记录去重。
        let last_definite_kill: Option<&&JclFileInfo> = boss_group
            .iter()
            .rev()
            .find(|jcl| {
                jcl_analyses
                    .get(&jcl.file_name)
                    .map(|a| a.is_kill)
                    .unwrap_or(false)
            });

        if let Some(kill_jcl) = last_definite_kill {
            // 有明确击杀信号，只取这一个 JCL（不 extend 所有 is_kill=true）
            success_jcls.push(*kill_jcl);
        }
    }

    if success_jcls.is_empty() {
        // 整组没有可计入的击杀，跳过；不能据此断言所有战斗都是拉脱。
        log::debug!(
            "[DropScanner] 副本 {} 的一个组没有可计入的击杀，跳过（共{}个JCL）",
            group.first().map(|j| j.raid_display_name.as_str()).unwrap_or(""),
            group.len()
        );
        return None;
    }

    let raid_display_name = success_jcls[0].raid_display_name.clone();
    // map_id 取自 JCL 文件名的副本 ID（与 JX3Box 掉落接口同源）
    let map_id = success_jcls[0].raid_id;
    // 使用组内第一个 JCL 的时间戳作为稳定的去重键，避免进行中副本因新 JCL 产生导致 start_time 变化
    let start_time = group.first().map(|j| j.timestamp).unwrap_or(success_jcls[0].timestamp);
    // last_jcl_time 取整组最后一个 JCL（含拉托），用于确定聊天记录结束范围
    let last_jcl_time = group.last().map(|j| j.timestamp).unwrap_or(0);

    // end_time = 最后一个通关 JCL 的 fight_end_ms（来自 FIGHT_TIME bFighting=false）
    let end_time = success_jcls
        .last()
        .and_then(|jcl| jcl_analyses.get(&jcl.file_name).map(|a| a.fight_end_ms))
        .filter(|t| *t > 0)
        .unwrap_or(last_jcl_time);

    // first_gold_time = 首个通关 JCL 的 fight_start_ms（来自 FIGHT_TIME bFighting=true）
    let first_gold_time = success_jcls
        .first()
        .and_then(|jcl| jcl_analyses.get(&jcl.file_name).map(|a| a.fight_start_ms))
        .filter(|t| *t > 0)
        .unwrap_or(start_time);

    // 收集 jcl_files, jcl_boss_names, bosses_killed
    // boss_name 优先使用 JclAnalysis 的内容搜索结果，回退到文件名 BOSS
    let mut jcl_files: Vec<String> = Vec::new();
    let mut jcl_boss_names: Vec<String> = Vec::new();
    for jcl in &success_jcls {
        let boss_name = jcl_analyses
            .get(&jcl.file_name)
            .and_then(|a| a.boss_name.clone())
            .unwrap_or_else(|| jcl.boss_name.clone());
        jcl_files.push(jcl.file_name.clone());
        jcl_boss_names.push(boss_name);
    }
    let bosses_killed = jcl_boss_names.clone();
    let boss_kill_count = success_jcls.len() as u32;

    log::info!(
        "[DropScanner] 副本 {} 聚类完成: 组内 {} 个JCL, 通关 {} 个, start={}, end={}, first_gold={}",
        raid_display_name,
        group.len(),
        boss_kill_count,
        start_time,
        end_time,
        first_gold_time
    );

    Some(RaidInstance {
        account_id: account_id.to_string(),
        raid_display_name,
        map_id,
        start_time,
        end_time,
        last_jcl_time,
        bosses_killed,
        jcl_files,
        jcl_boss_names,
        boss_kill_count,
        first_gold_time,
    })
}

/// 副本信息（从 raids 表加载）
#[derive(Clone)]
pub struct RaidEntry {
    /// raids.name（如 "阆风悬城"）
    pub name: String,
    /// raids.id（格式：{playerCount}人{difficulty}{name}，如 "25人普通阆风悬城"）
    pub raid_id: String,
    /// BOSS 列表（可能为空）
    pub bosses: Vec<(String, String)>,
}

/// 副本配置缓存（应用生命周期内有效，副本配置不会频繁变化）
static RAIDS_CACHE: OnceLock<Mutex<Option<Vec<RaidEntry>>>> = OnceLock::new();

fn get_cached_raids(conn: &Connection) -> Result<Vec<RaidEntry>, String> {
    let cache = RAIDS_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap();
    if let Some(raids) = guard.as_ref() {
        return Ok(raids.clone());
    }
    let raids = load_raids_with_bosses(conn)?;
    *guard = Some(raids.clone());
    Ok(raids)
}

/// 清空副本配置缓存（副本配置变更时调用）
/// 同时清空 JCL 解析缓存，因为 BOSS 配置变化会影响 JCL 分析结果
pub fn invalidate_raids_cache() {
    if let Some(cache) = RAIDS_CACHE.get() {
        let mut guard = cache.lock().unwrap();
        *guard = None;
    }
    // 清空 JCL 解析缓存表，避免副本配置变更后使用旧的 BOSS 分析结果
    if let Ok(conn) = crate::db::init_db() {
        if let Err(e) = conn.execute("DELETE FROM jcl_cache", []) {
            log::warn!("[DropScanner] 清空 JCL 缓存失败: {}", e);
        } else {
            log::info!("[DropScanner] 已清空 JCL 解析缓存（副本配置变更）");
        }
    }
}

/// 从 raids 表和 raid_bosses 表加载副本信息
///
/// raids 表存储所有副本，raid_bosses 表存储配置了 BOSS 追踪的副本的 BOSS 信息。
/// 即使没有配置 BOSS 追踪，副本也应该能被匹配（bosses 为空）。
fn load_raids_with_bosses(conn: &Connection) -> Result<Vec<RaidEntry>, String> {
    // 1. 从 raid_bosses 表加载 BOSS 信息
    // 新表结构：raid_name 列存 raids.name（副本基础名），同名副本天然共享同一组 BOSS 记录。
    // 因此 bosses_map 的 key 用 raids.name。
    let mut bosses_map: HashMap<String, Vec<(String, String)>> = HashMap::new();

    let mut stmt = conn
        .prepare("SELECT raid_name, boss_id, name FROM raid_bosses ORDER BY boss_order")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // raid_name（raids.name，如 "阆风悬城"）
                row.get::<_, String>(1)?, // boss_id
                row.get::<_, String>(2)?, // boss_name
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (raid_name, boss_id, boss_name) = row.map_err(|e| e.to_string())?;
        bosses_map
            .entry(raid_name)
            .or_insert_with(Vec::new)
            .push((boss_id, boss_name));
    }

    // 2. 从 raids 表加载所有副本
    let mut entries: Vec<RaidEntry> = Vec::new();

    let mut stmt = conn
        .prepare("SELECT id, name FROM raids")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // id（如 "25人普通阆风悬城"）
                row.get::<_, String>(1)?, // name（如 "阆风悬城"）
            ))
        })
        .map_err(|e| e.to_string())?;

    for row in rows {
        let (raid_id, name) = row.map_err(|e| e.to_string())?;

        // 用 raids.name 作为 key 查找 BOSS 配置（同名副本共享）
        let bosses = bosses_map.get(&name).cloned().unwrap_or_default();
        entries.push(RaidEntry {
            name: name.clone(),
            raid_id,
            bosses,
        });
    }

    // 3. Fallback：如果某个 raid 仍无 BOSS 配置（数据库未配置），
    // 尝试从同 name 的其他 raid 复制 BOSS 列表。
    // 新表结构下同名副本已天然共享，此分支通常不会触发，仅作兜底。
    let mut fallback_count = 0u32;
    for i in 0..entries.len() {
        if !entries[i].bosses.is_empty() {
            continue;
        }
        let name = entries[i].name.clone();
        let raid_id = entries[i].raid_id.clone();
        if let Some(fallback) = entries
            .iter()
            .find(|e| e.name == name && !e.bosses.is_empty() && e.raid_id != raid_id)
            .map(|e| e.bosses.clone())
        {
            log::info!(
                "[DropScanner] BOSS 配置 fallback: raid_id='{}' 使用同名 raid 的 BOSS 配置 ({} 个 BOSS)",
                entries[i].raid_id,
                fallback.len()
            );
            entries[i].bosses = fallback;
            fallback_count += 1;
        }
    }
    if fallback_count > 0 {
        log::info!(
            "[DropScanner] BOSS 配置 fallback 完成: 共 {} 个 raid 使用了同名 raid 的 BOSS 配置",
            fallback_count
        );
    }

    Ok(entries)
}

/// 从 raid_id 中解析人数
/// raid_id 格式：{playerCount}人{difficulty}{name}，如 "25人普通阆风悬城"
fn parse_player_count_from_raid_id(raid_id: &str) -> i32 {
    if let Some(pos) = raid_id.find('人') {
        if let Ok(n) = raid_id[..pos].parse::<i32>() {
            return n;
        }
    }
    25 // 默认25人
}

/// 难度优先级：普通(1) < 英雄(2) < 挑战(3)
fn difficulty_rank(raid_id: &str) -> u8 {
    if raid_id.contains("挑战") {
        3
    } else if raid_id.contains("英雄") {
        2
    } else {
        1 // 普通或其他
    }
}

/// 从 JCL 副本名中解析人数前缀
/// 返回 None 表示无人数前缀（如 "阆风悬城"）
/// 返回 Some(n) 表示有人数前缀（如 "25人普通阆风悬城" → Some(25)）
fn parse_jcl_player_count(jcl_raid_name: &str) -> Option<i32> {
    let mut chars = jcl_raid_name.chars();
    let mut num_str = String::new();
    while let Some(c) = chars.next() {
        if c.is_ascii_digit() {
            num_str.push(c);
        } else if c == '人' && !num_str.is_empty() {
            return num_str.parse().ok();
        } else {
            break;
        }
    }
    None
}

/// 从 JCL 副本名中检测难度前缀
/// 返回 None 表示无难度前缀
fn detect_jcl_difficulty(jcl_raid_name: &str) -> Option<u8> {
    if jcl_raid_name.contains("挑战") {
        Some(3)
    } else if jcl_raid_name.contains("英雄") {
        Some(2)
    } else if jcl_raid_name.contains("普通") {
        Some(1)
    } else {
        None
    }
}

/// 匹配 JCL 副本名到 raids 表的副本配置
///
/// JCL 副本名格式：
/// - 完整格式：{人数}人{难度}{副本名}，如 "25人普通阆风悬城"
/// - 无人数前缀：{难度}{副本名} 或 {副本名}，如 "阆风悬城"
///
/// 匹配策略：
/// 1. 精确匹配 raids.id
/// 2. 收集所有后缀匹配的候选（raids.id 双向后缀 + raids.name 后缀）
/// 3. 单候选直接返回
/// 4. 多候选时，根据 JCL 是否包含人数/难度前缀选择最佳匹配：
///    - JCL 无人数前缀 → 选择最低人数的候选
///    - JCL 无难度前缀 → 选择最低难度的候选
///    - JCL 有人数前缀 → 优先匹配相同人数
///    - JCL 有难度前缀 → 优先匹配相同难度
fn match_raid_name<'a>(jcl_raid_name: &str, raids: &'a [RaidEntry]) -> Option<&'a RaidEntry> {
    // 1. 精确匹配 raids.id
    for entry in raids {
        if entry.raid_id == jcl_raid_name {
            return Some(entry);
        }
    }

    // 2. 收集所有后缀匹配的候选
    //    raids.id 双向后缀匹配：处理 JCL 名是 raid_id 的子串或超串的情况
    //    raids.name 后缀匹配：处理 JCL 名包含完整副本基础名的情况
    let mut candidates: Vec<&RaidEntry> = raids
        .iter()
        .filter(|e| {
            jcl_raid_name.ends_with(&e.raid_id)
                || e.raid_id.ends_with(jcl_raid_name)
                || (!e.name.is_empty() && jcl_raid_name.ends_with(&e.name))
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // 3. 单候选直接返回
    if candidates.len() == 1 {
        return Some(candidates[0]);
    }

    // 4. 多候选时，根据 JCL 前缀信息选择最佳匹配
    //    - JCL 有人数前缀 → 人数为主排序键，难度为次排序键
    //    - JCL 无人数前缀 → 难度为主排序键（若有），人数为次排序键
    let jcl_player_count = parse_jcl_player_count(jcl_raid_name);
    let jcl_difficulty = detect_jcl_difficulty(jcl_raid_name);

    candidates.sort_by(|a, b| {
        let a_pc = parse_player_count_from_raid_id(&a.raid_id);
        let b_pc = parse_player_count_from_raid_id(&b.raid_id);
        let a_diff = difficulty_rank(&a.raid_id);
        let b_diff = difficulty_rank(&b.raid_id);

        // 人数比较：有人数前缀时按差值升序，无前缀时按人数升序
        let pc_cmp = match jcl_player_count {
            Some(jcl_pc) => {
                (a_pc - jcl_pc).abs().cmp(&(b_pc - jcl_pc).abs())
            }
            None => a_pc.cmp(&b_pc),
        };

        // 难度比较：有难度前缀时按差值升序，无前缀时按难度升序
        let diff_cmp = match jcl_difficulty {
            Some(jcl_diff) => {
                let a_dist = (a_diff as i32 - jcl_diff as i32).abs();
                let b_dist = (b_diff as i32 - jcl_diff as i32).abs();
                a_dist.cmp(&b_dist)
            }
            None => a_diff.cmp(&b_diff),
        };

        // 有人数前缀时人数优先，否则难度优先
        if jcl_player_count.is_some() {
            pc_cmp.then(diff_cmp)
        } else {
            diff_cmp.then(pc_cmp)
        }
    });

    log::debug!(
        "[DropScanner] match_raid_name 多候选匹配: jcl='{}', jcl_pc={:?}, jcl_diff={:?}, candidates={:?}, selected='{}'",
        jcl_raid_name,
        jcl_player_count,
        jcl_difficulty,
        candidates.iter().map(|c| c.raid_id.as_str()).collect::<Vec<_>>(),
        candidates.first().map(|c| c.raid_id.as_str()).unwrap_or("")
    );

    candidates.first().copied()
}

/// 判断 JCL BOSS 名是否为有效 BOSS（精确/模糊匹配 raid_bosses 表）
///
/// 当 raid_bosses 为空（副本未配置 BOSS 追踪）时返回 true，含义是"不做过滤"。
/// 这样无配置副本的 JCL 也会走 analyze_jcl 路径 A（文件名 BOSS 即真实 BOSS），
/// 通过同一运行时实体的参战加终局死亡或专属宝箱判定击杀，从而能正常扫描记录。
fn is_valid_boss(boss_name: &str, raid_bosses: &[(String, String)]) -> bool {
    // 无 BOSS 配置时，不做过滤（所有 BOSS 名都视为有效）
    if raid_bosses.is_empty() {
        return true;
    }
    // 精确匹配
    if raid_bosses.iter().any(|(_, name)| name == boss_name) {
        return true;
    }
    // 模糊匹配
    if raid_bosses.iter().any(|(_, name)| {
        boss_name.contains(name.as_str()) || name.contains(boss_name)
    }) {
        return true;
    }
    false
}

/// 宝箱名称兼容中点和英文点号；保留其他字符，避免把不同首领合并。
fn normalize_treasure_boss_name(name: &str) -> String {
    name.chars().filter(|c| !matches!(c, '·' | '.')).collect()
}

/// 先直接匹配完整首领名，再尝试唯一的去点匹配。
/// 例如“伊曼寂夜”可匹配“伊曼·寂夜”；去点后存在多个候选时不猜测归属。
fn match_treasure_box_boss<'a>(
    chest_boss_name: &str,
    raid_bosses: &'a [(String, String)],
) -> Option<&'a str> {
    if chest_boss_name.is_empty() {
        return None;
    }
    if let Some((_, name)) = raid_bosses.iter().find(|(_, name)| name == chest_boss_name) {
        return Some(name.as_str());
    }

    let normalized = normalize_treasure_boss_name(chest_boss_name);
    if normalized.is_empty() {
        return None;
    }
    let mut candidates = raid_bosses
        .iter()
        .filter(|(_, name)| normalize_treasure_boss_name(name) == normalized);
    let (_, name) = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(name.as_str())
}

/// 将 BOSS 名列表映射为 BOSS ID 列表
fn map_boss_names_to_ids(
    bosses_killed: &[String],
    raid_bosses: &[(String, String)],
) -> (Vec<String>, Vec<String>) {
    let mut boss_ids: Vec<String> = Vec::new();
    let mut boss_names: Vec<String> = Vec::new();
    // 去重集合：同一 boss_id 只保留首次出现，避免同一 BOSS 多次拉托+击杀导致重复计数
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 无 BOSS 配置时，不显示 BOSS 列表（bossIds/bossNames 均为空）
    if raid_bosses.is_empty() {
        return (boss_ids, boss_names);
    }

    for boss_name in bosses_killed {
        // 精确匹配
        let found = raid_bosses.iter().find(|(_, name)| name == boss_name);
        if let Some((id, name)) = found {
            if seen_ids.insert(id.clone()) {
                boss_ids.push(id.clone());
                boss_names.push(name.clone());
            }
            continue;
        }

        // 模糊匹配：JCL BOSS 名包含 raid_bosses 中的名（或反过来）
        // 例如 JCL="阿史那承庆"，raid_bosses.name="阿史那承庆"（配置已与 JCL 对齐）
        // 显示名优先使用 JCL 中的完整名
        let fuzzy = raid_bosses.iter().find(|(_, name)| {
            boss_name.contains(name.as_str()) || name.contains(boss_name.as_str())
        });
        if let Some((id, _)) = fuzzy {
            if seen_ids.insert(id.clone()) {
                boss_ids.push(id.clone());
                boss_names.push(boss_name.clone());
            }
            continue;
        }

        // 未匹配（小怪或非追踪 BOSS），跳过不加入列表
        log::debug!("[DropScanner] BOSS 名未在配置中，跳过: {}", boss_name);
    }

    (boss_ids, boss_names)
}

/// 分析 JCL 文件内容，提取战斗时间、BOSS 击杀状态和真实 BOSS 名
///
/// JCL 是 GBK 编码的 TAB 分隔文本（CRLF 换行），每行格式：
/// `CRC校验码\t游戏逻辑帧\tUnix时间戳\t客户端毫秒数\t事件类型\t{LUA_DATA}`
///
/// 本函数解析以下事件类型：
/// - FIGHT_TIME (type=1)：`{ bFighting, szUUID, nDuring }`
///   - bFighting=true 标记战斗开始，bFighting=false 标记战斗结束
///   - 用于提取 fight_start_ms / fight_end_ms（比文件名时间更精确）
/// - NPC_INFO (type=8)：`{ dwID, szName, dwTemplateID, ... }`
///   - 通过 dwTemplateID 匹配文件名中的 boss_id，找到 BOSS 的运行时 dwID
/// - NPC_FIGHT_HINT (type=9)：`{ dwID, bFight, fCurrentLife, ... }`
///   - 仅用 `bFight=true` 确认该运行时实体实际参战
///   - 击杀还必须由该实体的 type=28 或 HP=0 终局死亡确认
///
/// 同时支持双 BOSS 模型（路径 B）：
/// 当文件名 BOSS 名不在 raid_bosses 配置中时（小怪 JCL，如"须罗巨傀"对应唐怀仁战斗），
/// 从 NPC_INFO 事件中匹配配置中的真实 BOSS 名，并要求真实 BOSS 进入战斗（bFight=true）
/// 才认为该 JCL 包含该 BOSS 的战斗。不以对话文本/NPC_INFO 文本作为依据，
/// 避免出场对话 JCL 被误判为该 BOSS 的战斗。
///
/// 性能优化：
/// - 逐行读取（BufReader），避免一次性读入大文件（45MB+）
/// - 字节级事件类型预过滤，跳过不相关的事件类型
/// - GBK 解码仅对相关事件行执行
///
/// 参数：
/// - jcl_path: JCL 文件完整路径
/// - boss_template_id: BOSS 模板 ID（来自 JCL 文件名，如 137005）
/// - raid_bosses: raid_bosses 表的 (boss_id, boss_name) 列表
///
/// 所有副本共用解析版本，确保曾将服务器文本作为击杀依据的旧结果不会命中。
const JCL_ANALYSIS_CACHE_VERSION: &str = "jcl-analysis-v8|";

fn jcl_cache_key(jcl_path: &Path) -> String {
    format!("{}{}", JCL_ANALYSIS_CACHE_VERSION, jcl_path.to_string_lossy())
}

/// 从 SQLite 缓存读取 JCL 解析结果（跨会话复用）。
/// 所有副本使用版本化 path + mtime，旧缓存无需迁移或删除。
fn get_jcl_cache(conn: &Connection, file_path: &str, file_mtime: i64) -> Option<JclAnalysis> {
    let result: rusqlite::Result<(Option<String>, i64, i64, i64)> = conn.query_row(
        "SELECT boss_name, fight_start_ms, fight_end_ms, is_kill FROM jcl_cache WHERE file_path = ?1 AND file_mtime = ?2",
        params![file_path, file_mtime],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    );

    match result {
        Ok((boss_name, fight_start_ms, fight_end_ms, is_kill)) => {
            log::debug!("[DropScanner] JCL 缓存命中: {}", file_path);
            Some(JclAnalysis {
                boss_name,
                fight_start_ms,
                fight_end_ms,
                is_kill: is_kill != 0,
            })
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => None,
        Err(e) => {
            log::warn!("[DropScanner] JCL 缓存读取失败: {}", e);
            None
        }
    }
}

/// 将 JCL 解析结果写入 SQLite 缓存
fn set_jcl_cache(conn: &Connection, file_path: &str, file_mtime: i64, analysis: &JclAnalysis) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    if let Err(e) = conn.execute(
        "INSERT OR REPLACE INTO jcl_cache (file_path, file_mtime, boss_name, fight_start_ms, fight_end_ms, is_kill, cached_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            file_path,
            file_mtime,
            analysis.boss_name,
            analysis.fight_start_ms,
            analysis.fight_end_ms,
            if analysis.is_kill { 1 } else { 0 },
            now
        ],
    ) {
        log::warn!("[DropScanner] JCL 缓存写入失败: {}", e);
    }
}

/// 获取文件的修改时间（Unix 毫秒）
fn get_file_mtime(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|t| {
            t.duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// 带缓存的 JCL 分析：优先查 SQLite 缓存，未命中则解析文件并写回缓存
fn analyze_jcl_cached(
    conn: &Connection,
    jcl_path: &Path,
    jcl_boss_name: &str,
    boss_template_id: i64,
    raid_bosses: &[(String, String)],
) -> Option<JclAnalysis> {
    let cache_key = jcl_cache_key(jcl_path);
    let file_mtime = get_file_mtime(jcl_path);

    // 1. 查缓存
    if file_mtime > 0 {
        if let Some(cached) = get_jcl_cache(conn, &cache_key, file_mtime) {
            return Some(cached);
        }
    }

    // 2. 缓存未命中，解析文件
    let analysis = analyze_jcl(jcl_path, jcl_boss_name, boss_template_id, raid_bosses)?;

    // 3. 写回缓存
    if file_mtime > 0 {
        set_jcl_cache(conn, &cache_key, file_mtime, &analysis);
    }

    Some(analysis)
}

/// 解析 JCL 文件，提取 BOSS 击杀状态和战斗时间
fn analyze_jcl(
    jcl_path: &Path,
    jcl_boss_name: &str,
    boss_template_id: i64,
    raid_bosses: &[(String, String)],
) -> Option<JclAnalysis> {
    use std::io::BufRead;
    let file = std::fs::File::open(jcl_path).ok()?;
    let reader = std::io::BufReader::new(file);
    let gbk = encoding_rs::GBK;

    // 预编码所有 BOSS 名为 GBK 字节串，避免在循环中重复编码
    let needles: Vec<(&str, Vec<u8>)> = raid_bosses
        .iter()
        .filter(|(_, name)| !name.is_empty())
        .map(|(_, name)| {
            let (bytes, _, _) = gbk.encode(name);
            (name.as_str(), bytes.to_vec())
        })
        .collect();

    // 判断文件名 BOSS 是否在 raid_bosses 配置中（基于 BOSS 名称匹配）。
    // boss_id 是字符串格式（如 "langfengxuancheng_1"），不能用数值 template_id 比对。
    // 不在配置中说明是"小怪 JCL"（如"须罗巨傀"对应唐怀仁战斗），需要从 JCL 内容
    // 搜索配置中的真实 BOSS 名，并跟踪其状态。
    let file_boss_in_config = is_valid_boss(jcl_boss_name, raid_bosses);

    let mut fight_start_ms: i64 = 0;
    let mut fight_end_ms: i64 = 0;
    let mut boss_dwid: i64 = 0;
    let mut file_boss_evidence = RuntimeEntityEvidence::default();
    // 已发现的 BOSS 专属宝箱 NPC（"BOSS名+宝箱"），用于路径 A/B 判定
    let mut treasure_box_bosses: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 真实 BOSS（仅当文件名 BOSS 不在配置中时使用）
    // 例：唐怀仁战斗的 JCL 文件名是"须罗巨傀"（小怪），需从 NPC_INFO 中找"唐怀仁"
    // 通过"小怪路径"判定通关：
    // - 双 BOSS 死亡（须罗巨傀 + 唐怀仁 都死）
    // - 或"唐怀仁宝箱"NPC 出现
    let mut real_boss_dwid: i64 = 0;
    let mut real_boss_name: Option<String> = None;
    let mut real_boss_evidence = RuntimeEntityEvidence::default();
    // 归属映射与当前一轮的击杀证据分离：实体曾经参战即可识别归属，复位只会撤销击杀资格。
    let mut real_boss_ever_participated = false;
    // 同名实体只能否定，不能把其中一者的参战和另一者的死亡拼成击杀。
    let mut same_name_dwids: HashSet<i64> = HashSet::new();
    let mut same_name_resumed_after_file_death = false;

    for line in reader.split(b'\n') {
        let line_bytes: Vec<u8> = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        // 按 TAB 分割列：CRC\tframe\tunix_ts\tclient_ms\tevent_type\t{LUA_DATA}
        // 只需提取第3列（unix_ts）、第5列（event_type）、第6列（LUA_DATA）
        let mut parts: Vec<&[u8]> = Vec::with_capacity(6);
        let mut start = 0;
        for (i, &b) in line_bytes.iter().enumerate() {
            if b == b'\t' {
                parts.push(&line_bytes[start..i]);
                start = i + 1;
                if parts.len() >= 5 {
                    break;
                }
            }
        }
        if parts.len() >= 5 {
            parts.push(&line_bytes[start..]);
        }
        if parts.len() < 6 {
            continue;
        }

        // 解析事件类型，处理相关类型（基于茗伊 MY_CombatLogs.lua 源码）：
        // 1=FIGHT_TIME, 8=NPC_INFO, 9=NPC_FIGHT_HINT, 28=SYS_MSG_UI_OME_DEATH_NOTIFY
        let event_type: u8 = match std::str::from_utf8(parts[4])
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(t) => t,
            None => continue,
        };
        if event_type != 1 && event_type != 8 && event_type != 9 && event_type != 28 {
            continue;
        }

        // 解析 Unix 时间戳（秒）
        let timestamp_sec: i64 = match std::str::from_utf8(parts[2])
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(t) => t,
            None => continue,
        };

        // GBK 解码 LUA_DATA 部分
        let lua_str = gbk.decode(parts[5]).0;
        // 去除首尾花括号和空白
        let lua_trimmed = lua_str.trim();
        let lua_inner = lua_trimmed
            .strip_prefix('{')
            .and_then(|s| s.strip_suffix('}'))
            .unwrap_or(lua_trimmed);
        // 按逗号分割（简单分割，BOSS 名不含逗号）
        let fields: Vec<&str> = lua_inner.split(',').collect();

        match event_type {
            1 => {
                // FIGHT_TIME: { bFighting, szUUID, nDuring }
                if fields.is_empty() {
                    continue;
                }
                let bfight = fields[0].trim() == "true";
                if bfight && fight_start_ms == 0 {
                    fight_start_ms = timestamp_sec * 1000;
                }
                if !bfight {
                    fight_end_ms = timestamp_sec * 1000;
                }
            }
            8 => {
                // NPC_INFO: { dwID, szName, dwTemplateID, dwEmployer, nX, nY, nZ }
                // 用途 1：在未找到 BOSS dwID 时，通过 template_id 匹配找到 BOSS 运行时 dwID
                // 用途 2：检测"BOSS名+宝箱"NPC（击杀掉落的可靠信号，**精确匹配**配置 BOSS）
                // 用途 3：当文件名 BOSS 不在配置中时，从 NPC 名中匹配 raid_bosses 配置的 BOSS
                //        找到"真实 BOSS"dwID（如"须罗巨傀"JCL 中的"唐怀仁"）
                if fields.len() < 3 {
                    continue;
                }
                let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                let name = fields[1].trim();
                let template_id: i64 = fields[2].trim().parse().unwrap_or(0);

                // 用途 1：通过 template_id 匹配文件名 BOSS 的 dwID
                if boss_dwid == 0 && boss_template_id != 0 && template_id == boss_template_id {
                    boss_dwid = dwid;
                }

                // 用途 3：当文件名 BOSS 不在配置中时，通过 NPC 名匹配配置中的"真实 BOSS"
                if !file_boss_in_config && real_boss_dwid == 0 {
                    for (boss_name, needle) in &needles {
                        if needle.is_empty() || needle.len() > line_bytes.len() {
                            continue;
                        }
                        // NPC 名为 "XXX"（带引号），需检查 name 是否等于或包含配置 BOSS 名
                        let name_trimmed = name.trim_matches('"');
                        if name_trimmed == *boss_name {
                            real_boss_dwid = dwid;
                            real_boss_name = Some((*boss_name).to_string());
                            break;
                        }
                    }
                }

                // 用途 2：检测"BOSS名+宝箱"NPC（完整名称直接匹配或唯一去点匹配）
                // 例如：NPC 名为 "笑妆娘宝箱" 时，提取前缀 "笑妆娘"，
                //       若在 raid_bosses 配置中则记为对应 BOSS 的宝箱已出现
                let name_trimmed = name.trim_matches('"');
                if name_trimmed.ends_with("宝箱") {
                    let prefix_len = name_trimmed.len() - "宝箱".len();
                    let prefix = &name_trimmed[..prefix_len];
                    if let Some(boss_name) = match_treasure_box_boss(prefix, raid_bosses) {
                        treasure_box_bosses.insert(boss_name.to_string());
                    }
                    // 无 BOSS 配置时也支持去点后的完整名称匹配，并保留联名宝箱兼容。
                    // 例如 "罗芬和胡汤宝箱" 仍能匹配文件名 BOSS "罗芬"。
                    if raid_bosses.is_empty() && !jcl_boss_name.is_empty() {
                        let normalized_boss = normalize_treasure_boss_name(jcl_boss_name);
                        if prefix.contains(jcl_boss_name)
                            || (!normalized_boss.is_empty()
                                && normalize_treasure_boss_name(prefix) == normalized_boss)
                        {
                            treasure_box_bosses.insert(jcl_boss_name.to_string());
                        }
                    }
                }

                if file_boss_in_config && name_trimmed == jcl_boss_name {
                    same_name_dwids.insert(dwid);
                }
            }
            9 => {
                // NPC_FIGHT_HINT: { dwID, bFight, fCurrentLife, fMaxLife, nCurrentMana, nMaxMana }
                // 茗伊源码: D.InsertLog(LOG_TYPE.NPC_FIGHT_HINT, { dwID, bFight, fCurrentLife, fMaxLife, ... })
                // 跟踪 BOSS 的战斗开始/结束信号 + HP=0 击杀信号
                if fields.len() < 2 {
                    continue;
                }
                let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                let bfight = fields[1].trim() == "true";
                // fCurrentLife=0 是可靠击杀信号（BOSS 血量归零）
                let hp: f64 = if fields.len() >= 3 {
                    fields[2].trim().parse().unwrap_or(-1.0)
                } else {
                    -1.0
                };
                // 文件名 BOSS（小怪路径下是小怪 dwID）的 bFight 状态
                if boss_dwid != 0 && dwid == boss_dwid {
                    file_boss_evidence.observe_fight_hint(bfight, hp);
                } else if file_boss_evidence.terminal_death
                    && same_name_dwids.contains(&dwid)
                    && bfight
                {
                    same_name_resumed_after_file_death = true;
                }
                // 真实 BOSS（小怪路径下追踪唐怀仁等配置 BOSS）的 bFight 状态
                if !file_boss_in_config && real_boss_dwid != 0 && dwid == real_boss_dwid {
                    if bfight {
                        real_boss_ever_participated = true;
                    }
                    real_boss_evidence.observe_fight_hint(bfight, hp);
                }
            }
            28 => {
                // SYS_MSG_UI_OME_DEATH_NOTIFY: { dwCharacterID, dwKiller }
                // 茗伊源码: D.InsertLog(LOG_TYPE.SYS_MSG_UI_OME_DEATH_NOTIFY, { dwCharacterID, dwKiller })
                // 这是 JCL 中唯一的真正死亡事件，fields[0]=死亡目标ID
                if fields.is_empty() {
                    continue;
                }
                let dead_dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                if boss_dwid != 0 && dead_dwid == boss_dwid {
                    file_boss_evidence.observe_death_notify();
                }
                // 真实 BOSS 死亡（路径 B）
                if !file_boss_in_config && real_boss_dwid != 0 && dead_dwid == real_boss_dwid {
                    real_boss_evidence.observe_death_notify();
                }
            }
            _ => {}
        }
    }

    // 通关判定只使用专属宝箱，或同一运行时实体的“参战 + 终局死亡”证据。
    // SYS_MSG_UI_OME_DEATH_NOTIFY (type=28) 和 NPC_FIGHT_HINT 的 HP=0 是死亡证据；
    // 脱战、离场、朝向和成就消息都不参与判定。
    //
    // 路径 A：文件名 BOSS 已在配置中（或该副本无 BOSS 配置）。
    // 路径 B：文件名是小怪时，文件实体和配置首领必须分别满足相同的强证据；
    // 若两者已解析为同一 dwID，则按同一实体处理，避免把同名但不同实体拼接为击杀。
    let is_kill = if file_boss_in_config {
        let treasure_ok = if raid_bosses.is_empty() {
            !treasure_box_bosses.is_empty()
        } else {
            treasure_box_bosses.contains(jcl_boss_name)
        };
        treasure_ok
            || (boss_dwid != 0
                && file_boss_evidence.is_confirmed_kill()
                && !same_name_resumed_after_file_death)
    } else {
        let real_treasure_ok = real_boss_name
            .as_ref()
            .map(|name| treasure_box_bosses.contains(name))
            .unwrap_or(false);
        real_boss_dwid != 0
            && real_boss_ever_participated
            && (real_treasure_ok
                || if boss_dwid == real_boss_dwid {
                    real_boss_evidence.is_confirmed_kill()
                } else {
                    boss_dwid != 0
                        && file_boss_evidence.is_confirmed_kill()
                        && real_boss_evidence.is_confirmed_kill()
                })
    };

    // 确定返回的 boss_name：
    // - 路径 A（文件名 BOSS 在配置中）：直接用文件名 BOSS 名
    // - 路径 B（文件名 BOSS 不在配置中，小怪 JCL）：
    //   * 仅当真实 BOSS 的运行时实体进入战斗时才返回 real_boss_name
    //   * 否则返回 None（小怪 JCL，effective boss name 回退到文件名）
    //   注意：不以对话/NPC_INFO 文本作为依据，避免出场对话 JCL 被误判为该 BOSS 的战斗。
    let final_boss_name = if file_boss_in_config {
        Some(jcl_boss_name.to_string())
    } else if real_boss_ever_participated {
        real_boss_name.clone()
    } else {
        None
    };

    Some(JclAnalysis {
        boss_name: final_boss_name,
        fight_start_ms,
        fight_end_ms,
        is_kill,
    })
}

/// 从一个 JCL 提取可由同角色 10 金补强的完成候选。
///
/// 这是保守的独立路径，不影响 `analyze_jcl` 的强证据语义，也不持久化候选或金币：
/// - 只接受文件名与副本配置**精确唯一**对应的首领，避免小怪/别名/多实体拼接；
/// - type=1 必须恰有一组同 UUID 的开始和结束，且 UUID 精确对应当前账号目录 UID；
/// - 只计算该首领运行时实体、战斗区间内的 type=21 result[13]/[14]；
/// - 有效伤害必须刚好等于唯一观测 maxHP，且没有有效治疗、重入或另一轮战斗。
///
/// `None` 只表示不能安全补强，不会否定已有的 JCL 强击杀证据。
fn analyze_damage_completion_candidate(
    jcl_path: &Path,
    jcl_boss_name: &str,
    boss_template_id: i64,
    raid_bosses: &[(String, String)],
    expected_uid: &str,
) -> Option<DamageCompletionCandidate> {
    if expected_uid.is_empty() || boss_template_id == 0 {
        return None;
    }

    // 只有文件名首领在该副本配置中精确且唯一时，才允许补强。
    // 模糊名、无配置副本、由小怪文件推导出的真实首领都只保留强证据路径。
    if raid_bosses.iter().filter(|(_, name)| name == jcl_boss_name).count() != 1 {
        return None;
    }

    use std::io::BufRead;
    type Effect = (i64, i64, i64, i64, usize);
    type PendingEffect = (i64, i64, i64, Option<i64>, Option<i64>, usize);
    type FightHint = (i64, i64, i64, bool, f64, i64, usize);
    let file = std::fs::File::open(jcl_path).ok()?;
    let reader = std::io::BufReader::new(file);
    let gbk = encoding_rs::GBK;

    let mut fight_start: Option<(i64, i64, String, usize)> = None;
    let mut fight_end: Option<(i64, i64, String, usize)> = None;
    let mut invalid_attempt = false;
    let mut boss_dwid: Option<i64> = None;
    let mut has_multiple_boss_runtime = false;
    let mut has_template_name_mismatch = false;
    let mut fight_hints: Vec<FightHint> = Vec::new();
    let mut effects: Vec<Effect> = Vec::new();
    // NPC_INFO 有时晚于同秒的首批 type=21；先暂存，确认唯一运行时首领后再归属。
    // 只在尚未找到目标实体时缓存，避免大 JCL 为无关战斗事件持续占用内存。
    let mut pending_effects: Vec<PendingEffect> = Vec::new();
    let mut malformed_effect = false;
    let mut seen_type21_records: HashSet<Vec<u8>> = HashSet::new();

    for (line_number, line) in reader.split(b'\n').enumerate() {
        let line_bytes = match line {
            Ok(line) => line,
            Err(_) => return None,
        };
        let mut parts: Vec<&[u8]> = Vec::with_capacity(6);
        let mut start = 0;
        for (index, &byte) in line_bytes.iter().enumerate() {
            if byte == b'\t' {
                parts.push(&line_bytes[start..index]);
                start = index + 1;
                if parts.len() >= 5 {
                    break;
                }
            }
        }
        if parts.len() >= 5 {
            parts.push(&line_bytes[start..]);
        }
        if parts.len() < 6 {
            if !line_bytes.is_empty() { return None; }
            continue;
        }

        let event_type = std::str::from_utf8(parts[4]).ok()?.parse::<u8>().ok()?;
        if !matches!(event_type, 1 | 8 | 9 | 21) { continue; }
        let timestamp_sec = std::str::from_utf8(parts[2]).ok()?.parse::<i64>().ok()?;
        let client_ms = std::str::from_utf8(parts[3]).ok()?.parse::<i64>().ok()?;
        let (lua_str, _, decode_errors) = gbk.decode(parts[5]);
        if decode_errors {
            return None;
        }
        let lua_trimmed = lua_str.trim();
        let lua_inner = lua_trimmed.strip_prefix('{').and_then(|value| value.strip_suffix('}')).unwrap_or(lua_trimmed);
        let fields: Vec<&str> = lua_inner.split(',').collect();

        match event_type {
            1 => {
                if fields.len() < 2 {
                    invalid_attempt = true;
                    continue;
                }
                let in_fight = match fields[0].trim() {
                    "true" => true,
                    "false" => false,
                    _ => {
                        invalid_attempt = true;
                        continue;
                    }
                };
                let uuid = fields[1].trim().trim_matches('"').to_string();
                let target = if in_fight { &mut fight_start } else { &mut fight_end };
                if target.is_some() || !jcl_uuid_matches_uid(&uuid, expected_uid) {
                    invalid_attempt = true;
                } else {
                    *target = Some((timestamp_sec, client_ms, uuid, line_number));
                }
            }
            8 => {
                if fields.len() < 3 {
                    return None;
                }
                let dwid = fields[0].trim().parse::<i64>().ok()?;
                let template_id = fields[2].trim().parse::<i64>().ok()?;
                if dwid != 0 && template_id == boss_template_id {
                    if fields[1].trim().trim_matches('"') != jcl_boss_name {
                        // 同模板但不同名字的实体不能借用文件名首领的伤害。
                        has_template_name_mismatch = true;
                    } else {
                        match boss_dwid {
                            Some(existing) if existing != dwid => has_multiple_boss_runtime = true,
                            Some(_) => {}
                            None => {
                                boss_dwid = Some(dwid);
                                for (target_dwid, timestamp_sec, client_ms, damage, heal, line_number) in pending_effects.drain(..) {
                                    if target_dwid != dwid {
                                        continue;
                                    }
                                    match (damage, heal) {
                                        (Some(damage), Some(heal)) if damage >= 0 && heal == 0 => effects.push((timestamp_sec, client_ms, damage, heal, line_number)),
                                        _ => malformed_effect = true,
                                    }
                                }
                            }
                        }
                    }
                }
            }
            9 => {
                if fields.len() < 4 { return None; }
                let dwid = fields[0].trim().parse::<i64>().ok()?;
                let in_fight = match fields[1].trim() {
                    "true" => true,
                    "false" => false,
                    _ => return None,
                };
                let current_hp = fields[2].trim().parse::<f64>().ok().filter(|value| value.is_finite() && *value >= 0.0)?;
                let max_hp = fields[3].trim().parse::<f64>().ok().filter(|value| value.is_finite() && *value > 0.0).and_then(|value| {
                    let rounded = value.round();
                    ((value - rounded).abs() < 0.0001).then_some(rounded as i64)
                })?;
                fight_hints.push((dwid, timestamp_sec, client_ms, in_fight, current_hp, max_hp, line_number));
            }
            21 => {
                // type=21: {sourceDwid, targetDwid, ..., result={[13]=damageEff,[14]=healEff}}
                let mut duplicate_key = Vec::with_capacity(parts[0].len() + parts[1].len() + parts[3].len() + parts[5].len() + 3);
                duplicate_key.extend_from_slice(parts[0]);
                duplicate_key.push(0);
                duplicate_key.extend_from_slice(parts[1]);
                duplicate_key.push(0);
                duplicate_key.extend_from_slice(parts[3]);
                duplicate_key.push(0);
                duplicate_key.extend_from_slice(parts[5]);
                if !seen_type21_records.insert(duplicate_key) {
                    return None;
                }
                let target_dwid = fields.get(1).and_then(|value| value.trim().parse::<i64>().ok());
                let target_dwid = target_dwid?;
                let damage = DAMAGE_EFF_RE.captures(lua_inner).and_then(|captures| captures[1].parse::<i64>().ok());
                let heal = HEAL_EFF_RE.captures(lua_inner).and_then(|captures| captures[1].parse::<i64>().ok());
                if Some(target_dwid) == boss_dwid {
                    match (damage, heal) {
                        (Some(damage), Some(heal)) if damage >= 0 && heal == 0 => effects.push((timestamp_sec, client_ms, damage, heal, line_number)),
                        (Some(_), Some(_)) => return None,
                        _ => malformed_effect = true,
                    }
                } else if boss_dwid.is_none() {
                    pending_effects.push((target_dwid, timestamp_sec, client_ms, damage, heal, line_number));
                }
            }
            _ => unreachable!(),
        }
    }

    let (start_sec, start_client_ms, start_uuid, start_line) = fight_start?;
    let (end_sec, _end_client_ms, end_uuid, end_line) = fight_end?;
    if invalid_attempt
        || start_uuid != end_uuid
        || !jcl_uuid_matches_uid(&start_uuid, expected_uid)
        || !jcl_uuid_matches_uid(&end_uuid, expected_uid)
        || end_sec < start_sec
        || end_line <= start_line
        || boss_dwid.is_none()
        || has_multiple_boss_runtime
        || has_template_name_mismatch
        || malformed_effect
    {
        return None;
    }

    let boss_dwid = boss_dwid?;
    let mut saw_participation = false;
    let mut first_false_line: Option<usize> = None;
    let mut observed_max_hp: Option<i64> = None;
    let mut last_active_hp: Option<f64> = None;
    for (dwid, timestamp_sec, client_ms, in_fight, current_hp, max_hp, line_number) in fight_hints {
        if dwid != boss_dwid { continue; }
        let in_import_prefix = line_number < start_line
            && timestamp_sec >= start_sec.saturating_sub(1)
            && timestamp_sec <= start_sec
            && client_ms >= start_client_ms.saturating_sub(1000)
            && client_ms <= start_client_ms;
        if line_number >= end_line || (!in_import_prefix && (line_number <= start_line || timestamp_sec < start_sec || timestamp_sec > end_sec)) {
            return None;
        }
        if in_fight {
            if first_false_line.is_some() || last_active_hp.is_some_and(|previous| current_hp > previous) { return None; }
            saw_participation = true;
            last_active_hp = Some(current_hp);
        } else if saw_participation {
            first_false_line.get_or_insert(line_number);
        }
        if observed_max_hp.is_some_and(|previous| previous != max_hp) { return None; }
        observed_max_hp.get_or_insert(max_hp);
    }
    if !saw_participation { return None; }

    let max_hp = observed_max_hp?;
    let mut total_damage = 0i64;
    let mut total_heal = 0i64;
    let mut reached_max = false;
    for (timestamp_sec, client_ms, damage, heal, line_number) in effects {
        // 文件头/尾不完整或跨战斗的伤害不能参与，也不能忽略后伪造完整击杀。
        let in_import_prefix = line_number < start_line
            && timestamp_sec >= start_sec.saturating_sub(1)
            && timestamp_sec <= start_sec
            && client_ms >= start_client_ms.saturating_sub(1000)
            && client_ms <= start_client_ms;
        if line_number >= end_line || (!in_import_prefix && (line_number <= start_line || timestamp_sec < start_sec || timestamp_sec > end_sec)) {
            if damage != 0 || heal != 0 {
                return None;
            }
            continue;
        }
        if first_false_line.is_some_and(|false_line| line_number > false_line) && damage > 0 {
            return None;
        }
        if reached_max && damage > 0 {
            return None;
        }
        total_damage = total_damage.checked_add(damage)?;
        total_heal = total_heal.checked_add(heal)?;
        if total_damage == max_hp {
            reached_max = true;
        } else if total_damage > max_hp {
            return None;
        }
    }

    if !reached_max || total_damage != max_hp || total_heal != 0 {
        return None;
    }

    Some(DamageCompletionCandidate { boss_name: jcl_boss_name.to_string(), fight_end_ms: end_sec.saturating_mul(1000) })
}

/// type=1 的 szUUID 带有运行时包装，例如 `服务器::U123456/1`；ChatInfo 保存的是
/// 数字 user_global_id。只接受完整 U 段，绝不做数字子串匹配。
fn jcl_uuid_matches_uid(jcl_uuid: &str, expected_uid: &str) -> bool {
    let with_u = if let Some((_, suffix)) = jcl_uuid.rsplit_once("::U") {
        suffix
    } else if let Some(suffix) = jcl_uuid.strip_prefix('U') {
        suffix
    } else {
        return false;
    };
    let Some((uid, instance)) = with_u.split_once('/') else {
        return false;
    };
    !uid.is_empty()
        && uid.bytes().all(|byte| byte.is_ascii_digit())
        && !instance.is_empty()
        && instance.bytes().all(|byte| byte.is_ascii_digit())
        && uid == expected_uid
}

/// 收集本轮每份 JCL 中同角色的全部 type=1 结束时间，供外部 10 金做争用锚。
/// 它不判首领、不写缓存；任一文件损坏时调用方必须禁用补漏，不能把该文件附近的
/// 10 金借给别的候选。
fn collect_type1_end_anchors(jcl_path: &Path, expected_uid: &str) -> Option<Vec<i64>> {
    use std::io::BufRead;
    let file = std::fs::File::open(jcl_path).ok()?;
    let reader = std::io::BufReader::new(file);
    let gbk = encoding_rs::GBK;
    let mut ends = Vec::new();
    for line in reader.split(b'\n') {
        let line = line.ok()?;
        if line.is_empty() { continue; }
        let parts: Vec<&[u8]> = line.splitn(6, |byte| *byte == b'\t').collect();
        if parts.len() < 6 { return None; }
        let event_type = std::str::from_utf8(parts[4]).ok()?.parse::<u8>().ok()?;
        if event_type != 1 { continue; }
        let timestamp = std::str::from_utf8(parts[2]).ok()?.parse::<i64>().ok()?;
        let (lua, _, errors) = gbk.decode(parts[5]);
        if errors { return None; }
        let inner = lua.trim().strip_prefix('{').and_then(|value| value.strip_suffix('}')).unwrap_or(lua.trim());
        let fields: Vec<&str> = inner.split(',').collect();
        if fields.len() < 2 { return None; }
        match fields[0].trim() {
            "true" => {}
            "false" if jcl_uuid_matches_uid(fields[1].trim().trim_matches('"'), expected_uid) => ends.push(timestamp.saturating_mul(1000)),
            "false" => {}
            _ => return None,
        }
    }
    Some(ends)
}

/// BOSS 击杀奖励（单位：金）。JX3 金币单位为 砖/金/银/铜，
/// 1 金砖 = 10000 金，1 金 = 100 银，1 银 = 100 铜。
/// BOSS 击杀奖励固定为 10 金，在 chatlog 收入统计中需排除（不计入其他收入）。
const BOSS_KILL_REWARD_GOLD: i64 = 10;

/// 从 chatlog 数据库的 msg 字段解析金币的 金/银/铜 分量。
///
/// chatlog 的 `text` 字段（如 `你获得：1000。`）中的数字是
/// Gold/Silver/Copper 值的**字符串拼接**（如 Gold="10", Silver="0", Copper="0"
/// 拼成 "1000"），不是真实金额。必须解析 `msg` 字段中的 UI 标记
/// （`name="Text_Gold"` / `name="Text_Silver"` / `name="Text_Copper"`）
/// 才能获得真实的金/银/铜值。
///
/// 返回 (金, 银, 铜)，解析失败返回 None。
fn parse_money_from_msg(msg: &str) -> Option<(i64, i64, i64)> {
    // JX3 chatlog 的 msg 字段为 XML 格式，金额通过多个 text 节点展示：
    //   text="1"    name="Text_GoldB"   → 万金部分（≥10000金时出现）
    //   text="6082" name="Text_Gold"    → 金部分（<10000金）
    //   text="0"    name="Text_Silver"  → 银
    //   text="0"    name="Text_Copper" → 铜
    // 实际金额 = GoldB × 10000 + Gold
    let goldb = GOLDB_RE
        .captures(msg)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);
    let gold = GOLD_RE
        .captures(msg)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);
    let silver = SILVER_RE
        .captures(msg)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);
    let copper = COPPER_RE
        .captures(msg)
        .and_then(|c| c[1].parse::<i64>().ok())
        .unwrap_or(0);

    // 万金部分合并到金
    let total_gold = goldb * 10000 + gold;

    // 如果三个分量都为 0 且没有匹配到任何 Text_Gold 标记，说明 msg 格式不符
    if total_gold == 0 && silver == 0 && copper == 0
        && !msg.contains("Text_Gold")
    {
        return None;
    }

    Some((total_gold, silver, copper))
}

/// 候选 JCL 结束与同角色固定 10 金奖励的最大允许时间差。
///
/// 这是当前样本覆盖的候选时间窗；它不代表通用写盘延迟，也不降低任何 JCL 侧门槛。
const DAMAGE_MONEY_MATCH_WINDOW_SECS: i64 = 15;

/// 从单个角色确认为自己的 chatlog DB 中读取精确 10 金时间。
///
/// 这是完成候选的补充证据，故不能复用财务预加载：预加载允许多个可读文件参与收入
/// 统计，而这里逐库要求 ChatInfo.user_global_id 与当前 JCL UUID 精确相同。未知、缺失
/// 或不匹配的 DB 整库跳过，且不影响现有收入/掉落逻辑。
fn read_role_verified_ten_gold_times(chatlog_files: &[PathBuf], expected_uid: &str, candidates: &[DamageCompletionCandidate]) -> Vec<i64> {
    if expected_uid.is_empty() || candidates.is_empty() {
        return Vec::new();
    }

    let first_sec = candidates.iter().map(|candidate| candidate.fight_end_ms / 1000).min().unwrap_or(0).saturating_sub(DAMAGE_MONEY_MATCH_WINDOW_SECS);
    let last_sec = candidates.iter().map(|candidate| candidate.fight_end_ms / 1000).max().unwrap_or(0).saturating_add(DAMAGE_MONEY_MATCH_WINDOW_SECS);

    let mut reward_times = Vec::new();
    for path in chatlog_files {
        let conn = match Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) {
            Ok(conn) => conn,
            Err(error) => {
                log::warn!("[DropScanner] 补漏打开 chatlog 失败: {} - {}", path.display(), error);
                continue;
            }
        };
        let user_global_id: Option<String> = conn.query_row(
            "SELECT value FROM ChatInfo WHERE key = 'user_global_id'",
            [],
            |row| row.get::<_, Option<String>>(0),
        ).ok().flatten().and_then(|value| normalize_chatlog_user_global_id(&value));
        if user_global_id.as_deref() != Some(expected_uid) {
            log::debug!("[DropScanner] 补漏跳过非当前角色或未标识 chatlog: {}", path.display());
            continue;
        }

        let mut statement = match conn.prepare(
            "SELECT msg, time FROM ChatLog \
             WHERE type = 'MSG_MONEY' AND text LIKE '你获得：%' AND time >= ?1 AND time <= ?2",
        ) {
            Ok(statement) => statement,
            Err(error) => {
                log::warn!("[DropScanner] 补漏查询 chatlog 失败: {} - {}", path.display(), error);
                continue;
            }
        };
        let rows = statement.query_map(params![first_sec, last_sec], |row| {
            Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?))
        });
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                if row.0.as_deref().is_some_and(is_exact_boss_kill_reward_msg) {
                    reward_times.push(row.1);
                }
            }
        }
    }
    reward_times.sort_unstable();
    reward_times.dedup();
    reward_times
}

/// 仅供完成补漏使用的严格 10 金格式：金、银、铜三项必须实际出现在消息 XML 中。
/// 财务统计继续使用兼容旧日志的 `parse_money_from_msg`，这里不能把缺失字段默认成 0。
fn is_exact_boss_kill_reward_msg(msg: &str) -> bool {
    let gold = GOLD_RE.captures(msg).and_then(|captures| captures[1].parse::<i64>().ok());
    let silver = SILVER_RE.captures(msg).and_then(|captures| captures[1].parse::<i64>().ok());
    let copper = COPPER_RE.captures(msg).and_then(|captures| captures[1].parse::<i64>().ok());
    let gold_brick = GOLDB_RE.captures(msg).and_then(|captures| captures[1].parse::<i64>().ok()).unwrap_or(0);
    matches!((gold, silver, copper), (Some(gold), Some(0), Some(0)) if gold_brick == 0 && gold == BOSS_KILL_REWARD_GOLD)
}

/// ChatInfo 的 user_global_id 在不同茗伊版本中可能是裸数字，也可能是 JSON 字符串。
/// 仅接受这两种完整数字表示，拒绝任意引号裁剪或数字子串匹配。
fn normalize_chatlog_user_global_id(value: &str) -> Option<String> {
    let normalized = if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        value.to_string()
    } else {
        serde_json::from_str::<String>(value).ok()?
    };
    (!normalized.is_empty() && normalized.bytes().all(|byte| byte.is_ascii_digit())).then_some(normalized)
}

/// 从候选和已校验角色身份的 10 金中选出可补强的 JCL 文件。
///
/// 一个 10 金不能服务于多个候选；同一候选即使有多条同场 10 金，也最多补强一次。
/// `attempt_end_times` 包含本轮所有有完整战斗边界的 JCL 尝试，故一笔钱即使只落在
/// 一个候选、但同时邻近另一未达标/强证据尝试，也不会被借用。
fn select_damage_money_promotions(
    candidates: &HashMap<String, DamageCompletionCandidate>,
    attempt_end_times: &[i64],
    reward_times: &[i64],
) -> HashSet<String> {
    let mut result = HashSet::new();
    for (file_name, candidate) in candidates {
        let end_sec = candidate.fight_end_ms / 1000;
        let has_exclusive_reward = reward_times.iter().any(|reward_time| {
            (reward_time - end_sec).abs() <= DAMAGE_MONEY_MATCH_WINDOW_SECS
                && attempt_end_times
                    .iter()
                    .filter(|end_ms| ((**end_ms / 1000) - reward_time).abs() <= DAMAGE_MONEY_MATCH_WINDOW_SECS)
                    .count()
                    == 1
        });
        if has_exclusive_reward {
            result.insert(file_name.clone());
        }
    }
    result
}

/// 1 金砖 = 10000 金（JX3 标准换算）
const GOLD_PER_BRICK: i64 = 10000;

/// 从支出金额字符串解析总金币数。
///
/// 支出消息格式：`[角色名·服务器]花费[XX金砖XX金]购买了[物品名]`
/// 金额部分可能为：
///   - `2000金` → 2000
///   - `1金砖` → 10000
///   - `1金砖6000金` → 16000
///   - `0金` → 0
fn parse_expense_amount(amount_str: &str) -> i64 {
    let mut total = 0i64;

    // 先提取金砖部分
    if let Some(caps) = BRICK_RE.captures(amount_str) {
        total += caps[1].parse::<i64>().unwrap_or(0) * GOLD_PER_BRICK;
    }
    
    // 移除“X金砖”后，提取剩余的金部分
    let remaining = BRICK_RE.replace_all(amount_str, "");
    if let Some(caps) = EXPENSE_GOLD_RE.captures(&remaining) {
        total += caps[1].parse::<i64>().unwrap_or(0);
    }

    total
}

/// 去除角色名中的空格（全角/半角），用于 chatlog 匹配
fn normalize_role_name(name: &str) -> String {
    name.replace(' ', "").replace('\u{3000}', "")
}


/// 从 chatlog 数据库提取副本期间的物品分配、金币收入和支出。
///
/// chatlog 的 time 字段为**秒级** Unix 时间戳（非毫秒），调用方传入毫秒级时间戳，
/// 内部转换为秒级后查询。
///
/// 提取三类信息：
/// 1. **物品分配**（type='MSG_ITEM'）：格式 `分配者将：[物品名]分配给角色名·服务器。`
///    正则提取方括号内的物品名，作为 drops 列表。
/// 2. **金币收入**（type='MSG_MONEY'，text 以"你获得："开头）：
///    - BOSS 击杀奖励（Gold=10, Silver=0, Copper=0）不计入收入，仅计数 boss_kill_count。
///    - 其余"你获得："金币累加为 other_income_gold（排除 10 金）。
/// 3. **拍团底薪**（type='MSG_ROOM' 或 'MSG_WHISPER'，text 含"每人底薪：XXX金"）：
///    若存在则 base_salary = Some(XXX)，收入优先取底薪值。
/// 4. **支出**（type='MSG_ROOM' 或 'MSG_WHISPER'，text 含"[角色名]花费[XX金砖XX金]购买了[物品名]"）：
///    仅统计当前角色（role_name 匹配）的花费，金砖按 1金砖=10000金 换算。
///    支出消息可能出现在房间频道(MSG_ROOM)或密语频道(MSG_WHISPER)，均需查询。
///    注意：仅"花费了...购买了"格式计为支出；"记录给了"格式仅提取物品名加入 drops，不计入支出
///    （团长分配记录不代表角色实际购买了该物品）。
/// 5. **购买数量补充**：每条"购买了"消息以前后 2 秒内的本地入包日志
///    `你获得：[物品名] × N。`（MSG_ITEM）配对真实数量，多条到账累加且每条只消费一次；
///    找不到到账事件时数量回退为 1。无独立到账且与已接受交易同物品+同金额+间隔 ≤3 秒的
///    消息视为双频道回显并丢弃。
///
/// 返回 (drops, base_salary, other_income_gold, expense_gold, boss_kill_count, income_records, boss_kill_times, purchased_items)。
/// income_records 为非 BOSS 10金的收入记录列表 (time_sec, gold)，用于精确收入匹配。
/// boss_kill_times 为每个 BOSS 击杀（10金收入）的 time_sec 列表，用于击杀数验证。
/// purchased_items 为当前角色购买的物品映射（物品名 → PurchaseInfo），用于散件老板功能。
/// 调用方根据 base_salary 是否存在决定最终收入：
///   - 有底薪 → 在 income_records 中找 gold >= base_salary 的第一条记录，用该 gold 作为收入
///   - 无底薪 → 在 income_records 中找 time_sec > last_jcl_sec 的第一条记录（拍卖工资只发一次）
///     找不到时回退到收入总和
#[allow(dead_code)]
fn extract_drops_from_chatlog(
    chatlog_path: &PathBuf,
    start_time_ms: i64,
    end_time_ms: i64,
    role_name: &str,
) -> Result<(Vec<String>, Option<i64>, i64, i64, u32, Vec<(i64, i64)>, Vec<i64>, HashMap<String, PurchaseInfo>), String> {
    if !chatlog_path.exists() {
        return Ok((Vec::new(), None, 0, 0, 0, Vec::new(), Vec::new(), HashMap::new()));
    }

    let conn = Connection::open(chatlog_path)
        .map_err(|e| format!("打开 chatlog 数据库失败: {} - {}", chatlog_path.display(), e))?;

    // chatlog 的 time 是秒级时间戳，将毫秒级参数转换为秒级
    let start_sec = start_time_ms / 1000;
    let end_sec = end_time_ms / 1000;

    // 查询 MSG_ITEM（物品分配）、MSG_MONEY（金币获得）和 MSG_ROOM/MSG_WHISPER（底薪/支出）
    // MSG_MONEY 只查询 text 以"你获得："开头的收入消息（精确过滤，跳过修理等支出）
    // MSG_ROOM/MSG_WHISPER 查询包含"每人底薪"、"花费["且"购买了"（直接购买支出）、"记录给了"（团长分配支出）的消息
    // 支出消息可能出现在 MSG_ROOM（房间频道）或 MSG_WHISPER（密语频道），两种都需要查询
    let mut stmt = conn
        .prepare(
            "SELECT type, text, msg, time FROM ChatLog \
             WHERE time >= ?1 AND time <= ?2 AND ( \
               type = 'MSG_ITEM' \
               OR (type = 'MSG_MONEY' AND text LIKE '你获得：%') \
               OR ((type = 'MSG_ROOM' OR type = 'MSG_WHISPER') AND ( \
                 text LIKE '%每人底薪%' \
                 OR (text LIKE '%花费[%' AND text LIKE '%购买了%') \
                 OR text LIKE '%记录给了%' \
               )) \
             )",
        )
        .map_err(|e| format!("准备 chatlog 查询失败: {}", e))?;

    let rows = stmt
        .query_map(params![start_sec, end_sec], |row| {
            Ok(ChatlogRecord {
                msg_type: row.get::<_, String>(0)?,
                text: row.get::<_, String>(1)?,
                msg: row.get::<_, String>(2)?,
                time_sec: row.get::<_, i64>(3)?,
            })
        })
        .map_err(|e| format!("执行 chatlog 查询失败: {}", e))?;

    let mut records: Vec<ChatlogRecord> = Vec::new();
    for row in rows {
        match row {
            Ok(r) => records.push(r),
            Err(e) => log::warn!("[DropScanner] 读取 chatlog 行失败: {}", e),
        }
    }

    Ok(extract_drops_from_records(&records, start_time_ms, end_time_ms, role_name))
}

/// 预加载账号下所有 chatlog 文件中的相关记录到内存
///
/// 在 scan_raid_drops_with_raids 入口处调用一次，避免后续每个副本实例重复打开
/// SQLite 连接和全表扫描。返回按 time_sec 升序排序的记录列表。
///
/// 时间范围：[scan_start_ms, scan_end_ms + 2h]，2h 缓冲用于覆盖最后一个副本实例
/// 的 chatlog_end 扩展（last_jcl_time + 2h 或 now_ms）。
fn preload_chatlog_records(
    chatlog_files: &[PathBuf],
    chatlog_range_cache: &HashMap<PathBuf, Option<(i64, i64)>>,
    scan_start_ms: i64,
    scan_end_ms: i64,
) -> Vec<ChatlogRecord> {
    // 预加载范围：scan_end_ms + 2h，覆盖最后副本实例的 chatlog_end 扩展
    let buffer_ms: i64 = 2 * 60 * 60 * 1000;
    let preload_end_ms = scan_end_ms.saturating_add(buffer_ms);
    let preload_start_sec = scan_start_ms / 1000;
    let preload_end_sec = preload_end_ms / 1000;

    let mut all_records: Vec<ChatlogRecord> = Vec::new();
    let mut total_files_queried = 0u32;

    for path in chatlog_files {
        // 用 chatlog_range_cache 过滤不覆盖预加载范围的文件
        if !chatlog_file_covers_range_cached(path, scan_start_ms, preload_end_ms, chatlog_range_cache) {
            continue;
        }
        total_files_queried += 1;

        let conn = match Connection::open(path) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("[DropScanner] 预加载打开 chatlog 失败: {} - {}", path.display(), e);
                continue;
            }
        };

        let mut stmt = match conn.prepare(
            "SELECT type, text, msg, time FROM ChatLog \
             WHERE time >= ?1 AND time <= ?2 AND ( \
               type = 'MSG_ITEM' \
               OR (type = 'MSG_MONEY' AND text LIKE '你获得：%') \
               OR ((type = 'MSG_ROOM' OR type = 'MSG_WHISPER') AND ( \
                 text LIKE '%每人底薪%' \
                 OR (text LIKE '%花费[%' AND text LIKE '%购买了%') \
                 OR text LIKE '%记录给了%' \
               )) \
             )",
        ) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[DropScanner] 预加载 prepare 失败: {} - {}", path.display(), e);
                continue;
            }
        };

        let rows = stmt.query_map(params![preload_start_sec, preload_end_sec], |row| {
            Ok(ChatlogRecord {
                msg_type: row.get::<_, String>(0)?,
                text: row.get::<_, String>(1)?,
                msg: row.get::<_, String>(2)?,
                time_sec: row.get::<_, i64>(3)?,
            })
        });

        if let Ok(rows) = rows {
            for row in rows.flatten() {
                all_records.push(row);
            }
        }
    }

    // 按 time_sec 升序排序，支持后续二分查找
    all_records.sort_by_key(|r| r.time_sec);

    log::info!(
        "[DropScanner] chatlog 预加载完成: 查询 {} 个文件, 共 {} 条记录, 范围 [{}~{}+2h]",
        total_files_queried,
        all_records.len(),
        scan_start_ms,
        scan_end_ms
    );

    all_records
}

/// 从预加载的 chatlog 记录中提取掉落物、金币收入和支出（纯内存操作）
///
/// 这是 extract_drops_from_chatlog 的核心逻辑，接受预加载的记录列表，
/// 通过二分查找定位时间范围内的记录，避免重复打开 SQLite 连接。
///
/// 返回 (drops, base_salary, other_income_gold, expense_gold, boss_kill_count,
///       income_records, boss_kill_times, purchased_items)。
/// purchased_items 为当前角色购买的物品映射（物品名 → PurchaseInfo），用于散件老板功能。
fn extract_drops_from_records(
    records: &[ChatlogRecord],
    start_time_ms: i64,
    end_time_ms: i64,
    role_name: &str,
) -> (Vec<String>, Option<i64>, i64, i64, u32, Vec<(i64, i64)>, Vec<i64>, HashMap<String, PurchaseInfo>) {
    let start_sec = start_time_ms / 1000;
    let end_sec = end_time_ms / 1000;

    // 二分查找时间范围 [start_sec, end_sec]
    let start_idx = records.partition_point(|r| r.time_sec < start_sec);
    let end_idx = records.partition_point(|r| r.time_sec <= end_sec);
    let relevant = &records[start_idx..end_idx];

    let mut drops_set: HashSet<String> = HashSet::new();
    let mut base_salary: Option<i64> = None;
    // 非底薪的金币收入（排除 BOSS 10金），以铜为单位累加
    let mut other_income_copper: i64 = 0;
    let mut expense_gold: i64 = 0;
    let mut item_count = 0u32;
    let mut money_count = 0u32;
    let mut boss_kill_count = 0u32;
    // 每个 BOSS 击杀（10金收入）的 time_sec 列表，用于击杀数验证
    let mut boss_kill_times: Vec<i64> = Vec::new();
    // 非 BOSS 10金的收入记录列表：(time_sec, gold)，用于精确收入匹配
    let mut income_records: Vec<(i64, i64)> = Vec::new();

    let target_name = normalize_role_name(role_name);

    // 到账事件预收集：本地角色 MSG_ITEM 入包日志（你获得：[物品] × N。）
    // 用于给"花费购买了"消息补充真实数量（每条到账事件只被配对消费一次）。
    // (物品名, 数量, 时间秒, 是否已消费)
    let mut gain_events: Vec<(String, u32, i64, bool)> = Vec::new();
    for r in relevant.iter() {
        if r.msg_type == "MSG_ITEM" {
            if let Some(caps) = GAIN_QTY_RE.captures(&r.text) {
                let qty: u32 = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .filter(|&q| q > 0)
                    .unwrap_or(1);
                gain_events.push((caps[1].to_string(), qty, r.time_sec, false));
            }
        }
    }

    // 当前角色花钱购买的物品映射（物品名 → PurchaseInfo，用于散件老板功能）
    let mut purchased_items_map: HashMap<String, PurchaseInfo> = HashMap::new();
    // 已接受的购买交易：(物品名, 金额, 时间秒)，用于回显抑制：
    // 同一交易可能在 MSG_ROOM 和 MSG_WHISPER 双频道近乎同时播报，
    // 无独立到账事件且与已接受交易同物品+同金额+间隔 ≤3 秒的消息视为回显并丢弃。
    let mut accepted_purchases: Vec<(String, i64, i64)> = Vec::new();

    for record in relevant {
        let msg_type = &record.msg_type;
        let text = &record.text;
        let _msg = &record.msg;
        let time_sec = record.time_sec;

        if msg_type == "MSG_ITEM" {
            // 提取分配的物品名
            if let Some(caps) = ITEM_RE.captures(text) {
                let item_name = caps[1].to_string();
                drops_set.insert(item_name);
                item_count += 1;
            }
        } else if msg_type == "MSG_MONEY" {
            // SQL 已过滤 text LIKE '你获得：%'，此处直接解析金额
            // 解析 msg 字段获取真实的金/银/铜值
            if let Some((gold, silver, copper)) = parse_money_from_msg(_msg) {
                money_count += 1;
                // BOSS 击杀奖励：固定 10 金，不计入收入，仅计数
                if gold == BOSS_KILL_REWARD_GOLD && silver == 0 && copper == 0 {
                    boss_kill_count += 1;
                    boss_kill_times.push(time_sec);
                } else {
                    // 其他收入累加（以铜为单位）
                    other_income_copper += gold * 10000 + silver * 100 + copper;
                    // 记录非 BOSS 10金的收入记录，用于精确收入匹配
                    income_records.push((time_sec, gold));
                }
            }
        } else if msg_type == "MSG_ROOM" || msg_type == "MSG_WHISPER" {
            // 拍团底薪/支出消息可能出现在房间频道(MSG_ROOM)或密语频道(MSG_WHISPER)
            // 拍团底薪消息：取最后一条（最终分配金额）
            if let Some(caps) = SALARY_RE.captures(text) {
                let salary: i64 = caps[1].parse().unwrap_or(0);
                base_salary = Some(salary);
            }
            // 支出消息（直接购买格式）：[角色名]花费[金额]购买了[物品名]
            // 这是唯一确认的支出格式，"记录给了"只是团长分配记录不代表实际购买
            if let Some(caps) = EXPENSE_MSG_RE.captures(text) {
                let buyer_name = normalize_role_name(&caps[1]);
                // 匹配角色名（buyer_name 格式为"角色名·服务器"，检查前缀）
                if buyer_name == target_name
                    || buyer_name.starts_with(&format!("{}·", target_name))
                {
                    let amount = parse_expense_amount(&caps[2]);
                    let item_name = caps[3].to_string();

                    // 数量补充：优先取购买消息前后 2 秒内同物品的未消费到账事件
                    // （你获得：[物品] × N。，可多条累加，每条只消费一次）。
                    // 找不到到账事件时回退数量 1。
                    const GAIN_PAIR_WINDOW_SECS: i64 = 2;
                    let mut qty: u32 = 0;
                    for g in gain_events.iter_mut() {
                        if g.3 || g.0 != item_name {
                            continue;
                        }
                        if (g.2 - time_sec).abs() <= GAIN_PAIR_WINDOW_SECS {
                            g.3 = true;
                            qty += g.1;
                        }
                    }

                    // 回显抑制：无独立到账事件、且与已接受交易同物品+同金额+间隔 ≤3 秒，
                    // 视为 MSG_ROOM/MSG_WHISPER 双频道对同一笔交易的重复播报，跳过。
                    const ECHO_WINDOW_SECS: i64 = 3;
                    let is_echo = qty == 0
                        && accepted_purchases.iter().any(|(name, amt, t)| {
                            *name == item_name && *amt == amount && (*t - time_sec).abs() <= ECHO_WINDOW_SECS
                        });
                    if !is_echo {
                        if qty == 0 {
                            qty = 1; // 到账日志缺失时回退为 1 笔
                        }
                        expense_gold += amount;
                        accepted_purchases.push((item_name.clone(), amount, time_sec));
                        // 记录购买的物品（count=真实数量，total_price 累加金额，用于散件老板功能）
                        let info = purchased_items_map.entry(item_name.clone()).or_default();
                        info.count += qty;
                        info.total_price += amount;
                        // 当前角色购买的物品也加入 drops（属于副本掉落物分配）
                        drops_set.insert(item_name);
                    }
                }
            }
            // 团长分配记录和拍卖物品名提取仅在 MSG_ROOM（团队频道）处理
            // MSG_WHISPER（密语频道）可能包含玩家间私人交易或闲聊中的"购买了[X]"，
            // 不能作为副本掉落物，否则会导致 drops 数量虚高
            if msg_type == "MSG_ROOM" {
                // 团长分配记录：[分配者]将[物品名]以[金额]记录给了[接收者]
                // 仅提取物品名加入 drops（掉落物记录），不计入支出（分配不等于实际购买）
                if let Some(caps) = ALLOCATE_TO_RE.captures(text) {
                    let item_name = caps[2].to_string();
                    drops_set.insert(item_name);
                }
                // 拍卖物品名提取：MSG_ROOM 中的"购买了[物品名]"都是副本掉落物分配
                // （包括帮别人购买的情况，用于检测玄晶等稀有掉落）
                if let Some(caps) = PURCHASED_ITEM_RE.captures(text) {
                    let item_name = caps[1].to_string();
                    drops_set.insert(item_name);
                }
            }
        }
    }

    // HashSet 转 Vec
    let drops: Vec<String> = drops_set.into_iter().collect();

    // 铜转金（整数，截断银铜部分）
    let other_income_gold = other_income_copper / 10000;

    // 购买物品总件数（含重复购买）
    let purchased_total_count: u32 = purchased_items_map.values().map(|i| i.count).sum();

    log::info!(
        "[DropScanner] chatlog 提取完成: {} 个物品分配, {} 条金币获得, 底薪={:?}, 其他收入 {} 金, 支出 {} 金, BOSS击杀奖励 {} 次, 购买物品 {} 种 {} 件",
        item_count,
        money_count,
        base_salary,
        other_income_gold,
        expense_gold,
        boss_kill_count,
        purchased_items_map.len(),
        purchased_total_count
    );

    (drops, base_salary, other_income_gold, expense_gold, boss_kill_count, income_records, boss_kill_times, purchased_items_map)
}

/// 扫描账号目录下所有 chatlog 数据库文件
fn find_chatlog_files(account_dir: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let chat_log_dir = account_dir.join("userdata").join("chat_log");

    if !chat_log_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let entries = std::fs::read_dir(&chat_log_dir).map_err(|e| {
        format!("读取 chat_log 目录失败: {} - {}", chat_log_dir.display(), e)
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            if name.starts_with("chatlog_") && name.ends_with(".v2.db") {
                files.push(path);
            }
        }
    }

    Ok(files)
}

/// 查询 chatlog 文件的 ChatInfo.min_time 和 ChatInfo.max_time（秒级时间戳）
///
/// 用于预过滤：只对覆盖副本时间范围的 chatlog 文件执行完整 ChatLog 查询。
/// ChatInfo 表是 key-value 结构，只有几行，查询成本远低于扫描 ChatLog 表。
///
/// 返回 None 表示无法读取（文件损坏或无 ChatInfo 表），调用方应保守保留。
fn get_chatlog_time_range(path: &Path) -> Option<(i64, i64)> {
    let conn = Connection::open(path).ok()?;
    let min_time: i64 = conn
        .query_row(
            "SELECT value FROM ChatInfo WHERE key = 'min_time'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()?
        .parse()
        .ok()?;
    let max_time: i64 = conn
        .query_row(
            "SELECT value FROM ChatInfo WHERE key = 'max_time'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()?
        .parse()
        .ok()?;
    Some((min_time, max_time))
}

/// 判断 chatlog 文件是否可能包含指定时间范围 [start_ms, end_ms] 的记录
///
/// 通过查询 ChatInfo 表的 min_time / max_time 判断文件覆盖的时间范围。
/// chatlog 文件名为 hex hash（如 `chatlog_16faed.v2.db`），无法从文件名解析日期，
/// 必须查询 SQLite ChatInfo 表获取时间范围。
///
/// 特殊处理：max_time=0 表示文件仍在写入（活跃文件），max_time 视为无穷大；
/// min_time=0 表示文件可能为空或刚创建，min_time 视为 0（始终匹配）。
#[allow(dead_code)]
fn chatlog_file_covers_range(path: &Path, start_ms: i64, end_ms: i64) -> bool {
    // ChatInfo 时间是秒级，参数是毫秒级，统一转换为秒级比较
    let start_sec = start_ms / 1000;
    let end_sec = end_ms / 1000;
    match get_chatlog_time_range(path) {
        Some((min_time, max_time)) => {
            // max_time=0 表示活跃文件（仍在写入），视为无穷大
            let effective_max = if max_time == 0 { i64::MAX } else { max_time };
            // min_time=0 视为 0（始终匹配下界）
            let effective_min = min_time;
            // 文件时间范围 [effective_min, effective_max] 与查询范围 [start_sec, end_sec] 有交集
            effective_min <= end_sec && effective_max >= start_sec
        }
        None => true, // 无法读取 ChatInfo 时保守保留
    }
}

/// 带缓存的 chatlog 文件时间范围检查
fn chatlog_file_covers_range_cached(
    path: &Path,
    start_ms: i64,
    end_ms: i64,
    cache: &HashMap<PathBuf, Option<(i64, i64)>>,
) -> bool {
    let start_sec = start_ms / 1000;
    let end_sec = end_ms / 1000;
    match cache.get(path).and_then(|v| v.as_ref()) {
        Some(&(min_time, max_time)) => {
            let effective_max = if max_time == 0 { i64::MAX } else { max_time };
            min_time <= end_sec && effective_max >= start_sec
        }
        None => true, // 无法读取时保守保留
    }
}

/// 从 chatlog 数据库提取角色 ID（user_global_id）
///
/// 注意：当前未使用，role_id 改为通过 info.jx3dat + roles 表查询获取 UUID。
/// 保留此函数以备未来调试或交叉验证使用。
#[allow(dead_code)]
fn extract_role_id_from_chatlog(chatlog_path: &PathBuf) -> Result<Option<String>, String> {
    if !chatlog_path.exists() {
        return Ok(None);
    }

    let conn = Connection::open(chatlog_path)
        .map_err(|e| format!("打开 chatlog 数据库失败: {}", e))?;

    let result: Result<Option<String>, rusqlite::Error> = conn.query_row(
        "SELECT value FROM ChatInfo WHERE key = 'user_global_id'",
        [],
        |row| row.get(0),
    );

    match result {
        Ok(value) => Ok(value),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("查询 user_global_id 失败: {}", e)),
    }
}

/// 计算 CD 窗口起止时间（毫秒时间戳）
///
/// 25人本：周一 07:00 ~ 下周一 07:00（周 CD）
/// 10人本：周一 07:00 ~ 周五 07:00 或 周五 07:00 ~ 下周一 07:00（半月 CD）
///
/// 用于检查同一 CD 窗口内是否已存在手工记录，避免重复创建 pending。
fn calculate_cd_window(record_time_ms: i64, is_ten_person: bool) -> (i64, i64) {
    use chrono::{Datelike, Local, TimeZone, Timelike};

    let dt = match Local.timestamp_millis_opt(record_time_ms).single() {
        Some(d) => d,
        None => return (0, i64::MAX),
    };

    let day = dt.weekday().num_days_from_monday() as i64; // 0=Mon, 6=Sun
    let hour = dt.hour() as i64;

    // 计算到"本周一 07:00"需要回退的天数
    // 周一 07:00 前（hour < 7）回退到上周一
    let days_to_monday = if day == 0 && hour < 7 {
        7
    } else {
        day
    };

    let monday_date = dt.date_naive() - chrono::Duration::days(days_to_monday);
    let monday_07 = monday_date.and_hms_opt(7, 0, 0).unwrap();
    let monday_07_local = Local.from_local_datetime(&monday_07).unwrap();
    let window_start = monday_07_local.timestamp_millis();

    let next_monday_date = monday_date + chrono::Duration::days(7);
    let next_monday_07 = next_monday_date.and_hms_opt(7, 0, 0).unwrap();
    let next_monday_07_local = Local.from_local_datetime(&next_monday_07).unwrap();
    let next_monday_ms = next_monday_07_local.timestamp_millis();

    if is_ten_person {
        let friday_date = monday_date + chrono::Duration::days(4);
        let friday_07 = friday_date.and_hms_opt(7, 0, 0).unwrap();
        let friday_07_local = Local.from_local_datetime(&friday_07).unwrap();
        let friday_ms = friday_07_local.timestamp_millis();

        if record_time_ms >= window_start && record_time_ms < friday_ms {
            // 上半周：周一 07:00 ~ 周五 07:00
            (window_start, friday_ms)
        } else {
            // 下半周：周五 07:00 ~ 下周一 07:00
            (friday_ms, next_monday_ms)
        }
    } else {
        // 25人本：周一 07:00 ~ 下周一 07:00
        (window_start, next_monday_ms)
    }
}

/// 角色在线判断阈值：chatlog 最新 mtime 在此窗口内视为在线（毫秒）
const ROLE_ONLINE_THRESHOLD_MS: i64 = 5 * 60 * 1000; // 5 分钟

/// 副本"过期"阈值：副本最后一个 JCL 距今超过此值，视为已完成（毫秒）
/// 6 小时足以覆盖绝大多数副本时长（含多次拉托），超过此时间说明副本早已结束
const RAID_STALE_THRESHOLD_MS: i64 = 6 * 60 * 60 * 1000; // 6 小时

/// 判断角色是否在线（基于 chatlog 最新 mtime）
///
/// 用户要求：不能用 combat_logs mtime（挂机不打本时无新 JCL），
/// 改用 chatlog 最新记录时间。若 chatlog 文件 mtime 在最近 5 分钟内，
/// 视为角色仍在线（持续接收聊天消息）；否则视为离线。
///
/// 注意：传入的 `jx3_running` 仅作为辅助判断，主信号是 chatlog mtime。
/// 即使 JX3 进程在运行，若 chatlog 长时间无更新，也认为角色已离线（如切换角色、断线挂机）。
fn is_role_online(account_dir: &Path, jx3_running: bool) -> bool {
    if !jx3_running {
        return false;
    }

    let chat_log_dir = account_dir.join("userdata").join("chat_log");
    if !chat_log_dir.exists() {
        return false;
    }

    // 取 chat_log 目录 mtime 与目录内最新 chatlog_*.v2.db 文件 mtime 的较大值
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let threshold = now_ms - ROLE_ONLINE_THRESHOLD_MS;

    let dir_mtime_ms = std::fs::metadata(&chat_log_dir)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    if dir_mtime_ms >= threshold {
        return true;
    }

    // 目录 mtime 不够新，再检查最新 chatlog 文件 mtime
    let latest_file_mtime_ms = std::fs::read_dir(&chat_log_dir)
        .ok()
        .and_then(|entries| {
            let mut latest: i64 = 0;
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if !name.starts_with("chatlog_") || !name.ends_with(".v2.db") {
                        continue;
                    }
                } else {
                    continue;
                }
                if let Ok(m) = std::fs::metadata(&path).and_then(|m| m.modified()) {
                    if let Ok(d) = m.duration_since(UNIX_EPOCH) {
                        let ms = d.as_millis() as i64;
                        if ms > latest {
                            latest = ms;
                        }
                    }
                }
            }
            if latest > 0 { Some(latest) } else { None }
        })
        .unwrap_or(0);

    latest_file_mtime_ms >= threshold
}

/// 判断副本实例是否已完成扫描
///
/// 完成条件（满足任一即视为完成，记录可转为 pending 供用户确认）：
/// 1. 配置中的 BOSS 全部击杀：bosses_killed 中属于 raid_bosses 配置的 BOSS 数量 >= raid_bosses.len()
///    （只统计配置内的 BOSS，避免把"墨家机侍""狼牙士兵"等非配置 BOSS 的 JCL 计入）
/// 2. 出现底薪结算消息：has_salary = true（聊天记录中检测到"每人底薪：XXX金"）
/// 3. 收入命中 + 存在掉落分配：income_matched = true 且 has_drops = true
///    （无底薪副本已匹配到最后一个 JCL 后的第一条工资收入且已有掉落分配，
///    说明拍卖与结算完成，无需等待进程退出、角色离线或 6 小时兜底）
/// 4. JX3 进程退出：jx3_running = false
/// 5. 角色离线：role_online = false（聊天记录 mtime > 5 分钟未更新）
/// 6. 副本已切换：has_next_jcl = true（存在比当前实例 last_jcl_time 更晚的 JCL 时间戳，
///    说明用户已切换到其他副本，当前副本必然已结束。与 chatlog_end 截断逻辑保持一致）
/// 7. 时间兜底：副本最后一个 JCL 距今超过 RAID_STALE_THRESHOLD_MS（6 小时），
///    无论进程/角色状态如何均视为已完成。避免历史副本因当前 JX3 运行而被误判为"进行中"。
///
/// 由于通关判定改为基于 JCL 的运行时实体证据（参战加终局死亡或专属宝箱），
/// 不再依赖 chatlog 10金，所有条件均可可靠检查。
fn is_raid_complete(
    instance: &RaidInstance,
    raid_bosses: &[(String, String)],
    has_salary: bool,
    jx3_running: bool,
    role_online: bool,
    has_next_jcl: bool,
    income_matched: bool,
    has_drops: bool,
) -> bool {
    // 条件 1：配置中的 BOSS 全部击杀
    // 只统计 bosses_killed 中属于 raid_bosses 配置列表的 BOSS，避免非配置 BOSS（如小怪、特殊 NPC）
    // 的击杀 JCL 被误计入，导致 boss_kill_count 虚高误判副本完成。
    if !raid_bosses.is_empty() {
        let configured_boss_names: std::collections::HashSet<&str> = raid_bosses
            .iter()
            .map(|(_, name)| name.as_str())
            .collect();
        let killed_configured_count = instance
            .bosses_killed
            .iter()
            .filter(|name| configured_boss_names.contains(name.as_str()))
            .count() as u32;
        if killed_configured_count >= raid_bosses.len() as u32 {
            return true;
        }
    }
    // 条件 2：出现底薪结算消息
    if has_salary {
        return true;
    }
    // 条件 3：收入命中 + 存在掉落分配（无底薪副本的结算完成信号）
    // 无底薪副本没有底薪消息、也常因 BOSS 未全清而不满足条件 1；当已匹配到最后一条
    // JCL 后的工资收入且已有掉落分配时，拍卖与结算均已完成，无需再等进程退出或 6 小时兜底。
    if income_matched && has_drops {
        log::info!(
            "[DropScanner] 收入命中+有分配: 副本 '{}' 已匹配收入且存在掉落分配，判定已完成",
            instance.raid_display_name
        );
        return true;
    }
    // 条件 4：JX3 进程退出
    if !jx3_running {
        return true;
    }
    // 条件 5：角色离线
    if !role_online {
        return true;
    }
    // 条件 6：副本已切换 — 存在比当前实例 last_jcl_time 更晚的 JCL 时间戳
    // 说明用户已经切换到其他副本（或开始新副本），当前副本必然已结束。
    // 与 chatlog_end 截断逻辑保持一致：next_jcl_time.is_some() 即视为新副本活动已开始。
    // 解决场景：用户在第一个副本中途退出（未全清、无底薪）后进入第二个副本，
    // JX3 仍在运行、角色仍在线、6 小时兜底未触发，导致第一个副本一直显示"进行中"。
    if has_next_jcl {
        log::info!(
            "[DropScanner] 副本切换: 副本 '{}' 后续存在更晚的 JCL，判定已完成",
            instance.raid_display_name
        );
        return true;
    }
    // 条件 7：时间兜底 — 副本最后一个 JCL 距今超过阈值，视为已完成
    // 防止历史副本因当前 JX3 运行中 + 角色在线而被误判为"进行中"（scanning）
    if instance.last_jcl_time > 0 {
        let now_ms = chrono::Local::now().timestamp_millis();
        let elapsed = now_ms - instance.last_jcl_time;
        if elapsed > RAID_STALE_THRESHOLD_MS {
            log::info!(
                "[DropScanner] 时间兜底: 副本 '{}' 最后JCL距今 {}ms > 阈值 {}ms，判定已完成",
                instance.raid_display_name,
                elapsed,
                RAID_STALE_THRESHOLD_MS
            );
            return true;
        }
    }
    false
}

/// 构造 RaidRecord JSON 并写入 records 表
fn upsert_raid_drop_record(
    conn: &Connection,
    instance: &RaidInstance,
    drops: &[String],
    purchased_items: &HashMap<String, PurchaseInfo>,
    boss_ids: &[String],
    boss_names: &[String],
    role_id: &Option<String>,
    role_name: &str,
    role_server: &str,
    role_region: &str,
    raid_name: &str,
    raid_full_name: &str,
    gold_income: i64,
    gold_expense: i64,
    filtered_jcl_files: &[String],
    record_status: &str,
) -> Result<(), String> {
    let now = chrono::Local::now().to_rfc3339();

    // 检查 records 表是否已存在同账号、同副本难度、同 CD 周期的 auto_scan 记录（任意状态）
    // 注意：raid_name 列存储的是 raids.name 短名（如 "阆风悬城"），不区分 10人/25人/普通/英雄。
    // 仅用 raid_name 匹配会导致 10人记录被 25人实例误匹配（CD 窗口起点相同）。
    // 因此额外用 json_extract(data, '$.raidName') 匹配完整副本名（如 "25人普通阆风悬城"）。
    // 使用 CD 窗口范围匹配：同一 CD 周期内同一副本只保留一条记录。
    // 查询所有状态（含 confirmed/rejected），避免已确认/已拒绝的记录被重复创建。
    // 优先返回 pending/scanning（用于更新），其次 confirmed/rejected（用于跳过）。
    // 使用 raid_full_name 判断人数（含 "10人" 前缀），而非 JCL 原始显示名
    let is_ten_person = raid_full_name.contains("10人");
    let (window_start, window_end) = calculate_cd_window(instance.start_time, is_ten_person);

    // 按角色过滤：不同角色在同一账号下有独立的 CD
    let role_filter = role_id.as_ref().map(|rid| rid.as_str()).unwrap_or("");

    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, status FROM records
             WHERE account_id = ?1 AND raid_name = ?2
               AND record_date >= ?3 AND record_date < ?4
               AND source = 'auto_scan'
               AND json_extract(data, '$.raidName') = ?5
               AND (?6 = '' OR role_id = ?6)
             ORDER BY
               CASE status
                 WHEN 'scanning' THEN 0
                 WHEN 'pending' THEN 1
                 ELSE 2
               END
             LIMIT 1",
            params![instance.account_id, raid_name, window_start, window_end, raid_full_name, role_filter],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .ok();

    // 已有 confirmed 或 rejected 记录时跳过（已处理，不再重复扫描）
    if let Some((_, ref status)) = existing {
        if status == "confirmed" || status == "rejected" {
            log::info!(
                "[DropScanner] CD 窗口内已有 {} 的 auto_scan 记录，跳过: {} (window: {}~{})",
                status,
                instance.raid_display_name,
                window_start,
                window_end
            );
            return Ok(());
        }
    }

    // 无论是否存在 pending auto_scan 记录，都检查同 CD 窗口内是否已有同角色的手工记录。
    // 已有手工记录时跳过 pending 创建/更新，避免重复占用 CD。
    // 注意：之前曾用 `if existing.is_none()` 包裹此检查，导致已存在 pending 记录时
    // 跳过手工检查，即使手工记录已存在也会继续 UPDATE pending，造成重复记录。
    //
    // 注意：手工记录的 raid_name 列存的是 JSON raidName 完整名（如 "25人英雄阆风悬城"），
    // 而 auto 记录的 raid_name 列存的是 raids.name 短名（如 "阆风悬城"）。
    // 因此不能用 raid_name 列精确匹配，需查询同角色同 CD 窗口内所有 confirmed 记录，
    // 在 Rust 中解析 JSON data 字段的 raidName 比较。
    // 按 role_id 过滤：不同角色有独立 CD，同一账号下不同角色的手工记录不应互斥。
    log::info!(
        "[DropScanner] CD 检查: account_id={}, raid_display='{}', start_time={}, is_ten={}, window=[{}, {}]",
        instance.account_id,
        instance.raid_display_name,
        instance.start_time,
        is_ten_person,
        window_start,
        window_end
    );

    let mut stmt = conn
        .prepare(
            "SELECT data FROM records
             WHERE account_id = ?1
               AND record_date >= ?2
               AND record_date < ?3
               AND status = 'confirmed'
               AND (?4 = '' OR role_id = ?4)",
        )
        .map_err(|e| format!("准备手工记录查询失败: {}", e))?;

    let rows = stmt
        .query_map(params![instance.account_id, window_start, window_end, role_filter], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|e| format!("查询手工记录失败: {}", e))?;

    let mut manual_count = 0;
    let mut matched = false;
    for row in rows {
        let data_json = match row {
            Ok(d) => d,
            Err(e) => {
                log::warn!("[DropScanner] 读取手工记录行失败: {}", e);
                continue;
            }
        };
        manual_count += 1;
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data_json) {
            let manual_raid_name = json["raidName"].as_str().unwrap_or("");
            let manual_date = json["date"].as_i64().unwrap_or(0);
            log::info!(
                "[DropScanner] CD 检查: 手工记录 #{}: raidName='{}', date={}, match={}",
                manual_count,
                manual_raid_name,
                manual_date,
                manual_raid_name == raid_full_name
            );
            if manual_raid_name == raid_full_name {
                matched = true;
            }
        }
    }

    if matched {
        // 若存在 pending auto_scan 记录但已有手工记录，删除多余的 pending 记录
        if let Some((existing_id, _)) = &existing {
            conn.execute(
                "DELETE FROM records WHERE id = ?1 AND status = 'pending' AND source = 'auto_scan'",
                params![existing_id],
            )
            .map_err(|e| format!("删除重复 pending 记录失败: {}", e))?;
            log::info!(
                "[DropScanner] CD 窗口内已存在手工记录，删除多余 pending: {} (window: {}~{}, pending_id={})",
                instance.raid_display_name,
                window_start,
                window_end,
                existing_id
            );
        } else {
            log::info!(
                "[DropScanner] CD 窗口内已存在手工记录，跳过 pending: {} (window: {}~{}, 共 {} 条手工记录)",
                instance.raid_display_name,
                window_start,
                window_end,
                manual_count
            );
        }
        return Ok(());
    }

    log::info!(
        "[DropScanner] CD 检查: 未找到匹配手工记录（共 {} 条），将创建/更新 pending",
        manual_count
    );

    // 使用 existing_id（更新）或新生成的 UUID（插入）作为记录 ID。
    let record_id = existing.clone().map(|(id, _)| id).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // 掉落物分类（单表 drop_items + 批量 item_merged 接口）
    // 流程：drop/v2 获取副本可能掉落 → 与实际掉落取交集 → 缓存过滤 → 仅查未命中物品 → 纯字段分类
    // 网络请求失败不阻塞主流程，仅记录警告，降级为 unknown
    let mut has_xuanjing = false;
    let mut has_mount = false;
    let mut has_secret_book = false;
    let mut has_pet = false;
    let mut has_maju = false;
    let mut has_pendant = false;
    let mut has_appearance = false;
    let mut has_title = false;
    // 分类结果（name -> category），用于构建 notes
    let mut item_categories: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    if instance.map_id > 0 {
        let drop_names: Vec<String> = drops.to_vec();

        match crate::drop_table::classify_drops_sync(
            instance.map_id,
            Some(raid_name),
            &drop_names,
        ) {
            Ok(categories) => {
                log::info!(
                    "[DropScanner] 副本 {} 掉落分类完成: MapID={}, 实际掉落 {} 条, 分类命中 {} 条",
                    instance.raid_display_name,
                    instance.map_id,
                    drop_names.len(),
                    categories.len()
                );

                for (name, category) in &categories {
                    item_categories.insert(name.clone(), category.clone());
                    match category.as_str() {
                        "xuanjing" => has_xuanjing = true,
                        "mount" => has_mount = true,
                        "secret_book" => has_secret_book = true,
                        "pet" => has_pet = true,
                        "maju" => has_maju = true,
                        "pendant" => has_pendant = true,
                        "appearance" => has_appearance = true,
                        "title" => has_title = true,
                        _ => {}
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "[DropScanner] 副本 {} 掉落分类失败，降级为 unknown: MapID={}, 错误={}",
                    instance.raid_display_name,
                    instance.map_id,
                    e
                );
            }
        }
    } else {
        log::debug!(
            "[DropScanner] 副本 {} 无 MapID，跳过掉落分类",
            instance.raid_display_name
        );
    }

    // 构建 notes：特殊掉落名称 + 消费购买的装备
    // 特殊掉落：分类为 xuanjing/mount/secret_book/pet/maju/pendant/appearance/title 的物品
    // 消费购买的装备：purchased_items 中分类为 equipment 的物品
    let special_categories = [
        "xuanjing", "mount", "secret_book", "pet",
        "maju", "pendant", "appearance", "title",
    ];
    let mut special_drops: Vec<&str> = Vec::new();
    let mut purchased_equipment: Vec<&str> = Vec::new();

    for (name, category) in &item_categories {
        if special_categories.contains(&category.as_str()) {
            special_drops.push(name.as_str());
        }
    }
    for name in purchased_items.keys() {
        if let Some(cat) = item_categories.get(name) {
            if cat == "equipment" {
                purchased_equipment.push(name.as_str());
            }
        }
    }
    special_drops.sort();
    purchased_equipment.sort();

    // 计算散件清单 + 估价 + 散件支出（白名单材料 + 0 金装备，详见 compute_scraps_items）
    let (scraps_items, scraps_value, scraps_expense) = compute_scraps_items(conn, purchased_items, &item_categories, role_server);
    if !scraps_items.is_empty() {
        log::info!(
            "[DropScanner] 副本 {} 散件清单: {} 项, 估价 {} 金, 散件支出 {} 金（isScrapsBoss 默认 false，待用户在确认弹窗勾选）",
            instance.raid_display_name,
            scraps_items.len(),
            scraps_value,
            scraps_expense
        );
    }

    let notes = {
        let mut parts: Vec<String> = Vec::new();
        if !special_drops.is_empty() {
            parts.push(format!("特殊掉落: {}", special_drops.join("、")));
        }
        if !purchased_equipment.is_empty() {
            parts.push(format!("购买装备: {}", purchased_equipment.join("、")));
        }
        if parts.is_empty() {
            format!("自动扫描记录 - {}", now)
        } else {
            parts.join(" | ")
        }
    };

    // dropTable 字段保留为 null（新设计不再使用旧的理论掉落表结构）
    let drop_table: Option<crate::drop_table::DropItem> = None;

    // 构造 RaidRecord JSON
    // goldIncome 使用 chatlog 提取的分配金额作为默认值，用户确认时可在 UI 编辑。
    let record = serde_json::json!({
        "id": record_id,
        "accountId": instance.account_id,
        "roleId": role_id,
        "roleName": role_name,
        "server": role_server,
        "region": role_region,
        "raidName": raid_full_name,
        "mapId": instance.map_id,
        "date": instance.start_time,
        "goldIncome": gold_income,
        "goldExpense": gold_expense,
        "hasXuanjing": has_xuanjing,
        "hasMount": has_mount,
        "hasSecretBook": has_secret_book,
        "hasPet": has_pet,
        "hasMaJu": has_maju,
        "hasPendant": has_pendant,
        "hasAppearance": has_appearance,
        "hasTitle": has_title,
        "bossIds": boss_ids,
        "bossNames": boss_names,
        "type": "raid",
        "source": "auto",
        "status": record_status,
        "drops": drops,
        "dropTable": drop_table,
        "jclFiles": filtered_jcl_files,
        "scrapsItems": scraps_items,
        "scrapsValue": scraps_value,
        "scrapsExpense": scraps_expense,
        "isScrapsBoss": false,
        "notes": notes,
    });

    let record_str = record.to_string();
    let drops_json = serde_json::to_string(drops).unwrap_or_else(|_| "[]".to_string());
    let jcl_files_json = serde_json::to_string(filtered_jcl_files)
        .unwrap_or_else(|_| "[]".to_string());

    if let Some((existing_id, _)) = existing {
        // pending/scanning 状态，覆盖更新（含 status 字段）
        conn.execute(
            "UPDATE records SET
                data = ?1, drops = ?2, jcl_files = ?3, status = ?4, updated_at = ?5
             WHERE id = ?6",
            params![record_str, drops_json, jcl_files_json, record_status, now, existing_id],
        )
        .map_err(|e| format!("更新待确认记录失败: {}", e))?;

        log::info!(
            "[DropScanner] 更新待确认记录 [{}]: {} - {}",
            record_status,
            instance.raid_display_name,
            instance.start_time
        );
    } else {
        // 插入新记录
        conn.execute(
            "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type, source, status, drops, jcl_files, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'auto_scan', ?8, ?9, ?10, ?11, ?12)",
            params![
                record_id,
                record_str,
                raid_name,
                instance.account_id,
                role_id,
                instance.start_time,
                "raid",
                record_status,
                drops_json,
                jcl_files_json,
                now,
                now
            ],
        )
        .map_err(|e| format!("插入待确认记录失败: {}", e))?;

        log::info!(
            "[DropScanner] 新增 pending 记录: {} - {}",
            instance.raid_display_name,
            instance.start_time
        );
    }

    Ok(())
}

/// 扫描指定账号的掉落记录
///
/// 新流程（基于10金匹配 + 增量状态）：
/// 1. 扫描 combat_logs/*.jcl
/// 2. 查询 chatlog 获取所有10金记录
/// 3. 聚类为副本实例（带10金匹配，只计入识别到的击杀）
/// 4. 对每个副本实例，提取 chatlog 物品分配和金币收入
/// 5. 写入 records 表（source='auto_scan'）：
///    - 副本未完成（BOSS未全击杀 / 无底薪 / 进程在跑 / 角色在线）→ status='scanning'，UI 锁定不可确认
///    - 副本已完成（BOSS全击杀 / 有底薪 / 进程退出 / 角色离线）→ status='pending'，UI 可确认
///
/// 参数：
/// - `account_id`: 茗伊账号 UID
/// - `jx3_running`: JX3 进程是否运行（影响副本完成判断）
/// - `role_online`: 角色是否在线（基于 chatlog mtime，影响副本完成判断）
/// - `process_start_ms`: JX3 进程启动时间（用于 mtime 下限过滤，0 表示不过滤）
/// - `start_ms`: 副本时间下限（基于 JCL 文件名解析的副本开始时间，0 表示不过滤）
/// - `end_ms`: 副本时间上限（开区间，0 表示不过滤）
pub fn scan_raid_drops_internal(
    account_id: &str,
    jx3_running: bool,
    role_online: bool,
    process_start_ms: i64,
    start_ms: i64,
    end_ms: i64,
) -> Result<usize, String> {
    scan_raid_drops_with_raids(account_id, jx3_running, role_online, process_start_ms, start_ms, end_ms, None, None)
}

/// scan_raid_drops_internal 的扩展版本，支持传入预加载的副本配置
///
/// `pre_loaded_raids`: 若 Some，则跳过 init_db + get_cached_raids，直接使用传入的配置
///
/// `known_next_jcl_time`: 调用方已知的"下一条 JCL"时间戳（复核路径传入）。
/// 复核路径的 `end_ms` 就等于该值，而 `scan_jcl_files` 的上界是开区间
/// （`timestamp >= end_ms` 即丢弃），因此这条 JCL 必然不在本次扫描的
/// `all_jcl_timestamps` 中。若不显式传入，`has_next_jcl` 会恒为 false（条件 6 失效），
/// 且 `chatlog_end` 会回退到 `now_ms` 而把后一副本的结算事件并入当前副本。
/// 批量扫描路径的区间覆盖整个 CD 窗口，可自行观察到后续 JCL，传 None 即可。
pub fn scan_raid_drops_with_raids(
    account_id: &str,
    jx3_running: bool,
    role_online: bool,
    process_start_ms: i64,
    start_ms: i64,
    end_ms: i64,
    pre_loaded_raids: Option<&[RaidEntry]>,
    known_next_jcl_time: Option<i64>,
) -> Result<usize, String> {
    log::info!(
        "[DropScanner] 开始扫描账号 {} 的掉落记录 (jx3_running={}, role_online={}, process_start_ms={}, start_ms={}, end_ms={}, known_next_jcl={:?})",
        account_id, jx3_running, role_online, process_start_ms, start_ms, end_ms, known_next_jcl_time
    );

    let game_dir = get_game_directory()?;
    let game_path = PathBuf::from(&game_dir);
    let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);

    // 查找账号目录
    let account_dir_name = format!("{}@zhcn_hd", account_id);
    let account_dir = accounts_base.join(&account_dir_name);

    if !account_dir.exists() {
        return Err(format!("账号目录不存在: {}", account_dir.display()));
    }

    // 0. 加载副本配置（用于在聚类前过滤未配置的副本，避免无谓的10金匹配和"拉托"日志）
    let raids: Vec<RaidEntry> = if let Some(pre_loaded) = pre_loaded_raids {
        // 使用预加载的配置，跳过 init_db + get_cached_raids
        log::debug!(
            "[DropScanner] 账号 {} 使用预加载的 {} 个副本配置",
            account_id,
            pre_loaded.len()
        );
        pre_loaded.to_vec()
    } else {
        let conn = match db::init_db() {
            Ok(c) => c,
            Err(e) => {
                log::error!("[DropScanner] 账号 {} 初始化数据库失败: {}", account_id, e);
                return Err(e);
            }
        };
        match get_cached_raids(&conn) {
            Ok(r) => {
                log::info!(
                    "[DropScanner] 账号 {} 加载 {} 个副本配置",
                    account_id,
                    r.len()
                );
                r
            }
            Err(e) => {
                log::error!("[DropScanner] 账号 {} 加载副本信息失败: {}", account_id, e);
                return Err(e);
            }
        }
    };

    // 1. 扫描 JCL 文件（只扫描进程启动后的文件）
    let jcl_files = scan_jcl_files(&account_dir, process_start_ms, start_ms, end_ms)?;
    if jcl_files.is_empty() {
        log::info!("[DropScanner] 账号 {} 本次会话无 JCL 文件", account_id);
        return Ok(0);
    }

    log::info!(
        "[DropScanner] 账号 {} 扫描到 {} 个 JCL 文件",
        account_id,
        jcl_files.len()
    );

    // 1.5 过滤未配置的副本（如5人副本、试炼之地等不在 raids 表中的副本）
    //     在聚类前过滤，避免对未配置副本做10金匹配和"拉托"判定
    let mut skipped_unconfigured: Vec<String> = Vec::new();
    let configured_jcl_files: Vec<JclFileInfo> = jcl_files
        .iter()
        .filter(|jcl| {
            if match_raid_name(&jcl.raid_display_name, &raids).is_some() {
                true
            } else {
                skipped_unconfigured.push(jcl.raid_display_name.clone());
                false
            }
        })
        .cloned()
        .collect();

    if !skipped_unconfigured.is_empty() {
        let unique_names: HashSet<&str> = skipped_unconfigured.iter().map(|s| s.as_str()).collect();
        log::info!(
            "[DropScanner] 账号 {} 跳过 {} 个未配置副本的 JCL: {}",
            account_id,
            skipped_unconfigured.len(),
            unique_names.into_iter().collect::<Vec<_>>().join(", ")
        );
    }

    if configured_jcl_files.is_empty() {
        log::info!("[DropScanner] 账号 {} 过滤后无已配置副本的 JCL 文件", account_id);
        return Ok(0);
    }

    // 如果 JX3 进程在运行且本次会话有已配置副本的 JCL 文件产生，说明角色正在打本，
    // 强制判定为在线。chatlog mtime 可能滞后（茗伊插件不会实时写入磁盘），但 JCL 文件
    // 的产生是角色正在战斗的可靠信号。
    let forced_online = jx3_running && !role_online;
    let role_online = role_online || jx3_running;
    if forced_online {
        log::info!(
            "[DropScanner] 账号 {} 检测到本次会话 JCL 文件，强制判定角色在线（忽略 chatlog mtime）",
            account_id
        );
    }

    // 2. 查找 chatlog 文件
    let chatlog_files = find_chatlog_files(&account_dir)?;

    // 预缓存所有 chatlog 文件的时间范围，避免后续每个副本实例重复打开 SQLite
    let chatlog_range_cache: HashMap<PathBuf, Option<(i64, i64)>> = chatlog_files
        .iter()
        .map(|path| (path.clone(), get_chatlog_time_range(path)))
        .collect();

    // 2.5 预加载所有 chatlog 文件中的相关记录到内存
    //     避免后续每个副本实例重复打开 SQLite 连接和全表扫描
    //     性能优化：将 O(N*M) 次 SQLite 查询降为 O(M) 次（M=chatlog文件数）
    let preloaded_records = preload_chatlog_records(
        &chatlog_files,
        &chatlog_range_cache,
        start_ms,
        end_ms,
    );

    // 3. 分析每个 JCL 文件内容，提取战斗时间和击杀状态
    //    替代原 10金查询方案：通过 JCL 的运行时实体证据判定通关
    //    每个 JCL 只解析一次，结果缓存到 jcl_analyses
    //    同时使用 SQLite 持久化缓存（jcl_cache 表），跨会话复用解析结果
    let conn = match db::init_db() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[DropScanner] 账号 {} 初始化数据库失败: {}", account_id, e);
            return Err(e);
        }
    };
    let combat_logs_dir = account_dir.join("userdata").join("combat_logs");
    let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
    let mut damage_candidates: HashMap<String, DamageCompletionCandidate> = HashMap::new();
    for jcl in &configured_jcl_files {
        // 查找 JCL 对应的 raid_entry，获取 raid_bosses 配置
        let raid_entry = match match_raid_name(&jcl.raid_display_name, &raids) {
            Some(entry) => entry,
            None => continue, // 已过滤，理论上必匹配
        };
        let raid_bosses = &raid_entry.bosses;
        let jcl_path = combat_logs_dir.join(&jcl.file_name);
        let mut analysis = analyze_jcl_cached(&conn, &jcl_path, &jcl.boss_name, jcl.boss_id, raid_bosses).unwrap_or(JclAnalysis {
            boss_name: None,
            fight_start_ms: 0,
            fight_end_ms: 0,
            is_kill: false,
        });
        // BOSS 名优先级：文件名 BOSS 在 raid_bosses 配置中时使用文件名，
        // 否则使用 JCL 内容搜索结果（处理"须罗巨傀"→"唐怀仁"等小怪 JCL 情况）
        if is_valid_boss(&jcl.boss_name, raid_bosses) {
            analysis.boss_name = Some(jcl.boss_name.clone());
        }
        log::info!(
            "[DropScanner] JCL 分析: 文件='{}', BOSS='{}', is_kill={}, fight_start={}, fight_end={}, content_boss={:?}",
            jcl.file_name,
            jcl.boss_name,
            analysis.is_kill,
            analysis.fight_start_ms,
            analysis.fight_end_ms,
            analysis.boss_name
        );
        // 强证据已足够时不需要金币；否则只为满足完整通用门槛的直接配置首领建立候选。
        // 候选独立重读文件，确保 SQLite 缓存命中也不会丢失本轮补漏所需的 type=21 证据。
        if !analysis.is_kill {
            if let Some(candidate) = analyze_damage_completion_candidate(
                &jcl_path,
                &jcl.boss_name,
                jcl.boss_id,
                raid_bosses,
                account_id,
            ) {
                damage_candidates.insert(jcl.file_name.clone(), candidate);
            }
        }
        jcl_analyses.insert(jcl.file_name.clone(), analysis);
    }

    // 同角色、精确 10 金只补强完整伤害候选。这里的结果仅用于本次聚类；JCL 强证据
    // `is_kill`、SQLite 缓存和后续财务收入统计都不被改写。
    let (verified_reward_times, promoted_files) = if damage_candidates.is_empty() {
        (Vec::new(), HashSet::new())
    } else {
        let candidate_values: Vec<DamageCompletionCandidate> = damage_candidates.values().cloned().collect();
        let verified_reward_times = read_role_verified_ten_gold_times(&chatlog_files, account_id, &candidate_values);
        let mut attempt_end_times = Vec::new();
        let mut anchors_complete = true;
        for jcl in &jcl_files {
            match collect_type1_end_anchors(&combat_logs_dir.join(&jcl.file_name), account_id) {
                Some(ends) => attempt_end_times.extend(ends),
                None => {
                    anchors_complete = false;
                    log::warn!("[DropScanner] 10金补漏停用：无法完整读取尝试结束锚 '{}'", jcl.file_name);
                }
            }
        }
        let promoted = if anchors_complete {
            select_damage_money_promotions(&damage_candidates, &attempt_end_times, &verified_reward_times)
        } else {
            HashSet::new()
        };
        (verified_reward_times, promoted)
    };
    let mut effective_jcl_analyses = jcl_analyses.clone();
    for file_name in &promoted_files {
        if let (Some(candidate), Some(effective)) = (damage_candidates.get(file_name), effective_jcl_analyses.get_mut(file_name)) {
            effective.is_kill = true;
            log::info!(
                "[DropScanner] 10金补强击杀: 文件='{}', BOSS='{}', end={}, window={}s",
                file_name,
                candidate.boss_name,
                candidate.fight_end_ms,
                DAMAGE_MONEY_MATCH_WINDOW_SECS
            );
        }
    }
    if !damage_candidates.is_empty() {
        log::info!(
            "[DropScanner] 伤害+10金补漏: 候选={}，角色校验10金={}，补强={}",
            damage_candidates.len(),
            verified_reward_times.len(),
            promoted_files.len()
        );
    }

    // 4. 聚类（按 JCL 顺序，使用 is_kill 筛选已确认击杀，2小时阈值智能分组）
    //    使用预加载的 chatlog 记录进行工资检测，避免重复打开 SQLite
    //    预提取所有已配置 JCL 的时间戳（排序），用于后续 chatlog_end 截断：
    //    不管下一个副本是否通关，只要有 JCL 产生就说明新副本活动已开始，应截断 chatlog
    let all_jcl_timestamps: Vec<i64> = {
        let mut ts: Vec<i64> = configured_jcl_files.iter().map(|j| j.timestamp).collect();
        ts.sort_unstable();
        ts
    };
    let mut instances = cluster_raid_instances(
        account_id,
        configured_jcl_files,
        &effective_jcl_analyses,
        &preloaded_records,
    );
    log::info!(
        "[DropScanner] 账号 {} 聚类为 {} 个副本实例",
        account_id,
        instances.len()
    );

    // 6. 解析 info.jx3dat 获取角色身份，映射到数据库 UUID
    //    records 表的 account_id/role_id 是 UUID 格式（与 accounts/roles 表对齐），
    //    不能使用茗伊目录名的数字 uid，否则 UI 过滤不到 pending 记录。
    //    conn 已在 JCL 分析阶段创建，此处直接复用

    let db_identity = match resolve_db_role_identity(&conn, &account_dir, &game_path) {
        Ok(Some(id)) => id,
        Ok(None) => {
            log::warn!(
                "[DropScanner] 账号 {} 角色未在数据库登记，跳过扫描",
                account_id
            );
            return Ok(0);
        }
        Err(e) => {
            log::error!("[DropScanner] 账号 {} 解析角色身份失败: {}", account_id, e);
            return Err(e);
        }
    };

    // 用 UUID 覆盖每个实例的 account_id（聚类时用的是数字 uid，仅用于日志）
    for instance in instances.iter_mut() {
        instance.account_id = db_identity.account_id.clone();
    }

    // 7. 处理每个副本实例
    let mut inserted_count = 0;

    for instance in instances.iter_mut() {
        // 匹配 raids.name
        let raid_entry = match match_raid_name(&instance.raid_display_name, &raids) {
            Some(entry) => entry,
            None => {
                log::warn!(
                    "[DropScanner] 副本名未匹配: {}",
                    instance.raid_display_name
                );
                continue;
            }
        };
        // 当 JCL 文件名不含人数/难度时（如 "阆风悬城(793)"），match_raid_name 已按
        // 人数/难度优先级匹配到正确副本（如 10人普通阆风悬城）。用匹配到的完整副本名
        // （raid_entry.raid_id，格式 "{playerCount}人{difficulty}{name}"）覆盖原始 JCL 显示名，
        // 确保：
        // 1. records.data.raidName 与手工记录格式一致（手工记录用 constructRaidName 生成完整名）
        // 2. is_ten_person 判断（contains("10人")）正确选择 10/25 人 CD 周期
        // 3. 前端 RaidDetail/RoleRecordsModal 按 playerCount/difficulty 过滤时能命中
        instance.raid_display_name = raid_entry.raid_id.clone();
        let raid_name = &raid_entry.name;
        let raid_bosses = &raid_entry.bosses;

        log::info!(
            "[DropScanner] BOSS 匹配调试: raid_display='{}', matched_raid_id='{}', matched_raid_name='{}', raid_bosses_count={}, jcl_bosses_count={}, boss_kill_count={}",
            instance.raid_display_name,
            raid_entry.raid_id,
            raid_entry.name,
            raid_bosses.len(),
            instance.bosses_killed.len(),
            instance.boss_kill_count
        );
        if !raid_bosses.is_empty() {
            log::info!(
                "[DropScanner] 配置 BOSS: {:?}",
                raid_bosses.iter().map(|(id, name)| format!("{}={}", id, name)).collect::<Vec<_>>()
            );
        }
        if !instance.bosses_killed.is_empty() {
            log::info!(
                "[DropScanner] JCL BOSS: {:?}",
                instance.bosses_killed
            );
        }

        // BOSS 识别：将 jcl_boss_names 映射到 raid_bosses 表的 BOSS ID
        // （JCL 内容解析已在聚类前完成，boss_name 已是最佳值）
        let (boss_ids, boss_names) =
            map_boss_names_to_ids(&instance.bosses_killed, &raid_bosses);

        log::info!(
            "[DropScanner] BOSS 映射结果: boss_ids={:?}, boss_names={:?}",
            boss_ids,
            boss_names
        );

        // 聊天记录分析范围：
        // 开始 = instance.first_gold_time（首个BOSS击杀时间）
        // 结束 = 下一个 JCL 产生时间（不管是否通关），或（最后一个 JCL）副本结束后 2 小时
        // （底薪通常在副本结束后30-60分钟内发送，2小时窗口足够覆盖）
        // 注意：用 JCL 时间戳而非实例 start_time，避免未通关副本（如白帝江关无 BOSS 配置）
        //       虽然不形成实例但其 JCL 仍能正确截断 chatlog 窗口
        let chatlog_start = instance.first_gold_time;
        // 下一个 JCL 时间戳（副本切换信号，条件 6 与 chatlog 窗口截断都依赖它）
        // 详见 resolve_next_jcl_time 的说明：复核路径必须回退到调用方已知的切换点
        let next_jcl_time = resolve_next_jcl_time(
            &all_jcl_timestamps,
            instance.last_jcl_time,
            known_next_jcl_time,
        );

        let chatlog_end = if let Some(next_ts) = next_jcl_time {
            // 有下一个 JCL → 以切换点为窗口上界（右开）。
            // 这同时保证相邻两个副本实例的 chatlog 窗口不重叠：切换点之后的拍卖/底薪
            // 只可能属于后一副本，不会被并入当前副本，避免同一份结算被写入两条记录
            // （表现为两条记录 drops 完全相同、goldIncome 重复计入）。
            next_ts
        } else {
            // 没有后续 JCL：根据副本是否仍在进行中选择结束时间
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let raid_stale = instance.last_jcl_time > 0
                && now_ms - instance.last_jcl_time > RAID_STALE_THRESHOLD_MS;
            if jx3_running && role_online && !raid_stale {
                // 副本可能仍在进行（JX3运行 + 角色在线 + 最后JCL在6小时内）
                // 使用当前时间以覆盖实时底薪发送
                now_ms
            } else {
                // 副本已结束：使用 last_jcl_time + 2h 作为结束时间
                // 避免使用 SystemTime::now() 导致纳入后续家园活动等无关收入
                instance.last_jcl_time + 2 * 60 * 60 * 1000
            }
        };

        // 从 chatlog 提取掉落物、金币收入和支出
        // 优化：直接复用预加载的 chatlog 记录进行内存处理，避免重复打开 SQLite 连接
        // （预加载在 scan_raid_drops_with_raids 入口完成，覆盖范围 [scan_start, scan_end+2h]）
        let mut drops: HashSet<String> = HashSet::new();
        let mut base_salary: Option<i64> = None;
        let other_income: i64;
        let total_expense: i64;
        // 合并所有 chatlog 文件的收入记录 (time_sec, gold)，用于精确收入匹配
        let mut all_income_records: Vec<(i64, i64)> = Vec::new();
        // 累加所有 chatlog 文件的 boss_kill_times，用于击杀数验证
        let mut all_boss_kill_times: Vec<i64> = Vec::new();
        // 合并所有 chatlog 文件的购买物品映射（物品名 → PurchaseInfo，用于散件老板功能）
        let mut all_purchased_items: HashMap<String, PurchaseInfo> = HashMap::new();

        // 单次调用处理预加载记录，时间范围 [chatlog_start, chatlog_end] 通过二分查找定位
        let (chatlog_drops, salary, income, expense, _, income_records, boss_kill_times, purchased_items) =
            extract_drops_from_records(&preloaded_records, chatlog_start, chatlog_end, &db_identity.role_name);
        drops.extend(chatlog_drops);
        // 合并购买物品映射：同物品累加 count 和 total_price
        for (name, info) in purchased_items {
            let entry = all_purchased_items.entry(name).or_default();
            entry.count += info.count;
            entry.total_price += info.total_price;
        }
        if salary.is_some() {
            base_salary = salary;
        }
        other_income = income;
        total_expense = expense;
        all_income_records.extend(income_records);
        all_boss_kill_times.extend(boss_kill_times);
        log::info!(
            "[DropScanner] chatlog 内存处理: 预加载 {} 条记录, 查询 1 次",
            preloaded_records.len()
        );

        // 10 金不再反向裁剪 JCL 强证据。它只在聚类前与同角色、完整有效伤害候选
        // 联合使用；这里保留诊断计数，避免漏盘、跨文件或非击杀的同额收入删掉真击杀。
        let boss_all_killed = !raid_bosses.is_empty() && {
            let configured_names: std::collections::HashSet<&str> = raid_bosses
                .iter()
                .map(|(_, name)| name.as_str())
                .collect();
            let killed_configured = instance
                .bosses_killed
                .iter()
                .filter(|name| configured_names.contains(name.as_str()))
                .count() as u32;
            killed_configured >= raid_bosses.len() as u32
        };

        log::info!(
            "[DropScanner] 副本 {} 击杀证据: JCL有效={}, chatlog 10金诊断={}，不以10金反向裁剪",
            instance.raid_display_name,
            instance.boss_kill_count,
            all_boss_kill_times.len()
        );

        // 最终收入计算：
        // - 有底薪：在收入记录中找金额 >= 底薪值的第一条记录，用该金额作为收入
        //   （团长发放底薪时的"你获得：XXX金"记录，金额通常等于或略大于底薪）
        // - 无底薪：取最后一个 JCL 之后的第一条非 BOSS 10金收入记录作为收入
        //   （拍卖工资发放只发一次，即为最后一个 JCL 之后的第一条收入记录）
        // - 找不到匹配记录时：有底薪回退到底薪值，无底薪收入记为 0（不虚高）
        // 返回 (total_gold, income_matched)：income_matched 表示是否命中真实收入记录，
        // 供副本完成判定使用（无底薪场景"收入命中 + 有掉落分配"即视为结算完成）
        all_income_records.sort_by_key(|(t, _)| *t);
        let last_jcl_sec = instance.last_jcl_time / 1000;
        let (total_gold, income_matched) = if let Some(salary) = base_salary {
            // 有底薪：找金额 >= 底薪的第一条收入记录
            match all_income_records.iter().find(|(_, g)| *g >= salary) {
                Some((t, g)) => {
                    log::info!(
                        "[DropScanner] 收入匹配(底薪): 底薪={}, 匹配记录 gold={} time={} -> 收入={}",
                        salary, g, t, g
                    );
                    (*g, true)
                }
                None => {
                    log::info!(
                        "[DropScanner] 收入匹配(底薪): 底薪={}, 无匹配记录, 回退到底薪值 -> 收入={}",
                        salary, salary
                    );
                    (salary, false)
                }
            }
        } else {
            // 无底薪：取最后一个 JCL 之后的第一条收入记录（拍卖工资只发一次）
            match all_income_records.iter().find(|(t, _)| *t > last_jcl_sec) {
                Some((t, g)) => {
                    log::info!(
                        "[DropScanner] 收入匹配(无底薪): last_jcl_sec={}, 取 last_jcl 后第一条收入 gold={} time={} -> 收入={}",
                        last_jcl_sec, g, t, g
                    );
                    (*g, true)
                }
                None => {
                    log::info!(
                        "[DropScanner] 收入匹配(无底薪): last_jcl_sec={}, 无后续收入记录, 收入记为 0 (收入总和 {} 条记录不再回退)",
                        last_jcl_sec, all_income_records.len()
                    );
                    (0, false)
                }
            }
        };
        log::info!(
            "[DropScanner] 收入汇总: 底薪={:?}, 其他收入总和={}, 最终收入={}, 支出={}, 收入记录数={}",
            base_salary,
            other_income,
            total_gold,
            total_expense,
            all_income_records.len()
        );

        // 过滤小怪 JCL 文件：只保留 BOSS 名在 raid_bosses 表中的 JCL 文件
        // （如"墨家机侍"、"狼牙士兵"、"须罗巨傀"等小怪 JCL 不出现在 jclFiles 中）
        let filtered_jcl_files: Vec<String> = instance
            .jcl_files
            .iter()
            .zip(instance.jcl_boss_names.iter())
            .filter(|(_, boss_name)| is_valid_boss(boss_name, raid_bosses))
            .map(|(file, _)| file.clone())
            .collect();

        log::info!(
            "[DropScanner] JCL 过滤: 原始 {} 个, 过滤后 {} 个",
            instance.jcl_files.len(),
            filtered_jcl_files.len()
        );

        // 写入 records 表（使用数据库 UUID，而非茗伊数字 uid）
        // 副本完成判断：根据 BOSS 击杀数、底薪、进程状态、角色在线状态决定 status
        // - scanning：副本进行中，UI 锁定不可确认，等待后续扫描补充
        // - pending：副本已完成，UI 可确认
        let raid_complete = is_raid_complete(
            instance,
            raid_bosses,
            base_salary.is_some(),
            jx3_running,
            role_online,
            next_jcl_time.is_some(),
            income_matched,
            !drops.is_empty(),
        );
        let record_status = if raid_complete { "pending" } else { "scanning" };

        log::info!(
            "[DropScanner] 副本完成判断: raid='{}', boss_kill={}/{}, boss_all_killed={}, has_salary={}, jx3_running={}, role_online={}, has_next_jcl={}, income_matched={}, has_drops={} -> status='{}'",
            instance.raid_display_name,
            instance.boss_kill_count,
            raid_bosses.len(),
            boss_all_killed,
            base_salary.is_some(),
            jx3_running,
            role_online,
            next_jcl_time.is_some(),
            income_matched,
            !drops.is_empty(),
            record_status
        );

        let role_id_opt: Option<String> = Some(db_identity.role_id.clone());
        let drops_vec: Vec<String> = drops.into_iter().collect();

        if let Err(e) = upsert_raid_drop_record(
            &conn,
            instance,
            &drops_vec,
            &all_purchased_items,
            &boss_ids,
            &boss_names,
            &role_id_opt,
            &db_identity.role_name,
            &db_identity.server,
            &db_identity.region,
            raid_name,
            &instance.raid_display_name,
            total_gold,
            total_expense,
            &filtered_jcl_files,
            record_status,
        ) {
            log::error!(
                "[DropScanner] 写入记录失败: {} - {}",
                instance.raid_display_name,
                e
            );
            continue;
        }

        inserted_count += 1;
    }

    log::info!(
        "[DropScanner] 账号 {} 扫描完成，处理 {} 个副本实例",
        account_id,
        inserted_count
    );

    Ok(inserted_count)
}

/// Tauri 命令：扫描指定账号的掉落记录
///
/// 使用 async + spawn_blocking 将耗时的文件扫描移到后台线程，
/// 避免阻塞 Tauri 主线程导致 UI 卡死。
///
/// 自动获取 JX3 进程状态和角色在线状态（基于 chatlog mtime），
/// 传递给 `scan_raid_drops_internal` 用于副本完成判断。
#[tauri::command]
pub async fn scan_raid_drops(account_id: String) -> Result<usize, String> {
    tokio::task::spawn_blocking(move || {
        let game_dir = get_game_directory()?;
        let game_path = PathBuf::from(&game_dir);
        let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);
        let account_dir_name = format!("{}@zhcn_hd", account_id);
        let account_dir = accounts_base.join(&account_dir_name);

        // 获取 JX3 进程状态
        let runtime_status = crate::jx3_process::get_jx3_runtime_status_internal(&game_dir);
        let jx3_running = runtime_status.is_running;

        // 获取进程启动时间（毫秒），用于过滤 JCL 文件
        let process_start_ms = runtime_status
            .matched_process
            .as_ref()
            .map(|p| (p.start_time_unix as i64) * 1000)
            .unwrap_or(0);

        // 基于 chatlog mtime 判断角色在线状态
        let role_online = is_role_online(&account_dir, jx3_running);

        scan_raid_drops_internal(&account_id, jx3_running, role_online, process_start_ms, 0, 0)
    })
    .await
    .map_err(|e| format!("扫描线程异常: {}", e))?
}

/// 单个账号的扫描结果（用于批量扫描返回值）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountScanResult {
    /// 茗伊账号 UID
    pub account_id: String,
    /// 是否扫描成功
    pub success: bool,
    /// 扫描到的副本实例数量（成功时）
    pub instance_count: Option<usize>,
    /// 失败原因（失败时）
    pub error: Option<String>,
}

/// 已扫描的 Recent 角色 UID 集合（内存态，每次应用启动重置）
///
/// Recent 角色表示本次会话登录过但已切换走/下线，不会再产生新 JCL，
/// 因此只需扫描一次。Active 角色可能正在打本，需要每次轮询都扫描。
///
/// 当 Recent 角色重新变为 Active（玩家切回该角色）时，从集合中移除，
/// 这样下次变为 Recent 时会重新扫描一次。
static SCANNED_RECENT_UIDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn scanned_recent_uids() -> &'static Mutex<HashSet<String>> {
    SCANNED_RECENT_UIDS.get_or_init(|| Mutex::new(HashSet::new()))
}

/// 批量扫描所有活跃角色的掉落记录
///
/// 使用 `active_detector::detect_accounts_active_internal` 的活跃检测结果，
/// 只扫描本次 JX3 会话期间登录过的角色（`is_recently_active()` = Active 或 Recent），
/// 不扫描历史会话的 Idle 账号。
///
/// 扫描策略：
/// - **Active 角色**：每次轮询都扫描（可能正在打本，会产生新 JCL）
/// - **Recent 角色**：只扫描一次（已切换走，不会产生新 JCL），扫描后记入 `SCANNED_RECENT_UIDS`
///   当 Recent 角色重新变为 Active 时，从集合中移除，下次变为 Recent 时会重新扫描
///
/// 活跃检测信号（与前端 useActivePoller 一致）：
/// - info.jx3dat 登录写入
/// - userdata.db 持续更新
/// - chat_log 目录 mtime
/// - gkp 目录 mtime
/// - combat_logs 目录 mtime
///
/// 副本完成判断依赖两个外部信号：
/// - `jx3_running`: 整个批量扫描共用，来自 JX3 进程状态
/// - `role_online`: 每个账号独立计算，基于该账号 chatlog 最新 mtime（5 分钟阈值）
pub fn scan_all_active_raid_drops_internal() -> Result<Vec<(String, Result<usize, String>)>, String> {
    let scan_started_at_outer = std::time::Instant::now();
    let game_dir_outer = get_game_directory()?;

    // 1. 调用 active_detector 获取活跃检测结果
    let active_result_outer = crate::mingyi::active_detector::detect_accounts_active_internal(&game_dir_outer);
    if !active_result_outer.jx3_running {
        return Err("JX3 进程未运行".to_string());
    }

    // cutoff_ms：本次 JX3 启动时间（毫秒）
    // 轮询只在 JX3 运行时触发，jx3_start_time_unix 必然存在
    let cutoff_ms = (active_result_outer.jx3_start_time_unix as i64) * 1000;

    // 预加载副本配置（复核阶段和本次会话扫描共用）
    let preloaded_raids_outer: Option<Vec<RaidEntry>> = match db::init_db() {
        Ok(conn) => match get_cached_raids(&conn) {
            Ok(raids) => {
                log::info!(
                    "[DropScanner] 预加载副本配置完成，共 {} 个副本，将复用于复核和扫描",
                    raids.len()
                );
                Some(raids)
            }
            Err(e) => {
                log::warn!("[DropScanner] 预加载副本配置失败，降级为每账号独立加载: {}", e);
                None
            }
        },
        Err(e) => {
            log::warn!("[DropScanner] 预热数据库连接失败，降级为每账号独立加载: {}", e);
            None
        }
    };

    // ===== 复核阶段：处理历史遗留记录 =====
    let verify_started_at = std::time::Instant::now();
    let verify_result = verify_stale_records(
        cutoff_ms,
        &game_dir_outer,
        preloaded_raids_outer.as_deref(),
        &active_result_outer,
    );
    match &verify_result {
        Ok(stats) => {
            if stats.total > 0 {
                log::info!(
                    "[DropScanner] 复核阶段完成：共 {} 条, 翻转/补充 {} 条, 保留原数据 {} 条, 保留原状 {} 条, 失败 {} 条, 耗时 {}ms",
                    stats.total, stats.flipped, stats.preserved, stats.kept, stats.failed,
                    verify_started_at.elapsed().as_millis()
                );
            }
        }
        Err(e) => {
            log::warn!("[DropScanner] 复核阶段失败，继续本次会话扫描: {}", e);
        }
    }
    // ===== 复核阶段结束 =====

    // 委托给内部实现，避免重复 active_detector 调用
    scan_all_active_raid_drops_internal_impl(
        game_dir_outer,
        active_result_outer,
        preloaded_raids_outer.as_deref(),
        scan_started_at_outer,
    )
}

/// `scan_all_active_raid_drops_internal` 的内部实现
///
/// 接收外部已获取的 active_result 和 preloaded_raids，避免复核阶段重复调用。
fn scan_all_active_raid_drops_internal_impl(
    game_dir: String,
    active_result: crate::mingyi::active_detector::BatchActiveResult,
    preloaded_raids: Option<&[RaidEntry]>,
    scan_started_at: std::time::Instant,
) -> Result<Vec<(String, Result<usize, String>)>, String> {
    let jx3_running = active_result.jx3_running;

    // 从 active_result 获取进程启动时间（用于过滤 JCL 文件，只扫描本次会话产生的）
    // 直接使用 UNIX 时间戳，避免 RFC3339 字符串往返解析导致的 0 值问题
    let process_start_ms = (active_result.jx3_start_time_unix as i64) * 1000;

    // 计算当前 CD 窗口（本周一 07:00 ~ 下周一 07:00），
    // 作为 start_ms/end_ms 传入 scan_jcl_files 的日期前缀 + 精确时间过滤，
    // 即使 process_start_ms 过滤因故失效，CD 窗口仍能拦截历史 JCL 文件
    let now_ms = chrono::Local::now().timestamp_millis();
    let (cd_start_ms, cd_end_ms) = calculate_cd_window(now_ms, false);

    log::info!(
        "[DropScanner] 批量扫描: JX3 启动时间={}, process_start_ms={}, CD窗口=[{},{}], 检测到 {} 个角色",
        active_result.jx3_start_time.as_deref().unwrap_or("未知"),
        process_start_ms,
        cd_start_ms,
        cd_end_ms,
        active_result.roles.len()
    );

    // 2. 筛选本次会话登录过的角色（Active 或 Recent），不扫描 Idle 账号
    let active_roles: Vec<&crate::mingyi::active_detector::RoleActiveState> = active_result
        .roles
        .iter()
        .filter(|r| r.is_recently_active)
        .collect();

    // 获取已扫描 Recent UID 集合，用于跳过已扫描的 Recent 角色
    let mut scanned_set = scanned_recent_uids()
        .lock()
        .map_err(|e| format!("锁定 scanned_recent_uids 失败: {}", e))?;

    // 统计 Active 和 Recent 角色数量
    // 用 role.is_online 字段（被 active_detector 重排序设为 false for Recent），
    // 而非 active_level.is_online() 方法（对 Recent 也返回 true）
    let active_count = active_roles.iter().filter(|r| r.is_online).count();
    let recent_count = active_roles.len() - active_count;

    // 统计需要跳过的 Recent 角色数量
    let skipped_recent = active_roles
        .iter()
        .filter(|r| !r.is_online && scanned_set.contains(&r.uid))
        .count();

    log::info!(
        "[DropScanner] 活跃角色 {} 个（Active={}, Recent={}），跳过已扫描 Recent={}，Idle 角色已跳过",
        active_roles.len(),
        active_count,
        recent_count,
        skipped_recent
    );

    // 3. 对每个活跃角色执行扫描
    let mut results: Vec<(String, Result<usize, String>)> = Vec::new();
    let game_path = PathBuf::from(&game_dir);
    let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);

    for role in active_roles {
        let uid = &role.uid;
        let account_dir = accounts_base.join(format!("{}@zhcn_hd", uid));
        let is_active = role.is_online;

        // Recent 角色只扫描一次：如果已扫描过则跳过
        // Active 角色每次都扫描，并从已扫描集合中移除（以便后续变为 Recent 时重新扫描）
        if !is_active {
            if scanned_set.contains(uid) {
                log::info!(
                    "[DropScanner] 跳过已扫描的 Recent 角色: uid={}, name={}",
                    uid,
                    role.role_name
                );
                continue;
            }
        } else {
            // Active 角色从已扫描集合中移除（玩家切回了该角色）
            scanned_set.remove(uid);
        }

        // 基于该账号 chatlog mtime 判断角色在线状态
        let role_online = is_role_online(&account_dir, jx3_running);

        log::info!(
            "[DropScanner] 扫描活跃角色: uid={}, name={}, server={}, active_level={:?}, role_online={}",
            uid,
            role.role_name,
            role.server,
            role.active_level,
            role_online
        );

        // 传入预加载的 raids，跳过 scan_raid_drops_with_raids 内部的 init_db + get_cached_raids 调用
        let result = scan_raid_drops_with_raids(
            uid,
            jx3_running,
            role_online,
            process_start_ms,
            cd_start_ms,
            cd_end_ms,
            preloaded_raids,
            None, // 批量路径：区间覆盖整个 CD 窗口，可自行观察到后续 JCL
        );
        results.push((uid.clone(), result));

        // Recent 角色扫描完成后记入集合，后续轮询跳过
        if !is_active {
            scanned_set.insert(uid.clone());
        }
    }

    let success_count = results.iter().filter(|(_, r)| r.is_ok()).count();
    let failed_count = results.len() - success_count;
    let total_instances: usize = results
        .iter()
        .filter_map(|(_, r)| r.as_ref().ok().copied())
        .sum();
    let elapsed_ms = scan_started_at.elapsed().as_millis();

    log::info!(
        "[DropScanner] 批量扫描完成：活跃角色 {} 个（成功 {}，失败 {}），副本实例 {} 个，耗时 {}ms",
        results.len(),
        success_count,
        failed_count,
        total_instances,
        elapsed_ms
    );
    Ok(results)
}

/// 待复核的历史记录（status='scanning' 或 无掉落无工资的 pending）
struct StaleRecord {
    id: String,
    account_id: String,
    data: String,
    drops: String,
    // record_date 为副本开始时间（INTEGER 毫秒），用于查询筛选；
    // 复核逻辑当前不直接读取（保留用于未来扩展）
    #[allow(dead_code)]
    record_date: i64,
    status: String,
}

/// 查询需要复核的历史记录
///
/// 复核范围：
/// - `status='scanning'`：上次会话未收尾的副本，需判定是否已结束
/// - `status='pending' AND drops='[]' AND goldIncome=0`：孤儿 pending 记录，需补充掉落/工资
///
/// 通过 `record_date < cutoff_ms` 隔离本次 JX3 会话产生的记录（交给原轮询逻辑）。
/// 通过 `record_date >= stale_floor_ms`（7 天前）排除过期记录——
/// 超过 7 天的记录已在上游直接翻转 pending（扫描完成），无需重扫。
///
/// 为什么用 record_date 而非 created_at：
/// - `record_date` 为 INTEGER 毫秒，与 cutoff_ms 类型匹配，比较语义正确
/// - `created_at` 为 TEXT (ISO 8601 字符串)，与 INTEGER 比较时 SQLite 做字典序比较，
///   导致 "2026-08-03..." > "1723376400000"（'2' > '1'），历史记录永远进不了复核
/// - `record_date` 在 INSERT 时必有值；`created_at` 在 V14 迁移未回填旧记录，可能为 NULL
/// - 现有 `idx_records_pending(status, record_date DESC)` 复合索引完美匹配查询模式
fn query_stale_records_for_verify(
    conn: &Connection,
    cutoff_ms: i64,
    stale_floor_ms: i64,
) -> Result<Vec<StaleRecord>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, data, drops, record_date, status FROM records
             WHERE record_date < ?1 AND record_date >= ?2 AND (
               status = 'scanning'
               OR (status = 'pending' AND drops = '[]' AND json_extract(data, '$.goldIncome') = 0)
             )",
        )
        .map_err(|e| format!("查询待复核记录失败: {}", e))?;

    let records = stmt
        .query_map(params![cutoff_ms, stale_floor_ms], |row| {
            Ok(StaleRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                data: row.get(2)?,
                drops: row.get(3)?,
                record_date: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .map_err(|e| format!("遍历待复核记录失败: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(records)
}

/// 复核阶段统计
struct VerifyStats {
    /// 待复核记录总数
    total: usize,
    /// scanning→pending 或 pending 补充了数据
    flipped: usize,
    /// 新扫描未识别到掉落/工资，保留了原数据（仅 scanning 翻转 status）
    preserved: usize,
    /// 保留原状（pending 空记录仍空，或 scanning 仍 scanning）
    kept: usize,
    /// 复核失败的记录数
    failed: usize,
}

/// 单条记录复核结果
#[derive(Debug)]
enum VerifyOutcome {
    /// scanning→pending 或 pending 补充了数据
    Flipped,
    /// 新扫描未识别到掉落/工资，保留了原数据
    PreservedOriginal,
    /// 保留原状（无变化）
    Kept,
}

/// 超过 7 天的 scanning 记录直接翻转为 pending（扫描完成）
///
/// 副本"过期"阈值（RAID_STALE_THRESHOLD_MS）是 6 小时，超过 7 天几乎肯定是异常数据，
/// 无需重扫（JCL/chatlog 大概率已不可用），保留原 data/drops 仅改状态。
/// 返回翻转的记录数。
fn flip_expired_scanning_records(conn: &Connection) -> Result<usize, String> {
    let seven_days_ago_ms: i64 = chrono::Local::now().timestamp_millis() - 7 * 24 * 3600 * 1000;
    let timestamp = db::get_local_timestamp();
    conn.execute(
        "UPDATE records SET status = 'pending', updated_at = ?1 \
         WHERE status = 'scanning' AND record_date < ?2",
        rusqlite::params![timestamp, seven_days_ago_ms],
    )
    .map(|n| n as usize)
    .map_err(|e| format!("翻转过期 scanning 记录失败: {}", e))
}

/// 复核历史遗留记录的入口函数
///
/// 在 `scan_all_active_raid_drops_internal` 的本次会话扫描之前调用。
/// 对每条历史记录复用 `scan_raid_drops_with_raids` 重扫其副本实例时间范围，
/// 并采用"按需保留原数据"策略防止数据丢失。
///
/// - `cutoff_ms`：本次 JX3 启动时间（毫秒），只复核 `record_date < cutoff_ms` 的记录
/// - `game_dir`：游戏目录
/// - `preloaded_raids`：预加载的副本配置（与本次会话扫描共用）
/// - `active_result`：active_detector 扫描结果，复用以避免重复扫描游戏目录。
///   仅用 `roles` 字段构建 `(role_name, server) -> uid` 的查找表。
///
/// 性能与兜底：
/// - 7 天前的 scanning 记录直接翻转为 pending（扫描完成），进入用户确认流程
/// - 单次复核成本 O(stale_records)，不再遍历全部 130+ 账号目录
fn verify_stale_records(
    cutoff_ms: i64,
    game_dir: &str,
    preloaded_raids: Option<&[RaidEntry]>,
    active_result: &crate::mingyi::active_detector::BatchActiveResult,
) -> Result<VerifyStats, String> {
    let conn = db::init_db()?;

    // 兜底：超过 7 天的 scanning 记录直接翻转 pending（扫描完成），交给用户确认
    let expired = flip_expired_scanning_records(&conn)?;
    let seven_days_ago_ms: i64 = chrono::Local::now().timestamp_millis() - 7 * 24 * 3600 * 1000;
    if expired > 0 {
        log::info!(
            "[DropScanner] {} 条超过 7 天的 scanning 记录已直接标记为扫描完成（pending）",
            expired
        );
    }

    let stale_records = query_stale_records_for_verify(&conn, cutoff_ms, seven_days_ago_ms)?;
    drop(conn);

    // 从 active_result 构建 (role_name, server) -> uid 查找表
    // 避免对每条 scanning 记录都遍历游戏目录、读 info.jx3dat
    let mut uid_map: HashMap<(String, String), String> = HashMap::new();
    for role in &active_result.roles {
        let key = (role.role_name.trim().to_string(), role.server.clone());
        // 重复 (role_name, server) 时记录首个，避免歧义日志
        uid_map.entry(key).or_insert_with(|| role.uid.clone());
    }
    log::debug!(
        "[DropScanner] 复用 active_detector 结果构建 uid_map: {} 个账号",
        uid_map.len()
    );

    if stale_records.is_empty() {
        return Ok(VerifyStats {
            total: 0,
            flipped: 0,
            preserved: 0,
            kept: 0,
            failed: 0,
        });
    }

    log::info!(
        "[DropScanner] 轮询开始：发现 {} 条历史记录待复核",
        stale_records.len()
    );

    let game_path = PathBuf::from(game_dir);
    let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);

    let mut stats = VerifyStats {
        total: stale_records.len(),
        flipped: 0,
        preserved: 0,
        kept: 0,
        failed: 0,
    };

    for record in &stale_records {
        match verify_single_record(record, &accounts_base, preloaded_raids, &uid_map) {
            Ok(VerifyOutcome::Flipped) => stats.flipped += 1,
            Ok(VerifyOutcome::PreservedOriginal) => stats.preserved += 1,
            Ok(VerifyOutcome::Kept) => stats.kept += 1,
            Err(e) => {
                log::warn!(
                    "[DropScanner] 复核记录 [{}] 失败: {}",
                    record.id,
                    e
                );
                stats.failed += 1;
            }
        }
    }

    Ok(stats)
}

/// 从内存 uid_map 查找茗伊 uid
///
/// 查找顺序：
/// 1. 精确匹配 `(role_name, server)`
/// 2. 回退：去除空格后比较（游戏名字不允许包含空格，解析侧可能引入空格）
fn lookup_mingyi_uid(
    uid_map: &HashMap<(String, String), String>,
    role_name: &str,
    server: &str,
) -> Option<String> {
    if let Some(uid) = uid_map.get(&(role_name.to_string(), server.to_string())) {
        return Some(uid.clone());
    }
    // 回退：去除空格后比较（含全角空格）
    let strip = |s: &str| s.replace(' ', "").replace('\u{3000}', "");
    let target_name = strip(role_name);
    let target_server = strip(server);
    uid_map
        .iter()
        .find(|((n, s), _)| strip(n) == target_name && strip(s) == target_server)
        .map(|(_, v)| v.clone())
}

/// 无法定位茗伊账号目录时的兜底：直接将记录标记为扫描完成（status='pending'）
///
/// 适用场景：uid_map 查不到映射（如角色已删、active_detector 未识别）、账号目录不存在。
/// 保留原 data/drops 仅改状态，交给用户在 UI 确认或删除。
fn mark_record_scan_complete(record: &StaleRecord, reason: &str) -> Result<VerifyOutcome, String> {
    let conn = db::init_db()?;
    mark_record_scan_complete_with_conn(&conn, record, reason)
}

/// [`mark_record_scan_complete`] 的连接参数化版本，便于测试注入临时数据库
fn mark_record_scan_complete_with_conn(
    conn: &Connection,
    record: &StaleRecord,
    reason: &str,
) -> Result<VerifyOutcome, String> {
    let now_ms = chrono::Local::now().timestamp_millis();
    conn.execute(
        "UPDATE records SET status = 'pending', updated_at = ?1 WHERE id = ?2",
        params![now_ms, &record.id],
    )
    .map_err(|e| format!("标记扫描完成失败: {}", e))?;

    log::info!(
        "[DropScanner] 复核记录 [{}]: {}，直接标记为扫描完成（pending），保留原数据",
        record.id, reason
    );
    Ok(VerifyOutcome::Flipped)
}

/// 复核单条历史记录
///
/// 流程：
/// 1. 保存原记录的 data/drops（用于按需保留）
/// 2. 从 data 字段解析副本实例时间范围
/// 3. 动态计算 scan_end_ms（有下一个 JCL 用 next_jcl_time，否则 last_jcl_time + 2h）
/// 4. 调用 scan_raid_drops_with_raids 重扫（process_start_ms=0 绕过 mtime 过滤）
/// 5. 比较新旧数据，新扫描未识别到掉落/工资但原记录有 → 恢复原 data/drops，只改 status
/// 6. 否则接受 upsert 的结果
///
/// `uid_map`：`(role_name, server) -> 茗伊 uid` 查找表（来自 active_detector 内存结果）。
/// records 表的 account_id 是 UUID，不能直接用于拼接 `{uid}@zhcn_hd` 目录；
/// uid 不存数据库，运行时通过记录 data JSON 中的 roleName/server 查回茗伊 uid。
fn verify_single_record(
    record: &StaleRecord,
    accounts_base: &Path,
    preloaded_raids: Option<&[RaidEntry]>,
    uid_map: &HashMap<(String, String), String>,
) -> Result<VerifyOutcome, String> {
    // 1. 保存原记录的 data 和 drops（用于按需保留）
    let original_data = record.data.clone();
    let original_drops = record.drops.clone();
    let original_has_data = has_drops_or_salary(&original_data, &original_drops);
    let was_scanning = record.status == "scanning";

    // 2. 解析 data 字段
    let data: serde_json::Value = serde_json::from_str(&record.data)
        .map_err(|e| format!("解析 data JSON 失败: {}", e))?;

    let scan_start_ms = data["date"].as_i64().unwrap_or(0);
    if scan_start_ms == 0 {
        return Err("data.date 为 0，无法确定扫描起始时间".to_string());
    }

    // 3. 从 jclFiles 解析 last_jcl_time（成功击杀 JCL 的最大时间戳）
    let last_jcl_time = data["jclFiles"]
        .as_array()
        .and_then(|arr| {
            arr.iter()
                .filter_map(|f| f.as_str())
                .filter_map(|name| parse_jcl_filename(name).map(|j| j.timestamp))
                .max()
        })
        .unwrap_or(scan_start_ms);

    // 3.5 通过 data 中的 roleName/server 从内存 uid_map 查回茗伊 uid
    //     records.account_id 是 UUID，无法直接定位茗伊账号目录；
    //     uid 不存数据库，运行时由 active_detector 的内存结果提供映射
    let role_name = data["roleName"].as_str().unwrap_or("").trim().to_string();
    let server = data["server"].as_str().unwrap_or("").trim().to_string();
    let mingyi_uid = match lookup_mingyi_uid(uid_map, &role_name, &server) {
        Some(uid) => uid,
        None => {
            // 查不到映射：不再报错跳过，直接标记扫描完成，交给用户确认
            return mark_record_scan_complete(
                record,
                &format!(
                    "无法映射角色到茗伊账号（roleName={}, server={}）",
                    role_name, server
                ),
            );
        }
    };

    // 4. 查找下一个 JCL 时间戳
    let account_dir = accounts_base.join(format!("{}@zhcn_hd", mingyi_uid));
    if !account_dir.exists() {
        return mark_record_scan_complete(
            record,
            &format!("茗伊账号目录不存在: {}", account_dir.display()),
        );
    }

    let next_jcl_time = find_next_jcl_time_after(&account_dir, last_jcl_time)?;

    // 5. 动态计算 scan_end_ms
    // - 有下一个 JCL → 副本已切换，scan 范围到切换点即可
    // - 无下一个 JCL → 最后一个实例，+2h 兜底工资记录（chatlog flush 延迟）
    let scan_end_ms = if let Some(next_t) = next_jcl_time {
        next_t
    } else {
        last_jcl_time + 2 * 3600 * 1000
    };

    log::info!(
        "[DropScanner] 复核记录 [{}]: account(uuid)={}, mingyi_uid={}, status={}, scan_range=[{},{}], next_jcl={:?}",
        record.id,
        record.account_id,
        mingyi_uid,
        record.status,
        scan_start_ms,
        scan_end_ms,
        next_jcl_time
            .map(|t| t.to_string())
            .unwrap_or_else(|| "None".to_string())
    );

    // 6. 复用 scan_raid_drops_with_raids 重扫
    // - jx3_running=true：复核只在 JX3 运行时触发
    // - role_online：用 is_role_online 实际检测（基于 chatlog mtime）
    // - process_start_ms=0：绕过 mtime 过滤，必须扫到历史 JCL
    // - account_id 传茗伊 uid（函数内部按 `{uid}@zhcn_hd` 定位账号目录，
    //   并通过 roles 表将 uid 换回 UUID 写入 records）
    // - known_next_jcl_time=next_jcl_time：scan_end_ms 恰好等于该值，而扫描区间上界是
    //   开区间，该 JCL 不会进入扫描结果；必须显式传入才能让 has_next_jcl（条件 6）
    //   与 chatlog 窗口截断拿到"副本已切换"信号
    let role_online = is_role_online(&account_dir, true);
    let result = scan_raid_drops_with_raids(
        &mingyi_uid,
        true, // jx3_running
        role_online,
        0, // process_start_ms=0，绕过 mtime 过滤
        scan_start_ms,
        scan_end_ms,
        preloaded_raids,
        next_jcl_time,
    );

    match result {
        Ok(_instance_count) => {
            // 7. 扫描后查询新记录的 data、drops、status
            let conn = db::init_db()?;
            let row_result: rusqlite::Result<(String, String, String)> = conn.query_row(
                "SELECT data, drops, status FROM records WHERE id = ?1",
                params![&record.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            );

            match row_result {
                Ok((new_data, new_drops, new_status)) => {
                    let new_has_data = has_drops_or_salary(&new_data, &new_drops);

                    // 8. 按需保留原数据
                    //    新扫描未识别到掉落/工资，但原记录有 → 恢复原 data/drops，只改 status
                    if !new_has_data && original_has_data {
                        let now_ms = chrono::Local::now().timestamp_millis();
                        // 对于 scanning 记录，翻转 status='pending'
                        // 对于 pending 记录，status 保持 'pending'（恢复原数据即可）
                        conn.execute(
                            "UPDATE records SET data = ?1, drops = ?2, status = ?3, updated_at = ?4 WHERE id = ?5",
                            params![original_data, original_drops, "pending", now_ms, &record.id],
                        )
                        .map_err(|e| format!("恢复原数据失败: {}", e))?;

                        log::info!(
                            "[DropScanner] 复核记录 [{}]: 新扫描未识别到掉落/工资，保留原数据，status='pending'",
                            record.id
                        );
                        return Ok(VerifyOutcome::PreservedOriginal);
                    }

                    // 9. 新扫描有数据或原记录也无数据 → 接受 upsert 的结果
                    let outcome = if was_scanning && new_status == "pending" {
                        VerifyOutcome::Flipped // scanning→pending
                    } else if !was_scanning && new_has_data {
                        VerifyOutcome::Flipped // pending 补充了数据
                    } else if !was_scanning && !new_has_data {
                        VerifyOutcome::Kept // pending 空记录仍空
                    } else {
                        VerifyOutcome::Kept // scanning 保留 scanning
                    };

                    log::info!(
                        "[DropScanner] 复核记录 [{}]: {} (status={})",
                        record.id,
                        match &outcome {
                            VerifyOutcome::Flipped => "翻转/补充",
                            VerifyOutcome::PreservedOriginal => "保留原数据",
                            VerifyOutcome::Kept => "保留原状",
                        },
                        new_status
                    );
                    Ok(outcome)
                }
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // upsert 可能删除了该记录（如同 CD 窗口出现手工 confirmed 记录）
                    log::info!(
                        "[DropScanner] 复核记录 [{}]: 记录已被删除（可能被手工记录冲突清理）",
                        record.id
                    );
                    Ok(VerifyOutcome::Kept)
                }
                Err(e) => Err(format!("查询复核后记录失败: {}", e)),
            }
        }
        Err(e) => Err(format!("重扫失败: {}", e)),
    }
}

/// 判断记录是否有掉落或工资数据
///
/// 用于"按需保留原数据"策略：比较新旧记录是否识别到掉落/工资。
/// - drops 不为空数组
/// - 或 goldIncome > 0
/// - 或 goldExpense > 0
fn has_drops_or_salary(data: &str, drops: &str) -> bool {
    let drops_nonempty = drops != "[]" && !drops.is_empty();
    let gold_income = serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["goldIncome"].as_i64())
        .unwrap_or(0);
    let gold_expense = serde_json::from_str::<serde_json::Value>(data)
        .ok()
        .and_then(|v| v["goldExpense"].as_i64())
        .unwrap_or(0);
    drops_nonempty || gold_income > 0 || gold_expense > 0
}

/// Tauri 命令：批量扫描所有活跃账号的掉落记录
#[tauri::command]
pub async fn scan_all_active_raid_drops() -> Result<Vec<AccountScanResult>, String> {
    tokio::task::spawn_blocking(|| {
        let raw = scan_all_active_raid_drops_internal()?;
        Ok(raw
            .into_iter()
            .map(|(uid, res)| match res {
                Ok(n) => AccountScanResult {
                    account_id: uid,
                    success: true,
                    instance_count: Some(n),
                    error: None,
                },
                Err(e) => AccountScanResult {
                    account_id: uid,
                    success: false,
                    instance_count: None,
                    error: Some(e),
                },
            })
            .collect())
    })
    .await
    .map_err(|e| format!("批量扫描线程异常: {}", e))?
}

/// 按时间范围扫描所有账号的掉落记录（离线扫描，不依赖 JX3 进程状态）
///
/// 用于"扫描本周"等离线场景：扫描 `interface/my#data/` 下所有 `{uid}@zhcn_hd` 目录，
/// 对每个账号调用 `scan_raid_drops_internal`，传入副本时间范围 [start_ms, end_ms)。
///
/// 与 `scan_all_active_raid_drops` 的差异：
/// - 不依赖 JX3 进程运行状态（离线扫描）
/// - 扫描所有账号（不限于活跃角色）
/// - 按副本时间（JCL 文件名解析的副本开始时间）过滤，而非 mtime
/// - 副本完成判断使用 jx3_running=false, role_online=false（视为已离线，全部记为 pending）
///
/// 参数：
/// - `start_ms`: 副本时间下限（毫秒，闭区间）
/// - `end_ms`: 副本时间上限（毫秒，开区间）
/// - `process_start_ms`: JX3 进程启动时间（毫秒），> 0 时按 mtime 过滤 JCL 文件，仅扫描本次会话产生的文件
#[tauri::command]
pub async fn scan_raids_in_range(
    start_ms: i64,
    end_ms: i64,
    process_start_ms: i64,
) -> Result<Vec<AccountScanResult>, String> {
    tokio::task::spawn_blocking(move || {
        let scan_started_at = std::time::Instant::now();
        log::info!(
            "[DropScanner] 时间范围扫描开始：start_ms={}, end_ms={}, process_start_ms={}",
            start_ms, end_ms, process_start_ms
        );

        // 1. 扫描茗伊账号目录，提取所有 uid
        let game_dir = get_game_directory()?;
        let game_path = PathBuf::from(&game_dir);
        let accounts_base = game_path.join(MINGYI_ACCOUNTS_BASE_PATH);

        let mut account_ids: Vec<String> = Vec::new();
        if accounts_base.exists() {
            let entries = std::fs::read_dir(&accounts_base).map_err(|e| {
                format!("读取茗伊账号目录失败: {} - {}", accounts_base.display(), e)
            })?;
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = match path.file_name().and_then(|s| s.to_str()) {
                    Some(n) => n,
                    None => continue,
                };
                // 茗伊账号目录格式：{uid}@zhcn_hd
                if let Some(uid) = dir_name.strip_suffix("@zhcn_hd") {
                    if !uid.is_empty() {
                        account_ids.push(uid.to_string());
                    }
                }
            }
        }

        log::info!(
            "[DropScanner] 时间范围扫描: 共 {} 个茗伊账号",
            account_ids.len()
        );

        // 2. 预加载副本配置一次，传引用给各线程
        // 避免每个账号独立调用 init_db + get_cached_raids
        let pre_loaded_conn = match db::init_db() {
            Ok(c) => c,
            Err(e) => return Err(format!("预加载副本配置失败: {}", e)),
        };
        let pre_loaded_raids = match get_cached_raids(&pre_loaded_conn) {
            Ok(r) => r,
            Err(e) => return Err(format!("预加载副本配置失败: {}", e)),
        };
        drop(pre_loaded_conn); // 释放连接，各线程按需创建自己的写连接
        log::info!(
            "[DropScanner] 预加载 {} 个副本配置，将共享给各扫描线程",
            pre_loaded_raids.len()
        );

        // 3. 对每个账号执行扫描（离线模式：jx3_running=false, role_online=false）
        // 并行化：将账号分块到多个线程处理，块内顺序执行。
        // 并发度选择 4：SQLite journal_mode=DELETE 下写入互斥，
        // 过高并发反而增加 busy 等待；JCL/chatlog 读取是 I/O 密集型，4 并发已能显著加速。
        const NUM_THREADS: usize = 4;
        let chunk_size = account_ids.len().div_ceil(NUM_THREADS).max(1);
        let chunks: Vec<Vec<String>> = account_ids
            .chunks(chunk_size)
            .map(|c| c.to_vec())
            .collect();

        let raids_ref = &pre_loaded_raids;
        let chunk_results: Vec<Vec<AccountScanResult>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunks
                .into_iter()
                .map(|chunk| {
                    s.spawn(move || {
                        let mut local_results: Vec<AccountScanResult> = Vec::with_capacity(chunk.len());
                        for account_id in chunk {
                            log::info!(
                                "[DropScanner] 时间范围扫描账号: {} (start_ms={}, end_ms={})",
                                account_id, start_ms, end_ms
                            );
                            let result = scan_raid_drops_with_raids(
                                &account_id,
                                false, // 离线扫描：JX3 进程未运行
                                false, // 离线扫描：角色不在线
                                process_start_ms, // > 0 时按 mtime 过滤本次会话 JCL
                                start_ms,
                                end_ms,
                                Some(raids_ref),
                                None, // 范围扫描路径：区间由调用方给定，不注入额外的切换点信号
                            );
                            match result {
                                Ok(n) => {
                                    log::info!(
                                        "[DropScanner] 账号 {} 时间范围扫描完成: {} 个副本实例",
                                        account_id, n
                                    );
                                    local_results.push(AccountScanResult {
                                        account_id,
                                        success: true,
                                        instance_count: Some(n),
                                        error: None,
                                    });
                                }
                                Err(e) => {
                                    log::warn!(
                                        "[DropScanner] 账号 {} 时间范围扫描失败: {}",
                                        account_id, e
                                    );
                                    local_results.push(AccountScanResult {
                                        account_id,
                                        success: false,
                                        instance_count: None,
                                        error: Some(e),
                                    });
                                }
                            }
                        }
                        local_results
                    })
                })
                .collect();

            handles
                .into_iter()
                .filter_map(|h| h.join().ok())
                .collect()
        });

        // 合并结果（按 account_id 排序，保证输出稳定）
        let mut results: Vec<AccountScanResult> = chunk_results.into_iter().flatten().collect();
        results.sort_by(|a, b| a.account_id.cmp(&b.account_id));

        let total_instances: usize = results.iter().filter_map(|r| r.instance_count).sum();
        let success_count = results.iter().filter(|r| r.success).count();
        let failed_count = results.len() - success_count;
        let elapsed_ms = scan_started_at.elapsed().as_millis();

        log::info!(
            "[DropScanner] 时间范围扫描完成：账号 {} 个（成功 {}，失败 {}），副本实例 {} 个，耗时 {}ms",
            results.len(),
            success_count,
            failed_count,
            total_instances,
            elapsed_ms
        );

        Ok(results)
    })
    .await
    .map_err(|e| format!("时间范围扫描线程异常: {}", e))?
}

/// Tauri 命令：确认 pending 记录
///
/// 1. 从 records 读取 pending 记录的 data JSON
/// 2. 合并可选的编辑数据，强制设置 source='manual', status='confirmed'
/// 3. UPDATE records（更新 data、source、status、updated_at）
///
/// 可选传入 edit_data（RaidRecord 部分字段的 JSON 字符串），用于在确认时
/// 编辑收支金额、BOSS 选择、标记位等。提供时会合并到 data JSON 后再置为 confirmed。
#[tauri::command]
pub fn confirm_record(record_id: String, edit_data: Option<String>) -> Result<(), String> {
    let conn = db::init_db()?;

    // 从 records 读取当前 data JSON 和 status
    let (current_data, current_status): (String, String) = conn
        .query_row(
            "SELECT data, status FROM records WHERE id = ?1",
            params![record_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("查询 records 记录失败: {}", e))?;

    // 副本进行中（scanning）的记录不允许确认，UI 应禁用确认按钮
    // 此处再次校验作为防御性兜底
    if current_status == "scanning" {
        return Err("副本仍在进行中，无法确认记录".to_string());
    }

    let mut current_json: serde_json::Value =
        serde_json::from_str(&current_data).map_err(|e| format!("解析当前 data JSON 失败: {}", e))?;

    // 若提供编辑数据，合并到 current_json
    if let Some(edit_json_str) = edit_data {
        let edit_json: serde_json::Value =
            serde_json::from_str(&edit_json_str).map_err(|e| format!("解析编辑数据失败: {}", e))?;

        if let (Some(current_obj), Some(edit_obj)) =
            (current_json.as_object_mut(), edit_json.as_object())
        {
            for (key, value) in edit_obj {
                current_obj.insert(key.clone(), value.clone());
            }
        }
    }

    // 强制设置 source='manual', status='confirmed'
    if let Some(obj) = current_json.as_object_mut() {
        obj.insert("source".to_string(), serde_json::json!("manual"));
        obj.insert("status".to_string(), serde_json::json!("confirmed"));
    }

    let updated_data = serde_json::to_string(&current_json).unwrap_or(current_data);
    let now = chrono::Local::now().to_rfc3339();

    // UPDATE records：更新 data、source、status、updated_at
    conn.execute(
        "UPDATE records SET data = ?1, source = 'manual', status = 'confirmed', updated_at = ?2 WHERE id = ?3",
        params![updated_data, now, record_id],
    )
    .map_err(|e| format!("更新 records 记录失败: {}", e))?;

    log::info!("[DropScanner] 记录已确认: {}", record_id);
    Ok(())
}

/// 清空 JCL 缓存表
///
/// 用途：当 analyze_jcl 逻辑修复后，旧版缓存可能残留错误的 is_kill 判定。
/// 调用此命令清空 jcl_cache 表，下次扫描时所有 JCL 文件会重新解析。
///
/// 返回：被清除的缓存条目数量
#[tauri::command]
pub fn clear_jcl_cache() -> Result<i64, String> {
    let conn = db::init_db()?;
    let deleted = conn
        .execute("DELETE FROM jcl_cache", [])
        .map_err(|e| format!("清空 JCL 缓存失败: {}", e))?;
    log::info!("[DropScanner] 已清空 JCL 缓存，删除 {} 条记录", deleted);
    Ok(deleted as i64)
}

/// Tauri 命令：拒绝 pending 记录（CD 释放）
#[tauri::command]
pub fn reject_record(record_id: String) -> Result<(), String> {
    let conn = db::init_db()?;

    let now = chrono::Local::now().to_rfc3339();

    // 更新 records 状态和 data JSON
    if let Ok(data) = conn.query_row(
        "SELECT data FROM records WHERE id = ?1",
        params![record_id],
        |row| row.get::<_, String>(0),
    ) {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&data) {
            if let Some(obj) = json.as_object_mut() {
                obj.insert("status".to_string(), serde_json::json!("rejected"));
                let updated_data = serde_json::to_string(&json).unwrap_or(data);
                conn.execute(
                    "UPDATE records SET data = ?1, status = 'rejected', updated_at = ?2 WHERE id = ?3",
                    params![updated_data, now, record_id],
                )
                .map_err(|e| format!("驳回记录失败: {}", e))?;
                log::info!("[DropScanner] 记录已拒绝: {}", record_id);
                return Ok(());
            }
        }
    }

    // 如果 data JSON 解析失败，至少更新 status 列
    conn.execute(
        "UPDATE records SET status = 'rejected', updated_at = ?1 WHERE id = ?2",
        params![now, record_id],
    )
    .map_err(|e| format!("驳回记录失败: {}", e))?;

    log::info!("[DropScanner] 记录已拒绝: {}", record_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params as sql_params;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use uuid::Uuid;

    /// 创建临时测试数据库，返回 (Connection, temp_dir_path)
    /// 调用方必须持有 temp_dir_path 直到测试结束，否则目录会被提前删除。
    fn create_scan_test_db() -> (rusqlite::Connection, std::path::PathBuf) {
        let dir_name = format!(
            "jx3-raid-manager-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let temp_path = std::env::temp_dir().join(&dir_name);
        fs::create_dir_all(&temp_path).expect("temp dir should be created");

        let db_path = temp_path.join("test.db");
        let conn =
            crate::db::init_db_with_path(&db_path).expect("init_db_with_path should succeed");
        (conn, temp_path)
    }

    /// 写入一份仅用于 JCL 解析回归的 GBK 临时文件。
    ///
    /// 测试不能依赖玩家的真实战斗日志；调用方在断言后负责删除返回的目录。
    fn write_gbk_jcl_fixture(file_name: &str, contents: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir_name = format!(
            "jx3-raid-manager-jcl-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&temp_dir).expect("JCL fixture temp dir should be created");
        let path = temp_dir.join(file_name);
        let (gbk_bytes, _, _) = encoding_rs::GBK.encode(contents);
        fs::write(&path, gbk_bytes.as_ref()).expect("GBK JCL fixture should be written");
        (temp_dir, path)
    }

    /// 将脱敏的 GBK 回归 fixture 写回其原始文件名，以覆盖生产用的文件名副本识别逻辑。
    fn write_luoyang_fixture(
        fixture_file: &str,
        source_file_name: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let fixture_bytes: &[u8] = match fixture_file {
            "01-tuli-success.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/01-tuli-success.jcl"),
            "02-tian-success.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/02-tian-success.jcl"),
            "03-shenshi-minion.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/03-shenshi-minion.jcl"),
            "04-shibing-minion.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/04-shibing-minion.jcl"),
            "05-zhuyan-reset.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/05-zhuyan-reset.jcl"),
            "06-jingye-success.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/06-jingye-success.jcl"),
            "07-ashina-reset.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/07-ashina-reset.jcl"),
            "08-ashina-success.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/08-ashina-success.jcl"),
            "09-shiyi-success.jcl" => include_bytes!("drop_scanner/fixtures/luoyang/09-shiyi-success.jcl"),
            _ => panic!("未知洛阳 fixture: {}", fixture_file),
        };
        let dir_name = format!(
            "jx3-raid-manager-luoyang-fixture-test-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
        );
        let temp_dir = std::env::temp_dir().join(dir_name);
        fs::create_dir_all(&temp_dir).expect("洛阳 fixture 临时目录应可创建");
        let path = temp_dir.join(source_file_name);
        fs::write(&path, fixture_bytes).expect("洛阳 GBK fixture 应可写入");
        (temp_dir, path)
    }

    /// 在测试数据库中创建测试账号和角色
    fn create_test_account_role(conn: &rusqlite::Connection) -> (String, String) {
        let account_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type) VALUES (?1, ?2, 'OWN')",
            sql_params![account_id, "test_account"],
        )
        .expect("should insert account");

        let role_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO roles (id, account_id, name, server) VALUES (?1, ?2, ?3, ?4)",
            sql_params![role_id, account_id, "少年白了发", "测试服"],
        )
        .expect("should insert role");

        (account_id, role_id)
    }

    /// 插入一条 pending 状态的 records 记录（source='auto_scan'），返回 record_id 和 record_date
    fn insert_pending_scan_record(
        conn: &rusqlite::Connection,
        account_id: &str,
        role_id: &str,
    ) -> (String, i64) {
        let record_id = Uuid::new_v4().to_string();
        let now = chrono::Local::now().to_rfc3339();
        let record_date: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let data = serde_json::json!({
            "id": record_id,
            "accountId": account_id,
            "roleId": role_id,
            "raidName": "25人普通阆风悬城",
            "date": record_date,
            "type": "raid",
            "source": "auto",
            "status": "pending",
            "gold": 5000,
            "drops": ["笑妆娘", "唐醉", "张幼归", "无方", "阿史那承庆", "安䘵山"],
        })
        .to_string();

        conn.execute(
            "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type, source, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'raid', 'auto_scan', 'pending', ?7, ?8)",
            sql_params![record_id, data, "阆风悬城", account_id, role_id, record_date, now, now],
        )
        .expect("should insert scan record");

        (record_id, record_date)
    }

    /// 插入一条指定状态的扫描记录，返回 (record_id, record_date)
    /// data 中包含 roleName/server（复核 uid 映射依赖这两个字段）
    fn insert_scan_record_with_status(
        conn: &rusqlite::Connection,
        account_id: &str,
        role_id: &str,
        status: &str,
        record_date: i64,
        drops: &str,
    ) -> String {
        let record_id = Uuid::new_v4().to_string();
        let now = chrono::Local::now().to_rfc3339();
        let data = serde_json::json!({
            "id": record_id,
            "accountId": account_id,
            "roleId": role_id,
            "roleName": "少年白了发",
            "server": "梦江南",
            "raidName": "25人英雄阆风悬城",
            "date": record_date,
            "type": "raid",
            "source": "auto",
            "status": status,
            "goldIncome": 0,
            "goldExpense": 0,
        })
        .to_string();

        conn.execute(
            "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type, source, status, drops, created_at, updated_at)
             VALUES (?1, ?2, '阆风悬城', ?3, ?4, ?5, 'raid', 'auto_scan', ?6, ?7, ?8, ?9)",
            sql_params![record_id, data, account_id, role_id, record_date, status, drops, now, now],
        )
        .expect("should insert scan record");
        record_id
    }

    /// 验证 uid_map 查找：精确匹配、trim 匹配、去空格回退、未命中
    #[test]
    fn test_lookup_mingyi_uid() {
        let mut uid_map: HashMap<(String, String), String> = HashMap::new();
        uid_map.insert(
            ("少年白了发".to_string(), "梦江南".to_string()),
            "342273571708094124".to_string(),
        );
        uid_map.insert(
            ("风 闪".to_string(), "梦江南".to_string()),
            "999999999999999999".to_string(),
        );

        // 1. 精确匹配
        assert_eq!(
            lookup_mingyi_uid(&uid_map, "少年白了发", "梦江南"),
            Some("342273571708094124".to_string())
        );

        // 2. 查询侧带空格 → 精确匹配失败，去空格回退命中（游戏名不允许空格）
        assert_eq!(
            lookup_mingyi_uid(&uid_map, " 少年白了发 ", "梦江南"),
            Some("342273571708094124".to_string())
        );

        // 3. map 侧 key 带空格 → 去空格回退命中（含全角空格）
        assert_eq!(
            lookup_mingyi_uid(&uid_map, "风闪", "梦江南"),
            Some("999999999999999999".to_string())
        );

        // 4. 未命中：服务器不同
        assert_eq!(lookup_mingyi_uid(&uid_map, "少年白了发", "电信区"), None);

        // 5. 未命中：角色名不同
        assert_eq!(lookup_mingyi_uid(&uid_map, "糯闪", "梦江南"), None);

        // 6. 空 map
        let empty: HashMap<(String, String), String> = HashMap::new();
        assert_eq!(lookup_mingyi_uid(&empty, "少年白了发", "梦江南"), None);
    }

    /// 验证超过 7 天的 scanning 记录直接翻转 pending（扫描完成）
    #[test]
    fn test_flip_expired_scanning_records() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);

        let now_ms = chrono::Local::now().timestamp_millis();
        let eight_days_ago = now_ms - 8 * 24 * 3600 * 1000;
        let two_days_ago = now_ms - 2 * 24 * 3600 * 1000;

        // 8 天前的 scanning（应翻转）、2 天前的 scanning（应保留）、8 天前的 pending（不受影响）
        let expired_id =
            insert_scan_record_with_status(&conn, &account_id, &role_id, "scanning", eight_days_ago, "[]");
        let recent_id =
            insert_scan_record_with_status(&conn, &account_id, &role_id, "scanning", two_days_ago, "[]");
        let old_pending_id =
            insert_scan_record_with_status(&conn, &account_id, &role_id, "pending", eight_days_ago, "[]");

        let flipped = flip_expired_scanning_records(&conn).expect("翻转应成功");
        assert_eq!(flipped, 1, "应只翻转 1 条超过 7 天的 scanning 记录");

        let status_of = |id: &str| -> String {
            conn.query_row(
                "SELECT status FROM records WHERE id = ?1",
                sql_params![id],
                |row| row.get(0),
            )
            .expect("should query status")
        };
        assert_eq!(status_of(&expired_id), "pending", "8 天前的 scanning 应翻转 pending");
        assert_eq!(status_of(&recent_id), "scanning", "2 天前的 scanning 应保留待复核");
        assert_eq!(status_of(&old_pending_id), "pending", "8 天前的 pending 不应被重复处理");

        // 幂等：再次执行不产生新的翻转
        let flipped_again = flip_expired_scanning_records(&conn).expect("二次翻转应成功");
        assert_eq!(flipped_again, 0, "二次执行应无翻转（幂等）");
    }

    /// 验证复核兜底：无法定位账号目录时直接标记扫描完成，保留原 data/drops
    #[test]
    fn test_mark_record_scan_complete() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);

        let now_ms = chrono::Local::now().timestamp_millis();
        let record_date = now_ms - 3 * 24 * 3600 * 1000;
        let record_id =
            insert_scan_record_with_status(&conn, &account_id, &role_id, "scanning", record_date, "[]");

        // 用库中实际 data/drops 构造 StaleRecord，验证"保留原数据"语义
        let (db_data, db_drops): (String, String) = conn
            .query_row(
                "SELECT data, drops FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("should query record");
        let record = StaleRecord {
            id: record_id.clone(),
            account_id: account_id.clone(),
            data: db_data.clone(),
            drops: db_drops.clone(),
            record_date,
            status: "scanning".to_string(),
        };

        let outcome =
            mark_record_scan_complete_with_conn(&conn, &record, "测试：无法映射角色到茗伊账号")
                .expect("标记扫描完成应成功");
        assert!(matches!(outcome, VerifyOutcome::Flipped));

        // 状态翻转为 pending，data/drops 保留原值
        let (status, data, drops): (String, String, String) = conn
            .query_row(
                "SELECT status, data, drops FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("should query record");
        assert_eq!(status, "pending", "记录应被标记为扫描完成（pending）");
        assert_eq!(data, db_data, "原 data 应保留");
        assert_eq!(drops, db_drops, "原 drops 应保留");
    }

    /// 验证复核查询的 7 天下限：过期记录不再进入复核重扫范围
    #[test]
    fn test_query_stale_records_floor() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);

        let now_ms = chrono::Local::now().timestamp_millis();
        let cutoff_ms = now_ms; // 本次 JX3 启动时间
        let seven_days_ago_ms = now_ms - 7 * 24 * 3600 * 1000;

        // 3 天前的 scanning（在复核范围内）、8 天前的空 pending（超过下限，应排除）、
        // 3 天前的有掉落 pending（有数据，不属于孤儿记录，应排除）
        insert_scan_record_with_status(&conn, &account_id, &role_id, "scanning", now_ms - 3 * 24 * 3600 * 1000, "[]");
        insert_scan_record_with_status(&conn, &account_id, &role_id, "pending", now_ms - 8 * 24 * 3600 * 1000, "[]");
        insert_scan_record_with_status(&conn, &account_id, &role_id, "pending", now_ms - 3 * 24 * 3600 * 1000, r#"[{"name":"玄晶"}]"#);

        let records =
            query_stale_records_for_verify(&conn, cutoff_ms, seven_days_ago_ms).expect("查询应成功");
        assert_eq!(records.len(), 1, "应只有 1 条记录进入复核（3 天前的 scanning）");
        assert_eq!(records[0].status, "scanning");
    }

    /// 测试完整 confirm 流程：
    /// 插入 pending 记录 → 确认 → 验证记录在 records 表中状态已更新
    #[test]
    fn test_scan_records_confirm_flow() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);
        let (record_id, _record_date) = insert_pending_scan_record(&conn, &account_id, &role_id);

        // 验证 pending 记录存在
        let pending_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .expect("should query pending count");
        assert_eq!(pending_count, 1, "应有 1 条 pending 记录");

        // === 模拟 confirm_record 逻辑 ===
        let current_data: String = conn
            .query_row(
                "SELECT data FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| row.get(0),
            )
            .expect("should read record data");

        let mut current_json: serde_json::Value =
            serde_json::from_str(&current_data).expect("should parse JSON");

        if let Some(obj) = current_json.as_object_mut() {
            obj.insert("source".to_string(), serde_json::json!("manual"));
            obj.insert("status".to_string(), serde_json::json!("confirmed"));
        }

        let updated_data = serde_json::to_string(&current_json).unwrap_or(current_data);
        let now = chrono::Local::now().to_rfc3339();

        conn.execute(
            "UPDATE records SET data = ?1, source = 'manual', status = 'confirmed', updated_at = ?2 WHERE id = ?3",
            sql_params![updated_data, now, record_id],
        )
        .expect("should update records");

        // 验证 records 表有 1 条记录
        let records_count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .expect("should query records count");
        assert_eq!(
            records_count_after, 1,
            "confirm 后 records 表应有 1 条记录"
        );

        // 验证 records 表中的 data 包含 confirmed 状态
        let confirmed_data: String = conn
            .query_row(
                "SELECT data FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| row.get(0),
            )
            .expect("should read confirmed record data");
        let confirmed_json: serde_json::Value =
            serde_json::from_str(&confirmed_data).expect("should parse confirmed JSON");
        assert_eq!(
            confirmed_json["status"], "confirmed",
            "data 中 status 应为 confirmed"
        );
        assert_eq!(
            confirmed_json["source"], "manual",
            "data 中 source 应为 manual"
        );
    }

    /// 测试指定角色和时间参数的完整流程：
    /// 角色=少年白了发, 时间=2026-06-28 12:00 (CST)
    #[test]
    fn test_scan_records_shaonianbailaofa_noon() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);

        // 使用 2026-06-28 12:00:00 CST (UTC+8) 作为开始时间
        // UTC: 2026-06-28 04:00:00 → Unix 秒: 1782619200 → 毫秒: 1782619200000
        let record_date: i64 = 1782619200000;

        let record_id = Uuid::new_v4().to_string();
        let now = chrono::Local::now().to_rfc3339();

        let data = serde_json::json!({
            "id": record_id,
            "accountId": account_id,
            "roleId": role_id,
            "raidName": "25人普通阆风悬城",
            "date": record_date,
            "type": "raid",
            "source": "auto",
            "status": "pending",
            "gold": 5000,
            "drops": ["笑妆娘", "唐醉", "张幼归", "无方", "阿史那承庆", "安䘵山"],
            "roleName": "少年白了发",
        })
        .to_string();

        conn.execute(
            "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type, source, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'raid', 'auto_scan', 'pending', ?7, ?8)",
            sql_params![record_id, data, "阆风悬城", account_id, role_id, record_date, now, now],
        )
        .expect("should insert scan record");

        // 验证 pending 记录的时间戳
        let stored_date: i64 = conn
            .query_row(
                "SELECT record_date FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| row.get(0),
            )
            .expect("should query record_date");
        assert_eq!(stored_date, record_date, "record_date 应为 2026-06-28 12:00 CST");

        let pending_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE status = 'pending' AND account_id = ?1",
                sql_params![account_id],
                |row| row.get(0),
            )
            .expect("should query pending count");
        assert_eq!(pending_count, 1, "少年白了发 应有 1 条 pending 记录");

        // 确认记录
        let current_data: String = conn
            .query_row("SELECT data FROM records WHERE id = ?1", sql_params![record_id], |row| row.get(0))
            .expect("should read record data");

        let mut current_json: serde_json::Value =
            serde_json::from_str(&current_data).expect("should parse JSON");

        if let Some(obj) = current_json.as_object_mut() {
            obj.insert("source".to_string(), serde_json::json!("manual"));
            obj.insert("status".to_string(), serde_json::json!("confirmed"));
        }

        let updated_data = serde_json::to_string(&current_json).unwrap_or(current_data);

        conn.execute(
            "UPDATE records SET data = ?1, source = 'manual', status = 'confirmed', updated_at = ?2 WHERE id = ?3",
            sql_params![updated_data, now, record_id],
        ).expect("should update records");

        // 验证最终状态
        let records_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records WHERE id = ?1", sql_params![record_id], |row| row.get(0))
            .expect("should query records");
        assert_eq!(records_count, 1, "少年白了发 的记录应仍在 records 表中");

        let confirmed_data: String = conn
            .query_row("SELECT data FROM records WHERE id = ?1", sql_params![record_id], |row| row.get(0))
            .expect("should read confirmed data");
        let confirmed_json: serde_json::Value =
            serde_json::from_str(&confirmed_data).expect("should parse confirmed JSON");
        assert_eq!(confirmed_json["roleName"], "少年白了发");
        assert_eq!(confirmed_json["status"], "confirmed");
        assert_eq!(confirmed_json["date"].as_i64(), Some(record_date));
    }

    /// 测试完整 reject 流程：
    /// 插入 pending 记录 → 驳回 → 验证状态更新为 rejected
    #[test]
    fn test_scan_records_reject_flow() {
        let (conn, _temp_dir) = create_scan_test_db();
        let (account_id, role_id) = create_test_account_role(&conn);
        let (record_id, _record_date) = insert_pending_scan_record(&conn, &account_id, &role_id);

        // 验证 pending 记录存在
        let pending_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE status = 'pending'",
                [],
                |row| row.get(0),
            )
            .expect("should query pending count");
        assert_eq!(pending_count, 1, "应有 1 条 pending 记录");

        // === 模拟 reject_record 逻辑 ===
        let now = chrono::Local::now().to_rfc3339();

        if let Ok(data) = conn.query_row(
            "SELECT data FROM records WHERE id = ?1",
            sql_params![record_id],
            |row| row.get::<_, String>(0),
        ) {
            if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(obj) = json.as_object_mut() {
                    obj.insert("status".to_string(), serde_json::json!("rejected"));
                    let updated_data = serde_json::to_string(&json).unwrap_or(data);
                    conn.execute(
                        "UPDATE records SET data = ?1, status = 'rejected', updated_at = ?2 WHERE id = ?3",
                        sql_params![updated_data, now, record_id],
                    )
                    .expect("should reject record");
                }
            }
        }

        // 验证 records 状态已更新为 rejected
        let status: String = conn
            .query_row(
                "SELECT status FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| row.get(0),
            )
            .expect("should query status");
        assert_eq!(status, "rejected", "记录应被标记为 rejected");

        // 验证 records 表仍有 1 条记录（驳回不删除记录，只更新状态）
        let records_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .expect("should query records count");
        assert_eq!(records_count, 1, "驳回后 records 表应仍有 1 条记录");

        // 验证 data JSON 中也包含 rejected 状态
        let rejected_data: String = conn
            .query_row(
                "SELECT data FROM records WHERE id = ?1",
                sql_params![record_id],
                |row| row.get(0),
            )
            .expect("should read rejected data");
        let rejected_json: serde_json::Value =
            serde_json::from_str(&rejected_data).expect("should parse rejected JSON");
        assert_eq!(
            rejected_json["status"], "rejected",
            "data 中 status 应为 rejected"
        );
    }

    #[test]
    fn test_parse_jcl_filename_normal() {
        let result = parse_jcl_filename("2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.raid_display_name, "25人普通阆风悬城");
        assert_eq!(info.boss_name, "笑妆娘");
        assert_eq!(info.raid_id, 794);
        assert_eq!(info.boss_id, 137088);
    }

    #[test]
    fn test_parse_jcl_filename_with_special_boss() {
        let result = parse_jcl_filename("2026-06-14-16-50-08-英雄天龙寺(683)-枯荣大师·幻影(129124).jcl");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.raid_display_name, "英雄天龙寺");
        assert_eq!(info.boss_name, "枯荣大师·幻影");
    }

    #[test]
    fn test_parse_jcl_filename_empty_boss() {
        // BOSS 名为空的格式：副本名(副本ID)-(BOSS ID).jcl
        let result = parse_jcl_filename("2024-05-25-17-01-57-天山碎冰谷(127)-(46297).jcl");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.raid_display_name, "天山碎冰谷");
        assert_eq!(info.raid_id, 127);
        assert_eq!(info.boss_name, "");
        assert_eq!(info.boss_id, 46297);
    }

    #[test]
    fn test_parse_jcl_filename_empty_boss_zero_id() {
        // BOSS ID 为 0 且 BOSS 名为空
        let result = parse_jcl_filename("2024-05-19-23-49-12-华山之巅(129)-(0).jcl");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.raid_display_name, "华山之巅");
        assert_eq!(info.raid_id, 129);
        assert_eq!(info.boss_name, "");
        assert_eq!(info.boss_id, 0);
    }

    #[test]
    fn test_parse_jcl_filename_with_dot_in_name() {
        // 副本名包含 ·
        let result = parse_jcl_filename("2024-09-22-15-06-48-英雄上阳宫·观风殿(248)-(53903).jcl");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.raid_display_name, "英雄上阳宫·观风殿");
        assert_eq!(info.raid_id, 248);
        assert_eq!(info.boss_id, 53903);
    }

    #[test]
    fn test_parse_jcl_filename_invalid() {
        assert!(parse_jcl_filename("invalid.jcl").is_none());
        assert!(parse_jcl_filename("2026-06-13-invalid.jcl").is_none());
    }

    #[test]
    fn test_match_raid_name_exact() {
        let raids = vec![RaidEntry {
            name: "阆风悬城".to_string(),
            raid_id: "25人普通阆风悬城".to_string(),
            bosses: vec![("1".to_string(), "笑妆娘".to_string())],
        }];

        let result = match_raid_name("25人普通阆风悬城", &raids);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "阆风悬城");
    }

    #[test]
    fn test_match_raid_name_suffix() {
        let raids = vec![RaidEntry {
            name: "阆风悬城".to_string(),
            raid_id: "25人英雄阆风悬城".to_string(),
            bosses: vec![("1".to_string(), "笑妆娘".to_string())],
        }];

        // JCL 名后缀匹配 raids.id
        let result = match_raid_name("25人英雄阆风悬城", &raids);
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, "阆风悬城");

        // raids.name 后缀匹配
        let result2 = match_raid_name("25人普通阆风悬城", &raids);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap().name, "阆风悬城");
    }

    #[test]
    fn test_match_raid_name_no_match() {
        let raids: Vec<RaidEntry> = vec![];
        assert!(match_raid_name("不存在的副本", &raids).is_none());
    }

    #[test]
    fn test_match_raid_name_multi_candidates_no_prefix() {
        // JCL 无人数和难度前缀（如 "阆风悬城"），应匹配最低人数+最低难度
        let raids = vec![
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "10人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人英雄阆风悬城".to_string(),
                bosses: vec![],
            },
        ];

        let result = match_raid_name("阆风悬城", &raids);
        assert!(result.is_some());
        assert_eq!(result.unwrap().raid_id, "10人普通阆风悬城");
    }

    #[test]
    fn test_match_raid_name_multi_candidates_with_player_count() {
        // JCL 有人数无难度（如 "25人阆风悬城"），应匹配相同人数+最低难度
        let raids = vec![
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "10人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人英雄阆风悬城".to_string(),
                bosses: vec![],
            },
        ];

        let result = match_raid_name("25人阆风悬城", &raids);
        assert!(result.is_some());
        assert_eq!(result.unwrap().raid_id, "25人普通阆风悬城");
    }

    #[test]
    fn test_match_raid_name_multi_candidates_with_difficulty() {
        // JCL 有难度无人数（如 "英雄阆风悬城"），应匹配最低人数+相同难度
        let raids = vec![
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "10人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人普通阆风悬城".to_string(),
                bosses: vec![],
            },
            RaidEntry {
                name: "阆风悬城".to_string(),
                raid_id: "25人英雄阆风悬城".to_string(),
                bosses: vec![],
            },
        ];

        let result = match_raid_name("英雄阆风悬城", &raids);
        assert!(result.is_some());
        assert_eq!(result.unwrap().raid_id, "25人英雄阆风悬城");
    }

    #[test]
    fn test_cluster_raid_instances_single() {
        let jcl_files = vec![
            JclFileInfo {
                file_name: "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
                timestamp: 1718265239000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137088,
            },
            JclFileInfo {
                file_name: "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
                timestamp: 1718266200000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "唐醉".to_string(),
                boss_id: 137089,
            },
        ];

        // 两个 JCL 都通关（is_kill=true）
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: 1718265239000, fight_end_ms: 1718265299000, is_kill: true },
        );
        jcl_analyses.insert(
            "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
            JclAnalysis { boss_name: Some("唐醉".to_string()), fight_start_ms: 1718266200000, fight_end_ms: 1718266260000, is_kill: true },
        );

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].bosses_killed.len(), 2);
        assert_eq!(instances[0].boss_kill_count, 2);
    }

    #[test]
    fn test_cluster_raid_instances_different_raids() {
        let jcl_files = vec![
            JclFileInfo {
                file_name: "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
                timestamp: 1718265239000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137088,
            },
            JclFileInfo {
                file_name: "2026-06-13-19-00-00-英雄天龙寺(683)-枯荣大师(129124).jcl".to_string(),
                timestamp: 1718266800000,
                raid_display_name: "英雄天龙寺".to_string(),
                raid_id: 683,
                boss_name: "枯荣大师".to_string(),
                boss_id: 129124,
            },
        ];

        // 两个 JCL 都通关
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: 1718265239000, fight_end_ms: 1718265299000, is_kill: true },
        );
        jcl_analyses.insert(
            "2026-06-13-19-00-00-英雄天龙寺(683)-枯荣大师(129124).jcl".to_string(),
            JclAnalysis { boss_name: Some("枯荣大师".to_string()), fight_start_ms: 1718266800000, fight_end_ms: 1718266860000, is_kill: true },
        );

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[]);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn test_cluster_raid_instances_filter_wipes() {
        // 第二条 JCL is_kill=false（拉托），应被过滤
        let jcl_files = vec![
            JclFileInfo {
                file_name: "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
                timestamp: 1718265239000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137088,
            },
            JclFileInfo {
                file_name: "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
                timestamp: 1718266200000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "唐醉".to_string(),
                boss_id: 137089,
            },
        ];

        // 第一条通关，第二条拉托
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: 1718265239000 + 60_000, fight_end_ms: 1718265239000 + 120_000, is_kill: true },
        );
        jcl_analyses.insert(
            "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
            JclAnalysis { boss_name: Some("唐醉".to_string()), fight_start_ms: 1718266200000 + 60_000, fight_end_ms: 1718266200000 + 120_000, is_kill: false },
        );

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[]);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].bosses_killed.len(), 1);
        assert_eq!(instances[0].boss_kill_count, 1);
        // 第一个成功 JCL 的文件名时间 = start_time
        assert_eq!(instances[0].start_time, 1718265239000);
        // fight_start_ms = first_gold_time
        assert_eq!(instances[0].first_gold_time, 1718265239000 + 60_000);
        // 最后一个成功 JCL 的 fight_end_ms = end_time
        assert_eq!(instances[0].end_time, 1718265239000 + 120_000);
        // last_jcl_time 仍为最后一个 JCL（含拉托）的文件名时间
        assert_eq!(instances[0].last_jcl_time, 1718266200000);
    }

    #[test]
    fn test_cluster_raid_instances_all_wipes_skipped() {
        // 整组全为拉托（is_kill=false），应跳过不创建实例
        let jcl_files = vec![
            JclFileInfo {
                file_name: "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
                timestamp: 1718265239000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137088,
            },
            JclFileInfo {
                file_name: "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
                timestamp: 1718266200000,
                raid_display_name: "25人普通阆风悬城".to_string(),
                raid_id: 794,
                boss_name: "唐醉".to_string(),
                boss_id: 137089,
            },
        ];

        // 两个 JCL 都拉托
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-06-13-18-13-59-25人普通阆风悬城(794)-笑妆娘(137088).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: 1718265239000, fight_end_ms: 1718265299000, is_kill: false },
        );
        jcl_analyses.insert(
            "2026-06-13-18-30-00-25人普通阆风悬城(794)-唐醉(137089).jcl".to_string(),
            JclAnalysis { boss_name: Some("唐醉".to_string()), fight_start_ms: 1718266200000, fight_end_ms: 1718266260000, is_kill: false },
        );

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[]);
        assert_eq!(instances.len(), 0, "全为拉托应跳过不创建实例");
    }

    #[test]
    fn test_cluster_raid_instances_require_confirmed_kill_for_every_raid() {
        let raids = [
            ("10人普通洛阳之战", 828, "伊曼·寂夜", "伊曼·逐焰"),
            ("25人普通洛阳之战", 828, "伊曼·寂夜", "伊曼·逐焰"),
            ("25人英雄洛阳之战", 828, "伊曼·寂夜", "伊曼·逐焰"),
            ("25人英雄阆风悬城", 795, "唐怀仁", "须罗巨傀"),
        ];
        // None 表示缺少解析结果；明确成功后出现未确认或缺失日志，不应覆盖已有击杀。
        let scenarios: &[(&[Option<bool>], Option<usize>)] = &[
            (&[Some(false)], None),
            (&[Some(false), Some(false)], None),
            (&[None, None], None),
            (&[Some(false), Some(false), Some(true)], Some(2)),
            (&[Some(false), Some(true), Some(false)], Some(1)),
            (&[Some(true), Some(true)], Some(1)),
            (&[Some(true), None], Some(0)),
        ];

        for (raid_name, map_id, boss_name, entity_name) in raids {
            for file_boss_name in [boss_name, entity_name] {
                for &(outcomes, expected_index) in scenarios {
                    let mut analyses = HashMap::new();
                    let files: Vec<JclFileInfo> = outcomes
                        .iter()
                        .enumerate()
                        .map(|(index, outcome)| {
                            let timestamp = 1_700_000_000_000 + index as i64 * 60_000;
                            let file_name = format!("{}-{}-{}.jcl", raid_name, file_boss_name, index);
                            if let Some(is_kill) = outcome {
                                analyses.insert(
                                    file_name.clone(),
                                    JclAnalysis {
                                        boss_name: Some(boss_name.to_string()),
                                        fight_start_ms: timestamp,
                                        fight_end_ms: timestamp + 30_000,
                                        is_kill: *is_kill,
                                    },
                                );
                            }
                            JclFileInfo {
                                file_name,
                                timestamp,
                                raid_display_name: raid_name.to_string(),
                                raid_id: map_id,
                                boss_name: file_boss_name.to_string(),
                                boss_id: 1,
                            }
                        })
                        .collect();

                    let instances = cluster_raid_instances(
                        "test_account",
                        files.clone(),
                        &analyses,
                        &[],
                    );
                    let context = format!("{} / {} / {:?}", raid_name, file_boss_name, outcomes);
                    if let Some(index) = expected_index {
                        assert_eq!(instances.len(), 1, "{}", context);
                        let instance = &instances[0];
                        assert_eq!(instance.boss_kill_count, 1, "{}", context);
                        assert_eq!(instance.jcl_files, vec![files[index].file_name.clone()], "{}", context);
                        assert_eq!(instance.jcl_boss_names, vec![boss_name], "{}", context);
                        assert_eq!(instance.end_time, files[index].timestamp + 30_000, "{}", context);
                        assert_eq!(instance.last_jcl_time, files.last().unwrap().timestamp, "{}", context);
                    } else {
                        assert!(instances.is_empty(), "无确认击杀不得补记：{}", context);
                    }
                }
            }
        }
    }

    /// 洛阳之战逐焰文件仍映射为寂夜本体，但无明确完成信号时不能由聚类兜底记为击杀。
    #[test]
    fn test_analyze_jcl_luoyang_clone_uses_generic_boss_resolution() {
        let raid_bosses = vec![
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
        ];
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "luoyang-clone-only.jcl",
            concat!(
                "crc\t1\t1700000000\t0\t1\t{true,\"fight\",0}\n",
                "crc\t2\t1700000001\t0\t8\t{101,\"伊曼·逐焰\",61001,0,0,0,0,0}\n",
                "crc\t3\t1700000002\t0\t8\t{202,\"伊曼·寂夜\",61002,0,0,0,0,0}\n",
                "crc\t4\t1700000003\t0\t9\t{101,true,100}\n",
                "crc\t5\t1700000004\t0\t9\t{202,true,100}\n",
                "crc\t6\t1700000005\t0\t28\t{101,999}\n",
                "crc\t7\t1700000006\t0\t7\t{101}\n",
            ),
        );

        let analysis = analyze_jcl(&jcl_path, "伊曼·逐焰", 61001, &raid_bosses)
            .expect("临时 JCL 应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");

        assert_eq!(analysis.boss_name.as_deref(), Some("伊曼·寂夜"));
        assert!(
            !analysis.is_kill,
            "仅解析单份 JCL 时，逐焰的死亡/离场不产生寂夜的明确击杀信号"
        );

        // 覆盖静态配置中的 10人普通、25人普通、25人英雄三种副本显示名。
        // 逐焰拉脱不能仅因文件名与内容首领不同而提前计入寂夜。
        for raid_display_name in ["10人普通洛阳之战", "25人普通洛阳之战", "25人英雄洛阳之战"] {
            let clone_file = format!("{}-逐焰.jcl", raid_display_name);
            let next_boss_file = format!("{}-阿史那承庆.jcl", raid_display_name);
            let clone_timestamp = 1_700_000_000_000;
            let mut analyses = HashMap::new();
            analyses.insert(clone_file.clone(), analysis.clone());
            analyses.insert(
                next_boss_file.clone(),
                JclAnalysis {
                    boss_name: Some("阿史那承庆".to_string()),
                    fight_start_ms: clone_timestamp + 60_000,
                    fight_end_ms: clone_timestamp + 90_000,
                    is_kill: true,
                },
            );
            let instances = cluster_raid_instances(
                "test_account",
                vec![
                    JclFileInfo {
                        file_name: clone_file,
                        timestamp: clone_timestamp,
                        raid_display_name: raid_display_name.to_string(),
                        raid_id: 900,
                        boss_name: "伊曼·逐焰".to_string(),
                        boss_id: 61001,
                    },
                    JclFileInfo {
                        file_name: next_boss_file,
                        timestamp: clone_timestamp + 60_000,
                        raid_display_name: raid_display_name.to_string(),
                        raid_id: 900,
                        boss_name: "阿史那承庆".to_string(),
                        boss_id: 62001,
                    },
                ],
                &analyses,
                &[],
            );

            assert_eq!(instances.len(), 1, "{} 应产出后续 BOSS 的副本实例", raid_display_name);
            assert_eq!(
                instances[0].jcl_boss_names,
                vec!["阿史那承庆"],
                "{} 中无完成信号的逐焰文件不能由聚类兜底解析为寂夜击杀",
                raid_display_name
            );
        }
    }

    #[test]
    fn test_generic_reset_signals_are_raid_metadata_invariant() {
        let contents = concat!(
            "crc\t1\t1700000000\t0\t1\t{true,\"fight\",0}\n",
            "crc\t2\t1700000001\t0\t8\t{101,\"测试首领\",138376,0,0,0,0,192}\n",
            "crc\t3\t1700000002\t0\t9\t{101,true,100,100}\n",
            "crc\t4\t1700000003\t0\t9\t{101,false,100,100}\n",
            "crc\t5\t1700000004\t0\t7\t{101}\n",
            "crc\t6\t1700000005\t0\t1\t{false,\"fight\",5}\n",
        );
        for (raid_name, map_id, business_id) in [
            ("洛阳之战", 828, "luoyangzhizhan_4"),
            ("25人英雄阆风悬城", 795, "other_1"),
        ] {
            let file_name = format!(
                "2026-10-29-22-22-20-{}({})-测试首领(138376).jcl",
                raid_name, map_id
            );
            let raid_bosses = vec![(business_id.to_string(), "测试首领".to_string())];
            let (temp_dir, jcl_path) = write_gbk_jcl_fixture(&file_name, contents);
            let analysis = analyze_jcl(&jcl_path, "测试首领", 138376, &raid_bosses)
                .expect("临时 JCL 应可解析");
            fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
            assert!(
                !analysis.is_kill,
                "仅脱战、离场和朝向 192 不能判定击杀：{} / {} / {}",
                raid_name,
                map_id,
                business_id
            );
        }
    }

    #[test]
    fn test_luoyang_accepts_jiye_chest_without_achievement() {
        let raid_bosses = vec![
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
        ];
        let (chest_dir, chest_path) = write_gbk_jcl_fixture(
            "2026-10-29-22-09-07-洛阳之战(828)-伊曼·寂夜(138932).jcl",
            concat!(
                "crc\t1\t1700000200\t0\t1\t{true,\"fight\",0}\n",
                "crc\t2\t1700000201\t0\t8\t{202,\"伊曼·寂夜\",138932,0,0,0,0,0}\n",
                "crc\t3\t1700000202\t0\t9\t{202,true,100,100}\n",
                "crc\t4\t1700000203\t0\t8\t{303,\"伊曼寂夜宝箱\",0,0,0,0,0,0}\n",
                "crc\t5\t1700000204\t0\t1\t{false,\"fight\",4}\n",
            ),
        );
        let chest = analyze_jcl(&chest_path, "伊曼·寂夜", 138932, &raid_bosses)
            .expect("宝箱 JCL 应可解析");
        fs::remove_dir_all(chest_dir).expect("临时 JCL 目录应可删除");
        assert!(chest.is_kill, "没有成就消息时也应根据对应首领宝箱识别成功");
    }

    #[test]
    fn test_achievement_messages_do_not_affect_either_boss_path() {
        let raid_bosses = vec![
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
            ("luoyangzhizhan_4".to_string(), "阿史那承庆".to_string()),
        ];
        let cases = [
            (
                "阿史那承庆", 138376,
                concat!(
                    "crc\t2\t1700000301\t0\t8\t{101,\"阿史那承庆\",138376,0,0,0,0,0}\n",
                    "crc\t3\t1700000302\t0\t9\t{101,true,100,100}\n",
                ),
                "crc\t7\t1700000306\t0\t8\t{303,\"阿史那承庆宝箱\",140033,0,0,0,0,0}\n",
                "crc\t6\t1700000305\t0\t18\t{\"[全胜！洛阳之战]：4/5，击败阿史那承庆(完成)\\n\",\"MSG_SYS\"}\n",
            ),
            (
                "伊曼·逐焰", 138933,
                concat!(
                    "crc\t2\t1700000301\t0\t8\t{101,\"伊曼·逐焰\",138933,0,0,0,0,0}\n",
                    "crc\t3\t1700000302\t0\t8\t{202,\"伊曼·寂夜\",138932,0,0,0,0,0}\n",
                    "crc\t4\t1700000303\t0\t9\t{101,true,100,100}\n",
                    "crc\t5\t1700000304\t0\t9\t{202,true,100,100}\n",
                ),
                "crc\t7\t1700000306\t0\t8\t{303,\"伊曼寂夜宝箱\",140032,0,0,0,0,0}\n",
                "crc\t6\t1700000305\t0\t18\t{\"[全胜！洛阳之战]：3/5，击败伊曼寂夜、伊曼逐焰(完成)\\n\",\"MSG_SYS\"}\n",
            ),
        ];
        for (boss_name, boss_id, actors, chest, achievement) in cases {
            for with_chest in [false, true] {
                let mut baseline = None;
                for repetitions in [0, 1, 3] {
                    let mut contents = String::from(
                        "crc\t1\t1700000300\t0\t1\t{true,\"fight\",0}\n",
                    );
                    contents.push_str(actors);
                    for _ in 0..repetitions {
                        contents.push_str(achievement);
                    }
                    if with_chest {
                        contents.push_str(chest);
                    }
                    contents.push_str(
                        "crc\t8\t1700000307\t0\t1\t{false,\"fight\",7}\n",
                    );
                    let file_name = format!(
                        "2026-10-29-22-09-07-洛阳之战(828)-{}({}).jcl",
                        boss_name, boss_id
                    );
                    let (temp_dir, jcl_path) = write_gbk_jcl_fixture(&file_name, &contents);
                    let analysis = analyze_jcl(&jcl_path, boss_name, boss_id, &raid_bosses)
                        .expect("成就不变性样本应可解析");
                    fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
                    assert_eq!(analysis.is_kill, with_chest, "成就消息不能补充击杀依据");
                    let snapshot = (
                        analysis.boss_name,
                        analysis.fight_start_ms,
                        analysis.fight_end_ms,
                        analysis.is_kill,
                    );
                    if let Some(expected) = &baseline {
                        assert_eq!(&snapshot, expected, "成就出现、缺失或重复不应改变任何解析结果");
                    } else {
                        baseline = Some(snapshot);
                    }
                }
            }
        }
    }

    #[test]
    fn test_jcl_cache_key_versions_all_raids_after_analysis_changes() {
        for file_name in [
            "2026-10-29-22-29-08-洛阳之战(828)-阿史那承庆(138376).jcl",
            "2026-10-29-22-29-08-25人英雄阆风悬城(795)-阿史那承庆(138376).jcl",
        ] {
            assert_eq!(
                jcl_cache_key(Path::new(file_name)),
                format!("{}{}", JCL_ANALYSIS_CACHE_VERSION, file_name),
                "解析规则更新后，旧缓存均须重算"
            );
        }
    }

    #[test]
    fn test_jcl_versioned_cache_ignores_old_entries_and_still_uses_mtime() {
        let raid_bosses = vec![("business_1".to_string(), "普通首领".to_string())];
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-30-10-00-00-任意副本(999)-普通首领(61001).jcl",
            concat!(
                "crc\t1\t1700001100\t0\t1\t{true,\"fight\",0}\n",
                "crc\t2\t1700001101\t0\t8\t{101,\"普通首领\",61001,0,0,0,0,0}\n",
                "crc\t3\t1700001102\t0\t9\t{101,true,100,100}\n",
            ),
        );
        let conn = Connection::open_in_memory().expect("内存数据库应可创建");
        conn.execute_batch(
            "CREATE TABLE jcl_cache (
                file_path TEXT PRIMARY KEY,
                file_mtime INTEGER NOT NULL,
                boss_name TEXT,
                fight_start_ms INTEGER NOT NULL,
                fight_end_ms INTEGER NOT NULL,
                is_kill INTEGER NOT NULL,
                cached_at TEXT NOT NULL
            )",
        )
        .expect("内存缓存表应可创建");
        let mtime = get_file_mtime(&jcl_path);
        let raw_path = jcl_path.to_string_lossy().to_string();
        for old_key in [
            raw_path.clone(),
            format!("jcl-luoyang-v2|{}", raw_path),
            format!("jcl-analysis-v3|{}", raw_path),
            format!("jcl-analysis-v4|{}", raw_path),
            format!("jcl-analysis-v5|{}", raw_path),
            format!("jcl-analysis-v6|{}", raw_path),
            format!("jcl-analysis-v7|{}", raw_path),
        ] {
            set_jcl_cache(
                &conn,
                &old_key,
                mtime,
                &JclAnalysis {
                    boss_name: Some("普通首领".to_string()),
                    fight_start_ms: 1,
                    fight_end_ms: 2,
                    is_kill: true,
                },
            );
        }

        let first = analyze_jcl_cached(&conn, &jcl_path, "普通首领", 61001, &raid_bosses)
            .expect("版本化缓存首次应重新解析");
        assert!(!first.is_kill, "v7 的旧 true 缓存必须按实体证据重新解析为不确认");
        let cache_key = jcl_cache_key(&jcl_path);
        assert!(get_jcl_cache(&conn, &cache_key, mtime).is_some(), "新键应写入并复用");
        assert!(
            get_jcl_cache(&conn, &cache_key, mtime + 1).is_none(),
            "文件 mtime 改变时版本化缓存仍须失效"
        );
        let second = analyze_jcl_cached(&conn, &jcl_path, "普通首领", 61001, &raid_bosses)
            .expect("新键缓存命中应可读取");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(!second.is_kill);
    }

    #[test]
    fn test_v8_false_cache_hit_can_promote_without_cache_writeback() {
        let bosses = vec![("boss_1".to_string(), "测试首领".to_string())];
        let (temp_dir, path) = write_gbk_jcl_fixture(
            "cached-candidate.jcl",
            concat!(
                "crc\t1\t1700000000\t0\t1\t{true,\"fixture::U123456/1\",0}\n",
                "crc\t2\t1700000001\t0\t8\t{9001,\"测试首领\",61001,0,0,0,0}\n",
                "crc\t3\t1700000002\t0\t9\t{9001,true,100,100,0,0}\n",
                "crc\t4\t1700000003\t0\t21\t{111,9001,0,0,0,0,0,0,{[13]=100,[14]=0}}\n",
                "crc\t5\t1700000004\t0\t9\t{9001,false,100,100,0,0}\n",
                "crc\t6\t1700000005\t0\t1\t{false,\"fixture::U123456/1\",5}\n",
            ),
        );
        let conn = Connection::open_in_memory().expect("内存数据库应可创建");
        conn.execute_batch("CREATE TABLE jcl_cache (file_path TEXT PRIMARY KEY, file_mtime INTEGER NOT NULL, boss_name TEXT, fight_start_ms INTEGER NOT NULL, fight_end_ms INTEGER NOT NULL, is_kill INTEGER NOT NULL, cached_at TEXT NOT NULL)")
            .expect("缓存表应可创建");
        let first = analyze_jcl_cached(&conn, &path, "测试首领", 61001, &bosses).expect("首次解析");
        assert!(!first.is_kill);
        let cached = analyze_jcl_cached(&conn, &path, "测试首领", 61001, &bosses).expect("v8 false 缓存命中");
        assert!(!cached.is_kill, "缓存只存JCL强证据");
        let candidate = analyze_damage_completion_candidate(&path, "测试首领", 61001, &bosses, "123456").expect("缓存命中仍需独立重建候选");
        let mut candidates = HashMap::new();
        candidates.insert("cached-candidate.jcl".to_string(), candidate.clone());
        assert_eq!(select_damage_money_promotions(&candidates, &[candidate.fight_end_ms], &[candidate.fight_end_ms / 1000 - 5]), HashSet::from(["cached-candidate.jcl".to_string()]));
        let cache = get_jcl_cache(&conn, &jcl_cache_key(&path), get_file_mtime(&path)).expect("v8缓存仍存在");
        assert!(!cache.is_kill, "外部补强不得回写JCL缓存");
        fs::remove_dir_all(temp_dir).expect("临时目录应可删除");
    }

    #[test]
    fn test_treasure_box_name_matches_directly_or_without_dots() {
        let bosses = vec![("boss_1".to_string(), "伊曼·寂夜".to_string())];
        for name in ["伊曼·寂夜", "伊曼寂夜", "伊曼.寂夜", "伊·曼.寂夜"] {
            assert_eq!(match_treasure_box_boss(name, &bosses), Some("伊曼·寂夜"));
        }
        for name in ["", ".·", "伊曼寂夜幻影", "伊曼寂", "伊曼-寂夜"] {
            assert_eq!(match_treasure_box_boss(name, &bosses), None, "{}", name);
        }
        let plain = vec![("boss_1".to_string(), "伊曼寂夜".to_string())];
        assert_eq!(match_treasure_box_boss("伊曼·寂夜", &plain), Some("伊曼寂夜"));
        let ascii = vec![("boss_1".to_string(), "伊曼.寂夜".to_string())];
        assert_eq!(match_treasure_box_boss("伊曼寂夜", &ascii), Some("伊曼.寂夜"));
    }

    #[test]
    fn test_treasure_box_name_prefers_exact_match_and_rejects_dot_collisions() {
        let bosses = vec![
            ("boss_1".to_string(), "甲·乙".to_string()),
            ("boss_2".to_string(), "甲乙".to_string()),
        ];
        assert_eq!(match_treasure_box_boss("甲乙", &bosses), Some("甲乙"));
        assert_eq!(match_treasure_box_boss("甲·乙", &bosses), Some("甲·乙"));
        assert_eq!(match_treasure_box_boss("甲.乙", &bosses), None);
    }

    #[test]
    fn test_treasure_box_name_matching_is_generic_with_or_without_config() {
        for configured in [true, false] {
            for (boss_name, chest_name, expected_kill) in [
                ("伊曼·寂夜", "伊曼·寂夜宝箱", true),
                ("伊曼·寂夜", "伊曼寂夜宝箱", true),
                ("伊曼.寂夜", "伊曼寂夜宝箱", true),
                ("伊曼寂夜", "伊曼·寂夜宝箱", true),
                ("伊曼·寂夜", "伊曼寂夜幻影宝箱", false),
            ] {
                let bosses = if configured {
                    vec![("other_1".to_string(), boss_name.to_string())]
                } else {
                    Vec::new()
                };
                let file_name = format!(
                    "2026-10-30-10-00-00-测试副本(999)-{}(138932).jcl",
                    boss_name
                );
                let contents = format!(
                    concat!(
                        "crc\t1\t1700000200\t0\t1\t{{true,\"fight\",0}}\n",
                        "crc\t2\t1700000201\t0\t8\t{{101,\"{}\",138932,0,0,0,0,0}}\n",
                        "crc\t3\t1700000202\t0\t9\t{{101,true,100,100}}\n",
                        "crc\t4\t1700000203\t0\t8\t{{303,\"{}\",140032,0,0,0,0,0}}\n",
                        "crc\t5\t1700000204\t0\t1\t{{false,\"fight\",4}}\n"
                    ),
                    boss_name, chest_name
                );
                let (temp_dir, jcl_path) = write_gbk_jcl_fixture(&file_name, &contents);
                let analysis = analyze_jcl(&jcl_path, boss_name, 138932, &bosses)
                    .expect("宝箱匹配样本应可解析");
                fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
                assert_eq!(
                    analysis.is_kill, expected_kill,
                    "配置={}，首领={}，宝箱={}", configured, boss_name, chest_name
                );
            }
        }
    }

    #[test]
    fn test_luoyang_real_fixtures_and_prefix_clustering() {
        let raid_bosses = vec![
            ("luoyangzhizhan_1".to_string(), "突利和顺".to_string()),
            ("luoyangzhizhan_2".to_string(), "田承嗣".to_string()),
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
            ("luoyangzhizhan_4".to_string(), "阿史那承庆".to_string()),
            ("luoyangzhizhan_5".to_string(), "史朝义".to_string()),
        ];
        let fixtures = [
            ("01-tuli-success.jcl", "2026-10-29-21-43-36-洛阳之战(828)-突利和顺(137976).jcl", Some("突利和顺"), true),
            ("02-tian-success.jcl", "2026-10-29-21-53-40-洛阳之战(828)-田承嗣(139113).jcl", Some("田承嗣"), true),
            ("03-shenshi-minion.jcl", "2026-10-29-22-00-50-洛阳之战(828)-拜火燎原团神使(140546).jcl", None, false),
            ("04-shibing-minion.jcl", "2026-10-29-22-01-49-洛阳之战(828)-拜火燎原团士兵(140545).jcl", None, false),
            ("05-zhuyan-reset.jcl", "2026-10-29-22-05-01-洛阳之战(828)-伊曼·逐焰(138933).jcl", Some("伊曼·寂夜"), false),
            ("06-jingye-success.jcl", "2026-10-29-22-09-07-洛阳之战(828)-伊曼·寂夜(138932).jcl", Some("伊曼·寂夜"), true),
            ("07-ashina-reset.jcl", "2026-10-29-22-22-20-洛阳之战(828)-阿史那承庆(138376).jcl", Some("阿史那承庆"), false),
            ("08-ashina-success.jcl", "2026-10-29-22-29-08-洛阳之战(828)-阿史那承庆(138376).jcl", Some("阿史那承庆"), true),
            ("09-shiyi-success.jcl", "2026-10-29-22-40-04-洛阳之战(828)-史朝义(138261).jcl", Some("史朝义"), true),
        ];
        let expected_prefix_counts = [1, 2, 2, 2, 2, 3, 3, 4, 5];
        let ten_gold_fixture: serde_json::Value = serde_json::from_str(include_str!("drop_scanner/fixtures/luoyang/ten-gold.json"))
            .expect("10金 fixture 应为合法 JSON");
        let role_uid = ten_gold_fixture["roleUid"].as_str().expect("10金 fixture 应含 roleUid");
        let reward_times: Vec<i64> = ten_gold_fixture["times"]
            .as_array()
            .expect("10金 fixture 应含 times")
            .iter()
            .map(|value| value.as_i64().expect("10金时间必须为整数"))
            .collect();
        let mut jcl_files = Vec::new();
        let mut analyses = HashMap::new();
        let mut candidates = HashMap::new();

        for (index, (fixture_file, source_file, expected_boss, expected_kill)) in fixtures.iter().enumerate() {
            let info = parse_jcl_filename(source_file).expect("fixture 源文件名应可解析");
            let (temp_dir, jcl_path) = write_luoyang_fixture(fixture_file, source_file);
            let analysis = analyze_jcl(&jcl_path, &info.boss_name, info.boss_id, &raid_bosses)
                .expect("真实脱敏 fixture 应可解析");
            if *fixture_file == "09-shiyi-success.jcl" {
                assert!(!analysis.is_kill, "史朝义的 fixture 强证据应保持不足，由伤害+10金单独补强");
                let candidate = analyze_damage_completion_candidate(&jcl_path, &info.boss_name, info.boss_id, &raid_bosses, role_uid)
                    .expect("史朝义应具有完整有效伤害候选");
                candidates.insert(info.file_name.clone(), candidate);
            }
            fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");

            assert_eq!(analysis.boss_name.as_deref(), *expected_boss, "{} BOSS 归属", fixture_file);
            assert_eq!(analysis.is_kill, *expected_kill && *fixture_file != "09-shiyi-success.jcl", "{} JCL 强击杀判定", fixture_file);
            analyses.insert(info.file_name.clone(), analysis);
            jcl_files.push(info);

            let attempt_end_times: Vec<i64> = analyses
                .values()
                .filter(|analysis| analysis.fight_start_ms > 0 && analysis.fight_end_ms >= analysis.fight_start_ms)
                .map(|analysis| analysis.fight_end_ms)
                .collect();
            let promotions = select_damage_money_promotions(&candidates, &attempt_end_times, &reward_times);
            let mut effective_analyses = analyses.clone();
            for file_name in promotions {
                effective_analyses.get_mut(&file_name).expect("补强文件必须存在").is_kill = true;
            }
            let instances = cluster_raid_instances(
                "test_account",
                jcl_files.clone(),
                &effective_analyses,
                &[],
            );
            let actual_count = instances.first().map(|instance| instance.boss_kill_count).unwrap_or(0);
            assert_eq!(actual_count, expected_prefix_counts[index], "{} 前缀聚类", fixture_file);
        }
    }

    #[test]
    fn test_damage_completion_candidate_requires_uid_bound_single_attempt() {
        let raid_bosses = vec![("boss_1".to_string(), "测试首领".to_string())];
        let source_file = "2026-10-31-21-00-00-测试副本(999)-测试首领(61001).jcl";
        let complete = concat!(
            "crc\t1\t1700000000\t0\t1\t{true,\"fixture::U123456/1\",0}\n",
            "crc\t2\t1700000001\t0\t8\t{9001,\"测试首领\",61001,0,0,0,0}\n",
            "crc\t3\t1700000002\t0\t9\t{9001,true,100,100,0,0}\n",
            "crc\t4\t1700000003\t0\t21\t{111,9001,0,0,0,0,0,0,{[13]=100,[14]=0}}\n",
            "crc\t5\t1700000004\t0\t9\t{9001,false,100,100,0,0}\n",
            "crc\t6\t1700000005\t0\t1\t{false,\"fixture::U123456/1\",5}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(source_file, complete);
        let candidate = analyze_damage_completion_candidate(&jcl_path, "测试首领", 61001, &raid_bosses, "123456");
        assert_eq!(candidate, Some(DamageCompletionCandidate { boss_name: "测试首领".to_string(), fight_end_ms: 1_700_000_005_000 }));
        assert!(analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses).is_some_and(|analysis| !analysis.is_kill), "伤害候选不能污染 JCL 强证据");
        assert!(analyze_damage_completion_candidate(&jcl_path, "测试首领", 61001, &raid_bosses, "999999").is_none(), "UID 不匹配不能补漏");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");

        let wrong_name = complete.replacen("\"测试首领\",61001", "\"同模板小怪\",61001", 1);
        let (wrong_name_dir, wrong_name_path) = write_gbk_jcl_fixture(source_file, &wrong_name);
        assert!(analyze_damage_completion_candidate(&wrong_name_path, "测试首领", 61001, &raid_bosses, "123456").is_none(), "同模板但名字不符不能借用伤害");
        fs::remove_dir_all(wrong_name_dir).expect("临时 JCL 目录应可删除");

        let wrong_template = complete.replacen("\"测试首领\",61001", "\"测试首领\",61002", 1);
        let (wrong_template_dir, wrong_template_path) = write_gbk_jcl_fixture(source_file, &wrong_template);
        assert!(analyze_damage_completion_candidate(&wrong_template_path, "测试首领", 61001, &raid_bosses, "123456").is_none(), "模板不符不能补漏");
        fs::remove_dir_all(wrong_template_dir).expect("临时 JCL 目录应可删除");

        for (label, contents) in [
            ("bad-event", complete.replacen("\t1\t{true", "\tbad\t{true", 1)),
            ("bad-type8", complete.replacen("{9001,\"测试首领\",61001", "{bad,\"测试首领\",61001", 1)),
            ("bad-line", format!("{}损坏行", complete)),
            ("end-hint", format!("{}crc\t7\t1700000006\t0\t9\t{{9001,true,100,100,0,0}}\n", complete)),
        ] {
            let (dir, path) = write_gbk_jcl_fixture(source_file, &contents);
            assert!(analyze_damage_completion_candidate(&path, "测试首领", 61001, &raid_bosses, "123456").is_none(), "{} 不能候选", label);
            fs::remove_dir_all(dir).expect("临时 JCL 目录应可删除");
        }

        let pre_npc_reentry = complete.replacen(
            "crc\t2\t1700000001\t0\t8",
            concat!(
                "crc\t2\t1700000001\t0\t9\t{9001,true,100,100,0,0}\n",
                "crc\t3\t1700000001\t0\t9\t{9001,false,100,100,0,0}\n",
                "crc\t4\t1700000001\t0\t9\t{9001,true,100,100,0,0}\n",
                "crc\t5\t1700000001\t0\t8",
            ),
            1,
        );
        let (pre_npc_dir, pre_npc_path) = write_gbk_jcl_fixture(source_file, &pre_npc_reentry);
        assert!(analyze_damage_completion_candidate(&pre_npc_path, "测试首领", 61001, &raid_bosses, "123456").is_none(), "NPC_INFO 前重入不能被延迟归属绕过");
        fs::remove_dir_all(pre_npc_dir).expect("临时 JCL 目录应可删除");

        let reentered = complete.replace(
            "crc\t5\t1700000004\t0\t9\t{9001,false,100,100,0,0}\n",
            concat!(
                "crc\t5\t1700000004\t0\t9\t{9001,false,100,100,0,0}\n",
                "crc\t6\t1700000005\t0\t9\t{9001,true,100,100,0,0}\n",
            ),
        );
        let (reenter_dir, reenter_path) = write_gbk_jcl_fixture(source_file, &reentered);
        assert!(analyze_damage_completion_candidate(&reenter_path, "测试首领", 61001, &raid_bosses, "123456").is_none(), "同运行时 false->true 重入不能补漏");
        fs::remove_dir_all(reenter_dir).expect("临时 JCL 目录应可删除");
    }

    #[test]
    fn test_damage_money_promotions_rejects_nearby_attempt_and_ambiguous_rewards() {
        let candidate = |end_sec: i64| DamageCompletionCandidate { boss_name: "测试首领".to_string(), fight_end_ms: end_sec * 1000 };
        let mut candidates = HashMap::new();
        candidates.insert("candidate.jcl".to_string(), candidate(1_000));

        assert_eq!(
            select_damage_money_promotions(&candidates, &[1_000_000], &[1_005]),
            HashSet::from(["candidate.jcl".to_string()]),
            "一笔唯一且邻近的 10 金可补强"
        );
        assert_eq!(
            select_damage_money_promotions(&candidates, &[1_000_000], &[1_015]),
            HashSet::from(["candidate.jcl".to_string()]),
            "±15 秒边界应包含"
        );
        assert!(
            select_damage_money_promotions(&candidates, &[1_000_000], &[1_016]).is_empty(),
            "±16 秒必须排除"
        );
        assert!(
            select_damage_money_promotions(&candidates, &[1_000_000, 1_012_000], &[1_005]).is_empty(),
            "同一笔钱同时邻近强证据或未达标的另一尝试时不能借用"
        );
        assert_eq!(
            select_damage_money_promotions(&candidates, &[1_000_000], &[1_005, 1_007]),
            HashSet::from(["candidate.jcl".to_string()]),
            "同一场多笔 10 金只要存在唯一归属的一笔，仍只补强一次"
        );
    }

    #[test]
    fn test_type1_end_anchors_include_all_attempts_even_without_boss_config() {
        let contents = concat!(
            "crc\t1\t1700000000\t0\t1\t{true,\"fixture::U123456/1\",0}\n",
            "crc\t2\t1700000010\t0\t1\t{false,\"fixture::U123456/1\",10}\n",
            "crc\t3\t1700000020\t0\t1\t{true,\"fixture::U123456/1\",0}\n",
            "crc\t4\t1700000030\t0\t1\t{false,\"fixture::U123456/1\",10}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture("unconfigured-two-attempts.jcl", contents);
        assert_eq!(
            collect_type1_end_anchors(&jcl_path, "123456"),
            Some(vec![1_700_000_010_000, 1_700_000_030_000]),
            "同文件多次 type1 false 都必须成为金额争用锚"
        );
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
    }

    #[test]
    fn test_chatlog_user_global_id_normalization_is_strict() {
        assert_eq!(normalize_chatlog_user_global_id("123456"), Some("123456".to_string()));
        assert_eq!(normalize_chatlog_user_global_id("\"123456\""), Some("123456".to_string()));
        assert_eq!(normalize_chatlog_user_global_id("\"999999\""), Some("999999".to_string()), "错 UID 必须保留给调用方精确比较");
        assert_eq!(normalize_chatlog_user_global_id("null"), None);
        assert_eq!(normalize_chatlog_user_global_id("\"12x\""), None);
        assert_eq!(normalize_chatlog_user_global_id("prefix123456"), None);
    }

    #[test]
    fn test_exact_boss_reward_requires_all_currency_components() {
        let exact = r#"<text text="10" name="Text_Gold"/><text text="0" name="Text_Silver"/><text text="0" name="Text_Copper"/>"#;
        let missing_copper = r#"<text text="10" name="Text_Gold"/><text text="0" name="Text_Silver"/>"#;
        let non_zero_silver = r#"<text text="10" name="Text_Gold"/><text text="1" name="Text_Silver"/><text text="0" name="Text_Copper"/>"#;
        assert!(is_exact_boss_kill_reward_msg(exact));
        assert!(!is_exact_boss_kill_reward_msg(missing_copper), "补漏不能把缺省币种当成0");
        assert!(!is_exact_boss_kill_reward_msg(non_zero_silver));
    }

    /// 路径 B：仅出现寂夜 NPC_INFO（如出场对话）时，不能归属给寂夜。
    #[test]
    fn test_analyze_jcl_luoyang_dialogue_only_is_not_attributed_to_jiye() {
        let raid_bosses = vec![
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
        ];
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "luoyang-dialogue-only.jcl",
            concat!(
                "crc\t1\t1700000100\t0\t8\t{101,\"伊曼·逐焰\",61001,0,0,0,0,0}\n",
                "crc\t2\t1700000101\t0\t8\t{202,\"伊曼·寂夜\",61002,0,0,0,0,0}\n",
            ),
        );

        let analysis = analyze_jcl(&jcl_path, "伊曼·逐焰", 61001, &raid_bosses)
            .expect("临时 JCL 应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");

        assert_eq!(analysis.boss_name, None, "未实际进战的寂夜不能归属");
        assert!(!analysis.is_kill, "出场对话不能记为击杀");
    }

    /// 两份逐焰文件都记录到同一个寂夜完成信号时，只计一次寂夜，且全通仍为 5 个正式 BOSS。
    #[test]
    fn test_luoyang_jcl_path_b_maps_jiye_and_deduplicates_full_clear() {
        let raid_bosses = vec![
            ("luoyangzhizhan_1".to_string(), "突利和顺".to_string()),
            ("luoyangzhizhan_2".to_string(), "田承嗣".to_string()),
            ("luoyangzhizhan_3".to_string(), "伊曼·寂夜".to_string()),
            ("luoyangzhizhan_4".to_string(), "阿史那承庆".to_string()),
            ("luoyangzhizhan_5".to_string(), "史朝义".to_string()),
        ];
        let clear_jcl = concat!(
            "crc\t1\t1700000200\t0\t1\t{true,\"fight\",0}\n",
            "crc\t2\t1700000201\t0\t8\t{101,\"伊曼·逐焰\",61001,0,0,0,0,0}\n",
            "crc\t3\t1700000202\t0\t8\t{202,\"伊曼·寂夜\",61002,0,0,0,0,0}\n",
            "crc\t4\t1700000203\t0\t9\t{101,true,100}\n",
            "crc\t5\t1700000204\t0\t9\t{202,true,100}\n",
            "crc\t6\t1700000205\t0\t28\t{101,999}\n",
            "crc\t7\t1700000206\t0\t28\t{202,999}\n",
            "crc\t8\t1700000207\t0\t1\t{false,\"fight\",7}\n",
        );
        let source_file = "2026-10-29-22-09-07-洛阳之战(828)-伊曼·逐焰(138933).jcl";
        let (first_dir, first_path) = write_gbk_jcl_fixture(source_file, clear_jcl);
        let (second_dir, second_path) = write_gbk_jcl_fixture(source_file, clear_jcl);
        let first_analysis = analyze_jcl(&first_path, "伊曼·逐焰", 61001, &raid_bosses)
            .expect("首份逐焰 JCL 应可解析");
        let second_analysis = analyze_jcl(&second_path, "伊曼·逐焰", 61001, &raid_bosses)
            .expect("第二份逐焰 JCL 应可解析");
        fs::remove_dir_all(first_dir).expect("首份临时 JCL 目录应可删除");
        fs::remove_dir_all(second_dir).expect("第二份临时 JCL 目录应可删除");

        assert!(first_analysis.is_kill && second_analysis.is_kill, "双实体完成应判定为击杀");
        assert_eq!(first_analysis.boss_name.as_deref(), Some("伊曼·寂夜"));
        assert_eq!(second_analysis.boss_name.as_deref(), Some("伊曼·寂夜"));

        let timestamp = |offset_minutes: i64| 1_700_000_000_000 + offset_minutes * 60_000;
        let names = ["突利和顺", "田承嗣", "阿史那承庆", "史朝义"];
        let mut jcl_files = Vec::new();
        let mut analyses = HashMap::new();
        for (index, boss_name) in names.iter().enumerate().take(2) {
            let file_name = format!("luoyang-{}-{}.jcl", index, boss_name);
            jcl_files.push(JclFileInfo {
                file_name: file_name.clone(), timestamp: timestamp(index as i64),
                raid_display_name: "25人英雄洛阳之战".to_string(), raid_id: 900,
                boss_name: (*boss_name).to_string(), boss_id: 62000 + index as i64,
            });
            analyses.insert(file_name, JclAnalysis {
                boss_name: Some((*boss_name).to_string()), fight_start_ms: timestamp(index as i64),
                fight_end_ms: timestamp(index as i64) + 30_000, is_kill: true,
            });
        }
        for (index, analysis) in [first_analysis, second_analysis].into_iter().enumerate() {
            let file_name = format!("luoyang-jiyen-{}.jcl", index);
            jcl_files.push(JclFileInfo {
                file_name: file_name.clone(), timestamp: timestamp(2 + index as i64),
                raid_display_name: "25人英雄洛阳之战".to_string(), raid_id: 900,
                boss_name: "伊曼·逐焰".to_string(), boss_id: 61001,
            });
            analyses.insert(file_name, analysis);
        }
        for (index, boss_name) in names.iter().enumerate().skip(2) {
            let file_name = format!("luoyang-{}-{}.jcl", index + 2, boss_name);
            jcl_files.push(JclFileInfo {
                file_name: file_name.clone(), timestamp: timestamp(4 + index as i64),
                raid_display_name: "25人英雄洛阳之战".to_string(), raid_id: 900,
                boss_name: (*boss_name).to_string(), boss_id: 62000 + index as i64,
            });
            analyses.insert(file_name, JclAnalysis {
                boss_name: Some((*boss_name).to_string()), fight_start_ms: timestamp(4 + index as i64),
                fight_end_ms: timestamp(4 + index as i64) + 30_000, is_kill: true,
            });
        }

        let instances = cluster_raid_instances("test_account", jcl_files, &analyses, &[]);
        assert_eq!(instances.len(), 1, "同一次洛阳之战应只形成一个副本实例");
        let instance = &instances[0];
        assert_eq!(instance.boss_kill_count, 5, "同本体两份日志不能把全通多计为 6 杀");
        assert_eq!(
            instance.jcl_boss_names,
            vec!["突利和顺", "田承嗣", "伊曼·寂夜", "阿史那承庆", "史朝义"],
            "逐焰应映射为正式 BOSS 寂夜"
        );
        let (boss_ids, boss_names) = map_boss_names_to_ids(&instance.bosses_killed, &raid_bosses);
        assert_eq!(boss_ids.len(), 5, "全通应映射为 5 个正式 BOSS ID");
        assert_eq!(boss_ids[2], "luoyangzhizhan_3");
        assert_eq!(boss_names[2], "伊曼·寂夜");
    }

    #[test]
    fn test_path_a_accepts_death_notify_or_hp_zero_for_any_raid_metadata() {
        let raid_bosses = vec![
            ("business_id_changes_do_not_matter".to_string(), "测试首领".to_string()),
        ];
        let death_signals = [
            "crc\t4\t1700000403\t0\t28\t{101,999}\n",
            "crc\t4\t1700000403\t0\t9\t{101,false,0,100}\n",
        ];
        for death_signal in death_signals {
            let contents = format!(
                "crc\t1\t1700000400\t0\t1\t{{true,\"fight\",0}}\n\
                 crc\t2\t1700000401\t0\t8\t{{101,\"测试首领\",139113,0,0,0,0,0}}\n\
                 crc\t3\t1700000402\t0\t9\t{{101,true,100,100}}\n{}",
                death_signal
            );
            let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
                "2026-10-29-21-53-40-任意副本(999)-测试首领(139113).jcl",
                &contents,
            );
            let analysis = analyze_jcl(&jcl_path, "测试首领", 139113, &raid_bosses)
                .expect("路径 A 可靠死亡 JCL 应可解析");
            fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
            assert!(analysis.is_kill, "路径 A 的可靠死亡证据应判定为击杀");
        }
    }

    #[test]
    fn test_path_b_requires_reliable_death_for_both_entities() {
        let raid_bosses = vec![
            ("unrelated_business_id".to_string(), "配置首领".to_string()),
        ];
        let prefix = concat!(
            "crc\t1\t1700000500\t0\t1\t{true,\"fight\",0}\n",
            "crc\t2\t1700000501\t0\t8\t{101,\"文件实体\",61001,0,0,0,0,0}\n",
            "crc\t3\t1700000502\t0\t8\t{202,\"配置首领\",61002,0,0,0,0,0}\n",
            "crc\t4\t1700000503\t0\t9\t{101,true,100,100}\n",
            "crc\t5\t1700000504\t0\t9\t{202,true,100,100}\n",
        );
        let source_file = "2026-10-29-22-09-07-任意副本(999)-文件实体(61001).jcl";
        let two_hp_zero = format!(
            "{}crc\t6\t1700000505\t0\t9\t{{101,false,0,100}}\n\
             crc\t7\t1700000506\t0\t9\t{{202,false,0,100}}\n",
            prefix
        );
        let (success_dir, success_path) = write_gbk_jcl_fixture(source_file, &two_hp_zero);
        let success = analyze_jcl(&success_path, "文件实体", 61001, &raid_bosses)
            .expect("路径 B 双实体死亡 JCL 应可解析");
        fs::remove_dir_all(success_dir).expect("临时 JCL 目录应可删除");
        assert!(success.is_kill, "路径 B 两个实体均 HP 归零应判定为击杀");

        let only_clone_died = format!("{}crc\t6\t1700000505\t0\t28\t{{101,999}}\n", prefix);
        let (wipe_dir, wipe_path) = write_gbk_jcl_fixture(source_file, &only_clone_died);
        let wipe = analyze_jcl(&wipe_path, "文件实体", 61001, &raid_bosses)
            .expect("路径 B 单实体死亡 JCL 应可解析");
        fs::remove_dir_all(wipe_dir).expect("临时 JCL 目录应可删除");
        assert!(!wipe.is_kill, "路径 B 仅一方死亡不能判定配置首领完成");

        let real_reset_without_refight = format!(
            "{}crc\t6\t1700000505\t0\t9\t{{101,false,0,100}}\n\
             crc\t7\t1700000506\t0\t9\t{{202,false,0,100}}\n\
             crc\t8\t1700000507\t0\t9\t{{202,false,100,100}}\n\
             crc\t9\t1700000508\t0\t28\t{{202,999}}\n",
            prefix
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(source_file, &real_reset_without_refight);
        let analysis = analyze_jcl(&jcl_path, "文件实体", 61001, &raid_bosses)
            .expect("路径 B 复位反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(
            !analysis.is_kill,
            "配置实体复位后未重新参战，不能复用前一轮参战与后一轮死亡"
        );

        let merged_runtime_id = concat!(
            "crc\t1\t1700000520\t0\t8\t{101,\"文件实体\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000521\t0\t8\t{101,\"配置首领\",61002,0,0,0,0,0}\n",
            "crc\t3\t1700000522\t0\t9\t{101,true,100,100}\n",
            "crc\t4\t1700000523\t0\t28\t{101,999}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(source_file, merged_runtime_id);
        let analysis = analyze_jcl(&jcl_path, "文件实体", 61001, &raid_bosses)
            .expect("路径 B 同一实体反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(
            analysis.is_kill,
            "路径 B 的文件实体与配置实体明确解析为同一 dwID 时，可按单一正式实体确认"
        );

        let chest_without_real_participation = concat!(
            "crc\t1\t1700000510\t0\t8\t{101,\"文件实体\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000511\t0\t8\t{202,\"配置首领\",61002,0,0,0,0,0}\n",
            "crc\t3\t1700000512\t0\t9\t{101,true,100,100}\n",
            "crc\t4\t1700000513\t0\t8\t{303,\"配置首领宝箱\",0,0,0,0,0,0}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(source_file, chest_without_real_participation);
        let analysis = analyze_jcl(&jcl_path, "文件实体", 61001, &raid_bosses)
            .expect("未参战的配置首领宝箱反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(
            !analysis.is_kill,
            "配置首领未参战时，宝箱文本不能将小怪文件确认成首领击杀"
        );

        let chest_after_real_disengage = concat!(
            "crc\t1\t1700000530\t0\t8\t{101,\"文件实体\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000531\t0\t8\t{202,\"配置首领\",61002,0,0,0,0,0}\n",
            "crc\t3\t1700000532\t0\t9\t{202,true,100,100}\n",
            "crc\t4\t1700000533\t0\t28\t{202,999}\n",
            "crc\t5\t1700000534\t0\t9\t{202,false,100,100}\n",
            "crc\t6\t1700000535\t0\t8\t{303,\"配置首领宝箱\",0,0,0,0,0,0}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(source_file, chest_after_real_disengage);
        let analysis = analyze_jcl(&jcl_path, "文件实体", 61001, &raid_bosses)
            .expect("配置首领宝箱正例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(analysis.is_kill, "曾参战的配置首领离战后出现专属宝箱仍应确认");
        assert_eq!(analysis.boss_name.as_deref(), Some("配置首领"));
    }

    #[test]
    fn test_path_a_requires_the_same_participating_runtime_entity() {
        let raid_bosses = vec![("business_id".to_string(), "测试首领".to_string())];
        let prefix = concat!(
            "crc\t1\t1700000600\t0\t1\t{true,\"fight\",0}\n",
            "crc\t2\t1700000601\t0\t8\t{101,\"测试首领\",61001,0,0,0,0,0}\n",
            "crc\t3\t1700000602\t0\t8\t{202,\"测试首领\",61002,0,0,0,0,0}\n",
            "crc\t4\t1700000603\t0\t9\t{101,true,100,100}\n",
        );
        let cases = [
            ("陌生实体死亡", format!("{}crc\t5\t1700000604\t0\t28\t{{303,999}}\n", prefix)),
            ("同名另一实体死亡", format!("{}crc\t5\t1700000604\t0\t28\t{{202,999}}\n", prefix)),
            (
                "死亡后同一实体重新参战",
                format!(
                    "{}crc\t5\t1700000604\t0\t28\t{{101,999}}\ncrc\t6\t1700000605\t0\t9\t{{101,true,100,100}}\n",
                    prefix
                ),
            ),
        ];
        for (case_name, contents) in cases {
            let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
                "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
                &contents,
            );
            let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
                .expect("路径 A 反例应可解析");
            fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
            assert!(!analysis.is_kill, "{} 不得确认击杀", case_name);
        }

        let no_participation = concat!(
            "crc\t1\t1700000600\t0\t8\t{101,\"测试首领\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000601\t0\t28\t{101,999}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
            no_participation,
        );
        let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
            .expect("未参战反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(!analysis.is_kill, "未参战实体的死亡通知不能确认击杀");

        let death_then_reset_then_death = concat!(
            "crc\t1\t1700000600\t0\t8\t{101,\"测试首领\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000601\t0\t9\t{101,true,100,100}\n",
            "crc\t3\t1700000602\t0\t28\t{101,999}\n",
            "crc\t4\t1700000603\t0\t9\t{101,false,100,100}\n",
            "crc\t5\t1700000604\t0\t28\t{101,999}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
            death_then_reset_then_death,
        );
        let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
            .expect("复位后无重战的死亡反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(!analysis.is_kill, "同一实体复位后无重新参战的死亡不能确认击杀");

        let other_same_name_disengages = format!(
            "{}crc\t5\t1700000604\t0\t28\t{{101,999}}\ncrc\t6\t1700000605\t0\t9\t{{202,false,100,100}}\n",
            prefix
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
            &other_same_name_disengages,
        );
        let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
            .expect("同名实体脱战正例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(analysis.is_kill, "另一同名实体的满血脱战不能撤销正式实体的终局死亡");

        let other_same_name_refights = format!(
            "{}crc\t5\t1700000604\t0\t28\t{{101,999}}\ncrc\t6\t1700000605\t0\t9\t{{202,true,100,100}}\n",
            prefix
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
            &other_same_name_refights,
        );
        let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
            .expect("同名实体重新参战反例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(!analysis.is_kill, "另一同名实体继续参战时保守不确认击杀");

        let reset_then_refight = concat!(
            "crc\t1\t1700000610\t0\t8\t{101,\"测试首领\",61001,0,0,0,0,0}\n",
            "crc\t2\t1700000611\t0\t9\t{101,true,100,100}\n",
            "crc\t3\t1700000612\t0\t28\t{101,999}\n",
            "crc\t4\t1700000613\t0\t9\t{101,false,100,100}\n",
            "crc\t5\t1700000614\t0\t9\t{101,true,100,100}\n",
            "crc\t6\t1700000615\t0\t28\t{101,999}\n",
        );
        let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
            "2026-10-29-23-00-00-任意副本(999)-测试首领(61001).jcl",
            reset_then_refight,
        );
        let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
            .expect("重新参战后的死亡正例应可解析");
        fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
        assert!(analysis.is_kill, "复位后重新参战再死亡可确认击杀");
    }

    #[test]
    fn test_system_messages_do_not_change_entity_evidence() {
        let raid_bosses = vec![("business_1".to_string(), "测试首领".to_string())];
        let base = concat!(
            "crc\t1\t1700001000\t0\t1\t{true,\"fight\",0}\n",
            "crc\t2\t1700001001\t0\t8\t{101,\"测试首领\",61001,0,0,0,0,0}\n",
            "crc\t3\t1700001002\t0\t9\t{101,true,100,100}\n",
        );
        let messages = [
            "crc\t4\t1700001003\t0\t18\t{\"和徒弟共同完成满级团队秘境，增加师徒值。\\n\",\"MSG_SYS\"}\n",
            "crc\t4\t1700001003\t0\t18\t{\"[全胜！任意副本]：1/1，击败测试首领(完成)\\n\",\"MSG_SYS\"}\n",
            "crc\t4\t1700001003\t0\t18\t{\"和徒弟共同完成武林通鉴·团队秘境，增加师徒值。\\n\",\"MSG_SYS\"}\n",
            "crc\t4\t1700001003\t0\t18\t{\"你获得10金。\\n\",\"MSG_SYS\"}\n",
        ];

        for message in messages {
            let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
                "2026-10-30-10-00-00-任意副本(999)-测试首领(61001).jcl",
                &format!("{}{}", base, message),
            );
            let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
                .expect("消息无实体证据反例应可解析");
            fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
            assert!(!analysis.is_kill, "消息不能替代实体完成证据");
        }

        let death = "crc\t4\t1700001003\t0\t28\t{101,999}\n";
        let chest = "crc\t4\t1700001003\t0\t8\t{303,\"测试首领宝箱\",0,0,0,0,0,0}\n";
        for evidence in [death, chest] {
            for suffix in ["", messages[0]] {
                let (temp_dir, jcl_path) = write_gbk_jcl_fixture(
                    "2026-10-30-10-00-00-任意副本(999)-测试首领(61001).jcl",
                    &format!("{}{}{}", base, evidence, suffix),
                );
                let analysis = analyze_jcl(&jcl_path, "测试首领", 61001, &raid_bosses)
                    .expect("独立实体证据正例应可解析");
                fs::remove_dir_all(temp_dir).expect("临时 JCL 目录应可删除");
                assert!(analysis.is_kill, "消息增加或删除不应改变独立实体证据");
            }
        }
    }

    /// 测试按 JCL 顺序聚类：连续相同副本名归为一组，副本名变化开始新组。
    /// 不再使用30分钟间隔聚类。
    #[test]
    fn test_cluster_raid_instances_sequential_order() {
        // 时间戳（毫秒），基于 2026-06-29 UTC+8
        let ts = |h: u32, m: u32, s: u32| -> i64 {
            (1782691200 + (h as i64) * 3600 + (m as i64) * 60 + s as i64) * 1000
        };

        let jcl_files = vec![
            // 22:10 笑妆娘 — 拉托
            JclFileInfo {
                file_name: "2026-06-29-22-10-53-25人英雄阆风悬城(795)-笑妆娘(137205).jcl".to_string(),
                timestamp: ts(22, 10, 53),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137205,
            },
            // 22:33 笑妆娘 — 成功
            JclFileInfo {
                file_name: "2026-06-29-22-33-51-25人英雄阆风悬城(795)-笑妆娘(137205).jcl".to_string(),
                timestamp: ts(22, 33, 51),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "笑妆娘".to_string(),
                boss_id: 137205,
            },
            // 22:41 唐醉 — 成功
            JclFileInfo {
                file_name: "2026-06-29-22-41-58-25人英雄阆风悬城(795)-唐醉(137117).jcl".to_string(),
                timestamp: ts(22, 41, 58),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "唐醉".to_string(),
                boss_id: 137117,
            },
            // 22:52 柳公子 — 成功
            JclFileInfo {
                file_name: "2026-06-29-22-52-20-25人英雄阆风悬城(795)-柳公子(137135).jcl".to_string(),
                timestamp: ts(22, 52, 20),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "柳公子".to_string(),
                boss_id: 137135,
            },
            // 23:03 阿史那承庆 — 成功
            JclFileInfo {
                file_name: "2026-06-29-23-03-37-25人英雄阆风悬城(795)-阿史那承庆(137130).jcl".to_string(),
                timestamp: ts(23, 3, 37),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "阿史那承庆".to_string(),
                boss_id: 137130,
            },
            // 23:14 须罗巨傀（唐怀仁战斗中的小怪）— 成功
            JclFileInfo {
                file_name: "2026-06-29-23-14-04-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl".to_string(),
                timestamp: ts(23, 14, 4),
                raid_display_name: "25人英雄阆风悬城".to_string(),
                raid_id: 795,
                boss_name: "须罗巨傀".to_string(),
                boss_id: 137175,
            },
        ];

        // 构造 JCL 分析结果
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        // 22:10 笑妆娘 — 拉托
        jcl_analyses.insert(
            "2026-06-29-22-10-53-25人英雄阆风悬城(795)-笑妆娘(137205).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: ts(22, 10, 53), fight_end_ms: ts(22, 16, 0), is_kill: false },
        );
        // 22:33 笑妆娘 — 通关
        jcl_analyses.insert(
            "2026-06-29-22-33-51-25人英雄阆风悬城(795)-笑妆娘(137205).jcl".to_string(),
            JclAnalysis { boss_name: Some("笑妆娘".to_string()), fight_start_ms: ts(22, 33, 51), fight_end_ms: ts(22, 38, 20), is_kill: true },
        );
        // 22:41 唐醉 — 通关
        jcl_analyses.insert(
            "2026-06-29-22-41-58-25人英雄阆风悬城(795)-唐醉(137117).jcl".to_string(),
            JclAnalysis { boss_name: Some("唐醉".to_string()), fight_start_ms: ts(22, 41, 58), fight_end_ms: ts(22, 47, 24), is_kill: true },
        );
        // 22:52 柳公子 — 通关
        jcl_analyses.insert(
            "2026-06-29-22-52-20-25人英雄阆风悬城(795)-柳公子(137135).jcl".to_string(),
            JclAnalysis { boss_name: Some("柳公子".to_string()), fight_start_ms: ts(22, 52, 20), fight_end_ms: ts(22, 58, 28), is_kill: true },
        );
        // 23:03 阿史那承庆 — 通关
        jcl_analyses.insert(
            "2026-06-29-23-03-37-25人英雄阆风悬城(795)-阿史那承庆(137130).jcl".to_string(),
            JclAnalysis { boss_name: Some("阿史那承庆".to_string()), fight_start_ms: ts(23, 3, 37), fight_end_ms: ts(23, 10, 22), is_kill: true },
        );
        // 23:14 须罗巨傀 — 通关（内容搜索到"唐怀仁"）
        jcl_analyses.insert(
            "2026-06-29-23-14-04-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl".to_string(),
            JclAnalysis { boss_name: Some("唐怀仁".to_string()), fight_start_ms: ts(23, 14, 4), fight_end_ms: ts(23, 21, 59), is_kill: true },
        );

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[]);

        // 应该只有1个副本实例（连续相同副本名）
        assert_eq!(instances.len(), 1, "应聚类为1个副本实例");
        let inst = &instances[0];

        // 通关 JCL 数应为5（22:10 拉托被过滤）
        assert_eq!(
            inst.boss_kill_count, 5,
            "通关 JCL 数应为5（22:10 笑妆娘拉托被过滤）"
        );

        // 成功JCL文件应为5个（不含22:10 拉托）
        assert_eq!(inst.jcl_files.len(), 5, "成功JCL文件数应为5");
        assert!(
            !inst.jcl_files.iter().any(|f| f.contains("22-10-53")),
            "22:10 拉托 JCL 不应出现在成功JCL列表中"
        );

        // 验证BOSS名列表（须罗巨傀 JCL 的 boss_name 应为"唐怀仁"）
        assert_eq!(
            inst.jcl_boss_names,
            vec!["笑妆娘", "唐醉", "柳公子", "阿史那承庆", "唐怀仁"]
        );
    }

    /// 测试 parse_money_from_msg 解析万金（Text_GoldB）字段
    ///
    /// JX3 chatlog 中金额≥10000金时，msg 字段会拆分为 Text_GoldB（万金）和 Text_Gold（金）。
    /// 例如 16082金 = Text_GoldB="1" + Text_Gold="6082"
    #[test]
    fn test_parse_money_with_goldb() {
        // 真实 msg：16082金（1万金 + 6082金）
        let msg = r#"<text>text="你获得：" font= font=10 r=170 g=150 b=30 </text><text>text="1"  font=10 r=170 g=150 b=30  name="Text_GoldB" eventid=0 valign=1 </text><animate>path="UI/Image/Common/Money.UITex"  name="Image_GoldB" eventid=0 group=41  </animate><text>text="6082"  font=10 r=170 g=150 b=30  name="Text_Gold" eventid=0 valign=1 </text><image>path="UI/Image/Common/Money.UITex"  name="Image_Gold" eventid=0 frame=0  </image><text>text="0"  font=10 r=170 g=150 b=30  name="Text_Silver" eventid=0 valign=1 </text><image>path="UI/Image/Common/Money.UITex"  name="Image_Silver" eventid=0 frame=2  </image><text>text="0"  font=10 r=170 g=150 b=30  name="Text_Copper" eventid=0 valign=1 </text><image>path="UI/Image/Common/Money.UITex"  name="Image_Copper" eventid=0 frame=1  </image><text>text="。" font= font=10 r=170 g=150 b=30 </text>"#;

        let result = parse_money_from_msg(msg).expect("应解析成功");
        assert_eq!(result, (16082, 0, 0), "1万金+6082金=16082金");
    }

    /// 测试 parse_money_from_msg 解析普通金额（无 Text_GoldB）
    #[test]
    fn test_parse_money_without_goldb() {
        // 10金（BOSS击杀奖励）
        let msg = r#"<text>text="你获得：" font= font=10 r=170 g=150 b=30 </text><text>text="10"  font=10 r=170 g=150 b=30  name="Text_Gold" eventid=0 valign=1 </text><image>path="UI/Image/Common/Money.UITex"  name="Image_Gold" eventid=0 frame=0  </image><text>text="0"  font=10 r=170 g=150 b=30  name="Text_Silver" eventid=0 valign=1 </text><text>text="0"  font=10 r=170 g=150 b=30  name="Text_Copper" eventid=0 valign=1 </text>"#;
        let result = parse_money_from_msg(msg).expect("应解析成功");
        assert_eq!(result, (10, 0, 0), "10金");

        // 147金76银53铜
        let msg2 = r#"<text>text="你获得：" font= font=10 r=170 g=150 b=30 </text><text>text="147"  font=10 r=170 g=150 b=30  name="Text_Gold" eventid=0 valign=1 </text><text>text="76"  font=10 r=170 g=150 b=30  name="Text_Silver" eventid=0 valign=1 </text><text>text="53"  font=10 r=170 g=150 b=30  name="Text_Copper" eventid=0 valign=1 </text>"#;
        let result2 = parse_money_from_msg(msg2).expect("应解析成功");
        assert_eq!(result2, (147, 76, 53), "147金76银53铜");
    }

    /// 测试 parse_expense_amount 解析支出金额字符串
    #[test]
    fn test_parse_expense_amount() {
        // 纯金
        assert_eq!(parse_expense_amount("2000金"), 2000, "纯金");
        // 0金
        assert_eq!(parse_expense_amount("0金"), 0, "0金");
        // 纯金砖
        assert_eq!(parse_expense_amount("1金砖"), 10000, "1金砖=10000金");
        assert_eq!(parse_expense_amount("4金砖"), 40000, "4金砖=40000金");
        // 金砖+金
        assert_eq!(parse_expense_amount("1金砖6000金"), 16000, "1金砖6000金=16000金");
        assert_eq!(parse_expense_amount("9金砖9000金"), 99000, "9金砖9000金=99000金");
        assert_eq!(parse_expense_amount("2金砖9000金"), 29000, "2金砖9000金=29000金");
    }

    // ============== extract_drops_from_records 购买数量配对单元测试 ==============

    /// 测试辅助：构造一条 chatlog 记录
    fn chat_rec(msg_type: &str, text: &str, time_sec: i64) -> ChatlogRecord {
        ChatlogRecord {
            msg_type: msg_type.to_string(),
            text: text.to_string(),
            msg: String::new(),
            time_sec,
        }
    }

    /// 测试辅助：调用 extract_drops_from_records 并返回 purchased_items
    /// （时间范围按记录本身的最小/最大时间各放宽 60 秒）
    fn extract_purchased(records: Vec<ChatlogRecord>) -> HashMap<String, PurchaseInfo> {
        let lo = records.iter().map(|r| r.time_sec).min().unwrap_or(0);
        let hi = records.iter().map(|r| r.time_sec).max().unwrap_or(0);
        let (_, _, _, _, _, _, _, purchased) = extract_drops_from_records(
            &records,
            (lo - 60) * 1000,
            (hi + 60) * 1000,
            "少年白了发",
        );
        purchased
    }

    /// 购买消息前后 2 秒内的"你获得 ×N"应补充真实数量（含多条累加）
    #[test]
    fn test_purchase_qty_paired_from_gain_events() {
        let t = 1787587544i64;
        let records = vec![
            chat_rec("MSG_ITEM", "你获得：[玛瑙] × 17。", t),
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[玛瑙]", t + 1),
            // 同秒两条独立到账，被同一笔购买消息吸收
            chat_rec("MSG_ITEM", "你获得：[肆级五彩石]。", t + 5),
            chat_rec("MSG_ITEM", "你获得：[肆级五彩石]。", t + 5),
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[肆级五彩石]", t + 6),
            // 带后缀 ×2 的单条到账
            chat_rec("MSG_ITEM", "你获得：[上品茶饼·兑] × 2。", t + 10),
            chat_rec("MSG_WHISPER", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[上品茶饼·兑]", t + 11),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["玛瑙"].count, 17, "×17 到账应计入数量");
        assert_eq!(p["玛瑙"].total_price, 0);
        assert_eq!(p["肆级五彩石"].count, 2, "同秒两条 ×1 到账应累加");
        assert_eq!(p["上品茶饼·兑"].count, 2, "×2 后缀应解析为 2");
    }

    /// ±2 秒窗口边界：恰好 2 秒内可配对，3 秒外不可
    #[test]
    fn test_purchase_qty_pair_window_boundary() {
        let t = 1700000000i64;
        let records = vec![
            chat_rec("MSG_ITEM", "你获得：[猫眼石] × 6。", t - 2), // 恰好 2 秒前 → 配对
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[猫眼石]", t),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["猫眼石"].count, 6, "|Δt|=2 应在窗口内");

        let records2 = vec![
            chat_rec("MSG_ITEM", "你获得：[猫眼石] × 6。", t - 3), // 3 秒前 → 窗口外
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[猫眼石]", t),
        ];
        let p2 = extract_purchased(records2);
        assert_eq!(p2["猫眼石"].count, 1, "|Δt|=3 应超出窗口，回退数量 1");
    }

    /// 双频道回显：无独立到账事件、同物品+同金额+间隔 ≤3 秒的第二条消息应丢弃
    #[test]
    fn test_echo_duplicate_suppressed_without_grant() {
        let t = 1700000100i64;
        let records = vec![
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[5000金]购买了[雨夜禅]", t),
            chat_rec("MSG_WHISPER", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[5000金]购买了[雨夜禅]", t + 1),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["雨夜禅"].count, 1, "回显消息不应重复计数");
        assert_eq!(p["雨夜禅"].total_price, 5000, "回显消息不应重复累计金额");
    }

    /// 有独立到账事件时，间隔很近的两笔真实交易都应保留
    #[test]
    fn test_two_real_deals_with_own_grants_both_counted() {
        let t = 1700000200i64;
        let records = vec![
            chat_rec("MSG_ITEM", "你获得：[玄阙帽·五毒]。", t),
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[玄阙帽·五毒]", t),
            chat_rec("MSG_ITEM", "你获得：[玄阙帽·五毒]。", t + 1),
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[玄阙帽·五毒]", t + 1),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["玄阙帽·五毒"].count, 2, "两笔交易各有到账，均应计数");
    }

    /// 无到账日志的孤立购买回退数量 1；他人购买不统计
    #[test]
    fn test_fallback_and_other_buyer_filtered() {
        let t = 1700000300i64;
        let records = vec![
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[神藏链]", t),
            chat_rec("MSG_ROOM", "[房间][糯闪·梦江南]：[糯闪·梦江南]花费[0金]购买了[神藏链]", t + 1),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["神藏链"].count, 1, "仅统计目标角色，且回退数量 1");
    }

    /// 其他玩家的"XX获得："不是本地视角，不参与数量配对
    #[test]
    fn test_other_player_gain_not_paired() {
        let t = 1700000400i64;
        let records = vec![
            chat_rec("MSG_ITEM", "宴鸢·梦江南获得：[玛瑙] × 17。", t),
            chat_rec("MSG_ROOM", "[房间][少年白了发·梦江南]：[少年白了发·梦江南]花费[0金]购买了[玛瑙]", t + 1),
        ];
        let p = extract_purchased(records);
        assert_eq!(p["玛瑙"].count, 1, "他人到账不应配对，回退数量 1");
    }

    // ============== compute_scraps_items 单元测试 ==============

    /// 测试辅助：确保 drop_items 表存在（init_db_with_path 当前未创建该表）
    fn ensure_drop_items_table_for_test(conn: &rusqlite::Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS drop_items (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                item_type         INTEGER NOT NULL,
                item_id           INTEGER NOT NULL,
                item_name         TEXT NOT NULL UNIQUE,
                price             INTEGER,
                category          TEXT NOT NULL DEFAULT 'unknown',
                created_at        TEXT NOT NULL
            );
            "#,
        )
        .expect("should ensure drop_items table");
    }

    /// 插入测试用 drop_items 记录
    fn insert_test_drop_item(
        conn: &rusqlite::Connection,
        name: &str,
        price: Option<i64>,
        category: &str,
    ) {
        ensure_drop_items_table_for_test(conn);
        let now = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO drop_items (item_type, item_id, item_name, price, category, created_at)
             VALUES (5, 0, ?1, ?2, ?3, ?4)",
            sql_params![name, price, category, now],
        )
        .expect("should insert drop_item");
    }

    /// 构造购买记录
    fn make_purchase(count: u32, total_price: i64) -> PurchaseInfo {
        PurchaseInfo { count, total_price }
    }

    /// 测试：空购买记录应返回空散件清单
    #[test]
    fn test_compute_scraps_items_empty() {
        let (conn, _temp_dir) = create_scan_test_db();
        let purchased: HashMap<String, PurchaseInfo> = HashMap::new();
        let categories: HashMap<String, String> = HashMap::new();
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert!(items.is_empty(), "空购买记录应返回空散件清单");
        assert_eq!(value, 0, "空清单估价应为 0");
        assert_eq!(expense, 0, "空清单散件支出应为 0");
    }

    /// 测试：白名单材料识别正确（无网络时降级 -> manual 手填）
    #[test]
    fn test_compute_scraps_items_whitelist_material() {
        let (conn, _temp_dir) = create_scan_test_db();
        let mut purchased = HashMap::new();
        purchased.insert("维峰丹".to_string(), make_purchase(2, 0));
        let categories = HashMap::new();
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 1, "应识别 1 项白名单材料");
        assert_eq!(items[0].name, "维峰丹");
        assert_eq!(items[0].count, 2, "count 应为 2");
        assert_eq!(items[0].category, "material");
        assert_eq!(items[0].unit_price, None, "无网络时 unitPrice=None");
        assert_eq!(items[0].price_source, "manual");
        assert_eq!(value, 0, "unitPrice=None 按 0 计入");
        assert_eq!(expense, 0, "0 金获取的材料不计散件支出");
    }

    /// 测试：白名单材料非 0 金购买也计入（按设计 §1.2：0 金或非 0 金都算）
    #[test]
    fn test_compute_scraps_items_whitelist_material_nonzero_price() {
        let (conn, _temp_dir) = create_scan_test_db();
        let mut purchased = HashMap::new();
        // 玛瑙 500 金购买（白名单材料，无论价格都计入）
        purchased.insert("玛瑙".to_string(), make_purchase(1, 500));
        let categories = HashMap::new();
        let (items, _value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 1, "白名单材料无论价格都应计入散件");
        assert_eq!(items[0].name, "玛瑙");
        assert_eq!(items[0].category, "material");
        assert_eq!(expense, 500, "白名单材料的实际购买价应计入散件支出");
    }

    /// 测试：0 金装备（drop_items 有 Price）→ npc 卖价（铜→金换算）
    #[test]
    fn test_compute_scraps_items_zero_price_equipment_npc() {
        let (conn, _temp_dir) = create_scan_test_db();
        // drop_items.Price 单位为铜（JX3 接口原始单位），1,704,984 铜 ≈ 170 金
        insert_test_drop_item(&conn, "流漓腰带", Some(1704984), "equipment");
        let mut purchased = HashMap::new();
        purchased.insert("流漓腰带".to_string(), make_purchase(1, 0));
        let mut categories = HashMap::new();
        categories.insert("流漓腰带".to_string(), "equipment".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "流漓腰带");
        assert_eq!(items[0].category, "equipment");
        assert_eq!(items[0].unit_price, Some(170), "NPC 卖价应从铜换算为金");
        assert_eq!(items[0].price_source, "npc");
        assert_eq!(value, 170);
        assert_eq!(expense, 0, "0 金装备不计散件支出");
    }

    /// 测试：0 金装备（drop_items 无 Price）→ manual 手填
    #[test]
    fn test_compute_scraps_items_zero_price_equipment_no_price() {
        let (conn, _temp_dir) = create_scan_test_db();
        insert_test_drop_item(&conn, "无价装备", None, "equipment");
        let mut purchased = HashMap::new();
        purchased.insert("无价装备".to_string(), make_purchase(1, 0));
        let mut categories = HashMap::new();
        categories.insert("无价装备".to_string(), "equipment".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].unit_price, None);
        assert_eq!(items[0].price_source, "manual");
        assert_eq!(value, 0);
        assert_eq!(expense, 0);
    }

    /// 测试：非 0 金装备不计入散件
    #[test]
    fn test_compute_scraps_items_nonzero_equipment_excluded() {
        let (conn, _temp_dir) = create_scan_test_db();
        insert_test_drop_item(&conn, "竞拍装备", Some(50000), "equipment");
        let mut purchased = HashMap::new();
        purchased.insert("竞拍装备".to_string(), make_purchase(1, 50000));
        let mut categories = HashMap::new();
        categories.insert("竞拍装备".to_string(), "equipment".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert!(items.is_empty(), "非 0 金装备不应计入散件");
        assert_eq!(value, 0);
        assert_eq!(expense, 0, "付费购买的装备属于非散件支出");
    }

    /// 测试：不在白名单的材料不计入散件
    #[test]
    fn test_compute_scraps_items_non_whitelist_material_excluded() {
        let (conn, _temp_dir) = create_scan_test_db();
        let mut purchased = HashMap::new();
        // 副本外的杂物材料（不在白名单）
        purchased.insert("杂物材料".to_string(), make_purchase(1, 0));
        let mut categories = HashMap::new();
        categories.insert("杂物材料".to_string(), "material".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert!(items.is_empty(), "不在白名单的材料不应计入散件");
        assert_eq!(value, 0);
        assert_eq!(expense, 0, "非白名单材料（如小铁）不计散件支出");
    }

    /// 测试：0 金装备但 category 非 equipment 不计入
    #[test]
    fn test_compute_scraps_items_zero_price_non_equipment_excluded() {
        let (conn, _temp_dir) = create_scan_test_db();
        insert_test_drop_item(&conn, "神秘物品", Some(1000), "unknown");
        let mut purchased = HashMap::new();
        purchased.insert("神秘物品".to_string(), make_purchase(1, 0));
        let mut categories = HashMap::new();
        categories.insert("神秘物品".to_string(), "unknown".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert!(items.is_empty(), "0 金但非 equipment 类别不应计入散件");
        assert_eq!(value, 0);
        assert_eq!(expense, 0);
    }

    /// 测试：0 金装备但 drop_items 无 category 记录 → 保守策略不计入（设计 §8.4）
    #[test]
    fn test_compute_scraps_items_zero_price_no_category_excluded() {
        let (conn, _temp_dir) = create_scan_test_db();
        // 不插入 drop_items 记录，category 缺失
        let mut purchased = HashMap::new();
        purchased.insert("未知装备".to_string(), make_purchase(1, 0));
        let categories = HashMap::new();
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert!(items.is_empty(), "category 缺失时按保守策略不计入散件");
        assert_eq!(value, 0);
        assert_eq!(expense, 0);
    }

    /// 测试：同物品多次购买 count 累加正确
    #[test]
    fn test_compute_scraps_items_count_accumulation() {
        let (conn, _temp_dir) = create_scan_test_db();
        let mut purchased = HashMap::new();
        // 模拟同物品多次购买：count=3, total_price=0（3 次 0 金购买）
        purchased.insert("维峰丹".to_string(), make_purchase(3, 0));
        let categories = HashMap::new();
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].count, 3, "count 应累加为 3");
        assert_eq!(value, 0, "unitPrice=None 时 scrapsValue=0");
        assert_eq!(expense, 0);
    }

    /// 测试：混合场景（白名单材料 + 0 金装备 + 非 0 金装备 + 非白名单材料）
    /// 验证 scrapsValue 求和正确性（含 unitPrice=None 按 0 计入）
    #[test]
    fn test_compute_scraps_items_mixed() {
        let (conn, _temp_dir) = create_scan_test_db();
        // drop_items 记录（Price 单位为铜）
        insert_test_drop_item(&conn, "流漓腰带", Some(1704984), "equipment");
        insert_test_drop_item(&conn, "无价装备", None, "equipment");
        // 购买记录
        let mut purchased = HashMap::new();
        purchased.insert("维峰丹".to_string(), make_purchase(2, 0));     // 白名单材料 ×2
        purchased.insert("玛瑙".to_string(), make_purchase(1, 500));     // 白名单材料（非 0 金也计入）
        purchased.insert("流漓腰带".to_string(), make_purchase(1, 0));   // 0 金装备（NPC 估价 1704984 铜 ≈ 170 金）
        purchased.insert("无价装备".to_string(), make_purchase(1, 0));   // 0 金装备（无价 → manual）
        purchased.insert("竞拍装备".to_string(), make_purchase(1, 50000)); // 非 0 金装备（不计入）
        purchased.insert("杂物材料".to_string(), make_purchase(1, 0));   // 非白名单材料（不计入）
        // 分类
        let mut categories = HashMap::new();
        categories.insert("流漓腰带".to_string(), "equipment".to_string());
        categories.insert("无价装备".to_string(), "equipment".to_string());
        categories.insert("竞拍装备".to_string(), "equipment".to_string());
        categories.insert("杂物材料".to_string(), "material".to_string());
        let (items, value, expense) = compute_scraps_items(&conn, &purchased, &categories, "");
        // 应识别 4 项（维峰丹、玛瑙、流漓腰带、无价装备）
        assert_eq!(items.len(), 4, "应识别 4 项散件（白名单材料 2 + 0 金装备 2）");
        // 按名称排序：无价装备、流漓腰带、玛瑙、维峰丹
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["无价装备", "流漓腰带", "玛瑙", "维峰丹"], "应按名称排序");
        // 估价：流漓腰带 1×170 + 玛瑙 1×0 + 维峰丹 2×0 + 无价装备 1×0 = 170
        assert_eq!(value, 170, "scrapsValue 应为 170（仅含已获取单价的物品）");
        // 散件支出：仅玛瑙 500 金（维峰丹 0 金、装备均 0 金）
        assert_eq!(expense, 500, "scrapsExpense 应为白名单材料实际购买价合计");
    }

    /// 测试：ScrapsItem 序列化为 camelCase（与前端 types.ts 的 ScrapsItem 对齐）
    ///
    /// v2.1.53 曾因缺少 rename_all 输出 snake_case（unit_price / price_source），
    /// 导致前端读不到单价与来源、散件估价合计恒为 0。
    #[test]
    fn test_scraps_item_serializes_camel_case() {
        let item = ScrapsItem {
            name: "流漓腰带".to_string(),
            count: 2,
            unit_price: Some(170),
            total_price: 0,
            category: "equipment".to_string(),
            price_source: "npc".to_string(),
        };
        let json: serde_json::Value = serde_json::to_value(&item).expect("序列化失败");
        assert_eq!(json["unitPrice"], 170, "应输出 camelCase 键 unitPrice");
        assert_eq!(json["priceSource"], "npc", "应输出 camelCase 键 priceSource");
        assert_eq!(json["totalPrice"], 0, "应输出 camelCase 键 totalPrice");
        assert!(json.get("unit_price").is_none(), "不应残留 snake_case 键 unit_price");
        assert!(json.get("price_source").is_none(), "不应残留 snake_case 键 price_source");
    }

    /// 测试：输出按名称排序（UTF-8 字节序）
    #[test]
    fn test_compute_scraps_items_sorted_by_name() {
        let (conn, _temp_dir) = create_scan_test_db();
        let mut purchased = HashMap::new();
        // 故意以乱序插入
        purchased.insert("猫眼石".to_string(), make_purchase(1, 0));
        purchased.insert("维峰丹".to_string(), make_purchase(1, 0));
        purchased.insert("玛瑙".to_string(), make_purchase(1, 0));
        let categories = HashMap::new();
        let (items, _, _) = compute_scraps_items(&conn, &purchased, &categories, "");
        assert_eq!(items.len(), 3);
        // 期望按 UTF-8 字节序排序：猫眼石、玛瑙、维峰丹
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["猫眼石", "玛瑙", "维峰丹"], "应按名称排序");
    }


    /// 真实数据验证（干运行，不写数据库）：少年白了发 2026-06-29 25人英雄阆风悬城
    ///
    /// 验证内容：
    /// 1. 副本识别：应识别出 25人英雄阆风悬城
    /// 2. BOSS记录：应识别 5 个 BOSS（笑妆娘、唐醉、柳公子、阿史那承庆、须罗巨傀）
    /// 3. 收入：应从 chatlog 提取金币收入（与手工记录 16082 金对比）
    #[test]
    #[ignore]
    fn test_scan_shaonianbailaofa_dry_run() {
        let game_dir = PathBuf::from(r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd");
        let account_dir = game_dir
            .join("interface")
            .join("my#data")
            .join("432345564243886337@zhcn_hd");

        assert!(
            account_dir.exists(),
            "账号目录不存在: {}",
            account_dir.display()
        );

        // 1. 扫描 JCL 文件
        let jcl_files = scan_jcl_files(&account_dir, 0, 0, 0).expect("扫描 JCL 失败");
        println!("[DRY RUN] JCL 文件数: {}", jcl_files.len());
        for jcl in &jcl_files {
            println!(
                "  JCL: {} | boss={}",
                jcl.file_name, jcl.boss_name
            );
        }

        // 2. 查找 chatlog 文件
        let chatlog_files = find_chatlog_files(&account_dir).expect("查找 chatlog 失败");
        println!("[DRY RUN] chatlog 文件数: {}", chatlog_files.len());

        // 3. 加载副本配置并分析每个 JCL
        let conn = db::init_db().expect("初始化数据库失败");
        let raids = load_raids_with_bosses(&conn).expect("加载副本信息失败");
        let combat_logs_dir = account_dir.join("userdata").join("combat_logs");
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        for jcl in &jcl_files {
            let raid_entry = match match_raid_name(&jcl.raid_display_name, &raids) {
                Some(entry) => entry,
                None => continue,
            };
            let raid_bosses = &raid_entry.bosses;
            let jcl_path = combat_logs_dir.join(&jcl.file_name);
            let mut analysis = analyze_jcl_cached(&conn, &jcl_path, &jcl.boss_name, jcl.boss_id, raid_bosses).unwrap_or(JclAnalysis {
                boss_name: None,
                fight_start_ms: 0,
                fight_end_ms: 0,
                is_kill: false,
            });
            if is_valid_boss(&jcl.boss_name, raid_bosses) {
                analysis.boss_name = Some(jcl.boss_name.clone());
            }
            jcl_analyses.insert(jcl.file_name.clone(), analysis);
        }
        println!("[DRY RUN] JCL 分析完成: {} 个", jcl_analyses.len());

        // 4. 聚类
        let instances = cluster_raid_instances("432345564243886337", jcl_files, &jcl_analyses, &[]);
        println!("[DRY RUN] 副本实例数: {}", instances.len());

        let mut heroic_gold: i64 = 0;
        for (i, inst) in instances.iter().enumerate() {
            println!(
                "\n=== 副本实例 #{} ===\n  副本名: {}\n  BOSS击杀数: {}\n  BOSS列表: {:?}\n  JCL文件数: {}",
                i + 1,
                inst.raid_display_name,
                inst.boss_kill_count,
                inst.jcl_boss_names,
                inst.jcl_files.len()
            );

            // 5. 从 chatlog 提取掉落和金币
            let chatlog_start = inst.first_gold_time;
            let chatlog_end = inst.last_jcl_time + 2 * 60 * 60 * 1000;
            let mut drops: Vec<String> = Vec::new();
            let mut base_salary: Option<i64> = None;
            let mut other_income: i64 = 0;
            let mut total_expense: i64 = 0;
            for chatlog_path in &chatlog_files {
                if let Ok((chatlog_drops, salary, income, expense, _, _, _, _)) =
                    extract_drops_from_chatlog(chatlog_path, chatlog_start, chatlog_end, "少年白了发")
                {
                    for drop in chatlog_drops {
                        if !drops.contains(&drop) {
                            drops.push(drop);
                        }
                    }
                    if salary.is_some() {
                        base_salary = salary;
                    }
                    other_income += income;
                    total_expense += expense;
                }
            }
            let total_gold = base_salary.unwrap_or(other_income);
            println!("  掉落物: {:?}", drops);
            println!("  底薪: {:?}, 其他收入: {}, 支出: {}", base_salary, other_income, total_expense);
            println!("  金币收入: {} (手工记录: 16082)", total_gold);

            if inst.raid_display_name == "25人英雄阆风悬城" {
                heroic_gold = total_gold;
            }
        }

        // 验证：应识别出 25人英雄阆风悬城
        let heroic = instances
            .iter()
            .find(|i| i.raid_display_name == "25人英雄阆风悬城")
            .expect("应识别出 25人英雄阆风悬城");

        // 验证：25人英雄阆风悬城应有 5 个 BOSS 击杀
        assert_eq!(heroic.boss_kill_count, 5, "BOSS击杀数应为5");

        // 验证：BOSS列表应包含笑妆娘、唐醉、柳公子、阿史那承庆
        assert!(heroic.jcl_boss_names.contains(&"笑妆娘".to_string()), "应包含笑妆娘");
        assert!(heroic.jcl_boss_names.contains(&"唐醉".to_string()), "应包含唐醉");
        assert!(heroic.jcl_boss_names.contains(&"柳公子".to_string()), "应包含柳公子");
        assert!(
            heroic.jcl_boss_names.contains(&"阿史那承庆".to_string()),
            "应包含阿史那承庆"
        );

        // 验证：不应包含墨家机侍（小怪）
        assert!(
            !heroic.jcl_boss_names.contains(&"墨家机侍".to_string()),
            "不应包含墨家机侍（小怪）"
        );

        // 验证：金币收入应大于0（与手工记录16082对比，允许差异）
        assert!(heroic_gold > 0, "金币收入应大于0，实际: {}", heroic_gold);
        println!(
            "\n[验证] 金币收入对比: 扫描={} vs 手工=16082 (差异={})",
            heroic_gold,
            heroic_gold - 16082
        );
    }

    /// 验证去重逻辑：已存在手工记录时，自动扫描不创建新 pending
    ///
    /// 手工记录：2026-06-30 00:12, 25人英雄阆风悬城, 收入16082金
    /// 扫描应检测到该手工记录并跳过 pending 创建
    #[test]
    #[ignore]
    fn test_scan_shaonianbailaofa_dedup() {
        // 少年白了发的数据库 UUID（account_id）
        let db_account_id = "3913bc21-4623-4e3b-92cb-bc231dbabe7c";

        // 1. 查询 records (pending) 之前的状态
        let conn = db::init_db().expect("初始化数据库失败");

        let before_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE account_id = ?1 AND status IN ('pending', 'scanning')",
                params![db_account_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        println!("[DEDUP] records (pending) before: {} 条", before_count);

        // 2. 运行扫描（历史数据回放：JX3 进程未运行，角色离线，记录应为 pending）
        let result = scan_raid_drops_internal("432345564243886337", false, false, 0, 0, 0);
        println!("[DEDUP] 扫描结果: {:?}", result);

        // 3. 查询 records (pending) 之后的状态
        let after_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE account_id = ?1 AND status IN ('pending', 'scanning')",
                params![db_account_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        println!("[DEDUP] records (pending) after: {} 条", after_count);

        // 4. 检查是否有阆风悬城的新 pending
        //    raid_name 列存储 raids.name 短名（如 "阆风悬城"）
        let pending_langfeng: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records \
                 WHERE account_id = ?1 AND raid_name = '阆风悬城' AND status = 'pending'",
                params![db_account_id],
                |row| row.get(0),
            )
            .unwrap_or(0);
        println!("[DEDUP] 阆风悬城 pending 记录数: {}", pending_langfeng);

        // 验证：不应为阆风悬城创建新 pending（因为已有手工记录）
        // 如果 before == after，说明完全没创建新记录（理想情况）
        // 如果 after > before，需确认新记录不是阆风悬城的 pending
        if after_count > before_count {
            assert_eq!(
                pending_langfeng, 0,
                "已存在手工记录时，不应为阆风悬城创建新 pending"
            );
        }
        println!("[DEDUP] 去重验证通过：未为阆风悬城创建新 pending");
    }

    /// 验证 info.jx3dat 缺失 name 字段时，scan_raid_drops 仍能正确识别糯闪账号
    ///
    /// 复现条件：糯闪账号（uid=342273571708094124）的 info.jx3dat 缺失 name 字段，
    /// 修复前 resolve_db_role_identity 返回 None 导致整个账号被跳过。
    ///
    /// 验证点：
    /// 1. resolve_db_role_identity 对糯闪账号返回 Some，role_name="糯闪"
    /// 2. scan_jcl_files 能扫描到 7-25 的 25 英雄阆风 JCL
    /// 3. cluster_raid_instances 能聚类出 25 人英雄阆风悬城实例
    ///
    /// 注意：此测试不写入 records 表，仅验证扫描流程到聚类阶段
    #[test]
    #[ignore]
    fn test_scan_nuoshan_fallback_identity() {
        let game_dir = PathBuf::from(r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd");
        let account_dir = game_dir
            .join("interface")
            .join("my#data")
            .join("342273571708094124@zhcn_hd");

        assert!(
            account_dir.exists(),
            "账号目录不存在: {}",
            account_dir.display()
        );

        // 1. 验证 resolve_db_role_identity 能识别糯闪（修复前返回 None）
        let conn = db::init_db().expect("初始化数据库失败");
        let identity = resolve_db_role_identity(&conn, &account_dir, &game_dir)
            .expect("resolve_db_role_identity 失败")
            .expect("糯闪账号应能被识别（修复前会被跳过）");

        assert_eq!(identity.role_name, "糯闪", "role_name 应为 '糯闪'");
        assert_eq!(identity.server, "梦江南", "server 应为 '梦江南'");
        assert_eq!(identity.region, "电信区", "region 应为 '电信区'");
        println!(
            "[NUOSHAN] 角色身份: role_id={}, account_id={}, name={}",
            identity.role_id, identity.account_id, identity.role_name
        );

        // 2. 扫描 JCL 文件（CD 窗口：7-21 07:00 ~ 7-28 07:00，覆盖 7-25 副本）
        //    用 process_start_ms=0 表示不做进程时间过滤，仅用 CD 窗口过滤
        let cd_start = chrono::NaiveDate::from_ymd_opt(2026, 7, 20)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let cd_end = chrono::NaiveDate::from_ymd_opt(2026, 7, 27)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        let jcl_files = scan_jcl_files(&account_dir, 0, cd_start, cd_end)
            .expect("扫描 JCL 失败");
        println!("[NUOSHAN] CD 窗口内 JCL 文件数: {}", jcl_files.len());
        for jcl in &jcl_files {
            println!("  JCL: {} | boss={}", jcl.file_name, jcl.boss_name);
        }

        // 应扫描到 7-25 的 25 英雄阆风 JCL（9 个：笑妆娘、唐醉、墨家机侍、柳公子×2、狼牙士兵、阿史那承庆、唐怀仁、须罗巨傀）
        assert!(
            !jcl_files.is_empty(),
            "应扫描到糯闪的 JCL 文件"
        );
        let has_langfeng = jcl_files
            .iter()
            .any(|j| j.raid_display_name == "25人英雄阆风悬城");
        assert!(
            has_langfeng,
            "应包含 25人英雄阆风悬城 JCL"
        );

        // 3. 加载副本配置并分析 JCL
        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");
        let combat_logs_dir = account_dir.join("userdata").join("combat_logs");
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        for jcl in &jcl_files {
            let raid_entry = match match_raid_name(&jcl.raid_display_name, &raids) {
                Some(entry) => entry,
                None => continue,
            };
            let raid_bosses = &raid_entry.bosses;
            let jcl_path = combat_logs_dir.join(&jcl.file_name);
            let mut analysis = analyze_jcl_cached(
                &conn,
                &jcl_path,
                &jcl.boss_name,
                jcl.boss_id,
                raid_bosses,
            )
            .unwrap_or(JclAnalysis {
                boss_name: None,
                fight_start_ms: 0,
                fight_end_ms: 0,
                is_kill: false,
            });
            if is_valid_boss(&jcl.boss_name, raid_bosses) {
                analysis.boss_name = Some(jcl.boss_name.clone());
            }
            jcl_analyses.insert(jcl.file_name.clone(), analysis);
        }

        // 4. 聚类
        let instances = cluster_raid_instances(
            "342273571708094124",
            jcl_files,
            &jcl_analyses,
            &[],
        );
        println!("[NUOSHAN] 副本实例数: {}", instances.len());
        for (i, inst) in instances.iter().enumerate() {
            println!(
                "=== 实例 #{}: {} | BOSS击杀={} | JCL数={} ===",
                i + 1,
                inst.raid_display_name,
                inst.boss_kill_count,
                inst.jcl_files.len()
            );
        }

        // 应聚类出 25 人英雄阆风悬城实例
        let langfeng = instances
            .iter()
            .find(|i| i.raid_display_name == "25人英雄阆风悬城")
            .expect("应聚类出 25人英雄阆风悬城");

        // 25 英雄阆风应有 5 个 BOSS 击杀（笑妆娘、唐醉、柳公子、阿史那承庆、唐怀仁）
        assert!(
            langfeng.boss_kill_count >= 5,
            "25英雄阆风击杀数应 >= 5，实际: {}",
            langfeng.boss_kill_count
        );
        println!(
            "[NUOSHAN] 验证通过：糯闪账号被正确识别，25英雄阆风 {} 个 BOSS 击杀",
            langfeng.boss_kill_count
        );

        // 5. 从 chatlog 提取掉落和金币（仿照 dry_run 测试）
        let chatlog_files = find_chatlog_files(&account_dir).expect("查找 chatlog 失败");
        println!("[NUOSHAN] chatlog 文件数: {}", chatlog_files.len());

        // chatlog 窗口：首个金币时间 ~ last_jcl_time + 2h
        // 注意：修复后副本已结束时用 last_jcl_time + 2h 而非 SystemTime::now()
        let chatlog_start = langfeng.first_gold_time;
        let chatlog_end = langfeng.last_jcl_time + 2 * 60 * 60 * 1000;

        // 时间戳格式化（用 NaiveDateTime，避免引入 TimeZone trait）
        let fmt_ts = |ms: i64| {
            #[allow(deprecated)]
            chrono::NaiveDateTime::from_timestamp_millis(ms)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| format!("ts={}", ms))
        };
        println!(
            "[NUOSHAN] chatlog 窗口: {} ~ {}",
            fmt_ts(chatlog_start),
            fmt_ts(chatlog_end)
        );

        let mut drops: Vec<String> = Vec::new();
        let mut base_salary: Option<i64> = None;
        let mut other_income: i64 = 0;
        let mut total_expense: i64 = 0;
        for chatlog_path in &chatlog_files {
            if let Ok((chatlog_drops, salary, income, expense, _, _, _, _)) =
                extract_drops_from_chatlog(chatlog_path, chatlog_start, chatlog_end, "糯闪")
            {
                for drop in chatlog_drops {
                    if !drops.contains(&drop) {
                        drops.push(drop);
                    }
                }
                if salary.is_some() {
                    base_salary = salary;
                }
                other_income += income;
                total_expense += expense;
            }
        }
        let total_gold = base_salary.unwrap_or(other_income);

        println!("\n[NUOSHAN] === 25 英雄阆风收入统计 ===");
        println!("  掉落物数量: {}", drops.len());
        println!("  掉落物列表: {:?}", drops);
        println!("  底薪: {:?}", base_salary);
        println!("  其他收入合计: {} 金", other_income);
        println!("  支出合计: {} 金", total_expense);
        println!("  最终收入: {} 金", total_gold);
        println!("  副本开始: {}", fmt_ts(langfeng.first_gold_time));
        println!("  最后 JCL: {}", fmt_ts(langfeng.last_jcl_time));
    }

    /// 验证 is_valid_boss 在 raid_bosses 为空时返回 true（不过滤）
    /// 这是无 BOSS 配置副本能正常扫描的核心前提
    #[test]
    fn test_is_valid_boss_empty_config() {
        let empty_bosses: Vec<(String, String)> = vec![];
        assert!(is_valid_boss("任意BOSS名", &empty_bosses));
        assert!(is_valid_boss("", &empty_bosses));
        assert!(is_valid_boss("史朝义", &empty_bosses));
    }

    /// 验证 is_valid_boss 在 raid_bosses 非空时仍按精确/模糊匹配
    #[test]
    fn test_is_valid_boss_non_empty_config() {
        let bosses = vec![
            ("1".to_string(), "笑妆娘".to_string()),
            ("2".to_string(), "唐醉".to_string()),
        ];
        assert!(is_valid_boss("笑妆娘", &bosses));
        assert!(is_valid_boss("笑妆娘·幻影", &bosses)); // 模糊匹配
        assert!(!is_valid_boss("未知BOSS", &bosses));
    }

    /// 验证无 BOSS 配置的副本能正常聚类产出实例
    /// 模拟场景：副本"西津渡"未配置 raid_bosses，JCL 的 is_kill=true 且 boss_name=文件名
    #[test]
    fn test_cluster_raid_instances_unconfigured_raid() {
        let jcl_files = vec![
            JclFileInfo {
                file_name: "2026-07-10-20-00-00-25人普通西津渡(800)-史朝义(140000).jcl".to_string(),
                timestamp: 1752168000000,
                raid_display_name: "25人普通西津渡".to_string(),
                raid_id: 800,
                boss_name: "史朝义".to_string(),
                boss_id: 140000,
            },
            JclFileInfo {
                file_name: "2026-07-10-20-15-00-25人普通西津渡(800)-慕容野(140001).jcl".to_string(),
                timestamp: 1752168900000,
                raid_display_name: "25人普通西津渡".to_string(),
                raid_id: 800,
                boss_name: "慕容野".to_string(),
                boss_id: 140001,
            },
        ];

        // 模拟 analyze_jcl 路径 A 的返回值：
        // raid_bosses 为空 → file_boss_in_config=true → boss_name=文件名，is_kill=true
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-07-10-20-00-00-25人普通西津渡(800)-史朝义(140000).jcl".to_string(),
            JclAnalysis {
                boss_name: Some("史朝义".to_string()),
                fight_start_ms: 1752168000000,
                fight_end_ms: 1752168060000,
                is_kill: true,
            },
        );
        jcl_analyses.insert(
            "2026-07-10-20-15-00-25人普通西津渡(800)-慕容野(140001).jcl".to_string(),
            JclAnalysis {
                boss_name: Some("慕容野".to_string()),
                fight_start_ms: 1752168900000,
                fight_end_ms: 1752168960000,
                is_kill: true,
            },
        );

        let instances = cluster_raid_instances(
            "test_account",
            jcl_files,
            &jcl_analyses,
            &[],
        );

        // 核心断言：无配置副本也能产出实例（修复前 build_raid_instance 返回 None → 0 个实例）
        assert_eq!(instances.len(), 1, "无 BOSS 配置的副本应产出 1 个实例");
        assert_eq!(
            instances[0].boss_kill_count, 2,
            "应识别 2 个 BOSS 击杀"
        );
        assert!(
            instances[0]
                .jcl_boss_names
                .contains(&"史朝义".to_string()),
            "应包含史朝义"
        );
        assert!(
            instances[0]
                .jcl_boss_names
                .contains(&"慕容野".to_string()),
            "应包含慕容野"
        );
        assert_eq!(
            instances[0].raid_display_name,
            "25人普通西津渡",
            "副本名应正确"
        );
    }

    /// 验证无 BOSS 配置的副本在拉托时（is_kill=false）不产出实例
    /// 修复后仍应保持保守判定：无击杀信号 = 灭团 = 不生成记录
    #[test]
    fn test_cluster_raid_instances_unconfigured_all_wipes() {
        let jcl_files = vec![JclFileInfo {
            file_name: "2026-07-10-20-00-00-25人普通西津渡(800)-史朝义(140000).jcl".to_string(),
            timestamp: 1752168000000,
            raid_display_name: "25人普通西津渡".to_string(),
            raid_id: 800,
            boss_name: "史朝义".to_string(),
            boss_id: 140000,
        }];

        // is_kill=false（拉托），boss_name=文件名（路径 A 返回）
        let mut jcl_analyses: HashMap<String, JclAnalysis> = HashMap::new();
        jcl_analyses.insert(
            "2026-07-10-20-00-00-25人普通西津渡(800)-史朝义(140000).jcl".to_string(),
            JclAnalysis {
                boss_name: Some("史朝义".to_string()),
                fight_start_ms: 1752168000000,
                fight_end_ms: 1752168060000,
                is_kill: false,
            },
        );

        let instances = cluster_raid_instances(
            "test_account",
            jcl_files,
            &jcl_analyses,
            &[],
        );

        // 灭团时不产出实例（保守判定，与有配置副本行为一致）
        assert_eq!(
            instances.len(),
            0,
            "无 BOSS 配置副本全灭团时不应产出实例"
        );
    }

    /// 真实数据扫描：扫描本月（2026-07-01 ~ 2026-08-01）所有账号副本记录
    ///
    /// 用途：手动触发本月扫描，会写入 D:\Tools\Jx3RaidManager 数据库的 pending 记录
    /// 运行方式：
    ///   cargo test --package jx3-raid-manager --lib test_scan_this_month_real_data -- --ignored --nocapture
    ///
    /// 注意：此测试依赖 D:\Tools\Jx3RaidManager 下的应用数据库（通过 data-dir.json 配置）。
    /// 完成后请在应用内查看 PendingRecordsPanel 的待确认记录。
    #[test]
    #[ignore]
    fn test_scan_this_month_real_data() {
        use chrono::TimeZone;

        // 本月时间范围（北京时间）：
        //   start = 2026-07-01 00:00:00 +08:00
        //   end   = 2026-08-01 00:00:00 +08:00
        let start_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 7, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();
        let end_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();

        println!(
            "[本月扫描] 时间范围 (ms): start={}, end={}",
            start_ms, end_ms
        );
        println!(
            "[本月扫描] 本地起始: {}",
            chrono::Local.timestamp_millis_opt(start_ms).unwrap()
        );
        println!(
            "[本月扫描] 本地结束: {}",
            chrono::Local.timestamp_millis_opt(end_ms).unwrap()
        );

        // 初始化数据库（基于 data-dir.json 配置，应使用 D:\Tools\Jx3RaidManager）
        let _conn = crate::db::init_db().expect("初始化数据库失败");
        println!("[本月扫描] 数据库初始化完成");

        // 创建多线程 runtime 调用 async 函数
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("创建 tokio runtime 失败");

        let results = rt
            .block_on(async { scan_raids_in_range(start_ms, end_ms, 0).await })
            .expect("scan_raids_in_range 调用失败");

        println!("[本月扫描] 扫描完成，账号数: {}", results.len());
        let total_scanned: usize = results.iter().map(|r| r.instance_count.unwrap_or(0)).sum();
        let failed_count = results.iter().filter(|r| !r.success).count();
        println!("[本月扫描] 处理副本实例总数: {}", total_scanned);
        println!("[本月扫描] 失败账号数: {}", failed_count);

        for r in &results {
            if r.success {
                println!(
                    "  账号 {} -> 成功, 实例数 = {}",
                    r.account_id,
                    r.instance_count.unwrap_or(0)
                );
            } else {
                println!(
                    "  账号 {} -> 失败, 原因 = {}",
                    r.account_id,
                    r.error.as_deref().unwrap_or("未知")
                );
            }
        }

        assert!(
            results.iter().any(|r| r.success),
            "应至少有一个账号扫描成功"
        );
    }

    /// 验证扫描后 records 表的掉落数量是否合理
    ///
    /// 运行：cargo test --bin jx3-raid-manager test_verify_drops_count -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_verify_drops_count() {
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let mut stmt = conn
            .prepare(
                "SELECT data,
                        datetime(record_date/1000, 'unixepoch', '+08 hours') as start,
                        drops, status
                 FROM records
                 WHERE source='auto_scan'
                 ORDER BY record_date DESC
                 LIMIT 30",
            )
            .expect("prepare 失败");

        let rows: Vec<(String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");

        println!("[验证] 共 {} 条自动扫描记录\n", rows.len());
        for (data_json, start, drops_json, status) in &rows {
            let data: serde_json::Value = serde_json::from_str(data_json).unwrap_or_default();
            let raid = data.get("raidName").and_then(|v| v.as_str()).unwrap_or("");
            let role = data.get("roleName").and_then(|v| v.as_str()).unwrap_or("");
            let gold = data.get("goldIncome").and_then(|v| v.as_i64()).unwrap_or(0);
            let expense = data.get("goldExpense").and_then(|v| v.as_i64()).unwrap_or(0);
            let purchased: Vec<String> = data
                .get("purchasedItems")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                .unwrap_or_default();
            let drops: Vec<String> = serde_json::from_str(drops_json).unwrap_or_default();
            println!(
                "{} | {} | {} | 收入={}金 支出={}金 | drops={}件 purchased={}件 | {}",
                raid, role, start, gold, expense, drops.len(), purchased.len(), status
            );
            if !drops.is_empty() && drops.len() <= 30 {
                println!("  drops: {}", drops.join(", "));
            }
            if !purchased.is_empty() {
                println!("  purchased: {}", purchased.join(", "));
            }
        }
    }

    /// 重新扫描 7-12 风闪的副本，触发 pending 记录覆盖更新
    ///
    /// 用途：笑妆娘 JCL 缓存已修复（is_kill=true），需重新扫描覆盖旧的 pending 记录，
    /// 让 bossNames 字段包含笑妆娘。
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_rescan_fengshan_712 -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_rescan_fengshan_712() {
        use chrono::TimeZone;

        // 时间范围：2026-07-12 00:00 ~ 2026-07-13 00:00 (北京时间)
        let start_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 7, 12)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();
        let end_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 7, 13)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();

        println!("[7-12 重扫] 时间范围: {} ~ {}", start_ms, end_ms);

        let _conn = crate::db::init_db().expect("初始化数据库失败");
        println!("[7-12 重扫] 数据库初始化完成");

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("创建 tokio runtime 失败");

        let results = rt
            .block_on(async { scan_raids_in_range(start_ms, end_ms, 0).await })
            .expect("scan_raids_in_range 调用失败");

        println!("\n[7-12 重扫] 扫描完成，账号数: {}", results.len());
        let total_scanned: usize = results.iter().map(|r| r.instance_count.unwrap_or(0)).sum();
        println!("[7-12 重扫] 处理副本实例总数: {}", total_scanned);

        for r in &results {
            if r.success {
                println!(
                    "  账号 {} -> 成功, 实例数 = {}",
                    r.account_id,
                    r.instance_count.unwrap_or(0)
                );
            } else {
                println!(
                    "  账号 {} -> 失败, 原因 = {}",
                    r.account_id,
                    r.error.as_deref().unwrap_or("未知")
                );
            }
        }

        // 验证：重新读取风闪 7-12 pending 记录，确认 bossNames 包含笑妆娘
        let conn = crate::db::init_db().expect("再次初始化数据库失败");
        let data: String = conn
            .query_row(
                "SELECT data FROM records
                 WHERE role_id = '293f3c1f-9a2e-44c8-8911-f541074b0c7e'
                   AND source = 'auto_scan'
                   AND status = 'pending'
                   AND record_date >= ?1 AND record_date < ?2",
                sql_params![start_ms, end_ms],
                |r| r.get::<_, String>(0),
            )
            .expect("应找到风闪 7-12 pending 记录");

        let data_val: serde_json::Value =
            serde_json::from_str(&data).expect("解析 data JSON 失败");
        let boss_names = data_val["bossNames"]
            .as_array()
            .expect("bossNames 应为数组");
        println!("\n[验证] 重扫后 bossNames = {:?}", boss_names);

        let has_xiaozhuangniang = boss_names
            .iter()
            .any(|v| v.as_str() == Some("笑妆娘"));
        assert!(
            has_xiaozhuangniang,
            "重扫后 bossNames 应包含笑妆娘，实际: {:?}",
            boss_names
        );
        println!("[验证] ✓ bossNames 已包含笑妆娘");
    }

    /// 读取风闪 7-12 pending 记录的 bossNames 字段
    ///
    /// 用途：诊断为什么该 pending 记录没有笑妆娘 BOSS
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_read_fengshan_712_pending -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_read_fengshan_712_pending() {
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 先打印所有 pending 记录，确认数据库状态
        println!("=== 所有 pending 记录 ===");
        let mut stmt = conn
            .prepare(
                "SELECT id, role_id, record_date, raid_name FROM records
                 WHERE source='auto_scan' AND status='pending' ORDER BY record_date DESC",
            )
            .expect("prepare 失败");
        let pending_rows: Vec<(String, String, i64, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");
        for r in &pending_rows {
            let dt = chrono::DateTime::<chrono::Local>::from(
                std::time::UNIX_EPOCH + std::time::Duration::from_millis(r.2 as u64),
            );
            println!("  id={} role={} raid={} date={}", r.0, r.1, r.3, dt);
        }
        println!("pending 记录总数: {}", pending_rows.len());

        // 找风闪 7-12 14:52:51 那条（role_id=293f3c1f-9a2e-44c8-8911-f541074b0c7e）
        let target = pending_rows
            .iter()
            .find(|r| r.1 == "293f3c1f-9a2e-44c8-8911-f541074b0c7e")
            .expect("应能找到风闪 7-12 pending 记录");

        let data: String = conn
            .query_row(
                "SELECT data FROM records WHERE id=?1",
                sql_params![&target.0],
                |r| r.get::<_, String>(0),
            )
            .expect("读取 data 失败");

        println!("\n=== Target Pending Record ===");
        println!("id: {}", target.0);
        println!("role_id: {}", target.1);
        println!("record_date: {}", target.2);
        println!("raid_name: {}", target.3);
        let dt = chrono::DateTime::<chrono::Local>::from(
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(target.2 as u64),
        );
        println!("record_date_local: {}", dt);

        let data_val: serde_json::Value =
            serde_json::from_str(&data).expect("解析 data JSON 失败");
        println!("\n=== data pretty ===");
        println!("{}", serde_json::to_string_pretty(&data_val).unwrap());

        if let Some(boss_names) = data_val.get("bossNames") {
            println!("\n=== bossNames ===");
            println!("{}", serde_json::to_string_pretty(boss_names).unwrap());
        } else {
            println!("\n[bossNames 字段不存在]");
        }

        if let Some(jcl_files) = data_val.get("jclFiles") {
            println!("\n=== jclFiles ===");
            println!("{}", serde_json::to_string_pretty(jcl_files).unwrap());
        }

        if let Some(drops) = data_val.get("drops") {
            println!("\n=== drops ===");
            println!("{}", serde_json::to_string_pretty(drops).unwrap());
        }
    }

    /// 批量清空 JCL 缓存表（一次性维护操作）
    ///
    /// 用途：当 analyze_jcl 逻辑修复后，旧版缓存可能残留错误的 is_kill 判定，
    /// 调用此测试一次性清空所有缓存，下次扫描时所有 JCL 文件会重新解析。
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_clear_all_jcl_cache -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_clear_all_jcl_cache() {
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 先统计当前缓存状态
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM jcl_cache", [], |r| r.get::<_, i64>(0))
            .expect("查询总数失败");
        let is_kill_zero: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM jcl_cache WHERE is_kill = 0",
                [],
                |r| r.get::<_, i64>(0),
            )
            .expect("查询 is_kill=0 失败");
        println!("[清缓存] 当前 jcl_cache: 总计 {} 条, is_kill=0 共 {} 条", total, is_kill_zero);

        // 调用 clear_jcl_cache 函数（与 Tauri 命令共用同一实现）
        let deleted = super::clear_jcl_cache().expect("清空 JCL 缓存失败");
        println!("[清缓存] 已删除 {} 条缓存", deleted);

        // 验证
        let remaining: i64 = conn
            .query_row("SELECT COUNT(*) FROM jcl_cache", [], |r| r.get::<_, i64>(0))
            .expect("查询剩余总数失败");
        println!("[清缓存] 剩余 {} 条", remaining);
        assert_eq!(remaining, 0, "清空后 jcl_cache 应为空");
        println!("[清缓存] ✓ 已清空所有 JCL 缓存，下次扫描会重新解析");
    }

    /// 验证 jcl_cache 表清理后再扫描的状态
    ///
    /// 用途：执行 test_clear_all_jcl_cache + test_scan_this_month_real_data 后，
    /// 检查重新解析后的 jcl_cache 状态，确认 is_kill=0 的记录都是真正的灭团。
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_verify_jcl_cache_after_clear -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_verify_jcl_cache_after_clear() {
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM jcl_cache", [], |r| r.get::<_, i64>(0))
            .expect("查询总数失败");
        let is_kill_zero: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM jcl_cache WHERE is_kill = 0",
                [],
                |r| r.get::<_, i64>(0),
            )
            .expect("查询 is_kill=0 失败");
        let is_kill_one: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM jcl_cache WHERE is_kill = 1",
                [],
                |r| r.get::<_, i64>(0),
            )
            .expect("查询 is_kill=1 失败");
        println!("[验证] 重新解析后 jcl_cache: 总计 {} 条, is_kill=1 共 {} 条, is_kill=0 共 {} 条",
            total, is_kill_one, is_kill_zero);

        // 列出 is_kill=0 的 BOSS 分布（应都是真正灭团）
        let mut stmt = conn
            .prepare(
                "SELECT boss_name, COUNT(*) as cnt FROM jcl_cache
                 WHERE is_kill = 0 AND boss_name IS NOT NULL
                 GROUP BY boss_name ORDER BY cnt DESC",
            )
            .expect("prepare 失败");
        let boss_counts: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");
        println!("\n[验证] is_kill=0 的 BOSS 分布（应为真正灭团）:");
        for (boss, cnt) in &boss_counts {
            println!("  {} - {} 条", boss, cnt);
        }
    }

    /// 诊断风闪 7-12 14:52:51 笑妆娘 JCL 的击杀判定信号
    ///
    /// 用途：测试 analyze_jcl 各信号状态，看为什么 is_kill=false
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_fengshan_xiaozhuangniang_jcl -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_fengshan_xiaozhuangniang_jcl() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 笑妆娘 JCL 路径
        let jcl_path = PathBuf::from(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564258532896@zhcn_hd\userdata\combat_logs\2026-07-12-14-52-51-25人普通阆风悬城(794)-笑妆娘(137088).jcl",
        );
        assert!(jcl_path.exists(), "JCL 文件不存在: {}", jcl_path.display());

        // 加载 raids 配置
        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");
        let raid_entry = raids
            .iter()
            .find(|r| r.raid_id == "25人普通阆风悬城")
            .expect("应找到 25人普通阆风悬城 配置");
        let raid_bosses: Vec<(String, String)> = raid_entry.bosses.clone();
        println!("raid_bosses={:?}", raid_bosses);

        // 笑妆娘 template_id=137088（从 JCL 文件名提取）
        let boss_template_id: i64 = 137088;

        // 删除笑妆娘 JCL 的缓存（强制重新解析）
        let path_str = jcl_path.to_string_lossy().to_string();
        conn.execute(
            "DELETE FROM jcl_cache WHERE file_path = ?1",
            sql_params![&path_str],
        )
        .expect("删除缓存失败");
        println!("[诊断] 已删除笑妆娘 JCL 缓存，准备重新解析");

        // 调用 analyze_jcl_cached 重新解析（会触发 log::info! 诊断日志）
        let analysis = analyze_jcl_cached(
            &conn,
            &jcl_path,
            "笑妆娘",
            boss_template_id,
            &raid_bosses,
        )
        .expect("分析 JCL 失败");

        println!("\n=== 笑妆娘 JCL 分析结果 ===");
        println!("boss_name: {:?}", analysis.boss_name);
        println!("fight_start_ms: {}", analysis.fight_start_ms);
        println!("fight_end_ms: {}", analysis.fight_end_ms);
        println!("is_kill: {}", analysis.is_kill);

        // 同时打印相邻 JCL 的分析结果作为对比
        // (jcl_file_name, boss_name, template_id)
        let adjacent_jcls: [(&str, &str, i64); 3] = [
            ("2026-07-12-14-58-39-25人普通阆风悬城(794)-唐醉(137005).jcl", "唐醉", 137005),
            ("2026-07-12-15-08-29-25人普通阆风悬城(794)-柳公子(137019).jcl", "柳公子", 137019),
            ("2026-07-12-15-15-43-25人普通阆风悬城(794)-阿史那承庆(137017).jcl", "阿史那承庆", 137017),
        ];
        for (jcl_name, boss_name, template_id) in &adjacent_jcls {
            let p = jcl_path.parent().unwrap().join(jcl_name);
            if !p.exists() {
                println!("[对比] JCL 不存在: {}", p.display());
                continue;
            }
            let p_str = p.to_string_lossy().to_string();
            conn.execute(
                "DELETE FROM jcl_cache WHERE file_path = ?1",
                sql_params![&p_str],
            )
            .expect("删除缓存失败");
            let a = analyze_jcl_cached(&conn, &p, boss_name, *template_id, &raid_bosses)
                .expect("分析失败");
            println!(
                "\n[对比] {} → boss={:?} is_kill={} start={} end={}",
                jcl_name,
                a.boss_name,
                a.is_kill,
                a.fight_start_ms,
                a.fight_end_ms
            );
        }
    }

    /// 诊断：列出本月所有账号目录下的 JCL 文件，对比 records 表，定位哪些 JCL 被漏扫
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_list_month_jcl_files -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_list_month_jcl_files() {
        use chrono::TimeZone;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 本月时间范围
        let start_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 7, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();
        let end_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();

        // 加载 raids 配置
        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");
        println!("[配置] 共 {} 个副本", raids.len());

        // 获取游戏目录
        let game_dir = crate::mingyi::drop_scanner::get_game_directory().expect("获取游戏目录失败");
        let game_path = std::path::PathBuf::from(&game_dir);
        let accounts_base = game_path.join(crate::mingyi::drop_scanner::MINGYI_ACCOUNTS_BASE_PATH);

        println!("[扫描] 账号目录: {}", accounts_base.display());

        // 遍历所有账号目录
        let account_dirs = std::fs::read_dir(&accounts_base)
            .expect("读取账号目录失败")
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with("@zhcn_hd"))
            .collect::<Vec<_>>();

        println!("[扫描] 共 {} 个账号目录\n", account_dirs.len());

        let mut total_jcl = 0usize;
        let mut configured_count = 0usize;
        let mut unconfigured_count = 0usize;
        let mut unconfigured_raids: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

        for acc_dir in account_dirs {
            let acc_name = acc_dir.file_name().to_string_lossy().to_string();
            let combat_logs_dir = acc_dir.path().join("userdata").join("combat_logs");
            if !combat_logs_dir.exists() {
                continue;
            }

            // 用 scan_jcl_files 扫描本月 JCL（不传 process_start_ms 限制）
            match crate::mingyi::drop_scanner::scan_jcl_files(
                &acc_dir.path(),
                0, // 不限制进程启动时间
                start_ms,
                end_ms,
            ) {
                Ok(jcl_files) => {
                    if jcl_files.is_empty() {
                        continue;
                    }
                    println!("[账号 {}] 本月 JCL {} 个:", acc_name, jcl_files.len());
                    for jcl in &jcl_files {
                        total_jcl += 1;
                        let dt = chrono::Local.timestamp_millis_opt(jcl.timestamp).unwrap();
                        let matched = match_raid_name(&jcl.raid_display_name, &raids);
                        let status = if matched.is_some() {
                            let bosses_count = matched.map(|m| m.bosses.len()).unwrap_or(0);
                            if bosses_count == 0 {
                                configured_count += 1;
                                "已配置(无BOSS)"
                            } else {
                                configured_count += 1;
                                "已配置(有BOSS)"
                            }
                        } else {
                            unconfigured_count += 1;
                            *unconfigured_raids.entry(jcl.raid_display_name.clone()).or_insert(0) += 1;
                            "未配置"
                        };
                        println!(
                            "  - {} | {} | {}",
                            dt.format("%m-%d %H:%M"),
                            jcl.raid_display_name,
                            status
                        );
                    }
                }
                Err(e) => {
                    println!("[账号 {}] 扫描失败: {}", acc_name, e);
                }
            }
        }

        println!("\n[汇总] 总 JCL: {} | 已配置副本: {} | 未配置副本: {}", total_jcl, configured_count, unconfigured_count);
        println!("\n[未配置副本 JCL 分布]:");
        for (name, cnt) in unconfigured_raids.iter().collect::<std::collections::BTreeMap<_,_>>() {
            println!("  - {} : {} 个 JCL", name, cnt);
        }
    }

    /// 诊断：分析指定路径 JCL 的击杀判定信号，定位无 BOSS 配置副本 is_kill=false 的原因
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_unconfigured_raid_jcl -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_unconfigured_raid_jcl() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 测试 JCL：白帝江关、敖龙岛、西津渡、雷域大泽、狼牙堡·狼神殿（无 BOSS 配置）
        // 路径模式：E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\{uid}@zhcn_hd\userdata\combat_logs\*.jcl
        let combat_logs_dir = PathBuf::from(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564258532896@zhcn_hd\userdata\combat_logs",
        );
        assert!(combat_logs_dir.exists(), "目录不存在: {}", combat_logs_dir.display());

        // 列出该目录下本月白帝江关、敖龙岛、西津渡、雷域大泽的 JCL 文件
        let target_keywords = ["白帝江关", "敖龙岛", "西津渡", "雷域大泽", "狼牙堡·狼神殿"];
        let mut jcl_files: Vec<String> = std::fs::read_dir(&combat_logs_dir)
            .expect("读取目录失败")
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if name.ends_with(".jcl")
                    && name.starts_with("2026-07-")
                    && target_keywords.iter().any(|k| name.contains(k))
                {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();
        jcl_files.sort();
        println!("[扫描] 找到 {} 个候选 JCL 文件", jcl_files.len());

        // 加载 raids 配置
        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");

        // 清空这些 JCL 的缓存，强制重新解析
        for fname in &jcl_files {
            let full_path = combat_logs_dir.join(fname);
            let path_str = full_path.to_string_lossy().to_string();
            conn.execute(
                "DELETE FROM jcl_cache WHERE file_path = ?1",
                sql_params![&path_str],
            )
            .expect("删除缓存失败");
        }
        println!("[诊断] 已清空相关 JCL 缓存\n");

        for fname in &jcl_files {
            let full_path = combat_logs_dir.join(fname);
            // 解析文件名获取 BOSS 名和 ID
            let parsed = parse_jcl_filename(fname).expect("解析文件名失败");
            // 匹配 raids
            let raid_entry = match_raid_name(&parsed.raid_display_name, &raids);
            let raid_bosses: Vec<(String, String)> = raid_entry.map(|e| e.bosses.clone()).unwrap_or_default();
            let in_raids = raid_entry.is_some();
            let bosses_count = raid_bosses.len();

            println!(
                "=== JCL: {} ===",
                fname
            );
            println!(
                "  raid='{}' boss='{}'(id={}) | in_raids={} bosses_count={}",
                parsed.raid_display_name,
                parsed.boss_name,
                parsed.boss_id,
                in_raids,
                bosses_count
            );

            // 调用 analyze_jcl 分析
            let analysis = analyze_jcl(&full_path, &parsed.boss_name, parsed.boss_id, &raid_bosses)
                .expect("analyze_jcl 失败");
            println!(
                "  → is_kill={} boss_name={:?} fight_start={} fight_end={}",
                analysis.is_kill,
                analysis.boss_name,
                analysis.fight_start_ms,
                analysis.fight_end_ms
            );
            println!();
        }
    }

    /// 诊断：列出 raids 表配置 / raid_bosses 配置 / 本月扫描记录，定位"未配置 BOSS 副本被跳过"问题
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_check_unconfigured_raids -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_check_unconfigured_raids() {
        use chrono::TimeZone;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 1. raids 表中所有副本
        let mut stmt = conn
            .prepare("SELECT id, name FROM raids ORDER BY name")
            .expect("prepare raids 失败");
        let raids: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");
        println!("[raids] 共 {} 个副本", raids.len());
        for (id, name) in &raids {
            println!("  - id='{}' name='{}'", id, name);
        }

        // 2. raid_bosses 表中配置了 BOSS 的 raid_name（去重）
        let mut stmt = conn
            .prepare("SELECT DISTINCT raid_name FROM raid_bosses ORDER BY raid_name")
            .expect("prepare raid_bosses 失败");
        let bossed: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");
        println!("\n[raid_bosses] 已配置 BOSS 的副本短名共 {} 个:", bossed.len());
        for n in &bossed {
            println!("  - {}", n);
        }

        // 3. raids 表中无 BOSS 配置的副本
        let no_boss: Vec<&(String, String)> = raids
            .iter()
            .filter(|(_, name)| !bossed.iter().any(|b| b == name))
            .collect();
        println!("\n[无 BOSS 配置] raids 表中无 BOSS 配置的副本共 {} 个:", no_boss.len());
        for (id, name) in &no_boss {
            println!("  - id='{}' name='{}'", id, name);
        }

        // 4. 本月扫描记录
        let start_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 7, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();
        let end_ms = chrono::Local
            .from_local_datetime(
                &chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .unwrap()
            .timestamp_millis();
        let mut stmt = conn
            .prepare(
                "SELECT id, account_id, raid_name, status, source, record_date,
                        json_extract(data, '$.raidName') AS data_raidName,
                        json_extract(data, '$.roleName') AS data_roleName,
                        json_extract(data, '$.bossKillCount') AS bossKillCount,
                        json_extract(data, '$.bossNames') AS bossNames
                 FROM records
                 WHERE record_date >= ?1 AND record_date < ?2
                 ORDER BY record_date",
            )
            .expect("prepare records 失败");
        let rows: Vec<(String, String, String, String, String, i64, String, String, Option<i64>, String)> = stmt
            .query_map(sql_params![start_ms, end_ms], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                    r.get::<_, String>(9)?,
                ))
            })
            .expect("query_map 失败")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect 失败");
        println!("\n[本月 records] 共 {} 条记录:", rows.len());
        for (id, acc, raid_name, status, source, ts, drn, role, bkc, bns) in &rows {
            let dt = chrono::Local.timestamp_millis_opt(*ts).unwrap();
            println!(
                "  - {} | acc={} | raid='{}' dataRaid='{}' role='{}' | status={} source={} | killCount={} bossNames={} | {}",
                dt.format("%m-%d %H:%M"),
                &acc[..acc.len().min(8)],
                raid_name,
                drn,
                role,
                status,
                source,
                bkc.unwrap_or(0),
                bns,
                &id[..id.len().min(8)]
            );
        }

        // 5. 加载 load_raids_with_bosses 的实际结果
        let entries = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");
        let no_boss_entries: Vec<&RaidEntry> = entries.iter().filter(|e| e.bosses.is_empty()).collect();
        println!("\n[load_raids_with_bosses] 共 {} 个 RaidEntry，其中无 BOSS 配置 {} 个:", entries.len(), no_boss_entries.len());
        for e in &no_boss_entries {
            println!("  - raid_id='{}' name='{}'", e.raid_id, e.name);
        }
    }

    /// 验证须罗巨傀 JCL（路径 B：文件名 BOSS 不在配置中，真实 BOSS 为唐怀仁）的击杀判定。
    ///
    /// 场景：唐怀仁在 +97s 中途离场（阶段转换，state=0 存活），+394s 才真正死亡（state=192），
    ///       +441s 击杀后离场。修复前用 +97s 中途离场误判为击杀信号，修复后要求
    ///       LEAVE_SCENE 发生在 state=192 死亡之后，并以 state=192 作为直接死亡信号。
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_xuluojuwei_tanghuairen -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_xuluojuwei_tanghuairen() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let jcl_path = PathBuf::from(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\Interface\MY#DATA\432345564255147961@zhcn_hd\userdata\combat_logs\2026-07-29-21-52-04-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl",
        );
        assert!(jcl_path.exists(), "JCL 文件不存在: {}", jcl_path.display());

        // 加载 raids 配置
        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");
        let raid_entry = raids
            .iter()
            .find(|r| r.raid_id == "25人英雄阆风悬城")
            .expect("应找到 25人英雄阆风悬城 配置");
        let raid_bosses: Vec<(String, String)> = raid_entry.bosses.clone();
        println!("raid_bosses={:?}", raid_bosses);

        // 须罗巨傀 template_id=137175（从 JCL 文件名提取）
        let boss_template_id: i64 = 137175;

        // 删除 JCL 缓存（强制重新解析）
        let path_str = jcl_path.to_string_lossy().to_string();
        conn.execute(
            "DELETE FROM jcl_cache WHERE file_path = ?1",
            sql_params![&path_str],
        )
        .expect("删除缓存失败");

        let analysis = analyze_jcl_cached(
            &conn,
            &jcl_path,
            "须罗巨傀",
            boss_template_id,
            &raid_bosses,
        )
        .expect("分析 JCL 失败");

        println!("\n=== 须罗巨傀 JCL 分析结果 ===");
        println!("boss_name: {:?}", analysis.boss_name);
        println!("fight_start_ms: {}", analysis.fight_start_ms);
        println!("fight_end_ms: {}", analysis.fight_end_ms);
        println!("is_kill: {}", analysis.is_kill);

        // 须罗巨傀不在 raid_bosses 配置中，应通过路径 B 识别真实 BOSS 为唐怀仁
        assert_eq!(
            analysis.boss_name.as_deref(),
            Some("唐怀仁"),
            "应识别真实 BOSS 为唐怀仁"
        );
        // 唐怀仁在 +394s state=192 死亡且未重生，应判定为击杀
        assert!(
            analysis.is_kill,
            "唐怀仁被击杀（state=192 且未重生），is_kill 应为 true"
        );
    }

    /// 回归测试：糯闪 8/1 阆风悬城唐怀仁多 JCL 场景。
    ///
    /// 场景：4 个唐怀仁 JCL（唐怀仁/须罗巨傀 交替），前 3 个为拉脱，
    /// 唐怀仁在战斗中途阶段转换 LEAVE_SCENE（state=0 存活），无 state=192 死亡信号。
    /// 修复前：无 state=192 时 LEAVE_SCENE 被当作击杀信号，拉脱 JCL 误判为 is_kill=true。
    /// 修复后：无 state=192 时 LEAVE_SCENE 不作为击杀信号（不依赖时间阈值），
    ///        击杀判定由 state=192 / bFight=false / HP=0 等本质信号承担。
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_nuoshan_0801_tanghuairen_wipes -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_nuoshan_0801_tanghuairen_wipes() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let base = r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\Interface\MY#DATA\342273571708094124@zhcn_hd\userdata\combat_logs";

        // (文件名, 文件名BOSS, template_id, 期望is_kill)
        let jcls: Vec<(&str, &str, i64, bool)> = vec![
            // 22:45 唐怀仁 — 12s 拉脱（战斗过短）
            ("2026-08-01-22-45-34-25人英雄阆风悬城(795)-唐怀仁(137163).jcl", "唐怀仁", 137163, false),
            // 22:47 须罗巨傀（唐怀仁）— 371s 拉脱，+97s 中途离场
            ("2026-08-01-22-47-44-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl", "须罗巨傀", 137175, false),
            // 22:56 唐怀仁 — 270s 拉脱，+95s 中途离场
            ("2026-08-01-22-56-36-25人英雄阆风悬城(795)-唐怀仁(137163).jcl", "唐怀仁", 137163, false),
            // 23:07 须罗巨傀（唐怀仁）— 481s 击杀，state=192 + 死亡后离场
            ("2026-08-01-23-07-04-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl", "须罗巨傀", 137175, true),
        ];

        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");
        let raid_entry = raids
            .iter()
            .find(|r| r.raid_id == "25人英雄阆风悬城")
            .expect("应找到 25人英雄阆风悬城 配置");
        let raid_bosses: Vec<(String, String)> = raid_entry.bosses.clone();

        for (fname, file_boss, template_id, expected_kill) in &jcls {
            let jcl_path = PathBuf::from(base).join(fname);
            assert!(jcl_path.exists(), "JCL 文件不存在: {}", jcl_path.display());

            let path_str = jcl_path.to_string_lossy().to_string();
            conn.execute("DELETE FROM jcl_cache WHERE file_path = ?1", sql_params![&path_str])
                .expect("删除缓存失败");

            let analysis = analyze_jcl_cached(&conn, &jcl_path, file_boss, *template_id, &raid_bosses)
                .expect("分析 JCL 失败");

            let dur = if analysis.fight_end_ms > analysis.fight_start_ms {
                (analysis.fight_end_ms - analysis.fight_start_ms) / 1000
            } else {
                0
            };
            println!(
                "{}: fight={}s, is_kill={} (期望={})",
                fname, dur, analysis.is_kill, expected_kill
            );
            assert_eq!(
                analysis.is_kill, *expected_kill,
                "JCL {} 的 is_kill 判定错误: 期望 {}, 实际 {}",
                fname, expected_kill, analysis.is_kill
            );
        }
    }

    /// 回归测试：respawned=true 团灭 JCL 不应被误判为击杀。
    ///
    /// 场景：须罗巨傀触发 state=192（死亡），但 state 又变回 0（重生），
    /// 说明团灭后 BOSS 复位。这种情况虽然也有"分身陆续离场"信号，
    /// 但应通过 boss_respawned=true 排除击杀判定。
    ///
    /// 修复前：clone_leave_signal=true 单独判定击杀，导致 4 个 respawned 团灭 JCL
    ///        全部误判为 is_kill=true。
    /// 修复后：death_ok 前置条件增加 !boss_respawned，团灭 JCL 正确判 false。
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_respawned_wipes_not_kill -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_respawned_wipes_not_kill() {
        use std::path::PathBuf;

        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 4 个 JCL（分布在不同账号目录下，自动定位）
        // 使用 match_raid_name 查找 raid 配置（与生产代码一致），须罗巨傀走路径 B（真实 BOSS 为唐怀仁）。
        // (文件名, 期望is_kill)
        // - JCL 1/2: 唐怀仁重生或无死亡信号 → 团灭/未击杀
        // - JCL 3: 唐怀仁 state=192 后未重生且离场 → 合法击杀（旧路径 A 误判为灭团因追踪了须罗巨傀小怪的重生）
        // - JCL 4: 待验证
        let jcls: Vec<(&str, bool)> = vec![
            ("2026-07-01-23-44-25-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl", false),
            ("2026-07-01-23-54-00-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl", false),
            ("2026-07-08-21-33-56-25人英雄阆风悬城(795)-须罗巨傀(137175).jcl", true),
            ("2026-07-16-21-31-53-25人普通阆风悬城(794)-须罗巨傀(137067).jcl", false),
        ];

        let my_data_root = PathBuf::from(r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\Interface\MY#DATA");
        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");

        for (fname, expected_kill) in &jcls {
            // 跨账号定位 JCL
            let mut found: Option<PathBuf> = None;
            if let Ok(entries) = std::fs::read_dir(&my_data_root) {
                for entry in entries.flatten() {
                    let candidate = entry
                        .path()
                        .join("userdata")
                        .join("combat_logs")
                        .join(fname);
                    if candidate.exists() {
                        found = Some(candidate);
                        break;
                    }
                }
            }
            let jcl_path = found.unwrap_or_else(|| {
                panic!("未在 MY#DATA 下找到 JCL: {}", fname);
            });

            let info = parse_jcl_filename(fname).expect("文件名解析失败");
            let raid_entry = match_raid_name(&info.raid_display_name, &raids);
            let raid_bosses: Vec<(String, String)> = raid_entry
                .map(|e| e.bosses.clone())
                .unwrap_or_default();

            let path_str = jcl_path.to_string_lossy().to_string();
            conn.execute("DELETE FROM jcl_cache WHERE file_path = ?1", sql_params![&path_str])
                .expect("删除缓存失败");

            let analysis = analyze_jcl_cached(&conn, &jcl_path, &info.boss_name, info.boss_id, &raid_bosses)
                .expect("分析 JCL 失败");

            println!(
                "{}: fight={}s, is_kill={} (期望={})",
                fname,
                (analysis.fight_end_ms - analysis.fight_start_ms) / 1000,
                analysis.is_kill,
                expected_kill
            );
            assert_eq!(
                analysis.is_kill, *expected_kill,
                "JCL {} 的 is_kill 判定错误: 期望 {}, 实际 {}",
                fname, expected_kill, analysis.is_kill
            );
        }
    }

    /// 诊断：分析用户指定的 2026-08-01 25人英雄阆风悬城 笑妆娘 JCL 为何未识别为击杀。
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_0801_xiaozhuangniang -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_0801_xiaozhuangniang() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let jcl_path = PathBuf::from(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\342273571708094124@zhcn_hd\userdata\combat_logs\2026-08-01-22-03-28-25人英雄阆风悬城(795)-笑妆娘(137205).jcl",
        );
        assert!(jcl_path.exists(), "JCL 文件不存在: {}", jcl_path.display());

        let raids = load_raids_with_bosses(&conn).expect("加载副本配置失败");
        let raid_entry = raids
            .iter()
            .find(|r| r.raid_id == "25人英雄阆风悬城")
            .expect("应找到 25人英雄阆风悬城 配置");
        let raid_bosses: Vec<(String, String)> = raid_entry.bosses.clone();
        println!("raid_bosses={:?}", raid_bosses);

        // 笑妆娘 template_id=137205（从 JCL 文件名提取）
        let boss_template_id: i64 = 137205;

        // 删除该 JCL 的缓存（强制重新解析）
        let path_str = jcl_path.to_string_lossy().to_string();
        conn.execute(
            "DELETE FROM jcl_cache WHERE file_path = ?1",
            sql_params![&path_str],
        )
        .expect("删除缓存失败");
        println!("[诊断] 已删除该 JCL 缓存，准备重新解析");

        let analysis = analyze_jcl_cached(
            &conn,
            &jcl_path,
            "笑妆娘",
            boss_template_id,
            &raid_bosses,
        )
        .expect("分析 JCL 失败");

        println!("\n=== 0801 笑妆娘 JCL 分析结果 ===");
        println!("boss_name: {:?}", analysis.boss_name);
        println!("fight_start_ms: {}", analysis.fight_start_ms);
        println!("fight_end_ms: {}", analysis.fight_end_ms);
        println!("fight_duration: {}s",
            if analysis.fight_end_ms > analysis.fight_start_ms {
                (analysis.fight_end_ms - analysis.fight_start_ms) / 1000
            } else { 0 });
        println!("is_kill: {}", analysis.is_kill);
        // 修复前 is_kill=false（boss_respawned=true 阻止 clone_leave_signal 生效），
        // 修复后 is_kill=true（clone_leave_signal=true, 20 个分身跨度 199s）。
        assert!(analysis.is_kill, "笑妆娘 0801 JCL 应判定为击杀");
    }

    /// 全量扫描所有 JCL 文件，检查击杀识别是否正确
    ///
    /// 两阶段策略：
    /// 1. 阶段1：用现有缓存快速扫描所有 JCL，统计识别情况
    /// 2. 阶段2：对 is_kill=false 的已配置副本 JCL，清空缓存重新解析，确认修复后是否仍为 false
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_scan_all_jcl_check_kill -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_scan_all_jcl_check_kill() {
        use std::collections::HashMap;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 加载 raids 配置
        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");
        println!("[配置] 共 {} 个副本配置", raids.len());

        // 获取游戏目录：优先从配置读取，失败时使用硬编码路径（测试环境后备）
        let game_dir = crate::mingyi::drop_scanner::get_game_directory()
            .unwrap_or_else(|_| r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd".to_string());
        let game_path = std::path::PathBuf::from(&game_dir);
        let accounts_base = game_path.join(crate::mingyi::drop_scanner::MINGYI_ACCOUNTS_BASE_PATH);
        println!("[扫描] 游戏目录: {}", game_dir);
        println!("[扫描] 账号目录: {}\n", accounts_base.display());

        // 遍历所有账号目录
        let account_dirs: Vec<std::fs::DirEntry> = std::fs::read_dir(&accounts_base)
            .expect("读取账号目录失败")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && e.file_name().to_string_lossy().contains("@zhcn_hd")
            })
            .collect();
        println!("[扫描] 共 {} 个账号目录\n", account_dirs.len());

        // 统计数据
        let mut total_jcl = 0usize;
        let mut configured_jcl = 0usize;
        let mut unconfigured_jcl = 0usize;
        let mut kill_count = 0usize;
        let mut wipe_count = 0usize;
        let mut parse_fail_count = 0usize;
        // 按副本名分组统计：(击杀数, 拉托数)
        let mut raid_stats: HashMap<String, (usize, usize)> = HashMap::new();
        // 需要重新解析的 JCL（is_kill=false 的已配置副本）
        let mut need_reparse: Vec<(std::path::PathBuf, String, i64, Vec<(String, String)>)> = Vec::new();

        // 阶段1：用缓存快速扫描
        println!("=== 阶段1：用缓存快速扫描所有 JCL ===");
        for acc_dir in &account_dirs {
            let acc_name = acc_dir.file_name().to_string_lossy().to_string();
            let jcl_files = match scan_jcl_files(&acc_dir.path(), 0, 0, 0) {
                Ok(f) => f,
                Err(e) => {
                    println!("[账号 {}] 扫描失败: {}", acc_name, e);
                    continue;
                }
            };
            if jcl_files.is_empty() {
                continue;
            }
            println!("[账号 {}] JCL 文件数: {}", acc_name, jcl_files.len());

            for jcl in &jcl_files {
                total_jcl += 1;
                let full_path = acc_dir.path()
                    .join("userdata")
                    .join("combat_logs")
                    .join(&jcl.file_name);
                let raid_entry = match_raid_name(&jcl.raid_display_name, &raids);
                let raid_bosses: Vec<(String, String)> = raid_entry
                    .map(|e| e.bosses.clone())
                    .unwrap_or_default();

                if raid_entry.is_none() {
                    unconfigured_jcl += 1;
                    continue;
                }
                configured_jcl += 1;

                // 调用 analyze_jcl_cached（利用缓存）
                let analysis = analyze_jcl_cached(
                    &conn,
                    &full_path,
                    &jcl.boss_name,
                    jcl.boss_id,
                    &raid_bosses,
                );

                match analysis {
                    Some(a) => {
                        if a.is_kill {
                            kill_count += 1;
                            let stat = raid_stats.entry(jcl.raid_display_name.clone()).or_insert((0, 0));
                            stat.0 += 1;
                        } else {
                            wipe_count += 1;
                            let stat = raid_stats.entry(jcl.raid_display_name.clone()).or_insert((0, 0));
                            stat.1 += 1;
                            // 记录需要重新解析的 JCL
                            need_reparse.push((full_path, jcl.boss_name.clone(), jcl.boss_id, raid_bosses.clone()));
                        }
                    }
                    None => {
                        parse_fail_count += 1;
                    }
                }
            }
        }

        println!("\n=== 阶段1 统计结果 ===");
        println!("总 JCL 数: {}", total_jcl);
        println!("已配置副本 JCL: {}", configured_jcl);
        println!("未配置副本 JCL: {}", unconfigured_jcl);
        println!("击杀 (is_kill=true): {}", kill_count);
        println!("拉托 (is_kill=false): {}", wipe_count);
        println!("解析失败 (None): {}", parse_fail_count);
        println!("需要重新解析的 JCL (is_kill=false): {}", need_reparse.len());

        println!("\n[按副本名分组统计] (击杀/拉托):");
        let mut sorted_stats: Vec<_> = raid_stats.iter().collect();
        sorted_stats.sort_by(|a, b| a.0.cmp(b.0));
        for (name, (kill, wipe)) in &sorted_stats {
            println!("  - {}: 击杀={}, 拉托={}", name, kill, wipe);
        }

        // 阶段2：清空缓存重新解析 is_kill=false 的 JCL
        if need_reparse.is_empty() {
            println!("\n=== 阶段2：无需重新解析，所有已配置副本 JCL 均为击杀 ===");
            return;
        }

        println!("\n=== 阶段2：清空缓存重新解析 is_kill=false 的 JCL ===");
        let mut reparse_kill = 0usize;
        let mut reparse_wipe = 0usize;
        let mut reparse_fail = 0usize;
        let mut still_wipe: Vec<(String, bool)> = Vec::new();

        for (path, boss_name, boss_id, raid_bosses) in &need_reparse {
            let path_str = path.to_string_lossy().to_string();
            // 清空该 JCL 的缓存
            let _ = conn.execute(
                "DELETE FROM jcl_cache WHERE file_path = ?1",
                sql_params![&path_str],
            );
            // 重新解析
            match analyze_jcl(path, boss_name, *boss_id, raid_bosses) {
                Some(a) => {
                    if a.is_kill {
                        reparse_kill += 1;
                        still_wipe.push((path.file_name().unwrap_or_default().to_string_lossy().to_string(), false));
                    } else {
                        reparse_wipe += 1;
                        still_wipe.push((path.file_name().unwrap_or_default().to_string_lossy().to_string(), true));
                    }
                }
                None => {
                    reparse_fail += 1;
                    still_wipe.push((path.file_name().unwrap_or_default().to_string_lossy().to_string(), false));
                }
            }
        }

        println!("\n=== 阶段2 重新解析结果 ===");
        println!("重新解析 JCL 数: {}", need_reparse.len());
        println!("修复后变为击杀: {}", reparse_kill);
        println!("仍为拉托: {}", reparse_wipe);
        println!("解析失败: {}", reparse_fail);

        if reparse_wipe > 0 {
            println!("\n[仍为拉托的 JCL 列表] (可能为真实团灭):");
            for (name, is_wipe) in &still_wipe {
                if *is_wipe {
                    println!("  - {}", name);
                }
            }
        }

        // 最终统计
        println!("\n=== 最终统计 ===");
        println!("总 JCL 数: {}", total_jcl);
        println!("已配置副本 JCL: {}", configured_jcl);
        println!("未配置副本 JCL: {}", unconfigured_jcl);
        println!("最终击杀数: {}", kill_count + reparse_kill);
        println!("最终拉托数: {}", reparse_wipe);
        println!("解析失败数: {}", parse_fail_count + reparse_fail);
    }

    /// 诊断：分析 7-26 阿史那承庆 JCL 的击杀判定信号
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_0726_ashina -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_0726_ashina() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        // 获取 raid_bosses 配置
        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");
        let raid_entry = match_raid_name("25人英雄阆风悬城", &raids)
            .expect("未匹配到 25人英雄阆风悬城");
        let raid_bosses = raid_entry.bosses.clone();
        println!("raid_id={}", raid_entry.raid_id);
        println!("raid_bosses={:?}", raid_bosses);

        let jcl_path = PathBuf::from(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564255147961@zhcn_hd\userdata\combat_logs\2026-07-26-15-42-35-25人英雄阆风悬城(795)-阿史那承庆(137130).jcl",
        );
        assert!(jcl_path.exists(), "JCL 文件不存在: {}", jcl_path.display());

        // 清空缓存
        let path_str = jcl_path.to_string_lossy().to_string();
        let _ = conn.execute(
            "DELETE FROM jcl_cache WHERE file_path = ?1",
            sql_params![&path_str],
        );
        println!("[诊断] 已删除该 JCL 缓存，准备重新解析\n");

        let analysis = analyze_jcl(&jcl_path, "阿史那承庆", 137130, &raid_bosses)
            .expect("analyze_jcl 失败");

        println!("=== 7-26 阿史那承庆 JCL 分析结果 ===");
        println!("boss_name: {:?}", analysis.boss_name);
        println!("fight_start_ms: {}", analysis.fight_start_ms);
        println!("fight_end_ms: {}", analysis.fight_end_ms);
        println!("fight_duration: {}s",
            if analysis.fight_end_ms > analysis.fight_start_ms {
                (analysis.fight_end_ms - analysis.fight_start_ms) / 1000
            } else { 0 });
        println!("is_kill: {}", analysis.is_kill);

        // 进一步分析 JCL 原始事件，统计各类事件
        use std::io::BufRead;
        let file = std::fs::File::open(&jcl_path).expect("打开文件失败");
        let reader = std::io::BufReader::new(file);
        let gbk = encoding_rs::GBK;

        let mut event_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        let mut boss_npc_info_count = 0u32;
        let mut boss_fight_hint_count = 0u32;
        let mut boss_enter_scene_count = 0u32;
        let mut boss_leave_scene_count = 0u32;
        let mut boss_death_notify_count = 0u32;
        let mut boss_face_dir_192_count = 0u32;
        let mut boss_face_dir_changes: Vec<(i64, i64)> = Vec::new();
        let mut boss_fight_states: Vec<(i64, bool, i64)> = Vec::new();
        let mut boss_hp_zero_times: Vec<i64> = Vec::new();
        let mut same_name_npc_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut same_name_leave_times: Vec<i64> = Vec::new();
        let mut treasure_box_seen = false;
        let mut boss_dwid_found: i64 = 0;

        for line in reader.split(b'\n') {
            let line_bytes = match line {
                Ok(l) => l,
                Err(_) => continue,
            };
            let mut parts: Vec<&[u8]> = Vec::with_capacity(6);
            let mut start = 0;
            for (i, &b) in line_bytes.iter().enumerate() {
                if b == b'\t' {
                    parts.push(&line_bytes[start..i]);
                    start = i + 1;
                    if parts.len() >= 5 {
                        break;
                    }
                }
            }
            parts.push(&line_bytes[start..]);
            if parts.len() < 5 {
                continue;
            }
            let event_type: u32 = std::str::from_utf8(parts[4])
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            *event_counts.entry(event_type).or_insert(0) += 1;

            let lua_str = gbk.decode(parts[5]).0;
            let lua_trimmed = lua_str.trim();
            let lua_inner = lua_trimmed
                .strip_prefix('{')
                .and_then(|s| s.strip_suffix('}'))
                .unwrap_or(lua_trimmed);
            let fields: Vec<&str> = lua_inner.split(',').collect();
            let ts_sec: i64 = std::str::from_utf8(parts[2])
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            match event_type {
                6 => {
                    // NPC_ENTER_SCENE: { dwID, szName, dwTemplateID, ... }
                    if fields.len() >= 2 {
                        let name = fields[1].trim().trim_matches('"');
                        if name == "阿史那承庆" {
                            boss_enter_scene_count += 1;
                        }
                    }
                }
                7 => {
                    // NPC_LEAVE_SCENE: { dwID, szName, ... }
                    if fields.len() >= 2 {
                        let name = fields[1].trim().trim_matches('"');
                        // dwID 当前仅用于解析校验，不参与统计
                        let _dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                        if name == "阿史那承庆" {
                            boss_leave_scene_count += 1;
                            same_name_leave_times.push(ts_sec);
                        }
                    }
                }
                8 => {
                    // NPC_INFO: { dwID, szName, dwTemplateID, dwEmployer, nX, nY, nZ, nFaceDirection }
                    if fields.len() >= 3 {
                        let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                        let name = fields[1].trim().trim_matches('"');
                        let template_id: i64 = fields[2].trim().parse().unwrap_or(0);
                        if name == "阿史那承庆" {
                            boss_npc_info_count += 1;
                            same_name_npc_ids.insert(dwid);
                            if boss_dwid_found == 0 && template_id == 137130 {
                                boss_dwid_found = dwid;
                            }
                            if fields.len() >= 8 {
                                let face_dir: i64 = fields[7].trim().parse().unwrap_or(-1);
                                if face_dir == 192 {
                                    boss_face_dir_192_count += 1;
                                }
                                boss_face_dir_changes.push((ts_sec, face_dir));
                            }
                        }
                        if name.ends_with("宝箱") && name.contains("阿史那承庆") {
                            treasure_box_seen = true;
                        }
                    }
                }
                9 => {
                    // NPC_FIGHT_HINT: { bFight, szName, ..., nCurrentLife }
                    // 注意：茗伊源码 fields 顺序需验证
                    if fields.len() >= 2 {
                        let name = fields[1].trim().trim_matches('"');
                        if name == "阿史那承庆" {
                            boss_fight_hint_count += 1;
                            let b_fight = fields[0].trim() == "true";
                            // nCurrentLife 可能在 fields[2] 或其他位置
                            let n_current_life: i64 = if fields.len() >= 3 {
                                fields[2].trim().parse().unwrap_or(-1)
                            } else {
                                -1
                            };
                            boss_fight_states.push((ts_sec, b_fight, n_current_life));
                            if n_current_life == 0 {
                                boss_hp_zero_times.push(ts_sec);
                            }
                        }
                    }
                }
                28 => {
                    // SYS_MSG_UI_OME_DEATH_NOTIFY: { dwCharacterID, dwKiller }
                    if fields.len() >= 1 {
                        let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                        if same_name_npc_ids.contains(&dwid) {
                            boss_death_notify_count += 1;
                        }
                    }
                }
                _ => {}
            }
        }

        println!("\n=== JCL 原始事件统计 ===");
        println!("[事件类型分布]:");
        let mut sorted_events: Vec<_> = event_counts.iter().collect();
        sorted_events.sort_by_key(|(k, _)| **k);
        for (evt, cnt) in &sorted_events {
            println!("  type={}: {}", evt, cnt);
        }
        println!("\n[阿史那承庆相关事件]:");
        println!("  NPC_ENTER_SCENE: {}", boss_enter_scene_count);
        println!("  NPC_LEAVE_SCENE: {}", boss_leave_scene_count);
        println!("  NPC_INFO: {}", boss_npc_info_count);
        println!("  NPC_FIGHT_HINT: {}", boss_fight_hint_count);
        println!("  DEATH_NOTIFY: {}", boss_death_notify_count);
        println!("  face_dir=192 次数: {}", boss_face_dir_192_count);
        println!("  宝箱 NPC 出现: {}", treasure_box_seen);
        println!("  BOSS dwID: {}", boss_dwid_found);
        println!("  同名 NPC dwID 数: {}", same_name_npc_ids.len());
        println!("  同名 NPC 离场次数: {}", same_name_leave_times.len());
        if !same_name_leave_times.is_empty() {
            let span = same_name_leave_times.last().unwrap() - same_name_leave_times.first().unwrap();
            println!("  同名 NPC 离场跨度: {}s", span);
        }

        println!("\n[face_dir 变化（前30个）]:");
        for (ts, fd) in boss_face_dir_changes.iter().take(30) {
            println!("  ts={} face_dir={}", ts, fd);
        }
        if boss_face_dir_changes.len() > 30 {
            println!("  ...");
            println!("[face_dir 变化（最后10个）]:");
            for (ts, fd) in boss_face_dir_changes.iter().rev().take(10) {
                println!("  ts={} face_dir={}", ts, fd);
            }
        }

        println!("\n[FIGHT_HINT 状态变化（前20个）]:");
        for (ts, bf, hp) in boss_fight_states.iter().take(20) {
            println!("  ts={} bFight={} hp={}", ts, bf, hp);
        }
        if boss_fight_states.len() > 20 {
            println!("  ...");
            println!("[FIGHT_HINT 状态变化（最后10个）]:");
            for (ts, bf, hp) in boss_fight_states.iter().rev().take(10) {
                println!("  ts={} bFight={} hp={}", ts, bf, hp);
            }
        }

        if !boss_hp_zero_times.is_empty() {
            println!("\n[HP=0 时刻]:");
            for ts in &boss_hp_zero_times {
                println!("  ts={}", ts);
            }
        }

        // 查询数据库中该账号 7-26 的记录
        println!("\n=== 数据库记录查询 ===");

        // 先查 accounts 表
        println!("[accounts 表中包含 '432345564' 的账号]:");
        let mut stmt_acc = conn
            .prepare("SELECT id, account_name FROM accounts WHERE id LIKE '%432345564%' OR account_name LIKE '%432345564%'")
            .expect("查询 accounts 失败");
        let acc_rows = stmt_acc
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("查询 accounts 失败");
        let mut target_acc_id: Option<String> = None;
        for row in acc_rows {
            let (id, name) = row.unwrap();
            println!("  id={} account_name={}", id, name);
            target_acc_id = Some(id);
        }

        // 查 7-25 ~ 7-27 的记录（record_date 是毫秒时间戳）
        let ts_start = chrono::NaiveDate::from_ymd_opt(2026, 7, 25)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();
        let ts_end = chrono::NaiveDate::from_ymd_opt(2026, 7, 28)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp_millis();

        // 先查所有 pending 记录
        println!("\n=== 所有 pending 记录 ===");
        let mut stmt_pending = conn
            .prepare(
                "SELECT id, account_id, raid_name, record_date, status, source, data
                 FROM records
                 WHERE status = 'pending' OR status = 'scanning'
                 ORDER BY record_date DESC",
            )
            .expect("查询 pending 失败");
        let pending_rows = stmt_pending
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .expect("查询 pending 失败");
        for row in pending_rows {
            let (id, acc, raid, date_ts, status, source, data) = row.unwrap();
            let date_str = chrono::DateTime::from_timestamp_millis(date_ts)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| date_ts.to_string());
            println!("\n  --- pending id={} ---", id);
            println!("  acc={} raid={} date={} status={} source={}", acc, raid, date_str, status, source);
            // 解析 data JSON 的关键字段
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                println!("  raidName: {:?}", v.get("raidName").and_then(|x| x.as_str()));
                println!("  bossNames: {:?}", v.get("bossNames"));
                println!("  bossKillCount: {:?}", v.get("bossKillCount"));
                println!("  dropsCount: {:?}", v.get("dropsCount"));
                println!("  jclFiles: {:?}", v.get("jclFiles"));
            }
        }

        println!("\n[7-25 ~ 7-27 records 表记录] (ts {}~{}):", ts_start, ts_end);
        let mut stmt = conn
            .prepare(
                "SELECT id, account_id, raid_name, record_date, status, source,
                        json_extract(data, '$.raidName') as raid_full,
                        json_extract(data, '$.bossNames') as boss_names,
                        json_extract(data, '$.bossKillCount') as boss_kill_count,
                        data
                 FROM records
                 WHERE record_date >= ?1 AND record_date < ?2
                 ORDER BY record_date",
            )
            .expect("查询失败");
        let rows = stmt
            .query_map(sql_params![ts_start, ts_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })
            .expect("查询失败");
        for row in rows {
            let (
                id,
                account_id,
                raid_name,
                record_date_ts,
                status,
                source,
                raid_full,
                boss_names,
                boss_kill_count,
                data,
            ) = row.unwrap();
            let date_str = chrono::DateTime::from_timestamp_millis(record_date_ts)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| record_date_ts.to_string());
            println!(
                "\n  --- record id={} ---\n  acc={} raid={} date={} status={} source={}",
                id, account_id, raid_name, date_str, status, source
            );
            println!("    raidFull={:?}", raid_full);
            println!("    bossNames={:?}", boss_names);
            println!("    bossKillCount={:?}", boss_kill_count);
            // 解析完整 data
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                println!("    dropsCount: {:?}", v.get("dropsCount"));
                println!("    jclFiles: {:?}", v.get("jclFiles"));
                if let Some(boss_names_arr) = v.get("bossNames").and_then(|x| x.as_array()) {
                    println!("    bossNames (array):");
                    for (i, bn) in boss_names_arr.iter().enumerate() {
                        println!("      [{}] {:?}", i, bn);
                    }
                }
            }
        }

        // 查 jcl_cache 中阿史那承庆的缓存
        println!("\n[jcl_cache 中阿史那承庆的缓存]:");
        let mut stmt_cache = conn
            .prepare(
                "SELECT file_path, is_kill, boss_name
                 FROM jcl_cache
                 WHERE file_path LIKE '%432345564%' AND file_path LIKE '%阿史那承庆%'
                 ORDER BY file_path",
            )
            .expect("查询 jcl_cache 失败");
        let cache_rows = stmt_cache
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .expect("查询 jcl_cache 失败");
        for row in cache_rows {
            let (file_path, is_kill, boss_name) = row.unwrap();
            println!("  is_kill={} boss={} file={}", is_kill, boss_name, file_path);
        }

        // 查该账号 7-25~7-27 的所有 records（不限于阆风悬城）
        if let Some(acc_id) = target_acc_id {
            println!("\n[该账号 7-25~7-27 所有 records]:");
            let mut stmt2 = conn
                .prepare(
                    "SELECT id, raid_name, record_date, status, source,
                            json_extract(data, '$.bossNames') as boss_names
                     FROM records
                     WHERE account_id = ?1 AND record_date >= ?2 AND record_date < ?3
                     ORDER BY record_date",
                )
                .expect("查询失败");
            let rows2 = stmt2
                .query_map(sql_params![acc_id, ts_start, ts_end], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                    ))
                })
                .expect("查询失败");
            for row in rows2 {
                let (id, raid_name, date_ts, status, source, boss_names) = row.unwrap();
                let date_str = chrono::DateTime::from_timestamp_millis(date_ts)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| date_ts.to_string());
            println!("  id={} raid={} date={} status={} source={} bossNames={:?}",
                id, raid_name, date_str, status, source, boss_names);
            }
        }

        // 查所有账号
        println!("\n[所有 accounts]:");
        let mut stmt_all_acc = conn
            .prepare("SELECT id, account_name FROM accounts ORDER BY id")
            .expect("查询 accounts 失败");
        let all_acc_rows = stmt_all_acc
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("查询 accounts 失败");
        for row in all_acc_rows {
            let (id, name) = row.unwrap();
            println!("  id={} name={}", id, name);
        }

        // 查最近10条 records
        println!("\n[最近 10 条 records]:");
        let mut stmt_recent = conn
            .prepare(
                "SELECT id, account_id, raid_name, record_date, status, source,
                        json_extract(data, '$.bossNames') as boss_names
                 FROM records
                 ORDER BY record_date DESC
                 LIMIT 10",
            )
            .expect("查询 records 失败");
        let recent_rows = stmt_recent
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .expect("查询 records 失败");
        for row in recent_rows {
            let (id, acc, raid, date_ts, status, source, boss_names) = row.unwrap();
            let date_str = chrono::DateTime::from_timestamp_millis(date_ts)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| date_ts.to_string());
            println!("  id={} acc={} raid={} date={} status={} source={} bossNames={:?}",
                id, acc, raid, date_str, status, source, boss_names);
        }

        // 查 jcl_cache 总数
        let cache_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM jcl_cache", [], |row| row.get(0))
            .unwrap_or(0);
        println!("\n[jcl_cache 总记录数]: {}", cache_count);

        // 查 7-26 confirmed 手动记录的 bossIds
        println!("\n[7-26 confirmed 手动记录的 bossIds]:");
        let mut stmt_bossids = conn
            .prepare(
                "SELECT id, account_id, raid_name,
                        json_extract(data, '$.raidName') as raid_full,
                        json_extract(data, '$.bossIds') as boss_ids,
                        json_extract(data, '$.bossNames') as boss_names,
                        json_extract(data, '$.jclFiles') as jcl_files
                 FROM records
                 WHERE record_date >= ?1 AND record_date < ?2
                   AND status = 'confirmed'
                 ORDER BY record_date",
            )
            .expect("查询失败");
        let bossids_rows = stmt_bossids
            .query_map(sql_params![ts_start, ts_end], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .expect("查询失败");
        for row in bossids_rows {
            let (id, acc, raid, raid_full, boss_ids, boss_names, jcl_files) = row.unwrap();
            println!("\n  id={} acc={} raid={} raidFull={:?}", id, acc, raid, raid_full);
            println!("    bossIds={:?}", boss_ids);
            println!("    bossNames={:?}", boss_names);
            if let Some(jf) = jcl_files {
                if jf.contains("阿史那承庆") {
                    println!("    jclFiles含阿史那承庆: YES");
                }
            }
        }
    }

    /// 诊断：分析 7-29 阿史那承庆 JCL 的击杀判定信号
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_analyze_0729_ashina -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_analyze_0729_ashina() {
        use std::path::PathBuf;
        let conn = crate::db::init_db().expect("初始化数据库失败");

        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");
        let raid_entry = match_raid_name("25人英雄阆风悬城", &raids)
            .expect("未匹配到 25人英雄阆风悬城");
        let raid_bosses = raid_entry.bosses.clone();

        let jcl_files = vec![
            (
                "21:40:23",
                PathBuf::from(
                    r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564255147961@zhcn_hd\userdata\combat_logs\2026-07-29-21-40-23-25人英雄阆风悬城(795)-阿史那承庆(137130).jcl",
                ),
            ),
            (
                "21:41:54",
                PathBuf::from(
                    r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564255147961@zhcn_hd\userdata\combat_logs\2026-07-29-21-41-54-25人英雄阆风悬城(795)-阿史那承庆(137130).jcl",
                ),
            ),
        ];

        for (label, jcl_path) in &jcl_files {
            println!("\n\n========== JCL {} ==========", label);
            if !jcl_path.exists() {
                println!("文件不存在，跳过");
                continue;
            }
            let file_size = std::fs::metadata(jcl_path).map(|m| m.len()).unwrap_or(0);
            println!("文件大小: {} bytes", file_size);

            let path_str = jcl_path.to_string_lossy().to_string();
            let _ = conn.execute(
                "DELETE FROM jcl_cache WHERE file_path = ?1",
                sql_params![&path_str],
            );

            let analysis = analyze_jcl(&jcl_path, "阿史那承庆", 137130, &raid_bosses)
                .expect("analyze_jcl 失败");

            println!("fight_start_ms: {}", analysis.fight_start_ms);
            println!("fight_end_ms: {}", analysis.fight_end_ms);
            println!("fight_duration: {}s",
                if analysis.fight_end_ms > analysis.fight_start_ms {
                    (analysis.fight_end_ms - analysis.fight_start_ms) / 1000
                } else { 0 });
            println!("is_kill: {}", analysis.is_kill);

            // 分析原始事件
            use std::io::BufRead;
            let file = std::fs::File::open(jcl_path).expect("打开文件失败");
            let reader = std::io::BufReader::new(file);
            let gbk = encoding_rs::GBK;

            let mut event_counts: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
            let mut boss_npc_info_count = 0u32;
            let mut boss_fight_hint_count = 0u32;
            let mut boss_enter_scene_count = 0u32;
            let mut boss_leave_scene_count = 0u32;
            let mut boss_death_notify_count = 0u32;
            let mut boss_face_dir_192_count = 0u32;
            let mut boss_face_dir_changes: Vec<(i64, i64)> = Vec::new();
            let mut boss_fight_states: Vec<(i64, bool, i64)> = Vec::new();
            let mut boss_hp_zero_times: Vec<i64> = Vec::new();
            let mut same_name_npc_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
            let mut same_name_leave_times: Vec<i64> = Vec::new();
            let mut treasure_box_seen = false;
            let mut boss_dwid_found: i64 = 0;
            let mut all_npc_names: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut player_fight_states: Vec<(i64, bool)> = Vec::new();
            let mut death_notify_events: Vec<(i64, String)> = Vec::new();
            let mut _boss_last_npc_info: Vec<(i64, String)> = Vec::new();
            let mut all_boss_npc_info: Vec<(i64, String)> = Vec::new();
            // 统计涉及 BOSS dwID 的技能事件
            let boss_dwid_const: i64 = 1074278014;
            let mut boss_skill_effect_count = 0u32;
            let mut boss_skill_effect_times: Vec<(i64, bool, bool)> = Vec::new();
            let mut boss_skill_hit_count = 0u32;
            let mut boss_skill_hit_times: Vec<(i64, bool, bool)> = Vec::new();
            let mut boss_skill_cast_count = 0u32;
            let mut boss_skill_cast_times: Vec<i64> = Vec::new();
            // 统计所有事件的时间分布，检测记录中断
            let mut all_event_times: Vec<i64> = Vec::new();
            let mut total_line_count = 0u32;

            for line in reader.split(b'\n') {
                let line_bytes = match line {
                    Ok(l) => l,
                    Err(_) => continue,
                };
                let mut parts: Vec<&[u8]> = Vec::with_capacity(6);
                let mut start = 0;
                for (i, &b) in line_bytes.iter().enumerate() {
                    if b == b'\t' {
                        parts.push(&line_bytes[start..i]);
                        start = i + 1;
                        if parts.len() >= 5 {
                            break;
                        }
                    }
                }
                parts.push(&line_bytes[start..]);
                if parts.len() < 5 {
                    continue;
                }
                let event_type: u32 = std::str::from_utf8(parts[4])
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                *event_counts.entry(event_type).or_insert(0) += 1;
                total_line_count += 1;

                let lua_str = gbk.decode(parts[5]).0;
                let lua_trimmed = lua_str.trim();
                let lua_inner = lua_trimmed
                    .strip_prefix('{')
                    .and_then(|s| s.strip_suffix('}'))
                    .unwrap_or(lua_trimmed);
                let fields: Vec<&str> = lua_inner.split(',').collect();
                let ts_sec: i64 = std::str::from_utf8(parts[2])
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                all_event_times.push(ts_sec);

                match event_type {
                    1 => {
                        if fields.len() >= 1 {
                            let b_fight = fields[0].trim() == "true";
                            player_fight_states.push((ts_sec, b_fight));
                        }
                    }
                    6 => {
                        if fields.len() >= 2 {
                            let name = fields[1].trim().trim_matches('"');
                            if name == "阿史那承庆" {
                                boss_enter_scene_count += 1;
                            }
                        }
                    }
                    7 => {
                        if fields.len() >= 2 {
                            let name = fields[1].trim().trim_matches('"');
                            if name == "阿史那承庆" {
                                boss_leave_scene_count += 1;
                                same_name_leave_times.push(ts_sec);
                            }
                        }
                    }
                    8 => {
                        if fields.len() >= 3 {
                            let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                            let name = fields[1].trim().trim_matches('"');
                            let template_id: i64 = fields[2].trim().parse().unwrap_or(0);
                            if name == "阿史那承庆" {
                                boss_npc_info_count += 1;
                                same_name_npc_ids.insert(dwid);
                                if boss_dwid_found == 0 && template_id == 137130 {
                                    boss_dwid_found = dwid;
                                }
                                if fields.len() >= 8 {
                                    let face_dir: i64 = fields[7].trim().parse().unwrap_or(-1);
                                    if face_dir == 192 {
                                        boss_face_dir_192_count += 1;
                                    }
                                    boss_face_dir_changes.push((ts_sec, face_dir));
                                }
                                // 记录所有 BOSS NPC_INFO 的完整字段
                                all_boss_npc_info.push((ts_sec, lua_inner.to_string()));
                            }
                            all_npc_names.insert(name.to_string());
                            if name.contains("宝箱") {
                                treasure_box_seen = true;
                                println!("[!] 发现宝箱NPC: {} ts={}", name, ts_sec);
                            }
                        }
                    }
                    9 => {
                        if fields.len() >= 2 {
                            let name = fields[1].trim().trim_matches('"');
                            if name == "阿史那承庆" {
                                boss_fight_hint_count += 1;
                                let b_fight = fields[0].trim() == "true";
                                let n_current_life: i64 = if fields.len() >= 3 {
                                    fields[2].trim().parse().unwrap_or(-1)
                                } else { -1 };
                                boss_fight_states.push((ts_sec, b_fight, n_current_life));
                                if n_current_life == 0 {
                                    boss_hp_zero_times.push(ts_sec);
                                }
                            }
                        }
                    }
                    28 => {
                        // 记录所有 DEATH_NOTIFY 事件
                        death_notify_events.push((ts_sec, lua_inner.to_string()));
                        if fields.len() >= 1 {
                            let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                            if same_name_npc_ids.contains(&dwid) {
                                boss_death_notify_count += 1;
                                println!("[!] DEATH_NOTIFY 匹配 BOSS dwID={} ts={}", dwid, ts_sec);
                            }
                        }
                    }
                    19 => {
                        // SKILL_CAST_LOG: fields[0]=施法者dwID
                        if fields.len() >= 1 {
                            let caster: i64 = fields[0].trim().parse().unwrap_or(0);
                            if caster == boss_dwid_const {
                                boss_skill_cast_count += 1;
                                boss_skill_cast_times.push(ts_sec);
                            }
                        }
                    }
                    21 => {
                        // SKILL_EFFECT_LOG: fields[0]=施法者, fields[1]=目标
                        if fields.len() >= 2 {
                            let caster: i64 = fields[0].trim().parse().unwrap_or(0);
                            let target: i64 = fields[1].trim().parse().unwrap_or(0);
                            if caster == boss_dwid_const || target == boss_dwid_const {
                                boss_skill_effect_count += 1;
                                boss_skill_effect_times.push((ts_sec, caster == boss_dwid_const, target == boss_dwid_const));
                            }
                        }
                    }
                    25 => {
                        // SKILL_HIT_LOG: fields[0]=施法者, fields[1]=目标
                        if fields.len() >= 2 {
                            let caster: i64 = fields[0].trim().parse().unwrap_or(0);
                            let target: i64 = fields[1].trim().parse().unwrap_or(0);
                            if caster == boss_dwid_const || target == boss_dwid_const {
                                boss_skill_hit_count += 1;
                                boss_skill_hit_times.push((ts_sec, caster == boss_dwid_const, target == boss_dwid_const));
                            }
                        }
                    }
                    _ => {}
                }
            }

            println!("\n[事件类型分布]:");
            let mut sorted_events: Vec<_> = event_counts.iter().collect();
            sorted_events.sort_by_key(|(k, _)| **k);
            for (evt, cnt) in &sorted_events {
                println!("  type={}: {}", evt, cnt);
            }
            println!("\n[阿史那承庆相关事件]:");
            println!("  NPC_ENTER_SCENE: {}", boss_enter_scene_count);
            println!("  NPC_LEAVE_SCENE: {}", boss_leave_scene_count);
            println!("  NPC_INFO: {}", boss_npc_info_count);
            println!("  NPC_FIGHT_HINT: {}", boss_fight_hint_count);
            println!("  DEATH_NOTIFY(匹配BOSS): {}", boss_death_notify_count);
            println!("  face_dir=192 次数: {}", boss_face_dir_192_count);
            println!("  宝箱 NPC 出现: {}", treasure_box_seen);
            println!("  BOSS dwID: {}", boss_dwid_found);
            println!("  同名 NPC dwID 数: {}", same_name_npc_ids.len());
            println!("  同名 NPC 离场次数: {}", same_name_leave_times.len());
            if !same_name_leave_times.is_empty() {
                let span = same_name_leave_times.last().unwrap() - same_name_leave_times.first().unwrap();
                println!("  同名 NPC 离场跨度: {}s", span);
            }

            println!("\n[含宝箱/阿史/承庆的 NPC 名称]:");
            let mut names: Vec<_> = all_npc_names.iter().collect();
            names.sort();
            for n in names {
                if n.contains("宝箱") || n.contains("阿史") || n.contains("承庆") {
                    println!("  {}", n);
                }
            }

            println!("\n[face_dir 变化（前20个）]:");
            for (ts, fd) in boss_face_dir_changes.iter().take(20) {
                println!("  ts={} face_dir={}", ts, fd);
            }
            if boss_face_dir_changes.len() > 20 {
                println!("  ...");
                println!("[face_dir 变化（最后10个）]:");
                for (ts, fd) in boss_face_dir_changes.iter().rev().take(10) {
                    println!("  ts={} face_dir={}", ts, fd);
                }
            }

            println!("\n[FIGHT_HINT 状态变化（前20个）]:");
            for (ts, bf, hp) in boss_fight_states.iter().take(20) {
                println!("  ts={} bFight={} hp={}", ts, bf, hp);
            }
            if boss_fight_states.len() > 20 {
                println!("  ...");
                println!("[FIGHT_HINT 状态变化（最后10个）]:");
                for (ts, bf, hp) in boss_fight_states.iter().rev().take(10) {
                    println!("  ts={} bFight={} hp={}", ts, bf, hp);
                }
            }

            if !boss_hp_zero_times.is_empty() {
                println!("\n[HP=0 时刻]:");
                for ts in &boss_hp_zero_times {
                    println!("  ts={}", ts);
                }
            }

            println!("\n[玩家 FIGHT_TIME 状态变化]:");
            for (ts, bf) in player_fight_states.iter().take(10) {
                println!("  ts={} bFight={}", ts, bf);
            }
            if player_fight_states.len() > 10 {
                println!("  ...");
                for (ts, bf) in player_fight_states.iter().rev().take(5) {
                    println!("  ts={} bFight={}", ts, bf);
                }
            }

            // 打印所有 DEATH_NOTIFY 事件
            println!("\n[所有 DEATH_NOTIFY 事件]:");
            for (ts, data) in &death_notify_events {
                println!("  ts={} data={}", ts, data);
            }

            // 打印所有 BOSS NPC_INFO 的完整字段
            println!("\n[所有阿史那承庆 NPC_INFO 完整字段]:");
            for (ts, data) in &all_boss_npc_info {
                println!("  ts={} fields={}", ts, data);
            }

            // 打印涉及 BOSS 的技能事件统计
            println!("\n[涉及BOSS的技能事件统计]:");
            println!("  SKILL_CAST(BOSS施法): {}", boss_skill_cast_count);
            println!("  SKILL_EFFECT(BOSS参与): {}", boss_skill_effect_count);
            println!("  SKILL_HIT(BOSS参与): {}", boss_skill_hit_count);
            println!("  总事件行数: {}", total_line_count);

            // 统计所有事件的时间分布，检测记录中断
            if !all_event_times.is_empty() {
                all_event_times.sort();
                let first_ts = *all_event_times.first().unwrap();
                let last_ts = *all_event_times.last().unwrap();
                let total_span = last_ts - first_ts;
                println!("\n[所有事件时间分布（检测记录中断）]:");
                println!("  首事件 ts={}", first_ts);
                println!("  末事件 ts={}", last_ts);
                println!("  时间跨度: {}s", total_span);
                println!("  平均事件密度: {:.1} 次/秒", all_event_times.len() as f64 / total_span as f64);
                // 按30秒一个桶统计
                println!("  [按30秒桶分布]:");
                let mut bucket = first_ts;
                loop {
                    let bucket_end = bucket + 30;
                    let count = all_event_times.iter().filter(|ts| **ts >= bucket && **ts < bucket_end).count();
                    let skill_effect_in_bucket = boss_skill_effect_times.iter()
                        .filter(|(ts, _, _)| *ts >= bucket && *ts < bucket_end)
                        .count();
                    println!("    ts={}~{}: 总事件={}次, BOSS技能事件={}次", bucket, bucket_end, count, skill_effect_in_bucket);
                    if bucket_end >= last_ts {
                        break;
                    }
                    bucket = bucket_end;
                }
            }
            if !boss_skill_effect_times.is_empty() {
                let first = boss_skill_effect_times.first().unwrap().0;
                let last = boss_skill_effect_times.last().unwrap().0;
                println!("  SKILL_EFFECT 时间范围: ts={} ~ ts={} (跨度{}s)", first, last, last - first);
                // 按时间段统计：每60秒一个桶
                let bucket_start = first;
                let mut bucket = bucket_start;
                println!("  [SKILL_EFFECT 按时间段分布(每60秒)]:");
                loop {
                    let bucket_end = bucket + 60;
                    let count = boss_skill_effect_times.iter().filter(|(ts, _, _)| *ts >= bucket && *ts < bucket_end).count();
                    if count > 0 {
                        println!("    ts={}~{}: {}次", bucket, bucket_end, count);
                    }
                    if bucket_end >= last {
                        break;
                    }
                    bucket = bucket_end;
                }
            }
            if !boss_skill_hit_times.is_empty() {
                let first = boss_skill_hit_times.first().unwrap().0;
                let last = boss_skill_hit_times.last().unwrap().0;
                println!("  SKILL_HIT 时间范围: ts={} ~ ts={} (跨度{}s)", first, last, last - first);
            }
        }
    }

    /// 提取 JCL 关键信号（简化版，用于新旧逻辑对比）
    /// 只提取 is_kill 判定所需的信号，不处理 real_boss 路径（路径 B）
    struct KillSignals {
        treasure_box_count: u32,
        boss_fight_true_seen: bool,
        boss_fight_false_seen: bool,
        boss_hp_zero_seen: bool,
        boss_death_notify_seen: bool,
        boss_face_dir_dead: bool,
        boss_face_dir_respawned: bool,
        boss_leave_scene_seen: bool,
        same_name_leave_scene_count: u32,
        clone_leave_span_sec: i64,
    }

    fn extract_kill_signals(
        jcl_path: &std::path::Path,
        boss_name: &str,
        boss_template_id: i64,
        raid_bosses: &[(String, String)],
    ) -> Option<KillSignals> {
        use std::io::BufRead;
        let file = std::fs::File::open(jcl_path).ok()?;
        let reader = std::io::BufReader::new(file);
        let gbk = encoding_rs::GBK;

        let file_boss_in_config = is_valid_boss(boss_name, raid_bosses);

        let mut boss_dwid: i64 = 0;
        let mut boss_fight_true_seen = false;
        let mut boss_fight_false_seen = false;
        let mut boss_hp_zero_seen = false;
        let mut boss_death_notify_seen = false;
        let mut boss_face_dir_dead = false;
        let mut boss_face_dir_respawned = false;
        let mut boss_leave_scene_seen = false;
        let mut treasure_box_count: u32 = 0;
        let mut same_name_dwids: std::collections::HashSet<i64> = std::collections::HashSet::new();
        let mut same_name_leave_scene_count: u32 = 0;
        let mut first_clone_leave_ms: i64 = 0;
        let mut last_clone_leave_ms: i64 = 0;

        for line in reader.split(b'\n') {
            let line_bytes = line.ok()?;
            let mut parts: Vec<&[u8]> = Vec::with_capacity(6);
            let mut start = 0;
            for (i, &b) in line_bytes.iter().enumerate() {
                if b == b'\t' {
                    parts.push(&line_bytes[start..i]);
                    start = i + 1;
                    if parts.len() >= 5 { break; }
                }
            }
            if parts.len() >= 5 { parts.push(&line_bytes[start..]); }
            if parts.len() < 6 { continue; }

            let event_type: u8 = match std::str::from_utf8(parts[4]).ok().and_then(|s| s.parse().ok()) {
                Some(t) => t,
                None => continue,
            };
            if event_type != 1 && event_type != 7 && event_type != 8 && event_type != 9 && event_type != 28 {
                continue;
            }

            let timestamp_sec: i64 = match std::str::from_utf8(parts[2]).ok().and_then(|s| s.parse().ok()) {
                Some(t) => t,
                None => continue,
            };

            let lua_str = gbk.decode(parts[5]).0;
            let lua_trimmed = lua_str.trim();
            let lua_inner = lua_trimmed.strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or(lua_trimmed);
            let fields: Vec<&str> = lua_inner.split(',').collect();

            match event_type {
                1 => {
                    if !fields.is_empty() {
                        let bfight = fields[0].trim() == "true";
                        if raid_bosses.is_empty() && bfight {
                            boss_fight_true_seen = true;
                        }
                    }
                }
                8 => {
                    if fields.len() < 3 { continue; }
                    let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                    let name = fields[1].trim();
                    let template_id: i64 = fields[2].trim().parse().unwrap_or(0);
                    let name_trimmed = name.trim_matches('"');

                    if boss_dwid == 0 && boss_template_id != 0 && template_id == boss_template_id {
                        boss_dwid = dwid;
                    }

                    // 同名 NPC（分身/幻影）
                    if file_boss_in_config && name_trimmed == boss_name {
                        same_name_dwids.insert(dwid);
                    }

                    // 宝箱检测
                    if name_trimmed.ends_with("宝箱") {
                        let prefix_len = name_trimmed.len() - "宝箱".len();
                        let prefix = &name_trimmed[..prefix_len];
                        for (_bid, bname) in raid_bosses {
                            if prefix == *bname {
                                treasure_box_count += 1;
                                break;
                            }
                        }
                        if raid_bosses.is_empty() && !boss_name.is_empty() && prefix.contains(boss_name) {
                            treasure_box_count += 1;
                        }
                    }

                    // face_dir 追踪
                    if boss_dwid != 0 && dwid == boss_dwid && fields.len() >= 8 {
                        let face_dir: i64 = fields[7].trim().parse().unwrap_or(0);
                        if face_dir == 192 {
                            boss_face_dir_dead = true;
                        } else if face_dir == 0 && boss_face_dir_dead {
                            boss_face_dir_respawned = true;
                        }
                    }
                }
                9 => {
                    if fields.len() < 3 { continue; }
                    let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                    let bfight = fields[1].trim() == "true";
                    let hp: f64 = fields[2].trim().parse().unwrap_or(-1.0);
                    if same_name_dwids.contains(&dwid) || dwid == boss_dwid {
                        if bfight { boss_fight_true_seen = true; } else { boss_fight_false_seen = true; }
                        if hp == 0.0 { boss_hp_zero_seen = true; }
                    }
                }
                28 => {
                    if fields.is_empty() { continue; }
                    let dead_dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                    if same_name_dwids.contains(&dead_dwid) || dead_dwid == boss_dwid {
                        boss_death_notify_seen = true;
                    }
                }
                7 => {
                    if fields.is_empty() { continue; }
                    let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                    if dwid == boss_dwid {
                        boss_leave_scene_seen = true;
                    } else if same_name_dwids.contains(&dwid) {
                        same_name_leave_scene_count += 1;
                        let ts_ms = timestamp_sec * 1000;
                        if first_clone_leave_ms == 0 { first_clone_leave_ms = ts_ms; }
                        last_clone_leave_ms = ts_ms;
                    }
                }
                _ => {}
            }
        }

        let clone_leave_span_sec = if first_clone_leave_ms > 0 {
            (last_clone_leave_ms - first_clone_leave_ms) / 1000
        } else {
            0
        };

        Some(KillSignals {
            treasure_box_count,
            boss_fight_true_seen,
            boss_fight_false_seen,
            boss_hp_zero_seen,
            boss_death_notify_seen,
            boss_face_dir_dead,
            boss_face_dir_respawned,
            boss_leave_scene_seen,
            same_name_leave_scene_count,
            clone_leave_span_sec,
        })
    }

    /// 计算新旧 is_kill
    /// use_new=true 用新逻辑（!respawned），use_new=false 用旧逻辑（span>=120）
    fn eval_is_kill_with_signals(s: &KillSignals, use_new: bool) -> bool {
        let treasure_ok = s.treasure_box_count > 0;
        let has_death_sign = s.boss_death_notify_seen || s.boss_hp_zero_seen || s.boss_face_dir_dead;
        let no_respawn = !s.boss_face_dir_respawned;

        let clone_leave_signal = if use_new {
            s.same_name_leave_scene_count > 0 && has_death_sign && no_respawn
        } else {
            s.same_name_leave_scene_count > 0 && s.clone_leave_span_sec >= 120 && has_death_sign
        };

        let bfight_false_is_kill = s.boss_fight_false_seen && !s.boss_leave_scene_seen;
        let leave_scene_is_kill = s.boss_leave_scene_seen && !s.boss_fight_false_seen && no_respawn && s.boss_face_dir_dead;

        let death_ok = s.boss_fight_true_seen
            && (s.boss_death_notify_seen
                || s.boss_hp_zero_seen
                || bfight_false_is_kill
                || clone_leave_signal
                || leave_scene_is_kill);

        treasure_ok || death_ok
    }

    /// 一致性测试：扫描所有 JCL，对比新旧 clone_leave_signal 逻辑的 is_kill 判定
    ///
    /// 运行：
    ///   cargo test --bin jx3-raid-manager test_consistency_old_vs_new -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_consistency_old_vs_new() {
        let conn = crate::db::init_db().expect("初始化数据库失败");
        let raids = load_raids_with_bosses(&conn).expect("load_raids_with_bosses 失败");

        let game_dir = crate::mingyi::drop_scanner::get_game_directory()
            .unwrap_or_else(|_| r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd".to_string());
        let game_path = std::path::PathBuf::from(&game_dir);
        let accounts_base = game_path.join(crate::mingyi::drop_scanner::MINGYI_ACCOUNTS_BASE_PATH);

        let account_dirs: Vec<std::fs::DirEntry> = std::fs::read_dir(&accounts_base)
            .expect("读取账号目录失败")
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && e.file_name().to_string_lossy().contains("@zhcn_hd")
            })
            .collect();

        let mut total = 0usize;
        let mut changed = 0usize;
        let mut old_kill_new_false = 0usize; // 旧误判为击杀，新修正为拉托
        let mut old_false_new_kill = 0usize; // 旧漏判，新识别为击杀
        let mut by_boss: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut details: Vec<(String, String, bool, bool, KillSignals)> = Vec::new();

        for acc_dir in &account_dirs {
            let acc_name = acc_dir.file_name().to_string_lossy().to_string();
            let jcl_files = match scan_jcl_files(&acc_dir.path(), 0, 0, 0) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if jcl_files.is_empty() { continue; }

            for jcl in &jcl_files {
                let raid_entry = match_raid_name(&jcl.raid_display_name, &raids);
                if raid_entry.is_none() { continue; }
                let raid_bosses = raid_entry.unwrap().bosses.clone();

                let full_path = acc_dir.path()
                    .join("userdata")
                    .join("combat_logs")
                    .join(&jcl.file_name);

                total += 1;

                let signals = match extract_kill_signals(&full_path, &jcl.boss_name, jcl.boss_id, &raid_bosses) {
                    Some(s) => s,
                    None => continue,
                };

                let old_kill = eval_is_kill_with_signals(&signals, false);
                let new_kill = eval_is_kill_with_signals(&signals, true);

                if old_kill != new_kill {
                    changed += 1;
                    if old_kill && !new_kill { old_kill_new_false += 1; }
                    else { old_false_new_kill += 1; }
                    *by_boss.entry(jcl.boss_name.clone()).or_insert(0) += 1;
                    details.push((acc_name.clone(), jcl.file_name.clone(), old_kill, new_kill, signals));
                }
            }
        }

        println!("\n=== JCL is_kill 新旧逻辑一致性报告 ===");
        println!("总 JCL 文件数: {}", total);
        println!("判定变化文件数: {}", changed);
        println!("  旧=True->新=False (旧误判为击杀，新修正): {}", old_kill_new_false);
        println!("  旧=False->新=True (新逻辑多识别): {}", old_false_new_kill);

        if !by_boss.is_empty() {
            println!("\n按 BOSS 分类变化数:");
            let mut sorted: Vec<_> = by_boss.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            for (name, count) in &sorted {
                println!("  {} : {} 个", name, count);
            }
        }

        if !details.is_empty() {
            println!("\n=== 不一致详情 ===");
            let mut report = String::new();
            report.push_str(&format!("=== JCL is_kill 新旧逻辑一致性报告 ===\n"));
            report.push_str(&format!("总 JCL 文件数: {}\n", total));
            report.push_str(&format!("判定变化文件数: {}\n", changed));
            report.push_str(&format!("  旧=True->新=False (旧误判为击杀，新修正): {}\n", old_kill_new_false));
            report.push_str(&format!("  旧=False->新=True (新逻辑多识别): {}\n", old_false_new_kill));
            report.push_str(&format!("\n按 BOSS 分类变化数:\n"));
            let mut sorted: Vec<_> = by_boss.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(b.0));
            for (name, count) in &sorted {
                report.push_str(&format!("  {} : {} 个\n", name, count));
            }
            report.push_str("\n=== 不一致详情 ===\n");
            for (acc, file, old_k, new_k, s) in &details {
                let line = format!("\n[{}] {}\n  信号: hp0={} death={} face192={} respawn={} box={} fight_false={} leave={} clone={} span={}s\n  旧 IS_KILL={} -> 新 IS_KILL={}\n",
                    acc, file,
                    s.boss_hp_zero_seen, s.boss_death_notify_seen, s.boss_face_dir_dead,
                    s.boss_face_dir_respawned, s.treasure_box_count,
                    s.boss_fight_false_seen, s.boss_leave_scene_seen,
                    s.same_name_leave_scene_count, s.clone_leave_span_sec,
                    old_k, new_k);
                report.push_str(&line);
                print!("{}", line);
            }
            let report_path = std::env::temp_dir().join("jcl_consistency_report.txt");
            std::fs::write(&report_path, &report).expect("写入报告失败");
            println!("\n报告已写入: {}", report_path.display());
        } else {
            println!("\n*** 所有 JCL 新旧逻辑完全一致 ***");
        }
    }

    /// 复核路径场景：扫描区间上界恰为下一条 JCL（开区间把它排除），
    /// 必须依赖调用方传入的 known_next_jcl_time 才能识别"副本已切换"。
    /// 时间戳取自 2026-09-13 实机记录：九老洞 20:07:44~20:19:05，下一条 JCL 为 20:25:29。
    #[test]
    fn test_resolve_next_jcl_time_falls_back_to_known_switch_point() {
        let scanned = vec![
            1789301264000, // 20:07:44 魏华
            1789301378000, // 20:09:38 钟不归
            1789301513000, // 20:11:53 新月卫
            1789301544000, // 20:12:24 岑伤
            1789301670000, // 20:14:30 鬼筹
            1789301803000, // 20:16:43 麒麟
            1789301945000, // 20:19:05 暗梦仙体（本实例最后一个 JCL）
        ];
        let last_jcl_time = 1789301945000;
        let known = Some(1789302329000); // 20:25:29 勒齐那，等于 scan_end_ms，被开区间排除
        assert_eq!(
            resolve_next_jcl_time(&scanned, last_jcl_time, known),
            Some(1789302329000)
        );
    }

    /// 批量路径场景：区间内即可观察到后续副本的 JCL，优先采用扫描结果
    #[test]
    fn test_resolve_next_jcl_time_prefers_scanned_timestamp() {
        let scanned = vec![1000, 2000, 3500];
        assert_eq!(resolve_next_jcl_time(&scanned, 2000, Some(9999)), Some(3500));
    }

    /// known_next_jcl_time 不晚于 last_jcl_time 时不生效（守卫），避免误判其他实例
    #[test]
    fn test_resolve_next_jcl_time_ignores_stale_known() {
        let scanned = vec![1000, 2000];
        assert_eq!(resolve_next_jcl_time(&scanned, 2000, Some(2000)), None);
        assert_eq!(resolve_next_jcl_time(&scanned, 2000, Some(1500)), None);
    }

    /// 无后续 JCL（最后一个实例）→ None，交由进程退出/角色离线/时间兜底等条件判定
    #[test]
    fn test_resolve_next_jcl_time_none_without_switch() {
        let scanned = vec![1000, 2000];
        assert_eq!(resolve_next_jcl_time(&scanned, 2000, None), None);
    }

    /// 构造"进行中"的测试副本实例：last_jcl_time 为 1 小时前，不会触发 6 小时时间兜底
    fn build_incomplete_scan_raid_instance() -> RaidInstance {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        RaidInstance {
            account_id: "test_account".to_string(),
            raid_display_name: "测试副本".to_string(),
            map_id: 0,
            start_time: now_ms - 3600_000,
            end_time: now_ms - 1800_000,
            last_jcl_time: now_ms - 3600_000,
            bosses_killed: vec![],
            jcl_files: vec![],
            jcl_boss_names: vec![],
            boss_kill_count: 0,
            first_gold_time: now_ms - 3600_000,
        }
    }

    /// 验证无底薪 + 收入命中 + 已有掉落分配 → 立即判定完成 (pending)
    #[test]
    fn test_is_raid_complete_income_hit_with_drops_pending() {
        let instance = build_incomplete_scan_raid_instance();
        let empty_bosses: Vec<(String, String)> = vec![];
        assert!(is_raid_complete(
            &instance,
            &empty_bosses,
            false, // 无底薪
            true,  // jx3_running
            true,  // role_online
            false, // has_next_jcl
            true,  // income_matched
            true,  // has_drops
        ));
    }

    /// 验证有分配但收入未命中（老一刚分配工资未发，或新 JCL 更新 last_jcl 后旧收入不再匹配）
    /// → 保持 scanning，不误判完成
    #[test]
    fn test_is_raid_complete_drops_without_income_stays_scanning() {
        let instance = build_incomplete_scan_raid_instance();
        let empty_bosses: Vec<(String, String)> = vec![];
        assert!(!is_raid_complete(
            &instance,
            &empty_bosses,
            false,
            true,
            true,
            false,
            false, // income_matched = false
            true,  // has_drops = true
        ));
    }

    /// 验证收入命中但无任何分配记录 → 保持 scanning（不因误命中收入提前完成）
    #[test]
    fn test_is_raid_complete_income_hit_without_drops_stays_scanning() {
        let instance = build_incomplete_scan_raid_instance();
        let empty_bosses: Vec<(String, String)> = vec![];
        assert!(!is_raid_complete(
            &instance,
            &empty_bosses,
            false,
            true,
            true,
            false,
            true,  // income_matched
            false, // has_drops = false
        ));
    }

    /// 回归保护：有底薪时底薪条件恒优先，不受新增 income_matched 参数影响
    #[test]
    fn test_is_raid_complete_salary_flow_unchanged() {
        let instance = build_incomplete_scan_raid_instance();
        let empty_bosses: Vec<(String, String)> = vec![];
        assert!(is_raid_complete(
            &instance,
            &empty_bosses,
            true, // 有底薪 → 条件 2 直接判定完成
            true,
            true,
            false,
            false, // income_matched = false
            false, // has_drops = false
        ));
    }
}
