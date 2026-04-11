use chrono::NaiveDateTime;
use regex::Regex;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use winreg::enums::*;
use winreg::RegKey;

// 引入 db 模块用于数据库操作
use crate::db;

// 引入 kungfu_data 模块获取统一的心法数据
use crate::kungfu_data;

const GKP_BASE_PATH: &str = r"interface\my#data";
const USERDATA_BASE_PATH: &str = "userdata";
const GAME_RUNTIME_SUFFIX: [&str; 4] = ["Game", "JX3", "bin", "zhcn_hd"];

// 茗伊数据库路径常量
const MING_YI_DB_PATH: &str =
    r"Interface\MY#DATA\!all-users@zhcn_hd\userdata\role_statistics\equip_stat.v4.db";

// 当前游戏等级
const CURRENT_LEVEL: i32 = 130;

// 预编译正则表达式，避免重复编译开销
static KUNGFU_DESC_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"推荐心法[：:]([^()]+)\(([^()]+)\)").unwrap());

// 茗伊数据库的角色信息
#[derive(Debug, Clone)]
pub struct MingYiRoleInfo {
    pub owner_name: String,
    pub server_name: String,
    pub force_id: i32,
    pub level: i32,
    pub scores: Vec<i32>,            // 4套装备分数
    pub effective_suit: i32,         // 当前使用套装的索引 (1-4)
    pub kungfu_id: Option<i32>,      // 心法ID
    pub kungfu_name: Option<String>, // 心法名称
}

// 解析 ownerscore 字段，支持多种格式：
// - 数组格式: "[100,200,300,400]"
// - 字典格式: "{1:100,2:200,3:300,4:400}"
// - 单值格式: "{404898}" (等价于第1套)
// - 稀疏格式: "{[2]=515657}" 或 "{0,0,0,671428,[7]=4608}"
// - 混合格式: "{520999,[3]=527449}"
fn parse_ownerscore(ownerscore: &str) -> Vec<i32> {
    let trimmed = ownerscore.trim();

    // 如果为空或为 "0.0"，返回空数组
    if trimmed.is_empty() || trimmed == "0.0" || trimmed == "0" {
        return vec![0, 0, 0, 0];
    }

    // 尝试解析数组格式 [100,200,300,400]
    if trimmed.starts_with('[') {
        let content = trimmed.trim_start_matches('[').trim_end_matches(']');
        let values: Vec<i32> = content
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        // 确保有4个值
        let mut result = vec![0; 4];
        for (i, v) in values.into_iter().take(4).enumerate() {
            result[i] = v;
        }
        return result;
    }

    // 解析字典/稀疏格式 {key:value,...} 或 {[index]=value,...} 或 {value,value,...}
    if trimmed.starts_with('{') {
        let content = trimmed.trim_start_matches('{').trim_end_matches('}');
        let mut scores = vec![0; 4];
        let mut sequential_index: usize = 0; // 用于裸数值的顺序索引

        for pair in content.split(',') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }

            // 情况1: 括号索引格式 {[2]=515657}
            if pair.starts_with('[') {
                if let Some(bracket_end) = pair.find("]=") {
                    let inner = &pair[1..bracket_end];
                    if let (Ok(index), Ok(value)) = (
                        inner.parse::<usize>(),
                        pair[bracket_end + 2..].parse::<i32>(),
                    ) {
                        if index >= 1 && index <= 4 {
                            scores[index - 1] = value;
                        }
                    }
                }
                continue;
            }

            // 情况2: 字典格式 {3:520351}
            if let Some(colon_pos) = pair.find(':') {
                let key_str = &pair[..colon_pos];
                let value_str = &pair[colon_pos + 1..];
                if let (Ok(key), Ok(value)) = (
                    key_str.trim().parse::<usize>(),
                    value_str.trim().parse::<i32>(),
                ) {
                    if key >= 1 && key <= 4 {
                        scores[key - 1] = value;
                    }
                }
                continue;
            }

            // 情况3: 裸数值 {520999} - 按顺序映射到第1,2,3,4套
            if let Ok(value) = pair.parse::<i32>() {
                sequential_index += 1;
                if sequential_index <= 4 {
                    scores[sequential_index - 1] = value;
                }
            }
        }
        return scores;
    }

    vec![0, 0, 0, 0]
}

// 从 desc 中用正则匹配 "推荐心法：门派(心法)" 格式
// 返回 (force_id, kungfu_name) 元组
fn parse_recommended_kungfu_from_desc(desc: &str) -> Option<(i32, String)> {
    let caps = KUNGFU_DESC_REGEX.captures(desc)?;

    let force_name = caps.get(1)?.as_str().trim();
    let kungfu_name = caps.get(2)?.as_str().trim();

    let force_id = kungfu_data::get_force_id_by_name(force_name)?;
    Some((force_id, kungfu_name.to_string()))
}

// 从装备描述中解析心法
// 特殊处理无相楼：如果检测到"推荐心法：无相楼(幽罗引)"，直接返回幽罗引，不需要校验门派一致性
fn resolve_kungfu_from_descs(descs: &[String], role_force_id: i32) -> Option<(i32, String)> {
    // 直接使用静态引用，避免重复 clone
    let kungfu_map = kungfu_data::get_kungfu_force_name_to_id_map();

    // 特殊处理无相楼：如果检测到"推荐心法：无相楼(幽罗引)"，直接返回幽罗引，不需要校验门派一致性
    const TANGJIAN_FORCE_ID: i32 = 8; // 藏剑门派ID

    for desc in descs {
        if desc.contains("推荐心法：无相楼(幽罗引)") {
            // 无相楼特殊处理：直接返回幽罗引，不需要校验门派一致性
            const YOULUOYIN_ID: i32 = 10821;
            return Some((YOULUOYIN_ID, "幽罗引".to_string()));
        }

        if let Some((force_id, kungfu_name)) = parse_recommended_kungfu_from_desc(desc) {
            if force_id == role_force_id {
                // 特殊处理藏剑：如果门派为藏剑且心法为"均可"，返回问水诀
                let final_kungfu_name = if force_id == TANGJIAN_FORCE_ID && kungfu_name == "均可" {
                    "问水诀".to_string()
                } else {
                    kungfu_name
                };
                if let Some(&kungfu_id) = kungfu_map.get(&(force_id, final_kungfu_name.clone())) {
                    return Some((kungfu_id, final_kungfu_name));
                }
            }
        }
    }
    None
}

