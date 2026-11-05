use super::*;

#[test]
fn a_late_receipt_revokes_frames_saved_after_its_timestamp() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch("CREATE TABLE trial_records(id TEXT PRIMARY KEY, source TEXT, source_key TEXT); CREATE TABLE trial_auto_scan_tombstones(source_key TEXT PRIMARY KEY); INSERT INTO trial_records VALUES ('run','auto','run');").unwrap();
    crate::db::ensure_trial_draws_table(&conn).unwrap();
    let draw = |key: &str, at| crate::db::AutoTrialDraw {
        source_key: key.into(),
        receipt_time: at,
        receipt_hash: Some(key.into()),
        reward_items: vec![],
        reward_source_hashes: vec![],
        stamina_cost: None,
        stamina_costs: vec![],
        stamina_source_hashes: vec![],
    };
    crate::db::db_upsert_auto_trial_draws_with_connection(
        &mut conn,
        "run",
        &[draw("first", 27_000)],
    )
    .unwrap();
    let target = Target {
        uid: "late".into(),
        map: 144,
    };
    let mut session = Session::default();
    session.finished(Completion {
        target: target.clone(),
        run_key: "run".into(),
        draw_key: "first".into(),
        terminal: 24_000,
        receipt: 27_000,
        next_boundary: None,
        retry_after: 0,
    });
    // 首笔画面缺失，第二笔日志尚未到达，暂时只有第二个面板的画面。
    for at in [29_200, 29_450] {
        session.samples.push_back(Sample {
            target: target.clone(),
            at,
            selected: Some((42, 3)),
            reset_visible: false,
            equipment: Some((
                3,
                catalog::TooltipEvidence {
                    name: "测试装备".into(),
                    level: 1050,
                    bind: catalog::TooltipBind::Bound,
                },
            )),
        });
    }
    apply_completions(&mut session, 90_000, |key, slot, window| {
        assert_eq!(window.start, 26_000, "撤销边界不能被收窄到首个稳定帧29.2秒");
        crate::db::db_apply_auto_trial_draw_flip_with_connection(
            &mut conn,
            key,
            slot.into(),
            window.start,
            window.end,
        )?;
        Ok(())
    });
    apply_equipment_reviews(&mut session, 90_000, |finish, slot, window, _| {
        assert_eq!(window.start, 26_000);
        crate::db::db_apply_auto_trial_draw_equipment_with_connection(
            &mut conn,
            &finish.draw_key,
            slot.into(),
            3,
            "51426",
            window.start,
            window.equipment_end,
        )?;
        Ok(())
    });
    let saved: (i64, String) = conn
        .query_row(
            "SELECT flipped_index,card_3 FROM trial_draws WHERE source_key='first'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(saved, (3, "51426".into()));
    crate::db::db_upsert_auto_trial_draws_with_connection(
        &mut conn,
        "run",
        &[draw("later", 29_000)],
    )
    .unwrap();
    let revoked: (i64, String) = conn
        .query_row(
            "SELECT flipped_index,card_3 FROM trial_draws WHERE source_key='first'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        revoked,
        (0, String::new()),
        "迟到第二笔必须撤销第一笔可能错归的牌位与装备"
    );
}

