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

/// JCL 文件名解析结果
#[derive(Debug, Clone)]
struct JclFileInfo {
    /// 文件名（不含路径）
    file_name: String,
    /// 时间戳（毫秒）
    timestamp: i64,
    /// JCL 副本名（如 "25人英雄阆风悬城"）
    raid_display_name: String,
    /// 副本 ID（仅用于调试/测试断言）
    #[allow(dead_code)]
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
/// - BOSS 击杀状态（基于 NPC_FIGHT_HINT 的 bFight True→False 转变 + 宝箱信号判定通关）
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
    /// 是否击杀 BOSS（true=通关，false=拉托）
    /// 判定依据：宝箱路径 或 NPC_FIGHT_HINT 的 bFight True→False 转变
    is_kill: bool,
}

/// 副本实例（聚类后的一场副本）
#[derive(Debug)]
struct RaidInstance {
    /// 账号 UID
    account_id: String,
    /// JCL 副本名（如 "25人英雄阆风悬城"）
    raid_display_name: String,
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
) -> Result<Option<DbRoleIdentity>, String> {
    let info_path = account_dir.join("info.jx3dat");
    let identity = match parse_info_jx3dat(&info_path) {
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

/// 将 JCL 文件列表聚类为副本实例
///
/// 聚类规则（基于 JCL 内容分析的方案）：
/// 1. 按时间戳升序排序所有 JCL
/// 2. 按 JCL 顺序遍历，连续相同 raid_display_name 的 JCL 归为同一组
///    （raid_display_name 变化即开始新组，不再使用30分钟间隔聚类）
/// 3. 对每组使用 JclAnalysis.is_kill 过滤拉托（只保留通关 JCL）
/// 4. 构建 RaidInstance（只含通关 JCL）
/// 副本会话时间间隔阈值（毫秒）
///
/// 当两个相邻 JCL 的时间间隔超过此阈值时，即使副本名相同也视为不同的副本会话。
/// 一次副本会话通常 30~90 分钟，相邻 JCL 间隔通常 < 30 分钟（含拉托重打）。
/// 2 小时阈值可稳定区分同副本的多次独立会话（含跨天、跨周场景）。
const RAID_SESSION_GAP_MS: i64 = 2 * 60 * 60 * 1000;

/// 检查指定时间范围内 chatlog 是否有"每人底薪"记录
/// 用于 2 小时阈值分割决策和中间工资检查
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

fn cluster_raid_instances(
    account_id: &str,
    jcl_files: Vec<JclFileInfo>,
    jcl_analyses: &HashMap<String, JclAnalysis>,
    chatlog_files: &[PathBuf],
    chatlog_range_cache: &HashMap<PathBuf, Option<(i64, i64)>>,
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
        // - 时间间隔 > 2h 但前组无击杀 → 不分割（避免对纯拉托组做无谓分割）
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
                let has_salary = has_salary_between(chatlog_files, gap_start, gap_end, chatlog_range_cache);
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
                // 只有前组有击杀时才做工资检查（避免对纯拉托组做无谓查询）
                let prev_has_kill = current_group.iter().any(|jcl| {
                    jcl_analyses.get(&jcl.file_name)
                        .map(|a| a.is_kill)
                        .unwrap_or(false)
                });
                if prev_has_kill && has_salary_between(chatlog_files, gap_start, gap_end, chatlog_range_cache) {
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
/// 通关判定优先使用 JCL 内部信号（宝箱/bFight=false/HP=0/分身离场等）。
/// 若某个连续相同 BOSS 的子组内无任何 JCL 内部击杀信号，则取该子组最后一个 JCL 作为击杀（组级别兜底）。
/// 若整组全为拉托且无兜底击杀，返回 None 跳过。
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
        // （一个 BOSS 子组内可能有多场战斗，只有最后一次击杀才是真正的通关）
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
        } else if !boss_group.is_empty() {
            // 无明确击杀信号，组级别兜底：最后一个 JCL 为击杀
            // 适用场景：部分 BOSS 击杀时不触发 bFight=false 也不产生宝箱（如墨家机侍→唐怀仁、须罗巨傀）
            let last_jcl = boss_group.last().unwrap();
            // 跳过纯小怪子组：JclAnalysis.boss_name=None 说明是路径 B 小怪 JCL
            let analysis_boss_name = jcl_analyses
                .get(&last_jcl.file_name)
                .and_then(|a| a.boss_name.as_deref());
            if analysis_boss_name.is_none() {
                log::debug!(
                    "[DropScanner] 子组最后一个 JCL 的 boss_name=None（纯小怪/出场对话），跳过兜底: {}",
                    last_jcl.file_name
                );
                continue;
            }
            // 仅当文件名 BOSS 与内容 BOSS 不同时才兜底（说明是小怪 JCL，真实 BOSS 从内容中识别）
            // 文件名 BOSS 与内容 BOSS 相同且 is_kill=false → 真正的灭团，不兜底
            let file_boss = &last_jcl.boss_name;
            let analysis_boss = analysis_boss_name.unwrap_or("");
            if file_boss == analysis_boss {
                log::debug!(
                    "[DropScanner] BOSS {} is_kill=false 且文件名BOSS与内容BOSS一致，判定为灭团不兜底: {}",
                    file_boss,
                    last_jcl.file_name
                );
                continue;
            }
            let boss_name = get_effective_boss_name(last_jcl, jcl_analyses);
            log::info!(
                "[DropScanner] BOSS {} (文件名:{}) 无JCL内部击杀信号，组级别兜底取最后一个JCL为击杀: {}",
                boss_name,
                file_boss,
                last_jcl.file_name
            );
            success_jcls.push(*last_jcl);
        }
    }

    if success_jcls.is_empty() {
        // 整组都是拉托，跳过
        log::debug!(
            "[DropScanner] 副本 {} 的一个组全为拉托，跳过（共{}个JCL）",
            group.first().map(|j| j.raid_display_name.as_str()).unwrap_or(""),
            group.len()
        );
        return None;
    }

    let raid_display_name = success_jcls[0].raid_display_name.clone();
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

/// 匹配 JCL 副本名到 raids.name
///
/// JCL 副本名格式：{人数}人{难度}{副本名} 或 {难度}{副本名} 或 {副本名}
/// 匹配策略：先精确匹配 raids.id，再后缀匹配 raids.id
fn match_raid_name<'a>(jcl_raid_name: &str, raids: &'a [RaidEntry]) -> Option<&'a RaidEntry> {
    // 精确匹配 raids.id
    // raids.id 格式：{playerCount}人{difficulty}{name}，如 "25人普通阆风悬城"
    // JCL 副本名也是类似格式，如 "25人普通阆风悬城"
    for entry in raids {
        if entry.raid_id == jcl_raid_name {
            return Some(entry);
        }
    }

    // 后缀匹配：raids.id 是 JCL 副本名的后缀（或反过来）
    for entry in raids {
        if jcl_raid_name.ends_with(&entry.raid_id) || entry.raid_id.ends_with(jcl_raid_name) {
            return Some(entry);
        }
    }

    // 最后尝试用 raids.name 后缀匹配
    // 例如 JCL="25人英雄阆风悬城"，raids.name="阆风悬城"
    for entry in raids {
        if jcl_raid_name.ends_with(&entry.name) && !entry.name.is_empty() {
            return Some(entry);
        }
    }

    None
}

