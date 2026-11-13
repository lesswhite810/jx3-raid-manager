use rusqlite::Connection;

/// V23 迁移：移除「装备待核对」状态列。
///
/// 「确认本轮无装备」与「已核对全部牌面」两项待确认状态已从产品中移除：
/// 所有已确认牌位的样本都直接视为已核对，概率统计不再依赖待核对标记。
/// 历史迁移（V20 添加、V21/V22 沿用）的语义保持不变，由本版本统一删除
/// 两张表上的 `equipment_unreviewed` 列。幂等：列不存在时跳过。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    for (table, context) in [
        ("trial_records", "试炼记录"),
        ("trial_draws", "翻牌明细"),
    ] {
        let column_exists: bool = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = 'equipment_unreviewed'"
                ),
                [],
                |row| row.get(0),
            )
            .map_err(|e| format!("检查 {context}表 equipment_unreviewed 列失败: {}", e))?;
        if column_exists {
            conn.execute(
                &format!("ALTER TABLE {table} DROP COLUMN equipment_unreviewed"),
                [],
            )
            .map_err(|e| format!("移除 {context}表 equipment_unreviewed 列失败: {}", e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_column_when_present_and_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (id TEXT PRIMARY KEY, equipment_unreviewed INTEGER NOT NULL DEFAULT 0);
             CREATE TABLE trial_draws (id TEXT PRIMARY KEY, equipment_unreviewed INTEGER NOT NULL DEFAULT 1);
             INSERT INTO trial_records VALUES ('r', 1);
             INSERT INTO trial_draws VALUES ('d', 1);",
        )
        .unwrap();

        migrate(&conn).unwrap();
        migrate(&conn).unwrap();

        for table in ["trial_records", "trial_draws"] {
            let has_column: bool = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) > 0 FROM pragma_table_info('{table}') WHERE name = 'equipment_unreviewed'"
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(!has_column, "{table} 的 equipment_unreviewed 列应被移除");
        }

        let record_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
            .unwrap();
        let draw_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_draws", [], |row| row.get(0))
            .unwrap();
        assert_eq!((record_count, draw_count), (1, 1), "删除列不能丢数据");
    }

    #[test]
    fn skips_missing_column_on_fresh_install() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE trial_records (id TEXT PRIMARY KEY);
             CREATE TABLE trial_draws (id TEXT PRIMARY KEY);",
        )
        .unwrap();

        migrate(&conn).unwrap();

        let record_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM trial_records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(record_count, 0);
    }
}
