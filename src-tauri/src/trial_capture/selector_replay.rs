use super::vision;

/// 用户提供的 PNG 仅在本机转换为去掉 23px 标题栏的 BGRA，不随源码分发。
///
/// 这回放和 Cards 截图路径相同：完整客户区缓冲先清黑，仅复制中心 17%..83%、
/// 18%..75% 的区域。这样可验证页眉/底栏并没有依赖未采集的全屏像素。
#[test]
#[ignore = "requires TRIAL_EQUIPMENT_REPLAY_DIR with client-area backs.bgra and reward.bgra"]
fn real_selector_chrome_survives_the_card_flip_animation() {
    let dir = std::path::PathBuf::from(std::env::var_os("TRIAL_EQUIPMENT_REPLAY_DIR").unwrap());
    const WIDTH: usize = 2560;

    for name in ["backs.bgra", "reward.bgra"] {
        let original = std::fs::read(dir.join(name)).unwrap();
        let height = original.len() / (WIDTH * 4);
        let frame = cards_roi_frame(&original, WIDTH, height);
        let cards_present = vision::cards_visible(&frame, WIDTH, height)
            || vision::card_observation(&frame, WIDTH, height, None).is_some();
        assert!(cards_present, "{name} 应保留完整五牌或金框");
        assert_eq!(
            vision::selector_visible(&frame, WIDTH, height, cards_present),
            Some(true),
            "{name}"
        );

        let animation = without_card_row(frame, WIDTH, height);
        assert!(
            !vision::cards_visible(&animation, WIDTH, height),
            "{name} 动画帧不应残留五牌"
        );
        assert_eq!(
            vision::selector_visible(&animation, WIDTH, height, false),
            Some(true),
            "{name} 的页眉和三按钮应在无牌动画帧维持选择器"
        );
    }
}

fn cards_roi_frame(original: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut frame = vec![0; original.len()];
    let left = width * 17 / 100;
    let right = width * 83 / 100;
    let top = height * 18 / 100;
    let bottom = height * 75 / 100;
    for y in top..bottom {
        let start = (y * width + left) * 4;
        let end = (y * width + right) * 4;
        frame[start..end].copy_from_slice(&original[start..end]);
    }
    frame
}

fn without_card_row(mut frame: Vec<u8>, width: usize, height: usize) -> Vec<u8> {
    // 五张牌在客户区 45% 左右，覆盖到约 57%；这段保留 y=18%..32% 页眉和
    // y=59%..65% 底栏，模拟翻转时牌面暂时消失而选择器仍在的实际情况。
    for y in height * 32 / 100..height * 59 / 100 {
        for x in width * 17 / 100..width * 83 / 100 {
            let offset = (y * width + x) * 4;
            frame[offset..offset + 4].copy_from_slice(&[0, 0, 0, 255]);
        }
    }
    frame
}
