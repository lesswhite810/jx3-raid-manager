import { invoke } from '@tauri-apps/api/core';

/**
 * 主题类型
 * - original: 原方案（v2.1.53 之前的靛蓝 + 锌灰风格）— 默认
 * - jianghu:  江湖纸笺（v2.2.0 沉香褐基调，可选）
 * - dark:     暗色模式
 */
export type ThemeType = 'original' | 'jianghu' | 'dark';

export const DEFAULT_THEME: ThemeType = 'original';

export const THEME_LABELS: Record<ThemeType, string> = {
    original: '经典',
    jianghu: '江湖纸笺',
    dark: '暗色',
};

export interface ThemeConfig {
    theme: ThemeType;
}

const BROWSER_THEME_KEY = 'jx3_theme_config';
const DEFAULT_CONFIG: ThemeConfig = { theme: DEFAULT_THEME };

const isTauriEnv = (): boolean => typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

/**
 * 兼容旧版主题值（minimal → original；任何非法值 → DEFAULT_THEME）
 */
const normalizeTheme = (raw: unknown): ThemeType => {
    if (raw === 'jianghu' || raw === 'dark' || raw === 'original') {
        return raw;
    }
    return DEFAULT_THEME;
};

/**
 * 加载主题配置
 *
 * Tauri 环境：通过 invoke('get_theme') 从 SQLite app_config 表读取（与其他应用配置统一存储）。
 * 浏览器环境（开发/测试）：从 localStorage 读取，便于无后端的单元测试。
 */
export const loadThemeConfig = async (): Promise<ThemeConfig> => {
    try {
        if (!isTauriEnv()) {
            const stored = localStorage.getItem(BROWSER_THEME_KEY);
            if (!stored) return DEFAULT_CONFIG;
            const parsed = JSON.parse(stored);
            return { theme: normalizeTheme(parsed.theme) };
        }

        const stored = await invoke<string | null>('get_theme');
        if (!stored) return DEFAULT_CONFIG;
        return { theme: normalizeTheme(stored) };
    } catch (error) {
        console.error('Failed to load theme config:', error);
        return DEFAULT_CONFIG;
    }
};

/**
 * 保存主题配置
 *
 * Tauri 环境：通过 invoke('set_theme') 写入 SQLite app_config 表。
 * 浏览器环境：写入 localStorage 兜底（仅用于开发环境）。
 */
export const saveThemeConfig = async (config: ThemeConfig): Promise<void> => {
    try {
        if (!isTauriEnv()) {
            localStorage.setItem(BROWSER_THEME_KEY, JSON.stringify(config));
            return;
        }

        await invoke<void>('set_theme', { theme: config.theme });
    } catch (error) {
        console.error('Failed to save theme config:', error);
        // 不抛出错误，避免阻塞 UI 切换；下次启动会自动回退到默认
    }
};