// 读取茗伊数据库中的角色信息
// 从茗伊数据库的 owner_name 中提取真实角色名
// 兼容 "角色名·区服" 格式，例如 "秦算卦·唯我独尊" -> "秦算卦"
fn normalize_mingyi_role_name(owner_name: &str, server_name: &str) -> String {
    // 检查是否包含 "·区服" 格式（通常包含服务器名称）
    if let Some(idx) = owner_name.find('·') {
        let potential_server = &owner_name[idx + '·'.len_utf8()..];
        // 如果 "·" 后面的内容与 server_name 匹配，说明是带区服后缀的格式
        if potential_server == server_name {
            return owner_name[..idx].to_string();
        }
    }
    // 如果不匹配，返回原名称
    owner_name.to_string()
}

fn read_mingyi_role_info(game_directory: &Path) -> Result<Vec<MingYiRoleInfo>, String> {
    let db_path = game_directory.join(MING_YI_DB_PATH);

    if !db_path.exists() {
        return Ok(Vec::new()); // 数据库不存在不算错误
    }

    let conn =
        rusqlite::Connection::open(&db_path).map_err(|e| format!("打开茗伊数据库失败: {}", e))?;

    // 查询角色信息，只取当前等级的角色
    let mut stmt = conn
        .prepare(
            "SELECT ownerkey, ownername, servername, ownerforce, ownerlevel, ownerscore, ownersuitindex
             FROM OwnerInfo
             WHERE ownerlevel = ?",
        )
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let role_iter = stmt
        .query_map([CURRENT_LEVEL], |row| {
            let owner_key_str: String = row.get(0)?;
            let owner_name: String = row.get(1)?;
            let server_name: String = row.get(2)?;
            let force_id: i32 = row.get(3)?;
            let level: i32 = row.get(4)?;
            let ownerscore: String = row.get(5)?;
            let suit_index: Option<i32> = row.get(6).ok();
            Ok((
                owner_key_str,
                owner_name,
                server_name,
                force_id,
                level,
                ownerscore,
                suit_index,
            ))
        })
        .map_err(|e| format!("查询失败: {}", e))?;

    // 第一步：收集所有角色及其有效套装的 ownerkey
    #[derive(Debug)]
    struct RoleBasicInfo {
        owner_key: i64,
        owner_name: String,
        server_name: String,
        force_id: i32,
        level: i32,
        scores: Vec<i32>,
        effective_suit: Option<i32>,
    }

    let mut role_basic_infos: Vec<RoleBasicInfo> = Vec::new();
    let mut equip_query_params: Vec<(i64, i32)> = Vec::new(); // (owner_key, suit_index)

    for role_result in role_iter {
        let (owner_key_str, owner_name, server_name, force_id, level, ownerscore, suit_index) =
            role_result.map_err(|e| format!("读取数据失败: {}", e))?;
        let owner_key: i64 = owner_key_str.parse().unwrap_or(0);
        let scores = parse_ownerscore(&ownerscore);

        // 确定使用哪套装备解析心法
        let effective_suit = suit_index
            .and_then(|idx| {
                if idx >= 1 && idx <= 4 {
                    Some(idx)
                } else {
                    None
                }
            })
            .or_else(|| {
                scores
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, &score)| score)
                    .map(|(i, _)| (i + 1) as i32)
                    .filter(|&idx| idx >= 1 && idx <= 4 && scores[(idx - 1) as usize] > 0)
            });

        if let Some(suit) = effective_suit {
            equip_query_params.push((owner_key, suit));
        }

        role_basic_infos.push(RoleBasicInfo {
            owner_key,
            owner_name,
            server_name,
            force_id,
            level,
            scores,
            effective_suit,
        });
    }

    // 第二步：批量查询所有角色对应套装的装备描述
    use std::collections::HashMap;
    let mut equip_descs_map: HashMap<(i64, i32), Vec<String>> = HashMap::new();

    if !equip_query_params.is_empty() {
        // 构建批量查询 SQL
        let placeholders: Vec<String> = equip_query_params
            .iter()
            .map(|_| "(ownerkey = ? AND suitindex = ? AND boxtype = 0)".to_string())
            .collect();
        let sql = format!(
            "SELECT ownerkey, suitindex, `desc` FROM EquipItems WHERE {}",
            placeholders.join(" OR ")
        );

        let mut stmt = conn.prepare(&sql).map_err(|e| format!("准备装备查询失败: {}", e))?;

        let mut params: Vec<String> = Vec::with_capacity(equip_query_params.len() * 2);
        for (owner_key, suit_index) in &equip_query_params {
            params.push(owner_key.to_string());
            params.push(suit_index.to_string());
        }

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            params.iter().map(|p| p as &dyn rusqlite::ToSql).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let ownerkey_str: String = row.get(0)?;
            let suitindex: i32 = row.get(1)?;
            let desc: String = row.get(2)?;
            Ok((ownerkey_str, suitindex, desc))
        }).map_err(|e| format!("查询装备描述失败: {}", e))?;

        for row in rows {
            if let Ok((ownerkey_str, suitindex, desc)) = row {
                if let Ok(ok) = ownerkey_str.parse::<i64>() {
                    equip_descs_map
                        .entry((ok, suitindex))
                        .or_insert_with(Vec::new)
                        .push(desc);
                }
            }
        }
    }

    // 第三步：处理每个角色，解析心法
    let mut roles = Vec::new();
    for role_info in role_basic_infos {
        let normalized_name = normalize_mingyi_role_name(&role_info.owner_name, &role_info.server_name);

        // 从预加载的装备描述中解析心法
        let (kungfu_id, kungfu_name) = if let Some(suit) = role_info.effective_suit {
            let descs = equip_descs_map.get(&(role_info.owner_key, suit)).cloned().unwrap_or_default();
            resolve_kungfu_from_descs(&descs, role_info.force_id)
                .map(|(k_id, k_name)| (Some(k_id), Some(k_name)))
                .unwrap_or((None, None))
        } else {
            (None, None)
        };

        roles.push(MingYiRoleInfo {
            owner_name: normalized_name,
            server_name: role_info.server_name,
            force_id: role_info.force_id,
            level: role_info.level,
            scores: role_info.scores,
            effective_suit: role_info.effective_suit.unwrap_or(1),
            kungfu_id,
            kungfu_name,
        });
    }

    Ok(roles)
}

