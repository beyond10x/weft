import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';
import { essUi } from './build/ess-ui-plugin.ts';

// Three pages: the chooser (index.html) and the two consoles, each its own entry with its own
// document. `base: './'` keeps the build relocatable (served under any path).
export default defineConfig({
  base: './',
  plugins: [react(), essUi()],
  build: {
    outDir: '../../target/console/dist',
    emptyOutDir: true,
    chunkSizeWarningLimit: 1200,
    rollupOptions: {
      input: {
        index: 'index.html',
        operator: 'operator/index.html',
        account: 'account/index.html',
      },
    },
  },
  server: { host: '127.0.0.1', port: 4416 },
});
