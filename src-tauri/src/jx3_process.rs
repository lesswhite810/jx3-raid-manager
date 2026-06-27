use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

/// JX3 实际进程名（实测确认，Windows 上保留原始大小写）
const JX3_PROCESS_NAME: &str = "JX3ClientX64.exe";

/// 全局 System 实例缓存（性能优化关键）
///
/// 首次调用用 `System::new()` 创建空实例（不加载任何信息）；
/// 后续调用复用同一实例，避免重复分配 Process 列表内存。
static SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn get_system() -> &'static Mutex<System> {
    SYSTEM.get_or_init(|| Mutex::new(System::new()))
}

/// 将 UNIX 时间戳（秒）转为 RFC3339 格式字符串（本地时区）
fn unix_seconds_to_rfc3339(seconds: u64) -> String {
    use chrono::TimeZone;
    chrono::Local
        .timestamp_opt(seconds as i64, 0)
        .single()
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

/// JX3 进程信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Jx3ProcessInfo {
    pub pid: u32,
    pub name: String,
    /// 进程启动时间（RFC3339 格式，本地时区）
    pub start_time: String,
    /// UNIX 时间戳（秒），用于内部时间窗口计算
    pub start_time_unix: u64,
    /// 可执行文件路径
    pub exe_path: String,
    /// 工作目录
    pub work_directory: String,
}

/// JX3 运行时状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Jx3RuntimeStatus {
    pub is_running: bool,
    /// 匹配配置游戏目录的进程（取启动时间最早的）
    pub matched_process: Option<Jx3ProcessInfo>,
    /// 所有 JX3 进程列表（用于多开检测）
    pub all_processes: Vec<Jx3ProcessInfo>,
    /// 匹配配置游戏目录的进程数量（多开时每个进程对应一个当前在线角色）
    pub matched_process_count: u32,
    pub multi_instance_detected: bool,
    pub multi_instance_hint: Option<String>,
}

