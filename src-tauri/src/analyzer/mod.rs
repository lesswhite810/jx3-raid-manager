//! 自动分析模块
//!
//! 提供游戏目录扫描和聊天记录解析功能

#![allow(dead_code)]

mod parser;
mod scanner;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 基础类型定义
// ============================================================================

/// 角色信息（用于扫描和查询）- 前端传入的角色ID转换后的信息
#[derive(Debug, Clone)]
pub struct SelectedRoleInfo {
    pub id: String,              // 角色标识，由前端传递，用于数据库查询
    pub guid: String,            // 游戏目录解析出的UID，用于拼接目录路径
    pub name: String,            // 角色名（不含区服）
    pub server: String,           // 区服
    pub chat_log_path: String,   // 聊天记录目录路径
    pub gkp_path: String,        // GKP目录路径
}

/// 时间范围
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: i64,
    pub end: i64,
}

/// GKP 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GkpFileInfo {
    pub path: String,
    pub filename: String,
    pub start_time: i64,
    pub end_time: i64,
    pub dungeon_name: Option<String>,
    /// 人数（如 25）
    pub player_count: Option<u32>,
    /// 难度（普通/英雄/挑战）
    pub difficulty: Option<String>,
    /// 角色名
    pub role_name: Option<String>,
    /// 角色 GUID
    pub role_guid: Option<String>,
}

/// Chat Log 文件信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogInfo {
    pub path: String,
    pub filename: String,
    pub start_time: i64,
    pub end_time: i64,
    pub record_count: usize,
    /// 角色名称（从目录名获取）
    pub role_name: Option<String>,
    /// 角色 GUID
    pub guid: Option<String>,
}

/// 聊天记录条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatLogEntry {
    pub time: i64,
    pub text: String,
    pub msg: String,
}

/// 文件扫描统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileScanStats {
    pub chat_log_count: usize,
    pub gkp_count: usize,
    pub filtered_chat_log_count: usize,
    pub filtered_gkp_count: usize,
}

// ============================================================================
// 新版分析接口数据结构
// ============================================================================

/// 特殊物品
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialItem {
    pub name: String,
    pub buyer: String,
    pub price: i64,
    pub is_worker_bought: bool,
}

/// 掉落标记
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DropFlags {
    pub has_xuanjing: bool,
    pub has_maju: bool,
    pub has_pet: bool,
    pub has_pendant: bool,
    pub has_mount: bool,
    pub has_appearance: bool,
    pub has_title: bool,
    pub has_secret_book: bool,
}

/// 支出明细
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExpenseDetail {
    pub scattered: i64,
    pub iron: i64,
    pub special: i64,
    pub other: i64,
}

/// 收支信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IncomeInfo {
    pub income: i64,
    pub expense: i64,
    pub expense_detail: ExpenseDetail,
    pub net_income: i64,
    pub drop_flags: DropFlags,
    pub special_items: Vec<SpecialItem>,
}

/// 单条副本记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRecord {
    pub dungeon_name: String,
    pub difficulty: String,
    pub player_count: u32,
    pub start_time: i64,
    pub end_time: i64,
    pub leader_name: String,
    pub income: IncomeInfo,
    pub notes: String,
}

/// 单个角色的分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleAnalyzeResult {
    pub role_id: String,
    pub role_name: String,
    pub records: Vec<AnalyzeRecord>,
}

/// 分析响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeResponse {
    pub roles: Vec<RoleAnalyzeResult>,
}

// ============================================================================
// Tauri 命令
// ============================================================================

/// 将角色 UUID 转换为角色名称（不含区服）
fn convert_role_ids_to_names(role_ids: Vec<String>) -> Vec<String> {
    if role_ids.is_empty() {
        return role_ids;
    }

    match crate::db::db_get_role_names_by_ids(role_ids.clone()) {
        Ok(names) => {
            if names.is_empty() {
                log::warn!("未找到 UUID 对应的角色名称，保持原列表");
                role_ids
            } else {
                log::info!("UUID 转换为角色名称: {:?}", names);
                names
            }
        }
        Err(e) => {
            log::error!("转换 UUID 为角色名称失败: {}", e);
            role_ids
        }
    }
}

/// 将角色 UUID 转换为角色信息（包含区服）
fn convert_role_ids_to_role_infos(role_ids: Vec<String>) -> Vec<SelectedRoleInfo> {
    if role_ids.is_empty() {
        return vec![];
    }

    match crate::db::db_get_role_infos_by_ids(&role_ids) {
        Ok(infos) => {
            if infos.is_empty() {
                log::warn!("未找到 UUID 对应的角色信息");
                vec![]
            } else {
                infos.into_iter().map(|info| SelectedRoleInfo {
                    id: info.id,
                    guid: String::new(),
                    name: info.name,
                    server: info.server,
                    chat_log_path: String::new(),
                    gkp_path: String::new(),
                }).collect()
            }
        }
        Err(e) => {
            log::error!("转换 UUID 为角色信息失败: {}", e);
            vec![]
        }
    }
}

