use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::db;

/// 关闭主窗口时的行为。
///
/// - `Ask`：每次关闭都弹窗询问（默认，兼容历史用户与首次使用）
/// - `Minimize`：直接隐藏到系统托盘，进程继续运行
/// - `Exit`：直接退出应用
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CloseBehavior {
    Ask,
    Minimize,
    Exit,
}

impl CloseBehavior {
    /// 存入 app_config 表的字符串值
    fn as_str(self) -> &'static str {
        match self {
            CloseBehavior::Ask => "ask",
            CloseBehavior::Minimize => "minimize",
            CloseBehavior::Exit => "exit",
        }
    }

    /// 解析存储值；缺失或非法值一律回落到「每次询问」，避免旧配置把用户锁死在某个行为上。
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "minimize" => CloseBehavior::Minimize,
            "exit" => CloseBehavior::Exit,
            _ => CloseBehavior::Ask,
        }
    }
}

impl Default for CloseBehavior {
    fn default() -> Self {
        CloseBehavior::Ask
    }
}

/// 副本管理的显示模式。
///
/// - `Current`：当前模式，进入副本管理直接显示 CD 概览表（默认，新用户与升级用户一致）
/// - `History`：历史模式，还原页签结构（团队副本 / 试炼之地 / 百战），团队副本默认落在收藏界面
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RaidViewMode {
    Current,
    History,
}

impl RaidViewMode {
    /// 存入 app_config 表的字符串值
    fn as_str(self) -> &'static str {
        match self {
            RaidViewMode::Current => "current",
            RaidViewMode::History => "history",
        }
    }

    /// 解析存储值；缺失或非法值一律回落到「当前模式」，避免旧配置把用户锁在旧界面上。
    fn parse(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "history" => RaidViewMode::History,
            _ => RaidViewMode::Current,
        }
    }
}

impl Default for RaidViewMode {
    fn default() -> Self {
        RaidViewMode::Current
    }
}

/// 应用配置（key-value 存储的封装视图）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub game_directory: Option<String>,
    pub setup_completed: bool,
    pub last_scan_mingyi_at: Option<String>,
    pub auto_scan_enabled: bool,
    /// 试炼结算截图识别独立于常规 JCL 自动扫描，旧配置默认关闭。
    pub trial_screenshot_recognition_enabled: bool,
    pub auto_refresh_equip_score: bool,
    /// 关闭主窗口时的行为，旧配置默认每次询问。
    pub close_behavior: CloseBehavior,
    /// 副本管理的显示模式，旧配置默认当前模式（CD 概览）。
    pub raid_view_mode: RaidViewMode,
    /// 是否在 CD 概览中展示周常副本列，旧配置默认开启。
    pub show_weekly_raids: bool,
    /// 是否自动隐藏非满级角色，旧配置默认关闭。
    pub hide_non_max_level_roles: bool,
}

const KEY_GAME_DIRECTORY: &str = "game_directory";
const KEY_SETUP_COMPLETED: &str = "setup_completed";
const KEY_LAST_SCAN_MINGYI_AT: &str = "last_scan_mingyi_at";
const KEY_AUTO_SCAN_ENABLED: &str = "auto_scan_enabled";
const KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED: &str = "trial_screenshot_recognition_enabled";
const KEY_AUTO_REFRESH_EQUIP_SCORE: &str = "auto_refresh_equip_score";
const KEY_CLOSE_BEHAVIOR: &str = "close_behavior";
const KEY_RAID_VIEW_MODE: &str = "raid_view_mode";
const KEY_SHOW_WEEKLY_RAIDS: &str = "show_weekly_raids";
const KEY_HIDE_NON_MAX_LEVEL_ROLES: &str = "hide_non_max_level_roles";
const KEY_THEME: &str = "theme";

// 两个开关可能由前端并发切换。将“写入 -> 读取组合状态 -> 发布给采集线程”串行化，
// 避免旧快照在较新的写入之后重新开启采集。
static TRIAL_CAPTURE_CONFIG_LOCK: Mutex<()> = Mutex::new(());

