// 试炼之地结算翻牌位识别。
//
// 这里刻意不做文字 OCR：结算页中被翻开的牌有稳定的金色外框，而鼠标悬浮的
// 物品提示框可能盖住其它牌。识别会从同排五张奖励牌的底边实际测出牌列，再从
// 左数出金框所在牌位；证据不完整时宁可返回 `None`。

use std::convert::TryInto;

#[derive(Debug, Clone, Copy)]
struct Band {
    start: usize,
    end: usize,
}

impl Band {
    fn width(self) -> usize {
        self.end.saturating_sub(self.start).saturating_add(1)
    }

    fn center(self) -> f32 {
        (self.start + self.end) as f32 / 2.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Frame {
    center_x: f32,
    width: usize,
    slot: u8,
}

/// 屏幕物理像素中的闭区间外矩形。宽高均已钳制到采集帧范围内。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rect {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

impl Rect {
    pub fn width(self) -> usize {
        self.right.saturating_sub(self.left)
    }

    pub fn height(self) -> usize {
        self.bottom.saturating_sub(self.top)
    }
}

/// 同一帧中分离出的已翻牌、鼠标悬停牌和说明窗候选。
///
/// 装备写入鼠标指向的 hovered_slot。只有该牌与 selected_slot 相同才算实际翻到。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardObservation {
    pub selected_slot: u8,
    pub hovered_slot: Option<u8>,
    pub tooltip_rects: Vec<Rect>,
}

/// 从 BGRA8 游戏窗口截图中识别结算页已翻开的牌位（1 到 5）。
///
/// 仅在检测到完整的金色矩形框、同排存在完整五张奖励牌底边时返回结果。牌位
/// 按实际牌列从左数，普通界面的金色按钮、孤立高亮和多张同时高亮都不会被当成
/// 一次确定翻牌。
pub(super) fn flipped_slot(pixels: &[u8], width: usize, height: usize) -> Option<u8> {
    let pixel_count = width.checked_mul(height)?;
    let byte_count = pixel_count.checked_mul(4)?;
    if pixels.len() < byte_count || width < 640 || height < 360 {
        return None;
    }

    // 五张奖励牌始终位于结算弹窗的中部。范围使用窗口比例，兼容分辨率和 UI 缩放。
    let left = width * 17 / 100;
    let right = width * 83 / 100;
    let top = height * 32 / 100;
    let bottom = height * 75 / 100;
    if right <= left || bottom <= top {
        return None;
    }

    let roi_height = bottom - top;
    let minimum_vertical = (roi_height / 20).max(12);
    let mut vertical = vec![0usize; width];

    for y in top..bottom {
        let row = y * width * 4;
        for x in left..right {
            if is_gold(&pixels[row + x * 4..row + x * 4 + 4]) {
                vertical[x] += 1;
            }
        }
    }

    let x_bands = bands(&vertical, left, right, minimum_vertical);
    // 金色噪声可制造大量竖线。后续每一对竖线都需要重新扫描一个矩形，先在这里
    // 拒绝异常图像，避免 O(n² × 像素数) 的耗时路径。
    const MAX_X_BANDS: usize = 24;
    const MAX_FRAME_PAIRS: usize = 48;
    const MAX_Y_BANDS: usize = 12;
    const MAX_FRAMES: usize = 12;
    if x_bands.len() < 2 || x_bands.len() > MAX_X_BANDS {
        return None;
    }

    let min_width = (width * 25 / 1000).max(24);
    let max_width = (width * 105 / 1000).max(min_width + 1);
    let min_height = (height * 55 / 1000).max(40);
    let max_height = height * 27 / 100;
    let mut frames = Vec::new();
    let mut checked_pairs = 0usize;

    for (left_index, left_band) in x_bands.iter().enumerate() {
        for right_band in x_bands.iter().skip(left_index + 1) {
            let frame_width = right_band.center() - left_band.center();
            if frame_width < min_width as f32 || frame_width > max_width as f32 {
                continue;
            }
            checked_pairs += 1;
            if checked_pairs > MAX_FRAME_PAIRS {
                return None;
            }

            let x_start = left_band.start.max(left);
            let x_end = right_band.end.min(right.saturating_sub(1));
            if x_end <= x_start {
                continue;
            }

            let horizontal = horizontal_gold_counts(pixels, width, x_start, x_end, top, bottom);
            let minimum_horizontal = ((x_end - x_start + 1) / 6).max(10);
            let y_bands = bands(&horizontal, 0, horizontal.len(), minimum_horizontal);
            if y_bands.len() > MAX_Y_BANDS {
                return None;
            }

            for (top_index, top_band) in y_bands.iter().enumerate() {
                for bottom_band in y_bands.iter().skip(top_index + 1) {
                    let frame_height = bottom_band.center() - top_band.center();
                    if frame_height < min_height as f32 || frame_height > max_height as f32 {
                        continue;
                    }

                    let aspect = frame_width / frame_height;
                    if !(0.42..=1.12).contains(&aspect) {
                        continue;
                    }

                    let center_x = (left_band.center() + right_band.center()) / 2.0;
                    let frame_width = frame_width.round() as usize;
                    let frame_bottom = top + bottom_band.center().round() as usize;
                    let Some(slot) = slot_from_complete_card_row(
                        pixels,
                        width,
                        height,
                        left,
                        right,
                        center_x,
                        frame_width,
                        frame_bottom,
                    ) else {
                        continue;
                    };

                    if frames.len() >= MAX_FRAMES {
                        return None;
                    }

                    frames.push(Frame {
                        center_x,
                        width: frame_width,
                        slot,
                    });
                }
            }
        }
    }

    resolve_frame(frames)
}

/// 从一帧中一次得到翻牌与悬停牌观测。无结算页、未翻牌、鼠标在牌外、或没有
/// 紧邻鼠标的深色物品说明窗时，都不会产生装备 OCR 候选。
pub(super) fn card_observation(
    pixels: &[u8],
    width: usize,
    height: usize,
    cursor: Option<(usize, usize)>,
) -> Option<CardObservation> {
    let selected_slot = flipped_slot(pixels, width, height)?;
    let hovered_slot = hovered_slot_in_complete_row(pixels, width, height, cursor);
    let tooltip_rects = hovered_slot
        .is_some()
        .then(|| tooltip_rects_near_cursor(pixels, width, height, cursor))
        .unwrap_or_default();
    Some(CardObservation {
        selected_slot,
        hovered_slot,
        tooltip_rects,
    })
}

/// 结算页是否已经展示了完整的五张奖励牌。
///
/// 这个判定不读取金色翻牌框，也不读取鼠标位置，因此可用于“尚未翻牌”的结算页。
/// 它仍要求一条实际的五牌底边，而不是仅凭弹窗、按钮或单个装饰边推断；截图
/// 线程可据此从空场景的低频采样切到结算页的短周期采样。
pub(super) fn cards_visible(pixels: &[u8], width: usize, height: usize) -> bool {
    let Some(expected_bytes) = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(4))
    else {
        return false;
    };
    if pixels.len() < expected_bytes {
        return false;
    }
    let Some((left, right, top, bottom)) = settlement_roi(width, height) else {
        return false;
    };

