// Start this loopback-only baker, then open its printed URL in a browser.
// It renders real WebGL LOD2 models, never enlarges a thumbnail or uses Blender.
import { createServer } from "vite";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { buildShipModelAssets } from "./build-ship-model-assets.mjs";
import { buildShipDerivatives } from "./build-ship-derivatives.mjs";
import { SHIP_MODELS } from "./ship-model-catalog.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const output = path.join(root, "art-src/ship-models/game-ready-v1");
await mkdir(output, { recursive: true });
await buildShipModelAssets();
const assets = JSON.parse(await readFile(path.join(root, "public/art/derived/ship-models/v1/manifest.json"), "utf8"));
const manifest = { recipe: "lod2-ortho-pbr-v1", ships: {} };
const received = new Set();
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const views = ["top", "perspective", "bottom"];
const previous = await readFile(path.join(output, "bake-manifest.json"), "utf8").then(JSON.parse).catch(() => null);
const plan = [];
for (const { kind, title } of SHIP_MODELS) {
  const model = assets.ships[kind], old = previous?.ships[kind];
  const pending = [];
  manifest.ships[kind] = { source: model.source, sourceSha256: model.sha256, triangles: model.triangles };
  for (const view of views) {
    // Reuse only verified renders of these exact model bytes. Adding a hull
    // must not repaint older ships due to a different browser/GPU PNG encoder.
    const bytes = await readFile(path.join(output, `${kind}-${view}.png`)).catch(() => null);
    if (previous?.recipe === manifest.recipe && old?.sourceSha256 === model.sha256
        && bytes && hash(bytes) === old[`${view}Sha256`]) {
      manifest.ships[kind][`${view}Sha256`] = old[`${view}Sha256`];
      received.add(`${kind}/${view}`);
    } else pending.push(view);
  }
  plan.push({ kind, title, views: pending });
}
const server = await createServer({ root, configFile: false,
  server: { host: "127.0.0.1", port: 8780, strictPort: true },
  plugins: [{ name: "ship-model-baker", configureServer(vite) {
    vite.middlewares.use(async (req, res, next) => {
      if (!req.url?.startsWith("/__ship-bake/")) return next();
      if (req.method === "GET" && req.url === "/__ship-bake/plan") {
        res.setHeader("Content-Type", "application/json"); res.end(JSON.stringify(plan)); return;
      }
      // Only same-origin requests can write the fixed, allowlisted render files.
      if (req.method !== "POST" || req.headers.origin !== "http://127.0.0.1:8780") { res.writeHead(403).end(); return; }
      try {
        const parts = req.url.slice("/__ship-bake/".length).split("/");
        if (parts[0] === "done") {
          if (received.size !== SHIP_MODELS.length * 3) throw new Error("Incomplete render set");
          await writeFile(path.join(output, "bake-manifest.json"), JSON.stringify(manifest, null, 2) + "\n");
          const result = await buildShipDerivatives();
          res.setHeader("Content-Type", "application/json"); res.end(JSON.stringify(result));
          console.log("Model artwork completed:", result);
          return;
        }
        const [kind, view] = parts;
        if (!Object.hasOwn(assets.ships, kind) || !views.includes(view)) { res.writeHead(404).end(); return; }
        if (received.has(`${kind}/${view}`)) { req.resume(); res.end("verified render retained"); return; }
        const chunks = []; let size = 0;
        for await (const chunk of req) { size += chunk.length; if (size > 8_000_000) throw new Error("Render too large"); chunks.push(chunk); }
        const png = Buffer.concat(chunks);
        if (png.subarray(0, 8).toString("hex") !== "89504e470d0a1a0a") throw new Error("Expected PNG");
        const expected = view === "top" ? 1254 : 768;
        if (png.readUInt32BE(16) !== expected || png.readUInt32BE(20) !== expected) throw new Error("Unexpected render dimensions");
        const file = path.join(output, `${kind}-${view}.png`);
        await writeFile(file, png);
        const model = assets.ships[kind];
        manifest.ships[kind] ??= { source: model.source, sourceSha256: model.sha256, triangles: model.triangles };
        manifest.ships[kind][`${view}Sha256`] = hash(png);
        received.add(`${kind}/${view}`);
        res.end("ok");
      } catch (error) { console.error(error); res.writeHead(500).end(String(error)); }
    });
  } }],
});
await server.listen();
console.log("Ship art baker: http://127.0.0.1:8780/scripts/ship-model-bake.html");
