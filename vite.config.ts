import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
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
  // 确保所有依赖都打包到本地，不使用外部CDN
  build: {
    chunkSizeWarningLimit: 700,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes('node_modules')) {
            return undefined;
          }

          if (id.includes('react') || id.includes('scheduler')) {
            return 'react-vendor';
          }

          if (id.includes('recharts') || id.includes('d3-')) {
            return 'charts-vendor';
          }

          if (id.includes('react-datepicker') || id.includes('date-fns') || id.includes('dayjs')) {
            return 'date-vendor';
          }

          if (id.includes('antd') || id.includes('@ant-design')) {
            return 'antd-vendor';
          }

          if (id.includes('@google/genai')) {
            return 'ai-vendor';
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
