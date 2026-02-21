use rusqlite::{params, Connection};
use crate::db::migration::error_to_string;

/// V1 Migration: Split accounts table into structured accounts and roles tables
pub fn apply_v1_migration(conn: &Connection) -> Result<(), String> {
    let timestamp = chrono::Utc::now().to_rfc3339();

    // Step 1: Check if old accounts table exists
    let old_accounts_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='accounts'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if old_accounts_exists == 0 {
        return Ok(());
    }

    // Step 2: Create new structured tables
    conn.execute_batch(
        r#"
        -- Rename old accounts table
        ALTER TABLE accounts RENAME TO accounts_legacy;

        -- Create new accounts table (with password field)
        CREATE TABLE accounts (
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

        -- Create new roles table
        CREATE TABLE roles (
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

        -- Create indexes for better query performance
        CREATE INDEX IF NOT EXISTS idx_roles_account_id ON roles(account_id);
        CREATE INDEX IF NOT EXISTS idx_accounts_name ON accounts(account_name);
    "#,
    )
    .map_err(error_to_string)?;

    // Step 3: Migrate data from JSON to structured tables
    let mut stmt = conn
        .prepare("SELECT id, data FROM accounts_legacy")
        .map_err(error_to_string)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(error_to_string)?;

    for row in rows {
        let (account_id, data_str): (String, String) = row.map_err(error_to_string)?;
        let data: serde_json::Value =
            serde_json::from_str(&data_str).map_err(|e| format!("JSON parse error: {}", e))?;

        // Extract account fields (drop username, keep accountName and password)
        let account_name = data["accountName"]
            .as_str()
            .or(data["username"].as_str())
            .unwrap_or("")
            .to_string();
        let account_type = data["type"].as_str().unwrap_or("OWN").to_string();
        let password = data["password"].as_str().map(|s| s.to_string());
        let notes = data["notes"].as_str().map(|s| s.to_string());
        let hidden = data["hidden"].as_bool().unwrap_or(false) as i32;
        let disabled = data["disabled"].as_bool().unwrap_or(false) as i32;

        // Insert into new accounts table
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type, password, notes, hidden, disabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                account_id,
                account_name,
                account_type,
                password,
                notes,
                hidden,
                disabled,
                timestamp,
                timestamp
            ],
        )
        .map_err(error_to_string)?;

        // Extract and insert roles
        if let Some(roles_array) = data["roles"].as_array() {
            for role_json in roles_array {
                let role_id = role_json["id"]
                    .as_str()
                    .or(role_json["name"].as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                let role_name = role_json["name"].as_str().unwrap_or("").to_string();
                let server = role_json["server"].as_str().map(|s| s.to_string());
                let region = role_json["region"].as_str().map(|s| s.to_string());
                let sect = role_json["sect"].as_str().map(|s| s.to_string());
                let role_disabled = role_json["disabled"].as_bool().unwrap_or(false) as i32;

                // equipmentScore might be string or number
                let equipment_score = role_json["equipmentScore"]
                    .as_i64()
                    .or(role_json["equipmentScore"]
                        .as_str()
                        .and_then(|s| s.parse().ok()));

                conn.execute(
                    "INSERT INTO roles (id, account_id, name, server, region, sect, equipment_score, disabled, created_at, updated_at)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    params![
                        role_id,
                        account_id,
                        role_name,
                        server,
                        region,
                        sect,
                        equipment_score,
                        role_disabled,
                        timestamp,
                        timestamp
                    ],
                )
                .map_err(error_to_string)?;
            }
        }
    }

    Ok(())
}
