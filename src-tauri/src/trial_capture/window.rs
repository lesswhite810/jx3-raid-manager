//! 仅针对前台单开 JX3 客户端的内存帧获取。
//!
//! 不激活窗口、不向窗口发送消息、也不写入图片文件。普通牌位观测只复制结算
//! 面板所在的中心区域；悬停装备需要时才复制完整客户区。连续观察复用 CPU 和
//! GDI 缓冲区，避免连续翻牌时反复分配整张屏幕和创建位图。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CaptureMode {
    Cards,
    Full,
}

/// 截图识别最多处理 4K 像素数，覆盖 3840×2160 与 5120×1440。超出时放弃像素
/// 识别，日志奖励和精力记录仍可正常处理，避免 8K 帧造成大块内存和 GDI 压力。
const MAX_CAPTURE_PIXELS: usize = 3_840 * 2_160;

fn capture_dimensions_allowed(width: usize, height: usize) -> bool {
    width != 0
        && height != 0
        && width
            .checked_mul(height)
            .is_some_and(|pixels| pixels <= MAX_CAPTURE_PIXELS)
}

/// `pixels` 始终保持完整客户区 BGRA 坐标系。Cards 模式区域外为黑色，避免
/// vision 读到上一帧 Full 的 Tooltip 或其它界面残留。
#[derive(Debug, Clone)]
pub(super) struct WindowFrame {
    pub pid: u32,
    pub width: usize,
    pub height: usize,
    pub cursor: Option<(usize, usize)>,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CaptureArea {
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
}

impl CaptureArea {
    fn width(self) -> usize {
        self.right.saturating_sub(self.left)
    }