#[test]
fn backs_switch_to_fast_sampling_and_animation_keeps_the_fast_tail() {
    let mut schedule = CaptureSchedule::default();
    assert_eq!(schedule.mode(10_000), window::CaptureMode::Cards);
    assert_eq!(schedule.interval(10_000), Duration::from_secs(1));
    let backs = vision::synthetic_settlement(1280, 720, &[]);
    assert!(vision::cards_visible(&backs, 1280, 720));
    assert!(vision::card_observation(&backs, 1280, 720, None).is_none());
    schedule.observed(10_000, true, false);
    assert_eq!(schedule.interval(10_000), Duration::from_millis(250));
    schedule.observed(10_250, false, false);
    assert_eq!(schedule.interval(12_999), Duration::from_millis(250));
    assert_eq!(schedule.interval(13_001), Duration::from_secs(1));
    schedule.observed(14_000, true, true);
    assert_eq!(schedule.mode(14_000), window::CaptureMode::Full);
    schedule.captured(window::CaptureMode::Full, 14_000);
    assert_eq!(schedule.mode(14_250), window::CaptureMode::Cards);
    assert_eq!(schedule.mode(14_500), window::CaptureMode::Full);
    schedule.observed(14_250, true, false);
    assert_eq!(schedule.mode(14_750), window::CaptureMode::Cards);
    let mut cold = CaptureSchedule::default();
    cold.recheck_uncertain_frame(20_000);
    assert_eq!(cold.interval(20_000), Duration::from_millis(250));
    assert_eq!(cold.mode(20_000), window::CaptureMode::Cards);
}

#[test]
fn startup_and_slow_frames_have_bounded_work_without_catchup() {
    let mut schedule = CaptureSchedule::default();
    assert_eq!(
        schedule.delay(10_000, true, Duration::from_millis(4)),
        Duration::from_millis(500)
    );
    schedule.observed(10_000, true, false);
    assert_eq!(
        schedule.delay(10_000, false, Duration::from_millis(4)),
        Duration::from_millis(250)
    );
    assert_eq!(
        schedule.delay(10_000, false, Duration::from_millis(80)),
        Duration::from_millis(720)
    );
    assert_eq!(
        schedule.delay(10_000, true, Duration::from_millis(80)),
        Duration::from_millis(720)
    );
}

