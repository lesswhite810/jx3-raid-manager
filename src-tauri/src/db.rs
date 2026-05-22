use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::kungfu_data::get_sect_by_martial;
use crate::runtime_mode::{self, RuntimeMode};

mod migration;
pub mod migrations;

#[cfg(test)]
mod upgrade_tests;

const DATABASE_NAME: &str = "jx3-raid-manager.db";
const LOG_FILE_NAME: &str = "jx3-raid-manager.log";
const DATA_DIR_BOOTSTRAP_FILE: &str = "data-dir.json";
const DATA_DIR_INSTALLER_STATE_FILE: &str = "data-dir.ini";

/// 当前数据库 schema 版本
pub const CURRENT_SCHEMA_VERSION: i32 = 13;

/// 数据库连接单例
static DB_INITIALIZED: Mutex<bool> = Mutex::new(false);

/// 获取当前本地时间的 RFC3339 格式字符串
pub fn get_local_timestamp() -> String {
    chrono::Local::now().to_rfc3339()
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataDirBootstrapConfig {
    custom_data_dir: Option<String>,
    pending_migration_from: Option<String>,
}

/// 检测是否为安装版（通过检查同目录下是否有 uninstall.exe）
fn is_install_mode() -> bool {
    matches!(
        runtime_mode::detect_current_runtime_mode(),
        Ok(RuntimeMode::Installer)
    )
}

/// 获取安装目录（仅在安装版中有效）
fn get_install_dir() -> Option<PathBuf> {
    runtime_mode::current_executable_dir().ok()
}

fn ensure_directory_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        if path.is_dir() {
            return Ok(());
        }
        return Err(format!("目标路径不是目录: {}", path.display()));
    }

    fs::create_dir_all(path).map_err(|e| format!("无法创建目录 {}: {}", path.display(), e))
}

fn get_home_app_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;
    Ok(home_dir.join(".jx3-raid-manager"))
}

fn get_data_dir_bootstrap_path() -> Result<PathBuf, String> {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::config_local_dir)
        .ok_or_else(|| "无法获取本地配置目录".to_string())?;

    let bootstrap_dir = base_dir.join("jx3-raid-manager");
    ensure_directory_exists(&bootstrap_dir)?;
    Ok(bootstrap_dir.join(DATA_DIR_BOOTSTRAP_FILE))
}

fn get_data_dir_installer_state_path() -> Result<PathBuf, String> {
    let bootstrap_path = get_data_dir_bootstrap_path()?;
    let bootstrap_dir = bootstrap_path
        .parent()
        .ok_or_else(|| "无法获取数据目录状态文件所在目录".to_string())?;
    Ok(bootstrap_dir.join(DATA_DIR_INSTALLER_STATE_FILE))
}

fn read_data_dir_bootstrap_config() -> Result<DataDirBootstrapConfig, String> {
    let config_path = get_data_dir_bootstrap_path()?;
    if !config_path.exists() {
        return Ok(DataDirBootstrapConfig::default());
    }

    let content =
        fs::read_to_string(&config_path).map_err(|e| format!("读取数据目录配置失败: {}", e))?;

    serde_json::from_str::<DataDirBootstrapConfig>(&content)
        .map_err(|e| format!("解析数据目录配置失败: {}", e))
}

fn write_data_dir_bootstrap_config(config: &DataDirBootstrapConfig) -> Result<(), String> {
    let config_path = get_data_dir_bootstrap_path()?;
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("序列化数据目录配置失败: {}", e))?;
    fs::write(&config_path, content).map_err(|e| format!("写入数据目录配置失败: {}", e))?;
    sync_data_dir_installer_state(config)
}

fn get_custom_data_dir(config: &DataDirBootstrapConfig) -> Option<PathBuf> {
    config.custom_data_dir.as_ref().and_then(|path| {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    })
}

fn canonicalize_existing_directory(path: &Path) -> Result<PathBuf, String> {
    ensure_directory_exists(path)?;
    fs::canonicalize(path).map_err(|e| format!("解析目录路径失败 {}: {}", path.display(), e))
}

fn directories_match(left: &Path, right: &Path) -> Result<bool, String> {
    let left_path = canonicalize_existing_directory(left)?;
    let right_path = canonicalize_existing_directory(right)?;
    Ok(left_path == right_path)
}

fn matches_managed_file_prefix(file_name: &str, prefix: &str) -> bool {
    file_name == prefix
        || file_name.starts_with(&format!("{prefix}."))
        || file_name.starts_with(&format!("{prefix}-"))
}

fn is_managed_app_data_file_name(file_name: &str) -> bool {
    matches_managed_file_prefix(file_name, DATABASE_NAME)
        || matches_managed_file_prefix(file_name, LOG_FILE_NAME)
}

fn get_managed_app_data_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    if !path.is_dir() {
        return Err(format!("数据目录不是文件夹: {}", path.display()));
    }

    let mut files = Vec::new();
    for entry in
        fs::read_dir(path).map_err(|e| format!("读取目录失败 {}: {}", path.display(), e))?
    {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let entry_path = entry.path();
        if !entry_path.is_file() {
            continue;
        }

        let Some(file_name) = entry.file_name().to_str().map(|value| value.to_string()) else {
            continue;
        };

        if is_managed_app_data_file_name(&file_name) {
            files.push(entry_path);
        }
    }

    files.sort();
    Ok(files)
}

fn has_persisted_app_data(path: &Path) -> bool {
    get_managed_app_data_files(path)
        .map(|files| !files.is_empty())
        .unwrap_or(false)
}

fn files_are_identical(left: &Path, right: &Path) -> Result<bool, String> {
    let left_metadata =
        fs::metadata(left).map_err(|e| format!("读取文件信息失败 {}: {}", left.display(), e))?;
    let right_metadata =
        fs::metadata(right).map_err(|e| format!("读取文件信息失败 {}: {}", right.display(), e))?;

    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let mut left_file =
        fs::File::open(left).map_err(|e| format!("打开文件失败 {}: {}", left.display(), e))?;
    let mut right_file =
        fs::File::open(right).map_err(|e| format!("打开文件失败 {}: {}", right.display(), e))?;

    let mut left_buffer = [0_u8; 8 * 1024];
    let mut right_buffer = [0_u8; 8 * 1024];

    loop {
        let left_read = left_file
            .read(&mut left_buffer)
            .map_err(|e| format!("读取文件失败 {}: {}", left.display(), e))?;
        let right_read = right_file
            .read(&mut right_buffer)
            .map_err(|e| format!("读取文件失败 {}: {}", right.display(), e))?;

        if left_read != right_read {
            return Ok(false);
        }

        if left_read == 0 {
            return Ok(true);
        }

        if left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
    }
}

fn target_file_is_authoritative(source: &Path, target: &Path) -> Result<bool, String> {
    if files_are_identical(source, target)? {
        return Ok(true);
    }

    let source_modified = fs::metadata(source)
        .map_err(|e| format!("读取文件信息失败 {}: {}", source.display(), e))?
        .modified()
        .ok();
    let target_modified = fs::metadata(target)
        .map_err(|e| format!("读取文件信息失败 {}: {}", target.display(), e))?
        .modified()
        .ok();

    Ok(matches!(
        (source_modified, target_modified),
        (Some(source_time), Some(target_time)) if target_time >= source_time
    ))
}

fn move_file_with_fallback(source: &Path, target: &Path) -> Result<(), String> {
    match fs::rename(source, target) {
        Ok(_) => Ok(()),
        Err(_) => {
            fs::copy(source, target).map_err(|e| {
                format!(
                    "复制数据文件失败: {} -> {} ({})",
                    source.display(),
                    target.display(),
                    e
                )
            })?;
            fs::remove_file(source)
                .map_err(|e| format!("删除源文件失败 {}: {}", source.display(), e))
        }
    }
}

fn migrate_managed_app_data_files(source: &Path, target: &Path) -> Result<bool, String> {
    if !source.exists() {
        return Ok(false);
    }

    ensure_directory_exists(target)?;
    if directories_match(source, target)? {
        return Ok(false);
    }

    let managed_files = get_managed_app_data_files(source)?;
    let mut changed = false;

    for source_path in managed_files {
        let file_name = source_path
            .file_name()
            .ok_or_else(|| format!("无法获取源文件名: {}", source_path.display()))?;
        let target_path = target.join(file_name);

        if target_path.exists() {
            if target_file_is_authoritative(&source_path, &target_path)? {
                fs::remove_file(&source_path)
                    .map_err(|e| format!("删除源文件失败 {}: {}", source_path.display(), e))?;
                changed = true;
                log::info!("已清理旧数据文件: {:?} -> {:?}", source_path, target_path);
            } else {
                log::warn!(
                    "跳过冲突数据文件迁移，目标目录已存在无法安全覆盖的文件: {:?} -> {:?}",
                    source_path,
                    target_path
                );
            }
            continue;
        }

        move_file_with_fallback(&source_path, &target_path)?;
        changed = true;
        log::info!("已迁移数据文件: {:?} -> {:?}", source_path, target_path);
    }

    Ok(changed)
}

fn can_clear_pending_migration(source: &Path, target: &Path) -> Result<bool, String> {
    if source.as_os_str().is_empty() || !source.exists() {
        return Ok(true);
    }

    if directories_match(source, target)? {
        return Ok(true);
    }

    Ok(!has_persisted_app_data(source))
}

fn resolve_target_app_dir(
    config: &DataDirBootstrapConfig,
) -> Result<(PathBuf, String, bool), String> {
    if let Some(custom_dir) = get_custom_data_dir(config) {
        return Ok((custom_dir, "custom".to_string(), is_install_mode()));
    }

    if is_install_mode() {
        let install_dir = get_install_dir().ok_or_else(|| "无法获取安装目录".to_string())?;
        return Ok((install_dir, "install".to_string(), true));
    }

    Ok((get_home_app_dir()?, "user_home".to_string(), false))
}

fn resolve_effective_app_dir(config: &DataDirBootstrapConfig, target_dir: &Path) -> PathBuf {
    config
        .pending_migration_from
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| target_dir.to_path_buf())
}

fn write_data_dir_installer_state(
    config: &DataDirBootstrapConfig,
    target_dir: &Path,
    location: &str,
) -> Result<(), String> {
    let installer_state_path = get_data_dir_installer_state_path()?;
    let effective_dir = resolve_effective_app_dir(config, target_dir);
    let custom_dir = get_custom_data_dir(config)
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let pending_migration_from = config
        .pending_migration_from
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    let content = format!(
        "[data]\nlocation={}\neffectiveDataDir={}\nresolvedTargetDir={}\ncustomDataDir={}\npendingMigrationFrom={}\n",
        location,
        effective_dir.to_string_lossy(),
        target_dir.to_string_lossy(),
        custom_dir,
        pending_migration_from
    );

    fs::write(&installer_state_path, content)
        .map_err(|e| format!("写入安装器数据目录状态失败: {}", e))
}

fn sync_data_dir_installer_state(config: &DataDirBootstrapConfig) -> Result<(), String> {
    let (target_dir, location, _) = resolve_target_app_dir(config)?;
    write_data_dir_installer_state(config, &target_dir, &location)
}

fn read_previous_effective_data_dir() -> Result<Option<PathBuf>, String> {
    let state_path = get_data_dir_installer_state_path()?;
    if !state_path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&state_path).map_err(|e| format!("读取安装器状态文件失败: {}", e))?;

    for line in content.lines() {
        if let Some(value) = line.strip_prefix("effectiveDataDir=") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                let path = PathBuf::from(trimmed);
                if path.exists() && has_persisted_app_data(&path) {
                    return Ok(Some(path));
                }
            }
            break;
        }
    }

    Ok(None)
}

fn maybe_migrate_app_data(
    target_dir: &Path,
    config: &mut DataDirBootstrapConfig,
) -> Result<(), String> {
    let mut migration_sources: Vec<PathBuf> = Vec::new();

    if let Some(source) = config.pending_migration_from.as_ref() {
        let source_path = PathBuf::from(source.trim());
        if !source_path.as_os_str().is_empty() {
            migration_sources.push(source_path);
        }
    }

    let home_dir = get_home_app_dir()?;
    if home_dir != target_dir {
        migration_sources.push(home_dir);
    }

    if let Some(previous_dir) = read_previous_effective_data_dir()? {
        if previous_dir != *target_dir && !migration_sources.contains(&previous_dir) {
            log::info!("检测到历史数据目录 {:?}，加入迁移源", previous_dir);
            migration_sources.push(previous_dir);
        }
    }

    let mut migrated = false;
    for source_dir in migration_sources {
        if !source_dir.exists() {
            continue;
        }

        if directories_match(&source_dir, target_dir)? || !has_persisted_app_data(&source_dir) {
            continue;
        }

        if migrate_managed_app_data_files(&source_dir, target_dir)? {
            migrated = true;
            log::info!("已迁移数据目录内容: {:?} -> {:?}", source_dir, target_dir);
        }
    }

    let should_clear_pending = if let Some(source) = config.pending_migration_from.as_ref() {
        can_clear_pending_migration(Path::new(source.trim()), target_dir)?
    } else {
        false
    };

    if should_clear_pending {
        config.pending_migration_from = None;
        write_data_dir_bootstrap_config(config)?;
    } else if migrated && config.pending_migration_from.is_some() {
        log::warn!("数据迁移后源目录仍保留文件，保留待迁移状态以便后续继续处理");
    }
    Ok(())
}

pub fn get_app_dir() -> Result<PathBuf, String> {
    let mut config = read_data_dir_bootstrap_config()?;
    let (target_dir, location, _) = resolve_target_app_dir(&config)?;
    ensure_directory_exists(&target_dir)?;
    maybe_migrate_app_data(&target_dir, &mut config)?;
    write_data_dir_installer_state(&config, &target_dir, &location)?;
    Ok(target_dir)
}

/// 获取当前数据目录信息
pub fn get_app_dir_info() -> Result<(PathBuf, String, bool, bool), String> {
    let config = read_data_dir_bootstrap_config()?;
    let (target_dir, location, is_install) = resolve_target_app_dir(&config)?;
    write_data_dir_installer_state(&config, &target_dir, &location)?;
    Ok((
        target_dir,
        location,
        is_install,
        get_custom_data_dir(&config).is_some(),
    ))
}

pub fn get_db_path() -> Result<PathBuf, String> {
    let app_dir = get_app_dir()?;
    Ok(app_dir.join(DATABASE_NAME))
}

#[derive(serde::Serialize)]
pub struct DirectoryDeleteResult {
    pub deleted: bool,
    pub path: String,
}

fn format_directory_delete_message(target_type: &str, display_path: &str, deleted: bool) -> String {
    if deleted {
        format!("已删除{target_type}目录: {display_path}")
    } else {
        format!("未找到{target_type}目录，跳过删除: {display_path}")
    }
}

#[derive(Debug)]
struct RecordLookupMetadata {
    raid_name: Option<String>,
    account_id: Option<String>,
    role_id: Option<String>,
    record_date: Option<i64>,
    record_type: String,
}

fn record_lookup_metadata_from_json(record: &serde_json::Value) -> RecordLookupMetadata {
    RecordLookupMetadata {
        raid_name: record["raidName"].as_str().map(|value| value.to_string()),
        account_id: record["accountId"].as_str().map(|value| value.to_string()),
        role_id: record["roleId"].as_str().map(|value| value.to_string()),
        record_date: parse_record_date(&record["date"]),
        record_type: record["type"]
            .as_str()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "raid".to_string()),
    }
}

