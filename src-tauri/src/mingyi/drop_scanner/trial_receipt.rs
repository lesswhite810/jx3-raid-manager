//! 试炼之地终局后的本人翻牌入包回执。
//!
//! JCL 不写入 `MSG_ITEM`，因此每张牌、物品和精力均只从角色身份已核实的茗伊
//! ChatLog 读取；绝不把 JCL 中的玩家聊天文本当作本人入包。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::{Connection, OpenFlags};

/// 一轮结算后可连续翻牌较久；没有下一可信试炼事件时最多保留十五分钟。
/// 调用方必须把下一可信战斗、关卡或终局的最早时间传入，以免旧终局吸收新进度。
const RECEIPT_WINDOW_MS: i64 = 15 * 60_000;
/// 实时核验只碰可能仍在写入本轮数据的聊天库，避免逐个打开陈旧历史库。
const CHATLOG_MTIME_LOOKBACK_MS: i64 = 5 * 60_000;

const STAMINA_RECEIPT_PREFIX: &str = "消耗精力";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TrialReceiptSearch {
    Realtime,
    Historical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrialDrawReward {
    pub name: String,
    pub quantity: i64,
    pub source_hash: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrialStaminaPart {
    pub cost: i64,
    pub source_hash: i64,
}

/// `ambiguous` 表示同秒有多张银叶子；此时不猜测同秒其它行属于哪张牌。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrialDrawEvidence {
    pub draw_key: String,
    pub receipt_at_ms: i64,
    pub receipt_hash: i64,
    pub leaf_quantity: i64,
    pub rewards: Vec<TrialDrawReward>,
    pub stamina_cost: Option<i64>,
    pub stamina_parts: Vec<TrialStaminaPart>,
    pub ambiguous: bool,
}

impl TrialDrawEvidence {
    /// 生产扫描与历史回放共用的持久化投影；不在转换层补造或分摊证据。
    pub(super) fn to_auto_trial_draw(&self) -> crate::db::AutoTrialDraw {
        crate::db::AutoTrialDraw {
            source_key: self.draw_key.clone(),
            receipt_time: self.receipt_at_ms,
            receipt_hash: Some(self.receipt_hash.to_string()),
            reward_items: self
                .rewards
                .iter()
                .map(|reward| crate::db::TrialDrawReward {
                    name: reward.name.clone(),
                    quantity: reward.quantity,
                })
                .collect(),
            reward_source_hashes: self
                .rewards
                .iter()
                .map(|reward| reward.source_hash.to_string())
                .collect(),
            stamina_cost: self.stamina_cost,
            stamina_costs: self.stamina_parts.iter().map(|part| part.cost).collect(),
            stamina_source_hashes: self
                .stamina_parts
                .iter()
                .map(|part| part.source_hash.to_string())
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChatReceiptRow {
    hash: i64,
    time: i64,
    kind: String,
    text: String,
}

/// 收集终局后本地角色获得“银叶子·试炼之地”的去重秒级时间，统一转为毫秒。
///
/// `next_trusted_event_at_ms` 是下一条可信战斗、关卡或终局事件的起点（开区间）。
/// 它与固定十五分钟上限共同限制查询，避免把后一轮或继续翻牌的入包借给本轮。
/// 返回空数组代表当前没有足够证据，调用方可以等待下一次扫描重试。
///
/// 只查询 mtime 接近终局的 chatlog，且 ChatInfo.user_global_id 必须与 JCL UID
/// 完全一致。`MSG_ITEM.text` 还必须是完整的本地入包格式，不能以宽松前缀匹配。
#[cfg(test)]
fn collect_trial_receipt_times_ms(
    account_dir: &Path,
    expected_uid: &str,
    terminal_at_ms: i64,
    next_trusted_event_at_ms: Option<i64>,
) -> Vec<i64> {
    collect_trial_draws(
        account_dir,
        expected_uid,
        terminal_at_ms,
        next_trusted_event_at_ms,
        TrialReceiptSearch::Realtime,
    )
    .into_iter()
    .map(|draw| draw.receipt_at_ms)
    .collect::<std::collections::BTreeSet<_>>()
    .into_iter()
    .collect()
}

/// 迟到对账专用：在标准窗口关闭后（封存式落盘超过 15 分钟）重新查询聊天库。
///
/// 与 [`collect_trial_draws`] 完全一致，仅把查询上界从 `terminal+15min` 放宽到
/// `terminal+window_ms`（仍受下一可信事件硬上界约束），并强制走全量文件枚举
/// （等价 Historical 搜索，不受实时 mtime 过滤影响）。只允许扩大，不允许缩小：
/// 传入更短窗口时仍按标准窗口查询，保证不改变已有行为。
pub(super) fn collect_trial_draws_late(
    account_dir: &Path,
    expected_uid: &str,
    terminal_at_ms: i64,
    next_trusted_event_at_ms: Option<i64>,
    window_ms: i64,
) -> Vec<TrialDrawEvidence> {
    collect_trial_draws_with_window(
        account_dir,
        expected_uid,
        terminal_at_ms,
        next_trusted_event_at_ms,
        TrialReceiptSearch::Historical,
        window_ms,
    )
}

/// 从角色身份已核验的 ChatLog 解析终局后的每一次翻牌。
///
/// ChatLog 的主键是 `(time, hash)`，所以同秒不同消息不能仅按秒去重。仅当同秒
/// 存在一条银叶子时，才将同秒的可信物品和精力行归给该牌；多条银叶子仍保留
/// 独立叶子来源，但不分摊其它行。
pub(super) fn collect_trial_draws(
    account_dir: &Path,
    expected_uid: &str,
    terminal_at_ms: i64,
    next_trusted_event_at_ms: Option<i64>,
    search: TrialReceiptSearch,
) -> Vec<TrialDrawEvidence> {
    collect_trial_draws_with_window(
        account_dir,
        expected_uid,
        terminal_at_ms,
        next_trusted_event_at_ms,
        search,
        RECEIPT_WINDOW_MS,
    )
}

fn collect_trial_draws_with_window(
    account_dir: &Path,
    expected_uid: &str,
    terminal_at_ms: i64,
    next_trusted_event_at_ms: Option<i64>,
    search: TrialReceiptSearch,
    window_ms: i64,
) -> Vec<TrialDrawEvidence> {
    let Some((start_sec, end_sec)) =
        receipt_window_seconds(terminal_at_ms, next_trusted_event_at_ms, window_ms)
    else {
        return Vec::new();
    };
    if !is_numeric_uid(expected_uid) {
        return Vec::new();
    }

    let earliest_mtime_ms = terminal_at_ms.saturating_sub(CHATLOG_MTIME_LOOKBACK_MS);
    let mut rows_by_source = BTreeMap::new();
    for path in chatlog_files(account_dir, earliest_mtime_ms, search) {
        let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            log::debug!("[TrialReceipt] 无法只读打开 chatlog: {}", path.display());
            continue;
        };
        if chatlog_uid(&conn).as_deref() != Some(expected_uid) {
            continue;
        }
        if !chatlog_covers_seconds(&conn, start_sec, end_sec) {
            continue;
        }
        let mut statement = match conn.prepare(
            "SELECT hash, time, type, text FROM ChatLog \
             WHERE type IN ('MSG_ITEM', 'MSG_THEW_STAMINA') AND time >= ?1 AND time < ?2 \
             ORDER BY time ASC, hash ASC",
        ) {
            Ok(statement) => statement,
            Err(error) => {
                log::debug!(
                    "[TrialReceipt] 查询 chatlog 失败 {}: {}",
                    path.display(),
                    error
                );
                continue;
            }
        };
        let rows = statement.query_map(rusqlite::params![start_sec, end_sec], |row| {
            Ok(ChatReceiptRow {
                hash: row.get(0)?,
                time: row.get(1)?,
                kind: row.get(2)?,
                text: row.get(3)?,
            })
        });
        if let Ok(rows) = rows {
            for row in rows.flatten() {
                rows_by_source.entry((row.time, row.hash)).or_insert(row);
            }
        }
    }
    build_draw_evidence(expected_uid, rows_by_source.into_values().collect())
}

fn receipt_window_seconds(
    terminal_at_ms: i64,
    next_trusted_event_at_ms: Option<i64>,
    window_ms: i64,
) -> Option<(i64, i64)> {
    if terminal_at_ms <= 0 {
        return None;
    }
    let upper_ms = terminal_at_ms.saturating_add(window_ms).min(
        next_trusted_event_at_ms
            .filter(|time| *time > terminal_at_ms)
            .unwrap_or(i64::MAX),
    );
    let start_sec = terminal_at_ms.div_euclid(1000);
    // ChatLog 只有秒精度；上界采用 ceil 并保持 SQL 开区间，恰好落在终局秒的
    // 入包仍属于本轮，恰好到下一可信事件秒的入包则不属于本轮。
    let end_sec = (upper_ms.saturating_add(999)).div_euclid(1000);
    (end_sec > start_sec).then_some((start_sec, end_sec))
}

fn parse_exact_item_receipt(text: &str) -> Option<(String, i64)> {
    // 实际 ChatLog 的 text 字段会保留 UI 行尾换行；只移除 CR/LF，不放宽其它后缀。
    let text = text.trim_end_matches(['\r', '\n']);
    let receipt = text.strip_prefix("你获得：[")?;
    let (name, suffix) = receipt.split_once("]")?;
    if name.is_empty() {
        return None;
    }
    let quantity = if suffix == "。" {
        1
    } else {
        suffix
            .strip_prefix(" × ")?
            .strip_suffix("。")?
            .parse::<i64>()
            .ok()?
    };
    (quantity > 0).then_some((name.to_owned(), quantity))
}

fn parse_stamina_receipt(text: &str) -> Option<i64> {
    let text = text.trim_end_matches(['\r', '\n']);
    text.strip_prefix(STAMINA_RECEIPT_PREFIX)?
        .strip_suffix("点。")?
        .parse::<i64>()
        .ok()
        .filter(|cost| *cost > 0)
}

fn build_draw_evidence(expected_uid: &str, rows: Vec<ChatReceiptRow>) -> Vec<TrialDrawEvidence> {
    let mut rows_by_time: BTreeMap<i64, Vec<ChatReceiptRow>> = BTreeMap::new();
    for row in rows {
        rows_by_time.entry(row.time).or_default().push(row);
    }

    let mut draws = Vec::new();
    for (time, rows) in rows_by_time {
        let leaves = rows
            .iter()
            .filter(|row| row.kind == "MSG_ITEM")
            .filter_map(|row| {
                let (name, quantity) = parse_exact_item_receipt(&row.text)?;
                (name == "银叶子·试炼之地").then_some((row.hash, quantity))
            })
            .collect::<Vec<_>>();
        if leaves.is_empty() {
            continue;
        }

        let ambiguous = leaves.len() != 1;
        let companion_rewards = (!ambiguous)
            .then(|| {
                rows.iter()
                    .filter(|row| row.kind == "MSG_ITEM")
                    .filter_map(|row| {
                        let (name, quantity) = parse_exact_item_receipt(&row.text)?;
                        (name != "银叶子·试炼之地").then_some(TrialDrawReward {
                            name,
                            quantity,
                            source_hash: row.hash,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let stamina_parts = (!ambiguous)
            .then(|| {
                rows.iter()
                    .filter(|row| row.kind == "MSG_THEW_STAMINA")
                    .filter_map(|row| {
                        parse_stamina_receipt(&row.text).map(|cost| TrialStaminaPart {
                            cost,
                            source_hash: row.hash,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let stamina_cost =
            (!stamina_parts.is_empty()).then(|| stamina_parts.iter().map(|part| part.cost).sum());

        for (hash, quantity) in leaves {
            // 银叶子的实际数量也是这次翻牌可展示的奖励。即使同秒多叶且其它行
            // 归属不明，每张牌仍可安全保留自己的叶子来源与数量。
            let mut rewards = vec![TrialDrawReward {
                name: "银叶子·试炼之地".to_owned(),
                quantity,
                source_hash: hash,
            }];
            rewards.extend(companion_rewards.iter().cloned());
            draws.push(TrialDrawEvidence {
                draw_key: format!("trial-draw:{expected_uid}:{time}:{hash}"),
                receipt_at_ms: time.saturating_mul(1000),
                receipt_hash: hash,
                leaf_quantity: quantity,
                rewards,
                stamina_cost,
                stamina_parts: stamina_parts.clone(),
                ambiguous,
            });
        }
    }
    draws
}

fn chatlog_files(
    account_dir: &Path,
    earliest_mtime_ms: i64,
    search: TrialReceiptSearch,
) -> Vec<PathBuf> {
    let account_dir = account_dir.to_path_buf();
    let all_files = super::find_chatlog_files(&account_dir).unwrap_or_default();
    let files: Vec<PathBuf> = all_files
        .iter()
        .filter(|entry| match search {
            TrialReceiptSearch::Historical => true,
            TrialReceiptSearch::Realtime => fs::metadata(entry)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .is_some_and(|duration| duration.as_millis() as i64 >= earliest_mtime_ms),
        })
        .cloned()
        .collect();
    // 诊断：实时窗口按 mtime 过滤掉所有聊天库时明确记录。茗伊封存式落盘
    // 会让活跃库长期不动，导致实时窗口一个文件都查不到，这是"迟到回执"
    // 场景下最容易被忽视的静默失败点。
    if search == TrialReceiptSearch::Realtime && files.len() < all_files.len() {
        log::info!(
            "[TrialReceipt] 实时 mtime 过滤: {} 个聊天库中仅 {} 个满足最近落盘要求 (earliest_mtime_ms={earliest_mtime_ms})",
            all_files.len(),
            files.len()
        );
    }
    files
}

fn chatlog_uid(conn: &Connection) -> Option<String> {
    let value = conn
        .query_row(
            "SELECT value FROM ChatInfo WHERE key = 'user_global_id'",
            [],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()?;
    super::normalize_chatlog_user_global_id(&value)
}

/// 用 ChatInfo 的轻量元数据过滤掉同样是近期修改、但时间范围不相交的聊天库。
fn chatlog_covers_seconds(conn: &Connection, start_sec: i64, end_sec: i64) -> bool {
    let range = conn
        .prepare("SELECT key, value FROM ChatInfo WHERE key IN ('min_time', 'max_time')")
        .ok()
        .and_then(|mut statement| {
            let mut min_time = None;
            let mut max_time = None;
            for row in statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .ok()?
                .flatten()
            {
                match row.0.as_str() {
                    "min_time" => min_time = row.1.parse::<i64>().ok(),
                    "max_time" => max_time = row.1.parse::<i64>().ok(),
                    _ => {}
                }
            }
            Some((min_time?, max_time?))
        });
    match range {
        Some((min_time, max_time)) => {
            min_time <= end_sec && (max_time == 0 || max_time >= start_sec)
        }
        // 当前活跃库有时尚未写入 max_time；保守地保留给精确 MSG_ITEM 查询，不扫全表。
        None => true,
    }
}

fn is_numeric_uid(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn temp_account() -> PathBuf {
        let path = std::env::temp_dir().join(format!("trial-receipt-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(path.join("userdata").join("chat_log")).unwrap();
        path
    }

    fn add_chatlog(account: &Path, uid: &str, rows: &[(i64, &str, &str, i64)], min: i64, max: i64) {
        let path = account.join("userdata/chat_log/chatlog_current.v2.db");
        let conn = Connection::open(path).unwrap();
        conn.execute_batch("CREATE TABLE ChatInfo(key TEXT, value TEXT); CREATE TABLE ChatLog(hash INTEGER, type TEXT, text TEXT, time INTEGER, PRIMARY KEY(time, hash));").unwrap();
        conn.execute("INSERT INTO ChatInfo VALUES ('user_global_id', ?1)", [uid])
            .unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('min_time', ?1)",
            [min.to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ChatInfo VALUES ('max_time', ?1)",
            [max.to_string()],
        )
        .unwrap();
        for (hash, kind, text, time) in rows {
            conn.execute(
                "INSERT INTO ChatLog VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![hash, kind, text, time],
            )
            .unwrap();
        }
    }

    #[test]
    fn chatlog_accepts_only_exact_local_leaf_receipt_after_terminal() {
        let account = temp_account();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let terminal = now_ms / 1000 * 1000;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "123456",
            &[
                (
                    1,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 41。",
                    second - 1,
                ),
                (
                    2,
                    "MSG_ITEM",
                    "别人获得：[银叶子·试炼之地] × 41。",
                    second + 1,
                ),
                (
                    3,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地]（伪造）。",
                    second + 2,
                ),
                (
                    4,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 41。\n",
                    second + 3,
                ),
                (5, "MSG_ITEM", "你获得：[银叶子·试炼之地] × 8。", second + 3),
            ],
            second - 10,
            second + 10,
        );
        assert_eq!(
            collect_trial_receipt_times_ms(&account, "123456", terminal, None),
            vec![(second + 3) * 1000]
        );
        let _ = fs::remove_dir_all(account);
    }

    #[test]
    fn rejects_nearby_receipts_outside_next_terminal_boundary() {
        let account = temp_account();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let terminal = now_ms / 1000 * 1000;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "123456",
            &[
                (
                    1,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 41。",
                    second - 1,
                ),
                (
                    2,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 41。",
                    second + 4,
                ),
                (
                    3,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 41。",
                    second + 901,
                ),
            ],
            second - 10,
            second + 910,
        );
        assert!(collect_trial_receipt_times_ms(
            &account,
            "123456",
            terminal,
            Some((second + 3) * 1000)
        )
        .is_empty());
        assert_eq!(
            collect_trial_receipt_times_ms(&account, "123456", terminal, None),
            vec![(second + 4) * 1000]
        );
        let _ = fs::remove_dir_all(account);
    }

    #[test]
    fn rejects_cross_uid_and_non_positive_terminal() {
        let account = temp_account();
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let terminal = now_ms / 1000 * 1000;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "654321",
            &[(
                1,
                "MSG_ITEM",
                "你获得：[银叶子·试炼之地] × 41。",
                second + 2,
            )],
            second,
            second + 5,
        );
        assert!(collect_trial_receipt_times_ms(&account, "123456", terminal, None).is_empty());
        assert!(collect_trial_receipt_times_ms(&account, "123456", -1, None).is_empty());
        let _ = fs::remove_dir_all(account);
    }

    #[test]
    fn retains_each_draw_reward_and_all_same_second_stamina_parts() {
        let account = temp_account();
        let terminal = 1_793_718_659_000_i64;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "123456",
            &[
                (
                    11,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 31。",
                    second + 1,
                ),
                (12, "MSG_ITEM", "你获得：[九花玉露散] × 10。", second + 1),
                (
                    21,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 29。",
                    second + 21,
                ),
                (22, "MSG_ITEM", "你获得：[福禄宝箱]。", second + 21),
                (23, "MSG_THEW_STAMINA", "消耗精力1000点。", second + 21),
                (
                    31,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 32。",
                    second + 23,
                ),
                (32, "MSG_THEW_STAMINA", "消耗精力1000点。", second + 23),
                (
                    41,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 33。",
                    second + 28,
                ),
                (42, "MSG_THEW_STAMINA", "消耗精力125点。", second + 28),
                (43, "MSG_THEW_STAMINA", "消耗精力875点。", second + 28),
            ],
            second,
            second + 30,
        );

        let draws = collect_trial_draws(
            &account,
            "123456",
            terminal,
            None,
            TrialReceiptSearch::Historical,
        );
        assert_eq!(
            draws
                .iter()
                .map(|draw| draw.leaf_quantity)
                .collect::<Vec<_>>(),
            vec![31, 29, 32, 33]
        );
        assert_eq!(draws[0].rewards[0].name, "银叶子·试炼之地");
        assert!(draws[0]
            .rewards
            .iter()
            .any(|reward| reward.name == "九花玉露散"));
        assert_eq!(draws[1].stamina_cost, Some(1000));
        assert_eq!(draws[2].stamina_cost, Some(1000));
        assert_eq!(draws[3].stamina_cost, Some(1000));
        assert_eq!(
            draws[3]
                .stamina_parts
                .iter()
                .map(|part| part.cost)
                .collect::<Vec<_>>(),
            vec![125, 875]
        );
        let persisted = draws[3].to_auto_trial_draw();
        assert_eq!(persisted.reward_items[0].quantity, 33);
        assert_eq!(persisted.stamina_costs, vec![125, 875]);
        assert_eq!(persisted.stamina_source_hashes, vec!["42", "43"]);
        assert!(draws.iter().all(|draw| !draw.ambiguous));
        let _ = fs::remove_dir_all(account);
    }

    #[test]
    fn stamina_uses_each_logged_amount_without_assuming_a_pool_split_or_total() {
        for amounts in [
            vec![1, 999],
            vec![999, 1],
            vec![500, 500],
            vec![1],
            vec![73],
        ] {
            let mut rows = vec![ChatReceiptRow {
                hash: 1,
                time: 100,
                kind: "MSG_ITEM".into(),
                text: "你获得：[银叶子·试炼之地]。".into(),
            }];
            for (index, cost) in amounts.iter().enumerate() {
                rows.push(ChatReceiptRow {
                    hash: index as i64 + 2,
                    time: 100,
                    kind: "MSG_THEW_STAMINA".into(),
                    text: format!("消耗精力{cost}点。"),
                });
            }
            let draws = build_draw_evidence("123456", rows);
            assert_eq!(draws.len(), 1);
            assert_eq!(draws[0].stamina_cost, Some(amounts.iter().sum()));
            assert_eq!(
                draws[0]
                    .stamina_parts
                    .iter()
                    .map(|part| part.cost)
                    .collect::<Vec<_>>(),
                amounts
            );
        }
    }

    #[test]
    fn retains_slow_draw_after_one_minute_until_the_next_trusted_boundary() {
        let account = temp_account();
        let terminal = 1_793_718_659_000_i64;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "123456",
            &[
                (
                    1,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 31。",
                    second + 61,
                ),
                (2, "MSG_THEW_STAMINA", "消耗精力1000点。", second + 61),
                (
                    3,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 32。",
                    second + 121,
                ),
            ],
            second,
            second + 130,
        );
        let draws = collect_trial_draws(
            &account,
            "123456",
            terminal,
            Some((second + 90) * 1000),
            TrialReceiptSearch::Historical,
        );
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].leaf_quantity, 31);
        assert_eq!(draws[0].stamina_cost, Some(1000));
        let _ = fs::remove_dir_all(account);
    }

    #[test]
    fn same_second_multiple_leaves_are_preserved_without_splitting_rewards_or_stamina() {
        let account = temp_account();
        let terminal = 1_793_718_659_000_i64;
        let second = terminal / 1000;
        add_chatlog(
            &account,
            "123456",
            &[
                (
                    1,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 31。",
                    second + 1,
                ),
                (
                    2,
                    "MSG_ITEM",
                    "你获得：[银叶子·试炼之地] × 32。",
                    second + 1,
                ),
                (3, "MSG_ITEM", "你获得：[九花玉露散] × 10。", second + 1),
                (4, "MSG_THEW_STAMINA", "消耗精力1000点。", second + 1),
            ],
            second,
            second + 2,
        );
        let draws = collect_trial_draws(
            &account,
            "123456",
            terminal,
            None,
            TrialReceiptSearch::Historical,
        );
        assert_eq!(draws.len(), 2);
        assert_eq!(draws[0].receipt_hash, 1);
        assert_eq!(draws[1].receipt_hash, 2);
        assert!(draws.iter().all(|draw| draw.ambiguous));
        assert_eq!(
            draws
                .iter()
                .map(|draw| draw.rewards[0].quantity)
                .collect::<Vec<_>>(),
            vec![31, 32]
        );
        assert!(draws.iter().all(|draw| draw.rewards.len() == 1));
        assert!(draws.iter().all(|draw| draw.stamina_cost.is_none()));
        assert!(draws.iter().all(|draw| draw.stamina_parts.is_empty()));
        let _ = fs::remove_dir_all(account);
    }
}
