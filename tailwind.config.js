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
    "./constants/**/*.{js,ts,jsx,tsx}",
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
      },
      textColor: {
        main: "rgb(var(--text-main) / <alpha-value>)",
        muted: "rgb(var(--text-muted) / <alpha-value>)",
      },
      backgroundColor: {
        base: "rgb(var(--bg-base) / <alpha-value>)",
        surface: "rgb(var(--bg-surface) / <alpha-value>)",
      },
      borderColor: {
        base: "rgb(var(--border-base) / <alpha-value>)",
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      fontFamily: {
        sans: ["Inter", "ui-sans-serif", "system-ui", "-apple-system", "BlinkMacSystemFont", "Segoe UI", "Roboto", "Helvetica Neue", "Arial", "sans-serif"],
      }
    },
  },
  plugins: [],
};