use rusqlite::{params, Connection};

/// V17 迁移：副本名修正
///
/// 同步 static_raids.json 中的副本命名变更：
/// 1. "持国回忆录" → "持国天王回忆录"
///    - 更新 raids 表 id（10人普通持国回忆录 → 10人普通持国天王回忆录，
///      25人英雄持国回忆录 → 25人英雄持国天王回忆录）
///    - 更新 raids.name
///    - 更新 records.data 中 json_extract 的 raidName 字段
///
/// 2. 太原之战·夜守孤城、太原之战·逐虎驱狼 配置清理
///    - 删除错误的 "25人普通" 配置（与用户认知不符）
///    - 删除错误的 "10人英雄" 配置（与用户认知不符）
///    - 添加正确的 "10人挑战"、"25人挑战" 配置
///    - 实际只删除"10人英雄"/"25人普通"对应的 raids.id 记录，
///      新配置由 init_static_raids 在 V17 之后自动写入（INSERT OR IGNORE 跳过已存在）
///
/// 迁移策略：
/// 1. 先处理"持国天王回忆录"重命名（id 变更触发 records.data 同步）
/// 2. 再处理太原之战 系列配置清理（仅删除多余 id，新配置由 init_static_raids 自动写入）
/// 3. 最后调用 init_static_raids（apply_migration 完成后由 upgrade_db 统一调用）
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V17 迁移开始 ==========");

    rename_chiguo_raid(conn)?;
    cleanup_taiyuan_raids(conn)?;

    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_config (key, value, updated_at) VALUES ('v17_raid_name_fix_migrated', 'true', ?1)",
        params![now],
    )
    .map_err(|e| e.to_string())?;

    log::info!("========== V17 迁移完成 ==========");
    Ok(())
}

