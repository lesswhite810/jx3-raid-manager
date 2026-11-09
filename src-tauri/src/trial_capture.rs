//! 试炼最终通关 JCL 才启动限时取帧，以本人结算入包关联牌位与悬停装备。
//! 第一、二关、战斗和跨天存档都只记录日志，不触发截图。仅暂存识别结果。
mod async_ocr;
pub(crate) mod catalog;
#[cfg(test)]
mod continuity_tests;
mod ocr;
#[cfg(all(test, windows))]
mod performance_replay;
mod presence;
#[cfg(all(test, windows))]
mod replay_equipment;
#[cfg(test)]
mod sampling_tests;
#[cfg(all(test, windows))]
mod selector_replay;
mod session_equipment;
mod vision;
mod window;

use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant};
use tauri::Emitter;

const WATCH_MS: i64 = 15 * 60_000;
// ChatLog 写盘可能晚于结算窗口。只保留几分钟的牌位与时间戳，像素在每次识别后释放。
const RETENTION_MS: i64 = 5 * 60_000;
const REWARD_WINDOW_MS: i64 = 8_000;
/// A player may leave a reward panel open and continue flipping much later.
const MAX_RECEIPT_DELAY_MS: i64 = WATCH_MS;
/// Short, bounded wait for the scanner to deliver neighbouring leaf receipts.
/// This is deliberately independent from the fifteen-minute game watch.
const DRAW_SETTLE_MS: i64 = 60_000;
const IDLE_SAMPLE_INTERVAL: Duration = Duration::from_millis(1_000);
const PANEL_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
const STARTUP_SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
// 翻牌动画可能短暂隐藏底边，保留快档，不能在两次翻牌之间退回慢档。
const PANEL_FAST_TAIL_MS: i64 = 3_000;
const TOOLTIP_FRAME_INTERVAL_MS: i64 = 500;

#[derive(Default)]
struct CaptureSchedule {
    fast_until: Option<i64>,
    tooltip_needed: bool,
    last_full_frame: Option<i64>,
}

impl CaptureSchedule {
    fn mode(&self, now: i64) -> window::CaptureMode {
        if self.tooltip_needed
            && self
                .last_full_frame
                .is_none_or(|last| now.saturating_sub(last) >= TOOLTIP_FRAME_INTERVAL_MS)
        {
            window::CaptureMode::Full
        } else {
            window::CaptureMode::Cards
        }
    }

    fn captured(&mut self, mode: window::CaptureMode, now: i64) {
        if mode == window::CaptureMode::Full {
            self.last_full_frame = Some(now);
        }
    }

    fn observed(&mut self, now: i64, panel_visible: bool, tooltip_needed: bool) {
        if panel_visible {
            self.fast_until = Some(now.saturating_add(PANEL_FAST_TAIL_MS));
        }
        self.tooltip_needed = panel_visible && tooltip_needed;
    }

    fn recheck_uncertain_frame(&mut self, now: i64) {
        self.fast_until = Some(now.saturating_add(PANEL_SAMPLE_INTERVAL.as_millis() as i64));
        self.tooltip_needed = false;
    }

    fn interval(&self, now: i64) -> Duration {
        if self.fast_until.is_some_and(|until| now <= until) {
            PANEL_SAMPLE_INTERVAL
        } else {
            IDLE_SAMPLE_INTERVAL
        }
    }

    /// 慢取帧/分析后主动休息，串行识别的工作时间约占该周期至多1/10。
    /// 这是墙钟耗时的保守限速，不是整机CPU使用率或游戏FPS保证。
    fn delay(&self, now: i64, starting: bool, work: Duration) -> Duration {
        let base = if starting {
            STARTUP_SAMPLE_INTERVAL
        } else {
            self.interval(now)
        };
        base.max(work.saturating_mul(9))
    }
}

#[derive(Default)]
struct CaptureDiagnostics {
    attempts: u64,
    frames: u64,
    full_frames: u64,
    work: Duration,
    max_work: Duration,
}

impl CaptureDiagnostics {
    fn record(&mut self, cost: Duration, captured: bool, mode: window::CaptureMode) {
        self.attempts += 1;
        self.frames += u64::from(captured);
        self.full_frames += u64::from(captured && mode == window::CaptureMode::Full);
        self.work += cost;
        self.max_work = self.max_work.max(cost);
    }

    fn finish(&mut self) {
        if self.attempts > 0 {
            log::info!(
                "[TrialCapture] 采集结束: 检查{}次, 有效{}帧(完整{}), 累计取帧分析{}ms, 单次最高{}ms",
                self.attempts, self.frames, self.full_frames, self.work.as_millis(), self.max_work.as_millis(),
            );
            *self = Self::default();
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Target {
    uid: String,
    map: i64,
}

struct Watch {
    target: Target,
    at: i64,
    until: i64,
}

#[derive(Clone)]
struct Completion {
    target: Target,
    /// 父轮的稳定来源键只用于关联。截图结果永远写入具体翻牌子记录。
    run_key: String,
    /// 一笔可唯一确认的 ChatLog 奖励事件。不能用秒级时间替代。
    draw_key: String,
    terminal: i64,
    receipt: i64,
    /// Next terminal or battle start supplied by the scanner. It is a hard
    /// boundary even if its own draw receipts have not arrived yet.
    next_boundary: Option<i64>,
    retry_after: i64,
}

struct Sample {
    target: Target,
    at: i64,
    // PID 与从左数的牌位。同一进程连续两帧一致才可信。
    selected: Option<(u32, u8)>,
    /// 完整五张奖励牌仍可见但尚未翻开，是确认“继续翻牌”已回到牌背的唯一
    /// 画面证据。抓帧失败、失焦或遮挡的 `None` 不能当作新一张牌的分界。
    reset_visible: bool,
    equipment: Option<(u8, catalog::TooltipEvidence)>,
}

struct EquipmentReview {
    finish: Completion,
    selected: (u32, u8),
    selected_start: i64,
}

/// A receipt only owns the middle of the gap to its neighbours.  This keeps
/// two quick flips from ever consuming the same captured frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EvidenceWindow {
    start: i64,
    /// Short window used to decide the gold card.
    end: i64,
    /// Tooltip may be inspected later, but never beyond the next receipt.
    equipment_end: i64,
}

/// 同一笔入包在本轮牌面中的实际稳定段。`window.start` 被收紧到稳定段的首帧，
/// 以免第二次翻牌的 OCR 借到上一张尚未消失的 Tooltip。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedEvidence {
    selected: (u32, u8),
    window: EvidenceWindow,
}

/// 一次只保留一个提示框的文字指纹与解析结果，不缓存截图。
type TooltipIdentity = (u32, u8, u8, vision::Rect, u64);
type TooltipKey = (u64, u32, u8, u8, vision::Rect, u64);
static TOOLTIP_GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
struct TooltipCache {
    key: Option<TooltipIdentity>,
    generation: u64,
    evidence: Option<catalog::TooltipEvidence>,
    attempts: u8,
    last_attempt: Option<Instant>,
}

impl TooltipCache {
    fn candidate(
        &mut self,
        frame: &window::WindowFrame,
        card: &vision::CardObservation,
    ) -> Option<(u8, vision::Rect)> {
        let (Some(hovered), [rect]) = (card.hovered_slot, card.tooltip_rects.as_slice()) else {
            self.key = None;
            return None;
        };
        // 只散列说明窗中文字的高亮笔画，跳过半透明背景中的场景动画。
        // 标题、绑定或品级变化时重新识别；成功结果复用，未识别到时限次重试。
        let mut hash = DefaultHasher::new();
        // 装备标题、绑定、品级都在左栏。排除右侧精炼文本和提示窗底部的动画。
        for y in (rect.top..rect.top + rect.height() * 3 / 4).step_by(2) {
            for x in (rect.left..rect.left + rect.width() * 2 / 3).step_by(2) {
                let offset = (y * frame.width + x) * 4;
                let px = &frame.pixels[offset..offset + 3];
                let (b, g, r) = (px[0], px[1], px[2]);
                if (r > 180 && b > 155 && g < 100)
                    || (r > 185 && g > 160 && b < 85)
                    || (r.min(g).min(b) > 200 && y < rect.top + rect.height() / 3)
                {
                    hash.write_usize(offset);
                }
            }
        }
        let key = (frame.pid, card.selected_slot, hovered, *rect, hash.finish());
        if self.key != Some(key) {
            self.key = Some(key);
            self.generation = TOOLTIP_GENERATION.fetch_add(1, Ordering::Relaxed);
            self.attempts = 0;
            self.evidence = None;
            return None;
        }
        Some((hovered, *rect))
    }

