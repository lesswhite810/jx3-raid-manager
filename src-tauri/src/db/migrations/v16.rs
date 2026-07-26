use rusqlite::Connection;

/// V16 迁移：records 表索引优化
///
/// 设计要点：
/// 1. 新增表达式索引 `idx_records_data_raid_name`：加速 `json_extract(data, '$.raidName')` 查询。
///    该查询出现在 `upsert_raid_drop_record` 的 CD 窗口去重检查（drop_scanner.rs L2440），
///    是扫描 hot path，原全表扫描，现可走索引。
/// 2. 新增复合索引 `idx_records_cd_lookup`：覆盖 CD 窗口去重检查的 6 列过滤条件
///    `(account_id, raid_name, source, record_date)`，配合表达式索引可将全表扫描降为索引范围查找。
/// 3. 新增复合索引 `idx_records_pending`：覆盖 `status IN ('pending','scanning') ORDER BY record_date DESC`
///    查询，同时满足过滤和排序，避免额外排序步骤。
/// 4. 新增复合索引 `idx_records_manual_cd`：覆盖手工记录查询
///    `account_id + status + record_date` 组合。
/// 5. SQLite 3.9+ 支持表达式索引，rusqlite 0.31 bundled 版本符合要求。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V16 迁移开始 ==========");

    conn.execute_batch(
        r#"
        -- 表达式索引：加速 json_extract(data, '$.raidName') 查询
        -- 用于 upsert_raid_drop_record 的 CD 窗口去重检查（hot path）
        CREATE INDEX IF NOT EXISTS idx_records_data_raid_name
        ON records(json_extract(data, '$.raidName'));

        -- 复合索引：覆盖 CD 窗口去重检查的过滤条件
        -- 查询模式：account_id + raid_name + source + record_date(范围)
        CREATE INDEX IF NOT EXISTS idx_records_cd_lookup
        ON records(account_id, raid_name, source, record_date);

        -- 复合索引：覆盖 pending/scanning 查询 + 排序
        -- 查询模式：status IN (...) ORDER BY record_date DESC
        CREATE INDEX IF NOT EXISTS idx_records_pending
        ON records(status, record_date DESC);

        -- 复合索引：覆盖手工记录查询
        -- 查询模式：account_id + status + record_date(范围)
        CREATE INDEX IF NOT EXISTS idx_records_manual_cd
        ON records(account_id, status, record_date);
        "#,
    )
    .map_err(|e| format!("V16 创建索引失败: {}", e))?;

    // 写入迁移完成标记
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO app_config (key, value, updated_at) VALUES ('v16_records_indexes_migrated', 'true', ?1)",
        rusqlite::params![now],
    )
    .map_err(|e| format!("V16 写入迁移标记失败: {}", e))?;

    log::info!("========== V16 迁移完成 ==========");
    Ok(())
}
