// §signals: the Signals pop-out's derivations, from served-view fixtures. It
// never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {
  "../../icons": { label: (slug) => ({ alloys: "Alloys", machinery: "Machinery", point_defense_screen: "Point Defense Screen",
    geologist: "Geologist", recon_suite: "Recon Suite" })[slug] ?? slug },
  "./fleet": { fleetBaseSpeed: () => 100, transitSpeedMultiplier: () => 1, WARP_FACTOR: 5,
    shipKindLabel: (kind) => ({ raider: "Interceptor", corvette: "Corvette" })[kind] ?? kind },
};
function compile(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {} });
  return exports;
}
const copy = (value) => JSON.parse(JSON.stringify(value));
const trader = deps["./trader"] = compile("core/derive/trader.ts");
const signals = compile("core/derive/signals.ts");

// Mirrors sim::signals::hurt_chance.
assert.equal(Math.round(signals.hurtChance(1) * 100), 30);
assert.equal(Math.round(signals.hurtChance(2) * 100), 15);
assert.equal(signals.hurtChance(4), 0.05, "a floor");
assert.equal(signals.signalDur(42), "0:42");
assert.equal(signals.signalDur(3 * 3_600 + 12 * 60), "3h 12m");
assert.equal(signals.signalDur(42 * 60), "42m");

const art = signals.signalCardArt("ghost_ship");
assert.equal(art.src, "/art/derived/signals/v1/ghost-ship-card-384.webp");
assert.match(signals.signalFrameArt("teos_lost_crate").srcset, /teos-lost-crate-1280\.webp 1280w/);
assert.equal(Object.keys(signals.SIGNAL_INFO).length, 13, "one entry per signal type");
// §traders: the lost crate is the founder's own trader's, as the server names it.
const crate = (slug) => copy(signals.signalInfo("teos_lost_crate", trader.TRADERS[slug]));
assert.deepEqual([crate("teo").name, crate("ines").name, crate("mags").name, crate("rafi").name],
  ["Teo's lost crate", "Ines's lost crate", "Mags's lost crate", "Rafi's lost crate"]);
assert.equal(crate("mags").blurb, "A crate stencilled “Second Wind”.");
assert.deepEqual(crate("ines").mayHold, ["Ines's goods", "A finder's fee"]);
assert.equal(crate("ines").slug, "teos-lost-crate", "one painting for all three");
assert.deepEqual(copy(signals.signalInfo("cargo_pod", trader.TRADERS.ines)), copy(signals.SIGNAL_INFO.cargo_pod), "the rest keep theirs");

const now = 100_000;
const view = {
  next_at: now + 5 * 3_600 + 40 * 60, open_max: 5, first_ten: 4, first_ten_next: "a research dossier",
  signals: [
    { id: "a", number: 12, kind: "ghost_ship", hazardous: true, pos: { x: 18_000, y: 0 }, detected_at: now - 3 * 3_600, xp: 90 },
    { id: "b", number: 13, kind: "cargo_pod", hazardous: false, pos: { x: -9_000, y: 0 }, detected_at: now - 3_700, xp: 45 },
    { id: "c", number: 14, kind: "distress_beacon", hazardous: false, pos: { x: 0, y: 10_000 }, detected_at: now - 600, xp: 45 },
    { id: "d", number: 11, kind: "drifting_wreck", hazardous: false, pos: { x: 0, y: -12_000 }, detected_at: now - 9_000, xp: 45,
      report: { at: now - 120, fleet: "f1", captain: "Mara Venn", xp: 45, hurt_until: now + 3_600,
        finds: [{ kind: "module_aboard", module: "point_defense_screen" }, { kind: "cache", site: "s", goods: [["alloys", 34], ["machinery", 8]] }] } },
  ],
};
const ghost = (id, kind, pos, extra = {}) => ({ id, kind, pos, own: true, vel: { x: 0, y: 0 }, composition: [{ kind, count: 1 }], ...extra });
const ghosts = [
  ghost("f1", "raider", { x: 0, y: 0 }, { docked: "home", captain: { name: "Mara Venn", level: 2, attributes: { fieldcraft: 1 } } }),
  ghost("f2", "corvette", { x: 0, y: 0 }, { docked: "home", captain: { name: "Ada Voss", level: 3, attributes: { fieldcraft: 1 } } }),
  ghost("f3", "raider", { x: 0, y: 5_000 }, { captain: { name: "Jun Arclight", level: 2, attributes: { fieldcraft: 2 } },
    expedition: { site: "c", task: "signal" } }),
];
const home = { x: 0, y: 0 };

