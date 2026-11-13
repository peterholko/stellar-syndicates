// §charter-crates (Peter, Oct 31 2026): the charter's supplies come in
// crates released a chapter at a time; a released crate pops up instead of a
// guide card, step 22 secures the stores, and the trader's stock-up counts
// sealed crates.
// No game socket.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}, globals = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone, ...globals });
  return exports;
};

const cast = { family: "showboat", surname: "Marrow", nephew: "Jace Marrow", uncle: "Mordecai Marrow", brother: "Rook Marrow" };
const options = [
  { key: "orbital_defense", costs: [{ commodity: "alloys", units: 25 }, { commodity: "machinery", units: 10 }] },
  { key: "warehouse", costs: [{ commodity: "alloys", units: 30 }, { commodity: "machinery", units: 8 }] },
  { key: "vault", costs: [{ commodity: "alloys", units: 40 }, { commodity: "machinery", units: 10 }] },
  { key: "academy", costs: [{ commodity: "alloys", units: 25 }, { commodity: "electronics", units: 15 }] },
];
// Any released crate (Peter, Nov 4 2026: none comes after the raid now;
// Nov 5: none at step 10 or 13 either).
const released = { id: "vendetta", released: true, step: 27, goods: { alloys: 70, machinery: 18 } };
const reach = { id: "reach", released: false, step: 23, goods: { alloys: 60, machinery: 28 } };
const st = { playerId: "1", simTime: 100, commandCenter: { x: 0, y: 0 },
  galaxy: { c: 400, systems: [{ id: "home", name: "Alphard", pos: { x: 0, y: 0 } }], hyperlimit: 900, build_options: options },
  systems: [], ghosts: [], battles: [], pendingOrders: new Map(), storyTold: new Set(), captains: [], explorationSites: [],
  founding: { stage: "build_gun_platform", local_opening: true, pirates: cast, vendetta: null, stipends: [], survey_candidates: [],
    crates: [released, reach] } };

const protocol = load("protocol.ts");
const vendetta = load("core/derive/vendetta.ts");
const equipment = load("core/derive/equipment.ts", { "../../protocol": protocol });
const clock = { state: st, liveSimTime: () => st.simTime };
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol,
  "./geo": load("core/derive/geo.ts"), "./equipment": equipment });
const icons = load("icons.ts");
let stock = 0;
const merchants = load("core/derive/merchant.ts", { "./industry": { availableStock: () => stock } });
const market = { bodyPoolUsage: b => ({ infrastructure: { used: 0, total: b.free ?? 0 } }), MINERAL_DEPOSITS: [] };
const guide = load("core/derive/founding.ts", { "../../protocol": protocol, "./fleet": fleets, "./market": market,
  "./format": { fmtDur: n => `${Math.round(n)}s` }, "../../icons": icons, "./merchant": merchants, "./vendetta": vendetta });
const home = { id: "home", owner: "1", builds: [], assignments: [], storage_used: 10, storage_cap: 1000,
  bodies: [{ id: 0, name: "Alphard I", kind: "terrestrial", structures: { command_center: 1 }, free: 1 },
    { id: 2, name: "Alphard III", kind: "terrestrial", structures: { mining_complex: 1 }, free: 2 }] };
st.systems = [home];

// §crate-panel (Peter, Nov 3 2026): a released crate is no step: it pops up
// to be accepted (test-story-events). The guide keeps to the step's card.
let plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.title, "Build a Laser Battery");
assert.ok(![plan.primary, ...plan.waiting].some(card => /crate/i.test(`${card.title} ${card.action}`)), "no crate card");
for (const file of ["core/derive/founding.ts", "shell/deck/founding-guide.ts", "shell/mobile/surfaces.ts"]) {
  assert.doesNotMatch(source(file), /collect-crate|CollectCharterCrate/, `${file}: the panel collects crates, not the guide`);
}

// A sealed crate ahead shows nothing either.
st.founding.crates = [reach];
plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.title, "Build a Laser Battery");

// Step 22: a Warehouse (on a world with a free site, the landing world last),
// then a Vault under the Command Center.
st.founding = { ...st.founding, stage: "secure_stores", crates: [], vendetta: { depart_at: null, fleet: null, arrived: false, won: null } };
plan = guide.foundingPlan(st, home);
assert.equal(plan.chapter.name, "Vendetta");
assert.deepEqual(JSON.parse(JSON.stringify(plan.checklist)),
  [{ title: "Secure your stores", done: false }, { title: "Defeat Mordecai Marrow", done: false }]);
assert.equal(plan.primary.title, "Build a Warehouse");
assert.equal(plan.primary.action, "build-store-warehouse");
assert.equal(plan.primary.why, "+700 storage.");
assert.equal(guide.foundingBuildTarget("build-store-warehouse", st, home).body.id, 2, "keeps the landing world's site");
assert.equal(guide.bestStoreBody({ bodies: [home.bodies[0]] }).id, 0, "the landing world when it is the only one");
home.bodies[1].structures.warehouse = 1;
plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.title, "Build a Vault");
assert.equal(plan.primary.action, "build-vault");
assert.equal(guide.foundingBuildTarget("build-vault", st, home).body.id, 0, "under the Command Center");
assert.ok(plan.waiting.some(card => card.action === "build-interceptor"), "the uncle's oath still stands");

// A build counts only a crate already released (Peter, Nov 5 2026: no
// Pirates crate, so the first Interceptor buys its Alloys from the trader).
const trader = { goods: [{ commodity: "alloys", stock: 120 }, { commodity: "electronics", stock: 127 }] };
st.founding.crates = [];
const plain = value => JSON.parse(JSON.stringify(value));
assert.deepEqual(plain(merchants.merchantShortfall(st, home, "academy", trader)), { alloys: 25, electronics: 15 });
st.founding.crates = [reach];
assert.deepEqual(plain(merchants.merchantShortfall(st, home, "academy", trader)), { alloys: 25, electronics: 15 },
  "a crate still sealed is no help to a build now");
st.founding.crates = [{ ...reach, released: true }];
assert.deepEqual(plain(merchants.merchantShortfall(st, home, "academy", trader)), { electronics: 15 },
  "no Alloys: a released crate brings them");

console.log("charter crates ok");
