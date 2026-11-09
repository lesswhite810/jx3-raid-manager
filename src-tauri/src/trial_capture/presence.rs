//! 文件信号只证明近期活动，不能当成实时地图。采集须同时通过角色、最新 JCL
//! 和最终通关门禁；战斗/第一二关只关闭旧结算。停止采集不删除结构化翻牌证据。
use super::Target;
use std::collections::HashMap;

const ACTIVITY_LEASE_MS: i64 = 90_000;
const EVENT_LEASE_MS: i64 = 90_000;
const VISUAL_UNCERTAINTY_MS: i64 = 1_000;
const STARTUP_GRACE_MS: i64 = 2_000;

pub(super) enum CaptureFrame {
    Present,
    Absent,
    Uncertain,
    Unavailable,
}

impl From<Option<bool>> for CaptureFrame {
    fn from(selector: Option<bool>) -> Self {
        match selector {
            Some(true) => Self::Present,
            Some(false) => Self::Absent,
            None => Self::Uncertain,
        }
    }
}

#[derive(Default)]
pub(super) struct CapturePresence {
    activity_at: i64,
    process_start: i64,
    uid: Option<String>,
    scene: Option<(i64, bool)>,
    clearance: Option<(Target, i64)>,
    battle_boundary: Option<i64>,
    target: Option<Target>,
    event_until: i64,
    panel_seen: bool,
    bootstrap_until: Option<i64>,
    uncertain_since: Option<i64>,
    closed: HashMap<String, i64>,
}

impl CapturePresence {
    fn stop(&mut self, at: i64) {
        if let Some(target) = self.target.take() {
            self.closed
                .entry(target.uid)
                .and_modify(|old| *old = (*old).max(at))
                .or_insert(at);
        }
        self.panel_seen = false;
        self.bootstrap_until = None;
        self.uncertain_since = None;
    }

    /// 已确认选择器消失/进程退出后退休本次终局；同一轮的迟到奖励只关联旧证据。
    pub(super) fn end_capture(&mut self, at: i64) {
        self.stop(at);
        self.clearance = None;
    }

    pub(super) fn activity(&mut self, start: i64, uid: Option<String>, at: i64) {
        if at < self.activity_at {
            return;
        }
        if self.process_start != start || self.uid != uid {
            self.stop(at);
            self.scene = None;
            self.clearance = None;
            self.battle_boundary = None;
        }
        if start > 0 && self.process_start != start {
            // 新进程首批日志通常早于本次检测时刻。同 UID 的旧进程关闭水位
            // 不能吞掉它们；process_start 下界已经排除了上一进程的事件。
            self.closed.clear();
        }
        self.activity_at = at;
        self.process_start = start;
        self.uid = uid;
        // 旧事件本来就不能启动采集，水位只需覆盖仍可能重扫的观察窗口。
        self.closed
            .retain(|_, closed| at.saturating_sub(*closed) <= super::WATCH_MS);
    }

    pub(super) fn scene(&mut self, uid: &str, at: i64, trial: bool) {
        if self.uid.as_deref() != Some(uid) {
            return;
        }
        if at == 0 {
            // 慢扫描可能在快速通关前枚举了空快照，之后才投递此消息。
            // 未知不是新的离场证据；角色/进程切换已由activity清理，不能在这里
            // 撤销快通道已核验的较新场景与终局。
            return;
        }
        if at < self.process_start {
            return;
        }
        if let Some((previous, was_trial)) = self.scene {
            // 同秒出现不同场景，保守采用非试炼；迟到旧扫描不能覆盖新场景。
            if at < previous || (at == previous && !was_trial) {
                return;
            }
        }
        self.scene = Some((at, trial));
        if !trial {
            self.stop(at);
            self.clearance = None;
        } else if self
            .clearance
            .as_ref()
            .is_some_and(|(_, terminal)| at > *terminal)
        {
            // 新一场试炼战斗也不能继承上一轮已打开的结算观察。
            self.boundary(uid, at);
        }
    }

