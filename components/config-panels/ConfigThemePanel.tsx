import React from 'react';
import { Palette } from 'lucide-react';
import { ThemeSelector } from '../ThemeSelector';

/**
 * 系统配置 · 主题与外观
 *
 * 仅包含主题切换器（ThemeSelector）。
 * 主题是用户在三个内置主题之间的选择：原方案 / 江湖纸笺 / 暗色。
 */
export const ConfigThemePanel: React.FC = () => {
    return (
        <div className="bg-surface p-6 rounded-xl shadow-sm border border-base">
            <div className="flex items-center gap-3 mb-4">
                <div className="w-10 h-10 bg-primary/10 text-primary rounded-lg flex items-center justify-center">
                    <Palette className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-main">主题与外观</h3>
            </div>
            <ThemeSelector />
        </div>
    );
};
