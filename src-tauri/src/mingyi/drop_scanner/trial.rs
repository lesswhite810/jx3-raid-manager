//! 试炼之地 JCL 通关识别。翻牌牌位和牌面没有可靠日志证据，因此不在此推断。

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

pub(super) const TRIAL_RUN_GAP_MS: i64 = 45 * 60 * 1000;

/// 游戏周按北京时间周一 07:00 刷新。Unix 纪元后的第一个刷新点为 UTC 周日 23:00。
pub(super) fn game_week_start_ms(timestamp_ms: i64) -> i64 {
    const HOUR_MS: i64 = 3_600_000;
    const WEEK_MS: i64 = 7 * 24 * HOUR_MS;
    const FIRST_RESET_MS: i64 = (3 * 24 + 23) * HOUR_MS;
    let since_reset = (timestamp_ms.rem_euclid(WEEK_MS) - FIRST_RESET_MS).rem_euclid(WEEK_MS);
    timestamp_ms.saturating_sub(since_reset)
}
static STAGE_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^你通过了第(\d+)层的第([12])关。$").expect("trial stage regex")
});
static FINISH_RE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"^你通过了第(\d+)层的所有关卡。$").expect("trial finish regex")
});

/// 缓存优先保留修改时间较新的试炼 JCL，并清理长时间未访问的条目，避免角色、
/// 周目和旧战斗文件长期累计；淘汰只会让下次扫描重新解析原始 JCL。
const FILE_CACHE_MAX_ENTRIES: usize = 96;
const FILE_CACHE_IDLE: Duration = Duration::from_secs(30 * 60);

/// 内存缓存避免 30 秒轮询反复解码同一个未变化的 GBK JCL。
static FILE_CACHE: LazyLock<Mutex<FileCache>> =
    LazyLock::new(|| Mutex::new(FileCache::new(FILE_CACHE_MAX_ENTRIES)));