    fn height(self) -> usize {
        self.bottom.saturating_sub(self.top)
    }
}

/// 与 vision.rs 的结算牌列比例一致，并保留结算页页眉以区分翻牌动画；仍只复制
/// 约 38% 客户区，外扩两个物理像素避免比例舍入截断金框。
fn capture_area(mode: CaptureMode, width: usize, height: usize) -> Option<CaptureArea> {
    if width == 0 || height == 0 {
        return None;
    }
    if mode == CaptureMode::Full {
        return Some(CaptureArea {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        });
    }
    const PAD: usize = 2;
    let left = (width * 17 / 100).saturating_sub(PAD);
    let top = (height * 18 / 100).saturating_sub(PAD);
    let right = (width * 83 / 100).saturating_add(PAD).min(width);
    let bottom = (height * 75 / 100).saturating_add(PAD).min(height);
    (right > left && bottom > top).then_some(CaptureArea {
        left,
        top,
        right,
        bottom,
    })
}

/// 准备可复用的完整客户区缓冲。只有 Cards 紧接 Full、首次取帧或尺寸变化时
/// 才清空整张缓冲；连续 Cards 的区域外已经是黑色，无需再写一遍。
fn prepare_frame_pixels(
    pixels: &mut Vec<u8>,
    previous_dimensions: Option<(usize, usize)>,
    previous_mode: Option<CaptureMode>,
    mode: CaptureMode,
    width: usize,
    height: usize,
) -> Option<bool> {
    let byte_length = width.checked_mul(height)?.checked_mul(4)?;
    let dimensions_changed = previous_dimensions != Some((width, height));
    let clear_cards_background = mode == CaptureMode::Cards
        && (dimensions_changed || previous_mode != Some(CaptureMode::Cards));
    pixels.resize(byte_length, 0);
    if clear_cards_background {
        pixels.fill(0);
    }
    Some(clear_cards_background)
}

/// 分辨率变化时必须释放旧的完整像素与 ROI 缓冲。仅 clear 会保留 Vec 容量，
/// 会让先前 4K 帧的内存一直留到观察结束。
fn discard_cpu_buffers(frame: &mut Option<WindowFrame>, readback: &mut Vec<u8>) {
    *frame = None;
    *readback = Vec::new();
}

/// 进程枚举完成后的 JX3 状态。只有完整枚举且没有任何 JX3 时才是 Offline；
/// Unavailable 表示无法确定，不能把临时系统错误误当作游戏退出。
#[derive(Debug, Clone, PartialEq, Eq)]
enum Jx3ProcessState {
    Offline,
    Single(u32),
    Multiple(Vec<u32>),
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionProcessDecision {
    Bind(u32),
    Continue,
    Skip,
    End,
}

/// 已绑定的 PID 消失或被单个新 PID 替换时，结算面板不可能再属于原会话，立刻
/// 结束观察。多开但原 PID 仍在、或枚举暂不可用时只跳过当前帧，避免误停。
fn session_process_decision(
    bound_pid: Option<u32>,
    process: &Jx3ProcessState,
) -> SessionProcessDecision {
    match (bound_pid, process) {
        (_, Jx3ProcessState::Offline) => SessionProcessDecision::End,
        (None, Jx3ProcessState::Single(pid)) => SessionProcessDecision::Bind(*pid),
        (Some(bound), Jx3ProcessState::Single(pid)) if bound == *pid => {
            SessionProcessDecision::Continue
        }
        (Some(_), Jx3ProcessState::Single(_)) => SessionProcessDecision::End,
        (Some(bound), Jx3ProcessState::Multiple(pids)) if !pids.contains(&bound) => {
            SessionProcessDecision::End
        }
        (_, Jx3ProcessState::Multiple(_)) | (_, Jx3ProcessState::Unavailable) => {
            SessionProcessDecision::Skip
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[derive(Default)]
pub(super) struct CaptureSession;

#[cfg(not(target_os = "windows"))]
impl CaptureSession {
    pub(super) fn capture_foreground(&mut self, _mode: CaptureMode) -> Option<&WindowFrame> {
        None
    }

    pub(super) fn session_ended(&self) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{
        GetLastError, ERROR_NO_MORE_FILES, HANDLE, HWND, INVALID_HANDLE_VALUE, POINT, RECT,
    };
    use windows_sys::Win32::Graphics::Gdi::{
        BitBlt, ClientToScreen, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        GetDC, GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, RGBQUAD, SRCCOPY,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetAncestor, GetClientRect, GetCursorPos, GetForegroundWindow, GetWindow, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, WindowFromPoint, GA_ROOT, GW_HWNDPREV,
    };

    use super::{
        capture_area, capture_dimensions_allowed, discard_cpu_buffers, prepare_frame_pixels,
        session_process_decision, CaptureArea, CaptureMode, Jx3ProcessState,
        SessionProcessDecision, WindowFrame,
    };

    const JX3_PROCESS_NAME: &str = "JX3ClientX64.exe";
    struct Snapshot(HANDLE);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            // SAFETY: ToolHelp 成功时返回的快照句柄必须关闭一次。
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.0) };
        }
    }

    struct ScreenDc(HDC);
    impl Drop for ScreenDc {
        fn drop(&mut self) {
            // SAFETY: GetDC(NULL) 获得的屏幕 DC 需由 ReleaseDC(NULL, ...) 归还。
            unsafe { ReleaseDC(null_mut(), self.0) };
        }
    }

    /// 一个连续观察会话复用同尺寸 GDI 表面。Readback 时短暂恢复 DC 的原对象，
    /// 读取后立即重新选回 bitmap，因此析构能安全恢复并释放句柄。
    struct ReusableSurface {
        dc: HDC,
        bitmap: HBITMAP,
        previous: HGDIOBJ,
        width: usize,
        height: usize,
    }

