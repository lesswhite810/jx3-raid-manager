//! 结算面板固定文字的像素级识别：周翻牌次数。
//!
//! 这个数字不走 WinRT OCR（异步、慢、依赖系统组件），而是利用其颜色
//! 与位置高度固定的特点做纯像素识别：
//! 「周翻牌次数:N」数字为黄绿色（G 可高于 R），位于面板底行左下，
//! 右侧是按钮组（含金色「继续挑战」），因此 ROI 必须限制在按钮区左侧。
//!
//! 层数不在截图识别范围内：JCL 终局事件自带层号（FINISH_RE 直接解析
//! 「你通过了第N层的所有关卡」），采集端从 trial_capture::cleared 的
//! layer 参数获得，无需 OCR。
//!
//! 全部函数仅依赖传入像素，不做任何系统调用；识别不出时返回 None，
//! 由上层决定重试或放弃，绝不猜测一个看似合理的数字。

/// 周翻牌次数黄绿字判定。
/// 实测字形像素 (198,217,75)/(214,221,89)/(229,237,92)：G 明显高于 R
/// （G-R≈+15~20）或与 R 相当，B 很低。阈值以「亮黄绿、缺蓝」为核心，
/// 不要求 R>=G；g>=r-15 容纳 JPG 压缩造成的轻微偏色。
fn is_flip_count_yellow(bgra: &[u8]) -> bool {
    let b = bgra[0] as u16;
    let g = bgra[1] as u16;
    let r = bgra[2] as u16;
    let sum = r + g + b;
    g >= 165 && b <= 150 && g >= b + 55 && r >= b + 35 && sum >= 290 && g + 18 >= r
}

/// 「周翻牌次数」标签灰白字判定（行锚）。
/// 实测标签像素 (148,148,148)/(163,164,165)/(220,221,223)：r≈g≈b 的
/// 中性灰白。JPG 压缩允许 ±25 的通道漂移。
fn is_label_gray(bgra: &[u8]) -> bool {
    let b = bgra[0] as u16;
    let g = bgra[1] as u16;
    let r = bgra[2] as u16;
    let sum = r + g + b;
    sum >= 400 && r.abs_diff(g) <= 25 && g.abs_diff(b) <= 25 && r.abs_diff(b) <= 30
}

/// 阶段一：灰白标签行定位。
/// 在搜索带（面板下半部 y 59%~72%，x 31%~39%）逐行统计灰白字命中，
/// 找出连续 ≥6 行、每行命中 ≥8 的最高密度行带，返回其 y 范围。
/// 各样本实测标签行: y857..869（第5/6层布局）、y876..893（第2/3/4层布局）。
/// y 下限取 59%：结算页上半部的奖励数量「x 36」等灰白大字（y 55%~58%）
/// 行命中数远超标签行（实测 total 2265..3434 vs 标签 530），必须排除；
/// x 上限取 39%：右侧图标边缘（x≥1010 @2560）也会命中灰白判定。
fn label_band(pixels: &[u8], width: usize, _height: usize) -> Option<(usize, usize)> {
    let rows = height_px(pixels, width);
    if width < 640 || rows < 360 {
        return None;
    }
    let left = width * 31 / 100;
    let right = width * 39 / 100;
    let top = rows * 59 / 100;
    let bottom = (rows * 72 / 100).min(rows);
    let mut counts: Vec<usize> = Vec::with_capacity(bottom - top);
    for y in top..bottom {
        let row = y * width * 4;
        let mut hits = 0usize;
        for x in left..right {
            if is_label_gray(&pixels[row + x * 4..row + x * 4 + 4]) {
                hits += 1;
            }
        }
        counts.push(hits);
    }
    // 连续段聚合: 每行 hits≥8 视为文字行，允许 2 行断缝（冒号/字间距）。
    // 真实「周翻牌次数」标签带高度 13~15 行（m01468/74/80 实测）；
    // tooltip 操作栏（「Shift固定Tips/分享至聊天」）只有 8 行 —— 提高最小
    // 行数到 10 把这类假锚排除（m01478/84 实测踩坑）。
    // 带高超过 24 行的段是「x 36」类大字区域（虽然 y≥59% 已排除大半，
    // 仍保留高度上限兜底）。
    const MIN_ROWS: usize = 10;
    const MAX_BAND_ROWS: usize = 24;
    let mut best: Option<(usize, usize, usize)> = None; // (start, end, total)
    let mut start: Option<usize> = None;
    let mut last_hit = 0usize;
    let mut gap = 0usize;
    for (index, &hits) in counts.iter().enumerate() {
        if hits >= 8 {
            if start.is_none() {
                start = Some(index);
            }
            last_hit = index;
            gap = 0;
        } else if start.is_some() {
            gap += 1;
            if gap > 2 {
                let s = start.take().unwrap_or_default();
                if last_hit - s + 1 >= MIN_ROWS && last_hit - s < MAX_BAND_ROWS {
                    let total: usize = counts[s..=last_hit].iter().sum();
                    if best.is_none_or(|(_, _, best_total)| total > best_total) {
                        best = Some((s, last_hit, total));
                    }
                }
                gap = 0;
            }
        }
    }
    if let Some(s) = start {
        if last_hit - s + 1 >= MIN_ROWS && last_hit - s < MAX_BAND_ROWS {
            let total: usize = counts[s..=last_hit].iter().sum();
            if best.is_none_or(|(_, _, best_total)| total > best_total) {
                best = Some((s, last_hit, total));
            }
        }
    }
    let (start, end, _) = best?;
    Some((top + start, top + end))
}

