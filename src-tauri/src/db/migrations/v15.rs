use rusqlite::{params, Connection};

/// V15 迁移：物品分类表 drop_items
///
/// 设计要点：
/// 1. 单表 drop_items 存储所有从 JX3Box 接口获取的物品信息（含分类字段）。
/// 2. item_name 为 UNIQUE 键，跨副本去重；item_type + item_id 联合索引用于缓存命中判断。
/// 3. 不再维护 drop_table_cache 副本标记表，扫描时按需调用 drop/v2 接口获取可能掉落，
///    与本地实际掉落取交集后，仅查询 drop_items 表中未命中的物品。
/// 4. 完整存储 item_merged 接口返回的所有 64 个字段（含 9 个 JSON 字段），便于后续扩展。
/// 5. 保留 item_cache 旧表迁移：若存在旧 item_cache 表，DROP 后由新表替代。
///    （item_cache 的数据由首次扫描后按需重建，不做数据迁移。）
/// 6. 同步移除 app_config 中的 item_cache_synced 标记（改为 drop_items 表自管状态）。
pub fn migrate(conn: &Connection) -> Result<(), String> {
    log::info!("========== V15 迁移开始 ==========");

    // 1. 若存在旧 item_cache 表，删除（数据由新流程按需重建）
    let has_old_item_cache: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='item_cache'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if has_old_item_cache {
        log::info!("[V15] 检测到旧 item_cache 表，执行删除");
        conn.execute("DROP TABLE IF EXISTS item_cache", [])
            .map_err(|e| e.to_string())?;
        conn.execute("DROP INDEX IF EXISTS idx_item_cache_category", [])
            .map_err(|e| e.to_string())?;
    }

    // 2. 若存在旧 drop_table_cache 表，删除（新设计不再使用副本标记表）
    let has_old_drop_table_cache: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='drop_table_cache'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false);

    if has_old_drop_table_cache {
        log::info!("[V15] 检测到旧 drop_table_cache 表，执行删除");
        conn.execute("DROP TABLE IF EXISTS drop_table_cache", [])
            .map_err(|e| e.to_string())?;
    }

    // 3. 创建新的 drop_items 表
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS drop_items (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            -- drop/v2 接口字段
            map_id            INTEGER,
            item_type         INTEGER NOT NULL,
            item_id           INTEGER NOT NULL,
            item_ext_id       INTEGER,
            boss_name         TEXT,
            applicable_school_ids TEXT,
            -- item_merged 接口字段（简单类型）
            ui_id             INTEGER,
            source            TEXT,
            source_id         INTEGER,
            item_name         TEXT NOT NULL UNIQUE,
            description       TEXT,
            genre             INTEGER,
            sub_type          INTEGER,
            detail_type       INTEGER,
            price             INTEGER,
            level             INTEGER,
            bind_type         INTEGER,
            max_durability    INTEGER,
            abrade_rate       INTEGER,
            max_exist_time    INTEGER,
            max_exist_amount  INTEGER,
            can_trade         INTEGER,
            can_destroy       INTEGER,
            quality           INTEGER,
            skill_id          INTEGER,
            skill_level       INTEGER,
            belong_school     TEXT,
            magic_kind        TEXT,
            magic_type        TEXT,
            get_type          TEXT,
            icon_id           INTEGER,
            can_set_color     INTEGER,
            auc_genre         INTEGER,
            auc_sub_type      INTEGER,
            require_camp      INTEGER,
            max_strength_level INTEGER,
            can_apart         INTEGER,
            can_exterior      INTEGER,
            can_change_magic  INTEGER,
            can_shared        INTEGER,
            repair_price_rebate INTEGER,
            can_stack         INTEGER,
            can_consume       INTEGER,
            require_level     INTEGER,
            require_gender    INTEGER,
            require_homeland_level INTEGER,
            can_use_on_horse  INTEGER,
            can_use_in_fight  INTEGER,
            can_good_camp_use INTEGER,
            can_evil_camp_use INTEGER,
            can_neutral_camp_use INTEGER,
            type_label        TEXT,
            appearance        TEXT,
            cool_down         TEXT,
            is_quest          INTEGER,
            wu_cai_html       TEXT,
            is_equip          INTEGER,
            equip_usage       INTEGER,
            image_url         TEXT,
            id_key            INTEGER,
            -- item_merged 接口字段（复杂类型，JSON 存储）
            diamonds          TEXT,
            requires          TEXT,
            recommend         TEXT,
            recommend_xfs     TEXT,
            attribute_types   TEXT,
            set_info          TEXT,
            get_source        TEXT,
            attributes        TEXT,
            furniture_attributes TEXT,
            -- 管理字段
            category          TEXT NOT NULL DEFAULT 'unknown',
            class_source      TEXT NOT NULL DEFAULT 'api',
            created_at        TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_drop_items_category ON drop_items(category);
        CREATE INDEX IF NOT EXISTS idx_drop_items_item_type_id ON drop_items(item_type, item_id);
        "#,
    )
    .map_err(|e| e.to_string())?;

    // 4. 移除 app_config 中的 item_cache_synced 标记（若存在）
    let now = chrono::Local::now().to_rfc3339();
    conn.execute(
        "DELETE FROM app_config WHERE key = 'item_cache_synced'",
        [],
    )
    .ok();

    // 5. 写入迁移完成标记
    conn.execute(
        "INSERT OR REPLACE INTO app_config (key, value, updated_at) VALUES ('v15_drop_items_migrated', 'true', ?1)",
        params![now],
    )
    .map_err(|e| e.to_string())?;

    log::info!("========== V15 迁移完成 ==========");
    Ok(())
}
