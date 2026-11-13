// §captain-intro: assigning a captain from the guide opens a panel that
// introduces them. Lore for every portrait, the sim's specialties, the panel.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

const lore = deps["./core/derive/captain-lore"] = compile("core/derive/captain-lore.ts");
// Every portrait the protocol knows has a background and the sim's specialty.
const portraits = [...src("protocol.ts").match(/export type CaptainPortrait =([^;]+);/)[1].matchAll(/"([a-z_]+)"/g)].map((m) => m[1]);
assert.equal(portraits.length, 21);
assert.deepEqual(Object.keys(lore.CAPTAIN_LORE).sort(), [...portraits].sort(), "a background for every captain");
const sim = readFileSync(new URL("../../crates/sim/src/captain.rs", import.meta.url), "utf8");
const founding = sim.slice(sim.indexOf("pub fn founding(player: PlayerId"));
const focuses = [...founding.matchAll(/CaptainPortrait::(\w+),\s*CaptainAttribute::(\w+)/g)].slice(0, 21);
assert.equal(focuses.length, 21);
const snake = (name) => name.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
for (const [, portrait, focus] of focuses) {
  assert.equal(lore.CAPTAIN_LORE[snake(portrait)].focus, focus.toLowerCase(), `${portrait}'s specialty mirrors the sim`);
}
for (const [portrait, { background }] of Object.entries(lore.CAPTAIN_LORE)) {
  assert.ok(background.length > 40 && background.length < 160, `${portrait}: a line or two`);
  assert.doesNotMatch(background, /\b(he|she|his|her|hers|him)\b/i, `${portrait}: no pronouns guessed from a portrait`);
}
assert.equal(lore.attributeEffect("command", 1), "", "a first point does nothing yet");
assert.equal(lore.attributeEffect("command", 2), "+2% damage");
assert.equal(lore.attributeEffect("navigation", 9), "−10% jump spool", "capped as the sim caps it");
assert.equal(lore.attributeEffect("fieldcraft", 3), "−6% survey time");
assert.equal(lore.attributeEffect("logistics", 2), "−2% fuel use");

// The panel.
deps["./core/derive/captains"] = { captainTitle: (title) => ({ lieutenant: "Lieutenant" })[title] ?? title };
deps["./icons"] = { label: (key) => key[0].toUpperCase() + key.slice(1) };
deps["./core/derive/fleet"] = { shipKindLabel: (kind) => ({ raider: "Interceptor" })[kind] ?? kind };
const intro = compile("captain-intro.ts", { document: {} });
const captain = { id: 3, name: "Sera Okafor", portrait: "sera_okafor", assigned_fleet: null, stationed_system: "home",
  recovering_until: null, loss_fate: null, report: { id: 3, name: "Sera Okafor", portrait: "sera_okafor", level: 1, title: "lieutenant",
    portrait_age: "young", command_capacity: 4, xp: 0, next_level_xp: 100, unspent: 0,
    attributes: { command: 1, navigation: 2, fieldcraft: 1, logistics: 1 } } };
const html = intro.captainIntroHtml({ captain, fleet: "Interceptor" });
assert.match(html, /<img class="captain-intro__portrait" src="\/art\/captains\/sera_okafor_young\.png" alt="Portrait of Sera Okafor"/);
assert.match(html, /<h1 id="captain-intro-title"[^>]*>Sera Okafor<\/h1>/);
assert.match(html, /<p class="captain-intro__rank">Navigation specialist<\/p>/);
assert.match(html, /Plotted jump corridors for the Survey Office/);
assert.match(html, /<div class="is-focus"><dt>Navigation<\/dt><dd><b>2<\/b><small>−2% jump spool<\/small><\/dd><\/div>/);
assert.match(html, /<dt>Command<\/dt><dd><b>1<\/b><\/dd>/);
assert.match(html, /<p class="captain-intro__level">Level 1 · Lieutenant<\/p>/, "rank, not command points");
assert.doesNotMatch(html, /authority/i);
for (const shell of ["shell/deck/roster.ts", "shell/deck/fleet.ts", "shell/mobile/parity.ts"]) {
  assert.doesNotMatch(src(shell), /· authority \$\{/, `${shell}: no "authority" beside a captain`);
}
assert.match(html, /data-captain-act="close">Not now<\/button><button type="button" class="settlement-primary" data-captain-act="assign">Assign to Interceptor<\/button>/);

// Only a guide's single AssignCaptain is introduced; the order goes on Assign only.
let shown = null;
const pick = intro.introduceCaptainFor;
const state = { captains: [captain], ghosts: [{ id: "f1", kind: "raider" }] };
const fake = compile("captain-intro.ts", { document: { createElement() { return { setAttribute() {}, addEventListener() {}, querySelector() { return null; }, showModal() { shown = this.innerHTML; } }; }, body: { append() {} } } });
let sent = 0;
assert.equal(fake.introduceCaptainFor(state, [{ type: "AssignCaptain", captain_id: 3, fleet_id: "f1" }], () => sent++), true);
assert.match(shown, /Assign to Interceptor/);
assert.equal(sent, 0, "nothing is sent until Assign");
assert.equal(pick(state, [{ type: "AttackFleet", fleet_id: "f1", target_id: "p" }], () => sent++), false, "other orders keep their confirm");
assert.equal(pick(state, [{ type: "AssignCaptain", captain_id: 99, fleet_id: "f1" }], () => sent++), false, "an unknown captain falls back");

// Both shells open it from the guide's Assign captain.
for (const shell of ["shell/deck/founding-guide.ts", "shell/mobile/surfaces.ts"]) {
  assert.match(src(shell), /if \(commands && introduceCaptainFor\(this\.ctx\.state, commands, \(\) => \{\n\s*this\.ctx\.intent\.beginFleetCommand\(commands\);\n\s*this\.ctx\.intent\.confirmPendingIntent\(\);\n\s*\}\)\) return;/, shell);
}

console.log("Captain intro: a background and the sim's specialty for all 21 captains, attribute effects, the panel, and the guide's Assign in both shells passed.");