fn parse_record_date(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value.as_str().and_then(|date| {
            date.parse::<i64>().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc3339(date)
                    .ok()
                    .map(|parsed| parsed.timestamp_millis())
            })
        })
    })
}

#[tauri::command]
pub fn db_delete_directory(
    path: String,
    target_type: String,
) -> Result<DirectoryDeleteResult, String> {
    let target_path = PathBuf::from(path.trim());
    let display_path = target_path.display().to_string();

    if !target_path.exists() {
        log::debug!(
            "{}",
            format_directory_delete_message(&target_type, &display_path, false)
        );
        return Ok(DirectoryDeleteResult {
            deleted: false,
            path: display_path,
        });
    }

    if !Path::new(&target_path).is_dir() {
        let error_message = format!("目标路径不是目录: {display_path}");
        log::error!("{}", error_message);
        return Err(error_message);
    }

    std::fs::remove_dir_all(&target_path).map_err(|error| {
        let error_message = format!("删除{target_type}目录失败: {display_path} ({error})");
        log::error!("{}", error_message);
        error_message
    })?;

    log::info!(
        "{}",
        format_directory_delete_message(&target_type, &display_path, true)
    );

    Ok(DirectoryDeleteResult {
        deleted: true,
        path: display_path,
    })
}

/// 初始化数据库（单例模式，只初始化一次）
///
/// 安装与升级路径完全分离：
/// - 安装路径：数据库不存在时，直接创建最新版本的完整结构
/// - 升级路径：数据库存在但版本较低时，自动识别版本并按序执行增量迁移
pub fn init_db() -> Result<Connection, String> {
    let mut initialized = DB_INITIALIZED.lock().map_err(|e| e.to_string())?;

    if *initialized {
        return Connection::open(get_db_path()?).map_err(|e| e.to_string());
    }

    let path = get_db_path()?;
    let db_exists = path.exists();

    log::info!(
        "[INIT] 数据库初始化开始，DB路径: {:?}, db_exists: {}",
        path, db_exists
    );

    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")
        .ok();

    ensure_version_tables(&conn)?;

    let current_version = get_schema_version(&conn)?;
    log::debug!(
        "[INIT] 当前数据库版本: {}, 目标版本: {}",
        current_version, CURRENT_SCHEMA_VERSION
    );

    if !db_exists {
        install_fresh_db(&conn)?;
    } else if current_version > CURRENT_SCHEMA_VERSION {
        log::warn!(
            "[INIT] 数据库版本 V{} 高于应用版本 V{}，可能是应用降级导致，将执行表结构修复",
            current_version, CURRENT_SCHEMA_VERSION
        );
        ensure_baseline_tables(&conn)?;
        ensure_equipment_columns(&conn)?;
        ensure_critical_columns(&conn)?;
        log::info!("[INIT] 表结构修复完成，数据库版本保持 V{}", current_version);
    } else if current_version < CURRENT_SCHEMA_VERSION {
        upgrade_db(&conn, current_version)?;
    } else {
        log::info!("[INIT] 已是最新版本 V{}，无需迁移", current_version);
    }

    migration::init_static_raids(&conn)?;
    ensure_equipment_columns(&conn)?;

    *initialized = true;
    log::info!("[INIT] 数据库初始化完成，当前版本 V{}", CURRENT_SCHEMA_VERSION);

    Ok(conn)
}

/// 使用指定路径初始化数据库（用于测试）
///
/// 此函数用于测试场景，允许指定数据库路径进行初始化和升级。
#[cfg(test)]
pub fn init_db_with_path(path: &std::path::Path) -> Result<Connection, String> {
    let db_exists = path.exists();

    log::info!(
        "[INIT_TEST] 数据库初始化开始，DB路径: {:?}, db_exists: {}",
        path, db_exists
    );

    let conn = Connection::open(path).map_err(|e| e.to_string())?;

    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")
        .ok();

    ensure_version_tables(&conn)?;

    let current_version = get_schema_version(&conn)?;
    log::debug!(
        "[INIT_TEST] 当前数据库版本: {}, 目标版本: {}",
        current_version, CURRENT_SCHEMA_VERSION
    );

    if !db_exists {
        install_fresh_db(&conn)?;
    } else if current_version < CURRENT_SCHEMA_VERSION {
        upgrade_db(&conn, current_version)?;
    } else {
        log::info!("[INIT_TEST] 已是最新版本 V{}，无需迁移", current_version);
    }

    migration::init_static_raids(&conn)?;
    ensure_equipment_columns(&conn)?;

    log::info!("[INIT_TEST] 数据库初始化完成，当前版本 V{}", CURRENT_SCHEMA_VERSION);

    Ok(conn)
}

/// 确保版本追踪表存在（安装与升级共用）
fn ensure_version_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_versions (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL,
            description TEXT
        );

        CREATE TABLE IF NOT EXISTS migration_flags (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );
    "#,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 安装路径：全新安装，直接创建最新版本的完整数据库结构
///
/// 此函数仅在数据库文件不存在时调用，一次性创建所有表、索引和初始数据，
/// 无需执行增量迁移脚本，确保新安装用户体验最快启动速度。
fn install_fresh_db(conn: &Connection) -> Result<(), String> {
    log::info!(
        "[INSTALL] 全新安装，创建最新版本结构 V{}",
        CURRENT_SCHEMA_VERSION
    );

    create_latest_schema(conn)?;
    set_schema_version(conn, CURRENT_SCHEMA_VERSION, "初始安装")?;
    migration::init_static_raids(conn)?;

    log::info!("[INSTALL] 全新安装完成");
    Ok(())
}

/// 升级路径：从当前版本升级到最新版本
///
/// 流程：
/// 1. 升级前验证（完整性检查、版本范围校验）
/// 2. 确保基线表存在（仅 V0 旧数据库需要）
/// 3. 按序执行增量迁移脚本（每个版本独立事务）
/// 4. 确保装备表列完整（历史兼容）
/// 5. 初始化静态副本数据
/// 6. 升级后验证（表结构完整性、版本号一致性）
fn upgrade_db(conn: &Connection, current_version: i32) -> Result<(), String> {
    if current_version == 0 {
        log::info!("[UPGRADE] 从旧版本升级（无版本记录），执行完整迁移");
    } else {
        log::info!(
            "[UPGRADE] 从 V{} 升级到 V{}",
            current_version, CURRENT_SCHEMA_VERSION
        );
    }

    validate_pre_upgrade(conn, current_version)?;

    // 始终确保基线表存在（包括 records, config, cache, equipments）
    // 这样可以确保从任何版本升级时都有完整的表结构
    ensure_baseline_tables(conn)?;

    let start_version = if current_version == 0 { 1 } else { current_version + 1 };
    for version in start_version..=CURRENT_SCHEMA_VERSION {
        run_migration_with_validation(conn, version)?;
    }

    migration::init_static_raids(conn)?;

    validate_post_upgrade(conn)?;

    log::info!("[UPGRADE] 升级完成，当前版本 V{}", CURRENT_SCHEMA_VERSION);
    Ok(())
}

/// 升级前验证：检查数据库完整性和前置条件
fn validate_pre_upgrade(conn: &Connection, current_version: i32) -> Result<(), String> {
    log::info!("[VALIDATE] 升级前验证开始");

    let integrity: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap_or_else(|_| "unknown".to_string());

    if integrity != "ok" {
        log::error!("[VALIDATE] 数据库完整性检查失败: {}", integrity);
        return Err(format!("数据库完整性检查失败: {}", integrity));
    }
    log::debug!("[VALIDATE] 数据库完整性检查通过");

    if current_version < 0 || current_version > CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "无效的数据库版本: V{}，有效范围: 0 ~ V{}",
            current_version, CURRENT_SCHEMA_VERSION
        ));
    }

    log::info!("[VALIDATE] 升级前验证通过");
    Ok(())
}

/// 升级后验证：检查数据库结构和数据完整性
fn validate_post_upgrade(conn: &Connection) -> Result<(), String> {
    log::info!("[VALIDATE] 升级后验证开始");

    let expected_tables = [
        "schema_versions", "migration_flags",
        "records", "config", "cache", "equipments",
        "accounts", "roles",
        "raids", "raid_bosses", "raid_versions",
        "favorite_raids",
        "instance_types", "role_instance_visibility",
        "raid_role_visibility",
        "trial_records", "baizhan_records",
        "game_versions", "seasons",
    ];

    for table_name in &expected_tables {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                [table_name],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if exists == 0 {
            log::error!("[VALIDATE] 缺少预期表: {}", table_name);
            return Err(format!("升级后验证失败：缺少预期表 {}", table_name));
        }
    }
    log::debug!("[VALIDATE] 所有预期表存在（{} 个）", expected_tables.len());

    let version = get_schema_version(conn)?;
    if version != CURRENT_SCHEMA_VERSION {
        return Err(format!(
            "升级后版本不匹配：期望 V{}，实际 V{}",
            CURRENT_SCHEMA_VERSION, version
        ));
    }
    log::debug!("[VALIDATE] 版本号验证通过: V{}", version);

    let expected_indexes = [
        "idx_roles_account_id",
        "idx_accounts_name",
        "idx_accounts_sort_order",
        "idx_riv_role_id",
        "idx_riv_instance_type_id",
        "idx_rrv_role_id",
        "idx_rrv_raid_key",
        "idx_records_raid_name",
        "idx_records_account_id",
        "idx_records_role_id",
        "idx_records_record_date",
        "idx_seasons_version_id",
        "idx_raids_season_id",
    ];

    let mut missing_indexes = Vec::new();
    for index_name in &expected_indexes {
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?",
                [index_name],
                |row| row.get(0),
            )
            .unwrap_or(0);

        if exists == 0 {
            missing_indexes.push(*index_name);
        }
    }

    if !missing_indexes.is_empty() {
        log::warn!("[VALIDATE] 缺少预期索引: {:?}", missing_indexes);
    } else {
        log::debug!("[VALIDATE] 所有预期索引存在（{} 个）", expected_indexes.len());
    }

    log::info!("[VALIDATE] 升级后验证通过");
    Ok(())
}

/// 执行单个迁移版本并验证结果
///
/// 每个迁移在独立事务中执行，失败时自动回滚。
/// 迁移成功后验证版本号已正确写入。
fn run_migration_with_validation(conn: &Connection, version: i32) -> Result<(), String> {
    log::info!("[MIGRATE] 执行迁移脚本：V{}", version);

    conn.execute("BEGIN TRANSACTION", [])
        .map_err(|e| e.to_string())?;

    let result = migration::apply_migration(conn, version);
    if let Err(e) = result {
        log::error!("[MIGRATE] V{} 迁移失败，执行 ROLLBACK，错误: {}", version, e);
        conn.execute("ROLLBACK", []).ok();
        return Err(format!("迁移 V{} 失败: {}", version, e));
    }

    set_schema_version(conn, version, &format!("升级到 V{}", version))?;

    let commit_result = conn.execute("COMMIT", []);
    if let Err(e) = commit_result {
        log::error!("[MIGRATE] V{} COMMIT 失败，错误: {}", version, e);
        conn.execute("ROLLBACK", []).ok();
        return Err(format!("迁移 V{} 提交失败: {}", version, e));
    }

    let after_version = get_schema_version(conn)?;
    if after_version != version {
        log::error!(
            "[MIGRATE] V{} 迁移后版本验证失败，期望: V{}，实际: V{}",
            version, version, after_version
        );
        return Err(format!(
            "迁移 V{} 后版本验证失败：期望 V{}，实际 V{}",
            version, version, after_version
        ));
    }

    log::info!("[MIGRATE] V{} 迁移完成", version);
    Ok(())
}

/// 确保基线表存在（仅用于 V0 旧数据库升级路径）
///
/// V0 表示数据库文件存在但没有 schema_versions 版本记录，
/// 即在版本化迁移系统引入之前就已存在的数据库。
/// 此函数确保这些旧数据库具备迁移脚本所依赖的基础表。
fn ensure_baseline_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            data TEXT,
            raid_name TEXT,
            account_id TEXT,
            role_id TEXT,
            record_date INTEGER,
            record_type TEXT
        );

        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS equipments (
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
    "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("[BASELINE] 基线表检查完成");
    Ok(())
}

/// 确保装备表包含所有必需列（历史兼容处理）
///
/// 旧版本数据库的 equipments 表可能缺少部分列，
/// 此函数在升级路径中补齐缺失列，确保业务逻辑正常工作。
/// 新安装的数据库通过 create_latest_schema 已包含完整列，无需此处理。
fn ensure_equipment_columns(conn: &Connection) -> Result<(), String> {
    let columns_to_check = [
        ("bind_type", "INTEGER"),
        ("type_label", "TEXT"),
        ("attribute_types", "TEXT"),
        ("attributes", "TEXT"),
        ("recommend", "TEXT"),
        ("diamonds", "TEXT"),
    ];

    let mut added = 0;
    for (col_name, col_type) in &columns_to_check {
        if !column_exists(conn, "equipments", col_name)? {
            log::info!("[EQUIP-MIGRATE] 装备表添加缺失列: {} {}", col_name, col_type);
            conn.execute(
                &format!("ALTER TABLE equipments ADD COLUMN {} {}", col_name, col_type),
                [],
            )
            .map_err(|e| format!("添加装备表列 {} 失败: {}", col_name, e))?;
            added += 1;
        }
    }

    if added > 0 {
        log::info!("[EQUIP-MIGRATE] 装备表补齐 {} 个缺失列", added);
    }

    Ok(())
}

/// 确保关键表的列完整（用于应用降级场景）
///
/// 当数据库版本高于应用版本时（用户从高版本应用复制数据库到低版本应用），
/// 数据库表可能缺少某些列。此函数检查并补齐所有关键列。
fn ensure_critical_columns(conn: &Connection) -> Result<(), String> {
    let critical_columns = [
        ("trial_records", "role_name", "TEXT DEFAULT ''"),
        ("trial_records", "server", "TEXT DEFAULT ''"),
        ("trial_records", "record_type", "TEXT DEFAULT 'trial'"),
        ("baizhan_records", "role_name", "TEXT"),
        ("baizhan_records", "server", "TEXT"),
        ("baizhan_records", "record_type", "TEXT DEFAULT 'baizhan'"),
        ("accounts", "sort_order", "INTEGER DEFAULT 0"),
        ("accounts", "password", "TEXT"),
        ("accounts", "notes", "TEXT"),
        ("accounts", "hidden", "INTEGER DEFAULT 0"),
        ("accounts", "disabled", "INTEGER DEFAULT 0"),
        ("roles", "martial", "TEXT"),
        ("roles", "equipment_score", "INTEGER"),
        ("roles", "disabled", "INTEGER DEFAULT 0"),
        ("raids", "season_id", "INTEGER"),
        ("records", "raid_name", "TEXT"),
        ("records", "account_id", "TEXT"),
        ("records", "role_id", "TEXT"),
        ("records", "record_date", "INTEGER"),
        ("records", "record_type", "TEXT"),
    ];

    let mut added = 0;
    for (table, col_name, col_type) in &critical_columns {
        if !table_exists(conn, table)? {
            continue;
        }
        if !column_exists(conn, table, col_name)? {
            log::warn!(
                "[CRITICAL-COLUMNS] 表 {} 缺少列 {}，正在补齐",
                table, col_name
            );
            conn.execute(
                &format!("ALTER TABLE {} ADD COLUMN {} {}", table, col_name, col_type),
                [],
            )
            .map_err(|e| format!("补齐列 {}.{} 失败: {}", table, col_name, e))?;
            added += 1;
        }
    }

    if added > 0 {
        log::info!("[CRITICAL-COLUMNS] 共补齐 {} 个缺失列", added);
    }

    Ok(())
}

