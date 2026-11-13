//! 真实日志回放仅在显式指定本机样本目录时运行。
//!
//! 不内置账号路径、UID 或角色名；它们必须由运行命令的环境变量提供。回放的
//! ChatLog 只读打开，持久化部分使用内存 SQLite，避免碰到用户的生产数据库。

use std::path::PathBuf;

use rusqlite::Connection;

use crate::db::{self, AutoTrialRecord, TrialDrawReward};
use crate::mingyi::active_detector::parse_info_jx3dat;

use super::trial::{self, TrialCandidateStatus};
use super::trial_receipt::{self, TrialDrawEvidence, TrialReceiptSearch};

const SAMPLE_DAY_START_MS: i64 = 1_793_635_200_000;
const SAMPLE_DAY_END_MS: i64 = 1_793_721_600_000;
const SAMPLE_TERMINAL_MS: i64 = 1_793_718_659_000;

fn required_env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| {
        panic!("真实试炼回放需要环境变量 {key}；此测试不会使用或写入默认游戏目录/生产数据库")
    })
}

fn sample_identity() -> (PathBuf, String, String, String) {
    let account_dir = PathBuf::from(required_env("JX3_TRIAL_DRAW_REPLAY_ACCOUNT_DIR"));
    let expected_uid = required_env("JX3_TRIAL_DRAW_REPLAY_UID");
    let expected_role = required_env("JX3_TRIAL_DRAW_REPLAY_ROLE");

    let folder_uid = account_dir
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("@zhcn_hd"));
    assert_eq!(
        folder_uid,
        Some(expected_uid.as_str()),
        "账号目录名必须与待回放 UID 一致"
    );

    let mut identity = parse_info_jx3dat(&account_dir.join("info.jx3dat"), None)
        .expect("账号目录必须包含可解析的 info.jx3dat 身份信息");
    assert_eq!(identity.uid, expected_uid, "info.jx3dat UID 不匹配");
    // 新版插件可能不再在 info 中写 name。显式指定同一游戏目录时，
    // 只读查询已有身份回退表；仍要求 UID 和预期角色名同时匹配，不绕过身份校验。
    if identity.role_name.is_empty() {
        let game_dir = PathBuf::from(required_env("JX3_TRIAL_DRAW_REPLAY_GAME_DIR"));
        let expected_account = game_dir
            .join(crate::game_directory::MINGYI_ACCOUNTS_BASE_PATH)
            .join(format!("{expected_uid}@zhcn_hd"));
        assert_eq!(
            expected_account.canonicalize().expect("核验游戏目录失败"),
            account_dir.canonicalize().expect("核验账号目录失败"),
            "身份回退库必须来自样本所在的同一游戏目录"
        );
        let owner_db = Connection::open_with_flags(
            game_dir
                .join(crate::game_directory::MINGYI_ACCOUNTS_BASE_PATH)
                .join("!all-users@zhcn_hd/userdata/role_statistics/equip_stat.v4.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .expect("只读打开茗伊角色身份库失败");
        identity.role_name = owner_db
            .query_row(
                "SELECT ownername FROM OwnerInfo WHERE ownerkey = ?1",
                [expected_uid.parse::<i64>().expect("样本 UID 必须是数字")],
                |row| row.get(0),
            )
            .expect("身份库中必须存在相同 UID 的角色名");
    }
    assert_eq!(
        identity.role_name, expected_role,
        "info.jx3dat 角色名不匹配"
    );
    (
        account_dir,
        identity.uid,
        identity.role_name,
        identity.server,
    )
}

fn assert_saved_draws(conn: &Connection, parent_key: &str, expected: &[TrialDrawEvidence]) {
    let parent_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM trial_records WHERE source_key = ?1",
            [parent_key],
            |row| row.get(0),
        )
        .expect("读取内存库父记录失败");
    assert_eq!(parent_count, 1, "一轮通关只能有一个父记录");

    let mut statement = conn
        .prepare(
            "SELECT sequence, receipt_time, reward_items, stamina_cost, stamina_costs,
                    flipped_index, card_1, card_2, card_3, card_4, card_5
             FROM trial_draws
             WHERE trial_record_id = (SELECT id FROM trial_records WHERE source_key = ?1)
             ORDER BY sequence ASC",
        )
        .expect("准备读取内存翻牌记录失败");
    let rows = statement
        .query_map([parent_key], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<i64>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
            ))
        })
        .expect("查询内存翻牌记录失败")
        .collect::<Result<Vec<_>, _>>()
        .expect("读取内存翻牌记录失败");

    assert_eq!(rows.len(), expected.len());
    for (index, (row, evidence)) in rows.iter().zip(expected).enumerate() {
        let (
            sequence,
            receipt_time,
            reward_items_json,
            stamina_cost,
            stamina_costs_json,
            flipped_index,
            card_1,
            card_2,
            card_3,
            card_4,
            card_5,
        ) = row;
        assert_eq!(*sequence, (index + 1) as i64, "翻牌序号必须从 1 连续编号");
        assert_eq!(*receipt_time, evidence.receipt_at_ms);
        assert_eq!(*stamina_cost, evidence.stamina_cost);
        assert_eq!(
            serde_json::from_str::<Vec<i64>>(stamina_costs_json).expect("精力明细必须是 JSON"),
            evidence
                .stamina_parts
                .iter()
                .map(|part| part.cost)
                .collect::<Vec<_>>()
        );
        let rewards = serde_json::from_str::<Vec<TrialDrawReward>>(reward_items_json)
            .expect("奖励明细必须是 JSON");
        assert_eq!(
            rewards,
            evidence
                .rewards
                .iter()
                .map(|reward| TrialDrawReward {
                    name: reward.name.clone(),
                    quantity: reward.quantity,
                })
                .collect::<Vec<_>>(),
            "持久化奖励必须与生产映射一致"
        );

        // JCL 与 ChatLog 都没有牌位或装备归属证据，不能把历史单次字段虚构到任一翻牌。
        assert_eq!(*flipped_index, 0, "历史翻牌位应保持未知");
        assert!(
            [card_1, card_2, card_3, card_4, card_5]
                .iter()
                .all(|card| card.is_empty()),
            "无截图证据时五张牌都应保持未知"
        );
    }
}

