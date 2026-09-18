// Real UI helpers/controllers with received-report fixtures. No running galaxy.
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import sharp from "sharp";
import { encodeMessage } from "../src/wire.mjs";
import { decode } from "@msgpack/msgpack";

const state = { playerId: "1", systems: [], galaxy: { build_options: [] } };
const deps = { "../../state": { state } };
const plain = x => JSON.parse(JSON.stringify(x));
function compile(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone });
  return exports;
}
const icons = deps["../../icons"] = compile("icons.ts");
const market = deps["./market"] = deps["../../core/derive/market"] = compile("core/derive/market.ts");
const refining = deps["../core/derive/refining"] = compile("core/derive/refining.ts");
deps["../icons"] = icons;
deps["../core/derive/format"] = { fmtDur: n => `${Math.round(n)}s` };
deps["../refining"] = compile("shell/refining.ts");
deps["./planet-sites"] = compile("shell/deck/planet-sites.ts");
const { PlanetPanel, planetStructures } = compile("shell/deck/planet.ts");
const { productionPlan } = compile("core/derive/industry.ts");
deps["../industry"] = { handleIndustryAction: () => false };
deps["../exploration"] = { handleExplorationAction: () => false };
const { MobileParitySurfaces } = compile("shell/mobile/parity.ts");

const ferrite = { output: "alloys", rate: 1, inputs: [["metallic_ore",1.5],["fuel",.3]], byproducts: [] };
const cuprite = { output: "conductive_metals", rate: .75, inputs: [["cuprite_ore",1.5],["fuel",.2]], byproducts: [["alloys",.15]] };
const smelter = { key: "smelter", label: "Smelter", conversion: ferrite, refining_recipes: [ferrite,cuprite] };
state.galaxy.build_options = [smelter];
assert.equal(icons.label("metallic_ore"),"Ferrite Ore");
assert.equal(market.COMMODITIES.length,22);
assert.equal(market.assignedRecipe(smelter,null),ferrite);
assert.equal(market.assignedRecipe(smelter,"cuprite_ore"),cuprite);
assert.deepEqual(plain(market.recipeOutputs(cuprite)),[["conductive_metals",1],["alloys",.15]]);
assert.match(market.commodityPurpose("cuprite_ore",[smelter]),/Sell raw.*Conductive Metals.*Alloys/);

for (const name of ["ferrite_ore","cuprite_ore","titanium_ore","crystalline_ore","rare_metal_ore","conductive_metals","titanium"]) {
  for (const size of [64,128]) {
    const path = new URL(`../public/art/ui_icons/resource/ores-2026-09-16/${name}-${size}.png`,import.meta.url);
    assert.ok(existsSync(path));
    const image = sharp(readFileSync(path)), meta = await image.metadata();
    assert.equal(meta.width,size); assert.equal(meta.height,size);
    assert.equal(meta.hasAlpha,true); assert.equal((await image.stats()).isOpaque,false);
  }
  const markup = icons.commodityIcon(name === "ferrite_ore" ? "metallic_ore" : name);
  assert.match(markup,/srcset=.*128.png 2x/);
  assert.ok(markup.includes(`title="${icons.label(name === "ferrite_ore" ? "metallic_ore" : name)}"`));
}

const body = { id: 1, name: "Freya II", environment: "uninhabitable", size: "medium", structures: { smelter: 1 }, deposits: [], industrial_slots: 3, resource_slots: 1, infrastructure_slots: 2 };
const assignment = { body_id: 1, structure: "smelter", title: "Smelter", tier: 1,
  workers: 1, refining_ore: null, specialists: {}, suspended: null,
  throughput: 1, staffing: 1, skill: 1, food: 1, site: 1, outputs: [["alloys",1]] };
const report = { id: "home", owner: "1", assignments: [assignment], bodies: [body], builds: [],
  converters: [{body_id:1,structure:"smelter",status:"running",rated_output:1,site:1}], stockpile: [], workforce: {units:2,posted:1} };
const model = { system: {id:"home",name:"Freya"}, report, body, mine: true, art:"barren.png", delay:17,
  catalog:[smelter], descriptions:{smelter:"Refines ore."}, workforceStructures:new Set(["smelter"]),
  populationHtml:"",surveyHtml:"",developmentHtml:"",queueHtml:"" };
state.market = { staleness:12, prices:[ ["metallic_ore",8],["cuprite_ore",12],["alloys",28],["conductive_metals",36],["fuel",14] ]
  .map(([commodity,price])=>({commodity,price,available_buy:10000,available_sell:10000})) };
