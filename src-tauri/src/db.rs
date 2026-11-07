use rusqlite::{params, Connection, OptionalExtension, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::app_config;
use crate::kungfu_data::get_sect_by_martial;
use crate::runtime_mode::{self, RuntimeMode};

mod migration;
pub mod migrations;

#[cfg(test)]
mod upgrade_tests;

const DATABASE_NAME: &str = "jx3-raid-manager.db";
const LOG_FILE_NAME: &str = "jx3-raid-manager.log";
// 旧版 bootstrap 文件名（安装版与便携版共享同一份配置，切换版本时会互相串数据）
// 升级后按 runtime mode 隔离配置，此常量仅用于一次性历史迁移
const LEGACY_DATA_DIR_BOOTSTRAP_FILE: &str = "data-dir.json";

/// 当前数据库 schema 版本
pub const CURRENT_SCHEMA_VERSION: i32 = 22;

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
    /// 用户已确认覆盖目标目录中已存在的数据库文件
    /// 设置后，下次迁移时遇到目标文件冲突会强制覆盖，迁移完成后清除
    #[serde(default)]
    force_overwrite_target: bool,
}

/// 检测是否为安装版（通过检查同目录下是否有 uninstall.exe）
fn is_install_mode() -> bool {
    matches!(
        runtime_mode::detect_current_runtime_mode(),
        Ok(RuntimeMode::Installer)
    )
}

/// 返回当前 runtime mode 的文件名后缀，用于 bootstrap 配置按 mode 隔离
/// 安装版与便携版各自维护独立的 bootstrap 文件，避免切换版本时互相串数据
fn runtime_mode_suffix() -> &'static str {
    if is_install_mode() {
        "installer"
    } else {
        "portable"
    }
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

/// 一次性从旧版共享 bootstrap 文件迁移到 mode-specific 文件
///
/// 旧版安装版与便携版共享同一份 data-dir.json，切换版本时会互相串数据。
/// 升级后按 runtime mode 隔离配置：首次启动时把旧文件复制到 mode-specific 文件。
/// 只保留 custom_data_dir（用户显式设置的自定义目录，与 mode 无关），
/// 清空 pending_migration_from（旧 mode 的迁移源不应跨 mode 触发数据迁移）。
/// 不复制 .ini 状态文件（effectiveDataDir 是 mode-specific 的，跨 mode 复制会串数据）。
fn migrate_legacy_bootstrap_if_needed(mode_specific_path: &Path) -> Result<(), String> {
    if mode_specific_path.exists() {
        return Ok(());
    }
    let legacy_path = mode_specific_path
        .parent()
        .ok_or_else(|| "无法获取 bootstrap 目录".to_string())?
        .join(LEGACY_DATA_DIR_BOOTSTRAP_FILE);
    if !legacy_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&legacy_path)
        .map_err(|e| format!("读取旧版 bootstrap 配置失败: {} ({})", legacy_path.display(), e))?;
    let mut legacy_config: DataDirBootstrapConfig = serde_json::from_str(&content)
        .map_err(|e| format!("解析旧版 bootstrap 配置失败: {} ({})", legacy_path.display(), e))?;

    // 只保留 custom_data_dir（用户显式设置，与 mode 无关）
    // 清空 pending_migration_from（旧 mode 的迁移源不应跨 mode 触发迁移）
    legacy_config.pending_migration_from = None;

    let mode_specific_content = serde_json::to_string_pretty(&legacy_config)
        .map_err(|e| format!("序列化 mode-specific 配置失败: {}", e))?;
    fs::write(mode_specific_path, mode_specific_content).map_err(|e| {
        format!(
            "写入 mode-specific 配置失败: {} ({})",
            mode_specific_path.display(),
            e
        )
    })?;

    log::info!(
        "[Bootstrap] 已从旧版共享配置迁移到 mode-specific: {:?} -> {:?}（已清空 pending_migration_from 避免跨 mode 串数据）",
        legacy_path,
        mode_specific_path
    );
    Ok(())
}

fn get_data_dir_bootstrap_path() -> Result<PathBuf, String> {
    let base_dir = dirs::data_local_dir()
        .or_else(dirs::config_local_dir)
        .ok_or_else(|| "无法获取本地配置目录".to_string())?;

    let bootstrap_dir = base_dir.join("jx3-raid-manager");
    ensure_directory_exists(&bootstrap_dir)?;

    let mode_specific_name = format!("data-dir-{}.json", runtime_mode_suffix());
    let mode_specific_path = bootstrap_dir.join(&mode_specific_name);
    migrate_legacy_bootstrap_if_needed(&mode_specific_path)?;
    Ok(mode_specific_path)
}

fn get_data_dir_installer_state_path() -> Result<PathBuf, String> {
    let bootstrap_path = get_data_dir_bootstrap_path()?;
    let bootstrap_dir = bootstrap_path
        .parent()
        .ok_or_else(|| "无法获取数据目录状态文件所在目录".to_string())?;
    let mode_specific_name = format!("data-dir-{}.ini", runtime_mode_suffix());
    Ok(bootstrap_dir.join(&mode_specific_name))
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

fn migrate_managed_app_data_files(
    source: &Path,
    target: &Path,
    force_overwrite: bool,
) -> Result<bool, String> {
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
            if force_overwrite {
                // 用户已确认覆盖：直接用源文件替换目标文件
                fs::remove_file(&target_path).map_err(|e| {
                    format!("覆盖目标文件失败 {}: {}", target_path.display(), e)
                })?;
                move_file_with_fallback(&source_path, &target_path)?;
                changed = true;
                log::warn!(
                    "已按用户确认强制覆盖目标数据文件: {:?} -> {:?}",
                    source_path,
                    target_path
                );
            } else if files_are_identical(&source_path, &target_path)? {
                // 文件完全相同：安全删除源文件（内容已存在于目标）
                fs::remove_file(&source_path)
                    .map_err(|e| format!("删除源文件失败 {}: {}", source_path.display(), e))?;
                changed = true;
                log::info!("已清理重复数据文件: {:?} -> {:?}", source_path, target_path);
            } else {
                // 文件内容不同：保留源文件，避免有数据的 db 被空 db 覆盖丢失。
                // 之前基于修改时间的 target_file_is_authoritative 判断不安全：
                // 新创建的空 db 修改时间新于有数据的旧 db，会误判并删除有数据的源文件。
                // 保留两者，让用户手动处理冲突（源文件保留在原目录作为备份）。
                log::warn!(
                    "跳过冲突数据文件迁移（源与目标内容不同，保留源文件作为备份）: {:?} -> {:?}",
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

    // 不再把 get_home_app_dir() 作为迁移源：
    // 便携版默认目录是 ~/.jx3-raid-manager，若保留此迁移源，安装版启动时会把便携版
    // 的数据迁移到安装目录，导致跨 mode 串数据。bootstrap 配置已按 mode 隔离，
    // 同一 mode 内的目录变更通过 pending_migration_from 和 mode-specific .ini
    // 的 effectiveDataDir 记录，足以覆盖自定义目录切换场景。

    if let Some(previous_dir) = read_previous_effective_data_dir()? {
        if previous_dir != *target_dir && !migration_sources.contains(&previous_dir) {
            log::info!("检测到历史数据目录 {:?}，加入迁移源", previous_dir);
            migration_sources.push(previous_dir);
        }
    }

    let mut migrated = false;
    let force_overwrite = config.force_overwrite_target;
    for source_dir in migration_sources {
        if !source_dir.exists() {
            continue;
        }

        if directories_match(&source_dir, target_dir)? || !has_persisted_app_data(&source_dir) {
            continue;
        }

        if migrate_managed_app_data_files(&source_dir, target_dir, force_overwrite)? {
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
    } else if migrated && config.pending_migration_from.is_some() {
        log::warn!("数据迁移后源目录仍保留文件，保留待迁移状态以便后续继续处理");
    }

    // 强制覆盖是一次性标记，迁移完成后立即清除，避免后续误覆盖
    if migrated && config.force_overwrite_target {
        config.force_overwrite_target = false;
        log::info!("[Bootstrap] 已清除 force_overwrite_target 标记");
    }
    Ok(())
}

pub fn get_app_dir() -> Result<PathBuf, String> {
    let config = read_data_dir_bootstrap_config()?;
    let (target_dir, location, _) = resolve_target_app_dir(&config)?;
    ensure_directory_exists(&target_dir)?;
    // 注意：此处不再调用 maybe_migrate_app_data。
    // 迁移逻辑已移至 init_db 启动阶段显式执行，避免应用运行期间任何 get_app_dir/get_db_path
    // 调用意外触发迁移，导致用户通过 db_set_custom_data_dir 设置的 force_overwrite 标记
    // 和 pending_migration_from 在写入后被立即消费清除。
    write_data_dir_installer_state(&config, &target_dir, &location)?;
    Ok(target_dir)
}

/// 在应用启动阶段显式执行数据目录迁移。
///
/// 仅在 init_db 中调用一次，确保迁移不会在应用运行期间被其他 get_app_dir 调用触发。
fn ensure_app_data_migrated() -> Result<(), String> {
    let mut config = read_data_dir_bootstrap_config()?;
    let (target_dir, location, _) = resolve_target_app_dir(&config)?;
    ensure_directory_exists(&target_dir)?;
    maybe_migrate_app_data(&target_dir, &mut config)?;
    write_data_dir_bootstrap_config(&config)?;
    write_data_dir_installer_state(&config, &target_dir, &location)?;
    Ok(())
}

/// 获取当前正在使用的应用数据目录，但不触发迁移。
///
/// 与 `get_app_dir` 的区别：仅读取配置确定当前有效目录，不会执行迁移逻辑。
/// 用于 `db_set_custom_data_dir` / `db_reset_custom_data_dir` 等场景，
/// 避免在写入新配置前意外触发旧配置的迁移，导致 `force_overwrite` 标记被忽略。
///
/// 逻辑：
/// - 若 `pending_migration_from` 不为空，说明上一次切换目录的迁移尚未完成，
///   当前数据仍在迁移源目录，返回该源目录。
/// - 否则返回 `resolve_target_app_dir` 的目标目录。
fn get_current_app_dir_no_migrate() -> Result<PathBuf, String> {
    let config = read_data_dir_bootstrap_config()?;
    if let Some(source) = config.pending_migration_from.as_ref() {
        let source_path = PathBuf::from(source.trim());
        if !source_path.as_os_str().is_empty() && source_path.exists() {
            return Ok(source_path);
        }
    }
    let (target_dir, _, _) = resolve_target_app_dir(&config)?;
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
    #[allow(dead_code)]
    source: String,
    #[allow(dead_code)]
    status: String,
    #[allow(dead_code)]
    drops: Option<String>,
    #[allow(dead_code)]
    jcl_files: Option<String>,
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
        source: record["source"]
            .as_str()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "manual".to_string()),
        status: record["status"]
            .as_str()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "confirmed".to_string()),
        drops: record.get("drops")
            .filter(|v| !v.is_null())
            .map(|v| v.to_string()),
        jcl_files: record.get("jclFiles")
            .filter(|v| !v.is_null())
            .map(|v| v.to_string()),
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

    // 安全校验：只允许删除 <游戏目录>/userdata 下的账号目录或角色目录。
    // 之前这里直接对任意传入路径 remove_dir_all，前端一旦拼错路径（或参数被构造）
    // 就会递归删除用户机器上的任意目录。校验范式与 char_sync.rs 的 starts_with 一致。
    ensure_deletable_userdata_path(&target_path, &display_path)?;

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

/// 校验待删除目录位于 `<游戏目录>/userdata/` 之下，且不是 userdata 本身。
///
/// 规则：
/// - 必须已配置游戏目录，否则拒绝（宁可不删，也不猜路径）
/// - 目标目录 canonicalize 后必须 starts_with `<游戏目录>/userdata` 的 canonicalize 结果
/// - 目标不得等于 userdata 目录本身（否则会一次性删掉所有账号）
fn ensure_deletable_userdata_path(target_path: &Path, display_path: &str) -> Result<(), String> {
    let config = app_config::get_app_config_internal()
        .map_err(|e| format!("读取游戏目录配置失败，拒绝删除 {display_path}: {e}"))?;
    let raw_game_dir = config
        .game_directory
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("未配置游戏目录，拒绝删除目录: {display_path}"))?;

    let runtime_game_dir =
        PathBuf::from(crate::game_directory::resolve_game_runtime_directory(&raw_game_dir));
    let userdata_dir = runtime_game_dir.join("userdata");

    let canonical_userdata = userdata_dir.canonicalize().map_err(|e| {
        format!("无法解析 userdata 目录 {userdata_dir:?}，拒绝删除 {display_path}: {e}")
    })?;
    let canonical_target = target_path.canonicalize().map_err(|e| {
        format!("无法解析目标目录 {display_path}，拒绝删除: {e}")
    })?;

    if canonical_target == canonical_userdata {
        let message = format!("拒绝删除 userdata 根目录: {display_path}");
        log::error!("{}", message);
        return Err(message);
    }

    if !canonical_target.starts_with(&canonical_userdata) {
        let message = format!(
            "目标目录不在游戏 userdata 目录下，拒绝删除: {display_path}（userdata: {}）",
            canonical_userdata.display()
        );
        log::error!("{}", message);
        return Err(message);
    }

    Ok(())
}

/// 应用持久化 PRAGMA（写入数据库文件头，只需在首次初始化时设置一次）
///
/// - `journal_mode=WAL`：WAL 模式支持多读 + 单写并发，避免 DELETE 模式的写独占锁
/// - `wal_autocheckpoint=1000`：WAL 文件达到 1000 页时自动 checkpoint
fn apply_persistent_pragmas(conn: &Connection) {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=1000;"
    ).ok();
}

/// 应用连接级 PRAGMA（每次新建连接都需要设置）
///
/// - `synchronous=NORMAL`：WAL 模式下安全性等同 FULL+DELETE，但避免每次 commit fsync
/// - `foreign_keys=ON`：开启外键约束
/// - `busy_timeout=5000`：5 秒忙等待，避免 SQLITE_BUSY
/// - `cache_size=-65536`：64MB 页缓存（负数表示 KB）
/// - `mmap_size=268435456`：256MB 内存映射 IO
/// - `temp_store=MEMORY`：临时表和排序走内存
fn apply_connection_pragmas(conn: &Connection) {
    conn.execute_batch(
        "PRAGMA synchronous=NORMAL; \
         PRAGMA foreign_keys=ON; \
         PRAGMA busy_timeout=5000; \
         PRAGMA cache_size=-65536; \
         PRAGMA mmap_size=268435456; \
         PRAGMA temp_store=MEMORY;"
    ).ok();
}

/// 初始化数据库（单例模式，只初始化一次）
///
/// 安装与升级路径完全分离：
/// - 安装路径：数据库不存在时，直接创建最新版本的完整结构
/// - 升级路径：数据库存在但版本较低时，自动识别版本并按序执行增量迁移
pub fn init_db() -> Result<Connection, String> {
    let mut initialized = DB_INITIALIZED.lock().map_err(|e| e.to_string())?;

    if *initialized {
        let path = get_db_path()?;
        // 快速路径校验：文件存在且大小 > 0 才跳过初始化
        // 防止数据库文件在运行期间被删除/替换为空文件后，
        // DB_INITIALIZED 缓存导致返回空数据库连接
        if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.len() > 0 {
                let conn = Connection::open(path).map_err(|e| e.to_string())?;
                // 快速路径只需应用连接级 PRAGMA
                // 持久化 PRAGMA（journal_mode=WAL、wal_autocheckpoint）已写入数据库文件头，无需重复设置
                apply_connection_pragmas(&conn);
                return Ok(conn);
            }
        }
        // 文件不存在或大小为 0，重置标志重新初始化
        log::warn!("[INIT] 数据库文件缺失或为空，重新初始化");
        *initialized = false;
    }

    // 完整初始化路径：先执行数据目录迁移，再获取 db_path。
    // 迁移只在启动阶段执行一次，避免运行期间 get_app_dir 调用意外触发迁移
    // 导致 db_set_custom_data_dir 写入的 force_overwrite 标记被提前消费清除。
    ensure_app_data_migrated()?;

    let path = get_db_path()?;

    let db_exists = path.exists();

    log::info!(
        "[INIT] 数据库初始化开始，DB路径: {:?}, db_exists: {}",
        path, db_exists
    );

    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    // 完整初始化路径：先设置持久化 PRAGMA（journal_mode=WAL），再设置连接级 PRAGMA
    apply_persistent_pragmas(&conn);
    apply_connection_pragmas(&conn);

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

    // 补救：确保 V14 的表结构变更已应用（开发期可能 schema_versions 标记为 V14 但表结构未更新）
    run_post_init_repairs(&conn)?;

    *initialized = true;
    log::info!("[INIT] 数据库初始化完成，当前版本 V{}", CURRENT_SCHEMA_VERSION);

    Ok(conn)
}

/// 升级 / 安装之后的统一收尾序列（生产与测试共用）。
///
/// 这些 `ensure_*` 是「schema_versions 已标记但物理结构未更新」的历史补救，
/// 生产启动时必须执行。测试路径此前漏掉了它们，导致测试通过而真实启动仍
/// 可能缺列/缺表。抽成公共函数后两条路径不会再分叉。
fn run_post_init_repairs(conn: &Connection) -> Result<(), String> {
    ensure_raid_bosses_table(conn)?;
    ensure_records_columns(conn)?;
    ensure_jcl_cache_table(conn)?;
    ensure_drop_items_table(conn)?;
    ensure_critical_columns(conn)?;
    migration::init_static_raids(conn)?;
    ensure_equipment_columns(conn)?;
    ensure_app_config_table(conn)?;
    // V18 兜底：全新安装与「版本过高修复」路径都不走增量迁移，必须显式建表
    ensure_char_sync_rollback_marks_table(conn)?;
    // V14 新增引导功能：历史用户已配置游戏目录且有账号数据时，自动标记引导完成
    auto_complete_setup_for_legacy_users(conn)?;
    Ok(())
}

/// 安装阶段预创建数据库（不触发数据目录迁移）
///
/// 供 `--prepare-install-data` 使用。与 `init_db` 的区别：
/// - **跳过 `ensure_app_data_migrated`**，避免安装时迁移副作用导致数据丢失
/// - 迁移逻辑留给应用正常启动时的 `init_db` 处理
///
/// 背景：安装时 `--prepare-install-data` 调用 `init_db` 会触发迁移，
/// 如果目标目录已存在空 db（修改时间新于有数据的源 db），
/// `target_file_is_authoritative` 会误判并删除有数据的源 db，导致数据丢失。
pub fn init_db_skip_migration() -> Result<Connection, String> {
    let mut initialized = DB_INITIALIZED.lock().map_err(|e| e.to_string())?;

    if *initialized {
        let path = get_db_path()?;
        if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.len() > 0 {
                let conn = Connection::open(path).map_err(|e| e.to_string())?;
                apply_connection_pragmas(&conn);
                return Ok(conn);
            }
        }
        log::warn!("[INIT] 数据库文件缺失或为空，重新初始化");
        *initialized = false;
    }

    // 注意：不调用 ensure_app_data_migrated，避免安装时迁移副作用
    let path = get_db_path()?;
    let db_exists = path.exists();

    log::info!(
        "[INIT] 安装阶段预创建数据库，DB路径: {:?}, db_exists: {}",
        path, db_exists
    );

    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    apply_persistent_pragmas(&conn);
    apply_connection_pragmas(&conn);

    ensure_version_tables(&conn)?;

    let current_version = get_schema_version(&conn)?;

    if !db_exists {
        install_fresh_db(&conn)?;
    } else if current_version > CURRENT_SCHEMA_VERSION {
        log::warn!(
            "[INIT] 数据库版本 V{} 高于应用版本 V{}，将执行表结构修复",
            current_version, CURRENT_SCHEMA_VERSION
        );
        ensure_baseline_tables(&conn)?;
        ensure_equipment_columns(&conn)?;
        ensure_critical_columns(&conn)?;
    } else if current_version < CURRENT_SCHEMA_VERSION {
        upgrade_db(&conn, current_version)?;
    } else {
        log::info!("[INIT] 已是最新版本 V{}，无需迁移", current_version);
    }

    run_post_init_repairs(&conn)?;

    *initialized = true;
    log::info!("[INIT] 安装阶段数据库预创建完成，当前版本 V{}", CURRENT_SCHEMA_VERSION);

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

    // 测试路径同样使用 WAL + NORMAL 配置，保持与生产环境一致
    apply_persistent_pragmas(&conn);
    apply_connection_pragmas(&conn);

    ensure_version_tables(&conn)?;

    let current_version = get_schema_version(&conn)?;
    log::debug!(
        "[INIT_TEST] 当前数据库版本: {}, 目标版本: {}",
        current_version, CURRENT_SCHEMA_VERSION
    );

    if !db_exists {
        install_fresh_db(&conn)?;
    } else if current_version > CURRENT_SCHEMA_VERSION {
        // 与生产 `init_db` 保持一致：应用降级后数据库版本过高时不能只告警，
        // 必须执行表结构修复，否则后续命令会因缺列/缺表而失败。
        log::warn!(
            "[INIT_TEST] 数据库版本 V{} 高于应用版本 V{}，将执行表结构修复",
            current_version, CURRENT_SCHEMA_VERSION
        );
        ensure_baseline_tables(&conn)?;
        ensure_equipment_columns(&conn)?;
        ensure_critical_columns(&conn)?;
    } else if current_version < CURRENT_SCHEMA_VERSION {
        upgrade_db(&conn, current_version)?;
    } else {
        log::info!("[INIT_TEST] 已是最新版本 V{}，无需迁移", current_version);
    }

    run_post_init_repairs(&conn)?;

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

#[cfg(test)]
pub(crate) fn install_fresh_db_for_test(conn: &Connection) -> Result<(), String> {
    ensure_version_tables(conn)?;
    install_fresh_db(conn)
}

/// 测试用入口：直接驱动真实的 `upgrade_db`。
///
/// 存在的意义是让测试与生产走**同一份**升级逻辑，而不是在测试里重写一遍
/// 迁移循环。任何对 `upgrade_db` 的修改都会立刻被端到端测试覆盖。
#[cfg(test)]
pub(crate) fn upgrade_db_for_test(conn: &Connection, from_version: i32) -> Result<(), String> {
    upgrade_db(conn, from_version)
}