/// 从 app_config 表读取指定 key 的值
fn read_value(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
    match conn.query_row(
        "SELECT value FROM app_config WHERE key = ?1",
        params![key],
        |row| row.get(0),
    ) {
        Ok(value) => Ok(Some(value)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// 更新指定 key 的值（UPSERT 语义）
fn upsert_value(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, &now],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 将字符串值转为 Option<String>，空字符串视为 None
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.is_empty())
}

/// 从既有连接读取完整应用配置，便于设置项写入后用同一连接计算运行时状态。
fn read_app_config(conn: &rusqlite::Connection) -> Result<AppConfig, String> {
    let game_directory = non_empty(read_value(conn, KEY_GAME_DIRECTORY)?);
    let setup_completed_str = read_value(conn, KEY_SETUP_COMPLETED)?.unwrap_or_default();
    let setup_completed = setup_completed_str.eq_ignore_ascii_case("true");
    let last_scan_mingyi_at = non_empty(read_value(conn, KEY_LAST_SCAN_MINGYI_AT)?);
    let auto_scan_enabled_str = read_value(conn, KEY_AUTO_SCAN_ENABLED)?.unwrap_or_default();
    let auto_scan_enabled = auto_scan_enabled_str.eq_ignore_ascii_case("true");
    // 截图识别为新增能力。缺少该 key 的历史配置必须保持关闭。
    let trial_screenshot_recognition_enabled =
        read_value(conn, KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED)?
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");
    let auto_refresh_equip_score_str =
        read_value(conn, KEY_AUTO_REFRESH_EQUIP_SCORE)?.unwrap_or_default();
    // 未配置时默认开启（兼容历史用户与首次启动）
    let auto_refresh_equip_score = auto_refresh_equip_score_str.is_empty()
        || auto_refresh_equip_score_str.eq_ignore_ascii_case("true");
    // 关闭行为是新增配置项：缺少该 key 的历史配置回落到「每次询问」。
    let close_behavior = read_value(conn, KEY_CLOSE_BEHAVIOR)?
        .map(|raw| CloseBehavior::parse(&raw))
        .unwrap_or_default();
    // 副本管理显示模式同为新增配置项：缺少该 key 的历史配置回落到「当前模式」。
    let raid_view_mode = read_value(conn, KEY_RAID_VIEW_MODE)?
        .map(|raw| RaidViewMode::parse(&raw))
        .unwrap_or_default();
    // 展示周常是新增配置项：未配置时默认开启（兼容历史用户与首次启动）。
    let show_weekly_raids_str = read_value(conn, KEY_SHOW_WEEKLY_RAIDS)?.unwrap_or_default();
    let show_weekly_raids =
        show_weekly_raids_str.is_empty() || show_weekly_raids_str.eq_ignore_ascii_case("true");
    // 自动隐藏非满级角色是新增配置项：缺少该 key 的历史配置必须保持关闭。
    let hide_non_max_level_roles =
        read_value(conn, KEY_HIDE_NON_MAX_LEVEL_ROLES)?
            .unwrap_or_default()
            .eq_ignore_ascii_case("true");

    Ok(AppConfig {
        game_directory,
        setup_completed,
        last_scan_mingyi_at,
        auto_scan_enabled,
        trial_screenshot_recognition_enabled,
        auto_refresh_equip_score,
        close_behavior,
        raid_view_mode,
        show_weekly_raids,
        hide_non_max_level_roles,
    })
}

/// 读取完整应用配置
pub fn get_app_config_internal() -> Result<AppConfig, String> {
    let conn = db::init_db()?;
    read_app_config(&conn)
}

/// 截图采集的运行条件。常规自动扫描仍只使用 `auto_scan_enabled`。
pub fn is_trial_screenshot_capture_enabled(config: &AppConfig) -> bool {
    config.auto_scan_enabled && config.trial_screenshot_recognition_enabled
}

/// 在配置写入成功后才切换截图采集，关闭任一开关会同步清理待处理识别。
fn refresh_trial_screenshot_capture(conn: &rusqlite::Connection) -> Result<(), String> {
    let config = read_app_config(conn)?;
    crate::trial_capture::set_enabled(is_trial_screenshot_capture_enabled(&config));
    crate::mingyi::drop_scanner::live::configure(
        is_trial_screenshot_capture_enabled(&config),
        &crate::game_directory::resolve_game_runtime_directory(
            config.game_directory.as_deref().unwrap_or(""),
        ),
    );
    Ok(())
}

/// 获取应用配置（Tauri 命令）
#[tauri::command]
pub fn get_app_config() -> Result<AppConfig, String> {
    get_app_config_internal()
}

/// 设置游戏目录（Tauri 命令）
///
/// 游戏目录以 app_config.game_directory 为唯一存储源，
/// 不再同步到 config_json。
#[tauri::command]
pub fn set_game_directory(path: String) -> Result<(), String> {
    // 必须与 reset_setup / set_auto_scan_enabled 等命令持有同一把锁：
    // 本命令会 clear 采集缓存并重建截图采集器，与并发切换开关的命令交错时
    // 会出现「刚重建又被清空」的竞态，导致采集器状态与配置不一致。
    let _capture_config_guard = TRIAL_CAPTURE_CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_GAME_DIRECTORY, &path)?;
    crate::mingyi::drop_scanner::live::clear();
    crate::trial_capture::clear();
    refresh_trial_screenshot_capture(&conn)?;

    log::info!("[AppConfig] 游戏目录已更新: {}", path);
    Ok(())
}

