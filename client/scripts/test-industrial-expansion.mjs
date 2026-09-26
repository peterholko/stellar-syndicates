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
const fleets={fleetCargoCapacity:g=>g.kind==="convoy"?250:0,fleetFuelCapacity:()=>160,fleetBaseSpeed:()=>40,
  economyCapable:g=>g.kind==="convoy",transitSpeedMultiplier:g=>g.transit==="economy"?.25:g.transit==="stealth"?.5:1,
  transitLabel:(m="full")=>m==="economy"?"Economy":m==="stealth"?"Stealth":"Full speed",
  guardCapable:g=>g.own&&g.kind==="raider",shipKindLabel:k=>k==="convoy"?"Freighter":"Interceptor",WARP_FACTOR:5};
const market=compile("core/derive/market.ts");
const derive=compile("core/derive/industry.ts",{"./fleet":fleets,"./market":market});
const icons=compile("icons.ts");
const protocol=compile("protocol.ts");
const shell=compile("shell/industry.ts",{"../core/derive/fleet":fleets,"../core/derive/industry":derive,"../icons":icons,
  "../core/derive/geo":{foundingHomeSystemId:()=>"home"},
  "../protocol":protocol,"../core/derive/market":{COMMODITIES:["metallic_ore","fuel","alloys","biomass"]}});
const commands=compile("core/fleetorders.ts",{"./derive/industry":derive,"./derive/fleet":fleets,"../icons":icons});
const body={id:0,name:"Freya II",habitable:true,deposits:[{resource:"biomass"}],structures:{shipyard:2}};
const line={structure:"smelter",workers:1,throughput:1,staffing:.5,skill:1,food:1,site:1,outputs:[["alloys",.1]],suspended:null};
const site={id:"home",owner:"1",stockpile:[{commodity:"metallic_ore",units:120}],bodies:[body],assignments:[line],workforce:{posted:2,units:1},industry:{reservations:[],outpost:null,projects:[]}};
const other={...structuredClone(site),id:"mine",assignments:[{...line,structure:"extractor",outputs:[["metallic_ore",.5]]}],stockpile:[{commodity:"metallic_ore",units:40}]};
const state={playerId:"1",founding:{expansion_unlocked:true},systems:[site,other],ghosts:[{id:"f",kind:"convoy",own:true,pos:{x:0,y:0},cargo_manifest:[],fuel:100},
  {id:"e",kind:"raider",own:true,pos:{x:0,y:0}}],research:{programmes:[]},
  galaxy:{c:400,hub:{x:10000,y:0},systems:[{id:"home",name:"Freya",pos:{x:0,y:0}},{id:"mine",name:"Ore colony",pos:{x:30000,y:0}}],
    build_options:[{key:"smelter",label:"Smelter",conversion:{output:"alloys",rate:.2,inputs:[["metallic_ore",2]]}}],
    industry_catalog:{projects:[{kind:"orbital_assembly",costs:[["alloys",50]],inputs:[["fuel",.2]],workers:3,build_secs:180}],outpost_costs:[["alloys",30]],outpost_build_secs:60,skim_fuel_per_s:1,max_stops:8}}};
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
  {port:{kind:"system",id:"home"},load:{metallic_ore:40,biomass:20},unload:{fuel:10},sell:false},
  {port:{kind:"hub"},load:{fuel:10},unload:{metallic_ore:40,biomass:20},sell:true}]};
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
assert.equal(shell.industryUiSignature(state).route.transit,"economy","new routes default to economy");
shell.handleIndustryInput(new Input({industryInput:"transit"},"full"),state);
assert.equal(shell.industryUiSignature(state).route.transit,"full");
shell.handleIndustryInput(new Input({industryInput:"transit"},"economy"),state);
shell.handleIndustryInput(new Input({industryInput:"name"},"My persistent draft"),state);
shell.handleIndustryInput(new Input({industryInput:"good"},"fuel"),state);
shell.handleIndustryAction({dataset:{industryAct:"add-cargo",stop:"0",side:"load"}},ctx);
for(let i=0;i<10;i++){site.stockpile[0].units++;shell.industryHtml(state);}
assert.equal(shell.industryUiSignature(state).route.name,"My persistent draft");
assert.equal(shell.industryUiSignature(state).route.stops[0].load.fuel,25);
shell.handleIndustryInput(new Input({industryInput:"escort"},"e"),state);
shell.handleIndustryAction({dataset:{industryAct:"assign"}},ctx);
assert.equal(sent.length,0,"assign never bypasses fleet confirmation");
assert.equal(staged[0][0].route.transit,"economy","cruise travels with the confirmed route");
assert.deepEqual(plain(staged[0].map(c=>c.type)),["SetFreightRoute","GuardFleet"]);
assert.equal(staged[0][1].interceptor_id,"e");
state.ghosts[0].industry={kind:"route",run:{route:structuredClone(route),stop:1,phase:"loading",visits:3}};
assert.match(shell.fleetIndustryHtml(state.ghosts[0],state),/stop 2\/2/);
assert.equal(shell.fleetIndustryHtml({...state.ghosts[0],own:false},state),"");
shell.handleIndustryAction({dataset:{industryAct:"copy",fleet:"f"}},ctx);
assert.notEqual(shell.industryUiSignature(state).route,state.ghosts[0].industry.run.route);

site.stockpile.push({commodity:"alloys",units:50});
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
  assert.equal(plan.manifest.fuel,10);
}
const kit=derive.openingImportPlan(state,site,"transport");
assert.equal(Object.values(kit.manifest).reduce((n,v)=>n+v,0),50);
assert.equal(Object.values(kit.remainder).reduce((n,v)=>n+v,0),35,"a 75-unit yard kit needs more than one 40-unit materials basket");
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
assert.ok(founding.foundingLocalGoals(state,site).some(g=>g.action==="recovery"));
assert.ok(founding.foundingLocalGoals(state,site).some(g=>g.action==="build-service-berth"));
assert.match(shell.industryHtml(state),/Dispatch is a good stopping point/);
const received=plain(site);
shell.handleIndustryAction({dataset:{industryAct:"refining-floor",on:"true"}},ctx);
assert.deepEqual(plain(sent.at(-1)),{type:"SetRefiningReserve",system_id:"home",ore:50,fuel:20});
assert.deepEqual(plain(site),received,"sending a policy never changes the served report optimistically");
console.log("Local opening: bounded goal baskets, honest shortfalls, one-off return, confirmation, report-only guide and refining floors pass.");