/// 升级前自动快照：把当前数据库完整备份为 `<db>.pre-v{n}.bak`。
///
/// 设计取舍：
/// - 使用 SQLite 在线备份 API 而不是文件复制：数据库运行在 WAL 模式下，
///   直接复制 `.db` 会漏掉 `-wal` 中尚未落盘的事务，得到不一致的快照。
/// - 备份失败**不阻断升级**：磁盘满或权限异常时仍然优先让用户能打开应用，
///   失败仅记录告警（升级后仍可手动用 `db_backup` 命令补做）。
/// - 只保留本次升级对应的那一份快照，不做多份轮转，避免用户目录无限增长。
fn backup_before_upgrade(conn: &Connection, from_version: i32) {
    let Some(db_path) = conn.path() else {
        log::debug!("[BACKUP] 内存数据库，跳过升级前备份");
        return;
    };
    let db_path = std::path::PathBuf::from(db_path);
    if !db_path.exists() {
        log::debug!("[BACKUP] 数据库文件不存在，跳过升级前备份");
        return;
    }

    let backup_path = db_path.with_extension(format!("db.pre-v{}.bak", from_version));

    let result = (|| -> Result<(), String> {
        // 已存在的旧快照先移除，避免备份到已过期的副本
        if backup_path.exists() {
            std::fs::remove_file(&backup_path)
                .map_err(|e| format!("清理旧快照失败: {}", e))?;
        }
        let mut dst =
            Connection::open(&backup_path).map_err(|e| format!("创建快照文件失败: {}", e))?;
        rusqlite::backup::Backup::new(conn, &mut dst)
            .map_err(|e| format!("初始化快照失败: {}", e))?
            .run_to_completion(100, std::time::Duration::from_millis(50), None)
            .map_err(|e| format!("写入快照失败: {}", e))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            let size = std::fs::metadata(&backup_path).map(|m| m.len()).unwrap_or(0);
            log::info!(
                "[BACKUP] 升级前快照已生成: {:?} ({} 字节)",
                backup_path, size
            );
        }
        Err(e) => {
            log::warn!("[BACKUP] 升级前快照失败（升级继续）: {}", e);
            let _ = std::fs::remove_file(&backup_path);
        }
    }
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

    // 迁移前快照：V19 起迁移会对 trial_records 做 ALTER + 全表 UPDATE，
    // 一旦迁移中途失败或结果不符合预期，用户需要一份可回退的副本。
    // 内存库（测试）没有可备份的物理文件，直接跳过。
    backup_before_upgrade(conn, current_version);

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

/// 升级后必须存在的唯一索引：缺失即判定升级失败。
///
/// 唯一索引承担数据去重语义，缺失不会让升级「看起来失败」，而是让数据
/// 在后续写入中静默出现重复，因此不能与普通索引一样只告警。
const UNIQUE_INDEXES: [&str; 1] = ["idx_trial_records_source_key"];

/// 升级后验证：检查数据库结构和数据完整性
fn validate_post_upgrade(conn: &Connection) -> Result<(), String> {
    log::info!("[VALIDATE] 升级后验证开始");

    let expected_tables = [
        "schema_versions",
        "records",
        "app_config",
        "cache",
        "equipments",
        "accounts",
        "roles",
        "raids",
        "raid_bosses",
        "favorite_raids",
        "instance_types",
        "role_instance_visibility",
        "raid_role_visibility",
        "trial_records",
        "baizhan_records",
        "trial_draws",
        "trial_draw_equipment_manual_overrides",
        "game_versions",
        "seasons",
        "char_sync_rollback_marks",
        "trial_auto_scan_tombstones",
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
        "idx_records_data_raid_name",
        "idx_records_cd_lookup",
        "idx_records_pending",
        "idx_records_manual_cd",
        "idx_seasons_version_id",
        "idx_raids_season_id",
        "idx_trial_records_source_key",
        "idx_trial_draws_record_sequence",
        "idx_trial_draw_equipment_manual_overrides_draw_card",
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

    // 唯一索引缺失必须阻断：`idx_trial_records_source_key` 是「同一 source_key
    // 不重复」的唯一保障，缺它时升级若仍报通过，去重语义会静默失效并伴随性能
    // 退化。普通索引缺失只告警，唯一索引缺失直接失败。
    if !missing_indexes.is_empty() {
        let missing_unique: Vec<&str> = missing_indexes
            .iter()
            .copied()
            .filter(|name| UNIQUE_INDEXES.contains(name))
            .collect();
        if !missing_unique.is_empty() {
            log::error!("[VALIDATE] 缺少唯一索引: {:?}", missing_unique);
            return Err(format!(
                "升级后验证失败：缺少唯一索引 {:?}",
                missing_unique
            ));
        }
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
            record_type TEXT,
            source TEXT DEFAULT 'manual',
            status TEXT DEFAULT 'confirmed',
            drops TEXT,
            jcl_files TEXT,
            created_at TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );

        -- JCL 解析缓存表（跨会话复用，避免重复解析 GBK 编码的 JCL 文件）
        CREATE TABLE IF NOT EXISTS jcl_cache (
            file_path TEXT PRIMARY KEY,
            file_mtime INTEGER NOT NULL,
            boss_name TEXT,
            fight_start_ms INTEGER NOT NULL,
            fight_end_ms INTEGER NOT NULL,
            is_kill INTEGER NOT NULL,
            cached_at TEXT NOT NULL
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

/// 确保 raid_bosses 表为 V14 新结构（联合主键 raid_name + boss_id）
///
/// 开发期间 schema_versions 可能已标记 V14，但 V14 迁移代码后续修改增加了
/// raid_bosses 重构逻辑，导致旧数据库标记已迁移但表结构未更新。
/// 此函数检测旧结构（有 id 列无 boss_id 列）并执行重构。
fn ensure_raid_bosses_table(conn: &Connection) -> Result<(), String> {
    let has_id_column: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('raid_bosses') WHERE name='id'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !has_id_column {
        return Ok(());
    }

    log::warn!("[ensure_raid_bosses_table] 检测到旧结构 raid_bosses 表（id 列），执行重构");

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS raid_bosses_new (
            raid_name TEXT NOT NULL,
            boss_id TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL,
            PRIMARY KEY (raid_name, boss_id)
        );
        "#,
    )
    .map_err(|e| format!("创建 raid_bosses_new 失败: {}", e))?;

    let migrated = conn
        .execute(
            r#"
            INSERT OR IGNORE INTO raid_bosses_new (raid_name, boss_id, name, boss_order)
            SELECT
                COALESCE(r.name, old.raid_name) AS raid_name,
                old.id AS boss_id,
                old.name,
                old.boss_order
            FROM raid_bosses old
            LEFT JOIN raids r ON r.id = old.raid_name
            "#,
            [],
        )
        .map_err(|e| format!("迁移 raid_bosses 数据失败: {}", e))?;

    conn.execute(
        "UPDATE raid_bosses_new SET name = '阿史那承庆' WHERE name = '阿史'",
        [],
    )
    .ok();

    conn.execute_batch(
        r#"
        DROP TABLE raid_bosses;
        ALTER TABLE raid_bosses_new RENAME TO raid_bosses;
        "#,
    )
    .map_err(|e| format!("重命名 raid_bosses_new 失败: {}", e))?;

    log::info!("[ensure_raid_bosses_table] raid_bosses 表重构完成，迁移 {} 条记录", migrated);
    Ok(())
}

/// 确保 records 表有 source / status / drops / jcl_files / created_at / updated_at 列和索引（V14 新增）
fn ensure_records_columns(conn: &Connection) -> Result<(), String> {
    let columns_to_add = [
        ("source", "TEXT DEFAULT 'manual'"),
        ("status", "TEXT DEFAULT 'confirmed'"),
        ("drops", "TEXT"),
        ("jcl_files", "TEXT"),
        ("created_at", "TEXT"),
        ("updated_at", "TEXT"),
    ];

    for (col_name, col_type) in &columns_to_add {
        let has_col: bool = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('records') WHERE name='{}'",
                    col_name
                ),
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if !has_col {
            conn.execute(
                &format!("ALTER TABLE records ADD COLUMN {} {}", col_name, col_type),
                [],
            )
            .map_err(|e| e.to_string())?;
            log::info!("[ensure_records_columns] 已添加 records.{} 列", col_name);
        }
    }

    // 创建索引
    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_records_status ON records(status);
        CREATE INDEX IF NOT EXISTS idx_records_source ON records(source);
        "#,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 确保 jcl_cache 表存在（V14 新增，JCL 解析缓存）
fn ensure_jcl_cache_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS jcl_cache (
            file_path TEXT PRIMARY KEY,
            file_mtime INTEGER NOT NULL,
            boss_name TEXT,
            fight_start_ms INTEGER NOT NULL,
            fight_end_ms INTEGER NOT NULL,
            is_kill INTEGER NOT NULL,
            cached_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| format!("创建 jcl_cache 表失败: {}", e))?;
    Ok(())
}

/// 确保 drop_items 表存在（全新安装时 create_latest_schema 不包含此表，需兜底创建）
fn ensure_drop_items_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS drop_items (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            map_id            INTEGER,
            item_type         INTEGER NOT NULL,
            item_id           INTEGER NOT NULL,
            item_ext_id       INTEGER,
            boss_name         TEXT,
            applicable_school_ids TEXT,
            ui_id             INTEGER,
            source            TEXT,
            source_id         INTEGER,
            item_name         TEXT NOT NULL UNIQUE,
            description       TEXT,
            genre             INTEGER,
            sub_type          INTEGER,
            detail_type       INTEGER,
            price             INTEGER,
            level             INTEGER,
            bind_type         INTEGER,
            max_durability    INTEGER,
            abrade_rate       INTEGER,
            max_exist_time    INTEGER,
            max_exist_amount  INTEGER,
            can_trade         INTEGER,
            can_destroy       INTEGER,
            quality           INTEGER,
            skill_id          INTEGER,
            skill_level       INTEGER,
            belong_school     TEXT,
            magic_kind        TEXT,
            magic_type        TEXT,
            get_type          TEXT,
            icon_id           INTEGER,
            can_set_color     INTEGER,
            auc_genre         INTEGER,
            auc_sub_type      INTEGER,
            require_camp      INTEGER,
            max_strength_level INTEGER,
            can_apart         INTEGER,
            can_exterior      INTEGER,
            can_change_magic  INTEGER,
            can_shared        INTEGER,
            repair_price_rebate INTEGER,
            can_stack         INTEGER,
            can_consume       INTEGER,
            require_level     INTEGER,
            require_gender    INTEGER,
            require_homeland_level INTEGER,
            can_use_on_horse  INTEGER,
            can_use_in_fight  INTEGER,
            can_good_camp_use INTEGER,
            can_evil_camp_use INTEGER,
            can_neutral_camp_use INTEGER,
            type_label        TEXT,
            appearance        TEXT,
            cool_down         TEXT,
            is_quest          INTEGER,
            wu_cai_html       TEXT,
            is_equip          INTEGER,
            equip_usage       INTEGER,
            image_url         TEXT,
            id_key            INTEGER,
            diamonds          TEXT,
            requires          TEXT,
            recommend         TEXT,
            recommend_xfs     TEXT,
            attribute_types   TEXT,
            set_info          TEXT,
            get_source        TEXT,
            attributes        TEXT,
            furniture_attributes TEXT,
            category          TEXT NOT NULL DEFAULT 'unknown',
            class_source      TEXT NOT NULL DEFAULT 'api',
            season_id         INTEGER,
            created_at        TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_drop_items_category ON drop_items(category);
        CREATE INDEX IF NOT EXISTS idx_drop_items_item_type_id ON drop_items(item_type, item_id);
        "#,
    )
    .map_err(|e| format!("创建 drop_items 表失败: {}", e))?;
    Ok(())
}

/// 确保 app_config 表存在（历史数据库兼容处理）
///
/// 部分历史数据库版本号已升级到 V14 但 app_config 表缺失，
/// 兜底创建 V18 的「跨角色设置同步已回滚标记」表。
///
/// 背景：该表最初只由 `db/migrations/v18.rs` 创建，而全新安装走的是
/// `install_fresh_db` → `create_latest_schema`，完全不执行增量迁移；
/// 「数据库版本高于应用版本」的表结构修复路径同样不会执行 V18 迁移。
/// 结果：从 v2.2.2 起全新安装的用户，跨角色同步的回滚标记读写在 SQLite 层直接报错，
/// 而前端只 `console.error`，表现为标记静默丢失。
///
/// 这里在 init_db 的补救区无条件兜底建表（幂等），覆盖三条路径：
/// 全新安装、增量升级、版本过高修复。
fn ensure_char_sync_rollback_marks_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS char_sync_rollback_marks (
            backup_dir TEXT PRIMARY KEY,
            rolled_back_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_char_sync_rollback_marks_rolled_back_at
        ON char_sync_rollback_marks (rolled_back_at DESC);
        "#,
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 此函数在 init_db 末尾兜底创建表并插入默认值，避免 set_game_directory 等命令失败。
fn ensure_app_config_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        "#,
    )
    .map_err(|e| e.to_string())?;

    let now = chrono::Local::now().to_rfc3339();
    for (key, default_value) in [
        ("game_directory", ""),
        ("setup_completed", "false"),
        ("last_scan_mingyi_at", ""),
        ("config_json", ""),
        ("local_storage_migrated", "false"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, default_value, &now],
        )
        .map_err(|e| e.to_string())?;
    }

    // 补救：如果旧 config 表仍存在（开发期 V14 标记已写入但合并未执行），
    // 执行与 V14 迁移相同的合并逻辑
    let config_table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='config'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if config_table_exists {
        if let Some(json_str) = conn
            .query_row::<String, _, _>("SELECT value FROM config WHERE id = 1", [], |row| {
                row.get(0)
            })
            .ok()
        {
            // 迁移完整 JSON blob 到 config_json
            conn.execute(
                "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'config_json' AND value = ''",
                params![json_str, &now],
            )
            .map_err(|e| e.to_string())?;

            // 提取 game_directory
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let game_dir = parsed
                    .get("game")
                    .and_then(|g| g.get("gameDirectory"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if !game_dir.is_empty() {
                    conn.execute(
                        "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'game_directory' AND value = ''",
                        params![game_dir, &now],
                    )
                    .map_err(|e| e.to_string())?;
                    log::info!("[ensure_app_config_table] 从旧 config 表迁移 game_directory={}", game_dir);
                }
            }
        }
        // 删除旧 config 表
        conn.execute("DROP TABLE IF EXISTS config", [])
            .map_err(|e| e.to_string())?;
        log::info!("[ensure_app_config_table] 已删除旧 config 表，配置统一到 app_config");
    }

    // 补救：如果旧 migration_flags 表仍存在，迁移 local_storage_migrated
    let flags_table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='migration_flags'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    if flags_table_exists {
        if let Some(val) = conn
            .query_row::<String, _, _>(
                "SELECT value FROM migration_flags WHERE key = 'local_storage_migrated'",
                [],
                |row| row.get(0),
            )
            .ok()
        {
            if val == "true" {
                conn.execute(
                    "UPDATE app_config SET value = 'true', updated_at = ?1 WHERE key = 'local_storage_migrated'",
                    params![&now],
                )
                .map_err(|e| e.to_string())?;
                log::info!("[ensure_app_config_table] 从旧 migration_flags 表迁移 local_storage_migrated=true");
            }
        }
        conn.execute("DROP TABLE IF EXISTS migration_flags", [])
            .map_err(|e| e.to_string())?;
        log::info!("[ensure_app_config_table] 已删除旧 migration_flags 表");
    }

    Ok(())
}

/// V14 引导功能历史用户兼容处理
///
/// V14 新增了 SetupGuide 引导流程，要求用户配置游戏目录并导入角色。
/// 但历史用户已经在旧版本中配置过游戏目录（存在 config 表或 app_config 表），
/// 并且已经在账号管理中导入过角色（accounts 表有数据）。
/// 这类用户升级到 V14 后不应该再次走引导流程。
///
/// 判定条件（同时满足才自动标记完成）：
/// 1. setup_completed 当前不为 'true'（避免重复处理）
/// 2. app_config.game_directory 非空
/// 3. accounts 表存在且至少有 1 条数据
///
/// 满足条件时，将 setup_completed 设为 'true'，使前端 App.tsx 跳过引导界面。
fn auto_complete_setup_for_legacy_users(conn: &Connection) -> Result<(), String> {
    // 1. 检查 setup_completed 当前值，已为 true 则跳过
    let setup_completed: String = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'setup_completed'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "false".to_string());

    if setup_completed.eq_ignore_ascii_case("true") {
        return Ok(());
    }

    // 2. 检查是否有使用痕迹：game_directory 非空 或 accounts 表有数据（满足任一即可）
    let game_directory: String = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'game_directory'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_default();

    let has_game_dir = !game_directory.trim().is_empty();

    // 检查 accounts 表是否存在且有数据
    let accounts_table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='accounts'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let account_count: i64 = if accounts_table_exists {
        conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))
            .unwrap_or(0)
    } else {
        0
    };

    // 只要配置过游戏目录或导入过账号，就视为历史用户，自动标记引导完成
    if !has_game_dir && account_count == 0 {
        return Ok(());
    }

    // 4. 满足全部条件，自动标记引导完成
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "UPDATE app_config SET value = 'true', updated_at = ?1 WHERE key = 'setup_completed'",
        params![&now],
    )
    .map_err(|e| e.to_string())?;

    log::info!(
        "[auto_complete_setup_for_legacy_users] 检测到历史用户已配置游戏目录且存在 {} 个账号，自动标记引导完成",
        account_count
    );

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
        ("trial_records", "source", "TEXT NOT NULL DEFAULT 'manual'"),
        ("trial_records", "status", "TEXT NOT NULL DEFAULT 'confirmed'"),
        ("trial_records", "source_key", "TEXT"),
        ("trial_records", "equipment_unreviewed", "INTEGER NOT NULL DEFAULT 0"),
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
        ("drop_items", "season_id", "INTEGER"),
        ("records", "raid_name", "TEXT"),
        ("records", "account_id", "TEXT"),
        ("records", "role_id", "TEXT"),
        ("records", "record_date", "INTEGER"),
        ("records", "record_type", "TEXT"),
        ("records", "source", "TEXT DEFAULT 'manual'"),
        ("records", "status", "TEXT DEFAULT 'confirmed'"),
        ("records", "drops", "TEXT"),
        ("records", "jcl_files", "TEXT"),
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

    if table_exists(conn, "trial_records")? {
        conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_trial_records_source_key
             ON trial_records(source_key) WHERE source_key IS NOT NULL",
            [],
        )
        .map_err(|e| format!("创建试炼自动扫描去重索引失败: {}", e))?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS trial_auto_scan_tombstones (
                source_key TEXT PRIMARY KEY,
                deleted_at TEXT NOT NULL
            )",
            [],
        )
        .map_err(|e| format!("创建试炼自动扫描删除标记表失败: {}", e))?;

        ensure_trial_draws_table(conn)?;
    }

    Ok(())
}

/// V21 compatibility repair for fresh installs and database-version rollback.
/// The incremental migration owns normal upgrades; this helper keeps the
/// existing defensive initialization paths structurally complete as well.
pub(crate) fn ensure_trial_draws_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trial_draws (
            id TEXT PRIMARY KEY,
            trial_record_id TEXT NOT NULL,
            source_key TEXT NOT NULL UNIQUE,
            sequence INTEGER NOT NULL DEFAULT 0,
            receipt_time INTEGER NOT NULL,
            receipt_hash TEXT,
            reward_items TEXT NOT NULL DEFAULT '[]',
            reward_sources TEXT NOT NULL DEFAULT '[]',
            stamina_cost INTEGER,
            stamina_costs TEXT NOT NULL DEFAULT '[]',
            stamina_sources TEXT NOT NULL DEFAULT '[]',
            flipped_index INTEGER NOT NULL DEFAULT 0,
            position_manual INTEGER NOT NULL DEFAULT 0,
            auto_observed_start_ms INTEGER,
            auto_observed_end_ms INTEGER,
            auto_equipment_observed_start_ms INTEGER,
            auto_equipment_observed_end_ms INTEGER,
            card_1 TEXT NOT NULL DEFAULT '',
            card_2 TEXT NOT NULL DEFAULT '',
            card_3 TEXT NOT NULL DEFAULT '',
            card_4 TEXT NOT NULL DEFAULT '',
            card_5 TEXT NOT NULL DEFAULT '',
            equipment_unreviewed INTEGER NOT NULL DEFAULT 1,
            source TEXT NOT NULL DEFAULT 'auto',
            updated_at TEXT,
            FOREIGN KEY (trial_record_id) REFERENCES trial_records(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_trial_draws_record_sequence
            ON trial_draws(trial_record_id, receipt_time, source_key);",
    )
    .map_err(|e| format!("创建试炼多翻牌明细表失败: {}", e))?;
    // Development builds may already have created a V21 table before a later
    // V21-compatible field was added. Keep startup repair idempotent.
    for (column, definition) in [
        ("reward_sources", "TEXT NOT NULL DEFAULT '[]'"),
        ("stamina_sources", "TEXT NOT NULL DEFAULT '[]'"),
        ("position_manual", "INTEGER NOT NULL DEFAULT 0"),
        ("auto_observed_start_ms", "INTEGER"),
        ("auto_observed_end_ms", "INTEGER"),
        ("auto_equipment_observed_start_ms", "INTEGER"),
        ("auto_equipment_observed_end_ms", "INTEGER"),
    ] {
        if !column_exists(conn, "trial_draws", column)? {
            conn.execute(
                &format!("ALTER TABLE trial_draws ADD COLUMN {column} {definition}"),
                [],
            )
            .map_err(|e| format!("补齐 trial_draws.{column} 失败: {e}"))?;
        }
    }
    ensure_trial_draw_equipment_manual_overrides_table(conn)?;
    Ok(())
}

