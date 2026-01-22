use rusqlite::{Connection, Result, params};
use std::path::PathBuf;

const DATABASE_NAME: &str = "jx3-raid-manager.db";

pub fn get_db_path() -> Result<PathBuf, String> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| "无法获取用户主目录".to_string())?;
    
    let db_path = home_dir.join(".jx3-raid-manager").join(DATABASE_NAME);
    
    std::fs::create_dir_all(
        db_path.parent().ok_or_else(|| "无法获取父目录".to_string())?
    ).map_err(|e| e.to_string())?;
    
    Ok(db_path)
}

pub fn init_db() -> Result<Connection, String> {
    let path = get_db_path()?;
    
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    
    conn.execute_batch(r#"
        CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            data TEXT
        );
        
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
        
        CREATE TABLE IF NOT EXISTS migration_log (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            localstorage_data_exists INTEGER DEFAULT 0,
            localstorage_data_hash TEXT,
            migrated_at TEXT,
            migrated_version TEXT,
            status TEXT DEFAULT 'pending',
            error_message TEXT
        );
    "#).map_err(|e| e.to_string())?;
    
    Ok(conn)
}

#[tauri::command]
pub fn db_init() -> Result<(), String> {
    init_db()?;
    Ok(())
}

#[tauri::command]
pub fn db_get_migration_status() -> Result<Option<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT status, migrated_at, error_message FROM migration_log WHERE id = 1").map_err(|e| e.to_string())?;
    let result: Option<String> = stmt.query_row([], |row| row.get(0)).ok();
    Ok(result)
}

#[tauri::command]
pub fn db_check_migration_completed() -> Result<bool, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let status: Option<String> = conn.query_row(
        "SELECT status FROM migration_log WHERE id = 1", 
        [], |row| row.get(0)
    ).ok();
    
    Ok(status == Some("completed".to_string()))
}

#[tauri::command]
pub fn db_set_migration_status(status: String, error_message: Option<String>) -> Result<(), String> {
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
    let mut stmt = conn.prepare("SELECT data FROM accounts").map_err(|e| e.to_string())?;
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
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&accounts)
        .map_err(|e| e.to_string())?;
    
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM accounts", []).map_err(|e| e.to_string())?;
    
    for account in parsed {
        let id = account["id"].as_str().unwrap_or_default().to_string();
        tx.execute(
            "INSERT INTO accounts (id, data) VALUES (?, ?)",
            params![id, account.to_string()],
        ).map_err(|e| e.to_string())?;
    }
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_records() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT data FROM records").map_err(|e| e.to_string())?;
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
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&records)
        .map_err(|e| e.to_string())?;
    
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    
    for record in parsed {
        let id = record["id"].as_str().unwrap_or_default().to_string();
        tx.execute(
            "INSERT OR REPLACE INTO records (id, data) VALUES (?, ?)",
            params![id, record.to_string()],
        ).map_err(|e| e.to_string())?;
    }
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_raids() -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT data FROM raids").map_err(|e| e.to_string())?;
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
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&raids)
        .map_err(|e| e.to_string())?;
    
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 全量同步：先清空表，再插入
    tx.execute("DELETE FROM raids", []).map_err(|e| e.to_string())?;
    
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
        ).map_err(|e| e.to_string())?;
    }
    
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_config() -> Result<Option<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT value FROM config WHERE id = 1").map_err(|e| e.to_string())?;
    let config: Option<String> = stmt.query_row([], |row| row.get(0)).ok();
    Ok(config)
}