    impl ReusableSurface {
        fn new(screen_dc: HDC, width: usize, height: usize) -> Option<Self> {
            let width_i32 = i32::try_from(width).ok()?;
            let height_i32 = i32::try_from(height).ok()?;
            let dc = unsafe { CreateCompatibleDC(screen_dc) };
            if dc.is_null() {
                return None;
            }
            let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width_i32, height_i32) };
            if bitmap.is_null() {
                unsafe { DeleteDC(dc) };
                return None;
            }
            let previous = unsafe { SelectObject(dc, bitmap) };
            if previous.is_null() {
                unsafe {
                    DeleteObject(bitmap);
                    DeleteDC(dc);
                };
                return None;
            }
            Some(Self {
                dc,
                bitmap,
                previous,
                width,
                height,
            })
        }

        fn read_from_screen(
            &mut self,
            screen_dc: HDC,
            source_x: i32,
            source_y: i32,
            bytes: &mut [u8],
        ) -> bool {
            let (Some(width_i32), Some(height_i32), Some(height_u32)) = (
                i32::try_from(self.width).ok(),
                i32::try_from(self.height).ok(),
                u32::try_from(self.height).ok(),
            ) else {
                return false;
            };
            let Some(expected) = self
                .width
                .checked_mul(self.height)
                .and_then(|size| size.checked_mul(4))
            else {
                return false;
            };
            if bytes.len() != expected {
                return false;
            }
            if unsafe {
                BitBlt(
                    self.dc, 0, 0, width_i32, height_i32, screen_dc, source_x, source_y, SRCCOPY,
                )
            } == 0
            {
                return false;
            }
            // GetDIBits 要求位图不被 memory DC 选中。
            if unsafe { SelectObject(self.dc, self.previous) }.is_null() {
                return false;
            }
            let mut bitmap_info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width_i32,
                    biHeight: -height_i32,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    biSizeImage: u32::try_from(expected).unwrap_or(0),
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }],
            };
            let lines = unsafe {
                GetDIBits(
                    self.dc,
                    self.bitmap,
                    0,
                    height_u32,
                    bytes.as_mut_ptr().cast::<c_void>(),
                    &mut bitmap_info,
                    DIB_RGB_COLORS,
                )
            };
            let restored = unsafe { SelectObject(self.dc, self.bitmap) };
            lines == height_i32 && !restored.is_null()
        }
    }

    impl Drop for ReusableSurface {
        fn drop(&mut self) {
            // SAFETY: previous 由本 DC 首次 SelectObject 返回；恢复后才释放资源。
            unsafe {
                SelectObject(self.dc, self.previous);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            };
        }
    }

    /// 完整枚举当前进程快照。只在成功走到 ERROR_NO_MORE_FILES 时才认定 Offline。
    fn jx3_process_state() -> Jx3ProcessState {
        let handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if handle == INVALID_HANDLE_VALUE {
            return Jx3ProcessState::Unavailable;
        }
        let _snapshot = Snapshot(handle);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            cntUsage: 0,
            th32DefaultHeapID: 0,
            th32ModuleID: 0,
            cntThreads: 0,
            th32ParentProcessID: 0,
            pcPriClassBase: 0,
            dwFlags: 0,
            th32ProcessID: 0,
            szExeFile: [0; 260],
        };
        if unsafe { Process32FirstW(handle, &mut entry) } == 0 {
            return if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                Jx3ProcessState::Offline
            } else {
                Jx3ProcessState::Unavailable
            };
        }
        let mut pids = Vec::new();
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|&character| character == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..length])
                .eq_ignore_ascii_case(JX3_PROCESS_NAME)
            {
                pids.push(entry.th32ProcessID);
            }
            if unsafe { Process32NextW(handle, &mut entry) } == 0 {
                if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
                    return Jx3ProcessState::Unavailable;
                }
                break;
            }
        }
        match pids.len() {
            0 => Jx3ProcessState::Offline,
            1 => Jx3ProcessState::Single(pids[0]),
            _ => Jx3ProcessState::Multiple(pids),
        }
    }

    /// 不枚举进程的前台窗口查询。采样前做一次完整单开核验；采样后只验证该
    /// HWND/PID 和客户区仍一致，避免每帧生成两个全系统进程快照。
    fn foreground_window() -> Option<(HWND, u32, usize, usize, RECT)> {
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 || unsafe { IsIconic(hwnd) } != 0
        {
            return None;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let mut client = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetClientRect(hwnd, &mut client) } == 0 {
            return None;
        }
        let width = usize::try_from(client.right - client.left).ok()?;
        let height = usize::try_from(client.bottom - client.top).ok()?;
        if pid == 0 || !capture_dimensions_allowed(width, height) {
            return None;
        }
        let mut top_left = POINT { x: 0, y: 0 };
        let mut bottom_right = POINT {
            x: client.right,
            y: client.bottom,
        };
        if unsafe { ClientToScreen(hwnd, &mut top_left) } == 0
            || unsafe { ClientToScreen(hwnd, &mut bottom_right) } == 0
        {
            return None;
        }
        Some((
            hwnd,
            pid,
            width,
            height,
            RECT {
                left: top_left.x,
                top: top_left.y,
                right: bottom_right.x,
                bottom: bottom_right.y,
            },
        ))
    }

    fn cursor_in_client(rect: &RECT, width: usize, height: usize) -> Option<(usize, usize)> {
        let mut cursor = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor) } == 0 {
            return None;
        }
        let x = usize::try_from(cursor.x - rect.left).ok()?;
        let y = usize::try_from(cursor.y - rect.top).ok()?;
        (x < width && y < height).then_some((x, y))
    }

    fn rectangles_intersect(first: &RECT, second: &RECT) -> bool {
        first.left < second.right
            && first.right > second.left
            && first.top < second.bottom
            && first.bottom > second.top
    }

    fn client_area_is_unobscured(hwnd: HWND, rect: &RECT) -> bool {
        let mut candidate = unsafe { GetWindow(hwnd, GW_HWNDPREV) };
        let mut checked = 0usize;
        while !candidate.is_null() && checked < 128 {
            checked += 1;
            if unsafe { IsWindowVisible(candidate) } != 0 && unsafe { IsIconic(candidate) } == 0 {
                let mut candidate_rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if unsafe { GetWindowRect(candidate, &mut candidate_rect) } == 0
                    || rectangles_intersect(rect, &candidate_rect)
                {
                    return false;
                }
            }
            candidate = unsafe { GetWindow(candidate, GW_HWNDPREV) };
        }
        if !candidate.is_null() {
            return false;
        }
        let center = POINT {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        };
        let hit = unsafe { WindowFromPoint(center) };
        !hit.is_null() && unsafe { GetAncestor(hit, GA_ROOT) } == hwnd
    }

    pub(crate) struct CaptureSession {
        frame: Option<WindowFrame>,
        readback: Vec<u8>,
        cards_surface: Option<ReusableSurface>,
        full_surface: Option<ReusableSurface>,
        last_mode: Option<CaptureMode>,
        capture_dimensions: Option<(usize, usize)>,
        bound_pid: Option<u32>,
        ended: bool,
    }

    impl Default for CaptureSession {
        fn default() -> Self {
            Self {
                frame: None,
                readback: Vec::new(),
                cards_surface: None,
                full_surface: None,
                last_mode: None,
                capture_dimensions: None,
                bound_pid: None,
                ended: false,
            }
        }
    }

    impl CaptureSession {
        fn reset_for_resolution_change(&mut self, width: usize, height: usize) {
            if self.capture_dimensions == Some((width, height)) {
                return;
            }
            // 两套 GDI surface 也可能仍是上一分辨率；一并释放，直到当前模式
            // 再按需创建。这个分支仅发生在缩放/分辨率切换，不影响正常复用。
            discard_cpu_buffers(&mut self.frame, &mut self.readback);
            self.cards_surface = None;
            self.full_surface = None;
            self.last_mode = None;
            self.capture_dimensions = Some((width, height));
        }

        fn ensure_surface(
            &mut self,
            screen_dc: HDC,
            mode: CaptureMode,
            area: CaptureArea,
        ) -> Option<()> {
            let surface = match mode {
                CaptureMode::Cards => &mut self.cards_surface,
                CaptureMode::Full => &mut self.full_surface,
            };
            if surface.as_ref().is_some_and(|surface| {
                surface.width == area.width() && surface.height == area.height()
            }) {
                return Some(());
            }
            *surface = ReusableSurface::new(screen_dc, area.width(), area.height());
            surface.as_ref().map(|_| ())
        }

        pub(crate) fn capture_foreground(&mut self, mode: CaptureMode) -> Option<&WindowFrame> {
            if self.ended {
                return None;
            }
            // 每个 tick 都在读取前枚举一次；后面的前台复验不再重复枚举进程。
            match session_process_decision(self.bound_pid, &jx3_process_state()) {
                SessionProcessDecision::Bind(pid) => self.bound_pid = Some(pid),
                SessionProcessDecision::Continue => {}
                SessionProcessDecision::Skip => return None,
                SessionProcessDecision::End => {
                    self.ended = true;
                    return None;
                }
            }
            let (hwnd, pid, width, height, client_rect) = foreground_window()?;
            if self.bound_pid != Some(pid) || !client_area_is_unobscured(hwnd, &client_rect) {
                return None;
            }
            self.reset_for_resolution_change(width, height);
            let cursor_before = cursor_in_client(&client_rect, width, height);
            let area = capture_area(mode, width, height)?;
            let source_x = client_rect
                .left
                .checked_add(i32::try_from(area.left).ok()?)?;
            let source_y = client_rect.top.checked_add(i32::try_from(area.top).ok()?)?;
            let screen_dc = unsafe { GetDC(null_mut()) };
            if screen_dc.is_null() {
                return None;
            }
            let _screen_dc = ScreenDc(screen_dc);
            self.ensure_surface(screen_dc, mode, area)?;
            let readback_bytes = area.width().checked_mul(area.height())?.checked_mul(4)?;
            let previous_dimensions = self.frame.as_ref().map(|frame| (frame.width, frame.height));
            let previous_mode = self.last_mode;
            // Full 会直接写入完整客户区。即使随后的前台复验失败，下次 Cards
            // 仍必须清空区域外，不能把这次未交付的 Full 帧当成纯 Cards 帧。
            if mode == CaptureMode::Full {
                self.last_mode = Some(CaptureMode::Full);
            }
            let frame = self.frame.get_or_insert_with(|| WindowFrame {
                pid,
                width,
                height,
                cursor: None,
                pixels: Vec::new(),
            });
            frame.pid = pid;
            frame.width = width;
            frame.height = height;
            prepare_frame_pixels(
                &mut frame.pixels,
                previous_dimensions,
                previous_mode,
                mode,
                width,
                height,
            )?;
            let captured = match mode {
                CaptureMode::Cards => {
                    self.readback.resize(readback_bytes, 0);
                    self.cards_surface.as_mut().is_some_and(|surface| {
                        surface.read_from_screen(screen_dc, source_x, source_y, &mut self.readback)
                    })
                }
                CaptureMode::Full => self.full_surface.as_mut().is_some_and(|surface| {
                    surface.read_from_screen(screen_dc, source_x, source_y, &mut frame.pixels)
                }),
            };
            if !captured {
                return None;
            }
            let (verified_hwnd, verified_pid, verified_width, verified_height, verified_rect) =
                foreground_window()?;
            if verified_hwnd != hwnd
                || verified_pid != pid
                || verified_width != width
                || verified_height != height
                || verified_rect.left != client_rect.left
                || verified_rect.top != client_rect.top
                || verified_rect.right != client_rect.right
                || verified_rect.bottom != client_rect.bottom
                || !client_area_is_unobscured(hwnd, &client_rect)
            {
                return None;
            }
            let cursor = match (cursor_before, cursor_in_client(&client_rect, width, height)) {
                (Some(before), Some(after))
                    if before.0.abs_diff(after.0) <= 3 && before.1.abs_diff(after.1) <= 3 =>
                {
                    Some(after)
                }
                _ => None,
            };
            {
                let frame = self.frame.as_mut()?;
                frame.cursor = cursor;
                if mode == CaptureMode::Cards {
                    let row_bytes = area.width() * 4;
                    for row in 0..area.height() {
                        let source = row * row_bytes;
                        let destination = ((area.top + row) * width + area.left) * 4;
                        frame.pixels[destination..destination + row_bytes]
                            .copy_from_slice(&self.readback[source..source + row_bytes]);
                    }
                }
            }
            self.last_mode = Some(mode);
            self.frame.as_ref()
        }

        pub(crate) fn session_ended(&self) -> bool {
            self.ended
        }
    }
}

