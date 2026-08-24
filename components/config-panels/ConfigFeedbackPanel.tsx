import React from 'react';
import { MessageCircle } from 'lucide-react';

/**
 * 系统配置 · 问题反馈
 *
 * 显示 QQ 群二维码与群号，方便用户反馈问题。
 */
export const ConfigFeedbackPanel: React.FC = () => {
    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-4">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <MessageCircle className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-main">问题反馈</h3>
            </div>
            <div className="flex items-center gap-5 p-4 bg-base/30 rounded-lg">
                <img
                    src="/qq-group-qr.jpg"
                    alt="QQ群二维码"
                    className="w-28 h-28 rounded-lg border border-base object-contain bg-white flex-shrink-0"
                />
                <div className="flex flex-col justify-center min-w-0 flex-1">
                    <p className="text-sm text-main mb-1.5">加入 QQ 群反馈问题、提建议或交流使用心得</p>
                    <div className="flex items-baseline gap-2">
                        <span className="text-xs text-muted">群号</span>
                        <span className="text-lg font-bold text-primary select-all">1085903108</span>
                    </div>
                    <p className="text-xs text-muted mt-1.5">扫描左侧二维码或搜索群号加入</p>
                </div>
            </div>
        </div>
    );
};