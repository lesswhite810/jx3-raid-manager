/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./*.{js,ts,jsx,tsx}",
    "./src/**/*.{js,ts,jsx,tsx}",
    "./components/**/*.{js,ts,jsx,tsx}",
    "./hooks/**/*.{js,ts,jsx,tsx}",
    "./utils/**/*.{js,ts,jsx,tsx}",
    "./services/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: ["class", '[data-theme="dark"]'],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: {
        "2xl": "1400px",
      },
    },
    extend: {
      colors: {
        // 既有 token（保留，向后兼容 · 桌面工具型 chrome）
        base: "rgb(var(--bg-base) / <alpha-value>)",
        surface: "rgb(var(--bg-surface) / <alpha-value>)",
        main: "rgb(var(--text-main) / <alpha-value>)",
        muted: "rgb(var(--text-muted) / <alpha-value>)",
        border: "rgb(var(--border-base) / <alpha-value>)",
        primary: {
          DEFAULT: "rgb(var(--primary-base) / <alpha-value>)",
          hover: "rgb(var(--primary-hover) / <alpha-value>)",
          foreground: "rgb(var(--text-on-primary) / <alpha-value>)",
        },
        // ============================================================
        // 设计系统提案 token（2026-08-23 纳入 · 与既有 chrome 共存）
        // 来源：tmp/design-zip/colors_and_type.css §3 + DESIGN.md §3
        // 用 ds- 前缀避免与 Tailwind 默认调色板（neutral / amber / sky 等）冲突
        // ============================================================
        'ds-bg': 'var(--bg)',
        'ds-surface': 'var(--surface-ds)',
        'ds-surface-2': 'var(--surface-2)',
        'ds-surface-3': 'var(--surface-3)',
        'ds-fg': 'var(--fg)',
        'ds-fg-soft': 'var(--fg-soft)',
        'ds-fg-mute': 'var(--fg-mute)',
        'ds-fg-inverse': 'var(--fg-inverse)',
        'ds-accent': {
          DEFAULT: 'var(--accent)',
          soft: 'var(--accent-soft)',
          strong: 'var(--accent-strong)',
        },
        'ds-success': {
          DEFAULT: 'var(--success)',
          soft: 'var(--success-soft)',
          strong: 'var(--success-strong)',
        },
        'ds-warning': {
          DEFAULT: 'var(--warning)',
          soft: 'var(--warning-soft)',
          strong: 'var(--warning-strong)',
        },
        'ds-danger': {
          DEFAULT: 'var(--danger-ds)',
          soft: 'var(--danger-soft)',
        },
        'ds-info': {
          DEFAULT: 'var(--info)',
          soft: 'var(--info-soft)',
          strong: 'var(--info-strong)',
        },
        'ds-neutral': {
          DEFAULT: 'var(--neutral)',
          soft: 'var(--neutral-soft)',
        },
      },
      textColor: {
        main: "rgb(var(--text-main) / <alpha-value>)",
        muted: "rgb(var(--text-muted) / <alpha-value>)",
        // 设计系统提案文本色 alias
        'ds-fg': 'var(--fg)',
        'ds-fg-soft': 'var(--fg-soft)',
        'ds-fg-mute': 'var(--fg-mute)',
      },
      backgroundColor: {
        base: "rgb(var(--bg-base) / <alpha-value>)",
        surface: "rgb(var(--bg-surface) / <alpha-value>)",
        // 设计系统提案背景色 alias
        'ds-bg': 'var(--bg)',
        'ds-surface': 'var(--surface-ds)',
        'ds-surface-2': 'var(--surface-2)',
        'ds-surface-3': 'var(--surface-3)',
        'ds-accent': 'var(--accent)',
        'ds-accent-soft': 'var(--accent-soft)',
      },
      borderColor: {
        base: "rgb(var(--border-base) / <alpha-value>)",
        // 设计系统提案描边 alias
        'ds-border': 'var(--border-ds)',
        'ds-border-strong': 'var(--border-strong)',
        'ds-accent': 'var(--accent)',
        'ds-accent-strong': 'var(--accent-strong)',
      },
      borderRadius: {
        lg: "var(--radius)",                         // 既有 · 保留 = 8px（back-compat）
        md: "calc(var(--radius) - 2px)",             // 既有 · 保留 = 6px
        sm: "calc(var(--radius) - 4px)",             // 既有 · 保留 = 4px
        // 设计系统提案圆角（严格 2 档 · 新代码优先使用）
        card: "var(--r-md)",                         // 4px · 卡片 / 输入框 / Toast
        tag: "var(--r-sm)",                          // 2px · Tag / Chip / 按钮
      },
      fontFamily: {
        sans: ["Inter", "ui-sans-serif", "system-ui", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "Roboto", "Helvetica Neue", "Arial", "sans-serif"],
        // 设计系统提案字体
        // AGENTS.md §9 反模式：禁止 Inter/Roboto/Arial/Fraunces 作展示字体
        // display 用于标题/文档场景；body 用于正文/表单；mono 用于数字
        display: 'var(--font-display)',              // Source Han Serif SC（思源宋体）
        body: 'var(--font-body)',                    // PingFang SC（苹方）
        han: 'var(--font-display)',                  // 别名
        pingfang: 'var(--font-body)',                // 别名
        mono: 'var(--font-mono)',                    // SF Mono（数字 / 等宽 · 强制 tabular-nums）
      },
      fontSize: {
        // 设计系统提案字号（major-third · 1.25 倍）· 用 ds- 前缀避免覆盖 Tailwind 默认
        'ds-xs': ['var(--fs-xs)', { lineHeight: 'var(--lh-base)' }],
        'ds-sm': ['var(--fs-sm)', { lineHeight: 'var(--lh-base)' }],
        'ds-md': ['var(--fs-md)', { lineHeight: 'var(--lh-base)' }],
        'ds-lg': ['var(--fs-lg)', { lineHeight: 'var(--lh-base)' }],
        'ds-xl': ['var(--fs-xl)', { lineHeight: '1.4' }],
        'ds-2xl': ['var(--fs-2xl)', { lineHeight: '1.3' }],
        'ds-3xl': ['var(--fs-3xl)', { lineHeight: 'var(--lh-tight)' }],
        'ds-4xl': ['var(--fs-4xl)', { lineHeight: '1.2' }],
      },
      spacing: {
        // 设计系统提案间距（4 倍数 · 8 档）
        'ds-1': 'var(--s-1)',
        'ds-2': 'var(--s-2)',
        'ds-3': 'var(--s-3)',
        'ds-4': 'var(--s-4)',
        'ds-5': 'var(--s-5)',
        'ds-6': 'var(--s-6)',
        'ds-7': 'var(--s-7)',
        'ds-8': 'var(--s-8)',
      },
      boxShadow: {
        // 设计系统提案阴影（低强度 · 3 档）· Tailwind 默认 shadow-* 保留向后兼容
        'ds-flat': 'var(--shadow-flat)',             // 1px / 4% · 卡片悬浮
        'ds-stack': 'var(--shadow-stack)',           // 2px / 6% · 弹窗
        'ds-modal': 'var(--shadow-modal)',           // 6px / 8% · Toast / 抽屉
      },
      transitionDuration: {
        'ds-fast': 'var(--dur-fast)',                // 120ms · hover / focus
        'ds-base': 'var(--dur-base)',                // 180ms · 颜色 / 边距过渡
        'ds-slow': 'var(--dur-slow)',                // 280ms · 抽屉 / 弹窗
      },
      transitionTimingFunction: {
        'ds-ease': 'var(--ease)',                    // cubic-bezier(0.2, 0.6, 0.2, 1)
      },
    },
  },
  plugins: [],
};
