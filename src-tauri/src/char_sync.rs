/// 跨角色设置同步模块
///
/// 基于 `docs/plans/2026-08-30-cross-character-settings-sync-design.md` 设计。
/// 同步对象为 userdata 角色目录下的纯引擎明文文件：
/// - addon.jx3dat（插件启用状态）
/// - custom.dat + custom.dat.addon（聊天自定义）
/// - hotkey.data + hotkey_*.txt（热键索引+定义）
/// - userpreferences.jx3dat（核心：UI设置/技能栏/气场可见性等）
///
/// 不复制：userpreferencesasync.jx3dat（服务器同步标志位）、
/// userpreferences/*.dump（历史崩溃恢复）、CoinShopOutfitData.jx3dat（商城外观）。

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

const BACKUP_SUBDIR: &str = "char-sync-backups";
const MAX_BACKUPS: usize = 10;
const USERDATA_BASE: &str = "userdata";
/// 备份元数据文件名（记录精确的目标角色目录，用于回滚匹配）
const META_FILE: &str = "char-sync-meta.json";

/// 所有同步对象文件（固定文件名，不含 hotkey_*.txt glob）
const ALL_SYNC_FILES: &[&str] = &[
    "addon.jx3dat",
    "custom.dat",
    "custom.dat.addon",
    "hotkey.data",
    "userpreferences.jx3dat",
];

// ─── 数据结构 ───────────────────────────────────────────

/// 一个角色在磁盘上的定位信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleDirInfo {
    pub account_name: String,
    pub region: String,
    pub server: String,
    pub role_name: String,
    /// userdata 下的完整角色目录绝对路径
    pub role_dir: String,
    /// 从 info.jx3dat 解析的 uid（预留字段，MVP 未使用）
    #[allow(dead_code)]
    pub uid: Option<String>,
}

/// 同步选项
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncOptions {
    pub sync_addon_jx3dat: bool,
    pub sync_custom: bool,
    pub sync_hotkey: bool,
    pub sync_userprefs: bool,
    pub overwrite: bool,
}

/// 单次同步结果
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub success: bool,
    pub copied_files: Vec<String>,
    pub skipped_files: Vec<String>,
    pub backup_dir: Option<String>,
    pub error: Option<String>,
}

/// 备份条目（list_char_sync_backups 返回）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub backup_dir: String,
    pub timestamp: String,
    pub target_role: String,
    /// 精确目标角色目录（来自备份元数据；旧备份可能为空，需按角色名降级匹配）
    pub target_role_dir: Option<String>,
    pub file_count: i32,
}

/// 备份元数据（写入每个备份目录，用于精确回滚定位）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupMeta {
    target_role_dir: String,
    target_role_name: String,
    source_role_name: String,
    /// 与备份目录名一致的 yyyyMMdd-HHmmss 时间戳
    timestamp: String,
}

// ─── Tauri 命令 ─────────────────────────────────────────

/// 列出当前游戏目录下所有可同步角色
#[tauri::command]
pub fn list_syncable_roles(game_directory: String) -> Result<Vec<RoleDirInfo>, String> {
    // 补全路径（如 E:\Game\SeasunGame → E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd），
    // 与 detect_accounts_active 等现有命令保持一致
    let game_dir =
        PathBuf::from(crate::game_directory::resolve_game_runtime_directory(&game_directory));
    let userdata_path = game_dir.join(USERDATA_BASE);

    if !userdata_path.exists() {
        return Err(format!(
            "userdata 目录不存在，请先在设置中配置正确的游戏目录: {}",
            userdata_path.display()
        ));
    }

    let mut roles = Vec::new();

    for account_entry in read_dir_entries(&userdata_path)? {
        if !account_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let account_name = account_entry.file_name().to_string_lossy().to_string();

        for region_entry in read_dir_entries(&account_entry.path()).unwrap_or_default() {
            if !region_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let region_name = region_entry.file_name().to_string_lossy().to_string();

            for server_entry in read_dir_entries(&region_entry.path()).unwrap_or_default() {
                if !server_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let server_name = server_entry.file_name().to_string_lossy().to_string();

                for role_entry in read_dir_entries(&server_entry.path()).unwrap_or_default() {
                    if !role_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        continue;
                    }
                    let role_name = role_entry.file_name().to_string_lossy().to_string();
                    let role_dir = role_entry.path();

                    // 仅列出含至少一个同步对象文件的角色目录
                    if has_syncable_files(&role_dir) {
                        roles.push(RoleDirInfo {
                            account_name: account_name.clone(),
                            region: region_name.clone(),
                            server: server_name.clone(),
                            role_name: role_name.clone(),
                            role_dir: role_dir.to_string_lossy().to_string(),
                            uid: None,
                        });
                    }
                }
            }
        }
    }

    log::info!("扫描到 {} 个可同步角色", roles.len());
    Ok(roles)
}

