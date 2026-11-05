//! 结合角色的秘境结算奖励与经核实的首领物品，补记本轮扫描的首领进度。
//! 专属物品取自地图掉落表，按全名排除同图其他首领共享的物品：
//! <https://node.jx3box.com/fb/drop/v2/835?client=std>（2026-11-02）
//! 和 <https://node.jx3box.com/fb/drop/v2/828?client=std>（2026-11-03）。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};

use super::{
    analyze_completion_candidate, is_exact_boss_kill_reward_msg, normalize_chatlog_user_global_id,
    CompletionProof, JclAnalysis, JclFileInfo, RaidEntry, ITEM_RE,
};

const RECEIPT_WINDOW_SECS: i64 = 30;
const PERSONAL_LOOT_WINDOW_SECS: i64 = 30;
const SETTLEMENT_WINDOW_SECS: i64 = 20 * 60;
const NEIGHBOR_LOOKBACK_MS: i64 = 2 * 60 * 60 * 1000;

struct SettlementIdentity {
    map_id: i64,
    raid: &'static str,
    jcl_display_name: &'static str,
    boss_id: &'static str,
    boss: &'static str,
    template: i64,
    receipt: &'static str,
    exclusive_loot: &'static str,
    accept_personal_loot: bool,
}

const VERIFIED_ENCOUNTERS: &[SettlementIdentity] = &[
    SettlementIdentity {
        map_id: 835,
        raid: "25人普通洛阳之战",
        jcl_display_name: "25人普通洛阳之战",
        boss_id: "luoyangzhizhan_5",
        boss: "史朝义",
        template: 139_312,
        receipt: "你获得：[秘境宝藏·普通洛阳之战]。",
        exclusive_loot: include_str!("luoyang_835_final_loot.txt"),
        accept_personal_loot: false,
    },
    SettlementIdentity {
        map_id: 828,
        raid: "10人普通洛阳之战",
        jcl_display_name: "洛阳之战",
        boss_id: "luoyangzhizhan_5",
        boss: "史朝义",
        template: 138_261,
        receipt: "你获得：[秘境宝藏·洛阳之战]。",
        exclusive_loot: include_str!("luoyang_828_final_loot.txt"),
        accept_personal_loot: true,
    },
];

#[derive(Debug)]
struct SettlementCandidate {
    file_name: String,
    end_sec: i64,
}

#[derive(Debug)]
struct ChatRow {
    msg_type: String,
    text: String,
    msg: String,
    time_sec: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettlementRejectReason {
    NoRoleChatlog,
    NoRewardChain,
    NoVerifiedLoot,
}

impl SettlementRejectReason {
    fn label(self) -> &'static str {
        match self {
            Self::NoRoleChatlog => "角色聊天库不匹配",
            Self::NoRewardChain => "缺少同场10金结算",
            Self::NoVerifiedLoot => "缺少专属掉落结算链",
        }
    }
}

