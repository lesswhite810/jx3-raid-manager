use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V25 迁移：试炼截图采集增加「周翻牌次数」与「层数」识别字段。
/// - trial_capture_runs 新增 layer 字段（该轮终局的层数，-1 = 未识别）
/// - trial_capture_observations 新增 weekly_flip_count / layer 字段
///   （该帧识别到的周翻牌次数与层数，-1 = 本帧未识别）
///
/// 两张表都可能不存在（极旧版本先装 V25 才开始用试炼采集），此时
/// ensure_trial_capture_tables 会按最新结构建表，这里跳过即可。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V25 迁移开始 ==========");
    log::info!("V25 迁移：试炼截图采集新增周翻牌次数（weekly_flip_count）与层数（layer）字段");

    add_column_if_missing(
        conn,
        "trial_capture_runs",
        "layer",
        "ALTER TABLE trial_capture_runs ADD COLUMN layer INTEGER NOT NULL DEFAULT -1",
    )?;
    add_column_if_missing(
        conn,
        "trial_capture_observations",
        "weekly_flip_count",
        "ALTER TABLE trial_capture_observations ADD COLUMN weekly_flip_count INTEGER NOT NULL DEFAULT -1",
    )?;
    add_column_if_missing(
        conn,
        "trial_capture_observations",
        "layer",
        "ALTER TABLE trial_capture_observations ADD COLUMN layer INTEGER NOT NULL DEFAULT -1",
    )?;

    log::info!("========== V25 迁移完成 ==========");
    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    alter: &str,
) -> Result<(), String> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .map_err(error_to_string)?;
    if table_exists == 0 {
        log::info!("V25 迁移：表 {table} 不存在，跳过（建表时按最新结构创建）");
        return Ok(());
    }

    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(error_to_string)?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    if columns.iter().any(|name| name == column) {
        log::info!("V25 迁移：{table}.{column} 列已存在，跳过");
        return Ok(());
    }

    log::info!("V25 迁移：为 {table} 添加 {column} 列");
    conn.execute(alter, []).map_err(error_to_string)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_v25_migrate_adds_columns_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_capture_runs (
                run_key TEXT PRIMARY KEY,
                uid TEXT NOT NULL,
                map_id INTEGER NOT NULL,
                terminal_ms INTEGER NOT NULL,
                frame_count INTEGER NOT NULL DEFAULT 0,
                reconciled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
            );
            CREATE TABLE trial_capture_observations (
                run_key TEXT NOT NULL,
                at_ms INTEGER NOT NULL,
                pid INTEGER NOT NULL,
                slot INTEGER NOT NULL,
                reset_visible INTEGER NOT NULL,
                equipment_slot INTEGER NOT NULL DEFAULT 0,
                equipment_name TEXT NOT NULL DEFAULT '',
                equipment_level INTEGER NOT NULL DEFAULT 0,
                equipment_bind TEXT NOT NULL DEFAULT '',
                PRIMARY KEY (run_key, at_ms, pid, slot, reset_visible, equipment_slot, equipment_name, equipment_level, equipment_bind)
            );",
        )
        .unwrap();

        migrate(&conn).expect("V25 迁移应成功");

        let runs_layer: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('trial_capture_runs') WHERE name = 'layer'",
                [],
                |row: &rusqlite::Row| row.get(0),
            )
            .unwrap();
        assert_eq!(runs_layer, 1, "trial_capture_runs 应有 layer 列");

        for column in ["weekly_flip_count", "layer"] {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('trial_capture_observations') WHERE name = ?1",
                    [column],
                    |row: &rusqlite::Row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "trial_capture_observations 应有 {column} 列");
        }

        // 幂等：重复执行不应报错
        migrate(&conn).expect("V25 迁移应幂等");
    }

    #[test]
    fn test_v25_migrate_skips_missing_tables() {
        let conn = Connection::open_in_memory().unwrap();
        // 两张表都不存在：迁移不应失败（建表路径会按最新结构创建）。
        migrate(&conn).expect("V25 迁移在无表库上应成功");
    }
}
