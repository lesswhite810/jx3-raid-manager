use rusqlite::Connection;

/// V11 迁移：清理重复的旧格式装备数据
///
/// 对于同一名称的装备，如果同时存在旧格式（ID 包含下划线）和新格式（纯数字 ID），
/// 则删除旧格式装备，保留新格式装备。
/// 如果只有旧格式装备（没有对应的新格式），则保留旧格式装备以支持历史记录。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V11 迁移开始 ==========");
    log::info!("V11 迁移：清理重复的旧格式装备数据");

    let deleted_count: i64 = conn
        .query_row(
            r#"
            DELETE FROM equipments 
            WHERE id LIKE '%\_%' ESCAPE '\'
            AND name IN (
                SELECT DISTINCT e1.name 
                FROM equipments e1 
                INNER JOIN equipments e2 ON e1.name = e2.name 
                WHERE e1.id LIKE '%\_%' ESCAPE '\' 
                AND e2.id NOT LIKE '%\_%' ESCAPE '\'
            )
            "#,
            [],
            |row| row.get(0),
        )
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
