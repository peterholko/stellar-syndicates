// Start this loopback-only baker, then open its printed URL in a browser. It
// renders the Authority Nexus with real WebGL (the live map's lighting, camera
// and still vortex phase) and then builds the web assets. No proxy, no game.
import { createServer } from "vite";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { buildHubModelAssets, HUB_BAKE, HUB_SOURCE } from "./build-hub-model-assets.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const output = path.join(root, HUB_BAKE);
await mkdir(output, { recursive: true });
const PORT = 8783;
const SIZES = { top: [1254, 1254], hero: [1024, 768] };
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const manifest = { recipe: "nexus-ortho-top-perspective-hero-v1", source: HUB_SOURCE,
  sourceSha256: hash(await readFile(path.join(root, HUB_SOURCE))) };
const server = await createServer({ root, configFile: false,
  server: { host: "127.0.0.1", port: PORT, strictPort: true },
  plugins: [{ name: "hub-model-baker", configureServer(vite) {
    vite.middlewares.use(async (req, res, next) => {
      if (!req.url?.startsWith("/__hub-bake/")) return next();
      // Only same-origin requests can write the fixed, allowlisted render files.
      if (req.method !== "POST" || req.headers.origin !== `http://127.0.0.1:${PORT}`) { res.writeHead(403).end(); return; }
      try {
        const view = req.url.slice("/__hub-bake/".length);
        if (view === "done") {
          if (!manifest.topSha256 || !manifest.heroSha256) throw new Error("Incomplete render set");
          await writeFile(path.join(output, "bake-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
          const result = await buildHubModelAssets();
          res.setHeader("Content-Type", "application/json"); res.end(JSON.stringify(result));
          console.log("Hub artwork completed:", result);
          return;
        }
        if (!Object.hasOwn(SIZES, view)) { res.writeHead(404).end(); return; }
        const chunks = []; let size = 0;
        for await (const chunk of req) { size += chunk.length; if (size > 12_000_000) throw new Error("Render too large"); chunks.push(chunk); }
        const png = Buffer.concat(chunks);
        if (png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") throw new Error("Expected PNG");
        const [width, height] = SIZES[view];
        if (png.readUInt32BE(16) !== width || png.readUInt32BE(20) !== height) throw new Error("Unexpected render dimensions");
        await writeFile(path.join(output, `${view}.png`), png);
        manifest[`${view}Sha256`] = hash(png);
        res.end("ok");
      } catch (error) { console.error(error); res.writeHead(500).end(String(error)); }
    });
  } }],
});
await server.listen();
console.log(`Hub art baker: http://127.0.0.1:${PORT}/scripts/hub-model-bake.html`);
