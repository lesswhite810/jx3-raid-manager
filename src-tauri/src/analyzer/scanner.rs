//! 文件扫描模块
//!
//! 扫描游戏目录中的 GKP 文件和 chat_log 数据库文件
//!
//! 聊天记录路径结构:
//! {game_dir}\interface\my#data\{角色 guid}@zhcn_hd\{角色名}\userdata\chat_log\{db 文件}
//! 例: E:\Game\...\JX3\bin\zhcn_hd\interface\my#data\432345564241026543@zhcn_hd\发白发\userdata\chat_log\

use super::{ChatLogInfo, FileScanStats, GkpFileInfo, TimeRange};
use chrono;
use regex::Regex;
use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

/// 目录扫描结果（简化结构，仅包含从目录解析的信息）
/// 用于与数据库查询的角色信息合并
#[derive(Debug, Clone)]
pub struct ScannedRole {
    pub guid: String,           // 游戏角色唯一标识（从目录名解析）
    pub role_name: String,      // 角色名（从子目录名解析，不含区服）
}

/// 角色完整信息（包含路径，用于 GKP 和 ChatLog 扫描）
#[derive(Debug, Clone)]
pub struct RoleInfo {
    pub guid: String,
    pub role_name: String,           // 不带区服，如 "一汀鸥鹭"
    pub role_name_with_server: String, // 带区服，如 "一汀鸥鹭·梦江南"
    pub chat_log_path: String,
    pub gkp_path: String,
}

impl RoleInfo {
    /// 从扫描结果构建完整角色信息
    pub fn from_scanned(scanned: &ScannedRole, game_dir: &str, role_name_with_server: &str) -> Self {
        // 目录结构: {game_dir}\interface\my#data\{GUID}@zhcn_hd\userdata\
        let base_path = format!(
            "{}\\interface\\my#data\\{}@zhcn_hd\\userdata",
            game_dir, scanned.guid
        );
        RoleInfo {
            guid: scanned.guid.clone(),
            role_name: scanned.role_name.clone(),
            role_name_with_server: role_name_with_server.to_string(),
            chat_log_path: format!("{}\\chat_log", base_path),
            gkp_path: format!("{}\\gkp", base_path),
        }
    }
}

/// 角色扫描缓存
/// 避免在同一次分析中重复扫描角色目录
struct RoleScanCache {
    game_dir: String,
    roles: Vec<ScannedRole>,  // 缓存简化的扫描结果
}

impl RoleScanCache {
    fn new(game_dir: &str) -> Self {
        RoleScanCache {
            game_dir: game_dir.to_string(),
            roles: Vec::new(),
        }
    }

    fn is_valid(&self, game_dir: &str) -> bool {
        !self.roles.is_empty() && self.game_dir == game_dir
    }

    fn set(&mut self, game_dir: &str, roles: Vec<ScannedRole>) {
        self.game_dir = game_dir.to_string();
        self.roles = roles;
    }

    fn get(&self) -> &Vec<ScannedRole> {
        &self.roles
    }
}

/// 全局角色扫描缓存（使用 Mutex 保证线程安全）
static ROLE_CACHE: Mutex<Option<RoleScanCache>> = Mutex::new(None);

/// 获取缓存的角色列表（如果有效）
fn get_cached_roles(game_dir: &str) -> Option<Vec<ScannedRole>> {
    let guard = ROLE_CACHE.lock().ok()?;
    if let Some(ref cache) = *guard {
        if cache.is_valid(game_dir) {
            log::debug!("使用缓存的角色列表: {} 个", cache.get().len());
            return Some(cache.get().clone());
        }
    }
    None
}

/// 设置角色缓存
fn set_role_cache(game_dir: &str, roles: Vec<ScannedRole>) {
    if let Ok(mut guard) = ROLE_CACHE.lock() {
        let mut cache = RoleScanCache::new(game_dir);
        cache.set(game_dir, roles);
        *guard = Some(cache);
    }
}

/// 清除角色缓存
pub fn clear_role_cache() {
    if let Ok(mut guard) = ROLE_CACHE.lock() {
        *guard = None;
    }
    log::debug!("角色缓存已清除");
}