/// 执行跨角色设置同步
#[tauri::command]
pub fn sync_character_settings(
    game_directory: String,
    source: RoleDirInfo,
    target: RoleDirInfo,
    options: SyncOptions,
) -> Result<SyncResult, String> {
    // 补全路径后做安全校验（与 list_syncable_roles 一致）
    let game_dir =
        PathBuf::from(crate::game_directory::resolve_game_runtime_directory(&game_directory));
    let source_dir = PathBuf::from(&source.role_dir);
    let target_dir = PathBuf::from(&target.role_dir);

    // 路径安全校验：源和目标都必须在 game_directory 下
    if !source_dir.starts_with(&game_dir) {
        return Err("源角色目录不在当前游戏目录下，拒绝执行".to_string());
    }
    if !target_dir.starts_with(&game_dir) {
        return Err("目标角色目录不在当前游戏目录下，拒绝执行".to_string());
    }
    if source_dir == target_dir {
        return Err("源角色和目标角色不能相同".to_string());
    }
    if !source_dir.exists() {
        return Err(format!("源角色目录不存在: {}", source.role_dir));
    }
    if !target_dir.exists() {
        return Err(format!(
            "目标角色目录不存在: {}，请先登录目标角色让游戏引擎创建目录",
            target.role_dir
        ));
    }

    // 1. 先确定本次要复制的文件；为空直接拒绝，避免产生无意义的空备份
    let selected_files = build_selected_files(&options);
    if selected_files.is_empty() {
        return Err("请至少勾选一项同步内容".to_string());
    }

    // 2. 备份目标角色目录下的所有同步对象文件，并写入备份元数据
    let (backup_path, backup_timestamp) = get_backup_dir(&target.role_name)?;
    let backup_dir = backup_path.to_string_lossy().to_string();
    match backup_role_dir(&target_dir, &backup_path) {
        Ok(count) => log::info!(
            "已备份目标角色 {} 的 {} 个文件到 {}",
            target.role_name,
            count,
            backup_dir
        ),
        Err(e) => {
            return Err(format!("备份失败，同步中止: {}", e));
        }
    }

    // 写入元数据（精确记录目标角色目录，供回滚时定位）
    let meta = BackupMeta {
        target_role_dir: target.role_dir.clone(),
        target_role_name: target.role_name.clone(),
        source_role_name: source.role_name.clone(),
        timestamp: backup_timestamp,
    };
    if let Err(e) = write_backup_meta(&backup_path, &meta) {
        return Err(format!("备份元数据写入失败，同步中止: {}", e));
    }

    // 清理超出上限的旧备份
    if let Err(e) = cleanup_old_backups() {
        log::warn!("清理旧备份失败: {}", e);
    }

    // 3. 按选项复制源角色文件到目标角色目录
    let mut copied = Vec::new();
    let mut skipped = Vec::new();

    for file_name in &selected_files {
        let src = source_dir.join(file_name);
        let dst = target_dir.join(file_name);

        if !src.exists() {
            skipped.push(file_name.to_string());
            continue;
        }

        if dst.exists() && !options.overwrite {
            skipped.push(file_name.to_string());
            continue;
        }

        match fs::copy(&src, &dst) {
            Ok(_) => copied.push(file_name.to_string()),
            Err(e) => {
                return Ok(SyncResult {
                    success: false,
                    copied_files: copied,
                    skipped_files: skipped,
                    backup_dir: Some(backup_dir.clone()),
                    error: Some(format!("复制 {} 失败: {}", file_name, e)),
                });
            }
        }
    }

    // 4. 如果 sync_hotkey，复制 hotkey_*.txt 文件组
    if options.sync_hotkey {
        for txt_name in collect_hotkey_txt_files(&source_dir) {
            let src = source_dir.join(&txt_name);
            let dst = target_dir.join(&txt_name);

            if dst.exists() && !options.overwrite {
                skipped.push(txt_name);
                continue;
            }

            match fs::copy(&src, &dst) {
                Ok(_) => copied.push(txt_name),
                Err(e) => {
                    return Ok(SyncResult {
                        success: false,
                        copied_files: copied,
                        skipped_files: skipped,
                        backup_dir: Some(backup_dir.clone()),
                        error: Some(format!("复制 {} 失败: {}", txt_name, e)),
                    });
                }
            }
        }
    }

    log::info!(
        "跨角色同步完成: {} → {}，复制 {} 个文件，跳过 {} 个",
        source.role_name,
        target.role_name,
        copied.len(),
        skipped.len()
    );

    Ok(SyncResult {
        success: true,
        copied_files: copied,
        skipped_files: skipped,
        backup_dir: Some(backup_dir),
        error: None,
    })
}

