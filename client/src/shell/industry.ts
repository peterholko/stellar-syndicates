import type { CoreContext } from "./types";
import type { ViewState } from "../state";
import type { ColonyProjectKind, Commodity, FreightRoute, GhostView, Manifest, OutpostKind, ProjectTarget } from "../protocol";
import { fleetCargoManifest } from "../protocol";
import { commodityIcon, label } from "../icons";
import { COMMODITIES, MINERAL_DEPOSITS } from "../core/derive/market";
import { fleetCargoCapacity, fleetFuelCapacity, guardCapable, shipKindLabel } from "../core/derive/fleet";
import { colonyProjectReason, freightRouteReason, productionPlan, projectName, routePortId } from "../core/derive/industry";
import type { FleetCommand } from "../core/fleetorders";
import "../styles/industry.css";

const esc = (s: string): string => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
const act = (a: string, extra = "") => `data-deck-act="industry" data-mobile-act="industry" data-industry-act="${a}" ${extra}`;
const option = (id: string, name: string, current: string) => `<option value="${esc(id)}" ${id === current ? "selected" : ""}>${esc(name)}</option>`;
const field = (key: string, extra = "") => `data-industry-input="${key}" ${extra}`;
const amount = (n: number) => Number(n.toFixed(1)).toLocaleString();
const goods = (manifest: Manifest) => Object.entries(manifest).filter(([,n]) => n! > 0).map(([c,n]) => `${amount(n!)} ${label(c)}`).join(" · ") || "None";
const projectKinds: ColonyProjectKind[] = ["orbital_assembly", "agricultural_export", "deep_extraction"];
const targets: ProjectTarget[] = [{ kind: "cruiser" }, { kind: "academy" }, { kind: "colony" }, ...projectKinds.map(project => ({ kind: "development" as const, project }))];
const key = (t: ProjectTarget) => t.kind === "development" ? t.project : t.kind;
const fleetName = (g: GhostView) => `${shipKindLabel(g.kind)} · ${g.id}`;
type Ui = { owner: string | null; tab: string; fleet: string; system: string; route: FreightRoute; good: Commodity; qty: number;
  output: Commodity; rate: number; project: ColonyProjectKind; body: number; resource: Commodity;
  outpostSystem: string; outpostBody: number; outpostKind: OutpostKind; builder: string; skimSystem: string; skimFleet: string; feedback: string };
const views = new WeakMap<ViewState, Ui>();
function ui(st: ViewState): Ui {
  let v = views.get(st);
  if (!v || v.owner !== st.playerId) {
    v = { owner: st.playerId, tab: "routes", fleet: "", system: "", route: { name: "Supply circuit", fuel_reserve: 10, escort: null, repeat: true,
      stops: [{ port: { kind: "hub" }, load: {}, unload: {}, sell: false }, { port: { kind: "hub" }, load: {}, unload: {}, sell: false }] },
      good: "metallic_ore", qty: 25, output: "alloys", rate: 10, project: "agricultural_export", body: 0, resource: "metallic_ore",
      outpostSystem: "", outpostBody: 0, outpostKind: "extraction", builder: "", skimSystem: "", skimFleet: "", feedback: "" };
    const home = st.systems.find(s => s.owner === st.playerId);
    if (home) { v.system = home.id; v.route.stops[0].port = { kind: "system", id: home.id }; }
    views.set(st, v);
  }
  return v;
}
export const industryUiSignature = (st: ViewState) => ui(st);
export const showStandingIndustry = (st: ViewState): void => { ui(st).tab = "standing"; };
export const industryStandingVisible = (st: ViewState): boolean => ui(st).tab === "standing";
const sysName = (st: ViewState, id: string) => st.galaxy?.systems.find(s => s.id === id)?.name ?? id;

/** Shared desktop/mobile UI. Draft edits never mutate served fleet/site facts;
 * every fleet command stages the standard confirmation, and all progress below
 * is received light. A View refresh cannot erase an unfinished manifest. */
