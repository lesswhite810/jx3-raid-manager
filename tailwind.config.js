/** @type {import('tailwindcss').Config} */

// ds-* 语义色取自 CSS 变量（index.css 中为十六进制纯色值，不是通道三元组），
// 原写法是裸 'var(--x)'，不含 <alpha-value> 占位符。Tailwind 遇到这种值时会
// 直接丢掉透明度修饰符 —— 即 `bg-ds-success-soft/30` 整条规则不生成，
// 既不报错也无样式，属于静默失效。
//
// 这里补上 alpha 支持：仅当显式传入数字浓度（bg-ds-success-soft/30）时用
// color-mix 按比例混入 transparent；其余情况（含 Tailwind 传入的运行时
// --tw-*-opacity 变量）一律原样输出 var(--x)，与改动前完全一致，避免影响存量用法。
const dsColor = (cssVar) => ({ opacityValue }) =>
  opacityValue !== undefined && /^[\d.]+$/.test(String(opacityValue))
    ? `color-mix(in srgb, var(${cssVar}) calc(${opacityValue} * 100%), transparent)`
    : `var(${cssVar})`;

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
        'ds-bg': dsColor('--bg'),
        'ds-surface': dsColor('--surface-ds'),
        'ds-surface-2': dsColor('--surface-2'),
        'ds-surface-3': dsColor('--surface-3'),
        'ds-fg': dsColor('--fg'),
        'ds-fg-soft': dsColor('--fg-soft'),
        'ds-fg-mute': dsColor('--fg-mute'),
        'ds-fg-inverse': dsColor('--fg-inverse'),
        'ds-accent': {
          DEFAULT: dsColor('--accent'),
          soft: dsColor('--accent-soft'),
          strong: dsColor('--accent-strong'),
        },
        'ds-success': {
          DEFAULT: dsColor('--success'),
          soft: dsColor('--success-soft'),
          strong: dsColor('--success-strong'),
        },
        'ds-warning': {
          DEFAULT: dsColor('--warning'),
          soft: dsColor('--warning-soft'),
          strong: dsColor('--warning-strong'),
        },
        'ds-danger': {
          DEFAULT: dsColor('--danger-ds'),
          soft: dsColor('--danger-soft'),
        },
        'ds-info': {
          DEFAULT: dsColor('--info'),
          soft: dsColor('--info-soft'),
          strong: dsColor('--info-strong'),
        },
        'ds-neutral': {
          DEFAULT: dsColor('--neutral'),
          soft: dsColor('--neutral-soft'),
        },
      },
      textColor: {
        main: "rgb(var(--text-main) / <alpha-value>)",
        muted: "rgb(var(--text-muted) / <alpha-value>)",
        // 设计系统提案文本色 alias
        'ds-fg': dsColor('--fg'),
        'ds-fg-soft': dsColor('--fg-soft'),
        'ds-fg-mute': dsColor('--fg-mute'),
      },
      backgroundColor: {
        base: "rgb(var(--bg-base) / <alpha-value>)",
        surface: "rgb(var(--bg-surface) / <alpha-value>)",
        // 设计系统提案背景色 alias
        'ds-bg': dsColor('--bg'),
        'ds-surface': dsColor('--surface-ds'),
        'ds-surface-2': dsColor('--surface-2'),
        'ds-surface-3': dsColor('--surface-3'),
        'ds-accent': dsColor('--accent'),
        'ds-accent-soft': dsColor('--accent-soft'),
      },
      borderColor: {
        base: "rgb(var(--border-base) / <alpha-value>)",
        // 设计系统提案描边 alias
        'ds-border': dsColor('--border-ds'),
        'ds-border-strong': dsColor('--border-strong'),
        'ds-accent': dsColor('--accent'),
        'ds-accent-strong': dsColor('--accent-strong'),
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
