// Galaxy map points of interest: exploration contacts are cyan sensor circles,
// operation contracts are issuer-coloured tags. Glyph art provenance, the pure
// rules, the contract layer (with recording Pixi stand-ins) and the wiring.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import sharp from "sharp";
import ts from "typescript";
import { GLYPH_SHEETS, GLYPH_SOURCE } from "./build-map-glyphs.mjs";

const client = new URL("../", import.meta.url);
const read = path => readFileSync(new URL(path, client));
const src = path => read(`src/${path}`).toString("utf8");
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const compile = (path, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {} });
  return exports;
};

// --- The glyph set: every named glyph, cache-busted, a tintable white silhouette.
const { MAP_GLYPHS } = compile("map-glyphs.generated.ts");
const names = GLYPH_SHEETS.flatMap(([, cells]) => cells);
assert.deepEqual(Object.keys(MAP_GLYPHS).sort(), [...names].sort(), "one glyph per generated cell");
const validation = JSON.parse(read(`${GLYPH_SOURCE}/image-validation.json`));
for (const [file] of GLYPH_SHEETS) {
  assert.equal(hash(read(`${GLYPH_SOURCE}/${file}`)), validation.sheets.find(entry => entry.file === file).sha256, `${file}: the recorded generation`);
}
for (const [name, url] of Object.entries(MAP_GLYPHS)) {
  const [path, version] = url.split("?v=");
  const bytes = read(`public${path}`);
  assert.equal(version, hash(bytes).slice(0, 12), `${name}: content-hashed URL`);
  const { data, info } = await sharp(bytes).raw().toBuffer({ resolveWithObject: true });
  assert.deepEqual([info.width, info.height, info.channels], [128, 128, 4], `${name}: 128 px RGBA`);
  let minX = 128, maxX = -1, minY = 128, maxY = -1, coloured = 0;
  for (let i = 0; i < 128 * 128; i++) {
    if (data[i * 4 + 3] < 128) continue;
    if (data[i * 4] < 250 || data[i * 4 + 1] < 250 || data[i * 4 + 2] < 250) coloured++;
    const x = i % 128, y = Math.floor(i / 128);
    minX = Math.min(minX, x); maxX = Math.max(maxX, x); minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  }
  assert.equal(coloured, 0, `${name}: pure white ink, tinted on the map`);
  const span = Math.max(maxX - minX + 1, maxY - minY + 1);
  assert.ok(Math.abs(span - 128 * .84) <= 3, `${name}: fitted to the set's box (${span} px)`);
  assert.ok(Math.abs((minX + maxX + 1) / 2 - 64) <= 2 && Math.abs((minY + maxY + 1) / 2 - 64) <= 2, `${name}: centred`);
  assert.equal(data[3], 0, `${name}: transparent ground`);
}

// --- Rules.
const exploration = { siteTitle: site => site.details?.name ?? `Unknown contact ${site.id}`,
  siteStatus: site => site.status ?? (site.details ? "Recovery available" : "Unidentified · Scout needed") };
const markers = compile("core/derive/map-markers.ts", { "./exploration": exploration,
  "./format": { fmt: n => n.toLocaleString("en-US"), fmtDur: s => `${Math.round(s / 60)}m`, operationTitle: op => op.title ?? "Contract" } });
