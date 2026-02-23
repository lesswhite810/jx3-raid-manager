use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;
mod migration;
pub mod migrations;

const DATABASE_NAME: &str = "jx3-raid-manager.db";

/// 当前数据库 schema 版本
pub const CURRENT_SCHEMA_VERSION: i32 = 2;

/// 数据库连接单例
static DB_INITIALIZED: Mutex<bool> = Mutex::new(false);

pub fn get_app_dir() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir().ok_or_else(|| "无法获取用户主目录".to_string())?;

    let app_dir = home_dir.join(".jx3-raid-manager");

    if !app_dir.exists() {
        std::fs::create_dir_all(&app_dir).map_err(|e| format!("无法创建应用目录: {}", e))?;
    }

    Ok(app_dir)
}

pub fn get_db_path() -> Result<PathBuf, String> {
    let app_dir = get_app_dir()?;
    Ok(app_dir.join(DATABASE_NAME))
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
        log::info!("数据库初始化：全新安装，创建最新版本结构 (V{})", CURRENT_SCHEMA_VERSION);

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

        // 执行所有迁移（V1 到当前版本）
        for version in 1..=CURRENT_SCHEMA_VERSION {
            log::info!("执行迁移脚本：V{}", version);
            migration::apply_migration(&conn, version)?;
            set_schema_version(&conn, version, &format!("升级到 V{}", version))?;
            log::info!("迁移 V{} 完成", version);
        }

        // 初始化静态副本数据
        migration::init_static_raids(&conn)?;

    } else if current_version < CURRENT_SCHEMA_VERSION {
        // ========== 从中间版本升级场景 ==========
        log::info!("数据库初始化：从 V{} 升级到 V{}", current_version, CURRENT_SCHEMA_VERSION);

        // 执行增量迁移
        for version in (current_version + 1)..=CURRENT_SCHEMA_VERSION {
            log::info!("执行迁移脚本：V{}", version);
            migration::apply_migration(&conn, version)?;
            set_schema_version(&conn, version, &format!("升级到 V{}", version))?;
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
    let timestamp = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO schema_versions (version, applied_at, description) VALUES (?, ?, ?)",
        params![version, timestamp, description],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 创建最新版本的数据库结构
fn create_latest_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        -- ========== 基础记录表 ==========
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            data TEXT
        );

        -- ========== 配置表 ==========
        CREATE TABLE IF NOT EXISTS config (
            id INTEGER PRIMARY KEY,
            value TEXT
        );

        -- ========== 缓存表 ==========
        CREATE TABLE IF NOT EXISTS cache (
            key TEXT PRIMARY KEY,
            value TEXT,
            updated_at TEXT
        );

        -- ========== 装备表 ==========
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

        -- ========== 试炼记录表 ==========
        CREATE TABLE IF NOT EXISTS trial_records (
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
            date TEXT NOT NULL,
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
            date TEXT NOT NULL,
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
    "#,
    )
    .map_err(|e| e.to_string())?;

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
    let timestamp = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO migration_flags (key, value, updated_at) VALUES ('local_storage_migrated', 'true', ?)",
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
    let timestamp = chrono::Utc::now().to_rfc3339();

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
                "INSERT OR REPLACE INTO equipments (
                id, name, ui_id, icon_id, level, quality, bind_type, type_label,
                attribute_types, attributes, recommend, diamonds, 
                data, updated_at
            ) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
    pub date: String,
    pub notes: Option<String>,
}

