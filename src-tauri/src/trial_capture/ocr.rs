//! 试炼奖励悬浮提示的本地 OCR。
//!
//! 本模块只负责将已经定位好的提示框转成可供装备目录匹配的证据。提示框属于哪张
//! 奖励牌、是否已经翻牌均由调用方保留，不能因为读到了 Tooltip 就推断玩家已获得。

use regex::Regex;

use super::{
    catalog::{TooltipBind, TooltipEvidence},
    vision::Rect,
};

#[cfg(target_os = "windows")]
struct WinRtApartment {
    initialized: bool,
}

#[cfg(target_os = "windows")]
impl WinRtApartment {
    fn initialize() -> Self {
        // RoInitialize 的成功值为 S_OK 或 S_FALSE。若线程已在 STA 中，保留其
        // apartment 并继续尝试 OCR，避免错误地执行 RoUninitialize。
        let initialized = unsafe { RoInitialize(1) } >= 0;
        Self { initialized }
    }
}

#[cfg(target_os = "windows")]
impl Drop for WinRtApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { RoUninitialize() };
        }
    }
}

#[cfg(target_os = "windows")]
#[link(name = "runtimeobject")]
unsafe extern "system" {
    fn RoInitialize(init_type: u32) -> i32;
    fn RoUninitialize();
}

/// OCR 并严格解析一个试炼奖励 Tooltip。
///
/// Windows 之外和缺少识别语言包时安全返回 `None`。像素只在调用期间拷贝到
/// WinRT 的 SoftwareBitmap，返回后立即释放，不保存截图或 OCR 原文。
pub(super) fn recognize_trial_tooltip(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    rect: Rect,
) -> Option<TooltipEvidence> {
    if rect.width() < 80 || rect.height() < 80 {
        return None;
    }
    // 主面板的紫色标题会被金色星级、绿色精炼字干扰。单独按颜色切出标题行，
    // 再和正文 OCR 组合，避免把“无观项链·内·荒”拆成不完整的两段。
    let title_rect = purple_title_rect(pixels, window_width, window_height, rect)?;
    let title = recognize_title_text(pixels, window_width, window_height, title_rect)?;
    let body = recognize_text(pixels, window_width, window_height, rect)?;
    let text = format!("{}\n{body}", normalize_line(&title));
    parse_trial_tooltip(&text)
}

fn parse_trial_tooltip(text: &str) -> Option<TooltipEvidence> {
    let lines: Vec<String> = text
        .lines()
        .map(normalize_line)
        .filter(|line| !line.is_empty())
        .collect();

    // 当前穿戴 Tooltip 与奖励 Tooltip 可以同时出现，绝不将前者写入翻牌牌面。
    if lines.iter().any(|line| line.contains("当前装备")) {
        return None;
    }
    // “当前装备”标题被小字号 OCR 错读时，右侧穿戴面板仍会有这些固定比较栏。
    if lines.iter().any(|line| line.contains("背包/储物/家园"))
        || lines.iter().any(|line| line.contains("总数"))
        || lines.iter().any(|line| line.contains("未强化"))
    {
        return None;
    }
    // 没有“活动[试炼之地]”来源时，不把背包、聊天或其它界面文字当成试炼装备。
    if !lines
        .iter()
        .any(|line| line.contains("活动[试炼之地]") || line.contains("活动【试炼之地】"))
    {
        return None;
    }

    // 标题不写死当前赛季的“无观”。试炼来源、品级和绑定会再由本地目录白名单消歧。
    let name = lines.first()?.split(['★', '☆', '*']).next()?;
    if !(2..=48).contains(&name.chars().count()) {
        return None;
    }
    let mut levels = lines.iter().filter_map(|line| extract_item_level(line));
    let level = levels.next()?;
    if levels.any(|other| other != level) {
        return None;
    }
    let bind = if lines.iter().any(|line| line.contains("不可交易")) {
        TooltipBind::Bound
    } else if lines.iter().any(|line| line.contains("可交易")) {
        TooltipBind::Tradable
    } else {
        // 绑定类型与 1050 品等一起消歧。看不清时宁可不写入牌面。
        TooltipBind::Unknown
    };

    Some(TooltipEvidence {
        name: name.to_owned(),
        level,
        bind,
    })
}

fn normalize_line(line: &str) -> String {
    line.chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| match character {
            '．' | '。' => '·',
            _ => character,
        })
        .collect()
}