let previous = 0;
for (const zoom of [.5, 1, 2, 4, 8, 16, 24, 96]) {
  const size = markers.contactBadgePx(zoom);
  assert.ok(size >= previous && size >= 18 && size <= 28, `badges grow gently with zoom (${zoom}×)`);
  previous = size;
}
assert.equal(markers.contactBadgePx(1), 18);
assert.equal(markers.contactBadgePx(24), 28);
assert.equal(markers.contractTagPx(24), 24);
const unknown = markers.contactStyle({ id: "E1", details: null });
assert.deepEqual([unknown.glyph, unknown.unknown, unknown.pip], ["contact-unknown", true, null], "a signal in a dashed ring, no state");
const pips = { "Guarded · escorts recommended": 0xef6b5e, "Recovery available": 0x6fd08c, "New coordinates recovered": 0xf3c969 };
for (const [status, color] of Object.entries(pips)) {
  const style = markers.contactStyle({ id: "E2", status, details: { kind: "anomaly" } });
  assert.deepEqual([style.glyph, style.pip], ["site-anomaly", color], status);
}
const spent = markers.contactStyle({ id: "E3", status: "Depleted", details: { kind: "asteroids" } });
assert.ok(spent.alpha < .5 && spent.pip === null && spent.ring !== markers.CONTACT_COLOR, "a spent site greys out");
const kinds = ["combat_objective", "privateer_patrol", "pirate_bounty", "survey_expedition", "market_delivery", "rescue_salvage",
  "prize_recovery", "convoy_escort", "freight_escort", "authority_enforcement", "strategic_control", "regional_mandate", "syndicate_megaproject"];
const protocolKinds = [...src("protocol.ts").match(/export type OperationKind =([\s\S]*?);\n/)[1].matchAll(/kind: "([a-z_]+)"/g)].map(m => m[1]);
assert.deepEqual([...kinds].sort(), [...protocolKinds].sort(), "every operation kind is covered");
const op = (kind, extra = {}) => ({ id: `O-${kind}`, issuer: "authority", scope: { scope: "public" }, kind: { kind }, state: "offered",
  progress: 0, goal: 4, expires_at: 0, target_pos: { x: 0, y: 0 }, reward: { credits: 1200 }, joined: false, ...extra });
for (const kind of kinds) {
  const style = markers.contractStyle(op(kind));
  assert.ok(Object.hasOwn(MAP_GLYPHS, style.glyph) && style.glyph.startsWith("contract-"), `${kind}: a contract glyph`);
  assert.ok(style.label && style.label !== "Contract", `${kind}: a short label`);
}
const colors = Object.values(markers.ISSUER_COLOR);
assert.equal(new Set(colors).size, colors.length, "each issuer has its own colour");
assert.ok(!colors.includes(markers.CONTACT_COLOR), "no contract shares the contacts' cyan");
assert.equal(markers.contractStyle(op("pirate_bounty")).filled, false, "offered: hollow");
const running = markers.contractStyle(op("privateer_patrol", { state: "active", joined: true, progress: 1 }));
assert.deepEqual([running.filled, running.progress], [true, .25], "joined and running: filled, with progress");
assert.equal(markers.contractStyle(op("privateer_patrol", { state: "active", joined: false, progress: 1 })).progress, null, "someone else's run shows no bar");
assert.ok(markers.contractOnMap(op("pirate_bounty")) && !markers.contractOnMap(op("pirate_bounty", { state: "completed" })));
const freya = { id: "1", name: "Freya", pos: { x: 1000, y: 0 } };
assert.equal(markers.contractStar(op("survey_expedition", { target_pos: { x: 1000, y: 120 } }), [freya]), freya, "a survey of Freya belongs to Freya");
assert.equal(markers.contractStar(op("privateer_patrol", { target_pos: { x: 3000, y: 0 } }), [freya]), null);
assert.equal(markers.contractBrief(op("pirate_bounty", { title: "Suppress Kepler enclave", expires_at: 3600 }), 0),
  "Suppress Kepler enclave · Authority · 1,200 cr · 60m left · click to review the offer");
assert.equal(markers.contactBrief({ id: "E9", details: null }, "Scout"), "Unknown contact E9 · Unidentified · Scout needed · click to send Scout");