/// 扫描文件统计
#[tauri::command]
pub async fn analyzer_scan_files(
    game_dir: String,
    time_range: TimeRange,
    selected_roles: Option<Vec<String>>,
) -> Result<FileScanStats, String> {
    scanner::clear_role_cache();

    let role_ids = selected_roles.unwrap_or_default();
    let role_names = convert_role_ids_to_names(role_ids);

    log::info!(
        "开始扫描文件 | 目录: {} | 时间范围: {} - {}",
        game_dir,
        time_range.start,
        time_range.end
    );

    let stats = scanner::scan_files(&game_dir, &time_range, &role_names)?;

    log::info!(
        "扫描完成 | chat_log: {} | GKP: {}",
        stats.filtered_chat_log_count,
        stats.filtered_gkp_count
    );

    Ok(stats)
}

/// 获取 GKP 文件列表
#[tauri::command]
pub async fn analyzer_get_gkp_files(
    game_dir: String,
    time_range: TimeRange,
    selected_roles: Option<Vec<String>>,
    _selected_dungeons: Option<Vec<String>>,
) -> Result<Vec<GkpFileInfo>, String> {
    let role_ids = selected_roles.unwrap_or_default();

    // 获取副本列表用于过滤
    let dungeon_list = crate::db::db_get_raid_names().unwrap_or_default();

    // 扫描游戏目录获取角色信息
    let scanned_roles = scanner::scan_roles(&game_dir)?;

    // 转换角色ID为角色信息
    let role_infos = convert_role_ids_to_role_infos(role_ids);

    // 建立角色名到扫描结果的映射
    let scanned_map: std::collections::HashMap<String, &scanner::ScannedRole> = scanned_roles
        .iter()
        .map(|r| (r.role_name.clone(), r))
        .collect();

    // 收集所有选中角色的 GKP 文件
    let mut all_gkp_files = Vec::new();
    for info in &role_infos {
        if let Some(scanned) = scanned_map.get(&info.name) {
            let gkp_path = format!(
                "{}\\interface\\my#data\\{}@zhcn_hd\\userdata\\gkp",
                &game_dir, scanned.guid
            );
            if let Ok(files) = scanner::scan_gkp_files(&gkp_path, &time_range, &dungeon_list) {
                all_gkp_files.extend(files);
            }
        }
    }

    // 按时间排序
    all_gkp_files.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(all_gkp_files)
}

/// 获取 Chat Log 文件列表
#[tauri::command]
pub async fn analyzer_get_chat_log_files(
    game_dir: String,
    _time_range: TimeRange,
    selected_roles: Option<Vec<String>>,
) -> Result<Vec<ChatLogInfo>, String> {
    let role_ids = selected_roles.unwrap_or_default();

    // 扫描游戏目录获取角色信息
    let scanned_roles = scanner::scan_roles(&game_dir)?;

    // 转换角色ID为角色信息
    let role_infos = convert_role_ids_to_role_infos(role_ids);

    // 建立角色名到扫描结果的映射
    let scanned_map: std::collections::HashMap<String, &scanner::ScannedRole> = scanned_roles
        .iter()
        .map(|r| (r.role_name.clone(), r))
        .collect();

    // 收集所有选中角色的 chat_log 文件
    let mut all_chat_log_files = Vec::new();
    for info in &role_infos {
        if let Some(scanned) = scanned_map.get(&info.name) {
            let chat_log_path = format!(
                "{}\\interface\\my#data\\{}@zhcn_hd\\userdata\\chat_log",
                &game_dir, scanned.guid
            );
            if let Ok(files) = scanner::scan_chat_log_files(&chat_log_path) {
                all_chat_log_files.extend(files);
            }
        }
    }

    // 按时间排序
    all_chat_log_files.sort_by(|a, b| b.start_time.cmp(&a.start_time));
    Ok(all_chat_log_files)
}

