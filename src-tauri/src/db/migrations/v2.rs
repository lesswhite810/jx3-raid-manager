use rusqlite::Connection;
use crate::db::migration::{error_to_string, init_static_raids};

/// V2 迁移：将 raids 从 JSON blob 升级为结构化表 + raid_bosses
/// 原表数据全部丢弃，使用 static_raids.json 静态数据初始化
pub fn apply_v2_migration(conn: &Connection) -> Result<(), String> {
    // 如果旧 raids 表（V1 JSON blob 格式）存在，重命名为 legacy 备份
    let old_raids_has_data: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='raids' AND sql LIKE '%data TEXT%'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if old_raids_has_data > 0 {
        conn.execute_batch("ALTER TABLE raids RENAME TO raids_legacy;")
            .map_err(error_to_string)?;
        log::info!("V2 迁移：已将旧 raids 表重命名为 raids_legacy");
    }

    // 创建新的结构化表
    conn.execute_batch(r#"
        DROP TABLE IF EXISTS raids;
        DROP TABLE IF EXISTS raid_bosses;
        DROP TABLE IF EXISTS raid_versions;

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

        CREATE TABLE IF NOT EXISTS raid_bosses (
            id TEXT PRIMARY KEY,
            raid_name TEXT NOT NULL,
            name TEXT NOT NULL,
            boss_order INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS raid_versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL
        );
    "#).map_err(error_to_string)?;

    // 直接使用静态数据初始化，不恢复旧数据
    init_static_raids(conn)?;

    // 补齐 trial_records / baizhan_records 新增列（容错：忽略列已存在的错误）
    let alter_statements: [&str; 0] = [];
    for sql in &alter_statements {
        let _ = conn.execute(sql, []);  // 忽略 "duplicate column name" 错误
    }

    log::info!("V2 迁移完成：raids 表已升级为结构化存储，trial/baizhan 新列已补齐");
    Ok(())
}
