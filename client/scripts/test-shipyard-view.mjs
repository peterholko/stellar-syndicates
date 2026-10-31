// §shipyard-view: a hull under construction in its yard's interior. Scene data,
// the yard's job line, the Deck pop-out and the wiring in both shells.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {}, ...globals });
  return exports;
}
const plain = (value) => JSON.parse(JSON.stringify(value));

// Scene data: every yard hull stands in the yard the sim builds it in, on a plate that exists.
const scene = deps["../../shipyard-scene"] = deps["../../shipyard-scene"] = compile("shipyard-scene.ts");
assert.ok(scene.hullFill("tiny_freighter", "shipyard") < scene.hullFill("combat_freighter", "shipyard")
  && scene.hullFill("combat_freighter", "shipyard") < scene.hullFill("small_freighter", "shipyard"),
  "Combat Freighter sits between Tiny and Small Freighters in the yard");
deps["./equipment"] = compile("core/derive/equipment.ts");
const market = compile("core/derive/market.ts");
for (const [kind, { yard }] of Object.entries(market.SHIP_YARD)) {
  if (kind === "transport") continue;
  assert.equal(scene.HULL_YARD[kind], yard, `${kind} stands in its ${yard}`);
}
for (const yard of ["shipyard", "naval_drydock", "capital_slipway"]) {
  const { src: url, srcset } = scene.plateUrls(yard);
  for (const width of [768, 1280, 1672]) {
    assert.ok(srcset.includes(`-${width}.webp ${width}w`), `${yard} srcset ${width}`);
    assert.ok(existsSync(new URL(`../public${url.replace("-1280.webp", `-${width}.webp`)}`, import.meta.url)), `${yard} plate ${width} is derived`);
  }
  const s = scene.YARD_SCENES[yard];
  const hulls = Object.entries(scene.HULL_YARD).filter(([, y]) => y === yard).map(([kind]) => kind);
  const fills = hulls.map((kind) => scene.hullFill(kind, yard));
  assert.ok(Math.abs(Math.max(...fills) - s.fill) < 1e-9, `${yard}: its longest hull spans the yard's fill`);
  assert.ok(fills.every((fill) => fill > 0.15 && fill <= s.maxW), `${yard}: every hull reads and fits the clear volume`);
  assert.ok(s.cx > 0.3 && s.cx < 0.7 && s.cy > 0.3 && s.cy < 0.8, `${yard}: the hull stands in the plate's clear volume`);
}
assert.ok(scene.hullFill("survey_probe", "shipyard") < scene.hullFill("raider", "shipyard"), "a probe is smaller than an Interceptor");
assert.ok(scene.hullFill("raider", "shipyard") < scene.hullFill("convoy", "shipyard"));
assert.equal(scene.YARD_SCENES.shipyard.floor, true, "the planetary yard has a deck; the orbital ones are vacuum");
assert.equal(scene.YARD_SCENES.naval_drydock.floor, false);
assert.equal(scene.isYardKind("garrison"), false);

// The yard's line: building hulls first, the served order kept, progress from the served clock.
deps["./construction"] = deps["../../core/derive/construction"] = compile("core/derive/construction.ts");
deps["./fleet"] = { shipKindLabel: (kind) => ({ raider: "Interceptor", tiny_freighter: "Tiny Freighter", destroyer: "Destroyer" })[kind] ?? kind };
deps["./market"] = market;
deps["./format"] = { fmtBuildDur: (seconds) => `${Math.round(seconds)}s` };
deps["./station"] ??= compile("core/derive/station.ts");
deps["./worldbuild"] = compile("core/derive/worldbuild.ts");
const yard = deps["../../core/derive/shipyard"] = compile("core/derive/shipyard.ts");
const system = { id: "home", bodies: [{ id: 12, name: "Freya 12" }, { id: 13, name: "Freya 13" }], builds: [
  { id: 5, key: "habitat", body_id: 12, queued: false, start_time: 0, complete_time: 50 },
  { id: 6, key: "tiny_freighter", body_id: 12, yard: "shipyard", queued: true, start_time: null, complete_time: null },
  { id: 7, key: "raider", body_id: 12, yard: "shipyard", queued: false, start_time: 60, complete_time: 160, work: { fraction: 0.3, per_second: 0.01, at_time: 90 } },
  { id: 8, key: "destroyer", body_id: 13, yard: "naval_drydock", queued: false, start_time: 0, complete_time: 200 },
] };
const jobs = yard.yardJobs(system, 100);
assert.deepEqual(plain(jobs.map((job) => [job.key, job.kind, job.yard, job.active])),
  [["job:7", "raider", "shipyard", true], ["job:8", "destroyer", "naval_drydock", true], ["job:6", "tiny_freighter", "shipyard", false]],
  "hulls only, building ones first");
