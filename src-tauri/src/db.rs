use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::runtime_mode::{self, RuntimeMode};

mod migration;
pub mod migrations;

const DATABASE_NAME: &str = "jx3-raid-manager.db";
const LOG_FILE_NAME: &str = "jx3-raid-manager.log";
const DATA_DIR_BOOTSTRAP_FILE: &str = "data-dir.json";
const DATA_DIR_INSTALLER_STATE_FILE: &str = "data-dir.ini";

/// 当前数据库 schema 版本
pub const CURRENT_SCHEMA_VERSION: i32 = 7;

/// 数据库连接单例
static DB_INITIALIZED: Mutex<bool> = Mutex::new(false);

/// 获取当前本地时间的 RFC3339 格式字符串
fn get_local_timestamp() -> String {
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

    let content = fs::read_to_string(&config_path)
        .map_err(|e| format!("读取数据目录配置失败: {}", e))?;

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
    for entry in fs::read_dir(path).map_err(|e| format!("读取目录失败 {}: {}", path.display(), e))? {
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
    let left_metadata = fs::metadata(left)
        .map_err(|e| format!("读取文件信息失败 {}: {}", left.display(), e))?;
    let right_metadata = fs::metadata(right)
        .map_err(|e| format!("读取文件信息失败 {}: {}", right.display(), e))?;

    if left_metadata.len() != right_metadata.len() {
        return Ok(false);
    }

    let mut left_file = fs::File::open(left)
        .map_err(|e| format!("打开文件失败 {}: {}", left.display(), e))?;
    let mut right_file = fs::File::open(right)
        .map_err(|e| format!("打开文件失败 {}: {}", right.display(), e))?;

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
        let install_dir =
            get_install_dir().ok_or_else(|| "无法获取安装目录".to_string())?;
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
            log::info!(
                "已迁移数据目录内容: {:?} -> {:?}",
                source_dir,
                target_dir
            );
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

#[tauri::command]
pub fn db_delete_directory(path: String, target_type: String) -> Result<DirectoryDeleteResult, String> {
    let target_path = PathBuf::from(path.trim());
    let display_path = target_path.display().to_string();

    if !target_path.exists() {
        log::debug!("{}", format_directory_delete_message(&target_type, &display_path, false));
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

    log::info!("{}", format_directory_delete_message(&target_type, &display_path, true));

    Ok(DirectoryDeleteResult {
        deleted: true,
        path: display_path,
    })
}

/// 初始化数据库（单例模式，只初始化一次）
///
/// 流程：
/// 1. 如果数据库不存在 → 新安装，创建最新版本结构
/// 2. 如果数据库存在但版本较低 → 升级，执行增量迁移
/// 3. 如果数据库存在且是最新版本 → 直接返回连接
pub fn init_db() -> Result<Connection, String> {
    // 检查是否已初始化
    {
        let initialized = DB_INITIALIZED.lock().map_err(|e| e.to_string())?;
        if *initialized {
            // 已初始化，直接返回新连接（SQLite 支持多连接）
            return Connection::open(get_db_path()?).map_err(|e| e.to_string());
        }
    }

    // 执行初始化（首次）
    let path = get_db_path()?;
    let db_exists = path.exists();

    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    // 配置数据库为安全模式，确保数据真正写入磁盘
    conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE;")
        .ok();

    // 创建 schema_versions 表（用于记录版本）和 migration_flags 表
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

    // 获取当前版本
    let current_version = get_schema_version(&conn)?;

    if !db_exists {
        // ========== 全新安装场景 ==========
        // 数据库文件不存在，创建最新版本结构
        log::info!(
            "数据库初始化：全新安装，创建最新版本结构 (V{})",
            CURRENT_SCHEMA_VERSION
        );

        // 创建所有表（最新版本结构）
        create_latest_schema(&conn)?;

        // 记录版本号
        set_schema_version(&conn, CURRENT_SCHEMA_VERSION, "初始安装")?;

        // 初始化静态副本数据
        migration::init_static_raids(&conn)?;
    } else if current_version == 0 {
        // ========== 从旧版本升级场景 ==========
        // 数据库存在但没有版本记录，说明是旧版本
        log::info!("数据库初始化：从旧版本升级，执行所有迁移脚本");

        // 确保基础表存在（升级路径不会调用 create_latest_schema）
        ensure_base_tables(&conn)?;

        // 执行所有迁移（V1 到当前版本）
        for version in 1..=CURRENT_SCHEMA_VERSION {
            log::info!("执行迁移脚本：V{}", version);
            // 使用事务包装迁移，大幅提升性能
            conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;
            let result = migration::apply_migration(&conn, version);
            if let Err(e) = result {
                conn.execute("ROLLBACK", []).ok();
                return Err(e);
            }
            set_schema_version(&conn, version, &format!("升级到 V{}", version))?;
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            log::info!("迁移 V{} 完成", version);
        }

        // 初始化静态副本数据
        migration::init_static_raids(&conn)?;
    } else if current_version < CURRENT_SCHEMA_VERSION {
        // ========== 从中间版本升级场景 ==========
        log::info!(
            "数据库初始化：从 V{} 升级到 V{}",
            current_version,
            CURRENT_SCHEMA_VERSION
        );

        // 确保基础表存在
        ensure_base_tables(&conn)?;

        // 执行增量迁移
        for version in (current_version + 1)..=CURRENT_SCHEMA_VERSION {
            log::info!("执行迁移脚本：V{}", version);
            // 使用事务包装迁移，大幅提升性能
            conn.execute("BEGIN TRANSACTION", []).map_err(|e| e.to_string())?;
            let result = migration::apply_migration(&conn, version);
            if let Err(e) = result {
                conn.execute("ROLLBACK", []).ok();
                return Err(e);
            }
            set_schema_version(&conn, version, &format!("升级到 V{}", version))?;
            conn.execute("COMMIT", []).map_err(|e| e.to_string())?;
            log::info!("迁移 V{} 完成", version);
        }

        // 初始化静态副本数据（可能会添加新的预设副本）
        migration::init_static_raids(&conn)?;
    } else {
        // ========== 已是最新版本 ==========
        log::info!("数据库初始化：已是最新版本 V{}", current_version);
    }

    // 标记已初始化
    {
        let mut initialized = DB_INITIALIZED.lock().map_err(|e| e.to_string())?;
        *initialized = true;
    }

    Ok(conn)
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
    Ok(version)
}

/// 设置 schema 版本
fn set_schema_version(conn: &Connection, version: i32, description: &str) -> Result<(), String> {
    let timestamp = get_local_timestamp();
    conn.execute(
        "INSERT INTO schema_versions (version, applied_at, description) VALUES (?, ?, ?)
         ON CONFLICT(version) DO UPDATE SET applied_at = excluded.applied_at, description = excluded.description",
        params![version, timestamp, description],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 创建基础表（升级路径中也需要确保这些表存在）
fn ensure_base_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            data TEXT
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

    log::info!("基础表检查完成");
    Ok(())
}

/// 创建最新版本的数据库结构
fn create_latest_schema(conn: &Connection) -> Result<(), String> {
    // 先创建基础表
    ensure_base_tables(conn)?;

    conn.execute_batch(
        r#"
        -- ========== 试炼记录表 ==========
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

        -- ========== 百战记录表 ==========
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

        -- ========== 账号表 (V1+ 结构化格式) ==========
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

        -- ========== 角色表 (V1+ 结构化格式) ==========
        CREATE TABLE IF NOT EXISTS roles (
            id TEXT PRIMARY KEY,
            account_id TEXT NOT NULL,
            name TEXT NOT NULL,
            server TEXT,
            region TEXT,
            sect TEXT,
            equipment_score INTEGER,
            disabled INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_roles_account_id ON roles(account_id);
        CREATE INDEX IF NOT EXISTS idx_accounts_name ON accounts(account_name);
        CREATE INDEX IF NOT EXISTS idx_accounts_sort_order ON accounts(sort_order);

        -- ========== 副本表 (V2+ 结构化格式) ==========
        CREATE TABLE IF NOT EXISTS raids (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            difficulty TEXT NOT NULL DEFAULT '普通',
            player_count INTEGER NOT NULL DEFAULT 25,
            version TEXT,
            notes TEXT,
            is_active INTEGER DEFAULT 1,
            is_static INTEGER DEFAULT 0
        );

        -- ========== BOSS 表 (V2+) ==========
        CREATE TABLE IF NOT EXISTS raid_bosses (
            id TEXT PRIMARY KEY,
            raid_name TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL
        );

        -- ========== 副本版本表 (V2+) ==========
        CREATE TABLE IF NOT EXISTS raid_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        );

        -- ========== 副本收藏表 (V4+) ==========
        CREATE TABLE IF NOT EXISTS favorite_raids (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            raid_name TEXT NOT NULL UNIQUE,
            created_at TEXT NOT NULL
        );

        -- ========== 副本类型表 (V5+) ==========
        CREATE TABLE IF NOT EXISTS instance_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );

        -- ========== 角色副本可见性表 (V5+) ==========
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

        CREATE INDEX IF NOT EXISTS idx_riv_role_id ON role_instance_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_riv_instance_type_id ON role_instance_visibility(instance_type_id);

        -- ========== 团队副本角色可见性表 (V6+) ==========
        -- 专门用于存储团队副本级别的角色禁用/启用配置，与 instance_types 分离
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

        CREATE INDEX IF NOT EXISTS idx_rrv_role_id ON raid_role_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_rrv_raid_key ON raid_role_visibility(raid_key);
    "#,
    )
    .map_err(|e| e.to_string())?;

    // ==== 插入默认的 instance_types 数据 ====
    conn.execute_batch(
        r#"
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (1, 'raid', '团队副本');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (2, 'baizhan', '百战异闻录');
        INSERT OR IGNORE INTO instance_types (id, type, name) VALUES (3, 'trial', '试炼之地');
        "#
    ).map_err(|e| e.to_string())?;

    log::info!("数据库结构创建完成");
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
        // Try to add columns if not exists (Simple migration for dev)
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN bind_type INTEGER", []);
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN type_label TEXT", []);
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN attribute_types TEXT", []);
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN attributes TEXT", []);
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN recommend TEXT", []);
        let _ = tx.execute("ALTER TABLE equipments ADD COLUMN diamonds TEXT", []);

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
        .prepare("SELECT data FROM equipments ORDER BY level DESC")
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| Ok(row.get::<_, String>(0)?))
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        if let Ok(json_str) = row {
            if let Ok(item) = serde_json::from_str::<serde_json::Value>(&json_str) {
                result.push(item);
            }
        }
    }

    serde_json::to_string(&result).map_err(|e| e.to_string())
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
    let item: TrialRecord = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    let conn = init_db().map_err(|e| e.to_string())?;
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
    .map_err(|e| e.to_string())?;

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
pub fn db_get_accounts() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM accounts")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut accounts = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if let Ok(data) = row.get(0) {
            accounts.push(data);
        }
    }
    Ok(accounts)
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
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // 2. 获取现有的角色 ID 列表
    let existing_role_ids: Vec<String> = {
        let mut stmt = tx
            .prepare("SELECT id FROM roles")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
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

    // 4. 删除不在传入数据中的账号（会级联删除角色和可见性配置）
    for account_id in &existing_account_ids {
        if !incoming_account_ids.contains(account_id) {
            tx.execute("DELETE FROM accounts WHERE id = ?", params![account_id])
                .map_err(|e| e.to_string())?;
        }
    }

    // 5. 删除不在传入数据中的角色（会级联删除可见性配置）
    for role_id in &existing_role_ids {
        if !incoming_role_ids.contains(role_id) {
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
                let sect = role["sect"].as_str().map(|s| s.to_string());
                let r_disabled = role["disabled"].as_bool().unwrap_or(false) as i32;
                let equipment_score = role["equipmentScore"].as_i64();

                // 使用 INSERT ... ON CONFLICT DO UPDATE 避免触发级联删除
                tx.execute(
                    "INSERT INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET
                        account_id = excluded.account_id,
                        name = excluded.name,
                        server = excluded.server,
                        region = excluded.region,
                        sect = excluded.sect,
                        equipment_score = excluded.equipment_score,
                        disabled = excluded.disabled,
                        updated_at = excluded.updated_at",
                    params![role_id, id, name, server, region, sect, equipment_score, r_disabled, timestamp],
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
        SELECT r.id, r.account_id, r.name, r.server, r.region, r.sect, 
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
                "equipmentScore": row.get::<_, Option<i64>>(6)?,
                "disabled": row.get::<_, i32>(7)? != 0,
                "createdAt": row.get::<_, Option<String>>(8)?,
                "updatedAt": row.get::<_, Option<String>>(9)?,
                "accountName": row.get::<_, String>(10)?,
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
    let mut vis_stmt = conn.prepare("
        SELECT riv.role_id, it.type, riv.visible
        FROM role_instance_visibility riv
        JOIN instance_types it ON riv.instance_type_id = it.id
    ").map_err(|e| e.to_string())?;

    let mut vis_map: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> = std::collections::HashMap::new();
    let vis_rows = vis_stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)? == 1
        ))
    }).map_err(|e| e.to_string())?;

    for row_res in vis_rows {
        if let Ok((role_id, type_str, visible)) = row_res {
            let role_vis = vis_map.entry(role_id).or_insert_with(|| default_vis_map.clone());
            role_vis.insert(type_str, serde_json::json!(visible));
        }
    }

    // 单次 LEFT JOIN 查询获取账号和角色
    // Account 字段: 0-9, Role 字段: 10-18 (可能为 NULL)
    let mut stmt = conn
        .prepare(
            "
        SELECT
            a.id, a.account_name, a.account_type, a.sort_order, a.password, a.notes,
            a.hidden, a.disabled, a.created_at, a.updated_at,
            r.id, r.account_id, r.name, r.server, r.region,
            r.sect, r.equipment_score, r.disabled, r.created_at, r.updated_at
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
                    equipment_score: row.get(16)?,
                    disabled: row.get::<_, i32>(17)? != 0,
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
        a_sort_order
            .cmp(&b_sort_order)
            .then_with(|| {
                a["accountName"]
                    .as_str()
                    .unwrap_or("")
                    .cmp(b["accountName"].as_str().unwrap_or(""))
            })
    });

    log::info!("[db_get_accounts_with_roles] 查询完成，返回 {} 个账号", accounts.len());
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
    let mut vis_stmt = conn.prepare("
        SELECT riv.role_id, it.type, riv.visible
        FROM role_instance_visibility riv
        JOIN instance_types it ON riv.instance_type_id = it.id
        WHERE riv.role_id IN (SELECT id FROM roles WHERE account_id = ?)
    ").map_err(|e| e.to_string())?;

    let mut vis_map: std::collections::HashMap<String, serde_json::Map<String, serde_json::Value>> = std::collections::HashMap::new();
    let vis_rows = vis_stmt.query_map(params![account_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i32>(2)? == 1
        ))
    }).map_err(|e| e.to_string())?;

    for row_res in vis_rows {
        if let Ok((role_id, type_str, visible)) = row_res {
            let role_vis = vis_map.entry(role_id).or_insert_with(|| default_vis_map.clone());
            role_vis.insert(type_str, serde_json::json!(visible));
        }
    }

    let mut stmt = conn
        .prepare(
            "
        SELECT id, name, server, region, sect, equipment_score, disabled, created_at, updated_at 
        FROM roles WHERE account_id = ? ORDER BY name
    ",
        )
        .map_err(|e| e.to_string())?;

    let roles: Vec<serde_json::Value> = stmt
        .query_map(params![account_id], |row| {
            let role_id: String = row.get(0)?;
            let visibility = vis_map.get(&role_id).cloned().unwrap_or_else(|| default_vis_map.clone());

            Ok(serde_json::json!({
                "id": role_id,
                "name": row.get::<_, String>(1)?,
                "server": row.get::<_, Option<String>>(2)?,
                "region": row.get::<_, Option<String>>(3)?,
                "sect": row.get::<_, Option<String>>(4)?,
                "equipmentScore": row.get::<_, Option<i64>>(5)?,
                "disabled": row.get::<_, i32>(6)? != 0,
                "createdAt": row.get::<_, Option<String>>(7)?,
                "updatedAt": row.get::<_, Option<String>>(8)?,
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
        "INSERT INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            account_id = excluded.account_id,
            name = excluded.name,
            server = excluded.server,
            region = excluded.region,
            sect = excluded.sect,
            equipment_score = excluded.equipment_score,
            disabled = excluded.disabled,
            updated_at = excluded.updated_at",
        params![id, account_id, name, server, region, sect, equipment_score, disabled, timestamp],
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
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM accounts WHERE id = ?", params![account_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_delete_role_structured(role_id: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM roles WHERE id = ?", params![role_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_records() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM records")
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
        tx.execute(
            "INSERT INTO records (id, data) VALUES (?, ?)
             ON CONFLICT(id) DO UPDATE SET data = excluded.data",
            params![id, record.to_string()],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_raids() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 从 raids 读取结构化数据
    let mut stmt = conn
        .prepare("SELECT id, name, difficulty, player_count, version, notes, is_active, is_static FROM raids")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;

    let mut raids: Vec<serde_json::Value> = Vec::new();
    let mut raid_ids: Vec<String> = Vec::new();

    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        let id: String = row.get(0).map_err(|e| e.to_string())?;
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
        raid_ids.push(id);
    }
    drop(rows);
    drop(stmt);

    // 为每个副本加载 BOSS 列表（按副本名称关联）
    for (i, _raid_id) in raid_ids.iter().enumerate() {
        let raid_name = raids[i]["name"].as_str().unwrap_or_default().to_string();
        let mut boss_stmt = conn
            .prepare("SELECT id, name, boss_order FROM raid_bosses WHERE raid_name = ? ORDER BY boss_order")
            .map_err(|e| e.to_string())?;
        let mut boss_rows = boss_stmt
            .query(params![raid_name])
            .map_err(|e| e.to_string())?;
        let mut bosses: Vec<serde_json::Value> = Vec::new();

        while let Some(boss_row) = boss_rows.next().map_err(|e| e.to_string())? {
            let boss_id: String = boss_row.get(0).map_err(|e| e.to_string())?;
            let boss_name: String = boss_row.get(1).map_err(|e| e.to_string())?;
            let boss_order: i64 = boss_row.get(2).map_err(|e| e.to_string())?;
            bosses.push(serde_json::json!({
                "id": boss_id,
                "name": boss_name,
                "order": boss_order
            }));
        }

        if !bosses.is_empty() {
            raids[i]["bosses"] = serde_json::json!(bosses);
        }
    }

    // 序列化为 JSON 字符串数组（兼容前端解析方式）
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
        let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| e.to_string())?;
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
                    ).map_err(|e| e.to_string())?;
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

    conn.execute(
        "INSERT INTO records (id, data) VALUES (?, ?)
         ON CONFLICT(id) DO UPDATE SET data = excluded.data",
        params![id, record],
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
pub fn db_get_records_by_raid(_raid_id: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT data FROM records")
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
pub fn db_deduplicate_accounts() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 读取所有账号
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, data FROM accounts")
        .map_err(|e| e.to_string())?;
    let mut rows_iter = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
        if let (Ok(id), Ok(data)) = (row.get(0), row.get(1)) {
            rows.push((id, data));
        }
    }

    let total_before = rows.len();

    // 去重：保留每个 id 的第一条记录
    let mut seen = std::collections::HashSet::new();
    let mut accounts_to_keep: Vec<(String, String)> = Vec::new();
    let mut removed = 0;

    for (id, data) in rows {
        if seen.insert(id.clone()) {
            accounts_to_keep.push((id, data));
        } else {
            removed += 1;
        }
    }

    // 重新打开连接执行写入操作
    {
        let mut conn = init_db().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM accounts", [])
            .map_err(|e| e.to_string())?;

        for (id, data) in &accounts_to_keep {
            tx.execute(
                "INSERT INTO accounts (id, data) VALUES (?, ?)",
                params![id, data],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "✓ 账号去重完成！\n  处理前: {} 条\n  删除: {} 条\n  保留: {} 条",
        total_before,
        removed,
        accounts_to_keep.len()
    ))
}

#[allow(dead_code)]
#[tauri::command]
pub fn db_deduplicate_raids() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // 读取所有副本
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, data FROM raids")
        .map_err(|e| e.to_string())?;
    let mut rows_iter = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
        if let (Ok(id), Ok(data)) = (row.get(0), row.get(1)) {
            rows.push((id, data));
        }
    }

    let total_before = rows.len();

    // 去重逻辑：对于同名的副本，保留最新日期的
    let mut raids_by_name: std::collections::HashMap<String, (String, String)> =
        std::collections::HashMap::new();

    for (id, data) in rows {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
            let date = json["date"].as_str().unwrap_or("").to_string();
            let existing = raids_by_name.get(&id);
            if existing.is_none() {
                raids_by_name.insert(id.clone(), (date, data));
            } else {
                let existing_date = existing.as_ref().unwrap().0.clone();
                if date > existing_date {
                    raids_by_name.insert(id.clone(), (date, data));
                }
            }
        } else {
            if !raids_by_name.contains_key(&id) {
                raids_by_name.insert(id.clone(), (String::new(), data));
            }
        }
    }

    let removed = total_before - raids_by_name.len();

    // 重新打开连接执行写入操作
    {
        let mut conn = init_db().map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;

        tx.execute("DELETE FROM raids", [])
            .map_err(|e| e.to_string())?;

        for (id, (_, data)) in &raids_by_name {
            tx.execute(
                "INSERT INTO raids (id, data) VALUES (?, ?)",
                params![id, data],
            )
            .map_err(|e| e.to_string())?;
        }

        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(format!(
        "✓ 副本去重完成！\n  处理前: {} 条\n  删除: {} 条\n  保留: {} 条",
        total_before,
        removed,
        raids_by_name.len()
    ))
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
    let dest_path = PathBuf::from(backup_path);

    if src_path.exists() {
        std::fs::copy(&src_path, &dest_path).map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub fn db_restore(restore_path: String) -> Result<(), String> {
    let dest_path = get_db_path()?;
    let src_path = PathBuf::from(restore_path);

    if src_path.exists() {
        std::fs::create_dir_all(
            dest_path
                .parent()
                .ok_or_else(|| "无法获取父目录".to_string())?,
        )
        .map_err(|e| e.to_string())?;

        std::fs::copy(&src_path, &dest_path).map_err(|e| e.to_string())?;
    }

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
pub fn db_save_role_visibility(role_id: String, instance_type: String, visible: bool) -> Result<(), String> {
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
        .prepare(
            "SELECT role_id, visible FROM raid_role_visibility WHERE raid_key = ?1",
        )
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
pub fn db_save_raid_role_visibility(roleId: String, raidKey: String, visible: bool) -> Result<(), String> {
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
    pub location: String,       // "custom" | "install" | "user_home"
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

fn write_custom_data_dir_config(path: Option<&Path>, migration_source: Option<&Path>) -> Result<(), String> {
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
                    equipment_score: Some(123456),
                    disabled: false,
                }),
            },
            &visibility,
            &default_visibility,
        );

        let account = account_map.get("account-1").expect("account should exist");
        assert_eq!(account["roles"].as_array().map(|roles| roles.len()), Some(1));
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
        fs::write(source_dir.join(LOG_FILE_NAME), "log-content").expect("log file should be written");

        let mut config = DataDirBootstrapConfig {
            custom_data_dir: Some(target_dir.to_string_lossy().to_string()),
            pending_migration_from: Some(source_dir.to_string_lossy().to_string()),
        };

        maybe_migrate_app_data(&target_dir, &mut config).expect("migration should succeed");

        assert!(target_dir.join(DATABASE_NAME).exists(), "db file should exist in target");
        assert!(target_dir.join(LOG_FILE_NAME).exists(), "log file should exist in target");
        assert!(
            !source_dir.join(DATABASE_NAME).exists(),
            "db file should be removed from source after migration"
        );
        assert!(
            !source_dir.join(LOG_FILE_NAME).exists(),
            "log file should be removed from source after migration"
        );
        assert!(config.pending_migration_from.is_none(), "pending migration should be cleared");
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
        fs::write(source_dir.join(LOG_FILE_NAME), "log-content").expect("log file should be written");
        fs::write(unrelated_dir.join("note.txt"), "keep").expect("unrelated file should be written");

        let changed = migrate_managed_app_data_files(&source_dir, &target_dir)
            .expect("migration should succeed for nested target");

        assert!(changed, "managed files should be moved");
        assert!(target_dir.join(DATABASE_NAME).exists(), "db file should exist in nested target");
        assert!(target_dir.join(LOG_FILE_NAME).exists(), "log file should exist in nested target");
        assert!(
            !source_dir.join(DATABASE_NAME).exists(),
            "db file should be removed from source"
        );
        assert!(
            !source_dir.join(LOG_FILE_NAME).exists(),
            "log file should be removed from source"
        );
        assert!(unrelated_dir.join("note.txt").exists(), "unrelated file should be preserved");
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
}
