import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { commandCenter } from "./command-center-fixture.mjs";

const state = { playerId: "1", research: { programmes: [] }, researchCatalog: [{ id: "mat_prefab_construction", name: "Prefab Construction" }] };
const api = {};
vm.runInNewContext(ts.transpileModule(readFileSync(new URL("../src/core/derive/market.ts", import.meta.url), "utf8"),
  { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText, {
  exports: api, require: id => ({ "../../state": { state }, "../../icons": { label: s => s },
    "./fleet": { constructionStock: sys => ({ available: new Map(sys.stockpile.map(s => [s.commodity, s.units])) }) } })[id] ?? {},
});
const body = { id: 1, kind: "terrestrial", population: .002, structures: { command_center: 1, warehouse: 1, habitat: 1 },
  resource_slots: 2, industrial_slots: 4, infrastructure_slots: 2, construction_time_mult: 1.2, structure_build_time_mult: 1.2 };
const report = { builds: [], stockpile: ["alloys", "machinery", "electronics", "polymers"].map(commodity => ({ commodity, units: 1000 })) };
const option = () => api.structOption(commandCenter, report, body, api.bodyPoolUsage(body, report));
assert.equal(api.bodyPoolUsage(body, report).infrastructure.used, 2, "Landing Command is outside the pool");
assert.equal(option().buildable, true, "upgrade is allowed with a full infrastructure pool");
assert.equal(option().o.label, "Colony Command Center");
assert.equal(option().o.costs[0].units, 40);
assert.equal(api.structureBuildSeconds(option().o, body), 360);
report.stockpile[0].units = 39;
assert.equal(option().buildable, false, "the cheap level I recipe cannot pay for level II");
report.stockpile[0].units = 1000;
report.builds.push({ key: "command_center", body_id: 1, queued: true });
assert.equal(option().targetTier, 3);
assert.equal(option().o.costs[0].units, 120);
assert.match(option().reason, /Prefab Construction/);
state.research.programmes = [{ id: "mat_prefab_construction", state: "active" }];
assert.equal(option().buildable, false, "active research is not completed research");
state.research.programmes[0].state = "completed";
assert.equal(option().buildable, true);
// A separate research update cannot advance local reported construction speed.
assert.equal(api.structureBuildSeconds(option().o, body), 1080);
body.structures.command_center = 2;
report.builds = [];
body.structure_build_time_mult = 1.2 * .9 * .75;
assert.equal(api.structureBuildSeconds(option().o, body), 729);
report.builds.push({ key: "command_center", body_id: 1, queued: true });
assert.equal(option().buildable, false, "a pending headquarters counts toward the cap");
assert.match(option().reason, /Maximum/);
body.structures.command_center = 3;
report.builds = [];
assert.equal(option().buildable, false);
body.structures.command_center = 0;
assert.equal(option().o.label, "Landing Command");
assert.equal(option().buildable, true, "rebuilding uses no pool slot");
body.population = .0009;
assert.match(option().reason, /1,000 residents/);
body.population = .002;
body.kind = "gas_giant";
assert.equal(option().buildable, false);
assert.equal(api.structureLevel(commandCenter, 1).research_prerequisite, undefined);
console.log("Command centers: exact level recipes, research/cap/population gates, no-slot accounting and report-only durations pass.");
