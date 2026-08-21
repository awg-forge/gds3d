import { svelte } from "@sveltejs/vite-plugin-svelte";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

// @ts-expect-error Vite loads this config in Node, but the browser tsconfig omits Node globals.
const host = process.env.TAURI_DEV_HOST;
const sourceDir = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: sourceDir,
  plugins: [svelte()],
  publicDir: path.resolve(sourceDir, "../static"),
  resolve: {
    alias: {
      "@api": path.join(sourceDir, "api"),
      "@components": path.join(sourceDir, "lib/components"),
      "@domain": path.join(sourceDir, "domain"),
      "@i18n": path.join(sourceDir, "i18n/index.ts"),
      "@models": path.join(sourceDir, "models"),
      "@themes": path.join(sourceDir, "themes"),
    },
  },
  build: {
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 600,
    emptyOutDir: true,
    outDir: path.resolve(sourceDir, "../dist"),
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  optimizeDeps: {
    include: [
      "@lucide/svelte",
      "@tauri-apps/api/app",
      "@tauri-apps/api/core",
      "@tauri-apps/api/event",
      "@tauri-apps/api/window",
      "@tauri-apps/plugin-deep-link",
      "bits-ui",
    ],
  },
  clearScreen: false,
});