    fn should_attempt(&self) -> bool {
        self.evidence.is_none()
            && self.attempts < 3
            && self
                .last_attempt
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
    }

    fn observe_async(
        &mut self,
        frame: &window::WindowFrame,
        card: &vision::CardObservation,
        worker: &mut async_ocr::TooltipWorker,
    ) -> Option<(u8, catalog::TooltipEvidence)> {
        // 旧画面的结果只释放任务槽，不允许穿过新面板或新一轮的 generation。
        if let Some(((generation, pid, selected, hovered, rect, hash), evidence)) = worker.poll() {
            if generation == self.generation
                && self.key == Some((pid, selected, hovered, rect, hash))
            {
                self.evidence = evidence;
            }
        }
        let (hovered, _) = self.candidate(frame, card)?;
        if self.should_attempt() {
            let (pid, selected, hovered, rect, hash) = self.key?;
            if worker.request(
                (self.generation, pid, selected, hovered, rect, hash),
                &frame.pixels,
                frame.width,
                frame.height,
                rect,
            ) {
                self.last_attempt = Some(Instant::now());
                self.attempts += 1;
            }
        }
        self.evidence.clone().map(|item| (hovered, item))
    }

    #[cfg(test)]
    fn observe(
        &mut self,
        frame: &window::WindowFrame,
        card: &vision::CardObservation,
    ) -> Option<(u8, catalog::TooltipEvidence)> {
        let (hovered, rect) = self.candidate(frame, card)?;
        if self.should_attempt() {
            self.last_attempt = Some(Instant::now());
            self.attempts += 1;
            self.evidence =
                ocr::recognize_trial_tooltip(&frame.pixels, frame.width, frame.height, rect);
        }
        self.evidence.clone().map(|item| (hovered, item))
    }
}

enum Message {
    Enabled(bool),
    Clear,
    Activity {
        start: i64,
        uid: Option<String>,
        at: i64,
    },
    Scene {
        uid: String,
        at: i64,
        trial: bool,
    },
    BattleBoundary {
        uid: String,
        at: i64,
    },
    Cleared(Target, i64),
    Finished(Completion),
}

static SENDER: OnceLock<mpsc::Sender<Message>> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

fn send(message: Message) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(message);
    }
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
    send(Message::Enabled(enabled));
}

pub fn clear() {
    send(Message::Clear);
}

/// 新鲜的角色检测结果。没有唯一活动角色时只关闭采集，保留迟到回执所需证据。
pub fn update_activity(process_start_ms: i64, uid: Option<&str>, observed_at_ms: i64) {
    send(Message::Activity {
        start: process_start_ms,
        uid: uid.map(str::to_owned),
        at: observed_at_ms,
    });
}

/// 最新会话 JCL 的活动场景；不是当前地图查询，也不能独自启动截图。
pub fn observe_scene(uid: &str, at_ms: i64, is_trial: bool) {
    send(Message::Scene {
        uid: uid.to_owned(),
        at: at_ms,
        trial: is_trial,
    });
}

/// 只用于关闭上一轮结算观察，不会创建等待通关的截图任务。
pub fn battle_boundary(uid: &str, _map: i64, at: i64) {
    if at <= chrono::Utc::now().timestamp_millis() {
        send(Message::BattleBoundary {
            uid: uid.to_owned(),
            at,
        });
    }
}

/// 仅由本人最终通关信号调用。保存进度、关卡通过和战斗活动不得启动取帧。
pub fn cleared(uid: &str, map: i64, at: i64) {
    let now = chrono::Utc::now().timestamp_millis();
    if (now - WATCH_MS..=now).contains(&at) {
        send(Message::Cleared(
            Target {
                uid: uid.to_owned(),
                map,
            },
            at,
        ));
    } else {
        log::info!(
            "[TrialCapture] 终局未开启取帧: 终局时间 {} 距当前 {} 超出 15 分钟窗口 (uid={uid}, map={map})",
            at,
            now - at
        );
    }
}

/// 为一轮中的某一笔翻牌奖励开启截图关联。
///
/// `draw_key` 必须由扫描器从 ChatLog 的稳定记录标识生成。相同秒的多笔仍会
/// 入队以截断相邻翻牌，但本模块会让它们全部保持未知，绝不靠时序猜测牌位。
pub fn finished_draw(
    uid: &str,
    map: i64,
    terminal: i64,
    run_key: &str,
    draw_key: &str,
    receipt: i64,
    next_boundary: Option<i64>,
) {
    let now = chrono::Utc::now().timestamp_millis();
    if run_key.trim().is_empty() || draw_key.trim().is_empty() {
        log::info!(
            "[TrialCapture] 翻牌回执缺少稳定来源键，无法关联截图 (terminal={terminal}, receipt={receipt})"
        );
        return;
    }
    // 新鲜度只看回执是否落在终局归属窗口（terminal..terminal+15min）以及
    // 是否早于下一边界，绝不与扫描时的墙钟比较。茗伊封存式落盘会让回执
    // 的消息时间远远早于扫描时间，按 now 判定新鲜度会把所有迟到回执丢掉。
    if (terminal..=terminal + MAX_RECEIPT_DELAY_MS).contains(&receipt)
        && next_boundary.is_none_or(|boundary| receipt < boundary)
        && receipt <= now
    {
        let delay = now.saturating_sub(receipt);
        if delay > MAX_RECEIPT_DELAY_MS {
            log::info!(
                "[TrialCapture] 接受迟到回执（封存式落盘延迟 {}ms）: terminal={terminal}, receipt={receipt} (draw_key={draw_key})",
                delay
            );
        }
        send(Message::Finished(Completion {
            target: Target {
                uid: uid.to_owned(),
                map,
            },
            run_key: run_key.to_owned(),
            draw_key: draw_key.to_owned(),
            terminal,
            receipt,
            next_boundary,
            retry_after: 0,
        }));
        return;
    }
    // 诊断：明确指出是哪一道门禁拒绝了本次回执。
    if receipt < terminal || receipt > terminal + MAX_RECEIPT_DELAY_MS {
        log::info!(
            "[TrialCapture] 回执不在终局归属窗口内: terminal={terminal}, receipt={receipt} (draw_key={draw_key})"
        );
    } else if next_boundary.is_some_and(|boundary| receipt >= boundary) {
        log::info!(
            "[TrialCapture] 回执晚于下一边界被丢弃: receipt={receipt}, next_boundary={} (draw_key={draw_key})",
            next_boundary.unwrap_or_default()
        );
    } else {
        log::info!(
            "[TrialCapture] 回执时间异常（晚于当前时间）: receipt={receipt}, now={now} (draw_key={draw_key})"
        );
    }
}

