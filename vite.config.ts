import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // Loopback by default; set TAURI_DEV_HOST for physical-device testing.
    host: process.env.TAURI_DEV_HOST || '127.0.0.1',
    hmr: process.env.TAURI_DEV_HOST ? { host: process.env.TAURI_DEV_HOST } : undefined,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
});