#[cfg(target_os = "windows")]
pub(super) use platform::CaptureSession;

#[cfg(test)]
mod tests {
    use super::{
        capture_area, capture_dimensions_allowed, discard_cpu_buffers, prepare_frame_pixels,
        session_process_decision, CaptureMode, Jx3ProcessState, SessionProcessDecision,
        WindowFrame, MAX_CAPTURE_PIXELS,
    };

    #[test]
    fn cards_area_contains_the_vision_bounds_with_padding() {
        let area = capture_area(CaptureMode::Cards, 2560, 1369).unwrap();
        assert_eq!(
            (area.left, area.top, area.right, area.bottom),
            (433, 244, 2126, 1028)
        );
        assert!(area.left <= 2560 * 17 / 100 && area.right >= 2560 * 83 / 100);
        assert!(area.top <= 1369 * 18 / 100 && area.bottom >= 1369 * 75 / 100);
    }

    #[test]
    fn full_area_covers_the_complete_client() {
        let area = capture_area(CaptureMode::Full, 1920, 1080).unwrap();
        assert_eq!(
            (area.left, area.top, area.right, area.bottom),
            (0, 0, 1920, 1080)
        );
    }

    #[test]
    fn capture_budget_allows_common_wide_and_4k_clients_but_rejects_8k() {
        assert_eq!(MAX_CAPTURE_PIXELS, 3_840 * 2_160);
        assert!(capture_dimensions_allowed(3_840, 2_160));
        assert!(capture_dimensions_allowed(5_120, 1_440));
        assert!(!capture_dimensions_allowed(7_680, 4_320));
        assert!(!capture_dimensions_allowed(0, 1_080));
        assert!(!capture_dimensions_allowed(usize::MAX, 2));
    }

