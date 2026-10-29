// ESLint 9 Flat Config
//
// 设计原则：本次接入是「先能跑起来、只暴露不阻断」。
// 所有规则统一为 warn，配合 `npm run lint` 的 `--max-warnings 9999` 保证退出码为 0，
// 避免一次性引入上千条 error 让 CI 直接红掉。后续可按目录逐步把关键规则提升为 error。
//
// 注意：不要在这里加 type-aware 规则（需要 parserOptions.project，会显著拖慢 lint）。

import js from '@eslint/js';
import globals from 'globals';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';

/**
 * 把一份 config 里的 error 级别规则统一降级为 warn（已是 off/warn 的保持原样）。
 * 这样既能复用官方推荐集，又满足「只 warn 不 error」的接入约定。
 */
function downgradeErrorsToWarn(configs) {
  return configs.map((config) => {
    if (!config.rules) return config;
    const rules = {};
    for (const [name, severity] of Object.entries(config.rules)) {
      const level = Array.isArray(severity) ? severity[0] : severity;
      rules[name] = level === 'error' || level === 2 ? 'warn' : severity;
    }
    return { ...config, rules };
  });
}

export default tseslint.config(
  {
    name: 'jx3/ignores',
    ignores: [
      // 构建产物
      'dist/**',
      'dist-release/**',
      'dist-ssr/**',
      // Rust / Tauri 后端与构建缓存
      'src-tauri/**',
      // 依赖与本地缓存
      'node_modules/**',
      '**/*.tsbuildinfo',
      // 本地一次性排查脚本与临时产物
      'tmp/**',
      'scripts/**',
      'test-screenshots/**',
      '.tmp-screenshots/**',
      // 自动生成的静态数据，不参与 lint
      'src/data/kungfuData.ts',
    ],
  },

  js.configs.recommended,

  ...downgradeErrorsToWarn(tseslint.configs.recommended),

  {
    name: 'jx3/source-ts',
    files: ['**/*.{ts,tsx,mts,cts,js,jsx,mjs,cjs}'],
    languageOptions: {
      ecmaVersion: 'latest',
      sourceType: 'module',
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
      // 前端代码跑在 WebView 里，同时脚本与配置会用到 Node 全局
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      // === React Hooks ===
      'react-hooks/rules-of-hooks': 'warn',
      'react-hooks/exhaustive-deps': 'warn',

      // === React Refresh（Vite HMR 边界） ===
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],

      // === TypeScript ===
      '@typescript-eslint/no-explicit-any': 'warn',
      // 以 TS 版本为准，关掉核心规则避免重复报同一处
      'no-unused-vars': 'off',
      '@typescript-eslint/no-unused-vars': [
        'warn',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
      // 类型检查已由 tsc 负责，ESLint 不再重复判定全局符号
      'no-undef': 'off',

      // === 通用代码卫生 ===
      'no-console': 'warn',
      'no-empty': 'warn',
    },
  },

  {
    name: 'jx3/root-config-js',
    files: ['*.js', '*.mjs', '*.cjs'],
    rules: {
      // 根目录配置文件（tailwind/postcss/eslint）是 ESM，不需要额外放宽
      'no-undef': 'off',
    },
  },
);