/// V22 compatibility repair for per-card user evidence. A row in this table
/// means the user intentionally set that card, including intentionally
/// clearing it (`equipment_id IS NULL`). It must therefore remain separate
/// from the automatic observation fields on `trial_draws`.
pub(crate) fn ensure_trial_draw_equipment_manual_overrides_table(
    conn: &Connection,
) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS trial_draw_equipment_manual_overrides (
            draw_id TEXT NOT NULL,
            card_index INTEGER NOT NULL CHECK (card_index BETWEEN 1 AND 5),
            equipment_id TEXT,
            source TEXT NOT NULL DEFAULT 'manual' CHECK (source = 'manual'),
            updated_at TEXT NOT NULL,
            PRIMARY KEY (draw_id, card_index),
            FOREIGN KEY (draw_id) REFERENCES trial_draws(id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_trial_draw_equipment_manual_overrides_draw_card
            ON trial_draw_equipment_manual_overrides(draw_id, card_index);",
    )
    .map_err(|e| format!("创建试炼翻牌手工装备覆盖表失败: {e}"))
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
            record_type TEXT,
            source TEXT DEFAULT 'manual',
            status TEXT DEFAULT 'confirmed',
            drops TEXT,
            jcl_files TEXT,
            created_at TEXT,
            updated_at TEXT
        );

        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );

        -- JCL 解析缓存表（跨会话复用，避免重复解析 GBK 编码的 JCL 文件）
        CREATE TABLE IF NOT EXISTS jcl_cache (
            file_path TEXT PRIMARY KEY,
            file_mtime INTEGER NOT NULL,
            boss_name TEXT,
            fight_start_ms INTEGER NOT NULL,
            fight_end_ms INTEGER NOT NULL,
            is_kill INTEGER NOT NULL,
            cached_at TEXT NOT NULL
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
            raid_name TEXT NOT NULL,
            boss_id TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL,
            PRIMARY KEY (raid_name, boss_id)
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
            source TEXT NOT NULL DEFAULT 'manual',
            status TEXT NOT NULL DEFAULT 'confirmed',
            source_key TEXT,
            equipment_unreviewed INTEGER NOT NULL DEFAULT 0,
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
        CREATE UNIQUE INDEX IF NOT EXISTS idx_trial_records_source_key
            ON trial_records(source_key) WHERE source_key IS NOT NULL;
        CREATE TABLE IF NOT EXISTS trial_auto_scan_tombstones (
            source_key TEXT PRIMARY KEY,
            deleted_at TEXT NOT NULL
        );
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

        -- V16: records 表索引优化（表达式索引 + 复合索引）
        -- 表达式索引：加速 json_extract(data, '$.raidName') 查询（CD 窗口去重 hot path）
        CREATE INDEX IF NOT EXISTS idx_records_data_raid_name
        ON records(json_extract(data, '$.raidName'));
        -- 复合索引：覆盖 CD 窗口去重检查
        CREATE INDEX IF NOT EXISTS idx_records_cd_lookup
        ON records(account_id, raid_name, source, record_date);
        -- 复合索引：覆盖 pending/scanning 查询 + 排序
        CREATE INDEX IF NOT EXISTS idx_records_pending
        ON records(status, record_date DESC);
        -- 复合索引：覆盖手工记录查询
        CREATE INDEX IF NOT EXISTS idx_records_manual_cd
        ON records(account_id, status, record_date);

        CREATE INDEX IF NOT EXISTS idx_seasons_version_id ON seasons(version_id);
        CREATE INDEX IF NOT EXISTS idx_raids_season_id ON raids(season_id);

        -- ===== V14: 应用配置表（key-value 存储） =====
        CREATE TABLE IF NOT EXISTS app_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        -- ===== V18: 跨角色设置同步「已回滚」标记表 =====
        -- 与 db/migrations/v18.rs 保持一致。全新安装不走增量迁移，
        -- 必须在此显式建表，否则该功能在全新安装用户机器上不可用。
        CREATE TABLE IF NOT EXISTS char_sync_rollback_marks (
            backup_dir TEXT PRIMARY KEY,
            rolled_back_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_char_sync_rollback_marks_rolled_back_at
        ON char_sync_rollback_marks (rolled_back_at DESC);

        -- ===== 初始数据 =====
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (1, 'raid', '团队副本');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (2, 'baizhan', '百战异闻录');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (3, 'trial', '试炼之地');
    "#,
    )
    .map_err(|e| e.to_string())?;

    ensure_trial_draws_table(conn)?;

    // V14: 插入 app_config 默认值（与迁移脚本保持一致）
    let now = chrono::Local::now().to_rfc3339();
    for (key, default_value) in [
        ("game_directory", ""),
        ("setup_completed", "false"),
        ("last_scan_mingyi_at", ""),
        ("config_json", ""),
        ("local_storage_migrated", "false"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, default_value, &now],
        )
        .map_err(|e| e.to_string())?;
    }

    log::info!("[INSTALL] 数据库结构创建完成");
    Ok(())
}

/// 测试辅助：在指定路径上创建全新安装库并返回连接。
///
/// 生产路径是 `install_fresh_db`（内部创建 `create_latest_schema`），
/// 但那条路径依赖全局单例与真实数据目录，不适合单元测试。
/// 这里直接复刻 `install_fresh_db` 的语义（建表 + 写入 schema 版本 + 静态副本），
/// 供 upgrade_tests 验证表结构完整性。
#[cfg(test)]
pub fn create_latest_schema_for_test(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    // 与 init_db 的真实顺序一致：先建版本表，再建业务表，最后写版本号。
    ensure_version_tables(&conn)?;
    create_latest_schema(&conn)?;
    set_schema_version(&conn, CURRENT_SCHEMA_VERSION, "测试初始安装")?;
    migration::init_static_raids(&conn)?;
    ensure_char_sync_rollback_marks_table(&conn)?;
    Ok(conn)
}

/// 测试辅助：对给定连接执行升级后校验，供 upgrade_tests 断言期望表清单。
#[cfg(test)]
pub fn validate_post_upgrade_for_test(conn: &Connection, from_version: i32) -> Result<(), String> {
    let _ = from_version;
    validate_post_upgrade(conn)
}

/// 检查 localStorage 迁移是否已完成
#[tauri::command]
pub fn db_is_local_storage_migrated() -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let migrated: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM app_config WHERE key = 'local_storage_migrated' AND value = 'true'",
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
    let now = chrono::Local::now().to_rfc3339();

    conn.execute(
        "INSERT INTO app_config (key, value, updated_at) VALUES ('local_storage_migrated', 'true', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![now],
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
            "SELECT COUNT(*) FROM app_config WHERE key = 'local_storage_migrated' AND value = 'true'",
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
    /// Editing intent only: keep a concurrently recognized card position when the
    /// form itself did not change it. This value is never persisted.
    #[serde(rename = "preserveFlippedIndex", default, skip_serializing)]
    pub preserve_flipped_index: bool,
    /// Editing intent only: cards the form explicitly edited. Any other empty
    /// card in an incomplete automatic record retains a newer OCR observation.
    /// This value is never persisted.
    #[serde(
        rename = "editedCardIndices",
        default,
        skip_serializing
    )]
    pub edited_card_indices: Vec<i64>,
    #[serde(rename = "type", default)]
    pub record_type: String,
    #[serde(default = "default_trial_source")]
    pub source: String,
    #[serde(default = "default_trial_status")]
    pub status: String,
    #[serde(rename = "sourceKey", default)]
    pub source_key: Option<String>,
    /// `None` means an older client did not send an equipment-review decision.
    #[serde(rename = "equipmentUnreviewed", default)]
    pub equipment_unreviewed: Option<bool>,
    /// Per-flip facts for a continuous trial run. Older records deliberately
    /// remain empty: their single legacy card fields cannot be assigned to a
    /// particular flip without inventing evidence.
    #[serde(default)]
    pub draws: Vec<TrialDraw>,
    pub date: i64, // Changed from String to i64 (timestamp)
    pub notes: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrialDrawReward {
    pub name: String,
    pub quantity: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TrialDraw {
    pub id: String,
    #[serde(rename = "sourceKey")]
    pub source_key: String,
    pub sequence: i64,
    /// More than one receipt shares this second/timestamp, so persistence order
    /// is deterministic but the game-side flip order is not proven.
    #[serde(rename = "sequenceAmbiguous", default)]
    pub sequence_ambiguous: bool,
    #[serde(rename = "receiptTime")]
    pub receipt_time: i64,
    #[serde(rename = "receiptHash", default)]
    pub receipt_hash: Option<String>,
    #[serde(rename = "flippedIndex")]
    pub flipped_index: i64,
    #[serde(rename = "rewardItems", default)]
    pub reward_items: Vec<TrialDrawReward>,
    #[serde(rename = "staminaCost", default)]
    pub stamina_cost: Option<i64>,
    #[serde(rename = "staminaCosts", default)]
    pub stamina_costs: Vec<i64>,
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
    #[serde(
        rename = "equipmentUnreviewed",
        default = "default_draw_equipment_unreviewed"
    )]
    pub equipment_unreviewed: bool,
    /// Cards explicitly reviewed by the user. This includes a deliberately
    /// empty card, so consumers must not infer a fully reviewed five-card
    /// board merely from this list.
    #[serde(rename = "manualEquipmentCardIndexes", default)]
    pub manual_equipment_card_indexes: Vec<i64>,
    /// Child-form editing intent only. It is never persisted or returned:
    /// omitted cards retain a newer explicit per-card decision already stored
    /// for the same manual draw.
    #[serde(rename = "editedCardIndices", default, skip_serializing)]
    pub edited_card_indices: Vec<i64>,
    /// `manual` is a user-entered flip with no log receipt time. `auto` is an
    /// independently evidenced receipt. Consumers must not infer an ordering
    /// or one-to-one relationship between the two kinds.
    #[serde(default = "default_trial_source")]
    pub source: String,
}

fn default_draw_equipment_unreviewed() -> bool {
    true
}

fn default_trial_source() -> String {
    "manual".to_string()
}

fn default_trial_status() -> String {
    "confirmed".to_string()
}

fn normalize_trial_source(source: &str) -> &'static str {
    if source == "auto" { "auto" } else { "manual" }
}

fn normalize_trial_status(status: &str) -> &'static str {
    if status == "pending" { "pending" } else { "confirmed" }
}

/// A completed trial candidate emitted by the JCL scanner.
///
/// `source_key` must deterministically identify the terminal completion evidence.
#[derive(Debug, Clone)]
pub struct AutoTrialRecord {
    pub source_key: String,
    pub account_id: String,
    pub role_id: String,
    pub role_name: String,
    pub server: String,
    pub layer: i64,
    pub bosses: [String; 3],
    /// `confirmed` when terminal completion evidence is complete, otherwise `pending`.
    pub status: String,
    pub date: i64,
    pub notes: Option<String>,
}

/// One independently identifiable reward receipt belonging to an automatic
/// trial completion. `source_key` is a stable log-derived key (role + receipt
/// time/hash); it is intentionally separate from the terminal completion key.
#[derive(Debug, Clone)]
pub struct AutoTrialDraw {
    pub source_key: String,
    pub receipt_time: i64,
    /// ChatLog hash is kept as text so JavaScript callers never lose precision.
    pub receipt_hash: Option<String>,
    pub reward_items: Vec<TrialDrawReward>,
    pub reward_source_hashes: Vec<String>,
    /// `None` means the receipt had no proven stamina deduction.
    pub stamina_cost: Option<i64>,
    /// Individual same-second deductions, retained even when their sum is also
    /// available in `stamina_cost`.
    pub stamina_costs: Vec<i64>,
    pub stamina_source_hashes: Vec<String>,
}

impl AutoTrialDraw {
    fn only_its_own_receipt(&self) -> Self {
        let mut copy = self.clone();
        copy.reward_items.clear();
        copy.reward_source_hashes.clear();
        for (item, source) in self.reward_items.iter().zip(&self.reward_source_hashes) {
            if self.receipt_hash.as_ref() == Some(source) {
                copy.reward_items.push(item.clone());
                copy.reward_source_hashes.push(source.clone());
            }
        }
        copy.stamina_cost = None;
        copy.stamina_costs.clear();
        copy.stamina_source_hashes.clear();
        copy
    }
}

fn merge_draw_facts<T: Clone>(
    saved: Vec<T>,
    saved_sources: Vec<String>,
    incoming: &[T],
    incoming_sources: &[String],
    fallback: impl Fn(&T) -> String,
) -> (Vec<T>, Vec<String>) {
    let mut merged = saved;
    let mut keys: Vec<String> = merged
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let key = saved_sources.get(index).cloned().unwrap_or_default();
            if key.trim().is_empty() {
                fallback(value)
            } else {
                key
            }
        })
        .collect();
    for (index, value) in incoming.iter().enumerate() {
        let key = incoming_sources.get(index).cloned().unwrap_or_default();
        let key = if key.trim().is_empty() {
            fallback(value)
        } else {
            key
        };
        if !keys.iter().any(|saved_key| saved_key == &key) {
            keys.push(key);
            merged.push(value.clone());
        }
    }
    (merged, keys)
}

