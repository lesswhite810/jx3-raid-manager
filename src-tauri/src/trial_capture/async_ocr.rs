//! 有界的试炼 Tooltip OCR 工作线程。
//!
//! 截图线程只复制当前 Tooltip 的小矩形并提交一次任务；系统 OCR 的等待发生在
//! 独立线程。队列和结果各只有一个槽位，慢 OCR 不会积压旧画面或阻塞下一次翻牌
//! 的牌位取样。

use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError};

use super::{catalog::TooltipEvidence, ocr, vision::Rect, TooltipKey};

/// 与 OCR 模块自身的单张 Tooltip 边界保持一致，拒绝异常 UI 矩形。
const MAX_TOOLTIP_BYTES: usize = 2 * 1024 * 1024;

struct Task {
    key: TooltipKey,
    pixels: Vec<u8>,
    width: usize,
    height: usize,
}

/// 单工作项 OCR。`busy` 包含正在执行和已投递但尚未读取结果两种状态，因此不会
/// 在慢系统 OCR 后堆积历史提示框。
pub(super) struct TooltipWorker {
    sender: Option<SyncSender<Task>>,
    results: Receiver<(TooltipKey, Option<TooltipEvidence>)>,
    busy: bool,
}

impl TooltipWorker {
    pub(super) fn new() -> Self {
        Self::with_recognizer(|pixels, width, height| {
            ocr::recognize_trial_tooltip(
                &pixels,
                width,
                height,
                Rect {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                },
            )
        })
    }

    fn with_recognizer(
        recognize: impl Fn(Vec<u8>, usize, usize) -> Option<TooltipEvidence> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Task>(1);
        let (result_sender, results) =
            mpsc::sync_channel::<(TooltipKey, Option<TooltipEvidence>)>(1);
        let spawned = std::thread::Builder::new()
            .name("trial-tooltip-ocr".into())
            .spawn(move || {
                crate::background_work::lower_current_thread_priority();
                while let Ok(task) = receiver.recv() {
                    let result = recognize(task.pixels, task.width, task.height);
                    // 接收者已销毁代表应用关闭；不再等待或处理后续任务。
                    if result_sender.send((task.key, result)).is_err() {
                        return;
                    }
                }
            });
        match spawned {
            Ok(_) => Self {
                sender: Some(sender),
                results,
                busy: false,
            },
            Err(error) => {
                log::warn!("无法启动试炼装备 OCR 后台识别，已跳过装备识别: {error}");
                Self {
                    sender: None,
                    results,
                    busy: false,
                }
            }
        }
    }

    /// 复制已定位 Tooltip 的像素并投递。忙、异常边界或工作线程不可用时返回
    /// `false`；调用方可在后续稳定帧重试，不会阻塞截图线程。
    pub(super) fn request(
        &mut self,
        key: TooltipKey,
        pixels: &[u8],
        window_width: usize,
        window_height: usize,
        rect: Rect,
    ) -> bool {
        if self.busy {
            return false;
        }
        let Some(sender) = self.sender.as_ref() else {
            return false;
        };
        let Some((cropped, width, height)) =
            crop_tooltip(pixels, window_width, window_height, rect)
        else {
            return false;
        };
        match sender.try_send(Task {
            key,
            pixels: cropped,
            width,
            height,
        }) {
            Ok(()) => {
                self.busy = true;
                true
            }
            Err(TrySendError::Full(_)) => {
                // 理论上 busy 已覆盖该情况；保守保持不投递第二个任务。
                self.busy = true;
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                self.sender = None;
                false
            }
        }
    }

    /// 非阻塞取走一项结果。结果无论成功或失败都会释放唯一的工作槽位。
    pub(super) fn poll(&mut self) -> Option<(TooltipKey, Option<TooltipEvidence>)> {
        match self.results.try_recv() {
            Ok(result) => {
                self.busy = false;
                Some(result)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.busy = false;
                self.sender = None;
                None
            }
        }
    }
}

impl Default for TooltipWorker {
    fn default() -> Self {
        Self::new()
    }
}