// --- The contract layer, drawn into recording stand-ins.
class Node {
  constructor() { this.position = { set: (x, y) => { this.x = x; this.y = y; } }; this.anchor = { set() {} }; this.children = []; }
  addChild(...nodes) { this.children.push(...nodes); }
  destroy() { this.destroyed = true; }
}
class Text extends Node {
  constructor({ text, style }) { super(); this.text = text; this.style = { ...style }; }
  get width() { return this.text.length * 6; }
}
class Graphics extends Node {
  log = [];
  clear() { this.log = []; return this; }
  roundRect(...args) { this.log.push(["roundRect", ...args]); return this; }
  rect(...args) { this.log.push(["rect", ...args]); return this; }
  circle(...args) { this.log.push(["circle", ...args]); return this; }
  fill(style) { this.log.push(["fill", style]); return this; }
  stroke(style) { this.log.push(["stroke", style]); return this; }
}
const { OperationLayer } = compile("operationlayer.ts", {
  "pixi.js": { Container: Node, Sprite: Node, Text, Graphics, Texture: { EMPTY: null } },
  "./core/derive/map-markers": markers,
  "./map-glyph-textures": { loadMapGlyphs: async () => new Map(Object.keys(MAP_GLYPHS).map(name => [name, { glyph: name }])) },
});
const layer = new OperationLayer();
await layer.load();
const gfx = layer.root.children[0];
const offered = op("pirate_bounty", { id: "A", target_pos: { x: 5000, y: 0 } });
const joined = op("privateer_patrol", { id: "B", issuer: "syndicate", state: "active", joined: true, progress: 2, target_pos: { x: 8000, y: 0 } });
const survey = op("survey_expedition", { id: "C", issuer: "survey_office", target_pos: { x: 1000, y: 0 } });
const salvage = op("rescue_salvage", { id: "D", issuer: "salvage_office", target_pos: { x: 1000, y: 0 } });
const project = pos => ({ x: pos.x / 10, y: 300 + pos.y / 10 });
layer.draw([offered, joined, survey, salvage, op("pirate_bounty", { id: "gone", state: "expired" })], project, 2, [freya]);
const tag = id => layer.tags.get(id);
assert.equal(layer.tags.size, 4, "only offered and running contracts stand on the map");
const fills = gfx.log.filter(entry => entry[0] === "fill").map(entry => entry[1]);
assert.ok(fills.some(style => style.color === markers.ISSUER_COLOR.syndicate && style.alpha > .9), "a joined contract is filled in its issuer's colour");
assert.ok(gfx.log.some(entry => entry[0] === "stroke" && entry[1].color === markers.ISSUER_COLOR.authority), "an offered contract is outlined in its issuer's colour");
assert.equal(tag("A").sprite.texture.glyph, "contract-bounty");
assert.equal(tag("A").sprite.tint, markers.ISSUER_COLOR.authority, "a hollow tag's glyph takes the issuer colour");
assert.equal(tag("B").sprite.tint, 0x0b1220, "a filled tag's glyph is dark on the colour");
assert.deepEqual([tag("A").sprite.x, tag("A").sprite.y], [500, 300], "a free-space contract's glyph marks its point");
assert.equal(tag("A").text.visible, false, "zoomed out: glyph only");
assert.equal(tag("B").text.text, "Patrol 50%", "a joined contract always names itself, with its progress");
assert.ok(gfx.log.some(entry => entry[0] === "rect" && Math.abs(entry[3] - (tag("B").text.width + 6 + markers.contractTagPx(2)) * .5) < 1e-6), "and a half-full progress bar");
const star = project(freya.pos);
assert.ok(tag("C").sprite.x > star.x + 14 && tag("C").sprite.y < star.y - 14, "a contract at a star stands beside it, up and right");
assert.ok(tag("D").sprite.y < tag("C").sprite.y - markers.contractTagPx(2), "a second contract there stacks above the first");
assert.equal(layer.pick(tag("C").sprite.x, tag("C").sprite.y), "C");
assert.equal(layer.pick(star.x, star.y), null, "the star itself stays the star's");
layer.draw([offered], project, 8, [freya]);
assert.equal(tag("A").text.text, "Bounty", "closer in: the kind is spelled out");
assert.equal(tag("A").text.visible, true);
assert.ok(layer.tags.size === 1 && !layer.tags.has("B"), "finished contracts leave the map");

