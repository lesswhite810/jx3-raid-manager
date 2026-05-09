use rusqlite::{params, Connection};

fn column_exists(conn: &Connection, table_name: &str, column_name: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare(&format!("PRAGMA table_info({})", table_name))
        .map_err(|e| e.to_string())?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?;

    for column in columns {
        if column.map_err(|e| e.to_string())? == column_name {
            return Ok(true);
        }
    }

    Ok(false)
}

fn add_column_if_missing(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), String> {
    if column_exists(conn, table_name, column_name)? {
        return Ok(());
    }

    conn.execute(
        &format!(
            "ALTER TABLE {} ADD COLUMN {} {}",
            table_name, column_name, definition
        ),
        [],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_record_date(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value.as_str().and_then(|date| {
            date.parse::<i64>().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc3339(date)
                    .ok()
                    .map(|parsed| parsed.timestamp_millis())
            })
        })
    })
}

pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V12 迁移开始 ==========");
    log::info!("V12 迁移：为副本记录补充查询索引字段");

    add_column_if_missing(conn, "records", "raid_name", "TEXT")?;
    add_column_if_missing(conn, "records", "account_id", "TEXT")?;
    add_column_if_missing(conn, "records", "role_id", "TEXT")?;
    add_column_if_missing(conn, "records", "record_date", "INTEGER")?;
    add_column_if_missing(conn, "records", "record_type", "TEXT")?;

    let mut stmt = conn
        .prepare("SELECT id, data FROM records")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    let mut updates = Vec::new();
    for row in rows {
        let (id, data) = row.map_err(|e| e.to_string())?;
        let parsed: serde_json::Value = match serde_json::from_str(&data) {
            Ok(value) => value,
            Err(_) => continue,
        };

        updates.push((
            id,
            parsed["raidName"].as_str().map(|value| value.to_string()),
            parsed["accountId"].as_str().map(|value| value.to_string()),
            parsed["roleId"].as_str().map(|value| value.to_string()),
            parse_record_date(&parsed["date"]),
            parsed["type"]
                .as_str()
                .map(|value| value.to_string())
                .unwrap_or_else(|| "raid".to_string()),
        ));
    }

    for (id, raid_name, account_id, role_id, record_date, record_type) in updates {
        conn.execute(
            "UPDATE records
             SET raid_name = ?2, account_id = ?3, role_id = ?4, record_date = ?5, record_type = ?6
             WHERE id = ?1",
            params![id, raid_name, account_id, role_id, record_date, record_type],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute_batch(
        r#"
        CREATE INDEX IF NOT EXISTS idx_records_raid_name ON records(raid_name);
        CREATE INDEX IF NOT EXISTS idx_records_account_id ON records(account_id);
        CREATE INDEX IF NOT EXISTS idx_records_role_id ON records(role_id);
        CREATE INDEX IF NOT EXISTS idx_records_record_date ON records(record_date);
        "#,
    )
    .map_err(|e| e.to_string())?;

    log::info!("========== V12 迁移完成 ==========");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backfills_record_lookup_columns() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO records (id, data) VALUES (?1, ?2)",
            params![
                "record-1",
                r#"{"id":"record-1","raidName":"25人英雄阕风悬城","accountId":"account-1","roleId":"role-1","date":1710000000000}"#
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let metadata: (String, String, String, i64, String) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id, record_date, record_type FROM records WHERE id = 'record-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();

        assert_eq!(
            metadata,
            (
                "25人英雄阕风悬城".to_string(),
                "account-1".to_string(),
                "role-1".to_string(),
                1710000000000,
                "raid".to_string()
            )
        );
    }

    #[test]
    fn backfills_rfc3339_record_date() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO records (id, data) VALUES (?1, ?2)",
            params![
                "record-1",
                r#"{"id":"record-1","raidName":"25人英雄阕风悬城","date":"2026-05-08T12:00:00+08:00"}"#
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let record_date: i64 = conn
            .query_row(
                "SELECT record_date FROM records WHERE id = 'record-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(record_date, 1778212800000);
    }

    #[test]
    fn handles_missing_optional_fields() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO records (id, data) VALUES (?1, ?2)",
            params![
                "record-1",
                r#"{"id":"record-1","raidName":"25人英雄阕风悬城","date":1710000000000}"#
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let metadata: (Option<String>, Option<String>, Option<String>, i64, String) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id, record_date, record_type FROM records WHERE id = 'record-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();

        assert_eq!(metadata.0, Some("25人英雄阕风悬城".to_string()));
        assert_eq!(metadata.1, None);
        assert_eq!(metadata.2, None);
        assert_eq!(metadata.3, 1710000000000);
        assert_eq!(metadata.4, "raid");
    }

    #[test]
    fn handles_invalid_json() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO records (id, data) VALUES (?1, ?2)",
            params!["record-1", r#"{"id":"record-1","invalid json"#],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let metadata: (Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id, record_date, record_type FROM records WHERE id = 'record-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();

        assert_eq!(metadata.0, None);
        assert_eq!(metadata.1, None);
        assert_eq!(metadata.2, None);
        assert_eq!(metadata.3, None);
        assert_eq!(metadata.4, None);
    }

    #[test]
    fn handles_explicit_type_field() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO records (id, data) VALUES (?1, ?2)",
            params![
                "record-1",
                r#"{"id":"record-1","raidName":"试炼之地","accountId":"account-1","roleId":"role-1","date":1710000000000,"type":"trial"}"#
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let record_type: String = conn
            .query_row(
                "SELECT record_type FROM records WHERE id = 'record-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(record_type, "trial");
    }

    #[test]
    fn handles_mixed_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();

        conn.execute_batch(
            r#"
            INSERT INTO records (id, data) VALUES 
            ('record-1', '{"id":"record-1","raidName":"25人英雄阕风悬城","accountId":"account-1","roleId":"role-1","date":1710000000000}'),
            ('record-2', '{"id":"record-2","raidName":"百战异闻录","accountId":"account-2","date":1710100000000}'),
            ('record-3', '{"id":"record-3","date":1710200000000}'),
            ('record-4', 'invalid json');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 4);

        let record1: (String, String, String) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id FROM records WHERE id = 'record-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(record1, ("25人英雄阕风悬城".to_string(), "account-1".to_string(), "role-1".to_string()));

        let record2: (String, String, Option<String>) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id FROM records WHERE id = 'record-2'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(record2.0, "百战异闻录");
        assert_eq!(record2.1, "account-2");
        assert_eq!(record2.2, None);

        let record3: (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT raid_name, account_id, role_id FROM records WHERE id = 'record-3'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(record3.0, None);
        assert_eq!(record3.1, None);
        assert_eq!(record3.2, None);
    }
}