// 根据角色名称和服务器查找茗伊数据库中的角色信息
fn find_mingyi_role<'a>(
    mingyi_roles: &'a [MingYiRoleInfo],
    role_name: &str,
    server_name: &str,
) -> Option<&'a MingYiRoleInfo> {
    mingyi_roles
        .iter()
        .find(|r| r.owner_name == role_name && r.server_name == server_name)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRole {
    pub name: String,
    pub region: String,
    pub server: String,
    pub force_id: Option<i32>,       // 门派ID
    pub force_name: Option<String>,  // 门派名称
    pub kungfu_id: Option<i32>,      // 心法ID
    pub kungfu_name: Option<String>, // 心法名称
    pub level: Option<i32>,          // 角色等级
    pub score: Option<i32>,          // 装备分数（第一套）
    pub scores: Option<Vec<i32>>,    // 所有套装的装备分数 [第1套, 第2套, 第3套, 第4套]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedAccount {
    pub account_name: String,
    pub roles: Vec<ParsedRole>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GkpFileInfo {
    pub file_path: String,
    pub file_name: String,
    pub timestamp: i64,
    pub player_count: i32,
    pub map_name: String,
    pub role_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRole {
    pub name: String,
    pub server: String,
    pub region: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDirectoryScanResult {
    pub success: bool,
    pub accounts: Vec<ParsedAccount>,
    pub gkp_files: Vec<GkpFileInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathValidationDetails {
    pub checked_path: String,
    pub expected_structure: String,
    pub current_structure: String,
    pub missing_directories: Vec<String>,
    pub check_time: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePathValidationResult {
    pub is_valid: bool,
    pub message: String,
    pub details: Option<GamePathValidationDetails>,
}

fn trim_trailing_separators(path: &str) -> &str {
    path.trim_end_matches(['\\', '/'])
}

fn split_windows_path(path: &str) -> Vec<&str> {
    trim_trailing_separators(path)
        .split(['\\', '/'])
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn resolve_game_runtime_directory(path: &str) -> String {
    let trimmed = trim_trailing_separators(path.trim());
    if trimmed.is_empty() {
        return String::new();
    }

    let segments = split_windows_path(trimmed);
    if let Some(seasun_index) = segments
        .iter()
        .position(|segment| segment.eq_ignore_ascii_case("seasungame"))
    {
        let mut resolved: Vec<&str> = segments[..=seasun_index].to_vec();
        resolved.extend(GAME_RUNTIME_SUFFIX);
        return resolved.join("\\");
    }

    trimmed.to_string()
}

fn read_directory_entries(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    fs::read_dir(path)
        .map_err(|error| format!("读取目录失败: {} ({error})", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("遍历目录失败: {} ({error})", path.display()))
}

fn parse_gkp_file_name(file_name: &str) -> Option<(i64, i32, String)> {
    let suffix = ".gkp.jx3dat";
    let stripped = file_name.strip_suffix(suffix)?;
    let (timestamp_str, rest) = stripped.split_once('_')?;
    let people_marker = "人";
    let marker_index = rest.find(people_marker)?;
    let player_count = rest[..marker_index].parse::<i32>().ok()?;
    let map_name = rest[marker_index + people_marker.len()..]
        .trim()
        .to_string();
    if map_name.is_empty() {
        return None;
    }

    let naive = NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d-%H-%M-%S").ok()?;
    Some((naive.and_utc().timestamp_millis(), player_count, map_name))
}

fn scan_userdata_directory(
    game_directory: &Path,
    mingyi_roles: &[MingYiRoleInfo],
) -> Result<Vec<ParsedAccount>, String> {
    let userdata_path = game_directory.join(USERDATA_BASE_PATH);
    let mut accounts = Vec::new();

    for account_entry in read_directory_entries(&userdata_path)? {
        let Ok(file_type) = account_entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let account_name = account_entry.file_name().to_string_lossy().to_string();
        let mut roles = Vec::new();
        let account_path = account_entry.path();

        for region_entry in read_directory_entries(&account_path).unwrap_or_default() {
            let Ok(region_type) = region_entry.file_type() else {
                continue;
            };
            if !region_type.is_dir() {
                continue;
            }

            let region_name = region_entry.file_name().to_string_lossy().to_string();
            let region_path = region_entry.path();

            for server_entry in read_directory_entries(&region_path).unwrap_or_default() {
                let Ok(server_type) = server_entry.file_type() else {
                    continue;
                };
                if !server_type.is_dir() {
                    continue;
                }

                let server_name = server_entry.file_name().to_string_lossy().to_string();
                let server_path = server_entry.path();

                for role_entry in read_directory_entries(&server_path).unwrap_or_default() {
                    let Ok(role_type) = role_entry.file_type() else {
                        continue;
                    };
                    if !role_type.is_dir() {
                        continue;
                    }

                    let role_name = role_entry.file_name().to_string_lossy().to_string();

                    // 尝试从茗伊数据库获取职业和装备分数信息
                    let mingyi_info = find_mingyi_role(mingyi_roles, &role_name, &server_name);

                    // 获取当前套装对应的装分（effective_suit 指向当前使用的套装）
                    let current_score = mingyi_info
                        .as_ref()
                        .and_then(|info| {
                            info.scores.get((info.effective_suit - 1) as usize).copied()
                        })
                        .unwrap_or(0);

                    let (force_id, force_name, kungfu_id, kungfu_name, level, score, scores) =
                        if let Some(info) = mingyi_info {
                            (
                                Some(info.force_id),
                                kungfu_data::get_force_name(info.force_id),
                                info.kungfu_id,
                                info.kungfu_name.clone(),
                                Some(info.level),
                                Some(current_score),
                                Some(info.scores.clone()),
                            )
                        } else {
                            (None, None, None, None, None, None, None)
                        };

                    roles.push(ParsedRole {
                        name: role_name,
                        region: region_name.clone(),
                        server: server_name.clone(),
                        force_id,
                        force_name,
                        kungfu_id,
                        kungfu_name,
                        level,
                        score,
                        scores,
                    });
                }
            }
        }

        if !roles.is_empty() {
            accounts.push(ParsedAccount {
                account_name,
                roles,
            });
        }
    }

    Ok(accounts)
}

/// 简单的账号角色扫描，只获取基本信息，不包含门派/心法/装分
fn scan_userdata_directory_basic(game_directory: &Path) -> Result<Vec<ParsedAccount>, String> {
    let userdata_path = game_directory.join(USERDATA_BASE_PATH);
    let mut accounts = Vec::new();

    for account_entry in read_directory_entries(&userdata_path)? {
        let Ok(file_type) = account_entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let account_name = account_entry.file_name().to_string_lossy().to_string();
        let mut roles = Vec::new();
        let account_path = account_entry.path();

        for region_entry in read_directory_entries(&account_path).unwrap_or_default() {
            let Ok(region_type) = region_entry.file_type() else {
                continue;
            };
            if !region_type.is_dir() {
                continue;
            }

            let region_name = region_entry.file_name().to_string_lossy().to_string();
            let region_path = region_entry.path();

            for server_entry in read_directory_entries(&region_path).unwrap_or_default() {
                let Ok(server_type) = server_entry.file_type() else {
                    continue;
                };
                if !server_type.is_dir() {
                    continue;
                }

                let server_name = server_entry.file_name().to_string_lossy().to_string();
                let server_path = server_entry.path();

                for role_entry in read_directory_entries(&server_path).unwrap_or_default() {
                    let Ok(role_type) = role_entry.file_type() else {
                        continue;
                    };
                    if !role_type.is_dir() {
                        continue;
                    }

                    let role_name = role_entry.file_name().to_string_lossy().to_string();

                    // 只获取基本信息，不从茗伊数据库获取门派/心法/装分
                    roles.push(ParsedRole {
                        name: role_name,
                        region: region_name.clone(),
                        server: server_name.clone(),
                        force_id: None,
                        force_name: None,
                        kungfu_id: None,
                        kungfu_name: None,
                        level: None,
                        score: None,
                        scores: None,
                    });
                }
            }
        }

        if !roles.is_empty() {
            accounts.push(ParsedAccount {
                account_name,
                roles,
            });
        }
    }

    Ok(accounts)
}

fn scan_gkp_files_directory(
    game_directory: &Path,
    active_roles: &[ActiveRole],
) -> Result<Vec<GkpFileInfo>, String> {
    let gkp_base_path = game_directory.join(GKP_BASE_PATH);
    let mut files = Vec::new();

    for entry in read_directory_entries(&gkp_base_path)? {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let entry_name = entry.file_name().to_string_lossy().to_string();
        if !entry_name.ends_with("@zhcn_hd") {
            continue;
        }

        let user_dir_path = entry.path();
        let matched_role_name = if active_roles.is_empty() {
            None
        } else {
            read_directory_entries(&user_dir_path)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|sub_entry| {
                    let file_type = sub_entry.file_type().ok()?;
                    if !file_type.is_dir() {
                        return None;
                    }
                    let sub_name = sub_entry.file_name().to_string_lossy().to_string();
                    active_roles
                        .iter()
                        .find(|role| role.name == sub_name)
                        .map(|_| sub_name)
                })
                .next()
        };

        if !active_roles.is_empty() && matched_role_name.is_none() {
            continue;
        }

        let gkp_dir_path = user_dir_path.join("userdata").join("gkp");
        for gkp_entry in read_directory_entries(&gkp_dir_path).unwrap_or_default() {
            let Ok(gkp_type) = gkp_entry.file_type() else {
                continue;
            };
            if !gkp_type.is_file() {
                continue;
            }

            let file_name = gkp_entry.file_name().to_string_lossy().to_string();
            let Some((timestamp, player_count, map_name)) = parse_gkp_file_name(&file_name) else {
                continue;
            };

            files.push(GkpFileInfo {
                file_path: gkp_entry.path().display().to_string(),
                file_name,
                timestamp,
                player_count,
                map_name,
                role_name: matched_role_name.clone(),
            });
        }
    }

    Ok(files)
}

// 自动解析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoParseResult {
    pub success: bool,
    pub new_accounts: i32,     // 新增账号数
    pub updated_accounts: i32, // 更新账号数
    pub new_roles: i32,        // 新增角色数
    pub updated_roles: i32,    // 更新角色数
    pub error: Option<String>,
}

// 根据复合键生成确定性 UUID
fn generate_uuid_from_key(key: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    format!("{:016x}-auto-0000-0000-000000000000", hasher.finish())
}

// 自动解析游戏目录并将结果存入数据库
fn auto_parse_and_save(game_directory: &Path) -> Result<AutoParseResult, String> {
    let conn = db::init_db().map_err(|e| format!("数据库初始化失败: {}", e))?;
    let timestamp = db::get_local_timestamp();

    // 读取茗伊数据库
    let mingyi_roles = match read_mingyi_role_info(game_directory) {
        Ok(roles) => {
            log::info!("从茗伊数据库读取到 {} 个角色信息", roles.len());
            roles
        }
        Err(e) => {
            log::warn!("读取茗伊数据库失败: {}", e);
            Vec::new()
        }
    };

    // 扫描 userdata 目录
    let parsed_accounts = scan_userdata_directory(game_directory, &mingyi_roles)
        .map_err(|e| format!("扫描目录失败: {}", e))?;

    if parsed_accounts.is_empty() {
        return Ok(AutoParseResult {
            success: true,
            new_accounts: 0,
            updated_accounts: 0,
            new_roles: 0,
            updated_roles: 0,
            error: None,
        });
    }

    let mut new_accounts = 0;
    let updated_accounts = 0;
    let mut new_roles = 0;
    let updated_roles = 0;

    for parsed_account in &parsed_accounts {
        let account_id =
            generate_uuid_from_key(&format!("account:{}", parsed_account.account_name));

        // 检查账号是否存在（按 UUID 或按 account_name 双重检查）
        // 旧数据 UUID 是随机的，新 UUID 是确定性的，需要兼容
        let old_account_id: Option<String> = conn
            .query_row(
                "SELECT id FROM accounts WHERE account_name = ?",
                params![parsed_account.account_name.clone()],
                |row| row.get(0),
            )
            .ok();

        match old_account_id {
            Some(_) => {
                // 账号已存在，使用现有 ID，不改变其 UUID
            }
            None => {
                // 账号不存在，插入新账号
                conn.execute(
                    "INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at)
                     VALUES (?, ?, 'OWN', 0, ?, ?)",
                    params![account_id, parsed_account.account_name, timestamp, timestamp],
                )
                .map_err(|e| format!("插入账号失败: {}", e))?;
                new_accounts += 1;
            }
        }

        // 使用账号 ID（如果账号已存在则是其现有 ID，如果是新插入则是新生成的 ID）
        let account_id_to_use = old_account_id.unwrap_or_else(|| account_id);

        // 处理角色
        for parsed_role in &parsed_account.roles {
            let role_id = generate_uuid_from_key(&format!(
                "role:{}:{}:{}:{}",
                parsed_account.account_name,
                parsed_role.name,
                parsed_role.region,
                parsed_role.server
            ));

            // 按角色唯一键（account_id + name + server + region）查找旧记录
            let old_role_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM roles WHERE account_id = ? AND name = ? AND server = ? AND region = ?",
                    params![
                        account_id_to_use,
                        parsed_role.name.clone(),
                        parsed_role.server.clone(),
                        parsed_role.region.clone()
                    ],
                    |row| row.get(0),
                )
                .ok();

            match old_role_id {
                Some(ref existing_role_id) => {
                    // 检查茗伊数据库是否有该角色的信息
                    if parsed_role.kungfu_name.is_none() {
                        // 茗伊数据库中没有该角色，只修正 sect
                        let old_role_data: Option<(String, String)> = conn
                            .query_row(
                                "SELECT martial, sect FROM roles WHERE id = ?",
                                params![existing_role_id],
                                |row| {
                                    let martial: String = row.get(0)?;
                                    let sect: String = row.get(1)?;
                                    Ok((martial, sect))
                                },
                            )
                            .ok();

                        if let Some((old_martial, old_sect)) = old_role_data {
                            if !old_martial.is_empty() {
                                // martial 不为空，检查 sect 是否需要修正
                                if let Some(expected_sect) = kungfu_data::get_sect_by_martial(&old_martial) {
                                    if old_sect != expected_sect {
                                        // sect 不正确，修正 sect
                                        conn.execute(
                                            "UPDATE roles SET sect = ?, updated_at = ? WHERE id = ?",
                                            params![expected_sect, timestamp, existing_role_id],
                                        )
                                        .map_err(|e| format!("修正门派失败: {}", e))?;
                                        log::info!(
                                            "角色 {} 的门派已从 {} 修正为 {}",
                                            parsed_role.name,
                                            old_sect,
                                            expected_sect
                                        );
                                    }
                                }
                            }
                            // martial 为空或sect已正确，不做处理
                        }
                        // 跳过后续的通用更新逻辑
                        continue;
                    }

                    // 茗伊数据库中有该角色，正常更新门派、心法、装分
                    conn.execute(
                        "UPDATE roles SET sect = ?, martial = ?, equipment_score = ?, updated_at = ? WHERE id = ?",
                        params![
                            parsed_role.force_name.clone().unwrap_or_default(),
                            parsed_role.kungfu_name.clone().unwrap_or_default(),
                            parsed_role.score.unwrap_or(0) as i64,
                            timestamp,
                            existing_role_id,
                        ],
                    )
                    .map_err(|e| format!("更新角色信息失败: {}", e))?;
                }
                None => {
                    // 角色不存在，插入新角色
                    conn.execute(
                        "INSERT INTO roles (id, account_id, name, server, region, sect, martial, equipment_score, created_at, updated_at)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                        params![
                            role_id,
                            account_id_to_use,
                            parsed_role.name.clone(),
                            parsed_role.server.clone(),
                            parsed_role.region.clone(),
                            parsed_role.force_name.clone().unwrap_or_default(),
                            parsed_role.kungfu_name.clone().unwrap_or_default(),
                            parsed_role.score.unwrap_or(0) as i64,
                            timestamp,
                            timestamp,
                        ],
                    )
                    .map_err(|e| format!("插入角色失败: {}", e))?;
                    new_roles += 1;
                }
            }
        }
    }

    log::info!(
        "自动解析完成：新增 {} 个账号，新增 {} 个角色",
        new_accounts,
        new_roles
    );

    Ok(AutoParseResult {
        success: true,
        new_accounts,
        updated_accounts,
        new_roles,
        updated_roles,
        error: None,
    })
}

// 自动解析并保存到数据库的命令
#[tauri::command]
pub fn auto_parse_game_directory(game_directory: String) -> Result<AutoParseResult, String> {
    let runtime_game_directory = resolve_game_runtime_directory(&game_directory);
    let runtime_path = PathBuf::from(&runtime_game_directory);

    if runtime_path.to_string_lossy().is_empty() {
        return Ok(AutoParseResult {
            success: false,
            new_accounts: 0,
            updated_accounts: 0,
            new_roles: 0,
            updated_roles: 0,
            error: Some("非剑网三目录".to_string()),
        });
    }

    auto_parse_and_save(&runtime_path)
}

/// 导入本地账号 - 只从 userdata 目录导入账号和角色基本信息，不包含门派/心法/装分
#[tauri::command]
#[allow(dead_code)]
pub fn import_local_accounts(game_directory: String) -> Result<AutoParseResult, String> {
    let runtime_game_directory = resolve_game_runtime_directory(&game_directory);
    let runtime_path = PathBuf::from(&runtime_game_directory);

    if runtime_path.to_string_lossy().is_empty() {
        return Ok(AutoParseResult {
            success: false,
            new_accounts: 0,
            updated_accounts: 0,
            new_roles: 0,
            updated_roles: 0,
            error: Some("非剑网三目录".to_string()),
        });
    }

    let conn = db::init_db().map_err(|e| format!("数据库初始化失败: {}", e))?;
    let timestamp = db::get_local_timestamp();

    // 扫描 userdata 目录，只获取基本信息
    let parsed_accounts = scan_userdata_directory_basic(&runtime_path)
        .map_err(|e| format!("扫描目录失败: {}", e))?;

    if parsed_accounts.is_empty() {
        return Ok(AutoParseResult {
            success: true,
            new_accounts: 0,
            updated_accounts: 0,
            new_roles: 0,
            updated_roles: 0,
            error: None,
        });
    }

    let mut new_accounts = 0;
    let updated_accounts = 0;
    let mut new_roles = 0;
    let updated_roles = 0;

    for parsed_account in &parsed_accounts {
        let account_id = generate_uuid_from_key(&format!("account:{}", parsed_account.account_name));

        // 检查账号是否存在
        let old_account_id: Option<String> = conn
            .query_row(
                "SELECT id FROM accounts WHERE account_name = ?",
                params![parsed_account.account_name.clone()],
                |row| row.get(0),
            )
            .ok();

        match old_account_id {
            Some(_) => {
                // 账号已存在，不做任何更新
            }
            None => {
                // 账号不存在，插入新账号
                conn.execute(
                    "INSERT INTO accounts (id, account_name, account_type, sort_order, created_at, updated_at)
                     VALUES (?, ?, 'OWN', 0, ?, ?)",
                    params![account_id, parsed_account.account_name, timestamp, timestamp],
                )
                .map_err(|e| format!("插入账号失败: {}", e))?;
                new_accounts += 1;
            }
        }

        // 使用账号 ID（如果账号已存在则是其现有 ID，如果是新插入则是新生成的 ID）
        let account_id_to_use = old_account_id.unwrap_or_else(|| account_id);

        // 处理角色 - 只插入基本信息，不包含门派/心法/装分
        for parsed_role in &parsed_account.roles {
            let role_id = generate_uuid_from_key(&format!(
                "role:{}:{}:{}:{}",
                parsed_account.account_name,
                parsed_role.name,
                parsed_role.region,
                parsed_role.server
            ));

            // 按角色唯一键查找旧记录
            let old_role_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM roles WHERE account_id = ? AND name = ? AND server = ? AND region = ?",
                    params![
                        account_id_to_use,
                        parsed_role.name.clone(),
                        parsed_role.server.clone(),
                        parsed_role.region.clone()
                    ],
                    |row| row.get(0),
                )
                .ok();

            // 只处理新角色，不更新已有角色
            if old_role_id.is_none() {
                conn.execute(
                    "INSERT INTO roles (id, account_id, name, server, region, sect, martial, equipment_score, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, '', '', 0, ?, ?)",
                    params![
                        role_id,
                        account_id_to_use,
                        parsed_role.name.clone(),
                        parsed_role.server.clone(),
                        parsed_role.region.clone(),
                        timestamp,
                        timestamp,
                    ],
                )
                .map_err(|e| format!("插入角色失败: {}", e))?;
                new_roles += 1;
            }
        }
    }

    log::info!(
        "导入本地账号完成：新增 {} 个账号，新增 {} 个角色",
        new_accounts,
        new_roles
    );

    Ok(AutoParseResult {
        success: true,
        new_accounts,
        updated_accounts,
        new_roles,
        updated_roles,
        error: None,
    })
}

/// 角色分析 - 从茗伊数据库分析角色的门派、心法、装分并更新
#[tauri::command]
#[allow(dead_code)]
pub fn analyze_roles(game_directory: String) -> Result<AutoParseResult, String> {
    let runtime_game_directory = resolve_game_runtime_directory(&game_directory);
    let runtime_path = PathBuf::from(&runtime_game_directory);

    if runtime_path.to_string_lossy().is_empty() {
        return Ok(AutoParseResult {
            success: false,
            new_accounts: 0,
            updated_accounts: 0,
            new_roles: 0,
            updated_roles: 0,
            error: Some("非剑网三目录".to_string()),
        });
    }

    let conn = db::init_db().map_err(|e| format!("数据库初始化失败: {}", e))?;
    let timestamp = db::get_local_timestamp();

    // 读取茗伊数据库
    let mingyi_roles = match read_mingyi_role_info(&runtime_path) {
        Ok(roles) => {
            log::info!("从茗伊数据库读取到 {} 个角色信息", roles.len());
            roles
        }
        Err(e) => {
            log::warn!("读取茗伊数据库失败: {}", e);
            Vec::new()
        }
    };

    let updated_accounts = 0;
    let new_roles = 0;
    let mut updated_roles = 0;

    for mingyi_role in &mingyi_roles {
        // 按角色名和服务器查找已有角色
        let old_role_id: Option<String> = conn
            .query_row(
                "SELECT id FROM roles WHERE name = ? AND server = ?",
                params![mingyi_role.owner_name.clone(), mingyi_role.server_name.clone()],
                |row| row.get(0),
            )
            .ok();

        if let Some(existing_role_id) = old_role_id {
            // 更新已有角色的门派、心法、装分
            let force_name = kungfu_data::get_force_name(mingyi_role.force_id);
            let current_score = mingyi_role
                .scores
                .get((mingyi_role.effective_suit - 1) as usize)
                .copied()
                .unwrap_or(0);

            conn.execute(
                "UPDATE roles SET sect = ?, martial = ?, equipment_score = ?, updated_at = ? WHERE id = ?",
                params![
                    force_name.unwrap_or_default(),
                    mingyi_role.kungfu_name.clone().unwrap_or_default(),
                    current_score as i64,
                    timestamp,
                    existing_role_id,
                ],
            )
            .map_err(|e| format!("更新角色信息失败: {}", e))?;
            updated_roles += 1;
        }
        // 如果没有找到对应角色，不插入新角色（角色分析只更新已有角色）
    }

    log::info!(
        "角色分析完成：更新 {} 个角色",
        updated_roles
    );

    Ok(AutoParseResult {
        success: true,
        new_accounts: 0,
        updated_accounts,
        new_roles,
        updated_roles,
        error: None,
    })
}

#[tauri::command]
pub fn scan_game_directory(
    game_directory: String,
    active_roles: Option<Vec<ActiveRole>>,
) -> Result<GameDirectoryScanResult, String> {
    let runtime_game_directory = resolve_game_runtime_directory(&game_directory);
    let runtime_path = PathBuf::from(&runtime_game_directory);

    let mut errors = Vec::new();

    // 先读取茗伊数据库的角色信息
    let mingyi_roles = match read_mingyi_role_info(&runtime_path) {
        Ok(roles) => {
            log::info!("从茗伊数据库读取到 {} 个角色信息", roles.len());
            roles
        }
        Err(e) => {
            log::warn!("读取茗伊数据库失败: {}", e);
            Vec::new()
        }
    };

    let accounts = match scan_userdata_directory(&runtime_path, &mingyi_roles) {
        Ok(accounts) => accounts,
        Err(error) => {
            errors.push(error);
            Vec::new()
        }
    };

    let gkp_files =
        match scan_gkp_files_directory(&runtime_path, active_roles.as_deref().unwrap_or(&[])) {
            Ok(files) => files,
            Err(error) => {
                errors.push(error);
                Vec::new()
            }
        };

    Ok(GameDirectoryScanResult {
        success: true,
        accounts,
        gkp_files,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    })
}

#[tauri::command]
pub fn validate_game_directory(game_directory: String) -> Result<GamePathValidationResult, String> {
    let check_time = chrono::Utc::now().to_rfc3339();
    let runtime_path_string = resolve_game_runtime_directory(&game_directory);
    let expected_structure = r"SeasunGame\Game\JX3\bin\zhcn_hd".to_string();

    if runtime_path_string.is_empty() {
        return Ok(GamePathValidationResult {
            is_valid: false,
            message: "非剑网三目录".to_string(),
            details: Some(GamePathValidationDetails {
                checked_path: game_directory,
                expected_structure,
                current_structure: String::new(),
                missing_directories: vec!["SeasunGame".to_string()],
                check_time,
            }),
        });
    }

    let runtime_path = PathBuf::from(&runtime_path_string);
    if !runtime_path.is_absolute() {
        return Ok(GamePathValidationResult {
            is_valid: false,
            message: "非剑网三目录".to_string(),
            details: Some(GamePathValidationDetails {
                checked_path: game_directory,
                expected_structure,
                current_structure: runtime_path_string,
                missing_directories: vec!["SeasunGame".to_string()],
                check_time,
            }),
        });
    }

    let entries = match read_directory_entries(&runtime_path) {
        Ok(entries) => entries,
        Err(_) => {
            return Ok(GamePathValidationResult {
                is_valid: false,
                message: "非剑网三目录".to_string(),
                details: Some(GamePathValidationDetails {
                    checked_path: game_directory,
                    expected_structure,
                    current_structure: runtime_path.display().to_string(),
                    missing_directories: vec!["userdata".to_string(), "interface".to_string()],
                    check_time,
                }),
            })
        }
    };

    let existing_entries: Vec<String> = entries
        .iter()
        .map(|entry| entry.file_name().to_string_lossy().to_lowercase())
        .collect();
    let missing_directories = ["userdata", "interface"]
        .into_iter()
        .filter(|required| !existing_entries.iter().any(|entry| entry == required))
        .map(|value| value.to_string())
        .collect::<Vec<_>>();

    if !missing_directories.is_empty() {
        return Ok(GamePathValidationResult {
            is_valid: false,
            message: format!("目录结构不完整，请确认目录可自动补全到 {expected_structure}"),
            details: Some(GamePathValidationDetails {
                checked_path: game_directory,
                expected_structure,
                current_structure: runtime_path.display().to_string(),
                missing_directories,
                check_time,
            }),
        });
    }

    Ok(GamePathValidationResult {
        is_valid: true,
        message: "游戏目录验证成功".to_string(),
        details: Some(GamePathValidationDetails {
            checked_path: game_directory,
            expected_structure,
            current_structure: runtime_path.display().to_string(),
            missing_directories: Vec::new(),
            check_time,
        }),
    })
}

// 客户端类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ClientType {
    #[serde(rename = "zhcn_hd")]
    ZhcnHd, // 重制版正式服
    #[serde(rename = "zhcn_exp")]
    ZhcnExp, // 重制版测试服
    #[serde(rename = "zhcn_tw")]
    ZhcnTw, // 重制版国际服
    #[serde(rename = "classic_yq")]
    ClassicYq, // 缘起正式服
    #[serde(rename = "classic_exp")]
    ClassicExp, // 缘起测试服
}

impl ClientType {
    fn display_name(&self) -> &'static str {
        match self {
            ClientType::ZhcnHd => "重制版正式服",
            ClientType::ZhcnExp => "重制版测试服",
            ClientType::ZhcnTw => "重制版国际服",
            ClientType::ClassicYq => "缘起正式服",
            ClientType::ClassicExp => "缘起测试服",
        }
    }

    // 获取注册表路径和子键名
    fn reg_info(&self) -> (String, String, String) {
        match self {
            ClientType::ZhcnHd => (
                r"SOFTWARE\Kingsoft".to_string(),
                r"SeasunGame\JX3".to_string(),
                "JX3".to_string(),
            ),
            ClientType::ZhcnExp => (
                r"SOFTWARE\Kingsoft".to_string(),
                r"SeasunGame\JX3_EXP".to_string(),
                "JX3_EXP".to_string(),
            ),
            ClientType::ZhcnTw => (
                r"SOFTWARE\WOW6432Node\kingsoft\JX3".to_string(),
                r"zhcn_tw".to_string(),
                "zhcn_tw".to_string(),
            ),
            ClientType::ClassicYq => (
                r"SOFTWARE\Kingsoft".to_string(),
                r"SeasunGame\JX3_CLASSIC".to_string(),
                "JX3_CLASSIC".to_string(),
            ),
            ClientType::ClassicExp => (
                r"SOFTWARE\Kingsoft".to_string(),
                r"SeasunGame\JX3_CLASSIC_EXP".to_string(),
                "JX3_CLASSIC_EXP".to_string(),
            ),
        }
    }

    // 获取游戏工作目录（bin/{client_name}）
    fn get_workdir(&self, install_path: &Path) -> Option<PathBuf> {
        let client_name = match self {
            ClientType::ZhcnHd => "zhcn_hd",
            ClientType::ZhcnExp => "zhcn_exp",
            ClientType::ZhcnTw => "zhcn_tw",
            ClientType::ClassicYq => "classic_yq",
            ClientType::ClassicExp => "classic_exp",
        };
        let workdir = install_path.join("bin").join(client_name);
        if workdir.is_dir() {
            Some(workdir)
        } else {
            None
        }
    }
}

