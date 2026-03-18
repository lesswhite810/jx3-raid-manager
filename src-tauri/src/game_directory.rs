use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const GKP_BASE_PATH: &str = r"interface\my#data";
const USERDATA_BASE_PATH: &str = "userdata";
const GAME_RUNTIME_SUFFIX: [&str; 4] = ["Game", "JX3", "bin", "zhcn_hd"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedRole {
    pub name: String,
    pub region: String,
    pub server: String,
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
    let map_name = rest[marker_index + people_marker.len()..].trim().to_string();
    if map_name.is_empty() {
        return None;
    }

    let naive = NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d-%H-%M-%S").ok()?;
    Some((naive.and_utc().timestamp_millis(), player_count, map_name))
}

fn scan_userdata_directory(game_directory: &Path) -> Result<Vec<ParsedAccount>, String> {
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

                    roles.push(ParsedRole {
                        name: role_entry.file_name().to_string_lossy().to_string(),
                        region: region_name.clone(),
                        server: server_name.clone(),
                    });
                }
            }
        }

        if !roles.is_empty() {
            accounts.push(ParsedAccount { account_name, roles });
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

#[tauri::command]
pub fn scan_game_directory(
    game_directory: String,
    active_roles: Option<Vec<ActiveRole>>,
) -> Result<GameDirectoryScanResult, String> {
    let runtime_game_directory = resolve_game_runtime_directory(&game_directory);
    let runtime_path = PathBuf::from(&runtime_game_directory);

    let mut errors = Vec::new();
    let accounts = match scan_userdata_directory(&runtime_path) {
        Ok(accounts) => accounts,
        Err(error) => {
            errors.push(error);
            Vec::new()
        }
    };

    let gkp_files = match scan_gkp_files_directory(&runtime_path, active_roles.as_deref().unwrap_or(&[]))
    {
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