pub(super) fn clear_cache() -> Result<usize, String> {
    let mut cache = FILE_CACHE
        .lock()
        .map_err(|e| format!("清空试炼日志缓存失败: {e}"))?;
    let count = cache.len();
    cache.clear();
    Ok(count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrialCandidateStatus {
    Confirmed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialScanCandidate {
    pub dedupe_key: String,
    /// 试炼地图实例 ID。截图归档只以自动记录的来源键为准，这里用于把实时
    /// 截图观察窗口限定到同一轮试炼。
    pub map_id: i64,
    /// 终局系统事件的绝对 Unix 毫秒，不使用文件名时间替代。
    pub completed_at_ms: i64,
    pub layer: Option<i32>,
    /// 第一、二、三关。缺失项以空字符串落库，供待确认界面补齐。
    pub bosses: [String; 3],
    pub status: TrialCandidateStatus,
}

/// 已通过关卡的实时边界。它用于切断前一轮终局的截图/翻牌窗口，绝不启动截图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialStageSignal {
    pub map_id: i64,
    pub stage: usize,
    pub occurred_at_ms: i64,
}

/// 已通过 type=1 UID 核验的本次战斗活动锚点。它只可终止旧终局的截图观察，
/// 不得作为新截图观察的启动条件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrialCaptureAnchor {
    pub map_id: i64,
    pub occurred_at_ms: i64,
}

/// 正在进行中的试炼（已通过若干关但尚未出现终局事件）。它只用于 UI 提示，
/// 绝不作为截图观察或翻牌窗口的输入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrialInProgress {
    pub map_id: i64,
    pub layer: Option<i32>,
    /// 已通过的第几关（1 或 2）。
    pub stage: usize,
    /// 第一、二关已知的首领名，未过的关卡以空字符串占位。
    pub bosses: [String; 2],
    pub last_event_at_ms: i64,
    pub saved_progress: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrialScanResult {
    pub candidates: Vec<TrialScanCandidate>,
    pub stage_signals: Vec<TrialStageSignal>,
    pub capture_anchors: Vec<TrialCaptureAnchor>,
    pub in_progress: Option<TrialInProgress>,
}

impl TrialScanResult {
    /// 仅前两关可作为旧截图观察的停止边界。第三关的 Finish 必须直接交给
    /// `completion_capture_candidates`，不能先退休自己的终局代际。
    pub fn preterminal_capture_boundaries(&self) -> impl Iterator<Item = &TrialStageSignal> {
        self.stage_signals.iter().filter(|signal| signal.stage < 3)
    }

    /// 只有正式“所有关卡”终局才会生成候选；第一、二关、战斗活动和历史窗口都不会
    /// 出现在此处。实时截图仅可用这一集合开启终局后的翻牌观察。
    pub fn completion_capture_candidates(&self) -> &[TrialScanCandidate] {
        &self.candidates
    }
}

impl Deref for TrialScanResult {
    type Target = [TrialScanCandidate];

    fn deref(&self) -> &Self::Target {
        &self.candidates
    }
}

impl IntoIterator for TrialScanResult {
    type Item = TrialScanCandidate;
    type IntoIter = std::vec::IntoIter<TrialScanCandidate>;

    fn into_iter(self) -> Self::IntoIter {
        self.candidates.into_iter()
    }
}

#[derive(Debug, Clone)]
struct Meta {
    map_id: i64,
    /// 已核验 UID 的未结束战斗最近真实活动时间，只用于终止旧的截图观察，
    /// 不使用 mtime 或墙钟时间虚构活动。
    battle_activity_ms: i64,
    has_terminal: bool,
    /// 单个 JCL 若包含多个关卡/终局，文件名无法唯一归属，保持 None 生成待确认记录。
    boss_name: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Kind {
    Stage { layer: i32, stage: usize },
    Finish { layer: Option<i32> },
    Failed,
    WeeklyCredit,
}
#[derive(Debug, Clone)]
struct Event {
    timestamp_ms: i64,
    kind: Kind,
    /// 同一文件内，该关成功时游戏明确保存了进度。
    saved_progress: bool,
}
#[derive(Debug, Clone)]
struct TrialFile {
    meta: Meta,
    events: Vec<Event>,
}
#[derive(Debug, Clone)]
struct CachedFile {
    fingerprint: FileFingerprint,
    expected_uid: String,
    parsed: Option<TrialFile>,
    last_used: Instant,
}

struct FileCache {
    entries: HashMap<PathBuf, CachedFile>,
    max_entries: usize,
}

impl FileCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }

    fn contains_path(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    fn get(
        &mut self,
        path: &Path,
        fingerprint: FileFingerprint,
        expected_uid: &str,
    ) -> Option<Option<TrialFile>> {
        let now = Instant::now();
        self.prune_idle(now);
        let entry = self.entries.get_mut(path)?;
        if entry.fingerprint != fingerprint || entry.expected_uid != expected_uid {
            return None;
        }
        entry.last_used = now;
        Some(entry.parsed.clone())
    }

    fn insert(
        &mut self,
        path: PathBuf,
        fingerprint: FileFingerprint,
        expected_uid: String,
        parsed: Option<TrialFile>,
    ) -> bool {
        let now = Instant::now();
        self.prune_idle(now);
        if self.max_entries == 0 {
            return false;
        }
        if !self.entries.contains_key(&path) && self.entries.len() >= self.max_entries {
            let oldest = self
                .entries
                .iter()
                .min_by(|(left_path, left), (right_path, right)| {
                    cache_priority(left_path, left.fingerprint, right_path, right.fingerprint)
                })
                .map(|(path, entry)| (path.clone(), entry.fingerprint));
            let Some((oldest_path, oldest_fingerprint)) = oldest else {
                return false;
            };
            if cache_priority(&path, fingerprint, &oldest_path, oldest_fingerprint)
                != Ordering::Greater
            {
                return false;
            }
            self.entries.remove(&oldest_path);
        }
        self.entries.insert(
            path,
            CachedFile {
                fingerprint,
                expected_uid,
                parsed,
                last_used: now,
            },
        );
        true
    }

    fn prune_idle(&mut self, now: Instant) {
        self.entries
            .retain(|_, entry| now.duration_since(entry.last_used) <= FILE_CACHE_IDLE);
    }
}

/// 固定容量下，跨账号扫描也只保留全局最新的 JCL。修改时间相同时用路径打破平局，
/// 避免目录枚举顺序不同导致两个账号反复互相挤掉缓存。
fn cache_priority(
    left_path: &Path,
    left_fingerprint: FileFingerprint,
    right_path: &Path,
    right_fingerprint: FileFingerprint,
) -> Ordering {
    left_fingerprint
        .modified
        .cmp(&right_fingerprint.modified)
        .then_with(|| left_path.cmp(right_path))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileFingerprint {
    modified: std::time::SystemTime,
    size: u64,
}
#[derive(Debug, Clone)]
struct Run {
    map_id: i64,
    layer: Option<i32>,
    bosses: [Option<String>; 3],
    last_stage: usize,
    last_ms: i64,
    saved_at: Option<i64>,
    retry_boss: Option<String>,
}

impl Run {
    fn new(map_id: i64, timestamp_ms: i64) -> Self {
        Self {
            map_id,
            layer: None,
            bosses: [None, None, None],
            last_stage: 0,
            last_ms: timestamp_ms,
            saved_at: None,
            retry_boss: None,
        }
    }

    fn can_resume(&self, event: &Event, boss: Option<&str>) -> bool {
        // 跨游戏周绝不续接；本周内的跨时段续打（如周一打老一、周五继续老二）
        // 允许接回链条，无论是否写过「当前进度已保存。」存档——存档只是加分
        // 项，同层逐关递进本身就足以证明是同一条进度链。
        if event.timestamp_ms < self.last_ms
            || !self.runs_in_same_game_week(event.timestamp_ms)
            || self
                .retry_boss
                .as_deref()
                .is_some_and(|expected| Some(expected) != boss)
        {
            return false;
        }
        match event.kind {
            Kind::Stage { layer, stage } => {
                self.layer == Some(layer) && stage == self.last_stage + 1
            }
            Kind::Finish { layer } => {
                self.last_stage == 2 && (layer.is_none() || layer == self.layer)
            }
            Kind::Failed => true,
            Kind::WeeklyCredit => false,
        }
    }

    /// 存档时间与事件是否属于同一游戏周。无存档时退回用上次事件时间判断，
    /// 使未写过存档的跨时段续打同样受"仅本周"约束。
    fn runs_in_same_game_week(&self, event_ms: i64) -> bool {
        let anchor = self.saved_at.unwrap_or(self.last_ms);
        anchor > 0 && game_week_start_ms(anchor) == game_week_start_ms(event_ms)
    }
}

/// 从父扫描器已经筛选过的 JCL 列表中识别试炼通关。
///
/// 父层负责目录枚举、文件名解析、mtime 会话筛选和左边界。这里不再重复目录
/// 遍历或文件名正则解析，`start_ms/end_ms` 只按终局系统事件的实际时间过滤。
/// 每个文件还必须有 type=1 FIGHT_TIME 中精确匹配的 `U<uid>/<instance>`，防止
/// 同目录残留的其他角色日志被采纳。
pub fn scan_trial_records(
    jcl_files: &[super::JclFileInfo],
    account_dir: &Path,
    expected_uid: &str,
    start_ms: i64,
    end_ms: i64,
) -> Result<TrialScanResult, String> {
    if expected_uid.is_empty() || !expected_uid.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(TrialScanResult::default());
    }
    let mut parsed = Vec::new();
    let combat_logs = account_dir.join("userdata").join("combat_logs");
    // 输入按 JCL 时间升序。缓存只收纳本轮最后的近期工作集；更早的上下文日志仍会
    // 参与分组，但不会在容量不足时把近期文件逐个换出，导致每 30 秒全量重读。
    let cache_admissions: HashSet<&str> = jcl_files
        .iter()
        .rev()
        .filter(|jcl| jcl.raid_display_name == "试炼之地")
        .take(FILE_CACHE_MAX_ENTRIES)
        .map(|jcl| jcl.file_name.as_str())
        .collect();
    for jcl in jcl_files
        .iter()
        .filter(|jcl| jcl.raid_display_name == "试炼之地")
    {
        let path = combat_logs.join(&jcl.file_name);
        let cache_admitted = cache_admissions.contains(jcl.file_name.as_str());
        let metadata =
            fs::metadata(&path).map_err(|e| format!("读取试炼日志 {} 失败: {e}", jcl.file_name))?;
        let fingerprint = FileFingerprint {
            modified: metadata
                .modified()
                .map_err(|e| format!("读取试炼日志修改时间失败: {e}"))?,
            size: metadata.len(),
        };
        let meta = Meta {
            map_id: jcl.raid_id,
            battle_activity_ms: 0,
            has_terminal: false,
            boss_name: (!jcl.boss_name.trim().is_empty()).then(|| jcl.boss_name.clone()),
        };
        if let Some(file) = cached_parse(&path, meta, fingerprint, expected_uid, cache_admitted)
            .map_err(|e| format!("解析试炼日志 {} 失败: {e}", jcl.file_name))?
        {
            parsed.push(file);
        }
    }
    let mut result = group(expected_uid, parsed, start_ms, end_ms);
    // 进行中提示只对"当前确实还在打"的进度有意义：最后一次试炼事件距今超过
    // 一个战斗间隔（45 分钟）就视为已中断（周一打完老一后弃坑的情况），不再
    // 显示徽标。历史补扫（JCL 是几天前的）天然落在阈值之外。
    if let Some(progress) = result.in_progress.as_ref() {
        let now_ms = chrono::Utc::now().timestamp_millis();
        if now_ms.saturating_sub(progress.last_event_at_ms) > TRIAL_RUN_GAP_MS {
            result.in_progress = None;
        }
    }
    Ok(result)
}

fn cached_parse(
    path: &Path,
    meta: Meta,
    fingerprint: FileFingerprint,
    uid: &str,
    cache_admitted: bool,
) -> Result<Option<TrialFile>, String> {
    if let Ok(mut cache) = FILE_CACHE.lock() {
        if let Some(parsed) = cache.get(path, fingerprint, uid) {
            return Ok(parsed);
        }
    }
    let parsed = parse_file(path, meta, uid)?;
    if let Ok(mut cache) = FILE_CACHE.lock() {
        // 已缓存的文件即使本轮不在近期准入集，内容变化后也要替换旧解析结果；
        // 从未缓存的旧上下文则不准入，避免挤掉全局最新工作集。
        if cache_admitted || cache.contains_path(path) {
            cache.insert(
                path.to_path_buf(),
                fingerprint,
                uid.to_string(),
                parsed.clone(),
            );
        }
    }
    Ok(parsed)
}

fn parse_file(path: &Path, mut meta: Meta, uid: &str) -> Result<Option<TrialFile>, String> {
    let reader = BufReader::new(fs::File::open(path).map_err(|e| e.to_string())?);
    let mut uid_matched = false;
    let mut battle_activity_ms: Option<i64> = None;
    let mut events = Vec::new();
    let mut seen = HashSet::new();
    let mut save_times = HashSet::new();
    for line in reader.split(b'\n') {
        let line = line.map_err(|e| e.to_string())?;
        let mut fields = line.splitn(6, |byte| *byte == b'\t');
        let _ = fields.next();
        let _ = fields.next();
        let timestamp = fields.next();
        let _ = fields.next();
        let event_type = fields.next();
        let payload = fields.next();
        let (Some(timestamp), Some(event_type), Some(payload)) = (timestamp, event_type, payload)
        else {
            continue;
        };
        let Ok(timestamp_sec) = std::str::from_utf8(timestamp).unwrap_or("").parse::<i64>() else {
            continue;
        };
        let Ok(event_type) = std::str::from_utf8(event_type).unwrap_or("").parse::<u8>() else {
            continue;
        };
        if event_type != 1 && !matches!(event_type, 14 | 15 | 18 | 21) {
            continue;
        }
        let (decoded, _, errors) = encoding_rs::GBK.decode(payload);
        if errors {
            continue;
        }
        let timestamp_ms = timestamp_sec.saturating_mul(1000);
        if event_type == 1 {
            match extract_fight_uid_and_started(&decoded) {
                Some((actual_uid, started))
                    if super::jcl_uuid_matches_uid(&format!("U{actual_uid}/1"), uid) =>
                {
                    uid_matched = true;
                    if started {
                        battle_activity_ms = Some(
                            battle_activity_ms
                                .map(|known| known.max(timestamp_ms))
                                .unwrap_or(timestamp_ms),
                        );
                    }
                }
                Some(_) => return Ok(None),
                None => {}
            }
            continue;
        }
        if event_type == 21 {
            if uid_matched && is_combat_activity(&decoded) {
                battle_activity_ms = Some(
                    battle_activity_ms
                        .map(|known| known.max(timestamp_ms))
                        .unwrap_or(timestamp_ms),
                );
            }
            continue;
        }
        let Some(message) = local_system_message(event_type, &decoded) else {
            continue;
        };
        if message == "当前进度已保存。" {
            save_times.insert(timestamp_ms);
            continue;
        }
        let Some(kind) = parse_message(message) else {
            continue;
        };
        let event = Event {
            timestamp_ms,
            kind,
            saved_progress: false,
        };
        // 一条系统消息会并行写 type=14 / 15 / 18，只保留一次。
        if seen.insert((event.timestamp_ms, event.kind.clone())) {
            events.push(event);
        }
    }
    let stage_slots: HashSet<usize> = events
        .iter()
        .filter_map(|event| match event.kind {
            Kind::Stage { stage, .. } => Some(stage),
            // 同一 JCL 中的终局也会把文件名当作第三关 Boss，因此必须参与歧义检查。
            Kind::Finish { .. } => Some(3),
            Kind::Failed | Kind::WeeklyCredit => None,
        })
        .collect();
    if !uid_matched {
        return Ok(None);
    }
    // 保留经 UID 核验的通关事件，避免漏记。多关卡混在同一 JCL 时，文件名不能可靠
    // 归属任一 Boss，因此不填 Boss，后续记录会是 pending，交给用户补录。
    if stage_slots.len() > 1 {
        meta.boss_name = None;
    }
    if stage_slots.contains(&3) {
        // 有正式终局时以它为准，周任务通知有时与终局同秒写入。
        events.retain(|event| event.kind != Kind::WeeklyCredit);
    }
    meta.has_terminal = stage_slots.contains(&3);
    meta.battle_activity_ms = (!meta.has_terminal)
        .then(|| battle_activity_ms.unwrap_or(0))
        .unwrap_or(0);
    for event in &mut events {
        event.saved_progress =
            matches!(event.kind, Kind::Stage { .. }) && save_times.contains(&event.timestamp_ms);
    }
    events.sort_by_key(|event| event.timestamp_ms);
    Ok(Some(TrialFile { meta, events }))
}

pub(super) fn extract_fight_uid_and_started(payload: &str) -> Option<(&str, bool)> {
    let inner = payload
        .trim()
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .unwrap_or(payload);
    let started = inner.split(',').next()?.trim() == "true";
    let uuid = inner.split(',').nth(1)?.trim().trim_matches('"');
    let suffix = uuid
        .rsplit_once("::U")
        .map(|(_, value)| value)
        .or_else(|| uuid.strip_prefix('U'));
    let (actual_uid, instance) = suffix?.split_once('/')?;
    (!actual_uid.is_empty()
        && actual_uid.bytes().all(|b| b.is_ascii_digit())
        && !instance.is_empty()
        && instance.bytes().all(|b| b.is_ascii_digit()))
    .then_some((actual_uid, started))
}

pub(super) fn is_combat_activity(payload: &str) -> bool {
    payload.starts_with('{') && (payload.contains("[13]") || payload.contains("[14]"))
}

/// 只接受本地系统频道，排除世界/队伍聊天中转发的同文案。
pub(super) fn local_system_message(event_type: u8, payload: &str) -> Option<&str> {
    let payload = payload.trim();
    let message = if event_type == 14 && payload.ends_with(",0,9,\"\"}") {
        payload
            .strip_prefix("{\"")
            .and_then(|value| value.split_once("\","))
            .map(|(message, _)| message)
    } else if event_type == 15 {
        payload
            .strip_prefix("{\"MSG_NOTICE_GREEN\",\"")
            .and_then(|value| value.strip_suffix("\"}"))
    } else if event_type == 18 {
        payload
            .strip_prefix("{\"")
            .and_then(|value| value.strip_suffix("\",\"MSG_SYS\"}"))
    } else {
        None
    };
    message.map(|value| value.trim_end_matches("\\n"))
}

fn parse_message(message: &str) -> Option<Kind> {
    if let Some(c) = STAGE_RE.captures(message) {
        let layer = c[1].parse::<i32>().ok()?;
        let stage = c[2].parse::<usize>().ok()?;
        return (layer > 0).then_some(Kind::Stage { layer, stage });
    }
    if let Some(c) = FINISH_RE.captures(message) {
        return c[1]
            .parse::<i32>()
            .ok()
            .filter(|layer| *layer > 0)
            .map(|layer| Kind::Finish { layer: Some(layer) });
    }
    if message == "你败了。少侠请重新来过。" {
        return Some(Kind::Failed);
    }
    if matches!(
        message,
        "周任务活跃[试炼之地]：1/3" | "周任务活跃[试炼之地]：2/3" | "周任务活跃[试炼之地]：3/3"
    ) {
        return Some(Kind::WeeklyCredit);
    }
    (message == "恭喜你通过了所有的关卡。").then_some(Kind::Finish { layer: None })
}

/// 实时尾读与完整扫描共用同一套严格的本地系统文案判定。实时路径只需要知道
/// 是否为终局或一个停止旧截图的边界，不能自行放宽语法或把聊天转发当系统消息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LiveMessageKind {
    Stage,
    Boundary,
    Terminal,
}

