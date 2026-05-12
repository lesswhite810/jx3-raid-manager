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

struct RecordUpdate {
    id: String,
    raid_name: Option<String>,
    account_id: Option<String>,
    role_id: Option<String>,
    record_date: Option<i64>,
    record_type: String,
}

fn extract_updates_from_rows(conn: &Connection) -> Result<Vec<RecordUpdate>, String> {
    let needs_backfill: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM records WHERE raid_name IS NULL",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;

    if !needs_backfill {
        log::info!("V12 迁移：所有记录已包含索引字段，跳过回填");
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare("SELECT id, data FROM records WHERE raid_name IS NULL")
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

        updates.push(RecordUpdate {
            id,
            raid_name: parsed["raidName"].as_str().map(|v| v.to_string()),
            account_id: parsed["accountId"].as_str().map(|v| v.to_string()),
            role_id: parsed["roleId"].as_str().map(|v| v.to_string()),
            record_date: parse_record_date(&parsed["date"]),
            record_type: parsed["type"]
                .as_str()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "raid".to_string()),
        });
    }

    Ok(updates)
}

fn apply_updates_batched(conn: &Connection, updates: &[RecordUpdate]) -> Result<usize, String> {
    if updates.is_empty() {
        return Ok(0);
    }

    let mut update_stmt = conn
        .prepare(
            "UPDATE records
             SET raid_name = ?2, account_id = ?3, role_id = ?4, record_date = ?5, record_type = ?6
             WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;

    for record in updates {
        update_stmt
            .execute(params![
                record.id,
                record.raid_name,
                record.account_id,
                record.role_id,
                record.record_date,
                record.record_type,
            ])
            .map_err(|e| e.to_string())?;
    }

    Ok(updates.len())
}

pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V12 迁移开始 ==========");
    log::info!("V12 迁移：为副本记录补充查询索引字段");

    // 检查 records 表是否存在
    let table_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='records'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(false);

    if !table_exists {
        log::info!("V12 迁移：records 表不存在，跳过迁移");
        log::info!("========== V12 迁移完成 ==========");
        return Ok(());
    }

    add_column_if_missing(conn, "records", "raid_name", "TEXT")?;
    add_column_if_missing(conn, "records", "account_id", "TEXT")?;
    add_column_if_missing(conn, "records", "role_id", "TEXT")?;
    add_column_if_missing(conn, "records", "record_date", "INTEGER")?;
    add_column_if_missing(conn, "records", "record_type", "TEXT")?;

    let updates = extract_updates_from_rows(conn)?;
    let total = updates.len();

    if total > 0 {
        log::info!("V12 迁移：需要回填 {} 条记录", total);
        let updated = apply_updates_batched(conn, &updates)?;
        log::info!("V12 迁移：已回填 {} 条记录", updated);
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

    #[test]
    fn skips_backfill_when_already_complete() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE records (
                id TEXT PRIMARY KEY,
                data TEXT,
                raid_name TEXT,
                account_id TEXT,
                role_id TEXT,
                record_date INTEGER,
                record_type TEXT
            );
            INSERT INTO records (id, data, raid_name, account_id, role_id, record_date, record_type)
            VALUES ('record-1', '{}', 'test-raid', 'acc-1', 'role-1', 1710000000000, 'raid');
            "#,
        )
        .unwrap();

        migrate(&conn).unwrap();

        let raid_name: String = conn
            .query_row(
                "SELECT raid_name FROM records WHERE id = 'record-1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raid_name, "test-raid");
    }

    #[test]
    fn batched_update_processes_all_records() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE records (id TEXT PRIMARY KEY, data TEXT)",
            [],
        )
        .unwrap();

        for i in 0..10 {
            conn.execute(
                "INSERT INTO records (id, data) VALUES (?1, ?2)",
                params![
                    format!("record-{}", i),
                    format!(
                        r#"{{"id":"record-{}","raidName":"副本{}","accountId":"acc-{}","date":{}}}"#,
                        i, i, i, 1710000000000 + i as i64 * 1000
                    ),
                ],
            )
            .unwrap();
        }

        migrate(&conn).unwrap();

        let backfilled: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM records WHERE raid_name IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(backfilled, 10);

        let raid_name: String = conn
            .query_row(
                "SELECT raid_name FROM records WHERE id = 'record-5'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raid_name, "副本5");
    }
}
