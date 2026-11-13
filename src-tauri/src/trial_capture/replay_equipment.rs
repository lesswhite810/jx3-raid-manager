use super::*;

/// 用户提供的原图包含聊天信息，仅由环境变量指向本机截图，不随源码分发。
#[test]
#[ignore = "requires TRIAL_EQUIPMENT_REPLAY_DIR with the two client-area BGRA screenshots"]
fn reward_and_unflipped_screenshots_to_isolated_record() {
    let dir = std::path::PathBuf::from(std::env::var_os("TRIAL_EQUIPMENT_REPLAY_DIR").unwrap());
    let width = 2560;
    let frame = |file, cursor| {
        let pixels = std::fs::read(dir.join(file)).unwrap();
        window::WindowFrame {
            pid: 17,
            width,
            height: pixels.len() / (width * 4),
            cursor,
            pixels,
        }
    };
    let backs = frame("backs.bgra", Some((1060, 662)));
    assert!(vision::card_observation(&backs.pixels, width, backs.height, backs.cursor).is_none());
    assert!(vision::cards_visible(&backs.pixels, width, backs.height));
    assert!(vision::card_backs_visible(
        &backs.pixels,
        width,
        backs.height
    ));
    drop(backs);
    let reward = frame("reward.bgra", Some((1255, 743)));
    assert!(!vision::card_backs_visible(
        &reward.pixels,
        width,
        reward.height
    ));
    let card =
        vision::card_observation(&reward.pixels, width, reward.height, reward.cursor).unwrap();
    assert_eq!((card.selected_slot, card.hovered_slot), (2, Some(3)));
    let mut tooltip = TooltipCache::default();
    let mut session = Session::default();
    let target = Target {
        uid: "replay".into(),
        map: 144,
    };
    let finish = Completion {
        run_key: "replay-run".into(),
        draw_key: "trial-draw:replay:27000".into(),
        target: target.clone(),
        terminal: 24000,
        receipt: 27000,
        next_boundary: None,
        retry_after: 0,
    };
    session.finished(finish.clone());
    for at in [27_000, 27_500, 28_000] {
        session.samples.push_back(Sample {
            target: target.clone(),
            at,
            selected: Some((reward.pid, card.selected_slot)),
            reset_visible: false,
            equipment: tooltip.observe(&reward, &card),
        });
    }
    assert!(
        session.samples.back().unwrap().equipment.is_some(),
        "候选区域 {:?} 未读到装备",
        card.tooltip_rects
    );

    let mut db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(
        r#"CREATE TABLE trial_records(id TEXT PRIMARY KEY);
        CREATE TABLE trial_auto_scan_tombstones(source_key TEXT PRIMARY KEY);
        CREATE TABLE seasons(name TEXT,start_date INTEGER,end_date INTEGER,sort_order INTEGER);
        CREATE TABLE equipments(id TEXT, name TEXT, level INTEGER, bind_type INTEGER, data TEXT);
        INSERT INTO seasons VALUES ('苍生铸世',1,0,1);
        INSERT INTO equipments VALUES ('51426','无观项链·内·荒',1050,3,'{"SubType":4}'),
            ('51468','无观项链·内·荒',1050,2,'{"SubType":4}'),
            ('worn','无观项链·内·荒',985,3,'{"SubType":4}');"#,
    )
    .unwrap();
    crate::db::ensure_trial_draws_table(&db).unwrap();
    db.execute("INSERT INTO trial_records VALUES (?1)", [&finish.run_key])
        .unwrap();
    db.execute(
        "INSERT INTO trial_draws(id,trial_record_id,source_key,receipt_time)
            VALUES ('observed',?1,?2,?3),('later',?1,'later-receipt',120000)",
        rusqlite::params![finish.run_key, finish.draw_key, finish.receipt],
    )
    .unwrap();
    apply_completions(&mut session, 88_000, |key, slot, window| {
        crate::db::db_apply_auto_trial_draw_flip_with_connection(
            &mut db,
            key,
            slot.into(),
            window.start,
            window.end,
        )?;
        Ok(())
    });
    apply_equipment_reviews(&mut session, 88_000, |finish, selected, window, items| {
        assert_eq!(items.len(), 1);
        for (slot, evidence) in items {
            assert_eq!(
                evidence,
                catalog::TooltipEvidence {
                    name: "无观项链·内·荒".into(),
                    level: 1050,
                    bind: catalog::TooltipBind::Bound,
                }
            );
            assert_eq!(finish.draw_key, "trial-draw:replay:27000");
            assert_eq!(selected, 2);
            assert_eq!(slot, 3);
            let item = catalog::match_reward_tooltip(&db, finish.terminal, &evidence)?.unwrap();
            crate::db::db_apply_auto_trial_draw_equipment_with_connection(
                &mut db,
                &finish.draw_key,
                selected.into(),
                slot.into(),
                &item.equipment_id,
                window.start,
                window.equipment_end,
            )?;
        }
        Ok(())
    });
    let rows: Vec<(u8, String, String)> = db.prepare(
        "SELECT flipped_index,card_2,card_3 FROM trial_draws ORDER BY receipt_time"
    ).unwrap().query_map([], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))
        .unwrap().collect::<Result<_, _>>().unwrap();
    assert_eq!(
        rows,
        [
            (2, String::new(), "51426".into()),
            (0, String::new(), String::new())
        ]
    );
}

#[test]
#[ignore = "requires TRIAL_EQUIPMENT_REPLAY_DIR with the real reward BGRA screenshot"]
fn real_tooltip_is_recognized_by_the_bounded_async_worker() {
    let dir = std::path::PathBuf::from(std::env::var_os("TRIAL_EQUIPMENT_REPLAY_DIR").unwrap());
    let pixels = std::fs::read(dir.join("reward.bgra")).unwrap();
    let frame = window::WindowFrame {
        pid: 17,
        width: 2560,
        height: pixels.len() / (2560 * 4),
        cursor: Some((1255, 743)),
        pixels,
    };
    let card =
        vision::card_observation(&frame.pixels, frame.width, frame.height, frame.cursor).unwrap();
    let mut cache = TooltipCache::default();
    let mut worker = async_ocr::TooltipWorker::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    let result = loop {
        if let Some(result) = cache.observe_async(&frame, &card, &mut worker) {
            break result;
        }
        assert!(Instant::now() < deadline, "真实装备异步识别超时");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(result.0, 3);
    assert_eq!(result.1.name, "无观项链·内·荒");
    assert_eq!(result.1.level, 1050);
    assert_eq!(result.1.bind, catalog::TooltipBind::Bound);
    assert_eq!(cache.attempts, 1, "成功后应复用结果，不应重复 OCR");
    assert_eq!(
        cache.observe_async(&frame, &card, &mut worker),
        Some(result)
    );
    assert_eq!(cache.attempts, 1);
}
