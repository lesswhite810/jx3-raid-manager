//! 将结算页中已定位的装备 Tooltip 绑定到同一轮的具体牌位。
//!
//! 本模块只关联顺序采样，不做 OCR 或数据库写入。鼠标停在第 3 张而金框位于第
//! 2 张时，它返回 `(3, evidence)`，由外层按金框第 2 张判断哪一件才是实际获得。

use std::collections::VecDeque;

use super::{catalog::TooltipEvidence, Completion, EvidenceWindow, Sample};

pub(super) const LATE_TOOLTIP_WINDOW_MS: i64 = 45_000;
const CONTIGUOUS_FRAME_GAP_MS: i64 = 1_500;
const STABLE_EVIDENCE_MIN_GAP_MS: i64 = 200;

fn bounds(finish: &Completion, window: EvidenceWindow) -> Option<(i64, i64, i64)> {
    if finish.receipt < finish.terminal
        || window.end < window.start
        || window.equipment_end < window.start
    {
        return None;
    }
    let base_start = window.start;
    let base_end = window
        .end
        .min(finish.receipt.saturating_add(super::REWARD_WINDOW_MS));
    // `window` is already exclusive of neighbouring receipts. A later
    // tooltip is useful only while this same panel remains in that window.
    let extended_end = window.equipment_end;
    (extended_end >= base_start).then_some((base_start, base_end, extended_end))
}

fn belongs_to_panel(sample: &Sample, finish: &Completion, selected: (u32, u8)) -> bool {
    sample.target == finish.target && sample.selected == Some(selected)
}