/// 列出本地备份目录（即同步历史，无需 DB）
#[tauri::command]
pub fn list_char_sync_backups() -> Result<Vec<BackupEntry>, String> {
    let backups_root = get_backups_root()?;
    if !backups_root.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();

    for entry in read_dir_entries(&backups_root).unwrap_or_default() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let dir_path = entry.path();

        // 优先读取备份元数据（含精确目标角色目录），旧备份降级解析目录名
        // 目录名格式: <timestamp>-<target_role>，timestamp 格式: yyyyMMdd-HHmmss
        let (timestamp, target_role, target_role_dir) =
            if let Some(meta) = read_backup_meta(&dir_path) {
                (
                    meta.timestamp,
                    meta.target_role_name,
                    Some(meta.target_role_dir),
                )
            } else if let Some((timestamp, target_role)) = parse_backup_dir_name(&dir_name) {
                (timestamp, target_role, None)
            } else {
                continue;
            };

        let file_count = count_files_in_dir(&dir_path) as i32;
        entries.push(BackupEntry {
            backup_dir: dir_path.to_string_lossy().to_string(),
            timestamp,
            target_role,
            target_role_dir,
            file_count,
        });
    }

    // 按时间戳降序排列（最新的在前）
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    log::info!("扫描到 {} 个同步备份", entries.len());
    Ok(entries)
}

/// 从备份回滚目标角色设置
#[tauri::command]
pub fn rollback_char_sync(
    game_directory: String,
    backup_dir: String,
    target_role_dir: String,
) -> Result<(), String> {
    let backup_path = PathBuf::from(&backup_dir);
    let target_path = PathBuf::from(&target_role_dir);

    // 路径安全校验：目标角色目录必须在当前游戏目录下（补全路径后比较，与 sync 命令一致）
    let game_dir =
        PathBuf::from(crate::game_directory::resolve_game_runtime_directory(&game_directory));
    if !target_path.starts_with(&game_dir) {
        return Err("目标角色目录不在当前游戏目录下，拒绝执行".to_string());
    }

    // 路径安全校验：备份目录必须位于本模块管理的备份根目录下，
    // 避免传入任意目录后被整体复制进角色目录
    let backups_root = get_backups_root()?;
    if !backup_path.starts_with(&backups_root) {
        return Err(format!(
            "备份目录不在受管的备份目录下，拒绝执行: {}",
            backup_dir
        ));
    }

    if !backup_path.exists() {
        return Err(format!("备份目录不存在: {}", backup_dir));
    }
    if !target_path.exists() {
        return Err(format!(
            "目标角色目录不存在: {}",
            target_role_dir
        ));
    }

    // 从备份复制所有文件回目标角色目录（覆盖，跳过元数据文件）
    let mut restored = 0;
    for entry in read_dir_entries(&backup_path).unwrap_or_default() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if entry.file_name().to_string_lossy() == META_FILE {
            continue;
        }
        let file_name = entry.file_name();
        let dst = target_path.join(&file_name);
        match fs::copy(entry.path(), &dst) {
            Ok(_) => restored += 1,
            Err(e) => {
                return Err(format!(
                    "回滚文件 {} 失败: {}",
                    file_name.to_string_lossy(),
                    e
                ));
            }
        }
    }

    log::info!("回滚完成: 从 {} 恢复 {} 个文件到 {}", backup_dir, restored, target_role_dir);
    Ok(())
}

