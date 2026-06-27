use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::game_directory::MINGYI_ACCOUNTS_BASE_PATH;
use crate::jx3_process;

/// 活跃等级
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccountActiveLevel {
    /// 当前正在操作（最新 mtime 在 5 分钟内）
    Active,
    /// 本次会话登录过（最新 mtime 在进程时间窗口内但 > 5 分钟）
    Recent,
    /// 本次会话未登录（JX3 运行但该账号 mtime 早于进程启动时间）
    Idle,
    /// JX3 进程未运行
    Offline,
}

impl AccountActiveLevel {
    pub fn is_online(&self) -> bool {
        !matches!(self, Self::Offline)
    }
    pub fn is_recently_active(&self) -> bool {
        matches!(self, Self::Active | Self::Recent)
    }
}

/// 从 info.jx3dat 解析出的角色身份信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MingyiRoleIdentity {
    pub uid: String,
    pub role_name: String,
    pub server: String,
    pub region: String,
}

/// 角色活跃状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleActiveState {
    /// 茗伊 uid（目录名中的数字 ID）
    pub uid: String,
    /// 角色名（从 info.jx3dat 解析）
    pub role_name: String,
    /// 服务器（从 info.jx3dat 解析）
    pub server: String,
    /// 大区（从 info.jx3dat 解析）
    pub region: String,
    pub active_level: AccountActiveLevel,
    pub is_online: bool,
    pub is_recently_active: bool,
    /// 最新活动时间（RFC3339 格式）
    pub last_activity_time: Option<String>,
    /// 最新活动数据源描述
    pub last_activity_source: Option<String>,
}

/// 批量活跃检测的结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchActiveResult {
    pub jx3_running: bool,
    /// JX3 进程启动时间（RFC3339）
    pub jx3_start_time: Option<String>,
    /// JX3 进程结束时间（运行中为 None）
    pub jx3_end_time: Option<String>,
    pub jx3_process_count: u32,
    pub multi_instance_detected: bool,
    pub multi_instance_hint: Option<String>,
    /// 所有扫描到的角色活跃状态
    pub roles: Vec<RoleActiveState>,
    /// 扫描耗时（毫秒）
    pub scan_duration_ms: u64,
}

/// 获取文件或目录的 mtime（SystemTime）
fn get_path_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
}

/// 获取目录中最新文件的 mtime（用于 combat_logs/ 和 gkp/ 目录）
///
/// - `dir`: 目标目录
/// - `extension`: 文件扩展名过滤（如 "jcl"、"gkp.jx3dat"）；None 表示不过滤
/// - `window_start`: 时间窗口下限（仅返回窗口内的 mtime）
/// - `window_end`: 时间窗口上限
fn get_latest_file_mtime_in_dir(
    dir: &Path,
    extension_filter: Option<&str>,
    window_start: SystemTime,
    window_end: SystemTime,
) -> Option<SystemTime> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut latest: Option<SystemTime> = None;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(ext) = extension_filter {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !file_name.ends_with(ext) {
                continue;
            }
        }
        if let Some(mtime) = get_path_mtime(&path) {
            if mtime >= window_start && mtime <= window_end {
                latest = Some(match latest {
                    Some(current) if current > mtime => current,
                    _ => mtime,
                });
            }
        }
    }
    latest
}

/// 账号活动扫描结果（一次扫描得到两个 mtime）
#[derive(Debug, Clone)]
pub struct AccountActivityScan {
    /// 全量数据源最新 mtime（含 info.jx3dat 登录信号），用于显示最后活动时间
    pub latest_all: Option<(SystemTime, String)>,
    /// 运行时数据源最新 mtime（排除 info.jx3dat），用于排名判定当前是否仍在线
    pub latest_runtime: Option<(SystemTime, String)>,
}

