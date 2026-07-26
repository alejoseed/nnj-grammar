import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [tailwindcss()],
  server: {
    proxy: {
      // Forward API calls to the Rust backend (nnj-grammar-server).
      "/api": "http://127.0.0.1:7878",
    },
    fs: {
      allow: [fileURLToPath(new URL("..", import.meta.url))],
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
