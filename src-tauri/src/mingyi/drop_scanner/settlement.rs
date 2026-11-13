//! 结合角色的秘境结算奖励（10 金）或首领专属宝箱证据，补记本轮扫描的首领进度。
//!
//! 两条证据通道相互独立、任一成立即判击杀：
//! - 通道 A：角色 chatlog 在 JCL fight_end 前 15 秒到后 60 秒内出现精确 10 金
//!   的击杀结算奖励（boss 必定发放，但可能因剧情演出或落盘延迟迟到）；
//! - 通道 B：JCL 内出现了该首领的专属宝箱 NPC（茗伊可能漏写死亡事件，但
//!   击杀后宝箱仍会生成）。
//!
//! 不依赖副本/首领白名单：`match_raid_name` 命中的任何副本、且首领在副本
//! 配置中的 JCL 都按同一套规则补强。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags};

use super::{
    analyze_completion_candidate, is_exact_boss_kill_reward_msg, match_raid_name,
    normalize_chatlog_user_global_id, CompletionProof, JclAnalysis, JclFileInfo, RaidEntry,
};

/// 邻近尝试边界歧义判定的重叠容差（秒）。
const NEIGHBOR_BOUNDARY_TOLERANCE_SECS: i64 = 30;
/// 结算奖励窗口自然上限：fight_end 之后最多再等 20 分钟。
const SETTLEMENT_WINDOW_SECS: i64 = 20 * 60;
const NEIGHBOR_LOOKBACK_MS: i64 = 2 * 60 * 60 * 1000;
/// 通道 A 的奖励窗口：fight_end 前 15 秒到后 60 秒。
/// 样本：10 人本伊曼·寂夜的 10 金在 fight_end 前 5 秒；洛阳之战突利和顺的
/// 10 金在 fight_end 前 11~12 秒；25 人本史朝义的 10 金在 fight_end 后
/// 20 秒（剧情演出后发放）。
const SETTLEMENT_REWARD_BEFORE_SECS: i64 = 15;
const SETTLEMENT_REWARD_AFTER_SECS: i64 = 60;

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
    // 保留时间字段便于日志排查与后续扩展；当前判定只用类型与文本。
    #[allow(dead_code)]
    time_sec: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettlementRejectReason {
    NoRoleChatlog,
    NoRewardChain,
}

impl SettlementRejectReason {
    fn label(self) -> &'static str {
        match self {
            Self::NoRoleChatlog => "角色聊天库不匹配",
            Self::NoRewardChain => "缺少同场10金结算",
        }
    }
}

/// 为所有符合补强条件的 JCL 选择结算补强文件。
///
/// 输入中的所有 JCL（包括未配置副本）仅用于消除相邻尝试、切图和未闭合战斗
/// 的歧义；若不存在目标首领候选，本函数不会读取 chatlog 或重扫其它 JCL 内容。
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
        let Some(raid) = match_raid_name(&jcl.raid_display_name, raids) else {
            continue;
        };
        if !raid
            .bosses
            .iter()
            .any(|(_, name)| name == &jcl.boss_name)
        {
            continue;
        }
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
        // 先重读 JCL 校验战斗完整性，同时采集宝箱证据：打完有剧情演出的
        // BOSS（如史朝义）结算奖励最早出现在击杀瞬间附近，远早于 type=1 false。
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
                jcl.boss_name
            );
            continue;
        };
        if proof.fight_end_ms != analysis.fight_end_ms {
            log::debug!(
                "[DropScanner] 结算补强拒绝: BOSS={} 原因=战斗结束时间不一致",
                jcl.boss_name
            );
            continue;
        }
        // 通道 A：窗口内精确 10 金；通道 B：JCL 内本首领专属宝箱。
        // 两条证据独立兜底，任一成立即判击杀；都没有才记录拒绝原因。
        if !proof.chest_seen {
            if let Err(reason) = settlement_check(
                chatlog_files,
                expected_uid,
                candidate.end_sec,
                settlement_end_ms / 1000,
            ) {
                log::debug!(
                    "[DropScanner] 结算补强拒绝: BOSS={} 原因={}",
                    jcl.boss_name,
                    reason.label()
                );
                continue;
            }
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
                jcl.boss_name
            );
            continue;
        }
        promotions.insert(candidate.file_name);
    }
    promotions
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
            if analysis.fight_start_ms > 0
                && analysis.fight_end_ms >= analysis.fight_start_ms
            {
                let tolerance_ms = NEIGHBOR_BOUNDARY_TOLERANCE_SECS * 1000;
                if (analysis.fight_end_ms - candidate_end_ms).abs() <= tolerance_ms
                    || (analysis.fight_start_ms < settlement_end_ms
                        && analysis.fight_end_ms
                            > candidate_end_ms.saturating_sub(tolerance_ms))
                {
                    return true;
                }
                continue;
            }
            // 分析结果缺少结束边界（掉线、切场景或客户端异常退出）时不能直接判定
            // 歧义，改为读文件的结束锚：同类中断日志的末事件时间同样能定界。
        }
        // 未配置日志没有缓存边界；仅检查同会话附近文件，兼顾轮询成本与“较早开始、结算期仍在打”的歧义。
        let Some(ends) =
            super::collect_type1_end_anchors(&combat_logs_dir.join(&jcl.file_name), expected_uid)
        else {
            return true;
        };
        if ends.is_empty()
            || ends.into_iter().any(|end_ms| {
                end_ms >= candidate_end_ms.saturating_sub(NEIGHBOR_BOUNDARY_TOLERANCE_SECS * 1000)
            })
        {
            return true;
        }
    }
    false
}