/// 获取当前 schema 版本
fn get_schema_version(conn: &Connection) -> Result<i32, String> {
    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    log::debug!("[DEBUG] get_schema_version: 读取到当前版本 = {}", version);
    Ok(version)
}

/// 设置 schema 版本
fn set_schema_version(conn: &Connection, version: i32, description: &str) -> Result<(), String> {
    let timestamp = get_local_timestamp();
    log::debug!(
        "[DEBUG] set_schema_version: 即将写入 version={}, description={}",
        version,
        description
    );
    conn.execute(
        "INSERT INTO schema_versions (version, applied_at, description) VALUES (?, ?, ?)
         ON CONFLICT(version) DO UPDATE SET applied_at = excluded.applied_at, description = excluded.description",
        params![version, timestamp, description],
    )
    .map_err(|e| e.to_string())?;
    log::debug!("[DEBUG] set_schema_version: version={} 写入成功", version);
    Ok(())
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
            [table_name],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

fn column_exists(conn: &Connection, table_name: &str, column_name: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table_name))
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    for column in columns {
        if column.map_err(|e| e.to_string())? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

/// 创建最新版本的完整数据库结构（仅用于全新安装路径）
///
/// 此函数是安装路径的核心，一次性创建所有表、索引和初始数据。
/// 每个表的创建注释标注了对应的迁移版本号，便于与升级脚本对照维护。
/// 新增表或列时，必须同时在对应的迁移脚本中添加，确保升级路径完整。
fn create_latest_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        -- ===== 基线表（V0 前已存在） =====
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            data TEXT,
            raid_name TEXT,
            account_id TEXT,
            role_id TEXT,
            record_date INTEGER,
            record_type TEXT
        );

        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS equipments (
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

        -- ===== V1: 结构化账号表 =====
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            account_name TEXT NOT NULL,
            account_type TEXT NOT NULL DEFAULT 'OWN',
            sort_order INTEGER DEFAULT 0,
            password TEXT,
            notes TEXT,
            hidden INTEGER DEFAULT 0,
            disabled INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS roles (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            name TEXT NOT NULL,
            server TEXT,
            region TEXT,
            sect TEXT,
            martial TEXT,
            equipment_score INTEGER,
            disabled INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );

        -- ===== V10: 赛季系统 =====
        CREATE TABLE IF NOT EXISTS game_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS seasons (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            version_id INTEGER NOT NULL,
            start_date INTEGER NOT NULL,
            end_date INTEGER,
            sort_order INTEGER NOT NULL DEFAULT 0,
            trial_equip_level_min INTEGER DEFAULT 0,
            trial_equip_level_max INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (version_id) REFERENCES game_versions(id)
        );

        -- ===== V2: 结构化副本表 =====
        CREATE TABLE IF NOT EXISTS raids (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            difficulty TEXT NOT NULL DEFAULT '普通',
            player_count INTEGER NOT NULL DEFAULT 25,
            version TEXT,
            notes TEXT,
            is_active INTEGER DEFAULT 1,
            is_static INTEGER DEFAULT 0,
            season_id INTEGER REFERENCES seasons(id)
        );

        CREATE TABLE IF NOT EXISTS raid_bosses (
            id TEXT PRIMARY KEY,
            raid_name TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS raid_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        );

        -- ===== V4: 副本收藏表 =====
        CREATE TABLE IF NOT EXISTS favorite_raids (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            raid_name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL
        );

        -- ===== V5: 角色可见性配置 =====
        CREATE TABLE IF NOT EXISTS instance_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS role_instance_visibility (
            id TEXT PRIMARY KEY,
            role_id TEXT NOT NULL,
            instance_type_id INTEGER NOT NULL,
            visible INTEGER DEFAULT 1,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
            FOREIGN KEY (instance_type_id) REFERENCES instance_types(id) ON DELETE CASCADE,
            UNIQUE(role_id, instance_type_id)
        );

        -- ===== V6: 团队副本角色可见性 =====
        CREATE TABLE IF NOT EXISTS raid_role_visibility (
            id TEXT PRIMARY KEY,
            role_id TEXT NOT NULL,
            raid_key TEXT NOT NULL,
            visible INTEGER DEFAULT 1,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
            UNIQUE(role_id, raid_key)
        );

        -- ===== V3: 试炼和百战记录表 =====
        CREATE TABLE IF NOT EXISTS trial_records (
            id TEXT PRIMARY KEY,
            account_id TEXT,
            role_id TEXT,
            role_name TEXT,
            server TEXT,
            layer INTEGER,
            bosses TEXT,
            card_1 TEXT,
            card_2 TEXT,
            card_3 TEXT,
            card_4 TEXT,
            card_5 TEXT,
            flipped_index INTEGER,
            record_type TEXT DEFAULT 'trial',
            date INTEGER NOT NULL,
            notes TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS baizhan_records (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            role_id TEXT NOT NULL,
            role_name TEXT,
            server TEXT,
            date INTEGER NOT NULL,
            gold_income INTEGER DEFAULT 0,
            gold_expense INTEGER DEFAULT 0,
            notes TEXT,
            record_type TEXT DEFAULT 'baizhan',
            updated_at TEXT
        );

        -- ===== 索引 =====
        CREATE INDEX IF NOT EXISTS idx_roles_account_id ON roles(account_id);
        CREATE INDEX IF NOT EXISTS idx_accounts_name ON accounts(account_name);
        CREATE INDEX IF NOT EXISTS idx_accounts_sort_order ON accounts(sort_order);

        CREATE INDEX IF NOT EXISTS idx_riv_role_id ON role_instance_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_riv_instance_type_id ON role_instance_visibility(instance_type_id);

        CREATE INDEX IF NOT EXISTS idx_rrv_role_id ON raid_role_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_rrv_raid_key ON raid_role_visibility(raid_key);

        CREATE INDEX IF NOT EXISTS idx_records_raid_name ON records(raid_name);
        CREATE INDEX IF NOT EXISTS idx_records_account_id ON records(account_id);
        CREATE INDEX IF NOT EXISTS idx_records_role_id ON records(role_id);
        CREATE INDEX IF NOT EXISTS idx_records_record_date ON records(record_date);

        CREATE INDEX IF NOT EXISTS idx_seasons_version_id ON seasons(version_id);
        CREATE INDEX IF NOT EXISTS idx_raids_season_id ON raids(season_id);

        -- ===== 初始数据 =====
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (1, 'raid', '团队副本');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (2, 'baizhan', '百战异闻录');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (3, 'trial', '试炼之地');
    "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("[INSTALL] 数据库结构创建完成");
    Ok(())
}

/// 检查 localStorage 迁移是否已完成
#[tauri::command]
pub fn db_is_local_storage_migrated() -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let migrated: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM migration_flags WHERE key = 'local_storage_migrated' AND value = 'true'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(migrated > 0)
}

/// 标记 localStorage 迁移已完成
#[tauri::command]
pub fn db_set_local_storage_migrated() -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    conn.execute(
        "INSERT INTO migration_flags (key, value, updated_at) VALUES ('local_storage_migrated', 'true', ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![timestamp],
    )
    .map_err(|e| e.to_string())?;

    log::info!("localStorage 迁移标记已设置");
    Ok(())
}

/// 获取数据库版本信息（用于调试）
#[tauri::command]
pub fn db_get_version_info() -> Result<serde_json::Value, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_versions",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let local_storage_migrated: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM migration_flags WHERE key = 'local_storage_migrated' AND value = 'true'",
            [],
            |row| {
                let count: i64 = row.get(0)?;
                Ok(count > 0)
            },
        )
        .unwrap_or(false);

    Ok(serde_json::json!({
        "schemaVersion": version,
        "currentVersion": CURRENT_SCHEMA_VERSION,
        "isLatest": version == CURRENT_SCHEMA_VERSION,
        "localStorageMigrated": local_storage_migrated
    }))
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct Equipment {
    #[serde(rename = "ID")]
    pub id: String, // 使用 API 原始的 id 字段（如 "8_41486"）
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "UiID")]
    pub ui_id: String,
    #[serde(rename = "IconID")]
    pub icon_id: Option<i64>,
    #[serde(rename = "Level")]
    pub level: i64,
    #[serde(rename = "Quality")]
    pub quality: String,
    #[serde(rename = "BindType")]
    pub bind_type: Option<i64>,
    #[serde(rename = "TypeLabel")]
    pub type_label: Option<String>,

    // Explicit complex fields stored as JSON strings
    #[serde(rename = "AttributeTypes")]
    pub attribute_types: Option<serde_json::Value>,
    #[serde(rename = "attributes")]
    pub attributes: Option<Vec<serde_json::Value>>,
    #[serde(rename = "Recommend")]
    pub recommend: Option<String>,
    #[serde(rename = "Diamonds")]
    pub diamonds: Option<Vec<serde_json::Value>>,

    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[tauri::command]
pub fn db_save_equipments(equipments: String) -> Result<(), String> {
    let items: Vec<Equipment> = serde_json::from_str(&equipments).map_err(|e| e.to_string())?;
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    {
        let mut stmt = tx
            .prepare(
                "INSERT INTO equipments (
                id, name, ui_id, icon_id, level, quality, bind_type, type_label,
                attribute_types, attributes, recommend, diamonds,
                data, updated_at
            )
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                ui_id = excluded.ui_id,
                icon_id = excluded.icon_id,
                level = excluded.level,
                quality = excluded.quality,
                bind_type = excluded.bind_type,
                type_label = excluded.type_label,
                attribute_types = excluded.attribute_types,
                attributes = excluded.attributes,
                recommend = excluded.recommend,
                diamonds = excluded.diamonds,
                data = excluded.data,
                updated_at = excluded.updated_at",
            )
            .map_err(|e| e.to_string())?;

        for item in items {
            let data = serde_json::to_string(&item).unwrap_or_default();

            // Serialize complex fields to JSON strings
            let attr_types_json = item.attribute_types.as_ref().map(|v| v.to_string());
            let attrs_json = item
                .attributes
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());
            // Recommend is now a String, so use it directly (as ref)
            let recommend_str = item.recommend.as_ref();
            let diamonds_json = item
                .diamonds
                .as_ref()
                .map(|v| serde_json::to_string(v).unwrap_or_default());

            stmt.execute(params![
                item.id,
                item.name,
                item.ui_id,
                item.icon_id,
                item.level,
                item.quality,
                item.bind_type,
                item.type_label,
                attr_types_json,
                attrs_json,
                recommend_str,
                diamonds_json,
                data,
                timestamp
            ])
            .map_err(|e| e.to_string())?;
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_equipments() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data, type_label FROM equipments ORDER BY level DESC")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)))
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok((json_str, type_label)) = row {
            if let Ok(mut item) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(obj) = item.as_object_mut() {
                    if let Some(label) = type_label {
                        obj.insert("TypeLabel".to_string(), serde_json::Value::String(label));
                    }
                }
                result.push(item);
            }
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_clear_equipments() -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM equipments", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct TrialRecord {
    pub id: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "roleId")]
    pub role_id: String,
    #[serde(rename = "roleName", default)]
    pub role_name: String,
    #[serde(rename = "server", default)]
    pub server: String,
    pub layer: i64,
    pub bosses: Vec<String>, // JSON array
    #[serde(rename = "card1")]
    pub card_1: String,
    #[serde(rename = "card2")]
    pub card_2: String,
    #[serde(rename = "card3")]
    pub card_3: String,
    #[serde(rename = "card4")]
    pub card_4: String,
    #[serde(rename = "card5")]
    pub card_5: String,
    #[serde(rename = "flippedIndex")]
    pub flipped_index: i64,
    #[serde(rename = "type", default)]
    pub record_type: String,
    pub date: i64, // Changed from String to i64 (timestamp)
    pub notes: Option<String>,
}