export function industryHtml(st: ViewState): string {
  const v = ui(st);
  const tabs = `<nav class="industry-tabs" aria-label="Logistics sections">${["routes", "planner", "projects", "standing"].map(t =>
    `<button type="button" ${act("tab", `data-tab="${t}"`)} aria-pressed="${v.tab === t}">${t === "standing" ? "Standing rules" : label(t)}</button>`).join("")}</nav>`;
  return tabs + (v.feedback ? `<p class="industry-feedback" role="status">${esc(v.feedback)}</p>` : "")
    + (v.tab === "routes" ? routesHtml(st, v) : v.tab === "planner" ? plannerHtml(st, v) : v.tab === "projects" ? projectsHtml(st, v) : "");
}
function routesHtml(st: ViewState, v: Ui): string {
  const fleets = st.ghosts.filter(g => g.own && !g.tca && fleetCargoCapacity(g) > 0);
  if (!fleets.some(g => g.id === v.fleet)) v.fleet = fleets[0]?.id ?? "";
  const fleet = fleets.find(g => g.id === v.fleet);
  const portOptions = (current: string) => option("hub", "Market Hub", current) + st.systems.filter(s => s.owner === st.playerId).map(s => option(s.id, sysName(st,s.id), current)).join("");
  const reason = fleet ? freightRouteReason(st, fleet, v.route) : "Build a Freighter to assign a route.";
  const routes = fleets.filter(g => g.industry?.kind === "route").map(g => `<article class="industry-card"><h4>${esc(fleetName(g))}</h4>${fleetIndustryHtml(g, st)}
    <div class="industry-actions"><button type="button" ${act("copy", `data-fleet="${esc(g.id)}"`)}>Copy route to draft</button><button type="button" ${act("stop", `data-fleet="${esc(g.id)}"`)}>Stop route…</button></div></article>`).join("");
  const manifest = (m: Manifest, stop: number, side: "load" | "unload") => `<div class="industry-manifest"><b>${label(side)}</b>${Object.entries(m).filter(([,n]) => n! > 0).map(([c,n]) =>
    `<div>${commodityIcon(c as Commodity)}<span>${esc(label(c))}</span><input aria-label="${side} ${esc(label(c))}" type="number" min="0" max="${fleet ? fleetCargoCapacity(fleet) : 250}" value="${n}" ${field("manifest", `data-stop="${stop}" data-side="${side}" data-good="${c}"`)}></div>`).join("") || `<small>None</small>`}
    <button type="button" ${act("add-cargo", `data-stop="${stop}" data-side="${side}"`)}>+ ${label(v.good)}</button></div>`;
  return `<section class="industry-panel"><header><h3>Assigned Freighter routes</h3><small>Exact manifests per visit · return cargo supported</small></header>${routes}
    <div class="industry-grid"><label>Freighter<select ${field("fleet")}>${fleets.map(g => option(g.id, fleetName(g), v.fleet)).join("")}</select></label>
    <label>Route name<input maxlength="48" value="${esc(v.route.name)}" ${field("name")}></label>
    <label>Arrival Fuel reserve<input type="number" min="0" max="${fleet ? fleetFuelCapacity(fleet) : 0}" value="${v.route.fuel_reserve}" ${field("reserve")}></label>
    <label>Escort<select ${field("escort")}>${option("", "None", v.route.escort ?? "")}${st.ghosts.filter(g => g.id !== v.fleet && guardCapable(g)).map(g => option(g.id, fleetName(g),v.route.escort ?? "")).join("")}</select></label></div>
    <label class="industry-check"><input type="checkbox" ${field("repeat")} ${v.route.repeat ? "checked" : ""}>Repeat circuit</label>
    <div class="industry-grid"><label>Add commodity<select ${field("good")}>${COMMODITIES.map(c => option(c,label(c),v.good)).join("")}</select></label><label>Quantity<input type="number" min="1" value="${v.qty}" ${field("qty")}></label></div>
    ${v.route.stops.map((stop,i) => `<article class="industry-card" id="industry-stop-${i}"><header><b>Stop ${i+1}</b><button type="button" ${act("remove-stop",`data-stop="${i}"`)} ${v.route.stops.length <= 2 ? "disabled" : ""}>Remove</button></header>
      <select aria-label="Stop ${i+1} port" ${field("port",`data-stop="${i}"`)}>${portOptions(routePortId(stop.port))}</select>
      <div class="industry-grid">${manifest(stop.unload,i,"unload")}${manifest(stop.load,i,"load")}</div>
      ${stop.port.kind === "hub" ? `<label class="industry-check"><input type="checkbox" ${field("sell",`data-stop="${i}"`)} ${stop.sell ? "checked" : ""}>Sell only this visit’s unloaded goods</label>` : ""}</article>`).join("")}
    <div class="industry-actions"><button type="button" ${act("add-stop")} ${v.route.stops.length >= 8 ? "disabled" : ""}>+ Stop</button><button type="button" ${act("assign")} ${reason ? "disabled" : ""}>Assign route…</button></div>
    <small>${esc(reason ?? "Waits for cargo, storage, Fuel reserve and the assigned escort. Orders require confirmation.")}</small></section>` + skimmingHtml(st,v);
}
function plannerHtml(st: ViewState, v: Ui): string {
  const sites = st.systems.filter(s => s.owner === st.playerId);
  if (!sites.some(s => s.id === v.system)) v.system = sites[0]?.id ?? "";
  const s = sites.find(s => s.id === v.system);
  if (!s) return `<p>No colony reports available.</p>`;
  const p = productionPlan(st,s,v.output,v.rate);
  const limits = [p.capacityLimited && "Processing capacity", p.workforceLimited && "Workforce", p.feedstockLimited && "Feedstock", p.freightLimited && "Freight"].filter(Boolean);
  return `<section class="industry-panel"><h3>Production planner</h3><div class="industry-grid">
    <label>Colony<select ${field("system")}>${sites.map(s => option(s.id,sysName(st,s.id),v.system)).join("")}</select></label>
    <label>Output<select ${field("output")}>${COMMODITIES.map(c => option(c,label(c),v.output)).join("")}</select></label>
    <label>Target / minute<input type="number" min="0.1" step="1" value="${v.rate}" ${field("rate")}></label></div>
    <p><b>${limits.join(" · ") || "Rated output covered"}</b></p><p>Staffed ${amount(p.staffed)}/min · installed ${amount(p.rated)}/min · workforce ${s.workforce?.posted ?? 0}/${s.workforce?.units ?? 0}</p>
    <small>Planning from the latest received reports. Freight is a cruise-speed ceiling, not an arrival promise.</small>
    ${!p.recipe ? `<p>Extraction depends on surveyed deposits and assigned extraction teams.</p>` : `<h4>${esc(p.recipe.label)} inputs</h4>`}
    ${p.inputs.map(i => `<article class="industry-card"><header><b>${commodityIcon(i.commodity)} ${label(i.commodity)}</b><span>${amount(i.needed)}/min required</span></header>
      <p>Local ${amount(i.local)}/min · import ${amount(i.shortfall)}/min · route ceiling ${amount(i.freight)}/min</p>
      <small>Unreserved stock ${amount(i.stock)}${i.runway === null ? "" : ` · covers ~${amount(i.runway)} min`}</small>
      <div>${i.suppliers.map(x => `<p><b>${esc(x.name)}</b> · ${amount(x.surplus)}/min surplus · ${amount(x.stock)} stock${x.deposit ? " · local deposit" : ""}</p>`).join("") || `<p>No reported supplier. Survey another system or import from the Market.</p>`}</div></article>`).join("")}</section>`;
}
function projectsHtml(st: ViewState, v: Ui): string {
  const sites = st.systems.filter(s => s.owner === st.playerId);
  if (!sites.some(s => s.id === v.system)) v.system = sites[0]?.id ?? "";
  const s = sites.find(s => s.id === v.system), catalog = st.galaxy?.industry_catalog;
  const spec = catalog?.projects.find(p => p.kind === v.project);
  if (s && !s.bodies.some(b => b.id === v.body)) v.body = s.bodies[0]?.id ?? 0;
  const target: ProjectTarget = { kind: "development", project: v.project };
  const buildReason = s ? colonyProjectReason(st,s,v.body,v.project,v.resource) : "Choose a colony.";
  const cost = (t: ProjectTarget) => t.kind === "development" ? catalog?.projects.find(p => p.kind === t.project)?.costs ?? []
    : (st.galaxy?.build_options.find(b => b.key === (t.kind === "academy" ? "academy" : t.kind))?.costs ?? []).map(x => [x.commodity,x.units] as [Commodity,number]);
  const descriptions = { orbital_assembly: "Shipyard II · Composites + Precision Components + Fuel → Hull Sections and Drive Assemblies.",
    agricultural_export: "Habitable Biomass world · Biomass + Polymers + Fuel → bulk Provisions.",
    deep_extraction: "Mineral deposit · Fuel + Machinery → deep reserves of one chosen mineral." };
  return `<section class="industry-panel"><h3>Projects & reserves</h3><label>Colony<select ${field("system")}>${sites.map(s => option(s.id,sysName(st,s.id),v.system)).join("")}</select></label>
    ${s ? `<section><h4>Protected materials</h4><small>Stock floors fill as goods arrive. Matching construction spends its own reserve; exports and other projects cannot.</small>
    ${targets.map(t => { const r = s.industry?.reservations.find(r => key(r.target) === key(t)); return `<article class="industry-card"><header><b>${projectName(t)}</b><button type="button" ${act("reserve",`data-target="${key(t)}" data-on="${r ? "false" : "true"}"`)}>${r ? "Release" : "Reserve"}</button></header>
      <small>${esc(goods(Object.fromEntries(cost(t))))}</small>${r ? `<small>Reserved goal · ${esc(goods(r.goods))}</small>` : ""}</article>`; }).join("")}</section>
    ${s.industry?.outpost ? `<p>${label(s.industry.outpost.kind)} outpost · ${s.industry.outpost.supplied ? "supplied" : "waiting for supplies"}</p>` : `<section><h4>Major colony development</h4>
      ${s.industry?.projects.map(p => { const spec = catalog?.projects.find(s => s.kind === p.kind); const percent = Math.min(100,p.work / (spec?.build_secs ?? 180)*100); return `<article class="industry-card"><header><b>${projectName({kind:"development",project:p.kind})}</b><button type="button" ${act("active",`data-project="${p.kind}" data-on="${!p.active}"`)}>${p.active ? "Pause" : "Resume"}</button></header><progress max="100" value="${percent}"></progress><small>${!p.active ? "Paused" : percent < 100 ? `Construction ${Math.floor(percent)}%` : p.supplied ? goods(Object.fromEntries(p.outputs.map(([c,n]) => [c,n*60]))) + " / min" : "Waiting for workers / inputs / access"}</small></article>`; }).join("") ?? ""}
      <div class="industry-grid"><label>Project<select ${field("project")}>${projectKinds.map(k => option(k,projectName({kind:"development",project:k}),v.project)).join("")}</select></label>
      <label>World<select ${field("body")}>${s.bodies.map(b => option(String(b.id),b.name,String(v.body))).join("")}</select></label>
      ${v.project === "deep_extraction" ? resourceSelect(v.resource,"resource",MINERAL_DEPOSITS) : ""}</div>
      <p>${descriptions[v.project]}</p><small>${spec ? `${spec.workers} workforce units · ${spec.build_secs}s staffed construction · ${goods(Object.fromEntries(spec.costs))}` : "Catalog unavailable"}</small>
      <small>Imports / min: ${spec ? goods(Object.fromEntries(spec.inputs.map(([c,n]) => [c,n*60]))) : "—"}</small>
      <button type="button" ${act("build-project")} ${buildReason ? "disabled" : ""}>Build ${projectName(target)}</button>${buildReason ? `<small>${esc(buildReason)}</small>` : ""}</section>`}` : `<p>No owned colonies.</p>`}</section>` + outpostsHtml(st,v);
}
function resourceSelect(value: Commodity, name: string, list = COMMODITIES): string {
  return `<label>Resource<select ${field(name)}>${list.map(c => option(c,label(c),value)).join("")}</select></label>`;
}
function outpostsHtml(st: ViewState, v: Ui): string {
  const sites = st.systems.filter(s => s.owner === null && s.bodies.length && !s.bodies.some(b => b.habitable) && s.bodies.some(b => b.deposits !== null));
  if (!sites.some(s => s.id === v.outpostSystem)) v.outpostSystem = sites[0]?.id ?? "";
  const site = sites.find(s => s.id === v.outpostSystem);
  if (site && !site.bodies.some(b => b.id === v.outpostBody)) v.outpostBody = site.bodies[0]?.id ?? 0;
  const fleets = st.ghosts.filter(g => g.own && !g.tca && fleetCargoCapacity(g) > 0);
  if (!fleets.some(g => g.id === v.builder)) v.builder = fleets[0]?.id ?? "";
  const f = fleets.find(g => g.id === v.builder), kit = st.galaxy?.industry_catalog?.outpost_costs ?? [];
  const ready = f && kit.length && kit.every(([c,n]) => fleetCargoManifest(f).filter(x => x.commodity === c).reduce((sum,x) => sum+x.units,0) >= n);
  const suitable = v.outpostKind === "research" || site?.bodies.find(b => b.id === v.outpostBody)?.deposits?.some(d => d.resource === v.resource);
  const expanded = !st.founding || st.founding.expansion_unlocked;
  return `<section class="industry-panel"><h3>Uninhabitable-system outposts</h3><p>Bring a Freighter with ${goods(Object.fromEntries(kit))}. It survives deployment.</p>
    <small>Upkeep / min: 1.8 Provisions · 1.2 Fuel · 0.3 Machinery. Extraction stops without supplies or under blockade. Research outposts support your active programme.</small>
    <div class="industry-grid"><label>Surveyed system<select ${field("outpostSystem")}>${sites.map(s => option(s.id,sysName(st,s.id),v.outpostSystem)).join("")}</select></label>
    <label>World<select ${field("outpostBody")}>${site?.bodies.map(b => option(String(b.id),b.name,String(v.outpostBody))).join("") ?? ""}</select></label>
    <label>Type<select ${field("outpostKind")}>${option("extraction","Extraction",v.outpostKind)}${option("research","Research",v.outpostKind)}</select></label>
    <label>Freighter<select ${field("builder")}>${fleets.map(g => option(g.id,fleetName(g),v.builder)).join("")}</select></label>
    ${v.outpostKind === "extraction" ? resourceSelect(v.resource,"resource",[...MINERAL_DEPOSITS,"volatiles","biomass"]) : ""}</div>
    <button type="button" ${act("deploy")} ${site && ready && suitable && expanded ? "" : "disabled"}>Establish outpost…</button><small>${!expanded ? "Complete the founding surveys to unlock expansion." : !sites.length ? "Survey an uninhabitable system first." : !ready ? "Load the deployment materials aboard your Freighter first." : !suitable ? "Select a resource present on this world." : "Bring operating supplies and protection."}</small></section>`;
}
function skimmingHtml(st: ViewState,v: Ui): string {
  const sites = st.systems.filter(s => s.bodies.some(b => b.kind === "gas_giant"));
  const fleets = st.ghosts.filter(g => g.own && !g.tca);
  if (!sites.some(s => s.id === v.skimSystem)) v.skimSystem = sites[0]?.id ?? "";
  if (!fleets.some(g => g.id === v.skimFleet)) v.skimFleet = fleets[0]?.id ?? "";
  const unlocked = st.research?.programmes.some(p => p.id === "prop_expedition_v_ramscoop" && p.state === "completed");
  return `<section class="industry-panel"><h3>Gas-giant Fuel harvesting</h3><p>Refill tanks; Freighters collect excess as cargo for Fuel Tenders.</p>
    <div class="industry-grid"><label>Fleet<select ${field("skimFleet")}>${fleets.map(g => option(g.id,fleetName(g),v.skimFleet)).join("")}</select></label>
    <label>Gas-giant system<select ${field("skimSystem")}>${sites.map(s => option(s.id,sysName(st,s.id),v.skimSystem)).join("")}</select></label></div>
    <button type="button" ${act("skim")} ${unlocked && v.skimFleet && v.skimSystem ? "" : "disabled"}>Harvest Fuel…</button>
    <small>${unlocked ? `${st.galaxy?.industry_catalog?.skim_fuel_per_s ?? 1} Fuel/s on station; pauses near combat or blockades.` : "Requires Ramscoop Skimming research."}</small></section>`;
}
export function fleetIndustryHtml(g: GhostView, st: ViewState): string {
  if (!g.own || !g.industry) return "";
  const i = g.industry;
  const text = i.kind === "route" ? `${i.run.route.name} · stop ${i.run.stop+1}/${i.run.route.stops.length} · ${i.run.hold ?? label(i.run.phase)}${i.run.route.escort ? " · escorted" : ""}`
    : i.kind === "deploy" ? `${label(i.outpost)} outpost → ${sysName(st,i.system)} · ${Math.floor(i.work / (st.galaxy?.industry_catalog?.outpost_build_secs ?? 60)*100)}%`
    : `Fuel harvesting → ${sysName(st,i.system)} · ${i.status} · ${amount(i.harvested)} collected`;
  return `<div class="industry-status"><b>${esc(text)}</b><small>Latest fleet report</small></div>`;
}
export function handleIndustryInput(el: HTMLInputElement | HTMLSelectElement, st: ViewState): boolean {
  const name = el.dataset.industryInput;
  if (!name) return false;
  const v = ui(st), n = Math.max(0, Number(el.value) || 0), stop = v.route.stops[Number(el.dataset.stop)];
  if (name === "name") v.route.name = el.value;
  else if (name === "reserve") v.route.fuel_reserve = n;
  else if (name === "escort") v.route.escort = el.value || null;
  else if (name === "repeat" && el instanceof HTMLInputElement) v.route.repeat = el.checked;
  else if (name === "port" && stop) { stop.port = el.value === "hub" ? {kind:"hub"} : {kind:"system",id:el.value}; if (stop.port.kind !== "hub") stop.sell = false; }
  else if (name === "sell" && stop && el instanceof HTMLInputElement) stop.sell = el.checked;
  else if (name === "manifest" && stop && (el.dataset.side === "load" || el.dataset.side === "unload")) stop[el.dataset.side][el.dataset.good as Commodity] = Math.floor(n);
  else if (name === "qty") v.qty = Math.max(1,Math.floor(n));
  else if (name === "rate") v.rate = Math.max(.1,n);
  else if (name === "body" || name === "outpostBody") v[name] = n;
  else if (["fleet","system","good","output","project","resource","outpostSystem","outpostKind","builder","skimSystem","skimFleet"].includes(name)) (v as unknown as Record<string,unknown>)[name] = el.value;
  v.feedback = "";
  return true;
}
export function handleIndustryAction(el: HTMLElement, ctx: CoreContext): boolean {
  const a = el.dataset.industryAct;
  if (!a) return false;
  const st = ctx.state, v = ui(st), fleet = st.ghosts.find(g => g.own && g.id === v.fleet);
  if (a === "tab") v.tab = el.dataset.tab ?? "routes";
  else if (a === "copy") { const source = st.ghosts.find(g => g.own && g.id === el.dataset.fleet)?.industry; if (source?.kind === "route") { v.route = structuredClone(source.run.route); v.feedback = "Route copied. Choose a Freighter and confirm assignment."; } }
  else if (a === "stop" && el.dataset.fleet) ctx.intent.beginFleetCommand({type:"SetFreightRoute",fleet_id:el.dataset.fleet,route:null});
  else if (a === "add-stop" && v.route.stops.length < 8) v.route.stops.push({port:{kind:"hub"},load:{},unload:{},sell:false});
  else if (a === "remove-stop" && v.route.stops.length > 2) v.route.stops.splice(Number(el.dataset.stop),1);
  else if (a === "add-cargo") { const stop = v.route.stops[Number(el.dataset.stop)]; const side = el.dataset.side; if (stop && (side === "load" || side === "unload")) stop[side][v.good] = v.qty; }
  else if (a === "assign" && fleet) {
    const reason = freightRouteReason(st,fleet,v.route);
    if (reason) { v.feedback = reason; return true; }
    const commands: FleetCommand[] = [{type:"SetFreightRoute",fleet_id:fleet.id,route:structuredClone(v.route)}];
    if (v.route.escort) commands.push({type:"GuardFleet",interceptor_id:v.route.escort,target_id:fleet.id});
    ctx.intent.beginFleetCommand(commands);
  } else if (a === "reserve") { const target = targets.find(t => key(t) === el.dataset.target); if (target) { ctx.send({type:"ReserveProject",system_id:v.system,target,reserve:el.dataset.on === "true"}); v.feedback = "Reservation order sent; awaiting the system report."; } }
  else if (a === "build-project") {
    const s = st.systems.find(s => s.id === v.system), reason = s ? colonyProjectReason(st,s,v.body,v.project,v.resource) : "Choose a colony.";
    if (reason) v.feedback = reason;
    else { ctx.send({type:"StartColonyProject",system_id:v.system,body_id:v.body,project:v.project,commodity:v.resource}); v.feedback = "Project order sent; local workers and materials required."; }
  }
  else if (a === "active") { ctx.send({type:"SetColonyProjectActive",system_id:v.system,project:el.dataset.project as ColonyProjectKind,active:el.dataset.on === "true"}); v.feedback = "Project instruction sent."; }
  else if (a === "deploy") ctx.intent.beginFleetCommand({type:"DeployOutpost",fleet_id:v.builder,system_id:v.outpostSystem,body_id:v.outpostBody,outpost:v.outpostKind,commodity:v.resource});
  else if (a === "skim") ctx.intent.beginFleetCommand({type:"SkimFuel",fleet_id:v.skimFleet,system_id:v.skimSystem});
  return true;
}
