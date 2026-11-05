use rusqlite::Connection;

/// V20 迁移：标记由截图识别牌位但尚未确认装备的自动试炼记录。
///
/// V19 已经可能存在于开发版用户库，因此不能修改旧迁移。仅把自动来源、
/// 尚未翻牌且五个卡位都为空的旧记录视为装备待复核；所有手工记录及已有
/// 装备信息保持原有口径。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    let column_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('trial_records') WHERE name = 'equipment_unreviewed'",
            [],
            |row| row.get(0),
        )
        .map_err(|e| format!("检查 trial_records.equipment_unreviewed 失败: {}", e))?;
    if !column_exists {
        conn.execute(
            "ALTER TABLE trial_records
             ADD COLUMN equipment_unreviewed INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("为 trial_records 添加 equipment_unreviewed 失败: {}", e))?;

        conn.execute(
            "UPDATE trial_records
             SET equipment_unreviewed = 1
             WHERE source = 'auto'
               AND COALESCE(flipped_index, 0) = 0
               AND COALESCE(card_1, '') = ''
               AND COALESCE(card_2, '') = ''
               AND COALESCE(card_3, '') = ''
               AND COALESCE(card_4, '') = ''
               AND COALESCE(card_5, '') = ''",
            [],
        )
        .map_err(|e| format!("标记旧试炼待复核装备失败: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_only_unknown_automatic_records_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL DEFAULT 'manual',
                flipped_index INTEGER,
                card_1 TEXT,
                card_2 TEXT,
                card_3 TEXT,
                card_4 TEXT,
                card_5 TEXT
            );
            INSERT INTO trial_records
                (id, source, flipped_index, card_1, card_2, card_3, card_4, card_5)
            VALUES
                ('auto-unknown', 'auto', 0, '', '', '', '', ''),
                ('auto-flipped', 'auto', 2, '', '', '', '', ''),
                ('auto-equipped', 'auto', 0, '装备', '', '', '', ''),
                ('manual-empty', 'manual', 0, '', '', '', '', '');",
        )
        .unwrap();

        migrate(&conn).unwrap();
        migrate(&conn).unwrap();

        let saved: Vec<(String, i64)> = {
            let mut statement = conn
                .prepare("SELECT id, equipment_unreviewed FROM trial_records ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(
            saved,
            vec![
                ("auto-equipped".to_string(), 0),
                ("auto-flipped".to_string(), 0),
                ("auto-unknown".to_string(), 1),
                ("manual-empty".to_string(), 0),
            ]
        );

        // A partially retried migration must not turn an explicit later manual
        // confirmation back into an unknown-equipment record.
        conn.execute(
            "UPDATE trial_records SET equipment_unreviewed = 0 WHERE id = 'auto-unknown'",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        let preserved: i64 = conn
            .query_row(
                "SELECT equipment_unreviewed FROM trial_records WHERE id = 'auto-unknown'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(preserved, 0);
    }
}
