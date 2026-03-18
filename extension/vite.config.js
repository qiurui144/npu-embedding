import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const target = process.env.BUILD_TARGET;

/** Content Script — IIFE, 不能是 ES module */
function contentConfig() {
  return defineConfig({
    build: {
      outDir: resolve(__dirname, 'dist'),
      emptyOutDir: false,
      lib: {
        entry: resolve(__dirname, 'src/content/index.js'),
        name: 'NpuWebhookContent',
        formats: ['iife'],
        fileName: () => 'content/index.js',
      },
      rollupOptions: {
        output: { extend: true },
      },
    },
    define: { 'process.env.NODE_ENV': '"production"' },
  });
}

/** Background Worker — ES module */
function backgroundConfig() {
  return defineConfig({
    build: {
      outDir: resolve(__dirname, 'dist'),
      emptyOutDir: false,
      lib: {
        entry: resolve(__dirname, 'src/background/worker.js'),
        formats: ['es'],
        fileName: () => 'background/worker.js',
      },
    },
    define: { 'process.env.NODE_ENV': '"production"' },
  });
}

/** HTML pages (popup / sidepanel / options) — Preact, root=src 让 HTML 输出不带 src/ 前缀 */
function pagesConfig() {
  return defineConfig({
    root: resolve(__dirname, 'src'),
    base: './',
    plugins: [preact()],
    build: {
      outDir: resolve(__dirname, 'dist'),
      emptyOutDir: true,
      rollupOptions: {
        input: {
          sidepanel: resolve(__dirname, 'src/sidepanel/index.html'),
          popup: resolve(__dirname, 'src/popup/index.html'),
          options: resolve(__dirname, 'src/options/index.html'),
        },
        output: {
          entryFileNames: '[name]/[name].js',
          chunkFileNames: 'shared/[name]-[hash].js',
          assetFileNames: '[name]/[name].[ext]',
        },
      },
    },
  });
}

const configs = { content: contentConfig, background: backgroundConfig, pages: pagesConfig };
const factory = configs[target] || configs.pages;
export default factory();
