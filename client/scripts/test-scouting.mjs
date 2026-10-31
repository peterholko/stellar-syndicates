// §scouting (Oct 28 2026): the scout report on a rival system, both shells
// (core/derive/scouting.ts), and its wiring.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (file) => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
const scouting = {};
vm.runInNewContext(ts.transpileModule(src("core/derive/scouting.ts"), { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText, { exports: scouting, require: () => ({}) });

const prices = { alloys: 26, metallic_ore: 8 };
const price = (c) => prices[c];
assert.equal(scouting.scoutReport({ intel: null, scouted_stores: null }, price), null, "no look, no report");
const report = scouting.scoutReport({
  intel: { defense_tier: 2, shipyard_tier: 1, observed_at: 100 },
  scouted_stores: { reach: [{ commodity: "alloys", units: 50 }, { commodity: "metallic_ore", units: 300 }], vault: 200, observed_at: 120 },
}, price);
assert.equal(report.defence, "Defence ×2 · Shipyard ×1");
assert.deepEqual(report.reach.map((s) => s.commodity), ["alloys", "metallic_ore"], "dearest first, as served");
assert.equal(report.value, 50 * 26 + 300 * 8, "valued at the arrived Hub quotes");
assert.equal(report.vault, 200);
assert.equal(report.at, 120, "ages from the newest look");
assert.equal(scouting.scoutReport({ scouted_stores: { reach: [{ commodity: "void_crystal", units: 2 }], vault: 0, observed_at: 5 } }, price).value,
  null, "no value without every quote");
assert.match(scouting.COUNTER_SCOUTING_HINT, /more Scouts than the system's counter-scouting/);

const deck = src("shell/deck/empire.ts"), mobile = src("shell/mobile/surfaces.ts");
assert.match(deck, /this\.scoutHtml\(dynamic\) \+ this\.groundActivityHtml\(dynamic\)/, "desktop: rival systems show the report");
assert.match(mobile, /mine \|\| !dynamic\?\.owner \? `<section class="m-section"><h3>Stockpile<\/h3>[^`]*` : this\.scoutHtml\(dynamic\)/,
  "mobile: a rival's stockpile row becomes the report");
assert.match(src("protocol.ts"), /scouted_stores\?: \{ reach: StockSlot\[\]; vault: number; observed_at: number \} \| null;/);
console.log("PASS: scout report (defences, stores in reach, Vault, value, age) and its wiring on both shells.");
