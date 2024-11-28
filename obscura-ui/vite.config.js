import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import svgr from 'vite-plugin-svgr';
// import topLevelAwait from 'vite-plugin-top-level-await';
import { visualizer } from 'rollup-plugin-visualizer';
import removeConsole from 'vite-plugin-remove-console';
// https://vitejs.dev/config/
// https://tauri.app/v1/guides/getting-started/setup/vite#create-the-frontend
export default defineConfig({
  // WkWebkitWebview specifics
  base: '',

  plugins: [
    svgr(),
    react(),
    // topLevelAwait({
    //   promiseExportName: '__TLA',
    //   promiseImportName: i => `__TLA_${i}`
    // }),
    visualizer(),
    // removeConsole({includes: ['log', 'assert', 'info', 'error']})
  ],
  // prevent vite from obscuring rust errors
  clearScreen: false,
  // Tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    // open in browser if not running with tauri
    // WK_WEB_VIEW is defined in the XCode client (hot reload) scheme under run pre-action
    open: process.env.TAURI_ARCH === undefined && process.env.WK_WEB_VIEW === undefined
  },
  // to make use of `TAURI_PLATFORM`, `TAURI_ARCH`, `TAURI_FAMILY`,
  // `TAURI_PLATFORM_VERSION`, `TAURI_PLATFORM_TYPE` and `TAURI_DEBUG`
  // env variables
  envPrefix: ['VITE_', 'TAURI_'],

  build: {
    target: ['es2021', 'safari14'],
    // don't minify for debug builds
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    // produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_DEBUG,
    outDir: 'build',
  },

  resolve: {
    alias: {
      "$licenses.json": process.env.LICENSE_JSON,
    },
  },
})