#[tauri::command]
pub fn db_add_trial_record(record: String) -> Result<(), String> {
    let item: TrialRecord = serde_json::from_str(&record).map_err(|e| e.to_string())?;
    let conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Ensure bosses are serialized
    let bosses_json = serde_json::to_string(&item.bosses).unwrap_or_default();

    conn.execute(
        "INSERT OR REPLACE INTO trial_records (
            id, account_id, role_id, layer, bosses, 
            card_1, card_2, card_3, card_4, card_5, flipped_index, date, notes, updated_at
        ) 
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            item.id,
            item.account_id,
            item.role_id,
            item.layer,
            bosses_json,
            item.card_1,
            item.card_2,
            item.card_3,
            item.card_4,
            item.card_5,
            item.flipped_index,
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
        SELECT id, account_id, role_id, layer, bosses, 
               card_1, card_2, card_3, card_4, card_5, flipped_index,
               date, notes, updated_at
        FROM trial_records 
        ORDER BY date DESC
    ",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            let bosses_str: String = row.get(4)?;

            Ok(TrialRecord {
                id: row.get(0)?,
                account_id: row.get(1)?,
                role_id: row.get(2)?,
                layer: row.get(3)?,
                bosses: serde_json::from_str(&bosses_str).unwrap_or_default(),
                card_1: row.get(5)?,
                card_2: row.get(6)?,
                card_3: row.get(7)?,
                card_4: row.get(8)?,
                card_5: row.get(9)?,
                flipped_index: row.get(10)?,
                date: row.get(11)?,
                notes: row.get(12)?,
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
    pub date: String,
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
    let timestamp = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO baizhan_records (
            id, account_id, role_id, role_name, server,
            date, gold_income, gold_expense, notes, record_type, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                record_type: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "baizhan".to_string()),
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
    let timestamp = chrono::Utc::now().to_rfc3339();

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

    // 全量同步：先清空表，再插入（注意级联或手动清空对应关联表）
    tx.execute("DELETE FROM roles", []).map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM accounts", []).map_err(|e| e.to_string())?;

    let timestamp = chrono::Utc::now().to_rfc3339();

    for account in parsed {
        let id = account["id"].as_str().unwrap_or_default().to_string();
        let account_name = account["accountName"].as_str().unwrap_or("").to_string();
        let account_type = account["type"].as_str().unwrap_or("OWN").to_string();
        let password = account["password"].as_str().map(|s| s.to_string());
        let notes = account["notes"].as_str().map(|s| s.to_string());
        let hidden = account["hidden"].as_bool().unwrap_or(false) as i32;
        let disabled = account["disabled"].as_bool().unwrap_or(false) as i32;

        tx.execute(
            "INSERT INTO accounts (id, account_name, account_type, password, notes, hidden, disabled, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![id, account_name, account_type, password, notes, hidden, disabled, timestamp],
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

                tx.execute(
                    "INSERT INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        SELECT id, account_name, account_type, hidden, disabled, password, notes, created_at, updated_at 
        FROM accounts ORDER BY account_name
    ",
        )
        .map_err(|e| e.to_string())?;

    let accounts: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "accountName": row.get::<_, String>(1)?,
                "type": row.get::<_, String>(2)?,
                "password": row.get::<_, Option<String>>(3)?,
                "notes": row.get::<_, Option<String>>(4)?,
                "hidden": row.get::<_, i32>(5)? != 0,
                "disabled": row.get::<_, i32>(6)? != 0,
                "createdAt": row.get::<_, Option<String>>(7)?,
                "updatedAt": row.get::<_, Option<String>>(8)?,
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

#[tauri::command]
pub fn db_get_accounts_with_roles() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

    // Get all accounts
    let mut account_stmt = conn
        .prepare(
            "
        SELECT id, account_name, account_type, password, notes, hidden, disabled, created_at, updated_at 
        FROM accounts ORDER BY account_name
    ",
        )
        .map_err(|e| e.to_string())?;

    let account_rows = account_stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "accountName": row.get::<_, String>(1)?,
                    "type": row.get::<_, String>(2)?,
                    "password": row.get::<_, Option<String>>(3)?,
                    "notes": row.get::<_, Option<String>>(4)?,
                    "hidden": row.get::<_, i32>(5)? != 0,
                    "disabled": row.get::<_, i32>(6)? != 0,
                    "createdAt": row.get::<_, Option<String>>(7)?,
                    "updatedAt": row.get::<_, Option<String>>(8)?,
                    "roles": Vec::<serde_json::Value>::new(),
                }),
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect::<Vec<(String, serde_json::Value)>>();

    // Build account map
    let mut account_map: std::collections::HashMap<String, serde_json::Value> =
        account_rows.into_iter().collect();

    // Get all roles
    let mut role_stmt = conn
        .prepare(
            "
        SELECT r.id, r.account_id, r.name, r.server, r.region, r.sect, 
               r.equipment_score, r.disabled, r.created_at, r.updated_at
        FROM roles r ORDER BY r.name
    ",
        )
        .map_err(|e| e.to_string())?;

    let role_rows = role_stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "account_id": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "server": row.get::<_, Option<String>>(3)?,
                "region": row.get::<_, Option<String>>(4)?,
                "sect": row.get::<_, Option<String>>(5)?,
                "equipmentScore": row.get::<_, Option<i64>>(6)?,
                "disabled": row.get::<_, i32>(7)? != 0,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect::<Vec<serde_json::Value>>();

    // Assign roles to accounts
    for role in role_rows {
        let account_id = role["account_id"].as_str();
        if let Some(acc_id) = account_id {
            if let Some(account) = account_map.get_mut(acc_id) {
                let roles = account["roles"].as_array_mut().unwrap();
                roles.push(role);
            }
        }
    }

    // Return accounts as array
    let accounts: Vec<serde_json::Value> = account_map.into_values().collect();
    serde_json::to_string(&accounts).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn db_get_roles_by_account(account_id: String) -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;

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
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "server": row.get::<_, Option<String>>(2)?,
                "region": row.get::<_, Option<String>>(3)?,
                "sect": row.get::<_, Option<String>>(4)?,
                "equipmentScore": row.get::<_, Option<i64>>(5)?,
                "disabled": row.get::<_, i32>(6)? != 0,
                "createdAt": row.get::<_, Option<String>>(7)?,
                "updatedAt": row.get::<_, Option<String>>(8)?,
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
    let timestamp = chrono::Utc::now().to_rfc3339();

    let id = account["id"].as_str().unwrap_or_default().to_string();
    let account_name = account["accountName"].as_str().unwrap_or("").to_string();
    let account_type = account["type"].as_str().unwrap_or("OWN").to_string();
    let password = account["password"].as_str().map(|s| s.to_string());
    let notes = account["notes"].as_str().map(|s| s.to_string());
    let hidden = account["hidden"].as_bool().unwrap_or(false) as i32;
    let disabled = account["disabled"].as_bool().unwrap_or(false) as i32;

    // Use Option<String> directly with rusqlite params
    conn.execute(
        "INSERT OR REPLACE INTO accounts (id, account_name, account_type, password, notes, hidden, disabled, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, account_name, account_type, password, notes, hidden, disabled, timestamp],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn db_save_role_structured(role_json: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let role: serde_json::Value = serde_json::from_str(&role_json).map_err(|e| e.to_string())?;
    let timestamp = chrono::Utc::now().to_rfc3339();

    let id = role["id"].as_str().unwrap_or_default().to_string();
    let account_id = role["accountId"].as_str().unwrap_or("").to_string();
    let name = role["name"].as_str().unwrap_or("").to_string();
    let server = role["server"].as_str().map(|s| s.to_string());
    let region = role["region"].as_str().map(|s| s.to_string());
    let sect = role["sect"].as_str().map(|s| s.to_string());
    let disabled = role["disabled"].as_bool().unwrap_or(false) as i32;
    let equipment_score = role["equipmentScore"].as_i64();

    conn.execute(
        "INSERT OR REPLACE INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![id, account_id, name, server, region, sect, equipment_score, disabled, timestamp],
    ).map_err(|e| e.to_string())?;

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

    // 增量同步：使用 INSERT OR REPLACE 更新记录，不删除历史数据
    for record in parsed {
        let id = record["id"].as_str().unwrap_or_default().to_string();
        if id.is_empty() {
            continue;
        }
        tx.execute(
            "INSERT OR REPLACE INTO records (id, data) VALUES (?, ?)",
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
        let mut boss_rows = boss_stmt.query(params![raid_name]).map_err(|e| e.to_string())?;
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

    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM raid_bosses", [])
        .map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM raids", [])
        .map_err(|e| e.to_string())?;

    let mut boss_saved_names = std::collections::HashSet::new();
    for raid in &parsed {
        let name = raid["name"].as_str().unwrap_or_default();
        let difficulty = raid["difficulty"].as_str().unwrap_or("普通");
        let player_count = raid["playerCount"].as_i64().unwrap_or(25);
        let version = raid["version"].as_str().unwrap_or_default();
        let notes = raid["notes"].as_str().unwrap_or_default();
        let is_active = if raid["isActive"].as_bool().unwrap_or(true) { 1 } else { 0 };
        let is_static = if raid["static"].as_bool().unwrap_or(false) { 1 } else { 0 };
        let id = format!("{}人{}{}", player_count, difficulty, name);

        tx.execute(
            "INSERT OR REPLACE INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![id, name, difficulty, player_count, version, notes, is_active, is_static],
        ).map_err(|e| e.to_string())?;

        // 插入 raid_bosses（按副本名称去重，只写一次）
        if !boss_saved_names.contains(name) {
            if let Some(bosses) = raid["bosses"].as_array() {
                for boss in bosses {
                    let boss_id = boss["id"].as_str().unwrap_or_default();
                    let boss_name = boss["name"].as_str().unwrap_or_default();
                    let boss_order = boss["order"].as_i64().unwrap_or(0);
                    tx.execute(
                        "INSERT OR REPLACE INTO raid_bosses (id, raid_name, name, boss_order) VALUES (?, ?, ?, ?)",
                        params![boss_id, name, boss_name, boss_order],
                    ).map_err(|e| e.to_string())?;
                }
                boss_saved_names.insert(name.to_string());
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
        "INSERT OR REPLACE INTO config (id, value) VALUES (1, ?)",
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
        "INSERT OR REPLACE INTO records (id, data) VALUES (?, ?)",
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
    let updated_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO cache (key, value, updated_at) VALUES (?, ?, ?)",
        params![key, value, updated_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