    #[test]
    fn resolution_change_drops_full_and_roi_buffer_capacities() {
        let mut frame = Some(WindowFrame {
            pid: 1,
            width: 3_840,
            height: 2_160,
            cursor: None,
            // 小样本即可验证丢弃 Vec 本身会释放容量，避免测试制造 4K 分配。
            pixels: vec![0; 4_096],
        });
        let mut readback = vec![0; 2_048];
        assert!(frame
            .as_ref()
            .is_some_and(|frame| frame.pixels.capacity() > 0));
        assert!(readback.capacity() > 0);

        discard_cpu_buffers(&mut frame, &mut readback);

        assert!(frame.is_none());
        assert_eq!(readback.len(), 0);
        assert_eq!(readback.capacity(), 0);
    }

    #[test]
    fn reused_frame_clears_only_when_cards_could_see_old_full_pixels() {
        let mut pixels = Vec::new();
        assert!(prepare_frame_pixels(&mut pixels, None, None, CaptureMode::Cards, 10, 8,).unwrap());
        assert!(pixels.iter().all(|pixel| *pixel == 0));

        // 模拟 Cards 只覆写中心 ROI；同尺寸连续 Cards 不再清整张缓冲。
        pixels.fill(0);
        pixels[4 * (3 * 10 + 4)] = 17;
        assert!(!prepare_frame_pixels(
            &mut pixels,
            Some((10, 8)),
            Some(CaptureMode::Cards),
            CaptureMode::Cards,
            10,
            8,
        )
        .unwrap());
        assert_eq!(pixels[4 * (3 * 10 + 4)], 17);

        // Full 覆写完整客户区；随后切回 Cards 必须擦掉区域外旧内容。
        pixels.fill(91);
        assert!(!prepare_frame_pixels(
            &mut pixels,
            Some((10, 8)),
            Some(CaptureMode::Cards),
            CaptureMode::Full,
            10,
            8,
        )
        .unwrap());
        assert!(prepare_frame_pixels(
            &mut pixels,
            Some((10, 8)),
            Some(CaptureMode::Full),
            CaptureMode::Cards,
            10,
            8,
        )
        .unwrap());
        assert!(pixels.iter().all(|pixel| *pixel == 0));

        // 尺寸变化也会清空，即使上一次模式同为 Cards。
        pixels.fill(45);
        assert!(prepare_frame_pixels(
            &mut pixels,
            Some((10, 8)),
            Some(CaptureMode::Cards),
            CaptureMode::Cards,
            8,
            10,
        )
        .unwrap());
        assert_eq!(pixels.len(), 8 * 10 * 4);
        assert!(pixels.iter().all(|pixel| *pixel == 0));
    }

