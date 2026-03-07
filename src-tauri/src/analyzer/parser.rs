//! 聊天记录解析模块
//!
//! 读取 SQLite 数据库中的聊天记录并解析

#![allow(dead_code)]

use super::{ChatLogEntry, TimeRange};
use std::path::PathBuf;

/// 聊天记录表名（固定）
const CHAT_LOG_TABLE: &str = "ChatLog";

/// 读取聊天记录条目
/// 注意：time_range 是毫秒，数据库中的 time 是秒
pub fn read_chat_log_entries(
    db_path: &str,
    time_range: &TimeRange,
    batch_size: usize,
    offset: usize,
) -> Result<Vec<ChatLogEntry>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = time_range.start / 1000;
    // 如果结束时间大于 2050 年，视为"无限"
    let end_sec = if time_range.end > 2524579200000 {
        i64::MAX
    } else {
        time_range.end / 1000
    };

    let query = format!(
        "SELECT time, text, msg FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         ORDER BY time ASC \
         LIMIT ?3 OFFSET ?4",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let entries = stmt
        .query_map(
            rusqlite::params![start_sec, end_sec, batch_size as i64, offset as i64],
            |row| {
                // 数据库中的 time 是秒，前端需要毫秒
                let time_sec: i64 = row.get(0)?;
                Ok(ChatLogEntry {
                    time: time_sec * 1000,
                    text: row.get(1)?,
                    msg: row.get(2)?,
                })
            },
        )
        .map_err(|e| format!("查询失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取记录失败: {}", e))?;

    // 首批读取时打印解析条件
    if offset == 0 {
        // 注意：数据库中的 time 是本地时间戳（将本地时间直接当作 UTC 计算的时间戳）
        // 使用 DateTime::from_timestamp 获取 UTC 时间，然后提取 naive 部分
        // 这样可以正确显示本地时间戳对应的本地时间
        #[allow(deprecated)]
        let start_str = chrono::NaiveDateTime::from_timestamp_opt(start_sec, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| format!("{}s", start_sec));
        let end_str = if end_sec == i64::MAX {
            "无限制".to_string()
        } else {
            #[allow(deprecated)]
            chrono::NaiveDateTime::from_timestamp_opt(end_sec, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| format!("{}s", end_sec))
        };
        log::info!(
            "读取聊天记录 | 文件: {} | 表: {} | 时间范围: {} ~ {} | 批大小: {} | 首批数量: {}",
            db_path, CHAT_LOG_TABLE, start_str, end_str, batch_size, entries.len()
        );
    } else {
        log::debug!(
            "读取聊天记录 | 文件: {} | 偏移: {} | 数量: {}",
            db_path, offset, entries.len()
        );
    }

    Ok(entries)
}

/// 读取可能包含团长信息的聊天记录（使用SQL过滤减少内存数据量）
/// gkp_start 和 gkp_end 是 GKP 文件的时间范围（毫秒）
pub fn read_leader_chat_log_entries(
    db_path: &str,
    gkp_start: i64,
    gkp_end: i64,
) -> Result<Vec<ChatLogEntry>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = gkp_start / 1000;
    let end_sec = gkp_end / 1000;

    // 使用 SQL LIKE 过滤可能包含团长信息的记录
    // 匹配：战斗倒计时、拍卖记录、总收入播报
    let query = format!(
        "SELECT time, text, msg FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         AND (text LIKE '%【团队倒计时】%' OR text LIKE '%将%记录给了%' OR text LIKE '%拍卖房间目前总收入为%') \
         ORDER BY time ASC",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let entries = stmt
        .query_map(
            rusqlite::params![start_sec, end_sec],
            |row| {
                let time_sec: i64 = row.get(0)?;
                Ok(ChatLogEntry {
                    time: time_sec * 1000,
                    text: row.get(1)?,
                    msg: row.get(2)?,
                })
            },
        )
        .map_err(|e| format!("查询失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取记录失败: {}", e))?;

    log::info!(
        "读取团长相关聊天记录 | 文件: {} | 表: {} | 数量: {}",
        db_path,
        CHAT_LOG_TABLE,
        entries.len()
    );

    Ok(entries)
}

/// 消费记录结构
#[derive(Debug, Clone)]
pub struct ExpenseEntry {
    pub time: i64,
    pub price: i64,
    pub item_name: String,
    pub is_worker_bought: bool,
}