    /// 战斗和第一、二关只推进停止边界，绝不取得采集资格。
    pub(super) fn boundary(&mut self, uid: &str, at: i64) {
        if self.uid.as_deref() != Some(uid) || at < self.process_start {
            return;
        }
        self.battle_boundary = Some(self.battle_boundary.unwrap_or(0).max(at));
        if self
            .clearance
            .as_ref()
            .is_some_and(|(_, terminal)| at > *terminal)
        {
            self.stop(at);
            self.clearance = None;
            self.closed
                .entry(uid.to_owned())
                .and_modify(|old| *old = (*old).max(at))
                .or_insert(at);
        }
    }

    /// 先登记经过日志核验的终局，再独立判断是否足够新鲜，可以开始截图。
    /// 较早的终局仍可供当前新到的追加翻牌匹配，但不能单独开启采集。
    pub(super) fn cleared(&mut self, target: &Target, terminal: i64, now: i64) -> bool {
        if !self.eligible(target, now)
            || terminal < self.process_start
            || !(now.saturating_sub(super::WATCH_MS)..=now).contains(&terminal)
            || self.scene.is_none_or(|(at, _)| terminal < at)
            || self.battle_boundary.is_some_and(|at| terminal < at)
            || self
                .clearance
                .as_ref()
                .is_some_and(|(_, previous)| terminal < *previous)
        {
            self.log_rejection("终局", target, terminal, now);
            return false;
        }
        if self
            .clearance
            .as_ref()
            .is_none_or(|(old, at)| old != target || *at != terminal)
        {
            if self
                .closed
                .get(&target.uid)
                .is_some_and(|at| terminal <= *at)
            {
                return false;
            }
            self.clearance = Some((target.clone(), terminal));
            self.target = None;
            self.panel_seen = false;
            self.bootstrap_until = None;
            self.uncertain_since = None;
            self.event_until = 0;
        }
        self.admit_event(target, terminal, now, true)
    }

    pub(super) fn draw(&mut self, target: &Target, terminal: i64, receipt: i64, now: i64) -> bool {
        if !self
            .clearance
            .as_ref()
            .is_some_and(|(known, at)| known == target && *at == terminal)
            || receipt < terminal
            || receipt > terminal.saturating_add(super::MAX_RECEIPT_DELAY_MS)
        {
            self.log_rejection("翻牌回执", target, receipt, now);
            return false;
        }
        self.admit_event(target, receipt, now, false)
    }

    /// 统一的准入拒绝诊断。只记录事件级失败（终局/翻牌回执被拒），
    /// 不用于逐帧轮询（`allows` 每 tick 调用，避免日志风暴）。
    fn log_rejection(&self, kind: &str, target: &Target, event_at: i64, now: i64) {
        if !self.eligible(target, now) {
            let reason = if self.process_start == 0 {
                "未知游戏进程"
            } else if self.uid.as_deref() != Some(target.uid.as_str()) {
                "角色不匹配"
            } else if !self.scene.is_some_and(|(_, trial)| trial) {
                "当前不在试炼场景"
            } else {
                "活动租约过期(>90s)"
            };
            log::info!(
                "[TrialCapture] {kind}准入被拒({reason}): event_at={event_at}, now={now}, uid={}",
                target.uid
            );
            return;
        }
        if !(now.saturating_sub(EVENT_LEASE_MS)..=now).contains(&event_at) {
            log::info!(
                "[TrialCapture] {kind}事件时间超出 90 秒租约被拒: event_at={event_at}, now={now}, 延迟 {}ms, uid={}",
                now.saturating_sub(event_at),
                target.uid
            );
        }
    }

    fn eligible(&self, target: &Target, now: i64) -> bool {
        self.process_start > 0
            && self.uid.as_deref() == Some(target.uid.as_str())
            && (self.activity_at..=self.activity_at.saturating_add(ACTIVITY_LEASE_MS))
                .contains(&now)
            && self.scene.is_some_and(|(_, trial)| trial)
    }