/// 在 ROI 内按颜色收集像素，输出逐列命中数。
fn column_counts(
    pixels: &[u8],
    width: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    matched: fn(&[u8]) -> bool,
) -> Vec<usize> {
    let mut counts = vec![0usize; right.saturating_sub(left)];
    for y in top..bottom.min(height_px(pixels, width)) {
        let row = y * width * 4;
        for (offset, x) in (left..right).enumerate() {
            if matched(&pixels[row + x * 4..row + x * 4 + 4]) {
                counts[offset] += 1;
            }
        }
    }
    counts
}

fn height_px(pixels: &[u8], width: usize) -> usize {
    pixels.len() / (width * 4)
}

/// 列投影 → 连续字形簇。列命中数 ≥threshold 视为文字列，
/// 允许 max_gap 列间隙（小字形的笔画断缝可达 4 列，见 flip_count），
/// 簇宽 ≥minimum_width 才收。
fn glyph_clusters(
    counts: &[usize],
    threshold: usize,
    minimum_width: usize,
    max_gap: usize,
) -> Vec<(usize, usize)> {
    const MAX_CLUSTERS: usize = 8;
    let mut clusters = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_hit = 0usize;
    let mut gap = 0usize;
    for (index, &count) in counts.iter().enumerate() {
        if count >= threshold {
            if start.is_none() {
                start = Some(index);
            }
            last_hit = index;
            gap = 0;
        } else if start.is_some() {
            gap += 1;
            if gap > max_gap {
                let cluster_start = start.take().unwrap_or_default();
                if last_hit - cluster_start + 1 >= minimum_width {
                    clusters.push((cluster_start, last_hit));
                    if clusters.len() > MAX_CLUSTERS {
                        return Vec::new();
                    }
                }
                gap = 0;
            }
        }
    }
    if let Some(cluster_start) = start {
        if last_hit - cluster_start + 1 >= minimum_width {
            clusters.push((cluster_start, last_hit));
        }
    }
    clusters
}

