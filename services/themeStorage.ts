import { BaseDirectory, readTextFile, writeTextFile, exists, createDir } from '@tauri-apps/api/fs';

export type ThemeType = 'minimal' | 'dark';

export interface ThemeConfig {
    theme: ThemeType;
}

const THEME_FILE = 'theme.json';
const DEFAULT_THEME: ThemeConfig = { theme: 'minimal' };

export const loadThemeConfig = async (): Promise<ThemeConfig> => {
    try {
        const fileExists = await exists(THEME_FILE, { dir: BaseDirectory.AppConfig });
        if (!fileExists) {
            return DEFAULT_THEME;
        }

        const content = await readTextFile(THEME_FILE, { dir: BaseDirectory.AppConfig });
        return JSON.parse(content);
    } catch (error) {
        console.error('Failed to load theme config:', error);
        return DEFAULT_THEME;
    }
};

export const saveThemeConfig = async (config: ThemeConfig): Promise<void> => {
    try {
        // Ensure the directory exists
        const dirExists = await exists('', { dir: BaseDirectory.AppConfig });
        if (!dirExists) {
            await createDir('', { dir: BaseDirectory.AppConfig, recursive: true });
        }

        await writeTextFile(THEME_FILE, JSON.stringify(config, null, 2), {
            dir: BaseDirectory.AppConfig,
        });
    } catch (error) {
        console.error('Failed to save theme config:', error);
    }
};
