import { defineConfig } from "vite";

// The client is a static SPA. In development it runs on Vite's own port and
// connects to the Rust server's WebSocket on port 8080 (see src/net.ts). A
// production build (`npm run build`) emits to `dist/`, which the Rust server
// serves directly for a one-command run.
export default defineConfig({
  server: {
    port: 5173,
    host: true,
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