/// 收入记录结构
#[derive(Debug, Clone)]
pub struct IncomeEntry {
    pub time: i64,
    pub amount: i64,  // 金为单位
}

/// 拍团分配信息
#[derive(Debug, Clone)]
pub struct TeamDistributionInfo {
    pub time: i64,
    pub total_income: i64,      // 拍团总收入（金）
    pub subsidy_total: i64,     // 补贴总费用（金）
    pub available: i64,         // 实际可用分配金额（金）
    pub distribute_count: i64,  // 分配人数
    pub base_salary: i64,       // 每人底薪（金）
}

/// 从 msg 字段解析收入金额
/// 格式：<text>text="1" name="Text_GoldB">...</text><text>text="3800" name="Text_Gold">...</text>...
/// Text_GoldB = 金砖, Text_Gold = 金, Text_Silver = 银, Text_Copper = 铜
fn parse_income_from_msg(msg: &str) -> i64 {
    let mut total_gold: i64 = 0;

    // 匹配 <text ... name="Text_GoldB" ...>数字</text>
    let _goldb_re = regex::Regex::new(r#"<text[^>]*name="Text_GoldB"[^>]*>.*?text="(\d+)"[^>]*>.*?</text>"#).unwrap();
    // 匹配 <text ... text="数字" ... name="Text_GoldB" ...>
    let goldb_re2 = regex::Regex::new(r#"<text[^>]*text="(\d+)"[^>]*name="Text_GoldB"[^>]*>"#).unwrap();

    // 匹配 <text ... text="数字" ... name="Text_Gold" ...>
    let gold_re = regex::Regex::new(r#"<text[^>]*text="(\d+)"[^>]*name="Text_Gold"[^>]*>"#).unwrap();

    // 解析金砖
    if let Some(caps) = goldb_re2.captures(msg) {
        let bricks: i64 = caps[1].parse().unwrap_or(0);
        total_gold += bricks * 10000;  // 1金砖 = 10000金
    }

    // 解析金
    if let Some(caps) = gold_re.captures(msg) {
        let gold: i64 = caps[1].parse().unwrap_or(0);
        total_gold += gold;
    }

    // 银和铜忽略

    total_gold
}

/// 读取收入记录（使用 SQL 过滤）
/// 条件：text like "%你获得：%" and type = 'MSG_MONEY'
pub fn read_income_chat_log_entries(
    db_path: &str,
    gkp_start: i64,
    gkp_end: i64,
) -> Result<Vec<IncomeEntry>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = gkp_start / 1000;
    let end_sec = gkp_end / 1000;

    log::info!(
        "收入记录查询 | 时间范围(秒): {} - {} | 条件: text LIKE '%你获得：%' AND type='MSG_MONEY'",
        start_sec, end_sec
    );

    let query = format!(
        "SELECT time, msg FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         AND text LIKE '%你获得：%' \
         AND type = 'MSG_MONEY' \
         ORDER BY time ASC",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let mut incomes = Vec::new();

    let rows = stmt.query_map(
        rusqlite::params![start_sec, end_sec],
        |row| {
            let time_sec: i64 = row.get(0)?;
            let msg: String = row.get(1)?;
            Ok((time_sec * 1000, msg))
        },
    ).map_err(|e| format!("查询失败: {}", e))?;

    for row in rows {
        if let Ok((time, msg)) = row {
            let amount = parse_income_from_msg(&msg);
            if amount > 0 {
                incomes.push(IncomeEntry { time, amount });
                log::debug!("收入记录解析: 时间={}, 金额={}金", time, amount);
            }
        }
    }

    log::info!(
        "读取收入记录 | 文件: {} | 数量: {}",
        db_path,
        incomes.len()
    );

    Ok(incomes)
}

/// 读取拍团分配记录（使用 SQL 过滤）
/// 条件：text like "%：拍团目前总收入为：%"
pub fn read_team_distribution_entries(
    db_path: &str,
    gkp_start: i64,
    gkp_end: i64,
) -> Result<Vec<TeamDistributionInfo>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = gkp_start / 1000;
    let end_sec = gkp_end / 1000;

    log::info!(
        "拍团记录查询 | 时间范围(秒): {} - {} | 条件: text LIKE '%：拍团目前总收入为：%'",
        start_sec, end_sec
    );

    let query = format!(
        "SELECT time, text FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         AND text LIKE '%：拍团目前总收入为：%' \
         ORDER BY time ASC",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let mut distributions = Vec::new();

    let rows = stmt.query_map(
        rusqlite::params![start_sec, end_sec],
        |row| {
            let time_sec: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            Ok((time_sec * 1000, text))
        },
    ).map_err(|e| format!("查询失败: {}", e))?;

    // 正则匹配：[房间][玩家]：拍团目前总收入为：(\d+)金，补贴总费用：(\d+)金，实际可用分配金额：(\d+)金，分配人数：(\d+)，每人底薪：(\d+)金
    let re = regex::Regex::new(
        r#"拍团目前总收入为：(\d+)金，补贴总费用：(\d+)金，实际可用分配金额：(\d+)金，分配人数：(\d+)，每人底薪：(\d+)金"#
    ).unwrap();

    for row in rows {
        if let Ok((time, text)) = row {
            if let Some(caps) = re.captures(&text) {
                let info = TeamDistributionInfo {
                    time,
                    total_income: caps[1].parse().unwrap_or(0),
                    subsidy_total: caps[2].parse().unwrap_or(0),
                    available: caps[3].parse().unwrap_or(0),
                    distribute_count: caps[4].parse().unwrap_or(0),
                    base_salary: caps[5].parse().unwrap_or(0),
                };
                log::info!(
                    "拍团记录解析: 时间={} | 总收入={}金 | 补贴={}金 | 可用={}金 | 人数={} | 底薪={}金",
                    info.time, info.total_income, info.subsidy_total,
                    info.available, info.distribute_count, info.base_salary
                );
                distributions.push(info);
            }
        }
    }

    log::info!(
        "读取拍团记录 | 文件: {} | 数量: {}",
        db_path,
        distributions.len()
    );

    Ok(distributions)
}

/// 读取可能包含消费信息的聊天记录（使用SQL过滤减少内存数据量）
/// role_name 是角色名称（可能包含区服）
pub fn read_expense_chat_log_entries(
    db_path: &str,
    gkp_start: i64,
    gkp_end: i64,
    role_name: &str,
) -> Result<Vec<ExpenseEntry>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = gkp_start / 1000;
    let end_sec = gkp_end / 1000;

    // 使用 SQL LIKE 过滤可能包含消费记录的记录
    // 匹配格式：[房间][角色名]：[角色名]花费[价格金]购买了[物品名]
    // 需要同时匹配角色名、"花费"和"购买了"
    let like_pattern = format!("%{}%花费%购买了%", role_name);
    log::info!(
        "消费记录查询 | 时间范围(秒): {} - {} | LIKE模式: {}",
        start_sec, end_sec, like_pattern
    );

    let query = format!(
        "SELECT time, text FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         AND text LIKE ?3 \
         ORDER BY time ASC",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let mut expenses = Vec::new();

    let rows = stmt.query_map(
        rusqlite::params![start_sec, end_sec, like_pattern],
        |row| {
            let time_sec: i64 = row.get(0)?;
            let text: String = row.get(1)?;
            Ok((time_sec * 1000, text))
        },
    ).map_err(|e| format!("查询失败: {}", e))?;

    for row in rows {
        if let Ok((time, text)) = row {
            // 使用正则解析消费记录
            // 格式：[房间][糊我一脸春泥·梦江南]：[糊我一脸春泥·梦江南]花费[9000金]购买了[于阗玉邦·伤·帽]
            // 解析价格和物品名
            if let Some(caps) = RE_PURCHASE.captures(&text) {
                // 检查是否是当前角色
                let buyer = &caps[1];
                if buyer == role_name {
                    let price_str = &caps[2];
                    let item_name = caps[3].to_string();

                    // 使用 parse_gold_string 解析价格，支持砖、金、银、铜
                    // 返回的是铜，需要转换为金
                    let price_copper = parse_gold_string(price_str);
                    let price_gold = price_copper / 10000;

                    log::debug!("消费记录解析: 物品={}, 原价={}, 铜={}, 金={}", item_name, price_str, price_copper, price_gold);

                    expenses.push(ExpenseEntry {
                        time,
                        price: price_gold,
                        item_name,
                        is_worker_bought: true,
                    });
                }
            }
        }
    }

    log::info!(
        "读取消费记录 | 文件: {} | 角色: {} | 数量: {}",
        db_path,
        role_name,
        expenses.len()
    );

    Ok(expenses)
}

/// 读取所有聊天记录条目
/// 注意：time_range 是毫秒，数据库中的 time 是秒
pub fn read_all_chat_log_entries(
    db_path: &str,
    time_range: &TimeRange,
) -> Result<Vec<ChatLogEntry>, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Err(format!("数据库文件不存在: {}", db_path));
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = time_range.start / 1000;
    // 如果结束时间大于 2050 年，视为"无限"
    let end_sec = if time_range.end > 2524579200000 {
        i64::MAX
    } else {
        time_range.end / 1000
    };

    let query = format!(
        "SELECT time, text, msg FROM {} \
         WHERE time >= ?1 AND time <= ?2 \
         ORDER BY time ASC",
        CHAT_LOG_TABLE
    );

    let mut stmt = conn.prepare(&query)
        .map_err(|e| format!("准备查询失败: {}", e))?;

    let entries = stmt
        .query_map(
            rusqlite::params![start_sec, end_sec],
            |row| {
                // 数据库中的 time 是秒，前端需要毫秒
                let time_sec: i64 = row.get(0)?;
                Ok(ChatLogEntry {
                    time: time_sec * 1000,
                    text: row.get(1)?,
                    msg: row.get(2)?,
                })
            },
        )
        .map_err(|e| format!("查询失败: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("读取记录失败: {}", e))?;

    log::info!(
        "读取全部聊天记录 | 文件: {} | 表: {} | 数量: {}",
        db_path,
        CHAT_LOG_TABLE,
        entries.len()
    );

    Ok(entries)
}

/// 获取聊天记录总数
/// 注意：time_range 是毫秒，数据库中的 time 是秒
pub fn get_chat_log_count(db_path: &str, time_range: &TimeRange) -> Result<usize, String> {
    let path = PathBuf::from(db_path);

    if !path.exists() {
        return Ok(0);
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| format!("打开数据库失败: {}", e))?;

    // 将毫秒转换为秒
    let start_sec = time_range.start / 1000;
    // 如果结束时间大于 2050 年，视为"无限"
    let end_sec = if time_range.end > 2524579200000 {
        i64::MAX
    } else {
        time_range.end / 1000
    };

    let query = format!(
        "SELECT COUNT(*) FROM {} WHERE time >= ?1 AND time <= ?2",
        CHAT_LOG_TABLE
    );

    let count: usize = conn
        .query_row(
            &query,
            rusqlite::params![start_sec, end_sec],
            |row| row.get(0),
        )
        .unwrap_or(0);

    Ok(count)
}

// ============================================================================
// 解析正则表达式
// ============================================================================

lazy_static::lazy_static! {
    /// 团长识别 - 优先级1：拍卖记录
    /// 匹配格式: [房间][团长名]: 将 [物品] ... 记录给了
    static ref RE_LEADER_AUCTION: regex::Regex = regex::Regex::new(
        r#"\[房间\]\[([^\]]+)\].*?将\s*\[([^\]]+)\].*?记录给了"#
    ).unwrap();

    /// 团长识别 - 优先级2：总收入播报
    /// 匹配格式: [房间][团长名]: 拍卖房间目前总收入为
    static ref RE_LEADER_INCOME: regex::Regex = regex::Regex::new(
        r#"\[房间\]\[([^\]]+)\].*?拍卖房间目前总收入为"#
    ).unwrap();

    /// 团长识别 - 优先级3：战斗倒计时
    /// 匹配格式: [团队][团长名]：【团队倒计时】开始倒数！
    static ref RE_LEADER_COUNTDOWN: regex::Regex = regex::Regex::new(
        r#"\[团队\]\[([^\]]+)\].*?【团队倒计时】开始倒数！"#
    ).unwrap();

    /// 团队总收入
    static ref RE_TEAM_INCOME: regex::Regex = regex::Regex::new(
        r#"拍团总入账为：(\d+)金.*?补贴总费用：(\d+)金.*?实际可用分配金额：(\d+)金.*?分配人数：(\d+)"#
    ).unwrap();

    /// 个人工资 - 金币标签
    static ref RE_PERSONAL_INCOME: regex::Regex = regex::Regex::new(
        r#"你获得：.*?(?:<Text_Golds?>|Text_Golds?=\")[^>]*>([^<]+)<"#
    ).unwrap();

    /// 物品购买
    /// 格式：[房间]...：[买家]花费[数字金/金砖/银/铜]购买了[物品]
    /// 注意：价格和单位都在方括号内，如 [5000金]
    static ref RE_PURCHASE: regex::Regex = regex::Regex::new(
        r#"\[房间\].*?\[([^\]]+)\]花费\[(\d+)(?:金砖|金|银|铜)?\]购买了\[([^\]]+)\]"#
    ).unwrap();

    /// 罚款
    static ref RE_FINE: regex::Regex = regex::Regex::new(
        r#"\[房间\].*?\*?.*?向团队里增加了.*?(\d+)金"#
    ).unwrap();

    /// 自动记录开始
    static ref RE_AUTO_START: regex::Regex = regex::Regex::new(
        r#"开始自动记录\s*\[([^\]]+)\]"#
    ).unwrap();

    /// 自动记录结束
    static ref RE_AUTO_END: regex::Regex = regex::Regex::new(
        r#"结束自动记录\s*\[([^\]]+)\]"#
    ).unwrap();
}

/// 识别团长
pub fn identify_leader(entries: &[ChatLogEntry]) -> Option<String> {
    // 遍历聊天记录，找到第一个匹配的消息就返回
    for entry in entries {
        // 优先级1：战斗倒计时
        if let Some(caps) = RE_LEADER_COUNTDOWN.captures(&entry.text) {
            return Some(caps[1].to_string());
        }

        // 优先级2：拍卖记录
        if let Some(caps) = RE_LEADER_AUCTION.captures(&entry.text) {
            return Some(caps[1].to_string());
        }

        // 优先级3：总收入播报
        if let Some(caps) = RE_LEADER_INCOME.captures(&entry.text) {
            return Some(caps[1].to_string());
        }
    }

    None
}

/// 解析团队总收入
pub fn parse_team_income(msg: &str) -> Option<TeamIncomeInfo> {
    if let Some(caps) = RE_TEAM_INCOME.captures(msg) {
        return Some(TeamIncomeInfo {
            total_income: caps[1].parse().unwrap_or(0),
            subsidy_total: caps[2].parse().unwrap_or(0),
            available: caps[3].parse().unwrap_or(0),
            distribute_count: caps[4].parse().unwrap_or(0),
        });
    }
    None
}

/// 团队收入信息
#[derive(Debug, Clone)]
pub struct TeamIncomeInfo {
    pub total_income: i64,
    pub subsidy_total: i64,
    pub available: i64,
    pub distribute_count: i64,
}

/// 解析个人工资
pub fn parse_personal_income(msg: &str) -> i64 {
    let mut total_copper: i64 = 0;

    // 匹配金币
    if let Some(caps) = RE_PERSONAL_INCOME.captures(msg) {
        let gold_str = &caps[1];

        // 解析金额，可能包含 "砖"、"金"、"银"、"铜"
        total_copper = parse_gold_string(gold_str);
    }

    // 转换为金币（小于10金视为搬砖）
    let gold = total_copper / 10000;
    if gold < 10 {
        0
    } else {
        gold
    }
}

/// 解析金额字符串（支持砖、金、银、铜）
fn parse_gold_string(s: &str) -> i64 {
    let mut total: i64 = 0;

    // 匹配砖 (1砖 = 10000金 = 100000000铜)
    let brick_re = regex::Regex::new(r"(\d+)砖").unwrap();
    if let Some(caps) = brick_re.captures(s) {
        let bricks: i64 = caps[1].parse().unwrap_or(0);
        total += bricks * 100000000;
    }

    // 匹配金 (1金 = 10000铜)
    let gold_re = regex::Regex::new(r"(\d+)金").unwrap();
    if let Some(caps) = gold_re.captures(s) {
        let gold: i64 = caps[1].parse().unwrap_or(0);
        total += gold * 10000;
    }

    // 匹配银 (1银 = 100铜)
    let silver_re = regex::Regex::new(r"(\d+)银").unwrap();
    if let Some(caps) = silver_re.captures(s) {
        let silver: i64 = caps[1].parse().unwrap_or(0);
        total += silver * 100;
    }

    // 匹配铜
    let copper_re = regex::Regex::new(r"(\d+)铜").unwrap();
    if let Some(caps) = copper_re.captures(s) {
        let copper: i64 = caps[1].parse().unwrap_or(0);
        total += copper;
    }

    // 如果没有单位，直接作为金处理
    if total == 0 {
        if let Ok(gold) = s.trim().parse::<i64>() {
            total = gold * 10000;
        }
    }

    total
}

/// 购买记录
#[derive(Debug, Clone)]
pub struct PurchaseInfo {
    pub buyer: String,
    pub price: i64,
    pub item: String,
}

/// 解析物品购买
pub fn parse_purchase(msg: &str) -> Option<PurchaseInfo> {
    if let Some(caps) = RE_PURCHASE.captures(msg) {
        return Some(PurchaseInfo {
            buyer: caps[1].to_string(),
            price: caps[2].parse().unwrap_or(0),
            item: caps[3].to_string(),
        });
    }
    None
}

/// 解析罚款
pub fn parse_fine(msg: &str, worker_name: &str) -> i64 {
    if msg.contains(worker_name) {
        if let Some(caps) = RE_FINE.captures(msg) {
            return caps[1].parse().unwrap_or(0);
        }
    }
    0
}

/// 解析自动记录区间
pub fn parse_auto_record_segments(entries: &[ChatLogEntry]) -> Vec<(i64, i64, String)> {
    let mut segments: Vec<(i64, i64, String)> = Vec::new();
    let mut current_start: Option<(i64, String)> = None;

    log::debug!("开始解析自动记录区间，共 {} 条聊天记录", entries.len());

    for entry in entries {
        // 检查开始
        if let Some(caps) = RE_AUTO_START.captures(&entry.msg) {
            current_start = Some((entry.time, caps[1].to_string()));
            log::debug!("找到自动记录开始 | 时间: {} | 副本: {}", entry.time, &caps[1]);
        }

        // 检查结束
        if let Some(_caps) = RE_AUTO_END.captures(&entry.msg) {
            if let Some((start_time, dungeon_name)) = current_start.take() {
                segments.push((start_time, entry.time, dungeon_name.clone()));
                log::debug!("找到自动记录结束 | 开始: {} | 结束: {} | 副本: {}",
                    start_time, entry.time, dungeon_name);
            }
        }
    }

    log::debug!("自动记录区间解析完成，找到 {} 个区间", segments.len());
    for (i, (start, end, name)) in segments.iter().enumerate() {
        log::debug!("  区间{}: {} - {} ({})", i + 1, start, end, name);
    }

    segments
}
// ============================================================================
// 收支解析（新版）
// ============================================================================

use super::{DropFlags, ExpenseDetail, IncomeInfo, SpecialItem};

/// 物品分类关键词
const SCATTERED_KEYWORDS: &[&str] = &["五行石", "五彩石", "上品茶饼", "猫眼石", "玛瑙"];
const IRON_KEYWORDS: &[&str] = &["陨铁"];
const SPECIAL_KEYWORDS: &[&str] = &[
    "玄晶", "化玉", "绮丽", "月华", "天乙", "星源",  // 玄晶
    "马鞍", "马鞭", "缰绳", "护甲", "马具",          // 马具
    "宠物", "跟宠",                                    // 宠物
    "挂件", "腰挂", "背挂", "面挂",                    // 挂件
    "坐骑", "马匹",                                    // 坐骑
    "外观", "成衣", "校服", "盒子",                    // 外观
    "称号",                                            // 称号
    "秘籍", "残页", "断篇",                            // 秘籍
];

/// 掉落关键词映射
const DROP_KEYWORDS: &[(&str, &str)] = &[
    ("xuanjing", "玄晶,化玉,绮丽,月华,天乙,星源"),
    ("maju", "马鞍,马鞭,缰绳,护甲,马具"),
    ("pet", "宠物,跟宠"),
    ("pendant", "挂件,腰挂,背挂,面挂"),
    ("mount", "坐骑,马匹"),
    ("appearance", "外观,成衣,校服,盒子"),
    ("title", "称号"),
    ("secret_book", "秘籍,残页,断篇"),
];

/// 物品类别
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemCategory {
    Special,
    Scattered,
    Iron,
    Other,
}

/// 判断物品类别
pub fn classify_item(item_name: &str) -> ItemCategory {
    // 简单处理：直接检查关键词（不过滤括号）
    if SPECIAL_KEYWORDS.iter().any(|kw| item_name.contains(kw)) {
        return ItemCategory::Special;
    }
    if SCATTERED_KEYWORDS.iter().any(|kw| item_name.contains(kw)) {
        return ItemCategory::Scattered;
    }
    if IRON_KEYWORDS.iter().any(|kw| item_name.contains(kw)) {
        return ItemCategory::Iron;
    }
    ItemCategory::Other
}

/// 生成掉落标记
pub fn generate_drop_flags(items: &[SpecialItem]) -> DropFlags {
    let mut flags = DropFlags::default();
    
    for item in items {
        let name = &item.name;
        
        // 玄晶
        if ["玄晶", "化玉", "绮丽", "月华", "天乙", "星源"].iter().any(|kw| name.contains(kw)) {
            flags.has_xuanjing = true;
        }
        // 马具
        if ["马鞍", "马鞭", "缰绳", "护甲", "马具"].iter().any(|kw| name.contains(kw)) {
            flags.has_maju = true;
        }
        // 宠物
        if ["宠物", "跟宠"].iter().any(|kw| name.contains(kw)) {
            flags.has_pet = true;
        }
        // 挂件
        if ["挂件", "腰挂", "背挂", "面挂"].iter().any(|kw| name.contains(kw)) {
            flags.has_pendant = true;
        }
        // 坐骑
        if ["坐骑", "马匹"].iter().any(|kw| name.contains(kw)) {
            flags.has_mount = true;
        }
        // 外观
        if ["外观", "成衣", "校服", "盒子"].iter().any(|kw| name.contains(kw)) {
            flags.has_appearance = true;
        }
        // 称号
        if name.contains("称号") {
            flags.has_title = true;
        }
        // 秘籍
        if ["秘籍", "残页", "断篇"].iter().any(|kw| name.contains(kw)) {
            flags.has_secret_book = true;
        }
    }
    
    flags
}

/// 解析收支信息
/// 
/// 遍历聊天记录，提取：
/// - 个人工资收入
/// - 购买物品支出（按类别分类）
/// - 特殊物品列表
/// - 掉落标记
pub fn parse_income_info(entries: &[ChatLogEntry], worker_name: &str) -> IncomeInfo {
    let mut income: i64 = 0;
    let mut expense: i64 = 0;
    let mut expense_detail = ExpenseDetail::default();
    let mut special_items: Vec<SpecialItem> = Vec::new();
    
    for entry in entries {
        let content = if !entry.msg.is_empty() {
            &entry.msg
        } else {
            &entry.text
        };
        
        // 匹配收入：你获得：
        let parsed_income = parse_personal_income(content);
        if parsed_income > income {
            income = parsed_income;
        }
        
        // 匹配支出：[房间][当前角色]花费X购买了[物品]
        if content.contains(worker_name) {
            if let Some(caps) = RE_PURCHASE.captures(content) {
                if &caps[1] == worker_name {
                    let price: i64 = caps[2].parse().unwrap_or(0);
                    let item_name = caps[3].to_string();
                    
                    expense += price;
                    
                    // 分类
                    let category = classify_item(&item_name);
                    match category {
                        ItemCategory::Scattered => expense_detail.scattered += price,
                        ItemCategory::Iron => expense_detail.iron += price,
                        ItemCategory::Special => expense_detail.special += price,
                        ItemCategory::Other => expense_detail.other += price,
                    }
                    
                    // 添加到特殊物品列表
                    special_items.push(SpecialItem {
                        name: item_name,
                        buyer: worker_name.to_string(),
                        price,
                        is_worker_bought: true,
                    });
                    
                    log::debug!(
                        "支出: {} 花费 {} 购买 {} [{:?}]",
                        worker_name, price, &caps[3], category
                    );
                }
            }
        }
    }
    
    // 躺拍检测（收入 > 0 且 <= 10金）
    let is_lying_flat = income > 0 && income <= 10;
    if is_lying_flat {
        income = 0;
        log::debug!("检测到躺拍，收入置为 0");
    }
    
    let net_income = income - expense;
    let drop_flags = generate_drop_flags(&special_items);
    
    log::info!(
        "收支解析完成 | 收入: {} | 支出: {} | 净收入: {}{}",
        income, expense, net_income,
        if is_lying_flat { " (躺拍)" } else { "" }
    );
    
    IncomeInfo {
        income,
        expense,
        expense_detail,
        net_income,
        drop_flags,
        special_items,
    }
}

/// 使用 SQL 解析的支出记录构建 IncomeInfo（新版收入计算逻辑）
///
/// 收入计算逻辑：
/// 1. 从收入记录中筛选拍团记录时间之后，且金额 >= 每人底薪的第一条记录
/// 2. 将筛选出的金额作为最终收入
///
/// expenses: 从 SQL 查询获取的消费记录
/// income_entries: 从 SQL 查询获取的收入记录
/// distributions: 从 SQL 查询获取的拍团分配记录
/// worker_name: 角色名称
pub fn build_income_info_from_expense(
    expenses: &[ExpenseEntry],
    income_entries: &[IncomeEntry],
    distributions: &[TeamDistributionInfo],
    worker_name: &str,
) -> IncomeInfo {
    // 打印传入的信息
    log::info!("========== build_income_info_from_expense ==========");
    log::info!(
        "角色: {} | 消费记录数: {} | 收入记录数: {} | 拍团记录数: {}",
        worker_name, expenses.len(), income_entries.len(), distributions.len()
    );

    // 打印消费记录明细
    if expenses.is_empty() {
        log::info!("无消费记录");
    } else {
        log::info!("消费记录明细:");
        for (i, exp) in expenses.iter().enumerate() {
            log::info!(
                "  [{}] 时间: {} | 价格: {}金 | 物品: {}",
                i + 1, exp.time, exp.price, exp.item_name
            );
        }
    }

    // 打印收入记录明细
    if income_entries.is_empty() {
        log::info!("无收入记录");
    } else {
        log::info!("收入记录明细:");
        for (i, inc) in income_entries.iter().enumerate() {
            log::info!(
                "  [{}] 时间: {} | 金额: {}金",
                i + 1, inc.time, inc.amount
            );
        }
    }

    // 打印拍团记录明细
    if distributions.is_empty() {
        log::info!("无拍团记录");
    } else {
        log::info!("拍团记录明细:");
        for (i, dist) in distributions.iter().enumerate() {
            log::info!(
                "  [{}] 时间: {} | 总收入: {}金 | 底薪: {}金 | 人数: {}",
                i + 1, dist.time, dist.total_income, dist.base_salary, dist.distribute_count
            );
        }
    }
    log::info!("====================================================");

    let mut income: i64 = 0;
    let mut expense: i64 = 0;
    let mut expense_detail = ExpenseDetail::default();
    let mut special_items: Vec<SpecialItem> = Vec::new();

    // 1. 使用 SQL 查询的支出记录统计支出
    for exp in expenses {
        expense += exp.price;

        // 分类
        let category = classify_item(&exp.item_name);
        match category {
            ItemCategory::Scattered => expense_detail.scattered += exp.price,
            ItemCategory::Iron => expense_detail.iron += exp.price,
            ItemCategory::Special => expense_detail.special += exp.price,
            ItemCategory::Other => expense_detail.other += exp.price,
        }

        // 添加到特殊物品列表
        special_items.push(SpecialItem {
            name: exp.item_name.clone(),
            buyer: worker_name.to_string(),
            price: exp.price,
            is_worker_bought: exp.is_worker_bought,
        });

        log::debug!(
            "SQL解析支出: {} 花费 {} 购买 {}",
            worker_name, exp.price, exp.item_name
        );
    }

    // 2. 计算收入：从拍团记录时间之后，筛选金额 >= 每人底薪的第一条收入记录
    if !distributions.is_empty() {
        // 取最后一条拍团记录（通常是最终分配）
        if let Some(last_dist) = distributions.last() {
            let dist_time = last_dist.time;
            let base_salary = last_dist.base_salary;

            log::info!(
                "筛选收入记录 | 拍团时间: {} | 底薪: {}金",
                dist_time, base_salary
            );

            // 筛选：时间 > 拍团时间 且 金额 >= 底薪的第一条记录
            for inc in income_entries {
                if inc.time > dist_time && inc.amount >= base_salary {
                    income = inc.amount;
                    log::info!(
                        "找到符合条件的收入记录 | 时间: {} | 金额: {}金 (底薪: {}金)",
                        inc.time, inc.amount, base_salary
                    );
                    break;
                }
            }

            if income == 0 {
                log::info!(
                    "未找到符合条件的收入记录 (时间 > {} 且 金额 >= {}金)",
                    dist_time, base_salary
                );
            }
        }
    } else {
        // 无拍团记录时，取第一条大于10金的收入记录
        for inc in income_entries {
            if inc.amount > 10 {
                income = inc.amount;
                log::info!("无拍团记录，取第一条>10金收入: {}金 (时间: {})", income, inc.time);
                break;
            }
        }
        if income == 0 {
            log::info!("无拍团记录，且无>10金的收入记录");
        }
    }

    let net_income = income - expense;
    let drop_flags = generate_drop_flags(&special_items);

    log::info!(
        "收支解析完成 | 收入: {}金 | 支出: {}金 | 净收入: {}金",
        income, expense, net_income
    );

    IncomeInfo {
        income,
        expense,
        expense_detail,
        net_income,
        drop_flags,
        special_items,
    }
}