/// 提取同一张结算面板中稳定的“第 N 张牌展示这件装备”证据。
///
/// Tooltip 必须连续两帧一致，间隔 200 至 1500 毫秒。第一个符合条件的面板必须
/// 出现在入包前后 8 秒内，随后可在未中断的相同金框面板中延后查看 45 秒。角色、
/// PID、选中金框、采样连续性、下一笔入包或下一轮终局任一条件失效时，后续帧不再
/// 参与本轮。每张牌只返回一条证据，OCR 结果相互矛盾时该牌被放弃。
pub(super) fn resolve_card_equipment(
    samples: &VecDeque<Sample>,
    finish: &Completion,
    selected: (u32, u8),
    window: EvidenceWindow,
) -> (Vec<(u8, TooltipEvidence)>, bool) {
    let Some((base_start, base_end, extended_end)) = bounds(finish, window) else {
        return (Vec::new(), false);
    };
    if !(1..=5).contains(&selected.1) {
        return (Vec::new(), false);
    }

    let mut started = false;
    let mut previous_at = None;
    let mut previous_hover = None;
    let mut observed = [None; 5];
    let mut stable = [false; 5];
    let mut conflicted = [false; 5];
    let mut panel_open = false;

    for sample in samples
        .iter()
        .filter(|sample| (base_start..=extended_end).contains(&sample.at))
    {
        if !belongs_to_panel(sample, finish, selected)
            || previous_at.is_some_and(|previous| {
                !(0..=CONTIGUOUS_FRAME_GAP_MS).contains(&(sample.at - previous))
            })
        {
            // 一旦已进入可信面板，后续样本是另一张面板或空画面，不可重新开始。
            if started {
                panel_open = false;
                break;
            }
            previous_at = None;
            continue;
        }
        if sample.at > base_end && !started {
            break;
        }
        started = true;
        panel_open = true;
        previous_at = Some(sample.at);

        let Some((card_slot, evidence)) = sample.equipment.as_ref() else {
            previous_hover = None;
            continue;
        };
        if !(1..=5).contains(card_slot) {
            previous_hover = None;
            continue;
        }
        let idx = usize::from(*card_slot - 1);
        conflicted[idx] |= observed[idx].is_some_and(|known| known != evidence);
        observed[idx] = Some(evidence);
        stable[idx] |= previous_hover.is_some_and(|(slot, at)| {
            slot == *card_slot
                && (STABLE_EVIDENCE_MIN_GAP_MS..=CONTIGUOUS_FRAME_GAP_MS)
                    .contains(&(sample.at - at))
        });
        previous_hover = Some((*card_slot, sample.at));
    }

    (
        observed
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| stable[*idx] && !conflicted[*idx])
            .filter_map(|(idx, item)| item.map(|e| (idx as u8 + 1, e.clone())))
            .collect(),
        panel_open,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> super::super::Target {
        super::super::Target {
            uid: "role-1".into(),
            map: 144,
        }
    }

    fn evidence(name: &str) -> TooltipEvidence {
        TooltipEvidence {
            name: name.into(),
            level: 1050,
            bind: super::super::catalog::TooltipBind::Bound,
        }
    }

    fn sample(
        at: i64,
        selected: Option<(u32, u8)>,
        equipment: Option<(u8, TooltipEvidence)>,
    ) -> Sample {
        Sample {
            target: target(),
            at,
            selected,
            reset_visible: false,
            equipment,
        }
    }

    fn completion() -> Completion {
        Completion {
            target: target(),
            run_key: "trial:role-1".into(),
            draw_key: "trial-draw:role-1:12".into(),
            terminal: 10_000,
            receipt: 12_000,
            next_boundary: None,
            retry_after: 0,
        }
    }

    fn window() -> EvidenceWindow {
        EvidenceWindow {
            start: 11_000,
            end: 20_000,
            equipment_end: 57_000,
        }
    }

    #[test]
    fn two_brief_hovers_separated_by_a_blank_frame_are_not_stable() {
        let samples = VecDeque::from([
            sample(12_000, Some((42, 2)), Some((2, evidence("甲")))),
            sample(12_500, Some((42, 2)), None),
            sample(13_000, Some((42, 2)), Some((2, evidence("甲")))),
        ]);
        assert!(
            resolve_card_equipment(&samples, &completion(), (42, 2), window())
                .0
                .is_empty()
        );
    }

    #[test]
    fn records_hovered_card_without_calling_it_the_flipped_card() {
        let samples = VecDeque::from([
            sample(11_900, Some((42, 2)), Some((3, evidence("无观项链·内·荒")))),
            sample(12_400, Some((42, 2)), Some((3, evidence("无观项链·内·荒")))),
        ]);
        assert_eq!(
            resolve_card_equipment(&samples, &completion(), (42, 2), window()).0,
            vec![(3, evidence("无观项链·内·荒"))]
        );
    }

    #[test]
    fn accepts_late_hover_only_when_panel_has_stayed_open() {
        let samples = VecDeque::from([
            sample(12_000, Some((42, 2)), None),
            sample(13_000, Some((42, 2)), None),
            sample(14_000, Some((42, 2)), None),
            sample(15_000, Some((42, 2)), None),
            sample(16_000, Some((42, 2)), None),
            sample(17_000, Some((42, 2)), None),
            sample(18_000, Some((42, 2)), None),
            sample(19_000, Some((42, 2)), None),
            sample(20_000, Some((42, 2)), None),
            // 已超过入包后 8 秒，仍因同一面板连续存在而被接受。
            sample(21_000, Some((42, 2)), Some((3, evidence("无观项链·内·荒")))),
            sample(21_500, Some((42, 2)), Some((3, evidence("无观项链·内·荒")))),
        ]);
        assert_eq!(
            resolve_card_equipment(&samples, &completion(), (42, 2), window())
                .0
                .len(),
            1
        );
    }

    #[test]
    fn broken_panel_never_restarts_for_late_hovering() {
        let samples = VecDeque::from([
            sample(12_000, Some((42, 2)), None),
            sample(13_000, None, None),
            sample(13_500, Some((42, 2)), Some((3, evidence("甲")))),
            sample(14_000, Some((42, 2)), Some((3, evidence("甲")))),
        ]);
        assert!(
            resolve_card_equipment(&samples, &completion(), (42, 2), window())
                .0
                .is_empty()
        );
    }

    #[test]
    fn rejects_pid_slot_and_ocr_conflicts_and_obeys_next_boundary() {
        let samples = VecDeque::from([
            sample(12_000, Some((99, 2)), Some((2, evidence("甲")))),
            sample(12_500, Some((42, 2)), Some((2, evidence("甲")))),
            sample(13_000, Some((42, 2)), Some((2, evidence("乙")))),
        ]);
        assert!(
            resolve_card_equipment(&samples, &completion(), (42, 2), window())
                .0
                .is_empty()
        );

        let next = VecDeque::from([
            sample(12_800, Some((42, 2)), Some((3, evidence("甲")))),
            sample(13_300, Some((42, 2)), Some((3, evidence("甲")))),
        ]);
        assert!(resolve_card_equipment(
            &next,
            &completion(),
            (42, 2),
            EvidenceWindow {
                start: 11_000,
                end: 12_999,
                equipment_end: 12_999,
            },
        )
        .0
        .is_empty());
    }
}