#[test]
fn adaptive_sampling_recognizes_four_draws_with_dense_frames_at_different_poll_phases() {
    let width = 1280;
    let height = 720;
    let backs = vision::synthetic_selector_panel(width, height, &[]);
    let mut animation = backs.clone();
    for y in height * 32 / 100..height * 59 / 100 {
        for x in 0..width {
            let offset = (y * width + x) * 4;
            animation[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    assert!(!vision::cards_visible(&animation, width, height));
    assert_eq!(
        vision::selector_visible(&animation, width, height, false),
        Some(true)
    );
    // 按真实样本的 20 秒、2 秒、5 秒间隔回放，并覆盖连续翻到同一位置。
    let receipts = [27_000, 47_000, 49_000, 54_000];
    let slots = [2, 4, 4, 1];
    let faces = slots.map(|slot| vision::synthetic_selector_panel(width, height, &[slot]));
    let empty = vec![0; width * height * 4];
    for ui_delay in [0, 300, 1500] {
        for phase in [0, 137, 499, 997] {
            let mut session = Session::default();
            let target = Target {
                uid: "sampling".into(),
                map: 144,
            };
            let mut schedule = CaptureSchedule::default();
            let mut presence = presence::CapturePresence::default();
            presence.activity(1_000, Some(target.uid.clone()), 23_000);
            presence.scene(&target.uid, 23_000, true);
            // 最终通关前不截图，并覆盖选择器晚于日志出现的启动竞态。
            assert!(!presence.allows(&target, 23_000));
            assert!(presence.cleared(&target, 24_000, 24_000));
            let mut now = 24_000 + phase;
            while now < 56_000 {
                assert!(presence.allows(&target, now), "连续翻牌期间不能停止采集");
                let current = receipts.iter().rposition(|at| now >= *at + 200);
                let pixels = if now < 24_000 + ui_delay {
                    &empty
                } else if receipts.iter().any(|at| (*at..*at + 200).contains(&now)) {
                    &animation
                } else {
                    match current {
                        Some(index) if index == 3 || now < receipts[index + 1] - 600 => {
                            &faces[index]
                        }
                        _ => &backs,
                    }
                };
                let observation = vision::card_observation(pixels, width, height, None);
                let panel_visible =
                    observation.is_some() || vision::cards_visible(pixels, width, height);
                let reset_visible =
                    observation.is_none() && vision::card_backs_visible(pixels, width, height);
                let selector = vision::selector_visible(pixels, width, height, panel_visible);
                schedule.observed(now, selector == Some(true), false);
                presence.frame(now, selector.into());
                session.samples.push_back(Sample {
                    target: target.clone(),
                    at: now,
                    selected: observation.map(|card| (42, card.selected_slot)),
                    reset_visible,
                    equipment: None,
                });
                // 包含一次取帧/分析的模拟耗时；OCR 使用独立 worker，不计入此等待。
                now += schedule
                    .delay(now, presence.is_starting(now), Duration::from_millis(40))
                    .as_millis() as i64
                    + 40;
            }
            presence.frame(57_000, presence::CaptureFrame::Absent);
            assert!(!presence.allows(&target, 57_000), "确认关闭的第一帧即停止");
            presence.boundary(&target.uid, 62_000);
            let retained_samples = session.samples.len();
            let completions: Vec<_> = receipts
                .iter()
                .enumerate()
                .map(|(i, receipt)| Completion {
                    target: target.clone(),
                    run_key: "run".into(),
                    draw_key: format!("draw-{i}"),
                    terminal: 24_000,
                    receipt: *receipt,
                    next_boundary: None,
                    retry_after: 0,
                })
                .collect();
            for finish in &completions {
                let allow_watch =
                    presence.draw(&finish.target, finish.terminal, finish.receipt, 65_000);
                assert!(!allow_watch, "离场后的旧回执不得唤醒截图");
                session.finished_with_watch(finish.clone(), allow_watch);
            }
            assert!(session.watch.is_none(), "迟到回执只关联已经采集的证据");
            assert_eq!(session.samples.len(), retained_samples);
            let mut saved = Vec::new();
            apply_completions(&mut session, 120_000, |key, slot, _| {
                saved.push((key.to_owned(), slot));
                Ok(())
            });
            assert_eq!(
                saved,
                [
                    ("draw-0".into(), 2),
                    ("draw-1".into(), 4),
                    ("draw-2".into(), 4),
                    ("draw-3".into(), 1)
                ],
                "poll phase {phase}, ui delay {ui_delay}"
            );
            for finish in completions {
                session.finished_with_watch(finish, false);
            }
            apply_completions(&mut session, 121_000, |_, _, _| {
                panic!("重复扫描不能再写一次")
            });
        }
    }
}

#[test]
fn late_clearance_detection_does_not_assign_the_last_card_to_missed_earlier_draws() {
    let target = Target {
        uid: "late-clearance".into(),
        map: 144,
    };
    let mut presence = presence::CapturePresence::default();
    presence.activity(1_000, Some(target.uid.clone()), 23_000);
    presence.scene(&target.uid, 23_000, true);
    presence.boundary(&target.uid, 23_000);
    assert!(!presence.allows(&target, 23_000));
    // 通关在24秒发生，但扫描55秒才发现；已错过前三张及换牌过渡，
    // 只有最后一副牌仍可见，无法证明它对应哪次回执。
    assert!(presence.cleared(&target, 24_000, 55_000));
    let mut session = Session::default();
    for at in [55_000, 55_250] {
        session.samples.push_back(Sample {
            target: target.clone(),
            at,
            selected: Some((42, 1)),
            reset_visible: false,
            equipment: None,
        });
    }
    for (index, receipt) in [27_000, 47_000, 49_000, 54_000].iter().enumerate() {
        session.finished_with_watch(
            Completion {
                target: target.clone(),
                run_key: "run".into(),
                draw_key: format!("draw-{index}"),
                terminal: 24_000,
                receipt: *receipt,
                next_boundary: None,
                retry_after: 0,
            },
            false,
        );
    }
    let mut saved = Vec::new();
    apply_completions(&mut session, 120_000, |key, slot, _| {
        saved.push((key.to_owned(), slot));
        Ok(())
    });
    assert!(
        saved.is_empty(),
        "缺少换牌过渡时保持未知，不能借最后一副牌猜测任何一次翻牌"
    );
}
