use rusqlite::Connection;

/// V11 迁移：清理重复的旧格式装备数据
///
/// 对于同一名称的装备，如果同时存在旧格式（ID 包含下划线）和新格式（纯数字 ID），
/// 则删除旧格式装备，保留新格式装备。
/// 如果只有旧格式装备（没有对应的新格式），则保留旧格式装备以支持历史记录。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V11 迁移开始 ==========");
    log::info!("V11 迁移：清理重复的旧格式装备数据");

    conn.execute_batch(
        r#"
        CREATE TEMP TABLE IF NOT EXISTS temp_old_format_duplicates AS
        SELECT e1.rowid
        FROM equipments e1
        WHERE e1.id LIKE '%\_%' ESCAPE '\'
        AND EXISTS (
            SELECT 1 
            FROM equipments e2 
            WHERE e2.name = e1.name 
            AND e2.id NOT LIKE '%\_%' ESCAPE '\'
        );

        DELETE FROM equipments 
        WHERE rowid IN (SELECT rowid FROM temp_old_format_duplicates);

        DROP TABLE IF EXISTS temp_old_format_duplicates;
        "#,
    )
    .map_err(|e| format!("V11 迁移失败: {}", e))?;

    let deleted_count: i64 = conn
        .query_row("SELECT changes()", [], |row| row.get(0))
        .unwrap_or(0);

    if deleted_count > 0 {
        log::info!("V11 迁移：删除 {} 条重复的旧格式装备", deleted_count);
    }

    let remaining_old: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM equipments WHERE id LIKE '%\\_%' ESCAPE '\\'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if remaining_old > 0 {
        log::info!("V11 迁移：保留 {} 条旧格式装备（无对应新格式）", remaining_old);
    }

    log::info!("========== V11 迁移完成 ==========");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        
        conn.execute(
            "CREATE TABLE equipments (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                icon_id INTEGER,
                quality INTEGER
            )",
            [],
        )
        .unwrap();
        
        conn
    }

    #[test]
    fn test_v11_migration_removes_duplicates() {
        let conn = setup_test_db();
        
        conn.execute(
            "INSERT INTO equipments (id, name, icon_id, quality) VALUES 
            ('12345', '测试装备1', 100, 5),
            ('12345_67890', '测试装备1', 100, 5),
            ('54321', '测试装备2', 200, 4),
            ('54321_98765', '测试装备2', 200, 4),
            ('11111_22222', '测试装备3', 300, 3)",
            [],
        )
        .unwrap();

        let before_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(before_count, 5);

        migrate(&conn).unwrap();

        let after_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(after_count, 3);

        let old_format_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM equipments WHERE id LIKE '%\\_%' ESCAPE '\\'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_format_count, 1);

        let new_format_ids: Vec<String> = conn
            .prepare("SELECT id FROM equipments WHERE id NOT LIKE '%\\_%' ESCAPE '\\' ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(new_format_ids, vec!["12345", "54321"]);

        let old_format_id: String = conn
            .query_row(
                "SELECT id FROM equipments WHERE id LIKE '%\\_%' ESCAPE '\\'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(old_format_id, "11111_22222");
    }

    #[test]
    fn test_v11_migration_no_duplicates() {
        let conn = setup_test_db();
        
        conn.execute(
            "INSERT INTO equipments (id, name, icon_id, quality) VALUES 
            ('12345', '测试装备1', 100, 5),
            ('54321', '测试装备2', 200, 4),
            ('11111_22222', '测试装备3', 300, 3)",
            [],
        )
        .unwrap();

        let before_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(before_count, 3);

        migrate(&conn).unwrap();

        let after_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(after_count, 3);
    }

    #[test]
    fn test_v11_migration_empty_table() {
        let conn = setup_test_db();

        migrate(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM equipments", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