// ─── 辅助函数 ──────────────────────────────────────────

/// 读取目录条目（仿 game_directory.rs 的 read_directory_entries）
fn read_dir_entries(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    fs::read_dir(path)
        .map_err(|e| format!("读取目录失败: {} ({})", path.display(), e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("遍历目录失败: {} ({})", path.display(), e))
}

/// 检查角色目录是否含至少一个同步对象文件
fn has_syncable_files(role_dir: &Path) -> bool {
    for file in ALL_SYNC_FILES {
        if role_dir.join(file).exists() {
            return true;
        }
    }
    // 检查 hotkey_*.txt
    !collect_hotkey_txt_files(role_dir).is_empty()
}

/// 收集 hotkey_*.txt 文件名（已排序）
fn collect_hotkey_txt_files(dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("hotkey_") && name.ends_with(".txt") {
                files.push(name);
            }
        }
    }
    files.sort();
    files
}

/// 根据 SyncOptions 构建要复制的固定文件名列表（不含 hotkey_*.txt glob）
fn build_selected_files(options: &SyncOptions) -> Vec<&'static str> {
    let mut files = Vec::new();
    if options.sync_addon_jx3dat {
        files.push("addon.jx3dat");
    }
    if options.sync_custom {
        files.push("custom.dat");
        files.push("custom.dat.addon");
    }
    if options.sync_hotkey {
        files.push("hotkey.data");
    }
    if options.sync_userprefs {
        files.push("userpreferences.jx3dat");
    }
    files
}

/// 备份目标角色目录下的所有同步对象文件
fn backup_role_dir(target_dir: &Path, backup_dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(backup_dir).map_err(|e| format!("创建备份目录失败: {}", e))?;

    let mut count = 0;

    for file in ALL_SYNC_FILES {
        let src = target_dir.join(file);
        if src.exists() {
            let dst = backup_dir.join(file);
            fs::copy(&src, &dst)
                .map_err(|e| format!("备份 {} 失败: {}", file, e))?;
            count += 1;
        }
    }

    // 备份 hotkey_*.txt
    for name in collect_hotkey_txt_files(target_dir) {
        let src = target_dir.join(&name);
        let dst = backup_dir.join(&name);
        fs::copy(&src, &dst)
            .map_err(|e| format!("备份 {} 失败: {}", name, e))?;
        count += 1;
    }

    Ok(count)
}

/// 写入备份元数据
fn write_backup_meta(backup_dir: &Path, meta: &BackupMeta) -> Result<(), String> {
    let json = serde_json::to_string_pretty(meta)
        .map_err(|e| format!("序列化备份元数据失败: {}", e))?;
    fs::write(backup_dir.join(META_FILE), json)
        .map_err(|e| format!("写入 {} 失败: {}", META_FILE, e))
}

/// 读取备份元数据（不存在或损坏时返回 None，由调用方降级）
fn read_backup_meta(backup_dir: &Path) -> Option<BackupMeta> {
    let content = fs::read_to_string(backup_dir.join(META_FILE)).ok()?;
    serde_json::from_str(&content).ok()
}

/// 获取备份根目录
fn get_backups_root() -> Result<PathBuf, String> {
    let app_dir = crate::db::get_app_dir()?;
    Ok(app_dir.join(BACKUP_SUBDIR))
}

/// 获取本次备份目录路径与时间戳: <backups_root>/<timestamp>-<target_role>/
fn get_backup_dir(target_role: &str) -> Result<(PathBuf, String), String> {
    let root = get_backups_root()?;
    let timestamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    // 角色名可能含特殊字符，做简单清洗
    let safe_role = target_role
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let dir_name = format!("{}-{}", timestamp, safe_role);
    let mut dir = root.join(&dir_name);
    // 同一秒内对同一目标角色重复同步会命中同一目录名，追加序号避免两次备份合并
    let mut seq = 2;
    while dir.exists() {
        dir = root.join(format!("{}-{}", dir_name, seq));
        seq += 1;
    }
    Ok((dir, timestamp))
}

