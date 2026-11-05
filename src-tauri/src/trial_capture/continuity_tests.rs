use std::collections::VecDeque;

use super::*;

fn target() -> Target {
    Target {
        uid: "role-1".into(),
        map: 144,
    }
}

fn completion(receipt: i64, suffix: &str) -> Completion {
    Completion {
        target: target(),
        run_key: "trial:role-1:144:24000".into(),
        draw_key: format!("trial-draw:role-1:{receipt}:{suffix}"),
        terminal: 24_000,
        receipt,
        next_boundary: None,
        retry_after: 0,
    }
}

fn sample(
    at: i64,
    selected: Option<(u32, u8)>,
    panel_visible: bool,
    equipment: Option<(u8, catalog::TooltipEvidence)>,
) -> Sample {
    Sample {
        target: target(),
        at,
        selected,
        reset_visible: panel_visible && selected.is_none(),
        equipment,
    }
}

fn equipment(name: &str) -> catalog::TooltipEvidence {
    catalog::TooltipEvidence {
        name: name.into(),
        level: 1050,
        bind: catalog::TooltipBind::Bound,
    }
}

fn samples(session: &mut Session, values: &[(i64, Option<(u32, u8)>, bool)]) {
    session.samples = values
        .iter()
        .map(|(at, selected, panel_visible)| sample(*at, *selected, *panel_visible, None))
        .collect::<VecDeque<_>>();
}

fn evidence(session: &Session, finish: &Completion) -> Option<SelectedEvidence> {
    session
        .evidence_window(finish)
        .and_then(|window| session.selected_evidence(finish, window))
}

fn apply(session: &mut Session, now: i64) -> Vec<(String, u8, i64)> {
    let mut saved = Vec::new();
    apply_completions(session, now, |key, slot, window| {
        saved.push((key.to_owned(), slot, window.start));
        Ok(())
    });
    saved
}

#[test]
fn dense_residual_then_card_backs_and_new_slot_binds_each_receipt_once() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    // 250ms 高速采样：第二笔的时间窗会从 28 秒开始，仍能看见第一张牌残留。
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 2)), true),
            (28_250, Some((42, 2)), true),
            (28_750, None, true), // 完整五牌牌背：可信“继续翻牌”过渡。
            (29_200, Some((42, 3)), true),
            (29_450, Some((42, 3)), true),
        ],
    );
    session.finished(first.clone());
    session.finished(later.clone());

    assert_eq!(
        evidence(&session, &first).map(|item| item.selected),
        Some((42, 2))
    );
    assert_eq!(
        evidence(&session, &later).map(|item| item.selected),
        Some((42, 3))
    );
}

#[test]
fn reset_before_a_long_gap_window_is_kept_as_context_without_borrowing_old_frames() {
    let first = completion(27_000, "a");
    let later = completion(47_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            // The card backs are before the receipts' midpoint (37 seconds).
            (32_000, None, true),
            (46_200, Some((42, 2)), true),
            (46_450, Some((42, 2)), true),
        ],
    );
    session.finished(first.clone());
    session.finished(later.clone());

    let saved = apply(&mut session, 110_000);
    assert_eq!(
        saved,
        vec![
            (first.draw_key, 2, 26_000),
            // Association retains its receipt window; the narrow card segment
            // below is only used for the later equipment review.
            (later.draw_key.clone(), 2, 46_000),
        ]
    );
    assert_eq!(
        evidence(&session, &later).map(|item| item.window.start),
        Some(46_200)
    );
}

#[test]
fn mixed_process_ids_in_the_current_window_never_fill_a_draw() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((41, 2)), true),
            (27_450, Some((41, 2)), true),
            (28_000, Some((41, 2)), true),
            (28_250, Some((41, 2)), true),
            (28_750, None, true),
            (29_200, Some((42, 3)), true),
            (29_450, Some((42, 3)), true),
        ],
    );
    session.finished(first);
    session.finished(later.clone());

    assert_eq!(evidence(&session, &later), None);
}

