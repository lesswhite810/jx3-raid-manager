use crate::db::migration::error_to_string;
use rusqlite::{params, Connection};

/// V7 迁移：
/// 1. 为账号表补充 sort_order 字段
/// 2. 按现有账号名称顺序回填排序值，兼容旧数据
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V7 迁移：开始补充账号排序字段");

    add_sort_order_column(conn)?;
    backfill_sort_order(conn)?;
    create_sort_order_index(conn)?;

    log::info!("V7 迁移：完成");
    Ok(())
}

fn add_sort_order_column(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("PRAGMA table_info(accounts)")
        .map_err(error_to_string)?;

    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    if columns.iter().any(|column| column == "sort_order") {
        return Ok(());
    }

    conn.execute(
        "ALTER TABLE accounts ADD COLUMN sort_order INTEGER DEFAULT 0",
        [],
    )
    .map_err(error_to_string)?;

    Ok(())
}

fn backfill_sort_order(conn: &Connection) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT id FROM accounts ORDER BY account_name, created_at, id")
        .map_err(error_to_string)?;

    let account_ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    for (sort_order, account_id) in account_ids.iter().enumerate() {
        conn.execute(
            "UPDATE accounts SET sort_order = ?1 WHERE id = ?2",
            params![sort_order as i64, account_id],
        )
        .map_err(error_to_string)?;
    }

    Ok(())
}

fn create_sort_order_index(conn: &Connection) -> Result<(), String> {
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_accounts_sort_order ON accounts(sort_order)",
        [],
    )
    .map_err(error_to_string)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：在内存临时数据库中重建 V6 及之前的 accounts 表（不含 sort_order）
    fn setup_v6_accounts_table(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                account_name TEXT NOT NULL,
                account_type TEXT NOT NULL DEFAULT 'OWN',
                password TEXT,
                notes TEXT,
                hidden INTEGER NOT NULL DEFAULT 0,
                disabled INTEGER NOT NULL DEFAULT 0,
                created_at TEXT,
                updated_at TEXT
            );
            "#,
        )
        .unwrap();
    }

    /// 辅助函数：插入测试账号
    fn insert_test_account(
        conn: &Connection,
        id: &str,
        name: &str,
        created_at: &str,
    ) {
        conn.execute(
            "INSERT INTO accounts (id, account_name, account_type, created_at) VALUES (?1, ?2, 'OWN', ?3)",
            rusqlite::params![id, name, created_at],
        )
        .unwrap();
    }

    /// 测试：空账号表也能正常执行迁移
    #[test]
    fn empty_accounts_table() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v6_accounts_table(&conn);

        migrate(&conn).unwrap();

        // 验证字段已添加
        let has_column: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM pragma_table_info('accounts') WHERE name = 'sort_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_column, "sort_order 字段应该已创建");

        // 验证索引已创建
        let has_index: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_accounts_sort_order'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_index, "索引 idx_accounts_sort_order 应该已创建");
    }

    /// 测试：幂等性 —— 迁移执行两次不应报错
    #[test]
    fn idempotent_migration() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v6_accounts_table(&conn);
        insert_test_account(&conn, "a1", "测试账号", "2024-01-01T00:00:00+08:00");

        migrate(&conn).unwrap();
        migrate(&conn).unwrap(); // 不应 panic 或报错

        let sort_order: i64 = conn
            .query_row(
                "SELECT sort_order FROM accounts WHERE id = 'a1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(sort_order, 0, "重复执行后 sort_order 值应保持一致");
    }

    /// 测试：按 account_name + created_at + id 顺序回填 sort_order
    #[test]
    fn backfill_respects_sort_order() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v6_accounts_table(&conn);

        // 按"错误"插入顺序插入账号（id 乱序）
        insert_test_account(&conn, "c3", "账号C", "2024-01-03T00:00:00+08:00");
        insert_test_account(&conn, "a1", "账号A", "2024-01-01T00:00:00+08:00");
        insert_test_account(&conn, "b2", "账号B", "2024-01-02T00:00:00+08:00");

        migrate(&conn).unwrap();

        // 查询排序后的账号
        let names: Vec<String> = conn
            .prepare("SELECT account_name FROM accounts ORDER BY account_name, created_at, id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(names, vec!["账号A", "账号B", "账号C"]);

        // 验证对应 sort_order
        let orders: Vec<i64> = conn
            .prepare("SELECT sort_order FROM accounts ORDER BY account_name, created_at, id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(orders, vec![0, 1, 2]);
    }

    /// 测试：created_at 相同时按 id 排序
    #[test]
    fn backfill_uses_id_as_final_tiebreaker() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v6_accounts_table(&conn);

        // 两个账号名称相同、created_at 也相同，按 id 排序
        insert_test_account(&conn, "z99", "同名账号", "2024-01-01T00:00:00+08:00");
        insert_test_account(&conn, "a11", "同名账号", "2024-01-01T00:00:00+08:00");

        migrate(&conn).unwrap();

        let ids: Vec<String> = conn
            .prepare("SELECT id FROM accounts ORDER BY account_name, created_at, id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(ids, vec!["a11", "z99"]);

        let orders: Vec<i64> = conn
            .prepare("SELECT sort_order FROM accounts ORDER BY account_name, created_at, id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert_eq!(orders, vec![0, 1]);
    }

    /// 测试：大量账号的排序值连续不间断
    #[test]
    fn large_account_set_sequential_ordering() {
        let conn = Connection::open_in_memory().unwrap();
        setup_v6_accounts_table(&conn);

        // 插入 50 条测试账号
        for i in 0..50 {
            let id = format!("acc_{:03}", i);
            let name = format!("账号_{:03}", 49 - i); // 逆序插入
            let created = format!("2024-01-{:02}T00:00:00+08:00", (i % 28) + 1);
            insert_test_account(&conn, &id, &name, &created);
        }

        migrate(&conn).unwrap();

        let orders: Vec<i64> = conn
            .prepare("SELECT sort_order FROM accounts ORDER BY account_name, created_at, id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        let expected: Vec<i64> = (0..50).collect();
        assert_eq!(orders, expected, "50条账号的 sort_order 应为 0-49 连续序列");
    }
}
