import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import kungfuDataPlugin from "./vite-plugin-kungfu-data";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react(), kungfuDataPlugin()],
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
