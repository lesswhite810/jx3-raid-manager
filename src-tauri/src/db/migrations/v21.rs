use rusqlite::Connection;

/// V21：将一轮试炼的每次翻牌存为独立、可去重的明细。
///
/// 不迁移旧 `trial_records.card_*`：历史单牌没有足够证据归属到哪一笔
/// 翻牌，强行拆分会造成奖励和统计重复。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    super::super::ensure_trial_draws_table(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_draw_table_without_rewriting_legacy_cards() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (
                id TEXT PRIMARY KEY,
                card_1 TEXT,
                card_2 TEXT,
                card_3 TEXT,
                card_4 TEXT,
                card_5 TEXT
            );
            INSERT INTO trial_records VALUES ('legacy', 'old-card', '', '', '', '');",
        )
        .unwrap();

        migrate(&conn).unwrap();
        migrate(&conn).unwrap();

        let legacy_card: String = conn
            .query_row(
                "SELECT card_1 FROM trial_records WHERE id = 'legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let draw_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_draws", [], |row| row.get(0))
            .unwrap();
        assert_eq!(legacy_card, "old-card");
        assert_eq!(draw_count, 0);
    }
}
