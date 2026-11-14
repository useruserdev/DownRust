// SPDX-License-Identifier: GPL-3.0-or-later
import {defineConfig} from 'vite';
import react from '@vitejs/plugin-react';
import {astryxStylex} from '@astryxdesign/build/vite';

export default defineConfig({
  plugins: [...astryxStylex(), react()],   // order matters: astryxStylex first
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {ignored: ['**/src-tauri/**', '**/crates/**', '**/target/**', '**/extension/**']},
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {input: {main: 'index.html', dropzone: 'dropzone.html'}},
  },
});