    let minimum_width = (width * 25 / 1000).max(24);
    // 先以较稀疏的像素探针筛出可能的牌底边，再对该行做完整的连续边扫描。
    // 五张牌的总边长远大于单个按钮或文字横线；异常画面即使让所有行都命中，
    // 最终成本也只多一次等宽的行扫描，并由 valid_card_row 严格拒绝。
    const PROBE_STRIDE: usize = 8;
    let minimum_probes = (minimum_width * 2 / PROBE_STRIDE).max(8);
    for y in top..bottom {
        let row = y * width * 4;
        let probes = (left..right)
            .step_by(PROBE_STRIDE)
            .filter(|x| is_card_edge(&pixels[row + x * 4..row + x * 4 + 4]))
            .count();
        if probes < minimum_probes {
            continue;
        }
        let runs = card_edge_runs(pixels, width, y, left, right, minimum_width);
        if valid_card_row(&runs).is_some() {
            return true;
        }
    }
    false
}

/// 结算翻牌选择器是否仍在画面中。
///
/// 牌在翻转动画中会短暂同时消失，不能把那一帧直接当作面板关闭。此函数先接受
/// 已确认的五牌/金框，再在无牌时要求居中页眉和底部三按钮同时出现。任何一侧
/// 遮挡、只出现单个相似控件或阈值边缘都返回 `None`，避免将不完整画面误判为
/// 关闭；只有两组锚点都明确缺失才返回 `Some(false)`。
pub(super) fn selector_visible(
    pixels: &[u8],
    width: usize,
    height: usize,
    cards_present: bool,
) -> Option<bool> {
    if !has_complete_frame(pixels, width, height) {
        return None;
    }
    if cards_present {
        return Some(true);
    }
    match (
        selector_header_anchor(pixels, width, height),
        selector_footer_anchor(pixels, width, height),
    ) {
        (SelectorAnchor::Present, SelectorAnchor::Present) => Some(true),
        (SelectorAnchor::Absent, SelectorAnchor::Absent) => Some(false),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectorAnchor {
    Present,
    Absent,
    Unknown,
}

fn has_complete_frame(pixels: &[u8], width: usize, height: usize) -> bool {
    width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(4))
        .is_some_and(|expected| pixels.len() >= expected && width >= 640 && height >= 360)
}

/// 页眉位于居中结算窗的上沿。两张实拍分别落在客户区 y=18% 与 y=22%，因此
/// 搜索带留出 UI 缩放余量；单行蓝绿色背景或标题文字不会独立成立。
fn selector_header_anchor(pixels: &[u8], width: usize, height: usize) -> SelectorAnchor {
    let left = width * 25 / 100;
    let right = width * 75 / 100;
    let top = height * 17 / 100;
    let bottom = height * 28 / 100;
    let span = right.saturating_sub(left);
    if span == 0 || bottom <= top {
        return SelectorAnchor::Unknown;
    }

    let mut consecutive = 0usize;
    let mut near = false;
    for y in top..bottom {
        let row = y * width * 4;
        let mut teal = 0usize;
        for x in left..right {
            if is_selector_header_tint(&pixels[row + x * 4..row + x * 4 + 4]) {
                teal += 1;
            }
        }
        if teal * 100 >= span * 60 {
            consecutive += 1;
            if consecutive >= (height * 4 / 1000).max(5) {
                return SelectorAnchor::Present;
            }
        } else {
            if teal * 100 >= span * 24 {
                near = true;
            }
            consecutive = 0;
        }
    }
    if near {
        SelectorAnchor::Unknown
    } else {
        SelectorAnchor::Absent
    }
}

fn is_selector_header_tint(bgra: &[u8]) -> bool {
    let b = bgra[0] as i16;
    let g = bgra[1] as i16;
    let r = bgra[2] as i16;
    // 结算窗页眉是低亮度青灰色（BGRA 中 B/G 高于 R）。上下方场景纹理、
    // 金色按钮、深色 Tooltip 和白色标题不能满足这一整段背景色。
    (55..=150).contains(&b)
        && (55..=155).contains(&g)
        && (15..=75).contains(&r)
        && b - r >= 12
        && g - r >= 8
        && b - g <= 30
}

/// 底栏至少有“重新开始 / 继续挑战 / 退出挑战”三枚连续按钮。它们可分别为
/// 青色或金色，也可能有文字切开底色；因此合并短文字空隙后检查三枚等高矩形的
/// 宽度、间隔和跨行稳定性，而不是把某个青色按钮当作结算页。
fn selector_footer_anchor(pixels: &[u8], width: usize, height: usize) -> SelectorAnchor {
    let left = width * 42 / 100;
    let right = width * 72 / 100;
    let top = height * 58 / 100;
    let bottom = height * 68 / 100;
    let mut consecutive = 0usize;
    let mut near = false;
    let mut previous: Option<[Band; 3]> = None;
    for y in top..bottom {
        let Some(layout) = selector_button_layout(pixels, width, y, left, right) else {
            consecutive = 0;
            previous = None;
            continue;
        };
        let stable = previous.is_some_and(|old| button_layouts_align(old, layout, width));
        if stable {
            consecutive += 1;
            if consecutive >= (height * 3 / 1000).max(3) {
                return SelectorAnchor::Present;
            }
        } else {
            consecutive = 1;
        }
        previous = Some(layout);
        near = true;
    }
    if near || selector_has_two_button_hint(pixels, width, top, bottom, left, right) {
        SelectorAnchor::Unknown
    } else {
        SelectorAnchor::Absent
    }
}

fn selector_button_layout(
    pixels: &[u8],
    width: usize,
    y: usize,
    left: usize,
    right: usize,
) -> Option<[Band; 3]> {
    let row = y * width * 4;
    let mut runs = Vec::new();
    let mut start = None;
    for x in left..right {
        if is_selector_button_fill(&pixels[row + x * 4..row + x * 4 + 4]) {
            if start.is_none() {
                start = Some(x);
            }
        } else if start.is_some() {
            runs.push(Band {
                start: start.take().unwrap_or_default(),
                end: x.saturating_sub(1),
            });
        }
    }
    if let Some(start) = start {
        runs.push(Band {
            start,
            end: right.saturating_sub(1),
        });
    }

    let minimum_width = (width * 3 / 100).max(32);
    let maximum_width = (width * 9 / 100).max(minimum_width + 1);
    runs.retain(|run| (minimum_width..=maximum_width).contains(&run.width()));
    if runs.len() < 3 || runs.len() > 4 {
        return None;
    }
    let buttons: [Band; 3] = runs[runs.len() - 3..].try_into().ok()?;
    let average_width = buttons.iter().map(|run| run.width()).sum::<usize>() as f32 / 3.0;
    if buttons.iter().any(|run| {
        (run.width() as f32) < average_width * 0.65 || (run.width() as f32) > average_width * 1.45
    }) {
        return None;
    }
    let gaps = buttons
        .windows(2)
        .map(|pair| pair[1].start.saturating_sub(pair[0].end))
        .collect::<Vec<_>>();
    let maximum_gap = width * 6 / 100;
    if gaps.iter().any(|gap| *gap > maximum_gap) {
        return None;
    }
    Some(buttons)
}

fn is_selector_button_fill(bgra: &[u8]) -> bool {
    let b = bgra[0] as i16;
    let g = bgra[1] as i16;
    let r = bgra[2] as i16;
    let teal = (85..=155).contains(&b)
        && (85..=155).contains(&g)
        && (35..=100).contains(&r)
        && b - r >= 25
        && g - r >= 25
        && (b - g).abs() <= 35;
    let gold = (60..=135).contains(&b)
        && (105..=190).contains(&g)
        && (130..=230).contains(&r)
        && r - g >= 15
        && g - b >= 20;
    teal || gold
}

fn button_layouts_align(first: [Band; 3], second: [Band; 3], width: usize) -> bool {
    first.iter().zip(second).all(|(old, new)| {
        (old.center() - new.center()).abs() <= (width * 12 / 1000) as f32
            && old.width().abs_diff(new.width()) <= width * 12 / 1000
    })
}

fn selector_has_two_button_hint(
    pixels: &[u8],
    width: usize,
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
) -> bool {
    let minimum_width = (width * 3 / 100).max(32);
    for y in top..bottom {
        let row = y * width * 4;
        let mut runs = 0usize;
        let mut start = None;
        for x in left..right {
            let fill = is_selector_button_fill(&pixels[row + x * 4..row + x * 4 + 4]);
            match (start, fill) {
                (None, true) => start = Some(x),
                (Some(begin), false) if x.saturating_sub(begin) >= minimum_width => {
                    runs += 1;
                    start = None;
                }
                (Some(_), false) => start = None,
                _ => {}
            }
        }
        if start.is_some_and(|begin| right.saturating_sub(begin) >= minimum_width) {
            runs += 1;
        }
        if runs >= 2 {
            return true;
        }
    }
    false
}

/// 五张牌是否仍是同一套未翻开的牌背。
///
/// 这比 [`cards_visible`] 更严格：完整的五牌底边只说明结算面板还在，不能说明
/// 玩家尚未翻牌。这里还会比对每张牌内部相同的徽记纹理；任意一张不满足牌背
/// 颜色、面积或形状一致性时都返回 `false`。用于确认新一轮的牌背重置，因此
/// 看不清（包括 Tooltip 覆盖）也必须保持未知。
pub(super) fn card_backs_visible(pixels: &[u8], width: usize, height: usize) -> bool {
    let Some(expected_bytes) = width
        .checked_mul(height)
        .and_then(|count| count.checked_mul(4))
    else {
        return false;
    };
    if pixels.len() < expected_bytes {
        return false;
    }
    let Some((left, right, top, bottom)) = settlement_roi(width, height) else {
        return false;
    };

    let minimum_width = (width * 25 / 1000).max(24);
    const PROBE_STRIDE: usize = 8;
    let minimum_probes = (minimum_width * 2 / PROBE_STRIDE).max(8);
    for y in top..bottom {
        let row = y * width * 4;
        let probes = (left..right)
            .step_by(PROBE_STRIDE)
            .filter(|x| is_card_edge(&pixels[row + x * 4..row + x * 4 + 4]))
            .count();
        if probes < minimum_probes {
            continue;
        }
        let runs = card_edge_runs(pixels, width, y, left, right, minimum_width);
        let Some(cards) = valid_card_row(&runs) else {
            continue;
        };
        // 牌底边和顶边都可能满足颜色阈值。两侧都复核，只有完整五张牌的同一
        // 组徽记一致才算牌背；这避免依赖某一个 UI 缩放下边框的精确方向。
        if card_row_has_matching_backs(pixels, width, height, y, cards, true)
            || card_row_has_matching_backs(pixels, width, height, y, cards, false)
        {
            return true;
        }
    }
    false
}

const BACK_SIGNATURE_COLUMNS: usize = 20;
const BACK_SIGNATURE_ROWS: usize = 24;
const BACK_SIGNATURE_BITS: usize = BACK_SIGNATURE_COLUMNS * BACK_SIGNATURE_ROWS;
const BACK_SIGNATURE_WORDS: usize = BACK_SIGNATURE_BITS.div_ceil(64);
const MIN_BACK_SIGNATURE_PIXELS: usize = 80;
const MAX_BACK_SIGNATURE_PIXELS: usize = 260;

#[derive(Clone, Copy, Default)]
struct BackSignature {
    words: [u64; BACK_SIGNATURE_WORDS],
    count: usize,
    dark_count: usize,
}

impl BackSignature {
    fn insert(&mut self, index: usize) {
        self.words[index / 64] |= 1u64 << (index % 64);
        self.count += 1;
    }

    fn matches(self, other: Self) -> bool {
        let common = self
            .words
            .iter()
            .zip(other.words)
            .map(|(left, right)| (left & right).count_ones() as usize)
            .sum::<usize>();
        let smaller = self.count.min(other.count);
        // 实拍中被鼠标轻微高亮的一张牌背会有色调变化；仍要求四成以上同一位置
        // 命中，且至少 50 个采样点重叠，不能被相似的纯色奖励背景误触发。
        common >= 50 && common * 100 >= smaller * 42
    }
}

fn card_row_has_matching_backs(
    pixels: &[u8],
    width: usize,
    height: usize,
    edge_y: usize,
    cards: [CardEdgeRun; 5],
    edge_is_bottom: bool,
) -> bool {
    let mut signatures = [BackSignature::default(); 5];
    for (index, card) in cards.into_iter().enumerate() {
        let card_height = card.width() * 136 / 100;
        let card_top = if edge_is_bottom {
            edge_y.saturating_sub(card_height)
        } else {
            edge_y
        };
        if card_top.saturating_add(card_height) >= height {
            return false;
        }
        let signature = card_back_signature(pixels, width, card, card_top, card_height);
        // 牌背底色是淡金属色；已翻奖励牌通常有深色物品底图或物品图标。这个条件
        // 与徽记形状同时成立，排除五张恰好相同的银叶子、药品等奖励展示。
        if !(MIN_BACK_SIGNATURE_PIXELS..=MAX_BACK_SIGNATURE_PIXELS).contains(&signature.count)
            || signature.dark_count > 20
        {
            return false;
        }
        signatures[index] = signature;
    }

    // 每一张都必须与一张基准牌背相同。这样即使其中一张受到高亮影响，也不能把
    // “四张牌背 + 一张已翻奖励”当作下一轮的重新发牌。
    signatures.iter().copied().any(|reference| {
        signatures
            .iter()
            .copied()
            .all(|signature| signature.matches(reference))
    })
}

fn card_back_signature(
    pixels: &[u8],
    width: usize,
    card: CardEdgeRun,
    card_top: usize,
    card_height: usize,
) -> BackSignature {
    let mut signature = BackSignature::default();
    for grid_y in 0..BACK_SIGNATURE_ROWS {
        let y = card_top
            + card_height * 13 / 100
            + grid_y * (card_height * 70 / 100) / (BACK_SIGNATURE_ROWS - 1);
        for grid_x in 0..BACK_SIGNATURE_COLUMNS {
            let x = card.start
                + card.width() * 16 / 100
                + grid_x * (card.width() * 68 / 100) / (BACK_SIGNATURE_COLUMNS - 1);
            let pixel = &pixels[(y * width + x) * 4..(y * width + x) * 4 + 4];
            if pixel[0] < 55 && pixel[1] < 55 && pixel[2] < 55 {
                signature.dark_count += 1;
            }
            if is_card_back_ink(pixel) {
                signature.insert(grid_y * BACK_SIGNATURE_COLUMNS + grid_x);
            }
        }
    }
    signature
}

fn is_card_back_ink(bgra: &[u8]) -> bool {
    let b = bgra[0] as i16;
    let g = bgra[1] as i16;
    let r = bgra[2] as i16;
    // 未翻开的剑形徽记是偏金色的亮线条（BGRA 中 B 明显低于 R/G）。实际奖励
    // 的图标、文字和暗底会有
    // 各种颜色，但不会在五张牌同一相对坐标形成这组笔画。
    (60..=150).contains(&b)
        && (105..=195).contains(&g)
        && (115..=205).contains(&r)
        && g - b >= 22
        && r - b >= 30
        && r >= g - 12
}

fn hovered_slot_in_complete_row(
    pixels: &[u8],
    width: usize,
    height: usize,
    cursor: Option<(usize, usize)>,
) -> Option<u8> {
    let (cursor_x, cursor_y) = cursor?;
    let (left, right, top, bottom) = settlement_roi(width, height)?;
    if cursor_x < left || cursor_x >= right || cursor_y < top || cursor_y >= bottom {
        return None;
    }

    let minimum_width = (width * 25 / 1000).max(24);
    let mut matched = None;
    // 卡片边框可能有 1 至数个像素的抗锯齿。每条符合条件的底边必须推导出
    // 同一张牌，否则画面存在重叠或非标准面板，拒绝交给 OCR。
    for y in top..bottom {
        let runs = card_edge_runs(pixels, width, y, left, right, minimum_width);
        let Some(row) = valid_card_row(&runs) else {
            continue;
        };
        let card_width = row[0].width();
        let card_height = card_width * 136 / 100;
        let card_top = y.saturating_sub(card_height);
        if !(card_top..=y).contains(&cursor_y) {
            continue;
        }
        let (index, card) = row
            .iter()
            .enumerate()
            .find(|(_, card)| (card.start..=card.end).contains(&cursor_x))?;
        // 奖励说明只在卡片左侧的物品图标列触发。卡片标题、数量、按钮或两
        // 张牌之间的鼠标位置都不采样，避免把残留 Tooltip 关联到错误牌位。
        let icon_left = card.start + card_width * 7 / 100;
        let icon_right = card.start + card_width * 47 / 100;
        let icon_top = card_top + card_width * 7 / 100;
        let icon_bottom = y.saturating_sub(card_width * 7 / 100);
        if !(icon_left..=icon_right).contains(&cursor_x)
            || !(icon_top..=icon_bottom).contains(&cursor_y)
        {
            return None;
        }
        let slot = (index + 1) as u8;
        if matched.replace(slot).is_some_and(|known| known != slot) {
            return None;
        }
    }
    matched
}

fn settlement_roi(width: usize, height: usize) -> Option<(usize, usize, usize, usize)> {
    if width < 640 || height < 360 {
        return None;
    }
    let left = width * 17 / 100;
    let right = width * 83 / 100;
    let top = height * 32 / 100;
    let bottom = height * 75 / 100;
    (right > left && bottom > top).then_some((left, right, top, bottom))
}

/// 物品说明窗会紧挨鼠标左侧或右侧展开，且上沿通常位于鼠标上方。
///
/// 返回两个不重叠的候选区域，而不是猜测哪个才是说明窗。OCR 必须以装备名、
/// 品级和非“当前装备”语义确认其中一个，两个都像有效装备时拒绝。这样既适配
/// UI 自动避让，也不会把右侧当前穿戴栏直接当作翻牌奖励。
pub(super) fn tooltip_candidate_rects(
    width: usize,
    height: usize,
    cursor: Option<(usize, usize)>,
) -> Option<Vec<Rect>> {
    let (x, y) = cursor?;
    if x >= width || y >= height || width < 640 || height < 360 {
        return None;
    }
    let panel_width = (width * 18 / 100).clamp(250, 560).min(width);
    let above = (height * 48 / 100).clamp(220, 760);
    let below = (height * 18 / 100).clamp(100, 320);
    let top = y.saturating_sub(above);
    let bottom = y.saturating_add(below).min(height);
    if bottom.saturating_sub(top) < 120 {
        return None;
    }
    let gap = (width / 250).clamp(6, 18);
    // Tooltip 外框有半透明阴影，指针停在边缘时左边线可能比尖端只早几像素。
    // 右侧搜索因此向左留出一个小缝，不会漏掉标题列。
    let right_left = x.saturating_sub(gap);
    let right = Rect {
        left: right_left,
        top,
        right: right_left.saturating_add(panel_width).min(width),
        bottom,
    };
    let left = Rect {
        left: x.saturating_sub(gap.saturating_add(panel_width)),
        top,
        right: x.saturating_sub(gap),
        bottom,
    };
    let mut candidates = Vec::with_capacity(2);
    if right.width() >= 120 {
        candidates.push(right);
    }
    if left.width() >= 120 {
        candidates.push(left);
    }
    (!candidates.is_empty()).then_some(candidates)
}

/// 检测紧邻鼠标的深色说明窗列。只接受连续多行可见的深色面板边界，且面板
/// 紧贴鼠标左侧或右侧，普通结算背景和远处“当前装备”栏不会触发 OCR。
pub(super) fn tooltip_rects_near_cursor(
    pixels: &[u8],
    width: usize,
    height: usize,
    cursor: Option<(usize, usize)>,
) -> Vec<Rect> {
    let Some(rough_candidates) = tooltip_candidate_rects(width, height, cursor) else {
        return Vec::new();
    };
    let (cursor_x, _) = cursor.unwrap_or_default();
    let mut result: Vec<Rect> = Vec::new();
    for rough in rough_candidates {
        let mut spans: Vec<(usize, usize, usize)> = Vec::new();
        for y in rough.top..rough.bottom {
            for (left, right) in dark_runs(pixels, width, y, rough.left, rough.right) {
                if right.saturating_sub(left) < 180 {
                    continue;
                }
                let next_to_cursor = (left >= cursor_x && left - cursor_x <= 36)
                    || (right <= cursor_x && cursor_x - right <= 36);
                if !next_to_cursor {
                    continue;
                }
                if let Some(span) = spans
                    .iter_mut()
                    .find(|span| span.0.abs_diff(left) <= 6 && span.1.abs_diff(right) <= 6)
                {
                    span.2 += 1;
                } else {
                    spans.push((left, right, 1));
                }
            }
        }
        // 同一 Tooltip 顶部可能与旁侧界面在少量行连成更宽的深色带。保留
        // 贴鼠标边界相同的最窄稳定带，避免把“当前装备”栏并入 OCR 区域。
        let mut stable: Vec<(usize, usize)> = spans
            .into_iter()
            .filter(|(_, _, votes)| *votes >= 6)
            .map(|(left, right, _)| (left, right))
            .collect();
        stable.sort_by_key(|(left, right)| (*left, right - left));
        for (left, right) in stable {
            let same_edge = result.iter().any(|existing: &Rect| {
                existing.top == rough.top
                    && existing.bottom == rough.bottom
                    && existing.left.abs_diff(left) <= 6
            });
            if !same_edge {
                let rect = Rect {
                    left,
                    top: rough.top,
                    right,
                    bottom: rough.bottom,
                };
                if let Some(trimmed) = trim_to_dark_panel(pixels, width, rect, cursor) {
                    // 装备标题位于 Tooltip 上部且使用亮紫色。没有它说明只是
                    // 底部操作栏或普通深色 UI，禁止浪费一次系统 OCR。
                    if has_purple_title(pixels, width, trimmed) {
                        result.push(trimmed);
                    }
                }
            }
        }
    }
    dedupe_overlapping_tooltips(result)
}

fn has_purple_title(pixels: &[u8], width: usize, rect: Rect) -> bool {
    let title_bottom = (rect.top + rect.height() * 30 / 100)
        .min(rect.top.saturating_add(110))
        .min(rect.bottom);
    let mut count = 0usize;
    for y in rect.top..title_bottom {
        for x in rect.left..rect.right {
            let pixel = &pixels[(y * width + x) * 4..(y * width + x) * 4 + 4];
            let b = pixel[0] as i16;
            let g = pixel[1] as i16;
            let r = pixel[2] as i16;
            let purple =
                b >= 75 && r >= 70 && g <= 105 && (b - r).abs() <= 80 && b + r - g * 2 >= 80;
            if purple {
                count += 1;
                if count >= 12 {
                    return true;
                }
            }
        }
    }
    false
}

fn dedupe_overlapping_tooltips(mut rects: Vec<Rect>) -> Vec<Rect> {
    rects.sort_by_key(|rect| rect.width() * rect.height());
    let mut result: Vec<Rect> = Vec::new();
    'candidate: for candidate in rects {
        for known in &mut result {
            let overlap_width = candidate
                .right
                .min(known.right)
                .saturating_sub(candidate.left.max(known.left));
            let overlap_height = candidate
                .bottom
                .min(known.bottom)
                .saturating_sub(candidate.top.max(known.top));
            let overlap = overlap_width * overlap_height;
            let smaller =
                (candidate.width() * candidate.height()).min(known.width() * known.height());
            if smaller > 0 && overlap * 100 >= smaller * 82 {
                // 同一标题在抗锯齿或相邻栏干扰下会有两条近似暗边。左侧取更早
                // 的边以保留装备名，右侧取更早的收束边以排除当前装备栏。
                known.left = known.left.min(candidate.left);
                known.right = known.right.min(candidate.right);
                known.top = known.top.max(candidate.top);
                known.bottom = known.bottom.min(candidate.bottom);
                continue 'candidate;
            }
        }
        result.push(candidate);
    }
    result
}