fn extract_item_level(line: &str) -> Option<i64> {
    // OCR 偶尔把“品质等级1050(+79)”分成少量空白，normalize_line 已去除空白。
    static LEVEL: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    LEVEL
        .get_or_init(|| {
            Regex::new(r"(?:品级|品质等级)[^0-9]{0,4}([0-9]{3,4})(?:[^0-9]|$)").unwrap()
        })
        .captures(line)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
}

#[cfg(target_os = "windows")]
fn recognize_text(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    rect: Rect,
) -> Option<String> {
    let bytes = crop_bgra(pixels, window_width, window_height, rect)?;
    recognize_bgra(bytes, rect.width(), rect.height())
}

#[cfg(target_os = "windows")]
fn recognize_title_text(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    rect: Rect,
) -> Option<String> {
    let mut bytes = crop_bgra(pixels, window_width, window_height, rect)?;
    mask_purple_title(&mut bytes);
    recognize_bgra(bytes, rect.width(), rect.height())
}

#[cfg(target_os = "windows")]
fn recognize_bgra(bytes: Vec<u8>, width: usize, height: usize) -> Option<String> {
    use windows::{
        core::HSTRING,
        Globalization::Language,
        Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Security::Cryptography::CryptographicBuffer,
    };

    // 截图采集线程是普通 Rust 线程，显式初始化 WinRT MTA 才能等待异步 OCR 完成。
    let _mta = WinRtApartment::initialize();
    // 结算 Tooltip 字号很小。双线性放大三倍后交给系统 OCR，提高中文笔画清晰度。
    // 调用方限频采样，避免每帧都进行该计算。
    let (bytes, bitmap_width, bitmap_height) = scale_bgra_bilinear(&bytes, width, height, 3)?;
    let buffer = CryptographicBuffer::CreateFromByteArray(&bytes).ok()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        i32::try_from(bitmap_width).ok()?,
        i32::try_from(bitmap_height).ok()?,
        BitmapAlphaMode::Ignore,
    )
    .ok()?;
    // 系统首选语言可能是英语，但游戏 Tooltip 是中文。优先固定到已安装的简体中文
    // OCR 模型，避免英文模型把装备标题和“不可交易”读成乱码。
    let language = Language::CreateLanguage(&HSTRING::from("zh-Hans-CN")).ok()?;
    let engine = if OcrEngine::IsLanguageSupported(&language).ok()? {
        OcrEngine::TryCreateFromLanguage(&language).ok()?
    } else {
        // 少数旧系统只安装 zh-CN，仍不退回英文引擎。
        let fallback = Language::CreateLanguage(&HSTRING::from("zh-CN")).ok()?;
        if !OcrEngine::IsLanguageSupported(&fallback).ok()? {
            return None;
        }
        OcrEngine::TryCreateFromLanguage(&fallback).ok()?
    };
    let result = engine.RecognizeAsync(&bitmap).ok()?.get().ok()?;
    result
        .Lines()
        .ok()?
        .into_iter()
        .filter_map(|line| line.Text().ok().map(|text| text.to_string()))
        .reduce(|mut all, line| {
            all.push('\n');
            all.push_str(&line);
            all
        })
}

#[cfg(not(target_os = "windows"))]
fn recognize_text(
    _pixels: &[u8],
    _window_width: usize,
    _window_height: usize,
    _rect: Rect,
) -> Option<String> {
    None
}

#[cfg(not(target_os = "windows"))]
fn recognize_title_text(
    _pixels: &[u8],
    _window_width: usize,
    _window_height: usize,
    _rect: Rect,
) -> Option<String> {
    None
}

fn crop_bgra(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    rect: Rect,
) -> Option<Vec<u8>> {
    let rect_width = rect.width();
    let rect_height = rect.height();
    if rect.right > window_width
        || rect.bottom > window_height
        || pixels.len() < window_width.checked_mul(window_height)?.checked_mul(4)?
    {
        return None;
    }
    // 单张 Tooltip 上限约 2 MiB，拒绝异常矩形，避免错误 UI 造成大量复制和 OCR。
    let byte_count = rect_width.checked_mul(rect_height)?.checked_mul(4)?;
    if byte_count > 2 * 1024 * 1024 {
        return None;
    }
    let mut cropped = vec![0; byte_count];
    for row in 0..rect_height {
        let source_start = ((rect.top + row) * window_width + rect.left).checked_mul(4)?;
        let destination_start = row.checked_mul(rect_width)?.checked_mul(4)?;
        cropped[destination_start..destination_start + rect_width * 4]
            .copy_from_slice(&pixels[source_start..source_start + rect_width * 4]);
    }
    Some(cropped)
}