// 扫描到的客户端信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Jx3ClientInfo {
    pub client_type: String,
    pub display_name: String,
    pub install_path: String,
    pub work_directory: String,
    pub version: Option<String>,
}

// 扫描结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanClientsResult {
    pub success: bool,
    pub clients: Vec<Jx3ClientInfo>,
    pub error: Option<String>,
}

// 从注册表读取字符串值
fn read_registry_string(hkey: &RegKey, subkey: &str, value_name: &str) -> Option<String> {
    hkey.open_subkey(subkey)
        .ok()?
        .enum_values()
        .filter_map(|v| v.ok())
        .find(|(name, _)| name.eq_ignore_ascii_case(value_name))
        .and_then(|(_, value)| match value {
            winreg::RegValue { bytes, .. } => {
                // 移除末尾的空字节并转换为字符串
                let s = String::from_utf8_lossy(&bytes);
                Some(s.trim_end_matches('\0').to_string())
            }
        })
        .filter(|s| !s.is_empty())
}

// 扫描单个客户端
fn scan_single_client(client_type: &ClientType) -> Option<Jx3ClientInfo> {
    let (reg_parent, _reg_subkey, _reg_name) = client_type.reg_info();

    // 尝试从 HKLM 读取
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);

    // 尝试 XLauncherV2 路径（新启动器）
    let xlauncher_path = format!(r"{}\SeasunGame\{}", reg_parent, _reg_name);
    let install_path: String = read_registry_string(&hklm, &xlauncher_path, "InstallPath")
        .or_else(|| {
            // 回退到旧路径
            read_registry_string(
                &hklm,
                &format!(r"{}\{}", reg_parent, _reg_name),
                "InstallPath",
            )
        })?;

    let install_path = PathBuf::from(&install_path);
    if !install_path.exists() {
        return None;
    }

    // 获取工作目录
    let work_directory = client_type
        .get_workdir(&install_path)
        .map(|p| p.display().to_string())?;

    // 获取版本号（从 version.cfg 读取）
    let version = if let Ok(version_cfg) =
        std::fs::read_to_string(PathBuf::from(&work_directory).join("version.cfg"))
    {
        // 解析 version.cfg 中的版本号
        // 格式: [Version]\nSword3.version=3-0-123-45678
        version_cfg
            .lines()
            .find(|line| line.starts_with("Sword3.version="))
            .map(|line| line.trim_start_matches("Sword3.version=").to_string())
    } else {
        None
    };

    Some(Jx3ClientInfo {
        client_type: serde_json::to_string(client_type)
            .ok()?
            .trim_matches('"')
            .to_string(),
        display_name: client_type.display_name().to_string(),
        install_path: install_path.display().to_string(),
        work_directory,
        version,
    })
}

// 扫描所有已安装的剑网3客户端
#[tauri::command]
pub fn scan_jx3_clients() -> ScanClientsResult {
    let client_types = vec![
        ClientType::ZhcnHd,
        ClientType::ZhcnExp,
        ClientType::ZhcnTw,
        ClientType::ClassicYq,
        ClientType::ClassicExp,
    ];

    let mut clients = Vec::new();
    let mut errors = Vec::new();

    for client_type in client_types {
        match scan_single_client(&client_type) {
            Some(info) => clients.push(info),
            None => {
                // 单个客户端不存在不记录错误，这是正常情况
                log::debug!("未检测到客户端: {:?}", client_type);
            }
        }
    }

    if clients.is_empty() {
        errors.push("未在注册表中找到任何已安装的剑网3客户端".to_string());
    }

    ScanClientsResult {
        success: !clients.is_empty(),
        clients,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
    }
}