    fn admit_event(&mut self, target: &Target, event_at: i64, now: i64, bootstrap: bool) -> bool {
        if !self.eligible(target, now)
            || !(now.saturating_sub(EVENT_LEASE_MS)..=now).contains(&event_at)
            || event_at < self.process_start
            || self.scene.is_none_or(|(at, _)| event_at < at)
            || self
                .closed
                .get(&target.uid)
                .is_some_and(|at| event_at <= *at)
        {
            return false;
        }
        if self.target.as_ref() != Some(target) {
            self.target = Some(target.clone());
            self.panel_seen = false;
            // 只允许新鲜终局的首次采集资格等待 UI 出现。追加翻牌和重复扫描
            // 都不得重开或延长这个窗口，避免迟到回执复活已经退休的终局。
            self.bootstrap_until = bootstrap.then_some(now.saturating_add(STARTUP_GRACE_MS));
            self.uncertain_since = None;
            self.event_until = 0;
        }
        // 用事件时间续期，重复扫描旧日志不会推迟到期时间。
        self.event_until = self
            .event_until
            .max(event_at.saturating_add(EVENT_LEASE_MS));
        true
    }

    pub(super) fn allows(&self, target: &Target, now: i64) -> bool {
        self.target.as_ref() == Some(target)
            && self.eligible(target, now)
            && (self.panel_seen || now <= self.event_until)
    }

    /// 新鲜终局刚取得采集资格、但尚未看到有效面板时的短暂启动窗口。
    /// 调用方可据此将首次取样频率降为 500ms，窗口外继续沿用普通调度。
    pub(super) fn is_starting(&self, now: i64) -> bool {
        self.target.as_ref().is_some_and(|target| {
            !self.panel_seen
                && self.eligible(target, now)
                && self.bootstrap_until.is_some_and(|until| now < until)
        })
    }

    pub(super) fn frame(&mut self, now: i64, panel: CaptureFrame) {
        match panel {
            CaptureFrame::Present => {
                self.panel_seen = true;
                self.uncertain_since = None;
            }
            // 新鲜终局可能先于 UI 被 JCL 识别：首次两秒内的有效缺席不退休。
            // 已见面板后，或窗口到期后的任一有效缺席，仍在当帧立即停止。
            CaptureFrame::Absent if !self.is_starting(now) => self.end_capture(now),
            CaptureFrame::Absent => {}
            CaptureFrame::Uncertain => match self.uncertain_since {
                // 必须由后续有效帧复核；不能只按墙钟把失焦当成面板关闭。
                Some(since) if now.saturating_sub(since) >= VISUAL_UNCERTAINTY_MS => {
                    self.end_capture(now);
                }
                None => self.uncertain_since = Some(now),
                _ => {}
            },
            CaptureFrame::Unavailable => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> (CapturePresence, Target) {
        let target = Target {
            uid: "1234".into(),
            map: 144,
        };
        let mut gate = CapturePresence::default();
        gate.activity(1_000, Some(target.uid.clone()), 10_000);
        gate.scene(&target.uid, 5_000, true);
        (gate, target)
    }

    #[test]
    fn no_trial_no_capture_and_old_history_cannot_wake_it() {
        let (mut gate, target) = ready();
        assert!(!gate.allows(&target, 10_000));
        assert!(!gate.cleared(&target, 999, 10_000));
        gate.scene(&target.uid, 9_000, false);
        assert!(!gate.cleared(&target, 10_000, 10_000));
        gate.scene(&target.uid, 8_000, true);
        assert!(!gate.cleared(&target, 10_000, 10_000));
        gate.scene(&target.uid, 9_000, true);
        assert!(!gate.cleared(&target, 10_000, 10_000), "同秒场景不猜测");
    }

    #[test]
    fn repeated_events_do_not_extend_the_no_panel_lease() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.activity(1_000, Some(target.uid.clone()), 90_000);
        assert!(gate.cleared(&target, 10_000, 99_000));
        assert!(gate.allows(&target, 100_000));
        assert!(!gate.allows(&target, 100_001));
        assert!(!gate.cleared(&target, 10_000, 100_001));
        assert!(gate.cleared(&target, 100_000, 100_001));
        assert!(gate.allows(&target, 100_001));
    }

    #[test]
    fn a_slow_multi_draw_panel_outlives_the_event_lease() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        for at in (10_000..800_000).step_by(30_000) {
            gate.activity(1_000, Some(target.uid.clone()), at);
            gate.frame(at, CaptureFrame::Present);
            assert!(gate.allows(&target, at));
        }
    }

