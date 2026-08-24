import React, { createContext, useContext, useEffect, useState } from 'react';
import { loadThemeConfig, saveThemeConfig, ThemeType, DEFAULT_THEME } from '../services/themeStorage';

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
    const [theme, setThemeState] = useState<ThemeType>(DEFAULT_THEME);
    const [isInitialized, setIsInitialized] = useState(false);

    useEffect(() => {
        const initTheme = async () => {
            const config = await loadThemeConfig();
            setThemeState(config.theme);
            applyTheme(config.theme);
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