/// 扫描并统计文件
/// selected_roles: 只统计这些角色（角色名）的文件，如果为空则统计所有角色
pub fn scan_files(game_dir: &str, time_range: &TimeRange, selected_roles: &[String]) -> Result<FileScanStats, String> {
    log::info!("========== 开始扫描文件 ==========");
    log::info!("游戏目录: {}", game_dir);
    log::info!("时间范围: {} - {}", time_range.start, time_range.end);

    let mut stats = FileScanStats {
        chat_log_count: 0,
        gkp_count: 0,
        filtered_chat_log_count: 0,
        filtered_gkp_count: 0,
    };

    // 扫描 chat_log 文件 - 按角色目录结构
    log::info!("========== 开始扫描角色目录 ==========");
    let scanned = scan_roles(game_dir)?;

    // 将 ScannedRole 转换为 RoleInfo（统计文件时不需要区服信息）
    let roles: Vec<RoleInfo> = scanned.iter()
        .map(|s| RoleInfo::from_scanned(s, game_dir, &s.role_name))
        .collect();

    // 过滤角色：如果指定了selected_roles，则只统计这些角色
    let filtered_roles: Vec<&RoleInfo> = if selected_roles.is_empty() {
        roles.iter().collect()
    } else {
        let selected_set: std::collections::HashSet<&String> = selected_roles.iter().collect();
        roles.iter().filter(|r| selected_set.contains(&r.role_name)).collect()
    };

    if !selected_roles.is_empty() {
        log::info!("只统计选中的角色: {:?}, 匹配到 {} 个", selected_roles, filtered_roles.len());
    }

    log::info!("找到 {} 个角色（已过滤）", filtered_roles.len());

    // 扫描每个角色的 chat_log 和 GKP 文件
    for role in &filtered_roles {
        log::info!("角色: {} | GUID: {} | 路径: {}", role.role_name, role.guid, role.chat_log_path);

        // ===== 扫描 chat_log 文件 =====
        let chat_log_dir = PathBuf::from(&role.chat_log_path);
        if chat_log_dir.exists() {
            if let Ok(entries) = fs::read_dir(&chat_log_dir) {
                let db_files: Vec<_> = entries.filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|ext| ext == "db").unwrap_or(false))
                    .collect();

                let total_db = db_files.len();
                stats.chat_log_count += total_db;

                log::debug!("角色 {} 共有 {} 个 db 文件", role.role_name, total_db);

                for entry in db_files {
                    let path = entry.path();
                    let filename = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    if let Ok(metadata) = fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            let modified_ms = modified
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_millis() as i64)
                                .unwrap_or(0);

                            if modified_ms >= time_range.start {
                                stats.filtered_chat_log_count += 1;
                                log::trace!("DB 文件符合时间范围: {}", filename);
                            }
                        }
                    }
                }
            }
        } else {
            log::warn!("角色的 chat_log 目录不存在: {}", role.chat_log_path);
        }

        // ===== 扫描 GKP 文件 =====
        let gkp_dir = PathBuf::from(&role.gkp_path);
        if gkp_dir.exists() {
            let entries: Vec<_> = match fs::read_dir(&gkp_dir) {
                Ok(e) => e.filter_map(|e| e.ok()).collect(),
                Err(e) => {
                    log::warn!("读取角色 {} 的 GKP 目录失败: {}", role.role_name, e);
                    vec![]
                }
            };

            stats.gkp_count += entries.len();
            log::debug!("角色 {} 的 GKP 目录共有 {} 个文件", role.role_name, entries.len());

            for entry in entries {
                let path = entry.path();
                if path.extension().map(|e| e == "jx3dat").unwrap_or(false) {
                    let filename = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // 解析文件名获取时间
                    let (_start_time, end_time, _, _, _) = parse_gkp_filename(&filename);

                    // 检查是否在时间范围内（使用文件名中的时间）
                    if end_time > 0 && end_time >= time_range.start {
                        stats.filtered_gkp_count += 1;
                        log::trace!("GKP 文件符合时间范围: {}", filename);
                    }
                }
            }
        }
    }

    log::info!("========== 文件扫描完成 ==========");
    log::info!("总 chat_log 文件: {}", stats.chat_log_count);
    log::info!("符合时间范围的 chat_log 文件: {}", stats.filtered_chat_log_count);
    log::info!("总 GKP 文件: {}", stats.gkp_count);
    log::info!("符合时间范围的 GKP 文件: {}", stats.filtered_gkp_count);

    Ok(stats)
}