// --- Wiring.
const render = src("render.ts");
assert.match(render, /this\.operationLayer\.root, \/\/ known contract\/objective sites, under routes and fleets/);
assert.match(render, /operationPick\(sx: number, sy: number\): string \| null \{ return this\.operationLayer\.pick\(sx, sy\); \}/);
assert.match(render, /state\.explorationJournal, state\.galaxy\?\.systems, this\.zoomFactor\(\)\)/, "contacts size with zoom");
assert.doesNotMatch(src("explorationlayer.ts"), /\/art\/exploration\//, "the map no longer draws the painted site art; panels keep it");
assert.match(src("shell/deck/map.ts"), /contractBrief\(operation, liveSimTime\(\)\)/, "contracts explain themselves on hover");
assert.match(src("shell/deck/map.ts"), /contactBrief\(site,/, "and so do contacts");
assert.match(src("shell/deck/index.ts"), /case "operation":\n\s*this\.router\.go\(\{ name: "operations", query: \{ contract: target\.id \} \}\);/, "a contract click opens its card");
assert.match(src("shell/mobile/map.ts"), /case "operation":/);

console.log(`Map markers: ${names.length} provenance-checked white glyphs, contact badges and state pips, contract tags for all ${kinds.length} kinds in issuer colours (hollow/filled/progress, beside stars, stacked, picked) and hover/click wiring passed.`);

// --serve: the real renderer over a canned region with every kind of contact
// and contract, an own Interceptor and a rival. ?zoom=N&x=&y= frames it.
// No game server.
if (process.argv.includes("--serve")) {
  const { createServer } = await import("node:http");
  const { resolve, sep } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(client);
  const module = path => JSON.stringify(resolve(clientRoot, `src/${path}`));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "markers-fixture", resolveId: id => id.endsWith("virtual:markers-fixture") ? "\0markers-fixture" : undefined,
      load: id => id === "\0markers-fixture" ? `
      import { renderer } from ${module("render.ts")};
      import { state } from ${module("state.ts")};
      const params = new URLSearchParams(location.search);
      (async () => {
      state.playerId = "me"; state.link = "online";
      const sys = (id, name, x, y) => ({ id, name, pos: { x, y }, band: "rich", body_count: 3 });
      state.galaxy = { hub: { x: 0, y: 0 }, radius: 300000, nebulas: [], c: 400, sensor_range: 80000,
        jump_range: 50000, jump_spool_s: 10, hyperlimit: 900, scout_sensor_mult: 1, convoy_sensor_mult: 1,
        sensor_array_base: 0, sensor_array_per_tier: 0, defense_platform_radius: 0, build_options: [],
        systems: [sys("1", "Freya", 60000, -20000), sys("2", "Vega", -90000, 40000), sys("3", "Aster", 30000, 110000),
          sys("4", "Lyra", -40000, -120000), sys("5", "Orin", 95000, 60000), sys("6", "Kepler", 130000, -60000)] };
      const site = (id, x, y, details = null) => ({ id, pos: { x, y }, reported_at: 0, details });
      const d = (kind, name, extra = {}) => ({ kind, name, cargo: {}, modules: {}, programme: "", research_fraction: 0, ...extra });
      state.explorationSites = [
        site("E101", 70000, 10000), site("E102", 110000, 20000),
        site("E103", 80000, 40000, d("derelict", "Wreck of the Calliope", { cargo: { alloys: 40 } })),
        site("E104", 50000, 30000, d("station", "Lantern Relay")),
        site("E105", 100000, -20000, d("asteroids", "Cinder Belt")),
        site("E106", 120000, 30000, d("anomaly", "Violet Fold", { guarded: true, cargo: { electronics: 12 } })),
        site("E107", 85000, -40000, d("precursor", "Spire of Ashur", { opportunity: { task: "study", requirement: "scout", seconds: 600, costs: {}, cargo: {}, has_lead: false } })),
      ];
      const op = (id, kind, target, extra = {}) => ({ id, issuer: "authority", scope: { scope: "public" }, kind, state: "offered",
        progress: 0, goal: 1, stage: 0, reported_at: 0, offered_at: 0, starts_at: 0, expires_at: 7200, target_pos: target,
        reward: { credits: 1200, authority_standing: 2, captain_xp: 40, research_insight: 0 }, joined: false, ...extra });
      state.operations = [
        op("O1", { kind: "pirate_bounty", system: "6", tier: 2 }, { x: 130000, y: -60000 }),
        op("O2", { kind: "privateer_patrol", pos: { x: 75000, y: -5000 } }, { x: 75000, y: -5000 }, { state: "active", joined: true, scope: { scope: "private", player: "me" } }),
        op("O3", { kind: "rescue_salvage", pos: { x: 60000, y: 55000 }, commodity: "alloys", units: 30, source_fleet: "F9" }, { x: 60000, y: 55000 }, { issuer: "market" }),
        op("O4", { kind: "survey_expedition", system: "1" }, { x: 60000, y: -20000 }, { issuer: "survey_office" }),
        op("O5", { kind: "pirate_bounty", system: "5", tier: 1 }, { x: 95000, y: 60000 }, { state: "active", scope: { scope: "syndicate", syndicate: "S1" } }),
      ];
      const ghost = (id, kind, x, y, own, extra = {}) => ({ id, owner: own ? "me" : "rival", own, kind, pos: { x, y }, vel: { x: -1, y: 0 },
        age: own ? 0 : 40, jumped: false, route: null, count_class: "one", composition: [{ kind, count: 1 }], cargo: null, docked: null, passengers: {}, ...extra });
      state.ghosts = [ghost("F1", "raider", 74000, 14000, true), ghost("F2", "corvette", 106000, 2000, false),
        // An own Scout cruising in warp: its wake must trail past the hull at every zoom.
        ghost("F3", "scout", 90000, 10000, true, { vel: { x: 0.8, y: -0.6 }, drive: { cruising: "warp" } })];
      await renderer.init(document.getElementById("app"));
      renderer.setGalaxy(state.galaxy);
      const zoom = Number(params.get("zoom") ?? 4);
      const focus = { x: Number(params.get("x") ?? 90000), y: Number(params.get("y") ?? 10000) };
      const apply = () => { renderer.zoomByFactor(zoom / renderer.zoomFactor()); renderer.centerOnWorld(focus, 0); };
      let settling = 40;
      window.__r = renderer; window.__focus = focus;
      renderer.stateVersion++;
      const frame = () => { if (settling-- > 0) apply(); renderer.update(state); document.documentElement.dataset.zoom = renderer.zoomFactor().toFixed(2); requestAnimationFrame(frame); };
      requestAnimationFrame(frame);
      })();
    ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:markers-fixture", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const files = new Map(output.map(file => [`/${file.fileName}`, file.type === "chunk" ? file.code : file.source]));
  const publicRoot = resolve(clientRoot, "public");
  const types = { glb: "model/gltf-binary", svg: "image/svg+xml", webp: "image/webp", png: "image/png", jpg: "image/jpeg", json: "application/json" };
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Map markers fixture</title><style>html,body{margin:0;height:100%;background:#03060c;overflow:hidden}#app{position:fixed;inset:0}</style></head><body><div id="app"></div><script type="module" src="/fixture.js"></script></body></html>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (files.has(url.pathname)) { response.setHeader("Content-Type", "text/javascript"); response.end(files.get(url.pathname)); return; }
    if (!url.pathname.startsWith("/art/")) { response.writeHead(404).end(); return; }
    const path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
    if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
    const type = types[path.split(".").pop()];
    if (type) response.setHeader("Content-Type", type);
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  const port = Number(process.argv.find(arg => arg.startsWith("--port="))?.slice("--port=".length)) || 0;
  server.listen(port, "127.0.0.1", () => console.log(`Map markers fixture: http://127.0.0.1:${server.address().port}/`));
}
