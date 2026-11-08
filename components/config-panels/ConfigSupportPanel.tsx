import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { HandHeart, X } from 'lucide-react';

/**
 * 系统配置 · 赞赏
 *
 * 只展示一张本地打包的微信赞赏码原图，不设金额档位，也不提供任何付费能力：
 * - 完全自愿，与功能解锁无关，扫码不产生任何权益
 * - 不自动弹出，仅用户主动打开配置的该分类时才出现
 * - 图片走 public/ 静态资源，不联网拉取，也不会被远端替换
 * - 原图是整张赞赏卡（含底部文案与横幅），小程序码圆环只占画面约 43%，
 *   所以默认尺寸之外额外提供点击放大，放大后圆环直径接近 240px，保证可扫
 *
 * 引导语保持在 20 字以内，避免变成软性募捐话术。
 */
export const ConfigSupportPanel: React.FC = () => {
    const [isZoomed, setIsZoomed] = useState(false);
    const closeButtonRef = useRef<HTMLButtonElement>(null);
    const triggerRef = useRef<HTMLButtonElement>(null);

    const closeZoom = useCallback(() => {
        setIsZoomed(false);
        triggerRef.current?.focus();
    }, []);

    useEffect(() => {
        if (!isZoomed) return;

        const animationFrame = window.requestAnimationFrame(() => closeButtonRef.current?.focus());
        /**
         * 放大层盖在系统配置弹窗之上，而弹窗自身监听了 window 的 keydown 关闭。
         * 这里用捕获阶段拦下 Escape 并阻止冒泡，避免按一次 ESC 把配置弹窗一起关掉。
         */
        const handleKeyDown = (event: KeyboardEvent) => {
            if (event.key !== 'Escape') return;
            event.preventDefault();
            event.stopPropagation();
            closeZoom();
        };
        window.addEventListener('keydown', handleKeyDown, true);
        return () => {
            window.cancelAnimationFrame(animationFrame);
            window.removeEventListener('keydown', handleKeyDown, true);
        };
    }, [closeZoom, isZoomed]);

    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-4">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <HandHeart className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-main">赞赏</h3>
            </div>

            <div className="flex flex-col items-center bg-base/30 rounded-lg px-4 py-6">
                <button
                    ref={triggerRef}
                    type="button"
                    onClick={() => setIsZoomed(true)}
                    aria-haspopup="dialog"
                    aria-label="放大查看微信赞赏码"
                    className="w-[320px] max-w-full rounded-lg overflow-hidden border border-base bg-white transition-colors hover:border-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                >
                    <img
                        src="/wechat-reward-qr.png"
                        alt="微信赞赏码"
                        className="w-full h-auto block"
                    />
                </button>
                <p className="text-sm text-main mt-4">完全免费，赞赏纯属自愿</p>
                <p className="text-xs text-muted mt-1.5">全部功能始终免费，扫码不会解锁任何额外功能</p>
                <p className="text-xs text-muted mt-1">点击图片可放大，方便手机扫码</p>
            </div>

            {isZoomed && createPortal(
                <div
                    className="fixed inset-0 z-[120] bg-ds-scrim backdrop-blur-sm"
                    onClick={closeZoom}
                >
                    <div className="fixed inset-0 z-[121] flex items-center justify-center p-4 pointer-events-none">
                        <div
                            role="dialog"
                            aria-modal="true"
                            aria-label="微信赞赏码"
                            className="bg-surface rounded-xl border border-base shadow-ds-modal p-4 pointer-events-auto flex flex-col items-center max-h-full"
                            onClick={(e) => e.stopPropagation()}
                        >
                            <div className="flex items-center justify-between w-full mb-2">
                                <span className="text-sm font-medium text-main">微信赞赏码</span>
                                <button
                                    ref={closeButtonRef}
                                    type="button"
                                    onClick={closeZoom}
                                    className="text-muted hover:text-main hover:bg-base p-1 rounded-lg transition-colors"
                                    aria-label="关闭"
                                >
                                    <X className="w-4 h-4" />
                                </button>
                            </div>
                            <img
                                src="/wechat-reward-qr.png"
                                alt="微信赞赏码"
                                className="w-[min(560px,calc(100vw-4rem))] h-auto max-h-[calc(90vh-4rem)] object-contain rounded-lg border border-base bg-white"
                            />
                            <p className="text-xs text-muted mt-2">扫码纯属自愿，不会解锁任何额外功能</p>
                        </div>
                    </div>
                </div>,
                document.body,
            )}
        </div>
    );
};