/// 扫描所有角色目录
/// 返回目录解析的角色信息列表（仅包含角色名与GUID）
///
/// 参数:
/// - game_dir: 游戏目录路径
///
/// 目录结构：
/// {game_dir}\interface\my#data\{GUID}@zhcn_hd\
/// ├── 司蘅\                    ← 角色名目录（中文）
/// ├── userdata\
/// │   └── chat_log\            ← 聊天记录在这里（固定位置）
/// └── ...
///
/// 返回值说明：
/// - 仅返回从目录结构解析的信息（guid + role_name）
/// - 路径信息（chat_log_path, gkp_path）由调用方根据 game_dir 和 guid 动态构建
/// - 带区服名称（role_name_with_server）由调用方从数据库查询补充
pub fn scan_roles(game_dir: &str) -> Result<Vec<ScannedRole>, String> {
    // 尝试从缓存获取
    if let Some(cached_roles) = get_cached_roles(game_dir) {
        return Ok(cached_roles);
    }

    let my_data_dir = PathBuf::from(game_dir)
        .join("interface")
        .join("my#data");

    log::debug!("扫描角色目录: {}", my_data_dir.display());

    if !my_data_dir.exists() {
        log::warn!("my#data 目录不存在!");
        return Ok(vec![]);
    }

    let mut roles: Vec<ScannedRole> = Vec::new();

    // 扫描所有以 @zhcn_hd 结尾的目录
    let guid_dirs = fs::read_dir(&my_data_dir)
        .map_err(|e| format!("读取目录失败: {}", e))?;

    log::debug!("开始遍历 GUID 目录...");

    for guid_dir in guid_dirs.filter_map(|e| e.ok()) {
        let guid_path = guid_dir.path();
        let dir_name = guid_dir.file_name().to_string_lossy().to_string();

        // 检查是否是角色目录 (以 @zhcn_hd 结尾)
        if !dir_name.contains("@zhcn_hd") {
            log::trace!("跳过非角色目录: {}", dir_name);
            continue;
        }

        log::debug!("找到 GUID 目录: {}", dir_name);

        // 提取 GUID
        let guid = dir_name.split('@').next().unwrap_or(&dir_name).to_string();
        log::debug!("提取 GUID: {}", guid);

        // 聊天日志路径是固定的: {GUID}@zhcn_hd\userdata\chat_log
        let userdata_path = guid_path.join("userdata");
        let chat_log_path = userdata_path.join("chat_log");

        // 如果 chat_log 目录不存在，跳过
        if !chat_log_path.exists() {
            log::trace!("Chat_log 目录不存在: {}", chat_log_path.display());
            continue;
        }

        // 查找中文名称的子目录作为角色名
        let mut role_name = guid.clone(); // 默认使用 GUID

        if let Ok(sub_dirs) = fs::read_dir(&guid_path) {
            for sub_dir in sub_dirs.filter_map(|e| e.ok()) {
                let sub_path = sub_dir.path();

                // 检查是否是目录
                if !sub_path.is_dir() {
                    continue;
                }

                let sub_name = sub_dir.file_name().to_string_lossy().to_string();

                // 跳过 @zhcn_hd 结尾的
                if sub_name.contains("@zhcn_hd") {
                    continue;
                }

                // 检查是否是有效的中文字符
                if contains_chinese(&sub_name) {
                    role_name = sub_name.clone();
                    log::debug!("找到角色名目录: {} -> {}", sub_name, guid);
                    break; // 找到一个中文目录就够了
                }
            }
        }

        log::info!("✅ 找到角色 | 角色名: {} | GUID: {}", role_name, guid);

        // 仅返回目录解析的信息
        roles.push(ScannedRole {
            guid: guid.clone(),
            role_name,
        });
    }

    log::info!("角色扫描完成，共找到 {} 个角色", roles.len());
    for role in &roles {
        log::debug!("  - {} (GUID: {})", role.role_name, role.guid);
    }

    // 保存到缓存
    set_role_cache(game_dir, roles.clone());

    Ok(roles)
}

/// 检查字符串是否包含中文字符
fn contains_chinese(s: &str) -> bool {
    s.chars().any(|c| {
        ('\u{4E00}'..='\u{9FFF}').contains(&c) ||  // CJK统一汉字
        ('\u{3400}'..='\u{4DBF}').contains(&c) ||  // CJK统一汉字扩展A
        ('\u{F900}'..='\u{FAFF}').contains(&c)      // CJK兼容象形文字
    })
}

