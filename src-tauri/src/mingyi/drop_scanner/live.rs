//! 试炼终局的低开销实时尾读。
//!
//! 常规扫描仍是唯一的落库、ChatLog 奖励/精力补齐路径。这里仅在唯一、刚刚观测到
//! 的活动角色上读取很小的 JCL 增量，尽快把已核验的最终通关交给截图观察器。

mod change_watch;

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use chrono::Utc;

use crate::game_directory::MINGYI_ACCOUNTS_BASE_PATH;
use crate::trial_capture;

use super::trial::{self, LiveMessageKind};
use change_watch::ChangeWatch;

const TICK: Duration = Duration::from_millis(750);
const HEALTHY_RESCAN: Duration = Duration::from_secs(60);
const FALLBACK_RESCAN: Duration = Duration::from_secs(5);
const ACTIVE_LEASE_MS: i64 = 90_000;
const FRESH_TERMINAL_MS: i64 = 90_000;
const FILE_LIMIT: usize = 6;
const TICK_BYTE_BUDGET: usize = 256 * 1024;
const MAX_LINE_BYTES: usize = 16 * 1024;
const EVENT_CACHE_LIMIT: usize = 96;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LiveStats {
    pub bytes_read: u64,
    pub lines_parsed: u64,
    pub parse_micros: u64,
    pub directory_entries_seen: u64,
    pub directory_micros: u64,
    pub fallback_rescans: u64,
    pub watch_rescans: u64,
}

static BYTES_READ: AtomicU64 = AtomicU64::new(0);
static LINES_PARSED: AtomicU64 = AtomicU64::new(0);
static PARSE_MICROS: AtomicU64 = AtomicU64::new(0);
static DIRECTORY_ENTRIES_SEEN: AtomicU64 = AtomicU64::new(0);
static DIRECTORY_MICROS: AtomicU64 = AtomicU64::new(0);
static FALLBACK_RESCANS: AtomicU64 = AtomicU64::new(0);
static WATCH_RESCANS: AtomicU64 = AtomicU64::new(0);

#[allow(dead_code)]
pub(crate) fn stats() -> LiveStats {
    LiveStats {
        bytes_read: BYTES_READ.load(Ordering::Relaxed),
        lines_parsed: LINES_PARSED.load(Ordering::Relaxed),
        parse_micros: PARSE_MICROS.load(Ordering::Relaxed),
        directory_entries_seen: DIRECTORY_ENTRIES_SEEN.load(Ordering::Relaxed),
        directory_micros: DIRECTORY_MICROS.load(Ordering::Relaxed),
        fallback_rescans: FALLBACK_RESCANS.load(Ordering::Relaxed),
        watch_rescans: WATCH_RESCANS.load(Ordering::Relaxed),
    }
}