/// 采集端与回执端共用的持久化主键：同 uid + 副本 + 终局时间唯一标识一轮翻牌。
/// 注意与扫描器的 `dedupe_key`（JCL 角色全局 ID）无关，只在本模块内保持一致。
fn capture_run_key(target: &Target, terminal: i64) -> String {
    format!("{}:{}:{}", target.uid, target.map, terminal)
}

/// 从持久化观测还原内存样本。返回 `None` 表示该行不含有效面板信息。
fn sample_from_observation(
    observation: &crate::db::TrialCaptureObservation,
    target: &Target,
) -> Option<Sample> {
    let selected = (observation.pid >= 0
        && (1..=5).contains(&observation.slot))
    .then_some((observation.pid as u32, observation.slot as u8));
    let equipment = ((1..=5).contains(&observation.equipment_slot)
        && !observation.equipment_name.trim().is_empty())
    .then(|| {
        (
            observation.equipment_slot as u8,
            catalog::TooltipEvidence {
                name: observation.equipment_name.clone(),
                level: observation.equipment_level,
                bind: match observation.equipment_bind.as_str() {
                    "tradable" => catalog::TooltipBind::Tradable,
                    "bound" => catalog::TooltipBind::Bound,
                    _ => catalog::TooltipBind::Unknown,
                },
            },
        )
    });
    let has_content = selected.is_some() || equipment.is_some() || observation.reset_visible;
    has_content.then(|| Sample {
        target: target.clone(),
        at: observation.at_ms,
        selected,
        reset_visible: observation.reset_visible,
        equipment,
    })
}

/// 迟到回执到达时把持久化帧回灌进内存样本队列。按 (at, pid, slot) 去重，
/// 已在内存中的帧保持原样，随后按时间排序保证分段逻辑可用。
fn rehydrate_samples(
    session: &mut Session,
    finish: &Completion,
    conn: &rusqlite::Connection,
) -> usize {
    let run_key = capture_run_key(&finish.target, finish.terminal);
    let observations = match crate::db::db_load_trial_capture_observations_with_connection(
        conn,
        &run_key,
    ) {
        Ok(observations) => observations,
        Err(error) => {
            log::warn!("[TrialCapture] 读取持久化截图观测失败 (run_key={run_key}): {error}");
            return 0;
        }
    };
    if observations.is_empty() {
        return 0;
    }
    let existing: std::collections::HashSet<(i64, i64, i64)> = session
        .samples
        .iter()
        .map(|sample| {
            let (pid, slot) = sample
                .selected
                .map_or((-1, 0), |(pid, slot)| (i64::from(pid), i64::from(slot)));
            (sample.at, pid, slot)
        })
        .collect();
    let mut restored = 0usize;
    for observation in &observations {
        let Some(sample) = sample_from_observation(observation, &finish.target) else {
            continue;
        };
        let (pid, slot) = sample
            .selected
            .map_or((-1, 0), |(pid, slot)| (i64::from(pid), i64::from(slot)));
        if existing.contains(&(sample.at, pid, slot)) {
            continue;
        }
        session.samples.push_back(sample);
        restored += 1;
    }
    if restored > 0 {
        let mut samples: Vec<_> = session.samples.drain(..).collect();
        samples.sort_by_key(|sample| sample.at);
        session.samples.extend(samples);
        log::info!(
            "[TrialCapture] 迟到回执回灌 {} 帧持久化观测 (run_key={run_key}, 共 {} 帧)",
            restored,
            observations.len()
        );
    }
    restored
}

#[derive(Default)]
struct Session {
    watch: Option<Watch>,
    samples: VecDeque<Sample>,
    pending: Vec<Completion>,
    handled: Vec<Completion>,
    filled: Vec<(String, i64)>,
    equipment_reviews: Vec<EquipmentReview>,
}

impl Session {
    fn watch(&mut self, target: Target, at: i64, until: i64) {
        if let Some(watch) = self
            .watch
            .as_mut()
            .filter(|watch| watch.at == at && watch.target == target)
        {
            // A delayed receipt can arrive near the end of the initial
            // fifteen-minute game watch; retain only the short tooltip tail.
            // A scanner-supplied next-battle boundary is always allowed to
            // shorten the active watch.
            watch.until = if until > watch.until
                && until - watch.until <= session_equipment::LATE_TOOLTIP_WINDOW_MS
            {
                until
            } else {
                watch.until.min(until)
            };
            return;
        }
        if self.watch.as_ref().is_none_or(|watch| at > watch.at) {
            self.watch = Some(Watch { target, at, until });
        }
    }

    #[cfg(test)]
    fn finished(&mut self, finish: Completion) {
        self.finished_with_watch(finish, true);
    }