/// 扫描 GKP 文件列表（简化版）
///
/// 职责：扫描指定目录下的 GKP 文件，筛选符合时间范围和副本列表的文件
///
/// 参数:
/// - gkp_path: GKP 目录路径（如: E:\...\userdata\gkp）
/// - time_range: 时间范围
/// - dungeon_list: 副本名称列表（用于过滤，为空则不过滤）
///
/// 返回: 符合条件的 GKP 文件列表
pub fn scan_gkp_files(
    gkp_path: &str,
    time_range: &TimeRange,
    dungeon_list: &[String],
) -> Result<Vec<GkpFileInfo>, String> {
    let gkp_dir = PathBuf::from(gkp_path);

    if !gkp_dir.exists() {
        log::debug!("GKP 目录不存在: {}", gkp_path);
        return Ok(vec![]);
    }

    // 构建副本名称集合用于快速查找
    let dungeon_set: std::collections::HashSet<&str> = dungeon_list
        .iter()
        .map(|s| s.trim())
        .collect();
    let has_dungeon_filter = !dungeon_set.is_empty();

    if has_dungeon_filter {
        log::info!("使用副本列表过滤，共 {} 个副本: {:?}", dungeon_set.len(), dungeon_list);
    }

    let entries: Vec<_> = match fs::read_dir(&gkp_dir) {
        Ok(e) => e.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            log::warn!("读取 GKP 目录失败: {} | {}", gkp_path, e);
            return Ok(vec![]);
        }
    };

    log::debug!("GKP 目录共有 {} 个文件: {}", entries.len(), gkp_path);

    let mut files: Vec<GkpFileInfo> = Vec::new();

    for entry in entries {
        let path = entry.path();
        if path.extension().map(|e| e == "jx3dat").unwrap_or(false) {
            let filename = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // 解析文件名获取开始时间、副本名、人数和难度
            let (start_time, _, dungeon_name, player_count, difficulty) = parse_gkp_filename(&filename);

            // 使用文件修改时间作为结束时间
            // 注意：需要将系统 UTC 时间戳转换为本地时间戳格式（与数据库一致）
            let end_time = fs::metadata(&path)
                .and_then(|m| m.modified())
                .map(|t| {
                    // 将系统时间转换为 chrono DateTime<Local>
                    let local_time: chrono::DateTime<chrono::Local> = t.into();
                    // 获取本地时间的 naive 格式，然后转换为本地时间戳
                    // 本地时间戳 = 将本地时间直接当作 UTC 计算的时间戳
                    local_time.naive_local().and_utc().timestamp_millis()
                })
                .unwrap_or(start_time);

            // 检查是否在时间范围内
            if end_time > 0 && end_time < time_range.start {
                log::debug!(
                    "GKP 文件超出时间范围(跳过): {} | 副本: {:?} | 结束时间: {} < 范围开始: {}",
                    filename, dungeon_name, end_time, time_range.start
                );
                continue;
            }

            // 如果有副本过滤，检查副本名称是否在列表中
            if has_dungeon_filter {
                if let Some(ref name) = dungeon_name {
                    if !dungeon_set.contains(name.as_str()) {
                        log::debug!(
                            "GKP 文件副本不在列表中(跳过): {} | 副本: {:?}",
                            filename, name
                        );
                        continue;
                    }
                } else {
                    log::debug!("GKP 文件无法解析副本名(跳过): {}", filename);
                    continue;
                }
            }

            log::debug!(
                "GKP 文件(保留): {} | 副本: {:?} | 人数: {:?} | 难度: {:?} | 时间: {} - {}",
                filename, dungeon_name, player_count, difficulty, start_time, end_time
            );

            files.push(GkpFileInfo {
                path: path.to_string_lossy().to_string(),
                filename,
                start_time,
                end_time,
                dungeon_name,
                player_count,
                difficulty,
                role_name: None,
                role_guid: None,
            });
        }
    }

    // 按时间排序
    files.sort_by(|a, b| b.start_time.cmp(&a.start_time));

    log::info!("GKP 文件扫描完成 | 目录: {} | 符合条件: {} 个", gkp_path, files.len());
    Ok(files)
}