#[test]
#[ignore = "需要显式提供本机的已核验真实日志样本"]
fn replays_one_clearance_with_four_independent_draws_twice() {
    let (account_dir, uid, role_name, server) = sample_identity();

    // 限定当天文件名窗口，仍让 trial 分组器看见凌晨保存的第一关与晚间续打。
    let (_, trial_files, _) = super::scan_jcl_files_with_trial_context(
        &account_dir,
        0,
        SAMPLE_DAY_START_MS,
        SAMPLE_DAY_END_MS,
        SAMPLE_DAY_START_MS,
    )
    .expect("枚举真实试炼 JCL 失败");
    let first_scan = trial::scan_trial_records(&trial_files, &account_dir, &uid, 0, 0)
        .expect("首次真实 JCL 分组失败");
    let second_scan = trial::scan_trial_records(&trial_files, &account_dir, &uid, 0, 0)
        .expect("补扫真实 JCL 分组失败");
    assert_eq!(
        first_scan.candidates, second_scan.candidates,
        "补扫不能改变同一轮分组"
    );
    assert_eq!(first_scan.candidates.len(), 1, "当天应只识别到一轮完整试炼");
    assert_eq!(
        first_scan
            .completion_capture_candidates()
            .iter()
            .map(|candidate| candidate.completed_at_ms)
            .collect::<Vec<_>>(),
        vec![SAMPLE_TERMINAL_MS],
        "凌晨第一关和第二关的保存进度不能产生截图终局候选"
    );
    let candidate = &first_scan.candidates[0];
    assert_eq!(candidate.completed_at_ms, SAMPLE_TERMINAL_MS);
    assert_eq!(candidate.layer, Some(6));
    assert_eq!(candidate.bosses, ["寅", "濯尘", "叶冬辰"]);
    assert_eq!(candidate.status, TrialCandidateStatus::Confirmed);

    let draws = trial_receipt::collect_trial_draws(
        &account_dir,
        &uid,
        candidate.completed_at_ms,
        None,
        TrialReceiptSearch::Historical,
    );
    assert_eq!(draws.len(), 4, "终局后应保留四次独立翻牌回执");
    assert!(draws.iter().all(|draw| !draw.ambiguous));
    assert_eq!(
        draws
            .iter()
            .map(|draw| draw.receipt_at_ms)
            .collect::<Vec<_>>(),
        vec![
            1_793_718_660_000,
            1_793_718_680_000,
            1_793_718_682_000,
            1_793_718_687_000,
        ],
        "凌晨和第二关后的叶子奖励绝不能归入终局翻牌"
    );
    assert_eq!(
        draws
            .iter()
            .map(|draw| draw.leaf_quantity)
            .collect::<Vec<_>>(),
        vec![31, 29, 32, 33]
    );
    assert_eq!(
        draws
            .iter()
            .map(|draw| {
                draw.rewards
                    .iter()
                    .map(|reward| (reward.name.as_str(), reward.quantity))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                ("银叶子·试炼之地", 31),
                ("九花玉露散", 10),
                ("五行石（一级）", 200),
            ],
            vec![
                ("银叶子·试炼之地", 29),
                ("福禄宝箱·雾海寻龙", 1),
                ("九花玉露散", 8),
            ],
            vec![
                ("银叶子·试炼之地", 32),
                ("彩·耐受·血魂·破势(肆)", 1),
                ("九花玉露散", 10),
            ],
            vec![
                ("银叶子·试炼之地", 33),
                ("匡义令·月", 36),
                ("九花玉露散", 8),
            ],
        ],
        "四次翻牌各应保留三项同秒、同来源的奖励"
    );
    assert_eq!(
        draws
            .iter()
            .map(|draw| draw.stamina_cost)
            .collect::<Vec<_>>(),
        vec![None, Some(1000), Some(1000), Some(1000)]
    );
    assert_eq!(
        draws[3]
            .stamina_parts
            .iter()
            .map(|part| part.cost)
            .collect::<Vec<_>>(),
        vec![125, 875]
    );

    let auto_record = AutoTrialRecord {
        source_key: candidate.dedupe_key.clone(),
        account_id: "trial-draw-replay".to_string(),
        role_id: uid,
        role_name,
        server,
        layer: i64::from(candidate.layer.expect("完整通关必须有层数")),
        bosses: candidate.bosses.clone(),
        status: "confirmed".to_string(),
        date: candidate.completed_at_ms,
        notes: None,
    };
    let auto_draws = draws
        .iter()
        .map(TrialDrawEvidence::to_auto_trial_draw)
        .collect::<Vec<_>>();
    let mut conn = Connection::open_in_memory().expect("创建隔离内存数据库失败");
    db::install_fresh_db_for_test(&conn).expect("初始化隔离内存数据库失败");

    assert_eq!(
        db::db_upsert_auto_trial_records_with_connection(&mut conn, &[auto_record.clone()])
            .expect("首次保存通关失败"),
        1
    );
    assert_eq!(
        db::db_upsert_auto_trial_draws_with_connection(
            &mut conn,
            &auto_record.source_key,
            &auto_draws
        )
        .expect("首次保存翻牌失败"),
        4
    );
    // 第二次补扫必须不多算通关，也不重复创建翻牌子记录。
    assert_eq!(
        db::db_upsert_auto_trial_records_with_connection(&mut conn, &[auto_record])
            .expect("第二次保存通关失败"),
        0
    );
    assert_eq!(
        db::db_upsert_auto_trial_draws_with_connection(
            &mut conn,
            &candidate.dedupe_key,
            &auto_draws
        )
        .expect("第二次保存翻牌失败"),
        0
    );
    assert_saved_draws(&conn, &candidate.dedupe_key, &draws);
}
