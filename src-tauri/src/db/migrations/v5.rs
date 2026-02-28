use crate::db::migration::error_to_string;
use rusqlite::{params, Connection};

/// V5 迁移：添加角色可见性配置功能
///
/// 变更内容：
/// - 创建 instance_types 表（副本类型表）
/// - 创建 role_instance_visibility 表（角色-副本可见性关系表）
/// - 初始化副本类型数据（团队副本、百战、试炼之地）
/// - 为现有角色生成默认可见性记录（全部可见）
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("V5 迁移：添加角色可见性配置功能...");

    // 1. 创建 instance_types 表
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS instance_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            type TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL
        );
        "#,
    )
    .map_err(error_to_string)?;

    log::info!("V5 迁移：instance_types 表创建完成");

    // 2. 创建 role_instance_visibility 表
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS role_instance_visibility (
            id TEXT PRIMARY KEY,
            role_id TEXT NOT NULL,
            instance_type_id INTEGER NOT NULL,
            visible INTEGER DEFAULT 1,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE,
            FOREIGN KEY (instance_type_id) REFERENCES instance_types(id) ON DELETE CASCADE,
            UNIQUE(role_id, instance_type_id)
        );

        CREATE INDEX IF NOT EXISTS idx_riv_role_id ON role_instance_visibility(role_id);
        CREATE INDEX IF NOT EXISTS idx_riv_instance_type_id ON role_instance_visibility(instance_type_id);
        "#,
    )
    .map_err(error_to_string)?;

    log::info!("V5 迁移：role_instance_visibility 表创建完成");

    // 3. 初始化副本类型数据（使用 INSERT OR IGNORE 保证幂等）
    let instance_types = [
        ("raid", "团队副本"),
        ("baizhan", "百战"),
        ("trial", "试炼之地"),
    ];

    for (type_code, name) in instance_types {
        conn.execute(
            "INSERT OR IGNORE INTO instance_types (type, name) VALUES (?1, ?2)",
            params![type_code, name],
        )
        .map_err(error_to_string)?;
    }

    log::info!("V5 迁移：副本类型数据初始化完成");

    // 4. 为现有角色生成默认可见性记录
    migrate_existing_roles_visibility(conn)?;

    log::info!("V5 迁移：角色可见性配置功能添加完成");
    Ok(())
}

/// 为现有角色生成默认可见性记录
fn migrate_existing_roles_visibility(conn: &Connection) -> Result<(), String> {
    // 检查是否已有可见性记录（幂等检查）
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM role_instance_visibility",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if count > 0 {
        log::info!("V5 迁移：已有 {} 条可见性记录，跳过迁移", count);
        return Ok(());
    }

    // 获取所有副本类型 ID
    let type_ids: Vec<i32> = conn
        .prepare("SELECT id FROM instance_types")
        .map_err(error_to_string)?
        .query_map([], |row| row.get(0))
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    if type_ids.is_empty() {
        log::info!("V5 迁移：无副本类型数据，跳过角色可见性迁移");
        return Ok(());
    }

    // 获取所有角色 ID
    let role_ids: Vec<String> = conn
        .prepare("SELECT id FROM roles")
        .map_err(error_to_string)?
        .query_map([], |row| row.get(0))
        .map_err(error_to_string)?
        .filter_map(|r| r.ok())
        .collect();

    if role_ids.is_empty() {
        log::info!("V5 迁移：无角色数据，跳过可见性迁移");
        return Ok(());
    }

    // 为每个角色生成默认可见性记录（全部可见）
    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut inserted_count = 0;

    for role_id in &role_ids {
        for type_id in &type_ids {
            let id = generate_uuid();
            conn.execute(
                "INSERT INTO role_instance_visibility (id, role_id, instance_type_id, visible, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 1, ?4, ?4)",
                params![id, role_id, type_id, &timestamp],
            )
            .map_err(error_to_string)?;
            inserted_count += 1;
        }
    }

    log::info!(
        "V5 迁移：已为 {} 个角色生成 {} 条可见性记录",
        role_ids.len(),
        inserted_count
    );
    Ok(())
}

/// 生成 UUID
fn generate_uuid() -> String {
    uuid::Uuid::new_v4().to_string()
}