/// 标记引导流程完成（Tauri 命令）
#[tauri::command]
pub fn complete_setup() -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_SETUP_COMPLETED, "true")?;
    log::info!("[AppConfig] 启动引导已完成");
    Ok(())
}

/// 重置配置，回到未引导状态（Tauri 命令）
#[tauri::command]
pub fn reset_setup() -> Result<(), String> {
    let _capture_config_guard = TRIAL_CAPTURE_CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_GAME_DIRECTORY, "")?;
    upsert_value(&conn, KEY_SETUP_COMPLETED, "false")?;
    upsert_value(&conn, KEY_LAST_SCAN_MINGYI_AT, "")?;
    upsert_value(&conn, KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED, "false")?;
    upsert_value(&conn, KEY_CLOSE_BEHAVIOR, CloseBehavior::Ask.as_str())?;
    upsert_value(&conn, KEY_RAID_VIEW_MODE, RaidViewMode::Current.as_str())?;
    upsert_value(&conn, KEY_SHOW_WEEKLY_RAIDS, "true")?;
    upsert_value(&conn, KEY_HIDE_NON_MAX_LEVEL_ROLES, "false")?;
    refresh_trial_screenshot_capture(&conn)?;
    log::info!("[AppConfig] 应用配置已重置，将回到引导界面");
    Ok(())
}

