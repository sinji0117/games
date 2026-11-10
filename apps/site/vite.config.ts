import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      // changeOrigin: false — Host 를 그대로 넘겨야 서버의 Origin 확인(CSRF)과 맞음
      "/api": { target: process.env.VITE_API_TARGET ?? "http://localhost:8080", changeOrigin: false, ws: true },
    },
  },
});