#[tauri::command]
pub fn db_get_config_debug() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    
    let mut result = String::new();
    result.push_str("=== 配置数据调试报告 ===\n\n");
    
    // 检查配置表是否存在
    let table_exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='config'",
        [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    result.push_str(&format!("配置表存在: {}\n", if table_exists > 0 { "是" } else { "否" }));
    
    // 检查是否有配置记录
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM config WHERE id = 1", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    result.push_str(&format!("配置记录数: {}\n", count));
    
    if count > 0 {
        let config: Option<String> = conn.query_row(
            "SELECT value FROM config WHERE id = 1", [], |row| row.get(0)
        ).ok();
        
        if let Some(config_str) = &config {
            result.push_str(&format!("配置数据长度: {} bytes\n", config_str.len()));
            result.push_str(&format!("配置数据预览: {}\n", &config_str[..std::cmp::min(config_str.len(), 200)]));
            
            // 尝试解析 JSON
            match serde_json::from_str::<serde_json::Value>(&config_str) {
                Ok(json) => {
                    result.push_str("JSON解析: 成功\n");
                    result.push_str(&format!("配置字段数: {}\n", json.as_object().map(|o| o.len()).unwrap_or(0)));
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
    conn.execute("DELETE FROM config WHERE id = 1", []).map_err(|e| e.to_string())?;
    
    // 插入新配置
    conn.execute(
        "INSERT INTO config (id, value) VALUES (1, ?)",
        params![default_config],
    ).map_err(|e| e.to_string())?;
    
    Ok(format!("✓ 配置已重置\n  新配置长度: {} bytes", default_config.len()))
}

#[tauri::command]
pub fn db_save_config(config: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO config (id, value) VALUES (1, ?)",
        params![config],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_add_record(record: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let parsed: serde_json::Value = serde_json::from_str(&record)
        .map_err(|e| e.to_string())?;
    
    let id = parsed["id"].as_str().unwrap_or_default().to_string();
    
    conn.execute(
        "INSERT OR REPLACE INTO records (id, data) VALUES (?, ?)",
        params![id, record],
    ).map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub fn db_delete_record(record_id: String) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM records WHERE id = ?",
        params![record_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn db_get_records_by_raid(_raid_id: String) -> Result<Vec<String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT data FROM records").map_err(|e| e.to_string())?;
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
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM accounts", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));
    
    // 检查是否有重复的 id（理论上不应该有，因为是 PRIMARY KEY）
    let duplicate_accounts: Vec<(String, i64)> = conn.prepare("
        SELECT id, COUNT(*) as cnt 
        FROM accounts 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ").map_err(|e| e.to_string())?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    
    if duplicate_accounts.is_empty() {
        result.push_str("✓ 无重复账号（id 字段）\n");
    } else {
        result.push_str(&format!("✗ 发现 {} 个重复的 id:\n", duplicate_accounts.len()));
        for (id, cnt) in &duplicate_accounts {
            result.push_str(&format!("  - {}: {} 次\n", id, cnt));
        }
    }
    
    // 分析记录重复
    result.push_str("\n【记录表 (records) 分析】\n");
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM records", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));
    
    let duplicate_records: Vec<(String, i64)> = conn.prepare("
        SELECT id, COUNT(*) as cnt 
        FROM records 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ").map_err(|e| e.to_string())?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    
    if duplicate_records.is_empty() {
        result.push_str("✓ 无重复记录\n");
    } else {
        result.push_str(&format!("✗ 发现 {} 个重复的 id:\n", duplicate_records.len()));
        for (id, cnt) in &duplicate_records {
            result.push_str(&format!("  - {}: {} 次\n", id, cnt));
        }
    }
    
    // 分析副本重复（这个可能有重复因为 name 可能相同但日期不同）
    result.push_str("\n【副本表 (raids) 分析】\n");
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM raids", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    result.push_str(&format!("总记录数: {}\n", count));
    
    let duplicate_raids: Vec<(String, i64)> = conn.prepare("
        SELECT id, COUNT(*) as cnt 
        FROM raids 
        GROUP BY id 
        HAVING COUNT(*) > 1
    ").map_err(|e| e.to_string())?
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
    let mut stmt = conn.prepare("SELECT id, data FROM accounts").map_err(|e| e.to_string())?;
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
        
        tx.execute("DELETE FROM accounts", []).map_err(|e| e.to_string())?;
        
        for (id, data) in &accounts_to_keep {
            tx.execute(
                "INSERT INTO accounts (id, data) VALUES (?, ?)",
                params![id, data],
            ).map_err(|e| e.to_string())?;
        }
        
        tx.commit().map_err(|e| e.to_string())?;
    }
    
    Ok(format!(
        "✓ 账号去重完成！\n  处理前: {} 条\n  删除: {} 条\n  保留: {} 条",
        total_before, removed, accounts_to_keep.len()
    ))
}

#[tauri::command]
pub fn db_deduplicate_raids() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    
    // 读取所有副本
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut stmt = conn.prepare("SELECT id, data FROM raids").map_err(|e| e.to_string())?;
    let mut rows_iter = stmt.query([]).map_err(|e| e.to_string())?;
    while let Some(row) = rows_iter.next().map_err(|e| e.to_string())? {
        if let (Ok(id), Ok(data)) = (row.get(0), row.get(1)) {
            rows.push((id, data));
        }
    }
    
    let total_before = rows.len();
    
    // 去重逻辑：对于同名的副本，保留最新日期的
    let mut raids_by_name: std::collections::HashMap<String, (String, String)> = std::collections::HashMap::new();
    
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
        
        tx.execute("DELETE FROM raids", []).map_err(|e| e.to_string())?;
        
        for (id, (_, data)) in &raids_by_name {
            tx.execute(
                "INSERT INTO raids (id, data) VALUES (?, ?)",
                params![id, data],
            ).map_err(|e| e.to_string())?;
        }
        
        tx.commit().map_err(|e| e.to_string())?;
    }
    
    Ok(format!(
        "✓ 副本去重完成！\n  处理前: {} 条\n  删除: {} 条\n  保留: {} 条",
        total_before, removed, raids_by_name.len()
    ))
}

#[tauri::command]
pub fn db_add_unique_constraint_raids() -> Result<String, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    
    // 由于 raids 表使用 name 作为 id，可能存在重复，
    // 我们先尝试添加唯一索引，如果失败说明有重复数据
    match conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_raids_id ON raids(id)",
        []
    ) {
        Ok(_) => Ok("✓ 已添加 raids.id 唯一索引\n  (注意: 如果存在重复数据，索引创建会失败)".to_string()),
        Err(e) => Ok(format!(
            "⚠ 添加唯一索引失败: {}\n  原因: 表中存在重复的 id\n  建议: 先执行副本去重操作",
            e
        ))
    }
}

#[tauri::command]
pub fn db_backup(backup_path: String) -> Result<(), String> {
    let src_path = get_db_path()?;
    let dest_path = PathBuf::from(backup_path);
    
    if src_path.exists() {
        std::fs::copy(&src_path, &dest_path)
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}

#[tauri::command]
pub fn db_restore(restore_path: String) -> Result<(), String> {
    let dest_path = get_db_path()?;
    let src_path = PathBuf::from(restore_path);
    
    if src_path.exists() {
        std::fs::create_dir_all(
            dest_path.parent().ok_or_else(|| "无法获取父目录".to_string())?
        ).map_err(|e| e.to_string())?;
        
        std::fs::copy(&src_path, &dest_path)
            .map_err(|e| e.to_string())?;
    }
    
    Ok(())
}
