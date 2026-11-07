import React, { createContext, useContext, useEffect, useState } from 'react';
import {
    loadThemeConfig,
    saveThemeConfig,
    readCachedTheme,
    writeCachedTheme,
    ThemeType,
    DEFAULT_THEME
} from '../services/themeStorage';

interface ThemeContextType {
    theme: ThemeType;
    setTheme: (theme: ThemeType) => void;
    /**
     * 兼容旧版 toggle 调用：dark <-> original
     * 注：jianghu 是新增的第三种主题，toggle 不会进入 jianghu。
     * 后续重构（Header 改造完成后）会移除该方法，调用方迁移到 setTheme + 显式选择。
     */
    toggleTheme: () => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    // 首帧先用同步缓存里的主题启动，避免深色用户看到一屏亮色；
    // 真实来源仍是后端 SQLite，异步读出来后再校正。
    const [theme, setThemeState] = useState<ThemeType>(() => readCachedTheme());
    const [isInitialized, setIsInitialized] = useState(false);

    useEffect(() => {
        const initTheme = async () => {
            // 先把缓存主题落到 <html> 上（内联脚本已试过一次，这里兜底浏览器/测试环境）
            applyTheme(readCachedTheme());
            const config = await loadThemeConfig();
            setThemeState(config.theme);
            applyTheme(config.theme);
            writeCachedTheme(config.theme);
            setIsInitialized(true);
        };
        initTheme();
    }, []);

    const applyTheme = (newTheme: ThemeType) => {
        const root = window.document.documentElement;
        root.setAttribute('data-theme', newTheme);
        if (newTheme === 'dark') {
            root.classList.add('dark');
        } else {
            root.classList.remove('dark');
        }
    };

    const setTheme = (newTheme: ThemeType) => {
        setThemeState(newTheme);
        applyTheme(newTheme);
        writeCachedTheme(newTheme);
        saveThemeConfig({ theme: newTheme });
    };

    const toggleTheme = () => {
        const next: ThemeType = theme === 'dark' ? DEFAULT_THEME : 'dark';
        setTheme(next);
    };

    if (!isInitialized) {
        return null;
    }

    return (
        <ThemeContext.Provider value={{ theme, setTheme, toggleTheme }}>
            {children}
        </ThemeContext.Provider>
    );
};

export const useTheme = () => {
    const context = useContext(ThemeContext);
    if (context === undefined) {
        throw new Error('useTheme must be used within a ThemeProvider');
    }
    return context;
};
