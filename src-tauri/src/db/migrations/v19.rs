use rusqlite::Connection;

/// V19 迁移：试炼之地自动扫描记录。
///
/// 旧记录全部来自手工录入，按已确认记录保留。自动扫描记录另存来源、状态和
/// 稳定的源证据键，删除自动记录后保留 tombstone，防止下次扫描重新插回。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    for (column, definition) in [
        ("source", "TEXT NOT NULL DEFAULT 'manual'"),
        ("status", "TEXT NOT NULL DEFAULT 'confirmed'"),
        ("source_key", "TEXT"),
    ] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records') WHERE name = ?1",
                [column],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !exists {
            conn.execute(
                &format!(
                    "ALTER TABLE trial_records ADD COLUMN {} {}",
                    column, definition
                ),
                [],
            )
            .map_err(|e| format!("为 trial_records 添加 {} 失败: {}", column, e))?;
        }
    }

    // 任何升级前的记录都是此前 UI 手工保存的数据。
    conn.execute(
        "UPDATE trial_records
         SET source = COALESCE(NULLIF(source, ''), 'manual'),
             status = COALESCE(NULLIF(status, ''), 'confirmed')",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_trial_records_source_key
         ON trial_records(source_key) WHERE source_key IS NOT NULL",
        [],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS trial_auto_scan_tombstones (
            source_key TEXT PRIMARY KEY,
            deleted_at TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_manual_trial_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (
                id TEXT PRIMARY KEY, account_id TEXT, role_id TEXT, layer INTEGER,
                bosses TEXT, card_1 TEXT, card_2 TEXT, card_3 TEXT, card_4 TEXT,
                card_5 TEXT, flipped_index INTEGER, record_type TEXT, date INTEGER,
                notes TEXT, updated_at TEXT
            );
            INSERT INTO trial_records (id, date) VALUES ('legacy', 1);",
        )
        .unwrap();

        migrate(&conn).unwrap();

        let saved: (String, String) = conn
            .query_row(
                "SELECT source, status FROM trial_records WHERE id = 'legacy'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(saved, ("manual".to_string(), "confirmed".to_string()));
        let index_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_trial_records_source_key'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_exists);
    }
}
