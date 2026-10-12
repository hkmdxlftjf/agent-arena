import path from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    proxy: {
      "/api": "http://127.0.0.1:18480",
      "/ws": { target: "ws://127.0.0.1:18480", ws: true },
      "/mcp": "http://127.0.0.1:18480",
      "/healthz": "http://127.0.0.1:18480",
      "/.well-known": "http://127.0.0.1:18480",
      "/skill": "http://127.0.0.1:18480",
    },
  },
});