    fn finished_with_watch(&mut self, finish: Completion, allow_watch: bool) {
        if self
            .pending
            .iter()
            .chain(&self.handled)
            .any(|known| known.draw_key == finish.draw_key)
        {
            return;
        }
        // A sibling that arrived after an earlier OCR was committed may make
        // its old evidence window invalid. DB clears only automatic values in
        // that case; requeue the old draw too so retained frames can prove it
        // again under the newly split window. If no frames remain, it stays
        // unknown rather than inheriting the new draw's card.
        let reconsider = self
            .handled
            .iter()
            .filter(|old| {
                old.target == finish.target
                    && old.run_key == finish.run_key
                    && old.receipt < finish.receipt
                    && finish.receipt <= old.receipt.saturating_add(REWARD_WINDOW_MS)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !reconsider.is_empty() {
            self.filled
                .retain(|(key, _)| !reconsider.iter().any(|old| old.draw_key == *key));
            self.equipment_reviews.retain(|review| {
                !reconsider
                    .iter()
                    .any(|old| old.draw_key == review.finish.draw_key)
            });
            self.pending.extend(reconsider.into_iter().map(|mut old| {
                old.retry_after = 0;
                old
            }));
        }
        // 同一轮的每笔翻牌都独立入队。重扫只靠 draw_key 去重，不能用父轮
        // source_key 把第二、三次翻牌吞掉。
        let until =
            (finish.terminal + MAX_RECEIPT_DELAY_MS + session_equipment::LATE_TOOLTIP_WINDOW_MS)
                .min(finish.next_boundary.unwrap_or(i64::MAX).saturating_sub(1));
        if allow_watch {
            self.watch(finish.target.clone(), finish.terminal, until);
        }
        self.pending.push(finish);
    }

    fn evidence_window(&self, finish: &Completion) -> Option<EvidenceWindow> {
        self.evidence_window_with(finish, &[])
    }

    /// `extra` is the current drain batch.  `apply_completions` temporarily
    /// takes `pending` to make retries safe, so its later siblings must still
    /// participate in the boundary calculation.
    fn evidence_window_with(
        &self,
        finish: &Completion,
        extra: &[Completion],
    ) -> Option<EvidenceWindow> {
        let peers = self
            .pending
            .iter()
            .chain(&self.handled)
            .chain(extra)
            .filter(|other| {
                other.draw_key != finish.draw_key
                    && other.target == finish.target
                    && other.run_key == finish.run_key
            });
        // ChatLog only supplies a second for this case.  Do not guess which
        // leaf owns a frame when two independently stored draw rows share it.
        if peers.clone().any(|other| other.receipt == finish.receipt) {
            return None;
        }
        let previous = peers
            .clone()
            .filter(|other| other.receipt < finish.receipt)
            .map(|other| other.receipt)
            .max();
        let next = peers
            .filter(|other| other.receipt > finish.receipt)
            .map(|other| other.receipt)
            .min();
        let start = previous
            .map(|at| (at.saturating_add(finish.receipt).saturating_add(1)) / 2)
            .unwrap_or(i64::MIN)
            .max(finish.terminal.max(finish.receipt.saturating_sub(1_000)));
        let neighbour_end = next
            .map(|at| (finish.receipt.saturating_add(at).saturating_sub(1)) / 2)
            .unwrap_or(i64::MAX)
            .min(finish.next_boundary.unwrap_or(i64::MAX).saturating_sub(1));
        let end = neighbour_end.min(finish.receipt.saturating_add(REWARD_WINDOW_MS));
        let equipment_end = neighbour_end.min(
            finish
                .receipt
                .saturating_add(session_equipment::LATE_TOOLTIP_WINDOW_MS),
        );
        (end >= start).then_some(EvidenceWindow {
            start,
            end,
            equipment_end,
        })
    }

    fn selected_evidence(
        &self,
        finish: &Completion,
        window: EvidenceWindow,
    ) -> Option<SelectedEvidence> {
        // 结算前便已有同样的选中牌面时，无法证明画面属于新一轮。
        if self.samples.iter().any(|s| {
            s.target == finish.target
                && s.selected.is_some()
                && (finish.terminal - 3000..finish.terminal - 1000).contains(&s.at)
        }) {
            return None;
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum Transition {
            Initial,
            Reset,
            Changed,
        }

        #[derive(Clone, Copy, Debug)]
        struct Segment {
            selected: (u32, u8),
            start: i64,
            end: i64,
            /// 只有当前回执独占窗口中的两帧才可以确认稳定。窗口外的画面仅用于
            /// 找到牌背/换牌过渡，绝不能拿来补足新回执的两帧证据。
            last_window_at: Option<i64>,
            stable: bool,
            transition: Transition,
        }

        let previous_receipt = self
            .pending
            .iter()
            .chain(&self.handled)
            .filter(|other| {
                other.target == finish.target
                    && other.run_key == finish.run_key
                    && other.receipt < finish.receipt
            })
            .map(|other| other.receipt)
            .max();
        let has_previous_receipt = previous_receipt.is_some();
        // 两笔入包相距较远时，第二笔的独占窗口只保留“入包前 1 秒”。牌背或
        // 换牌可能早已发生；从前一笔回执开始读取上下文，只借它判定分段，不能
        // 用它构成当前回执的稳定双帧。
        let context_start = previous_receipt.unwrap_or(window.start).min(window.start);
        let mut completed = Vec::<Segment>::new();
        let mut current: Option<Segment> = None;
        let mut pending_reset = false;
        let mut window_pid = None;
        let mut mixed_window_pid = false;

        let finish_current = |current: &mut Option<Segment>, completed: &mut Vec<Segment>| {
            if let Some(segment) = current.take() {
                completed.push(segment);
            }
        };

        for sample in self
            .samples
            .iter()
            .filter(|s| (context_start..=window.end).contains(&s.at))
        {
            if sample.target != finish.target {
                finish_current(&mut current, &mut completed);
                continue;
            }
            let Some(selected) = sample.selected else {
                finish_current(&mut current, &mut completed);
                // 只有完整、未翻开的五张牌是可信重置；窗口失焦、最小化和遮挡
                // 都不能证明牌背，不能把随后同牌位误当重翻。
                if sample.reset_visible {
                    pending_reset = true;
                }
                continue;
            };

            if sample.at >= window.start {
                if window_pid.is_some_and(|pid| pid != selected.0) {
                    mixed_window_pid = true;
                }
                window_pid.get_or_insert(selected.0);
            }

            let previous = current.or_else(|| completed.last().copied());
            match current.as_mut() {
                Some(segment)
                    if segment.selected == selected
                        && (0..=1500).contains(&(sample.at - segment.end)) =>
                {
                    if sample.at >= window.start
                        && segment
                            .last_window_at
                            .is_some_and(|previous| (200..=1500).contains(&(sample.at - previous)))
                    {
                        segment.stable = true;
                    }
                    segment.end = sample.at;
                    if sample.at >= window.start {
                        segment.last_window_at = Some(sample.at);
                    }
                }
                _ => {
                    finish_current(&mut current, &mut completed);
                    let transition = if pending_reset {
                        Transition::Reset
                    } else if previous.is_some_and(|segment| segment.selected != selected) {
                        // 不同牌位连续稳定出现，本身足以证明已从上一张切到下一张；
                        // 同牌位则必须看见牌背，不能只凭时间中点猜测。
                        Transition::Changed
                    } else {
                        Transition::Initial
                    };
                    current = Some(Segment {
                        selected,
                        start: sample.at,
                        end: sample.at,
                        last_window_at: (sample.at >= window.start).then_some(sample.at),
                        stable: false,
                        transition,
                    });
                    pending_reset = false;
                }
            }
        }
        finish_current(&mut current, &mut completed);

        if mixed_window_pid {
            return None;
        }

        let segment = completed.last().copied()?;
        if !segment.stable {
            return None;
        }
        // 第二笔及后续翻牌，若最后看到的还是上一张同牌位且中间没有牌背，
        // 不能把它自动归给新回执。不同牌位稳定切换则可作为新的分段证据。
        if has_previous_receipt && segment.transition == Transition::Initial {
            return None;
        }
        // 第一笔尚未有已知的前一张牌时，第二个稳定面板（即使牌位恰好相同）
        // 都可能属于一笔尚未写入 ChatLog 的追加翻牌。保持未知，等待可区分的
        // 回执，不能因为最后一段更晚就抢先归属。
        if !has_previous_receipt
            && completed
                .iter()
                .filter(|candidate| candidate.stable)
                .count()
                > 1
        {
            return None;
        }
        Some(SelectedEvidence {
            selected: segment.selected,
            window: EvidenceWindow {
                start: segment.start.max(window.start),
                end: window.end,
                equipment_end: window.equipment_end,
            },
        })
    }

    #[cfg(test)]
    fn selected_slot(&self, finish: &Completion, window: EvidenceWindow) -> Option<(u32, u8)> {
        self.selected_evidence(finish, window)
            .map(|evidence| evidence.selected)
    }

    fn active_watch(&self, now: i64) -> Option<&Watch> {
        self.watch.as_ref().filter(|watch| now <= watch.until)
    }

    fn prune(&mut self, now: i64) {
        // 茗伊 ChatLog 是封存式落盘，迟到回执的 receipt 远远早于当前墙钟。
        // 保留水位必须以"当前正在等待的工作"里最早的终局时间为准，否则
        // 迟到回执会被立即裁掉、永远等不到解析。
        let active_floor = self
            .pending
            .iter()
            .chain(self.equipment_reviews.iter().map(|review| &review.finish))
            .map(|finish| finish.terminal)
            .min()
            .map_or(now, |terminal| now.min(terminal));
        let floor = active_floor.saturating_sub(RETENTION_MS);
        // 保留最后一次观察的时间水位。扫描器会反复看到同一关，过期信号不能再打开截图。
        self.samples.retain(|sample| sample.at >= floor);
        self.handled
            .retain(|finish| finish.receipt >= floor || finish.terminal >= floor);
        self.filled.retain(|(_, at)| *at >= floor);
        self.pending
            .retain(|finish| finish.receipt >= floor || finish.terminal >= floor);
        self.equipment_reviews.retain(|review| {
            review.finish.receipt >= floor || review.finish.terminal >= floor
        });
    }
}

pub fn start(app: tauri::AppHandle, enabled: bool) {
    let (sender, receiver) = mpsc::channel();
    if SENDER.set(sender).is_err() {
        return;
    }
    ENABLED.store(enabled, Ordering::Relaxed);
    if let Err(error) = std::thread::Builder::new()
        .name("trial-card-capture".into())
        .spawn(move || run(app, receiver, enabled))
    {
        ENABLED.store(false, Ordering::Relaxed);
        log::error!("无法启动试炼牌位识别: {error}");
    }
}

fn run(app: tauri::AppHandle, receiver: mpsc::Receiver<Message>, mut enabled: bool) {
    crate::background_work::lower_current_thread_priority();
    let mut session = Session::default();
    let mut presence = presence::CapturePresence::default();
    let mut tooltip = TooltipCache::default();
    let mut tooltip_worker = async_ocr::TooltipWorker::new();
    let mut capture = window::CaptureSession::default();
    let mut schedule = CaptureSchedule::default();
    let mut diagnostics = CaptureDiagnostics::default();
    let mut next_tick = Instant::now();
    loop {
        let before_wait = chrono::Utc::now().timestamp_millis();
        let watching = session
            .active_watch(before_wait)
            .is_some_and(|watch| presence.allows(&watch.target, before_wait));
        if !watching {
            diagnostics.finish();
            // 阻塞等待下一轮前归还像素、GDI 和提示缓存，不在后台长期保留大缓冲。
            capture = window::CaptureSession::default();
            schedule = CaptureSchedule::default();
            tooltip = TooltipCache::default();
        }
        let active =
            watching || !session.pending.is_empty() || !session.equipment_reviews.is_empty();
        let message = if active {
            match receiver.recv_timeout(next_tick.saturating_duration_since(Instant::now())) {
                Ok(msg) => Some(msg),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        } else {
            match receiver.recv() {
                Ok(msg) => Some(msg),
                Err(_) => return,
            }
        };
        let now = chrono::Utc::now().timestamp_millis();
        session.prune(now);
        for command in message.into_iter().chain(receiver.try_iter()) {
            match command {
                Message::Enabled(value) => {
                    enabled = value;
                    if !enabled {
                        session = Session::default();
                        presence = presence::CapturePresence::default();
                        tooltip = TooltipCache::default();
                        capture = window::CaptureSession::default();
                        schedule = CaptureSchedule::default();
                    }
                }
                Message::Clear => {
                    session = Session::default();
                    presence = presence::CapturePresence::default();
                    tooltip = TooltipCache::default();
                    capture = window::CaptureSession::default();
                    schedule = CaptureSchedule::default();
                }
                Message::Activity { start, uid, at } => presence.activity(start, uid, at),
                Message::Scene { uid, at, trial } => presence.scene(&uid, at, trial),
                Message::BattleBoundary { uid, at } => presence.boundary(&uid, at),
                Message::Cleared(target, at) if enabled => {
                    let new_terminal = session
                        .watch
                        .as_ref()
                        .is_none_or(|watch| watch.target != target || watch.at != at);
                    if presence.cleared(&target, at, now) {
                        session.watch(target.clone(), at, at + WATCH_MS);
                        // 注册本轮采集会话，供扫描侧在 ChatLog 封存落盘后补查迟到回执。
                        match crate::db::init_db() {
                            Ok(conn) => {
                                let run = crate::db::TrialCaptureRun {
                                    run_key: capture_run_key(&target, at),
                                    uid: target.uid.clone(),
                                    map_id: target.map,
                                    terminal_ms: at,
                                    frame_count: 0,
                                    reconciled: false,
                                };
                                if let Err(error) =
                                    crate::db::db_upsert_trial_capture_run_with_connection(&conn, &run)
                                {
                                    log::warn!("[TrialCapture] 登记采集会话失败: {error}");
                                }
                            }
                            Err(error) => {
                                log::warn!("[TrialCapture] 打开数据库登记采集会话失败: {error}")
                            }
                        }
                        if new_terminal {
                            next_tick = Instant::now();
                        }
                    }
                }
                Message::Finished(finish) if enabled => {
                    if let Some(boundary) = finish.next_boundary.filter(|at| *at <= now) {
                        presence.boundary(&finish.target.uid, boundary);
                    }
                    let allow_watch =
                        presence.draw(&finish.target, finish.terminal, finish.receipt, now);
                    // 迟到回执（封存式 ChatLog 落盘后）会把内存帧早已裁掉，
                    // 必须先回灌持久化观测再入队，本 tick 的 apply 才有证据可用。
                    match crate::db::init_db() {
                        Ok(conn) => {
                            rehydrate_samples(&mut session, &finish, &conn);
                        }
                        Err(error) => {
                            log::warn!("[TrialCapture] 回灌持久化观测时打开数据库失败: {error}")
                        }
                    }
                    // 停止后仍可用已取得的帧补齐迟到奖励，不能因旧回执重新采集。
                    session.finished_with_watch(finish, allow_watch);
                }
                _ => {}
            }
        }
        if !enabled || !is_enabled() || Instant::now() < next_tick {
            continue;
        }
        let mut frame_work = Duration::ZERO;
        if let Some(watch) = session
            .active_watch(now)
            .filter(|watch| presence.allows(&watch.target, now))
        {
            let capture_started = Instant::now();
            let mut sample_at = now;
            let mode = schedule.mode(now);
            schedule.captured(mode, now);
            let observation = capture.capture_foreground(mode).map(|frame| {
                // 用像素实际读回的时间关联回执，不用消息处理前的循环时间。
                sample_at = chrono::Utc::now().timestamp_millis();
                let card = vision::card_observation(
                    &frame.pixels,
                    frame.width,
                    frame.height,
                    frame.cursor,
                );
                let panel_visible = card.is_some()
                    || vision::cards_visible(&frame.pixels, frame.width, frame.height);
                let selector = vision::selector_visible(
                    &frame.pixels,
                    frame.width,
                    frame.height,
                    panel_visible,
                );
                let reset_visible = card.is_none()
                    && panel_visible
                    && vision::card_backs_visible(&frame.pixels, frame.width, frame.height);
                let tooltip_needed = card
                    .as_ref()
                    .is_some_and(|card| card.hovered_slot.is_some());
                let item = if mode == window::CaptureMode::Full {
                    if let Some(card) = card.as_ref() {
                        tooltip.observe_async(frame, card, &mut tooltip_worker)
                    } else {
                        tooltip.key = None;
                        None
                    }
                } else {
                    // ROI 帧只证明牌位，不能把区域之外的黑像素或旧提示用于 OCR。
                    // 相同悬停之间穿插轻量 ROI 帧时保留指纹，下个完整帧仍需复核。
                    let same_hover = card.as_ref().is_some_and(|card| {
                        tooltip.key.is_some_and(|(pid, selected, hovered, _, _)| {
                            pid == frame.pid
                                && selected == card.selected_slot
                                && card.hovered_slot == Some(hovered)
                        })
                    });
                    if !same_hover {
                        tooltip.key = None;
                    }
                    None
                };
                (
                    card.map(|card| (frame.pid, card.selected_slot)),
                    item,
                    panel_visible,
                    tooltip_needed,
                    reset_visible,
                    selector,
                )
            });
            if observation.is_none() {
                tooltip.key = None;
            }
            if capture.session_ended() {
                // 进程退出/被替换已由无像素检查确认，不等待角色日志轮询。
                presence.end_capture(sample_at);
            } else {
                let frame = match observation.as_ref() {
                    Some((_, _, _, _, _, selector)) => (*selector).into(),
                    None => presence::CaptureFrame::Unavailable,
                };
                presence.frame(sample_at, frame);
            }
            let captured = observation.is_some();
            let visual_uncertain = observation
                .as_ref()
                .is_some_and(|(_, _, _, _, _, selector)| selector.is_none());
            let (selected, equipment, _, tooltip_needed, reset_visible, selector) =
                observation.unwrap_or((None, None, false, false, false, None));
            // 结构化观测当场落库（无像素），封存式 ChatLog 落盘后仍可回灌补录。
            // 有牌位或装备信息才值得写库，纯空帧由内存队列承担。
            let persist_observation = selected.is_some() || equipment.is_some();
            if persist_observation {
                let run_key = capture_run_key(&watch.target, watch.at);
                let (equipment_slot, equipment_name, equipment_level, equipment_bind) =
                    equipment
                        .as_ref()
                        .map_or((0, String::new(), 0, String::new()), |(slot, evidence)| {
                            (
                                i64::from(*slot),
                                evidence.name.clone(),
                                evidence.level,
                                match evidence.bind {
                                    catalog::TooltipBind::Tradable => "tradable".to_owned(),
                                    catalog::TooltipBind::Bound => "bound".to_owned(),
                                    catalog::TooltipBind::Unknown => "unknown".to_owned(),
                                },
                            )
                        });
                let observation = crate::db::TrialCaptureObservation {
                    run_key,
                    at_ms: sample_at,
                    pid: selected.map_or(-1, |(pid, _)| i64::from(pid)),
                    slot: selected.map_or(0, |(_, slot)| i64::from(slot)),
                    reset_visible,
                    equipment_slot,
                    equipment_name,
                    equipment_level,
                    equipment_bind,
                };
                match crate::db::init_db() {
                    Ok(conn) => {
                        if let Err(error) = crate::db::db_insert_trial_capture_observation_with_connection(&conn, &observation) {
                            log::warn!("[TrialCapture] 持久化截图观测失败: {error}");
                        }
                    }
                    Err(error) => {
                        log::warn!("[TrialCapture] 持久化截图观测时打开数据库失败: {error}")
                    }
                }
            }
            schedule.observed(sample_at, selector == Some(true), tooltip_needed);
            if visual_uncertain {
                schedule.recheck_uncertain_frame(sample_at);
            }
            session.samples.push_back(Sample {
                target: watch.target.clone(),
                at: sample_at,
                selected,
                reset_visible,
                equipment,
            });
            frame_work = capture_started.elapsed();
            diagnostics.record(frame_work, captured, mode);
        }
        if !session
            .active_watch(now)
            .is_some_and(|watch| presence.allows(&watch.target, now))
        {
            diagnostics.finish();
            // 当帧确认关闭就归还截图资源；迟到记录只使用下面的结构化证据。
            capture = window::CaptureSession::default();
            tooltip = TooltipCache::default();
            schedule = CaptureSchedule::default();
        }
        if !is_enabled() {
            session = Session::default();
            continue;
        }
        apply_completions(&mut session, now, |key, slot, observed| {
            let outcome = crate::db::db_apply_auto_trial_draw_flip(
                key,
                i64::from(slot),
                observed.start,
                observed.end,
            )?;
            if outcome == crate::db::TrialAutoFlipUpdateOutcome::Missing {
                return Err("等待通关记录写入".into());
            }
            if outcome == crate::db::TrialAutoFlipUpdateOutcome::Updated {
                log::info!("[TrialCapture] 已补录第 {slot} 张牌，翻牌来源 {key}");
                if let Err(error) = app.emit("trial-flip-updated", ()) {
                    log::warn!("刷新试炼翻牌结果失败: {error}");
                }
            }
            Ok(())
        });
        apply_equipment_reviews(&mut session, now, |finish, selected, observed, items| {
            let mut conn = crate::db::init_db()?;
            for (slot, evidence) in items {
                if !is_enabled() {
                    break;
                }
                let Some(item) = catalog::match_reward_tooltip(&conn, finish.terminal, &evidence)?
                else {
                    continue;
                };
                let result = crate::db::db_apply_auto_trial_draw_equipment_with_connection(
                    &mut conn,
                    &finish.draw_key,
                    i64::from(selected),
                    i64::from(slot),
                    &item.equipment_id,
                    observed.start,
                    observed.equipment_end,
                )?;
                if result == crate::db::TrialAutoEquipmentUpdateOutcome::Updated {
                    log::info!(
                        "[TrialCapture] 第 {slot} 张牌识别到 {}（{} 品），翻开第 {selected} 张",
                        item.name,
                        item.level
                    );
                    if let Err(error) = app.emit("trial-flip-updated", ()) {
                        log::warn!("刷新试炼装备结果失败: {error}");
                    }
                }
            }
            Ok(())
        });
        // 始终从处理结束后计时，不补发积压帧。连续翻牌期间不因复核旧牌而降频。
        let schedule_at = chrono::Utc::now().timestamp_millis();
        next_tick = Instant::now()
            + schedule.delay(schedule_at, presence.is_starting(schedule_at), frame_work);
    }
}

fn apply_equipment_reviews(
    session: &mut Session,
    now: i64,
    mut save: impl FnMut(
        &Completion,
        u8,
        EvidenceWindow,
        Vec<(u8, catalog::TooltipEvidence)>,
    ) -> Result<(), String>,
) {
    for mut review in std::mem::take(&mut session.equipment_reviews) {
        let finish = &review.finish;
        if now < finish.retry_after {
            session.equipment_reviews.push(review);
            continue;
        }
        let Some(window) = session.evidence_window(finish) else {
            // A same-second sibling must stay unknown. Never borrow the
            // sibling's pictures for tooltip OCR either.
            continue;
        };
        let sample_window = EvidenceWindow {
            start: window.start.max(review.selected_start),
            ..window
        };
        let (items, open) = session_equipment::resolve_card_equipment(
            &session.samples,
            finish,
            review.selected,
            sample_window,
        );
        let recent = session.samples.back().is_some_and(|s| now - s.at <= 1_500);
        if open
            && recent
            && now < finish.receipt + session_equipment::LATE_TOOLTIP_WINDOW_MS
            && now <= window.equipment_end
        {
            session.equipment_reviews.push(review);
            continue;
        }
        if !items.is_empty() {
            if let Err(error) = save(finish, review.selected.1, window, items) {
                log::warn!("保存试炼装备失败: {error}");
                review.finish.retry_after = now + 5_000;
                session.equipment_reviews.push(review);
            }
        }
    }
}

fn apply_completions(
    session: &mut Session,
    now: i64,
    mut save: impl FnMut(&str, u8, EvidenceWindow) -> Result<(), String>,
) {
    let mut pending = std::mem::take(&mut session.pending);
    pending.sort_by_key(|finish| finish.receipt);
    for finish in &pending {
        if session
            .filled
            .iter()
            .any(|(key, _)| key == &finish.draw_key)
        {
            continue;
        }
        // Do not commit an early frame just because a following receipt has
        // not been parsed yet.  ChatLog can arrive late; retaining one minute
        // lets a subsequent leaf shrink this draw's exclusive window first.
        if now <= finish.receipt + DRAW_SETTLE_MS || now < finish.retry_after {
            session.pending.push(finish.clone());
            continue;
        }
        let window = session.evidence_window_with(finish, &pending);
        let selected = window.and_then(|window| session.selected_evidence(finish, window));
        if let Some(evidence) = selected {
            let (pid, slot) = evidence.selected;
            // 持久化完整关联窗，迟到的相邻回执才能撤销曾借用它画面的自动值。
            // 缩窄后的稳定段只用于当前装备解析，不能替代数据库的撤销边界。
            let observed = window.expect("selected evidence requires an association window");
            if let Err(error) = save(&finish.draw_key, slot, observed) {
                log::warn!("保存试炼牌位失败: {error}");
                let mut retry = finish.clone();
                retry.retry_after = now + 5_000;
                session.pending.push(retry);
                continue;
            }
            session
                .filled
                .push((finish.draw_key.clone(), finish.receipt));
            session.equipment_reviews.push(EquipmentReview {
                finish: finish.clone(),
                selected: (pid, slot),
                selected_start: evidence.window.start,
            });
        } else {
            log::debug!(
                "[TrialCapture] 结算入包 {} 暂无可唯一确认的牌位，来源 {}",
                finish.receipt,
                finish.draw_key
            );
        }
        session.handled.push(finish.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(uid: &str) -> Target {
        Target {
            uid: uid.into(),
            map: 144,
        }
    }
    fn completion() -> Completion {
        Completion {
            target: target("1234"),
            run_key: "trial:1234:144:24000".into(),
            draw_key: "trial-draw:1234:27000:a".into(),
            terminal: 24_000,
            receipt: 27_000,
            next_boundary: None,
            retry_after: 0,
        }
    }
    fn sample(session: &mut Session, uid: &str, at: i64, selected: Option<(u32, u8)>) {
        session.samples.push_back(Sample {
            target: target(uid),
            at,
            selected,
            reset_visible: false,
            equipment: None,
        });
    }

    #[test]
    fn buffered_card_is_bound_to_the_delayed_terminal_once() {
        let mut s = Session::default();
        s.watch(target("1234"), 1_000, 1_000 + WATCH_MS);
        sample(&mut s, "1234", 27_500, Some((208, 2)));
        sample(&mut s, "1234", 28_000, Some((208, 2)));
        let f = completion();
        s.finished(f.clone());
        let mut saved = Vec::new();
        apply_completions(&mut s, 88_000, |key, slot, _| {
            saved.push((key.to_string(), slot));
            Ok(())
        });
        s.finished(f.clone());
        s.watch(target("1234"), 1_000, 1_000 + WATCH_MS);
        // Keep observing this live reward session: a later “继续翻牌” may
        // still produce another independent receipt and panel.
        assert!(s.active_watch(88_000).is_some());
        assert!(s.pending.is_empty());
        assert_eq!(saved, vec![(f.draw_key, 2)]);
    }

    #[test]
    fn unstable_wrong_role_pid_or_time_remains_unknown() {
        let f = completion();
        for (uid, at, second) in [
            ("1234", 27_502, Some((208, 2))),
            ("1234", 30_000, Some((208, 2))),
            ("9999", 28_000, Some((208, 2))),
            ("1234", 28_000, Some((209, 2))),
            ("1234", 28_000, Some((208, 3))),
            ("1234", 28_000, None),
        ] {
            let mut s = Session::default();
            sample(&mut s, "1234", 27_500, Some((208, 2)));
            sample(&mut s, uid, at, second);
            assert_eq!(
                s.selected_slot(&f, s.evidence_window(&f).unwrap()),
                None,
                "{uid} {at} {second:?}"
            );
        }
    }

    #[test]
    fn nearby_second_receipt_cannot_fill_the_first_draw_without_its_own_frames() {
        let first = completion();
        let later = Completion {
            receipt: first.receipt + 2_000,
            draw_key: "trial-draw:1234:29000:b".into(),
            ..first.clone()
        };
        let mut s = Session::default();
        // The game was still animating during the first flip. Only the second
        // card has two stable frames; a shared whole-run sample pool would
        // incorrectly write both rows as card 2.
        sample(&mut s, "1234", 28_750, None);
        s.samples.back_mut().unwrap().reset_visible = true;
        sample(&mut s, "1234", 29_200, Some((208, 2)));
        sample(&mut s, "1234", 29_700, Some((208, 2)));
        s.finished(first.clone());
        s.finished(later.clone());
        let mut slots = Vec::new();
        apply_completions(&mut s, 90_000, |key, slot, _| {
            slots.push((key.to_owned(), slot));
            Ok(())
        });
        assert_eq!(slots, [(later.draw_key, 2)]);
        assert!(s.pending.is_empty());
    }

    #[test]
    fn same_card_on_two_nearby_receipts_is_still_two_independent_draws() {
        let first = completion();
        let later = Completion {
            receipt: first.receipt + 2_000,
            draw_key: "trial-draw:1234:29000:b".into(),
            ..first.clone()
        };
        let mut s = Session::default();
        sample(&mut s, "1234", 27_200, Some((208, 2)));
        sample(&mut s, "1234", 27_700, Some((208, 2)));
        // 同位置重翻也必须观察到牌背，不能沿用上一张常驻金框。
        sample(&mut s, "1234", 28_750, None);
        s.samples.back_mut().unwrap().reset_visible = true;
        sample(&mut s, "1234", 29_200, Some((208, 2)));
        sample(&mut s, "1234", 29_700, Some((208, 2)));
        s.finished(first.clone());
        s.finished(later.clone());
        let mut slots = Vec::new();
        apply_completions(&mut s, 90_000, |key, slot, _| {
            slots.push((key.to_owned(), slot));
            Ok(())
        });
        assert_eq!(slots, [(first.draw_key, 2), (later.draw_key, 2)]);
    }

    #[test]
    fn stale_panel_or_overlapping_receipts_do_not_write() {
        let f = completion();
        let mut s = Session::default();
        sample(&mut s, "1234", 27_500, Some((208, 2)));
        sample(&mut s, "1234", 28_000, Some((208, 2)));
        s.finished(f.clone());
        s.finished(Completion {
            draw_key: "other".into(),
            receipt: f.receipt,
            ..f.clone()
        });
        apply_completions(&mut s, 90_000, |_, _, _| panic!("歧义窗口不得写入"));
        sample(&mut s, "1234", 22_000, Some((208, 2)));
        assert_eq!(s.evidence_window(&f), None);
    }

    #[test]
    fn same_second_ambiguous_draws_are_empty_but_still_cut_off_the_prior_draw() {
        let first = completion();
        let ambiguous_a = Completion {
            receipt: first.receipt + 2_000,
            draw_key: "trial-draw:1234:29000:a".into(),
            ..first.clone()
        };
        let ambiguous_b = Completion {
            draw_key: "trial-draw:1234:29000:b".into(),
            ..ambiguous_a.clone()
        };
        let mut s = Session::default();
        sample(&mut s, "1234", 27_200, Some((208, 4)));
        sample(&mut s, "1234", 27_700, Some((208, 4)));
        sample(&mut s, "1234", 29_200, Some((208, 1)));
        sample(&mut s, "1234", 29_700, Some((208, 1)));
        s.finished(first.clone());
        s.finished(ambiguous_a);
        s.finished(ambiguous_b);
        let mut saved = Vec::new();
        apply_completions(&mut s, 90_000, |key, slot, _| {
            saved.push((key.to_owned(), slot));
            Ok(())
        });
        assert_eq!(saved, [(first.draw_key, 4)]);
    }

    #[test]
    fn next_battle_boundary_never_lends_its_frames_to_the_old_draw() {
        let f = Completion {
            next_boundary: Some(28_000),
            ..completion()
        };
        let mut s = Session::default();
        sample(&mut s, "1234", 28_200, Some((208, 5)));
        sample(&mut s, "1234", 28_700, Some((208, 5)));
        s.finished(f);
        apply_completions(&mut s, 90_000, |_, _, _| {
            panic!("下一场战斗的牌面不得写给前一轮")
        });
    }

    #[test]
    fn late_nearby_receipt_rechecks_an_automatic_observation_with_retained_frames() {
        let first = completion();
        let later = Completion {
            receipt: first.receipt + 2_000,
            draw_key: "trial-draw:1234:29000:late".into(),
            ..first.clone()
        };
        let mut s = Session::default();
        sample(&mut s, "1234", 27_200, Some((208, 2)));
        sample(&mut s, "1234", 27_700, Some((208, 2)));
        s.finished(first.clone());
        let mut saved = Vec::new();
        apply_completions(&mut s, 88_000, |key, slot, _| {
            saved.push((key.to_owned(), slot));
            Ok(())
        });
        assert_eq!(saved, [(first.draw_key.clone(), 2)]);

        // The late receipt causes DB to revoke an overlapping automatic
        // observation. Capture must not leave that row permanently blank
        // when the old two frames remain valid after window splitting.
        sample(&mut s, "1234", 29_200, Some((208, 3)));
        sample(&mut s, "1234", 29_700, Some((208, 3)));
        s.finished(later.clone());
        apply_completions(&mut s, 90_000, |key, slot, _| {
            saved.push((key.to_owned(), slot));
            Ok(())
        });
        assert_eq!(
            saved,
            [
                (first.draw_key.clone(), 2),
                (first.draw_key, 2),
                (later.draw_key, 3),
            ]
        );
    }

    #[test]
    fn late_receipt_survives_prune_while_pending_and_rehydrates_persisted_frames() {
        let f = completion();
        let mut s = Session::default();
        sample(&mut s, "1234", 27_500, Some((208, 2)));
        sample(&mut s, "1234", 28_000, Some((208, 2)));
        s.finished(f.clone());
        // 封存式落盘：回执已等了很久才被扫描到，期间 prune 会用 now 裁剪旧样本。
        // 挂起的终局必须把保留水位压到自己的终局时间，绝不能裁掉 pending。
        s.prune(1_800_000);
        assert!(!s.pending.is_empty(), "迟到回执不得被 prune 丢弃");
        assert_eq!(s.selected_slot(&f, s.evidence_window(&f).unwrap()), Some((208, 2)));
        let mut saved = Vec::new();
        apply_completions(&mut s, 1_800_000, |key, slot, _| {
            saved.push((key.to_string(), slot));
            Ok(())
        });
        assert_eq!(saved, vec![(f.draw_key, 2)]);
    }

    #[test]
    fn prune_drops_everything_after_active_work_is_settled() {
        let mut s = Session::default();
        sample(&mut s, "1234", 27_500, Some((208, 2)));
        s.prune(1_800_000);
        assert!(s.samples.is_empty(), "没有挂起工作时样本仍按墙钟裁剪");
    }

    /// 用外部提供的去标题栏 BGRA 原图回放识别与写库，不接触游戏进程或用户数据库。
    #[test]
    #[ignore = "requires TRIAL_CARD_BGRA and TRIAL_CARD_WIDTH for the slot-2 screenshot"]
    fn replay_slot_two_screenshot_to_isolated_trial_record() {
        let bytes = std::fs::read(std::env::var("TRIAL_CARD_BGRA").unwrap()).unwrap();
        let width: usize = std::env::var("TRIAL_CARD_WIDTH").unwrap().parse().unwrap();
        let height = bytes.len() / (width * 4);
        assert_eq!(bytes.len(), width * height * 4);
        let slot = vision::flipped_slot(&bytes, width, height).expect("应识别到奖励牌的金框");
        assert_eq!(slot, 2);
        let f = completion();
        let mut s = Session::default();
        for at in [27_500, 28_000] {
            sample(&mut s, "1234", at, Some((208, slot)));
        }
        s.finished(f.clone());
        let mut recorded = Vec::new();
        apply_completions(&mut s, 88_000, |key, detected, _| {
            recorded.push((key.to_owned(), detected));
            Ok(())
        });
        assert_eq!(recorded, [(f.draw_key, 2)]);
    }
}