/// 扫描 Chat Log 文件列表（简化版）
///
/// 职责：扫描指定目录下的 db 文件，返回文件名和时间范围
///
/// 参数:
/// - chat_log_path: Chat Log 目录路径（如: E:\...\userdata\chat_log）
///
/// 返回: db 文件列表（包含文件路径和时间范围）
pub fn scan_chat_log_files(
    chat_log_path: &str,
) -> Result<Vec<ChatLogInfo>, String> {
    let chat_log_dir = PathBuf::from(chat_log_path);

    if !chat_log_dir.exists() {
        log::debug!("Chat Log 目录不存在: {}", chat_log_path);
        return Ok(vec![]);
    }

    let entries = match fs::read_dir(&chat_log_dir) {
        Ok(e) => e.filter_map(|e| e.ok()).collect::<Vec<_>>(),
        Err(e) => {
            log::warn!("无法读取 Chat Log 目录 {}: {}", chat_log_dir.display(), e);
            return Ok(vec![]);
        }
    };

    log::debug!("Chat Log 目录共有 {} 个文件: {}", entries.len(), chat_log_path);

    let mut files: Vec<ChatLogInfo> = Vec::new();

    for entry in entries {
        let path = entry.path();
        if path.extension().map(|e| e == "db").unwrap_or(false) {
            let filename = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // 查询数据库获取时间范围（通过 SQL 查询 MIN/MAX）
            let (min_time, max_time) = query_chat_log_time_range(&path.to_string_lossy())
                .unwrap_or((0, 0));

            // 跳过空文件（没有记录）
            if min_time == 0 && max_time == 0 {
                log::debug!("Chat Log 文件无记录(跳过): {}", filename);
                continue;
            }

            // 将数据库返回的时间（秒）转换为毫秒
            let start_time_ms = min_time * 1000;
            let end_time_ms = max_time * 1000;

            log::debug!(
                "Chat Log 文件: {} | 时间范围: {} - {}",
                filename, start_time_ms, end_time_ms
            );

            files.push(ChatLogInfo {
                path: path.to_string_lossy().to_string(),
                filename,
                start_time: start_time_ms,
                end_time: end_time_ms,
                record_count: 0,  // 不再统计记录数
                role_name: None,
                guid: None,
            });
        }
    }

    // 按时间排序
    files.sort_by(|a, b| b.start_time.cmp(&a.start_time));

    log::info!("Chat Log 文件扫描完成 | 目录: {} | 文件数: {}", chat_log_path, files.len());
    Ok(files)
}