/// 只为当前已核验配置选择 JCL 补强文件。
///
/// 输入中的所有 JCL（包括未配置副本）仅用于消除相邻尝试、切图和未闭合战斗的歧义；
/// 若不存在目标首领候选，本函数不会读取 chatlog 或重扫其它 JCL 内容。
pub(super) fn select_settlement_promotions(
    chatlog_files: &[PathBuf],
    expected_uid: &str,
    all_jcl_files: &[JclFileInfo],
    jcl_analyses: &std::collections::HashMap<String, JclAnalysis>,
    raids: &[RaidEntry],
    combat_logs_dir: &Path,
    scan_end_ms: i64,
    known_next_jcl_time: Option<i64>,
) -> HashSet<String> {
    if expected_uid.is_empty() {
        return HashSet::new();
    }

    let mut promotions = HashSet::new();
    for jcl in all_jcl_files {
        let Some((identity, raid)) = verified_encounter(jcl, raids) else {
            continue;
        };
        let Some(analysis) = jcl_analyses.get(&jcl.file_name) else {
            continue;
        };
        if analysis.is_kill
            || analysis.fight_start_ms <= 0
            || analysis.fight_end_ms < analysis.fight_start_ms
        {
            continue;
        }
        let candidate = SettlementCandidate {
            file_name: jcl.file_name.clone(),
            end_sec: analysis.fight_end_ms / 1000,
        };
        let Some(settlement_end_ms) = settlement_end_ms(
            candidate.end_sec,
            all_jcl_files,
            scan_end_ms,
            known_next_jcl_time,
        ) else {
            continue;
        };
        // 先通过角色绑定的窄时间账本确认结算链，再重读可能很大的 JCL。
        if let Err(reason) = settlement_check(
            identity,
            chatlog_files,
            expected_uid,
            candidate.end_sec,
            settlement_end_ms / 1000,
        ) {
            log::debug!(
                "[DropScanner] 结算补强拒绝: BOSS={} 原因={}",
                identity.boss,
                reason.label()
            );
            continue;
        }
        let Some(proof) = analyze_completion_candidate(
            &combat_logs_dir.join(&jcl.file_name),
            &jcl.boss_name,
            jcl.boss_id,
            &raid.bosses,
            expected_uid,
            CompletionProof::EncounterSettlement {
                map_id: jcl.raid_id,
            },
        ) else {
            log::debug!(
                "[DropScanner] 结算补强拒绝: BOSS={} 原因=战斗候选验证未通过",
                identity.boss
            );
            continue;
        };
        if proof.fight_end_ms != analysis.fight_end_ms {
            log::debug!(
                "[DropScanner] 结算补强拒绝: BOSS={} 原因=战斗结束时间不一致",
                identity.boss
            );
            continue;
        }
        if has_ambiguous_attempt_boundary(
            &candidate,
            all_jcl_files,
            jcl_analyses,
            combat_logs_dir,
            expected_uid,
            settlement_end_ms,
        ) {
            log::debug!(
                "[DropScanner] 结算补强拒绝: BOSS={} 原因=邻近战斗边界有歧义",
                identity.boss
            );
            continue;
        }
        promotions.insert(candidate.file_name);
    }
    promotions
}

fn verified_encounter<'a>(
    jcl: &JclFileInfo,
    raids: &'a [RaidEntry],
) -> Option<(&'static SettlementIdentity, &'a RaidEntry)> {
    let identity = VERIFIED_ENCOUNTERS.iter().find(|e| {
        e.map_id == jcl.raid_id
            && e.boss == jcl.boss_name
            && e.template == jcl.boss_id
            && (e.jcl_display_name == jcl.raid_display_name || e.raid == jcl.raid_display_name)
    })?;
    let raid = raids.iter().find(|raid| raid.raid_id == identity.raid)?;
    raid.bosses
        .iter()
        .any(|(id, name)| id == identity.boss_id && name == identity.boss)
        .then_some((identity, raid))
}

fn settlement_end_ms(
    end_sec: i64,
    all_jcl_files: &[JclFileInfo],
    scan_end_ms: i64,
    known_next_jcl_time: Option<i64>,
) -> Option<i64> {
    let after_end = end_sec.saturating_mul(1000);
    if (scan_end_ms != 0 && scan_end_ms <= after_end)
        || known_next_jcl_time.is_some_and(|time| time <= after_end)
    {
        return None;
    }
    let natural_limit = after_end.saturating_add(SETTLEMENT_WINDOW_SECS.saturating_mul(1000));
    let next_file = all_jcl_files
        .iter()
        .map(|file| file.timestamp)
        .filter(|&time| time > after_end)
        .min();
    [
        Some(natural_limit),
        (scan_end_ms > after_end).then_some(scan_end_ms),
        next_file,
        known_next_jcl_time.filter(|&time| time > after_end),
    ]
    .into_iter()
    .flatten()
    .min()
    .filter(|&time| time > after_end)
}

