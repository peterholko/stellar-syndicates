import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";
import { PROTOCOL_VERSION } from "./src/wire.mjs";

/** §client-updates (Nov 11 2026, docs/client-updates.md): stamps each build so
 * an open tab can learn a newer one is out. The stamp hashes every emitted
 * file's name, which Vite derives from content, so it changes exactly when the
 * client does. It goes into each page's `ss-build` meta, and into
 * `version.json` beside them with the wire protocol (src/client-update.ts). */
export function buildStamp(): Plugin {
  let outDir = "dist";
  return {
    name: "ss-build-stamp",
    apply: "build",
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    writeBundle(_options, bundle) {
      const files = Object.keys(bundle);
      const build = createHash("sha256").update(files.filter((f) => !f.endsWith(".html")).sort().join("\n"))
        .digest("hex").slice(0, 12);
      for (const page of files.filter((f) => f.endsWith(".html"))) {
        const path = resolve(outDir, page);
        writeFileSync(path, readFileSync(path, "utf8")
          .replace('<meta name="ss-build" content="dev" />', `<meta name="ss-build" content="${build}" />`));
      }
      writeFileSync(resolve(outDir, "version.json"), `${JSON.stringify({ build, protocol: PROTOCOL_VERSION })}\n`);
    },
  };
}

// The client is a static SPA. In development it runs on Vite's own port and
// proxies account and game traffic to Rust on port 8080. Cookies/passwords stay
// on the page origin; no credentialed wildcard CORS or URL-token auth. A
// production build (`npm run build`) emits to `dist/`, which the Rust server
// serves directly for a one-command run.
export default defineConfig({
  plugins: [buildStamp()],
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
    // The operator dashboard is its own page; it shares nothing with the game
    // bundle beyond the design tokens.
    rollupOptions: {
      input: {
        index: "index.html",
        admin: "admin.html",
      },
    },
  },
});
