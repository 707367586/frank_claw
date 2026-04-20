import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:18800",
        changeOrigin: false,
      },
      "/pico/ws": {
        target: "ws://127.0.0.1:18800",
        ws: true,
        changeOrigin: false,
      },
    },
  },
});