fn has_ambiguous_attempt_boundary(
    candidate: &SettlementCandidate,
    all_jcl_files: &[JclFileInfo],
    jcl_analyses: &std::collections::HashMap<String, JclAnalysis>,
    combat_logs_dir: &Path,
    expected_uid: &str,
    settlement_end_ms: i64,
) -> bool {
    let candidate_end_ms = candidate.end_sec.saturating_mul(1000);
    for jcl in all_jcl_files {
        if jcl.file_name == candidate.file_name {
            continue;
        }
        if jcl.timestamp < candidate_end_ms.saturating_sub(NEIGHBOR_LOOKBACK_MS)
            || jcl.timestamp >= settlement_end_ms
        {
            continue;
        }
        if let Some(analysis) = jcl_analyses.get(&jcl.file_name) {
            // 已配置 JCL 直接复用本轮分析出的战斗边界，避免重复读大文件。
            if analysis.fight_start_ms <= 0 || analysis.fight_end_ms < analysis.fight_start_ms {
                return true;
            }
            if (analysis.fight_end_ms - candidate_end_ms).abs() <= RECEIPT_WINDOW_SECS * 1000
                || (analysis.fight_start_ms < settlement_end_ms
                    && analysis.fight_end_ms
                        > candidate_end_ms.saturating_sub(RECEIPT_WINDOW_SECS * 1000))
            {
                return true;
            }
            continue;
        }
        // 未配置日志没有缓存边界；仅检查同会话附近文件，兼顾轮询成本与“较早开始、结算期仍在打”的歧义。
        let Some(ends) =
            super::collect_type1_end_anchors(&combat_logs_dir.join(&jcl.file_name), expected_uid)
        else {
            return true;
        };
        if ends.is_empty()
            || ends
                .into_iter()
                .any(|end_ms| end_ms >= candidate_end_ms.saturating_sub(RECEIPT_WINDOW_SECS * 1000))
        {
            return true;
        }
    }
    false
}

fn settlement_check(
    identity: &SettlementIdentity,
    chatlog_files: &[PathBuf],
    expected_uid: &str,
    end_sec: i64,
    settlement_end_sec: i64,
) -> Result<(), SettlementRejectReason> {
    let mut matched_uid = false;
    let mut matched_reward = false;
    for path in chatlog_files {
        let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            continue;
        };
        if chatlog_uid(&conn).as_deref() != Some(expected_uid) {
            continue;
        }
        matched_uid = true;
        let rows = read_settlement_rows(&conn, end_sec, settlement_end_sec);
        let has_reward = rows.iter().any(is_exact_ten_gold);
        if !has_reward {
            continue;
        }
        matched_reward = true;
        if rows.iter().any(|reward| {
            is_exact_ten_gold(reward)
                && receipt_and_final_loot_follow(
                    reward,
                    &rows,
                    end_sec,
                    settlement_end_sec,
                    identity,
                )
        }) {
            return Ok(());
        }
    }
    if !matched_uid {
        Err(SettlementRejectReason::NoRoleChatlog)
    } else if matched_uid {
        // A role-bound chatlog exists, but the exact loot chain did not match.
        // Keep the distinction between a missing 10-gold reward and a missing
        // receipt/loot chain for audit logs without exposing the UID or path.
        if matched_reward {
            Err(SettlementRejectReason::NoVerifiedLoot)
        } else {
            Err(SettlementRejectReason::NoRewardChain)
        }
    } else {
        Err(SettlementRejectReason::NoRoleChatlog)
    }
}

fn chatlog_uid(conn: &Connection) -> Option<String> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM ChatInfo WHERE key = 'user_global_id'",
            [],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    normalize_chatlog_user_global_id(value?.as_str())
}