#[derive(Default)]
struct Control {
    enabled: bool,
    game_directory: Option<PathBuf>,
    activity: Option<Activity>,
    last_observed_at_ms: Option<i64>,
    generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Activity {
    game_directory: PathBuf,
    process_start_ms: i64,
    uid: String,
}

type Shared = Arc<(Mutex<Control>, Condvar)>;
static CONTROL: LazyLock<Shared> =
    LazyLock::new(|| Arc::new((Mutex::new(Control::default()), Condvar::new())));
static STARTED: AtomicU64 = AtomicU64::new(0);

/// Starts one inert worker. It performs no filesystem work until both configuration and a
/// current unambiguous role binding have been supplied.
pub(crate) fn start() {
    if STARTED.fetch_add(1, Ordering::AcqRel) != 0 {
        return;
    }
    let control = Arc::clone(&CONTROL);
    std::thread::Builder::new()
        .name("trial-live-tail".to_owned())
        .spawn(move || worker(control))
        .map_err(|error| log::warn!("[TrialLive] 启动实时试炼监听失败: {error}"))
        .ok();
}

/// Installs the resolved runtime directory and the conjunction of the two user-facing
/// switches. A meaningful change retires all tails, so results read before a directory or
/// switch change cannot leak into the new session.
pub(crate) fn configure(enabled: bool, game_directory: &str) {
    let path = (!game_directory.trim().is_empty()).then(|| PathBuf::from(game_directory));
    let (lock, wake) = &**CONTROL;
    let Ok(mut control) = lock.lock() else {
        return;
    };
    if control.enabled == enabled && control.game_directory == path {
        return;
    }
    let directory_changed = control.game_directory != path;
    control.enabled = enabled;
    control.game_directory = path;
    if directory_changed {
        // An old async detector result must never be replayed against a newly configured game
        // directory before that directory has supplied its own active-role observation.
        control.activity = None;
        control.last_observed_at_ms = None;
    }
    control.generation = control.generation.wrapping_add(1);
    if enabled {
        if let (Some(activity), Some(observed_at_ms)) =
            (control.activity.as_ref(), control.last_observed_at_ms)
        {
            if Utc::now().timestamp_millis().saturating_sub(observed_at_ms) <= ACTIVE_LEASE_MS {
                // `set_enabled(true)` is queued before configure. Replaying the still-fresh
                // binding before waking the tail worker closes the enable-time race.
                trial_capture::update_activity(
                    activity.process_start_ms,
                    Some(&activity.uid),
                    observed_at_ms,
                );
            }
        }
    }
    wake.notify_one();
}

/// Accepts only a fresh result from the currently configured game directory. The return value
/// indicates acceptance; the matching capture activity is already queued under the lock.
pub(crate) fn activity(
    game_directory: &str,
    process_start_ms: i64,
    active_uid: Option<&str>,
    observed_at_ms: i64,
) -> bool {
    let directory = PathBuf::from(game_directory);
    let (lock, wake) = &**CONTROL;
    let Ok(mut control) = lock.lock() else {
        return false;
    };
    if control.game_directory.as_ref() != Some(&directory) {
        return false;
    }
    if control
        .last_observed_at_ms
        .is_some_and(|old| observed_at_ms < old)
    {
        return false;
    }
    // Store the watermark even when no UID is available. A later delayed Active result cannot
    // revive the old role after a newer ambiguous/offline observation.
    control.last_observed_at_ms = Some(observed_at_ms);
    let next = active_uid
        .filter(|uid| !uid.is_empty() && uid.bytes().all(|byte| byte.is_ascii_digit()))
        .filter(|_| process_start_ms > 0)
        .map(|uid| Activity {
            game_directory: directory,
            process_start_ms,
            uid: uid.to_owned(),
        });
    if control.activity != next {
        control.activity = next;
        control.generation = control.generation.wrapping_add(1);
    }
    // The capture worker must receive Activity before this worker can be woken to emit the
    // first terminal. Do this under the same control lock as the accepted generation change.
    trial_capture::update_activity(
        process_start_ms,
        control
            .activity
            .as_ref()
            .map(|activity| activity.uid.as_str()),
        observed_at_ms,
    );
    wake.notify_one();
    true
}

/// Retires the current binding. The caller clears capture immediately afterwards, after this
/// control lock has made any in-flight delivery fail its generation check.
pub(crate) fn clear() {
    let (lock, wake) = &**CONTROL;
    if let Ok(mut control) = lock.lock() {
        control.activity = None;
        control.generation = control.generation.wrapping_add(1);
        wake.notify_one();
    }
}

fn worker(control: Shared) {
    crate::background_work::lower_current_thread_priority();
    let mut engine = Engine::default();
    let mut delay = Duration::ZERO;
    loop {
        let snapshot = {
            let (lock, wake) = &*control;
            let Ok(mut guard) = lock.lock() else {
                return;
            };
            while !control_is_active(&guard, Utc::now().timestamp_millis()) {
                engine.reset();
                guard = match wake.wait(guard) {
                    Ok(next) => next,
                    Err(_) => return,
                };
                delay = Duration::ZERO;
            }
            if !delay.is_zero() {
                guard = match wake.wait_timeout(guard, delay) {
                    Ok((next, _)) => next,
                    Err(_) => return,
                };
            }
            Snapshot::from_control(&guard)
        };
        let started = Instant::now();
        let actions = engine.tick(snapshot);
        for action in actions {
            deliver_if_current(&control, action);
        }
        delay = live_delay(engine.has_backlog, started.elapsed());
    }
}

fn control_is_active(control: &Control, now: i64) -> bool {
    control.enabled
        && control.activity.is_some()
        && control.last_observed_at_ms.is_some_and(|observed| {
            (observed..=observed.saturating_add(ACTIVE_LEASE_MS)).contains(&now)
        })
}

fn live_delay(backlog: bool, work: Duration) -> Duration {
    let base = if backlog {
        Duration::from_millis(100)
    } else {
        TICK
    };
    base.max(work.saturating_mul(9))
}

#[derive(Clone)]
struct Snapshot {
    enabled: bool,
    generation: u64,
    activity: Option<Activity>,
    observed_at_ms: Option<i64>,
}

impl Snapshot {
    fn from_control(control: &Control) -> Self {
        Self {
            enabled: control.enabled,
            generation: control.generation,
            activity: control.activity.clone(),
            observed_at_ms: control.last_observed_at_ms,
        }
    }
}

fn deliver_if_current(control: &Shared, action: Action) {
    let (lock, _) = &**control;
    let Ok(current) = lock.lock() else {
        return;
    };
    let accepted = current.enabled
        && current.generation == action.generation
        && current.activity.as_ref() == Some(&action.activity)
        && current
            .last_observed_at_ms
            .is_some_and(|observed| observed >= action.observed_at_ms)
        && Utc::now()
            .timestamp_millis()
            .saturating_sub(current.last_observed_at_ms.unwrap_or_default())
            <= ACTIVE_LEASE_MS;
    if !accepted {
        return;
    }
    // Keep the control lock until the message is sent: configure/clear cannot interleave after
    // the generation check and deliver an old session's terminal to the capture worker.
    match action.kind {
        ActionKind::Scene { at, is_trial } => {
            trial_capture::observe_scene(&action.activity.uid, at, is_trial)
        }
        ActionKind::Boundary { at } => trial_capture::battle_boundary(&action.activity.uid, 0, at),
        ActionKind::Terminal { at, map_id } => {
            trial_capture::observe_scene(&action.activity.uid, at, true);
            trial_capture::cleared(&action.activity.uid, map_id, at);
        }
    }
}

#[derive(Debug, Clone)]
struct Action {
    generation: u64,
    activity: Activity,
    observed_at_ms: i64,
    kind: ActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Scene { at: i64, is_trial: bool },
    Boundary { at: i64 },
    Terminal { at: i64, map_id: i64 },
}

impl Action {
    fn at(&self) -> i64 {
        match self.kind {
            ActionKind::Scene { at, .. }
            | ActionKind::Boundary { at }
            | ActionKind::Terminal { at, .. } => at,
        }
    }
}

#[derive(Default)]
struct Engine {
    has_backlog: bool,
    bound: Option<Activity>,
    generation: u64,
    files: HashMap<PathBuf, Tail>,
    watch: Option<ChangeWatch>,
    next_fallback: Option<Instant>,
    sent_terminals: HashSet<(i64, i64)>,
    sent_order: Vec<(i64, i64)>,
    pending_scenes: Vec<FileMeta>,
    latest_terminal: i64,
    last_boundary: i64,
    observed_at_ms: i64,
    warned_watch: bool,
}

impl Engine {
    fn tick(&mut self, snapshot: Snapshot) -> Vec<Action> {
        let now_ms = Utc::now().timestamp_millis();
        let Some(activity) = snapshot.activity.filter(|activity| {
            snapshot.enabled
                && snapshot
                    .observed_at_ms
                    .is_some_and(|observed| now_ms.saturating_sub(observed) <= ACTIVE_LEASE_MS)
                && activity.process_start_ms > 0
        }) else {
            self.reset();
            return Vec::new();
        };
        self.observed_at_ms = snapshot.observed_at_ms.unwrap_or_default();
        if self.bound.as_ref() != Some(&activity) || self.generation != snapshot.generation {
            self.reset();
            self.bound = Some(activity.clone());
            self.generation = snapshot.generation;
            let directory = logs_directory(&activity);
            // Watch first, then enumerate. A change racing the enumeration remains latched and
            // schedules one coalesced follow-up refresh on the next tick.
            self.watch = ChangeWatch::new(&directory);
            if self.watch.is_none() && !self.warned_watch {
                self.warned_watch = true;
                log::warn!("[TrialLive] 目录变更监听不可用，试炼新日志将以 5 秒兜底发现");
            }
            self.refresh(&directory, &activity, now_ms, false);
            self.next_fallback = Some(Instant::now() + self.rescan_delay());
        }

        let directory = logs_directory(&activity);
        let mut watch_dirty = false;
        let mut watch_invalid = false;
        if let Some(watch) = self.watch.as_mut() {
            watch_dirty = watch.changed();
            watch_invalid = !watch.is_valid();
        }
        if watch_invalid {
            self.watch = None;
            if !self.warned_watch {
                self.warned_watch = true;
                log::warn!("[TrialLive] 目录变更监听失效，试炼新日志将以 5 秒兜底发现");
            }
        }
        let fallback_due = self.next_fallback.is_none_or(|at| Instant::now() >= at);
        if watch_dirty || fallback_due {
            self.refresh(&directory, &activity, now_ms, watch_dirty);
            self.next_fallback = Some(Instant::now() + self.rescan_delay());
        }

        let pending_scenes = std::mem::take(&mut self.pending_scenes);
        let mut actions = pending_scenes
            .into_iter()
            .filter(|meta| meta.at >= activity.process_start_ms && meta.at <= now_ms)
            .map(|meta| {
                self.action(
                    &activity,
                    ActionKind::Scene {
                        at: meta.at,
                        is_trial: false,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut remaining = TICK_BYTE_BUDGET;
        let mut raw = Vec::new();
        let mut paths: Vec<PathBuf> = self.files.keys().cloned().collect();
        // Catch-up has a fixed byte budget. Prefer the newest JCL start time so a large older
        // file cannot delay the active fight's terminal behind the other five candidates.
        paths.sort_by(|left, right| {
            self.files
                .get(right)
                .map(|tail| tail.meta.at)
                .cmp(&self.files.get(left).map(|tail| tail.meta.at))
                .then_with(|| left.cmp(right))
        });
        for path in paths {
            if remaining == 0 {
                break;
            }
            let Some(tail) = self.files.get_mut(&path) else {
                continue;
            };
            if !tail.meta.is_trial {
                continue;
            }
            let started = Instant::now();
            let (events, read) = tail.read(&path, &activity.uid, remaining);
            remaining = remaining.saturating_sub(read);
            BYTES_READ.fetch_add(read as u64, Ordering::Relaxed);
            LINES_PARSED.fetch_add(events.len() as u64, Ordering::Relaxed);
            PARSE_MICROS.fetch_add(started.elapsed().as_micros() as u64, Ordering::Relaxed);
            raw.extend(events.into_iter().map(|event| (tail.meta, event)));
        }

        raw.sort_by_key(|(_, event)| event.at());
        self.has_backlog = remaining == 0;
        for (meta, event) in raw {
            match event {
                Parsed::Battle(at) | Parsed::Stage(at) | Parsed::Boundary(at)
                    if self.accept_boundary(at) =>
                {
                    // 原始战斗事件可能一秒内写入成百上千行；同秒只需一个停止边界，
                    // 避免向截图线程的通道堆积大量重复消息。
                    actions.push(self.action(&activity, ActionKind::Boundary { at }))
                }
                Parsed::Terminal(at) if self.fresh_terminal(at, &activity, now_ms) => {
                    self.latest_terminal = self.latest_terminal.max(at);
                    let terminal_key = (meta.map_id, at);
                    if self.sent_terminals.insert(terminal_key) {
                        self.sent_order.push(terminal_key);
                        if self.sent_order.len() > EVENT_CACHE_LIMIT {
                            if let Some(expired) = self.sent_order.first().copied() {
                                self.sent_order.remove(0);
                                self.sent_terminals.remove(&expired);
                            }
                        }
                        actions.push(self.action(
                            &activity,
                            ActionKind::Terminal {
                                at,
                                map_id: meta.map_id,
                            },
                        ));
                    }
                }
                _ => {}
            }
        }
        actions.sort_by(|left, right| {
            left.at()
                .cmp(&right.at())
                .then_with(|| action_priority(left.kind).cmp(&action_priority(right.kind)))
        });
        actions
    }

    fn action(&self, activity: &Activity, kind: ActionKind) -> Action {
        Action {
            generation: self.generation,
            activity: activity.clone(),
            observed_at_ms: self.observed_at_ms,
            kind,
        }
    }

    fn accept_boundary(&mut self, at: i64) -> bool {
        if at < self.latest_terminal || at <= self.last_boundary {
            return false;
        }
        self.last_boundary = at;
        true
    }

    fn fresh_terminal(&self, at: i64, activity: &Activity, now_ms: i64) -> bool {
        at >= activity.process_start_ms
            && at <= now_ms
            && now_ms.saturating_sub(at) <= FRESH_TERMINAL_MS
    }

    fn rescan_delay(&self) -> Duration {
        self.watch
            .as_ref()
            .is_some_and(ChangeWatch::is_valid)
            .then_some(HEALTHY_RESCAN)
            .unwrap_or(FALLBACK_RESCAN)
    }

    fn refresh(&mut self, directory: &Path, activity: &Activity, now_ms: i64, watch_dirty: bool) {
        if watch_dirty {
            WATCH_RESCANS.fetch_add(1, Ordering::Relaxed);
        } else {
            FALLBACK_RESCANS.fetch_add(1, Ordering::Relaxed);
        }
        let scan_started = Instant::now();
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        // `read_dir` is flat but potentially large. Avoid metadata/stat calls for historical
        // files: retain only six current-process filename candidates while iterating names.
        let mut found = Vec::with_capacity(FILE_LIMIT);
        let mut entries_seen = 0_u64;
        for entry in entries.flatten() {
            entries_seen += 1;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(meta) = parse_filename(name) else {
                continue;
            };
            if meta.at < activity.process_start_ms || meta.at > now_ms {
                continue;
            }
            found.push((path, meta));
            found.sort_by(|left, right| {
                right
                    .1
                    .at
                    .cmp(&left.1.at)
                    .then_with(|| right.0.cmp(&left.0))
            });
            found.truncate(FILE_LIMIT);
        }
        DIRECTORY_ENTRIES_SEEN.fetch_add(entries_seen, Ordering::Relaxed);
        DIRECTORY_MICROS.fetch_add(scan_started.elapsed().as_micros() as u64, Ordering::Relaxed);
        let wanted: HashSet<PathBuf> = found.iter().map(|(path, _)| path.clone()).collect();
        self.files.retain(|path, _| wanted.contains(path));
        for (path, meta) in found {
            if self.files.contains_key(&path) {
                continue;
            }
            self.files.insert(path, Tail::new(meta));
            // File-name time is only used as a conservative non-trial scene boundary. It is not
            // terminal proof and never starts capture.
            if !meta.is_trial
                && meta.at >= now_ms.saturating_sub(FRESH_TERMINAL_MS)
                && meta.at <= now_ms
            {
                self.pending_scenes.push(meta);
            }
        }
    }

    fn reset(&mut self) {
        self.has_backlog = false;
        self.bound = None;
        self.files.clear();
        self.watch = None;
        self.next_fallback = None;
        self.latest_terminal = 0;
        self.last_boundary = 0;
        self.sent_terminals.clear();
        self.sent_order.clear();
        self.pending_scenes.clear();
    }
}

fn action_priority(kind: ActionKind) -> u8 {
    match kind {
        ActionKind::Scene { .. } => 0,
        ActionKind::Boundary { .. } => 1,
        ActionKind::Terminal { .. } => 2,
    }
}

fn logs_directory(activity: &Activity) -> PathBuf {
    activity
        .game_directory
        .join(MINGYI_ACCOUNTS_BASE_PATH)
        .join(format!("{}@zhcn_hd", activity.uid))
        .join("userdata")
        .join("combat_logs")
}

#[derive(Debug, Clone, Copy)]
struct FileMeta {
    at: i64,
    map_id: i64,
    is_trial: bool,
}

fn parse_filename(name: &str) -> Option<FileMeta> {
    let parsed = super::parse_jcl_filename(name)?;
    Some(FileMeta {
        at: parsed.timestamp,
        map_id: parsed.raid_id,
        is_trial: parsed.raid_display_name == "试炼之地",
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Identity {
    Unknown,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Copy)]
enum Parsed {
    Battle(i64),
    Stage(i64),
    Boundary(i64),
    Terminal(i64),
}

impl Parsed {
    fn at(self) -> i64 {
        match self {
            Self::Battle(at) | Self::Stage(at) | Self::Boundary(at) | Self::Terminal(at) => at,
        }
    }
}

struct Tail {
    meta: FileMeta,
    offset: u64,
    line: Vec<u8>,
    discarding: bool,
    identity: Identity,
    terminal_seen: bool,
    pending_terminal: Option<i64>,
    prefix: Vec<u8>,
}

impl Tail {
    fn new(meta: FileMeta) -> Self {
        Self {
            meta,
            offset: 0,
            line: Vec::with_capacity(1024),
            discarding: false,
            identity: Identity::Unknown,
            terminal_seen: false,
            pending_terminal: None,
            prefix: Vec::new(),
        }
    }

    fn reset(&mut self) {
        self.offset = 0;
        self.line.clear();
        self.discarding = false;
        self.identity = Identity::Unknown;
        self.terminal_seen = false;
        self.pending_terminal = None;
        self.prefix.clear();
    }

    fn read(&mut self, path: &Path, uid: &str, budget: usize) -> (Vec<Parsed>, usize) {
        let Ok(metadata) = fs::metadata(path) else {
            return (Vec::new(), 0);
        };
        let length = metadata.len();
        if length < self.offset || self.replaced(path, length) {
            self.reset();
        }
        if length <= self.offset || budget == 0 {
            return (Vec::new(), 0);
        }
        let available = usize::try_from(length.saturating_sub(self.offset)).unwrap_or(usize::MAX);
        let wanted = available.min(budget);
        let Ok(mut file) = File::open(path) else {
            return (Vec::new(), 0);
        };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return (Vec::new(), 0);
        }
        let mut chunk = vec![0_u8; wanted];
        let Ok(read) = file.read(&mut chunk) else {
            return (Vec::new(), 0);
        };
        chunk.truncate(read);
        self.offset = self.offset.saturating_add(read as u64);
        self.capture_prefix(path, length);
        let mut events = Vec::new();
        for byte in chunk {
            if self.discarding {
                if byte == b'\n' {
                    self.discarding = false;
                }
                continue;
            }
            if self.line.len() == MAX_LINE_BYTES {
                self.line.clear();
                self.discarding = byte != b'\n';
                continue;
            }
            self.line.push(byte);
            if byte == b'\n' {
                self.line.pop();
                if let Some(event) = parse_line(&self.line, uid, &mut self.identity) {
                    let ignore_after_terminal =
                        self.terminal_seen && !matches!(event, Parsed::Stage(_));
                    if matches!(event, Parsed::Terminal(_)) {
                        self.terminal_seen = true;
                    } else if matches!(event, Parsed::Stage(_)) {
                        self.terminal_seen = false;
                    }
                    if !ignore_after_terminal {
                        if let Parsed::Terminal(at) = event {
                            // 本轮已落盘文件的后续块仍可能包含相反UID，先保留候选，
                            // 追赶到当前EOF且身份一致后再允许启动截图。
                            self.pending_terminal = Some(at);
                        } else {
                            events.push(event);
                        }
                    }
                }
                self.line.clear();
            }
        }
        if self.identity == Identity::Rejected {
            events.clear();
            self.pending_terminal = None;
        } else if self.offset >= length
            && self.identity == Identity::Verified
            && self.line.is_empty()
        {
            if let Some(at) = self.pending_terminal.take() {
                events.push(Parsed::Terminal(at));
            }
        }
        events.sort_by_key(|event| event.at());
        (events, read)
    }

    /// Re-reading at most 64 bytes detects truncate-and-replace even when the replacement has
    /// already grown beyond the previous offset. Appends do not invalidate a completed prefix.
    fn replaced(&self, path: &Path, length: u64) -> bool {
        if self.prefix.len() != 64 || length < 64 {
            return false;
        }
        let Ok(mut file) = File::open(path) else {
            return false;
        };
        let mut current = [0_u8; 64];
        file.read_exact(&mut current).is_ok() && current.as_slice() != self.prefix.as_slice()
    }

    fn capture_prefix(&mut self, path: &Path, length: u64) {
        if self.prefix.len() == 64 || length < 64 {
            return;
        }
        let Ok(mut file) = File::open(path) else {
            return;
        };
        let mut prefix = [0_u8; 64];
        if file.read_exact(&mut prefix).is_ok() {
            self.prefix = prefix.to_vec();
        }
    }
}

fn parse_line(line: &[u8], expected_uid: &str, identity: &mut Identity) -> Option<Parsed> {
    if *identity == Identity::Rejected {
        return None;
    }
    let mut fields = line.splitn(6, |byte| *byte == b'\t');
    let _ = fields.next();
    let _ = fields.next();
    let timestamp = fields.next()?;
    let _ = fields.next();
    let event_type = fields.next()?;
    let payload = fields.next()?;
    let at = std::str::from_utf8(timestamp)
        .ok()?
        .parse::<i64>()
        .ok()?
        .saturating_mul(1000);
    let kind = std::str::from_utf8(event_type).ok()?.parse::<u8>().ok()?;
    if kind != 1 && !matches!(kind, 14 | 15 | 18 | 21) {
        return None;
    }
    let (payload, _, had_errors) = encoding_rs::GBK.decode(payload);
    if had_errors {
        return None;
    }
    if kind == 1 {
        let (actual_uid, started) = trial::extract_fight_uid_and_started(&payload)?;
        *identity = if super::jcl_uuid_matches_uid(&format!("U{actual_uid}/1"), expected_uid) {
            Identity::Verified
        } else {
            // A contrary type=1 makes the entire file unusable. In particular, a terminal
            // buffered before identity resolution can never be emitted later.
            Identity::Rejected
        };
        return (*identity == Identity::Verified && started).then_some(Parsed::Battle(at));
    }
    if *identity != Identity::Verified {
        // 身份行也可能在本次写入的后面。只缓存严格终局候选，不允许任何
        // 未核验事件发出；Tail在追赶到EOF且Verified之后才返回终局。
        return matches!(
            trial::live_system_message_kind(kind, &payload),
            Some(LiveMessageKind::Terminal)
        )
        .then_some(Parsed::Terminal(at));
    }
    if kind == 21 {
        return trial::is_combat_activity(&payload).then_some(Parsed::Battle(at));
    }
    match trial::live_system_message_kind(kind, &payload)? {
        LiveMessageKind::Stage => Some(Parsed::Stage(at)),
        LiveMessageKind::Boundary => Some(Parsed::Boundary(at)),
        LiveMessageKind::Terminal => Some(Parsed::Terminal(at)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_waits_for_eof_identity_in_both_chunk_orders() {
        for split in [false, true] {
            let path = fixture_path();
            let mut content = identity(1_000, "1234");
            content.extend(terminal(2_000));
            let split_at = content.len();
            content.extend(identity(3_000, "9999"));
            fs::write(&path, &content).unwrap();
            let mut reader = tail();
            let (first, _) =
                reader.read(&path, "1234", if split { split_at } else { content.len() });
            assert!(!first
                .iter()
                .any(|event| matches!(event, Parsed::Terminal(_))));
            let (second, _) = reader.read(&path, "1234", content.len());
            assert!(!second
                .iter()
                .any(|event| matches!(event, Parsed::Terminal(_))));
            assert_eq!(reader.identity, Identity::Rejected);
            fs::remove_file(path).unwrap();
        }
        let path = fixture_path();
        let mut content = terminal(2_000);
        let split_at = content.len();
        content.extend(identity(3_000, "1234"));
        fs::write(&path, &content).unwrap();
        let mut reader = tail();
        assert!(reader.read(&path, "1234", split_at).0.is_empty());
        assert_eq!(
            reader
                .read(&path, "1234", content.len())
                .0
                .iter()
                .filter(|event| matches!(event, Parsed::Terminal(2_000)))
                .count(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn backlog_catches_up_in_small_batches_with_a_work_budget() {
        assert_eq!(
            live_delay(true, Duration::from_millis(2)),
            Duration::from_millis(100)
        );
        assert_eq!(live_delay(false, Duration::from_millis(2)), TICK);
        assert_eq!(
            live_delay(true, Duration::from_millis(50)),
            Duration::from_millis(450)
        );
        assert!(!control_is_active(&Control::default(), 1_000));
    }

    #[test]
    fn same_second_battle_events_emit_only_one_boundary() {
        let mut engine = Engine::default();
        assert!(engine.accept_boundary(10_000));
        for _ in 0..10_000 {
            assert!(!engine.accept_boundary(10_000));
        }
        engine.latest_terminal = 12_000;
        assert!(!engine.accept_boundary(11_000));
        assert!(engine.accept_boundary(12_000));
        assert!(engine.accept_boundary(13_000));
    }

    fn fixture_path() -> PathBuf {
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace parent")
            .join("tmp");
        fs::create_dir_all(&base).expect("create workspace tmp");
        base.join(format!("trial-live-{}.jcl", uuid::Uuid::new_v4()))
    }

    fn line(at: i64, event_type: u8, payload: &str) -> Vec<u8> {
        let (gbk, _, _) = encoding_rs::GBK.encode(payload);
        let mut value = format!("0\t0\t{}\t0\t{event_type}\t", at / 1_000).into_bytes();
        value.extend_from_slice(gbk.as_ref());
        value.push(b'\n');
        value
    }

    fn identity(at: i64, uid: &str) -> Vec<u8> {
        line(at, 1, &format!(r#"{{true,"服务器::U{uid}/1",0}}"#))
    }

    fn terminal(at: i64) -> Vec<u8> {
        line(
            at,
            15,
            r#"{"MSG_NOTICE_GREEN","你通过了第6层的所有关卡。"}"#,
        )
    }

    fn stage(at: i64) -> Vec<u8> {
        line(at, 15, r#"{"MSG_NOTICE_GREEN","你通过了第6层的第1关。"}"#)
    }

    fn tail() -> Tail {
        Tail::new(FileMeta {
            at: 0,
            map_id: 144,
            is_trial: true,
        })
    }

    #[test]
    fn split_gbk_terminal_waits_for_a_complete_line_and_exact_uid() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut bytes = identity(at, "123456");
        bytes.extend(terminal(at + 1_000));
        let split = bytes.len() - 3;
        fs::write(&path, &bytes[..split]).expect("write partial fixture");
        let mut tail = tail();
        assert!(!tail
            .read(&path, "123456", 256 * 1024)
            .0
            .iter()
            .any(|event| matches!(event, Parsed::Terminal(_))));
        fs::write(&path, &bytes).expect("finish fixture");
        assert!(matches!(
            tail.read(&path, "123456", 256 * 1024).0.as_slice(),
            [Parsed::Terminal(value)] if *value == at + 1_000
        ));
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn wrong_uid_can_never_unlock_a_buffered_terminal() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut bytes = identity(at, "654321");
        bytes.extend(terminal(at + 1_000));
        bytes.extend(identity(at + 2_000, "123456"));
        bytes.extend(terminal(at + 3_000));
        fs::write(&path, bytes).expect("write fixture");
        assert!(tail().read(&path, "123456", 256 * 1024).0.is_empty());
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn stage_only_and_repeated_reads_do_not_start_a_terminal() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut bytes = identity(at, "123456");
        bytes.extend(stage(at + 1_000));
        fs::write(&path, bytes).expect("write fixture");
        let mut tail = tail();
        assert!(matches!(
            tail.read(&path, "123456", 256 * 1024).0.as_slice(),
            [Parsed::Battle(_), Parsed::Stage(_)]
        ));
        assert!(tail.read(&path, "123456", 256 * 1024).0.is_empty());
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn terminal_suppresses_later_same_file_battle_but_keeps_a_new_stage_boundary() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut bytes = identity(at, "123456");
        bytes.extend(terminal(at + 1_000));
        bytes.extend(line(at + 2_000, 21, "{[13]=1}"));
        bytes.extend(stage(at + 3_000));
        fs::write(&path, bytes).expect("write fixture");
        assert!(matches!(
            tail().read(&path, "123456", 256 * 1024).0.as_slice(),
            [Parsed::Battle(_), Parsed::Terminal(_), Parsed::Stage(_)]
        ));
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn truncate_or_replace_resets_identity_before_parsing_new_bytes() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut first = identity(at, "123456");
        first.extend(stage(at + 1_000));
        fs::write(&path, first).expect("write first fixture");
        let mut tail = tail();
        let _ = tail.read(&path, "123456", 256 * 1024);
        fs::write(&path, terminal(at + 2_000)).expect("replace fixture");
        assert!(tail.read(&path, "123456", 256 * 1024).0.is_empty());
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn huge_line_is_bounded_and_following_terminal_still_parses() {
        let path = fixture_path();
        let at = 1_700_000_000_000;
        let mut bytes = identity(at, "123456");
        bytes.extend(std::iter::repeat(b'x').take(MAX_LINE_BYTES + 100));
        bytes.push(b'\n');
        bytes.extend(terminal(at + 1_000));
        fs::write(&path, bytes).expect("write fixture");
        let mut tail = tail();
        assert!(matches!(
            tail.read(&path, "123456", 256 * 1024).0.as_slice(),
            [Parsed::Battle(_), Parsed::Terminal(_)]
        ));
        assert!(tail.line.capacity() <= MAX_LINE_BYTES * 2);
        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn stale_activity_never_wakes_file_work() {
        let activity = Activity {
            game_directory: PathBuf::from(r"E:\Game\JX3"),
            process_start_ms: 1,
            uid: "123456".to_owned(),
        };
        let actions = Engine::default().tick(Snapshot {
            enabled: true,
            generation: 1,
            activity: Some(activity),
            observed_at_ms: Some(Utc::now().timestamp_millis() - ACTIVE_LEASE_MS - 1),
        });
        assert!(actions.is_empty());
    }

    #[test]
    #[ignore = "set JX3_TRIAL_LIVE_REPLAY_JCL and JX3_TRIAL_LIVE_REPLAY_UID to replay a local JCL without DB access"]
    fn local_real_jcl_replay_reports_nonzero_bytes_and_one_or_more_terminals() {
        crate::background_work::lower_current_thread_priority();
        let path = PathBuf::from(
            std::env::var("JX3_TRIAL_LIVE_REPLAY_JCL").expect("real JCL path is required"),
        );
        let uid = std::env::var("JX3_TRIAL_LIVE_REPLAY_UID").expect("real JCL UID is required");
        let mut tail = tail();
        let mut read_total = 0_usize;
        let mut terminals = 0_usize;
        let mut batches = 0;
        let mut total_work = Duration::ZERO;
        let started = Instant::now();
        let length = fs::metadata(&path).unwrap().len() as usize;
        loop {
            let before = Instant::now();
            let (events, read) = tail.read(&path, &uid, TICK_BYTE_BUDGET);
            let work = before.elapsed();
            total_work += work;
            read_total += read;
            batches += usize::from(read > 0);
            terminals += events
                .iter()
                .filter(|event| matches!(event, Parsed::Terminal(_)))
                .count();
            if read == 0 || read_total >= length {
                break;
            }
            std::thread::sleep(live_delay(true, work));
        }
        assert_eq!(
            read_total, length,
            "real JCL must be read incrementally without gaps"
        );
        assert!(
            terminals > 0,
            "real JCL must contain a strict final trial message"
        );
        assert!(
            tail.read(&path, &uid, TICK_BYTE_BUDGET).0.is_empty(),
            "重复检查不重复终局"
        );
        println!("TRIAL_LOG_REPLAY bytes={read_total} batches={batches} terminals={terminals} wall_ms={} work_ms={:.3}",
            started.elapsed().as_millis(), total_work.as_secs_f64() * 1000.0);
    }
}
