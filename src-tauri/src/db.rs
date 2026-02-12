use rusqlite::{params, Connection, Result};
use std::path::PathBuf;

mod migration;

const DATABASE_NAME: &str = "jx3-raid-manager.db";

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

pub fn init_db() -> Result<Connection, String> {
    let path = get_db_path()?;

    let conn = Connection::open(&path).map_err(|e| e.to_string())?;

    // Create base tables (accounts will be created/renamed by migration)
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS records (
            id TEXT PRIMARY KEY,
            data TEXT
        );
        
        CREATE TABLE IF NOT EXISTS raids (
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
            date TEXT,
            notes TEXT,
            updated_at TEXT
        );
    "#,
    )
    .map_err(|e| e.to_string())?;

    // Initialize schema versions table and apply migrations
    migration::init_schema_versions(&conn).map_err(|e| e.to_string())?;
    migration::apply_migrations(&conn).map_err(|e| e.to_string())?;

    Ok(conn)
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
        "INSERT INTO trial_records (
            id, account_id, role_id, layer, bosses, 
            card_1, card_2, card_3, card_4, card_5, flipped_index,
            date, notes, updated_at
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
               date, notes 
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

#[tauri::command]
pub fn db_init() -> Result<(), String> {
    init_db()?;
    Ok(())
}

#[tauri::command]
pub fn db_get_migration_status() -> Result<Option<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT status, migrated_at, error_message FROM migration_log WHERE id = 1")
        .map_err(|e| e.to_string())?;
    let result: Option<String> = stmt.query_row([], |row| row.get(0)).ok();
    Ok(result)
}

#[tauri::command]
pub fn db_check_migration_completed() -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let status: Option<String> = conn
        .query_row("SELECT status FROM migration_log WHERE id = 1", [], |row| {
            row.get(0)
        })
        .ok();

    Ok(status == Some("completed".to_string()))
}

#[tauri::command]
pub fn db_set_migration_status(
    status: String,
    error_message: Option<String>,
) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let timestamp = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO migration_log (id, status, migrated_at, error_message) VALUES (1, ?, ?, ?)",
        params![status, timestamp, error_message.unwrap_or_default()],
    ).map_err(|e| e.to_string())?;

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

    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM accounts", [])
        .map_err(|e| e.to_string())?;

    for account in parsed {
        let id = account["id"].as_str().unwrap_or_default().to_string();
        tx.execute(
            "INSERT INTO accounts (id, data) VALUES (?, ?)",
            params![id, account.to_string()],
        )
        .map_err(|e| e.to_string())?;
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
pub fn db_get_schema_version() -> Result<i32, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    migration::get_current_version(&conn)
}

#[tauri::command]
pub fn db_check_migration_needed() -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    migration::is_migration_needed(&conn)
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

    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM records", [])
        .map_err(|e| e.to_string())?;

    for record in parsed {
        let id = record["id"].as_str().unwrap_or_default().to_string();
        tx.execute(
            "INSERT INTO records (id, data) VALUES (?, ?)",
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
    let mut stmt = conn
        .prepare("SELECT data FROM raids")
        .map_err(|e| e.to_string())?;
    let mut rows = stmt.query([]).map_err(|e| e.to_string())?;
    let mut raids = Vec::new();
    while let Some(row) = rows.next().map_err(|e| e.to_string())? {
        if let Ok(data) = row.get(0) {
            raids.push(data);
        }
    }
    Ok(raids)
}

#[tauri::command]
pub fn db_save_raids(raids: String) -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&raids).map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM raids", [])
        .map_err(|e| e.to_string())?;

    for raid in parsed {
        // Find existing id logic used name, keeping consistent even if potentially risky if no unique id
        // The original code used raid["name"] as id.
        // Create a unique composite ID to avoid UNIQUE constraint violations
        let name = raid["name"].as_str().unwrap_or_default();
        let player_count = raid["playerCount"].as_i64().unwrap_or(0);
        let difficulty = raid["difficulty"].as_str().unwrap_or("NORMAL");
        let id = format!("{}-{}-{}", name, player_count, difficulty);

        tx.execute(
            "INSERT INTO raids (id, data) VALUES (?, ?)",
            params![id, raid.to_string()],
        )
        .map_err(|e| e.to_string())?;
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