fn read_settlement_rows(conn: &Connection, end_sec: i64, settlement_end_sec: i64) -> Vec<ChatRow> {
    let Ok(mut statement) = conn.prepare(
        "SELECT type, text, msg, time FROM ChatLog WHERE time > ?1 AND time < ?2 AND type IN ('MSG_MONEY', 'MSG_ITEM', 'MSG_ROOM', 'MSG_WHISPER') ORDER BY time ASC",
    ) else { return Vec::new(); };
    let Ok(rows) = statement.query_map(params![end_sec, settlement_end_sec], |row| {
        Ok(ChatRow {
            msg_type: row.get(0)?,
            text: row.get(1)?,
            msg: row.get(2)?,
            time_sec: row.get(3)?,
        })
    }) else {
        return Vec::new();
    };
    rows.flatten().collect()
}

fn receipt_and_final_loot_follow(
    reward: &ChatRow,
    rows: &[ChatRow],
    end_sec: i64,
    settlement_end_sec: i64,
    identity: &SettlementIdentity,
) -> bool {
    rows.iter()
        .filter(|receipt| {
            receipt.msg_type == "MSG_ITEM"
                && receipt.text.trim() == identity.receipt
                && receipt.time_sec == reward.time_sec
                && receipt.time_sec > end_sec
                && receipt.time_sec <= end_sec.saturating_add(RECEIPT_WINDOW_SECS)
        })
        .any(|receipt| {
            rows.iter().any(|distribution| {
                distribution.msg_type == "MSG_ITEM"
                    && distribution.time_sec >= receipt.time_sec
                    && distribution.time_sec < settlement_end_sec
                    && !rows.iter().any(|row| {
                        (row.msg_type == "MSG_ROOM" || row.msg_type == "MSG_WHISPER")
                            && row.time_sec <= distribution.time_sec
                            && row.text.contains("每人底薪")
                    })
                    && verified_loot_item(identity, receipt.time_sec, distribution)
            })
        })
}

fn verified_loot_item(identity: &SettlementIdentity, receipt_sec: i64, row: &ChatRow) -> bool {
    let text = row.text.trim_end();
    let item = if text.starts_with("分配者将：[") {
        ITEM_RE
            .captures(text)
            .and_then(|caps| caps.get(1).map(|item| item.as_str()))
    } else if identity.accept_personal_loot
        && row.time_sec <= receipt_sec.saturating_add(PERSONAL_LOOT_WINDOW_SECS)
    {
        // 十人本可直接给本人发物品。完整匹配个人入账，排除队友获得、拍卖和放弃拾取。
        text.strip_prefix("你获得：[")
            .and_then(|item| item.strip_suffix("]。"))
    } else {
        None
    };
    item.is_some_and(|item| identity.exclusive_loot.lines().any(|name| name == item))
}

fn is_exact_ten_gold(row: &ChatRow) -> bool {
    row.msg_type == "MSG_MONEY"
        && row.text.starts_with("你获得：")
        && is_exact_boss_kill_reward_msg(&row.msg)
}

