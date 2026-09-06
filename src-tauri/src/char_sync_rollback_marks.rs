//! 跨角色设置同步：备份回滚标记 CRUD
//!
//! 跨角色设置同步随 v2.2.2 首次发布，用户机器上不存在历史数据，因此**不做 localStorage 迁移**：
//! 早期实现曾暂存在 webview localStorage（`jx3rm.char-sync.rollback-marks-v1`），
//! 该功能从未发布，标记统一由本模块的命令直接读写 SQLite（V18 建表）。
//!
//! 所有命令接收 camelCase 参数（与前端 services/charSync.ts 保持一致），
//! 返回 HashMap 序列化为 JSON 对象（前端对应 Record<string, string>）。

use crate::db::init_db;
use serde::Deserialize;
use std::collections::HashMap;

/// 列出所有已回滚的备份标记
///
/// 返回值：key=backup_dir，value=rolled_back_at ISO 时间字符串
#[tauri::command]
pub fn list_char_sync_rollback_marks() -> Result<HashMap<String, String>, String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT backup_dir, rolled_back_at FROM char_sync_rollback_marks")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut map: HashMap<String, String> = HashMap::new();
    for r in rows.flatten() {
        map.insert(r.0, r.1);
    }
    Ok(map)
}

/// 写入或覆盖单个回滚标记
///
/// 同一 backup_dir 重复写入取最新时间戳（INSERT OR REPLACE）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetRollbackMarkArgs {
    pub backup_dir: String,
    /// ISO 时间字符串（如 `2026-09-02T00:00:00+08:00`）
    pub timestamp: String,
}

#[tauri::command]
pub fn set_char_sync_rollback_mark(args: SetRollbackMarkArgs) -> Result<(), String> {
    let conn = init_db().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
        rusqlite::params![args.backup_dir, args.timestamp],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 清空所有回滚标记（事务包裹）
///
/// 通常不需要调用：SQLite 数据库重建后表自然为空。
/// 保留此命令便于将来按角色/账号粒度重置时复用。
#[tauri::command]
pub fn clear_char_sync_rollback_marks() -> Result<(), String> {
    let mut conn = init_db().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM char_sync_rollback_marks", [])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    fn setup_table(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE char_sync_rollback_marks (
                backup_dir TEXT PRIMARY KEY,
                rolled_back_at TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
    }

    #[test]
    fn test_set_and_list_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        setup_table(&conn);

        // 直接使用 SQL 模拟（避免 init_db 单例）
        conn.execute(
            "INSERT OR REPLACE INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
            rusqlite::params!["backup-a", "2026-09-02T00:00:00+08:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
            rusqlite::params!["backup-b", "2026-09-02T00:01:00+08:00"],
        )
        .unwrap();

        let mut stmt = conn
            .prepare("SELECT backup_dir, rolled_back_at FROM char_sync_rollback_marks")
            .unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|(d, _)| d == "backup-a"));
        assert!(rows.iter().any(|(d, _)| d == "backup-b"));
    }

    #[test]
    fn test_set_or_replace_overwrites() {
        let conn = Connection::open_in_memory().unwrap();
        setup_table(&conn);

        conn.execute(
            "INSERT OR REPLACE INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
            rusqlite::params!["backup-x", "2026-09-02T00:00:00+08:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
            rusqlite::params!["backup-x", "2026-09-02T01:00:00+08:00"],
        )
        .unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM char_sync_rollback_marks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "INSERT OR REPLACE 应覆盖而非新增");

        let ts: String = conn
            .query_row(
                "SELECT rolled_back_at FROM char_sync_rollback_marks WHERE backup_dir = ?1",
                rusqlite::params!["backup-x"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ts, "2026-09-02T01:00:00+08:00");
    }

    #[test]
    fn test_clear_transaction() {
        let mut conn = Connection::open_in_memory().unwrap();
        setup_table(&conn);

        for i in 0..3 {
            conn.execute(
                "INSERT INTO char_sync_rollback_marks (backup_dir, rolled_back_at) VALUES (?1, ?2)",
                rusqlite::params![format!("backup-{}", i), "2026-09-02T00:00:00+08:00"],
            )
            .unwrap();
        }

        let tx = conn.transaction().unwrap();
        tx.execute("DELETE FROM char_sync_rollback_marks", []).unwrap();
        tx.commit().unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM char_sync_rollback_marks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}