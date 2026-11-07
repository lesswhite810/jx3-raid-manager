use rusqlite::Connection;

/// V22：保存逐牌的人工装备覆盖。
///
/// 覆盖表只记录新的人工操作；不把 V21 已有牌面推断成手工确认，避免把
/// 历史自动识别或未知信息误称为人工完整核对。`equipment_id IS NULL` 是
/// 有意义的人工清空，后续自动识别必须保留它。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    // 显式建覆盖表：`ensure_trial_draws_table` 内部虽然会顺带创建它，但那是
    // 一个隐式副作用。历史迁移的语义不能随 db.rs 的重构漂移，因此这里再调用
    // 一次（`CREATE TABLE IF NOT EXISTS`，幂等、零风险）。
    super::super::ensure_trial_draw_equipment_manual_overrides_table(conn)?;
    super::super::ensure_trial_draws_table(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrades_v21_draws_without_reclassifying_existing_cards() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (id TEXT PRIMARY KEY);
             CREATE TABLE trial_draws (
                id TEXT PRIMARY KEY,
                trial_record_id TEXT NOT NULL,
                source_key TEXT NOT NULL UNIQUE,
                sequence INTEGER NOT NULL DEFAULT 0,
                receipt_time INTEGER NOT NULL,
                receipt_hash TEXT,
                reward_items TEXT NOT NULL DEFAULT '[]',
                reward_sources TEXT NOT NULL DEFAULT '[]',
                stamina_cost INTEGER,
                stamina_costs TEXT NOT NULL DEFAULT '[]',
                stamina_sources TEXT NOT NULL DEFAULT '[]',
                flipped_index INTEGER NOT NULL DEFAULT 0,
                position_manual INTEGER NOT NULL DEFAULT 0,
                auto_observed_start_ms INTEGER,
                auto_observed_end_ms INTEGER,
                auto_equipment_observed_start_ms INTEGER,
                auto_equipment_observed_end_ms INTEGER,
                card_1 TEXT NOT NULL DEFAULT '', card_2 TEXT NOT NULL DEFAULT '',
                card_3 TEXT NOT NULL DEFAULT '', card_4 TEXT NOT NULL DEFAULT '',
                card_5 TEXT NOT NULL DEFAULT '',
                equipment_unreviewed INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL DEFAULT 'auto', updated_at TEXT
             );
             INSERT INTO trial_records VALUES ('record');",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO trial_draws (
                id, trial_record_id, source_key, sequence, receipt_time,
                card_1, equipment_unreviewed
             ) VALUES ('draw', 'record', 'v21-source', 1, 1, 'old-card', 0)",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();
        migrate(&conn).unwrap();

        let saved: (String, i64) = conn
            .query_row(
                "SELECT card_1, equipment_unreviewed FROM trial_draws WHERE id = 'draw'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let override_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM trial_draw_equipment_manual_overrides",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved, ("old-card".to_string(), 0));
        assert_eq!(override_count, 0, "V21 牌面不能被推断为人工证据");
    }
}
