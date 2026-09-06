use rusqlite::{params, Connection};

/// V18 迁移：跨角色设置同步的「已回滚到此版本」标记表
///
/// 背景：
/// - 早期实现曾把该标记暂存在 webview localStorage（`jx3rm.char-sync.rollback-marks-v1`）
/// - 但跨角色设置同步本身是本版本（v2.2.2）首次发布的新功能，用户机器上不存在历史数据
/// - 因此**不做 localStorage 迁移**，标记统一直接读写 SQLite；前端不再访问该 localStorage key
///
/// 收益：
/// - NSIS 卸载钩子与「重新初始化」会随数据库一并清理，不再残留于 webview localStorage
/// - 标记纳入统一的备份与生命周期管理
///
/// 变更：
/// 1. 创建 `char_sync_rollback_marks` 表（backup_dir 主键，rolled_back_at ISO 时间字符串）
/// 2. 创建索引 `idx_char_sync_rollback_marks_rolled_back_at`，按时间倒序展示时加速
/// 3. 在 app_config 写入一次性迁移标志位 `v18_rollback_marks_table_created`
///
/// 前端策略：
/// - 无历史数据可迁移，启动时不需要任何兼容逻辑
/// - 回滚标记一律通过 list / set / clear_char_sync_rollback_mark 命令读写本表
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V18 迁移开始 ==========");

    // 1. 建表（幂等）
    conn.execute(
        "CREATE TABLE IF NOT EXISTS char_sync_rollback_marks (
            backup_dir TEXT PRIMARY KEY,
            rolled_back_at TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    log::info!("[V18] 创建 char_sync_rollback_marks 表");

    // 2. 索引（幂等）
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_char_sync_rollback_marks_rolled_back_at
         ON char_sync_rollback_marks (rolled_back_at DESC)",
        [],
    )
    .map_err(|e| e.to_string())?;
    log::info!("[V18] 创建 idx_char_sync_rollback_marks_rolled_back_at 索引");

    // 3. 写入迁移标志位（幂等：INSERT OR REPLACE）
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_config (key, value, updated_at)
         VALUES ('v18_rollback_marks_table_created', 'true', ?1)",
        params![now],
    )
    .map_err(|e| e.to_string())?;
    log::info!("[V18] 写入迁移标志位 v18_rollback_marks_table_created");

    log::info!("========== V18 迁移完成 ==========");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_prerequisites(conn: &Connection) {
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

    #[test]
    fn test_v18_creates_table_and_index() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        migrate(&conn).unwrap();

        // 表存在
        let table_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='char_sync_rollback_marks'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(table_count, 1, "char_sync_rollback_marks 表应被创建");

        // 索引存在
        let index_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_char_sync_rollback_marks_rolled_back_at'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 1, "索引应被创建");

        // 表可写入
        conn.execute(
            "INSERT INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
            params!["test-backup", "2026-09-02T00:00:00+08:00"],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM char_sync_rollback_marks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_v18_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        migrate(&conn).unwrap();
        // 重复执行不应报错
        migrate(&conn).unwrap();

        let flag: String = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = 'v18_rollback_marks_table_created'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(flag, "true");
    }

    #[test]
    fn test_v18_writes_migration_flag() {
        let conn = Connection::open_in_memory().unwrap();
        setup_prerequisites(&conn);

        migrate(&conn).unwrap();

        let value: String = conn
            .query_row(
                "SELECT value FROM app_config WHERE key = 'v18_rollback_marks_table_created'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, "true");
    }
}