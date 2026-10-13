// Blockade stages as players read them (core/derive/blockade.ts): one source
// for the map, the System View, the panels, the inbox and mobile.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = file => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
const load = (file, deps = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(file), { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText, { exports, require: name => deps[name] ?? {}, Intl, Date });
  return exports;
};
const fmtDur = secs => `${Math.round(secs)}s`;
const b = load("core/derive/blockade.ts", { "./format": { fmtDur } });
let tests = 0;
const test = (name, fn) => { fn(); tests++; console.log(`PASS ${name}`); };

const blockade = over => ({ by: "9", by_name: "Vesper Combine", since: 100, by_me: false, held_since: null,
  siege_since: null, siege_secs: 600, capturable: true, ...over });

test("stages follow the served clocks", () => {
  assert.equal(b.blockadeStage(blockade()), "contested");
  assert.equal(b.blockadeStage(blockade({ held_since: 200 })), "held");
  assert.equal(b.blockadeStage(blockade({ held_since: 200, siege_since: 200 })), "siege");
});

test("the capture clock is the system's own, not the galaxy base", () => {
  const clock = b.siegeClock(blockade({ held_since: 200, siege_since: 200, siege_secs: 800 }), 600, 360);
  assert.equal(clock.pct, 50);
  assert.equal(clock.left, 400);
  assert.equal(clock.eta, 1000);
  assert.equal(clock.ripe, false);
  assert.equal(b.siegeClock(blockade({ held_since: 200, siege_since: 200, siege_secs: null }), 560, 360).ripe, true,
    "falls back to the base when the loop hasn't filled it");
  assert.equal(b.siegeClock(blockade({ held_since: 200 }), 900, 360), null, "no clock while only held");
});

test("badges read the same on every surface", () => {
  assert.equal(b.blockadeBadge(blockade(), 300, 600), "CONTESTED");
  assert.equal(b.blockadeBadge(blockade({ by_me: true }), 300, 600), "CONTESTING");
  assert.equal(b.blockadeBadge(blockade({ held_since: 200 }), 300, 600), "BLOCKADED");
  assert.equal(b.blockadeBadge(blockade({ held_since: 200, by_me: true }), 300, 600), "BLOCKADING");
  assert.equal(b.blockadeBadge(blockade({ held_since: 200, siege_since: 200 }), 350, 600), "SIEGE 25%");
});

test("each side is told what the stage means and what to do", () => {
  const owner = b.blockadeBrief(blockade({ held_since: 200 }), 300, 600, 12);
  assert.match(owner.title, /Blockaded by Vesper Combine/);
  assert.match(owner.detail, /cut off/);
  assert.match(owner.detail, /raise the system's station a level/);
  const contested = b.blockadeBrief(blockade(), 300, 600);
  assert.equal(contested.tone, "warn");
  assert.match(contested.detail, /Shipping runs/);
  const siege = b.blockadeBrief(blockade({ held_since: 200, siege_since: 200 }), 500, 600);
  assert.match(siege.detail, /Falls to a marine landing in 300s/);
  const besieger = b.blockadeBrief(blockade({ held_since: 200, siege_since: 200, by_me: true }), 900, 600, 14);
  assert.equal(besieger.title, "Siege ripe");
  assert.match(besieger.detail, /about 14 marines/);
  const home = b.blockadeBrief(blockade({ held_since: 200, by_me: true, capturable: false }), 300, 600);
  assert.match(home.detail, /never captured/);
  assert.match(b.blockadeBrief(blockade({ by_me: true }), 300, 600).detail, /if your fleet retreats, the blockade is off/);
});

test("orders are refused before they are sent, with the reason", () => {
  const fleet = (kinds, own = true) => ({ own, composition: kinds.map(kind => ({ kind, count: 2 })), pos: { x: 0, y: 0 } });
  const war = { relations: [{ other: "9", name: "Vesper", state: "war", separation_until: 0 }], incoming: [] };
  assert.equal(b.blockadeOrderBlock(fleet(["raider", "cruiser"]), "9", "1", war, 50), "");
  assert.match(b.blockadeOrderBlock(fleet(["cruiser"]), "9", "1", war, 50), /needs an interceptor/);
  assert.match(b.blockadeOrderBlock(fleet(["raider"]), "1", "1", war, 50), /another corporation owns/);
  const peace = { relations: [{ other: "9", name: "Vesper", state: "neutral", separation_until: 0 }], incoming: [] };
  assert.match(b.blockadeOrderBlock(fleet(["raider"]), "9", "1", peace, 50), /declared war or your right of reprisal/);
  const reprisal = { relations: [{ ...peace.relations[0], reprisal_until: 80 }], incoming: [] };
  assert.equal(b.blockadeOrderBlock(fleet(["raider"]), "9", "1", reprisal, 50), "");
  const pact = { relations: [{ other: "9", name: "Vesper", state: "ceasefire", separation_until: 0 }], incoming: [] };
  assert.match(b.blockadeOrderBlock(fleet(["raider"]), "9", "1", pact, 50), /ceasefire with Vesper/);
});

test("a blockading fleet says where and at what stage", () => {
  const view = { simTime: 400, galaxy: { siege_secs: 600, systems: [{ id: "s1", name: "Vidar", pos: { x: 1000, y: 0 } }] },
    systems: [{ id: "s1", blockade: blockade({ by_me: true, held_since: 200, siege_since: 250 }) }] };
  const g = { own: true, pos: { x: 1100, y: 0 }, composition: [] };
  assert.equal(b.blockadeStatus(g, view), "Blockading Vidar · siege 25%");
  assert.equal(b.blockadeStatus({ ...g, pos: { x: 9000, y: 0 } }, view), "Sailing to blockade");
});

test("a held blockade says how often it catches a runner", () => {
  assert.equal(b.blockadeOdds(blockade({ catch: 0.6, catch_stabilized: 0.5 })), "", "nothing to run while contested");
  const held = blockade({ held_since: 200, catch: 0.657, catch_stabilized: 0.51 });
  assert.equal(b.blockadeOdds(held), "caught 66% · 51% with a Warp Stabilizer");
  assert.equal(b.blockadeOdds(blockade({ held_since: 200, catch: 0.03, catch_stabilized: 0.03 })), "caught 3%",
    "a capital line: a stabilizer has no interceptor to shrug off");
  assert.match(b.blockadeBrief(held, 300, 600).detail, /set to run blockades tries to slip through: caught 66%/);
  assert.match(b.blockadeBrief({ ...held, by_me: true }, 300, 600).detail, /Freighters running it: caught 66%/);
});

test("a waiting delivery says where, and what running costs", () => {
  const view = { galaxy: { systems: [{ id: "s1", name: "Vidar", pos: { x: 0, y: 0 } }] },
    systems: [{ id: "s1", blockade: blockade({ held_since: 200, catch: 0.3, catch_stabilized: 0 }) }] };
  assert.equal(b.heldOutsideStatus({ own: true, held_outside: "s1" }, view),
    "Waiting outside the blockade of Vidar · running it: caught 30% · 0% with a Warp Stabilizer");
  assert.equal(b.heldOutsideStatus({ own: true }, view), "");
  assert.match(b.runBlockadesCopy(false).hint, /Warp Stabilizer and Blockade Runners research help/);
  assert.equal(b.runBlockadesCopy(true).label, "Running held blockades");
});

console.log(`${tests} blockade tests passed`);
