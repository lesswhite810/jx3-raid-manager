import React from 'react';
import { useTheme } from '../contexts/ThemeContext';
import { ThemeType, THEME_LABELS } from '../services/themeStorage';
import { Check, Sun, Moon, Scroll } from 'lucide-react';

/**
 * 主题选择器
 *
 * 提供三套主题选择（original / jianghu / dark），单选卡片形式。
 * 选中状态实时持久化到本地或 Tauri AppConfig（通过 useTheme）。
 *
 * @see contexts/ThemeContext.tsx
 * @see services/themeStorage.ts
 */
export const ThemeSelector: React.FC = () => {
    const { theme, setTheme } = useTheme();

    const options: Array<{
        value: ThemeType;
        description: string;
        icon: React.ReactNode;
        preview: React.ReactNode;
    }> = [
        {
            value: 'original',
            description: '靛蓝主色 + 锌灰界面，v2.1.53 之前的稳定视觉',
            icon: <Sun className="w-5 h-5" />,
            preview: (
                <div className="flex items-center gap-1">
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#4f46e5' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#fafafa', border: '1px solid #e4e4e7' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#059669' }} />
                </div>
            ),
        },
        {
            value: 'jianghu',
            description: '沉香褐主色 + 米杏背景，长时挂机不刺眼（v2.2.0 实验）',
            icon: <Scroll className="w-5 h-5" />,
            preview: (
                <div className="flex items-center gap-1">
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#a8633f' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#faf7f2', border: '1px solid #e3dccc' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#5d8770' }} />
                </div>
            ),
        },
        {
            value: 'dark',
            description: '暗色锌灰界面，夜间使用更舒适',
            icon: <Moon className="w-5 h-5" />,
            preview: (
                <div className="flex items-center gap-1">
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#818cf8' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#18181b', border: '1px solid #3f3f46' }} />
                    <div className="w-3 h-3 rounded-tag" style={{ backgroundColor: '#34d399' }} />
                </div>
            ),
        },
    ];

    return (
        <div className="space-y-2">
            {options.map((opt) => {
                const isActive = theme === opt.value;
                return (
                    <button
                        key={opt.value}
                        type="button"
                        onClick={() => setTheme(opt.value)}
                        className={`w-full flex items-center gap-3 p-3 rounded-card border text-left transition-all duration-ds-fast ease-ds-ease ${
                            isActive
                                ? 'border-ds-accent bg-ds-accent-soft'
                                : 'border-ds-border hover:border-ds-border-strong bg-ds-surface'
                        }`}
                        aria-pressed={isActive}
                    >
                        <div
                            className={`w-9 h-9 rounded-card flex items-center justify-center flex-shrink-0 ${
                                isActive
                                    ? 'bg-ds-accent text-ds-fg-inverse'
                                    : 'bg-ds-surface-2 text-ds-fg-soft'
                            }`}
                        >
                            {opt.icon}
                        </div>
                        <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                                <span className="text-sm font-medium text-ds-fg">
                                    {THEME_LABELS[opt.value]}
                                </span>
                                {opt.preview}
                                {isActive && (
                                    <span className="ml-auto inline-flex items-center gap-1 text-xs text-ds-accent-strong">
                                        <Check className="w-3 h-3" />
                                        当前使用
                                    </span>
                                )}
                            </div>
                            <div className="text-xs text-ds-fg-mute mt-0.5 truncate">
                                {opt.description}
                            </div>
                        </div>
                    </button>
                );
            })}
        </div>
    );
};
