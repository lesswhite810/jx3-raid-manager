import { configDefaults, defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import kungfuDataPlugin from "./vite-plugin-kungfu-data";

// https://vitejs.dev/config/
export default defineConfig(() => ({
  plugins: [react(), kungfuDataPlugin()],
  test: {
    // tmp/ 为本地一次性排查脚本的临时目录（已被 .gitignore 忽略），
    // 其中的 *.test.cjs 是历史残留且不含测试套件，需排除以免本地测试误红。
    exclude: [...configDefaults.exclude, "tmp/**"],
    // 覆盖率基线观察用（v8 provider）。先不设 thresholds 硬门槛，
    // 待基线稳定后再决定是否收紧；`npm run coverage` 会读取这里的配置。
    coverage: {
      provider: "v8",
      reporter: ["text", "html"],
      include: ["services/**/*.ts", "utils/**/*.ts", "hooks/**/*.ts", "contexts/**/*.tsx"],
      exclude: [
        "**/*.test.ts",
        "**/*.test.tsx",
        // 自动生成的静态数据，无测试意义
        "src/data/**",
        // 仅浏览器 dev 环境下的 IPC mock 数据
        "services/mockInvoke.ts",
      ],
    },
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  esbuild: {
    // 生产构建移除调试日志，保留 error/warn 用于问题排查
    drop: process.env.NODE_ENV === 'production' ? ['console'] : [],
  },
  // 确保所有依赖都打包到本地，不使用外部CDN
  build: {
    chunkSizeWarningLimit: 700,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined;
          }

          if (id.includes('recharts') || id.includes('d3-')) {
            return 'charts-vendor';
          }

          if (id.includes('react-datepicker') || id.includes('date-fns')) {
            return 'date-vendor';
          }

          return 'vendor';
        },
      },
    },
    commonjsOptions: {
      transformMixedEsModules: true,
    },
  },
}));