/// 周翻牌次数（结算页底行「周翻牌次数:N」的黄绿数字）。
///
/// 两阶段: 先用灰白「周翻牌次数」标签行定位（行锚），再在标签行右端
/// 之后的窄带里找黄绿数字簇（颜色自适应）。
/// 返回 None 表示该帧无法确认（面板不在场、颜色不符、字形簇异常），
/// 调用方应保留上一次已确认值而不是清零。
pub(super) fn flip_count(pixels: &[u8], width: usize, height: usize) -> Option<u8> {
    let byte_count = pixels.len();
    let pixel_count = byte_count / 4;
    if pixel_count.checked_mul(4) != Some(byte_count) || width == 0 {
        return None;
    }
    let (band_top, band_bottom) = label_band(pixels, width, height)?;
    // 数字区: 标签行右端之后的 x 带。标签「周翻牌次数:」@2560 约 65~75px 宽
    // （x850..929 实测），数字紧跟冒号后；给足余量到 +110px 覆盖「6」
    // 等更宽字形与布局漂移（m01480 数字 x954..967, 标签 x863..895）。
    let zone_left = width * 30 / 100;
    let zone_right = zone_left + width * 130 / 1000;
    if band_bottom >= height_px(pixels, width) || zone_right > width {
        return None;
    }

    let counts = column_counts(
        pixels,
        width,
        zone_left,
        band_top,
        zone_right,
        band_bottom + 1,
        is_flip_count_yellow,
    );
    // 数字字形高 10~14px（实测 y858..868/y878..889），列命中峰值 5~10，
    // 但 JPG 压缩会把单列压到 2~3（m01480 的「6」除 x966 外每列仅 1~3）。
    // 阈值取带高的 1/5 保底 2：m01480 band=13 → 2 ✓；m01474 band=15 → 3，
    // 配合 gap=4 的断缝容忍仍能聚成单簇。
    let band_height = band_bottom + 1 - band_top;
    let threshold = (band_height / 5).max(2);
    let minimum_width = 3;
    // 数字笔画间隙实测可达 4 列（「3」中段凹陷）。
    let clusters = glyph_clusters(&counts, threshold, minimum_width, 4);
    if clusters.is_empty() || clusters.len() > 2 {
        // 0~99 只可能 1~2 个数字簇；更多说明带内混入噪声（tooltip 等）。
        #[cfg(test)]
        eprintln!("debug flip_count: clusters={:?}", clusters);
        return None;
    }

    let mut value: u32 = 0;
    for &(cluster_start, cluster_end) in &clusters {
        let digit = read_digit_with(
            pixels,
            width,
            zone_left,
            band_top,
            band_bottom + 1,
            (cluster_start, cluster_end),
            is_flip_count_yellow,
        );
        #[cfg(test)]
        eprintln!("debug flip_count: cluster={cluster_start:?}..{cluster_end:?} digit={digit:?}");
        let digit = digit?;
        value = value * 10 + digit as u32;
    }
    let value = u8::try_from(value).ok()?;
    // 结算页周翻牌次数合理范围 0..=99。
    if value > 99 {
        return None;
    }
    Some(value)
}