/// 检测所有 JX3 进程
///
/// 性能优化：
/// 1. 复用全局 System 实例（避免重复分配）
/// 2. 仅刷新进程列表（不刷新 CPU/内存/磁盘/网络）
/// 3. 使用 `processes_by_exact_name` 按名过滤
pub fn find_all_jx3_processes() -> Vec<Jx3ProcessInfo> {
    let mut system_lock = match get_system().lock() {
        Ok(guard) => guard,
        Err(e) => {
            log::error!("[JX3Process] 获取 System 锁失败: {}", e);
            return Vec::new();
        }
    };

    // 仅刷新进程列表，不刷新 CPU/内存/磁盘/网络
    system_lock.refresh_specifics(
        RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
    );

    let target_name = std::ffi::OsStr::new(JX3_PROCESS_NAME);
    let mut processes = Vec::new();

    for process in system_lock.processes_by_exact_name(target_name) {
        let pid = process.pid().as_u32();
        let name = process.name().to_string_lossy().to_string();
        let start_time_unix = process.start_time();
        let start_time = unix_seconds_to_rfc3339(start_time_unix);
        let exe_path = process
            .exe()
            .map(|p: &std::path::Path| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let work_directory = process
            .cwd()
            .map(|p: &std::path::Path| p.to_string_lossy().to_string())
            .unwrap_or_default();

        processes.push(Jx3ProcessInfo {
            pid,
            name,
            start_time,
            start_time_unix,
            exe_path,
            work_directory,
        });
    }

    // 按启动时间升序排序（最早的在前）
    processes.sort_by_key(|p| p.start_time_unix);
    processes
}

/// 规范化路径用于比较（canonicalize 失败时降级到 lowercase）
fn normalize_path_for_compare(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match PathBuf::from(trimmed).canonicalize() {
        Ok(p) => p.to_string_lossy().to_lowercase(),
        Err(_) => trimmed.to_lowercase(),
    }
}

/// 判断进程是否与配置的游戏目录匹配
///
/// 策略：
/// 1. 首选 `canonicalize` 路径精确匹配
/// 2. 降级到字符串包含匹配（双向 contains，忽略大小写）
fn is_process_matching_directory(process: &Jx3ProcessInfo, game_directory: &str) -> bool {
    if game_directory.is_empty() {
        return true; // 未配置目录时视为全部匹配
    }

    let normalized_game = normalize_path_for_compare(game_directory);
    let exe_path = &process.exe_path;
    let work_dir = &process.work_directory;

    // 检查 exe_path 或 work_directory 是否与 game_directory 匹配
    for candidate in [exe_path.as_str(), work_dir.as_str()] {
        if candidate.is_empty() {
            continue;
        }
        let normalized_candidate = normalize_path_for_compare(candidate);
        if normalized_candidate.is_empty() {
            continue;
        }
        // 精确匹配或包含匹配
        if normalized_candidate == normalized_game
            || normalized_candidate.contains(&normalized_game)
            || normalized_game.contains(&normalized_candidate)
        {
            return true;
        }
    }

    false
}

/// 获取 JX3 运行时状态
pub fn get_jx3_runtime_status_internal(game_directory: &str) -> Jx3RuntimeStatus {
    let all_processes = find_all_jx3_processes();

    if all_processes.is_empty() {
        return Jx3RuntimeStatus {
            is_running: false,
            matched_process: None,
            all_processes,
            matched_process_count: 0,
            multi_instance_detected: false,
            multi_instance_hint: None,
        };
    }

    // 筛选与配置目录匹配的进程
    let matched: Vec<&Jx3ProcessInfo> = all_processes
        .iter()
        .filter(|p| is_process_matching_directory(p, game_directory))
        .collect();

    // 取匹配进程中启动时间最早的（all_processes 已按启动时间升序排序）
    let matched_process = matched.first().copied().cloned();

    let matched_process_count = matched.len() as u32;
    let multi_instance_detected = matched.len() > 1;
    let multi_instance_hint = if multi_instance_detected {
        Some(format!(
            "检测到 {} 个 JX3 客户端同时运行，活跃检测可能不准确",
            matched.len()
        ))
    } else {
        None
    };

    Jx3RuntimeStatus {
        is_running: matched_process.is_some(),
        matched_process,
        all_processes,
        matched_process_count,
        multi_instance_detected,
        multi_instance_hint,
    }
}

/// 获取 JX3 运行时状态（Tauri 命令）
#[tauri::command]
pub fn get_jx3_runtime_status(game_directory: String) -> Result<Jx3RuntimeStatus, String> {
    Ok(get_jx3_runtime_status_internal(&game_directory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_jx3_processes_returns_vec() {
        // 仅验证函数可调用且返回 Vec（不依赖 JX3 是否运行）
        let processes = find_all_jx3_processes();
        // JX3 未运行时应返回空 Vec
        if processes.is_empty() {
            println!("[TEST] JX3 进程未运行，返回空 Vec（符合预期）");
        } else {
            println!("[TEST] 检测到 {} 个 JX3 进程", processes.len());
            for p in &processes {
                println!(
                    "[TEST] PID={}, Name={}, StartTime={}, Exe={}",
                    p.pid, p.name, p.start_time, p.exe_path
                );
            }
        }
    }

    #[test]
    fn test_is_process_matching_directory_empty_game_dir() {
        let process = Jx3ProcessInfo {
            pid: 1234,
            name: JX3_PROCESS_NAME.to_string(),
            start_time: "2026-01-01T00:00:00+08:00".to_string(),
            start_time_unix: 1700000000,
            exe_path: "E:\\Game\\JX3\\bin\\zhcn_hd\\JX3ClientX64.exe".to_string(),
            work_directory: "E:\\Game\\JX3\\bin\\zhcn_hd".to_string(),
        };
        // 空游戏目录应视为匹配
        assert!(is_process_matching_directory(&process, ""));
    }

    #[test]
    fn test_is_process_matching_directory_exact_match() {
        let process = Jx3ProcessInfo {
            pid: 1234,
            name: JX3_PROCESS_NAME.to_string(),
            start_time: "2026-01-01T00:00:00+08:00".to_string(),
            start_time_unix: 1700000000,
            exe_path: "E:\\Game\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd\\JX3ClientX64.exe".to_string(),
            work_directory: "E:\\Game\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd".to_string(),
        };
        // 游戏目录与 exe 路径匹配
        assert!(is_process_matching_directory(
            &process,
            "E:\\Game\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd"
        ));
    }

    #[test]
    fn test_is_process_matching_directory_no_match() {
        let process = Jx3ProcessInfo {
            pid: 1234,
            name: JX3_PROCESS_NAME.to_string(),
            start_time: "2026-01-01T00:00:00+08:00".to_string(),
            start_time_unix: 1700000000,
            exe_path: "D:\\AnotherPath\\JX3\\JX3ClientX64.exe".to_string(),
            work_directory: "D:\\AnotherPath\\JX3".to_string(),
        };
        // 不同路径不应匹配
        assert!(!is_process_matching_directory(
            &process,
            "E:\\Game\\SeasunGame\\Game\\JX3\\bin\\zhcn_hd"
        ));
    }

    #[test]
    fn test_get_runtime_status_no_processes() {
        // 不依赖 JX3 是否运行，仅验证返回结构正确
        let status = get_jx3_runtime_status_internal("E:\\nonexistent\\path");
        // 如果 JX3 未运行，应返回 is_running=false
        if !status.is_running {
            assert!(status.matched_process.is_none());
            assert!(!status.multi_instance_detected);
        }
    }
}