    #[test]
    fn process_presence_ends_only_when_the_bound_game_is_gone_or_replaced() {
        assert_eq!(
            session_process_decision(None, &Jx3ProcessState::Offline),
            SessionProcessDecision::End
        );
        assert_eq!(
            session_process_decision(None, &Jx3ProcessState::Single(42)),
            SessionProcessDecision::Bind(42)
        );
        assert_eq!(
            session_process_decision(Some(42), &Jx3ProcessState::Single(42)),
            SessionProcessDecision::Continue
        );
        assert_eq!(
            session_process_decision(Some(42), &Jx3ProcessState::Single(77)),
            SessionProcessDecision::End
        );
        // 原客户端仍在时，多开只跳过本帧，不能误停本轮结算识别。
        assert_eq!(
            session_process_decision(Some(42), &Jx3ProcessState::Multiple(vec![42, 77])),
            SessionProcessDecision::Skip
        );
        // 原客户端已消失，即使另一个 JX3 还在也不能拿它的画面续用。
        assert_eq!(
            session_process_decision(Some(42), &Jx3ProcessState::Multiple(vec![77, 88])),
            SessionProcessDecision::End
        );
        assert_eq!(
            session_process_decision(Some(42), &Jx3ProcessState::Unavailable),
            SessionProcessDecision::Skip
        );
    }
}

