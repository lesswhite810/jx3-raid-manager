//! 试炼结算提示中的装备，与本地已同步的试炼装备目录进行保守匹配。
//!
//! 翻牌卡位只保存 JX3Box 装备 ID。目录同步时会剔除非试炼来源和不符合部位的
//! 条目，因此这里再次应用同一来源规则，且只返回唯一候选。识别不完整不能把
//! 同名、同品级但绑定类型不同的装备猜成某一件。

use rusqlite::Connection;
use serde::Deserialize;

const TRIAL_EQUIPMENT_SOURCES: &str = include_str!("../../../data/trialEquipmentSources.json");

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TooltipEvidence {
    /// 游戏物品标题，不含 Tooltip 其余说明。
    pub name: String,
    /// Tooltip 的“装备等级”。
    pub level: i64,
    /// Tooltip 是否明确显示可交易或不可交易。
    pub bind: TooltipBind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TooltipBind {
    Tradable,
    Bound,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchedTrialEquipment {
    /// 现有 trial_records.card_1..card_5 使用的 JX3Box `ID` 字符串。
    pub equipment_id: String,
    pub name: String,
    pub level: i64,
    pub bind_type: i64,
}

#[derive(Debug, Deserialize)]
struct TrialEquipmentSources {
    default: TrialEquipmentSource,
    #[serde(default)]
    seasons: std::collections::HashMap<String, TrialEquipmentSource>,
}

#[derive(Debug, Clone, Deserialize)]
struct TrialEquipmentSource {
    keyword: String,
    slots: Vec<TrialEquipmentSlot>,
}

#[derive(Debug, Clone, Deserialize)]
struct TrialEquipmentSlot {
    #[serde(rename = "type")]
    equip_type: String,
    #[serde(default, rename = "subType")]
    sub_type: Option<i64>,
    #[serde(default, rename = "detailType")]
    detail_type: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct EquipmentData {
    #[serde(rename = "SubType")]
    sub_type: Option<i64>,
    #[serde(rename = "DetailType")]
    detail_type: Option<i64>,
}

#[derive(Debug)]
struct Candidate {
    equipment_id: String,
    name: String,
    level: i64,
    bind_type: Option<i64>,
    data: String,
}

/// 根据结算发生时刻使用的赛季和本地缓存匹配 Tooltip。
///
/// `completed_at_ms` 是 JCL 中结算终局的毫秒时间戳。目录同步仅保留当前赛季
/// 条目，正常截图采集在这段结算窗口内运行。找不到当时赛季、缓存缺失或候选不唯一
/// 时均返回 `None`，由人工补录处理。
pub(crate) fn match_reward_tooltip(
    conn: &Connection,
    completed_at_ms: i64,
    evidence: &TooltipEvidence,
) -> Result<Option<MatchedTrialEquipment>, String> {
    if evidence.level <= 0 || matches!(evidence.bind, TooltipBind::Unknown) {
        return Ok(None);
    }
    let title = normalize_name(&evidence.name);
    if title.is_empty() {
        return Ok(None);
    }

    let season_name = season_name_at(conn, completed_at_ms)?;
    let Some(season_name) = season_name else {
        return Ok(None);
    };
    let source = source_for_season(&season_name)?;

    let mut statement = conn
        .prepare("SELECT id, name, level, bind_type, data FROM equipments WHERE level = ?1")
        .map_err(|error| error.to_string())?;
    let candidates = statement
        .query_map([evidence.level], |row| {
            Ok(Candidate {
                equipment_id: row.get(0)?,
                name: row.get(1)?,
                level: row.get(2)?,
                bind_type: row.get(3)?,
                data: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let mut matches = candidates
        .into_iter()
        .filter(|candidate| normalize_name(&candidate.name) == title)
        .filter(|candidate| candidate_is_from_source(candidate, &source))
        .filter(|candidate| bind_matches(candidate.bind_type, evidence.bind))
        .collect::<Vec<_>>();

    if matches.len() != 1 {
        return Ok(None);
    }
    let candidate = matches.pop().expect("length already checked");
    let bind_type = candidate.bind_type.expect("bind filter already checked");
    Ok(Some(MatchedTrialEquipment {
        equipment_id: candidate.equipment_id,
        name: candidate.name,
        level: candidate.level,
        bind_type,
    }))
}

fn season_name_at(conn: &Connection, completed_at_ms: i64) -> Result<Option<String>, String> {
    let timestamp = completed_at_ms.div_euclid(1_000);
    match conn.query_row(
        "SELECT name FROM seasons
         WHERE start_date <= ?1 AND (end_date IS NULL OR end_date = 0 OR end_date > ?1)
         ORDER BY sort_order DESC LIMIT 1",
        [timestamp],
        |row| row.get(0),
    ) {
        Ok(name) => Ok(Some(name)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn source_for_season(season_name: &str) -> Result<TrialEquipmentSource, String> {
    let sources: TrialEquipmentSources = serde_json::from_str(TRIAL_EQUIPMENT_SOURCES)
        .map_err(|error| format!("解析试炼装备来源失败: {error}"))?;
    Ok(sources
        .seasons
        .get(season_name)
        .cloned()
        .unwrap_or(sources.default))
}

fn candidate_is_from_source(candidate: &Candidate, source: &TrialEquipmentSource) -> bool {
    if !candidate.name.contains(&source.keyword) {
        return false;
    }
    let Ok(data) = serde_json::from_str::<EquipmentData>(&candidate.data) else {
        return false;
    };
    let Some(sub_type) = data.sub_type else {
        return false;
    };
    source.slots.iter().any(|slot| {
        equipment_type_matches(slot, sub_type, data.detail_type)
            && slot.sub_type.is_none_or(|expected| expected == sub_type)
            && slot
                .detail_type
                .is_none_or(|expected| data.detail_type == Some(expected))
    })
}

fn equipment_type_matches(
    slot: &TrialEquipmentSlot,
    sub_type: i64,
    detail_type: Option<i64>,
) -> bool {
    match slot.equip_type.as_str() {
        "weapon" => detail_type == Some(6) || sub_type == 1,
        "trinket" => matches!(sub_type, 4 | 5 | 7),
        "armor" => !(detail_type == Some(6) || sub_type == 1 || matches!(sub_type, 4 | 5 | 7)),
        _ => false,
    }
}

fn bind_matches(bind_type: Option<i64>, expected: TooltipBind) -> bool {
    let Some(bind_type) = bind_type else {
        return false;
    };
    match expected {
        TooltipBind::Tradable => matches!(bind_type, 1 | 2),
        // 本地 UI 与同步数据目前只把 3 定义为“拾取后绑定”。同步缺字段会
        // 落成 0，不能把这种未知值误作 Tooltip 中明确的“不可交易”。
        TooltipBind::Bound => bind_type == 3,
        TooltipBind::Unknown => false,
    }
}

/// Tooltip 标题里的星级是装饰，OCR 也可能在字符间插入空白。仅清除这两类
/// 展示差异，名称主体仍必须逐字相同。
fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '★' | '☆'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE seasons (name TEXT, start_date INTEGER, end_date INTEGER, sort_order INTEGER);
             CREATE TABLE equipments (id TEXT, name TEXT, level INTEGER, bind_type INTEGER, data TEXT);
             INSERT INTO seasons VALUES ('苍生铸世', 1, 0, 1);",
        )
        .unwrap();
        conn
    }

    fn add_equipment(
        conn: &Connection,
        id: &str,
        name: &str,
        level: i64,
        bind_type: i64,
        sub_type: i64,
        detail_type: Option<i64>,
    ) {
        let data = serde_json::json!({
            "SubType": sub_type,
            "DetailType": detail_type,
        })
        .to_string();
        conn.execute(
            "INSERT INTO equipments VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, name, level, bind_type, data],
        )
        .unwrap();
    }

    #[test]
    fn matches_unique_bound_current_season_item_and_returns_existing_id() {
        let conn = setup_database();
        add_equipment(&conn, "51426", "无观项链·内·荒★★★★★★", 1050, 3, 4, None);
        add_equipment(&conn, "51468", "无观项链·内·荒★★★★★★", 1050, 2, 4, None);

        let found = match_reward_tooltip(
            &conn,
            1_700_000_000_000,
            &TooltipEvidence {
                name: "无观项链 · 内 · 荒 ★★★★★★".to_string(),
                level: 1050,
                bind: TooltipBind::Bound,
            },
        )
        .unwrap();

        assert_eq!(
            found,
            Some(MatchedTrialEquipment {
                equipment_id: "51426".to_string(),
                name: "无观项链·内·荒★★★★★★".to_string(),
                level: 1050,
                bind_type: 3,
            })
        );
    }

    #[test]
    fn rejects_ambiguous_tradable_variants_and_unknown_bind_state() {
        let conn = setup_database();
        add_equipment(&conn, "1", "无观项链·内·荒", 1050, 1, 4, None);
        add_equipment(&conn, "2", "无观项链·内·荒", 1050, 2, 4, None);

        for bind in [TooltipBind::Tradable, TooltipBind::Unknown] {
            assert!(match_reward_tooltip(
                &conn,
                1_700_000_000_000,
                &TooltipEvidence {
                    name: "无观项链·内·荒".to_string(),
                    level: 1050,
                    bind,
                },
            )
            .unwrap()
            .is_none());
        }
    }

    #[test]
    fn rejects_wrong_trial_slot_or_another_season_name() {
        let conn = setup_database();
        add_equipment(&conn, "armor", "无观上衣·内·荒", 1050, 3, 2, None);
        add_equipment(&conn, "wrong", "非试炼项链·内·荒", 1050, 3, 4, None);

        assert!(match_reward_tooltip(
            &conn,
            1_700_000_000_000,
            &TooltipEvidence {
                name: "无观上衣·内·荒".to_string(),
                level: 1050,
                bind: TooltipBind::Bound,
            },
        )
        .unwrap()
        .is_none());
        assert!(match_reward_tooltip(
            &conn,
            1_700_000_000_000,
            &TooltipEvidence {
                name: "非试炼项链·内·荒".to_string(),
                level: 1050,
                bind: TooltipBind::Bound,
            },
        )
        .unwrap()
        .is_none());
    }
}