/// 查询 chat_log 数据库的时间范围（MIN/MAX）
/// 返回: (min_time, max_time) 单位：秒
fn query_chat_log_time_range(db_path: &str) -> Result<(i64, i64), String> {
    if !Path::new(db_path).exists() {
        return Ok((0, 0));
    }

    let conn = Connection::open(db_path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 表名固定为 ChatLog
    let query = "SELECT COALESCE(MIN(time), 0), COALESCE(MAX(time), 0) FROM ChatLog";

    let (min_time, max_time) = conn.query_row(
        query,
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    ).unwrap_or((0, 0));

    Ok((min_time, max_time))
}

/// 解析 GKP 文件名
/// 格式:
/// - 2024-01-06-19-46-25_范阳夜变.gkp.jx3dat (无人数)
/// - 2024-01-06-19-46-25_25人范阳夜变.gkp.jx3dat (有人数无难度)
/// - 2024-01-06-19-46-25_25人普通范阳夜变.gkp (有人数和难度.jx3dat)
/// 注意：文件名中的时间是本地时间（如北京时间 UTC+8）
/// 返回: (start_time, end_time, dungeon_name, player_count, difficulty)
fn parse_gkp_filename(filename: &str) -> (i64, i64, Option<String>, Option<u32>, Option<String>) {
    // 匹配格式: 2024-01-06-19-46-25_25人普通范阳夜变.gkp.jx3dat
    // 时间_人数?难度?副本名.gkp.jx3dat
    let pattern = Regex::new(r"^(\d{4}-\d{2}-\d{2}-\d{2}-\d{2}-\d{2})_(\d+人)?(普通|英雄|挑战)?(.+?)\.gkp\.jx3dat$").unwrap();

    if let Some(caps) = pattern.captures(filename) {
        // 解析时间戳
        let datetime_str = &caps[1];
        let player_count_str = caps.get(2).map(|m| m.as_str());
        let difficulty_str = caps.get(3).map(|m| m.as_str());
        let dungeon_name_raw = &caps[4];

        // 解析日期时间字符串（文件名中的时间是本地时间）
        let parts: Vec<&str> = datetime_str.split('-').collect();
        if parts.len() == 6 {
            if let (Ok(year), Ok(month), Ok(day), Ok(hour), Ok(minute), Ok(second)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<i32>(),
                parts[2].parse::<i32>(),
                parts[3].parse::<i32>(),
                parts[4].parse::<i32>(),
                parts[5].parse::<i32>(),
            ) {
                // 使用 chrono 将本地时间转换为时间戳
                // 构建本地时间（文件名中的时间是本地时间）
                let naive_time = chrono::NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                    .and_then(|d| d.and_hms_opt(hour as u32, minute as u32, second as u32));

                if let Some(naive) = naive_time {
                    // 注意：剑网3游戏数据库中的 time 字段存储的是"本地时间戳"
                    // 即直接将本地时间（如 20:32:53）当作 UTC 时间计算的时间戳
                    // 因此这里使用 naive.and_utc().timestamp_millis() 而不是 Local.from_local_datetime()
                    // 这样才能与游戏数据库的时间戳保持一致
                    let timestamp_ms = naive.and_utc().timestamp_millis();

                    // 解析人数
                    let player_count = player_count_str.and_then(|s| {
                        s.trim_end_matches("人").parse::<u32>().ok()
                    });

                    // 难度默认为"普通"
                    let difficulty = difficulty_str.map(|s| s.to_string())
                        .or_else(|| Some("普通".to_string()));

                    // 副本名称（去除可能的尾部空白）
                    let dungeon_name = Some(dungeon_name_raw.trim().to_string());

                    log::debug!(
                        "解析 GKP 文件名成功: {} -> 本地时间: {} -> 时间戳: {}, 人数: {:?}, 难度: {:?}, 副本: {:?}",
                        filename, datetime_str, timestamp_ms, player_count, difficulty, dungeon_name
                    );

                    return (timestamp_ms, timestamp_ms, dungeon_name, player_count, difficulty);
                }
            }
        }
    }

    // 如果正则不匹配，尝试旧方法（可能是其他格式）
    log::warn!("GKP 文件名格式不匹配: {}", filename);

    (0, 0, Some(filename.to_string()), None, None)
}

/// 查询 chat_log 数据库统计信息
/// 注意：表名固定为 ChatLog
/// 注意：返回整个文件的时间范围（不限制），以便后续与 GKP 时间进行交集匹配
fn query_chat_log_stats(db_path: &str, time_range: &TimeRange) -> Result<(usize, i64, i64), String> {
    if !Path::new(db_path).exists() {
        log::warn!("数据库文件不存在: {}", db_path);
        return Ok((0, 0, 0));
    }

    let conn = Connection::open(db_path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 表名固定为 ChatLog
    let table_name = "ChatLog";

    // 将毫秒转换为秒
    let start_sec = time_range.start / 1000;
    // 如果结束时间大于 2050 年（相对较大的时间戳），则视为"无限"，使用 i64::MAX
    // 这样可以处理前端传递的 Date.now() + 10 年这种情况
    let end_sec = if time_range.end > 2524579200000 {
        // 2050-01-01 00:00:00 UTC 以毫秒为单位，视为"无限"
        i64::MAX
    } else {
        time_range.end / 1000
    };

    log::debug!("查询 ChatLog 文件: {} | 过滤时间范围(秒): {} - {}", db_path, start_sec, end_sec);

    // 1. 查询整个文件的 MIN/MAX（不限制时间范围，用于判断文件覆盖的时间范围）
    let full_query = format!(
        "SELECT COALESCE(MIN(time), 0), COALESCE(MAX(time), 0) FROM {}",
        table_name
    );

    let (min_time, max_time) = conn.query_row(
        &full_query,
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    ).unwrap_or((0, 0));

    log::debug!("ChatLog 文件时间范围(原始): {} - {} 秒", min_time, max_time);

    // 2. 查询符合时间过滤条件的记录数（用于判断是否有有效数据）
    let filtered_query = format!(
        "SELECT COUNT(*) FROM {} WHERE time >= ?1 AND time <= ?2",
        table_name
    );

    let record_count = conn.query_row(
        &filtered_query,
        rusqlite::params![start_sec, end_sec],
        |row| row.get::<_, usize>(0),
    ).unwrap_or(0);

    log::debug!("ChatLog 过滤时间范围(秒): {} - {}", start_sec, end_sec);

    Ok((record_count, min_time, max_time))
}