/// 副本改名：持国回忆录 → 持国天王回忆录
///
/// 注意：SQLite 不支持直接 UPDATE 主键，需通过事务：
/// 1. 临时禁用外键约束（PRAGMA defer_foreign_keys = ON）
/// 2. 备份关联记录
/// 3. 删除旧 id
/// 4. 插入新 id
/// 5. 更新关联 records.data
fn rename_chiguo_raid(conn: &Connection) -> Result<(), String> {
    log::info!("[V17] 处理副本名修正：持国回忆录 → 持国天王回忆录");

    // 旧 id → 新 id 映射
    let id_renames = [
        ("10人普通持国回忆录", "10人普通持国天王回忆录"),
        ("25人英雄持国回忆录", "25人英雄持国天王回忆录"),
    ];

    // 1. 检查旧 id 是否存在（不存在则说明数据库已是新版本，跳过）
    let mut has_old_id = false;
    for (old_id, _) in &id_renames {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM raids WHERE id = ?1",
                params![old_id],
                |row| row.get(0),
            )
            .unwrap_or(false);
        if exists {
            has_old_id = true;
            break;
        }
    }

    if !has_old_id {
        log::info!("[V17] 数据库中无旧副本 id，跳过改名");
        return Ok(());
    }

    // 2. 由于 SQLite UPDATE 主键的特殊性，先收集所有引用旧 id 的 records.data，
    //    然后删除旧 raids id（会因外键失败），使用新 id 重新插入
    //    这里采用更安全的方式：临时关闭外键检查
    conn.execute("PRAGMA foreign_keys = OFF", [])
        .map_err(|e| e.to_string())?;

    for (old_id, new_id) in &id_renames {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM raids WHERE id = ?1",
                params![old_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if !exists {
            continue;
        }

        // 读取旧 raids 记录的所有字段
        let (name, difficulty, player_count, version, notes, is_active, is_static): (
            String, String, i64, Option<String>, Option<String>, Option<i64>, Option<i64>,
        ) = conn
            .query_row(
                "SELECT name, difficulty, player_count, version, notes, is_active, is_static FROM raids WHERE id = ?1",
                params![old_id],
                |row| Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                )),
            )
            .map_err(|e| e.to_string())?;

        // 新 name
        let new_name = name.replace("持国回忆录", "持国天王回忆录");

        // 检查新 id 是否已存在（避免重复插入）
        let new_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM raids WHERE id = ?1",
                params![new_id],
                |row| row.get(0),
            )
            .unwrap_or(false);

        if new_exists {
            log::info!("[V17] 新 id {} 已存在，仅删除旧 id {}", new_id, old_id);
            conn.execute("DELETE FROM raids WHERE id = ?1", params![old_id])
                .map_err(|e| e.to_string())?;
            continue;
        }

        // 删除旧 id 记录
        conn.execute("DELETE FROM raids WHERE id = ?1", params![old_id])
            .map_err(|e| e.to_string())?;

        // 插入新 id 记录
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version, notes, is_active, is_static) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![new_id, new_name, difficulty, player_count, version, notes, is_active, is_static],
        )
        .map_err(|e| e.to_string())?;

        log::info!("[V17] 副本 id 重命名：{} → {}（name: {} → {}）", old_id, new_id, name, new_name);

        // 更新关联 records.data 中 json_extract 的 raidName 字段
        // records.data 是 JSON 字符串，需要更新其中 $.raidName 的值
        let mut stmt = conn
            .prepare("SELECT id, data FROM records WHERE json_extract(data, '$.raidName') = ?1")
            .map_err(|e| e.to_string())?;

        let records: Vec<(String, String)> = stmt
            .query_map(params![old_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        drop(stmt);

        let mut updated_records = 0;
        for (record_id, data_str) in records {
            let mut data: serde_json::Value = match serde_json::from_str(&data_str) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if let Some(obj) = data.as_object_mut() {
                obj.insert("raidName".to_string(), serde_json::Value::String(new_id.to_string()));
            }

            let new_data = match serde_json::to_string(&data) {
                Ok(s) => s,
                Err(_) => continue,
            };

            conn.execute(
                "UPDATE records SET data = ?1 WHERE id = ?2",
                params![new_data, record_id],
            )
            .map_err(|e| e.to_string())?;
            updated_records += 1;
        }

        if updated_records > 0 {
            log::info!("[V17] 更新 {} 条 records.data 的 raidName：{} → {}", updated_records, old_id, new_id);
        }
    }

    conn.execute("PRAGMA foreign_keys = ON", [])
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// 太原之战 系列配置清理
///
/// 删除错误的副本配置（10人英雄、25人普通），由 init_static_raids 在迁移完成后
/// 自动通过 INSERT OR IGNORE 写入正确的 10人挑战、25人挑战 配置。
///
/// 仅处理用户确认需要修正的两个副本：
/// - 太原之战·夜守孤城
/// - 太原之战·逐虎驱狼
fn cleanup_taiyuan_raids(conn: &Connection) -> Result<(), String> {
    log::info!("[V17] 处理太原之战 系列配置清理");

    // 旧错误配置 → 删除
    let obsolete_ids = [
        "10人英雄太原之战·夜守孤城",
        "25人普通太原之战·夜守孤城",
        "10人英雄太原之战·逐虎驱狼",
        "25人普通太原之战·逐虎驱狼",
    ];

    let mut deleted_total = 0;
    for old_id in &obsolete_ids {
        let deleted = conn
            .execute("DELETE FROM raids WHERE id = ?1", params![old_id])
            .map_err(|e| e.to_string())?;
        if deleted > 0 {
            log::info!("[V17] 删除错误配置：{}", old_id);
            deleted_total += deleted;
        }
    }

    if deleted_total > 0 {
        log::info!("[V17] 共删除 {} 条太原之战 系列错误配置", deleted_total);
    } else {
        log::info!("[V17] 无需删除太原之战 系列配置（数据库已是新版本）");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_prerequisites(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE raids (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                difficulty TEXT NOT NULL DEFAULT '普通',
                player_count INTEGER NOT NULL DEFAULT 25,
                version TEXT,
                notes TEXT,
                is_active INTEGER DEFAULT 1,
                is_static INTEGER DEFAULT 0,
                season_id INTEGER
            );
            CREATE TABLE records (
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
            CREATE TABLE app_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE schema_versions (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL,
                description TEXT
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn test_v17_renames_chiguo_raid_ids() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 插入旧 id 记录
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version) VALUES (?, ?, ?, ?, ?)",
            params!["10人普通持国回忆录", "持国回忆录", "普通", 10, "巴蜀风云"],
        ).unwrap();
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version) VALUES (?, ?, ?, ?, ?)",
            params!["25人英雄持国回忆录", "持国回忆录", "英雄", 25, "巴蜀风云"],
        ).unwrap();

        migrate(&conn).unwrap();

        // 验证旧 id 不存在
        let old_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM raids WHERE id IN ('10人普通持国回忆录', '25人英雄持国回忆录')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_count, 0, "旧 id 应被删除");

        // 验证新 id 存在且 name 已修正
        let new_name: String = conn
            .query_row(
                "SELECT name FROM raids WHERE id = '10人普通持国天王回忆录'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_name, "持国天王回忆录");

        let new_name2: String = conn
            .query_row(
                "SELECT name FROM raids WHERE id = '25人英雄持国天王回忆录'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(new_name2, "持国天王回忆录");
    }

    #[test]
    fn test_v17_updates_records_data_raid_name() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 插入旧 id 记录和关联 records
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version) VALUES (?, ?, ?, ?, ?)",
            params!["10人普通持国回忆录", "持国回忆录", "普通", 10, "巴蜀风云"],
        ).unwrap();
        conn.execute(
            "INSERT INTO records (id, data, raid_name) VALUES (?, ?, ?)",
            params!["rec1", r#"{"raidName":"10人普通持国回忆录","bosses":[]}"#, "持国回忆录"],
        ).unwrap();

        migrate(&conn).unwrap();

        // 验证 records.data 中的 raidName 已更新
        let data: String = conn
            .query_row("SELECT data FROM records WHERE id = 'rec1'", [], |row| row.get(0))
            .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(parsed["raidName"], "10人普通持国天王回忆录");
    }

    #[test]
    fn test_v17_deletes_obsolete_taiyuan_configs() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 插入错误的旧配置
        for id in &[
            "10人英雄太原之战·夜守孤城",
            "25人普通太原之战·夜守孤城",
            "10人英雄太原之战·逐虎驱狼",
            "25人普通太原之战·逐虎驱狼",
        ] {
            conn.execute(
                "INSERT INTO raids (id, name, difficulty, player_count, version) VALUES (?, ?, ?, ?, ?)",
                params![id, "太原之战·夜守孤城", "普通", 25, "安史之乱"],
            ).unwrap();
        }

        migrate(&conn).unwrap();

        // 验证错误配置已删除
        for id in &[
            "10人英雄太原之战·夜守孤城",
            "25人普通太原之战·夜守孤城",
            "10人英雄太原之战·逐虎驱狼",
            "25人普通太原之战·逐虎驱狼",
        ] {
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM raids WHERE id = ?1", params![id], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "错误配置应被删除: {}", id);
        }
    }

    #[test]
    fn test_v17_idempotent_no_old_data() {
        // 数据库中已是新版本（无旧记录），迁移应安全执行
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        // 只插入新版本记录
        conn.execute(
            "INSERT INTO raids (id, name, difficulty, player_count, version) VALUES (?, ?, ?, ?, ?)",
            params!["10人普通持国天王回忆录", "持国天王回忆录", "普通", 10, "巴蜀风云"],
        ).unwrap();

        migrate(&conn).unwrap();

        // 新记录应保留
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM raids WHERE id = '10人普通持国天王回忆录'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_v17_writes_migration_flag() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        migrate(&conn).unwrap();

        let value: String = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = 'v17_raid_name_fix_migrated'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, "true");
    }
}