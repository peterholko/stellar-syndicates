import { defineConfig } from "vite";

// The client is a static SPA. In development it runs on Vite's own port and
// proxies account and game traffic to Rust on port 8080. Cookies/passwords stay
// on the page origin; no credentialed wildcard CORS or URL-token auth. A
// production build (`npm run build`) emits to `dist/`, which the Rust server
// serves directly for a one-command run.
export default defineConfig({
  server: {
    port: 5173,
    host: true,
    proxy: {
      "/api": { target: "http://127.0.0.1:8080" },
      "/ws": { target: "ws://127.0.0.1:8080", ws: true },
      "/status": { target: "http://127.0.0.1:8080" },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
