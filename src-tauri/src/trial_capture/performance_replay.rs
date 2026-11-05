//! 本机实拍分析基准。只回放提供的像素，不操作游戏，不等同于游戏 FPS 测试。
use super::*;

#[test]
#[ignore = "requires TRIAL_EQUIPMENT_REPLAY_DIR; run with --release --exact --nocapture"]
fn replay_frame_cost_at_live_cadence() {
    crate::background_work::lower_current_thread_priority();
    let dir = std::path::PathBuf::from(std::env::var_os("TRIAL_EQUIPMENT_REPLAY_DIR").unwrap());
    let mut work = Duration::ZERO;
    let mut max_work = Duration::ZERO;
    let started = Instant::now();
    let mut frames = 0usize;
    let width = 2560;
    for name in ["backs.bgra", "reward.bgra"] {
        let pixels = std::fs::read(dir.join(name)).unwrap();
        let height = pixels.len() / (width * 4);
        for _ in 0..20 {
            let before = Instant::now();
            let card = vision::card_observation(&pixels, width, height, None);
            let visible = card.is_some() || vision::cards_visible(&pixels, width, height);
            let selector = vision::selector_visible(&pixels, width, height, visible);
            let backs =
                card.is_none() && visible && vision::card_backs_visible(&pixels, width, height);
            std::hint::black_box((card, backs));
            assert_eq!(selector, Some(true));
            let cost = before.elapsed();
            work += cost;
            max_work = max_work.max(cost);
            frames += 1;
            let mut schedule = CaptureSchedule::default();
            schedule.observed(1_000, true, false);
            std::thread::sleep(schedule.delay(1_000, false, cost));
        }
    }
    println!(
        "TRIAL_FRAME_REPLAY frames={frames} wall_ms={} work_ms={} max_work_ms={:.3} mean_work_ms={:.3}",
        started.elapsed().as_millis(), work.as_millis(), max_work.as_secs_f64() * 1_000.0,
        work.as_secs_f64() * 1_000.0 / frames as f64,
    );
}