/// 一次扫描账号目录的所有数据源，同时返回全量 mtime 和运行时 mtime
///
/// 数据源（实测验证）：
/// 1. `info.jx3dat` — 登录信号（长安记银方案核心，明文可直接读取 mtime）
///    - 仅计入 `latest_all`，不计入 `latest_runtime`
///    - 原因：info.jx3dat 只在登录时写入一次，退出时不更新，
///      若参与排名会让"刚退出但近期登录过"的角色误判为 active
/// 2. `userdata/userdata.db` — 茗伊实时数据库（在线时持续更新）
/// 3. `userdata/chat_log/` 目录 mtime + 最新 chatlog_*.v2.db 文件 mtime — 聊天日志信号
/// 4. `userdata/gkp/` 目录 mtime + 最新 .gkp.jx3dat — 收支信号
/// 5. `userdata/combat_logs/` 目录 mtime + 最新 .jcl — 副本信号（可能不存在）
/// 6. 账号目录本身 — 兜底
///
/// 数据源 2-6 同时计入 `latest_all` 和 `latest_runtime`
///
/// 性能优化：先检查 info.jx3dat + userdata.db 的 mtime（仅 stat，无 read_dir），
/// 若两者都不在时间窗口内（说明本会话未登录该账号），跳过 chat_log/gkp/combat_logs
/// 的 read_dir 文件遍历（性能瓶颈），仅保留账号目录兜底 stat。
/// 实测：29 个账号目录从 3.7s 降至 <0.5s（27 个未登录账号快速跳过）。
pub fn scan_account_activity(
    account_dir: &Path,
    window_start: SystemTime,
    window_end: SystemTime,
) -> AccountActivityScan {
    let mut all_candidates: Vec<(SystemTime, String)> = Vec::new();
    let mut runtime_candidates: Vec<(SystemTime, String)> = Vec::new();

    let in_window = |m: &SystemTime| *m >= window_start && *m <= window_end;

    // 数据源 1: info.jx3dat（登录信号，仅计入 latest_all）
    let info_file = account_dir.join("info.jx3dat");
    let info_mtime = get_path_mtime(&info_file);
    let info_in_window = info_mtime.as_ref().map(in_window).unwrap_or(false);
    if let Some(m) = info_mtime {
        if info_in_window {
            all_candidates.push((m, "info.jx3dat".to_string()));
        }
    }

    // 数据源 2: userdata/userdata.db（茗伊实时数据库，在线时持续更新）
    let userdata_db = account_dir.join("userdata").join("userdata.db");
    let userdata_mtime = get_path_mtime(&userdata_db);
    let userdata_in_window = userdata_mtime.as_ref().map(in_window).unwrap_or(false);
    if let Some(m) = userdata_mtime {
        if userdata_in_window {
            let entry = (m, "userdata.db".to_string());
            all_candidates.push(entry.clone());
            runtime_candidates.push(entry);
        }
    }

    // 性能优化：若 info.jx3dat 和 userdata.db 都不在窗口内，
    // 说明本会话未登录该账号，跳过 read_dir 文件遍历（性能瓶颈）
    let need_full_scan = info_in_window || userdata_in_window;

    if need_full_scan {
        // 数据源 3: userdata/chat_log/ 目录 mtime + 最新 chatlog_*.v2.db 文件 mtime
        let chat_log_dir = account_dir.join("userdata").join("chat_log");
        let chat_log_dir_mtime = get_path_mtime(&chat_log_dir).filter(in_window);
        let latest_chatlog_db_mtime = get_latest_file_mtime_in_dir(
            &chat_log_dir,
            Some(".v2.db"),
            window_start,
            window_end,
        );
        let chat_log_candidate = match (chat_log_dir_mtime, latest_chatlog_db_mtime) {
            (Some(dir_m), Some(db_m)) => Some(if dir_m > db_m { dir_m } else { db_m }),
            (Some(m), None) | (None, Some(m)) => Some(m),
            (None, None) => None,
        };
        if let Some(m) = chat_log_candidate {
            let entry = (m, "chat_log/".to_string());
            all_candidates.push(entry.clone());
            runtime_candidates.push(entry);
        }

        // 数据源 4: userdata/gkp/ 目录 mtime + 最新 .gkp.jx3dat mtime
        let gkp_dir = account_dir.join("userdata").join("gkp");
        let gkp_dir_mtime = get_path_mtime(&gkp_dir).filter(in_window);
        let latest_gkp_mtime = get_latest_file_mtime_in_dir(
            &gkp_dir,
            Some(".gkp.jx3dat"),
            window_start,
            window_end,
        );
        let gkp_candidate = match (gkp_dir_mtime, latest_gkp_mtime) {
            (Some(dir_m), Some(gkp_m)) => Some(if dir_m > gkp_m { dir_m } else { gkp_m }),
            (Some(m), None) | (None, Some(m)) => Some(m),
            (None, None) => None,
        };
        if let Some(m) = gkp_candidate {
            let entry = (m, "gkp/".to_string());
            all_candidates.push(entry.clone());
            runtime_candidates.push(entry);
        }

        // 数据源 5: userdata/combat_logs/ 目录 mtime + 最新 .jcl mtime
        let combat_logs_dir = account_dir.join("userdata").join("combat_logs");
        let combat_logs_mtime = get_path_mtime(&combat_logs_dir).filter(in_window);
        let latest_jcl_mtime = get_latest_file_mtime_in_dir(
            &combat_logs_dir,
            Some(".jcl"),
            window_start,
            window_end,
        );
        let combat_logs_candidate = match (combat_logs_mtime, latest_jcl_mtime) {
            (Some(dir_m), Some(jcl_m)) => Some(if dir_m > jcl_m { dir_m } else { jcl_m }),
            (Some(m), None) | (None, Some(m)) => Some(m),
            (None, None) => None,
        };
        if let Some(m) = combat_logs_candidate {
            let entry = (m, "combat_logs/".to_string());
            all_candidates.push(entry.clone());
            runtime_candidates.push(entry);
        }
    }

    // 数据源 6: 账号目录本身（兜底，仅 stat）
    if let Some(m) = get_path_mtime(account_dir) {
        if in_window(&m) {
            let entry = (m, "account_dir".to_string());
            all_candidates.push(entry.clone());
            runtime_candidates.push(entry);
        }
    }

    AccountActivityScan {
        latest_all: all_candidates.into_iter().max_by_key(|(m, _)| *m),
        latest_runtime: runtime_candidates.into_iter().max_by_key(|(m, _)| *m),
    }
}