assert.ok(Math.abs(jobs[0].progress - 0.4) < 1e-9, "the earned-work segment: 0.3 + 10 s × 1%");
assert.ok(Math.abs(jobs[1].progress - 0.5) < 1e-9, "a fixed span: 100 of 200 s");
assert.equal(jobs[2].progress, null, "waiting for a slip: no progress");
assert.equal(jobs[2].status, "waiting for a slip");
assert.equal(jobs[0].where, "Shipyard · Freya 12");
assert.equal(jobs[1].where, "Orbital Shipyard · Freya 13");
assert.equal(yard.pickYardJob(jobs, "job:8").key, "job:8", "the one picked");
assert.equal(yard.pickYardJob(jobs, null, "tiny_freighter").key, "job:6", "else the newest of the kind just ordered");
assert.equal(yard.pickYardJob(jobs, "gone", null).key, "job:7", "else the first building");
assert.equal(yard.pickYardJob([], null), null);
// With no job of the hull it opened on: seen building, then gone, it launched;
// else an order is Sending until due (two light legs), then maybe refused.
assert.deepEqual(plain(yard.yardFallback(true, null, 0, 100)), { state: "launched" });
assert.equal(yard.yardFallback(false, null, 0, 100), null, "opened from a hull line, never seen: nothing to say");
assert.deepEqual(plain(yard.yardFallback(false, 100, 10, 130)), { state: "sending" }, "due at 123, Sending to 138");
assert.deepEqual(plain(yard.yardFallback(false, 100, 10, 139)), { state: "silent" });
assert.equal(yard.yardFallback(false, 100, 10, 724), null, "forgotten past 600 s");

// The Deck pop-out.
deps["../../core/derive/fleet"] = deps["./fleet"];
deps["../../core/derive/market"] = market;
deps["../../core/derive/worldbuild"] = deps["./worldbuild"];
deps["../../core/derive/geo"] = { commandDelayTo: () => 0 };
deps["../../state"] = { liveSimTime: () => 100 };
deps["../dom"] = { renderDeferred: () => false, setHtml: (root, html) => { root.innerHTML = html; } };
deps["../signature"] = { sheetFingerprint: (value) => JSON.stringify(value) };
deps["../../protocol"] = compile("protocol.ts");
deps["./format"] = { ...deps["./format"], fmt: (n) => Math.round(n).toLocaleString() };
deps["../../core/derive/orders"] = compile("core/derive/orders.ts");
const { DeckShipyardRoutes } = compile("shell/deck/shipyard.ts");
const state = { systems: [system] };
const root = { id: "deck-workspace-body", innerHTML: "" };
const went = [];
const panel = new DeckShipyardRoutes(root, { state }, { go: (route) => went.push(route) });
const route = { name: "shipyard", params: { id: "home", systemLabel: "Freya" }, query: { kind: "raider" } };
assert.equal(panel.render({ name: "market" }), false);
panel.render(route, true);
assert.match(root.innerHTML, /<ship-construction-view id="shipyard-view" kind="raider" yard="shipyard" progress="0\.4000"><\/ship-construction-view>/);
assert.match(root.innerHTML, /<b>Interceptor<\/b><small>Shipyard · Freya 12<\/small>/);
assert.match(root.innerHTML, /role="progressbar"[^>]*aria-valuenow="40"/);
assert.match(root.innerHTML, /<nav class="deck-shipyard__tabs"[\s\S]*data-job="job:8"[\s\S]*<small>50%<\/small>[\s\S]*<small>waiting for a slip<\/small>/, "every hull a tab");
// §hull stats (Peter, Oct 27 2026): the hull's figures under the bay and its line.
assert.match(root.innerHTML, /<\/em><\/div><div class="deck-hull-stats"><span class="deck-stat"><small>Speed<\/small><b>100<\/b><\/span><span class="deck-stat"><small>Hull<\/small><b>200<\/b><\/span><span class="deck-stat"><small>Attack<\/small><b>3<\/b><\/span><span class="deck-stat"><small>Defense<\/small><b>2<\/b><\/span><\/div><div class="deck-shipyard__dock">/);
assert.doesNotMatch(root.innerHTML, /Cargo/, "an Interceptor carries none");
assert.equal(panel.handleAction({ dataset: { deckAct: "shipyard-pick", job: "job:8" } }, route), true);
assert.match(root.innerHTML, /kind="destroyer" yard="naval_drydock" progress="0\.5000"/, "a tab puts its hull in the bay");
assert.match(root.innerHTML, /<small>Attack<\/small><b>2\.4<\/b>/, "a fractional weight keeps its decimal");
panel.handleAction({ dataset: { deckAct: "shipyard-pick", job: "job:6" } }, route);
assert.match(root.innerHTML, /kind="tiny_freighter" yard="shipyard"><\/ship-construction-view>/, "waiting: all blueprint, no progress");
assert.match(root.innerHTML, /<small>Hull<\/small><b>1,500<\/b>[\s\S]*<small>Cargo<\/small><b>50<\/b>/, "a hauler shows its cargo");
assert.equal(panel.handleAction({ dataset: { deckAct: "shipyard-build" } }, route), true);
assert.deepEqual(plain(went.at(-1)), { name: "build", params: { systemId: "home", systemLabel: "Freya" }, query: { mode: "ships" } });
// Just ordered, before its report: the hull shows as sent.
const empty = { state: { systems: [{ ...system, builds: [] }] } };
const fresh = new DeckShipyardRoutes(root, empty, { go() {} });
fresh.render({ ...route, query: { kind: "corvette", sent: "100" } }, true);
assert.match(root.innerHTML, /kind="corvette" yard="shipyard"><\/ship-construction-view>[\s\S]*<small>Shipyard<\/small>[\s\S]*<em>Sending<\/em>/);
fresh.render({ ...route, query: { kind: "corvette", sent: "50" } }, true);
assert.match(root.innerHTML, /No report yet · it may have been refused[\s\S]*<em><\/em>/, "overdue: maybe refused, not Sending forever");
// §holds: the Combat Freighter's hold is loot, and says so.
fresh.render({ ...route, query: { kind: "combat_freighter", sent: "100" } }, true);
assert.match(root.innerHTML, /<small>Speed<\/small><b>70<\/b>[\s\S]*<small>Defense<\/small><b>2<\/b>[\s\S]*<small>Loot<\/small><b>80<\/b>/);
fresh.render({ ...route, query: { kind: "corvette" } }, true);
assert.match(root.innerHTML, /No hulls in the yard\./, "a hull line's kind alone is no order");
fresh.render({ ...route, query: {} }, true);
assert.match(root.innerHTML, /No hulls in the yard\./);
// Seen building, then gone: launched, the whole hull, never Sending again.
const live = { state: { systems: [system] } };
const watcher = new DeckShipyardRoutes(root, live, { go() {} });
const ordered = { ...route, query: { kind: "raider", sent: "40" } };
watcher.render(ordered, true);
assert.match(root.innerHTML, /kind="raider" yard="shipyard" progress="0\.4000"/);
live.state.systems = [{ ...system, builds: system.builds.filter((job) => job.key !== "raider") }];
watcher.render(ordered, true);
assert.match(root.innerHTML, /kind="raider" yard="shipyard" progress="1\.0000"><\/ship-construction-view>[\s\S]*<b>Interceptor<\/b><small>Shipyard<\/small>[\s\S]*<em>Launched<\/em>/);
assert.doesNotMatch(root.innerHTML, /Sending|role="progressbar"/);

