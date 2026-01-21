import React, { createContext, useContext, useEffect, useState } from 'react';
import { loadThemeConfig, saveThemeConfig, ThemeType } from '../services/themeStorage';

interface ThemeContextType {
    theme: ThemeType;
    toggleTheme: () => void;
    setTheme: (theme: ThemeType) => void;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
    const [theme, setThemeState] = useState<ThemeType>('minimal');
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
        if (newTheme === 'dark') {
            root.classList.add('dark');
            root.setAttribute('data-theme', 'dark');
        } else {
            root.classList.remove('dark');
            root.setAttribute('data-theme', 'minimal');
        }
    };

    const setTheme = (newTheme: ThemeType) => {
        setThemeState(newTheme);
        applyTheme(newTheme);
        saveThemeConfig({ theme: newTheme });
    };

    const toggleTheme = () => {
        setTheme(theme === 'minimal' ? 'dark' : 'minimal');
    };

    if (!isInitialized) {
        return null;
    }

    return (
        <ThemeContext.Provider value={{ theme, toggleTheme, setTheme }}>
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
