use crate::db::migration::error_to_string;
use rusqlite::Connection;

/// 判断是否为门派名称（而不是心法名称）
fn is_force_name(name: &str) -> bool {
    matches!(
        name,
        "江湖"
            | "少林"
            | "万花"
            | "天策"
            | "纯阳"
            | "七秀"
            | "五毒"
            | "唐门"
            | "藏剑"
            | "丐帮"
            | "明教"
            | "苍云"
            | "长歌"
            | "霸刀"
            | "蓬莱"
            | "凌雪"
            | "衍天"
            | "药宗"
            | "刀宗"
            | "万灵"
            | "段氏"
            | "无相楼"
            | "通用"
    )
}

/// 将心法名称映射为对应的门派名称
fn get_force_name_by_kungfu(kungfu_name: &str) -> Option<&'static str> {
    match kungfu_name {
        // 少林
        "洗髓经" | "易筋经" => Some("少林"),
        // 万花
        "花间游" | "离经易道" => Some("万花"),
        // 天策
        "傲血战意" | "铁牢律" => Some("天策"),
        // 纯阳
        "紫霞功" | "太虚剑意" => Some("纯阳"),
        // 七秀
        "云裳心经" | "冰心诀" => Some("七秀"),
        // 五毒
        "毒经" | "补天诀" => Some("五毒"),
        // 唐门
        "惊羽诀" | "天罗诡道" => Some("唐门"),
        // 藏剑
        "问水诀" => Some("藏剑"),
        // 丐帮
        "笑尘诀" => Some("丐帮"),
        // 明教
        "焚影圣诀" | "明尊琉璃体" => Some("明教"),
        // 苍云
        "铁骨衣" | "分山劲" => Some("苍云"),
        // 长歌
        "莫问" | "相知" => Some("长歌"),
        // 霸刀
        "北傲诀" => Some("霸刀"),
        // 蓬莱
        "凌海诀" => Some("蓬莱"),
        // 凌雪
        "隐龙诀" => Some("凌雪"),
        // 衍天
        "太玄经" => Some("衍天"),
        // 药宗
        "灵素" | "无方" => Some("药宗"),
        // 刀宗
        "孤锋诀" => Some("刀宗"),
        // 万灵
        "山海心决" => Some("万灵"),
        // 段氏
        "周天功" => Some("段氏"),
        // 无相楼
        "幽罗引" => Some("无相楼"),
        _ => None,
    }
}

/// V8 迁移：
/// - 新增 martial（心法）字段
/// - 将旧 sect（心法名）映射为门派名写入 sect
/// - 将原 sect 值迁入 martial
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V8 迁移开始 ==========");
    log::info!("V8 迁移：补充角色心法字段（martial）");

    add_martial_column(conn)?;

    log::info!("========== V8 迁移完成 ==========");
    Ok(())
}

fn add_martial_column(conn: &Connection) -> Result<(), String> {
    log::info!("V8 迁移：开始检查 roles 表结构");

    let mut stmt = conn
        .prepare("PRAGMA table_info(roles)")
        .map_err(error_to_string)?;

    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    log::info!("V8 迁移：当前 roles 表的列: {:?}", columns);

    if columns.iter().any(|column| column == "martial") {
        log::info!("V8 迁移：martial 列已存在，跳过添加列");
        return Ok(());
    }

    log::info!("V8 迁移：添加 martial 列到 roles 表");
    conn.execute("ALTER TABLE roles ADD COLUMN martial TEXT", [])
        .map_err(error_to_string)?;
    log::info!("V8 迁移：martial 列添加成功");

    // 预先加载所有需要处理的角色数据
    log::info!("V8 迁移：开始查询需要迁移的角色数据");
    let roles: Vec<(String, String)> = conn
        .prepare("SELECT id, sect FROM roles WHERE sect IS NOT NULL AND sect != ''")
        .map_err(error_to_string)?
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let sect: String = row.get(1)?;
            Ok((id, sect))
        })
        .map_err(error_to_string)?
        .filter_map(|row| row.ok())
        .collect();

    log::info!("V8 迁移：找到 {} 条需要处理的角色记录", roles.len());

    if roles.is_empty() {
        log::info!("V8 迁移：无数据需要迁移");
        return Ok(());
    }

    // 在内存中分类：已知门派 / 已映射心法 / 未知值
    let mut to_migrate: Vec<(String, String, String)> = Vec::new();
    let mut to_unknown: Vec<(String, String)> = Vec::new();
    let mut skipped_count = 0;

    for (role_id, old_sect) in &roles {
        if is_force_name(old_sect) {
            skipped_count += 1;
        } else if let Some(force_name) = get_force_name_by_kungfu(old_sect) {
            to_migrate.push((role_id.clone(), force_name.to_string(), old_sect.clone()));
        } else {
            to_unknown.push((role_id.clone(), old_sect.clone()));
        }
    }

    log::info!(
        "V8 迁移：分类完成。需迁移 {} 条，已是门派名 {} 条，无法识别 {} 条",
        to_migrate.len(),
        skipped_count,
        to_unknown.len()
    );

    // 批量更新：在事务中执行所有 UPDATE
    conn.execute("BEGIN TRANSACTION", [])
        .map_err(error_to_string)?;

    let mut migrated_count = 0;
    let mut unknown_count = 0;

    // 批量迁移已映射的心法
    if !to_migrate.is_empty() {
        for (role_id, sect, martial) in &to_migrate {
            conn.execute(
                "UPDATE roles SET sect = ?, martial = ? WHERE id = ?",
                rusqlite::params![sect, martial, role_id],
            )
            .map_err(|e| {
                let _ = conn.execute("ROLLBACK", []);
                error_to_string(e)
            })?;
            migrated_count += 1;
        }
    }

    // 批量处理无法识别的值
    if !to_unknown.is_empty() {
        for (role_id, martial) in &to_unknown {
            conn.execute(
                "UPDATE roles SET sect = '通用', martial = ? WHERE id = ?",
                rusqlite::params![martial, role_id],
            )
            .map_err(|e| {
                let _ = conn.execute("ROLLBACK", []);
                error_to_string(e)
            })?;
            unknown_count += 1;
        }
    }

    conn.execute("COMMIT", []).map_err(error_to_string)?;

    log::info!(
        "V8 迁移：完成。已迁移 {} 条，无法识别 {} 条，martial 非空 {} 条",
        migrated_count,
        unknown_count,
        migrated_count + unknown_count
    );

    Ok(())
}
