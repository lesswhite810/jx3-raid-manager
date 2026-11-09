import React, { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { MessageCircle, X } from 'lucide-react';

/**
 * 系统配置 · 问题反馈
 *
 * 主体是一张本地打包的 QQ 群二维码，放大到与「赞赏」面板同级的大小，并支持点击弹出放大层：
 * - 图片走 public/ 静态资源，不联网拉取
 * - 素材已裁成正方形，只保留二维码本体与白色静默区，去掉了外面的分享卡留白
 * - 群号大字展示并允许选中复制，扫码不方便时可以直接搜群号
 *
 * 布局说明：配置弹窗是固定 h-[640px] + max-h-[90vh]，内容区高度有限，
 * 所以外层用 h-full + flex-1 min-h-0 铺满，图片尺寸取
 * min(300px, calc(90vh - 310px))（310px 为标题、内边距、群号与提示两行文案占位），
 * 窗口较小时图片跟着缩小，任何高度下都不会顶出滚动条。
 */
export const ConfigFeedbackPanel: React.FC = () => {
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
        <div className="bg-surface p-5 rounded-xl shadow-sm border border-base h-full flex flex-col min-h-0">
            <div className="flex items-center gap-3 mb-3 flex-shrink-0">
                <div className="w-9 h-9 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <MessageCircle className="w-4 h-4" />
                </div>
                <h3 className="text-base font-bold text-main">问题反馈</h3>
            </div>

            <div className="flex-1 min-h-0 flex flex-col items-center justify-center bg-base/30 rounded-lg px-4 py-4">
                <button
                    ref={triggerRef}
                    type="button"
                    onClick={() => setIsZoomed(true)}
                    aria-haspopup="dialog"
                    aria-label="放大查看 QQ 群二维码"
                    className="group relative inline-flex max-w-full cursor-zoom-in rounded-lg overflow-hidden border border-base bg-white transition-colors hover:border-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
                >
                    <img
                        src="/qq-group-qr.png"
                        alt="QQ群二维码"
                        className="block h-[min(300px,calc(90vh_-_310px))] w-[min(300px,calc(90vh_-_310px))] max-w-full object-contain"
                    />
                    <span className="absolute bottom-2 right-2 rounded-md bg-black/55 px-2 py-1 text-xs text-white opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100">
                        点击放大
                    </span>
                </button>
                <div className="flex items-baseline gap-2 mt-3 flex-shrink-0">
                    <span className="text-xs text-muted">群号</span>
                    <span className="text-lg font-bold text-primary select-all">1085903108</span>
                </div>
                <p className="text-xs text-muted mt-1 flex-shrink-0">点击图片可放大，扫码或搜索群号加入</p>
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
                            aria-label="QQ群二维码"
                            className="bg-surface rounded-xl border border-base shadow-ds-modal p-4 pointer-events-auto flex flex-col items-center max-h-full"
                            onClick={(e) => e.stopPropagation()}
                        >
                            <div className="flex items-center justify-between w-full mb-2">
                                <span className="text-sm font-medium text-main">QQ群二维码</span>
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
                                src="/qq-group-qr.png"
                                alt="QQ群二维码"
                                className="w-[min(560px,calc(100vw-4rem))] h-auto max-h-[calc(90vh-4rem)] object-contain rounded-lg border border-base bg-white"
                            />
                            <p className="text-xs text-muted mt-2">扫码加入 QQ 群反馈问题、提建议或交流使用心得</p>
                        </div>
                    </div>
                </div>,
                document.body,
            )}
        </div>
    );
};