/// 判断 JCL BOSS 名是否为有效 BOSS（精确/模糊匹配 raid_bosses 表）
fn is_valid_boss(boss_name: &str, raid_bosses: &[(String, String)]) -> bool {
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

/// 将 BOSS 名列表映射为 BOSS ID 列表
fn map_boss_names_to_ids(
    bosses_killed: &[String],
    raid_bosses: &[(String, String)],
) -> (Vec<String>, Vec<String>) {
    let mut boss_ids: Vec<String> = Vec::new();
    let mut boss_names: Vec<String> = Vec::new();
    // 去重集合：同一 boss_id 只保留首次出现，避免同一 BOSS 多次拉托+击杀导致重复计数
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

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
///   - 跟踪 BOSS 的 bFight 状态变化
///   - **通关判定**：bFight 从 true 变为 false（BOSS 退出战斗=被击杀）
///   - 拉托时 BOSS 只有 bFight=true（无 false），且会有 NPC_LEAVE_SCENE 事件
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
/// 从 SQLite 缓存读取 JCL 解析结果（跨会话复用）
/// 缓存键：file_path + file_mtime，文件未修改时直接返回缓存
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
    let path_str = jcl_path.to_string_lossy().to_string();
    let file_mtime = get_file_mtime(jcl_path);

    // 1. 查缓存
    if file_mtime > 0 {
        if let Some(cached) = get_jcl_cache(conn, &path_str, file_mtime) {
            return Some(cached);
        }
    }

    // 2. 缓存未命中，解析文件
    let analysis = analyze_jcl(jcl_path, jcl_boss_name, boss_template_id, raid_bosses)?;

    // 3. 写回缓存
    if file_mtime > 0 {
        set_jcl_cache(conn, &path_str, file_mtime, &analysis);
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
    let mut boss_fight_true_seen = false;
    let mut boss_fight_false_seen = false;
    let mut boss_hp_zero_seen = false;
    // 注意：主 BOSS LEAVE_SCENE 不可靠（拉托时也会离场），路径 A 不使用。
    // 仅路径 B（小怪 JCL）使用，因为路径 B 的真实 BOSS 击杀时可能无 bFight=false。
    let mut boss_leave_scene_seen = false;
    // 已发现的 BOSS 专属宝箱 NPC（"BOSS名+宝箱"），用于路径 A/B 判定
    let mut treasure_box_bosses: std::collections::HashSet<String> = std::collections::HashSet::new();

    // 真实 BOSS（仅当文件名 BOSS 不在配置中时使用）
    // 例：唐怀仁战斗的 JCL 文件名是"须罗巨傀"（小怪），需从 NPC_INFO 中找"唐怀仁"
    // 通过"小怪路径"判定通关：
    // - 双 BOSS 死亡（须罗巨傀 + 唐怀仁 都死）
    // - 或"唐怀仁宝箱"NPC 出现
    let mut real_boss_dwid: i64 = 0;
    let mut real_boss_name: Option<String> = None;
    let mut real_boss_fight_true_seen = false;
    let mut real_boss_fight_false_seen = false;
    // 路径 B 使用：真实 BOSS（如唐怀仁）击杀时可能无 bFight=false，需 leave_scene 兜底
    let mut real_boss_leave_scene_seen = false;
    let mut real_boss_hp_zero_seen = false;

    // 路径 B 使用：追踪真实 BOSS 死亡/重生状态
    // NPC_INFO 最后一个字段为状态码（0=存活, 192=死亡）。
    // 团灭时 BOSS 会重生（state 从 192 回到 0），击杀时 BOSS 保持死亡或直接离场。
    // 实测：7月7日团灭JCL中唐怀仁 state 192→0（重生），7月3日/7月7日击杀均无重生。
    let mut real_boss_entered_dead_state = false;
    let mut real_boss_respawned = false;

    // 同名 NPC 追踪（BOSS 分身/幻影）
    // 某些 BOSS（如"笑妆娘"、"柳公子"）有多个 templateId 的同名 NPC（分身/幻影），
    // 击杀时主 BOSS 可能不触发 bFight=false，但分身/幻影会 LEAVE_SCENE。
    // 注意：团灭时分身也会集中离场，因此不能仅凭"有分身离场"判定击杀。
    // 可靠区分信号：分身离场时间跨度（clone_leave_span）。
    //   - 击杀：分身在整个战斗过程中陆续离场，跨度大（实测 229~374 秒）
    //   - 团灭：分身在战斗结束时集中离场，跨度小（实测 56 秒）
    //   阈值 120 秒可完美区分两类情况。
    let mut same_name_dwids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut same_name_leave_scene_count: u32 = 0;
    // 分身（非主 BOSS 的同名 NPC）离场时间戳，用于计算离场时间跨度
    let mut first_clone_leave_ms: i64 = 0;
    let mut last_clone_leave_ms: i64 = 0;

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

        // 解析事件类型，处理相关类型：
        // 1=FIGHT_TIME, 5=实体状态（含HP=0），7=NPC_LEAVE_SCENE, 8=NPC_INFO, 9=NPC_FIGHT_HINT
        let event_type: u8 = match std::str::from_utf8(parts[4])
            .ok()
            .and_then(|s| s.parse().ok())
        {
            Some(t) => t,
            None => continue,
        };
        if event_type != 1
            && event_type != 5
            && event_type != 7
            && event_type != 8
            && event_type != 9
        {
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

                // 用途 2：检测"BOSS名+宝箱"NPC（**精确匹配配置中的 BOSS**）
                // 例如：NPC 名为 "笑妆娘宝箱" 时，提取前缀 "笑妆娘"，
                //       若在 raid_bosses 配置中则记为对应 BOSS 的宝箱已出现
                let name_trimmed = name.trim_matches('"');
                if name_trimmed.ends_with("宝箱") {
                    let prefix_len = name_trimmed.len() - "宝箱".len();
                    let prefix = &name_trimmed[..prefix_len];
                    // 检查前缀是否正好是配置中的某个 BOSS 名
                    for (_boss_id, boss_name) in raid_bosses {
                        if prefix == *boss_name {
                            treasure_box_bosses.insert(boss_name.clone());
                            break;
                        }
                    }
                }

                // 用途 4：当文件名 BOSS 在配置中时，追踪所有同名 NPC 的 dwID（多阶段 BOSS）
                // 某些 BOSS（如"唐醉"）有多个 templateId 的同名 NPC（分身/幻影），
                // 主 BOSS 被击杀后可能不触发 LEAVE_SCENE，但分身/幻影会离场。
                if file_boss_in_config && name_trimmed == jcl_boss_name {
                    same_name_dwids.insert(dwid);
                }

                // 用途 5：追踪真实 BOSS 死亡/重生状态（仅路径 B）
                // NPC_INFO 格式: { dwID, name, templateID, employer, nX, nY, nZ, state }
                // state=0 表示存活，state=192 表示死亡。
                // 团灭时 BOSS 会重生（state 从 192 回到 0），击杀时不会重生。
                // 注意：文件 BOSS（小怪）在战斗中会反复死亡/重生，不能用作团灭信号，
                //       仅追踪真实 BOSS（如唐怀仁）的重生。
                if !file_boss_in_config
                    && fields.len() >= 8
                    && real_boss_dwid != 0
                    && dwid == real_boss_dwid
                {
                    let state: i64 = fields[7].trim().parse().unwrap_or(-1);
                    if state == 192 {
                        real_boss_entered_dead_state = true;
                    } else if state == 0 && real_boss_entered_dead_state {
                        real_boss_respawned = true;
                    }
                }
            }
            9 => {
                // NPC_FIGHT_HINT: { dwID, bFight, fCurrentLife, fMaxLife, ... }
                // 跟踪 BOSS（文件名 BOSS + 真实 BOSS）的战斗开始/结束信号
                if fields.len() < 2 {
                    continue;
                }
                let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                let bfight = fields[1].trim() == "true";

                // 文件名 BOSS（小怪路径下是小怪 dwID）的 bFight 状态
                if boss_dwid != 0 && dwid == boss_dwid {
                    if bfight {
                        boss_fight_true_seen = true;
                    } else {
                        boss_fight_false_seen = true;
                    }
                }
                // 真实 BOSS（小怪路径下追踪唐怀仁等配置 BOSS）的 bFight 状态
                if !file_boss_in_config && real_boss_dwid != 0 && dwid == real_boss_dwid {
                    if bfight {
                        real_boss_fight_true_seen = true;
                    } else {
                        real_boss_fight_false_seen = true;
                    }
                }
            }
            7 => {
                // NPC_LEAVE_SCENE: { dwID }
                // 主 BOSS LEAVE_SCENE 不可靠（拉托时也会离场），路径 A 不使用。
                // 路径 B（小怪 JCL）使用：真实 BOSS 击杀时可能无 bFight=false，需 leave_scene 兜底。
                // 同名 NPC 分身离场：需结合离场时间跨度区分击杀/团灭（见下方 clone_leave_signal）。
                if fields.is_empty() {
                    continue;
                }
                let dwid: i64 = fields[0].trim().parse().unwrap_or(0);

                // 文件名 BOSS 离开场景（仅路径 B 使用）
                if boss_dwid != 0 && dwid == boss_dwid {
                    boss_leave_scene_seen = true;
                }
                // 真实 BOSS 离开场景（仅路径 B 使用）
                if !file_boss_in_config && real_boss_dwid != 0 && dwid == real_boss_dwid {
                    real_boss_leave_scene_seen = true;
                }
                // 同名 NPC 离开场景（BOSS 分身/幻影离场）
                // 追踪分身（非主 BOSS）的离场时间戳，用于计算时间跨度区分击杀/团灭。
                if file_boss_in_config && same_name_dwids.contains(&dwid) {
                    same_name_leave_scene_count += 1;
                    if dwid != boss_dwid {
                        let ts_ms = timestamp_sec * 1000;
                        if first_clone_leave_ms == 0 {
                            first_clone_leave_ms = ts_ms;
                        }
                        last_clone_leave_ms = ts_ms;
                    }
                }
            }
            5 => {
                // 实体状态: { dwID, alive_flag, hp_current, hp_max, ?, ? }
                // 例如死亡时 hp_current=0, hp_max=0, alive_flag=false
                // 用于 HP=0 击杀兜底信号
                if fields.len() < 4 {
                    continue;
                }
                let dwid: i64 = fields[0].trim().parse().unwrap_or(0);
                let flag = fields[1].trim();
                let hp: i64 = fields[2].trim().parse().unwrap_or(-1);
                let hp_max: i64 = fields[3].trim().parse().unwrap_or(-1);

                if flag == "false" && hp == 0 && hp_max == 0 {
                    if boss_dwid != 0 && dwid == boss_dwid {
                        boss_hp_zero_seen = true;
                    }
                    if !file_boss_in_config && real_boss_dwid != 0 && dwid == real_boss_dwid {
                        real_boss_hp_zero_seen = true;
                    }
                }
            }
            _ => {}
        }
    }

    // 通关判定逻辑（3 类分类：击杀 / 拉托 / 小怪）
    //
    // **死亡信号的可靠性**（实测验证）：
    // - NPC_LEAVE_SCENE（主 BOSS 离开场景）**不能**区分击杀与拉托！
    //   实测发现拉托时 BOSS 也会 LEAVE_SCENE（墨家机侍拉托时有 1 条 LEAVE_SCENE）。
    // - NPC_FIGHT_HINT bFight true→false 是可靠死亡信号：
    //   击杀时 BOSS 会触发 bFight=false（唐醉、阿史那承庆均有此信号），
    //   拉托时 BOSS 不会触发 bFight=false（墨家机侍拉托时无此信号）。
    // - 同名 NPC 分身 LEAVE_SCENE **不能**单独区分击杀与拉托！
    //   某些 BOSS（笑妆娘、柳公子）击杀时主 BOSS 不触发 bFight=false，
    //   其同名分身/幻影会 LEAVE_SCENE；但团灭时分身也会集中离场。
    //   可靠区分信号：分身离场时间跨度（clone_leave_span）。
    //     - 击杀：分身陆续离场，跨度 >= 120 秒（实测 229~374 秒）
    //     - 团灭：分身集中离场，跨度 < 120 秒（实测 56 秒）
    // - 实体状态 HP=0 是可靠死亡信号：BOSS 血量为 0 时才会触发（本批数据未出现）。
    //
    // 路径 A：文件名 BOSS 在 raid_bosses 配置中（普通 BOSS 如"唐醉"、"笑妆娘"）
    //   1) 宝箱路径（首选）：该 BOSS 名+"宝箱" NPC 出现
    //   2) 死亡路径：必须同时满足
    //      - boss_dwid 已追踪到（!= 0）
    //      - boss_fight_true_seen（确认 BOSS 真的进入过战斗）
    //      - **boss_fight_false_seen 或 boss_hp_zero_seen 或 clone_leave_signal**
    //        （bFight=false / HP 归零 / 分身长时间跨度离场，任一即可确认死亡）
    //   注意：路径 A **不使用** boss_leave_scene_seen，因为拉托时主 BOSS 也会离场。
    //
    // 路径 B：文件名 BOSS 不在配置中（小怪 JCL 如"须罗巨傀"对应唐怀仁战斗）
    //   Q1: JCL 中能找到配置 BOSS 吗？
    //     - 否 → 识别为"小怪"（仅小怪战斗，无副本 BOSS）
    //     - 是 → 进入 Q2
    //   Q2: 该配置 BOSS 是否真正进入战斗（bFight=true）？
    //     - 否 → 识别为"小怪"（配置 BOSS 路过没打/剧情短暂出场）
    //     - 是 → 进入击杀/拉托判定
    //   击杀条件（任一即可）：
    //     ① 配置 BOSS 宝箱出现
    //     ② 双 BOSS 都死亡：文件 BOSS（小怪）+ 真实 BOSS（配置 BOSS）
    //        死亡路径要求 bFight=true + (bFight=false 或 leave_scene 或 hp_zero)
    //   团灭排除：真实 BOSS 重生（NPC_INFO state 192→0）时判定为团灭，不算击杀。
    //   注意：路径 B **使用** leave_scene 兜底，因为真实 BOSS（如唐怀仁）击杀时
    //   可能无 bFight=false。团灭时 BOSS 也会 leave_scene，但会同时触发重生信号，
    //   通过 real_boss_respawned 排除团灭情况。
    let is_kill = if file_boss_in_config {
        // 路径 A
        let treasure_ok = treasure_box_bosses.contains(jcl_boss_name);
        // 分身离场时间跨度信号：分身陆续离场（跨度>=120s）判定为击杀，
        // 集中离场（跨度<120s）判定为团灭。仅当有分身离场时才计算。
        let clone_leave_span_sec = if first_clone_leave_ms > 0 {
            (last_clone_leave_ms - first_clone_leave_ms) / 1000
        } else {
            0
        };
        let clone_leave_signal =
            same_name_leave_scene_count > 0 && clone_leave_span_sec >= 120;
        // bFight=false 是击杀信号，但若同时有 LEAVE_SCENE 则是拉托脱战（BOSS脱战后离场回原位）。
        // 实测：16-34-59 阿史那承庆 bFight=false(HP=91.7%) + LEAVE_SCENE = 拉托；
        //       16-43-09 阿史那承庆 bFight=false(HP满) + 无LEAVE = 击杀。
        let bfight_false_is_kill = boss_fight_false_seen && !boss_leave_scene_seen;
        let death_ok = boss_dwid != 0
            && boss_fight_true_seen
            && (bfight_false_is_kill || boss_hp_zero_seen || clone_leave_signal);
        treasure_ok || death_ok
    } else {
        // 路径 B
        if real_boss_dwid == 0 || !real_boss_fight_true_seen {
            // 配置 BOSS 不存在或未进入战斗 → 小怪
            false
        } else {
            let real_treasure_ok = real_boss_name
                .as_ref()
                .map(|n| treasure_box_bosses.contains(n))
                .unwrap_or(false);
            // 死亡路径：bFight=true + (bFight=false 或 leave_scene 或 hp_zero)
            // 路径 B 的真实 BOSS（如唐怀仁）击杀时可能无 bFight=false，需 leave_scene 兜底。
            // 团灭排除：真实 BOSS 重生（state 192→0）说明团灭后 BOSS 复位，不算击杀。
            // 实测：7月7日团灭JCL中唐怀仁 state 192→0（重生），7月3日/7月7日击杀均无重生。
            let file_boss_died = boss_dwid != 0
                && boss_fight_true_seen
                && (boss_fight_false_seen || boss_leave_scene_seen || boss_hp_zero_seen);
            let real_boss_died = real_boss_dwid != 0
                && real_boss_fight_true_seen
                && !real_boss_respawned
                && (real_boss_fight_false_seen
                    || real_boss_leave_scene_seen
                    || real_boss_hp_zero_seen);
            real_treasure_ok || (file_boss_died && real_boss_died)
        }
    };

    // 确定返回的 boss_name：
    // - 路径 A（文件名 BOSS 在配置中）：直接用文件名 BOSS 名
    // - 路径 B（文件名 BOSS 不在配置中，小怪 JCL）：
    //   * 仅当真实 BOSS 真正进入战斗（real_boss_fight_true_seen=true）时才返回 real_boss_name
    //   * 否则返回 None（小怪 JCL，effective boss name 回退到文件名）
    //   注意：不以对话/NPC_INFO 文本作为依据，避免出场对话 JCL 被误判为该 BOSS 的战斗。
    let final_boss_name = if file_boss_in_config {
        Some(jcl_boss_name.to_string())
    } else if real_boss_fight_true_seen {
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
///
/// 返回 (drops, base_salary, other_income_gold, expense_gold, boss_kill_count, income_records, boss_kill_times)。
/// income_records 为非 BOSS 10金的收入记录列表 (time_sec, gold)，用于精确收入匹配。
/// boss_kill_times 为每个 BOSS 击杀（10金收入）的 time_sec 列表，用于击杀数验证。
/// 调用方根据 base_salary 是否存在决定最终收入：
///   - 有底薪 → 在 income_records 中找 gold >= base_salary 的第一条记录，用该 gold 作为收入
///   - 无底薪 → 在 income_records 中找最后一个 BOSS 后最近的一条记录，用该 gold 作为收入
fn extract_drops_from_chatlog(
    chatlog_path: &PathBuf,
    start_time_ms: i64,
    end_time_ms: i64,
    role_name: &str,
) -> Result<(Vec<String>, Option<i64>, i64, i64, u32, Vec<(i64, i64)>, Vec<i64>), String> {
    if !chatlog_path.exists() {
        return Ok((Vec::new(), None, 0, 0, 0, Vec::new(), Vec::new()));
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
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| format!("执行 chatlog 查询失败: {}", e))?;

    // 物品分配消息：`分配者将：[物品名]分配给角色名·服务器。`
    // 拍团底薪消息：`每人底薪：XXX金`
    // 支出消息（直接购买）：`[角色名·服务器]花费[金额]购买了[物品名]`
    // 支出消息（团长分配）：`[分配者]将[物品名]以[金额]记录给了[角色名·服务器]`
    // 拍卖物品名提取：`购买了[物品名]`（包括帮别人购买的情况）

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

    // 去重：记录已通过"花费购买了"格式检测到的支出 (物品名, 金额, 时间窗口)
    // 用于避免同一笔交易在 MSG_ROOM 和 MSG_WHISPER 两个频道中重复计算
    // 时间窗口：30秒内的相同物品+金额视为同一笔交易
    let mut expense_dedup: HashSet<(String, i64, i64)> = HashSet::new();

    for row in rows {
        let (msg_type, text, _msg, time_sec) = match row {
            Ok(t) => t,
            Err(e) => {
                log::warn!("[DropScanner] 读取 chatlog 行失败: {}", e);
                continue;
            }
        };

        if msg_type == "MSG_ITEM" {
            // 提取分配的物品名
            if let Some(caps) = ITEM_RE.captures(&text) {
                let item_name = caps[1].to_string();
                drops_set.insert(item_name);
                item_count += 1;
            }
        } else if msg_type == "MSG_MONEY" {
            // SQL 已过滤 text LIKE '你获得：%'，此处直接解析金额
            // 解析 msg 字段获取真实的金/银/铜值
            if let Some((gold, silver, copper)) = parse_money_from_msg(&_msg) {
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
            if let Some(caps) = SALARY_RE.captures(&text) {
                let salary: i64 = caps[1].parse().unwrap_or(0);
                base_salary = Some(salary);
            }
            // 支出消息（直接购买格式）：[角色名]花费[金额]购买了[物品名]
            // 这是唯一确认的支出格式，"记录给了"只是团长分配记录不代表实际购买
            if let Some(caps) = EXPENSE_MSG_RE.captures(&text) {
                let buyer_name = normalize_role_name(&caps[1]);
                // 匹配角色名（buyer_name 格式为"角色名·服务器"，检查前缀）
                if buyer_name == target_name
                    || buyer_name.starts_with(&format!("{}·", target_name))
                {
                    let amount = parse_expense_amount(&caps[2]);
                    let item_name = caps[3].to_string();
                    // 去重：同一笔交易可能在 MSG_ROOM 和 MSG_WHISPER 两个频道都出现
                    let time_bucket = time_sec / 30;
                    let dedup_key = (item_name.clone(), amount, time_bucket);
                    if !expense_dedup.contains(&dedup_key) {
                        // 还需检查 ±1 个时间窗口（两条频道消息可能有几秒偏差）
                        let prev_bucket = (item_name.clone(), amount, time_bucket - 1);
                        let next_bucket = (item_name.clone(), amount, time_bucket + 1);
                        if !expense_dedup.contains(&prev_bucket) && !expense_dedup.contains(&next_bucket) {
                            expense_gold += amount;
                            expense_dedup.insert(dedup_key);
                        }
                    }
                }
            }
            // 团长分配记录：[分配者]将[物品名]以[金额]记录给了[接收者]
            // 仅提取物品名加入 drops（掉落物记录），不计入支出（分配不等于实际购买）
            if let Some(caps) = ALLOCATE_TO_RE.captures(&text) {
                let item_name = caps[2].to_string();
                drops_set.insert(item_name);
            }
            // 拍卖物品名提取：所有"购买了[物品名]"的物品都加入 drops
            // （包括帮别人购买的情况，用于检测玄晶等稀有掉落）
            if let Some(caps) = PURCHASED_ITEM_RE.captures(&text) {
                let item_name = caps[1].to_string();
                drops_set.insert(item_name);
            }
        }
    }

    // HashSet 转 Vec
    let drops: Vec<String> = drops_set.into_iter().collect();

    // 铜转金（整数，截断银铜部分）
    let other_income_gold = other_income_copper / 10000;

    log::info!(
        "[DropScanner] chatlog 提取完成: {} 个物品分配, {} 条金币获得, 底薪={:?}, 其他收入 {} 金, 支出 {} 金, BOSS击杀奖励 {} 次",
        item_count,
        money_count,
        base_salary,
        other_income_gold,
        expense_gold,
        boss_kill_count
    );

    Ok((drops, base_salary, other_income_gold, expense_gold, boss_kill_count, income_records, boss_kill_times))
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
/// 3. JX3 进程退出：jx3_running = false
/// 4. 角色离线：role_online = false（chatlog mtime > 5 分钟未更新）
///
/// 由于通关判定改为基于 JCL 数据（NPC_FIGHT_HINT 的 bFight True→False），
/// 不再依赖 chatlog 10金，所有条件均可可靠检查。
fn is_raid_complete(
    instance: &RaidInstance,
    raid_bosses: &[(String, String)],
    has_salary: bool,
    jx3_running: bool,
    role_online: bool,
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
    // 条件 3：JX3 进程退出
    if !jx3_running {
        return true;
    }
    // 条件 4：角色离线
    if !role_online {
        return true;
    }
    false
}

/// 构造 RaidRecord JSON 并写入 records 表
fn upsert_raid_drop_record(
    conn: &Connection,
    instance: &RaidInstance,
    drops: &[String],
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

    // 检查 records 表是否已存在同账号、同副本、同 CD 周期的 auto_scan 记录（任意状态）
    // 注意：raid_name 列存储的是 raids.name（如 "阆风悬城"），不是 JCL 显示名。
    // 使用 CD 窗口范围匹配：同一 CD 周期内同一副本只保留一条记录。
    // 查询所有状态（含 confirmed/rejected），避免已确认/已拒绝的记录被重复创建。
    // 优先返回 pending/scanning（用于更新），其次 confirmed/rejected（用于跳过）。
    let is_ten_person = instance.raid_display_name.contains("10人");
    let (window_start, window_end) = calculate_cd_window(instance.start_time, is_ten_person);

    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT id, status FROM records
             WHERE account_id = ?1 AND raid_name = ?2
               AND record_date >= ?3 AND record_date < ?4
               AND source = 'auto_scan'
             ORDER BY
               CASE status
                 WHEN 'scanning' THEN 0
                 WHEN 'pending' THEN 1
                 ELSE 2
               END
             LIMIT 1",
            params![instance.account_id, raid_name, window_start, window_end],
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

    // 若不存在同源 auto_scan 记录，进一步检查同 CD 窗口内是否已有手工记录。
    // 已有手工记录时跳过 pending 创建，避免重复占用 CD。
    //
    // 注意：手工记录的 raid_name 列存的是 JSON raidName 完整名（如 "25人英雄阆风悬城"），
    // 而 auto 记录的 raid_name 列存的是 raids.name 短名（如 "阆风悬城"）。
    // 因此不能用 raid_name 列精确匹配，需查询同账号同 CD 窗口内所有 confirmed 记录，
    // 在 Rust 中解析 JSON data 字段的 raidName 比较。
    if existing.is_none() {
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
                   AND status = 'confirmed'",
            )
            .map_err(|e| format!("准备手工记录查询失败: {}", e))?;

        let rows = stmt
            .query_map(params![instance.account_id, window_start, window_end], |row| {
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
                    manual_raid_name == instance.raid_display_name
                );
                if manual_raid_name == instance.raid_display_name {
                    matched = true;
                }
            }
        }

        if matched {
            log::info!(
                "[DropScanner] CD 窗口内已存在手工记录，跳过 pending: {} (window: {}~{}, 共 {} 条手工记录)",
                instance.raid_display_name,
                window_start,
                window_end,
                manual_count
            );
            return Ok(());
        }

        log::info!(
            "[DropScanner] CD 检查: 未找到匹配手工记录（共 {} 条），将创建 pending",
            manual_count
        );
    }

    // 使用 existing_id（更新）或新生成的 UUID（插入）作为记录 ID。
    let record_id = existing.clone().map(|(id, _)| id).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // 掉落物中包含"玄晶"时自动标记（与前端 PendingRecordsPanel 编辑弹窗逻辑一致）
    let has_xuanjing = drops.iter().any(|d| d.contains("玄晶"));

    // 构造 RaidRecord JSON
    // goldIncome 使用 chatlog 提取的分配金额作为默认值，用户确认时可在 UI 编辑。
    let record = serde_json::json!({
        "id": record_id,
        "accountId": instance.account_id,
        "roleId": role_id,
        "roleName": role_name,
        "server": format!("{} {}", role_region, role_server),
        "raidName": raid_full_name,
        "date": instance.start_time,
        "goldIncome": gold_income,
        "goldExpense": gold_expense,
        "hasXuanjing": has_xuanjing,
        "bossIds": boss_ids,
        "bossNames": boss_names,
        "type": "raid",
        "source": "auto",
        "status": record_status,
        "drops": drops,
        "jclFiles": filtered_jcl_files,
        "notes": format!("自动扫描记录 - {}", now),
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
/// 3. 聚类为副本实例（带10金匹配，过滤拉托）
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
    scan_raid_drops_with_raids(account_id, jx3_running, role_online, process_start_ms, start_ms, end_ms, None)
}

/// scan_raid_drops_internal 的扩展版本，支持传入预加载的副本配置
/// `pre_loaded_raids`: 若 Some，则跳过 init_db + get_cached_raids，直接使用传入的配置
pub fn scan_raid_drops_with_raids(
    account_id: &str,
    jx3_running: bool,
    role_online: bool,
    process_start_ms: i64,
    start_ms: i64,
    end_ms: i64,
    pre_loaded_raids: Option<&[RaidEntry]>,
) -> Result<usize, String> {
    log::info!(
        "[DropScanner] 开始扫描账号 {} 的掉落记录 (jx3_running={}, role_online={}, process_start_ms={}, start_ms={}, end_ms={})",
        account_id, jx3_running, role_online, process_start_ms, start_ms, end_ms
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
        .into_iter()
        .filter(|jcl| {
            if match_raid_name(&jcl.raid_display_name, &raids).is_some() {
                true
            } else {
                skipped_unconfigured.push(jcl.raid_display_name.clone());
                false
            }
        })
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

    // 3. 分析每个 JCL 文件内容，提取战斗时间和击杀状态
    //    替代原 10金查询方案：通过 JCL 的 NPC_FIGHT_HINT (bFight True→False) 判定通关
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
        jcl_analyses.insert(jcl.file_name.clone(), analysis);
    }

    // 4. 聚类（按 JCL 顺序，使用 is_kill 过滤拉托，2小时阈值智能分组）
    let mut instances = cluster_raid_instances(
        account_id,
        configured_jcl_files,
        &jcl_analyses,
        &chatlog_files,
        &chatlog_range_cache,
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

    let db_identity = match resolve_db_role_identity(&conn, &account_dir) {
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
    // 预提取各实例的 start_time，避免 iter_mut 借用冲突
    let instance_start_times: Vec<i64> = instances.iter().map(|i| i.start_time).collect();

    for (index, instance) in instances.iter_mut().enumerate() {
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
        // 结束 = 下一副本实例的 start_time 或当前扫描时间
        // （底薪通常在副本结束后30-60分钟内发送，用当前时间可确保覆盖）
        let chatlog_start = instance.first_gold_time;
        let chatlog_end = if index + 1 < instance_start_times.len() {
            instance_start_times[index + 1]
        } else {
            // 最后一个副本实例：以当前扫描时间为结束时间
            // （底薪通常在副本结束后30-60分钟内发送，用当前时间可确保覆盖）
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(instance.last_jcl_time + 2 * 60 * 60 * 1000)
        };

        // 从 chatlog 提取掉落物、金币收入和支出
        // 用文件名日期预过滤，只打开覆盖副本时间范围的 chatlog 文件
        let mut drops: HashSet<String> = HashSet::new();
        let mut base_salary: Option<i64> = None;
        let mut other_income: i64 = 0;
        let mut total_expense: i64 = 0;
        let mut chatlog_query_count = 0u32;
        // 合并所有 chatlog 文件的收入记录 (time_sec, gold)，用于精确收入匹配
        let mut all_income_records: Vec<(i64, i64)> = Vec::new();
        // 累加所有 chatlog 文件的 boss_kill_times，用于击杀数验证
        let mut all_boss_kill_times: Vec<i64> = Vec::new();
        for chatlog_path in &chatlog_files {
            // 文件名日期预过滤：跳过不覆盖 [chatlog_start, chatlog_end] 的文件
            if !chatlog_file_covers_range_cached(chatlog_path, chatlog_start, chatlog_end, &chatlog_range_cache) {
                continue;
            }
            chatlog_query_count += 1;
            if let Ok((chatlog_drops, salary, income, expense, _, income_records, boss_kill_times)) =
                extract_drops_from_chatlog(chatlog_path, chatlog_start, chatlog_end, &db_identity.role_name)
            {
                drops.extend(chatlog_drops);
                // 底薪取最后一个非 None 值（最终分配金额，跨文件时后文件优先）
                if salary.is_some() {
                    base_salary = salary;
                }
                other_income += income;
                total_expense += expense;
                all_income_records.extend(income_records);
                all_boss_kill_times.extend(boss_kill_times);
            }
        }
        log::info!(
            "[DropScanner] chatlog 文件预过滤: 总 {} 个, 实际查询 {} 个",
            chatlog_files.len(),
            chatlog_query_count
        );

        // Boss 击杀数验证：以 chatlog 10金次数校验 JCL 判定的击杀数。
        // 但 chatlog 10金校验仅在 chatlog 确认落盘后（游戏退出或角色离线）执行，
        // 因为茗伊插件在游戏运行期间不实时刷盘，chatlog 数据可能不完整。
        // 判定信号：jx3_running=false 或 role_online=false 表示 chatlog 已落盘。
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

        // chatlog 是否已落盘：游戏退出或角色离线时，chatlog 不再被实时写入，视为已落盘
        let chatlog_flushed = !jx3_running || !role_online;

        if boss_all_killed {
            log::info!(
                "[DropScanner] 副本 {} BOSS已全清({}/{}), 跳过 chatlog 10金校验",
                instance.raid_display_name,
                instance.boss_kill_count,
                raid_bosses.len()
            );
        } else if !chatlog_flushed {
            // 游戏运行中且角色在线：chatlog 可能未完全落盘，跳过校验，保留 JCL 判定
            log::info!(
                "[DropScanner] 副本 {} BOSS未全清({}/{}), chatlog 未落盘(jx3_running={}, role_online={}), 跳过10金校验保留JCL判定",
                instance.raid_display_name,
                instance.boss_kill_count,
                raid_bosses.len(),
                jx3_running,
                role_online
            );
        } else if chatlog_query_count > 0 {
            // chatlog 已落盘：执行原有 10金校验逻辑
            let chatlog_boss_kills = all_boss_kill_times.len() as u32;
            if chatlog_boss_kills < instance.boss_kill_count {
                log::warn!(
                    "[DropScanner] 副本 {} BOSS击杀数校验: JCL判定={}, chatlog 10金={}, 以 chatlog 为准",
                    instance.raid_display_name, instance.boss_kill_count, chatlog_boss_kills
                );
                // 从后往前裁剪 bosses_killed，保留 chatlog_boss_kills 个
                while instance.bosses_killed.len() > chatlog_boss_kills as usize {
                    instance.bosses_killed.pop();
                }
                instance.boss_kill_count = chatlog_boss_kills;
            }
        } else {
            // chatlog 已落盘但无数据（chatlog_query_count == 0），保留 JCL 判定
            log::info!(
                "[DropScanner] 副本 {} chatlog 已落盘但无10金记录, 保留JCL判定",
                instance.raid_display_name
            );
        }

        // 最终收入计算：
        // - 有底薪：在收入记录中找金额 >= 底薪值的第一条记录，用该金额作为收入
        //   （团长发放底薪时的"你获得：XXX金"记录，金额通常等于或略大于底薪）
        // - 无底薪：使用收入记录总和作为收入
        //   （无底薪参考时，单条记录可能是小额系统消息，总和相关收入更可靠）
        // - 找不到匹配记录时回退：有底薪回退到底薪值，无底薪使用收入总和
        all_income_records.sort_by_key(|(t, _)| *t);
        let last_jcl_sec = instance.last_jcl_time / 1000;
        let total_gold = if let Some(salary) = base_salary {
            // 有底薪：找金额 >= 底薪的第一条收入记录
            match all_income_records.iter().find(|(_, g)| *g >= salary) {
                Some((t, g)) => {
                    log::info!(
                        "[DropScanner] 收入匹配(底薪): 底薪={}, 匹配记录 gold={} time={} -> 收入={}",
                        salary, g, t, g
                    );
                    *g
                }
                None => {
                    log::info!(
                        "[DropScanner] 收入匹配(底薪): 底薪={}, 无匹配记录, 回退到底薪值 -> 收入={}",
                        salary, salary
                    );
                    salary
                }
            }
        } else {
            // 无底薪：使用收入记录总和
            log::info!(
                "[DropScanner] 收入匹配(无底薪): last_jcl_sec={}, 使用收入总和 {} (共 {} 条记录) -> 收入={}",
                last_jcl_sec, other_income, all_income_records.len(), other_income
            );
            other_income
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
        );
        let record_status = if raid_complete { "pending" } else { "scanning" };

        log::info!(
            "[DropScanner] 副本完成判断: raid='{}', boss_kill={}/{}, boss_all_killed={}, has_salary={}, jx3_running={}, role_online={} -> status='{}'",
            instance.raid_display_name,
            instance.boss_kill_count,
            raid_bosses.len(),
            boss_all_killed,
            base_salary.is_some(),
            jx3_running,
            role_online,
            record_status
        );

        let role_id_opt: Option<String> = Some(db_identity.role_id.clone());
        let drops_vec: Vec<String> = drops.into_iter().collect();

        if let Err(e) = upsert_raid_drop_record(
            &conn,
            instance,
            &drops_vec,
            &boss_ids,
            &boss_names,
            &role_id_opt,
            &db_identity.role_name,
            &db_identity.server,
            &db_identity.region,
            raid_name,
            &raid_entry.raid_id,
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
    let game_dir = get_game_directory()?;

    // 1. 调用 active_detector 获取活跃检测结果（与前端 useActivePoller 一致）
    let active_result = crate::mingyi::active_detector::detect_accounts_active_internal(&game_dir);
    if !active_result.jx3_running {
        return Err("JX3 进程未运行".to_string());
    }
    let jx3_running = active_result.jx3_running;

    // 从 active_result 获取进程启动时间（用于过滤 JCL 文件，只扫描本次会话产生的）
    let process_start_ms = active_result
        .jx3_start_time
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);

    log::info!(
        "[DropScanner] 批量扫描: JX3 启动时间={}, process_start_ms={}, 检测到 {} 个角色",
        active_result.jx3_start_time.as_deref().unwrap_or("未知"),
        process_start_ms,
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

    // 预热副本配置缓存（避免每个账号重复加载）
    if let Ok(conn) = db::init_db() {
        if let Err(e) = get_cached_raids(&conn) {
            log::warn!("[DropScanner] 预热副本配置缓存失败: {}", e);
        }
    }

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

        let result = scan_raid_drops_internal(uid, jx3_running, role_online, process_start_ms, 0, 0);
        results.push((uid.clone(), result));

        // Recent 角色扫描完成后记入集合，后续轮询跳过
        if !is_active {
            scanned_set.insert(uid.clone());
        }
    }

    log::info!("[DropScanner] 批量扫描完成: 共扫描 {} 个活跃角色", results.len());
    Ok(results)
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
#[tauri::command]
pub async fn scan_raids_in_range(start_ms: i64, end_ms: i64) -> Result<Vec<AccountScanResult>, String> {
    tokio::task::spawn_blocking(move || {
        log::info!(
            "[DropScanner] 开始按时间范围扫描所有账号: start_ms={}, end_ms={}",
            start_ms, end_ms
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
                                0,     // 不按 mtime 过滤
                                start_ms,
                                end_ms,
                                Some(raids_ref),
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
        log::info!(
            "[DropScanner] 时间范围扫描完成: 共扫描 {} 个账号, 处理 {} 个副本实例",
            results.len(),
            total_instances
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

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[], &HashMap::new());
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

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[], &HashMap::new());
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

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[], &HashMap::new());
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

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[], &HashMap::new());
        assert_eq!(instances.len(), 0, "全为拉托应跳过不创建实例");
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

        let instances = cluster_raid_instances("test_account", jcl_files, &jcl_analyses, &[], &HashMap::new());

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
        let instances = cluster_raid_instances("432345564243886337", jcl_files, &jcl_analyses, &[], &HashMap::new());
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
                if let Ok((chatlog_drops, salary, income, expense, _, _, _)) =
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

}
