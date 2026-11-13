use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// V24 迁移：
/// - game_versions 新增 level_cap 字段（版本等级上限，NULL = 未配置）
/// - roles 新增 level 字段（角色等级，NULL = 未扫描）
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V24 迁移开始 ==========");
    log::info!("V24 迁移：新增版本等级上限（level_cap）与角色等级（level）字段");

    add_level_cap_column(conn)?;
    add_level_column(conn)?;

    log::info!("========== V24 迁移完成 ==========");
    Ok(())
}

fn add_level_cap_column(conn: &Connection) -> Result<(), String> {
    log::info!("V24 迁移：开始检查 game_versions 表结构");

    let mut stmt = conn
        .prepare("PRAGMA table_info(game_versions)")
        .map_err(error_to_string)?;

    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    log::info!("V24 迁移：当前 game_versions 表的列: {:?}", columns);

    if columns.iter().any(|column| column == "level_cap") {
        log::info!("V24 迁移：level_cap 列已存在，跳过添加列");
        return Ok(());
    }

    log::info!("V24 迁移：添加 level_cap 列到 game_versions 表");
    conn.execute("ALTER TABLE game_versions ADD COLUMN level_cap INTEGER", [])
        .map_err(error_to_string)?;
    log::info!("V24 迁移：level_cap 列添加成功");

    Ok(())
}

fn add_level_column(conn: &Connection) -> Result<(), String> {
    log::info!("V24 迁移：开始检查 roles 表结构");

    let mut stmt = conn
        .prepare("PRAGMA table_info(roles)")
        .map_err(error_to_string)?;

    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    log::info!("V24 迁移：当前 roles 表的列: {:?}", columns);

    if columns.iter().any(|column| column == "level") {
        log::info!("V24 迁移：level 列已存在，跳过添加列");
        return Ok(());
    }

    log::info!("V24 迁移：添加 level 列到 roles 表");
    conn.execute("ALTER TABLE roles ADD COLUMN level INTEGER", [])
        .map_err(error_to_string)?;
    log::info!("V24 迁移：level 列添加成功");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_v24_migrate_adds_both_columns_idempotently() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE roles (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                account_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                server TEXT NOT NULL,
                region TEXT NOT NULL,
                sect TEXT,
                martial TEXT,
                equipment_score INTEGER,
                disabled INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE game_versions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();

        migrate(&conn).expect("V24 迁移应成功");

        let level_cap_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('game_versions') WHERE name = 'level_cap'",
                [],
                |row: &rusqlite::Row| row.get(0),
            )
            .unwrap();
        assert_eq!(level_cap_exists, 1, "game_versions 应有 level_cap 列");

        let level_exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('roles') WHERE name = 'level'",
                [],
                |row: &rusqlite::Row| row.get(0),
            )
            .unwrap();
        assert_eq!(level_exists, 1, "roles 应有 level 列");

        // 幂等：重复执行不应报错
        migrate(&conn).expect("V24 迁移应幂等");
    }
}