state.research = { programmes:[] };
assert.equal(refining.refiningEstimate(state,report,1,ferrite),null,"no capabilities means unknown, not current-research inference");
report.refining_sites = [{body_id:1,tier:1,built:true,recovery:1,work_rate:1}];
const estimate = refining.refiningEstimate(state,report,1,ferrite);
assert.equal(estimate.raw,market.marketAverageQuote(8,50,"sell")*50);
assert.equal(estimate.fuel,10);
assert.equal(estimate.refined,market.marketAverageQuote(28,33,"sell")*33-market.marketAverageQuote(14,10,"buy")*10);
assert.equal(estimate.seconds,50/1.5);
assert.equal(estimate.priceAge,12);
const multiYield = refining.refiningEstimate(state,report,1,cuprite);
assert.deepEqual(plain(multiYield.outputs),[["conductive_metals",33],["alloys",5]]);
assert.equal(multiYield.fuel,7,"whole Fuel purchase covers the entire batch");
state.research.programmes = [{id:"mat_ore_recovery",state:"completed"}];
assert.equal(refining.refiningEstimate(state,report,1,ferrite).refined,estimate.refined,"corporate news cannot upgrade an old local factory report");
report.refining_sites[0].recovery = 1.15;
assert.ok(refining.refiningEstimate(state,report,1,ferrite).refined>estimate.refined);
report.refining_sites[0].work_rate = 0;
assert.equal(refining.refiningEstimate(state,report,1,ferrite).seconds,null);
report.refining_sites[0].recovery = report.refining_sites[0].work_rate = 1;
assert.equal(refining.refiningEstimate(state,{...report,owner:"2"},1,ferrite),null);
const alloyPrice = state.market.prices.find(p=>p.commodity==="alloys");
alloyPrice.price = 1;
assert.ok(refining.refiningEstimate(state,report,1,ferrite).difference<0,"refining is not guaranteed profit");
alloyPrice.available_sell = 0;
assert.equal(refining.refiningEstimate(state,report,1,ferrite),null,"don't pretend a batch can clear unavailable depth");
alloyPrice.price=28; alloyPrice.available_sell=10000;
assert.equal(refining.refiningEstimate({...state,market:null},report,1,ferrite),null,"never use base prices while market news is absent");
model.economy = state;
const panel = new PlanetPanel(), commands = [];
const act = data => panel.handleAction({deckAct:data.action,...data},model,c => commands.push(plain(c)));
panel.focus("smelter");
assert.match(panel.render(model),/Raw versus refined estimate.*Estimate · 50 Ferrite Ore/s);
const prior = plain(planetStructures(model)[0].outputs);
act({action:"planet-ore",ore:"cuprite_ore"});
assert.equal(commands.length,0,"selecting a recipe is only a draft");
act({action:"planet-review"});
assert.match(panel.render(model),/Order travel ~17s/);
assert.equal(commands.length,0,"review does not send");
act({action:"planet-confirm"});
assert.equal(commands.length,1);
assert.equal(commands[0].refining_ore,"cuprite_ore");
assert.deepEqual(decode(encodeMessage(commands[0]).subarray(4)),commands[0],"binary command carries selected ore");
assert.deepEqual(plain(planetStructures(model)[0].outputs),prior,"pending recipe never changes reported output");
assert.match(panel.render(model),/awaiting report/);
assignment.refining_ore = "cuprite_ore";
assignment.outputs = [["conductive_metals",.75],["alloys",.1125]];
report.converters[0].rated_output = .75;
assert.doesNotMatch(panel.render(model),/awaiting report/);
assert.deepEqual(plain(planetStructures(model)[0].outputs),[["conductive_metals",45],["alloys",6.75]]);

state.systems = [report]; state.ghosts = [];
const plan = productionPlan(state,report,"conductive_metals",30);
assert.deepEqual(plain(plan.inputs.map(i => i.commodity)),["cuprite_ore","fuel"]);
assert.equal(plan.rated,45);
const alloys = productionPlan(state,report,"alloys",5);
assert.equal(alloys.rated,6.75,"secondary output is not a second fully staffed refinery");
assert.equal(alloys.inputs[0].needed,0,"existing secondary yield supplies this smaller plan");

const phoneCommands = [];
const mobile = new MobileParitySurfaces({state,send:c => phoneCommands.push(plain(c))},{refresh(){}},{});
const mobileAct = (action,extra={}) => mobile.handleClick({target:{closest:() => ({dataset:{mobileAct:action,system:"home",body:"1",...extra}})}});
assert.equal(mobileAct("refining-select",{ore:"metallic_ore"}),true);
assert.equal(phoneCommands.length,0);
mobileAct("refining-confirm");
assert.equal(phoneCommands[0].refining_ore,"metallic_ore");
assert.equal(assignment.refining_ore,"cuprite_ore","mobile confirmation is not optimistic state mutation");
console.log("Ore economy: recipes, secondary yields, delayed confirmation on both shells, planner, binary command and seven alpha icons pass.");