/// 从 Lua return 格式的 info.jx3dat 中提取字段值
///
/// 文件格式示例：
/// ```text
/// return {region="电信五区",uid="432345564243886337",name="角色名",server="梦江南",...}
/// ```
///
/// 提取 `key="value"` 中的 value 部分
fn extract_lua_string_field(text: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = text.find(&pattern)? + pattern.len();
    let rest = &text[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// 解析 info.jx3dat 文件，提取角色身份信息
///
/// info.jx3dat 为 GBK 编码的 Lua return 语句（XOR key 全0，明文可读）
fn parse_info_jx3dat(path: &Path) -> Option<MingyiRoleIdentity> {
    let bytes = std::fs::read(path).ok()?;
    // GBK 解码（茗伊文件使用 GBK 编码）
    let (text, _, had_errors) = encoding_rs::GBK.decode(&bytes);
    if had_errors {
        log::warn!("[ActiveDetector] info.jx3dat GBK 解码存在异常: {:?}", path);
    }
    let text = text.as_ref();

    let uid = extract_lua_string_field(text, "uid")?;
    let role_name = extract_lua_string_field(text, "name").unwrap_or_default();
    let server = extract_lua_string_field(text, "server").unwrap_or_default();
    let region = extract_lua_string_field(text, "region").unwrap_or_default();

    Some(MingyiRoleIdentity {
        uid,
        role_name,
        server,
        region,
    })
}

/// 判断目录名是否为茗伊账号目录（格式：`{uid}@zhcn_hd`）
///
/// 排除特殊目录：
/// - `!all-users@zhcn_hd`（全局共享目录）
/// - `#cache`（缓存目录）
/// - `#电信区_梦江南@zhcn_hd` 等（以 # 开头但包含下划线的区服目录）
fn is_mingyi_account_dir(dir_name: &str) -> bool {
    if !dir_name.ends_with("@zhcn_hd") {
        return false;
    }
    // 排除 ! 开头的特殊目录
    if dir_name.starts_with('!') {
        return false;
    }
    // 排除 # 开头的区服目录（如 #电信区_梦江南@zhcn_hd）
    if dir_name.starts_with('#') {
        return false;
    }
    true
}

/// 将 SystemTime 转为 RFC3339 字符串
fn system_time_to_rfc3339(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs() as i64;
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(secs, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// 将 UNIX 时间戳（秒）转为 SystemTime
fn unix_to_system_time(secs: u64) -> SystemTime {
    UNIX_EPOCH + std::time::Duration::from_secs(secs)
}

/// 计算活跃等级
///
/// - `latest_mtime`: 账号最新活动时间
/// - `window_start`: 进程启动时间（窗口下限）
/// - `window_end`: 当前时间（窗口上限）
/// - `jx3_running`: JX3 进程是否运行
fn determine_active_level(
    latest_mtime: Option<SystemTime>,
    window_start: SystemTime,
    window_end: SystemTime,
    jx3_running: bool,
) -> AccountActiveLevel {
    if !jx3_running {
        return AccountActiveLevel::Offline;
    }
    let mtime = match latest_mtime {
        Some(t) => t,
        None => return AccountActiveLevel::Idle,
    };

    // mtime 早于窗口下限（JX3 运行但本次会话未登录此账号）
    if mtime < window_start {
        return AccountActiveLevel::Idle;
    }

    // 只要 mtime 在进程窗口内（本次会话登录过），即为 active
    // 不再用 5 分钟区分 active/recent：用户视角下"在线即活跃"
    let _ = window_end;
    AccountActiveLevel::Active
}

/// 扫描所有茗伊账号目录并检测活跃状态
///
/// 性能优化：
/// 1. 先获取 JX3 进程状态，若无匹配进程直接返回（不扫描任何文件）
/// 2. 扫描 `interface/my#data/` 下所有 `{uid}@zhcn_hd` 目录
/// 3. 每个目录读取 info.jx3dat 解析角色身份 + 检测 6 个数据源 mtime
pub fn detect_accounts_active_internal(game_directory: &str) -> BatchActiveResult {
    let scan_start = SystemTime::now();

    // 1. 获取 JX3 进程状态
    let runtime_status = jx3_process::get_jx3_runtime_status_internal(game_directory);

    // 2. 若无匹配进程，所有角色 offline
    if !runtime_status.is_running {
        let roles: Vec<RoleActiveState> = Vec::new();
        let scan_duration_ms = scan_start
            .elapsed()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        return BatchActiveResult {
            jx3_running: false,
            jx3_start_time: None,
            jx3_end_time: None,
            jx3_process_count: runtime_status.all_processes.len() as u32,
            multi_instance_detected: runtime_status.multi_instance_detected,
            multi_instance_hint: runtime_status.multi_instance_hint,
            roles,
            scan_duration_ms,
        };
    }

    // 3. 规划时间窗口 [process_start, now]
    let matched = runtime_status.matched_process.as_ref().unwrap();
    let window_start = unix_to_system_time(matched.start_time_unix);
    let window_end = SystemTime::now();
    let jx3_start_time = Some(matched.start_time.clone());
    // 进程运行中，无结束时间
    let jx3_end_time: Option<String> = None;

    // 4. 扫描所有茗伊账号目录
    let game_dir = PathBuf::from(game_directory);
    let accounts_base = game_dir.join(MINGYI_ACCOUNTS_BASE_PATH);

    let mut roles: Vec<RoleActiveState> = Vec::new();
    // 收集每个角色的 latest_runtime_mtime（用于排名判定当前在线角色）
    // 排除 info.jx3dat：它只在登录时写入一次，退出时不更新，
    // 若参与排名会导致"刚退出但近期登录过的角色"误判为 active
    let mut latest_runtime_mtimes: Vec<Option<SystemTime>> = Vec::new();

    let entries = match std::fs::read_dir(&accounts_base) {
        Ok(e) => e,
        Err(err) => {
            log::warn!(
                "[ActiveDetector] 读取茗伊账号目录失败: {:?} => {}",
                accounts_base,
                err
            );
            let scan_duration_ms = scan_start
                .elapsed()
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            return BatchActiveResult {
                jx3_running: true,
                jx3_start_time,
                jx3_end_time,
                jx3_process_count: runtime_status.all_processes.len() as u32,
                multi_instance_detected: runtime_status.multi_instance_detected,
                multi_instance_hint: runtime_status.multi_instance_hint,
                roles,
                scan_duration_ms,
            };
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !is_mingyi_account_dir(&dir_name) {
            continue;
        }

        // 解析 info.jx3dat 获取角色身份
        let identity = parse_info_jx3dat(&path.join("info.jx3dat"));

        // 提取 uid（目录名去掉 @zhcn_hd 后缀）
        let uid = dir_name.trim_end_matches("@zhcn_hd").to_string();

        let (role_name, server, region) = match &identity {
            Some(id) => (id.role_name.clone(), id.server.clone(), id.region.clone()),
            None => (String::new(), String::new(), String::new()),
        };

        // 一次扫描所有数据源，同时获取 latest_all（含 info.jx3dat，用于显示）
        // 和 latest_runtime（排除 info.jx3dat，用于排名判定当前是否仍在线）
        let scan = scan_account_activity(&path, window_start, window_end);
        let (latest_mtime, source) = scan
            .latest_all
            .map(|(t, s)| (Some(t), Some(s)))
            .unwrap_or((None, None));
        let latest_runtime_mtime = scan.latest_runtime.map(|(t, _)| t);
        latest_runtime_mtimes.push(latest_runtime_mtime);

        let active_level = determine_active_level(latest_mtime, window_start, window_end, true);
        let last_activity_time = latest_mtime.map(system_time_to_rfc3339);

        roles.push(RoleActiveState {
            uid,
            role_name,
            server,
            region,
            active_level,
            is_online: active_level.is_online(),
            is_recently_active: active_level.is_recently_active(),
            last_activity_time,
            last_activity_source: source,
        });
    }

    // 重新判定：取最新的 N 个 latest_runtime_mtime 角色为当前在线（active）
    // N = 匹配的 JX3 进程数（多开时每个进程对应一个当前在线角色）
    //
    // 用 latest_runtime_mtime（排除 info.jx3dat）排名：
    // - 切换角色：新角色数据源持续更新，旧角色停止 → 新角色排前
    // - 退出角色：退出角色数据源不再更新，仍在线角色持续更新 → 在线角色排前
    //   （关键：info.jx3dat 只在登录时写入一次，退出时不更新，
    //    若参与排名会让"刚退出但近期登录过"的角色误判为 active）
    // - 多开：多个角色都在线，latest_runtime_mtime 都持续更新 → 都排前
    let n = runtime_status.matched_process_count as usize;
    let mut sorted_mtimes: Vec<SystemTime> = latest_runtime_mtimes.iter().filter_map(|&m| m).collect();
    sorted_mtimes.sort_unstable_by(|a, b| b.cmp(a)); // 降序
    let active_threshold = sorted_mtimes.get(n.saturating_sub(1)).copied();

    if let Some(threshold) = active_threshold {
        for (i, role) in roles.iter_mut().enumerate() {
            let is_active = latest_runtime_mtimes
                .get(i)
                .copied()
                .flatten()
                .map(|m| m >= threshold)
                .unwrap_or(false);
            if !is_active && role.active_level == AccountActiveLevel::Active {
                // 不是最新 N 个的角色，降级为 idle（已切换走或已退出）
                role.active_level = AccountActiveLevel::Idle;
                role.is_online = false;
                role.is_recently_active = false;
            }
        }
    }

    let scan_duration_ms = scan_start
        .elapsed()
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    BatchActiveResult {
        jx3_running: true,
        jx3_start_time,
        jx3_end_time,
        jx3_process_count: runtime_status.all_processes.len() as u32,
        multi_instance_detected: runtime_status.multi_instance_detected,
        multi_instance_hint: runtime_status.multi_instance_hint,
        roles,
        scan_duration_ms,
    }
}

/// 缓存的活跃检测结果
struct CachedActiveResult {
    result: BatchActiveResult,
    scanned_at: Instant,
    game_directory: String,
}

/// 全局缓存（OnceLock 初始化一次，Mutex 保护读写）
static CACHED_RESULT: OnceLock<Mutex<Option<CachedActiveResult>>> = OnceLock::new();

fn get_cache() -> &'static Mutex<Option<CachedActiveResult>> {
    CACHED_RESULT.get_or_init(|| Mutex::new(None))
}

/// 缓存有效期（秒）
///
/// 设为 25 秒：前端轮询间隔 30 秒，每次调用时缓存已过期，
/// 触发后台刷新并返回旧缓存，前端永远不阻塞等待扫描。
const CACHE_TTL_SECS: u64 = 25;

/// 批量检测账号活跃状态（Tauri 命令）
///
/// 采用"后台扫描 + 缓存"策略，确保前端 invoke 永远不阻塞：
/// - 缓存有效（<25s 且 game_directory 相同）：直接返回缓存（<1ms）
/// - 缓存过期但有旧结果：返回旧结果（<1ms），后台 spawn_blocking 刷新
/// - 无缓存（首次）：spawn_blocking 同步扫描，await 等待
///
/// 注意：命令不再接收 account_ids 参数，改为扫描所有茗伊目录。
/// 调用方通过角色名+服务器匹配数据库角色，聚合到账号级别。
#[tauri::command]
pub async fn detect_accounts_active(game_directory: String) -> Result<BatchActiveResult, String> {
    // 1. 检查缓存是否有效
    {
        let cache = get_cache().lock().unwrap();
        if let Some(cached) = &*cache {
            let age = Instant::now().duration_since(cached.scanned_at);
            if age.as_secs() < CACHE_TTL_SECS && cached.game_directory == game_directory {
                return Ok(cached.result.clone());
            }
        }
    }

    // 2. 缓存过期或不存在。检查是否有旧缓存可先返回
    let old_result = {
        let cache = get_cache().lock().unwrap();
        cache
            .as_ref()
            .filter(|c| c.game_directory == game_directory)
            .map(|c| c.result.clone())
    };

    if let Some(result) = old_result {
        // 有旧缓存：后台刷新，立即返回旧结果
        let dir = game_directory.clone();
        tokio::task::spawn_blocking(move || {
            let result = detect_accounts_active_internal(&dir);
            let mut cache = get_cache().lock().unwrap();
            *cache = Some(CachedActiveResult {
                result,
                scanned_at: Instant::now(),
                game_directory: dir,
            });
        });
        return Ok(result);
    }

    // 3. 无缓存（首次或 game_directory 变化）：同步扫描
    let dir = game_directory.clone();
    let result = tokio::task::spawn_blocking(move || detect_accounts_active_internal(&dir))
        .await
        .map_err(|e| format!("扫描任务失败: {}", e))?;

    let mut cache = get_cache().lock().unwrap();
    *cache = Some(CachedActiveResult {
        result: result.clone(),
        scanned_at: Instant::now(),
        game_directory,
    });

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_active_level_offline() {
        let now = SystemTime::now();
        let start = now - std::time::Duration::from_secs(3600);
        let level = determine_active_level(None, start, now, false);
        assert_eq!(level, AccountActiveLevel::Offline);
    }

    #[test]
    fn test_determine_active_level_active() {
        let now = SystemTime::now();
        let start = now - std::time::Duration::from_secs(3600);
        let mtime = now - std::time::Duration::from_secs(60); // 1 分钟前
        let level = determine_active_level(Some(mtime), start, now, true);
        assert_eq!(level, AccountActiveLevel::Active);
    }

    #[test]
    fn test_determine_active_level_active_long_ago() {
        // 只要 mtime 在进程窗口内即为 active（不再因 >5 分钟降级为 recent）
        let now = SystemTime::now();
        let start = now - std::time::Duration::from_secs(3600);
        let mtime = now - std::time::Duration::from_secs(600); // 10 分钟前，但在窗口内
        let level = determine_active_level(Some(mtime), start, now, true);
        assert_eq!(level, AccountActiveLevel::Active);
    }

    #[test]
    fn test_determine_active_level_idle() {
        let now = SystemTime::now();
        let start = now - std::time::Duration::from_secs(3600);
        let mtime = start - std::time::Duration::from_secs(3600); // 早于窗口下限
        let level = determine_active_level(Some(mtime), start, now, true);
        assert_eq!(level, AccountActiveLevel::Idle);
    }

    #[test]
    fn test_determine_active_level_idle_when_no_mtime() {
        let now = SystemTime::now();
        let start = now - std::time::Duration::from_secs(3600);
        let level = determine_active_level(None, start, now, true);
        assert_eq!(level, AccountActiveLevel::Idle);
    }

    #[test]
    fn test_active_level_methods() {
        assert!(AccountActiveLevel::Active.is_online());
        assert!(AccountActiveLevel::Active.is_recently_active());
        assert!(AccountActiveLevel::Recent.is_online());
        assert!(AccountActiveLevel::Recent.is_recently_active());
        assert!(AccountActiveLevel::Idle.is_online());
        assert!(!AccountActiveLevel::Idle.is_recently_active());
        assert!(!AccountActiveLevel::Offline.is_online());
        assert!(!AccountActiveLevel::Offline.is_recently_active());
    }

    #[test]
    fn test_is_mingyi_account_dir() {
        // 正常的 uid 目录
        assert!(is_mingyi_account_dir("432345564243886337@zhcn_hd"));
        assert!(is_mingyi_account_dir("810647932957028033@zhcn_hd"));

        // 排除 !all-users
        assert!(!is_mingyi_account_dir("!all-users@zhcn_hd"));

        // 排除 # 开头的区服目录
        assert!(!is_mingyi_account_dir("#电信区_梦江南@zhcn_hd"));
        assert!(!is_mingyi_account_dir("#双线区_天鹅坪@zhcn_hd"));

        // 排除 #cache
        assert!(!is_mingyi_account_dir("#cache"));

        // 非 @zhcn_hd 后缀
        assert!(!is_mingyi_account_dir("some_dir"));
        assert!(!is_mingyi_account_dir("432345564243886337@zhcn"));
    }

    #[test]
    fn test_extract_lua_string_field() {
        let text = r#"return {region="电信五区",uid="432345564243886337",name="角色名",server="梦江南"}"#;
        assert_eq!(
            extract_lua_string_field(text, "uid"),
            Some("432345564243886337".to_string())
        );
        assert_eq!(
            extract_lua_string_field(text, "name"),
            Some("角色名".to_string())
        );
        assert_eq!(
            extract_lua_string_field(text, "server"),
            Some("梦江南".to_string())
        );
        assert_eq!(
            extract_lua_string_field(text, "region"),
            Some("电信五区".to_string())
        );
        // 不存在的字段
        assert_eq!(extract_lua_string_field(text, "nonexistent"), None);
    }

    #[test]
    fn test_detect_accounts_active_no_process() {
        // JX3 未运行时应返回空角色列表
        let result = detect_accounts_active_internal("E:\\nonexistent\\path");
        assert!(!result.jx3_running);
        assert!(result.roles.is_empty());
    }

    #[test]
    fn test_system_time_to_rfc3339() {
        let now = SystemTime::now();
        let rfc = system_time_to_rfc3339(now);
        assert!(!rfc.is_empty());
        // 应能解析回 DateTime
        assert!(chrono::DateTime::parse_from_rfc3339(&rfc).is_ok());
    }
}