// Wiring: queueing opens it in both shells; hull lines open it; the plates are derived.
const empire = src("shell/deck/empire.ts");
assert.match(empire, /this\.closeBuildWorkbench\(route\);\n\s*this\.hooks\.go\(\{ name: "shipyard", params: \{ id: dynamic\.id, systemLabel: system\.name \}, query: \{ kind, sent: String\(liveSimTime\(\)\) \} \}\);/);
assert.match(empire, /<small>Hulls in the yard<\/small>\$\{hulls\.map\(\(job\) =>\n\s*`<button type="button" data-deck-act="open-shipyard"/);
assert.match(src("shell/mobile/parity.ts"), /this\.hooks\.openSheet\(\{ id: "yard", props: \{ systemId: system, kind: this\.selectedHull, sent: liveSimTime\(\) \} \}\);/);
assert.match(src("shell/deck/workspace.ts"), /"signals", "shipyard"[,\]]/, "a pop-out over the map");
assert.match(empire, /<ship-model-preview kind="\$\{hull\}"><\/ship-model-preview>\$\{hullStatsHtml\(hull\)\}<div class="deck-quantity">/, "the builder's figures sit under its model");
assert.match(src("shell/mobile/parity.ts"), /<ship-model-preview kind="\$\{hull\}"><\/ship-model-preview>\$\{hullStatsHtml\(hull\)\}\$\{costs\}/);
assert.match(src("shell/mobile/parity.ts"), /\$\{view\}\$\{info\}\$\{bar\}\$\{hullStatsHtml\(hull\)\}\$\{build\}/, "and under the mobile yard's bay");
assert.match(readFileSync(new URL("./build-art-derivatives.mjs", import.meta.url), "utf8"), /buildShipyardInteriorArt\(\)/);
assert.match(src("ship-construction-runtime.ts"), /localClippingEnabled = true/, "the cut is a clipping plane");

console.log("Shipyard view: scene data and plates, hull fills, the yard's line, the Deck pop-out (tabs, sending, refused, launched, empty) and the wiring in both shells passed.");