pub(super) fn live_system_message_kind(event_type: u8, payload: &str) -> Option<LiveMessageKind> {
    let message = local_system_message(event_type, payload)?;
    match parse_message(message)? {
        Kind::Stage { .. } => Some(LiveMessageKind::Stage),
        Kind::Failed | Kind::WeeklyCredit => Some(LiveMessageKind::Boundary),
        Kind::Finish { .. } => Some(LiveMessageKind::Terminal),
    }
}

fn group(uid: &str, files: Vec<TrialFile>, start_ms: i64, end_ms: i64) -> TrialScanResult {
    let mut result = TrialScanResult::default();
    let mut active: Option<Run> = None;
    for file in files {
        if file.meta.battle_activity_ms > 0
            && (start_ms <= 0 || file.meta.battle_activity_ms >= start_ms)
            && (end_ms <= 0 || file.meta.battle_activity_ms < end_ms)
        {
            result.capture_anchors.push(TrialCaptureAnchor {
                map_id: file.meta.map_id,
                occurred_at_ms: file.meta.battle_activity_ms,
            });
        }
        if active
            .as_ref()
            .is_some_and(|run| run.map_id != file.meta.map_id)
        {
            active = None;
        }
        for event in file.events {
            let nearby = active.as_ref().is_some_and(|run| {
                event.timestamp_ms >= run.last_ms
                    && event.timestamp_ms - run.last_ms <= TRIAL_RUN_GAP_MS
            });
            let compatible = nearby
                || active
                    .as_ref()
                    .is_some_and(|run| run.can_resume(&event, file.meta.boss_name.as_deref()));
            match event.kind {
                Kind::Stage { layer, stage } => {
                    if (start_ms <= 0 || event.timestamp_ms >= start_ms)
                        && (end_ms <= 0 || event.timestamp_ms < end_ms)
                    {
                        result.stage_signals.push(TrialStageSignal {
                            map_id: file.meta.map_id,
                            stage,
                            occurred_at_ms: event.timestamp_ms,
                        });
                    }
                    let new_run = active.as_ref().is_some_and(|run| {
                        !compatible
                            || run.layer.is_some_and(|value| value != layer)
                            // 第二关已通过后又有前置关卡通过，上一轮的终局可能缺失。
                            // 新一轮不能拿上一轮的第一关补成完整记录。
                            || stage <= run.last_stage
                    });
                    if active.is_none() || new_run {
                        active = Some(Run::new(file.meta.map_id, event.timestamp_ms));
                    }
                    let run = active.as_mut().expect("trial run initialized");
                    run.layer = Some(layer);
                    run.last_stage = stage;
                    run.bosses[stage - 1] = file.meta.boss_name.clone();
                    run.last_ms = event.timestamp_ms;
                    run.saved_at = event.saved_progress.then_some(event.timestamp_ms);
                    run.retry_boss = None;
                }
                Kind::Finish { layer } => {
                    if !compatible {
                        active = Some(Run::new(file.meta.map_id, event.timestamp_ms));
                    }
                    let run = active.as_mut().expect("trial run initialized");
                    if let Some(layer) = layer {
                        if run.layer.is_some_and(|value| value != layer) {
                            *run = Run::new(file.meta.map_id, event.timestamp_ms);
                        }
                        run.layer = Some(layer);
                    }
                    run.bosses[2] = file.meta.boss_name.clone();
                    run.last_ms = event.timestamp_ms;
                    if (start_ms <= 0 || event.timestamp_ms >= start_ms)
                        && (end_ms <= 0 || event.timestamp_ms < end_ms)
                    {
                        result
                            .candidates
                            .push(candidate(uid, run, event.timestamp_ms));
                        result.stage_signals.push(TrialStageSignal {
                            map_id: file.meta.map_id,
                            stage: 3,
                            occurred_at_ms: event.timestamp_ms,
                        });
                    }
                    active = None;
                }
                Kind::Failed => {
                    if compatible {
                        if let Some(run) = active.as_mut() {
                            if run.retry_boss.is_some() && run.retry_boss != file.meta.boss_name {
                                run.saved_at = None;
                            }
                            run.retry_boss.clone_from(&file.meta.boss_name);
                            // 跨时段失败不能把旧存档续成普通的 45 分钟连续战斗。
                            if nearby {
                                run.last_ms = event.timestamp_ms;
                            }
                        }
                    } else {
                        active = None;
                    }
                }
                Kind::WeeklyCredit => active = None,
            }
        }
    }
    // 循环结束后仍存活且已通过至少一关的 Run 即为进行中的试炼。失败重试中
    // （retry_boss 非空）也视为进行中：玩家停在同层等待重新挑战。
    if let Some(run) = active.as_ref().filter(|run| run.last_stage > 0) {
        result.in_progress = Some(TrialInProgress {
            map_id: run.map_id,
            layer: run.layer,
            stage: run.last_stage,
            bosses: [
                run.bosses[0].clone().unwrap_or_default(),
                run.bosses[1].clone().unwrap_or_default(),
            ],
            last_event_at_ms: run.last_ms,
            saved_progress: run.saved_at.is_some(),
        });
    }
    result
}

