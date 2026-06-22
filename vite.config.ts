import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// The frontend entry (index.html) lives in src-tauri/ alongside the Tauri
// config, so Vite's root points there. The build output still lands in the
// project-root dist/ to match tauri.conf.json's frontendDist ("../dist").
export default defineConfig({
  root: "src-tauri",
  cacheDir: "../node_modules/.vite",
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2020",
    minify: !process.env.TAURI_DEBUG,
    sourcemap: !!process.env.TAURI_DEBUG
  }
});