/// 读取聊天记录
#[tauri::command]
pub async fn analyzer_read_chat_log(
    db_path: String,
    time_range: TimeRange,
    batch_size: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<ChatLogEntry>, String> {
    let batch = batch_size.unwrap_or(1000);
    let off = offset.unwrap_or(0);
    parser::read_chat_log_entries(&db_path, &time_range, batch, off)
}

// ============================================================================
// 分析核心逻辑
// ============================================================================

/// 执行完整分析
///
/// 流程：
/// 1. 通过 convert_role_ids_to_role_infos 查询角色信息列表（包含服务器）
/// 2. 调用 scan_roles 扫描游戏目录，获取角色名与 GUID 对应关系
/// 3. 使用扫描结果完善 role_infos（补充 guid、chat_log_path、gkp_path）
/// 4. 遍历 role_infos，扫描 GKP 和聊天记录文件
/// 5. 解析收支数据，使用 {角色名·服务器} 格式匹配
#[tauri::command]
pub async fn analyzer_analyze(
    game_dir: String,
    time_range: TimeRange,
    selected_roles: Option<Vec<String>>,
) -> Result<AnalyzeResponse, String> {
    let start = std::time::Instant::now();

    // 清除缓存
    scanner::clear_role_cache();

    // 1. 通过 convert_role_ids_to_role_infos 查询角色信息列表（包含 id, name, server）
    //    注意：guid、chat_log_path、gkp_path 此时为空，需要后续补充
    let role_ids = selected_roles.unwrap_or_default();
    let role_infos = convert_role_ids_to_role_infos(role_ids.clone());

    log::info!(
        "========== 开始分析 | 目录: {} | 时间: {} - {} ==========",
        game_dir,
        time_range.start,
        time_range.end
    );

    // 2. 扫描游戏目录，获取角色名与 GUID 对应关系（仅返回目录解析的信息）
    log::info!("开始扫描游戏目录...");
    let scanned_roles = scanner::scan_roles(&game_dir)?;
    log::info!("目录扫描完成，找到 {} 个角色", scanned_roles.len());

    // 3. 建立角色名到扫描结果的映射，用于完善 role_infos
    let scanned_map: std::collections::HashMap<String, &scanner::ScannedRole> = scanned_roles
        .iter()
        .map(|r| (r.role_name.clone(), r))
        .collect();

    log::debug!("扫描到的角色: {:?}", scanned_roles.iter().map(|r| &r.role_name).collect::<Vec<_>>());
    log::info!("role_infos 数量: {}", role_infos.len());

    // 4. 使用扫描结果完善 role_infos
    //    根据 role_name 匹配，补充 guid、chat_log_path、gkp_path
    let mut enriched_role_infos: Vec<SelectedRoleInfo> = Vec::new();
    for mut info in role_infos {
        if let Some(scanned) = scanned_map.get(&info.name) {
            // 补充 GUID
            info.guid = scanned.guid.clone();
            // 补充聊天记录目录路径
            info.chat_log_path = format!(
                "{}\\interface\\my#data\\{}@zhcn_hd\\userdata\\chat_log",
                game_dir, scanned.guid
            );
            // 补充 GKP 目录路径
            info.gkp_path = format!(
                "{}\\interface\\my#data\\{}@zhcn_hd\\userdata\\gkp",
                game_dir, scanned.guid
            );
            log::debug!(
                "完善角色信息: {} | GUID: {} | chat_log: {} | gkp: {}",
                info.name, info.guid, info.chat_log_path, info.gkp_path
            );
        } else {
            log::warn!("角色 {} 在游戏目录中未找到对应信息", info.name);
        }
        enriched_role_infos.push(info);
    }
    let role_infos = enriched_role_infos;

    log::info!(
        "角色信息列表（含目录）: {:?}",
        role_infos.iter().map(|i| format!("{}|id:{}|guid:{}|{}|{}|{}", i.name, i.id, i.guid, i.server, i.chat_log_path, i.gkp_path)).collect::<Vec<_>>()
    );

    // 5. 获取副本列表（在循环之前获取一次，避免重复查询数据库）
    let dungeon_list = crate::db::db_get_raid_names().unwrap_or_else(|e| {
        log::error!("从数据库获取副本列表失败: {}", e);
        vec![]
    });
    log::info!("副本列表: {:?}", dungeon_list);

    // 6. 遍历角色，对每个角色执行扫描和解析
    let mut role_results: HashMap<String, RoleAnalyzeResult> = HashMap::new();

    for role_info in &role_infos {
        let role_name = role_info.name.clone();

        log::info!(
            "处理角色: {} | GKP目录: {} | ChatLog目录: {}",
            role_name,
            role_info.gkp_path,
            role_info.chat_log_path
        );

        // 6.1 扫描该角色的 GKP 文件（简化调用）
        let gkp_files = scanner::scan_gkp_files(
            &role_info.gkp_path,
            &time_range,
            &dungeon_list,
        )?;
        log::info!("角色 {} 找到 {} 个 GKP 文件", role_name, gkp_files.len());

        if gkp_files.is_empty() {
            log::info!("角色 {} 无 GKP 文件，跳过", role_name);
            continue;
        }

        // 6.2 扫描该角色的 chat_log 文件（简化调用）
        let chat_log_files = scanner::scan_chat_log_files(
            &role_info.chat_log_path,
        )?;
        log::info!("角色 {} 找到 {} 个 chat_log 文件", role_name, chat_log_files.len());

        // 6.3 遍历该角色的 GKP 文件解析收支
        for gkp in &gkp_files {
            log::info!(
                "处理 GKP: {} | 角色: {} | 副本: {:?}",
                gkp.filename,
                role_name,
                gkp.dungeon_name
            );

            // 支出解析时使用格式：{角色名·服务器}
            let role_name_with_server = if role_info.server.is_empty() {
                role_info.name.clone()
            } else {
                format!("{}·{}", role_info.name, role_info.server)
            };

            // 4.4 查找时间交集的 chat_log
            let matched_chat_logs: Vec<_> = chat_log_files
                .iter()
                .filter(|cl| cl.start_time <= gkp.end_time && cl.end_time >= gkp.start_time)
                .collect();

            if matched_chat_logs.is_empty() {
                log::warn!("GKP {} 无匹配的 chat_log，跳过", gkp.filename);
                continue;
            }

            // 4.5 使用 SQL 过滤获取团长信息
            let mut leader_name = String::new();
            for cl in &matched_chat_logs {
                match parser::read_leader_chat_log_entries(&cl.path, gkp.start_time, gkp.end_time) {
                    Ok(entries) => {
                        if !entries.is_empty() {
                            // 识别团长
                            leader_name = parser::identify_leader(&entries)
                                .unwrap_or_else(|| String::new());
                            if !leader_name.is_empty() {
                                log::info!("识别到团长: {} from GKP {}", leader_name, gkp.filename);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("读取 chat_log {} 失败: {}", cl.path, e);
                    }
                }
            }

            // 4.6 使用 SQL 获取消费记录（使用 {角色名·服务器} 格式查询）
            let mut expenses: Vec<parser::ExpenseEntry> = Vec::new();
            for cl in &matched_chat_logs {
                match parser::read_expense_chat_log_entries(&cl.path, gkp.start_time, gkp.end_time, &role_name_with_server) {
                    Ok(exp) => {
                        expenses.extend(exp);
                    }
                    Err(e) => {
                        log::error!("读取消费记录失败 {}: {}", cl.path, e);
                    }
                }
            }

            log::info!("GKP {} 共 {} 条消费记录", gkp.filename, expenses.len());

            // 4.7 使用 SQL 获取收入记录
            let mut income_entries: Vec<parser::IncomeEntry> = Vec::new();
            for cl in &matched_chat_logs {
                match parser::read_income_chat_log_entries(&cl.path, gkp.start_time, gkp.end_time) {
                    Ok(entries) => {
                        income_entries.extend(entries);
                    }
                    Err(e) => {
                        log::error!("读取收入记录失败 {}: {}", cl.path, e);
                    }
                }
            }

            log::info!("GKP {} 共 {} 条收入记录", gkp.filename, income_entries.len());

            // 4.8 使用 SQL 获取拍团分配记录
            let mut distributions: Vec<parser::TeamDistributionInfo> = Vec::new();
            for cl in &matched_chat_logs {
                match parser::read_team_distribution_entries(&cl.path, gkp.start_time, gkp.end_time) {
                    Ok(entries) => {
                        distributions.extend(entries);
                    }
                    Err(e) => {
                        log::error!("读取拍团记录失败 {}: {}", cl.path, e);
                    }
                }
            }

            log::info!("GKP {} 共 {} 条拍团记录", gkp.filename, distributions.len());

            // 4.9 解析收支（使用 SQL 查询的收入 + 支出 + 拍团记录）
            let income_info = parser::build_income_info_from_expense(
                &expenses,
                &income_entries,
                &distributions,
                &role_name,
            );

            // 构建记录
            let record = AnalyzeRecord {
                dungeon_name: gkp.dungeon_name.clone().unwrap_or_else(|| "未知副本".to_string()),
                difficulty: gkp.difficulty.clone().unwrap_or_else(|| "普通".to_string()),
                player_count: gkp.player_count.unwrap_or(25),
                start_time: gkp.start_time,
                end_time: gkp.end_time,
                leader_name,
                income: income_info,
                notes: String::new(),
            };

            // 添加到角色结果
            role_results
                .entry(role_name.clone())
                .or_insert_with(|| RoleAnalyzeResult {
                    role_id: role_info.id.clone(),
                    role_name: role_name.clone(),
                    records: Vec::new(),
                })
                .records
                .push(record);
        }
    }

    let response = AnalyzeResponse {
        roles: role_results.into_values().collect(),
    };

    log::info!(
        "========== 分析完成 | 耗时: {}ms | 角色数: {} ==========",
        start.elapsed().as_millis(),
        response.roles.len()
    );

    Ok(response)
}