#[cfg(test)]
mod tests {
    const MAP_ID: i64 = VERIFIED_ENCOUNTERS[0].map_id;
    const RAID_DISPLAY_NAME: &str = VERIFIED_ENCOUNTERS[0].raid;
    const BOSS_NAME: &str = VERIFIED_ENCOUNTERS[0].boss;
    const BOSS_TEMPLATE_ID: i64 = VERIFIED_ENCOUNTERS[0].template;
    const TREASURE_RECEIPT: &str = VERIFIED_ENCOUNTERS[0].receipt;
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn ten_person_log_resolves_the_exact_raid_and_requires_its_own_settlement() {
        let mut file = target_jcl("洛阳之战(828)-史朝义(138261).jcl");
        file.raid_id = 828;
        file.raid_display_name = "洛阳之战".into();
        file.boss_id = 138_261;
        let mut raids = target_raids();
        assert!(verified_encounter(&file, &raids).is_none());
        let mut ten = raids[0].clone();
        ten.raid_id = "10人普通洛阳之战".into();
        raids.push(ten);
        let (identity, raid) = verified_encounter(&file, &raids).unwrap();
        assert_eq!(raid.raid_id, "10人普通洛阳之战");
        file.boss_id = BOSS_TEMPLATE_ID;
        assert!(verified_encounter(&file, &raids).is_none());

        let make_row = |kind: &str, text: &str, time_sec| ChatRow {
            msg_type: kind.to_owned(),
            text: text.to_owned(),
            msg: String::new(),
            time_sec,
        };
        let reward = make_row("MSG_MONEY", "你获得：1000。", 119);
        let receipt = make_row("MSG_ITEM", "你获得：[秘境宝藏·洛阳之战]。\n", 119);
        for (text, time, expected) in [
            ("你获得：[定尘血石·御·衣]。\n", 143, true),
            ("你获得：[定尘血石·御·衣]。", 149, true),
            ("你获得：[定尘血石·御·衣]。", 150, false),
            ("队友获得：[定尘血石·御·衣]。", 143, false),
            ("你获得：[定尘血石·疗·帽]。", 143, false),
            ("你获得：[探幽宝藏·防具·洛阳]。", 143, false),
        ] {
            let rows = [
                make_row("MSG_ITEM", &receipt.text, receipt.time_sec),
                make_row("MSG_ITEM", &receipt.text, receipt.time_sec),
                make_row("MSG_ITEM", text, time),
            ];
            assert_eq!(
                receipt_and_final_loot_follow(&reward, &rows, 100, 200, identity),
                expected,
                "{text} @ {time}"
            );
        }
        let wrong_raid = [
            make_row("MSG_ITEM", "你获得：[秘境宝藏·普通洛阳之战]。", 119),
            make_row("MSG_ITEM", "你获得：[定尘血石·御·衣]。", 143),
        ];
        assert!(!receipt_and_final_loot_follow(
            &reward,
            &wrong_raid,
            100,
            200,
            identity
        ));
        assert!(!verified_loot_item(
            &VERIFIED_ENCOUNTERS[0],
            119,
            &make_row("MSG_ITEM", "你获得：[苍黎衣·丐帮]。", 143),
        ));
    }