#[tauri::command]
pub fn db_add_trial_record(record: String) -> Result<(), String> {
    log::info!("[DB] 添加试炼之地记录: {}", record);
    
    let item: TrialRecord = serde_json::from_str(&record).map_err(|e| {
        let err_msg = format!("解析试炼记录失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;
    
    let conn = init_db().map_err(|e| {
        let err_msg = format!("初始化数据库失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;
    
    let timestamp = get_local_timestamp();

    // Ensure bosses are serialized
    let bosses_json = serde_json::to_string(&item.bosses).unwrap_or_default();

    conn.execute(
        "INSERT INTO trial_records (
            id, account_id, role_id, role_name, server, layer, bosses,
            card_1, card_2, card_3, card_4, card_5, flipped_index, record_type, date, notes, updated_at
        )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            role_id = excluded.role_id,
            role_name = excluded.role_name,
            server = excluded.server,
            layer = excluded.layer,
            bosses = excluded.bosses,
            card_1 = excluded.card_1,
            card_2 = excluded.card_2,
            card_3 = excluded.card_3,
            card_4 = excluded.card_4,
            card_5 = excluded.card_5,
            flipped_index = excluded.flipped_index,
            record_type = excluded.record_type,
            date = excluded.date,
            notes = excluded.notes,
            updated_at = excluded.updated_at",
        params![
            item.id,
            item.account_id,
            item.role_id,
            item.role_name,
            item.server,
            item.layer,
            bosses_json,
            item.card_1,
            item.card_2,
            item.card_3,
            item.card_4,
            item.card_5,
            item.flipped_index,
            item.record_type,
            item.date,
            item.notes,
            timestamp
        ],
    )
    .map_err(|e| {
        let err_msg = format!("保存试炼记录失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;

    log::info!("[DB] 试炼之地记录保存成功: id={}, roleId={}, layer={}", item.id, item.role_id, item.layer);
    Ok(())
}

#[tauri::command]
pub fn db_get_trial_records() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "
        SELECT id, account_id, role_id, role_name, server, layer, bosses, 
               card_1, card_2, card_3, card_4, card_5, flipped_index, record_type,
               date, notes, updated_at
        FROM trial_records 
        ORDER BY date DESC
    ",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let bosses_str: String = row.get(6)?;

            Ok(TrialRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                role_id: row.get(2)?,
                role_name: row.get(3)?,
                server: row.get(4)?,
                layer: row.get(5)?,
                bosses: serde_json::from_str(&bosses_str).unwrap_or_default(),
                card_1: row.get(7)?,
                card_2: row.get(8)?,
                card_3: row.get(9)?,
                card_4: row.get(10)?,
                card_5: row.get(11)?,
                flipped_index: row.get(12)?,
                record_type: row.get(13)?,
                date: row.get(14)?,
                notes: row.get(15)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok(item) = row {
            result.push(item);
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_delete_trial_record(id: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM trial_records WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ========== 百战记录 CRUD ==========

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct BaizhanRecord {
    pub id: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "roleId")]
    pub role_id: String,
    #[serde(rename = "roleName")]
    pub role_name: Option<String>,
    pub server: Option<String>,
    pub date: i64, // Changed from String to i64 (timestamp)
    #[serde(rename = "goldIncome")]
    pub gold_income: i64,
    #[serde(rename = "goldExpense", default)]
    pub gold_expense: Option<i64>,
    pub notes: Option<String>,
    #[serde(rename = "type", default = "default_baizhan_type")]
    pub record_type: String,
}

fn default_baizhan_type() -> String {
    "baizhan".to_string()
}

#[tauri::command]
pub fn db_add_baizhan_record(record: String) -> Result<(), String> {
    let item: BaizhanRecord = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    let conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    conn.execute(
        "INSERT INTO baizhan_records (
            id, account_id, role_id, role_name, server,
            date, gold_income, gold_expense, notes, record_type, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            role_id = excluded.role_id,
            role_name = excluded.role_name,
            server = excluded.server,
            date = excluded.date,
            gold_income = excluded.gold_income,
            gold_expense = excluded.gold_expense,
            notes = excluded.notes,
            record_type = excluded.record_type,
            updated_at = excluded.updated_at",
        params![
            item.id,
            item.account_id,
            item.role_id,
            item.role_name,
            item.server,
            item.date,
            item.gold_income,
            item.gold_expense.unwrap_or(0),
            item.notes,
            item.record_type,
            timestamp
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn db_get_baizhan_records() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, account_id, role_id, role_name, server,
                    date, gold_income, gold_expense, notes, record_type
             FROM baizhan_records
             ORDER BY date DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(BaizhanRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                role_id: row.get(2)?,
                role_name: row.get(3)?,
                server: row.get(4)?,
                date: row.get(5)?,
                gold_income: row.get(6)?,
                gold_expense: row.get(7)?,
                notes: row.get(8)?,
                record_type: row
                    .get::<_, Option<String>>(9)?
                    .unwrap_or_else(|| "baizhan".to_string()),
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok(item) = row {
            result.push(item);
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_delete_baizhan_record(id: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM baizhan_records WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_update_baizhan_record(record: String) -> Result<(), String> {
    let item: BaizhanRecord = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    let conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    conn.execute(
        "UPDATE baizhan_records SET
            account_id = ?, role_id = ?, role_name = ?, server = ?,
            date = ?, gold_income = ?, gold_expense = ?, notes = ?,
            record_type = ?, updated_at = ?
         WHERE id = ?",
        params![
            item.account_id,
            item.role_id,
            item.role_name,
            item.server,
            item.date,
            item.gold_income,
            item.gold_expense.unwrap_or(0),
            item.notes,
            item.record_type,
            timestamp,
            item.id
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn db_init() -> Result<(), String> {
    init_db()?;
    Ok(())
}

#[tauri::command]
pub fn db_save_accounts(accounts: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&accounts).map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let timestamp = get_local_timestamp();

    // 1. 获取现有的账号 ID 列表
    let existing_account_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM accounts")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 2. 获取现有的角色 ID 列表
    let existing_role_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM roles")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 3. 收集传入数据中的账号和角色 ID
    let mut incoming_account_ids: Vec<String> = Vec::new();
    let mut incoming_role_ids: Vec<String> = Vec::new();
    for account in &parsed {
        if let Some(id) = account["id"].as_str() {
            incoming_account_ids.push(id.to_string());
        }
        if let Some(roles) = account["roles"].as_array() {
            for role in roles {
                if let Some(role_id) = role["id"].as_str() {
                    incoming_role_ids.push(role_id.to_string());
                }
            }
        }
    }

    let incoming_account_set: HashSet<String> = incoming_account_ids.into_iter().collect();
    let incoming_role_set: HashSet<String> = incoming_role_ids.into_iter().collect();

    // 4. 删除不在传入数据中的账号及其关联数据（保留历史记录）
    for account_id in &existing_account_ids {
        if !incoming_account_set.contains(account_id) {
            let roles_to_delete: Vec<String> = {
                let mut stmt = tx
                    .prepare("SELECT id FROM roles WHERE account_id = ?")
                    .map_err(|e| e.to_string())?;
                let rows = stmt.query_map(params![account_id], |row| row.get(0)).map_err(|e| e.to_string())?;
                rows.filter_map(|r| r.ok()).collect()
            };
            for role_id in &roles_to_delete {
                tx.execute("DELETE FROM role_instance_visibility WHERE role_id = ?", params![role_id])
                    .map_err(|e| e.to_string())?;
                tx.execute("DELETE FROM raid_role_visibility WHERE role_id = ?", params![role_id])
                    .map_err(|e| e.to_string())?;
            }
            tx.execute("DELETE FROM roles WHERE account_id = ?", params![account_id])
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM accounts WHERE id = ?", params![account_id])
                .map_err(|e| e.to_string())?;
        }
    }

    // 5. 删除不在传入数据中的角色及其关联数据（保留历史记录）
    for role_id in &existing_role_ids {
        if !incoming_role_set.contains(role_id) {
            tx.execute("DELETE FROM role_instance_visibility WHERE role_id = ?", params![role_id])
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM raid_role_visibility WHERE role_id = ?", params![role_id])
                .map_err(|e| e.to_string())?;
            tx.execute("DELETE FROM roles WHERE id = ?", params![role_id])
                .map_err(|e| e.to_string())?;
        }
    }

    // 6. 插入或更新账号和角色
    for (sort_order, account) in parsed.iter().enumerate() {
        let id = account["id"].as_str().unwrap_or_default().to_string();
        let account_name = account["accountName"].as_str().unwrap_or("").to_string();
        let account_type = account["type"].as_str().unwrap_or("OWN").to_string();
        let sort_order = sort_order as i64;
        let password = account["password"].as_str().map(|s| s.to_string());
        let notes = account["notes"].as_str().map(|s| s.to_string());
        let hidden = account["hidden"].as_bool().unwrap_or(false) as i32;
        let disabled = account["disabled"].as_bool().unwrap_or(false) as i32;

        // 使用 INSERT ... ON CONFLICT DO UPDATE 避免触发级联删除
        // 注意：INSERT OR REPLACE 会先 DELETE 再 INSERT，会触发外键级联删除！
        tx.execute(
            "INSERT INTO accounts (id, account_name, account_type, sort_order, password, notes, hidden, disabled, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                account_name = excluded.account_name,
                account_type = excluded.account_type,
                sort_order = excluded.sort_order,
                password = excluded.password,
                notes = excluded.notes,
                hidden = excluded.hidden,
                disabled = excluded.disabled,
                updated_at = excluded.updated_at",
            params![id, account_name, account_type, sort_order, password, notes, hidden, disabled, timestamp],
        )
        .map_err(|e| e.to_string())?;

        if let Some(roles) = account["roles"].as_array() {
            for role in roles {
                let role_id = role["id"].as_str().unwrap_or_default().to_string();
                let name = role["name"].as_str().unwrap_or("").to_string();
                let server = role["server"].as_str().map(|s| s.to_string());
                let region = role["region"].as_str().map(|s| s.to_string());
                let martial = role["martial"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                let sect = get_sect_by_martial(&martial)
                    .unwrap_or_default()
                    .to_string();
                let r_disabled = role["disabled"].as_bool().unwrap_or(false) as i32;
                let equipment_score = role["equipmentScore"].as_i64();

                // 使用 INSERT ... ON CONFLICT DO UPDATE 避免触发级联删除
                tx.execute(
                    "INSERT INTO roles (id, account_id, name, server, region, sect, martial, equipment_score, disabled, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET
                        account_id = excluded.account_id,
                        name = excluded.name,
                        server = excluded.server,
                        region = excluded.region,
                        sect = excluded.sect,
                        martial = excluded.martial,
                        equipment_score = excluded.equipment_score,
                        disabled = excluded.disabled,
                        updated_at = excluded.updated_at",
                    params![role_id, id, name, server, region, sect, martial, equipment_score, r_disabled, timestamp],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

// New structured accounts API
#[tauri::command]
pub fn db_get_accounts_structured() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // Get all accounts
    let mut stmt = conn
        .prepare(
            "
        SELECT id, account_name, account_type, sort_order, hidden, disabled, password, notes, created_at, updated_at 
        FROM accounts ORDER BY sort_order, account_name
    ",
        )
        .map_err(|e| e.to_string())?;

    // SELECT 顺序: id, account_name, account_type, sort_order, hidden, disabled, password, notes, created_at, updated_at
    let accounts: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "accountName": row.get::<_, String>(1)?,
                "type": row.get::<_, String>(2)?,
                "sortOrder": row.get::<_, i64>(3)?,
                "hidden": row.get::<_, i32>(4)? != 0,
                "disabled": row.get::<_, i32>(5)? != 0,
                "password": row.get::<_, Option<String>>(6)?,
                "notes": row.get::<_, Option<String>>(7)?,
                "createdAt": row.get::<_, Option<String>>(8)?,
                "updatedAt": row.get::<_, Option<String>>(9)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&accounts).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_get_all_roles() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "
        SELECT r.id, r.account_id, r.name, r.server, r.region, r.sect, r.martial,
               r.equipment_score, r.disabled, r.created_at, r.updated_at,
               a.account_name
        FROM roles r JOIN accounts a ON r.account_id = a.id
        ORDER BY r.name
    ",
        )
        .map_err(|e| e.to_string())?;

    let roles: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "accountId": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "server": row.get::<_, Option<String>>(3)?,
                "region": row.get::<_, Option<String>>(4)?,
                "sect": row.get::<_, Option<String>>(5)?,
                "martial": row.get::<_, Option<String>>(6)?,
                "equipmentScore": row.get::<_, Option<i64>>(7)?,
                "disabled": row.get::<_, i32>(8)? != 0,
                "createdAt": row.get::<_, Option<String>>(9)?,
                "updatedAt": row.get::<_, Option<String>>(10)?,
                "accountName": row.get::<_, String>(11)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&roles).map_err(|e| e.to_string())
}

struct RoleJoinRow {
    role_id: String,
    account_id: Option<String>,
    name: Option<String>,
    server: Option<String>,
    region: Option<String>,
    sect: Option<String>,
    martial: Option<String>,
    equipment_score: Option<i64>,
    disabled: bool,
}

struct AccountJoinRow {
    account_id: String,
    account_name: String,
    account_type: String,
    sort_order: i64,
    password: Option<String>,
    notes: Option<String>,
    hidden: bool,
    disabled: bool,
    created_at: Option<String>,
    updated_at: Option<String>,
    role: Option<RoleJoinRow>,
}

fn upsert_account_from_join_row(
    account_map: &mut std::collections::HashMap<String, serde_json::Value>,
    row: AccountJoinRow,
    vis_map: &std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>>,
    default_vis_map: &serde_json::Map<String, serde_json::Value>,
) {
    let account_id = row.account_id.clone();

    if !account_map.contains_key(&account_id) {
        let account = serde_json::json!({
            "id": &account_id,
            "accountName": row.account_name,
            "type": row.account_type,
            "sortOrder": row.sort_order,
            "password": row.password,
            "notes": row.notes,
            "hidden": row.hidden,
            "disabled": row.disabled,
            "createdAt": row.created_at,
            "updatedAt": row.updated_at,
            "roles": Vec::<serde_json::Value>::new(),
        });
        account_map.insert(account_id.clone(), account);
    }

    if let Some(role) = row.role {
        let visibility = vis_map
            .get(&role.role_id)
            .cloned()
            .unwrap_or_else(|| default_vis_map.clone());
        let role_json = serde_json::json!({
            "id": role.role_id,
            "account_id": role.account_id,
            "name": role.name,
            "server": role.server,
            "region": role.region,
            "sect": role.sect,
            "martial": role.martial,
            "equipmentScore": role.equipment_score,
            "disabled": role.disabled,
            "visibility": visibility,
        });
        if let Some(account) = account_map.get_mut(&account_id) {
            let roles = account["roles"].as_array_mut().unwrap();
            roles.push(role_json);
        }
    }
}
#[tauri::command]
pub fn db_get_accounts_with_roles() -> Result<String, String> {
    log::info!("[db_get_accounts_with_roles] 开始查询账号...");
    let conn = init_db().map_err(|e| e.to_string())?;

    // ==== 获取所有角色的可见性配置 ====
    // 1. 获取所有支持的副本类型列表并作为默认值
    let mut default_vis_map = serde_json::Map::new();
    if let Ok(mut type_stmt) = conn.prepare("SELECT type FROM instance_types") {
        if let Ok(types_iter) = type_stmt.query_map([], |row| row.get::<_, String>(0)) {
            for t_res in types_iter {
                if let Ok(t_str) = t_res {
                    default_vis_map.insert(t_str, serde_json::json!(true));
                }
            }
        }
    }

    // 2. 获取角色的个性化配置
    let mut vis_stmt = conn
        .prepare(
            "
        SELECT riv.role_id, it.type, riv.visible
        FROM role_instance_visibility riv
        JOIN instance_types it ON riv.instance_type_id = it.id
    ",
        )
        .map_err(|e| e.to_string())?;

    let mut vis_map: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> =
        std::collections::HashMap::new();
    let vis_rows = vis_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? == 1,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row_res in vis_rows {
        if let Ok((role_id, type_str, visible)) = row_res {
            let role_vis = vis_map
                .entry(role_id)
                .or_insert_with(|| default_vis_map.clone());
            role_vis.insert(type_str, serde_json::json!(visible));
        }
    }

    // 单次 LEFT JOIN 查询获取账号和角色
    // Account 字段: 0-9, Role 字段: 10-19 (可能为 NULL)
    let mut stmt = conn
        .prepare(
            "
        SELECT
            a.id, a.account_name, a.account_type, a.sort_order, a.password, a.notes,
            a.hidden, a.disabled, a.created_at, a.updated_at,
            r.id, r.account_id, r.name, r.server, r.region,
            r.sect, r.martial, r.equipment_score, r.disabled, r.created_at, r.updated_at
        FROM accounts a
        LEFT JOIN roles r ON a.id = r.account_id
        ORDER BY a.sort_order, a.account_name, r.name
    ",
        )
        .map_err(|e| e.to_string())?;

    let mut account_map: std::collections::HashMap<String, serde_json::Value> =
        std::collections::HashMap::new();

    let rows = stmt
        .query_map([], |row| {
            let role_id: Option<String> = row.get(10)?;
            let role = if let Some(role_id) = role_id {
                Some(RoleJoinRow {
                    role_id,
                    account_id: row.get(11)?,
                    name: row.get(12)?,
                    server: row.get(13)?,
                    region: row.get(14)?,
                    sect: row.get(15)?,
                    martial: row.get(16)?,
                    equipment_score: row.get(17)?,
                    disabled: row.get::<_, i32>(18)? != 0,
                })
            } else {
                None
            };

            Ok(AccountJoinRow {
                account_id: row.get(0)?,
                account_name: row.get(1)?,
                account_type: row.get(2)?,
                sort_order: row.get(3)?,
                password: row.get(4)?,
                notes: row.get(5)?,
                hidden: row.get::<_, i32>(6)? != 0,
                disabled: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                role,
            })
        })
        .map_err(|e| e.to_string())?;

    for row_result in rows {
        let row = row_result.map_err(|e| e.to_string())?;
        upsert_account_from_join_row(&mut account_map, row, &vis_map, &default_vis_map);
    }
    // 返回账号数组
    let mut accounts: Vec<serde_json::Value> = account_map.into_values().collect();
    // 按 sortOrder 排序
    accounts.sort_by(|a, b| {
        let a_sort_order = a["sortOrder"].as_i64().unwrap_or_default();
        let b_sort_order = b["sortOrder"].as_i64().unwrap_or_default();
        a_sort_order.cmp(&b_sort_order).then_with(|| {
            a["accountName"]
                .as_str()
                .unwrap_or("")
                .cmp(b["accountName"].as_str().unwrap_or(""))
        })
    });

    log::info!(
        "[db_get_accounts_with_roles] 查询完成，返回 {} 个账号",
        accounts.len()
    );
    serde_json::to_string(&accounts).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_get_roles_by_account(account_id: String) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 1. 获取所有支持的副本类型列表并作为默认值
    let mut default_vis_map = serde_json::Map::new();
    if let Ok(mut type_stmt) = conn.prepare("SELECT type FROM instance_types") {
        if let Ok(types_iter) = type_stmt.query_map([], |row| row.get::<_, String>(0)) {
            for t_res in types_iter {
                if let Ok(t_str) = t_res {
                    default_vis_map.insert(t_str, serde_json::json!(true));
                }
            }
        }
    }

    // 2. 获取角色的个性化配置
    let mut vis_stmt = conn
        .prepare(
            "
        SELECT riv.role_id, it.type, riv.visible
        FROM role_instance_visibility riv
        JOIN instance_types it ON riv.instance_type_id = it.id
        WHERE riv.role_id IN (SELECT id FROM roles WHERE account_id = ?)
    ",
        )
        .map_err(|e| e.to_string())?;

    let mut vis_map: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> =
        std::collections::HashMap::new();
    let vis_rows = vis_stmt
        .query_map(params![account_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i32>(2)? == 1,
            ))
        })
        .map_err(|e| e.to_string())?;

    for row_res in vis_rows {
        if let Ok((role_id, type_str, visible)) = row_res {
            let role_vis = vis_map
                .entry(role_id)
                .or_insert_with(|| default_vis_map.clone());
            role_vis.insert(type_str, serde_json::json!(visible));
        }
    }

    let mut stmt = conn
        .prepare(
            "
        SELECT id, name, server, region, sect, martial, equipment_score, disabled, created_at, updated_at
        FROM roles WHERE account_id = ? ORDER BY name
    ",
        )
        .map_err(|e| e.to_string())?;

    let roles: Vec<serde_json::Value> = stmt
        .query_map(params![account_id], |row| {
            let role_id: String = row.get(0)?;
            let visibility = vis_map
                .get(&role_id)
                .cloned()
                .unwrap_or_else(|| default_vis_map.clone());

            Ok(serde_json::json!({
                "id": role_id,
                "name": row.get::<_, String>(1)?,
                "server": row.get::<_, Option<String>>(2)?,
                "region": row.get::<_, Option<String>>(3)?,
                "sect": row.get::<_, Option<String>>(4)?,
                "martial": row.get::<_, Option<String>>(5)?,
                "equipmentScore": row.get::<_, Option<i64>>(6)?,
                "disabled": row.get::<_, i32>(7)? != 0,
                "createdAt": row.get::<_, Option<String>>(8)?,
                "updatedAt": row.get::<_, Option<String>>(9)?,
                "visibility": visibility,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&roles).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_save_account_structured(account_json: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let account: serde_json::Value =
        serde_json::from_str(&account_json).map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    let id = account["id"].as_str().unwrap_or_default().to_string();
    let account_name = account["accountName"].as_str().unwrap_or("").to_string();
    let account_type = account["type"].as_str().unwrap_or("OWN").to_string();
    let sort_order = account["sortOrder"].as_i64().unwrap_or(0);
    let password = account["password"].as_str().map(|s| s.to_string());
    let notes = account["notes"].as_str().map(|s| s.to_string());
    let hidden = account["hidden"].as_bool().unwrap_or(false) as i32;
    let disabled = account["disabled"].as_bool().unwrap_or(false) as i32;

    // 检查是否为新建账号
    let is_new_account: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM accounts WHERE id = ?",
            params![id],
            |row| {
                let count: i32 = row.get(0)?;
                Ok(count == 0)
            },
        )
        .unwrap_or(true);

    // 使用 INSERT ... ON CONFLICT DO UPDATE 避免触发级联删除
    // 新建账号时设置 created_at，更新时只更新 updated_at
    if is_new_account {
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type, sort_order, password, notes, hidden, disabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                account_name = excluded.account_name,
                account_type = excluded.account_type,
                sort_order = excluded.sort_order,
                password = excluded.password,
                notes = excluded.notes,
                hidden = excluded.hidden,
                disabled = excluded.disabled,
                updated_at = excluded.updated_at",
            params![id, account_name, account_type, sort_order, password, notes, hidden, disabled, timestamp, timestamp],
        ).map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type, sort_order, password, notes, hidden, disabled, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                account_name = excluded.account_name,
                account_type = excluded.account_type,
                sort_order = excluded.sort_order,
                password = excluded.password,
                notes = excluded.notes,
                hidden = excluded.hidden,
                disabled = excluded.disabled,
                updated_at = excluded.updated_at",
            params![id, account_name, account_type, sort_order, password, notes, hidden, disabled, timestamp],
        ).map_err(|e| e.to_string())?;
    }

    Ok(())
}
#[tauri::command]
pub fn db_save_role_structured(role_json: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let role: serde_json::Value = serde_json::from_str(&role_json).map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    let id = role["id"].as_str().unwrap_or_default().to_string();
    let account_id = role["accountId"].as_str().unwrap_or("").to_string();
    let name = role["name"].as_str().unwrap_or("").to_string();
    let server = role["server"].as_str().map(|s| s.to_string());
    let region = role["region"].as_str().map(|s| s.to_string());
    let sect = role["sect"].as_str().map(|s| s.to_string());
    let martial = role["martial"].as_str().map(|s| s.to_string());
    let disabled = role["disabled"].as_bool().unwrap_or(false) as i32;
    let equipment_score = role["equipmentScore"].as_i64();

    // 检查是否为新建角色
    let is_new_role: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM roles WHERE id = ?",
            params![id],
            |row| {
                let count: i32 = row.get(0)?;
                Ok(count == 0)
            },
        )
        .unwrap_or(true);

    conn.execute(
        "INSERT INTO roles (id, account_id, name, server, region, sect, martial, equipment_score, disabled, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            name = excluded.name,
            server = excluded.server,
            region = excluded.region,
            sect = excluded.sect,
            martial = excluded.martial,
            equipment_score = excluded.equipment_score,
            disabled = excluded.disabled,
            updated_at = excluded.updated_at",
        params![id, account_id, name, server, region, sect, martial, equipment_score, disabled, timestamp],
    ).map_err(|e| e.to_string())?;

    // 如果是新建角色，自动创建可见性记录
    if is_new_role {
        create_default_visibility(&conn, &id)?;
    }

    Ok(())
}

/// 为新建角色创建默认可见性记录（全部可见）
fn create_default_visibility(conn: &Connection, role_id: &str) -> Result<(), String> {
    // 获取所有副本类型 ID
    let type_ids: Vec<i32> = conn
        .prepare("SELECT id FROM instance_types")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if type_ids.is_empty() {
        log::warn!("无副本类型数据，跳过创建默认可见性");
        return Ok(());
    }

    let timestamp = get_local_timestamp();
    let count = type_ids.len();

    for type_id in type_ids {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO role_instance_visibility (id, role_id, instance_type_id, visible, created_at, updated_at)
             VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            params![id, role_id, type_id, &timestamp],
        )
        .map_err(|e| format!("创建默认可见性失败: {}", e))?;
    }

    log::info!("已为角色 {} 创建 {} 条默认可见性记录", role_id, count);
    Ok(())
}

#[tauri::command]
pub fn db_delete_account_structured(account_id: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let role_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM roles WHERE account_id = ?")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map(params![&account_id], |row| row.get(0)).map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    for role_id in &role_ids {
        tx.execute("DELETE FROM role_instance_visibility WHERE role_id = ?", params![role_id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM raid_role_visibility WHERE role_id = ?", params![role_id])
            .map_err(|e| e.to_string())?;
    }

    tx.execute("DELETE FROM roles WHERE account_id = ?", params![account_id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM accounts WHERE id = ?", params![account_id])
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_delete_role_structured(role_id: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM role_instance_visibility WHERE role_id = ?", params![role_id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM raid_role_visibility WHERE role_id = ?", params![role_id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM roles WHERE id = ?", params![role_id])
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_records() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM records ORDER BY record_date DESC")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if let Ok(data) = row.get(0) {
            records.push(data);
        }
    }
    Ok(records)
}

#[tauri::command]
pub fn db_save_records(records: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(&records).map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 增量同步：使用 INSERT ... ON CONFLICT DO UPDATE 更新记录
    for record in parsed {
        let id = record["id"].as_str().unwrap_or_default().to_string();
        if id.is_empty() {
            continue;
        }
        let metadata = record_lookup_metadata_from_json(&record);
        tx.execute(
            "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                raid_name = excluded.raid_name,
                account_id = excluded.account_id,
                role_id = excluded.role_id,
                record_date = excluded.record_date,
                record_type = excluded.record_type",
            params![
                id,
                record.to_string(),
                metadata.raid_name,
                metadata.account_id,
                metadata.role_id,
                metadata.record_date,
                metadata.record_type
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_raids() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT id, name, difficulty, player_count, version, notes, is_active, is_static FROM raids")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

    let mut raids: Vec<serde_json::Value> = Vec::new();

    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let _id: String = row.get(0).map_err(|e| e.to_string())?;
        let name: String = row.get(1).map_err(|e| e.to_string())?;
        let difficulty: String = row.get(2).map_err(|e| e.to_string())?;
        let player_count: i64 = row.get(3).map_err(|e| e.to_string())?;
        let version: Option<String> = row.get(4).map_err(|e| e.to_string())?;
        let notes: Option<String> = row.get(5).map_err(|e| e.to_string())?;
        let is_active: i64 = row.get(6).map_err(|e| e.to_string())?;
        let is_static: i64 = row.get(7).map_err(|e| e.to_string())?;

        let mut raid = serde_json::json!({
            "name": name,
            "difficulty": difficulty,
            "playerCount": player_count,
            "isActive": is_active == 1,
            "static": is_static == 1
        });

        if let Some(v) = &version {
            if !v.is_empty() {
                raid["version"] = serde_json::json!(v);
            }
        }
        if let Some(n) = &notes {
            if !n.is_empty() {
                raid["notes"] = serde_json::json!(n);
            }
        }

        raids.push(raid);
    }
    drop(rows);
    drop(stmt);

    let mut boss_map: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    {
        let mut boss_stmt = conn
            .prepare("SELECT raid_name, id, name, boss_order FROM raid_bosses ORDER BY boss_order")
            .map_err(|e| e.to_string())?;
        let mut boss_rows = boss_stmt.query([]).map_err(|e| e.to_string())?;

        while let Some(boss_row) = boss_rows.next().map_err(|e| e.to_string())? {
            let raid_name: String = boss_row.get(0).map_err(|e| e.to_string())?;
            let boss_id: String = boss_row.get(1).map_err(|e| e.to_string())?;
            let boss_name: String = boss_row.get(2).map_err(|e| e.to_string())?;
            let boss_order: i64 = boss_row.get(3).map_err(|e| e.to_string())?;

            boss_map
                .entry(raid_name)
                .or_insert_with(Vec::new)
                .push(serde_json::json!({
                    "id": boss_id,
                    "name": boss_name,
                    "order": boss_order
                }));
        }
    }

    for raid in &mut raids {
        if let Some(name) = raid["name"].as_str() {
            if let Some(bosses) = boss_map.remove(name) {
                if !bosses.is_empty() {
                    raid["bosses"] = serde_json::json!(bosses);
                }
            }
        }
    }

    let result: Vec<String> = raids.iter().map(|r| r.to_string()).collect();
    Ok(result)
}

#[tauri::command]
pub fn db_get_raid_versions() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT name FROM raid_versions ORDER BY id DESC")
        .map_err(|e| e.to_string())?;

    let version_iter = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut versions = Vec::new();
    for version in version_iter {
        versions.push(version.map_err(|e| e.to_string())?);
    }

    Ok(versions)
}

// ========== 赛季管理 ==========

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    pub id: Option<i64>,
    pub name: String,
    pub sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Season {
    pub id: Option<i64>,
    pub name: String,
    pub version_id: i64,
    pub start_date: i64,
    pub end_date: Option<i64>,
    pub sort_order: i64,
    #[allow(dead_code)]
    pub trial_equip_level_min: Option<i64>,
    #[allow(dead_code)]
    pub trial_equip_level_max: Option<i64>,
}

#[tauri::command]
pub fn db_get_game_versions() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, sort_order FROM game_versions ORDER BY sort_order ASC")
        .map_err(|e| e.to_string())?;

    let version_iter = stmt
        .query_map([], |row| {
            Ok(GameVersion {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                sort_order: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut versions = Vec::new();
    for version in version_iter {
        versions.push(version.map_err(|e| e.to_string())?);
    }

    serde_json::to_string(&versions).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_save_game_version(version: String) -> Result<i64, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let item: GameVersion = serde_json::from_str(&version).map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    if let Some(id) = item.id {
        conn.execute(
            "UPDATE game_versions SET name = ?, sort_order = ? WHERE id = ?",
            params![item.name, item.sort_order, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO game_versions (name, sort_order, created_at) VALUES (?, ?, ?)",
            params![item.name, item.sort_order, &timestamp],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn db_get_seasons() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max FROM seasons ORDER BY sort_order ASC")
        .map_err(|e| e.to_string())?;

    let season_iter = stmt
        .query_map([], |row| {
            Ok(Season {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                version_id: row.get(2)?,
                start_date: row.get(3)?,
                end_date: row.get(4)?,
                sort_order: row.get(5)?,
                trial_equip_level_min: row.get(6)?,
                trial_equip_level_max: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut seasons = Vec::new();
    for season in season_iter {
        seasons.push(season.map_err(|e| e.to_string())?);
    }

    serde_json::to_string(&seasons).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_get_seasons_by_version(version_id: i64) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max FROM seasons WHERE version_id = ? ORDER BY sort_order ASC")
        .map_err(|e| e.to_string())?;

    let season_iter = stmt
        .query_map(params![version_id], |row| {
            Ok(Season {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                version_id: row.get(2)?,
                start_date: row.get(3)?,
                end_date: row.get(4)?,
                sort_order: row.get(5)?,
                trial_equip_level_min: row.get(6)?,
                trial_equip_level_max: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut seasons = Vec::new();
    for season in season_iter {
        seasons.push(season.map_err(|e| e.to_string())?);
    }

    serde_json::to_string(&seasons).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_save_season(season: String) -> Result<i64, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let item: Season = serde_json::from_str(&season).map_err(|e| e.to_string())?;
    let timestamp = get_local_timestamp();

    // 验证赛季结束时间不晚于版本结束时间
    if let Some(end_date) = item.end_date {
        let version_end_date: Option<i64> = conn
            .query_row(
                "SELECT end_date FROM game_versions WHERE id = ?",
                params![item.version_id],
                |row| row.get(0),
            )
            .ok();
        if let Some(ved) = version_end_date {
            if ved > 0 && end_date > ved {
                return Err("赛季结束时间不能晚于版本结束时间".to_string());
            }
        }
    }

    if let Some(id) = item.id {
        conn.execute(
            "UPDATE seasons SET name = ?, version_id = ?, start_date = ?, end_date = ?, sort_order = ?, trial_equip_level_min = ?, trial_equip_level_max = ? WHERE id = ?",
            params![item.name, item.version_id, item.start_date, item.end_date, item.sort_order, item.trial_equip_level_min, item.trial_equip_level_max, id],
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    } else {
        conn.execute(
            "INSERT INTO seasons (name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![item.name, item.version_id, item.start_date, item.end_date, item.sort_order, item.trial_equip_level_min, item.trial_equip_level_max, &timestamp],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }
}

#[tauri::command]
pub fn db_get_season_for_date(timestamp: i64) -> Result<Option<Season>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max FROM seasons WHERE start_date <= ? AND (end_date IS NULL OR end_date = 0 OR end_date > ?) ORDER BY sort_order DESC LIMIT 1")
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![timestamp, timestamp], |row| {
            Ok(Season {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                version_id: row.get(2)?,
                start_date: row.get(3)?,
                end_date: row.get(4)?,
                sort_order: row.get(5)?,
                trial_equip_level_min: row.get(6)?,
                trial_equip_level_max: row.get(7)?,
            })
        })
        .ok();

    Ok(result)
}

#[allow(dead_code)]
#[tauri::command]
pub fn db_get_current_season() -> Result<Option<Season>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().timestamp();
    let mut stmt = conn
        .prepare("SELECT id, name, version_id, start_date, end_date, sort_order, trial_equip_level_min, trial_equip_level_max FROM seasons WHERE start_date <= ? AND (end_date IS NULL OR end_date = 0 OR end_date > ?) ORDER BY sort_order DESC LIMIT 1")
        .map_err(|e| e.to_string())?;

    let result = stmt
        .query_row(params![now, now], |row| {
            Ok(Season {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                version_id: row.get(2)?,
                start_date: row.get(3)?,
                end_date: row.get(4)?,
                sort_order: row.get(5)?,
                trial_equip_level_min: row.get(6)?,
                trial_equip_level_max: row.get(7)?,
            })
        })
        .ok();

    Ok(result)
}

#[tauri::command]
pub fn db_save_raids(raids: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&raids).map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 1. 获取现有的 raids ID 列表
    let existing_raid_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM raids")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 2. 收集传入数据中的 raid ID
    let mut incoming_raid_ids: Vec<String> = Vec::new();
    for raid in &parsed {
        if let Some(id) = raid["id"].as_str() {
            incoming_raid_ids.push(id.to_string());
        }
    }

    // 3. 删除不在传入数据中的 raids（会级联删除 raid_bosses）
    for raid_id in &existing_raid_ids {
        if !incoming_raid_ids.contains(raid_id) {
            tx.execute("DELETE FROM raids WHERE id = ?", params![raid_id])
                .map_err(|e| e.to_string())?;
        }
    }

    // 4. 插入或更新 raids
    let mut boss_saved_names = std::collections::HashSet::new();
    for raid in &parsed {
        let name = raid["name"].as_str().unwrap_or_default();
        let difficulty = raid["difficulty"].as_str().unwrap_or("普通");
        let player_count = raid["playerCount"].as_i64().unwrap_or(25);
        let version = raid["version"].as_str().unwrap_or_default();
        let notes = raid["notes"].as_str().unwrap_or_default();
        let is_active = if raid["isActive"].as_bool().unwrap_or(true) {
            1
        } else {
            0
        };
        let is_static = if raid["static"].as_bool().unwrap_or(false) {
            1
        } else {
            0
        };
        let id = format!("{}人{}{}", player_count, difficulty, name);

        // 使用 INSERT ... ON CONFLICT DO UPDATE 处理新增和更新
        tx.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                difficulty = excluded.difficulty,
                player_count = excluded.player_count,
                version = excluded.version,
                notes = excluded.notes,
                is_active = excluded.is_active,
                is_static = excluded.is_static",
            params![id, name, difficulty, player_count, version, notes, is_active, is_static],
        ).map_err(|e| e.to_string())?;

        // 处理 raid_bosses
        if let Some(bosses) = raid["bosses"].as_array() {
            for boss in bosses {
                let boss_id = boss["id"].as_str().unwrap_or_default();
                let boss_name = boss["name"].as_str().unwrap_or_default();
                let boss_order = boss["order"].as_i64().unwrap_or(0);
                // 只保留不重复的 boss
                if !boss_saved_names.contains(boss_id) {
                    boss_saved_names.insert(boss_id);
                    tx.execute(
                        "INSERT INTO raid_bosses (id, raid_name, name, boss_order)
                         VALUES (?, ?, ?, ?)
                         ON CONFLICT(id) DO UPDATE SET
                            raid_name = excluded.raid_name,
                            name = excluded.name,
                            boss_order = excluded.boss_order",
                        params![boss_id, id, boss_name, boss_order],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_config() -> Result<Option<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value FROM config WHERE id = 1")
        .map_err(|e| e.to_string())?;
    let config: Option<String> = stmt.query_row([], |row| row.get(0)).ok();
    Ok(config)
}

#[tauri::command]
pub fn db_get_config_debug() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut result = String::new();
    result.push_str("=== 配置数据调试报告 ===\n\n");

    // 检查配置表是否存在
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='config'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    result.push_str(&format!(
        "配置表存在: {}\n",
        if table_exists > 0 { "是" } else { "否" }
    ));

    // 检查是否有配置记录
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM config WHERE id = 1", [], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;
    result.push_str(&format!("配置记录数: {}\n", count));

    if count > 0 {
        let config: Option<String> = conn
            .query_row("SELECT value FROM config WHERE id = 1", [], |row| {
                row.get(0)
            })
            .ok();

        if let Some(config_str) = &config {
            result.push_str(&format!("配置数据长度: {} bytes\n", config_str.len()));
            result.push_str(&format!(
                "配置数据预览: {}\n",
                &config_str[..std::cmp::min(config_str.len(), 200)]
            ));

            // 尝试解析 JSON
            match serde_json::from_str::<serde_json::Value>(&config_str) {
                Ok(json) => {
                    result.push_str("JSON解析: 成功\n");
                    result.push_str(&format!(
                        "配置字段数: {}\n",
                        json.as_object().map(|o| o.len()).unwrap_or(0)
                    ));
                }
                Err(e) => {
                    result.push_str(&format!("JSON解析: 失败 - {}\n", e));
                }
            }
        } else {
            result.push_str("配置数据: 为空\n");
        }
    }

    Ok(result)
}

#[tauri::command]
pub fn db_reset_config(default_config: String) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 删除旧配置
    conn.execute("DELETE FROM config WHERE id = 1", [])
        .map_err(|e| e.to_string())?;

    // 插入新配置
    conn.execute(
        "INSERT INTO config (id, value) VALUES (1, ?)",
        params![default_config],
    )
    .map_err(|e| e.to_string())?;

    Ok(format!(
        "✓ 配置已重置\n  新配置长度: {} bytes",
        default_config.len()
    ))
}

#[tauri::command]
pub fn db_save_config(config: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO config (id, value) VALUES (1, ?)
         ON CONFLICT(id) DO UPDATE SET value = excluded.value",
        params![config],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_add_record(record: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&record).map_err(|e| e.to_string())?;

    let id = parsed["id"].as_str().unwrap_or_default().to_string();

    let metadata = record_lookup_metadata_from_json(&parsed);

    conn.execute(
        "INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            data = excluded.data,
            raid_name = excluded.raid_name,
            account_id = excluded.account_id,
            role_id = excluded.role_id,
            record_date = excluded.record_date,
            record_type = excluded.record_type",
        params![
            id,
            record,
            metadata.raid_name,
            metadata.account_id,
            metadata.role_id,
            metadata.record_date,
            metadata.record_type
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn db_delete_record(record_id: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM records WHERE id = ?", params![record_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_records_by_raid(raid_id: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT data FROM records
             WHERE raid_name = ?
             ORDER BY record_date DESC",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query(params![&raid_id]).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if let Ok(data) = row.get(0) {
            records.push(data);
        }
    }
    Ok(records)
}

#[allow(dead_code)]
#[tauri::command]
pub fn db_analyze_duplicates() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut result = String::new();

    result.push_str("=== 数据库重复分析报告 ===\n\n");

    // 分析账号重复
    result.push_str("【账号表 (accounts) 分析】\n");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));

    // 检查是否有重复的 id（理论上不应该有，因为是 PRIMARY KEY）
    let duplicate_accounts: Vec<(String, i64)> = conn
        .prepare(
            "
        SELECT id, COUNT(*) as cnt 
        FROM accounts 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ",
        )
        .map_err(|e| e.to_string())?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if duplicate_accounts.is_empty() {
        result.push_str("✓ 无重复账号（id 字段）\n");
    } else {
        result.push_str(&format!(
            "✗ 发现 {} 个重复的 id:\n",
            duplicate_accounts.len()
        ));
        for (id, cnt) in &duplicate_accounts {
            result.push_str(&format!("  - {}: {} 次\n", id, cnt));
        }
    }

    // 分析记录重复
    result.push_str("\n【记录表 (records) 分析】\n");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));

    let duplicate_records: Vec<(String, i64)> = conn
        .prepare(
            "
        SELECT id, COUNT(*) as cnt 
        FROM records 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ",
        )
        .map_err(|e| e.to_string())?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if duplicate_records.is_empty() {
        result.push_str("✓ 无重复记录\n");
    } else {
        result.push_str(&format!(
            "✗ 发现 {} 个重复的 id:\n",
            duplicate_records.len()
        ));
        for (id, cnt) in &duplicate_records {
            result.push_str(&format!("  - {}: {} 次\n", id, cnt));
        }
    }

    // 分析副本重复（这个可能有重复因为 name 可能相同但日期不同）
    result.push_str("\n【副本表 (raids) 分析】\n");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM raids", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));

    let duplicate_raids: Vec<(String, i64)> = conn
        .prepare(
            "
        SELECT id, COUNT(*) as cnt 
        FROM raids 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ",
        )
        .map_err(|e| e.to_string())?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    if duplicate_raids.is_empty() {
        result.push_str("✓ 无重复副本\n");
    } else {
        result.push_str(&format!("✗ 发现 {} 个重复的 id:\n", duplicate_raids.len()));
        for (id, cnt) in &duplicate_raids {
            result.push_str(&format!("  - {}: {} 次\n", id, cnt));
        }
    }

    // 添加唯一性建议
    result.push_str("\n【唯一性约束建议】\n");
    result.push_str("- accounts.id: 已有 PRIMARY KEY 约束\n");
    result.push_str("- records.id: 已有 PRIMARY KEY 约束\n");
    result.push_str("- raids.id: 建议改用唯一标识符或复合主键 (name, date)\n");

    Ok(result)
}

#[allow(dead_code)]
#[tauri::command]
pub fn db_add_unique_constraint_raids() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 由于 raids 表使用 name 作为 id，可能存在重复，
    // 我们先尝试添加唯一索引，如果失败说明有重复数据
    match conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_raids_id ON raids(id)",
        [],
    ) {
        Ok(_) => Ok(
            "✓ 已添加 raids.id 唯一索引\n  (注意: 如果存在重复数据，索引创建会失败)".to_string(),
        ),
        Err(e) => Ok(format!(
            "⚠ 添加唯一索引失败: {}\n  原因: 表中存在重复的 id\n  建议: 先执行副本去重操作",
            e
        )),
    }
}

#[tauri::command]
pub fn db_backup(backup_path: String) -> Result<(), String> {
    let src_path = get_db_path()?;
    if !src_path.exists() {
        return Err("数据库文件不存在".to_string());
    }

    let dest_path = PathBuf::from(&backup_path);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建备份目录失败: {}", e))?;
    }

    let src_conn = init_db().map_err(|e| e.to_string())?;

    src_conn
        .backup(rusqlite::DatabaseName::Main, &dest_path, None)
        .map_err(|e| format!("备份数据库失败: {}", e))?;

    log::info!("数据库备份完成: {:?}", backup_path);
    Ok(())
}

#[tauri::command]
pub fn db_restore(restore_path: String) -> Result<(), String> {
    let dest_path = get_db_path()?;
    let src_path = PathBuf::from(&restore_path);

    if !src_path.exists() {
        return Err("恢复文件不存在".to_string());
    }

    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    let src_conn = rusqlite::Connection::open(&src_path)
        .map_err(|e| format!("打开恢复数据库失败: {}", e))?;

    src_conn
        .backup(rusqlite::DatabaseName::Main, &dest_path, None)
        .map_err(|e| format!("恢复数据库失败: {}", e))?;

    {
        let mut initialized = DB_INITIALIZED.lock().unwrap();
        *initialized = false;
    }

    log::info!("数据库恢复完成，请重启应用以加载新数据");
    Ok(())
}

#[tauri::command]
pub fn db_get_cache(key: String) -> Result<Option<(String, String)>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT value, updated_at FROM cache WHERE key = ?")
        .map_err(|e| e.to_string())?;
    let result: Option<(String, String)> = stmt
        .query_row(params![key], |row| Ok((row.get(0)?, row.get(1)?)))
        .ok();
    Ok(result)
}

#[tauri::command]
pub fn db_save_cache(key: String, value: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let updated_at = get_local_timestamp();
    conn.execute(
        "INSERT INTO cache (key, value, updated_at) VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, updated_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ========== 副本收藏相关命令 ==========

/// 获取所有收藏的副本名称列表（按版本顺序排序，同一版本内按 level 倒序）
#[tauri::command]
pub fn db_get_favorite_raids() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 使用 rowid 作为排序依据
    // raid_versions.id 越大版本越新，rowid 越大副本越靠后（静态数据中）
    let mut stmt = conn
        .prepare(
            r#"
            SELECT DISTINCT f.raid_name, r.rowid
            FROM favorite_raids f
            LEFT JOIN raids r ON f.raid_name = r.name
            LEFT JOIN raid_versions rv ON r.version = rv.name
            ORDER BY COALESCE(rv.id, 0) DESC, r.rowid DESC
            "#,
        )
        .map_err(|e| e.to_string())?;

    let names: Vec<String> = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(names)
}

/// 添加收藏副本
#[tauri::command]
pub fn db_add_favorite_raid(raid_name: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let created_at = get_local_timestamp();

    conn.execute(
        "INSERT OR IGNORE INTO favorite_raids (raid_name, created_at) VALUES (?, ?)",
        params![raid_name, created_at],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 移除收藏副本
#[tauri::command]
pub fn db_remove_favorite_raid(raid_name: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    conn.execute(
        "DELETE FROM favorite_raids WHERE raid_name = ?",
        params![raid_name],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 检查副本是否已收藏
#[tauri::command]
pub fn db_is_favorite_raid(raid_name: String) -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM favorite_raids WHERE raid_name = ?",
            params![raid_name],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    Ok(count > 0)
}

// ========== 角色可见性配置相关命令 (V5+) ==========

/// 获取所有副本类型
#[tauri::command]
pub fn db_get_instance_types() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let types: Vec<serde_json::Value> = conn
        .prepare("SELECT id, type, name FROM instance_types ORDER BY id")
        .map_err(|e| e.to_string())?
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, i32>(0)?,
                "type": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&types).map_err(|e| format!("序列化失败: {}", e))
}

/// 获取所有角色的可见性配置
#[tauri::command]
pub fn db_get_all_role_visibility() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let visibility: Vec<serde_json::Value> = conn
        .prepare(
            "SELECT riv.id, riv.role_id, riv.instance_type_id, it.type, riv.visible
             FROM role_instance_visibility riv
             JOIN instance_types it ON riv.instance_type_id = it.id",
        )
        .map_err(|e| e.to_string())?
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "roleId": row.get::<_, String>(1)?,
                "instanceTypeId": row.get::<_, i32>(2)?,
                "instanceType": row.get::<_, String>(3)?,
                "visible": row.get::<_, i32>(4)? == 1
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&visibility).map_err(|e| format!("序列化失败: {}", e))
}

/// 保存单个角色的可见性配置（用于账号管理中的大类配置：raid/baizhan/trial）
#[tauri::command]
pub fn db_save_role_visibility(
    role_id: String,
    instance_type: String,
    visible: bool,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 获取 instance_type_id（只允许预定义的类型）
    let instance_type_id: i32 = conn
        .query_row(
            "SELECT id FROM instance_types WHERE type = ?1",
            params![instance_type],
            |row| row.get(0),
        )
        .map_err(|e| format!("未找到副本类型: {}", e))?;

    let timestamp = get_local_timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO role_instance_visibility (id, role_id, instance_type_id, visible, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(role_id, instance_type_id)
         DO UPDATE SET visible = ?4, updated_at = ?5",
        params![id, role_id, instance_type_id, visible as i32, &timestamp],
    )
    .map_err(|e| format!("保存可见性失败: {}", e))?;

    Ok(())
}

// ========== 团队副本角色可见性配置 (V6+) ==========

/// 获取指定副本的所有角色可见性配置
#[allow(non_snake_case)]
#[tauri::command]
pub fn db_get_raid_role_visibility(raid_key: String) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let visibility: Vec<serde_json::Value> = conn
        .prepare("SELECT role_id, visible FROM raid_role_visibility WHERE raid_key = ?1")
        .map_err(|e| e.to_string())?
        .query_map(params![raid_key], |row| {
            Ok(serde_json::json!({
                "roleId": row.get::<_, String>(0)?,
                "visible": row.get::<_, i32>(1)? == 1
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    serde_json::to_string(&visibility).map_err(|e| format!("序列化失败: {}", e))
}

/// 保存团队副本中单个角色的可见性配置
#[allow(non_snake_case)]
#[tauri::command]
pub fn db_save_raid_role_visibility(
    roleId: String,
    raidKey: String,
    visible: bool,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| format!("初始化数据库失败: {}", e))?;

    let timestamp = get_local_timestamp();
    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO raid_role_visibility (id, role_id, raid_key, visible, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(role_id, raid_key)
         DO UPDATE SET visible = ?4, updated_at = ?5",
        params![&id, &roleId, &raidKey, visible as i32, &timestamp],
    )
    .map_err(|e| format!("保存团队副本角色可见性失败: {}", e))?;

    Ok(())
}

// ========== 数据目录管理 ==========

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDirInfo {
    pub current_path: String,
    pub location: String, // "custom" | "install" | "user_home"
    pub is_install_mode: bool,
    pub custom_dir_configured: bool,
}

/// 获取当前数据目录信息
#[tauri::command]
pub fn db_get_data_dir_info() -> Result<DataDirInfo, String> {
    let (app_dir, location, is_install, custom_configured) = get_app_dir_info()?;

    Ok(DataDirInfo {
        current_path: app_dir.to_string_lossy().to_string(),
        location,
        is_install_mode: is_install,
        custom_dir_configured: custom_configured,
    })
}

fn write_custom_data_dir_config(
    path: Option<&Path>,
    migration_source: Option<&Path>,
) -> Result<(), String> {
    let mut config = read_data_dir_bootstrap_config()?;
    config.custom_data_dir = path.map(|item| item.to_string_lossy().to_string());
    config.pending_migration_from = migration_source
        .filter(|item| !item.as_os_str().is_empty())
        .map(|item| item.to_string_lossy().to_string());
    write_data_dir_bootstrap_config(&config)
}

/// 设置自定义数据目录（持久化保存，重启后生效）
#[tauri::command]
pub fn db_set_custom_data_dir(path: String) -> Result<String, String> {
    let dir_path = PathBuf::from(&path);

    if dir_path.as_os_str().is_empty() {
        return Err("目录不能为空".to_string());
    }

    ensure_directory_exists(&dir_path)?;
    let current_dir = get_app_dir()?;

    let migration_source = if directories_match(&current_dir, &dir_path)? {
        None
    } else {
        Some(current_dir.as_path())
    };

    write_custom_data_dir_config(Some(dir_path.as_path()), migration_source)?;
    log::info!("自定义数据目录已保存，将在重启后启用: {:?}", dir_path);
    Ok(dir_path.to_string_lossy().to_string())
}

/// 恢复默认数据目录（安装版为安装目录，非安装版为用户目录）
#[tauri::command]
pub fn db_reset_custom_data_dir() -> Result<String, String> {
    let current_dir = get_app_dir()?;
    let install_mode = is_install_mode();
    let default_dir = if install_mode {
        get_install_dir().ok_or_else(|| "无法获取安装目录".to_string())?
    } else {
        get_home_app_dir()?
    };

    let migration_source = if directories_match(&current_dir, &default_dir)? {
        None
    } else {
        Some(current_dir.as_path())
    };

    ensure_directory_exists(&default_dir)?;
    write_custom_data_dir_config(None, migration_source)?;
    log::info!("已恢复默认数据目录，将在重启后启用: {:?}", default_dir);
    Ok(default_dir.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestTempDir {
        path: PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let unique = format!(
                "jx3-raid-manager-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time should be after unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).expect("temp dir should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).ok();
        }
    }

    #[test]
    fn preserves_account_without_roles_as_empty_roles_array() {
        let default_visibility = serde_json::Map::new();
        let mut account_map = std::collections::HashMap::new();

        upsert_account_from_join_row(
            &mut account_map,
            AccountJoinRow {
                account_id: "account-1".to_string(),
                account_name: "测试账号".to_string(),
                account_type: "OWN".to_string(),
                sort_order: 0,
                password: Some("secret".to_string()),
                notes: Some("notes".to_string()),
                hidden: false,
                disabled: false,
                created_at: None,
                updated_at: None,
                role: None,
            },
            &std::collections::HashMap::new(),
            &default_visibility,
        );

        let account = account_map.get("account-1").expect("account should exist");
        assert_eq!(account["id"], "account-1");
        assert_eq!(account["accountName"], "测试账号");
        assert_eq!(account["roles"], serde_json::json!([]));
    }

    #[test]
    fn appends_role_when_join_row_contains_role_data() {
        let mut default_visibility = serde_json::Map::new();
        default_visibility.insert("raid".to_string(), serde_json::json!(true));

        let mut visibility = std::collections::HashMap::new();
        let mut role_visibility = serde_json::Map::new();
        role_visibility.insert("raid".to_string(), serde_json::json!(false));
        visibility.insert("role-1".to_string(), role_visibility);

        let mut account_map = std::collections::HashMap::new();
        upsert_account_from_join_row(
            &mut account_map,
            AccountJoinRow {
                account_id: "account-1".to_string(),
                account_name: "测试账号".to_string(),
                account_type: "OWN".to_string(),
                sort_order: 0,
                password: None,
                notes: None,
                hidden: false,
                disabled: false,
                created_at: None,
                updated_at: None,
                role: Some(RoleJoinRow {
                    role_id: "role-1".to_string(),
                    account_id: Some("account-1".to_string()),
                    name: Some("角色A".to_string()),
                    server: Some("电一".to_string()),
                    region: Some("梦江南".to_string()),
                    sect: Some("太虚".to_string()),
                    martial: None,
                    equipment_score: Some(123456),
                    disabled: false,
                }),
            },
            &visibility,
            &default_visibility,
        );

        let account = account_map.get("account-1").expect("account should exist");
        assert_eq!(
            account["roles"].as_array().map(|roles| roles.len()),
            Some(1)
        );
        assert_eq!(account["roles"][0]["id"], "role-1");
        assert_eq!(account["roles"][0]["visibility"]["raid"], false);
    }

    #[test]
    fn migrates_data_files_by_removing_source_files() {
        let temp_dir = TestTempDir::new();
        let source_dir = temp_dir.path().join("source");
        let target_dir = temp_dir.path().join("target");

        fs::create_dir_all(&source_dir).expect("source dir should exist");
        fs::create_dir_all(&target_dir).expect("target dir should exist");
        fs::write(source_dir.join(DATABASE_NAME), "db-content").expect("db file should be written");
        fs::write(source_dir.join(LOG_FILE_NAME), "log-content")
            .expect("log file should be written");

        let mut config = DataDirBootstrapConfig {
            custom_data_dir: Some(target_dir.to_string_lossy().to_string()),
            pending_migration_from: Some(source_dir.to_string_lossy().to_string()),
        };

        maybe_migrate_app_data(&target_dir, &mut config).expect("migration should succeed");

        assert!(
            target_dir.join(DATABASE_NAME).exists(),
            "db file should exist in target"
        );
        assert!(
            target_dir.join(LOG_FILE_NAME).exists(),
            "log file should exist in target"
        );
        assert!(
            !source_dir.join(DATABASE_NAME).exists(),
            "db file should be removed from source after migration"
        );
        assert!(
            !source_dir.join(LOG_FILE_NAME).exists(),
            "log file should be removed from source after migration"
        );
        assert!(
            config.pending_migration_from.is_none(),
            "pending migration should be cleared"
        );
    }

    #[test]
    fn migrates_only_managed_files_when_target_is_nested_in_source() {
        let temp_dir = TestTempDir::new();
        let source_dir = temp_dir.path().join("source");
        let target_dir = source_dir.join("nested-target");
        let unrelated_dir = source_dir.join("screenshots");

        fs::create_dir_all(&source_dir).expect("source dir should exist");
        fs::create_dir_all(&unrelated_dir).expect("unrelated dir should exist");
        fs::write(source_dir.join(DATABASE_NAME), "db-content").expect("db file should be written");
        fs::write(source_dir.join(LOG_FILE_NAME), "log-content")
            .expect("log file should be written");
        fs::write(unrelated_dir.join("note.txt"), "keep")
            .expect("unrelated file should be written");

        let changed = migrate_managed_app_data_files(&source_dir, &target_dir)
            .expect("migration should succeed for nested target");

        assert!(changed, "managed files should be moved");
        assert!(
            target_dir.join(DATABASE_NAME).exists(),
            "db file should exist in nested target"
        );
        assert!(
            target_dir.join(LOG_FILE_NAME).exists(),
            "log file should exist in nested target"
        );
        assert!(
            !source_dir.join(DATABASE_NAME).exists(),
            "db file should be removed from source"
        );
        assert!(
            !source_dir.join(LOG_FILE_NAME).exists(),
            "log file should be removed from source"
        );
        assert!(
            unrelated_dir.join("note.txt").exists(),
            "unrelated file should be preserved"
        );
        assert!(
            !target_dir.join("nested-target").exists(),
            "nested target should not recursively contain itself"
        );
    }

    #[test]
    fn skips_migration_when_target_is_same_directory_alias() {
        let temp_dir = TestTempDir::new();
        let source_dir = temp_dir.path().join("source");
        let target_dir = source_dir.join(".");

        fs::create_dir_all(&source_dir).expect("source dir should exist");
        fs::write(source_dir.join(DATABASE_NAME), "db-content").expect("db file should be written");

        let changed = migrate_managed_app_data_files(&source_dir, &target_dir)
            .expect("same directory alias should not error");

        assert!(!changed, "same directory should not trigger migration");
        assert!(
            source_dir.join(DATABASE_NAME).exists(),
            "source file should remain in place when target is the same directory"
        );
    }

    #[test]
    fn filters_record_json_by_raid_name() {
        let records = vec![
            r#"{"id":"1","raidName":"25人英雄阕风悬城"}"#.to_string(),
            r#"{"id":"2","raidName":"百战异闻录"}"#.to_string(),
            r#"{"id":"3","raidName":"25人英雄阕风悬城"}"#.to_string(),
        ];

        let filtered: Vec<&String> = records
            .iter()
            .filter(|r| r.contains("阕风悬城"))
            .collect();

        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|record| record.contains("阕风悬城")));
    }

    fn create_test_conn() -> (Connection, PathBuf) {
        let temp_dir = TestTempDir::new();
        let db_path = temp_dir.path().join("test.db");
        let conn = Connection::open(&db_path).expect("should open test db");
        conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE; PRAGMA foreign_keys=ON;")
            .ok();
        (conn, db_path)
    }

    fn get_table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .expect("should prepare");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("should query")
            .filter_map(|r| r.ok())
            .collect();
        names
    }

    fn get_index_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .expect("should prepare");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .expect("should query")
            .filter_map(|r| r.ok())
            .collect();
        names
    }

    fn get_column_names(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .expect("should prepare");
        let names: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("should query")
            .filter_map(|r| r.ok())
            .collect();
        names
    }

    #[test]
    fn fresh_install_creates_all_tables_and_indexes() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        install_fresh_db(&conn).expect("fresh install should succeed");

        let expected_tables = [
            "accounts", "baizhan_records", "cache", "config", "equipments",
            "favorite_raids", "game_versions", "instance_types", "migration_flags",
            "raid_bosses", "raid_role_visibility", "raid_versions", "raids",
            "records", "role_instance_visibility", "roles", "schema_versions",
            "seasons", "trial_records",
        ];

        let tables = get_table_names(&conn);
        for table in &expected_tables {
            assert!(
                tables.contains(&table.to_string()),
                "缺少预期表: {}，现有表: {:?}",
                table, tables
            );
        }

        let expected_indexes = [
            "idx_accounts_name", "idx_accounts_sort_order",
            "idx_riv_instance_type_id", "idx_riv_role_id",
            "idx_raids_season_id", "idx_records_account_id",
            "idx_records_raid_name", "idx_records_record_date",
            "idx_records_role_id", "idx_roles_account_id",
            "idx_rrv_raid_key", "idx_rrv_role_id",
            "idx_seasons_version_id",
        ];

        let indexes = get_index_names(&conn);
        for idx in &expected_indexes {
            assert!(
                indexes.contains(&idx.to_string()),
                "缺少预期索引: {}，现有索引: {:?}",
                idx, indexes
            );
        }

        let version = get_schema_version(&conn).expect("should get version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION, "版本号应为 {}", CURRENT_SCHEMA_VERSION);

        let instance_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM instance_types", [], |row| row.get(0))
            .expect("should count");
        assert_eq!(instance_count, 3, "应有 3 个副本类型初始数据");
    }

    #[test]
    fn static_sync_restores_gongyuecheng_25_normal() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        install_fresh_db(&conn).expect("fresh install should succeed");

        conn.execute("DELETE FROM raids WHERE id = '25人普通会战弓月城'", [])
            .expect("should delete test raid");

        let before_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM raids WHERE id = '25人普通会战弓月城'", [], |row| row.get(0))
            .expect("should count before sync");
        assert_eq!(before_count, 0);

        migration::init_static_raids(&conn).expect("static sync should succeed");

        let restored: (String, i64, String, i64) = conn
            .query_row(
                "SELECT name, player_count, difficulty, is_active FROM raids WHERE id = '25人普通会战弓月城'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("should restore raid");

        assert_eq!(restored, ("会战弓月城".to_string(), 25, "普通".to_string(), 1));
    }

    #[test]
    fn fresh_install_equipments_has_all_columns() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        install_fresh_db(&conn).expect("fresh install should succeed");

        let expected_columns = [
            "id", "name", "ui_id", "icon_id", "level", "quality",
            "bind_type", "type_label", "attribute_types", "attributes",
            "recommend", "diamonds", "data", "updated_at",
        ];

        let columns = get_column_names(&conn, "equipments");
        for col in &expected_columns {
            assert!(
                columns.contains(&col.to_string()),
                "equipments 表缺少列: {}，现有列: {:?}",
                col, columns
            );
        }
    }

    #[test]
    fn v0_upgrade_creates_all_tables() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");

        upgrade_db(&conn, 0).expect("V0 upgrade should succeed");

        let expected_tables = [
            "accounts", "baizhan_records", "cache", "config", "equipments",
            "favorite_raids", "game_versions", "instance_types", "migration_flags",
            "raid_bosses", "raid_role_visibility", "raid_versions", "raids",
            "records", "role_instance_visibility", "roles", "schema_versions",
            "seasons", "trial_records",
        ];

        let tables = get_table_names(&conn);
        for table in &expected_tables {
            assert!(
                tables.contains(&table.to_string()),
                "V0 升级后缺少表: {}",
                table
            );
        }

        let version = get_schema_version(&conn).expect("should get version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION, "V0 升级后版本号应为 V{}", CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn v0_upgrade_equipments_has_all_columns() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        upgrade_db(&conn, 0).expect("V0 upgrade should succeed");

        let expected_columns = [
            "id", "name", "ui_id", "icon_id", "level", "quality",
            "bind_type", "type_label", "attribute_types", "attributes",
            "recommend", "diamonds", "data", "updated_at",
        ];

        let columns = get_column_names(&conn, "equipments");
        for col in &expected_columns {
            assert!(
                columns.contains(&col.to_string()),
                "V0 升级后 equipments 表缺少列: {}",
                col
            );
        }
    }

    #[test]
    fn intermediate_version_upgrade_succeeds() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        ensure_baseline_tables(&conn).expect("baseline tables should be created");

        for v in 1..=7 {
            conn.execute("BEGIN TRANSACTION", []).expect("begin tx");
            migration::apply_migration(&conn, v).expect("migration should succeed");
            set_schema_version(&conn, v, &format!("升级到 V{}", v)).expect("set version");
            conn.execute("COMMIT", []).expect("commit");
        }

        let version_before = get_schema_version(&conn).expect("should get version");
        assert_eq!(version_before, 7, "升级前版本应为 V7");

        upgrade_db(&conn, 7).expect("V7 升级应成功");

        let version_after = get_schema_version(&conn).expect("should get version");
        assert_eq!(version_after, CURRENT_SCHEMA_VERSION, "V7 升级后版本应为 V{}", CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn pre_upgrade_validation_rejects_corrupt_database() {
        let (conn, _path) = create_test_conn();

        let result = validate_pre_upgrade(&conn, -1);
        assert!(result.is_err(), "负版本号应被拒绝");

        let result = validate_pre_upgrade(&conn, CURRENT_SCHEMA_VERSION + 1);
        assert!(result.is_err(), "超范围版本号应被拒绝");
    }

    #[test]
    fn post_upgrade_validation_detects_missing_table() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        set_schema_version(&conn, CURRENT_SCHEMA_VERSION, "测试").expect("set version");

        let result = validate_post_upgrade(&conn);
        assert!(result.is_err(), "缺少表时应验证失败");
    }

    #[test]
    fn ensure_equipment_columns_adds_missing_columns() {
        let (conn, _path) = create_test_conn();

        conn.execute_batch(
            r#"
            CREATE TABLE equipments (
                id TEXT PRIMARY KEY,
                name TEXT,
                ui_id TEXT,
                icon_id INTEGER,
                level INTEGER,
                quality TEXT,
                data TEXT,
                updated_at TEXT
            );
            "#,
        ).expect("should create minimal equipments table");

        ensure_equipment_columns(&conn).expect("should add missing columns");

        let columns = get_column_names(&conn, "equipments");
        let expected_new = ["bind_type", "type_label", "attribute_types", "attributes", "recommend", "diamonds"];
        for col in &expected_new {
            assert!(
                columns.contains(&col.to_string()),
                "ensure_equipment_columns 应添加列: {}",
                col
            );
        }
    }

    #[test]
    fn ensure_equipment_columns_idempotent() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        install_fresh_db(&conn).expect("fresh install should succeed");

        let result = ensure_equipment_columns(&conn);
        assert!(result.is_ok(), "对已有完整列的表调用 ensure_equipment_columns 不应报错");
    }

    #[test]
    fn schema_version_records_all_migrations() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        upgrade_db(&conn, 0).expect("V0 upgrade should succeed");

        let mut stmt = conn
            .prepare("SELECT version FROM schema_versions ORDER BY version")
            .expect("should prepare");
        let versions: Vec<i32> = stmt
            .query_map([], |row| row.get(0))
            .expect("should query")
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(versions.len(), CURRENT_SCHEMA_VERSION as usize, "应有 {} 条版本记录", CURRENT_SCHEMA_VERSION);
        for (i, v) in versions.iter().enumerate() {
            assert_eq!(*v, (i + 1) as i32, "版本记录应按序排列");
        }
    }

    fn simulate_upgrade_from_version(conn: &Connection, from_version: i32) -> Result<(), String> {
        ensure_version_tables(conn)?;

        ensure_baseline_tables(conn)?;

        for v in 1..=from_version {
            conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;
            migration::apply_migration(conn, v)?;
            set_schema_version(conn, v, &format!("升级到 V{}", v))?;
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
        }

        upgrade_db(conn, from_version)?;

        Ok(())
    }

    fn verify_final_database_state(conn: &Connection) -> Result<(), String> {
        let version = get_schema_version(conn)?;
        if version != CURRENT_SCHEMA_VERSION {
            return Err(format!("版本不匹配: 期望 V{}, 实际 V{}", CURRENT_SCHEMA_VERSION, version));
        }

        let expected_tables = [
            "accounts", "baizhan_records", "cache", "config", "equipments",
            "favorite_raids", "game_versions", "instance_types", "migration_flags",
            "raid_bosses", "raid_role_visibility", "raid_versions", "raids",
            "records", "role_instance_visibility", "roles", "schema_versions",
            "seasons", "trial_records",
        ];

        for table in &expected_tables {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?",
                    [table],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            if exists == 0 {
                return Err(format!("缺少表: {}", table));
            }
        }

        let expected_columns = [
            "id", "name", "ui_id", "icon_id", "level", "quality",
            "bind_type", "type_label", "attribute_types", "attributes",
            "recommend", "diamonds", "data", "updated_at",
        ];

        for col in &expected_columns {
            if !column_exists(conn, "equipments", col)? {
                return Err(format!("equipments 表缺少列: {}", col));
            }
        }

        Ok(())
    }

    #[test]
    fn upgrade_from_v1_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 1).expect("V1 升级应成功");
        verify_final_database_state(&conn).expect("V1 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v2_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 2).expect("V2 升级应成功");
        verify_final_database_state(&conn).expect("V2 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v3_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 3).expect("V3 升级应成功");
        verify_final_database_state(&conn).expect("V3 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v4_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 4).expect("V4 升级应成功");
        verify_final_database_state(&conn).expect("V4 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v5_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 5).expect("V5 升级应成功");
        verify_final_database_state(&conn).expect("V5 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v6_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 6).expect("V6 升级应成功");
        verify_final_database_state(&conn).expect("V6 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v7_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 7).expect("V7 升级应成功");
        verify_final_database_state(&conn).expect("V7 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v8_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 8).expect("V8 升级应成功");
        verify_final_database_state(&conn).expect("V8 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v9_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 9).expect("V9 升级应成功");
        verify_final_database_state(&conn).expect("V9 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v10_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 10).expect("V10 升级应成功");
        verify_final_database_state(&conn).expect("V10 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v11_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 11).expect("V11 升级应成功");
        verify_final_database_state(&conn).expect("V11 升级后数据库状态验证失败");
    }

    #[test]
    fn upgrade_from_v12_to_latest() {
        let (conn, _path) = create_test_conn();
        simulate_upgrade_from_version(&conn, 12).expect("V12 升级应成功");
        verify_final_database_state(&conn).expect("V12 升级后数据库状态验证失败");
    }

    #[test]
    fn v1_migration_preserves_account_data() {
        let (conn, _path) = create_test_conn();

        conn.execute_batch(
            r#"
            CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                data TEXT
            );

            INSERT INTO accounts (id, data) VALUES ('acc-1', '{"accountName":"测试账号","type":"OWN","password":"test123","notes":"备注","hidden":false,"disabled":false,"roles":[{"id":"role-1","name":"角色A","server":"电信五区","sect":"天策"}]}');
            INSERT INTO accounts (id, data) VALUES ('acc-2', '{"accountName":"账号B","type":"SHARE","roles":[]}');
            "#,
        ).expect("should create old accounts table");

        ensure_version_tables(&conn).expect("version tables should be created");
        
        conn.execute("BEGIN TRANSACTION", []).expect("begin tx");
        migration::apply_migration(&conn, 1).expect("V1 migration should succeed");
        set_schema_version(&conn, 1, "升级到 V1").expect("set version");
        conn.execute("COMMIT", []).expect("commit");

        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .expect("should count accounts");
        assert_eq!(account_count, 2, "应保留 2 个账号");

        let role_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM roles", [], |row| row.get(0))
            .expect("should count roles");
        assert_eq!(role_count, 1, "应有 1 个角色");

        let account_name: String = conn
            .query_row("SELECT account_name FROM accounts WHERE id = 'acc-1'", [], |row| row.get(0))
            .expect("should get account name");
        assert_eq!(account_name, "测试账号", "账号名应正确迁移");

        let role_name: String = conn
            .query_row("SELECT name FROM roles WHERE id = 'role-1'", [], |row| row.get(0))
            .expect("should get role name");
        assert_eq!(role_name, "角色A", "角色名应正确迁移");

        let legacy_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts_legacy'", [], |row| row.get(0))
            .expect("should check legacy table");
        assert_eq!(legacy_exists, 1, "旧表应重命名为 accounts_legacy");
    }

    #[test]
    fn v3_migration_converts_date_to_integer() {
        let (conn, _path) = create_test_conn();

        conn.execute_batch(
            r#"
            CREATE TABLE trial_records (
                id TEXT PRIMARY KEY,
                account_id TEXT,
                role_id TEXT,
                layer INTEGER,
                bosses TEXT,
                card_1 TEXT,
                card_2 TEXT,
                card_3 TEXT,
                card_4 TEXT,
                card_5 TEXT,
                flipped_index INTEGER,
                date TEXT,
                notes TEXT,
                updated_at TEXT
            );

            INSERT INTO trial_records (id, account_id, role_id, layer, bosses, date, notes) 
            VALUES ('trial-1', 'acc-1', 'role-1', 10, '[]', '1704067200000', '测试记录');
            "#,
        ).expect("should create old trial_records table");

        ensure_version_tables(&conn).expect("version tables should be created");
        
        conn.execute("BEGIN TRANSACTION", []).expect("begin tx");
        migration::apply_migration(&conn, 3).expect("V3 migration should succeed");
        set_schema_version(&conn, 3, "升级到 V3").expect("set version");
        conn.execute("COMMIT", []).expect("commit");

        let date_type: String = conn
            .query_row("SELECT type FROM pragma_table_info('trial_records') WHERE name='date'", [], |row| row.get(0))
            .expect("should get date type");
        assert_eq!(date_type.to_uppercase(), "INTEGER", "date 字段应为 INTEGER 类型");

        let date_value: i64 = conn
            .query_row("SELECT date FROM trial_records WHERE id = 'trial-1'", [], |row| row.get(0))
            .expect("should get date value");
        assert_eq!(date_value, 1704067200000, "date 值应正确转换");

        let role_name_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('trial_records') WHERE name='role_name'", [], |row| row.get(0))
            .expect("should check role_name column");
        assert_eq!(role_name_exists, 1, "应有 role_name 字段");

        let record_type_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('trial_records') WHERE name='record_type'", [], |row| row.get(0))
            .expect("should check record_type column");
        assert_eq!(record_type_exists, 1, "应有 record_type 字段");
    }

    #[test]
    fn v10_migration_creates_season_data() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        ensure_baseline_tables(&conn).expect("baseline tables should be created");

        for v in 1..=9 {
            conn.execute("BEGIN TRANSACTION", []).expect("begin tx");
            migration::apply_migration(&conn, v).expect(&format!("V{} migration should succeed", v));
            set_schema_version(&conn, v, &format!("升级到 V{}", v)).expect("set version");
            conn.execute("COMMIT", []).expect("commit");
        }

        conn.execute("BEGIN TRANSACTION", []).expect("begin tx");
        migration::apply_migration(&conn, 10).expect("V10 migration should succeed");
        set_schema_version(&conn, 10, "升级到 V10").expect("set version");
        conn.execute("COMMIT", []).expect("commit");

        let version_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM game_versions", [], |row| row.get(0))
            .expect("should count versions");
        assert!(version_count > 0, "应有版本数据");

        let season_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM seasons", [], |row| row.get(0))
            .expect("should count seasons");
        assert!(season_count > 0, "应有赛季数据");

        let season_id_exists: i64 = conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('raids') WHERE name='season_id'", [], |row| row.get(0))
            .expect("should check season_id column");
        assert_eq!(season_id_exists, 1, "raids 表应有 season_id 字段");

        let idx_seasons_version_id: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_seasons_version_id'", [], |row| row.get(0))
            .expect("should check index");
        assert_eq!(idx_seasons_version_id, 1, "应有 idx_seasons_version_id 索引");
    }

    #[test]
    fn full_upgrade_preserves_data_integrity() {
        let (conn, _path) = create_test_conn();

        conn.execute_batch(
            r#"
            CREATE TABLE accounts (
                id TEXT PRIMARY KEY,
                data TEXT
            );

            INSERT INTO accounts (id, data) VALUES ('acc-1', '{"accountName":"完整测试账号","type":"OWN","roles":[{"id":"role-1","name":"测试角色","server":"电信五区","sect":"纯阳"}]}');

            CREATE TABLE trial_records (
                id TEXT PRIMARY KEY,
                account_id TEXT,
                role_id TEXT,
                layer INTEGER,
                bosses TEXT,
                card_1 TEXT,
                card_2 TEXT,
                card_3 TEXT,
                card_4 TEXT,
                card_5 TEXT,
                flipped_index INTEGER,
                date TEXT,
                notes TEXT,
                updated_at TEXT
            );

            INSERT INTO trial_records (id, account_id, role_id, layer, bosses, date, notes, updated_at) 
            VALUES ('trial-1', 'acc-1', 'role-1', 15, '[]', '1704153600000', '完整测试记录', '2024-01-01T00:00:00Z');
            "#,
        ).expect("should create old tables");

        ensure_version_tables(&conn).expect("version tables should be created");
        upgrade_db(&conn, 0).expect("V0 upgrade should succeed");

        let account_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM accounts WHERE account_name = '完整测试账号'", [], |row| row.get(0))
            .expect("should count accounts");
        assert_eq!(account_count, 1, "账号数据应保留");

        let role_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM roles WHERE name = '测试角色'", [], |row| row.get(0))
            .expect("should count roles");
        assert_eq!(role_count, 1, "角色数据应保留");

        let trial_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records WHERE notes = '完整测试记录'", [], |row| row.get(0))
            .expect("should count trial records");
        assert_eq!(trial_count, 1, "试炼记录应保留");

        let trial_date: i64 = conn
            .query_row("SELECT date FROM trial_records WHERE id = 'trial-1'", [], |row| row.get(0))
            .expect("should get trial date");
        assert_eq!(trial_date, 1704153600000, "试炼记录日期应正确转换");

        let version = get_schema_version(&conn).expect("should get version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION, "最终版本应为 V{}", CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migration_logs_no_errors() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");

        let result = upgrade_db(&conn, 0);
        assert!(result.is_ok(), "升级应成功，错误: {:?}", result.err());

        let version = get_schema_version(&conn).expect("should get version");
        assert_eq!(version, CURRENT_SCHEMA_VERSION, "版本应为 V{}", CURRENT_SCHEMA_VERSION);
    }
}
