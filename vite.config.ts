import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// De UI staat in ui/. Tauri laadt hem tijdens ontwikkelen van poort 5173
// en in de echte app uit ui/dist.
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "dist", emptyOutDir: true, target: "es2021" },
});
