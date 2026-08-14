import { defineConfig } from "vitest/config";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [tailwindcss()],
  ...(process.env.NIRMATA_VITE_CACHE_DIR ? { cacheDir: process.env.NIRMATA_VITE_CACHE_DIR } : {}),
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2022",
    sourcemap: false,
  },
  test: {
    environment: "jsdom",
    include: ["tests/**/*.test.ts", "tests/**/*.test.tsx"],
    restoreMocks: true,
  },
});