/// 通道 A：检查角色 chatlog 是否在 [fight_end-15s, min(fight_end+60s, 结算上限)]
/// 内出现精确 10 金的击杀结算奖励。
fn settlement_check(
    chatlog_files: &[PathBuf],
    expected_uid: &str,
    end_sec: i64,
    settlement_end_sec: i64,
) -> Result<(), SettlementRejectReason> {
    let window_start = end_sec.saturating_sub(SETTLEMENT_REWARD_BEFORE_SECS);
    let window_end = end_sec
        .saturating_add(SETTLEMENT_REWARD_AFTER_SECS)
        .min(settlement_end_sec);
    let mut matched_uid = false;
    for path in chatlog_files {
        let Ok(conn) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            continue;
        };
        if chatlog_uid(&conn).as_deref() != Some(expected_uid) {
            continue;
        }
        matched_uid = true;
        let rows = read_settlement_rows(&conn, window_start, window_end);
        if rows.iter().any(is_exact_ten_gold) {
            return Ok(());
        }
    }
    if matched_uid {
        Err(SettlementRejectReason::NoRewardChain)
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

fn read_settlement_rows(
    conn: &Connection,
    window_start_sec: i64,
    window_end_sec: i64,
) -> Vec<ChatRow> {
    let Ok(mut statement) = conn.prepare(
        "SELECT type, text, msg, time FROM ChatLog WHERE time >= ?1 AND time <= ?2 AND type IN ('MSG_MONEY', 'MSG_ITEM', 'MSG_ROOM', 'MSG_WHISPER') ORDER BY time ASC",
    ) else { return Vec::new(); };
    let Ok(rows) = statement.query_map(params![window_start_sec, window_end_sec], |row| {
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

fn is_exact_ten_gold(row: &ChatRow) -> bool {
    row.msg_type == "MSG_MONEY"
        && row.text.starts_with("你获得：")
        && is_exact_boss_kill_reward_msg(&row.msg)
}

#[cfg(test)]
mod tests {
    const MAP_ID: i64 = 835;
    const RAID_DISPLAY_NAME: &str = "25人普通洛阳之战";
    const BOSS_NAME: &str = "史朝义";
    const BOSS_TEMPLATE_ID: i64 = 139_312;
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 目标 JCL 的 fight_end=105 秒；奖励窗口为 [90, min(165, 结算上限)]。
    #[test]
    fn settlement_reward_window_boundaries() {
        let money = "<text text=\"10\" name=\"Text_Gold\"/><text text=\"0\" name=\"Text_Silver\"/><text text=\"0\" name=\"Text_Copper\"/>";
        let cases: [(i64, bool); 5] = [
            (90, true),   // 前 15 秒边界，含
            (89, false),  // 前 16 秒，拒
            (105, true),  // fight_end 当秒，含
            (165, true),  // 后 60 秒边界，含
            (166, false), // 后 61 秒，拒
        ];
        for (reward_time, expected) in cases {
            let path = temp_db_path("settlement-window");
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(type TEXT, text TEXT, msg TEXT, time INTEGER);").unwrap();
            conn.execute(
                "INSERT INTO ChatInfo VALUES ('user_global_id', '123456')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO ChatLog VALUES ('MSG_MONEY', '你获得：10金。', ?1, ?2)",
                params![money, reward_time],
            )
            .unwrap();
            drop(conn);
            assert_eq!(
                settlement_check(&[path.clone()], "123456", 105, 200).is_ok(),
                expected,
                "奖励时间 {reward_time}"
            );
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn select_settlement_promotions_requires_verified_role_bound_ten_gold() {
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

    /// 通道 B：JCL 内出现本首领的专属宝箱 NPC 时，即使没有任何 chatlog
    /// 证据也应补记击杀（宝箱与 10 金是独立兜底）。
    #[test]
    fn settlement_promotes_on_chest_evidence_without_chatlog() {
        let (dir, jcl_path) = write_target_jcl_with_chest("promotion-chest");
        let file_name = jcl_path.file_name().unwrap().to_string_lossy().into_owned();
        let promotions = select_settlement_promotions(
            &[],
            "123456",
            &[target_jcl(&file_name)],
            &target_analyses(&file_name),
            &target_raids(),
            &dir,
            130_000,
            None,
        );
        assert_eq!(
            promotions,
            HashSet::from([file_name]),
            "宝箱证据应独立补记击杀"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    /// 两条通道都不成立时不得补记：无宝箱 JCL + chatlog 只有窗口外的 10 金。
    #[test]
    fn settlement_rejects_reward_outside_window() {
        let path = temp_db_path("settlement-outside-window");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(type TEXT, text TEXT, msg TEXT, time INTEGER);") .unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('user_global_id', '123456')",
            [],
        )
        .unwrap();
        let money = "<text text=\"10\" name=\"Text_Gold\"/><text text=\"0\" name=\"Text_Silver\"/><text text=\"0\" name=\"Text_Copper\"/>";
        conn.execute(
            "INSERT INTO ChatLog VALUES ('MSG_MONEY', '你获得：10金。', ?1, 500)",
            [money],
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
            "窗口外的 10 金不能补记"
        );
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cached_unclosed_neighbor_falls_back_to_file_anchors() {
        // 已配置但分析结果没有结束边界（田承嗣式中途重开）的邻居，必须改用文件的
        // 结束锚判定，而不能凭 fight_end=0 一刀切成歧义。
        let candidate = SettlementCandidate {
            file_name: "target.jcl".to_owned(),
            end_sec: 100,
        };
        let dir = temp_jcl_dir("settlement-cached-unclosed");
        fs::create_dir_all(&dir).unwrap();
        let stale = "unclosed-old.jcl";
        let stale_content =
            "crc\t1\t40\t0\t1\t{true,\"realm::U123456/1\",0,835}\ncrc\t2\t60\t0\t18\t{\"某条系统消息。\",\"MSG_SYS\"}\n";
        let (encoded, _, _) = encoding_rs::GBK.encode(stale_content);
        fs::write(dir.join(stale), encoded.as_ref()).unwrap();
        let neighbor = |name: &str| JclFileInfo {
            file_name: name.to_owned(),
            timestamp: 40_000,
            raid_display_name: "其他副本".to_owned(),
            raid_id: 1,
            boss_name: "其他首领".to_owned(),
            boss_id: 1,
        };
        let unclosed = HashMap::from([(
            stale.to_owned(),
            JclAnalysis {
                boss_name: Some("其他首领".to_owned()),
                fight_start_ms: 40_000,
                fight_end_ms: 0,
                is_kill: false,
            },
        )]);
        assert!(
            !has_ambiguous_attempt_boundary(
                &candidate,
                &[neighbor(stale)],
                &unclosed,
                &dir,
                "123456",
                200_000,
            ),
            "末事件远早于候选窗口时，未闭合邻居不得判歧义"
        );
        let live = "unclosed-live.jcl";
        let live_content =
            "crc\t1\t40\t0\t1\t{true,\"realm::U123456/1\",0,835}\ncrc\t2\t95\t0\t18\t{\"某条系统消息。\",\"MSG_SYS\"}\n";
        let (encoded, _, _) = encoding_rs::GBK.encode(live_content);
        fs::write(dir.join(live), encoded.as_ref()).unwrap();
        let live_analysis = HashMap::from([(
            live.to_owned(),
            JclAnalysis {
                boss_name: Some("其他首领".to_owned()),
                fight_start_ms: 40_000,
                fight_end_ms: 0,
                is_kill: false,
            },
        )]);
        assert!(
            has_ambiguous_attempt_boundary(
                &candidate,
                &[neighbor(live)],
                &live_analysis,
                &dir,
                "123456",
                200_000,
            ),
            "末事件落在候选结束前 30 秒内仍须判歧义"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn settlement_promotion_survives_interrupted_neighbor_without_end_row() {
        // 掉线/切场景中断的邻居 JCL 没有 type=1 false，但它的最后一条事件远早于
        // 候选结束点，不构成「结算窗口内仍在打」的歧义，不得毒化补强。
        let path = temp_db_path("settlement-interrupted-neighbor");
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
        drop(conn);
        let (dir, jcl_path) = write_target_jcl("promotion-interrupted-neighbor");
        let file_name = jcl_path.file_name().unwrap().to_string_lossy().into_owned();
        let write_neighbor = |name: &str, last_event: i64| {
            let content = format!(
                "crc\t1\t40\t0\t1\t{{true,\"realm::U123456/1\",0,835}}\ncrc\t2\t{}\t0\t18\t{{\"某条系统消息。\",\"MSG_SYS\"}}\n",
                last_event
            );
            let (encoded, _, _) = encoding_rs::GBK.encode(&content);
            fs::write(dir.join(name), encoded.as_ref()).unwrap();
            JclFileInfo {
                file_name: name.to_owned(),
                timestamp: 40_000,
                raid_display_name: "其他副本".to_owned(),
                raid_id: 1,
                boss_name: "其他首领".to_owned(),
                boss_id: 1,
            }
        };
        // 末事件 60 秒，远早于候选结束点（105-30=75 秒），不构成歧义。
        let stale_neighbor = write_neighbor("interrupted-old.jcl", 60);
        assert_eq!(
            select_settlement_promotions(
                &[path.clone()],
                "123456",
                &[target_jcl(&file_name), stale_neighbor],
                &target_analyses(&file_name),
                &target_raids(),
                &dir,
                130_000,
                None,
            ),
            HashSet::from([file_name.clone()]),
            "中断邻居的末事件早于候选窗口时不得阻断补强"
        );
        // 末事件 104 秒落在候选结束前的 30 秒内，仍必须判歧义。
        let live_neighbor = write_neighbor("interrupted-live.jcl", 104);
        assert!(
            select_settlement_promotions(
                &[path.clone()],
                "123456",
                &[target_jcl(&file_name), live_neighbor],
                &target_analyses(&file_name),
                &target_raids(),
                &dir,
                130_000,
                None,
            )
            .is_empty(),
            "末事件落在候选结束前 30 秒内仍须视为歧义"
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
            settlement_check(&[path.clone()], "123456", 105, 200),
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

    /// 本机真实验证：2026-11-09 风闪 25人普通洛阳之战，史朝义 JCL（20-51-00）
    /// 曾被旧白名单+三件套逻辑处理；双通道改版后 10 金窗口（fight_end-10s~+60s）
    /// 应仍能把它补为击杀。
    /// 仅在装有该角色日志的机器上运行：cargo test --release settlement -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_world_shichaoyi_settlement_promotion() {
        let logs_dir = std::path::Path::new(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564258532896@zhcn_hd\userdata\combat_logs",
        );
        let chatlogs_dir = std::path::Path::new(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data\432345564258532896@zhcn_hd\userdata\chat_log",
        );
        let uid = "432345564258532896";
        let (all_files, analyses) = real_world_nov09_inputs(logs_dir, uid);
        let chatlog_files: Vec<PathBuf> = std::fs::read_dir(chatlogs_dir)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.extension()
                    .is_some_and(|ext| ext == "db")
                    .then_some(path)
            })
            .collect();
        assert!(!chatlog_files.is_empty(), "no chatlog dbs found");
        let raids = vec![RaidEntry {
            name: "洛阳之战".to_owned(),
            raid_id: "25人普通洛阳之战".to_owned(),
            bosses: vec![("luoyangzhizhan_5".to_owned(), "史朝义".to_owned())],
        }];
        // scan_end 取 21:08:19（生产当时扫描时刻），known_next 无后续副本。
        let scan_end_ms = 1_794_230_900_000;
        let promotions = select_settlement_promotions(
            &chatlog_files,
            uid,
            &all_files,
            &analyses,
            &raids,
            logs_dir,
            scan_end_ms,
            None,
        );
        let target = "2026-11-09-20-51-00-25人普通洛阳之战(835)-史朝义(139312).jcl";
        assert!(
            promotions.contains(target),
            "史朝义 20-51-00 应被结算补强，实际 promotions={promotions:?}"
        );
    }

    /// 从真实 combat_logs 目录构造 11-09 的 JclFileInfo 与 JclAnalysis 输入。
    /// 文件名解析副本/首领信息；fight 边界来自真实分析（田承嗣 20-11-41 未闭合）。
    fn real_world_nov09_inputs(
        _logs_dir: &std::path::Path,
        uid: &str,
    ) -> (Vec<JclFileInfo>, HashMap<String, JclAnalysis>) {
        let files = [
            ("2026-11-09-20-03-25-25人普通洛阳之战(835)-突利和顺(139306).jcl", 1794225205000i64, 1794226201000, true),
            ("2026-11-09-20-11-41-25人普通洛阳之战(835)-田承嗣(139329).jcl", 1794225501000, 0, false),
            ("2026-11-09-20-15-00-25人普通洛阳之战(835)-田承嗣(139329).jcl", 1794226100000, 1794226850000, true),
            ("2026-11-09-20-21-20-25人普通洛阳之战(835)-拜火燎原团士兵(140666).jcl", 1794226880000, 1794226960000, false),
            ("2026-11-09-20-29-30-25人普通洛阳之战(835)-伊曼·寂夜(139319).jcl", 1794227370000, 1794227453000, true),
            ("2026-11-09-20-32-49-25人普通洛阳之战(835)-阿史那承庆(139323).jcl", 1794227569000, 1794228035000, true),
            ("2026-11-09-20-45-49-25人普通洛阳之战(835)-煞将(139308).jcl", 1794227949000, 1794228397000, false),
            ("2026-11-09-20-47-33-25人普通洛阳之战(835)-李复(139332).jcl", 1794228253000, 1794228608000, false),
            ("2026-11-09-20-51-00-25人普通洛阳之战(835)-史朝义(139312).jcl", 1794228660000, 1794229061000, false),
        ];
        let mut infos = Vec::new();
        let mut analyses = HashMap::new();
        for (name, start_ms, end_ms, is_kill) in files {
            // 文件名末段 (模板ID) 前的名字即首领名；25人普通洛阳之战(835) 即副本。
            let stem = name.trim_end_matches(".jcl");
            // 结构：<date-time>-<副本名>(<map_id>)-<首领名>(<模板ID>)
            let boss_open = stem.rfind('(').unwrap();
            let boss_id: i64 = stem[boss_open + 1..]
                .trim_end_matches(')')
                .parse()
                .unwrap();
            let before = &stem[..boss_open];
            // before 以 "-史朝义" 结尾；boss_name 是最后一个 '-' 之后、boss_open 之前的段
            let dash = before.rfind('-').unwrap();
            let boss_name = before[dash + 1..].to_owned();
            let raid_before = &before[..dash];
            let map_open = raid_before.rfind('(').unwrap();
            let map_id: i64 = raid_before[map_open + 1..]
                .trim_end_matches(')')
                .parse()
                .unwrap();
            assert_eq!(map_id, 835, "{name}");
            infos.push(JclFileInfo {
                file_name: name.to_owned(),
                timestamp: start_ms,
                raid_display_name: "25人普通洛阳之战".to_owned(),
                raid_id: map_id,
                boss_name: boss_name.clone(),
                boss_id,
            });
            analyses.insert(
                name.to_owned(),
                JclAnalysis {
                    boss_name: Some(boss_name),
                    fight_start_ms: start_ms,
                    fight_end_ms: end_ms,
                    is_kill,
                },
            );
        }
        let _ = uid;
        (infos, analyses)
    }

    /// 11-11 真实数据回归：10人伊曼·寂夜（JCL 无死亡证据、fight_end 前 5 秒有 10金）
    /// 与 25人史朝义（剧情演出、fight_end 后 20 秒有 10金）应被双通道补强；
    /// 拉脱的阿史那承庆前两次尝试窗口内无 10金，不应误补。
    /// 仅在装有该角色日志的机器上运行：cargo test settlement -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_world_nov11_luoyang_promotions() {
        // 2026-11-11 00:00:00 (+08:00) 的 Unix 秒（由伊曼 fight_start=1794401405
        // 对应 20:50:05 反推）。
        const DAY_START_SEC: i64 = 1_794_326_400;
        let hms = |h: i64, m: i64, s: i64| (DAY_START_SEC + h * 3600 + m * 60 + s) * 1000;
        let bosses: Vec<(String, String)> = [
            ("luoyangzhizhan_1", "突利和顺"),
            ("luoyangzhizhan_2", "田承嗣"),
            ("luoyangzhizhan_3", "伊曼·寂夜"),
            ("luoyangzhizhan_4", "阿史那承庆"),
            ("luoyangzhizhan_5", "史朝义"),
        ]
        .iter()
        .map(|(id, name)| ((*id).to_owned(), (*name).to_owned()))
        .collect();

        // （角色 uid，JCL 副本显示名，map_id，raids.id，应补强文件，不应补强文件）
        let cases: [(&str, &str, i64, &str, &str, &[&str]); 2] = [
            (
                "432345564243886337",
                "洛阳之战",
                828,
                "10人普通洛阳之战",
                "2026-11-11-20-50-05-洛阳之战(828)-伊曼·寂夜(138932).jcl",
                &[],
            ),
            (
                "432345564255147961",
                "25人普通洛阳之战",
                835,
                "25人普通洛阳之战",
                "2026-11-11-23-52-15-25人普通洛阳之战(835)-史朝义(139312).jcl",
                &[
                    "2026-11-11-23-19-21-25人普通洛阳之战(835)-阿史那承庆(139323).jcl",
                    "2026-11-11-23-28-41-25人普通洛阳之战(835)-阿史那承庆(139323).jcl",
                ],
            ),
        ];

        for (uid, display, map_id, raid_id, expected, rejected) in cases {
            let base = std::path::Path::new(
                r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data",
            )
            .join(format!("{uid}@zhcn_hd"))
            .join("userdata");
            let logs_dir = base.join("combat_logs");
            let chatlogs_dir = base.join("chat_log");

            // 从文件名解析 11-11 洛阳之战 JCL，并用真实 analyze_jcl 生成分析结果。
            let mut infos = Vec::new();
            let mut analyses: HashMap<String, JclAnalysis> = HashMap::new();
            for entry in std::fs::read_dir(&logs_dir).unwrap() {
                let path = entry.unwrap().path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.starts_with("2026-11-11-") || !name.contains("洛阳之战") {
                    continue;
                }
                let stem = name.trim_end_matches(".jcl");
                let boss_open = stem.rfind('(').unwrap();
                let boss_id: i64 = stem[boss_open + 1..]
                    .trim_end_matches(')')
                    .parse()
                    .unwrap();
                let before = &stem[..boss_open];
                let dash = before.rfind('-').unwrap();
                let boss_name = before[dash + 1..].to_owned();
                let raid_before = &before[..dash];
                let map_open = raid_before.rfind('(').unwrap();
                let file_map_id: i64 = raid_before[map_open + 1..]
                    .trim_end_matches(')')
                    .parse()
                    .unwrap();
                assert_eq!(file_map_id, map_id, "{name}");
                let mut display_name = {
                    // 文件名结构：2026-11-11-HH-MM-SS-<副本名>(<map_id>)-
                    // 副本名可能含 '-'（如"堇懒懒的碧蝶·乾坤一掷"在 boss 段），
                    // 按前 5 段日期时间截断：2026/11/11/HH/MM/SS。
                    let head = raid_before[..map_open].to_owned();
                    let idx = head
                        .match_indices('-')
                        .nth(5)
                        .map(|(i, _)| i)
                        .unwrap_or(head.len());
                    head[idx..].trim_start_matches('-').to_owned()
                };
                if display_name.ends_with('-') {
                    display_name.pop();
                }
                assert_eq!(display_name, display, "{name}");
                // 文件名时间："2026-11-11-HH-MM-SS-..."
                let time_parts: Vec<i64> = stem
                    .split('-')
                    .skip(3)
                    .take(3)
                    .map(|p| p.parse().unwrap())
                    .collect();
                let timestamp = hms(time_parts[0], time_parts[1], time_parts[2]);
                let analysis = crate::mingyi::drop_scanner::analyze_jcl(
                    &path,
                    &boss_name,
                    boss_id,
                    &bosses,
                )
                .unwrap_or_else(|| panic!("analyze_jcl failed: {name}"));
                infos.push(JclFileInfo {
                    file_name: name.to_owned(),
                    timestamp,
                    raid_display_name: display_name,
                    raid_id: map_id,
                    boss_name: boss_name.clone(),
                    boss_id,
                });
                analyses.insert(name.to_owned(), analysis);
            }
            assert!(
                infos.iter().any(|f| f.file_name == expected),
                "{map_id} 缺少目标 JCL"
            );
            assert!(!analyses[expected].is_kill, "{expected} 应为 is_kill=false");

            let chatlog_files: Vec<PathBuf> = std::fs::read_dir(&chatlogs_dir)
                .unwrap()
                .filter_map(|entry| {
                    let path = entry.unwrap().path();
                    path.extension()
                        .is_some_and(|ext| ext == "db")
                        .then_some(path)
                })
                .collect();
            assert!(!chatlog_files.is_empty(), "{uid} no chatlog dbs found");

            let raids = vec![RaidEntry {
                name: "洛阳之战".to_owned(),
                raid_id: raid_id.to_owned(),
                bosses: bosses.clone(),
            }];
            let promotions = select_settlement_promotions(
                &chatlog_files,
                uid,
                &infos,
                &analyses,
                &raids,
                &logs_dir,
                hms(23, 59, 59),
                None,
            );
            println!("map_id={map_id} promotions={promotions:?}");
            assert!(
                promotions.contains(expected),
                "{expected} 应被结算补强，实际 promotions={promotions:?}"
            );
            for name in rejected {
                assert!(
                    !promotions.contains(*name),
                    "拉脱尝试 {name} 不应被补强，实际 promotions={promotions:?}"
                );
            }
        }
    }

    /// 全历史审计：对本机所有角色目录的全部 JCL 按生产逻辑跑新双通道补强，
    /// 输出会被补强的文件清单（新旧逻辑差异审计）。只读，不修改任何数据。
    /// 仅在装有历史日志的机器上运行：cargo test real_world_full_history -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_world_full_history_audit() {
        use crate::mingyi::drop_scanner::analyze_jcl;
        let data_root = std::path::Path::new(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data",
        );
        // 各角色共用同一组洛阳之战副本配置（含 10/25 两种显示名）。
        let luoyang_bosses: Vec<(String, String)> = [
            ("luoyangzhizhan_1", "突利和顺"),
            ("luoyangzhizhan_2", "田承嗣"),
            ("luoyangzhizhan_3", "伊曼·寂夜"),
            ("luoyangzhizhan_4", "阿史那承庆"),
            ("luoyangzhizhan_5", "史朝义"),
        ]
        .iter()
        .map(|(id, name)| ((*id).to_owned(), (*name).to_owned()))
        .collect();
        let raids = vec![
            RaidEntry {
                name: "洛阳之战".to_owned(),
                raid_id: "10人普通洛阳之战".to_owned(),
                bosses: luoyang_bosses.clone(),
            },
            RaidEntry {
                name: "洛阳之战".to_owned(),
                raid_id: "25人普通洛阳之战".to_owned(),
                bosses: luoyang_bosses.clone(),
            },
        ];

        let mut promoted_total = 0usize;
        for entry in std::fs::read_dir(data_root).unwrap() {
            let dir = entry.unwrap().path();
            if !dir.is_dir() || !dir.to_string_lossy().contains("@zhcn_hd") {
                continue;
            }
            let logs_dir = dir.join("userdata").join("combat_logs");
            let chatlogs_dir = dir.join("userdata").join("chat_log");
            if !logs_dir.is_dir() {
                continue;
            }
            let uid = dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .trim_end_matches("@zhcn_hd")
                .split('#')
                .next()
                .unwrap()
                .to_owned();
            if uid.is_empty() {
                continue;
            }
            let chatlog_files: Vec<PathBuf> = std::fs::read_dir(&chatlogs_dir)
                .map(|read| {
                    read.filter_map(|e| {
                        let path = e.unwrap().path();
                        path.extension()
                            .is_some_and(|ext| ext == "db")
                            .then_some(path)
                    })
                    .collect()
                })
                .unwrap_or_default();

            // 枚举全部 JCL：按文件名解析副本/首领，再跑真实 analyze_jcl。
            let mut infos = Vec::new();
            let mut analyses: HashMap<String, JclAnalysis> = HashMap::new();
            for file in std::fs::read_dir(&logs_dir).unwrap() {
                let path = file.unwrap().path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.ends_with(".jcl") {
                    continue;
                }
                let stem = name.trim_end_matches(".jcl");
                // 结构：2026-11-11-HH-MM-SS-<副本名>(<map_id>)-<首领名>(<模板ID>)
                // boss_open=最后一个'('；map_open=其之前的一个'('。无首领段（小怪/异常文件）跳过。
                let Some(boss_open) = stem.rfind('(') else {
                    continue;
                };
                let Some(map_open) = stem[..boss_open].rfind('(') else {
                    continue;
                };
                let Ok(boss_id) = stem[boss_open + 1..].trim_end_matches(')').parse::<i64>()
                else {
                    continue;
                };
                let before_boss = &stem[..boss_open];
                let Some(dash) = before_boss.rfind('-') else {
                    continue;
                };
                let boss_name = before_boss[dash + 1..].to_owned();
                let raid_before = &before_boss[..dash];
                let Ok(map_id) = raid_before[map_open + 1..].trim_end_matches(')').parse::<i64>()
                else {
                    continue;
                };
                let head = raid_before[..map_open].to_owned();
                let display_name = match head.match_indices('-').nth(5) {
                    Some((i, _)) => head[i..].trim_start_matches('-').to_owned(),
                    None => continue,
                };
                if display_name.is_empty() {
                    continue;
                }
                let Some(analysis) = analyze_jcl(&path, &boss_name, boss_id, &raids[0].bosses)
                else {
                    continue;
                };
                infos.push(JclFileInfo {
                    file_name: name.to_owned(),
                    timestamp: analysis.fight_start_ms,
                    raid_display_name: display_name,
                    raid_id: map_id,
                    boss_name: boss_name.clone(),
                    boss_id,
                });
                analyses.insert(name.to_owned(), analysis);
            }
            if infos.is_empty() {
                continue;
            }
            println!(
                "uid={uid} jcl总数={} 进入审计={}",
                std::fs::read_dir(&logs_dir)
                    .map(|read| read.count())
                    .unwrap_or(0),
                infos.len()
            );
            // scan_end 用最后一个 JCL 的 fight_end + 1 小时，模拟"历史完整扫描"。
            let scan_end_ms = infos
                .iter()
                .map(|f| analyses[&f.file_name].fight_end_ms)
                .max()
                .unwrap_or(0)
                + 3_600_000;
            let promotions = select_settlement_promotions(
                &chatlog_files,
                &uid,
                &infos,
                &analyses,
                &raids,
                &logs_dir,
                scan_end_ms,
                None,
            );
            if promotions.is_empty() {
                continue;
            }
            println!("=== uid={uid} 补强 {} 个 ===", promotions.len());
            let mut sorted: Vec<_> = promotions.iter().collect();
            sorted.sort();
            for name in sorted {
                let analysis = &analyses[name];
                println!(
                    "  {}  start={} end={} is_kill=false(旧)",
                    name,
                    analysis.fight_start_ms / 1000,
                    analysis.fight_end_ms / 1000
                );
            }
            promoted_total += promotions.len();
        }
        println!("=== 全历史补强总数: {promoted_total} ===");
    }

    /// 洛阳之战完整性审计：对本机所有角色目录的全部洛阳 JCL 按生产聚类逻辑
    /// （cluster_raid_instances，含双通道补强）聚成副本实例，统计每个实例的
    /// 击杀 BOSS 数，检查是否全部 5/5。只读，不修改任何数据。
    /// 仅在装有历史日志的机器上运行：cargo test real_world_luoyang_completeness -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_world_luoyang_completeness_audit() {
        use crate::mingyi::drop_scanner::{analyze_jcl, cluster_raid_instances};
        let data_root = std::path::Path::new(
            r"E:\Game\SeasunGame\Game\JX3\bin\zhcn_hd\interface\my#data",
        );
        let luoyang_bosses: Vec<(String, String)> = [
            ("luoyangzhizhan_1", "突利和顺"),
            ("luoyangzhizhan_2", "田承嗣"),
            ("luoyangzhizhan_3", "伊曼·寂夜"),
            ("luoyangzhizhan_4", "阿史那承庆"),
            ("luoyangzhizhan_5", "史朝义"),
        ]
        .iter()
        .map(|(id, name)| ((*id).to_owned(), (*name).to_owned()))
        .collect();
        let expected_names: Vec<&str> = luoyang_bosses
            .iter()
            .map(|(_, n)| n.as_str())
            .collect();
        let raids = vec![
            RaidEntry {
                name: "洛阳之战".to_owned(),
                raid_id: "10人普通洛阳之战".to_owned(),
                bosses: luoyang_bosses.clone(),
            },
            RaidEntry {
                name: "洛阳之战".to_owned(),
                raid_id: "25人普通洛阳之战".to_owned(),
                bosses: luoyang_bosses.clone(),
            },
        ];

        let mut instance_total = 0usize;
        let mut incomplete = 0usize;
        for entry in std::fs::read_dir(data_root).unwrap() {
            let dir = entry.unwrap().path();
            if !dir.is_dir() || !dir.to_string_lossy().contains("@zhcn_hd") {
                continue;
            }
            let logs_dir = dir.join("userdata").join("combat_logs");
            let chatlogs_dir = dir.join("userdata").join("chat_log");
            if !logs_dir.is_dir() {
                continue;
            }
            let uid = dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .trim_end_matches("@zhcn_hd")
                .split('#')
                .next()
                .unwrap()
                .to_owned();
            if uid.is_empty() {
                continue;
            }
            let chatlog_files: Vec<PathBuf> = std::fs::read_dir(&chatlogs_dir)
                .map(|read| {
                    read.filter_map(|e| {
                        let path = e.unwrap().path();
                        path.extension()
                            .is_some_and(|ext| ext == "db")
                            .then_some(path)
                    })
                    .collect()
                })
                .unwrap_or_default();

            // 枚举洛阳 JCL（两种显示名都收），跑真实 analyze_jcl。
            let mut infos = Vec::new();
            let mut analyses: HashMap<String, JclAnalysis> = HashMap::new();
            for file in std::fs::read_dir(&logs_dir).unwrap() {
                let path = file.unwrap().path();
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if !name.ends_with(".jcl") {
                    continue;
                }
                let stem = name.trim_end_matches(".jcl");
                let Some(boss_open) = stem.rfind('(') else {
                    continue;
                };
                let Some(map_open) = stem[..boss_open].rfind('(') else {
                    continue;
                };
                let Ok(boss_id) = stem[boss_open + 1..].trim_end_matches(')').parse::<i64>()
                else {
                    continue;
                };
                let before_boss = &stem[..boss_open];
                let Some(dash) = before_boss.rfind('-') else {
                    continue;
                };
                let boss_name = before_boss[dash + 1..].to_owned();
                let raid_before = &before_boss[..dash];
                let Ok(map_id) = raid_before[map_open + 1..].trim_end_matches(')').parse::<i64>()
                else {
                    continue;
                };
                let head = raid_before[..map_open].to_owned();
                let display_name = match head.match_indices('-').nth(5) {
                    Some((i, _)) => head[i..].trim_start_matches('-').to_owned(),
                    None => continue,
                };
                if !display_name.ends_with("洛阳之战") || map_id != 828 && map_id != 835 {
                    continue;
                }
                let Some(analysis) = analyze_jcl(&path, &boss_name, boss_id, &luoyang_bosses)
                else {
                    continue;
                };
                infos.push(JclFileInfo {
                    file_name: name.to_owned(),
                    timestamp: analysis.fight_start_ms,
                    raid_display_name: display_name,
                    raid_id: map_id,
                    boss_name: boss_name.clone(),
                    boss_id,
                });
                analyses.insert(name.to_owned(), analysis);
            }
            if infos.is_empty() {
                continue;
            }

            // 先跑结算补强（与生产一致），再聚类。
            let scan_end_ms = infos
                .iter()
                .map(|f| analyses[&f.file_name].fight_end_ms)
                .max()
                .unwrap_or(0)
                + 3_600_000;
            let promotions = select_settlement_promotions(
                &chatlog_files,
                &uid,
                &infos,
                &analyses,
                &raids,
                &logs_dir,
                scan_end_ms,
                None,
            );
            for name in &promotions {
                if let Some(a) = analyses.get_mut(name) {
                    a.is_kill = true;
                }
            }

            // 生产聚类逻辑（preloaded_records 传空：审计不含工资分割，
            // 洛阳单场通常 1h 内完成，2h 阈值足以分割跨场）。
            let instances = cluster_raid_instances(&uid, infos, &analyses, &[]);
            println!(
                "=== uid={uid} 洛阳实例 {} 个（补强 {} 个JCL）===",
                instances.len(),
                promotions.len()
            );
            for inst in &instances {
                instance_total += 1;
                let kills = &inst.bosses_killed;
                let missing: Vec<&str> = expected_names
                    .iter()
                    .copied()
                    .filter(|n| !kills.iter().any(|k| k == *n))
                    .collect();
                let start = chrono_like_local(inst.start_time);
                println!(
                    "  {} {} start={start} 击杀 {}/5 {:?}",
                    inst.raid_display_name,
                    if missing.is_empty() { "✓" } else { "✗" },
                    kills.len(),
                    if missing.is_empty() {
                        "完整".to_owned()
                    } else {
                        format!("缺: {missing:?}")
                    }
                );
                if !missing.is_empty() {
                    incomplete += 1;
                }
            }
        }
        println!(
            "=== 洛阳实例总数 {instance_total}，非 5/5 的 {incomplete} 个 ==="
        );
    }

    /// 毫秒时间戳 → "yyyy-MM-dd HH:mm" 本地(+8)字符串，仅审计打印用。
    fn chrono_like_local(ms: i64) -> String {
        let secs = ms / 1000 + 8 * 3600;
        let days = secs / 86400;
        let rem = secs % 86400;
        // 1970-01-01 起的日期转 y-m-d（简化算法，审计够用）
        let mut y = 1970i64;
        let mut d = days;
        loop {
            let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
            let len = if leap { 366 } else { 365 };
            if d < len {
                break;
            }
            d -= len;
            y += 1;
        }
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let month_lens = [
            31,
            if leap { 29 } else { 28 },
            31,
            30,
            31,
            30,
            31,
            31,
            30,
            31,
            30,
            31,
        ];
        let mut m = 1usize;
        for len in month_lens {
            if d < len {
                break;
            }
            d -= len;
            m += 1;
        }
        format!(
            "{y}-{m:02}-{:02} {:02}:{:02}",
            d + 1,
            rem / 3600,
            (rem % 3600) / 60
        )
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

    fn temp_jcl_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "jx3-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    fn write_target_jcl(label: &str) -> (PathBuf, PathBuf) {
        let dir = temp_jcl_dir(label);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("target.jcl");
        let content = "crc\t1\t100\t0\t1\t{true,\"realm::U123456/1\",0,835}\ncrc\t2\t101\t0\t8\t{9,\"史朝义\",139312}\ncrc\t3\t102\t0\t9\t{9,true,100,100}\ncrc\t4\t103\t0\t21\t{1,9,0,0,0,0,0,0,{[13]=1,[14]=0}}\ncrc\t5\t104\t0\t9\t{9,false,100,100}\ncrc\t6\t105\t0\t1\t{false,\"realm::U123456/1\",0,835}\n";
        let (encoded, _, _) = encoding_rs::GBK.encode(content);
        fs::write(&path, encoded.as_ref()).unwrap();
        (dir, path)
    }

    /// 在基础 JCL 上追加一条本首领的专属宝箱 NPC（type=8）。
    fn write_target_jcl_with_chest(label: &str) -> (PathBuf, PathBuf) {
        let dir = temp_jcl_dir(label);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("target.jcl");
        let content = "crc\t1\t100\t0\t1\t{true,\"realm::U123456/1\",0,835}\ncrc\t2\t101\t0\t8\t{9,\"史朝义\",139312}\ncrc\t2\t101\t0\t8\t{30,\"史朝义宝箱\",140026}\ncrc\t3\t102\t0\t9\t{9,true,100,100}\ncrc\t4\t103\t0\t21\t{1,9,0,0,0,0,0,0,{[13]=1,[14]=0}}\ncrc\t5\t104\t0\t9\t{9,false,100,100}\ncrc\t6\t105\t0\t1\t{false,\"realm::U123456/1\",0,835}\n";
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
