import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import svgr from 'vite-plugin-svgr';
import { visualizer } from 'rollup-plugin-visualizer';
// https://vitejs.dev/config/
export default defineConfig({
  // WkWebkitWebview specifics
  base: '',
  // Don't serve public static assets, have vite process all assets
  publicDir: false,

  plugins: [
    svgr(),
    react(),
    visualizer(),
  ],
  // prevent vite from obscuring rust errors
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // WK_WEB_VIEW will be defined when using the Dev Client scheme in XCode
    open: process.env.WK_WEB_VIEW === undefined
  },
  // env variables
  envPrefix: ['VITE_', 'OBS_WEB_'],

  build: {
    target: ['es2021', 'safari14'],
    minify: 'esbuild',
    // produce sourcemaps for debug builds
    sourcemap: false,
    outDir: 'build',
  },

  resolve: {
    alias: {
      "$licenses.json": process.env.LICENSE_JSON!
    }
  },
})