/// 清理超出上限的旧备份（保留最近 MAX_BACKUPS 个）
fn cleanup_old_backups() -> Result<(), String> {
    let root = get_backups_root()?;
    if !root.exists() {
        return Ok(());
    }

    let mut dirs: Vec<(PathBuf, String)> = Vec::new();
    for entry in read_dir_entries(&root).unwrap_or_default() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some((timestamp, _)) = parse_backup_dir_name(&name) {
            dirs.push((entry.path(), timestamp));
        }
    }

    if dirs.len() <= MAX_BACKUPS {
        return Ok(());
    }

    // 按时间戳升序排列（最旧的在前面）
    dirs.sort_by(|a, b| a.1.cmp(&b.1));

    let to_remove = dirs.len() - MAX_BACKUPS;
    for (path, ts) in dirs.iter().take(to_remove) {
        if let Err(e) = fs::remove_dir_all(path) {
            log::warn!("删除旧备份失败: {} ({})", path.display(), e);
        } else {
            log::info!("已清理旧备份: {} ({})", path.display(), ts);
        }
    }

    Ok(())
}

/// 解析备份目录名: "<timestamp>-<target_role>"
/// timestamp 格式: yyyyMMdd-HHmmss
///
/// 注意：目录名可能含多字节字符（如中文角色名/用户自建目录），
/// 因此一律用 `get()` 取切片，非字符边界时返回 None，避免 panic。
fn parse_backup_dir_name(name: &str) -> Option<(String, String)> {
    // 时间戳部分为前 15 个字符（yyyyMMdd-HHmmss）
    if name.len() < 17 {
        return None;
    }
    let timestamp = name.get(..15)?;
    // 校验时间戳格式
    if timestamp.chars().nth(8) != Some('-') {
        return None;
    }
    let prefix = timestamp.get(..8)?;
    let suffix = timestamp.get(9..)?;
    if !prefix.chars().all(|c| c.is_ascii_digit())
        || !suffix.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    // 第 16 个字符是分隔符 '-'
    if name.as_bytes()[15] != b'-' {
        return None;
    }
    let target_role = name.get(16..)?;
    if target_role.is_empty() {
        return None;
    }
    Some((timestamp.to_string(), target_role.to_string()))
}

/// 统计目录中的文件数（不含元数据文件）
fn count_files_in_dir(dir: &Path) -> usize {
    read_dir_entries(dir)
        .unwrap_or_default()
        .iter()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter(|e| e.file_name().to_string_lossy() != META_FILE)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_backup_dir_name_normal() {
        let (ts, role) = parse_backup_dir_name("20260906-120000-测试角色").unwrap();
        assert_eq!(ts, "20260906-120000");
        assert_eq!(role, "测试角色");
    }

    #[test]
    fn test_parse_backup_dir_name_with_seq_suffix() {
        let (ts, role) = parse_backup_dir_name("20260906-120000-角色A-2").unwrap();
        assert_eq!(ts, "20260906-120000");
        assert_eq!(role, "角色A-2");
    }

    /// 目录名前 15 字节落在多字节字符中间时，必须安全返回 None 而不是 panic
    #[test]
    fn test_parse_backup_dir_name_multibyte_boundary_no_panic() {
        // "12345678中文名" 共 17 字节，第 15 字节位于「名」的第二个字节
        assert!(parse_backup_dir_name("12345678中文名").is_none());
        // 全中文目录名
        assert!(parse_backup_dir_name("中文目录名中文目录名").is_none());
    }

    #[test]
    fn test_parse_backup_dir_name_rejects_invalid() {
        assert!(parse_backup_dir_name("short").is_none());
        assert!(parse_backup_dir_name("abcdefgh-ijklmn-role").is_none());
        assert!(parse_backup_dir_name("20260906-120000-").is_none());
    }
}
