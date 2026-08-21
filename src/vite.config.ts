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
      "@i18n": path.join(sourceDir, "i18n/index.ts"),
    },
  },
  build: {
    assetsInlineLimit: 0,
    chunkSizeWarningLimit: 1300,
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
    include: ["@tauri-apps/api/app"],
  },
  clearScreen: false,
});