#[test]
fn first_receipt_with_two_different_stable_cards_stays_unknown() {
    let first = completion(27_000, "a");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 3)), true),
            (28_250, Some((42, 3)), true),
        ],
    );
    session.finished(first);

    assert!(apply(&mut session, 90_000).is_empty());
}

#[test]
fn first_receipt_with_two_same_slot_segments_stays_unknown_until_the_later_receipt_arrives() {
    let first = completion(27_000, "a");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, None, true),
            (28_250, Some((42, 2)), true),
            (28_500, Some((42, 2)), true),
        ],
    );
    session.finished(first);

    assert!(apply(&mut session, 90_000).is_empty());
}

#[test]
fn card_backs_allow_same_slot_to_be_a_second_independent_flip() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 2)), true),
            (28_250, Some((42, 2)), true),
            (28_750, None, true),
            (29_200, Some((42, 2)), true),
            (29_450, Some((42, 2)), true),
        ],
    );
    session.finished(first.clone());
    session.finished(later.clone());

    assert_eq!(
        evidence(&session, &first).map(|item| item.selected),
        Some((42, 2))
    );
    assert_eq!(
        evidence(&session, &later).map(|item| item.selected),
        Some((42, 2))
    );
}

#[test]
fn same_slot_without_a_trusted_reset_stays_unknown_for_later_receipt() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 2)), true),
            (28_250, Some((42, 2)), true),
            (29_200, Some((42, 2)), true),
            (29_450, Some((42, 2)), true),
        ],
    );
    session.finished(first.clone());
    session.finished(later.clone());

    assert_eq!(
        evidence(&session, &first).map(|item| item.selected),
        Some((42, 2))
    );
    assert_eq!(evidence(&session, &later), None);
}

#[test]
fn capture_gap_is_not_a_trusted_reset_for_same_slot() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 2)), true),
            (28_250, Some((42, 2)), true),
            (28_750, None, false), // 失焦/遮挡，不能当成牌背。
            (29_200, Some((42, 2)), true),
            (29_450, Some((42, 2)), true),
        ],
    );
    session.finished(first);
    session.finished(later.clone());

    assert_eq!(evidence(&session, &later), None);
}

#[test]
fn different_stable_slot_without_backs_is_still_new_evidence() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_000, Some((42, 2)), true),
            (28_250, Some((42, 2)), true),
            (29_200, Some((42, 3)), true),
            (29_450, Some((42, 3)), true),
        ],
    );
    session.finished(first);
    session.finished(later.clone());

    assert_eq!(
        evidence(&session, &later).map(|item| item.selected),
        Some((42, 3))
    );
}

#[test]
fn single_frame_after_backs_is_not_enough_to_fill_later_draw() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    samples(
        &mut session,
        &[
            (27_200, Some((42, 2)), true),
            (27_450, Some((42, 2)), true),
            (28_750, None, true),
            (29_200, Some((42, 3)), true),
        ],
    );
    session.finished(first);
    session.finished(later.clone());

    assert_eq!(evidence(&session, &later), None);
}

#[test]
fn second_same_slot_equipment_starts_after_the_card_back_reset() {
    let first = completion(27_000, "a");
    let later = completion(29_000, "b");
    let mut session = Session::default();
    session.samples = VecDeque::from([
        sample(27_200, Some((42, 2)), true, None),
        sample(27_450, Some((42, 2)), true, None),
        // Old tooltip must not be permitted to win the later equipment review.
        sample(28_000, Some((42, 2)), true, Some((3, equipment("旧装备")))),
        sample(28_250, Some((42, 2)), true, Some((3, equipment("旧装备")))),
        sample(28_750, None, true, None),
        sample(29_200, Some((42, 2)), true, Some((3, equipment("新装备")))),
        sample(29_450, Some((42, 2)), true, Some((3, equipment("新装备")))),
    ]);
    session.finished(first);
    session.finished(later.clone());
    let selected = evidence(&session, &later).expect("牌背后的同牌位应是第二次翻牌");
    assert!(selected.window.start >= 29_200);
    assert_eq!(
        session_equipment::resolve_card_equipment(
            &session.samples,
            &later,
            selected.selected,
            selected.window,
        )
        .0,
        vec![(3, equipment("新装备"))]
    );
}