/// 试炼装备标题为紫色，星级为金色，精炼为绿色。只在 Tooltip 顶部左侧寻找
/// 连续紫色字，避免把正文的属性数值或右侧当前装备栏带进标题 OCR。
fn purple_title_rect(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    panel: Rect,
) -> Option<Rect> {
    if panel.right > window_width
        || panel.bottom > window_height
        || pixels.len() < window_width.checked_mul(window_height)?.checked_mul(4)?
    {
        return None;
    }
    let search_right = panel.left + panel.width() * 72 / 100;
    let search_bottom = panel.top + panel.height() * 24 / 100;
    let mut left = search_right;
    let mut right = panel.left;
    let mut top = search_bottom;
    let mut bottom = panel.top;
    let mut count = 0usize;
    for y in panel.top..search_bottom {
        for x in panel.left..search_right {
            let offset = (y * window_width + x) * 4;
            if is_title_purple(&pixels[offset..offset + 4]) {
                left = left.min(x);
                right = right.max(x);
                top = top.min(y);
                bottom = bottom.max(y);
                count += 1;
            }
        }
    }
    if count < 24 || right.saturating_sub(left) < 24 || bottom.saturating_sub(top) < 7 {
        return None;
    }
    let padding = 4usize;
    Some(Rect {
        left: left.saturating_sub(padding).max(panel.left),
        top: top.saturating_sub(padding).max(panel.top),
        right: (right + padding + 1).min(search_right),
        bottom: (bottom + padding + 1).min(search_bottom),
    })
}

fn is_title_purple(bgra: &[u8]) -> bool {
    let blue = bgra[0] as i16;
    let green = bgra[1] as i16;
    let red = bgra[2] as i16;
    red > 120 && blue > 115 && 100 * green < 65 * red && 100 * green < 65 * blue
}

/// 将紫色标题带转换为白底黑字，最大程度消除 Tooltip 渐变、金色星级与绿字精炼。
/// 采样阈值经实际结算图验证，四周白边由 `purple_title_rect` 的 padding 提供。
fn mask_purple_title(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let value = if is_title_purple(pixel) { 0 } else { 255 };
        pixel[0] = value;
        pixel[1] = value;
        pixel[2] = value;
        pixel[3] = 255;
    }
}