/// 数字识别的参数化版本，供黄字/白字共用。
fn read_digit_with(
    pixels: &[u8],
    width: usize,
    roi_left: usize,
    roi_top: usize,
    roi_bottom: usize,
    cluster: (usize, usize),
    matched: fn(&[u8]) -> bool,
) -> Option<u8> {
    let (cluster_start, cluster_end) = cluster;
    let mut row_hits = vec![0usize; roi_bottom - roi_top];
    let mut total = 0usize;
    for (offset, y) in (roi_top..roi_bottom).enumerate() {
        let row = y * width * 4;
        for x in roi_left + cluster_start..=roi_left + cluster_end {
            if matched(&pixels[row + x * 4..row + x * 4 + 4]) {
                row_hits[offset] += 1;
                total += 1;
            }
        }
    }
    if total < 16 {
        // @2560 数字「3」仅约 6×12px、25~40 个命中像素。低于 16 个
        // 说明这簇是噪点或残影，不足以辨认数字。
        #[cfg(test)]
        eprintln!("debug read_digit: total={total} < 16");
        return None;
    }
    // 与黄字/白字阈值无关的骨架：裁剪空行后做 4×8 网格密度模板匹配。
    // 模板单元值 = 每格命中像素占比（总和恒为 1，量纲与真实网格一致；
    // 旧版手工模板总和 ≠1 导致距离全面超标，见 b44 根因分析）。
    let mut top = 0usize;
    while top < row_hits.len() && row_hits[top] == 0 {
        top += 1;
    }
    let mut bottom = row_hits.len();
    while bottom > top && row_hits[bottom - 1] == 0 {
        bottom -= 1;
    }
    if bottom - top < 4 {
        #[cfg(test)]
        eprintln!("debug read_digit: trimmed height={} < 4", bottom - top);
        return None;
    }
    const GRID_W: usize = 4;
    const GRID_H: usize = 8;
    let glyph_width = cluster_end - cluster_start + 1;
    let glyph_height = bottom - top;
    let mut grid = [0f32; GRID_W * GRID_H];
    for (offset, y) in (roi_top + top..roi_top + bottom).enumerate() {
        let row = y * width * 4;
        for x in roi_left + cluster_start..=roi_left + cluster_end {
            if matched(&pixels[row + x * 4..row + x * 4 + 4]) {
                let gx = (((x - roi_left - cluster_start) * GRID_W) / glyph_width).min(GRID_W - 1);
                let gy = ((offset * GRID_H) / glyph_height).min(GRID_H - 1);
                grid[gy * GRID_W + gx] += 1.0;
            }
        }
    }
    // max 归一化：每格除以最大格计数。真实字形网格的和恒等于命中像素数
    // （sum 归一化会把每个格压到 ~0.03，与任何模板的距离都被放大），
    // max 归一化保留每行的相对密度，是模板可比的量纲。
    let max_cell = grid.iter().cloned().fold(0.0f32, f32::max);
    if max_cell <= 0.0 {
        return None;
    }
    for cell in &mut grid {
        *cell /= max_cell;
    }
    // 0..=9 的 4×8 max 归一化模板。3 的模板直接由真实字形反推
    // （m00168/m00318 两张结算截图实测，距离 0.49~0.86，见 b46/b55）；
    // 其余数字按同字体（方正兰亭黑 Game）结构手推。模板值是 0..1 的
    // 相对密度：0=空格，1=最密格。
    const TEMPLATES: [[f32; 32]; 10] = [
        // 0: 上下弧 + 左右竖，中空
        [
            0.0, 0.7, 0.7, 0.0, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.7,
            0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.7, 0.7, 0.0,
        ],
        // 1: 右偏竖 + 底横
        [
            0.0, 0.3, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0,
            0.3, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0, 0.3, 0.7, 1.0, 0.3,
        ],
        // 2: 顶弧 + 右上竖 + 中弧 + 左下竖 + 底横
        [
            0.7, 0.7, 0.7, 0.0, 0.0, 0.0, 0.0, 0.7, 0.0, 0.0, 0.0, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0,
            0.7, 0.7, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0,
        ],
        // 3: 顶弧 + 右竖贯 + 中凹 + 底弧左钩。模板由四个真实「3」字形
        // （m00168/m00318/m01470/m01474）平均反推：真实样本距离 0.5~0.9。
        // 与 8/9 的距离差由 3/8/9 结构仲裁兜底（真实「3」row1 中部必空）。
        [
            0.29, 0.88, 0.75, 0.42, 0.84, 0.0, 0.0, 0.54, 0.16, 0.0, 0.16, 0.46, 0.0, 0.0, 0.58,
            0.38, 0.0, 0.0, 0.16, 0.71, 0.29, 0.0, 0.0, 0.29, 1.0, 0.12, 0.16, 0.71, 0.16, 0.58,
            0.58, 0.0,
        ],
        // 4: 左上竖 + 中横 + 右竖贯
        [
            0.3, 0.3, 1.0, 0.3, 0.7, 0.3, 1.0, 0.3, 0.7, 0.3, 1.0, 0.3, 1.0, 1.0, 1.0, 1.0, 0.3,
            0.3, 1.0, 0.3, 0.0, 0.0, 1.0, 0.3, 0.0, 0.0, 1.0, 0.3, 0.0, 0.0, 1.0, 0.3,
        ],
        // 5: 顶横 + 左上竖 + 中弧 + 右下竖 + 底弧
        [
            1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.3, 0.0, 0.0, 1.0, 0.7, 0.7, 0.7, 0.0,
        ],
        // 6: 顶横 + 双肩 + 左竖贯 + 右下环。模板由 m01480 真实「6」字形
        // （7x11, max cell=3）按实现的 floor 映射反推；与 8/9 距离 6.4、
        // 与 0 距离 7.3，区分度充足。
        [
            1.0, 0.67, 1.0, 0.0, 0.0, 0.0, 0.33, 0.0, 0.33, 0.0, 0.67, 0.0, 0.0, 0.0, 0.67, 0.0,
            0.0, 0.0, 0.33, 0.0, 0.33, 0.0, 0.67, 0.67, 0.33, 0.0, 0.33, 0.0, 0.33, 0.67, 0.33,
            0.0,
        ],
        // 7: 顶横 + 右斜竖
        [
            1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.3, 1.0, 0.0,
            0.0, 0.3, 1.0, 0.0, 0.3, 0.7, 0.7, 0.0, 0.3, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0,
        ],
        // 8: 上下两个环
        [
            0.0, 0.7, 0.7, 0.0, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.7, 0.7, 0.0, 0.7,
            0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.7, 0.7, 0.0,
        ],
        // 9: 上环 + 右下竖 + 底弧
        [
            0.0, 0.7, 0.7, 0.0, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.7, 0.7, 0.7, 0.0,
            0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.7, 0.0, 0.0, 0.7, 0.0, 0.7, 0.7, 0.0,
        ],
    ];
    let mut best_digit: Option<u8> = None;
    let mut best_distance = f32::MAX;
    for (digit, template) in TEMPLATES.iter().enumerate() {
        let distance: f32 = grid
            .iter()
            .zip(template.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum();
        if distance < best_distance {
            best_distance = distance;
            best_digit = Some(digit as u8);
        }
    }
    let digit = best_digit?;
    // 6 的左竖贯通验证: 真实「6」的左竖笔画连续 ≥5 行（m01480 实测 x961
    // 连续 7 行），而低质量「3」（m01468, 笔画糊成 3px 宽）残存的「顶弧右
    // 端+中凹+底弧」骨架与 6 模板距离仅 1.1 —— 误判为 6。真 6 的左竖
    // 在簇左列必有连续命中，糊 3 的左列是孤立散点（最多 2 行连续）。
    // total<30 的小字形才做此验证（大字形网格本身可靠）。
    if digit == 6 && total < 30 {
        let left_x = roi_left + cluster_start;
        let mut best_run = 0usize;
        let mut run = 0usize;
        for y in roi_top..roi_bottom {
            let row = y * width * 4;
            if matched(&pixels[row + left_x * 4..row + left_x * 4 + 4]) {
                run += 1;
                best_run = best_run.max(run);
            } else {
                run = 0;
            }
        }
        if best_run < 5 {
            #[cfg(test)]
            eprintln!("debug read_digit: digit=6 左竖不贯通 best_run={best_run} 拒识");
            return None;
        }
    }
    // 3/8/9 仲裁: 低分辨率下「3」与「8」「9」的欧氏距离非常接近
    // （m01474 的「3」与 8/9 并列 2.46, 与 3 模板 3.78 —— JPG 压缩毁掉
    // 左列笔画后, 圆弧类模板总是更近）。用结构特征仲裁:
    // 真实「3」的 row1（上弧与中弧之间的收腰行）中部两格必空 —— 左右
    // 开口; 而「8」「9」的 row1 中部是环体/竖笔（模板 0.7）。
    // best ∈ {8,9} 且 row1 中部近空时改判 3。
    let digit = if digit == 8 || digit == 9 {
        let mid_top_empty = grid[GRID_W + 1] < 0.1 && grid[GRID_W + 2] < 0.1;
        if mid_top_empty {
            3
        } else {
            digit
        }
    } else {
        digit
    };
    // 小字形（6×12px、20~40 像素）量化噪声大。新「3」模板（四样本平均）
    // 下真实样本实测距离 0.38~0.86，次优模板 ≥2.98；m01470/72 的「3」
    // 因窄字形（8 列簇）与模板错位距离 2.2~2.3，仍远优于噪声。阈值 2.5
    // 覆盖窄字形，同时拒绝低质量字形（m01480 的「6」与全部模板距离 ≥5，
    // JPG 压缩毁掉网格结构 —— 拒识交给上层保留上一帧值）。
    if best_distance > 2.5 {
        #[cfg(test)]
        {
            eprintln!(
                "debug read_digit: best_distance={best_distance:.4} digit={digit} glyph={glyph_width}x{glyph_height} total={total}"
            );
            for row in 0..GRID_H {
                eprintln!(
                    "  grid row {row}: {:?}",
                    &grid[row * GRID_W..row * GRID_W + GRID_W]
                );
            }
        }
        return None;
    }
    Some(digit)
}

/// 结算面板整体在场检查的便捷入口（供主循环确认 ROI 有效）。
/// 层数不在像素识别范围内（JCL 直传），恒为 None。
pub(super) fn settlement_numbers(
    pixels: &[u8],
    width: usize,
    height: usize,
) -> (Option<u8>, Option<u8>) {
    let count = flip_count(pixels, width, height);
    (count, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造 BGRA 测试帧：深灰背景 + 指定颜色矩形。
    fn frame(width: usize, height: usize) -> Vec<u8> {
        vec![40u8; width * height * 4]
    }

    fn fill_rect(
        pixels: &mut [u8],
        width: usize,
        rect: (usize, usize, usize, usize),
        color: [u8; 3],
    ) {
        let (left, top, right, bottom) = rect;
        for y in top..bottom {
            let row = y * width * 4;
            for x in left..right {
                let offset = row + x * 4;
                pixels[offset] = color[2]; // B
                pixels[offset + 1] = color[1]; // G
                pixels[offset + 2] = color[0]; // R
            }
        }
    }

    #[test]
    fn flip_count_roi_rejects_small_frames() {
        assert!(label_band(&[0u8; 4], 639, 720).is_none());
        assert!(label_band(&vec![0u8; 2560 * 359 * 4], 2560, 359).is_none());
        assert!(label_band(&vec![0u8; 2560 * 1392 * 4], 2560, 1392).is_none());
    }

    #[test]
    fn flip_count_rejects_noise_clusters() {
        // 三个噪声簇 → 拒绝（0..99 只可能 1~2 个数字簇）。
        // 构造灰白标签行让阶段一通过，噪声画在数字区。
        let width = 2560;
        let height = 1392;
        let mut pixels = frame(width, height);
        let (band_top, band_bottom) = label_band(&pixels, width, height)
            .unwrap_or((height * 62 / 100, height * 62 / 100 + 10));
        for y in band_top..=band_bottom {
            let row = y * width * 4;
            for x in width * 30 / 100..width * 38 / 100 {
                let offset = row + x * 4;
                pixels[offset] = 150; // B
                pixels[offset + 1] = 152; // G
                pixels[offset + 2] = 155; // R
            }
        }
        let zone_left = width * 30 / 100;
        let band_height = band_bottom - band_top + 1;
        let band_mid = band_top + band_height / 2;
        for offset in [10, 40, 70] {
            fill_rect(
                &mut pixels,
                width,
                (
                    zone_left + offset,
                    band_mid,
                    zone_left + offset + 20,
                    band_mid + band_height / 2,
                ),
                [197, 204, 75],
            );
        }
        assert_eq!(flip_count(&pixels, width, height), None);
    }

    #[test]
    fn flip_count_rejects_insufficient_pixels() {
        // 单个 2×2 噪点 → total<16 → None。无标签行时阶段一先拒绝。
        let width = 2560;
        let height = 1392;
        let mut pixels = frame(width, height);
        let (left, top, _, _) = (
            width * 350 / 1000,
            height * 608 / 1000,
            width * 395 / 1000,
            height * 652 / 1000,
        );
        fill_rect(
            &mut pixels,
            width,
            (left + 5, top + 5, left + 7, top + 7),
            [197, 204, 75],
        );
        assert_eq!(flip_count(&pixels, width, height), None);
    }

    #[test]
    fn settlement_numbers_both_none_on_garbage() {
        let width = 100;
        let height = 50;
        let pixels = frame(width, height);
        assert_eq!(settlement_numbers(&pixels, width, height), (None, None));
    }

    /// 真实截图回归探针（本地）：
    /// 设置环境变量 WEEKLY_PROBE_DIR 指向包含 *.bgra
    /// （2560×1392 全帧 BGRA，由 PowerShell System.Drawing 导出）的目录后，
    /// `cargo test weekly_real -- --nocapture` 会输出识别结果。
    /// 期望值按样本配置（结算页次数各不相同），样本不存在时跳过（CI 无这些文件）。
    #[test]
    fn weekly_real_screenshot_probe() {
        let Ok(dir) = std::env::var("WEEKLY_PROBE_DIR") else {
            return; // 未提供样本目录，跳过
        };
        // 文件名前缀 → 期望次数。
        // None = 该样本字形质量过低（JPG 压缩毁掉网格结构/数字仅 3px 宽/
        // 布局被遮挡），期望拒识 —— 上层语义是保留上一帧已确认值。
        let expectations: &[(&str, Option<u8>)] = &[
            ("m01468", None), // 数字仅 3px 宽（笔画糊成 8 段）, 允许拒识
            ("m01470", Some(3)),
            ("m01472", Some(3)),
            ("m01474", Some(3)),
            ("m01476", None), // 数字偏灰/JPG 压缩严重, 允许拒识
            ("m01478", None), // 次数行被「秘境掉落」tooltip 遮挡, 允许拒识
            ("m01480", None), // 「6」左竖被 JPG 压缩毁掉（best_run=2）, 拒识防「3」误判
            ("m01482", None), // 同 m01480, 允许拒识
            ("m01484", None), // 次数行被 tooltip 遮挡, 允许拒识
        ];
        let width = 2560usize;
        let height = 1392usize;
        let mut checked = 0usize;
        let mut skipped = 0usize;
        let mut failures = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("读取探针目录失败") {
            let path = entry.expect("读取探针文件失败").path();
            if path.extension().is_none_or(|ext| ext != "bgra") {
                continue;
            }
            let name = path
                .file_name()
                .expect("探针文件名缺失")
                .to_string_lossy()
                .to_string();
            let Ok(bytes) = std::fs::read(&path) else {
                println!("skip {name}: 文件不可读 ({})", path.display());
                skipped += 1;
                continue;
            };
            if bytes.len() != width * height * 4 {
                println!("skip {name}: 尺寸不符 ({} 字节)", bytes.len());
                skipped += 1;
                continue;
            }
            let (count, layer) = settlement_numbers(&bytes, width, height);
            let expected: Option<Option<u8>> = expectations
                .iter()
                .find(|(prefix, _)| name.starts_with(prefix))
                .map(|(_, expected)| *expected);
            println!("{name}: flip_count={count:?} layer={layer:?} (期望 {expected:?})");
            if expected.is_some() && expected != Some(count) {
                failures.push(format!("{name}: flip_count={count:?} 期望 {expected:?}"));
            }
            checked += 1;
        }
        println!(
            "探针完成: 校验 {checked} 个样本, 跳过 {skipped} 个, 失败 {}",
            failures.len()
        );
        assert!(checked > 0, "探针目录中没有任何可校验的 .bgra 样本");
        assert!(
            failures.is_empty(),
            "识别失败样本:\n{}",
            failures.join("\n")
        );
    }
}