#[tauri::command]
pub fn db_add_trial_record(record: String) -> Result<(), String> {
    log::info!("[DB] 添加试炼之地记录: {}", record);
    
    let item: TrialRecord = serde_json::from_str(&record).map_err(|e| {
        let err_msg = format!("解析试炼记录失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;
    
    let mut conn = init_db().map_err(|e| {
        let err_msg = format!("初始化数据库失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;
    save_trial_record_with_connection(&mut conn, &item)
}

fn save_trial_record_with_connection(conn: &mut Connection, item: &TrialRecord) -> Result<(), String> {
    let timestamp = get_local_timestamp();
    let source = normalize_trial_source(&item.source);
    let status = normalize_trial_status(&item.status);
    let preserve_observed_card = |card_index| {
        !item.edited_card_indices.contains(&card_index)
    };
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;

    // Current UI preserves sourceKey. Older UI callbacks may omit it, but automatic
    // record ids carry the same deterministic key and must still respect tombstones.
    let replay_source_key = item
        .source_key
        .as_deref()
        .or_else(|| item.id.strip_prefix("trial-auto:"));
    if let Some(source_key) = replay_source_key {
        let deleted: bool = tx
            .query_row(
                "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
                params![source_key],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if deleted {
            return Err("该自动试炼记录已删除，请刷新页面后再操作".to_string());
        }
    }

    // A form can remain open while a later scan claims the same completion.
    // Re-read persisted ownership inside this transaction: the stale form must
    // not overwrite, replace, or silently diverge from receipt-backed child
    // rows merely because its original payload had no sourceKey yet.
    if source == "manual" && item.source_key.is_none() {
        let claimed_or_has_auto_draws: bool = tx
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM trial_records r
                    WHERE r.id = ?1 AND r.source = 'manual' AND (
                        r.source_key IS NOT NULL OR EXISTS (
                            SELECT 1 FROM trial_draws d
                            WHERE d.trial_record_id = r.id AND d.source = 'auto'
                        )
                    )
                )",
                params![item.id],
                |row| row.get(0),
            )
            .map_err(|e| format!("检查试炼记录归属失败: {e}"))?;
        if claimed_or_has_auto_draws {
            return Err("该试炼已被自动记录认领，请刷新后在翻牌明细中逐笔补录".to_string());
        }
    }

    // Ensure bosses are serialized
    let bosses_json = serde_json::to_string(&item.bosses).unwrap_or_default();

    tx.execute(
        "INSERT INTO trial_records (
            id, account_id, role_id, role_name, server, layer, bosses,
            card_1, card_2, card_3, card_4, card_5, flipped_index, record_type, source, status, source_key,
            equipment_unreviewed, date, notes, updated_at
        )
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, COALESCE(?, 0), ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            role_id = excluded.role_id,
            role_name = excluded.role_name,
            server = excluded.server,
            layer = excluded.layer,
            bosses = excluded.bosses,
            card_1 = CASE
                WHEN ?
                  AND trial_records.source = 'auto'
                  AND trial_records.equipment_unreviewed = 1
                  AND COALESCE(TRIM(excluded.card_1), '') = ''
                THEN trial_records.card_1
                ELSE excluded.card_1
            END,
            card_2 = CASE
                WHEN ?
                  AND trial_records.source = 'auto'
                  AND trial_records.equipment_unreviewed = 1
                  AND COALESCE(TRIM(excluded.card_2), '') = ''
                THEN trial_records.card_2
                ELSE excluded.card_2
            END,
            card_3 = CASE
                WHEN ?
                  AND trial_records.source = 'auto'
                  AND trial_records.equipment_unreviewed = 1
                  AND COALESCE(TRIM(excluded.card_3), '') = ''
                THEN trial_records.card_3
                ELSE excluded.card_3
            END,
            card_4 = CASE
                WHEN ?
                  AND trial_records.source = 'auto'
                  AND trial_records.equipment_unreviewed = 1
                  AND COALESCE(TRIM(excluded.card_4), '') = ''
                THEN trial_records.card_4
                ELSE excluded.card_4
            END,
            card_5 = CASE
                WHEN ?
                  AND trial_records.source = 'auto'
                  AND trial_records.equipment_unreviewed = 1
                  AND COALESCE(TRIM(excluded.card_5), '') = ''
                THEN trial_records.card_5
                ELSE excluded.card_5
            END,
            flipped_index = CASE
                WHEN ? THEN trial_records.flipped_index
                ELSE excluded.flipped_index
            END,
            record_type = excluded.record_type,
            source = CASE
                WHEN excluded.source_key IS NULL AND trial_records.source = 'auto' THEN trial_records.source
                ELSE excluded.source
            END,
            status = CASE
                WHEN excluded.source_key IS NULL AND trial_records.source = 'auto' THEN trial_records.status
                ELSE excluded.status
            END,
            source_key = COALESCE(trial_records.source_key, excluded.source_key),
            equipment_unreviewed = CASE
                WHEN ? IS NULL THEN trial_records.equipment_unreviewed
                ELSE excluded.equipment_unreviewed
            END,
            date = excluded.date,
            notes = excluded.notes,
            updated_at = excluded.updated_at",
        params![
            &item.id,
            &item.account_id,
            &item.role_id,
            &item.role_name,
            &item.server,
            item.layer,
            bosses_json,
            &item.card_1,
            &item.card_2,
            &item.card_3,
            &item.card_4,
            &item.card_5,
            item.flipped_index,
            &item.record_type,
            source,
            status,
            item.source_key.as_deref(),
            item.equipment_unreviewed,
            item.date,
            &item.notes,
            timestamp,
            preserve_observed_card(1),
            preserve_observed_card(2),
            preserve_observed_card(3),
            preserve_observed_card(4),
            preserve_observed_card(5),
            item.preserve_flipped_index,
            item.equipment_unreviewed,
        ],
    )
    .map_err(|e| {
        let err_msg = format!("保存试炼记录失败: {}", e);
        log::error!("[DB] {}", err_msg);
        err_msg
    })?;

    // A manual form may model one clearance with several independent flips.
    // These rows deliberately have no reward/stamina facts or receipt time:
    // user input is evidence for its selected position/card board only, not a
    // fabricated game receipt. Once a completion is claimed by automatic
    // evidence (`source_key` is present), this bulk path must stay inert so a
    // parent-form save cannot replace the scanner's receipt rows.
    if source == "manual" && item.source_key.is_none() && !item.draws.is_empty() {
        save_manual_trial_draws_with_transaction(&tx, item, &timestamp)?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    log::info!("[DB] 试炼之地记录保存成功: id={}, roleId={}, layer={}", item.id, item.role_id, item.layer);
    Ok(())
}

fn save_manual_trial_draws_with_transaction(
    tx: &rusqlite::Transaction<'_>,
    item: &TrialRecord,
    updated_at: &str,
) -> Result<(), String> {
    let mut seen_ids = HashSet::new();
    let mut seen_source_keys = HashSet::new();
    for draw in &item.draws {
        if draw.id.trim().is_empty() {
            return Err("手工翻牌明细缺少标识，请重新打开记录后再保存".to_string());
        }
        if !(0..=5).contains(&draw.flipped_index) {
            return Err("手工翻牌位必须在 1 到 5 之间，或保持未知".to_string());
        }
        if draw.edited_card_indices.iter().any(|card_index| !(1..=5).contains(card_index)) {
            return Err("手工装备牌位必须在 1 到 5 之间".to_string());
        }
        let expected_source_key = format!("manual:{}:{}", item.id, draw.id);
        if draw.source_key != expected_source_key {
            return Err("手工翻牌明细来源不匹配，请重新打开记录后再保存".to_string());
        }
        if !seen_ids.insert(draw.id.as_str()) || !seen_source_keys.insert(draw.source_key.as_str()) {
            return Err("同一轮中的手工翻牌明细不能重复".to_string());
        }
        let existing_owner: Option<(String, String)> = tx
            .query_row(
                "SELECT trial_record_id, source FROM trial_draws WHERE id = ?1",
                params![draw.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| format!("检查手工翻牌明细冲突失败: {e}"))?;
        if let Some((owner_id, existing_source)) = existing_owner {
            if owner_id != item.id || existing_source != "manual" {
                return Err("手工翻牌明细已属于其他记录，请重新打开记录后再保存".to_string());
            }
        }
    }

    let existing_manual_ids: HashSet<String> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM trial_draws
                 WHERE trial_record_id = ?1 AND source = 'manual'",
            )
            .map_err(|e| format!("读取已有手工翻牌明细失败: {e}"))?;
        let existing_rows = stmt.query_map(params![item.id], |row| row.get(0))
            .map_err(|e| format!("读取已有手工翻牌明细失败: {e}"))?
            .collect::<std::result::Result<HashSet<_>, _>>()
            .map_err(|e| format!("读取已有手工翻牌明细失败: {e}"))?;
        existing_rows
    };
    let desired_ids: HashSet<&str> = item.draws.iter().map(|draw| draw.id.as_str()).collect();

    // Delete only entries explicitly removed from this manual list. Retained
    // IDs are upserted below, keeping their per-card manual override rows (an
    // explicit clear is evidence too) intact.
    for stale_id in existing_manual_ids.iter().filter(|id| !desired_ids.contains(id.as_str())) {
        tx.execute(
            "DELETE FROM trial_draw_equipment_manual_overrides WHERE draw_id = ?1",
            params![stale_id],
        )
        .map_err(|e| format!("清理已删除手工翻牌装备覆盖失败: {e}"))?;
        tx.execute(
            "DELETE FROM trial_draws
             WHERE id = ?1 AND trial_record_id = ?2 AND source = 'manual'",
            params![stale_id, item.id],
        )
        .map_err(|e| format!("删除已移除手工翻牌明细失败: {e}"))?;
    }

    for (index, draw) in item.draws.iter().enumerate() {
        let equipment_unreviewed = i64::from(draw.equipment_unreviewed);
        tx.execute(
            "INSERT INTO trial_draws (
                id, trial_record_id, source_key, sequence, receipt_time, receipt_hash,
                reward_items, reward_sources, stamina_cost, stamina_costs, stamina_sources,
                flipped_index, position_manual, card_1, card_2, card_3, card_4, card_5,
                equipment_unreviewed, source, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, 0, NULL,
                '[]', '[]', NULL, '[]', '[]',
                ?5, 1, ?6, ?7, ?8, ?9, ?10,
                ?11, 'manual', ?12
             )
             ON CONFLICT(id) DO UPDATE SET
                trial_record_id = excluded.trial_record_id,
                source_key = excluded.source_key,
                sequence = excluded.sequence,
                receipt_time = 0,
                receipt_hash = NULL,
                reward_items = '[]', reward_sources = '[]',
                stamina_cost = NULL, stamina_costs = '[]', stamina_sources = '[]',
                flipped_index = excluded.flipped_index,
                position_manual = 1,
                card_1 = CASE WHEN EXISTS (
                    SELECT 1 FROM trial_draw_equipment_manual_overrides m
                    WHERE m.draw_id = trial_draws.id AND m.card_index = 1
                ) AND ?13 THEN trial_draws.card_1 ELSE excluded.card_1 END,
                card_2 = CASE WHEN EXISTS (
                    SELECT 1 FROM trial_draw_equipment_manual_overrides m
                    WHERE m.draw_id = trial_draws.id AND m.card_index = 2
                ) AND ?14 THEN trial_draws.card_2 ELSE excluded.card_2 END,
                card_3 = CASE WHEN EXISTS (
                    SELECT 1 FROM trial_draw_equipment_manual_overrides m
                    WHERE m.draw_id = trial_draws.id AND m.card_index = 3
                ) AND ?15 THEN trial_draws.card_3 ELSE excluded.card_3 END,
                card_4 = CASE WHEN EXISTS (
                    SELECT 1 FROM trial_draw_equipment_manual_overrides m
                    WHERE m.draw_id = trial_draws.id AND m.card_index = 4
                ) AND ?16 THEN trial_draws.card_4 ELSE excluded.card_4 END,
                card_5 = CASE WHEN EXISTS (
                    SELECT 1 FROM trial_draw_equipment_manual_overrides m
                    WHERE m.draw_id = trial_draws.id AND m.card_index = 5
                ) AND ?17 THEN trial_draws.card_5 ELSE excluded.card_5 END,
                equipment_unreviewed = excluded.equipment_unreviewed,
                source = 'manual',
                updated_at = excluded.updated_at",
            params![
                draw.id,
                item.id,
                draw.source_key,
                (index + 1) as i64,
                draw.flipped_index,
                draw.card_1.trim(),
                draw.card_2.trim(),
                draw.card_3.trim(),
                draw.card_4.trim(),
                draw.card_5.trim(),
                equipment_unreviewed,
                updated_at,
                !draw.edited_card_indices.contains(&1),
                !draw.edited_card_indices.contains(&2),
                !draw.edited_card_indices.contains(&3),
                !draw.edited_card_indices.contains(&4),
                !draw.edited_card_indices.contains(&5),
            ],
        )
        .map_err(|e| format!("保存第 {} 次手工翻牌失败: {e}", index + 1))?;
        for card_index in &draw.edited_card_indices {
            let equipment_id = match *card_index {
                1 => &draw.card_1,
                2 => &draw.card_2,
                3 => &draw.card_3,
                4 => &draw.card_4,
                5 => &draw.card_5,
                _ => unreachable!("manual card indices were validated above"),
            };
            let equipment_id = equipment_id.trim();
            tx.execute(
                "INSERT INTO trial_draw_equipment_manual_overrides
                     (draw_id, card_index, equipment_id, source, updated_at)
                 VALUES (?1, ?2, ?3, 'manual', ?4)
                 ON CONFLICT(draw_id, card_index) DO UPDATE SET
                     equipment_id = excluded.equipment_id,
                     source = excluded.source,
                     updated_at = excluded.updated_at",
                params![
                    draw.id,
                    card_index,
                    (!equipment_id.is_empty()).then_some(equipment_id),
                    updated_at,
                ],
            )
            .map_err(|e| format!("保存第 {} 次手工翻牌装备覆盖失败: {e}", index + 1))?;
        }
    }
    Ok(())
}

const AUTO_TRIAL_MANUAL_MATCH_WINDOW_MS: i64 = 20 * 60 * 1000;

fn normalized_trial_bosses(bosses: &[String]) -> Vec<String> {
    bosses.iter().map(|boss| boss.trim().to_string()).collect()
}

fn find_matching_manual_trial_record(
    conn: &Connection,
    record: &AutoTrialRecord,
) -> Result<Option<String>, String> {
    let earliest = record.date.saturating_sub(AUTO_TRIAL_MANUAL_MATCH_WINDOW_MS);
    let latest = record.date.saturating_add(AUTO_TRIAL_MANUAL_MATCH_WINDOW_MS);
    let mut stmt = conn
        .prepare(
            "SELECT id, layer, bosses, date FROM trial_records
             WHERE role_id = ?1 AND source = 'manual' AND source_key IS NULL
               AND date BETWEEN ?2 AND ?3",
        )
        .map_err(|e| e.to_string())?;
    let expected_bosses = normalized_trial_bosses(&record.bosses);
    let rows = stmt
        .query_map(params![record.role_id, earliest, latest], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut closest: Option<(String, i64)> = None;
    for row in rows {
        let (id, layer, bosses_json, date) = row.map_err(|e| e.to_string())?;
        let Ok(bosses) = serde_json::from_str::<Vec<String>>(&bosses_json) else {
            continue;
        };
        if layer != record.layer || normalized_trial_bosses(&bosses) != expected_bosses {
            continue;
        }
        let distance = (date - record.date).abs();
        match closest.as_ref() {
            None => {
                closest = Some((id, distance));
            }
            Some((best_id, best_distance)) if distance < *best_distance || (distance == *best_distance && id < *best_id) => {
                closest = Some((id, distance));
            }
            _ => {}
        }
    }
    Ok(closest.map(|(id, _)| id))
}

fn claim_matching_manual_trial_record(
    conn: &Connection,
    record: &AutoTrialRecord,
    updated_at: &str,
) -> Result<bool, String> {
    let Some(manual_record_id) = find_matching_manual_trial_record(conn, record)? else {
        return Ok(false);
    };
    snapshot_legacy_manual_trial_flip_if_needed(conn, &manual_record_id, updated_at)?;
    let claimed = conn
        .execute(
            "UPDATE trial_records SET source_key = ?1, updated_at = ?2
             WHERE id = ?3 AND source = 'manual' AND source_key IS NULL",
            params![record.source_key, updated_at, manual_record_id],
        )
        .map_err(|e| format!("认领手工试炼记录失败: {}", e))?;
    Ok(claimed == 1)
}

/// Once a manual parent gains child draws, UI and statistics deliberately use
/// the child model. Preserve old single-flip parent fields as an explicitly
/// un-attributed manual child before terminal evidence claims the parent. This
/// retains the user's information without guessing which later log receipt it
/// belongs to.
fn snapshot_legacy_manual_trial_flip_if_needed(
    conn: &Connection,
    record_id: &str,
    updated_at: &str,
) -> Result<(), String> {
    let has_manual_draw: bool = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM trial_draws
                WHERE trial_record_id = ?1 AND source = 'manual'
            )",
            params![record_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查旧手工翻牌快照失败: {e}"))?;
    if has_manual_draw {
        return Ok(());
    }
    let parent: Option<(i64, String, String, String, String, String, i64)> = conn
        .query_row(
            "SELECT flipped_index, card_1, card_2, card_3, card_4, card_5, equipment_unreviewed
             FROM trial_records WHERE id = ?1 AND source = 'manual'",
            params![record_id],
            |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
            )),
        )
        .optional()
        .map_err(|e| format!("读取旧手工翻牌快照失败: {e}"))?;
    let Some((flipped_index, card_1, card_2, card_3, card_4, card_5, equipment_unreviewed)) = parent else {
        return Ok(());
    };
    let has_explicit_board = (1..=5).contains(&flipped_index)
        || [card_1.as_str(), card_2.as_str(), card_3.as_str(), card_4.as_str(), card_5.as_str()]
            .iter()
            .any(|card| !card.trim().is_empty());
    if !has_explicit_board {
        return Ok(());
    }
    let draw_id = format!("trial-manual-legacy:{record_id}");
    let source_key = format!("manual:legacy:{record_id}");
    conn.execute(
        "INSERT OR IGNORE INTO trial_draws (
            id, trial_record_id, source_key, sequence, receipt_time, receipt_hash,
            reward_items, reward_sources, stamina_cost, stamina_costs, stamina_sources,
            flipped_index, position_manual, card_1, card_2, card_3, card_4, card_5,
            equipment_unreviewed, source, updated_at
         ) VALUES (
            ?1, ?2, ?3, 1, 0, NULL,
            '[]', '[]', NULL, '[]', '[]',
            ?4, 1, ?5, ?6, ?7, ?8, ?9,
            ?10, 'manual', ?11
         )",
        params![
            draw_id, record_id, source_key, flipped_index,
            card_1, card_2, card_3, card_4, card_5,
            equipment_unreviewed, updated_at,
        ],
    )
    .map_err(|e| format!("保存旧手工翻牌快照失败: {e}"))?;
    Ok(())
}

/// Inserts or updates scanner candidates using the scanner's existing database connection.
///
/// The stable source key prevents repeated polling from creating duplicates. A deleted
/// automatically collected record leaves a tombstone, so a historical scan cannot revive it.
/// An exact same-role, same-layer, same-three-boss manual record in a short time window wins.
/// Fully equal manual candidates are claimed in stable record-id order, one evidence key each.
pub fn db_upsert_auto_trial_records_with_connection(
    conn: &mut Connection,
    records: &[AutoTrialRecord],
) -> Result<usize, String> {
    if records.is_empty() {
        return Ok(0);
    }

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let now = get_local_timestamp();
    let mut changed = 0;

    for record in records {
        if record.source_key.trim().is_empty() {
            log::warn!("[TrialAutoScan] 跳过缺少 source_key 的试炼候选");
            continue;
        }
        if record.role_id.trim().is_empty() || record.date <= 0 {
            log::warn!("[TrialAutoScan] 跳过缺少角色或完成时间的试炼候选: {}", record.source_key);
            continue;
        }
        let status = normalize_trial_status(&record.status);

        let deleted: bool = tx
            .query_row(
                "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
                params![record.source_key],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if deleted {
            continue;
        }

        let existing: Option<(String, String, String)> = tx
            .query_row(
                "SELECT id, source, status FROM trial_records WHERE source_key = ?1",
                params![record.source_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if matches!(existing.as_ref().map(|(_, source, _)| source.as_str()), Some("manual")) {
            continue;
        }
        if let Some((pending_id, source, existing_status)) = existing.as_ref() {
            if source == "auto" && existing_status == "pending" && status == "confirmed" {
                if let Some(manual_record_id) = find_matching_manual_trial_record(&tx, record)? {
                    // Preserve independently recorded receipts/OCR before the
                    // pending parent is removed. The manually confirmed record
                    // becomes the owner of the same run without reinterpreting
                    // any card fields.
                    snapshot_legacy_manual_trial_flip_if_needed(&tx, &manual_record_id, &now)?;
                    tx.execute(
                        "UPDATE trial_draws SET trial_record_id = ?1 WHERE trial_record_id = ?2",
                        params![manual_record_id, pending_id],
                    )
                    .map_err(|e| format!("迁移待确认试炼翻牌明细失败: {e}"))?;
                    let removed = tx
                        .execute(
                            "DELETE FROM trial_records
                             WHERE id = ?1 AND source = 'auto' AND status = 'pending' AND source_key = ?2",
                            params![pending_id, record.source_key],
                        )
                        .map_err(|e| format!("合并待确认试炼记录失败: {}", e))?;
                    if removed == 1 {
                        let claimed = tx
                            .execute(
                                "UPDATE trial_records SET source_key = ?1, updated_at = ?2
                                 WHERE id = ?3 AND source = 'manual' AND source_key IS NULL",
                                params![record.source_key, now, manual_record_id],
                            )
                            .map_err(|e| format!("认领手工试炼记录失败: {e}"))?;
                        if claimed != 1 {
                            return Err("试炼记录合并时手工记录已变化，请重试扫描".to_string());
                        }
                        changed += 1;
                        log::info!("[TrialAutoScan] 完整证据已合并到手工试炼记录: {}", record.source_key);
                        continue;
                    }
                }
            }
        } else {
            if claim_matching_manual_trial_record(&tx, record, &now)? {
                log::info!("[TrialAutoScan] 已认领相同手工试炼记录，跳过自动补记: {}", record.source_key);
                continue;
            }
        }

        let bosses_json = serde_json::to_string(&record.bosses).map_err(|e| e.to_string())?;
        let record_id = format!("trial-auto:{}", record.source_key);
        let changed_rows = tx
            .execute(
                "INSERT INTO trial_records (
                    id, account_id, role_id, role_name, server, layer, bosses,
                    card_1, card_2, card_3, card_4, card_5, flipped_index,
                    record_type, source, status, source_key, equipment_unreviewed, date, notes, updated_at
                ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7,
                    '', '', '', '', '', 0,
                    'trial', 'auto', ?8, ?9, 1, ?10, ?11, ?12
                )
                ON CONFLICT(source_key) WHERE source_key IS NOT NULL DO UPDATE SET
                    account_id = excluded.account_id,
                    role_id = excluded.role_id,
                    role_name = excluded.role_name,
                    server = excluded.server,
                    layer = excluded.layer,
                    bosses = excluded.bosses,
                    date = excluded.date,
                    notes = excluded.notes,
                    status = CASE WHEN trial_records.status = 'confirmed' THEN 'confirmed' ELSE excluded.status END,
                    updated_at = excluded.updated_at
                WHERE trial_records.source = 'auto'
                  AND trial_records.status = 'pending'
                  AND excluded.status = 'confirmed'",
                params![
                    record_id,
                    record.account_id,
                    record.role_id,
                    record.role_name,
                    record.server,
                    record.layer,
                    bosses_json,
                    status,
                    record.source_key,
                    record.date,
                    record.notes,
                    now,
                ],
            )
            .map_err(|e| format!("保存自动试炼记录失败: {}", e))?;
        changed += changed_rows;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed)
}

/// Stores reward receipts that belong to one already-persisted automatic trial.
///
/// Repeated scans are idempotent by draw source key. Delayed scans may enrich
/// an empty reward/stamina observation, but can never replace a child the user
/// edited manually. The parent completion is intentionally not created here:
/// callers must save terminal evidence first, so draw-only data cannot count as
/// a second clearance.
pub fn db_upsert_auto_trial_draws_with_connection(
    conn: &mut Connection,
    parent_source_key: &str,
    draws: &[AutoTrialDraw],
) -> Result<usize, String> {
    if parent_source_key.trim().is_empty() || draws.is_empty() {
        return Ok(0);
    }

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let parent_deleted: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
            params![parent_source_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if parent_deleted {
        return Ok(0);
    }

    let parent: Option<(String, String)> = tx
        .query_row(
            "SELECT id, source FROM trial_records WHERE source_key = ?1",
            params![parent_source_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((parent_id, _parent_source)) = parent else {
        return Ok(0);
    };
    // A manually maintained parent can still have uniquely linked log receipts.
    // Keep its legacy cards untouched, but let the child facts arrive.

    let mut changed = 0;
    let now = get_local_timestamp();
    for draw in draws {
        if draw.source_key.trim().is_empty() || draw.receipt_time <= 0 {
            log::warn!("[TrialAutoScan] 跳过缺少来源键或领取时间的翻牌明细");
            continue;
        }
        let draw_deleted: bool = tx
            .query_row(
                "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
                params![draw.source_key],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if draw_deleted {
            continue;
        }

        // 聊天库可能在下一次读取时才出现同秒的另一张银叶子。此后只能把每张
        // 银叶子本身归到对应收据，同秒其余物品和扣精力行都不能再猜给其中一笔。
        let has_sibling: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM trial_draws WHERE trial_record_id=?1 AND receipt_time=?2 AND source_key!=?3)",
            params![parent_id, draw.receipt_time, draw.source_key], |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        let ambiguous = has_sibling || draws.iter().any(|other|
            other.receipt_time == draw.receipt_time && other.source_key != draw.source_key);
        let leaf_only;
        let draw = if ambiguous {
            leaf_only = draw.only_its_own_receipt();
            &leaf_only
        } else { draw };
        let reward_items = serde_json::to_string(&draw.reward_items)
            .map_err(|e| format!("序列化试炼翻牌奖励失败: {e}"))?;
        let reward_sources = serde_json::to_string(&draw.reward_source_hashes)
            .map_err(|e| format!("序列化试炼翻牌奖励来源失败: {e}"))?;
        let stamina_costs = serde_json::to_string(&draw.stamina_costs)
            .map_err(|e| format!("序列化试炼翻牌精力明细失败: {e}"))?;
        let stamina_sources = serde_json::to_string(&draw.stamina_source_hashes)
            .map_err(|e| format!("序列化试炼翻牌精力来源失败: {e}"))?;
        let existing: Option<(String, String, String, String, String)> = tx
            .query_row(
                "SELECT source, reward_items, reward_sources, stamina_costs, stamina_sources
                 FROM trial_draws WHERE source_key = ?1",
                params![draw.source_key],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;

        match existing {
            None => {
                tx.execute(
                    "INSERT INTO trial_draws (
                        id, trial_record_id, source_key, sequence, receipt_time, receipt_hash,
                        reward_items, reward_sources, stamina_cost, stamina_costs, stamina_sources, flipped_index, position_manual,
                        card_1, card_2, card_3, card_4, card_5, equipment_unreviewed, source, updated_at
                    ) VALUES (
                        ?1, ?2, ?3, 0, ?4, ?5,
                        ?6, ?7, ?8, ?9, ?10, 0, 0,
                        '', '', '', '', '', 1, 'auto', ?11
                    )",
                    params![
                        format!("trial-draw:{}", draw.source_key),
                        parent_id,
                        draw.source_key,
                        draw.receipt_time,
                        draw.receipt_hash,
                        reward_items,
                        reward_sources,
                        draw.stamina_cost,
                        stamina_costs,
                        stamina_sources,
                        now,
                    ],
                )
                .map_err(|e| format!("保存试炼翻牌明细失败: {e}"))?;
                invalidate_auto_trial_draw_observations_for_receipt(
                    &tx,
                    &parent_id,
                    draw.receipt_time,
                    None,
                )?;
                if has_sibling {
                    tx.execute(
                        "UPDATE trial_draws AS d SET
                           reward_items=(SELECT json_group_array(json(items.value)) FROM json_each(d.reward_items) items
                              JOIN json_each(d.reward_sources) hashes ON hashes.key=items.key WHERE hashes.value=d.receipt_hash),
                           reward_sources=(SELECT json_group_array(value) FROM json_each(d.reward_sources) WHERE value=d.receipt_hash),
                           stamina_cost=NULL, stamina_costs='[]', stamina_sources='[]', updated_at=?3
                         WHERE trial_record_id=?1 AND receipt_time=?2 AND source='auto'",
                        params![parent_id, draw.receipt_time, now],
                    ).map_err(|e| format!("纠正同秒翻牌的奖励归属失败: {e}"))?;
                }
                changed += 1;
            }
            Some((
                source,
                saved_rewards_json,
                saved_reward_sources_json,
                saved_stamina_costs_json,
                saved_stamina_sources_json,
            )) if source == "auto" => {
                let saved_rewards: Vec<TrialDrawReward> =
                    serde_json::from_str(&saved_rewards_json).unwrap_or_default();
                let saved_reward_sources: Vec<String> =
                    serde_json::from_str(&saved_reward_sources_json).unwrap_or_default();
                let saved_stamina_costs: Vec<i64> =
                    serde_json::from_str(&saved_stamina_costs_json).unwrap_or_default();
                let saved_stamina_sources: Vec<String> =
                    serde_json::from_str(&saved_stamina_sources_json).unwrap_or_default();
                let (merged_rewards, merged_reward_sources) = merge_draw_facts(
                    saved_rewards,
                    saved_reward_sources,
                    &draw.reward_items,
                    &draw.reward_source_hashes,
                    |item| format!("reward:{}:{}", item.name, item.quantity),
                );
                let (merged_costs, merged_cost_sources) = merge_draw_facts(
                    saved_stamina_costs,
                    saved_stamina_sources,
                    &draw.stamina_costs,
                    &draw.stamina_source_hashes,
                    |cost| format!("stamina:{cost}"),
                );
                let merged_rewards =
                    serde_json::to_string(&merged_rewards).map_err(|e| e.to_string())?;
                let merged_reward_sources =
                    serde_json::to_string(&merged_reward_sources).map_err(|e| e.to_string())?;
                let merged_costs_json =
                    serde_json::to_string(&merged_costs).map_err(|e| e.to_string())?;
                let merged_cost_sources =
                    serde_json::to_string(&merged_cost_sources).map_err(|e| e.to_string())?;
                let merged_stamina = if merged_costs.is_empty() {
                    None
                } else {
                    Some(merged_costs.iter().sum::<i64>())
                };
                if merged_rewards != saved_rewards_json
                    || merged_costs_json != saved_stamina_costs_json
                {
                    tx.execute(
                        "UPDATE trial_draws
                         SET reward_items = ?1, reward_sources = ?2,
                             stamina_cost = ?3, stamina_costs = ?4, stamina_sources = ?5,
                             updated_at = ?6
                         WHERE source_key = ?7 AND source = 'auto'",
                        params![
                            merged_rewards,
                            merged_reward_sources,
                            merged_stamina,
                            merged_costs_json,
                            merged_cost_sources,
                            now,
                            draw.source_key,
                        ],
                    )
                    .map_err(|e| format!("补全试炼翻牌明细失败: {e}"))?;
                    changed += 1;
                }
            }
            Some(_) => {
                // `source = manual` is an explicit user decision. Leave every
                // field, including receipt facts, intact on future scans.
            }
        }
    }

    // The log may arrive in chunks. Re-number only automatic receipts by their
    // immutable receipt order. A manual multi-flip has no receipt timestamp
    // and keeps the user's stable sequence, even after its parent is claimed
    // by terminal evidence.
    let ordered_ids: Vec<String> = {
        let mut stmt = tx
            .prepare(
                "SELECT id FROM trial_draws
                 WHERE trial_record_id = ?1 AND source = 'auto'
                 ORDER BY receipt_time ASC, source_key ASC",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![parent_id], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?
    };
    for (index, id) in ordered_ids.iter().enumerate() {
        tx.execute(
            "UPDATE trial_draws SET sequence = ?1 WHERE id = ?2 AND sequence != ?1",
            params![(index + 1) as i64, id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(changed)
}

fn invalidate_auto_trial_draw_observations_for_receipt(
    conn: &Connection,
    parent_id: &str,
    receipt_time: i64,
    exclude_source_key: Option<&str>,
) -> Result<usize, String> {
    conn.execute(
        "UPDATE trial_draws
         SET flipped_index = CASE WHEN position_manual = 0 THEN 0 ELSE flipped_index END,
             card_1 = CASE WHEN equipment_unreviewed = 0 OR EXISTS (
                 SELECT 1 FROM trial_draw_equipment_manual_overrides m
                 WHERE m.draw_id = trial_draws.id AND m.card_index = 1
             ) THEN card_1 ELSE '' END,
             card_2 = CASE WHEN equipment_unreviewed = 0 OR EXISTS (
                 SELECT 1 FROM trial_draw_equipment_manual_overrides m
                 WHERE m.draw_id = trial_draws.id AND m.card_index = 2
             ) THEN card_2 ELSE '' END,
             card_3 = CASE WHEN equipment_unreviewed = 0 OR EXISTS (
                 SELECT 1 FROM trial_draw_equipment_manual_overrides m
                 WHERE m.draw_id = trial_draws.id AND m.card_index = 3
             ) THEN card_3 ELSE '' END,
             card_4 = CASE WHEN equipment_unreviewed = 0 OR EXISTS (
                 SELECT 1 FROM trial_draw_equipment_manual_overrides m
                 WHERE m.draw_id = trial_draws.id AND m.card_index = 4
             ) THEN card_4 ELSE '' END,
             card_5 = CASE WHEN equipment_unreviewed = 0 OR EXISTS (
                 SELECT 1 FROM trial_draw_equipment_manual_overrides m
                 WHERE m.draw_id = trial_draws.id AND m.card_index = 5
             ) THEN card_5 ELSE '' END,
             equipment_unreviewed = CASE WHEN equipment_unreviewed = 0 THEN 0 ELSE 1 END,
             auto_observed_start_ms = NULL, auto_observed_end_ms = NULL,
             auto_equipment_observed_start_ms = NULL, auto_equipment_observed_end_ms = NULL,
             updated_at = ?1
         WHERE trial_record_id = ?2 AND source = 'auto'
           AND ((auto_observed_start_ms IS NOT NULL AND auto_observed_end_ms IS NOT NULL
                 AND auto_observed_start_ms <= ?3 AND auto_observed_end_ms >= ?3)
             OR (auto_equipment_observed_start_ms IS NOT NULL AND auto_equipment_observed_end_ms IS NOT NULL
                 AND auto_equipment_observed_start_ms <= ?3 AND auto_equipment_observed_end_ms >= ?3))
           AND (?4 IS NULL OR source_key != ?4)",
        params![
            get_local_timestamp(),
            parent_id,
            receipt_time,
            exclude_source_key
        ],
    )
    .map_err(|e| format!("撤销可能错归的试炼自动识别失败: {e}"))
}

fn has_conflicting_trial_draw_receipt(
    conn: &Connection,
    parent_id: &str,
    draw_source_key: &str,
    observed_start_ms: i64,
    observed_end_ms: i64,
) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) > 0 FROM trial_draws
         WHERE trial_record_id = ?1 AND source_key != ?2
           AND receipt_time BETWEEN ?3 AND ?4",
        params![parent_id, draw_source_key, observed_start_ms, observed_end_ms],
        |row| row.get(0),
    )
    .map_err(|e| format!("检查试炼翻牌观察窗冲突失败: {e}"))
}

/// Result of applying a card-position recognition result to a trial record.
///
/// Screen recognition is intentionally allowed to fill only an unknown card
/// position. It never creates a trial record or replaces a user-entered value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialAutoFlipUpdateOutcome {
    /// An unknown (`NULL` or `0`) position was filled.
    Updated,
    /// The matched record already had a non-zero position.
    Preserved,
    /// No record has been created for this completion evidence yet.
    Missing,
    /// The user deleted this completion evidence, so it must not be revived.
    Deleted,
}

/// Applies screenshot-recognized position to one stable draw receipt.
/// Unlike the legacy parent-level helper, this never conflates consecutive
/// flips in the same completion.
pub fn db_apply_auto_trial_draw_flip(
    draw_source_key: &str,
    flipped_index: i64,
    observed_start_ms: i64,
    observed_end_ms: i64,
) -> Result<TrialAutoFlipUpdateOutcome, String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    db_apply_auto_trial_draw_flip_with_connection(
        &mut conn,
        draw_source_key,
        flipped_index,
        observed_start_ms,
        observed_end_ms,
    )
}

pub fn db_apply_auto_trial_draw_flip_with_connection(
    conn: &mut Connection,
    draw_source_key: &str,
    flipped_index: i64,
    observed_start_ms: i64,
    observed_end_ms: i64,
) -> Result<TrialAutoFlipUpdateOutcome, String> {
    if draw_source_key.trim().is_empty() {
        return Err("试炼自动补牌缺少翻牌来源键".to_string());
    }
    if !(1..=5).contains(&flipped_index) {
        return Err("试炼自动补牌位必须在 1 到 5 之间".to_string());
    }
    if observed_start_ms <= 0 || observed_end_ms < observed_start_ms {
        return Err("试炼自动补牌缺少有效观察时间窗".to_string());
    }
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let deleted: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
            params![draw_source_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if deleted {
        return Ok(TrialAutoFlipUpdateOutcome::Deleted);
    }
    let existing: Option<(String, i64, i64, String, i64)> = tx
        .query_row(
            "SELECT source, flipped_index, position_manual, trial_record_id, receipt_time
             FROM trial_draws WHERE source_key = ?1",
            params![draw_source_key],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((source, saved_index, position_manual, record_id, receipt_time)) = existing else {
        return Ok(TrialAutoFlipUpdateOutcome::Missing);
    };
    if source != "auto" || position_manual != 0 || saved_index != 0 {
        return Ok(TrialAutoFlipUpdateOutcome::Preserved);
    }
    if has_conflicting_trial_draw_receipt(
        &tx,
        &record_id,
        draw_source_key,
        observed_start_ms,
        observed_end_ms,
    )? {
        return Ok(TrialAutoFlipUpdateOutcome::Preserved);
    }
    invalidate_auto_trial_draw_observations_for_receipt(
        &tx,
        &record_id,
        receipt_time,
        Some(draw_source_key),
    )?;
    let changed = tx
        .execute(
            "UPDATE trial_draws
             SET flipped_index = ?1, auto_observed_start_ms = ?2, auto_observed_end_ms = ?3, updated_at = ?4
             WHERE source_key = ?5 AND source = 'auto' AND position_manual = 0 AND flipped_index = 0",
            params![flipped_index, observed_start_ms, observed_end_ms, get_local_timestamp(), draw_source_key],
        )
        .map_err(|e| format!("自动补充试炼翻牌位失败: {e}"))?;
    if changed != 1 {
        return Err("试炼翻牌明细在自动补牌时发生变化，请重试".to_string());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(TrialAutoFlipUpdateOutcome::Updated)
}

/// Applies a recognized card position to exactly one known trial completion.
///
/// The caller must provide the stable `source_key` from the scanner. This helper
/// deliberately does not create rows: recognition can arrive before the scanner
/// has persisted the trial completion, and must then be retried by the caller.
/// Pending records are eligible, but their status is not changed by card
/// recognition.
#[cfg(test)]
pub fn db_apply_auto_trial_flip_with_connection(
    conn: &mut Connection,
    source_key: &str,
    flipped_index: i64,
) -> Result<TrialAutoFlipUpdateOutcome, String> {
    if source_key.trim().is_empty() {
        return Err("试炼自动补牌缺少 source_key".to_string());
    }
    if !(1..=5).contains(&flipped_index) {
        return Err("试炼自动补牌位必须在 1 到 5 之间".to_string());
    }

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let deleted: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
            params![source_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if deleted {
        return Ok(TrialAutoFlipUpdateOutcome::Deleted);
    }

    let existing_flip: Option<Option<i64>> = tx
        .query_row(
            "SELECT flipped_index FROM trial_records WHERE source_key = ?1",
            params![source_key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some(existing_flip) = existing_flip else {
        return Ok(TrialAutoFlipUpdateOutcome::Missing);
    };
    if matches!(existing_flip, Some(value) if value != 0) {
        return Ok(TrialAutoFlipUpdateOutcome::Preserved);
    }

    let changed = tx
        .execute(
            "UPDATE trial_records
             SET flipped_index = ?1, updated_at = ?2
             WHERE source_key = ?3 AND (flipped_index IS NULL OR flipped_index = 0)",
            params![flipped_index, get_local_timestamp(), source_key],
        )
        .map_err(|e| format!("自动补充试炼翻牌位失败: {}", e))?;
    if changed != 1 {
        return Err("试炼记录在自动补牌时发生变化，请重试".to_string());
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(TrialAutoFlipUpdateOutcome::Updated)
}

/// Result of storing an OCR-confirmed equipment card for an automatic trial.
///
/// A card tooltip can be observed independently from the selected card. The
/// result therefore only identifies an equipment appearance at a position. It
/// deliberately does not claim that the equipment review is complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialAutoEquipmentUpdateOutcome {
    /// The previously empty card slot was filled.
    Updated,
    /// A value already exists, or the record was manually reviewed.
    Preserved,
    /// The completion record is not available yet.
    Missing,
    /// The user deleted this completion record.
    Deleted,
}

/// Stores one catalog equipment id for one stable draw receipt. The selected
/// slot is checked again at write time, preventing an OCR tooltip from a later
/// flip being attached to an earlier card.
pub fn db_apply_auto_trial_draw_equipment_with_connection(
    conn: &mut Connection,
    draw_source_key: &str,
    expected_flipped_index: i64,
    card_index: i64,
    equipment_id: &str,
    observed_start_ms: i64,
    observed_end_ms: i64,
) -> Result<TrialAutoEquipmentUpdateOutcome, String> {
    if draw_source_key.trim().is_empty() {
        return Err("试炼自动补装备缺少翻牌来源键".to_string());
    }
    if !(1..=5).contains(&expected_flipped_index) || !(1..=5).contains(&card_index) {
        return Err("试炼自动补装备牌位必须在 1 到 5 之间".to_string());
    }
    let equipment_id = equipment_id.trim();
    if equipment_id.is_empty() {
        return Err("试炼自动补装备缺少装备编号".to_string());
    }
    if observed_start_ms <= 0 || observed_end_ms < observed_start_ms {
        return Err("试炼自动补装备缺少有效观察时间窗".to_string());
    }
    let card_column = match card_index {
        1 => "card_1",
        2 => "card_2",
        3 => "card_3",
        4 => "card_4",
        5 => "card_5",
        _ => unreachable!("card index was validated above"),
    };
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let deleted: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
            params![draw_source_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if deleted {
        return Ok(TrialAutoEquipmentUpdateOutcome::Deleted);
    }
    let existing: Option<(String, i64, i64, String, String, bool)> = tx
        .query_row(
            &format!(
                "SELECT d.source, d.equipment_unreviewed, d.flipped_index, d.{card_column}, d.trial_record_id,
                        EXISTS(
                            SELECT 1 FROM trial_draw_equipment_manual_overrides m
                            WHERE m.draw_id = d.id AND m.card_index = ?2
                        )
                 FROM trial_draws d WHERE d.source_key = ?1"
            ),
            params![draw_source_key, card_index],
            |row| Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            )),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((source, equipment_unreviewed, saved_index, saved_equipment, record_id, manually_reviewed)) = existing else {
        return Ok(TrialAutoEquipmentUpdateOutcome::Missing);
    };
    if source != "auto"
        || equipment_unreviewed == 0
        || saved_index != expected_flipped_index
        || manually_reviewed
        || !saved_equipment.trim().is_empty()
    {
        return Ok(TrialAutoEquipmentUpdateOutcome::Preserved);
    }
    if has_conflicting_trial_draw_receipt(
        &tx,
        &record_id,
        draw_source_key,
        observed_start_ms,
        observed_end_ms,
    )? {
        return Ok(TrialAutoEquipmentUpdateOutcome::Preserved);
    }
    let changed = tx
        .execute(
            &format!(
                "UPDATE trial_draws
                 SET {card_column} = ?1,
                     auto_equipment_observed_start_ms = ?2, auto_equipment_observed_end_ms = ?3,
                     updated_at = ?4
                 WHERE source_key = ?5 AND source = 'auto'
                   AND equipment_unreviewed = 1 AND flipped_index = ?6
                   AND COALESCE(TRIM({card_column}), '') = ''
                   AND NOT EXISTS (
                       SELECT 1 FROM trial_draw_equipment_manual_overrides m
                       WHERE m.draw_id = trial_draws.id AND m.card_index = {card_index}
                   )"
            ),
            params![
                equipment_id,
                observed_start_ms,
                observed_end_ms,
                get_local_timestamp(),
                draw_source_key,
                expected_flipped_index
            ],
        )
        .map_err(|e| format!("自动补充试炼翻牌装备失败: {e}"))?;
    if changed != 1 {
        return Err("试炼翻牌明细在自动补装备时发生变化，请重试".to_string());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(TrialAutoEquipmentUpdateOutcome::Updated)
}

/// Stores one canonical equipment id in an empty automatic trial card slot.
///
/// The caller must have already resolved the OCR text to exactly one known
/// trial equipment id and supplied the selected card observed in the same
/// frame. This helper never creates a record, never changes the selected card,
/// and never clears equipment_unreviewed. Seeing one tooltip is not proof that
/// all five cards were inspected. Existing card values, manual records, and
/// user-reviewed automatic records are preserved.
#[cfg(test)]
pub fn db_apply_auto_trial_equipment_with_connection(
    conn: &mut Connection,
    source_key: &str,
    expected_flipped_index: i64,
    card_index: i64,
    equipment_id: &str,
) -> Result<TrialAutoEquipmentUpdateOutcome, String> {
    if source_key.trim().is_empty() {
        return Err("试炼自动补装备缺少 source_key".to_string());
    }
    if !(1..=5).contains(&card_index) {
        return Err("试炼自动补装备牌位必须在 1 到 5 之间".to_string());
    }
    if !(1..=5).contains(&expected_flipped_index) {
        return Err("试炼自动补装备缺少已确认翻牌位".to_string());
    }
    let equipment_id = equipment_id.trim();
    if equipment_id.is_empty() {
        return Err("试炼自动补装备缺少装备编号".to_string());
    }

    let card_column = match card_index {
        1 => "card_1",
        2 => "card_2",
        3 => "card_3",
        4 => "card_4",
        5 => "card_5",
        _ => unreachable!("card index was validated above"),
    };

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let deleted: bool = tx
        .query_row(
            "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = ?1",
            params![source_key],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    if deleted {
        return Ok(TrialAutoEquipmentUpdateOutcome::Deleted);
    }

    let existing: Option<(String, i64, Option<i64>, String)> = tx
        .query_row(
            &format!(
                "SELECT source, equipment_unreviewed, flipped_index, {card_column}
                 FROM trial_records WHERE source_key = ?1"
            ),
            params![source_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let Some((source, equipment_unreviewed, saved_flipped_index, existing_equipment)) = existing else {
        return Ok(TrialAutoEquipmentUpdateOutcome::Missing);
    };

    // A manual save or a completed review owns all of its card fields. This
    // also prevents a delayed screenshot sample from overwriting an explicit
    // no-equipment review.
    if source != "auto"
        || equipment_unreviewed == 0
        || saved_flipped_index != Some(expected_flipped_index)
        || !existing_equipment.trim().is_empty()
    {
        return Ok(TrialAutoEquipmentUpdateOutcome::Preserved);
    }

    let changed = tx
        .execute(
            &format!(
                "UPDATE trial_records
                 SET {card_column} = ?1, updated_at = ?2
                 WHERE source_key = ?3
                   AND source = 'auto'
                   AND equipment_unreviewed = 1
                   AND flipped_index = ?4
                   AND COALESCE(TRIM({card_column}), '') = ''"
            ),
            params![
                equipment_id,
                get_local_timestamp(),
                source_key,
                expected_flipped_index,
            ],
        )
        .map_err(|e| format!("自动补充试炼装备失败: {e}"))?;
    if changed != 1 {
        return Err("试炼记录在自动补装备时发生变化，请重试".to_string());
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(TrialAutoEquipmentUpdateOutcome::Updated)
}

#[tauri::command]
pub fn db_get_trial_records() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "
        SELECT id, account_id, role_id, role_name, server, layer, bosses,
               card_1, card_2, card_3, card_4, card_5, flipped_index, record_type,
               source, status, source_key, equipment_unreviewed, date, notes, updated_at
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
                preserve_flipped_index: false,
                edited_card_indices: Vec::new(),
                record_type: row.get(13)?,
                source: row.get(14)?,
                status: row.get(15)?,
                source_key: row.get(16)?,
                equipment_unreviewed: Some(row.get::<_, i64>(17)? != 0),
                draws: Vec::new(),
                date: row.get(18)?,
                notes: row.get(19)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok(item) = row {
            result.push(item);
        }
    }

    // Fetch all child rows at once. A record list must not turn into one SQL
    // query per draw/run when a role has accumulated weeks of trial history.
    let mut draws_by_record: HashMap<String, Vec<TrialDraw>> = HashMap::new();
    let mut draw_stmt = conn
        .prepare(
            "SELECT id, trial_record_id, source_key, sequence, receipt_time, receipt_hash,
                    reward_items, stamina_cost, stamina_costs, flipped_index,
                    card_1, card_2, card_3, card_4, card_5, equipment_unreviewed, source
             FROM trial_draws
             ORDER BY trial_record_id ASC, sequence ASC, receipt_time ASC, source_key ASC",
        )
        .map_err(|e| e.to_string())?;
    let draw_rows = draw_stmt
        .query_map([], |row| {
            let reward_items_json: String = row.get(6)?;
            let stamina_costs_json: String = row.get(8)?;
            Ok((
                row.get::<_, String>(1)?,
                TrialDraw {
                    id: row.get(0)?,
                    source_key: row.get(2)?,
                    sequence: row.get(3)?,
                    sequence_ambiguous: false,
                    receipt_time: row.get(4)?,
                    receipt_hash: row.get(5)?,
                    reward_items: serde_json::from_str(&reward_items_json).unwrap_or_default(),
                    stamina_cost: row.get(7)?,
                    stamina_costs: serde_json::from_str(&stamina_costs_json).unwrap_or_default(),
                    flipped_index: row.get(9)?,
                    card_1: row.get(10)?,
                    card_2: row.get(11)?,
                    card_3: row.get(12)?,
                    card_4: row.get(13)?,
                    card_5: row.get(14)?,
                    equipment_unreviewed: row.get::<_, i64>(15)? != 0,
                manual_equipment_card_indexes: Vec::new(),
                edited_card_indices: Vec::new(),
                    source: row.get(16)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in draw_rows {
        let (record_id, draw) = row.map_err(|e| e.to_string())?;
        draws_by_record.entry(record_id).or_default().push(draw);
    }

    // The presence of a manual override is meaningful even when its equipment
    // id is NULL: the user may have deliberately cleared that card. Read the
    // sparse evidence in a deterministic order instead of encoding it in the
    // card text or treating all five positions as reviewed.
    let mut manual_cards_by_draw: HashMap<String, Vec<i64>> = HashMap::new();
    let mut manual_cards_stmt = conn
        .prepare(
            "SELECT draw_id, card_index
             FROM trial_draw_equipment_manual_overrides
             ORDER BY draw_id ASC, card_index ASC",
        )
        .map_err(|e| e.to_string())?;
    let manual_card_rows = manual_cards_stmt
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in manual_card_rows {
        let (draw_id, card_index) = row.map_err(|e| e.to_string())?;
        manual_cards_by_draw
            .entry(draw_id)
            .or_default()
            .push(card_index);
    }
    for record in &mut result {
        record.draws = draws_by_record.remove(&record.id).unwrap_or_default();
        for draw in &mut record.draws {
            draw.manual_equipment_card_indexes =
                manual_cards_by_draw.remove(&draw.id).unwrap_or_default();
        }
        let mut receipt_counts: HashMap<i64, usize> = HashMap::new();
        for draw in &record.draws {
            // Manual multi-flip entries intentionally use receipt_time=0 to
            // express an unknown game time. They are ordered by `sequence`,
            // not a same-second receipt group.
            if draw.source == "auto" && draw.receipt_time > 0 {
                *receipt_counts.entry(draw.receipt_time).or_default() += 1;
            }
        }
        for draw in &mut record.draws {
            draw.sequence_ambiguous = draw.source == "auto"
                && draw.receipt_time > 0
                && receipt_counts.get(&draw.receipt_time).copied().unwrap_or(0) > 1;
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
}

/// Manually sets or clears one draw's selected card. This intentionally only
/// changes the child row; legacy parent card fields belong to older single-draw
/// records and are not safe to reinterpret.
#[tauri::command]
pub fn db_update_trial_draw_flipped_index(
    record_id: String,
    draw_id: String,
    flipped_index: Option<i64>,
) -> Result<(), String> {
    let index = flipped_index.unwrap_or(0);
    if !(0..=5).contains(&index) {
        return Err("试炼翻牌位必须在 1 到 5 之间，或清空".to_string());
    }
    let conn = init_db().map_err(|e| e.to_string())?;
    let changed = conn
        .execute(
            "UPDATE trial_draws
             SET flipped_index = ?1, position_manual = 1, updated_at = ?2
             WHERE id = ?3 AND trial_record_id = ?4",
            params![index, get_local_timestamp(), draw_id, record_id],
        )
        .map_err(|e| format!("手工更新试炼翻牌位失败: {e}"))?;
    if changed != 1 {
        return Err("未找到对应的试炼翻牌明细，请刷新后重试".to_string());
    }
    Ok(())
}

/// Manually sets or clears exactly one equipment card on one child draw.
/// An explicit clear is kept as a NULL override row, so late OCR cannot treat
/// that card as an unknown automatic slot.
#[tauri::command]
pub fn db_update_trial_draw_equipment(
    draw_id: String,
    card_index: i64,
    equipment_id: Option<String>,
) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    db_update_trial_draw_equipment_with_connection(
        &mut conn,
        &draw_id,
        card_index,
        equipment_id.as_deref(),
    )
}

/// Explicitly marks one draw's complete five-card board as reviewed, or
/// returns that exact draw to the pending-review state. Confirmation uses the
/// currently rendered board as a compare-and-set snapshot, preventing a late
/// OCR result from being incorrectly confirmed as something the user saw.
/// The review marker is deliberately independent from per-card overrides:
/// confirming preserves the currently saved cards (including blanks), while
/// revoking never deletes any manual card decisions.
#[tauri::command]
pub fn db_set_trial_draw_equipment_reviewed(
    draw_id: String,
    reviewed: bool,
    expected_flipped_index: Option<i64>,
    expected_cards: Option<Vec<String>>,
) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    db_set_trial_draw_equipment_reviewed_with_connection(
        &mut conn,
        &draw_id,
        reviewed,
        expected_flipped_index,
        expected_cards.as_deref(),
    )
}

fn db_set_trial_draw_equipment_reviewed_with_connection(
    conn: &mut Connection,
    draw_id: &str,
    reviewed: bool,
    expected_flipped_index: Option<i64>,
    expected_cards: Option<&[String]>,
) -> Result<(), String> {
    if draw_id.trim().is_empty() {
        return Err("试炼翻牌明细不能为空".to_string());
    }
    if reviewed {
        if expected_flipped_index.is_none() {
            return Err("确认完整牌面时必须包含当前翻牌位".to_string());
        }
        if expected_cards.map_or(true, |cards| cards.len() != 5) {
            return Err("确认完整牌面时必须包含 5 张牌".to_string());
        }
    }

    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let current: Option<(i64, String, String, String, String, String)> = tx
        .query_row(
            "SELECT flipped_index, card_1, card_2, card_3, card_4, card_5
             FROM trial_draws WHERE id = ?1",
            params![draw_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("读取试炼翻牌明细失败: {e}"))?;
    let Some((flipped_index, card_1, card_2, card_3, card_4, card_5)) = current else {
        return Err("未找到对应的试炼翻牌明细，请刷新后重试".to_string());
    };
    if reviewed {
        let current_cards = [card_1, card_2, card_3, card_4, card_5];
        if expected_flipped_index != Some(flipped_index)
            || expected_cards != Some(current_cards.as_slice())
        {
            return Err("牌面已变化，请刷新后重新核对".to_string());
        }
    }
    let changed = tx
        .execute(
            "UPDATE trial_draws
             SET equipment_unreviewed = ?1, updated_at = ?2
             WHERE id = ?3",
            params![if reviewed { 0 } else { 1 }, get_local_timestamp(), draw_id],
        )
        .map_err(|e| format!("更新试炼翻牌装备核对状态失败: {e}"))?;
    if changed != 1 {
        return Err("未找到对应的试炼翻牌明细，请刷新后重试".to_string());
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn db_update_trial_draw_equipment_with_connection(
    conn: &mut Connection,
    draw_id: &str,
    card_index: i64,
    equipment_id: Option<&str>,
) -> Result<(), String> {
    if draw_id.trim().is_empty() {
        return Err("试炼翻牌明细不能为空".to_string());
    }
    if !(1..=5).contains(&card_index) {
        return Err("试炼装备牌位必须在 1 到 5 之间".to_string());
    }
    let equipment_id = equipment_id
        .map(str::trim)
        .filter(|equipment_id| !equipment_id.is_empty())
        .map(str::to_owned);
    let card_column = match card_index {
        1 => "card_1",
        2 => "card_2",
        3 => "card_3",
        4 => "card_4",
        5 => "card_5",
        _ => unreachable!("card index was validated above"),
    };
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let draw_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM trial_draws WHERE id = ?1)",
            params![draw_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查试炼翻牌明细失败: {e}"))?;
    if !draw_exists {
        return Err("未找到对应的试炼翻牌明细，请刷新后重试".to_string());
    }

    let now = get_local_timestamp();
    tx.execute(
        &format!("UPDATE trial_draws SET {card_column} = ?1, updated_at = ?2 WHERE id = ?3"),
        params![equipment_id.as_deref().unwrap_or(""), now, draw_id],
    )
    .map_err(|e| format!("手工更新试炼翻牌装备失败: {e}"))?;
    tx.execute(
        "INSERT INTO trial_draw_equipment_manual_overrides
             (draw_id, card_index, equipment_id, source, updated_at)
         VALUES (?1, ?2, ?3, 'manual', ?4)
         ON CONFLICT(draw_id, card_index) DO UPDATE SET
             equipment_id = excluded.equipment_id,
             source = excluded.source,
             updated_at = excluded.updated_at",
        params![draw_id, card_index, equipment_id, now],
    )
    .map_err(|e| format!("保存试炼翻牌装备手工覆盖失败: {e}"))?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_delete_trial_record(id: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    delete_trial_record_with_connection(&mut conn, &id)
}

fn delete_trial_record_with_connection(conn: &mut Connection, id: &str) -> Result<(), String> {
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let auto_source_key: Option<String> = tx
        .query_row(
            "SELECT source_key FROM trial_records WHERE id = ?1 AND source_key IS NOT NULL",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    // Keep child keys as tombstones too: an already queued screenshot result
    // must not be mistaken for an unknown row after its parent was deleted.
    tx.execute(
        "INSERT OR IGNORE INTO trial_auto_scan_tombstones (source_key, deleted_at)
         SELECT source_key, ?1 FROM trial_draws WHERE trial_record_id = ?2",
        params![get_local_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    // Keep this explicit in addition to the FK declaration: historical SQLite
    // connections were sometimes opened without foreign-key enforcement.
    tx.execute(
        "DELETE FROM trial_draw_equipment_manual_overrides
         WHERE draw_id IN (SELECT id FROM trial_draws WHERE trial_record_id = ?1)",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    // Do this explicitly in addition to the FK declaration. Some historical
    // SQLite connections were opened without foreign_keys enforcement.
    tx.execute(
        "DELETE FROM trial_draws WHERE trial_record_id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM trial_records WHERE id = ?", params![id])
        .map_err(|e| e.to_string())?;
    if let Some(source_key) = auto_source_key {
        tx.execute(
            "INSERT OR IGNORE INTO trial_auto_scan_tombstones (source_key, deleted_at) VALUES (?1, ?2)",
            params![source_key, get_local_timestamp()],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
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
pub fn db_save_accounts(accounts: String, allow_delete: Option<bool>) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    save_accounts_with_conn(&mut conn, &accounts, allow_delete)
}

/// `db_save_accounts` 的实际实现（连接可注入，便于单元测试）。
///
/// 删除护栏：默认「只增改不删」。
/// 前端 App 的自动保存会把整个 accounts 数组全量回写，而读接口在失败时返回 []，
/// 若不设护栏，一次读取失败就会把用户全部账号/角色删掉。
/// 显式删除必须走 db_delete_account_structured / db_delete_role_structured。
pub(crate) fn save_accounts_with_conn(
    conn: &mut Connection,
    accounts: &str,
    allow_delete: Option<bool>,
) -> Result<(), String> {
    let parsed: Vec<serde_json::Value> =
        serde_json::from_str(accounts).map_err(|e| e.to_string())?;

    let allow_delete = allow_delete.unwrap_or(false);

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
    //    仅在显式 allow_delete 时执行，见函数开头的护栏说明。
    if allow_delete {
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
            "accountId": role.account_id,
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

/// 显式删除单个副本（按 getRaidKey 拼出的 id）。
///
/// 为什么需要独立命令：`db_save_raids` 默认 allow_delete=false（只增改不删），
/// 用户主动删除副本时必须走本命令，否则界面删了、库里还在。
/// 删除 raids 行会级联清理 raid_bosses。
#[tauri::command]
pub fn db_delete_raid_structured(raid_id: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let is_static: bool = tx
        .query_row(
            "SELECT COALESCE(is_static, 0) FROM raids WHERE id = ?",
            params![&raid_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|v| v != 0)
        .unwrap_or(false);

    if is_static {
        return Err("预制副本不能删除，只能禁用".to_string());
    }

    tx.execute("DELETE FROM raid_bosses WHERE raid_name = (SELECT name FROM raids WHERE id = ?1)", params![&raid_id])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM raids WHERE id = ?", params![&raid_id])
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    log::info!("已删除副本: {}", raid_id);
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
pub fn db_get_pending_records() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    // 返回 pending（可确认）和 scanning（副本进行中，UI 锁定不可确认）记录
    // 排除已 confirmed / rejected 的记录
    let mut stmt = conn
        .prepare("SELECT data FROM records WHERE status IN ('pending', 'scanning') ORDER BY record_date DESC")
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
                metadata.record_type,
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
        .prepare("SELECT id, name, difficulty, player_count, version, notes, is_active, is_static, season_id FROM raids")
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
        let season_id: Option<i64> = row.get(8).map_err(|e| e.to_string())?;

        let mut raid = serde_json::json!({
            "name": name,
            "difficulty": difficulty,
            "playerCount": player_count,
            "isActive": is_active == 1,
            "static": is_static == 1
        });

        // season_id 用于前端判断副本所属赛季（周常区域据此区分本赛季与历史赛季）。
        // 历史数据可能为空，前端需按「未知赛季」处理，不能当作本赛季。
        if let Some(sid) = season_id {
            raid["seasonId"] = serde_json::json!(sid);
        }

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
            .prepare("SELECT raid_name, boss_id, name, boss_order FROM raid_bosses ORDER BY boss_order")
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

    // raid_bosses 表 raid_name 列存的是 raids.name（副本基础名），
    // 同名副本天然共享同一组 BOSS 记录。
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
        .prepare("SELECT name FROM game_versions ORDER BY sort_order DESC")
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

/// 查询当前生效赛季的 ID（start_date <= 当前时间 < end_date）。
///
/// 供 drop_items 赛季缓存失效检查使用：装备价格随游戏版本更新变化，
/// 缓存行的 season_id 与当前赛季不一致时需重新拉取。
/// 未配置任何赛季（或时间未覆盖当前日期）时返回 None。
pub fn query_current_season_id(conn: &Connection) -> Option<i64> {
    let now = chrono::Utc::now().timestamp();
    conn.query_row(
        "SELECT id FROM seasons WHERE start_date <= ?1 AND (end_date IS NULL OR end_date = 0 OR end_date > ?2) ORDER BY sort_order DESC LIMIT 1",
        params![now, now],
        |row| row.get(0),
    )
    .ok()
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
pub fn db_save_raids(raids: String, allow_delete: Option<bool>) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    save_raids_with_conn(&mut conn, &raids, allow_delete)
}

/// `db_save_raids` 的实际实现（连接可注入，便于单元测试）。
///
/// 删除护栏：默认「只增改不删」，与 db_save_accounts 一致。
/// 副本读取失败时前端会拿到空数组并全量回写，若无护栏会连带级联删掉 raid_bosses。
pub(crate) fn save_raids_with_conn(
    conn: &mut Connection,
    raids: &str,
    allow_delete: Option<bool>,
) -> Result<(), String> {
    let parsed: Vec<serde_json::Value> = serde_json::from_str(raids).map_err(|e| e.to_string())?;

    let allow_delete = allow_delete.unwrap_or(false);

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
    //    仅在显式 allow_delete 时执行，见函数开头的护栏说明。
    if allow_delete {
        for raid_id in &existing_raid_ids {
            if !incoming_raid_ids.contains(raid_id) {
                tx.execute("DELETE FROM raids WHERE id = ?", params![raid_id])
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    // 4. 插入或更新 raids
    // raid_bosses 按 (raid_name, boss_id) 联合主键去重，raid_name 列存 raids.name，
    // 同名副本（不同难度/人数）天然共享同一组 BOSS 记录。
    let mut boss_saved_keys: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
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

        // 处理 raid_bosses（raid_name 列存 raids.name，同名副本共享 BOSS）
        if let Some(bosses) = raid["bosses"].as_array() {
            for boss in bosses {
                let boss_id = boss["id"].as_str().unwrap_or_default();
                let boss_name = boss["name"].as_str().unwrap_or_default();
                let boss_order = boss["order"].as_i64().unwrap_or(0);
                let key = (name.to_string(), boss_id.to_string());
                if !boss_saved_keys.contains(&key) {
                    boss_saved_keys.insert(key);
                    tx.execute(
                        "INSERT INTO raid_bosses (raid_name, boss_id, name, boss_order)
                         VALUES (?, ?, ?, ?)
                         ON CONFLICT(raid_name, boss_id) DO UPDATE SET
                            name = excluded.name,
                            boss_order = excluded.boss_order",
                        params![name, boss_id, boss_name, boss_order],
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    // 副本配置已变更，清空 drop_scanner 的副本配置缓存，
    // 避免扫描中使用旧配置直到应用重启
    crate::mingyi::drop_scanner::invalidate_raids_cache();

    Ok(())
}

#[tauri::command]
pub fn db_get_config() -> Result<Option<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let config: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'config_json'",
            [],
            |row| row.get(0),
        )
        .ok();
    // 空字符串视为无配置
    Ok(config.filter(|s| !s.is_empty()))
}

#[tauri::command]
pub fn db_get_config_debug() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    let mut result = String::new();
    result.push_str("=== 配置数据调试报告 ===\n\n");

    // 检查 app_config 表
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_config'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    result.push_str(&format!(
        "app_config 表存在: {}\n",
        if table_exists > 0 { "是" } else { "否" }
    ));

    // 检查 config_json 键
    let config_value: Option<String> = conn
        .query_row(
            "SELECT value FROM app_config WHERE key = 'config_json'",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(config_str) = &config_value {
        if config_str.is_empty() {
            result.push_str("config_json: 为空\n");
        } else {
            result.push_str(&format!("配置数据长度: {} bytes\n", config_str.len()));
            result.push_str(&format!(
                "配置数据预览: {}\n",
                &config_str[..std::cmp::min(config_str.len(), 200)]
            ));

            match serde_json::from_str::<serde_json::Value>(config_str) {
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
        }
    } else {
        result.push_str("config_json: 不存在\n");
    }

    // 显示所有 app_config 键值
    result.push_str("\n--- app_config 全部键值 ---\n");
    let mut stmt = conn
        .prepare("SELECT key, value, updated_at FROM app_config ORDER BY key")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (key, value, updated_at) = row.map_err(|e| e.to_string())?;
        let preview = if value.len() > 100 {
            format!("{}...", &value[..100])
        } else {
            value.clone()
        };
        result.push_str(&format!("  {} = {} ({})\n", key, preview, updated_at));
    }

    Ok(result)
}

#[tauri::command]
pub fn db_reset_config(default_config: String) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().to_rfc3339();

    // 重置 config_json
    conn.execute(
        "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'config_json'",
        params![default_config, &now],
    )
    .map_err(|e| e.to_string())?;

    // 同步 game_directory
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&default_config) {
        let game_dir = parsed
            .get("game")
            .and_then(|g| g.get("gameDirectory"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        conn.execute(
            "UPDATE app_config SET value = ?1, updated_at = ?2 WHERE key = 'game_directory'",
            params![game_dir, &now],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "✓ 配置已重置\n  新配置长度: {} bytes",
        default_config.len()
    ))
}

#[tauri::command]
pub fn db_save_config(config: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let now = chrono::Local::now().to_rfc3339();

    // 更新 config_json
    conn.execute(
        "INSERT INTO app_config (key, value, updated_at) VALUES ('config_json', ?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![config, &now],
    )
    .map_err(|e| e.to_string())?;

    // 注意：不再从 config_json 反向覆盖 game_directory
    // game_directory 的唯一存储源是 app_config.game_directory，由 set_game_directory 命令写入
    // 旧代码从 config_json 中提取 gameDirectory 并覆盖 game_directory 会导致升级后游戏目录丢失

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
            metadata.record_type,
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

    // 使用 game_versions.sort_order 排序（raid_versions 表已废弃）
    // sort_order 越大版本越新，rowid 越大副本越靠后（静态数据中）
    let mut stmt = conn
        .prepare(
            r#"
            SELECT DISTINCT f.raid_name, r.rowid
            FROM favorite_raids f
            LEFT JOIN raids r ON f.raid_name = r.name
            LEFT JOIN game_versions gv ON r.version = gv.name
            ORDER BY COALESCE(gv.sort_order, 0) DESC, r.rowid DESC
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

/// 获取默认数据目录路径（不写入配置，用于切换目录前的冲突检查）
/// 安装版返回 exe 同目录，非安装版返回 ~/.jx3-raid-manager
#[tauri::command]
pub fn db_get_default_data_dir() -> Result<String, String> {
    let install_mode = is_install_mode();
    let default_dir = if install_mode {
        get_install_dir().ok_or_else(|| "无法获取安装目录".to_string())?
    } else {
        get_home_app_dir()?
    };
    Ok(default_dir.to_string_lossy().to_string())
}

fn write_custom_data_dir_config(
    path: Option<&Path>,
    migration_source: Option<&Path>,
    force_overwrite: bool,
) -> Result<(), String> {
    let mut config = read_data_dir_bootstrap_config()?;
    config.custom_data_dir = path.map(|item| item.to_string_lossy().to_string());
    config.pending_migration_from = migration_source
        .filter(|item| !item.as_os_str().is_empty())
        .map(|item| item.to_string_lossy().to_string());
    config.force_overwrite_target = force_overwrite;
    write_data_dir_bootstrap_config(&config)
}

/// 检查目标目录是否已存在数据库文件（用于切换目录前的冲突提示）
#[tauri::command]
pub fn db_check_target_dir_has_db(path: String) -> Result<bool, String> {
    let dir_path = PathBuf::from(&path);
    if dir_path.as_os_str().is_empty() {
        return Ok(false);
    }
    if !dir_path.exists() || !dir_path.is_dir() {
        return Ok(false);
    }
    Ok(has_persisted_app_data(&dir_path))
}

/// 设置自定义数据目录（持久化保存，重启后生效）
#[tauri::command]
pub fn db_set_custom_data_dir(
    path: String,
    force_overwrite: Option<bool>,
) -> Result<String, String> {
    let dir_path = PathBuf::from(&path);

    if dir_path.as_os_str().is_empty() {
        return Err("目录不能为空".to_string());
    }

    ensure_directory_exists(&dir_path)?;
    // 使用不触发迁移的方式获取当前目录，避免在写入新配置前意外触发旧配置的迁移，
    // 导致 force_overwrite 标记被忽略（get_app_dir 内部会调用 maybe_migrate_app_data）。
    let current_dir = get_current_app_dir_no_migrate()?;

    let migration_source = if directories_match(&current_dir, &dir_path)? {
        None
    } else {
        Some(current_dir.as_path())
    };

    write_custom_data_dir_config(
        Some(dir_path.as_path()),
        migration_source,
        force_overwrite.unwrap_or(false),
    )?;
    log::info!(
        "自定义数据目录已保存，将在重启后启用: {:?}（force_overwrite={}）",
        dir_path,
        force_overwrite.unwrap_or(false)
    );
    Ok(dir_path.to_string_lossy().to_string())
}

/// 恢复默认数据目录（安装版为安装目录，非安装版为用户目录）
#[tauri::command]
pub fn db_reset_custom_data_dir(force_overwrite: Option<bool>) -> Result<String, String> {
    // 使用不触发迁移的方式获取当前目录，避免在写入新配置前意外触发旧配置的迁移，
    // 导致 force_overwrite 标记被忽略（get_app_dir 内部会调用 maybe_migrate_app_data）。
    let current_dir = get_current_app_dir_no_migrate()?;
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
    write_custom_data_dir_config(None, migration_source, force_overwrite.unwrap_or(false))?;
    log::info!(
        "已恢复默认数据目录，将在重启后启用: {:?}（force_overwrite={}）",
        default_dir,
        force_overwrite.unwrap_or(false)
    );
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
            force_overwrite_target: false,
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

        let changed = migrate_managed_app_data_files(&source_dir, &target_dir, false)
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

        let changed = migrate_managed_app_data_files(&source_dir, &target_dir, false)
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
        apply_persistent_pragmas(&conn);
        apply_connection_pragmas(&conn);
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

    fn auto_trial_record(source_key: &str) -> AutoTrialRecord {
        AutoTrialRecord {
            source_key: source_key.to_string(),
            account_id: "account-1".to_string(),
            role_id: "role-1".to_string(),
            role_name: "测试角色".to_string(),
            server: "测试服".to_string(),
            layer: 6,
            bosses: ["寅".to_string(), "方鹤影".to_string(), "藤原樱奈".to_string()],
            status: "confirmed".to_string(),
            date: 1_793_715_000_000,
            notes: Some("自动识别".to_string()),
        }
    }

    fn trial_record(id: &str, source: &str, status: &str, source_key: Option<&str>) -> TrialRecord {
        TrialRecord {
            id: id.to_string(),
            account_id: "account-1".to_string(),
            role_id: "role-1".to_string(),
            role_name: "测试角色".to_string(),
            server: "测试服".to_string(),
            layer: 6,
            bosses: vec!["寅".to_string(), "方鹤影".to_string(), "藤原樱奈".to_string()],
            card_1: "card-1".to_string(),
            card_2: "".to_string(),
            card_3: "".to_string(),
            card_4: "".to_string(),
            card_5: "".to_string(),
            flipped_index: 1,
            preserve_flipped_index: false,
            edited_card_indices: Vec::new(),
            record_type: "trial".to_string(),
            source: source.to_string(),
            status: status.to_string(),
            source_key: source_key.map(str::to_string),
            equipment_unreviewed: Some(false),
            draws: Vec::new(),
            date: 1_793_715_000_000,
            notes: Some("手工备注".to_string()),
        }
    }

    fn manual_trial_draw(parent_id: &str, id: &str, sequence: i64) -> TrialDraw {
        TrialDraw {
            id: id.to_string(),
            source_key: format!("manual:{parent_id}:{id}"),
            sequence,
            sequence_ambiguous: false,
            receipt_time: 0,
            receipt_hash: Some("untrusted-client-receipt".to_string()),
            reward_items: vec![TrialDrawReward { name: "不应写入的奖励".to_string(), quantity: 1 }],
            stamina_cost: Some(1000),
            stamina_costs: vec![125, 875],
            flipped_index: if (1..=5).contains(&sequence) { sequence } else { 1 },
            card_1: format!("card-{sequence}"),
            card_2: String::new(),
            card_3: String::new(),
            card_4: String::new(),
            card_5: String::new(),
            equipment_unreviewed: true,
            manual_equipment_card_indexes: Vec::new(),
            edited_card_indices: Vec::new(),
            source: "manual".to_string(),
        }
    }

    #[test]
    fn manual_trial_multiple_draws_are_independent_and_do_not_invent_log_facts() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let mut record = trial_record("manual-multiple", "manual", "confirmed", None);
        record.draws = vec![
            manual_trial_draw(&record.id, "first", 99),
            manual_trial_draw(&record.id, "second", 3),
        ];

        save_trial_record_with_connection(&mut conn, &record).unwrap();
        let saved: Vec<(String, String, i64, i64, Option<String>, String, Option<i64>, String, i64, String)> = conn
            .prepare(
                "SELECT id, source, sequence, receipt_time, receipt_hash, reward_items,
                        stamina_cost, stamina_costs, flipped_index, card_1
                 FROM trial_draws WHERE trial_record_id = ?1 ORDER BY sequence",
            )
            .unwrap()
            .query_map(params![record.id], |row| Ok((
                row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
                row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?, row.get(9)?,
            )))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0], ("first".into(), "manual".into(), 1, 0, None, "[]".into(), None, "[]".into(), 1, "card-99".into()));
        assert_eq!(saved[1], ("second".into(), "manual".into(), 2, 0, None, "[]".into(), None, "[]".into(), 3, "card-3".into()));

        // A second save removes only the missing manual child and keeps an
        // explicit per-card decision on an unchanged child.
        db_update_trial_draw_equipment_with_connection(
            &mut conn,
            "first",
            1,
            Some("protected-manual-card"),
        ).unwrap();
        record.draws = vec![manual_trial_draw(&record.id, "first", 1)];
        save_trial_record_with_connection(&mut conn, &record).unwrap();
        let retained: (String, i64) = conn.query_row(
            "SELECT card_1, (SELECT COUNT(*) FROM trial_draw_equipment_manual_overrides WHERE draw_id = 'first')
             FROM trial_draws WHERE id = 'first'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(retained, ("protected-manual-card".to_string(), 1));
        let removed_second: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trial_draws WHERE id = 'second'", [], |row| row.get(0)
        ).unwrap();
        assert_eq!(removed_second, 0);

        // The same card can be changed explicitly by a fresh form. The
        // transient edited-card list makes that intent unambiguous.
        let mut explicitly_edited = manual_trial_draw(&record.id, "first", 1);
        explicitly_edited.card_1 = "new-explicit-card".to_string();
        explicitly_edited.edited_card_indices = vec![1];
        record.draws = vec![explicitly_edited];
        save_trial_record_with_connection(&mut conn, &record).unwrap();
        let edited: (String, Option<String>) = conn.query_row(
            "SELECT d.card_1, m.equipment_id
             FROM trial_draws d
             LEFT JOIN trial_draw_equipment_manual_overrides m
               ON m.draw_id = d.id AND m.card_index = 1
             WHERE d.id = 'first'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(edited, ("new-explicit-card".to_string(), Some("new-explicit-card".to_string())));

        // A log-derived child owned by the same parent remains untouched
        // rather than being guessed as a replacement for the manual flip.
        conn.execute(
            "INSERT INTO trial_draws (
                id, trial_record_id, source_key, sequence, receipt_time, reward_items,
                reward_sources, stamina_costs, stamina_sources, source
             ) VALUES ('auto-child', ?1, 'auto-child-source', 9, 12345, '[]', '[]', '[]', '[]', 'auto')",
            params![record.id],
        ).unwrap();
        record.draws = vec![manual_trial_draw(&record.id, "first", 1)];
        assert!(save_trial_record_with_connection(&mut conn, &record).is_err());
        let children: Vec<(String, String)> = conn.prepare(
            "SELECT id, source FROM trial_draws WHERE trial_record_id = ?1 ORDER BY id"
        ).unwrap().query_map(params![record.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap().collect::<std::result::Result<_, _>>().unwrap();
        assert_eq!(children, vec![("auto-child".into(), "auto".into()), ("first".into(), "manual".into())]);
    }

    #[test]
    fn stale_manual_multi_draw_form_rolls_back_after_automatic_claim() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let mut record = trial_record("manual-stale-claim", "manual", "confirmed", None);
        record.draws = vec![manual_trial_draw(&record.id, "manual-draw", 1)];
        save_trial_record_with_connection(&mut conn, &record).unwrap();
        conn.execute(
            "UPDATE trial_records SET source_key = 'late-terminal-evidence' WHERE id = ?1",
            params![record.id],
        ).unwrap();

        record.layer = 7;
        record.draws[0].card_1 = "stale-change".to_string();
        record.draws[0].edited_card_indices = vec![1];
        assert!(save_trial_record_with_connection(&mut conn, &record).is_err());
        let saved: (i64, String) = conn.query_row(
            "SELECT r.layer, d.card_1
             FROM trial_records r JOIN trial_draws d ON d.trial_record_id = r.id
             WHERE r.id = ?1 AND d.id = 'manual-draw'",
            params![record.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(saved, (6, "card-1".to_string()));
    }

    #[test]
    fn stale_legacy_manual_form_rolls_back_after_automatic_claim() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let mut stale_form = trial_record("legacy-stale-claim", "manual", "confirmed", None);
        stale_form.flipped_index = 5;
        stale_form.card_5 = "legacy-original".to_string();
        save_trial_record_with_connection(&mut conn, &stale_form).unwrap();
        let auto = auto_trial_record("legacy-stale-auto");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[auto]).unwrap();

        stale_form.card_5 = "stale-overwrite".to_string();
        stale_form.layer = 9;
        assert!(save_trial_record_with_connection(&mut conn, &stale_form).is_err());
        let saved: (i64, String) = conn.query_row(
            "SELECT r.layer, d.card_5
             FROM trial_records r JOIN trial_draws d ON d.trial_record_id = r.id
             WHERE r.id = ?1 AND d.source = 'manual'",
            params![stale_form.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(saved, (6, "legacy-original".to_string()));
    }

    #[test]
    fn manual_multi_draw_ipc_payload_persists_only_manual_card_evidence() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let value = serde_json::json!({
            "id": "manual-ipc-multiple",
            "accountId": "account-1",
            "roleId": "role-1",
            "roleName": "测试角色",
            "server": "测试服",
            "layer": 6,
            "bosses": ["寅", "方鹤影", "藤原樱奈"],
            "card1": "", "card2": "", "card3": "", "card4": "", "card5": "",
            "flippedIndex": 0,
            "type": "trial", "source": "manual", "status": "confirmed",
            "equipmentUnreviewed": true,
            "date": 1793715000000_i64,
            "draws": [
                {
                    "id": "ipc-first",
                    "sourceKey": "manual:manual-ipc-multiple:ipc-first",
                    "sequence": 1, "receiptTime": 0, "flippedIndex": 2,
                    "rewardItems": [], "card1": "51443", "card2": "", "card3": "", "card4": "", "card5": "",
                    "equipmentUnreviewed": true, "editedCardIndices": [1], "source": "manual"
                },
                {
                    "id": "ipc-second",
                    "sourceKey": "manual:manual-ipc-multiple:ipc-second",
                    "sequence": 2, "receiptTime": 0, "flippedIndex": 3,
                    "rewardItems": [], "card1": "", "card2": "", "card3": "51444", "card4": "", "card5": "",
                    "equipmentUnreviewed": true, "editedCardIndices": [3], "source": "manual"
                }
            ]
        });
        let record: TrialRecord = serde_json::from_value(value).unwrap();
        save_trial_record_with_connection(&mut conn, &record).unwrap();

        let rows: Vec<(String, String, i64, i64, String, String, Option<i64>, String, i64)> = conn.prepare(
            "SELECT id, source, sequence, receipt_time, card_1, card_3, stamina_cost, reward_items, flipped_index
             FROM trial_draws WHERE trial_record_id = ?1 ORDER BY sequence"
        ).unwrap().query_map(params![record.id], |row| Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?,
            row.get(5)?, row.get(6)?, row.get(7)?, row.get(8)?,
        ))).unwrap().collect::<std::result::Result<_, _>>().unwrap();
        assert_eq!(rows, vec![
            ("ipc-first".into(), "manual".into(), 1, 0, "51443".into(), "".into(), None, "[]".into(), 2),
            ("ipc-second".into(), "manual".into(), 2, 0, "".into(), "51444".into(), None, "[]".into(), 3),
        ]);
        let overrides: Vec<(String, i64, Option<String>)> = conn.prepare(
            "SELECT draw_id, card_index, equipment_id
             FROM trial_draw_equipment_manual_overrides ORDER BY draw_id, card_index"
        ).unwrap().query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap().collect::<std::result::Result<_, _>>().unwrap();
        assert_eq!(overrides, vec![
            ("ipc-first".into(), 1, Some("51443".into())),
            ("ipc-second".into(), 3, Some("51444".into())),
        ]);
    }

    #[test]
    fn automatic_claim_snapshots_legacy_manual_parent_flip_in_both_claim_paths() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();

        // Direct claim: no prior automatic parent exists.
        let direct_auto = auto_trial_record("legacy-direct-claim");
        let mut direct_manual = trial_record("legacy-direct-manual", "manual", "confirmed", None);
        direct_manual.flipped_index = 4;
        direct_manual.card_4 = "legacy-card-4".to_string();
        save_trial_record_with_connection(&mut conn, &direct_manual).unwrap();
        db_upsert_auto_trial_records_with_connection(&mut conn, &[direct_auto.clone()]).unwrap();
        let direct_saved: (String, i64, String, i64) = conn.query_row(
            "SELECT d.source, d.flipped_index, d.card_4, d.receipt_time
             FROM trial_draws d WHERE d.trial_record_id = ?1",
            params![direct_manual.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(direct_saved, ("manual".into(), 4, "legacy-card-4".into(), 0));

        // Pending merge: an automatic parent already exists and is later
        // replaced by the matching manual confirmation.
        let mut pending_auto = auto_trial_record("legacy-pending-claim");
        pending_auto.status = "pending".to_string();
        db_upsert_auto_trial_records_with_connection(&mut conn, &[pending_auto.clone()]).unwrap();
        let mut pending_manual = trial_record("legacy-pending-manual", "manual", "confirmed", None);
        pending_manual.flipped_index = 2;
        pending_manual.card_2 = "legacy-card-2".to_string();
        save_trial_record_with_connection(&mut conn, &pending_manual).unwrap();
        pending_auto.status = "confirmed".to_string();
        db_upsert_auto_trial_records_with_connection(&mut conn, &[pending_auto]).unwrap();
        let pending_saved: (String, i64, String, i64) = conn.query_row(
            "SELECT d.source, d.flipped_index, d.card_2, d.receipt_time
             FROM trial_draws d WHERE d.trial_record_id = ?1",
            params![pending_manual.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).unwrap();
        assert_eq!(pending_saved, ("manual".into(), 2, "legacy-card-2".into(), 0));
    }

    #[test]
    fn manual_trial_draw_protocol_rejects_invalid_source_key_without_partial_writes() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let mut record = trial_record("manual-protocol", "manual", "confirmed", None);
        let mut invalid = manual_trial_draw(&record.id, "first", 1);
        invalid.source_key = "manual:wrong-parent:first".to_string();
        record.draws = vec![invalid];

        assert!(save_trial_record_with_connection(&mut conn, &record).is_err());
        let parents: i64 = conn.query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0)).unwrap();
        let children: i64 = conn.query_row("SELECT COUNT(*) FROM trial_draws", [], |row| row.get(0)).unwrap();
        assert_eq!((parents, children), (0, 0));
    }

    #[test]
    fn auto_trial_records_are_idempotent_and_preserve_confirmation() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let record = auto_trial_record("terminal-event-1");

        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[record.clone()]).unwrap(), 1);
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[record.clone()]).unwrap(), 0);

        let saved: (String, String, i64, String, String, i64) = conn
            .query_row(
                "SELECT source, status, flipped_index, card_1, source_key, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'terminal-event-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!(saved, ("auto".to_string(), "confirmed".to_string(), 0, "".to_string(), "terminal-event-1".to_string(), 1));

        conn.execute("UPDATE trial_records SET status = 'confirmed' WHERE source_key = 'terminal-event-1'", [])
            .unwrap();
        let mut corrected = record;
        corrected.notes = Some("已补齐来源".to_string());
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[corrected]).unwrap(), 0);
        let status: String = conn
            .query_row("SELECT status FROM trial_records WHERE source_key = 'terminal-event-1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(status, "confirmed");

        let mut incomplete = auto_trial_record("terminal-event-pending");
        incomplete.status = "pending".to_string();
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[incomplete.clone()]).unwrap(), 1);
        incomplete.status = "confirmed".to_string();
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[incomplete]).unwrap(), 1);
        let upgraded_status: String = conn
            .query_row("SELECT status FROM trial_records WHERE source_key = 'terminal-event-pending'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(upgraded_status, "confirmed");
    }

    #[test]
    fn confirmed_auto_trial_fields_survive_scanner_replay_and_legacy_save() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let original = auto_trial_record("terminal-confirmed");
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[original.clone()]).unwrap(), 1);

        let mut replay = original.clone();
        replay.layer = 7;
        replay.bosses = ["雷神".to_string(), "苏髓".to_string(), "叶冬辰".to_string()];
        replay.notes = Some("较差重放证据".to_string());
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[replay]).unwrap(), 0);
        let scanner_saved: (i64, String, Option<String>) = conn
            .query_row(
                "SELECT layer, bosses, notes FROM trial_records WHERE source_key = 'terminal-confirmed'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(scanner_saved.0, 6);
        assert_eq!(scanner_saved.1, "[\"寅\",\"方鹤影\",\"藤原樱奈\"]");
        assert_eq!(scanner_saved.2.as_deref(), Some("自动识别"));

        let legacy_callback = trial_record("trial-auto:terminal-confirmed", "manual", "confirmed", None);
        save_trial_record_with_connection(&mut conn, &legacy_callback).unwrap();
        let legacy_saved: (String, String, String) = conn
            .query_row(
                "SELECT source, status, source_key FROM trial_records WHERE id = 'trial-auto:terminal-confirmed'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(legacy_saved, ("auto".to_string(), "confirmed".to_string(), "terminal-confirmed".to_string()));
    }

    #[test]
    fn confirmed_evidence_merges_pending_auto_candidate_into_manual_record() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let mut pending = auto_trial_record("terminal-merge");
        pending.status = "pending".to_string();
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[pending.clone()]).unwrap(), 1);

        let manual = trial_record("manual-merge", "manual", "confirmed", None);
        save_trial_record_with_connection(&mut conn, &manual).unwrap();
        let mut confirmed = pending;
        confirmed.status = "confirmed".to_string();
        confirmed.notes = Some("完整通关证据".to_string());
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[confirmed]).unwrap(), 1);

        let manual_saved: (String, String, i64, String, Option<String>) = conn
            .query_row(
                "SELECT source, source_key, flipped_index, card_1, notes
                 FROM trial_records WHERE id = 'manual-merge'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(manual_saved.0, "manual");
        assert_eq!(manual_saved.1, "terminal-merge");
        assert_eq!(manual_saved.2, 1);
        assert_eq!(manual_saved.3, "card-1");
        assert_eq!(manual_saved.4.as_deref(), Some("手工备注"));
        let pending_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records WHERE source_key = 'terminal-merge'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(pending_count, 1);
    }

    #[test]
    fn deleting_keyed_trial_blocks_stale_save_and_rescan() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let record = auto_trial_record("terminal-deleted");
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[record.clone()]).unwrap(), 1);
        delete_trial_record_with_connection(&mut conn, "trial-auto:terminal-deleted").unwrap();

        let tombstone_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM trial_auto_scan_tombstones WHERE source_key = 'terminal-deleted'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(tombstone_exists);
        let stale = trial_record("trial-auto:terminal-deleted", "auto", "confirmed", None);
        assert!(save_trial_record_with_connection(&mut conn, &stale).is_err());
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[record]).unwrap(), 0);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records WHERE source_key = 'terminal-deleted'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn auto_trial_records_do_not_replace_exact_manual_or_tombstoned_records() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        let record = auto_trial_record("terminal-event-manual");
        conn.execute(
            "INSERT INTO trial_records (
                id, account_id, role_id, role_name, server, layer, bosses,
                card_1, card_2, card_3, card_4, card_5, flipped_index,
                record_type, source, status, date, updated_at
             ) VALUES (
                'manual-1', 'account-1', 'role-1', '测试角色', '测试服', 6,
                '[\"寅\",\"方鹤影\",\"藤原樱奈\"]', '', '', '', '', '', 2,
                'trial', 'manual', 'confirmed', 1793715000000, 'now'
             )",
            [],
        )
        .unwrap();
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[record]).unwrap(), 0);
        let automatic_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records WHERE source = 'auto'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(automatic_count, 0);

        let mut subsequent = auto_trial_record("terminal-event-manual-next");
        subsequent.date += 60_000;
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[subsequent]).unwrap(), 1);
        let next_automatic_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records WHERE source_key = 'terminal-event-manual-next'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(next_automatic_count, 1, "同一手工记录只能认领一次自动候选");

        conn.execute(
            "INSERT INTO trial_auto_scan_tombstones (source_key, deleted_at) VALUES ('terminal-event-deleted', 'now')",
            [],
        )
        .unwrap();
        assert_eq!(db_upsert_auto_trial_records_with_connection(&mut conn, &[auto_trial_record("terminal-event-deleted")]).unwrap(), 0);
    }

    #[test]
    fn auto_trial_flip_fills_only_exact_unknown_position() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();

        let automatic = auto_trial_record("flip-auto-unknown");
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(&mut conn, &[automatic]).unwrap(),
            1
        );
        conn.execute(
            "UPDATE trial_records
             SET card_1 = '装备甲', notes = '保留备注'
             WHERE source_key = 'flip-auto-unknown'",
            [],
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-auto-unknown", 4).unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );
        let automatic_saved: (i64, String, String, String, String, i64) = conn
            .query_row(
                "SELECT flipped_index, card_1, notes, status, source, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'flip-auto-unknown'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!(
            automatic_saved,
            (
                4,
                "装备甲".to_string(),
                "保留备注".to_string(),
                "confirmed".to_string(),
                "auto".to_string(),
                1,
            )
        );
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-auto-unknown", 1).unwrap(),
            TrialAutoFlipUpdateOutcome::Preserved
        );

        let mut claimed_manual = trial_record(
            "flip-manual-pending",
            "manual",
            "pending",
            Some("flip-manual-pending"),
        );
        claimed_manual.flipped_index = 0;
        claimed_manual.card_1 = "手工装备".to_string();
        save_trial_record_with_connection(&mut conn, &claimed_manual).unwrap();
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-manual-pending", 2).unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );
        let manual_saved: (String, String, i64, String) = conn
            .query_row(
                "SELECT source, status, flipped_index, card_1
                 FROM trial_records WHERE source_key = 'flip-manual-pending'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            manual_saved,
            (
                "manual".to_string(),
                "pending".to_string(),
                2,
                "手工装备".to_string(),
            )
        );

        let manual_known = trial_record(
            "flip-manual-known",
            "manual",
            "confirmed",
            Some("flip-manual-known"),
        );
        save_trial_record_with_connection(&mut conn, &manual_known).unwrap();
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-manual-known", 5).unwrap(),
            TrialAutoFlipUpdateOutcome::Preserved
        );

        conn.execute(
            "UPDATE trial_records SET flipped_index = NULL WHERE source_key = 'flip-manual-known'",
            [],
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-manual-known", 5).unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );

        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-missing", 3).unwrap(),
            TrialAutoFlipUpdateOutcome::Missing
        );
        let missing_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_records WHERE source_key = 'flip-missing'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(missing_count, 0);

        let deleted = auto_trial_record("flip-deleted");
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(&mut conn, &[deleted]).unwrap(),
            1
        );
        delete_trial_record_with_connection(&mut conn, "trial-auto:flip-deleted").unwrap();
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "flip-deleted", 3).unwrap(),
            TrialAutoFlipUpdateOutcome::Deleted
        );
        let deleted_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_records WHERE source_key = 'flip-deleted'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(deleted_count, 0);

        assert!(db_apply_auto_trial_flip_with_connection(&mut conn, "flip-auto-unknown", 0).is_err());
    }

    #[test]
    fn auto_trial_equipment_fills_only_empty_unreviewed_automatic_card() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();

        assert_eq!(
            db_upsert_auto_trial_records_with_connection(
                &mut conn,
                &[auto_trial_record("equipment-auto")]
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "equipment-auto", 2).unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-auto",
                2,
                3,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-auto",
                2,
                2,
                "51468",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        let saved: (String, String, i64) = conn
            .query_row(
                "SELECT card_2, card_3, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'equipment-auto'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(saved, ("51468".to_string(), "51426".to_string(), 1));

        // The observed tooltip can be on card 3 while card 2 was selected.
        // If a user or a newer screenshot changed that selection, the older
        // observation is not allowed to write into the same completion.
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-auto",
                3,
                4,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved
        );
        let card_4: String = conn
            .query_row(
                "SELECT card_4 FROM trial_records WHERE source_key = 'equipment-auto'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(card_4.is_empty());

        // A later frame must not replace a card that has already been
        // recognized. The record remains unreviewed because not all five
        // cards were inspected.
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-auto",
                2,
                3,
                "99999",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved
        );

        conn.execute(
            "UPDATE trial_records SET equipment_unreviewed = 0
             WHERE source_key = 'equipment-auto'",
            [],
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-auto",
                2,
                4,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved
        );

        let manual = trial_record(
            "manual-equipment",
            "manual",
            "confirmed",
            Some("manual-equipment-source"),
        );
        save_trial_record_with_connection(&mut conn, &manual).unwrap();
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "manual-equipment-source",
                1,
                1,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved
        );

        let deleted = auto_trial_record("equipment-deleted");
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(&mut conn, &[deleted]).unwrap(),
            1
        );
        delete_trial_record_with_connection(&mut conn, "trial-auto:equipment-deleted").unwrap();
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-deleted",
                1,
                1,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Deleted
        );
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-missing",
                1,
                1,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Missing
        );
        assert!(db_apply_auto_trial_equipment_with_connection(
            &mut conn,
            "equipment-auto",
            2,
            0,
            "51426"
        )
        .is_err());
        assert!(db_apply_auto_trial_equipment_with_connection(
            &mut conn,
            "equipment-auto",
            2,
            1,
            ""
        )
        .is_err());
    }

    #[test]
    fn incomplete_automatic_card_observations_survive_stale_form_save() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(
                &mut conn,
                &[auto_trial_record("equipment-stale-form")]
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "equipment-stale-form", 2)
                .unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );
        assert_eq!(
            db_apply_auto_trial_equipment_with_connection(
                &mut conn,
                "equipment-stale-form",
                2,
                3,
                "51426",
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );

        let mut stale_form = trial_record(
            "trial-auto:equipment-stale-form",
            "manual",
            "confirmed",
            None,
        );
        stale_form.card_1.clear();
        stale_form.card_2.clear();
        stale_form.card_3.clear();
        stale_form.flipped_index = 0;
        stale_form.preserve_flipped_index = true;
        stale_form.card_2 = "手工选择的装备".to_string();
        stale_form.edited_card_indices = vec![2];
        stale_form.equipment_unreviewed = Some(true);
        save_trial_record_with_connection(&mut conn, &stale_form).unwrap();
        let saved: (String, String, i64, i64) = conn
            .query_row(
                "SELECT card_2, card_3, flipped_index, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'equipment-stale-form'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(
            saved,
            (
                "手工选择的装备".to_string(),
                "51426".to_string(),
                2,
                1
            )
        );

        // An explicit single-card edit is allowed to clear the observation
        // without forcing the player to claim every other card was checked.
        stale_form.edited_card_indices.push(3);
        save_trial_record_with_connection(&mut conn, &stale_form).unwrap();
        let explicitly_cleared: (String, i64) = conn
            .query_row(
                "SELECT card_3, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'equipment-stale-form'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(explicitly_cleared, (String::new(), 1));
    }

    #[test]
    fn trial_save_preserves_omitted_review_and_concurrent_flip_but_accepts_explicit_choices() {
        let (mut conn, _path) = create_test_conn();
        ensure_version_tables(&conn).unwrap();
        install_fresh_db(&conn).unwrap();

        let automatic = auto_trial_record("review-auto");
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(&mut conn, &[automatic]).unwrap(),
            1
        );
        assert_eq!(
            db_apply_auto_trial_flip_with_connection(&mut conn, "review-auto", 3).unwrap(),
            TrialAutoFlipUpdateOutcome::Updated
        );

        // An old form has no equipment-review field and did not touch the card
        // position, so neither a newly recognized position nor the unknown
        // equipment marker may be lost.
        let mut stale_form = trial_record("trial-auto:review-auto", "manual", "confirmed", None);
        stale_form.flipped_index = 0;
        stale_form.preserve_flipped_index = true;
        stale_form.equipment_unreviewed = None;
        stale_form.notes = Some("旧表单补充备注".to_string());
        save_trial_record_with_connection(&mut conn, &stale_form).unwrap();
        let after_stale_form: (i64, i64, String) = conn
            .query_row(
                "SELECT flipped_index, equipment_unreviewed, notes
                 FROM trial_records WHERE source_key = 'review-auto'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(after_stale_form, (3, 1, "旧表单补充备注".to_string()));

        // The current form always sends an explicit review decision. It can
        // therefore confirm an empty result, and a deliberate flip clear still
        // takes effect when preserve_flipped_index is false.
        let mut explicit_empty = stale_form;
        explicit_empty.preserve_flipped_index = false;
        explicit_empty.equipment_unreviewed = Some(false);
        save_trial_record_with_connection(&mut conn, &explicit_empty).unwrap();
        let explicit_empty_saved: (i64, i64) = conn
            .query_row(
                "SELECT flipped_index, equipment_unreviewed
                 FROM trial_records WHERE source_key = 'review-auto'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(explicit_empty_saved, (0, 0));

        let mut manual = trial_record("manual-review", "manual", "confirmed", None);
        manual.equipment_unreviewed = Some(true);
        save_trial_record_with_connection(&mut conn, &manual).unwrap();
        manual.equipment_unreviewed = None;
        save_trial_record_with_connection(&mut conn, &manual).unwrap();
        let manual_review: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_records WHERE id = 'manual-review'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(manual_review, 1);
    }

    #[test]
    fn fresh_install_creates_all_tables_and_indexes() {
        let (conn, _path) = create_test_conn();

        ensure_version_tables(&conn).expect("version tables should be created");
        install_fresh_db(&conn).expect("fresh install should succeed");

        let expected_tables = [
            "accounts",
            "app_config",
            "baizhan_records",
            "cache",
            "equipments",
            "favorite_raids",
            "game_versions",
            "instance_types",
            "jcl_cache",
            "raid_bosses",
            "raid_role_visibility",
            "raids",
            "records",
            "role_instance_visibility",
            "roles",
            "schema_versions",
            "seasons",
            "trial_records",
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
            "id",
            "name",
            "ui_id",
            "icon_id",
            "level",
            "quality",
            "bind_type",
            "type_label",
            "attribute_types",
            "attributes",
            "recommend",
            "diamonds",
            "data",
            "updated_at",
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
            "accounts",
            "app_config",
            "baizhan_records",
            "cache",
            "equipments",
            "favorite_raids",
            "game_versions",
            "instance_types",
            "jcl_cache",
            "raid_bosses",
            "raid_role_visibility",
            "raids",
            "records",
            "role_instance_visibility",
            "roles",
            "schema_versions",
            "seasons",
            "trial_records",
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
            "id",
            "name",
            "ui_id",
            "icon_id",
            "level",
            "quality",
            "bind_type",
            "type_label",
            "attribute_types",
            "attributes",
            "recommend",
            "diamonds",
            "data",
            "updated_at",
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
            "accounts", "app_config", "baizhan_records", "cache", "equipments",
            "favorite_raids", "game_versions", "instance_types",
            "jcl_cache",
            "raid_bosses", "raid_role_visibility", "raids",
            "records", "role_instance_visibility", "roles",
            "schema_versions", "seasons", "trial_records",
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
            "id",
            "name",
            "ui_id",
            "icon_id",
            "level",
            "quality",
            "bind_type",
            "type_label",
            "attribute_types",
            "attributes",
            "recommend",
            "diamonds",
            "data",
            "updated_at",
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
        assert_eq!(
            version, CURRENT_SCHEMA_VERSION,
            "版本应为 V{}",
            CURRENT_SCHEMA_VERSION
        );
    }

    fn auto_trial_draw(
        source_key: &str,
        receipt_time: i64,
        stamina_cost: Option<i64>,
    ) -> AutoTrialDraw {
        AutoTrialDraw {
            source_key: source_key.to_string(),
            receipt_time,
            receipt_hash: Some(format!("hash-{receipt_time}")),
            reward_items: vec![TrialDrawReward {
                name: "银叶子·试炼之地".to_string(),
                quantity: 31,
            }],
            reward_source_hashes: vec![format!("leaf-{receipt_time}")],
            stamina_cost,
            stamina_costs: stamina_cost.into_iter().collect(),
            stamina_source_hashes: stamina_cost
                .map(|_| format!("stamina-{receipt_time}"))
                .into_iter()
                .collect(),
        }
    }

    #[test]
    fn automatic_receipt_renumbering_preserves_manual_draw_sequences_after_claim() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let mut manual = trial_record("manual-claimed-with-draws", "manual", "confirmed", None);
        manual.draws = vec![
            manual_trial_draw(&manual.id, "manual-first", 1),
            manual_trial_draw(&manual.id, "manual-second", 2),
        ];
        save_trial_record_with_connection(&mut conn, &manual).unwrap();
        conn.execute(
            "UPDATE trial_records SET source_key = 'claimed-manual-draws' WHERE id = ?1",
            params![manual.id],
        ).unwrap();

        let second = auto_trial_draw("claimed-manual-draws:receipt:second", 20_000, Some(875));
        let first = auto_trial_draw("claimed-manual-draws:receipt:first", 10_000, Some(125));
        db_upsert_auto_trial_draws_with_connection(
            &mut conn,
            "claimed-manual-draws",
            &[second, first],
        ).unwrap();

        let saved: Vec<(String, String, i64)> = conn.prepare(
            "SELECT id, source, sequence FROM trial_draws
             WHERE trial_record_id = ?1 ORDER BY source ASC, sequence ASC, id ASC"
        ).unwrap().query_map(params![manual.id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap().collect::<std::result::Result<_, _>>().unwrap();
        assert_eq!(saved, vec![
            ("trial-draw:claimed-manual-draws:receipt:first".into(), "auto".into(), 1),
            ("trial-draw:claimed-manual-draws:receipt:second".into(), "auto".into(), 2),
            ("manual-first".into(), "manual".into(), 1),
            ("manual-second".into(), "manual".into(), 2),
        ]);
    }

    #[test]
    fn auto_trial_draws_are_independent_idempotent_and_do_not_recount_completion() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-multiple-draws");
        assert_eq!(
            db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap(),
            1
        );
        let first = auto_trial_draw("terminal-multiple-draws:draw:role:10:h1", 10_000, None);
        let second = auto_trial_draw(
            "terminal-multiple-draws:draw:role:20:h2",
            20_000,
            Some(1_000),
        );
        assert_eq!(
            db_upsert_auto_trial_draws_with_connection(
                &mut conn,
                &parent.source_key,
                &[second.clone(), first.clone()]
            )
            .unwrap(),
            2
        );
        assert_eq!(
            db_upsert_auto_trial_draws_with_connection(
                &mut conn,
                &parent.source_key,
                &[first, second]
            )
            .unwrap(),
            0,
            "重复扫描不能把同一翻牌重新计入"
        );

        let saved: Vec<(i64, i64, Option<i64>, String)> = conn
            .prepare("SELECT sequence, receipt_time, stamina_cost, reward_items FROM trial_draws ORDER BY sequence")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(saved.len(), 2);
        assert_eq!(saved[0].0, 1);
        assert_eq!(saved[0].1, 10_000);
        assert_eq!(saved[0].2, None, "无扣减证据必须保留 None");
        assert_eq!(saved[1].0, 2);
        assert_eq!(saved[1].2, Some(1_000));
        assert!(saved[0].3.contains("银叶子·试炼之地"));
        let completion_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(completion_count, 1, "多笔翻牌只对应一次通关");
    }

    #[test]
    fn draw_manual_edit_and_parent_delete_preserve_tombstone_semantics() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-draw-tombstone");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let draw = auto_trial_draw(
            "terminal-draw-tombstone:draw:role:10:h1",
            10_000,
            Some(1_000),
        );
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[draw.clone()])
            .unwrap();
        conn.execute(
            "UPDATE trial_draws SET flipped_index = 0, position_manual = 1 WHERE source_key = ?1",
            params![draw.source_key],
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_flip_with_connection(
                &mut conn,
                &draw.source_key,
                3,
                9_000,
                10_000
            )
            .unwrap(),
            TrialAutoFlipUpdateOutcome::Preserved,
            "手工清空也必须阻止迟到 OCR 覆盖"
        );
        delete_trial_record_with_connection(&mut conn, "trial-auto:terminal-draw-tombstone")
            .unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_flip_with_connection(
                &mut conn,
                &draw.source_key,
                3,
                9_000,
                10_000
            )
            .unwrap(),
            TrialAutoFlipUpdateOutcome::Deleted
        );
        assert_eq!(
            db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[draw])
                .unwrap(),
            0
        );
    }

    #[test]
    fn late_same_draw_stamina_parts_are_merged_and_total_recomputed() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-late-stamina");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let mut early = auto_trial_draw("terminal-late-stamina:draw:role:10:h1", 10_000, Some(125));
        early.stamina_source_hashes = vec!["stamina-125".to_string()];
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[early])
            .unwrap();
        let mut complete =
            auto_trial_draw("terminal-late-stamina:draw:role:10:h1", 10_000, Some(1_000));
        complete.stamina_costs = vec![125, 875];
        complete.stamina_source_hashes = vec!["stamina-125".to_string(), "stamina-875".to_string()];
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[complete])
            .unwrap();
        let saved: (Option<i64>, String) = conn
            .query_row(
                "SELECT stamina_cost, stamina_costs FROM trial_draws",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(saved.0, Some(1_000));
        assert_eq!(
            serde_json::from_str::<Vec<i64>>(&saved.1).unwrap(),
            vec![125, 875]
        );
    }

    #[test]
    fn a_late_same_second_receipt_keeps_only_its_own_leaf_on_each_draw() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("same-second-draws");
        db_upsert_auto_trial_records_with_connection(&mut conn, std::slice::from_ref(&parent)).unwrap();
        let mut a = auto_trial_draw("draw-a", 12_000, Some(1000));
        a.receipt_hash = Some("a".into());
        a.reward_source_hashes = vec!["a".into(), "box".into()];
        a.reward_items.push(TrialDrawReward { name: "宝箱".into(), quantity: 1 });
        let mut b = auto_trial_draw("draw-b", 12_000, None);
        b.receipt_hash = Some("b".into());
        b.reward_source_hashes = vec!["b".into()];
        for item in [&a, &b, &a] {
            db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, std::slice::from_ref(item)).unwrap();
        }
        let verified: i64 = conn.query_row(
            "SELECT COUNT(*) FROM trial_draws WHERE json_array_length(reward_items)=1
             AND json_extract(reward_items,'$[0].name')='银叶子·试炼之地'
             AND json_extract(reward_sources,'$[0]')=receipt_hash
             AND stamina_cost IS NULL AND stamina_costs='[]'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(verified, 2);
    }

    #[test]
    fn arbitrary_stamina_parts_merge_by_message_hash_without_guessing_total() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-arbitrary-stamina");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let mut first = auto_trial_draw("terminal-arbitrary-stamina:draw:one", 10_000, Some(1));
        first.stamina_source_hashes = vec!["cost-1".to_string()];
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[first]).unwrap();
        let mut late = auto_trial_draw("terminal-arbitrary-stamina:draw:one", 10_000, Some(1_000));
        late.stamina_costs = vec![1, 999];
        late.stamina_source_hashes = vec!["cost-1".to_string(), "cost-999".to_string()];
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late.clone()]).unwrap();
        assert_eq!(
            db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late]).unwrap(),
            0,
            "同一消息 hash 重扫不可重复计入"
        );
        let mut equal_parts = auto_trial_draw("terminal-arbitrary-stamina:draw:two", 20_000, Some(1_000));
        equal_parts.stamina_costs = vec![500, 500];
        equal_parts.stamina_source_hashes = vec!["cost-a".to_string(), "cost-b".to_string()];
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[equal_parts]).unwrap();
        let saved: Vec<(Option<i64>, String)> = conn
            .prepare("SELECT stamina_cost, stamina_costs FROM trial_draws ORDER BY receipt_time")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(saved[0].0, Some(1_000));
        assert_eq!(serde_json::from_str::<Vec<i64>>(&saved[0].1).unwrap(), vec![1, 999]);
        assert_eq!(saved[1].0, Some(1_000));
        assert_eq!(serde_json::from_str::<Vec<i64>>(&saved[1].1).unwrap(), vec![500, 500]);
    }

    #[test]
    fn confirming_manual_match_moves_pending_draws_without_losing_ocr_state() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let mut pending = auto_trial_record("terminal-pending-manual-merge");
        pending.status = "pending".to_string();
        db_upsert_auto_trial_records_with_connection(&mut conn, &[pending.clone()]).unwrap();
        let draw = auto_trial_draw(
            "terminal-pending-manual-merge:draw:role:10:h1",
            10_000,
            Some(1_000),
        );
        db_upsert_auto_trial_draws_with_connection(&mut conn, &pending.source_key, &[draw.clone()])
            .unwrap();
        db_apply_auto_trial_draw_flip_with_connection(
            &mut conn,
            &draw.source_key,
            3,
            9_000,
            10_000,
        )
        .unwrap();
        save_trial_record_with_connection(
            &mut conn,
            &trial_record("manual-target", "manual", "confirmed", None),
        )
        .unwrap();
        let mut confirmed = pending.clone();
        confirmed.status = "confirmed".to_string();
        db_upsert_auto_trial_records_with_connection(&mut conn, &[confirmed]).unwrap();
        let saved: (String, i64, String) = conn
            .query_row(
                "SELECT d.trial_record_id, d.flipped_index, r.source
                 FROM trial_draws d JOIN trial_records r ON r.id = d.trial_record_id
                 WHERE d.source_key = ?1",
                params![draw.source_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(saved.0, "manual-target");
        assert_eq!(saved.1, 3);
        assert_eq!(saved.2, "manual");
    }

    #[test]
    fn late_sibling_invalidates_only_overlapping_automatic_observation() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-observation-overlap");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let first = auto_trial_draw("terminal-observation-overlap:draw:one", 10_000, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[first.clone()])
            .unwrap();
        db_apply_auto_trial_draw_flip_with_connection(
            &mut conn,
            &first.source_key,
            2,
            9_000,
            11_000,
        )
        .unwrap();
        let late = auto_trial_draw("terminal-observation-overlap:draw:late", 10_500, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late]).unwrap();
        let saved: (i64, Option<i64>) = conn.query_row(
            "SELECT flipped_index, auto_observed_start_ms FROM trial_draws WHERE source_key = ?1",
            params![first.source_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(saved, (0, None));
    }

    #[test]
    fn multiple_equipment_observations_keep_draw_unreviewed_until_full_review() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-multiple-equipment");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let draw = auto_trial_draw("terminal-multiple-equipment:draw:one", 10_000, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[draw.clone()]).unwrap();
        db_apply_auto_trial_draw_flip_with_connection(&mut conn, &draw.source_key, 2, 9_000, 10_000).unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn, &draw.source_key, 2, 1, "51426", 9_000, 10_000,
            ).unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn, &draw.source_key, 2, 3, "51468", 9_000, 10_000,
            ).unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        let saved: (String, String, i64) = conn.query_row(
            "SELECT card_1, card_3, equipment_unreviewed FROM trial_draws WHERE source_key = ?1",
            params![draw.source_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).unwrap();
        assert_eq!(saved, ("51426".to_string(), "51468".to_string(), 1));
    }

    #[test]
    fn manual_draw_equipment_is_per_card_and_blocks_late_automatic_changes() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-manual-equipment");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let first = auto_trial_draw("terminal-manual-equipment:draw:first", 10_000, Some(125));
        let second = auto_trial_draw("terminal-manual-equipment:draw:second", 20_000, Some(875));
        db_upsert_auto_trial_draws_with_connection(
            &mut conn,
            &parent.source_key,
            &[first.clone(), second.clone()],
        )
        .unwrap();
        let first_id = format!("trial-draw:{}", first.source_key);
        let second_id = format!("trial-draw:{}", second.source_key);
        db_apply_auto_trial_draw_flip_with_connection(
            &mut conn,
            &first.source_key,
            2,
            9_000,
            11_000,
        )
        .unwrap();

        db_update_trial_draw_equipment_with_connection(
            &mut conn,
            &first_id,
            1,
            Some("manual-card"),
        )
        .unwrap();
        db_update_trial_draw_equipment_with_connection(&mut conn, &first_id, 2, None).unwrap();
        db_update_trial_draw_equipment_with_connection(
            &mut conn,
            &second_id,
            4,
            Some("second-only"),
        )
        .unwrap();

        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                1,
                "late-card-one",
                9_000,
                11_000,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved,
            "人工填写的牌位不能被 OCR 覆盖"
        );
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                2,
                "late-card-two",
                9_000,
                11_000,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved,
            "人工清空也必须阻止 OCR 覆盖"
        );
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                3,
                "auto-card-three",
                9_000,
                11_000,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated,
            "未锁定的其他卡仍可被自动补齐"
        );

        let before_late_receipt: (i64, Option<i64>, String, String, String, String, i64) = conn
            .query_row(
                "SELECT flipped_index, stamina_cost, card_1, card_2, card_3, source, equipment_unreviewed
                 FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
                )),
            )
            .unwrap();
        assert_eq!(before_late_receipt, (2, Some(125), "manual-card".into(), "".into(), "auto-card-three".into(), "auto".into(), 1));

        // A receipt appearing inside the old observation window may revoke the
        // automatic card position too, but it must retain both manual cards,
        // the reward facts, and the stamina fact.
        let late = auto_trial_draw("terminal-manual-equipment:draw:late", 10_500, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late]).unwrap();
        let after_late_receipt: (i64, Option<i64>, String, String, String, String, i64) = conn
            .query_row(
                "SELECT flipped_index, stamina_cost, card_1, card_2, card_3, source, equipment_unreviewed
                 FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| Ok((
                    row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?,
                )),
            )
            .unwrap();
        assert_eq!(after_late_receipt, (0, Some(125), "manual-card".into(), "".into(), "".into(), "auto".into(), 1));
        let second_card: String = conn
            .query_row("SELECT card_4 FROM trial_draws WHERE id = ?1", params![second_id], |row| row.get(0))
            .unwrap();
        assert_eq!(second_card, "second-only", "逐笔手工补录必须互相隔离");
        let overrides: Vec<(String, i64, Option<String>)> = conn
            .prepare(
                "SELECT draw_id, card_index, equipment_id
                 FROM trial_draw_equipment_manual_overrides ORDER BY draw_id, card_index",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(overrides.len(), 3);
        assert!(overrides.contains(&(first_id.clone(), 2, None)));

        assert!(db_update_trial_draw_equipment_with_connection(&mut conn, "missing", 1, Some("x")).is_err());
        assert!(db_update_trial_draw_equipment_with_connection(&mut conn, &first_id, 6, Some("x")).is_err());
        let after_rejected_updates: (String, i64) = conn
            .query_row(
                "SELECT card_1, (SELECT COUNT(*) FROM trial_draw_equipment_manual_overrides)
                 FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(after_rejected_updates, ("manual-card".to_string(), 3), "无效更新必须不留下部分写入");
        let index_exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index'
                 AND name = 'idx_trial_draw_equipment_manual_overrides_draw_card')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_exists);
    }

    #[test]
    fn complete_draw_review_preserves_all_cards_until_explicitly_revoked() {
        let (mut conn, path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-complete-draw-review");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let first = auto_trial_draw("terminal-complete-draw-review:draw:first", 10_000, Some(125));
        let sibling = auto_trial_draw("terminal-complete-draw-review:draw:sibling", 20_000, Some(875));
        db_upsert_auto_trial_draws_with_connection(
            &mut conn,
            &parent.source_key,
            &[first.clone(), sibling.clone()],
        )
        .unwrap();
        let first_id = format!("trial-draw:{}", first.source_key);
        let sibling_id = format!("trial-draw:{}", sibling.source_key);
        db_apply_auto_trial_draw_flip_with_connection(
            &mut conn,
            &first.source_key,
            2,
            9_000,
            11_000,
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                1,
                "ocr-card-one",
                9_000,
                11_000,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        db_update_trial_draw_equipment_with_connection(&mut conn, &first_id, 2, None).unwrap();
        db_update_trial_draw_equipment_with_connection(
            &mut conn,
            &sibling_id,
            4,
            Some("sibling-manual-card"),
        )
        .unwrap();

        // This action means all five currently visible values have been
        // checked. Empty slots are part of that decision, not OCR gaps.
        let initially_reviewed_cards = vec![
            "ocr-card-one".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        db_set_trial_draw_equipment_reviewed_with_connection(
            &mut conn,
            &first_id,
            true,
            Some(2),
            Some(&initially_reviewed_cards),
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                3,
                "late-ocr-card",
                9_000,
                11_000,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved,
            "完整核对后，空牌位也不能再被迟到 OCR 填入"
        );

        // A same-window later receipt may still revoke an automatically
        // detected position, but it cannot clear the reviewed board or reset
        // its confirmation state.
        let late = auto_trial_draw("terminal-complete-draw-review:draw:late", 10_500, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late]).unwrap();
        let after_late: (i64, String, String, String, String, String, i64) = conn
            .query_row(
                "SELECT flipped_index, card_1, card_2, card_3, card_4, card_5,
                        equipment_unreviewed
                 FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!(
            after_late,
            (0, "ocr-card-one".into(), "".into(), "".into(), "".into(), "".into(), 0)
        );
        let sibling_card: String = conn
            .query_row(
                "SELECT card_4 FROM trial_draws WHERE id = ?1",
                params![sibling_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sibling_card, "sibling-manual-card", "只能操作当前翻牌明细");

        // Revocation deliberately keeps both the saved board and previous
        // per-card manual evidence. Only unprotected empty slots may accept
        // future OCR after the user has made that decision.
        db_set_trial_draw_equipment_reviewed_with_connection(&mut conn, &first_id, false, None, None)
            .unwrap();
        let after_revoke: (String, String, i64, i64) = conn
            .query_row(
                "SELECT card_1, card_2, equipment_unreviewed,
                        (SELECT COUNT(*) FROM trial_draw_equipment_manual_overrides
                         WHERE draw_id = trial_draws.id)
                 FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(after_revoke, ("ocr-card-one".into(), "".into(), 1, 1));

        conn.execute(
            "UPDATE trial_draws SET flipped_index = 2, position_manual = 1 WHERE id = ?1",
            params![first_id],
        )
        .unwrap();
        let cards_seen_before_late_ocr = vec![
            "ocr-card-one".to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                4,
                "ocr-after-revoke",
                12_000,
                12_100,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Updated
        );
        let stale_confirm_error = db_set_trial_draw_equipment_reviewed_with_connection(
            &mut conn,
            &first_id,
            true,
            Some(2),
            Some(&cards_seen_before_late_ocr),
        )
        .unwrap_err();
        assert_eq!(stale_confirm_error, "牌面已变化，请刷新后重新核对");
        let stale_confirm_state: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stale_confirm_state, 1, "过期快照不能改变核对状态");
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                2,
                "must-not-replace-manual-empty",
                12_000,
                12_100,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved
        );

        let cards_seen_after_late_ocr = vec![
            "ocr-card-one".to_string(),
            String::new(),
            String::new(),
            "ocr-after-revoke".to_string(),
            String::new(),
        ];
        db_set_trial_draw_equipment_reviewed_with_connection(
            &mut conn,
            &first_id,
            true,
            Some(2),
            Some(&cards_seen_after_late_ocr),
        )
        .unwrap();
        assert_eq!(
            db_apply_auto_trial_draw_equipment_with_connection(
                &mut conn,
                &first.source_key,
                2,
                5,
                "late-after-confirm",
                12_000,
                12_100,
            )
            .unwrap(),
            TrialAutoEquipmentUpdateOutcome::Preserved,
            "与当前牌面一致的快照确认后仍应保护空牌"
        );

        assert!(db_set_trial_draw_equipment_reviewed_with_connection(
            &mut conn,
            "missing",
            true,
            Some(2),
            Some(&cards_seen_after_late_ocr),
        )
        .is_err());
        let state_after_missing: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state_after_missing, 0, "不存在的明细不能产生部分写入");

        // The immediate transaction must fail without changing the draw when
        // a concurrent save owns SQLite's write lock.
        let blocker = Connection::open(&path).unwrap();
        blocker.execute_batch("PRAGMA busy_timeout = 0; BEGIN IMMEDIATE;").unwrap();
        conn.execute_batch("PRAGMA busy_timeout = 0;").unwrap();
        assert!(db_set_trial_draw_equipment_reviewed_with_connection(
            &mut conn,
            &first_id,
            false,
            None,
            None,
        )
        .is_err());
        blocker.execute_batch("ROLLBACK;").unwrap();
        let state_after_locked_write: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_draws WHERE id = ?1",
                params![first_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(state_after_locked_write, 0, "并发失败不能回滚或篡改已存牌面");
    }

    #[test]
    fn deleting_parent_removes_manual_draw_equipment_overrides() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-delete-manual-equipment");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let draw = auto_trial_draw("terminal-delete-manual-equipment:draw", 10_000, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[draw.clone()]).unwrap();
        db_update_trial_draw_equipment_with_connection(
            &mut conn,
            &format!("trial-draw:{}", draw.source_key),
            5,
            Some("manual-card"),
        )
        .unwrap();
        delete_trial_record_with_connection(&mut conn, "trial-auto:terminal-delete-manual-equipment").unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_draw_equipment_manual_overrides", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn late_sibling_keeps_manual_position_but_clears_automatic_cards() {
        let (mut conn, _path) = create_test_conn();
        install_fresh_db_for_test(&conn).unwrap();
        let parent = auto_trial_record("terminal-manual-position-late-sibling");
        db_upsert_auto_trial_records_with_connection(&mut conn, &[parent.clone()]).unwrap();
        let first = auto_trial_draw("terminal-manual-position-late-sibling:draw:one", 10_000, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[first.clone()]).unwrap();
        conn.execute(
            "UPDATE trial_draws SET flipped_index = 3, position_manual = 1, card_1 = '51426',
             auto_observed_start_ms = 9000, auto_observed_end_ms = 11000
             WHERE source_key = ?1",
            params![first.source_key],
        ).unwrap();
        let late = auto_trial_draw("terminal-manual-position-late-sibling:draw:late", 10_500, None);
        db_upsert_auto_trial_draws_with_connection(&mut conn, &parent.source_key, &[late]).unwrap();
        let saved: (i64, String) = conn.query_row(
            "SELECT flipped_index, card_1 FROM trial_draws WHERE source_key = ?1",
            params![first.source_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap();
        assert_eq!(saved, (3, String::new()));
    }
}
