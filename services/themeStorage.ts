import { BaseDirectory, readTextFile, writeTextFile, exists, mkdir } from '@tauri-apps/plugin-fs';

export type ThemeType = 'minimal' | 'dark';

export interface ThemeConfig {
    theme: ThemeType;
}

const THEME_FILE = 'theme.json';
const DEFAULT_THEME: ThemeConfig = { theme: 'minimal' };
const BROWSER_THEME_KEY = 'jx3_theme_config';
const isTauriEnv = (): boolean => typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;

export const loadThemeConfig = async (): Promise<ThemeConfig> => {
    try {
        if (!isTauriEnv()) {
            const storedConfig = localStorage.getItem(BROWSER_THEME_KEY);
            return storedConfig ? JSON.parse(storedConfig) : DEFAULT_THEME;
        }

        const fileExists = await exists(THEME_FILE, { baseDir: BaseDirectory.AppConfig });
        if (!fileExists) {
            return DEFAULT_THEME;
        }

        const content = await readTextFile(THEME_FILE, { baseDir: BaseDirectory.AppConfig });
        return JSON.parse(content);
    } catch (error) {
        console.error('Failed to load theme config:', error);
        return DEFAULT_THEME;
    }
};

export const saveThemeConfig = async (config: ThemeConfig): Promise<void> => {
    try {
        if (!isTauriEnv()) {
            localStorage.setItem(BROWSER_THEME_KEY, JSON.stringify(config));
            return;
        }

        // Ensure the directory exists
        const dirExists = await exists('', { baseDir: BaseDirectory.AppConfig });
        if (!dirExists) {
            await mkdir('', { baseDir: BaseDirectory.AppConfig, recursive: true });
        }

        await writeTextFile(THEME_FILE, JSON.stringify(config, null, 2), {
            baseDir: BaseDirectory.AppConfig,
        });
    } catch (error) {
        console.error('Failed to save theme config:', error);
    }
};
