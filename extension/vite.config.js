import { defineConfig } from 'vite';
import preact from '@preact/preset-vite';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const target = process.env.BUILD_TARGET;

/** Content Script (AI 对话捕获) — IIFE, 不能是 ES module */
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

/**
 * G1 Content Script (浏览状态捕获) — IIFE, 独立 entry
 * per W3 batch B + R02 P0-2: vite 多 entry 在 lib mode 不支持，必须独立 build。
 * manifest.json 引用 dist/content/browse_capture.js 由此 target 产出。
 */
function browseCaptureConfig() {
  return defineConfig({
    build: {
      outDir: resolve(__dirname, 'dist'),
      emptyOutDir: false,
      lib: {
        entry: resolve(__dirname, 'src/content/browse_capture.js'),
        name: 'AttuneBrowseCapture',
        formats: ['iife'],
        fileName: () => 'content/browse_capture.js',
      },
      rollupOptions: { output: { extend: true } },
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

const configs = {
  content: contentConfig,
  content_browse_capture: browseCaptureConfig,
  background: backgroundConfig,
  pages: pagesConfig,
};
const factory = configs[target] || configs.pages;
export default factory();