fn trim_to_dark_panel(
    pixels: &[u8],
    width: usize,
    rect: Rect,
    cursor: Option<(usize, usize)>,
) -> Option<Rect> {
    let (_, cursor_y) = cursor?;
    let threshold = rect.width() * 29 / 100;
    let mut bands = Vec::new();
    let mut start = None;
    let mut last_active = 0usize;
    for y in rect.top..rect.bottom {
        let dark_count = (rect.left..rect.right)
            .filter(|x| {
                let pixel = &pixels[(y * width + *x) * 4..(y * width + *x) * 4 + 4];
                pixel[0] < 58 && pixel[1] < 55 && pixel[2] < 48
            })
            .count();
        if dark_count >= threshold {
            if start.is_none() {
                start = Some(y);
            }
            last_active = y;
        } else if let Some(begin) = start {
            // Tooltip 文本会短暂打断暗底色。小间隙仍属于同一面板。
            if y - last_active > 16 {
                if last_active + 1 - begin >= 100 {
                    bands.push((begin, last_active + 1));
                }
                start = None;
            }
        }
    }
    if let Some(begin) = start {
        if last_active + 1 - begin >= 100 {
            bands.push((begin, last_active + 1));
        }
    }
    let (top, bottom) = bands.into_iter().min_by_key(|(top, bottom)| {
        if (*top..=*bottom).contains(&cursor_y) {
            0
        } else {
            top.abs_diff(cursor_y).min(bottom.abs_diff(cursor_y))
        }
    })?;
    Some(Rect {
        top,
        bottom,
        ..rect
    })
}

