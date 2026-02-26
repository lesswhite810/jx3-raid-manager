use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V2 迁移：创建结构化的 raids 表并初始化静态数据
///
/// 注意：从 V2 开始 raids 数据直接使用静态数据，不需要迁移旧数据
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V2 迁移：创建 raids 结构化表...");

    // Step 1: 检查是否已存在新的 raids 表
    let new_table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='raids' AND sql LIKE '%name TEXT%'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    if new_table_exists > 0 {
        log::info!("V2 迁移：raids 表已是新格式，无需创建");
        return Ok(());
    }

    // Step 2: 检查是否存在旧的 raids 表
    let old_table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='raids'",
            [],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;

    // 如果存在旧表，重命名为 legacy
    if old_table_exists > 0 {
        conn.execute("ALTER TABLE raids RENAME TO raids_legacy;", [])
            .map_err(error_to_string)?;
        log::info!("V2 迁移：已将旧 raids 表重命名为 raids_legacy");
    }

    // Step 3: 创建新的结构化表
    conn.execute_batch(
        r#"
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
    "#,
    )
    .map_err(error_to_string)?;

    log::info!("V2 迁移：已创建新的 raids 表结构");
    Ok(())
}