/// 设置自动扫描开关（Tauri 命令）
#[tauri::command]
pub fn set_auto_scan_enabled(enabled: bool) -> Result<(), String> {
    let _capture_config_guard = TRIAL_CAPTURE_CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let conn = db::init_db()?;
    upsert_value(
        &conn,
        KEY_AUTO_SCAN_ENABLED,
        if enabled { "true" } else { "false" },
    )?;
    refresh_trial_screenshot_capture(&conn)?;
    log::info!(
        "[AppConfig] 自动扫描已{}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 设置试炼结算截图识别开关（Tauri 命令）。
///
/// 该开关仅影响截图采集；实际采集还要求自动扫描已开启。
#[tauri::command]
pub fn set_trial_screenshot_recognition_enabled(enabled: bool) -> Result<(), String> {
    let _capture_config_guard = TRIAL_CAPTURE_CONFIG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let conn = db::init_db()?;
    upsert_value(
        &conn,
        KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED,
        if enabled { "true" } else { "false" },
    )?;
    refresh_trial_screenshot_capture(&conn)?;
    log::info!(
        "[AppConfig] 试炼截图识别已{}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 设置启动刷新装分开关（Tauri 命令）
#[tauri::command]
pub fn set_auto_refresh_equip_score_enabled(enabled: bool) -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(
        &conn,
        KEY_AUTO_REFRESH_EQUIP_SCORE,
        if enabled { "true" } else { "false" },
    )?;

    log::info!(
        "[AppConfig] 启动刷新装分已{}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 设置关闭按钮行为（Tauri 命令）
///
/// 取值 ask / minimize / exit；非法值会被拒绝，避免前端拼错后静默变成默认行为。
#[tauri::command]
pub fn set_close_behavior(behavior: String) -> Result<(), String> {
    let parsed = match behavior.trim().to_ascii_lowercase().as_str() {
        "ask" => CloseBehavior::Ask,
        "minimize" => CloseBehavior::Minimize,
        "exit" => CloseBehavior::Exit,
        _ => {
            let msg = format!("[AppConfig] invalid close behavior: {}", behavior);
            log::warn!("{}", msg);
            return Err(msg);
        }
    };
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_CLOSE_BEHAVIOR, parsed.as_str())?;
    log::info!("[AppConfig] 关闭按钮行为已设置为: {}", parsed.as_str());
    Ok(())
}

/// 设置副本管理显示模式（Tauri 命令）
///
/// 取值 current / history；非法值会被拒绝，避免前端拼错后静默变成默认模式。
#[tauri::command]
pub fn set_raid_view_mode(mode: String) -> Result<(), String> {
    let parsed = match mode.trim().to_ascii_lowercase().as_str() {
        "current" => RaidViewMode::Current,
        "history" => RaidViewMode::History,
        _ => {
            let msg = format!("[AppConfig] invalid raid view mode: {}", mode);
            log::warn!("{}", msg);
            return Err(msg);
        }
    };
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_RAID_VIEW_MODE, parsed.as_str())?;
    log::info!("[AppConfig] 副本管理显示模式已设置为: {}", parsed.as_str());
    Ok(())
}

/// 设置 CD 概览展示周常开关（Tauri 命令）
#[tauri::command]
pub fn set_show_weekly_raids_enabled(enabled: bool) -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(
        &conn,
        KEY_SHOW_WEEKLY_RAIDS,
        if enabled { "true" } else { "false" },
    )?;

    log::info!(
        "[AppConfig] 展示周常已{}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 设置自动隐藏非满级角色开关（Tauri 命令）
#[tauri::command]
pub fn set_hide_non_max_level_roles_enabled(enabled: bool) -> Result<(), String> {
    let conn = db::init_db()?;
    upsert_value(
        &conn,
        KEY_HIDE_NON_MAX_LEVEL_ROLES,
        if enabled { "true" } else { "false" },
    )?;

    log::info!(
        "[AppConfig] 自动隐藏非满级角色已{}",
        if enabled { "开启" } else { "关闭" }
    );
    Ok(())
}

/// 获取主题设置（Tauri 命令）
///
/// 返回当前存储的主题值（original / jianghu / dark），未设置时返回 None。
#[tauri::command]
pub fn get_theme() -> Result<Option<String>, String> {
    let conn = db::init_db()?;
    read_value(&conn, KEY_THEME)
}

/// 设置主题（Tauri 命令）
///
/// 仅接受 original / jianghu / dark 三个合法值；非法值会被拒绝。
#[tauri::command]
pub fn set_theme(theme: String) -> Result<(), String> {
    if theme != "original" && theme != "jianghu" && theme != "dark" {
        let msg = format!("[AppConfig] invalid theme value: {}", theme);
        log::warn!("{}", msg);
        return Err(msg);
    }
    let conn = db::init_db()?;
    upsert_value(&conn, KEY_THEME, &theme)?;
    log::info!("[AppConfig] theme switched to: {}", theme);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_app_config_table(conn: &rusqlite::Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
    }

    /// 在临时文件中创建测试数据库
    fn setup_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        create_app_config_table(&conn);
        let now = chrono::Local::now().to_rfc3339();
        for key in [
            KEY_GAME_DIRECTORY,
            KEY_SETUP_COMPLETED,
            KEY_LAST_SCAN_MINGYI_AT,
        ] {
            let default_value = match key {
                KEY_SETUP_COMPLETED => "false",
                _ => "",
            };
            conn.execute(
                "INSERT INTO app_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params![key, default_value, &now],
            )
            .unwrap();
        }
        conn
    }

    #[test]
    fn test_read_write_roundtrip() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_GAME_DIRECTORY, "E:\\Game\\JX3").unwrap();
        let value = read_value(&conn, KEY_GAME_DIRECTORY).unwrap();
        assert_eq!(value, Some("E:\\Game\\JX3".to_string()));
    }

    #[test]
    fn test_non_empty_filters_empty_string() {
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(
            non_empty(Some("value".to_string())),
            Some("value".to_string())
        );
        assert_eq!(non_empty(None), None);
    }

    #[test]
    fn read_value_surfaces_database_errors_instead_of_treating_them_as_disabled() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        assert!(read_app_config(&conn).is_err());
    }

    #[test]
    fn test_auto_refresh_equip_score_defaults_to_true_when_missing() {
        let conn = setup_test_db();
        // 未写入该 key 时，read_value 返回 None -> unwrap_or_default 得到空字符串
        let raw = read_value(&conn, KEY_AUTO_REFRESH_EQUIP_SCORE).unwrap();
        assert!(raw.is_none());
        let raw_str = raw.unwrap_or_default();
        let enabled = raw_str.is_empty() || raw_str.eq_ignore_ascii_case("true");
        assert!(enabled, "缺少配置时应默认开启");
    }

    #[test]
    fn test_auto_refresh_equip_score_false_when_explicitly_disabled() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_AUTO_REFRESH_EQUIP_SCORE, "false").unwrap();
        let raw_str = read_value(&conn, KEY_AUTO_REFRESH_EQUIP_SCORE)
            .unwrap()
            .unwrap_or_default();
        let enabled = raw_str.is_empty() || raw_str.eq_ignore_ascii_case("true");
        assert!(!enabled, "显式设为 false 后应为关闭");
    }

    #[test]
    fn trial_screenshot_recognition_defaults_to_disabled_for_legacy_config() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_AUTO_SCAN_ENABLED, "true").unwrap();
        let config = read_app_config(&conn).unwrap();

        assert!(config.auto_scan_enabled);
        assert!(!config.trial_screenshot_recognition_enabled);
        assert!(!is_trial_screenshot_capture_enabled(&config));
    }

    #[test]
    fn trial_screenshot_preference_does_not_change_auto_scan_setting() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED, "true").unwrap();
        let config = read_app_config(&conn).unwrap();

        assert!(!config.auto_scan_enabled);
        assert!(config.trial_screenshot_recognition_enabled);
        assert!(!is_trial_screenshot_capture_enabled(&config));
    }

    #[test]
    fn trial_screenshot_recognition_is_persisted_across_reopen() {
        let path =
            std::env::temp_dir().join(format!("jx3-app-config-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            create_app_config_table(&conn);
            upsert_value(&conn, KEY_AUTO_SCAN_ENABLED, "true").unwrap();
            upsert_value(&conn, KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED, "true").unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert!(is_trial_screenshot_capture_enabled(&config));
            upsert_value(&conn, KEY_TRIAL_SCREENSHOT_RECOGNITION_ENABLED, "false").unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert!(config.auto_scan_enabled);
            assert!(!config.trial_screenshot_recognition_enabled);
            assert!(!is_trial_screenshot_capture_enabled(&config));
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn trial_capture_requires_both_master_and_its_own_switch() {        let mut config = read_app_config(&setup_test_db()).unwrap();
        config.auto_scan_enabled = true;
        config.trial_screenshot_recognition_enabled = false;
        assert!(!is_trial_screenshot_capture_enabled(&config));

        config.auto_scan_enabled = false;
        config.trial_screenshot_recognition_enabled = true;
        assert!(!is_trial_screenshot_capture_enabled(&config));

        config.auto_scan_enabled = true;
        assert!(is_trial_screenshot_capture_enabled(&config));
    }

    #[test]
    fn close_behavior_defaults_to_ask_when_missing() {
        let conn = setup_test_db();
        let config = read_app_config(&conn).unwrap();
        assert_eq!(config.close_behavior, CloseBehavior::Ask);
    }

    #[test]
    fn close_behavior_parses_known_values_and_falls_back_on_garbage() {
        assert_eq!(CloseBehavior::parse("ask"), CloseBehavior::Ask);
        assert_eq!(CloseBehavior::parse("minimize"), CloseBehavior::Minimize);
        assert_eq!(CloseBehavior::parse("exit"), CloseBehavior::Exit);
        assert_eq!(CloseBehavior::parse(" EXIT "), CloseBehavior::Exit);
        assert_eq!(CloseBehavior::parse(""), CloseBehavior::Ask);
        assert_eq!(CloseBehavior::parse("whatever"), CloseBehavior::Ask);
    }

    #[test]
    fn close_behavior_is_persisted_across_reopen() {
        let path =
            std::env::temp_dir().join(format!("jx3-close-behavior-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            create_app_config_table(&conn);
            upsert_value(&conn, KEY_CLOSE_BEHAVIOR, CloseBehavior::Minimize.as_str()).unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert_eq!(config.close_behavior, CloseBehavior::Minimize);
            // 用户改回「每次询问」后必须真的回到询问，而不是继续沿用旧选择。
            upsert_value(&conn, KEY_CLOSE_BEHAVIOR, CloseBehavior::Ask.as_str()).unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert_eq!(config.close_behavior, CloseBehavior::Ask);
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn close_behavior_does_not_disturb_other_switches() {
        let conn = setup_test_db();
        upsert_value(&conn, KEY_AUTO_SCAN_ENABLED, "true").unwrap();
        upsert_value(&conn, KEY_CLOSE_BEHAVIOR, CloseBehavior::Exit.as_str()).unwrap();
        let config = read_app_config(&conn).unwrap();
        assert_eq!(config.close_behavior, CloseBehavior::Exit);
        assert!(config.auto_scan_enabled);
        assert!(!config.trial_screenshot_recognition_enabled);
    }

    #[test]
    fn hide_non_max_level_roles_defaults_to_disabled_for_legacy_config() {
        let conn = setup_test_db();
        let config = read_app_config(&conn).unwrap();
        assert!(
            !config.hide_non_max_level_roles,
            "缺少 hide_non_max_level_roles 配置时（历史用户/首次启动）应默认关闭"
        );
    }

    #[test]
    fn hide_non_max_level_roles_is_persisted_across_reopen() {
        let path =
            std::env::temp_dir().join(format!("jx3-hide-non-max-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            create_app_config_table(&conn);
            let config = read_app_config(&conn).unwrap();
            assert!(!config.hide_non_max_level_roles);
            // 模拟 set_hide_non_max_level_roles_enabled(true) 的持久化写入
            upsert_value(&conn, KEY_HIDE_NON_MAX_LEVEL_ROLES, "true").unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert!(config.hide_non_max_level_roles, "重新打开后应保持开启");
            upsert_value(&conn, KEY_HIDE_NON_MAX_LEVEL_ROLES, "false").unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            let config = read_app_config(&conn).unwrap();
            assert!(!config.hide_non_max_level_roles, "关闭后应保持关闭");
        }
        std::fs::remove_file(&path).unwrap();
    }
}