fn crop_tooltip(
    pixels: &[u8],
    window_width: usize,
    window_height: usize,
    rect: Rect,
) -> Option<(Vec<u8>, usize, usize)> {
    let width = rect.width();
    let height = rect.height();
    let bytes = width.checked_mul(height)?.checked_mul(4)?;
    if width == 0
        || height == 0
        || bytes > MAX_TOOLTIP_BYTES
        || rect.right > window_width
        || rect.bottom > window_height
        || pixels.len() < window_width.checked_mul(window_height)?.checked_mul(4)?
    {
        return None;
    }
    let mut cropped = vec![0; bytes];
    for row in 0..height {
        let source = ((rect.top + row) * window_width + rect.left).checked_mul(4)?;
        let destination = row.checked_mul(width)?.checked_mul(4)?;
        cropped[destination..destination + width * 4]
            .copy_from_slice(&pixels[source..source + width * 4]);
    }
    Some((cropped, width, height))
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use super::{crop_tooltip, TooltipWorker};
    use crate::trial_capture::{
        catalog::TooltipBind, catalog::TooltipEvidence, vision::Rect, TooltipKey,
    };

    fn key(generation: u64) -> TooltipKey {
        (
            generation,
            17,
            2,
            3,
            Rect {
                left: 1,
                top: 1,
                right: 5,
                bottom: 5,
            },
            99,
        )
    }

    fn evidence() -> TooltipEvidence {
        TooltipEvidence {
            name: "测试装备".into(),
            level: 1050,
            bind: TooltipBind::Bound,
        }
    }

    #[test]
    fn copies_only_the_tooltip_and_rejects_oversized_or_out_of_bounds_rectangles() {
        let pixels = (0u8..=255).cycle().take(8 * 8 * 4).collect::<Vec<_>>();
        let rect = Rect {
            left: 2,
            top: 3,
            right: 5,
            bottom: 5,
        };
        let (cropped, width, height) = crop_tooltip(&pixels, 8, 8, rect).unwrap();
        assert_eq!((width, height, cropped.len()), (3, 2, 24));
        assert_eq!(&cropped[..12], &pixels[(3 * 8 + 2) * 4..(3 * 8 + 5) * 4]);
        assert!(crop_tooltip(
            &pixels,
            8,
            8,
            Rect {
                left: 7,
                top: 7,
                right: 9,
                bottom: 9,
            },
        )
        .is_none());
        assert!(crop_tooltip(
            &pixels,
            2_000,
            2_000,
            Rect {
                left: 0,
                top: 0,
                right: 2_000,
                bottom: 2_000,
            },
        )
        .is_none());
    }

    #[test]
    fn one_slow_job_never_blocks_capture_or_queues_another() {
        let (started_sender, started) = mpsc::sync_channel(2);
        let (release_sender, release) = mpsc::sync_channel(1);
        let mut worker = TooltipWorker::with_recognizer(move |_, _, _| {
            started_sender.send(()).unwrap();
            release.recv().unwrap();
            Some(evidence())
        });
        let pixels = vec![7; 8 * 8 * 4];
        assert!(worker.request(
            key(1),
            &pixels,
            8,
            8,
            Rect {
                left: 1,
                top: 1,
                right: 5,
                bottom: 5,
            },
        ));
        // recognizer 正在等待 release；若 request 在截图线程中同步 OCR，测试不会
        // 执行到这里。用握手证明非阻塞，不依赖高负载机器上的墙钟阈值。
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!worker.request(
            key(2),
            &pixels,
            8,
            8,
            Rect {
                left: 1,
                top: 1,
                right: 5,
                bottom: 5,
            },
        ));
        assert!(worker.poll().is_none());
        release_sender.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        let result = loop {
            if let Some(result) = worker.poll() {
                break result;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        };
        assert_eq!(result.0, key(1));
        assert_eq!(result.1, Some(evidence()));
        assert!(worker.request(
            key(2),
            &pixels,
            8,
            8,
            Rect {
                left: 1,
                top: 1,
                right: 5,
                bottom: 5,
            },
        ));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        release_sender.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.poll().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    #[test]
    fn dropping_the_worker_does_not_wait_for_a_blocked_ocr() {
        let (started_sender, started) = mpsc::sync_channel(1);
        let (release_sender, release) = mpsc::sync_channel(1);
        let mut worker = TooltipWorker::with_recognizer(move |_, _, _| {
            started_sender.send(()).unwrap();
            // 测试会在 worker 已释放后解除阻塞；结果通道关闭后 send 直接失败退出。
            release.recv().ok();
            Some(evidence())
        });
        let pixels = vec![7; 8 * 8 * 4];
        assert!(worker.request(
            key(1),
            &pixels,
            8,
            8,
            Rect {
                left: 1,
                top: 1,
                right: 5,
                bottom: 5,
            },
        ));
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(worker);
        // TooltipWorker 的 Drop 不持有 JoinHandle；若未来改成等待 OCR，此处会在
        // release 之前卡住，测试自然超时。
        release_sender.send(()).unwrap();
    }
}