fn scale_bgra_bilinear(
    pixels: &[u8],
    width: usize,
    height: usize,
    factor: usize,
) -> Option<(Vec<u8>, usize, usize)> {
    let scaled_width = width.checked_mul(factor)?;
    let scaled_height = height.checked_mul(factor)?;
    let byte_count = scaled_width.checked_mul(scaled_height)?.checked_mul(4)?;
    if factor < 2
        || byte_count > 12 * 1024 * 1024
        || pixels.len() < width.checked_mul(height)?.checked_mul(4)?
    {
        return None;
    }
    let mut scaled = vec![0; byte_count];
    for target_y in 0..scaled_height {
        let source_y =
            ((target_y as f32 + 0.5) / factor as f32 - 0.5).clamp(0.0, (height - 1) as f32);
        let top = source_y.floor() as usize;
        let bottom = (top + 1).min(height - 1);
        let vertical = source_y - top as f32;
        for target_x in 0..scaled_width {
            let source_x =
                ((target_x as f32 + 0.5) / factor as f32 - 0.5).clamp(0.0, (width - 1) as f32);
            let left = source_x.floor() as usize;
            let right = (left + 1).min(width - 1);
            let horizontal = source_x - left as f32;
            let top_left = (top * width + left) * 4;
            let top_right = (top * width + right) * 4;
            let bottom_left = (bottom * width + left) * 4;
            let bottom_right = (bottom * width + right) * 4;
            let destination = (target_y * scaled_width + target_x) * 4;
            for channel in 0..4 {
                let upper = pixels[top_left + channel] as f32 * (1.0 - horizontal)
                    + pixels[top_right + channel] as f32 * horizontal;
                let lower = pixels[bottom_left + channel] as f32 * (1.0 - horizontal)
                    + pixels[bottom_right + channel] as f32 * horizontal;
                scaled[destination + channel] = (upper * (1.0 - vertical) + lower * vertical)
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
        }
    }
    Some((scaled, scaled_width, scaled_height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_real_reward_tooltip_shape() {
        let evidence = parse_trial_tooltip(
            "无观项链·内·荒 ★★★★★★\n不可交易\n体质提高272(+20)\n内功攻击提高275(+21)\n内功破防等级提高275(+21)\n无双等级提高319(+24)\n品级1050(+79)\n获取途径\n活动[试炼之地]",
        )
        .unwrap();

        assert_eq!(evidence.name, "无观项链·内·荒");
        assert_eq!(evidence.level, 1050);
        assert_eq!(evidence.bind, TooltipBind::Bound);
    }

    #[test]
    fn rejects_current_equipment_even_when_trial_text_is_visible() {
        assert!(parse_trial_tooltip(
            "当前装备\n无观项链·内·荒 ★★★★★★\n不可交易\n品级985(+74)\n活动[试炼之地]"
        )
        .is_none());
        assert!(parse_trial_tooltip(
            "无观项链·内·荒\n不可交易\n品质等级985\n总数:1背包/储物/家园:0/1/0\n未强化\n活动[试炼之地]"
        )
        .is_none());
    }

    #[test]
    fn rejects_non_trial_and_unflipped_panels() {
        assert!(parse_trial_tooltip("周翻牌奖励\n继续翻牌").is_none());
        assert!(parse_trial_tooltip("无观项链·内·荒\n品级1050\n不可交易").is_none());
    }

    #[test]
    fn copies_only_the_requested_rect() {
        let pixels: Vec<u8> = (0..64).collect();
        let cropped = crop_bgra(
            &pixels,
            4,
            4,
            Rect {
                left: 1,
                top: 1,
                right: 3,
                bottom: 3,
            },
        )
        .unwrap();
        assert_eq!(
            cropped,
            vec![20, 21, 22, 23, 24, 25, 26, 27, 36, 37, 38, 39, 40, 41, 42, 43]
        );
    }

    #[test]
    fn normalizes_the_actual_system_ocr_punctuation_and_stars() {
        let evidence = parse_trial_tooltip(
            "无 观 项 链 ． 内 ． 荒 * * * 女 女 *\n不 可 交 易\n品 质 等 级 1050 （ + 79 ）\n活 动 [ 试 炼 之 地 ]",
        )
        .unwrap();
        assert_eq!(evidence.name, "无观项链·内·荒");
        assert_eq!(evidence.level, 1050);
        assert_eq!(evidence.bind, TooltipBind::Bound);
    }

    #[test]
    fn finds_only_the_purple_title_in_the_tooltip_header() {
        let mut pixels = vec![0u8; 160 * 120 * 4];
        // 紫色标题块，右侧的金色星块与绿色精炼块都不得扩展标题矩形。
        for y in 8..20 {
            for x in 12..76 {
                let offset = (y * 160 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[180, 40, 210, 255]);
            }
        }
        for y in 8..20 {
            for x in 82..104 {
                let offset = (y * 160 + x) * 4;
                pixels[offset..offset + 4].copy_from_slice(&[30, 160, 235, 255]);
            }
        }
        let rect = purple_title_rect(
            &pixels,
            160,
            120,
            Rect {
                left: 0,
                top: 0,
                right: 160,
                bottom: 100,
            },
        )
        .unwrap();
        assert!(rect.right < 82);
        assert!(rect.left <= 12 && rect.top <= 8);
    }

    /// 用 `JX3_TRIAL_OCR_REPLAY` 指向一张 2560×1369 的 BGRA 实拍帧后手工运行。
    /// 截图不入库，避免把玩家画面和聊天内容纳入源码。
    #[test]
    #[ignore = "requires a locally supplied JX3 BGRA replay frame"]
    fn replays_real_reward_tooltip() {
        let path = std::env::var("JX3_TRIAL_OCR_REPLAY").unwrap();
        let pixels = std::fs::read(path).unwrap();
        let evidence = recognize_trial_tooltip(
            &pixels,
            2560,
            1369,
            Rect {
                // 真实样图左侧奖励提示。右侧“当前装备”面板不在本矩形内。
                left: 1273,
                top: 320,
                right: 1690,
                bottom: 709,
            },
        )
        .unwrap();
        assert_eq!(evidence.name, "无观项链·内·荒");
        assert_eq!(evidence.level, 1050);
        assert_eq!(evidence.bind, TooltipBind::Bound);
        let worn = Rect {
            left: 1694,
            top: 320,
            right: 2113,
            bottom: 818,
        };
        assert!(recognize_trial_tooltip(&pixels, 2560, 1369, worn).is_none());
    }
}