fn dark_runs(
    pixels: &[u8],
    width: usize,
    y: usize,
    left: usize,
    right: usize,
) -> Vec<(usize, usize)> {
    if y >= pixels.len() / width.saturating_mul(4).max(1) || right <= left {
        return Vec::new();
    }
    let mut runs = Vec::new();
    let mut start = None;
    let row = y * width * 4;
    for x in left..right {
        let pixel = &pixels[row + x * 4..row + x * 4 + 4];
        let dark = pixel[0] < 58 && pixel[1] < 55 && pixel[2] < 48;
        match (start, dark) {
            (None, true) => start = Some(x),
            (Some(begin), false) => {
                runs.push((begin, x));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        runs.push((begin, right));
    }
    runs
}

fn valid_card_row(runs: &[CardEdgeRun]) -> Option<[CardEdgeRun; 5]> {
    let row: [CardEdgeRun; 5] = runs.try_into().ok()?;
    let average_width = row.iter().map(|run| run.width()).sum::<usize>() as f32 / 5.0;
    if row.iter().any(|run| {
        (run.width() as f32) < average_width * 0.70 || (run.width() as f32) > average_width * 1.30
    }) {
        return None;
    }
    let centers: Vec<f32> = row.iter().map(|run| run.center()).collect();
    let gaps: Vec<f32> = centers.windows(2).map(|pair| pair[1] - pair[0]).collect();
    let average_gap = gaps.iter().sum::<f32>() / gaps.len() as f32;
    if average_gap < average_width * 1.12
        || average_gap > average_width * 1.90
        || gaps
            .iter()
            .any(|gap| *gap < average_gap * 0.82 || *gap > average_gap * 1.18)
    {
        return None;
    }
    Some(row)
}

fn is_gold(bgra: &[u8]) -> bool {
    let b = bgra[0] as i16;
    let g = bgra[1] as i16;
    let r = bgra[2] as i16;

    // 金框有少量明暗渐变；与聊天文字、绿色装备词条和灰色卡框分离。
    r >= 150 && g >= 105 && b <= 145 && r >= g + 16 && g >= b + 16 && r + g >= 335
}

fn is_card_edge(bgra: &[u8]) -> bool {
    let b = bgra[0] as i16;
    let g = bgra[1] as i16;
    let r = bgra[2] as i16;

    // 未选牌底边是低饱和的暖灰/金棕色，和黑色面板、绿色装备文字区分开。
    r >= 100 && g >= 85 && b <= 150 && r >= g - 15 && g >= b - 30 && r + g + b >= 280
}

fn slot_from_complete_card_row(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    left: usize,
    right: usize,
    selected_center: f32,
    selected_width: usize,
    selected_bottom: usize,
) -> Option<u8> {
    let search_top = selected_bottom.saturating_sub(7);
    let search_bottom = (selected_bottom + 7).min(window_height.saturating_sub(1));
    let minimum_width = selected_width * 60 / 100;
    let maximum_width = selected_width * 145 / 100;

    for y in search_top..=search_bottom {
        let runs = card_edge_runs(pixels, window_width, y, left, right, minimum_width);
        if runs.len() != 5 || runs.iter().any(|run| run.width() > maximum_width) {
            continue;
        }

        // 五张卡的外框宽度及相邻中心距离应相近。这里从实际底边测量，故弹窗
        // 整体平移时仍能从左数，而少一张、拼接的 UI 条目或不规则金色元素会失败。
        let centers: Vec<f32> = runs.iter().map(|run| run.center()).collect();
        let average_width = runs.iter().map(|run| run.width()).sum::<usize>() as f32 / 5.0;
        if runs.iter().any(|run| {
            (run.width() as f32) < average_width * 0.70
                || (run.width() as f32) > average_width * 1.30
        }) || (selected_width as f32) < average_width * 0.72
            || (selected_width as f32) > average_width * 1.30
        {
            continue;
        }
        let gaps: Vec<f32> = centers.windows(2).map(|pair| pair[1] - pair[0]).collect();
        let average_gap = gaps.iter().sum::<f32>() / gaps.len() as f32;
        if average_gap < average_width * 1.12
            || average_gap > average_width * 1.90
            || gaps
                .iter()
                .any(|gap| *gap < average_gap * 0.82 || *gap > average_gap * 1.18)
        {
            continue;
        }

        for (index, run) in runs.iter().enumerate() {
            // 金色外框必须落在一张实际卡的底边区间，而不是介于两张卡之间。
            if (selected_center - run.center()).abs() <= average_width * 0.34 {
                return Some((index + 1) as u8);
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct CardEdgeRun {
    start: usize,
    end: usize,
}

impl CardEdgeRun {
    fn width(self) -> usize {
        self.end - self.start + 1
    }

    fn center(self) -> f32 {
        (self.start + self.end) as f32 / 2.0
    }
}

fn card_edge_runs(
    pixels: &[u8],
    width: usize,
    y: usize,
    left: usize,
    right: usize,
    minimum_width: usize,
) -> Vec<CardEdgeRun> {
    const MAX_GAP: usize = 3;
    const MAX_RUNS: usize = 8;
    let row = y * width * 4;
    let mut runs = Vec::new();
    let mut start = None;
    let mut last_edge = 0usize;
    let mut gap = 0usize;

    for x in left..right {
        if is_card_edge(&pixels[row + x * 4..row + x * 4 + 4]) {
            if start.is_none() {
                start = Some(x);
            }
            last_edge = x;
            gap = 0;
        } else if start.is_some() {
            gap += 1;
            if gap > MAX_GAP {
                let run_start = start.take().unwrap_or_default();
                if last_edge - run_start + 1 >= minimum_width {
                    runs.push(CardEdgeRun {
                        start: run_start,
                        end: last_edge,
                    });
                    if runs.len() > MAX_RUNS {
                        return Vec::new();
                    }
                }
                gap = 0;
            }
        }
    }
    if let Some(run_start) = start {
        if last_edge - run_start + 1 >= minimum_width {
            runs.push(CardEdgeRun {
                start: run_start,
                end: last_edge,
            });
        }
    }
    runs
}

fn bands(values: &[usize], start: usize, end: usize, threshold: usize) -> Vec<Band> {
    let mut result = Vec::new();
    let mut cursor = start;
    while cursor < end {
        if values[cursor] < threshold {
            cursor += 1;
            continue;
        }

        let band_start = cursor;
        cursor += 1;
        while cursor < end && values[cursor] >= threshold {
            cursor += 1;
        }
        result.push(Band {
            start: band_start,
            end: cursor - 1,
        });
    }
    result
}

fn horizontal_gold_counts(
    pixels: &[u8],
    width: usize,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
) -> Vec<usize> {
    let mut counts = vec![0usize; bottom - top];
    for (offset, y) in (top..bottom).enumerate() {
        let row = y * width * 4;
        for x in left..=right {
            if is_gold(&pixels[row + x * 4..row + x * 4 + 4]) {
                counts[offset] += 1;
            }
        }
    }
    counts
}

fn resolve_frame(frames: Vec<Frame>) -> Option<u8> {
    if frames.is_empty() {
        return None;
    }

    // 同一条边可能因抗锯齿形成相邻候选；但只要有两处不同的完整金框，便无法
    // 确认真正的翻牌，不以亮度分数强行选一张。
    let best = frames[0];
    for frame in frames.iter().skip(1) {
        let same_frame = frame.slot == best.slot
            && (frame.center_x - best.center_x).abs() <= best.width.max(frame.width) as f32 / 4.0;
        if !same_frame {
            return None;
        }
    }
    Some(best.slot)
}

/// 仅供试炼采样链路测试复用的五牌结算页合成帧。
///
/// 生产识别不会调用此函数；测试统一使用与本模块牌位单测相同的边框、间距和
/// 分辨率比例，避免另一套示意图掩盖采样状态机之间的不兼容。
#[cfg(test)]
pub(super) fn synthetic_settlement(width: usize, height: usize, selected: &[u8]) -> Vec<u8> {
    tests::settlement(width, height, selected)
}

/// 仅供选择器动画回归测试复用的结算窗夹具。
#[cfg(test)]
pub(super) fn synthetic_selector_panel(width: usize, height: usize, selected: &[u8]) -> Vec<u8> {
    tests::selector_panel(width, height, selected)
}

#[cfg(test)]
mod tests {
    use super::{
        card_backs_visible, card_observation, cards_visible, flipped_slot, selector_visible,
        tooltip_candidate_rects,
    };

    const GOLD: [u8; 4] = [56, 194, 244, 255]; // BGRA，接近结算页金框中间色。
    const MUTED: [u8; 4] = [98, 161, 175, 255];
    const TOOLTIP: [u8; 4] = [36, 31, 27, 255];

    #[test]
    fn identifies_each_slot_at_different_sizes() {
        for (width, height, selected) in [(1280, 720, 1), (1920, 1080, 3), (2560, 1392, 5)] {
            let image = settlement(width, height, &[selected]);
            assert_eq!(flipped_slot(&image, width, height), Some(selected));
            assert!(cards_visible(&image, width, height));
        }
    }

    #[test]
    fn sees_complete_cards_before_any_card_is_flipped() {
        for (width, height) in [(1280, 720), (1920, 1080), (2560, 1392)] {
            let image = settlement(width, height, &[]);
            assert_eq!(flipped_slot(&image, width, height), None);
            assert!(cards_visible(&image, width, height));
            assert!(card_backs_visible(&image, width, height));
        }
    }

    #[test]
    fn selector_keeps_the_panel_during_card_flip_animation() {
        let width = 1920;
        let height = 1080;
        let mut animation = selector_panel(width, height, &[2]);
        erase_synthetic_cards(&mut animation, width, height);
        assert!(!cards_visible(&animation, width, height));
        assert_eq!(
            selector_visible(&animation, width, height, false),
            Some(true)
        );

        let mut header_missing = animation.clone();
        fill(
            &mut header_missing,
            width,
            width * 25 / 100,
            height * 17 / 100,
            width * 75 / 100,
            height * 28 / 100,
            [0, 0, 0, 255],
        );
        assert_eq!(
            selector_visible(&header_missing, width, height, false),
            None
        );

        let mut footer_missing = animation.clone();
        fill(
            &mut footer_missing,
            width,
            width * 42 / 100,
            height * 58 / 100,
            width * 72 / 100,
            height * 68 / 100,
            [0, 0, 0, 255],
        );
        assert_eq!(
            selector_visible(&footer_missing, width, height, false),
            None
        );

        let empty = vec![0; width * height * 4];
        assert_eq!(selector_visible(&empty, width, height, false), Some(false));
        assert_eq!(selector_visible(&empty, width, height, true), Some(true));
        assert_eq!(
            selector_visible(&empty[..empty.len() - 1], width, height, false),
            None
        );

        let mut ordinary_gold = vec![0; width * height * 4];
        outline(
            &mut ordinary_gold,
            width,
            width * 42 / 100,
            height * 42 / 100,
            width * 52 / 100,
            height * 54 / 100,
            GOLD,
        );
        assert_eq!(
            selector_visible(&ordinary_gold, width, height, false),
            Some(false)
        );
    }

    #[test]
    fn card_backs_require_every_card_to_keep_the_same_back_emblem() {
        let width = 1920;
        let height = 1080;
        let backs = settlement(width, height, &[]);
        assert!(card_backs_visible(&backs, width, height));

        // 已翻奖励即使金框被 Tooltip 遮住，也不能把余下牌底边当作新一轮牌背。
        let mut selected = settlement(width, height, &[2]);
        fill(
            &mut selected,
            width,
            width * 49 / 100,
            height * 39 / 100,
            width * 61 / 100,
            height * 54 / 100,
            TOOLTIP,
        );
        assert!(!card_backs_visible(&selected, width, height));

        // 五张已翻但没有金框的同类奖励也必须排除；完整牌底边本身不能作为牌背。
        let mut faces_without_gold = settlement(width, height, &[]);
        erase_synthetic_back_emblems(&mut faces_without_gold, width, height);
        assert!(cards_visible(&faces_without_gold, width, height));
        assert!(!card_backs_visible(&faces_without_gold, width, height));

        // 五张同色实心图标会彼此高度相似，却没有徽记的留白与笔画结构。
        let mut uniform_rewards = settlement(width, height, &[]);
        fill_card_signature_areas(&mut uniform_rewards, width, height, [95, 145, 155, 255]);
        assert!(!card_backs_visible(&uniform_rewards, width, height));
    }

    #[test]
    fn keeps_selected_slot_when_tooltip_covers_another_card() {
        let width = 2560;
        let height = 1392;
        let mut image = settlement(width, height, &[2]);
        // 模拟实际截图：提示框压住第三张牌，但第二张金框仍完整可见。
        fill(
            &mut image,
            width,
            width * 49 / 100,
            height * 39 / 100,
            width * 61 / 100,
            height * 54 / 100,
            TOOLTIP,
        );
        assert_eq!(flipped_slot(&image, width, height), Some(2));
    }

    #[test]
    fn distinguishes_hovered_reward_from_selected_card() {
        let width = 2560;
        let height = 1392;
        let image = settlement(width, height, &[2]);
        let third_center = width / 2;
        let cursor = Some((third_center - width * 4 / 1000, height * 50 / 100));

        assert_eq!(flipped_slot(&image, width, height), Some(2));
        assert_eq!(
            card_observation(&image, width, height, cursor)
                .unwrap()
                .hovered_slot,
            Some(3)
        );
    }

    #[test]
    fn rejects_hover_outside_card_even_when_settlement_is_visible() {
        let width = 1920;
        let height = 1080;
        let image = settlement(width, height, &[2]);
        assert_eq!(
            card_observation(&image, width, height, Some((40, 40)))
                .unwrap()
                .hovered_slot,
            None
        );
        assert_eq!(
            card_observation(&image, width, height, None)
                .unwrap()
                .hovered_slot,
            None
        );
    }

    #[test]
    fn tooltip_candidates_stay_in_frame_on_both_sides_of_cursor() {
        for cursor in [Some((20, 30)), Some((1900, 1040)), Some((960, 540))] {
            let candidates = tooltip_candidate_rects(1920, 1080, cursor).unwrap();
            assert!(candidates.len() <= 2);
            for candidate in candidates {
                assert!(candidate.left < candidate.right);
                assert!(candidate.top < candidate.bottom);
                assert!(candidate.right <= 1920);
                assert!(candidate.bottom <= 1080);
            }
        }
        assert_eq!(tooltip_candidate_rects(1920, 1080, None), None);
    }

    #[test]
    fn card_observation_requires_tooltip_next_to_the_hovered_card() {
        let width = 1920;
        let height = 1080;
        let cursor = Some((width / 2 - width * 4 / 1000, height * 50 / 100));
        let mut image = settlement(width, height, &[2]);
        for pixel in image.chunks_exact_mut(4) {
            if pixel == [0, 0, 0, 0] {
                pixel.copy_from_slice(&[104, 80, 65, 255]);
            }
        }
        let observation = card_observation(&image, width, height, cursor).unwrap();
        assert_eq!(observation.selected_slot, 2);
        assert_eq!(observation.hovered_slot, Some(3));
        assert!(observation.tooltip_rects.is_empty());

        // 模拟鼠标右侧的物品说明窗。它和远处的装备栏不同，左边界紧挨鼠标。
        fill(
            &mut image,
            width,
            width / 2 + 8,
            height * 32 / 100,
            width / 2 + 8 + width * 18 / 100,
            height * 45 / 100,
            TOOLTIP,
        );
        assert!(card_observation(&image, width, height, cursor)
            .unwrap()
            .tooltip_rects
            .is_empty());
        let x = width / 2 + 20;
        let y = height * 32 / 100 + 10;
        fill(&mut image, width, x, y, x + 60, y + 7, [220, 40, 230, 255]);
        let observation = card_observation(&image, width, height, cursor).unwrap();
        assert_eq!(observation.hovered_slot, Some(3));
        assert_eq!(observation.tooltip_rects.len(), 1);
    }

    #[test]
    fn returns_none_for_empty_or_ambiguous_frames() {
        let width = 1920;
        let height = 1080;
        let empty = vec![0; width * height * 4];
        assert_eq!(flipped_slot(&empty, width, height), None);

        let ambiguous = settlement(width, height, &[2, 4]);
        assert_eq!(flipped_slot(&ambiguous, width, height), None);
    }

    #[test]
    fn counts_from_the_actual_row_when_the_panel_shifts_by_one_card() {
        let width = 1920;
        let height = 1080;
        let shift = (width * 5 / 100 * 143 / 100) as isize;
        let image = settlement_with_layout(width, height, &[2], shift, &[1, 2, 3, 4, 5]);
        // 若固定把窗口中点认作第 3 张，这里会把实际第 2 张错误写成第 3 张。
        assert_eq!(flipped_slot(&image, width, height), Some(2));
    }

    #[test]
    fn rejects_a_row_with_one_end_card_missing() {
        let width = 1920;
        let height = 1080;
        let image = settlement_with_layout(width, height, &[2], 0, &[1, 2, 3, 4]);
        assert_eq!(flipped_slot(&image, width, height), None);
        assert!(!cards_visible(&image, width, height));
    }

    #[test]
    fn rejects_lone_gold_frame_even_at_a_valid_card_position() {
        let width = 1920;
        let height = 1080;
        let mut image = vec![0; width * height * 4];
        let card_width = width * 5 / 100;
        let card_height = card_width * 136 / 100;
        let spacing = card_width * 143 / 100;
        let center = width as isize / 2 + (2 - 3) * spacing as isize;
        let left = center as usize - card_width / 2;
        outline(
            &mut image,
            width,
            left,
            height * 45 / 100,
            left + card_width,
            height * 45 / 100 + card_height,
            GOLD,
        );
        assert_eq!(flipped_slot(&image, width, height), None);
    }

    #[test]
    fn rejects_unrelated_gold_rectangle() {
        let width = 1920;
        let height = 1080;
        let mut image = vec![0; width * height * 4];
        // 位置不在五牌等距网格，不能当作结算翻牌。
        outline(&mut image, width, 280, 510, 390, 660, GOLD);
        assert_eq!(flipped_slot(&image, width, height), None);
    }

    #[test]
    fn rejects_dense_non_settlement_gold_noise_without_excessive_candidates() {
        let width = 1920;
        let height = 1080;
        let mut image = vec![0; width * height * 4];
        // 超过候选竖线阈值的金色噪声：必须快速返回，而非二次组合扫描。
        for x in (width * 17 / 100..width * 83 / 100).step_by(12) {
            fill(
                &mut image,
                width,
                x,
                height * 32 / 100,
                x + 2,
                height * 75 / 100,
                GOLD,
            );
        }
        assert_eq!(flipped_slot(&image, width, height), None);
        assert!(!cards_visible(&image, width, height));
    }

    #[test]
    fn cards_visible_rejects_truncated_frames() {
        let width = 1920;
        let height = 1080;
        let image = settlement(width, height, &[]);
        assert!(!cards_visible(&image[..image.len() - 1], width, height));
        assert!(!cards_visible(&image, 639, height));
    }

    pub(super) fn settlement(width: usize, height: usize, selected: &[u8]) -> Vec<u8> {
        settlement_with_layout(width, height, selected, 0, &[1, 2, 3, 4, 5])
    }

    pub(super) fn selector_panel(width: usize, height: usize, selected: &[u8]) -> Vec<u8> {
        let mut image = settlement(width, height, selected);
        fill(
            &mut image,
            width,
            width * 25 / 100,
            height * 20 / 100,
            width * 75 / 100,
            height * 24 / 100,
            [70, 70, 45, 255],
        );
        let button_top = height * 61 / 100;
        let button_bottom = height * 64 / 100;
        for (left, right, color) in [
            (45, 51, [114, 118, 64, 255]),
            (52, 58, [90, 160, 190, 255]),
            (59, 65, [114, 118, 64, 255]),
        ] {
            fill(
                &mut image,
                width,
                width * left / 100,
                button_top,
                width * right / 100,
                button_bottom,
                color,
            );
        }
        image
    }

    fn erase_synthetic_cards(image: &mut [u8], width: usize, height: usize) {
        fill(
            image,
            width,
            width * 20 / 100,
            height * 42 / 100,
            width * 80 / 100,
            height * 59 / 100,
            [0, 0, 0, 255],
        );
    }

    fn settlement_with_layout(
        width: usize,
        height: usize,
        selected: &[u8],
        shift: isize,
        visible: &[u8],
    ) -> Vec<u8> {
        let mut image = vec![0; width * height * 4];
        let card_width = width * 5 / 100;
        let card_height = card_width * 136 / 100;
        let top = height * 45 / 100;
        let spacing = card_width * 143 / 100;
        for slot in 1..=5u8 {
            if !visible.contains(&slot) {
                continue;
            }
            let center = width as isize / 2 + shift + (slot as isize - 3) * spacing as isize;
            let left = center as usize - card_width / 2;
            outline(
                &mut image,
                width,
                left,
                top,
                left + card_width,
                top + card_height,
                if selected.contains(&slot) {
                    GOLD
                } else {
                    MUTED
                },
            );
            if selected.is_empty() {
                paint_synthetic_card_back(
                    &mut image,
                    width,
                    left,
                    top,
                    left + card_width,
                    top + card_height,
                );
            }
        }
        image
    }

    fn erase_synthetic_back_emblems(image: &mut [u8], width: usize, height: usize) {
        fill_card_signature_areas(image, width, height, [30, 29, 27, 255]);
    }

    fn fill_card_signature_areas(image: &mut [u8], width: usize, height: usize, color: [u8; 4]) {
        let card_width = width * 5 / 100;
        let card_height = card_width * 136 / 100;
        let top = height * 45 / 100;
        let spacing = card_width * 143 / 100;
        for slot in 1..=5isize {
            let center = width as isize / 2 + (slot - 3) * spacing as isize;
            let left = center as usize - card_width / 2;
            fill(
                image,
                width,
                left + card_width * 16 / 100,
                top + card_height * 13 / 100,
                left + card_width * 84 / 100,
                top + card_height * 83 / 100,
                color,
            );
        }
    }

    fn paint_synthetic_card_back(
        image: &mut [u8],
        width: usize,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
    ) {
        let card_width = right.saturating_sub(left);
        let card_height = bottom.saturating_sub(top);
        // 背面底色和中央剑形徽记分别模拟实拍帧的暗金底、淡金笔画；徽记由多个
        // 对称笔画组成，避免测试夹具只依赖大块同色填充。
        fill(
            image,
            width,
            left + card_width * 4 / 100,
            top + card_height * 4 / 100,
            right.saturating_sub(card_width * 4 / 100),
            bottom.saturating_sub(card_height * 4 / 100),
            [72, 91, 98, 255],
        );
        let ink = [95, 145, 155, 255];
        let center = left + card_width / 2;
        let x = |percent: usize| left + card_width * percent / 100;
        let y = |percent: usize| top + card_height * percent / 100;
        fill(image, width, x(44), y(17), x(56), y(82), ink);
        fill(image, width, x(35), y(26), x(65), y(36), ink);
        fill(image, width, x(27), y(38), x(73), y(48), ink);
        fill(image, width, x(35), y(58), x(65), y(68), ink);
        fill(image, width, x(42), y(70), x(58), y(78), ink);
        // 把中心线切出细缝，使掩码具有牌背徽记的对称纹理，而非实心矩形。
        fill(
            image,
            width,
            center.saturating_sub(card_width * 3 / 100),
            y(25),
            center.saturating_add(card_width * 3 / 100),
            y(74),
            [72, 91, 98, 255],
        );
    }

    fn outline(
        image: &mut [u8],
        width: usize,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
        color: [u8; 4],
    ) {
        for thickness in 0..3 {
            fill(
                image,
                width,
                left + thickness,
                top + thickness,
                right - thickness,
                top + thickness,
                color,
            );
            fill(
                image,
                width,
                left + thickness,
                bottom - thickness,
                right - thickness,
                bottom - thickness,
                color,
            );
            fill(
                image,
                width,
                left + thickness,
                top + thickness,
                left + thickness,
                bottom - thickness,
                color,
            );
            fill(
                image,
                width,
                right - thickness,
                top + thickness,
                right - thickness,
                bottom - thickness,
                color,
            );
        }
    }

    fn fill(
        image: &mut [u8],
        width: usize,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
        color: [u8; 4],
    ) {
        let height = image.len() / 4 / width;
        for y in top.min(height)..=bottom.min(height.saturating_sub(1)) {
            for x in left.min(width)..=right.min(width.saturating_sub(1)) {
                let offset = (y * width + x) * 4;
                image[offset..offset + 4].copy_from_slice(&color);
            }
        }
    }
}