    #[test]
    fn selector_absence_stops_on_the_first_frame_and_retires_all_receipts() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        gate.frame(11_000, CaptureFrame::Absent);
        assert!(!gate.allows(&target, 11_000));
        assert!(!gate.draw(&target, 10_000, 12_000, 15_000));
        assert!(!gate.cleared(&target, 10_000, 15_000));
        assert!(!gate.draw(&target, 10_000, 15_000, 15_000));
        assert!(gate.cleared(&target, 15_000, 15_000));
        assert!(gate.allows(&target, 15_000));
    }

    #[test]
    fn failed_capture_does_not_count_as_a_closed_panel() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        gate.frame(11_000, CaptureFrame::Unavailable);
        gate.frame(13_000, CaptureFrame::Unavailable);
        gate.frame(30_000, CaptureFrame::Unavailable);
        assert!(gate.allows(&target, 30_000));
        gate.frame(31_000, CaptureFrame::Present);
        assert!(gate.allows(&target, 31_000));
    }

    #[test]
    fn valid_uncertain_frames_stop_after_a_short_recheck() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        for at in [10_000, 10_250, 10_500, 10_750] {
            gate.frame(at, CaptureFrame::Uncertain);
            assert!(gate.allows(&target, at));
        }
        gate.frame(11_000, CaptureFrame::Uncertain);
        assert!(!gate.allows(&target, 11_000));
        assert!(!gate.draw(&target, 10_000, 12_000, 12_000));
    }

    #[test]
    fn one_uncertain_frame_then_present_resets_the_recheck() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Uncertain);
        gate.frame(10_250, CaptureFrame::Present);
        gate.frame(11_000, CaptureFrame::Uncertain);
        assert!(gate.allows(&target, 11_000));
        gate.frame(11_250, CaptureFrame::Present);
        assert!(gate.allows(&target, 11_250));
    }

    #[test]
    fn unavailable_frames_do_not_retire_an_uncertain_selector_before_valid_recovery() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Uncertain);
        for at in [11_000, 13_000, 30_000] {
            gate.frame(at, CaptureFrame::Unavailable);
            assert!(gate.allows(&target, at));
        }
        gate.frame(31_000, CaptureFrame::Present);
        assert!(gate.allows(&target, 31_000));
        gate.frame(32_000, CaptureFrame::Uncertain);
        gate.frame(34_000, CaptureFrame::Unavailable);
        assert!(gate.allows(&target, 34_000));
        gate.frame(35_000, CaptureFrame::Uncertain);
        assert!(!gate.allows(&target, 35_000));
    }

    #[test]
    fn initial_absence_waits_only_until_the_startup_deadline() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.frame(10_000, CaptureFrame::Absent);
        assert!(gate.allows(&target, 10_000));
        assert!(gate.is_starting(10_000));
        gate.frame(11_999, CaptureFrame::Absent);
        assert!(gate.allows(&target, 11_999));
        gate.frame(12_000, CaptureFrame::Absent);
        assert!(!gate.allows(&target, 12_000));
        assert!(!gate.is_starting(12_000));
        assert!(!gate.cleared(&target, 10_000, 12_250));
        assert!(!gate.draw(&target, 10_000, 10_250, 12_250));
    }

    #[test]
    fn startup_window_captures_a_panel_that_appears_after_a_short_delay() {
        for appeared_at in [10_300, 11_500] {
            let (mut gate, target) = ready();
            assert!(gate.cleared(&target, 10_000, 10_000));
            gate.frame(appeared_at - 1, CaptureFrame::Absent);
            assert!(gate.is_starting(appeared_at - 1));
            gate.frame(appeared_at, CaptureFrame::Present);
            assert!(gate.allows(&target, appeared_at));
            assert!(!gate.is_starting(appeared_at));
            gate.frame(appeared_at + 1, CaptureFrame::Absent);
            assert!(
                !gate.allows(&target, appeared_at + 1),
                "见过面板后的有效缺席必须当帧关闭"
            );
        }
    }

    #[test]
    fn repeated_clearance_and_draw_do_not_extend_or_restart_startup_window() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.frame(11_999, CaptureFrame::Absent);
        assert!(gate.is_starting(11_999));
        assert!(gate.cleared(&target, 10_000, 11_999));
        assert!(gate.draw(&target, 10_000, 11_999, 11_999));
        gate.frame(12_000, CaptureFrame::Absent);
        assert!(!gate.allows(&target, 12_000));
        assert!(!gate.cleared(&target, 10_000, 12_001));
        assert!(!gate.draw(&target, 10_000, 12_000, 12_001));
    }

    #[test]
    fn role_exit_and_cross_day_events_never_keep_or_start_a_bootstrap_window() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        assert!(gate.is_starting(10_000));
        gate.activity(1_000, None, 10_001);
        assert!(!gate.allows(&target, 10_001));
        assert!(!gate.is_starting(10_001));

        let day = 86_400_000;
        gate.activity(day, Some(target.uid.clone()), day + 10_000);
        gate.scene(&target.uid, day + 5_000, true);
        assert!(
            !gate.cleared(&target, 10_000, day + 10_000),
            "跨天旧终局只能登记，不能取得新的截图启动资格"
        );
        assert!(!gate.is_starting(day + 10_000));
    }

    #[test]
    fn a_known_process_exit_stops_without_waiting_for_a_frame_or_activity_poll() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        gate.end_capture(10_250);
        assert!(!gate.allows(&target, 10_250));
        assert!(!gate.draw(&target, 10_000, 11_000, 11_000));
        assert!(!gate.cleared(&target, 10_000, 11_000));
    }

    #[test]
    fn role_exit_and_stale_detection_fail_closed_without_reviving_old_events() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        assert!(!gate.allows(&target, 100_001), "活跃检测中断后不持续采集");
        gate.activity(1_000, None, 11_000);
        assert!(!gate.allows(&target, 11_000));
        gate.activity(1_000, Some(target.uid.clone()), 10_500);
        assert!(!gate.cleared(&target, 11_000, 11_000));
        gate.activity(1_000, Some(target.uid.clone()), 12_000);
        gate.scene(&target.uid, 5_000, true);
        assert!(!gate.cleared(&target, 10_000, 12_000));
        assert!(gate.cleared(&target, 12_000, 12_000));
        gate.activity(0, None, 13_000);
        assert!(!gate.allows(&target, 13_000));
    }

    #[test]
    fn missing_scene_does_not_overwrite_a_late_file_with_a_fake_timestamp() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.scene(&target.uid, 0, false);
        assert!(gate.allows(&target, 10_000));
        gate.scene(&target.uid, 5_000, true);
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.activity(20_000, Some(target.uid.clone()), 30_000);
        gate.scene(&target.uid, 0, false);
        assert!(
            !gate.cleared(&target, 30_000, 30_000),
            "新会话仍必须有可信场景"
        );
    }

    #[test]
    fn a_new_process_accepts_its_first_events_for_the_same_uid() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        for at in [11_000, 12_000, 14_000] {
            gate.frame(at, CaptureFrame::Absent);
        }
        gate.activity(20_000, Some(target.uid.clone()), 30_000);
        gate.scene(&target.uid, 25_000, true);
        assert!(!gate.cleared(&target, 19_000, 30_000));
        assert!(gate.cleared(&target, 25_000, 30_000));
        gate.activity(1_000, Some(target.uid.clone()), 29_000);
        gate.scene(&target.uid, 15_000, false);
        assert!(gate.allows(&target, 30_000), "旧进程迟到检测不能覆盖新会话");
        gate.activity(20_000, None, 31_000);
        gate.activity(20_000, Some(target.uid.clone()), 32_000);
        gate.scene(&target.uid, 25_000, true);
        assert!(
            !gate.cleared(&target, 25_000, 32_000),
            "同进程切角水位仍有效"
        );
    }

    #[test]
    fn battle_and_saved_stages_never_open_capture_even_when_resumed_next_day() {
        let (mut gate, target) = ready();
        for at in [10_000, 20_000, 30_000] {
            gate.activity(1_000, Some(target.uid.clone()), at);
            gate.boundary(&target.uid, at);
            assert!(!gate.allows(&target, at));
            assert!(gate.clearance.is_none());
        }
        let day = 86_400_000;
        gate.activity(day, Some(target.uid.clone()), day + 10_000);
        gate.scene(&target.uid, day + 5_000, true);
        for at in [10_000, 20_000, 30_000, day + 10_000] {
            gate.boundary(&target.uid, at);
            assert!(!gate.allows(&target, day + 10_000));
        }
        assert!(gate.cleared(&target, day + 12_000, day + 12_000));
        assert!(gate.allows(&target, day + 12_000));
    }

    #[test]
    fn new_battle_closes_previous_panel_even_after_focus_loss_and_rejects_late_draws() {
        let (mut gate, target) = ready();
        assert!(gate.cleared(&target, 10_000, 10_000));
        gate.frame(10_000, CaptureFrame::Present);
        gate.frame(11_000, CaptureFrame::Unavailable);
        gate.boundary(&target.uid, 20_000);
        assert!(!gate.allows(&target, 20_000));
        assert!(!gate.cleared(&target, 10_000, 25_000));
        assert!(!gate.draw(&target, 10_000, 21_000, 25_000));
        assert!(gate.cleared(&target, 30_000, 30_000));
        gate.boundary(&target.uid, 20_000);
        assert!(
            gate.allows(&target, 30_000),
            "旧战斗边界不能关闭新的最终通关"
        );
        assert!(!gate.draw(&target, 10_000, 31_000, 31_000));
        assert!(gate.draw(&target, 30_000, 31_000, 31_000));
    }

    #[test]
    fn a_new_trial_jcl_also_closes_previous_clearance() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        gate.scene(&target.uid, 20_000, true);
        assert!(!gate.allows(&target, 20_000));
        assert!(!gate.draw(&target, 10_000, 21_000, 21_000));
    }

    #[test]
    fn a_new_terminal_can_replace_a_closed_panel_without_intermediate_stages() {
        let (mut gate, target) = ready();
        gate.cleared(&target, 10_000, 10_000);
        gate.frame(10_000, CaptureFrame::Present);
        for at in [11_000, 12_000, 14_000] {
            gate.frame(at, CaptureFrame::Absent);
        }
        assert!(!gate.allows(&target, 14_000));
        assert!(
            gate.cleared(&target, 30_000, 30_000),
            "终局不能先作为自己的停止边界"
        );
        assert!(!gate.draw(&target, 10_000, 31_000, 31_000));
        assert!(gate.draw(&target, 30_000, 31_000, 31_000));
    }

    #[test]
    fn a_delayed_additional_draw_must_match_a_known_clearance() {
        let (mut gate, target) = ready();
        assert!(!gate.draw(&target, 10_000, 11_000, 11_000));
        gate.activity(1_000, Some(target.uid.clone()), 110_000);
        assert!(
            !gate.cleared(&target, 10_000, 110_000),
            "旧通关仅登记，不能启动截图"
        );
        assert!(!gate.allows(&target, 110_000));
        assert!(gate.draw(&target, 10_000, 110_000, 110_000));
        assert!(!gate.draw(&target, 11_000, 111_000, 111_000));
    }
}
