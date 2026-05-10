use rusqlite::Connection;

pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V13 迁移开始 ==========");

    log::info!("V13 迁移：修正 instance_types 名称");
    conn.execute(
        "UPDATE instance_types SET name = '百战异闻录' WHERE type = 'baizhan' AND name = '百战'",
        [],
    )
    .map_err(|e| e.to_string())?;

    log::info!("========== V13 迁移完成 ==========");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE instance_types (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                type TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL
            );
            "#,
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_v13_fixes_baizhan_name() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO instance_types (type, name) VALUES ('baizhan', '百战')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let name: String = conn
            .query_row(
                "SELECT name FROM instance_types WHERE type = 'baizhan'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "百战异闻录");
    }

    #[test]
    fn test_v13_idempotent() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO instance_types (type, name) VALUES ('baizhan', '百战异闻录')",
            [],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let name: String = conn
            .query_row(
                "SELECT name FROM instance_types WHERE type = 'baizhan'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "百战异闻录");
    }
}
