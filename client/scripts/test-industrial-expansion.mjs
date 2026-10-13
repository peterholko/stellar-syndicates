import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
const source = file => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
class Input { constructor(dataset,value) { this.dataset=dataset; this.value=value; this.checked=false; } }
function compile(file,deps={}) {
  const exports={};
  vm.runInNewContext(ts.transpileModule(source(file),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText,
    {exports,require:id=>deps[id]??{},structuredClone,HTMLInputElement:Input});
  return exports;
}
const plain = x => JSON.parse(JSON.stringify(x));
const fleets={mineCapable:g=>g.composition?g.composition.some(c=>c.kind==="mining_barge"&&c.count>0):g.kind==="mining_barge",
  fleetCargoCapacity:g=>g.kind==="convoy"?250:g.kind==="mining_belt_station"?3000:0,fleetFuelCapacity:()=>160,fleetBaseSpeed:()=>40,
  slowCapable:g=>g.kind==="convoy",transitSpeedMultiplier:g=>g.transit==="slow"?.5:g.transit==="stealth"?.5:1,
  transitLabel:(m="full")=>m==="slow"?"Slow":m==="stealth"?"Stealth":"Full speed",
  regimeStore:g=>({fuel:g.transit==="slow"?"uranium":"deuterium",have:g.fuel??0,capacity:160}),
  guardCapable:g=>g.own&&g.kind==="raider",shipKindLabel:k=>k==="convoy"?"Freighter":"Interceptor",WARP_FACTOR:5,
  prospectCapable:g=>(g.composition??[{kind:g.kind,count:1}]).some(c=>(c.kind==="scout"||c.kind==="mining_barge")&&c.count>0),PROSPECT_SECS:60,
  estimatedFuelForLeg:()=>3};
const market=compile("core/derive/market.ts");
const derive=compile("core/derive/industry.ts",{"./fleet":fleets,"./market":market});
const icons=compile("icons.ts");
const protocol=compile("protocol.ts");
const shell=compile("shell/industry.ts",{"../core/derive/fleet":fleets,"../core/derive/industry":derive,"../icons":icons,
  "../core/derive/geo":{foundingHomeSystemId:()=>"home"},"../planetart":{planetArtwork:()=>({}),planetArtUrl:k=>`/planet/${k}.png`},
  "../hub-model.generated":{HUB_MODEL:{hero:"/hub.webp"}},
  "../protocol":protocol,"../core/derive/market":{COMMODITIES:["metallic_ore","deuterium","alloys","biomass","uranium"]}});
const beltsDerive=compile("core/derive/belts.ts",{"../../icons":icons,"./fleet":fleets,"../../protocol":protocol});
const commands=compile("core/fleetorders.ts",{"./derive/industry":derive,"./derive/fleet":fleets,"../icons":icons,
  "./derive/format":{fmtDur:n=>`${Math.round(n)}s`},"./derive/belts":beltsDerive});
const body={id:0,name:"Freya II",habitable:true,deposits:[{resource:"biomass"}],structures:{shipyard:2}};
const line={structure:"smelter",workers:1,throughput:1,staffing:.5,skill:1,food:1,site:1,outputs:[["alloys",.1]],suspended:null};
const site={id:"home",owner:"1",stockpile:[{commodity:"metallic_ore",units:120}],bodies:[body],assignments:[line],workforce:{posted:2,units:1},industry:{reservations:[],outpost:null,projects:[]}};
const other={...structuredClone(site),id:"mine",assignments:[{...line,structure:"extractor",outputs:[["metallic_ore",.5]]}],stockpile:[{commodity:"metallic_ore",units:40}]};
const state={playerId:"1",founding:{expansion_unlocked:true},systems:[site,other],ghosts:[{id:"f",kind:"convoy",own:true,pos:{x:0,y:0},cargo_manifest:[],fuel:100},
  {id:"e",kind:"raider",own:true,pos:{x:0,y:0}}],research:{programmes:[]},
  galaxy:{c:400,hub:{x:10000,y:0},systems:[{id:"home",name:"Freya",pos:{x:0,y:0}},{id:"mine",name:"Ore colony",pos:{x:30000,y:0}}],
    build_options:[{key:"smelter",label:"Smelter",conversion:{output:"alloys",rate:.2,inputs:[["metallic_ore",2]]}}],
    industry_catalog:{projects:[{kind:"orbital_assembly",costs:[["alloys",50]],inputs:[["composites",.58]],workers:3,build_secs:180}],outpost_costs:[["alloys",30]],outpost_build_secs:60,skim_helium3_per_s:1/6,max_stops:8}}};
const planned=derive.productionPlan(state,site,"alloys",10);
assert.equal(planned.rated,12); assert.equal(planned.staffed,6); assert.ok(planned.workforceLimited);
assert.equal(planned.capacityLimited,false); assert.equal(planned.inputs[0].needed,20);
assert.equal(planned.inputs[0].stock,120); assert.equal(planned.inputs[0].runway,6);
assert.equal(planned.inputs[0].suppliers[0].name,"Ore colony");
assert.equal(planned.inputs[0].suppliers[0].surplus,30); assert.ok(planned.freightLimited);
const unstaffed = {...site, assignments:[], converters:[{structure:"smelter",rated_output:.2,site:1,status:"needs_crew"}]};
assert.equal(derive.productionPlan(state,unstaffed,"alloys",10).rated,12);
assert.equal(derive.productionPlan(state,unstaffed,"alloys",10).workforceLimited,true,"a built empty factory needs workers, not another factory");
site.industry.reservations=[{target:{kind:"academy"},goods:{metallic_ore:100}}];
assert.equal(derive.productionPlan(state,site,"alloys",10).inputs[0].runway,1,"reserved goods are not production runway");
site.industry.refining_reserve={metallic_ore:50};
assert.equal(derive.productionPlan(state,site,"alloys",10).inputs[0].stock,0,"Smelter floors also protect unreserved inputs");
site.industry.reservations=[];
assert.equal(derive.productionPlan(state,site,"alloys",10).inputs[0].runway,3.5,"reported reserve floors reduce the Smelter runway");
assert.equal(derive.availableStock(site,"metallic_ore"),120,"refining floors never block a Freighter's export");
site.industry.refining_reserve={};
site.assignments.push({...line,structure:"extractor",outputs:[["metallic_ore",.5]]});
assert.equal(derive.productionPlan(state,site,"alloys",10).inputs[0].local,30,"do not count the planned smelter's old input basket twice");
state.systems.push({...structuredClone(other),id:"rival",owner:"2",stockpile:[{commodity:"metallic_ore",units:9999}]});
assert.equal(derive.productionPlan(state,site,"alloys",10).inputs[0].suppliers.length,1,"no rival inventory sourcing");

const route={name:"Ore outward, Fuel return",repeat:true,fuel_reserve:10,escort:"e",stops:[
  {port:{kind:"system",id:"home"},load:{metallic_ore:40,biomass:20},unload:{deuterium:10},sell:false},
  {port:{kind:"hub"},load:{deuterium:10},unload:{metallic_ore:40,biomass:20},sell:true}]};
assert.equal(derive.freightRouteReason(state,state.ghosts[0],route),null);
let bad=structuredClone(route); bad.stops[0].load.metallic_ore=300;
assert.match(derive.freightRouteReason(state,state.ghosts[0],bad),/capacity/);
bad=structuredClone(route); bad.stops.push(structuredClone(bad.stops[0]));
assert.match(derive.freightRouteReason(state,state.ghosts[0],bad),/Autonomous Freight/);
state.research.programmes.push({id:"prop_line_autonomous_freight",state:"completed"});
assert.equal(derive.freightRouteReason(state,state.ghosts[0],bad),null);
bad.fuel_reserve=200; assert.match(derive.freightRouteReason(state,state.ghosts[0],bad),/tank/);
const msg={type:"SetFreightRoute",fleet_id:"f",route};
const preview=commands.fleetCommandIntent(msg,state); route.name="Edited after preview";
assert.equal(preview.commands[0].route.name,"Ore outward, Fuel return","confirmation owns immutable payload");

const staged=[],sent=[];
const ctx={state,send:m=>sent.push(m),intent:{beginFleetCommand:m=>staged.push(structuredClone(m))}};
shell.industryHtml(state);
assert.equal(shell.industryUiSignature(state).route.transit,"slow","new routes default to Slow (Uranium) before Fusion Freight");
shell.handleIndustryInput(new Input({industryInput:"transit"},"full"),state);
assert.equal(shell.industryUiSignature(state).route.transit,"full");
shell.handleIndustryInput(new Input({industryInput:"transit"},"economy"),state);
assert.equal(shell.industryUiSignature(state).route.transit,"full","the retired Economy mode is refused");
shell.handleIndustryInput(new Input({industryInput:"transit"},"slow"),state);
shell.handleIndustryInput(new Input({industryInput:"name"},"My persistent draft"),state);
shell.handleIndustryInput(new Input({industryInput:"good"},"deuterium"),state);
shell.handleIndustryAction({dataset:{industryAct:"add-cargo",stop:"0",side:"load"}},ctx);
for(let i=0;i<10;i++){site.stockpile[0].units++;shell.industryHtml(state);}
assert.equal(shell.industryUiSignature(state).route.name,"My persistent draft");
assert.equal(shell.industryUiSignature(state).route.stops[0].load.deuterium,25);
shell.handleIndustryInput(new Input({industryInput:"escort"},"e"),state);
shell.handleIndustryAction({dataset:{industryAct:"assign"}},ctx);
assert.equal(sent.length,0,"assign never bypasses fleet confirmation");
assert.equal(staged[0][0].route.transit,"slow","cruise travels with the confirmed route");
assert.deepEqual(plain(staged[0].map(c=>c.type)),["SetFreightRoute","GuardFleet"]);
assert.equal(staged[0][1].interceptor_id,"e");
state.ghosts[0].industry={kind:"route",run:{route:structuredClone(route),stop:1,phase:"loading",visits:3}};
assert.match(shell.fleetIndustryHtml(state.ghosts[0],state),/stop 2\/2/);
assert.match(shell.fleetIndustryHtml(state.ghosts[0],state),/data-deck-act="fleet-logistics" data-mobile-act="open-logistics" data-fleet="f">Open in Logistics/,"a route line links to its trip");
shell.handleIndustryAction({dataset:{industryAct:"tab",tab:"planner"}},ctx);
shell.focusIndustryFleet(state,"f");
assert.deepEqual(plain((({tab,fleet,editing})=>({tab,fleet,editing}))(shell.industryUiSignature(state))),{tab:"routes",fleet:"f",editing:false},"opening a Freighter's trip shows its Routes view");
assert.equal(shell.fleetIndustryHtml({...state.ghosts[0],own:false},state),"");
shell.handleIndustryAction({dataset:{industryAct:"copy",fleet:"f"}},ctx);
assert.notEqual(shell.industryUiSignature(state).route,state.ghosts[0].industry.run.route);
shell.handleIndustryAction({dataset:{industryAct:"close-editor"}},ctx);
const trip=shell.industryHtml(state);
assert.match(trip,/Loading at Market Hub/,"a running route shows where its Freighter is");
assert.match(trip,/Stop route…/);assert.doesNotMatch(trip,/Assign route/,"no editor over a running route until asked");

site.stockpile.push({commodity:"alloys",units:50});
assert.match(derive.colonyProjectReason(state,site,0,"orbital_assembly","metallic_ore"),/Fusion Reactors/,"its Warp Cores wait for Fusion Reactors");
state.research.programmes.push({id:"mat_foundry_iv_fusion_reactors",state:"completed"});
assert.equal(derive.colonyProjectReason(state,site,0,"orbital_assembly","metallic_ore"),null);
site.industry.reservations.push({target:{kind:"cruiser"},goods:{alloys:50}});
assert.match(derive.colonyProjectReason(state,site,0,"orbital_assembly","metallic_ore"),/Deliver/);
site.industry.reservations= [{target:{kind:"development",project:"orbital_assembly"},goods:{alloys:50}}];
assert.equal(derive.colonyProjectReason(state,site,0,"orbital_assembly","metallic_ore"),null,"matching project spends only its own floor");
site.industry.projects.push({kind:"deep_extraction",work:1});
assert.match(derive.colonyProjectReason(state,site,0,"orbital_assembly","metallic_ore"),/Finish/);
console.log("Industrial expansion: served-only planner, project floors, exact manifests, stable drafts, route validation and confirmation pass.");

// Local-opening plans use reported deficits and a bounded basket, not an
// assumed complete construction kit or an automatic command on selection.
site.structures={}; site.stockpile=[]; site.industry.reservations=[];
state.founding={local_opening:true,stage:"local_recovery",recovery_site:"cache",recovered:false};
state.explorationSites=[{id:"cache",details:null}];
state.galaxy.build_options.push(...[
  ["shipyard",[["alloys",40],["machinery",20],["electronics",15]]],
  ["academy",[["alloys",25],["electronics",15],["provisions",20]]],
].map(([key,costs])=>({key,costs:costs.map(([commodity,units])=>({commodity,units}))})));
for(const goal of ["transport","refining","exploration"]){
  const plan=derive.openingImportPlan(state,site,goal);
  assert.ok(Object.values(plan.manifest).reduce((n,v)=>n+v,0)<=50);
  assert.equal(plan.manifest.uranium,undefined,"the freighter flies Slow and fills at the Hub; the basket buys no fuel");
}
const kit=derive.openingImportPlan(state,site,"transport");
assert.equal(Object.values(kit.manifest).reduce((n,v)=>n+v,0),50);
assert.equal(Object.values(kit.remainder).reduce((n,v)=>n+v,0),25,"a 75-unit yard kit needs more than one 50-unit materials basket");
site.stockpile=[{commodity:"alloys",units:40}];
assert.equal(derive.openingImportPlan(state,site,"transport").manifest.alloys,undefined,"arrived home stock reduces the shopping deficit");
state.research.programmes=[];
const sentBefore=sent.length, stagedBefore=staged.length;
shell.handleIndustryAction({dataset:{industryAct:"opening-plan",home:"home",goal:"transport"}},ctx);
const draft=shell.industryUiSignature(state).route;
assert.equal(sent.length,sentBefore);assert.equal(staged.length,stagedBefore);
assert.equal(draft.shopping_budget,3000);assert.equal(draft.repeat,false);
assert.equal(draft.stops.length,3);assert.equal(draft.stops[1].sell,true);
assert.equal(derive.freightRouteReason(state,state.ghosts[0],draft),null,"one-off home/Hub/home needs no automation research");
const repeated=structuredClone(draft);repeated.repeat=true;
assert.match(derive.freightRouteReason(state,state.ghosts[0],repeated),/Autonomous Freight/);
const invalidCap=structuredClone(draft);invalidCap.shopping_budget=-1;
assert.match(derive.freightRouteReason(state,state.ghosts[0],invalidCap),/shopping cap/);
shell.handleIndustryAction({dataset:{industryAct:"assign"}},ctx);
assert.equal(sent.length,sentBefore);
assert.deepEqual(plain(staged.at(-1).map(c=>c.type)),["SetFreightRoute","GuardFleet"]);
const founding=compile("core/derive/founding.ts");
assert.ok(founding.foundingLocalGoals(state,site).some(g=>g.action==="opening-imports"),"Landfall offers the guarded shipment");
assert.ok(!founding.foundingLocalGoals(state,site).some(g=>["recovery","build-service-berth","scout"].includes(g.action)),"charter v3 has no beacon, cache or Scout refit");
// §logistics: the charter's shipment is one card: the basket, its cap and the escort, sent behind the standard confirmation.
state.ghosts[0].industry=null;
shell.handleIndustryAction({dataset:{industryAct:"close-editor"}},ctx);
let card=shell.industryHtml(state);
assert.match(card,/Send your first shipment/);
assert.match(card,/data-coach="send-shipment"/,"the guide's coach ring has a target");
assert.match(card,/Covers all 35 Shipyard goods you lack/,"coverage counts the home deficit");
assert.match(card,/Waits for 50 ore · 0 in stock/,"the load waits for ore it does not have yet");
assert.match(card,/Prices arrive with the next Hub report/,"no money estimate before a price report");
const basketBefore=plain(derive.openingImportPlan(state,site,"transport").manifest);
assert.match(card,/data-good="uranium"/,"the basket offers Uranium for the home stockpile");
shell.handleIndustryAction({dataset:{industryAct:"basket",good:"uranium",step:"1"}},ctx);
assert.equal(shell.industryUiSignature(state).basket.uranium,(basketBefore.uranium??0)+1,"steppers edit the return basket");
const holdOf=fleets.fleetCargoCapacity;fleets.fleetCargoCapacity=g=>g.kind==="convoy"?47:0;
for(let i=0;i<15;i++)shell.handleIndustryAction({dataset:{industryAct:"basket",good:"machinery",step:"1"}},ctx);
assert.equal(Object.values(shell.industryUiSignature(state).basket).reduce((n,v)=>n+v,0),47,"the basket never outgrows the hold");
fleets.fleetCargoCapacity=holdOf;
const shipped=staged.length;
shell.handleIndustryAction({dataset:{industryAct:"send-shipment"}},ctx);
assert.equal(sent.length,sentBefore,"sending never bypasses fleet confirmation");
assert.equal(staged.length,shipped+1);
assert.deepEqual(plain(staged.at(-1).map(c=>c.type)),["SetFreightRoute","GuardFleet"]);
const shipment=staged.at(-1)[0].route;
assert.deepEqual(plain(shipment.stops[1].load),plain(shell.industryUiSignature(state).basket),"the Hub stop buys the edited basket");
assert.deepEqual(plain(shipment.stops[2].unload),plain(shipment.stops[1].load),"home unloads what the Hub loaded");
assert.equal(shipment.stops[0].load.metallic_ore,50);assert.equal(shipment.escort,"e");
state.market={staleness:0,prices:[["metallic_ore",27],["alloys",33],["machinery",41],["electronics",64],["uranium",6]].map(([commodity,price])=>({commodity,price,available_buy:999,available_sell:999,depth:1600}))};
const estimate=derive.routeHubEstimate(state,shipment);
assert.ok(estimate.sold>1300&&estimate.sold<1350,"50 ore sells for about 27 each, walking the book");
assert.ok(estimate.bought>0);
state.wallet={warehouse:[{commodity:"machinery",units:100}]};
assert.ok(derive.routeHubEstimate(state,shipment).bought<estimate.bought,"warehouse stock loads before anything is bought");
assert.deepEqual(derive.routeAboard(shipment),[50,shipment.stops[1].load ? Object.values(shipment.stops[1].load).reduce((n,v)=>n+v,0) : 0,0]);
assert.equal(shipment.transit,"slow","the first shipment flies Slow on Uranium");
assert.deepEqual(plain(derive.routeLegSeconds(state,state.ghosts[0],shipment)),[100,100],"10,000 su at Slow (40 × 5 × ½ su/s)");
state.market=null;state.wallet=null;
const received=plain(site);
shell.handleIndustryAction({dataset:{industryAct:"refining-floor",on:"true"}},ctx);
assert.deepEqual(plain(sent.at(-1)),{type:"SetRefiningReserve",system_id:"home",ore:50,fuel:0});
assert.deepEqual(plain(site),received,"sending a policy never changes the served report optimistically");

// §belts: a barge's belt order needs a barge and a charted belt holding the ore.
state.systems.push({ id: "belt-sys", owner: null, stockpile: [], assignments: [], industry: { reservations: [], outpost: null, projects: [] },
  bodies: [{ id: 3, name: "Sif Belt", kind: "asteroid_belt", structures: {}, deposits: [{ resource: "metallic_ore", richness: 0.2 }] }] });
state.galaxy.systems.push({ id: "belt-sys", name: "Sif", pos: { x: 18000, y: 0 } });
state.ghosts.push({ id: "b", kind: "mining_barge", own: true, pos: { x: 0, y: 0 }, composition: [{ kind: "mining_barge", count: 1 }] });
const mineOrder = { type: "MineBelt", fleet_id: "b", system_id: "belt-sys", body_id: 3, ore: "metallic_ore" };
assert.equal(commands.fleetCommandsValid([mineOrder], state), true);
assert.deepEqual(plain(commands.fleetCommandIntent(mineOrder, state).dest), { x: 18000, y: 0 }, "the preview points at the belt's system");
assert.equal(commands.fleetCommandsValid([{ ...mineOrder, fleet_id: "f" }], state), false, "only barges mine belts");
assert.equal(commands.fleetCommandsValid([{ ...mineOrder, ore: "cuprite_ore" }], state), false, "the belt must hold the ore");
state.ghosts.at(-1).industry = { kind: "mine", system: "belt-sys", body: 3, ore: "metallic_ore", mined: 37.4, fraction: 0.4, status: "full" };
assert.match(shell.fleetIndustryHtml(state.ghosts.at(-1), state), /Mining .*Sif Belt · hold full · bring it home to unload · 37/);
assert.doesNotMatch(shell.fleetIndustryHtml(state.ghosts.at(-1), state), /Open in Logistics/, "only a freight route links to Logistics");
// §pockets: a found pocket is mined by name, and a Scout or barge prospects.
const beltBody = state.systems.at(-1).bodies[0];
const pocketOrder = { ...mineOrder, ore: "titanium_ore", pocket: true };
assert.equal(commands.fleetCommandsValid([pocketOrder], state), false, "an unfound pocket is refused");
beltBody.pockets = [{ resource: "titanium_ore", richness: 0.9, reserves: 1800 }];
assert.equal(commands.fleetCommandsValid([pocketOrder], state), true, "a found pocket may be mined");
assert.match(commands.fleetCommandSummary(commands.fleetCommandIntent(pocketOrder, state), state), /Mine Titanium Ore pocket → Sif Belt/);
const prospectOrder = { type: "ProspectBelt", fleet_id: "b", system_id: "belt-sys", body_id: 3 };
assert.equal(commands.fleetCommandsValid([prospectOrder], state), true, "a barge prospects");
assert.equal(commands.fleetCommandsValid([{ ...prospectOrder, fleet_id: "f" }], state), false, "a freighter does not");
assert.deepEqual(plain(commands.fleetCommandIntent(prospectOrder, state).dest), { x: 18000, y: 0 }, "the preview points at the belt");
assert.match(commands.fleetCommandSummary(commands.fleetCommandIntent(prospectOrder, state), state), /Prospect Sif Belt · 60 s loud pass/);
state.ghosts.at(-1).industry = { kind: "mine", system: "belt-sys", body: 3, ore: "titanium_ore", pocket: true, mined: 12, fraction: 0, status: "mining" };
assert.match(shell.fleetIndustryHtml(state.ghosts.at(-1), state), /Mining Titanium Ore pocket → Sif Belt · mining · 12/);
state.ghosts.at(-1).industry = { kind: "prospect", system: "belt-sys", body: 3, dwell: 18.2, status: "prospecting" };
assert.match(shell.fleetIndustryHtml(state.ghosts.at(-1), state), /Prospecting Sif Belt · 42 s left/);
state.ghosts.at(-1).industry = { kind: "prospect", system: "belt-sys", body: 3, dwell: 0, status: "travelling" };
assert.match(shell.fleetIndustryHtml(state.ghosts.at(-1), state), /Prospecting Sif Belt · en route/);
state.ghosts.at(-1).industry = null;

// §pockets: the belt actions both shells draw, and the prospecting line.
const belts = beltsDerive;
const beltSite = state.systems.at(-1);
let acts = belts.beltActions(state.ghosts, "1", beltSite, beltBody);
assert.deepEqual(plain(acts.prospect.map(a => a.fleet)), ["b"], "the barge prospects");
assert.deepEqual(plain(acts.mine.map(a => [a.ore, a.pocket, a.text])),
  [["metallic_ore", false, "Mine Ferrite Ore"], ["titanium_ore", true, "Mine Titanium Ore pocket"]]);
beltBody.pockets[0].reserves = 0;
assert.equal(belts.beltActions(state.ghosts, "1", beltSite, beltBody).mine.length, 1, "a worked-out pocket offers no Mine");
beltSite.owner = "2";
acts = belts.beltActions(state.ghosts, "1", beltSite, beltBody);
assert.equal(acts.mine.length, 0); assert.equal(acts.note, "Another corporation holds this system.");
assert.equal(acts.prospect.length, 1, "prospecting a rival's belt is still knowledge");
beltSite.owner = null;
assert.equal(belts.prospectLine(beltBody), "1 pocket found");
beltBody.prospected = 3;
assert.equal(belts.prospectLine(beltBody), "Prospected 3× · 1 pocket found");
delete beltBody.pockets;
assert.equal(belts.prospectLine(beltBody), "Prospected 3× · no pockets found");
delete beltBody.prospected;
assert.equal(belts.prospectLine(beltBody), "Unprospected");
// §belts: the Mining Belt Station — anchoring, its place at a belt, loading.
const station = { id: "st", kind: "mining_belt_station", own: true, pos: { x: 18000, y: 0 },
  composition: [{ kind: "mining_belt_station", count: 1 }], cargo_manifest: [{ commodity: "metallic_ore", units: 900 }] };
state.ghosts.push(station);
assert.equal(belts.isStation(station), true);
assert.equal(belts.isStation({ kind: "convoy", composition: [{ kind: "mining_belt_station", count: 1 }, { kind: "convoy", count: 1 }] }), false, "only a lone station anchors");
acts = belts.beltActions(state.ghosts, "1", beltSite, beltBody);
assert.deepEqual(plain(acts.anchor.map(a => a.fleet)), ["st"], "a free station may anchor at an unowned belt");
const anchorOrder = { type: "AnchorStation", fleet_id: "st", system_id: "belt-sys", body_id: 3 };
assert.equal(commands.fleetCommandsValid([anchorOrder], state), true);
assert.match(commands.fleetCommandSummary(commands.fleetCommandIntent(anchorOrder, state), state), /Anchor at Sif Belt · 60 s on arrival/);
beltSite.owner = "1";
assert.equal(commands.fleetCommandsValid([anchorOrder], state), false, "a colony stockpiles its own ore");
assert.equal(belts.beltActions(state.ghosts, "1", beltSite, beltBody).anchor.length, 0);
beltSite.owner = "2"; beltSite.ally = true;
assert.equal(commands.fleetCommandsValid([anchorOrder], state), true, "an ally's belt is open");
beltSite.owner = null; delete beltSite.ally;
station.anchor = { system: "belt-sys", body: 3, phase: "anchoring", since: 100 };
assert.equal(belts.anchorStatus(station.anchor, 118.2), "Anchoring · 42 s");
assert.equal(belts.anchorStatus({ ...station.anchor, phase: "anchored" }, 500), "Anchored");
acts = belts.beltActions(state.ghosts, "1", beltSite, beltBody);
assert.equal(acts.anchor.length, 0, "a belt with your station offers no second anchor");
assert.deepEqual(plain([acts.station.fleet, acts.station.held, acts.station.capacity]), ["st", 900, 3000]);
assert.equal(commands.fleetCommandsValid([{ type: "UnanchorStation", fleet_id: "st" }], state), true);
const hauler = state.ghosts.find(g => g.id === "f");
const loadOrder = { type: "StationLoad", fleet_id: "f", station_id: "st", commodity: "metallic_ore", units: 250 };
assert.equal(commands.fleetCommandsValid([loadOrder], state), false, "only a freighter beside the station loads");
const parked = { ...hauler.pos };
hauler.pos = { x: 18100, y: 0 };
assert.equal(commands.fleetCommandsValid([loadOrder], state), true);
assert.match(commands.fleetCommandSummary(commands.fleetCommandIntent(loadOrder, state), state), /Load 250 Ferrite Ore from the station/);
// A station stop only loads.
const stationRun = { name: "Belt run", repeat: true, fuel_reserve: 0, escort: null, transit: "full", shopping_budget: null,
  stops: [{ port: { kind: "station", id: "st" }, load: { metallic_ore: 200 }, unload: {}, sell: false },
    { port: { kind: "system", id: "home" }, load: {}, unload: { metallic_ore: 200 }, sell: false }] };
assert.equal(derive.routePortId(stationRun.stops[0].port), "station:st");
assert.deepEqual(plain(derive.routePortPosition(state, stationRun.stops[0].port)), { x: 18000, y: 0 });
assert.equal(derive.freightRouteReason(state, hauler, stationRun), null);
stationRun.stops[0].unload = { deuterium: 5 };
assert.equal(derive.freightRouteReason(state, hauler, stationRun), "Stop 1: a station stop only loads.");
hauler.pos = parked;
state.ghosts.splice(state.ghosts.indexOf(station), 1);
state.systems.at(-1).bodies[0].deposits = null;
assert.equal(belts.prospectLine(beltBody), "", "an uncharted belt says nothing");
assert.equal(commands.fleetCommandsValid([mineOrder], state), false, "an uncharted belt is refused");
// §fuel-overhaul: fuel depots — "keep N fuel at a port", sent as a policy.
state.depotRules = [{ system: "home", commodity: "deuterium", target: 40 }];
shell.handleIndustryAction({dataset:{industryAct:"tab",tab:"standing"}},ctx);
const depots = shell.industryHtml(state);
assert.match(depots, /Fuel depots/);
assert.match(depots, /Deuterium · Freya/);
assert.match(depots, /Keep 40/);
shell.handleIndustryInput(new Input({industryInput:"depotFuel"},"uranium"),state);
shell.handleIndustryInput(new Input({industryInput:"depotTarget"},"120"),state);
shell.handleIndustryAction({dataset:{industryAct:"depot-set"}},ctx);
assert.deepEqual(plain(sent.at(-1)),{type:"SetDepotRule",system_id:"home",commodity:"uranium",target:120});
shell.handleIndustryInput(new Input({industryInput:"depotFuel"},"alloys"),state);
assert.equal(shell.industryUiSignature(state).depotFuel,"uranium","depots stock fuels only");
shell.handleIndustryInput(new Input({industryInput:"depotTarget"},"99999"),state);
assert.equal(shell.industryUiSignature(state).depotTarget,2000,"capped at the sim's DEPOT_MAX_TARGET");
shell.handleIndustryAction({dataset:{industryAct:"depot-clear",system:"home",good:"deuterium"}},ctx);
assert.deepEqual(plain(sent.at(-1)),{type:"SetDepotRule",system_id:"home",commodity:"deuterium",target:0});
console.log("Local opening: bounded goal baskets, honest shortfalls, one-off return, confirmation, report-only guide, refining floors and fuel depots pass.");
