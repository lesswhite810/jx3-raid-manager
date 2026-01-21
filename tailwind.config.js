/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./*.{js,ts,jsx,tsx}",
    "./components/**/*.{js,ts,jsx,tsx}",
    "./hooks/**/*.{js,ts,jsx,tsx}",
    "./utils/**/*.{js,ts,jsx,tsx}",
    "./services/**/*.{js,ts,jsx,tsx}",
    "./constants/**/*.{js,ts,jsx,tsx}",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: ['class', '[data-theme="dark"]'],
  theme: {
    extend: {
      colors: {
        base: 'rgb(var(--bg-base) / <alpha-value>)',
        surface: 'rgb(var(--bg-surface) / <alpha-value>)',
        main: 'rgb(var(--text-main) / <alpha-value>)',
        muted: 'rgb(var(--text-muted) / <alpha-value>)',
        border: 'rgb(var(--border-base) / <alpha-value>)',
        primary: {
          DEFAULT: 'rgb(var(--primary-base) / <alpha-value>)',
          hover: 'rgb(var(--primary-hover) / <alpha-value>)',
          text: 'rgb(var(--text-on-primary) / <alpha-value>)',
        },
      },
      textColor: {
        main: 'rgb(var(--text-main) / <alpha-value>)',
        muted: 'rgb(var(--text-muted) / <alpha-value>)',
      },
      backgroundColor: {
        base: 'rgb(var(--bg-base) / <alpha-value>)',
        surface: 'rgb(var(--bg-surface) / <alpha-value>)',
      },
      borderColor: {
        base: 'rgb(var(--border-base) / <alpha-value>)',
      }
    },
  },
  plugins: [],
}