fn candidate(uid: &str, run: &Run, completed_at_ms: i64) -> TrialScanCandidate {
    let status = if run.layer.is_some()
        && run
            .bosses
            .iter()
            .all(|boss| boss.as_deref().is_some_and(|name| !name.trim().is_empty()))
    {
        TrialCandidateStatus::Confirmed
    } else {
        TrialCandidateStatus::Pending
    };
    TrialScanCandidate {
        dedupe_key: format!("trial:{uid}:{}:{completed_at_ms}", run.map_id),
        map_id: run.map_id,
        completed_at_ms,
        layer: run.layer,
        bosses: run.bosses.clone().map(|boss| boss.unwrap_or_default()),
        status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(map: i64, boss: &str, events: Vec<Event>) -> TrialFile {
        let has_terminal = events
            .iter()
            .any(|event| matches!(event.kind, Kind::Finish { .. }));
        TrialFile {
            meta: Meta {
                map_id: map,
                battle_activity_ms: (!has_terminal)
                    .then(|| events.first().map(|event| event.timestamp_ms).unwrap_or(0))
                    .unwrap_or(0),
                has_terminal,
                boss_name: Some(boss.into()),
            },
            events,
        }
    }
    fn event(time: i64, kind: Kind) -> Event {
        Event {
            timestamp_ms: time,
            kind,
            saved_progress: false,
        }
    }

    fn saved_stage(time: i64, stage: usize) -> Event {
        Event {
            saved_progress: true,
            ..event(time, Kind::Stage { layer: 6, stage })
        }
    }

    fn fingerprint(seconds: u64, size: u64) -> FileFingerprint {
        FileFingerprint {
            modified: std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(seconds),
            size,
        }
    }

    fn cached_file(timestamp_ms: i64) -> Option<TrialFile> {
        Some(file(
            144,
            "寅",
            vec![event(timestamp_ms, Kind::Stage { layer: 6, stage: 1 })],
        ))
    }

    #[test]
    fn file_cache_reuses_only_an_exact_file_and_role_identity() {
        let mut cache = FileCache::new(2);
        let path = PathBuf::from("cache-hit.jcl");
        let original = cached_file(1_000);
        let first_fingerprint = fingerprint(1, 100);
        cache.insert(path.clone(), first_fingerprint, "123456".into(), original);

        let hit = cache
            .get(&path, first_fingerprint, "123456")
            .flatten()
            .expect("exact file and UID should hit the cache");
        assert_eq!(hit.events[0].timestamp_ms, 1_000);
        assert!(
            cache.get(&path, fingerprint(2, 100), "123456").is_none(),
            "same path with a changed modified time must be reparsed"
        );
        assert!(
            cache.get(&path, first_fingerprint, "654321").is_none(),
            "the same JCL cannot be reused for another role"
        );
        assert!(cache.insert(
            path.clone(),
            fingerprint(2, 100),
            "123456".into(),
            cached_file(2_000),
        ));
        let refreshed = cache
            .get(&path, fingerprint(2, 100), "123456")
            .flatten()
            .expect("changed file should replace its old cached parse");
        assert_eq!(refreshed.events[0].timestamp_ms, 2_000);
    }

    #[test]
    fn file_cache_keeps_newer_entries_and_expired_entries_can_be_reparsed() {
        let mut cache = FileCache::new(2);
        let first = PathBuf::from("cache-first.jcl");
        let second = PathBuf::from("cache-second.jcl");
        let third = PathBuf::from("cache-third.jcl");
        let uid = "123456";
        let first_fingerprint = fingerprint(1, 100);
        let second_fingerprint = fingerprint(2, 200);
        let third_fingerprint = fingerprint(3, 300);

        cache.insert(
            first.clone(),
            first_fingerprint,
            uid.into(),
            cached_file(1_000),
        );
        cache.insert(
            second.clone(),
            second_fingerprint,
            uid.into(),
            cached_file(2_000),
        );
        assert!(cache.insert(
            third.clone(),
            third_fingerprint,
            uid.into(),
            cached_file(3_000),
        ));
        assert_eq!(cache.len(), 2);
        assert!(cache.get(&first, first_fingerprint, uid).is_none());
        assert!(cache.get(&second, second_fingerprint, uid).is_some());
        assert!(cache.get(&third, third_fingerprint, uid).is_some());

        // A lower-priority historical file cannot evict newer cached work.
        assert!(!cache.insert(
            first.clone(),
            first_fingerprint,
            uid.into(),
            cached_file(1_000),
        ));
        assert!(cache.get(&first, first_fingerprint, uid).is_none());

        // Once idle entries expire, rereading the same source produces the same evidence.
        for entry in cache.entries.values_mut() {
            entry.last_used = Instant::now() - FILE_CACHE_IDLE - Duration::from_secs(1);
        }
        assert!(cache.insert(
            first.clone(),
            first_fingerprint,
            uid.into(),
            cached_file(1_000),
        ));
        let reparsed = cache
            .get(&first, first_fingerprint, uid)
            .flatten()
            .expect("reparsed entry should be available again");
        assert_eq!(reparsed.events[0].timestamp_ms, 1_000);

        cache
            .entries
            .get_mut(&first)
            .expect("first cache entry")
            .last_used = Instant::now() - FILE_CACHE_IDLE - Duration::from_secs(1);
        assert!(cache.get(&first, first_fingerprint, uid).is_none());
        assert!(
            !cache.entries.contains_key(&first),
            "idle entry must not remain resident forever"
        );
    }

    #[test]
    fn cache_admission_keeps_recent_working_set_hits_when_scan_exceeds_capacity() {
        let mut cache = FileCache::new(2);
        let paths = [
            PathBuf::from("old-context.jcl"),
            PathBuf::from("recent-second.jcl"),
            PathBuf::from("recent-third.jcl"),
        ];
        let fingerprints = [
            fingerprint(1, 100),
            fingerprint(2, 200),
            fingerprint(3, 300),
        ];
        let uid = "123456";
        let admissions: HashSet<&PathBuf> = paths.iter().rev().take(cache.max_entries).collect();
        let mut first_parsed = 0;
        for (index, (path, fingerprint)) in
            paths.iter().zip(fingerprints.iter().copied()).enumerate()
        {
            let admitted = admissions.contains(path);
            if cache.get(path, fingerprint, uid).is_none() {
                first_parsed += 1;
                if admitted {
                    cache.insert(
                        path.clone(),
                        fingerprint,
                        uid.into(),
                        cached_file(index as i64),
                    );
                }
            }
        }
        assert_eq!(first_parsed, 3);
        assert_eq!(cache.len(), 2);
        assert!(!cache.entries.contains_key(&paths[0]));
        assert!(cache.entries.contains_key(&paths[1]));
        assert!(cache.entries.contains_key(&paths[2]));

        let mut second_parsed = 0;
        let mut second_hits = 0;
        for (index, (path, fingerprint)) in
            paths.iter().zip(fingerprints.iter().copied()).enumerate()
        {
            let admitted = admissions.contains(path);
            if cache.get(path, fingerprint, uid).is_some() {
                second_hits += 1;
            } else {
                second_parsed += 1;
                if admitted {
                    cache.insert(
                        path.clone(),
                        fingerprint,
                        uid.into(),
                        cached_file(index as i64),
                    );
                }
            }
        }
        assert_eq!(second_parsed, 1, "only non-admitted context is reread");
        assert_eq!(second_hits, 2, "recent working set must remain cache hits");
        assert_eq!(cache.len(), 2);
        assert!(cache.entries.contains_key(&paths[1]));
        assert!(cache.entries.contains_key(&paths[2]));
    }

    #[test]
    fn global_newest_priority_prevents_alternating_accounts_from_thrashing_cache() {
        let mut cache = FileCache::new(3);
        let account_a = [
            (PathBuf::from("account-a/old.jcl"), fingerprint(100, 100)),
            (PathBuf::from("account-a/recent.jcl"), fingerprint(200, 200)),
        ];
        let account_b = [
            (PathBuf::from("account-b/recent.jcl"), fingerprint(300, 300)),
            (PathBuf::from("account-b/newest.jcl"), fingerprint(400, 400)),
        ];
        let uid = "123456";

        for (index, (path, fingerprint)) in account_a.iter().chain(account_b.iter()).enumerate() {
            assert!(cache.insert(
                path.clone(),
                *fingerprint,
                uid.into(),
                cached_file(index as i64),
            ));
        }
        assert!(!cache.entries.contains_key(&account_a[0].0));
        assert!(cache.entries.contains_key(&account_a[1].0));
        assert!(cache.entries.contains_key(&account_b[0].0));
        assert!(cache.entries.contains_key(&account_b[1].0));

        let mut misses = 0;
        let mut hits = 0;
        for (index, (path, fingerprint)) in account_a.iter().chain(account_b.iter()).enumerate() {
            if cache.get(path, *fingerprint, uid).is_some() {
                hits += 1;
            } else {
                misses += 1;
                assert!(
                    !cache.insert(
                        path.clone(),
                        *fingerprint,
                        uid.into(),
                        cached_file(index as i64),
                    ),
                    "older file may be reread but must not evict the global recent set"
                );
            }
        }
        assert_eq!(misses, 1);
        assert_eq!(hits, 3);
        assert_eq!(cache.len(), 3);
        assert!(cache.entries.contains_key(&account_a[1].0));
        assert!(cache.entries.contains_key(&account_b[0].0));
        assert!(cache.entries.contains_key(&account_b[1].0));
    }

    #[test]
    fn saved_checkpoint_can_resume_after_a_long_break_at_either_stage() {
        let morning = 1_793_638_576_000;
        let evening = 1_793_718_344_000;
        let second = Kind::Stage { layer: 6, stage: 2 };
        let finish = Kind::Finish { layer: None };
        for (first, next_stage, completion) in [
            (
                saved_stage(morning, 1),
                event(evening, second.clone()),
                evening + 315_000,
            ),
            (
                event(morning, Kind::Stage { layer: 6, stage: 1 }),
                saved_stage(morning + 300_000, 2),
                evening,
            ),
        ] {
            let files = vec![
                file(146, "寅", vec![first]),
                file(146, "濯尘", vec![event(morning + 24_000, Kind::Failed)]),
                file(146, "濯尘", vec![next_stage]),
                file(146, "叶冬辰", vec![event(completion, finish.clone())]),
            ];
            let candidates = group("123456", files, evening, 0);
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].status, TrialCandidateStatus::Confirmed);
            assert_eq!(candidates[0].bosses, ["寅", "濯尘", "叶冬辰"]);
            assert_eq!(candidates[0].completed_at_ms, completion);
        }
    }

    #[test]
    fn long_break_resumes_within_the_game_week_even_without_a_save() {
        // 放宽后的续接规则：只要同游戏周、同层逐关递进，跨时段（甚至没有写过
        // 存档）也能接回链条，bosses 与状态与有存档时一致。
        let t = 1_793_638_576_000;
        let later = t + 22 * 3_600_000;
        let mut first = saved_stage(t, 1);
        first.saved_progress = false;
        let files = vec![
            file(146, "寅", vec![first]),
            file(146, "濯尘", vec![event(later, Kind::Stage { layer: 6, stage: 2 })]),
            file(146, "叶冬辰", vec![event(later + 1000, Kind::Finish { layer: None })]),
        ];
        let candidates = group("123456", files, 0, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, TrialCandidateStatus::Confirmed);
        assert_eq!(candidates[0].bosses, ["寅", "濯尘", "叶冬辰"]);
    }

    #[test]
    fn long_break_requires_a_compatible_unconsumed_save_from_the_same_game_week() {
        let t = 1_793_638_576_000;
        let later = t + 22 * 3_600_000;
        let week_start = 1_793_574_000_000; // 2026-11-02 07:00 UTC+8
        let next_reset = week_start + 7 * 24 * 3_600_000;
        assert_eq!(game_week_start_ms(t), week_start);
        assert_eq!(game_week_start_ms(next_reset - 1), week_start);
        assert_eq!(game_week_start_ms(next_reset), next_reset);
        for (reason, interruption, time, layer) in [
            ("不同层数", None, later, 7),
            ("跨周", None, next_reset, 6),
            (
                "重试了另一位首领",
                Some(file(146, "方鹤影", vec![event(t + 1000, Kind::Failed)])),
                later,
                6,
            ),
            (
                "已在另一张试炼地图参战",
                Some(file(147, "濯尘", vec![])),
                later,
                6,
            ),
            (
                "上一轮已有周任务结算",
                Some(file(
                    146,
                    "叶冬辰",
                    vec![event(t + 1000, Kind::WeeklyCredit)],
                )),
                later,
                6,
            ),
        ] {
            let first = saved_stage(t, 1);
            let mut files = vec![file(146, "寅", vec![first])];
            files.extend(interruption);
            files.push(file(
                146,
                "濯尘",
                vec![event(time, Kind::Stage { layer, stage: 2 })],
            ));
            files.push(file(
                146,
                "叶冬辰",
                vec![event(time + 1000, Kind::Finish { layer: None })],
            ));
            let candidates = group("123456", files, 0, 0);
            assert_eq!(candidates.len(), 1, "{reason}");
            assert_eq!(
                candidates[0].status,
                TrialCandidateStatus::Pending,
                "{reason}"
            );
            assert_eq!(candidates[0].bosses, ["", "濯尘", "叶冬辰"], "{reason}");
        }
    }

    #[test]
    fn groups_retry_and_generic_finish_without_duplicate_channels() {
        let trials = vec![
            file(
                144,
                "寅",
                vec![event(1_000, Kind::Stage { layer: 6, stage: 1 })],
            ),
            file(144, "藤原樱奈", vec![event(1_100, Kind::Failed)]),
            file(
                144,
                "方鹤影",
                vec![event(1_200, Kind::Stage { layer: 6, stage: 2 })],
            ),
            file(
                144,
                "藤原樱奈",
                vec![event(1_300, Kind::Finish { layer: None })],
            ),
        ];
        let candidates = group("123456", trials, 0, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, TrialCandidateStatus::Confirmed);
        assert_eq!(
            candidates[0].bosses,
            [
                String::from("寅"),
                String::from("方鹤影"),
                String::from("藤原樱奈"),
            ]
        );
        assert_eq!(candidates[0].completed_at_ms, 1_300);
    }

    #[test]
    fn only_terminal_candidates_are_eligible_for_capture() {
        let result = group(
            "123456",
            vec![
                file(
                    144,
                    "寅",
                    vec![event(1_000, Kind::Stage { layer: 6, stage: 1 })],
                ),
                file(
                    144,
                    "苏髓",
                    vec![event(2_000, Kind::Stage { layer: 6, stage: 2 })],
                ),
                file(
                    144,
                    "藤原樱奈",
                    vec![event(3_000, Kind::Finish { layer: None })],
                ),
            ],
            2_000,
            3_001,
        );
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(
            result.capture_anchors,
            vec![TrialCaptureAnchor {
                map_id: 144,
                occurred_at_ms: 2_000,
            }],
            "第二关战斗只能停止旧观察"
        );
        assert_eq!(
            result
                .completion_capture_candidates()
                .iter()
                .map(|candidate| candidate.completed_at_ms)
                .collect::<Vec<_>>(),
            vec![3_000],
            "第一、二关时间边界不能成为截图候选"
        );
        assert_eq!(
            result
                .preterminal_capture_boundaries()
                .cloned()
                .collect::<Vec<_>>(),
            vec![TrialStageSignal {
                map_id: 144,
                stage: 2,
                occurred_at_ms: 2_000,
            }],
            "第三关终局不能先作为停止边界退休自己"
        );
        assert_eq!(
            result.stage_signals,
            vec![
                TrialStageSignal {
                    map_id: 144,
                    stage: 2,
                    occurred_at_ms: 2_000,
                },
                TrialStageSignal {
                    map_id: 144,
                    stage: 3,
                    occurred_at_ms: 3_000,
                },
            ]
        );
    }

    #[test]
    fn saved_day_one_stages_never_trigger_capture_before_day_two_terminal() {
        const DAY_MS: i64 = 24 * 60 * 60 * 1_000;
        let files = vec![
            file(
                144,
                "寅",
                vec![saved_stage(1_000, 1), event(1_100, Kind::Failed)],
            ),
            file(144, "濯尘", vec![saved_stage(2_000, 2)]),
            file(
                144,
                "叶冬辰",
                vec![event(DAY_MS + 3_000, Kind::Finish { layer: None })],
            ),
        ];

        let day_one = group("123456", files.clone(), 0, DAY_MS);
        let repeated_day_one = group("123456", files.clone(), 0, DAY_MS);
        assert!(day_one.completion_capture_candidates().is_empty());
        assert!(repeated_day_one.completion_capture_candidates().is_empty());
        assert_eq!(
            day_one
                .preterminal_capture_boundaries()
                .map(|signal| signal.stage)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "保存进度仍应保留为旧观察的停止边界"
        );

        let terminal_day = group("123456", files.clone(), DAY_MS, 2 * DAY_MS);
        assert_eq!(terminal_day.completion_capture_candidates().len(), 1);
        assert_eq!(
            terminal_day.completion_capture_candidates()[0].completed_at_ms,
            DAY_MS + 3_000
        );

        let historical_day = group("123456", files, 2 * DAY_MS, 0);
        assert!(historical_day.completion_capture_candidates().is_empty());
    }

    #[test]
    fn final_without_prior_stages_is_pending() {
        let candidates = group(
            "123456",
            vec![file(
                145,
                "叶冬辰",
                vec![event(1_000, Kind::Finish { layer: Some(9) })],
            )],
            0,
            0,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, TrialCandidateStatus::Pending);
        assert_eq!(
            candidates[0].bosses,
            [String::new(), String::new(), String::from("叶冬辰")]
        );
    }

    #[test]
    fn repeated_second_stage_cannot_borrow_first_boss_from_previous_run() {
        let records = group(
            "123456",
            vec![
                file(
                    144,
                    "寅",
                    vec![event(1_000, Kind::Stage { layer: 6, stage: 1 })],
                ),
                file(
                    144,
                    "苏髓",
                    vec![event(2_000, Kind::Stage { layer: 6, stage: 2 })],
                ),
                file(
                    144,
                    "方鹤影",
                    vec![event(3_000, Kind::Stage { layer: 6, stage: 2 })],
                ),
                file(
                    144,
                    "叶冬辰",
                    vec![event(4_000, Kind::Finish { layer: None })],
                ),
            ],
            0,
            0,
        );
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].status, TrialCandidateStatus::Pending);
        assert_eq!(records[0].bosses, ["", "方鹤影", "叶冬辰"]);
    }

    #[test]
    fn exact_uid_and_local_channel_are_required() {
        assert_eq!(
            extract_fight_uid_and_started("{true,\"服务器::U123456/1\",0}"),
            Some(("123456", true))
        );
        assert_ne!(
            extract_fight_uid_and_started("{true,\"服务器::U1234567/1\",0}"),
            Some(("123456", true))
        );
        assert_eq!(
            local_system_message(18, "{\"你通过了第6层的第1关。\\n\",\"MSG_SYS\"}"),
            Some("你通过了第6层的第1关。")
        );
        assert_eq!(
            local_system_message(15, "{\"MSG_NOTICE_GREEN\",\"恭喜你通过了所有的关卡。\"}"),
            Some("恭喜你通过了所有的关卡。")
        );
        assert_eq!(
            local_system_message(18, "{\"你通过了第6层的第1关。\",\"MSG_WORLD\"}"),
            None
        );
    }

    #[test]
    fn parses_real_shape_gbk_notice_rows_and_deduplicates_channels() {
        let content = concat!(
            "1\t1\t100\t0\t1\t{true,\"服务器::U123456/1\",0}\n",
            "1\t1\t120\t0\t15\t{\"MSG_NOTICE_GREEN\",\"恭喜你通过了所有的关卡。\"}\n",
            "1\t1\t120\t0\t18\t{\"恭喜你通过了所有的关卡。\\n\",\"MSG_SYS\"}\n"
        );
        let (gbk, _, _) = encoding_rs::GBK.encode(content);
        let path = std::env::temp_dir().join(format!("trial-gbk-{}.jcl", uuid::Uuid::new_v4()));
        fs::write(&path, gbk.as_ref()).expect("GBK fixture should be written");
        let parsed = parse_file(
            &path,
            Meta {
                map_id: 144,
                battle_activity_ms: 0,
                has_terminal: false,
                boss_name: Some("藤原樱奈".into()),
            },
            "123456",
        )
        .expect("fixture should parse")
        .expect("fixture should match uid");
        let _ = fs::remove_file(path);
        assert_eq!(parsed.events.len(), 1);
        assert_eq!(parsed.events[0].kind, Kind::Finish { layer: None });
    }

    #[test]
    fn combat_activity_cannot_create_a_terminal_capture_candidate() {
        let content = concat!(
            "1\t1\t200\t0\t1\t{true,\"服务器::U123456/1\",0}\n",
            "1\t1\t250\t0\t21\t{1,2,0,0,0,0,0,0,{[13]=5,[14]=0}}\n"
        );
        let (gbk, _, _) = encoding_rs::GBK.encode(content);
        let path =
            std::env::temp_dir().join(format!("trial-activity-{}.jcl", uuid::Uuid::new_v4()));
        fs::write(&path, gbk.as_ref()).expect("GBK fixture should be written");
        let current_third = parse_file(
            &path,
            Meta {
                map_id: 144,
                battle_activity_ms: 0,
                has_terminal: false,
                boss_name: Some("叶冬辰".into()),
            },
            "123456",
        )
        .expect("fixture should parse")
        .expect("fixture should match uid");
        let _ = fs::remove_file(path);
        assert!(current_third.events.is_empty());

        let old_second = file(
            144,
            "濯尘",
            vec![event(100_000, Kind::Stage { layer: 6, stage: 2 })],
        );
        let active = group("123456", vec![old_second, current_third], 200_000, 0);
        assert!(active.candidates.is_empty());
        assert!(active.completion_capture_candidates().is_empty());
        assert_eq!(
            active.capture_anchors,
            vec![TrialCaptureAnchor {
                map_id: 144,
                occurred_at_ms: 250_000,
            }],
            "战斗活动只作为停止旧观察的边界"
        );

        let historic_terminal = file(
            144,
            "藤原樱奈",
            vec![event(190_000, Kind::Finish { layer: None })],
        );
        let historic = group("123456", vec![historic_terminal], 200_000, 0);
        assert!(historic.completion_capture_candidates().is_empty());
        assert!(historic.stage_signals.is_empty());
    }

    #[test]
    fn ambiguous_multi_stage_jcl_keeps_terminal_evidence_as_pending() {
        let content = concat!(
            "1\t1\t100\t0\t1\t{true,\"服务器::U123456/1\",0}\n",
            "1\t1\t120\t0\t18\t{\"你通过了第6层的第1关。\",\"MSG_SYS\"}\n",
            "1\t1\t180\t0\t18\t{\"你通过了第6层的所有关卡。\",\"MSG_SYS\"}\n"
        );
        let (gbk, _, _) = encoding_rs::GBK.encode(content);
        let path =
            std::env::temp_dir().join(format!("trial-ambiguous-{}.jcl", uuid::Uuid::new_v4()));
        fs::write(&path, gbk.as_ref()).expect("GBK fixture should be written");
        let parsed = parse_file(
            &path,
            Meta {
                map_id: 144,
                battle_activity_ms: 0,
                has_terminal: false,
                boss_name: Some("藤原樱奈".into()),
            },
            "123456",
        )
        .expect("fixture should parse")
        .expect("terminal evidence should be retained");
        let _ = fs::remove_file(path);
        assert_eq!(parsed.meta.boss_name, None);
        let candidates = group("123456", vec![parsed], 0, 0);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].status, TrialCandidateStatus::Pending);
        assert_eq!(
            candidates[0].bosses,
            [String::new(), String::new(), String::new()]
        );
    }

    /// 本地回放辅助。仅在显式提供脱敏后的账号目录和 UID 时运行，不读取或写入数据库。
    #[test]
    #[ignore = "requires JX3_TRIAL_ACCOUNT_DIR and JX3_TRIAL_UID"]
    fn replays_local_trial_logs_when_configured() {
        let account_dir = std::env::var("JX3_TRIAL_ACCOUNT_DIR")
            .expect("JX3_TRIAL_ACCOUNT_DIR should be configured for local replay");
        let uid = std::env::var("JX3_TRIAL_UID")
            .expect("JX3_TRIAL_UID should be configured for local replay");
        let start_ms = 1_793_713_500_000;
        let end_ms = 1_793_717_100_000;
        let account_path = PathBuf::from(&account_dir);
        let jcl_files = super::super::scan_jcl_files(&account_path, 0, start_ms, end_ms)
            .expect("local JCL list should be built");
        let records = scan_trial_records(&jcl_files, &account_path, &uid, start_ms, end_ms)
            .expect("local trial logs should parse");
        assert_eq!(records.len(), 3);
        assert!(records
            .iter()
            .all(|record| record.status == TrialCandidateStatus::Confirmed));
        assert!(records.iter().all(|record| record.layer == Some(6)));
        assert_eq!(
            records
                .iter()
                .map(|record| record.bosses.clone())
                .collect::<Vec<_>>(),
            vec![
                [
                    String::from("雷神"),
                    String::from("苏髓"),
                    String::from("藤原樱奈")
                ],
                [
                    String::from("寅"),
                    String::from("藤原樱奈"),
                    String::from("叶冬辰")
                ],
                [
                    String::from("寅"),
                    String::from("方鹤影"),
                    String::from("藤原樱奈")
                ],
            ]
        );
    }
}