    #[test]
    fn select_settlement_promotions_requires_verified_role_bound_chain() {
        let path = temp_db_path("settlement-positive");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(type TEXT, text TEXT, msg TEXT, time INTEGER);") .unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('user_global_id', '123456')",
            [],
        )
        .unwrap();
        let money = "<text text=\"10\" name=\"Text_Gold\"/><text text=\"0\" name=\"Text_Silver\"/><text text=\"0\" name=\"Text_Copper\"/>";
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_MONEY', '你获得：10金。', ?1, 110)",
            [money],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_ITEM', ?1, '', 110)",
            [format!("{TREASURE_RECEIPT}\n")],
        )
        .unwrap();
        conn.execute("INSERT INTO ChatLog VALUES ('MSG_ITEM', '分配者将：[苍黎衣·丐帮]分配给队友。', '', 120)", []).unwrap();
        drop(conn);
        let (dir, jcl_path) = write_target_jcl("promotion");
        let file_name = jcl_path.file_name().unwrap().to_string_lossy().into_owned();
        let promotions = select_settlement_promotions(
            &[path.clone()],
            "123456",
            &[target_jcl(&file_name)],
            &target_analyses(&file_name),
            &target_raids(),
            &dir,
            130_000,
            None,
        );
        assert_eq!(promotions, HashSet::from([file_name]));
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settlement_rejects_non_native_loot_message_even_with_verified_identity() {
        let path = temp_db_path("settlement-negative");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(type TEXT, text TEXT, msg TEXT, time INTEGER);") .unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('user_global_id', '123456')",
            [],
        )
        .unwrap();
        let money = "<text text=\"10\" name=\"Text_Gold\"/><text text=\"0\" name=\"Text_Silver\"/><text text=\"0\" name=\"Text_Copper\"/>";
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_MONEY', '你获得：10金。', ?1, 110)",
            [money],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_ITEM', ?1, '', 110)",
            [TREASURE_RECEIPT],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_ITEM', '队友获得：[苍黎衣·丐帮]。', '', 120)",
            [],
        )
        .unwrap();
        drop(conn);
        let (dir, jcl_path) = write_target_jcl("allocation-negative");
        let file_name = jcl_path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            select_settlement_promotions(
                &[path.clone()],
                "123456",
                &[target_jcl(&file_name)],
                &target_analyses(&file_name),
                &target_raids(),
                &dir,
                130_000,
                None,
            )
            .is_empty(),
            "本地获得文本不是已验证的系统分配账本"
        );
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settlement_end_rejects_truncated_scan_boundary() {
        assert_eq!(settlement_end_ms(100, &[], 100_000, None), None);
        assert_eq!(settlement_end_ms(100, &[], 120_000, Some(100_000)), None);
    }

    #[test]
    fn receipt_chain_table_rejects_wrong_source_salary_and_right_edge() {
        struct Case {
            name: &'static str,
            receipt: &'static str,
            receipt_time: i64,
            allocation: &'static str,
            allocation_time: i64,
            salary_time: Option<i64>,
            settlement_end: i64,
            expected: bool,
        }
        let cases = [
            Case {
                name: "+30 秒边界",
                receipt: TREASURE_RECEIPT,
                receipt_time: 130,
                allocation: "分配者将：[苍黎衣·丐帮]分配给队友。",
                allocation_time: 150,
                salary_time: None,
                settlement_end: 200,
                expected: true,
            },
            Case {
                name: "+31 秒拒绝",
                receipt: TREASURE_RECEIPT,
                receipt_time: 131,
                allocation: "分配者将：[苍黎衣·丐帮]分配给队友。",
                allocation_time: 150,
                salary_time: None,
                settlement_end: 200,
                expected: false,
            },
            Case {
                name: "其他宝箱拒绝",
                receipt: "你获得：[秘境宝藏·普通其他副本]。",
                receipt_time: 110,
                allocation: "分配者将：[苍黎衣·丐帮]分配给队友。",
                allocation_time: 150,
                salary_time: None,
                settlement_end: 200,
                expected: false,
            },
            Case {
                name: "非系统分配拒绝",
                receipt: TREASURE_RECEIPT,
                receipt_time: 110,
                allocation: "队友获得：[苍黎衣·丐帮]。",
                allocation_time: 150,
                salary_time: None,
                settlement_end: 200,
                expected: false,
            },
            Case {
                name: "工资截断拒绝",
                receipt: TREASURE_RECEIPT,
                receipt_time: 110,
                allocation: "分配者将：[苍黎衣·丐帮]分配给队友。",
                allocation_time: 150,
                salary_time: Some(140),
                settlement_end: 200,
                expected: false,
            },
            Case {
                name: "右边界分配拒绝",
                receipt: TREASURE_RECEIPT,
                receipt_time: 110,
                allocation: "分配者将：[苍黎衣·丐帮]分配给队友。",
                allocation_time: 150,
                salary_time: None,
                settlement_end: 150,
                expected: false,
            },
        ];
        for case in cases {
            let reward = ChatRow {
                msg_type: "MSG_MONEY".to_owned(),
                text: "你获得：10金。".to_owned(),
                msg: String::new(),
                time_sec: case.receipt_time,
            };
            let mut rows = vec![
                ChatRow {
                    msg_type: "MSG_ITEM".to_owned(),
                    text: case.receipt.to_owned(),
                    msg: String::new(),
                    time_sec: case.receipt_time,
                },
                ChatRow {
                    msg_type: "MSG_ITEM".to_owned(),
                    text: case.allocation.to_owned(),
                    msg: String::new(),
                    time_sec: case.allocation_time,
                },
            ];
            if let Some(time_sec) = case.salary_time {
                rows.push(ChatRow {
                    msg_type: "MSG_ROOM".to_owned(),
                    text: "每人底薪：100金".to_owned(),
                    msg: String::new(),
                    time_sec,
                });
            }
            assert_eq!(
                receipt_and_final_loot_follow(
                    &reward,
                    &rows,
                    100,
                    case.settlement_end,
                    &VERIFIED_ENCOUNTERS[0]
                ),
                case.expected,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn verified_chatlog_uid_must_match_expected_role() {
        let path = temp_db_path("settlement-uid-mismatch");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(type TEXT, text TEXT, msg TEXT, time INTEGER);").unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('user_global_id', '654321')",
            [],
        )
        .unwrap();
        drop(conn);
        assert_eq!(
            settlement_check(&VERIFIED_ENCOUNTERS[0], &[path.clone()], "123456", 100, 200),
            Err(SettlementRejectReason::NoRoleChatlog)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cached_neighbor_overlapping_settlement_window_is_ambiguous() {
        let candidate = SettlementCandidate {
            file_name: "target.jcl".to_owned(),
            end_sec: 100,
        };
        let neighbor = JclFileInfo {
            file_name: "neighbor.jcl".to_owned(),
            timestamp: 99_000,
            raid_display_name: "其他副本".to_owned(),
            raid_id: 1,
            boss_name: "其他首领".to_owned(),
            boss_id: 1,
        };
        let analyses = HashMap::from([(
            neighbor.file_name.clone(),
            JclAnalysis {
                boss_name: None,
                fight_start_ms: 99_000,
                fight_end_ms: 130_000,
                is_kill: false,
            },
        )]);
        assert!(has_ambiguous_attempt_boundary(
            &candidate,
            &[neighbor],
            &analyses,
            Path::new("."),
            "123456",
            200_000,
        ));
    }

    fn temp_db_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "jx3-{label}-{}-{}.db",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn write_target_jcl(label: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "jx3-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("target.jcl");
        let content = "crc\t1\t100\t0\t1\t{true,\"realm::U123456/1\",0,835}\ncrc\t2\t101\t0\t8\t{9,\"史朝义\",139312}\ncrc\t3\t102\t0\t9\t{9,true,100,100}\ncrc\t4\t103\t0\t21\t{1,9,0,0,0,0,0,0,{[13]=1,[14]=0}}\ncrc\t5\t104\t0\t9\t{9,false,100,100}\ncrc\t6\t105\t0\t1\t{false,\"realm::U123456/1\",0,835}\n";
        let (encoded, _, _) = encoding_rs::GBK.encode(content);
        fs::write(&path, encoded.as_ref()).unwrap();
        (dir, path)
    }

    fn target_jcl(file_name: &str) -> JclFileInfo {
        JclFileInfo {
            file_name: file_name.to_owned(),
            timestamp: 105_000,
            raid_display_name: RAID_DISPLAY_NAME.to_owned(),
            raid_id: MAP_ID,
            boss_name: BOSS_NAME.to_owned(),
            boss_id: BOSS_TEMPLATE_ID,
        }
    }
    fn target_analyses(file_name: &str) -> HashMap<String, JclAnalysis> {
        HashMap::from([(
            file_name.to_owned(),
            JclAnalysis {
                boss_name: Some(BOSS_NAME.to_owned()),
                fight_start_ms: 100_000,
                fight_end_ms: 105_000,
                is_kill: false,
            },
        )])
    }
    fn target_raids() -> Vec<RaidEntry> {
        vec![RaidEntry {
            name: "洛阳之战".to_owned(),
            raid_id: RAID_DISPLAY_NAME.to_owned(),
            bosses: vec![("luoyangzhizhan_5".to_owned(), BOSS_NAME.to_owned())],
        }]
    }
}