const cards = signals.signalCards(view, ghosts, home, now);
assert.deepEqual(copy(cards.map((c) => [c.signal.id, c.status])), [["d", "reported"], ["c", "answering"], ["b", "open"], ["a", "open"]],
  "reports, then the one being answered, then the newest open");
assert.equal(cards[0].tag, "Report in");
assert.equal(cards[0].line, "Explored · 2m ago");
assert.equal(cards[1].line, "Interceptor · Jun Arclight · 0:10", "5,000 su at 500 su/s");
assert.equal(cards[2].line, "9,000 su · 1h ago");
assert.equal(cards[3].tag, "Hazardous");
assert.equal(signals.defaultSignal(cards), "d", "a fresh report first");
assert.equal(signals.signalsSummary(view, now), "3 open · next ~5h 40m");
assert.equal(signals.signalsSummary({ ...view, next_at: null }, now), "3 open · full");
assert.equal(signals.openSignals(view), 3);
assert.deepEqual(copy(signals.firstTenStrip(view)), { done: 4, next: "a research dossier" });
assert.equal(signals.firstTenStrip({ ...view, first_ten: 10 }), null, "gone once the ten are done");

const captains = [
  { id: 0, name: "Mara Venn", assigned_fleet: "f1", recovering_until: null, loss_fate: null, report: null },
  { id: 1, name: "Ada Voss", assigned_fleet: "f2", recovering_until: null, loss_fate: null, report: null, hurt_until: now + 3 * 3_600 + 12 * 60 },
  { id: 2, name: "Daichi Mori", assigned_fleet: null, stationed_system: "home", recovering_until: null, loss_fate: null,
    report: { level: 2, attributes: { fieldcraft: 2 } } },
  { id: 3, name: "Jun Arclight", assigned_fleet: "f3", recovering_until: null, loss_fate: null, report: null },
];
const rows = signals.sendRows(view.signals[0], captains, ghosts, home, now, (berth) => berth);
assert.deepEqual(copy(rows.map((r) => [r.title, r.blocked])), [
  ["Interceptor · Mara Venn", null],
  ["Corvette · Ada Voss", "Recovering · 3h 12m"],
  ["Interceptor · Jun Arclight", "On another signal"],
  ["No ship · Daichi Mori", "Give a ship in Officers"],
], "the ready ship first, the captain in reserve last");
assert.equal(rows[0].risk, "Injury 30%", "Fieldcraft 1 on a Hazardous signal");
assert.equal(rows[0].sub, "Lv 2 · Fieldcraft 1 · docked at home");
assert.equal(rows[0].trip, "~1:42 trip", "18k out and back at 500 su/s, plus the 30 s sweep");
const routine = signals.sendRows(view.signals[1], captains, ghosts, home, now, (berth) => berth);
assert.equal(routine[0].risk, null, "no injury risk on a Routine signal");
assert.equal(signals.sendRows(view.signals[2], captains, ghosts, home, now, (b) => b).find((r) => r.fleet?.id === "f3").blocked,
  "On its way");

const found = signals.findRows(view.signals[3], "Interceptor", now, (id) => id);
assert.deepEqual(copy(found), [
  { label: "Point Defense Screen crate", value: "aboard the Interceptor" },
  { label: "34 Alloys, 8 Machinery", value: "left for a Freighter" },
  { label: "Experience · Mara Venn", value: "+45", tone: "good" },
  { label: "Mara Venn hurt", value: "back in 1h 0m", tone: "warn" },
]);
assert.deepEqual(copy(signals.findRows({ ...view.signals[3], report: { ...view.signals[3].report, finds: [], hurt_until: undefined } }, null, now, (id) => id))
  .map((r) => r.label), ["Nothing of value", "Experience · Mara Venn"]);

console.log("signals: ok");
