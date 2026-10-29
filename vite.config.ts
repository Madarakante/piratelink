import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";

// Tauri expects a fixed port and fails if it is not available.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebView2 (Chromium 155) on the target machines; keep output modern + small.
    target: "chrome110",
    // Vite 8 ships rolldown/oxc; esbuild is no longer a dependency.
    minify: "oxc",
    cssMinify: true,
    sourcemap: false,
    chunkSizeWarningLimit: 600,
  },
});