#[cfg(all(test, windows))]
mod screenshot_replay {
    use super::*;

    #[test]
    #[ignore = "requires TRIAL_EQUIPMENT_REPLAY_DIR with reward.bgra and backs.bgra"]
    fn real_cards_roi_preserves_selected_and_unflipped_boards() {
        let dir = std::path::PathBuf::from(std::env::var_os("TRIAL_EQUIPMENT_REPLAY_DIR").unwrap());
        let width = 2560;
        for (name, expected_slot) in [("reward.bgra", Some(2)), ("backs.bgra", None)] {
            let original = std::fs::read(dir.join(name)).unwrap();
            let height = original.len() / (width * 4);
            let area = capture_area(CaptureMode::Cards, width, height).unwrap();
            let mut pixels = vec![0; original.len()];
            for y in area.top..area.bottom {
                let start = (y * width + area.left) * 4;
                let end = (y * width + area.right) * 4;
                pixels[start..end].copy_from_slice(&original[start..end]);
            }
            assert_eq!(
                super::super::vision::flipped_slot(&pixels, width, height),
                expected_slot,
                "{name}"
            );
            assert!(
                super::super::vision::cards_visible(&pixels, width, height),
                "{name}"
            );
            let scaled_area = area.width() * area.height() * 100;
            assert!(scaled_area > width * height * 35);
            assert!(scaled_area < width * height * 40);
        }
    }
}
