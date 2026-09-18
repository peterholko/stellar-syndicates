import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const exports = {};
vm.runInNewContext(ts.transpileModule(source("shell/mission.ts"), { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
} }).outputText, { exports, require: () => ({}) });
const plain = value => JSON.parse(JSON.stringify(value));
const g = { id: "E101", own: true, mission_profile: { priority: "balanced", screening: "automatic", withdrawal: "never" } };
const command = exports.missionCommand(g, "priority", "missile_ships");
assert.deepEqual(plain(command), { type: "SetFleetMission", fleet_id: "E101", mission: {
  priority: "missile_ships", screening: "automatic", withdrawal: "never",
} });
assert.equal(g.mission_profile.priority, "balanced", "a button builds an intent, never applies the setting");
assert.equal(exports.missionCommand({ ...g, own: false }, "priority", "missile_ships"), null);
assert.equal(exports.missionCommand(g, "priority", "invalid"), null);
assert.equal(exports.missionCommand(g, "__proto__", "transports"), null);
assert.equal(exports.missionCommand(g, "withdrawal", "hull50").mission.withdrawal, "hull50");

const pending = [{ configuration: { kind: "mission", mission: command.mission } }];
const waiting = exports.missionHtml(g, pending);
assert.match(waiting, /signal in flight/);
assert.match(waiting, /data-value="missile_ships" aria-pressed="true" disabled/);
assert.doesNotMatch(exports.missionHtml(g, [{ ...pending[0], lost: true }]), /signal in flight/);
assert.match(exports.missionHtml(g, [], true), /data-mobile-act="fleet-mission"/);
assert.match(exports.missionHtml(g, []), /data-deck-act="fleet-mission"/);
assert.equal(exports.missionHtml({ ...g, own: false }, []), "");
for (const faction of ["ashwake", "ironclad", "rift"]) assert.ok(exports.pirateFactionHtml({ pirate_faction: faction }).length > 30);
assert.equal(exports.pirateFactionHtml({}), "", "no faction is inferred from unseen truth");
for (const path of ["shell/deck/fleet.ts", "shell/mobile/surfaces.ts"]) {
  assert.match(source(path), /missionCommand\(fleet, button.dataset.field, button.dataset.value\)/);
  assert.match(source(path), /beginFleetCommand\((?:order|command)\)/);
}
for (const path of ["shell/deck/strategic.ts", "shell/mobile/parity.ts"]) {
  assert.match(source(path), /"pirate_bounty", "combat_objective"/);
  assert.match(source(path), /type: "MoveShip"/);
}
console.log("Combat preparation: profile drafts, validation, pending state, both confirmation handlers, faction disclosure and objective dispatch passed.");
