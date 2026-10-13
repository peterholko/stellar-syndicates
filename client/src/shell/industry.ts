import type { CoreContext } from "./types";
import type { ViewState } from "../state";
import type { ColonyProjectKind, Commodity, EntityId, FreightRoute, GhostView, Manifest, OutpostKind, ProjectTarget, RouteStop, SystemStateView } from "../protocol";
import { fleetCargoManifest } from "../protocol";
import { foundingHomeSystemId } from "../core/derive/geo";
import { commodityIcon, icon, ICONS, label, type IconKey } from "../icons";
import { COMMODITIES, MINERAL_DEPOSITS } from "../core/derive/market";
import { fleetCargoCapacity, fleetFuelCapacity, guardCapable, PROSPECT_SECS, shipKindLabel, transitLabel } from "../core/derive/fleet";
import { availableStock, colonyProjectReason, firstShipmentRoute, freightRouteReason, hubQuote, openingImportPlan, productionPlan, projectName,
  routeAboard, routeCruiseSpeed, routeHubEstimate, routeLegSeconds, routePortId, routePortPosition } from "../core/derive/industry";
import { planetArtUrl, planetArtwork, type PlanetArtKind } from "../planetart";
import { HUB_MODEL } from "../hub-model.generated";
import type { FleetCommand } from "../core/fleetorders";
import "../styles/industry.css";

const esc = (s: string): string => s.replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
const act = (a: string, extra = "") => `data-deck-act="industry" data-mobile-act="industry" data-industry-act="${a}" ${extra}`;
const option = (id: string, name: string, current: string) => `<option value="${esc(id)}" ${id === current ? "selected" : ""}>${esc(name)}</option>`;
const field = (key: string, extra = "") => `data-industry-input="${key}" ${extra}`;
const amount = (n: number) => Number(n.toFixed(1)).toLocaleString();
const goods = (manifest: Manifest) => Object.entries(manifest).filter(([,n]) => n! > 0).map(([c,n]) => `${amount(n!)} ${label(c)}`).join(" · ") || "None";
const listed = (m: Manifest) => (Object.entries(m) as [Commodity, number | undefined][]).filter(([,n]) => (n ?? 0) > 0) as [Commodity, number][];
const total = (m: Manifest) => listed(m).reduce((n, [,units]) => n + units, 0);
const cr = (n: number) => `${Math.round(n).toLocaleString()} cr`;
const dur = (secs: number) => {
  const s = Math.max(0, Math.round(secs));
  return s >= 3600 ? `${Math.floor(s / 3600)}h ${Math.floor(s % 3600 / 60)}m` : s >= 60 ? `${Math.floor(s / 60)}m${s % 60 ? ` ${s % 60}s` : ""}` : `${s}s`;
};
const projectKinds: ColonyProjectKind[] = ["orbital_assembly", "agricultural_export", "deep_extraction"];
const targets: ProjectTarget[] = [{ kind: "cruiser" }, { kind: "academy" }, { kind: "colony" }, ...projectKinds.map(project => ({ kind: "development" as const, project }))];
const key = (t: ProjectTarget) => t.kind === "development" ? t.project : t.kind;
const fleetName = (g: GhostView) => `${shipKindLabel(g.kind)} · ${g.id}`;
const TABS: [string, string][] = [["routes", "Routes"], ["planner", "Production"], ["projects", "Projects"], ["harvesting", "Harvesting"], ["standing", "Standing rules"]];
type Side = "load" | "unload";
type Ui = { owner: string | null; tab: string; fleet: string; system: string; route: FreightRoute; good: Commodity; qty: number;
  output: Commodity; rate: number; project: ColonyProjectKind; body: number; resource: Commodity;
  outpostSystem: string; outpostBody: number; outpostKind: OutpostKind; builder: string; skimSystem: string; skimFleet: string; feedback: string;
  /** §logistics: the route editor is open over the selected Freighter's plan or status. */
  editing: boolean; advanced: boolean; picker: { stop: number; side: Side } | null;
  /** A staged route order: once its pending report is seen and then clears, the editor closes. */
  sent: { fleet: string; seen: boolean } | null;
  /** The first shipment's return basket once the player changes it, and its Hub spending cap. */
  basket: Manifest | null; cap: number;
  /** §fuel-overhaul: the depot rule being composed. */
  depotSystem: string; depotFuel: Commodity; depotTarget: number };
const views = new WeakMap<ViewState, Ui>();
function ui(st: ViewState): Ui {
  let v = views.get(st);
  if (!v || v.owner !== st.playerId) {
    v = { owner: st.playerId, tab: "routes", fleet: "", system: "", route: supplyDraft(st),
      good: "metallic_ore", qty: 25, output: "alloys", rate: 10, project: "agricultural_export", body: 0, resource: "metallic_ore",
      outpostSystem: "", outpostBody: 0, outpostKind: "extraction", builder: "", skimSystem: "", skimFleet: "", feedback: "",
      editing: false, advanced: false, picker: null, sent: null, basket: null, cap: 3000,
      depotSystem: "", depotFuel: "deuterium", depotTarget: 50 };
    const home = st.systems.find(s => s.owner === st.playerId);
    if (home) v.system = home.id;
    views.set(st, v);
  }
  return v;
}
/** A new route starts as a Slow (Uranium) circuit between home and the Hub. */
function supplyDraft(st: ViewState): FreightRoute {
  const home = st.systems.find(s => s.owner === st.playerId);
  // Fusion Freight makes full-speed freight as cheap as Slow.
  const fusionFreight = !!st.research?.programmes.some(p => p.id === "prop_line_fusion_freight" && p.state === "completed");
  return { name: "Supply circuit", fuel_reserve: 10, escort: null, repeat: true, transit: fusionFreight ? "full" : "slow",
    stops: [{ port: home ? { kind: "system", id: home.id } : { kind: "hub" }, load: {}, unload: {}, sell: false }, { port: { kind: "hub" }, load: {}, unload: {}, sell: false }] };
}
export const industryUiSignature = (st: ViewState) => ui(st);
/** The served facts the Logistics panel reads beyond fleets and systems: the
 * charter step, Hub prices, route orders in flight, and a two-second clock
 * while a route is under way, so its ETA and leg progress keep moving. */
export const industryServedSignature = (st: ViewState) => [st.founding, st.market?.prices, [...(st.pendingOrders?.entries() ?? [])], st.depotRules,
  st.ghosts.some(g => g.own && g.industry?.kind === "route" && g.industry.run.phase !== "complete") ? Math.floor(st.simTime / 2) : 0];
export const showStandingIndustry = (st: ViewState): void => { ui(st).tab = "standing"; };
/** Open the Routes tab on one Freighter: its trip, plan or editor. */
export function focusIndustryFleet(st: ViewState, id: string): void {
  const v = ui(st);
  Object.assign(v, { tab: "routes", fleet: id, editing: false, picker: null });
}
export const industryStandingVisible = (st: ViewState): boolean => ui(st).tab === "standing";
const sysName = (st: ViewState, id: string) => st.galaxy?.systems.find(s => s.id === id)?.name ?? id;
/** §belts: a station port reads as its belt ("Veles Belt station"). */
const stationName = (st: ViewState, id: string) => {
  const a = st.ghosts.find(g => g.id === id)?.anchor;
  const belt = a ? st.systems.find(s => s.id === a.system)?.bodies.find(b => b.id === a.body)?.name : undefined;
  return belt ? `${belt} station` : `Station ${id}`;
};
const portName = (st: ViewState, port: RouteStop["port"]) =>
  port.kind === "hub" ? "Market Hub" : port.kind === "station" ? stationName(st, port.id) : sysName(st, port.id);

/** Shared desktop/mobile UI. Draft edits never mutate served fleet/site facts;
 * every fleet command stages the standard confirmation, and all progress below
 * is received light. A View refresh cannot erase an unfinished manifest. */
export function industryHtml(st: ViewState): string {
  const v = ui(st);
  const tabs = `<nav class="lx-tabs" aria-label="Logistics sections">${TABS.map(([t, name]) =>
    `<button type="button" ${act("tab", `data-tab="${t}"`)} aria-pressed="${v.tab === t}">${name}</button>`).join("")}</nav>`;
  const body = v.tab === "routes" ? routesHtml(st, v) : v.tab === "planner" ? plannerHtml(st, v) : v.tab === "projects" ? projectsHtml(st, v)
    : v.tab === "harvesting" ? skimmingHtml(st, v) : v.tab === "standing" ? depotHtml(st, v) : "";
  return `<div class="lx" data-lx-tab="${v.tab}">${tabs}${v.feedback ? `<p class="lx-feedback" role="status">${esc(v.feedback)}</p>` : ""}${body}</div>`;
}

// ─── Routes: fleets on the left, the selected Freighter's plan, trip or editor ───

const cargoFleets = (st: ViewState) => st.ghosts.filter(g => g.own && !g.tca && fleetCargoCapacity(g) > 0);
const aboard = (g: GhostView): Manifest => Object.fromEntries(fleetCargoManifest(g).map(c => [c.commodity, c.units]));
function homeSystem(st: ViewState): SystemStateView | undefined {
  return st.systems.find(s => s.owner === st.playerId && s.id === foundingHomeSystemId()) ?? st.systems.find(s => s.owner === st.playerId);
}
/** §charter v3: Landfall's guarded shipment, until its departure is reported. */
const SHIPMENT_STAGES = ["build_mine", "local_recovery", "prepare_export", "export_production", "build_convoy"];
const firstShipmentPending = (st: ViewState) => !!st.founding?.local_opening && !st.founding.export_dispatched && SHIPMENT_STAGES.includes(st.founding.stage);
/** A route (or other configure) order on its way to this fleet. */
const routeOrderPending = (st: ViewState, id: string) => !!st.pendingOrders?.get(id)?.some(o => o.kind === "configure" && !o.configuration && !o.lost);
/** Ship kinds read the same in every list; a second hull of a kind is numbered. */
function fleetNames(st: ViewState): Map<string, string> {
  const seen = new Map<string, number>(), names = new Map<string, string>();
  for (const g of st.ghosts.filter(g => g.own && !g.tca).sort((a, b) => a.id.localeCompare(b.id, undefined, { numeric: true }))) {
    const base = shipKindLabel(g.kind), n = (seen.get(base) ?? 0) + 1;
    seen.set(base, n);
    names.set(g.id, n > 1 ? `${base} ${n}` : base);
  }
  return names;
}
const hullIcon = (g: GhostView) => icon((g.kind in ICONS ? g.kind : "fleet") as IconKey, "md", shipKindLabel(g.kind));
function dockName(st: ViewState, g: GhostView): string | null {
  if (!g.docked) return null;
  if (g.docked === "hub") return "Market Hub";
  const system = st.galaxy?.systems.find(s => g.docked === s.id || g.docked === `E${s.id}`);
  return system?.name ?? null;
}
/** A port's picture: the Hub station, or a system's settled (else first) world. */
function portArt(st: ViewState, port: RouteStop["port"]): string {
  if (port.kind === "hub") return `<img class="lx-port__art" src="${HUB_MODEL.hero}" alt="">`;
  if (port.kind === "station") return `<span class="lx-port__art">${icon("mining_belt_station", "md")}</span>`;
  const bodies = st.systems.find(s => s.id === port.id)?.bodies ?? [];
  const world = bodies.find(b => b.habitable) ?? bodies[0];
  // §marginal: as in the System View, only a settleable world wears the lush art.
  const kind = (world?.kind === "terrestrial" ? (world.habitable ? "terrestrial" : "marginal")
    : world?.kind === "ocean" ? (world.habitable ? "ocean" : "marginal_ocean") : world?.kind) as PlanetArtKind | undefined;
  return kind && planetArtwork(kind) ? `<img class="lx-port__art" src="${planetArtUrl(kind, 96)}" alt="">` : `<span class="lx-port__art" aria-hidden="true"></span>`;
}
function defaultEscort(st: ViewState, freighter: GhostView): GhostView | undefined {
  const escorts = st.ghosts.filter(g => g.id !== freighter.id && guardCapable(g));
  return escorts.find(g => g.guard_target === freighter.id) ?? escorts.find(g => !!g.docked && g.docked === freighter.docked) ?? escorts[0];
}
function fleetState(st: ViewState, g: GhostView): string {
  if (routeOrderPending(st, g.id)) return `<span class="is-info">Route order sent</span>`;
  const i = g.industry;
  if (i?.kind === "route" && i.run.phase !== "complete") return i.run.hold ? `<span class="is-warn">${esc(i.run.hold)}</span>`
    : `<span class="is-good">On route · stop ${i.run.stop + 1}/${i.run.route.stops.length}</span>`;
  if (i?.kind === "skim") return "Harvesting Helium-3";
  if (i?.kind === "mine") return "Mining a belt";
  if (i?.kind === "prospect") return "Prospecting a belt";
  if (i?.kind === "deploy") return "Building an outpost";
  const dock = dockName(st, g);
  return dock ? `Docked at ${esc(dock)}` : "In flight";
}

function routesHtml(st: ViewState, v: Ui): string {
  const fleets = cargoFleets(st), names = fleetNames(st), home = homeSystem(st);
  const opening = firstShipmentPending(st) && !!home;
  if (!fleets.some(g => g.id === v.fleet)) v.fleet = (opening ? fleets.find(g => !g.industry) : undefined)?.id ?? fleets[0]?.id ?? "";
  const fleet = fleets.find(g => g.id === v.fleet);
  const run = fleet?.industry?.kind === "route" ? fleet.industry.run : null;
  // A confirmed edit closes the editor once its order has been seen and answered.
  if (v.sent && fleet && v.sent.fleet === fleet.id) {
    const pending = routeOrderPending(st, fleet.id);
    if (pending) v.sent.seen = true;
    else if (v.sent.seen) { v.sent = null; v.editing = false; v.picker = null; }
  }
  const main = !fleet ? `<div class="lx-empty"><b>No Freighter yet</b><p>Build a Freighter at a Shipyard to run cargo routes.</p></div>`
    : v.editing || (!run && !opening) ? editorHtml(st, v, fleet, names, !v.editing)
    : run ? statusHtml(st, fleet, run, names)
    : planHtml(st, v, fleet, home!, names);
  return `<div class="lx-routes">${railHtml(st, v, fleets, names)}<section class="lx-main" aria-label="Route">${main}</section></div>`;
}

function railHtml(st: ViewState, v: Ui, fleets: GhostView[], names: Map<string, string>): string {
  const escorts = st.ghosts.filter(g => guardCapable(g) && !fleets.includes(g));
  const freighters = fleets.map(g => `<button type="button" class="lx-rail__item" ${act("pick-fleet", `data-fleet="${esc(g.id)}"`)} aria-pressed="${g.id === v.fleet}">${hullIcon(g)}<span><b>${esc(names.get(g.id) ?? shipKindLabel(g.kind))}</b><small>${fleetState(st, g)}</small><small>Hold ${amount(total(aboard(g)))} / ${amount(fleetCargoCapacity(g))}</small></span></button>`).join("");
  const guards = escorts.map(g => {
    const target = g.guard_target ? names.get(g.guard_target) : null;
    const dock = dockName(st, g);
    return `<div class="lx-rail__item is-static">${hullIcon(g)}<span><b>${esc(names.get(g.id) ?? shipKindLabel(g.kind))}</b><small>${target ? `Guarding the ${esc(target)}` : dock ? `Docked at ${esc(dock)}` : "In flight"}</small></span></div>`;
  }).join("");
  return `<aside class="lx-rail" aria-label="Freighters and escorts"><h3>Freighters</h3>${freighters || `<p class="lx-none">None yet</p>`}`
    + `${guards ? `<h3>Escorts</h3>${guards}` : ""}`
    + `${fleets.length ? `<button type="button" class="lx-rail__new" ${act("new-route")}>+ New route</button>` : ""}</aside>`;
}

/** One stop's work in a line: "sell 50 Ferrite Ore, then buy 14 Electronics". */
function stopSummary(stop: RouteStop, route: FreightRoute): string {
  const list = (m: Manifest) => { const l = listed(m); return l.length <= 2 ? l.map(([c, n]) => `${amount(n)} ${label(c)}`).join(" and ") : `${amount(total(m))} units of ${l.length} goods`; };
  const hub = stop.port.kind === "hub", parts: string[] = [];
  if (listed(stop.unload).length) parts.push(`${hub && stop.sell ? "sell" : "unload"} ${list(stop.unload)}`);
  if (listed(stop.load).length) parts.push(`${hub && route.shopping_budget != null ? "buy" : "load"} ${list(stop.load)}`);
  return parts.join(", then ") || "pass through";
}
function stopVerb(stop: RouteStop, route: FreightRoute): string {
  const u = listed(stop.unload).length > 0, l = listed(stop.load).length > 0, hub = stop.port.kind === "hub";
  const off = hub && stop.sell ? "Sell" : "Unload", on = hub && route.shopping_budget != null ? "buy" : "load";
  return u && l ? `${off}, then ${on}` : u ? off : l ? on.charAt(0).toUpperCase() + on.slice(1) : "Visit";
}
function stopLines(st: ViewState, stop: RouteStop, route: FreightRoute): string {
  const row = (img: string, text: string, tail = "") => `<div class="lx-do">${img}<span>${text}</span>${tail}</div>`;
  const hub = stop.port.kind === "hub", lines: string[] = [];
  const both = listed(stop.unload).length > 0 && listed(stop.load).length > 0;
  for (const [c, n] of listed(stop.unload)) {
    if (!hub) { lines.push(row(commodityIcon(c), `${both ? "Unload " : ""}${esc(label(c))}`, `<em>${amount(n)}</em>`)); continue; }
    const value = stop.sell ? hubQuote(st, c, n, "sell") : null;
    lines.push(row(commodityIcon(c), `${stop.sell ? "Sell" : "Unload"} ${amount(n)} ${esc(label(c))}`, value != null ? `<em class="is-good">+${Math.round(value).toLocaleString()}</em>` : ""));
  }
  const buys = listed(stop.load);
  if (hub && route.shopping_budget != null && buys.length) {
    const cost = buys.reduce((sum, [c, n]) => sum + (hubQuote(st, c, n, "buy") ?? 0), 0);
    lines.push(row(icon("credits", "sm"), `Buy ${buys.length} good${buys.length === 1 ? "" : "s"}`, st.market?.prices.length ? `<em class="is-warn">−${Math.round(cost).toLocaleString()}</em>` : ""));
  } else for (const [c, n] of buys) lines.push(row(commodityIcon(c), `${both ? "Load " : ""}${esc(label(c))}`, `<em>${amount(n)}</em>`));
  return lines.join("") || `<p class="lx-none">Passes through</p>`;
}
type Live = { stop: number; moving: boolean; into: number; complete: boolean };
/** Stops in a row with the legs between them; `live` marks a trip under way. */
function itineraryHtml(st: ViewState, route: FreightRoute, legs: (number | null)[], live: Live | null, notes: Record<number, string> = {}): string {
  const cols = route.stops.map((s, i) => `${i ? "84px " : ""}${s.port.kind === "hub" ? "minmax(170px, 1.25fr)" : "minmax(150px, 1fr)"}`).join(" ");
  return `<div class="lx-route" style="grid-template-columns:${cols}" aria-label="Itinerary">${route.stops.map((stop, i) => {
    let leg = "";
    if (i) {
      const k = i - 1, secs = legs[k];
      const p = !live ? 0 : live.complete || k < live.stop - 1 ? 1 : k === live.stop - 1 ? (live.moving ? live.into : 1) : 0;
      leg = `<div class="lx-leg${p > 0 ? " is-moving" : ""}" style="--p:${(p * 100).toFixed(0)}%"><span>${secs == null ? "—" : `≈ ${dur(secs)}`}</span><i></i><span>${transitLabel(route.transit)}</span></div>`;
    }
    const state = !live ? "" : live.complete ? (i === route.stops.length - 1 ? " is-current" : " is-done") : i < live.stop ? " is-done" : i === live.stop ? " is-current" : "";
    return `${leg}<article class="lx-stop${state}"><header class="lx-port">${portArt(st, stop.port)}<div><b>${esc(portName(st, stop.port))}</b><small>${stopVerb(stop, route)}</small></div></header>${stopLines(st, stop, route)}${notes[i] ?? ""}</article>`;
  }).join("")}</div>`;
}

function planHtml(st: ViewState, v: Ui, fleet: GhostView, home: SystemStateView, names: Map<string, string>): string {
  const plan = openingImportPlan(st, home, "transport");
  const basket = v.basket ?? plan.manifest;
  const escort = defaultEscort(st, fleet);
  const route = firstShipmentRoute(home, fleet, escort?.id ?? null, basket, v.cap);
  const hold = fleetCargoCapacity(fleet), held = total(basket);
  const legs = routeLegSeconds(st, fleet, route), trip = legs.every(s => s != null) ? legs.reduce((a, b) => a! + b!, 0)! : null;
  const money = routeHubEstimate(st, route);
  const reason = freightRouteReason(st, fleet, route);
  const pending = routeOrderPending(st, fleet.id);
  const kit = label(plan.key), need = total(plan.deficit);
  const covered = listed(plan.deficit).reduce((n, [c, d]) => n + Math.min(d, basket[c] ?? 0), 0);
  const short = listed(plan.deficit).map(([c, d]) => [c, d - Math.min(d, basket[c] ?? 0)] as [Commodity, number]).filter(([,n]) => n > 0);
  const ore = route.stops[0].load.metallic_ore ?? 0, oreHere = Math.floor(availableStock(home, "metallic_ore"));
  const rows = [...new Set<Commodity>([...listed(plan.deficit).map(([c]) => c), ...listed(basket).map(([c]) => c), "uranium"])];
  const basketRows = rows.map(c => {
    const n = basket[c] ?? 0, price = hubQuote(st, c, n, "buy");
    return `<div class="lx-good">${commodityIcon(c)}<span>${esc(label(c))}${plan.deficit[c] ? ` <small>need ${plan.deficit[c]}</small>` : ""}</span><small>${price != null && n > 0 ? `≈ ${cr(price)}` : ""}</small>`
      + `<span class="lx-stepper"><button type="button" ${act("basket", `data-good="${c}" data-step="-1"`)} aria-label="One less ${esc(label(c))}" ${n <= 0 ? "disabled" : ""}>−</button><b>${n}</b>`
      + `<button type="button" ${act("basket", `data-good="${c}" data-step="1"`)} aria-label="One more ${esc(label(c))}" ${held >= hold ? "disabled" : ""}>+</button></span></div>`;
  }).join("");
  const shortText = short.map(([c, n]) => `${n} ${label(c)}`).join(", "), shortUnits = short.reduce((a, [,n]) => a + n, 0);
  const coverage = !need ? `${esc(sysName(st, home.id))} already holds the ${esc(kit)} materials.`
    : !short.length ? `Covers all ${need} ${esc(kit)} goods you lack.`
    : `Covers <b>${covered}</b> of the ${need} ${esc(kit)} goods you lack. ${held < hold ? `Room for ${hold - held} more: add the last ${shortUnits} (${esc(shortText)}).`
      : `The last ${shortUnits} (${esc(shortText)}) don't fit; buy them at the Market and book Authority freight home. <button type="button" class="lx-link" data-deck-act="logistics-market" data-mobile-act="hub-market">Open Market</button>`}`;
  const overCap = money != null && money.bought > v.cap;
  const net = money ? money.sold - money.bought : 0;
  const notes: Record<number, string> = oreHere < ore ? { 0: `<p class="lx-stop__note is-warn">Waits for ${ore} ore · ${oreHere} in stock</p>` } : {};
  const escortName = escort ? names.get(escort.id) ?? shipKindLabel(escort.kind) : "";
  const fleetDock = dockName(st, fleet);
  return `<div class="lx-lead"><div><span class="lx-label">Charter · step 2</span><h3>Send your first shipment</h3><p>Sell ore at the Market Hub and bring ${esc(kit)} materials home.</p></div><span class="lx-chip is-accent">${escort ? "Escorted" : "Unescorted"} · one trip</span></div>`
    + itineraryHtml(st, route, legs, null, notes)
    + `<div class="lx-split"><section class="lx-box" aria-label="Return basket"><h4>Return basket <small>${held} / ${hold} hold</small></h4><div class="lx-goods">${basketRows}</div>`
    + `<div class="lx-meter${held >= hold ? " is-full" : ""}"><span><span>Hold</span><span>${held} / ${hold}</span></span><i><b style="width:${Math.min(100, held / Math.max(1, hold) * 100).toFixed(0)}%"></b></i></div>`
    + `<p class="lx-note">${coverage}</p></section>`
    + `<section class="lx-box" aria-label="Trip"><h4>Trip</h4><dl class="lx-facts">`
    + `<dt>Freighter</dt><dd>${hullIcon(fleet)}<span>${esc(names.get(fleet.id) ?? shipKindLabel(fleet.kind))}${fleetDock ? ` · ${esc(fleetDock)}` : ""}</span></dd>`
    + `<dt>Escort</dt><dd>${escort ? `${hullIcon(escort)}<span>${esc(escortName)}</span><span class="lx-chip">guards it</span>` : `<span class="is-warn">No escort ready. The charter counts the shipment only with one guarding the Freighter.</span>`}</dd>`
    + `<dt>Spending cap</dt><dd><input type="number" min="0" max="1000000000" step="100" value="${v.cap}" aria-label="Spending cap in credits" ${field("plan-cap")}><small>Skips what it can't afford.</small></dd></dl>`
    + (money ? `<div class="lx-money" aria-label="Expected money"><span>Ore sale</span><span class="is-good">+${cr(money.sold)}</span><span>Basket</span><span class="is-warn">−${cr(money.bought)}</span>`
      + `<span class="lx-money__net">Net</span><span class="lx-money__net ${net >= 0 ? "is-good" : "is-warn"}">${net >= 0 ? "+" : "−"}${cr(Math.abs(net))}</span></div>` : `<p class="lx-note">Prices arrive with the next Hub report.</p>`)
    + `<p class="lx-note">You can log off once it departs. Sales show up when their report arrives.</p></section></div>`
    + `<footer class="lx-actions"><button type="button" ${act("customize")}>Customize route</button><span class="lx-actions__note">${reason ? `<span class="is-warn">${esc(reason)}</span>`
      : `${trip != null ? `≈ ${dur(trip)} round trip · ` : ""}${overCap ? `<span class="is-warn">the basket passes the cap; it skips what it can't afford</span>` : "within the cap"}`}</span>`
    + (pending ? `<button type="button" class="lx-primary" disabled>Order sent · confirming</button>`
      : `<button type="button" class="lx-primary" ${act("send-shipment")} data-coach="send-shipment" ${reason ? "disabled" : ""}>Send shipment</button>`) + `</footer>`;
}

function statusHtml(st: ViewState, fleet: GhostView, run: Extract<NonNullable<GhostView["industry"]>, { kind: "route" }>["run"], names: Map<string, string>): string {
  const route = run.route, stops = route.stops, n = stops.length;
  const complete = run.phase === "complete", moving = run.phase === "travelling" || run.phase === "dispatch";
  const at = stops[Math.min(run.stop, n - 1)], where = portName(st, at.port);
  const target = routePortPosition(st, at.port);
  const from = run.stop > 0 ? routePortPosition(st, stops[run.stop - 1].port) : route.repeat && run.visits > 0 ? routePortPosition(st, stops[n - 1].port) : null;
  const left = target ? Math.hypot(target.x - fleet.pos.x, target.y - fleet.pos.y) : null;
  const span = from && target ? Math.hypot(target.x - from.x, target.y - from.y) : null;
  const into = moving && left != null && span ? Math.max(0, Math.min(1, 1 - left / span)) : moving ? 0 : 1;
  const eta = moving && left != null ? left / routeCruiseSpeed(fleet, route.transit) : null;
  const legs = routeLegSeconds(st, fleet, route);
  const live: Live = { stop: run.stop, moving, into, complete };
  const hubShopping = at.port.kind === "hub" && route.shopping_budget != null;
  const doing = complete ? `Route complete · at ${where}`
    : moving ? `Heading to ${where}${eta != null ? ` · ≈ ${dur(eta)}` : ""}`
    : run.phase === "unloading" ? `${at.port.kind === "hub" && at.sell ? "Selling" : "Unloading"} at ${where}`
    : run.phase === "loading" ? `${hubShopping ? "Buying" : "Loading"} at ${where}`
    : run.phase === "fuel" ? `Refuelling at ${where}` : run.phase === "escort" ? `Waiting for the escort at ${where}` : `Held near ${where}`;
  const legsDone = complete ? n - 1 : Math.max(0, run.stop - (moving ? 1 : 0)) + (moving && run.stop > 0 ? into : 0);
  const progress = complete ? 100 : Math.min(100, legsDone / Math.max(1, route.repeat ? n : n - 1) * 100);
  const remaining = (m?: Manifest) => m && total(m) > 0 ? ` · ${amount(total(m))} left` : "";
  const now = !moving && !complete ? (run.phase === "unloading" ? remaining(run.remaining_unload) : run.phase === "loading" ? remaining(run.remaining_load) : "") : "";
  const list = stops.map((stop, i) => {
    const state = complete || i < run.stop ? "Done" : i === run.stop ? (moving ? "Next" : "Now") : "Later";
    return `<li class="is-${state.toLowerCase()}"><span>${state}</span><p><b>${esc(portName(st, stop.port))}</b> · ${esc(stopSummary(stop, route))}${i === run.stop && !complete ? esc(now) : ""}</p></li>`;
  }).join("");
  const escort = route.escort ? names.get(route.escort) ?? "escort" : null;
  const pending = routeOrderPending(st, fleet.id);
  return `<div class="lx-lead"><div><span class="lx-label">${route.name === "First shipment" ? "Charter · step 2" : "Route"}</span><h3>${esc(route.name)}${complete ? " · complete" : ""}</h3><p>${esc(doing)}</p></div>`
    + `<span class="lx-chip is-accent">${escort ? `Escorted by the ${esc(escort)}` : "Unescorted"} · ${route.repeat ? "repeats" : "one trip"}</span></div>`
    + (pending ? `<p class="lx-banner" role="status">Route order sent. The Freighter acts on it when the order arrives.</p>` : "")
    + itineraryHtml(st, route, legs, live)
    + `<section class="lx-status${run.hold ? " is-held" : complete ? " is-complete" : ""}" aria-label="Trip status"><p><b>${esc(run.hold ?? doing)}</b> · as reported</p>`
    + `<div class="lx-progress"><b style="width:${progress.toFixed(1)}%"></b></div>`
    + `<p class="lx-note">${complete ? "The Freighter is free for a new route." : route.shopping_budget != null && stops.some(s => s.port.kind === "hub") ? `Purchases stay within ${cr(route.shopping_budget)} per Hub visit. Sales and purchases show up when their reports arrive.` : "Cargo moves show up when their reports arrive."}</p></section>`
    + `<section class="lx-box" aria-label="Stops"><h4>Stops <small>as reported</small></h4><ol class="lx-log">${list}</ol></section>`
    + `<footer class="lx-actions"><span class="lx-actions__note">Orders to the Freighter travel at light speed; changes apply when they arrive.</span>`
    + `<button type="button" ${act("edit-route")}>Edit route</button>`
    + (complete ? `<button type="button" class="lx-primary" ${act("run-again")}>Run again…</button>` : `<button type="button" ${act("stop", `data-fleet="${esc(fleet.id)}"`)}>Stop route…</button>`)
    + `</footer>`;
}

function goodsOptions(st: ViewState, stop: RouteStop, current: Commodity): string {
  const system = stop.port.kind === "system" ? st.systems.find(s => stop.port.kind === "system" && s.id === stop.port.id) : undefined;
  // §belts: a station stop lists what its hold carries.
  const station = stop.port.kind === "station" ? st.ghosts.find(g => stop.port.kind === "station" && g.id === stop.port.id) : undefined;
  const held = station ? fleetCargoManifest(station) : [];
  const here = (c: Commodity) => station ? held.find(h => h.commodity === c)?.units ?? 0 : system ? Math.floor(availableStock(system, c)) : 0;
  const price = (c: Commodity) => st.market?.prices.find(p => p.commodity === c)?.price;
  return [...COMMODITIES].sort((a, b) => Number(here(b) > 0) - Number(here(a) > 0)).map(c => {
    const hint = system ? (here(c) > 0 ? ` · ${here(c)} here` : "") : price(c) != null ? ` · ${Math.round(price(c)!)} cr` : "";
    return option(c, `${label(c)}${hint}`, current);
  }).join("");
}

function editorHtml(st: ViewState, v: Ui, fleet: GhostView, names: Map<string, string>, standalone: boolean): string {
  const route = v.route, cap = fleetCargoCapacity(fleet), n = route.stops.length;
  const after = routeAboard(route, aboard(fleet));
  const reason = freightRouteReason(st, fleet, route);
  // Hub shopping only buys what fits; anywhere else an overfull load stalls the Freighter.
  const overflow = after.findIndex((units, i) => units > cap && !(route.stops[i].port.kind === "hub" && route.shopping_budget != null));
  const legs = routeLegSeconds(st, fleet, route), run = legs.every(s => s != null) ? legs.reduce((a, b) => a! + b!, 0)! : null;
  const money = routeHubEstimate(st, route);
  const maxStops = st.galaxy?.industry_catalog?.max_stops ?? 8;
  const shopping = route.shopping_budget != null;
  const stations = st.ghosts.filter(g => g.own && (g.kind === "mining_belt_station" || g.composition?.some(c => c.kind === "mining_belt_station")));
  const ports = (current: string) => option("hub", "Market Hub", current) + st.systems.filter(s => s.owner === st.playerId).map(s => option(s.id, sysName(st, s.id), current)).join("")
    + stations.map(g => option(`station:${g.id}`, stationName(st, g.id), current)).join("");
  const rows = (m: Manifest, stop: number, side: Side) => {
    const entries = Object.entries(m) as [Commodity, number][];
    return entries.length ? `<div class="lx-cargo">${entries.map(([c, units]) => `<div class="lx-cargo__row">${commodityIcon(c)}<span>${esc(label(c))}</span>`
      + `<input type="number" min="0" max="${cap}" value="${units}" aria-label="${side === "load" ? "Load" : "Unload"} ${esc(label(c))} at stop ${stop + 1}" ${field("manifest", `data-stop="${stop}" data-side="${side}" data-good="${c}"`)}>`
      + `<button type="button" class="lx-x" ${act("drop-cargo", `data-stop="${stop}" data-side="${side}" data-good="${c}"`)} aria-label="Remove ${esc(label(c))}">×</button></div>`).join("")}</div>`
      : `<p class="lx-none">Nothing</p>`;
  };
  const picker = (stop: number, side: Side) => v.picker?.stop === stop && v.picker.side === side
    ? `<div class="lx-picker" role="group" aria-label="Add cargo"><select aria-label="Commodity" ${field("good")}>${goodsOptions(st, route.stops[stop], v.good)}</select>`
      + `<input type="number" min="1" max="${cap}" value="${v.qty}" aria-label="Units" ${field("qty")}><button type="button" class="lx-primary" ${act("add-cargo", `data-stop="${stop}" data-side="${side}"`)}>Add</button>`
      + `<button type="button" class="lx-quiet" ${act("close-pick")}>Cancel</button></div>`
    : `<button type="button" class="lx-add" ${act("open-pick", `data-stop="${stop}" data-side="${side}"`)}>+ Add</button>`;
  const columns = route.stops.map((stop, i) => {
    const hub = stop.port.kind === "hub", station = stop.port.kind === "station";
    return `<article class="lx-estop${after[i] > cap ? " is-over" : ""}" aria-label="Stop ${i + 1}"><header><span class="lx-estop__n">${i + 1}</span>`
      + `<select aria-label="Stop ${i + 1} port" ${field("port", `data-stop="${i}"`)}>${ports(routePortId(stop.port))}</select>`
      + `${n > 2 ? `<button type="button" class="lx-x" ${act("remove-stop", `data-stop="${i}"`)} aria-label="Remove stop ${i + 1}">×</button>` : ""}</header>`
      // §belts: a station stop only loads.
      + (station ? "" : `<h5>${hub && stop.sell ? "Unload and sell" : "Unload"}</h5>${rows(stop.unload, i, "unload")}${picker(i, "unload")}`)
      + `<h5>${hub ? (shopping ? "Buy and load" : "Load from the warehouse") : station ? "Load from the station" : "Load"}</h5>${rows(stop.load, i, "load")}${picker(i, "load")}`
      + `${hub ? `<label class="lx-check"><input type="checkbox" ${field("sell", `data-stop="${i}"`)} ${stop.sell ? "checked" : ""}> Sell what it unloads here</label>` : ""}`
      + `<p class="lx-aboard"><span>Aboard after</span><b class="${after[i] > cap ? "is-warn" : ""}">${amount(after[i])} / ${amount(cap)}</b></p></article>`;
  }).join("");
  const escorts = st.ghosts.filter(g => g.id !== fleet.id && guardCapable(g));
  const settings = `<section class="lx-settings" aria-label="Route settings">`
    + `<label>Freighter<select ${field("fleet")}>${cargoFleets(st).map(g => option(g.id, names.get(g.id) ?? shipKindLabel(g.kind), v.fleet)).join("")}</select></label>`
    + `<label>Escort<select ${field("escort")}>${option("", "None", route.escort ?? "")}${escorts.map(g => option(g.id, names.get(g.id) ?? shipKindLabel(g.kind), route.escort ?? "")).join("")}</select></label>`
    + `<label class="lx-check"><input type="checkbox" ${field("shopping")} ${shopping ? "checked" : ""}> Buy missing goods at the Hub</label>`
    + (shopping ? `<label title="Credits it may spend at each Hub visit, fees included">Hub spending cap<input type="number" min="0" max="1000000000" step="100" value="${route.shopping_budget}" ${field("budget")}></label>` : "")
    + `<label class="lx-check"><input type="checkbox" ${field("repeat")} ${route.repeat ? "checked" : ""}> Repeat the circuit</label>`
    + `<button type="button" class="lx-quiet lx-settings__more-toggle" ${act("advanced")} aria-expanded="${v.advanced}">Advanced ${v.advanced ? "▴" : "▾"}</button>`
    + (v.advanced ? `<div class="lx-settings__more"><label>Cruise<select ${field("transit")}>${option("slow", "Slow · ½ speed · Uranium", route.transit ?? "full")}${option("full", "Full speed · Deuterium", route.transit ?? "full")}${option("stealth", "Stealth · Deuterium", route.transit ?? "full")}</select></label>`
      + `<label>${route.transit === "slow" ? "Uranium" : "Deuterium"} kept on arrival<input type="number" min="0" max="${fleetFuelCapacity(fleet)}" value="${route.fuel_reserve}" ${field("reserve")}></label>`
      + `<label>Route name<input maxlength="48" value="${esc(route.name)}" ${field("name")}></label></div>` : "")
    + `</section>`;
  const note = reason ? `<span class="is-warn">${esc(reason)}</span>`
    : overflow >= 0 ? `<span class="is-warn">After stop ${overflow + 1} the hold would carry ${amount(after[overflow])} of ${amount(cap)}. Unload more first.</span>`
    : [run != null ? `≈ ${dur(run)} per run` : "", money && money.sold > 0 ? `sells ≈ ${cr(money.sold)}` : "", money && money.bought > 0 ? `buys ≈ ${cr(money.bought)}` : ""].filter(Boolean).join(" · ")
      || "Departs when the cargo, fuel and escort are ready.";
  return `<div class="lx-lead"><div><span class="lx-label">Route editor</span><h3>${esc(route.name || "Route")}</h3><p>${route.stops.map(s => esc(portName(st, s.port))).join(" → ")} · ${route.repeat ? "repeats" : "one trip"}</p></div>`
    + `${standalone ? "" : `<button type="button" class="lx-quiet" ${act("close-editor")}>← Back</button>`}</div>`
    + settings
    + `<div class="lx-stops" style="grid-template-columns:repeat(${n}, minmax(210px, 1fr)) 64px">${columns}<button type="button" class="lx-addstop" ${act("add-stop")} ${n >= maxStops ? "disabled" : ""}><b>+</b>Stop</button></div>`
    + `<footer class="lx-actions"><span class="lx-actions__note">${note}</span>${standalone ? "" : `<button type="button" ${act("close-editor")}>Cancel</button>`}`
    + `<button type="button" class="lx-primary" ${act("assign")} data-coach="assign-route" ${reason || overflow >= 0 ? "disabled" : ""}>Assign route…</button></footer>`;
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
      <small>Usable input stock ${amount(i.stock)}${i.refiningFloor ? ` · keeps ${amount(i.refiningFloor)} for exports/fleets` : ""}${i.runway === null ? "" : ` · covers ~${amount(i.runway)} min`}</small>
      <div>${i.suppliers.map(x => `<p><b>${esc(x.name)}</b> · ${amount(x.surplus)}/min surplus · ${amount(x.stock)} stock${x.deposit ? " · local deposit" : ""}</p>`).join("") || `<p>No reported supplier. Survey another system or import from the Market.</p>`}</div></article>`).join("")}
      <section class="industry-card"><h4>Smelter reserve policy</h4><p>Refine surplus only. Keep ore for exports; the floor does not block loading your Freighters.</p><small>Received floor: ${s.industry?.refining_reserve?.metallic_ore ?? 0} of each ore</small><div class="industry-actions"><button type="button" ${act("refining-floor", 'data-on="true"')}>Keep 50 ore</button><button type="button" ${act("refining-floor", 'data-on="false"')}>Use all available ore</button></div></section></section>`;
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
  const descriptions = { orbital_assembly: "Shipyard II · Fusion Reactors · Composites + Precision Components + Iridium → Hull Sections and Warp Cores.",
    agricultural_export: "Habitable Biomass world · Biomass + Polymers → bulk Provisions.",
    deep_extraction: "Mineral deposit · Machinery → deep reserves of one chosen mineral." };
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
    <small>Upkeep / min: 1.8 Provisions · 0.3 Machinery. Extraction stops without supplies or under blockade. Research outposts support your active programme.</small>
    <div class="industry-grid"><label>Surveyed system<select ${field("outpostSystem")}>${sites.map(s => option(s.id,sysName(st,s.id),v.outpostSystem)).join("")}</select></label>
    <label>World<select ${field("outpostBody")}>${site?.bodies.map(b => option(String(b.id),b.name,String(v.outpostBody))).join("") ?? ""}</select></label>
    <label>Type<select ${field("outpostKind")}>${option("extraction","Extraction",v.outpostKind)}${option("research","Research",v.outpostKind)}</select></label>
    <label>Freighter<select ${field("builder")}>${fleets.map(g => option(g.id,fleetName(g),v.builder)).join("")}</select></label>
    ${v.outpostKind === "extraction" ? resourceSelect(v.resource,"resource",[...MINERAL_DEPOSITS,"volatiles","biomass"]) : ""}</div>
    <button type="button" ${act("deploy")} ${site && ready && suitable && expanded ? "" : "disabled"}>Establish outpost…</button><small>${!expanded ? "Complete the founding surveys to unlock expansion." : !sites.length ? "Survey an uninhabitable system first." : !ready ? "Load the deployment materials aboard your Freighter first." : !suitable ? "Select a resource present on this world." : "Bring operating supplies and protection."}</small></section>`;
}
/** §fuel-overhaul: the goods a depot rule may stock (sim industry::is_depot_fuel). */
const DEPOT_FUELS: Commodity[] = ["uranium", "deuterium", "helium3", "antimatter"];
const DEPOT_MAX = 2_000;
/** Fuel depots: "Keep N fuel at a port". The Authority reviews each rule at the
 * Hub on its own delayed picture of the port and books the shortfall on its
 * next freight departure, billing the Hub price plus the freight fee. */
function depotHtml(st: ViewState, v: Ui): string {
  const sites = st.systems.filter(s => s.owner === st.playerId);
  if (!sites.some(s => s.id === v.depotSystem)) v.depotSystem = sites[0]?.id ?? "";
  const stock = (system: string, c: Commodity) => Math.floor(st.systems.find(s => s.id === system)?.stockpile?.find(x => x.commodity === c)?.units ?? 0);
  const rows = (st.depotRules ?? []).map(r => `<article class="industry-card"><header><b>${commodityIcon(r.commodity)} ${esc(label(r.commodity))} · ${esc(sysName(st, r.system))}</b>`
    + `<button type="button" ${act("depot-clear", `data-system="${esc(r.system)}" data-good="${r.commodity}"`)}>Remove</button></header>`
    + `<small>Keep ${r.target.toLocaleString()} · stock ${stock(r.system, r.commodity).toLocaleString()}</small></article>`).join("");
  return `<section class="industry-panel"><h3>Fuel depots</h3><p>Authority freight tops each port up to its target.</p>${rows}`
    + `<div class="industry-grid"><label>Port<select ${field("depotSystem")}>${sites.map(s => option(s.id, sysName(st, s.id), v.depotSystem)).join("")}</select></label>`
    + `<label>Fuel<select ${field("depotFuel")}>${DEPOT_FUELS.map(c => option(c, label(c), v.depotFuel)).join("")}</select></label>`
    + `<label>Keep<input type="number" min="0" max="${DEPOT_MAX}" step="10" value="${v.depotTarget}" ${field("depotTarget")}></label></div>`
    + `<button type="button" ${act("depot-set")} ${v.depotSystem ? "" : "disabled"}>${v.depotTarget > 0 ? "Keep stocked" : "Clear rule"}</button>`
    + `<small title="Billed at the Hub price plus the freight fee. Light delay, storage room and raids apply; blockaded ports get nothing.">Hub price + freight fee · 0 clears</small></section>`;
}
function skimmingHtml(st: ViewState,v: Ui): string {
  const sites = st.systems.filter(s => s.bodies.some(b => b.kind === "gas_giant"));
  const fleets = st.ghosts.filter(g => g.own && !g.tca);
  if (!sites.some(s => s.id === v.skimSystem)) v.skimSystem = sites[0]?.id ?? "";
  if (!fleets.some(g => g.id === v.skimFleet)) v.skimFleet = fleets[0]?.id ?? "";
  const unlocked = st.research?.programmes.some(p => p.id === "prop_expedition_v_ramscoop" && p.state === "completed");
  return `<section class="industry-panel"><h3>Gas-giant Helium-3 harvesting</h3><p>Tops up fusion tanks after Fusion Reactors; Freighters collect the rest as cargo.</p>
    <div class="industry-grid"><label>Fleet<select ${field("skimFleet")}>${fleets.map(g => option(g.id,fleetName(g),v.skimFleet)).join("")}</select></label>
    <label>Gas-giant system<select ${field("skimSystem")}>${sites.map(s => option(s.id,sysName(st,s.id),v.skimSystem)).join("")}</select></label></div>
    <button type="button" ${act("skim")} ${unlocked && v.skimFleet && v.skimSystem ? "" : "disabled"}>Harvest Helium-3…</button>
    <small>${unlocked ? `${amount((st.galaxy?.industry_catalog?.skim_helium3_per_s ?? 1 / 6) * 60)} Helium-3/min on station; pauses near combat or blockades.` : "Requires Ramscoop Skimming research."}</small></section>`;
}
/** §belts: a barge job's status, as the fleet line reads it. */
const MINE_STATUS: Record<string, string> = {
  travelling: "en route", mining: "mining", full: "hold full · bring it home to unload",
  depleted: "deposit worked out", unsafe: "paused · threat nearby", claimed: "stopped · system claimed",
  "station full": "station and hold full · send a freighter",
};
const beltName = (st: ViewState, system: EntityId, body: number): string =>
  st.systems.find(s => s.id === system)?.bodies.find(b => b.id === body)?.name ?? sysName(st, system);
/** §pockets: a prospecting pass, as the fleet line reads it. */
function prospectStatus(i: { dwell: number; status: string }): string {
  if (i.status === "prospecting") return `${Math.max(1, Math.ceil(PROSPECT_SECS - i.dwell))} s left`;
  return MINE_STATUS[i.status] ?? i.status;
}
export function fleetIndustryHtml(g: GhostView, st: ViewState): string {
  if (!g.own || !g.industry) return "";
  const i = g.industry;
  const text = i.kind === "route" ? `${i.run.route.name} · stop ${i.run.stop+1}/${i.run.route.stops.length} · ${transitLabel(i.run.route.transit)} · ${i.run.hold ?? label(i.run.phase)}${i.run.route.escort ? " · escorted" : ""}`
    : i.kind === "deploy" ? `${label(i.outpost)} outpost → ${sysName(st,i.system)} · ${Math.floor(i.work / (st.galaxy?.industry_catalog?.outpost_build_secs ?? 60)*100)}%`
    : i.kind === "mine" ? `Mining ${label(i.ore)}${i.pocket ? " pocket" : ""} → ${beltName(st, i.system, i.body)} · ${MINE_STATUS[i.status] ?? i.status}${i.to_station && i.status === "mining" ? " · into station" : ""} · ${amount(i.mined)} cut`
    : i.kind === "prospect" ? `Prospecting ${beltName(st, i.system, i.body)} · ${prospectStatus(i)}`
    : `Helium-3 harvesting → ${sysName(st,i.system)} · ${i.status} · ${amount(i.harvested)} collected`;
  // §logistics: a route's line links to its trip in the Logistics panel.
  const open = i.kind === "route" ? `<button type="button" data-deck-act="fleet-logistics" data-mobile-act="open-logistics" data-fleet="${esc(g.id)}">Open in Logistics</button>` : "";
  return `<div class="industry-status"><b>${esc(text)}</b><small>Latest fleet report</small>${open}</div>`;
}
export function handleIndustryInput(el: HTMLInputElement | HTMLSelectElement, st: ViewState): boolean {
  const name = el.dataset.industryInput;
  if (!name) return false;
  const v = ui(st), n = Math.max(0, Number(el.value) || 0), stop = v.route.stops[Number(el.dataset.stop)];
  if (name === "name") v.route.name = el.value;
  else if (name === "transit" && (el.value === "slow" || el.value === "full" || el.value === "stealth")) v.route.transit = el.value;
  else if (name === "reserve") v.route.fuel_reserve = n;
  else if (name === "budget") v.route.shopping_budget = n;
  else if (name === "shopping" && el instanceof HTMLInputElement) v.route.shopping_budget = el.checked ? 3000 : null;
  else if (name === "escort") v.route.escort = el.value || null;
  else if (name === "repeat" && el instanceof HTMLInputElement) v.route.repeat = el.checked;
  else if (name === "port" && stop) {
    stop.port = el.value === "hub" ? {kind:"hub"} : el.value.startsWith("station:") ? {kind:"station",id:el.value.slice(8)} : {kind:"system",id:el.value};
    if (stop.port.kind !== "hub") stop.sell = false;
    if (stop.port.kind === "station") stop.unload = {};
  }
  else if (name === "sell" && stop && el instanceof HTMLInputElement) stop.sell = el.checked;
  else if (name === "manifest" && stop && (el.dataset.side === "load" || el.dataset.side === "unload")) stop[el.dataset.side][el.dataset.good as Commodity] = Math.floor(n);
  else if (name === "qty") v.qty = Math.max(1,Math.floor(n));
  else if (name === "plan-cap") v.cap = Math.min(1e9, Math.floor(n));
  else if (name === "rate") v.rate = Math.max(.1,n);
  else if (name === "depotTarget") v.depotTarget = Math.min(DEPOT_MAX, Math.floor(n));
  else if (name === "depotFuel" && DEPOT_FUELS.includes(el.value as Commodity)) v.depotFuel = el.value as Commodity;
  else if (name === "depotSystem") v.depotSystem = el.value;
  else if (name === "body" || name === "outpostBody") v[name] = n;
  else if (["fleet","system","good","output","project","resource","outpostSystem","outpostKind","builder","skimSystem","skimFleet"].includes(name)) (v as unknown as Record<string,unknown>)[name] = el.value;
  v.feedback = "";
  return true;
}
/** Stage a route (and its escort's guard order) behind the standard confirmation. */
function stageRoute(ctx: CoreContext, v: Ui, fleetId: string, route: FreightRoute): void {
  const commands: FleetCommand[] = [{type:"SetFreightRoute",fleet_id:fleetId,route:structuredClone(route)}];
  if (route.escort) commands.push({type:"GuardFleet",interceptor_id:route.escort,target_id:fleetId});
  v.sent = { fleet: fleetId, seen: false };
  ctx.intent.beginFleetCommand(commands);
}
export function handleIndustryAction(el: HTMLElement, ctx: CoreContext): boolean {
  const a = el.dataset.industryAct;
  if (!a) return false;
  const st = ctx.state, v = ui(st), fleet = st.ghosts.find(g => g.own && g.id === v.fleet);
  v.feedback = "";
  if (a === "tab") { v.tab = el.dataset.tab ?? "routes"; v.picker = null; }
  else if (a === "refining-floor") {
    const protect = el.dataset.on === "true";
    // §fuel-overhaul: smelting burns no fuel, so only the ore floor matters.
    ctx.send({type:"SetRefiningReserve",system_id:v.system,ore:protect ? 50 : 0,fuel:0});
    v.feedback = "Refining policy sent. Waiting for the system report.";
  }
  else if (a === "depot-set" && v.depotSystem) {
    ctx.send({type:"SetDepotRule",system_id:v.depotSystem,commodity:v.depotFuel,target:v.depotTarget});
    v.feedback = v.depotTarget > 0 ? "Depot rule sent." : "Depot rule cleared.";
  }
  else if (a === "depot-clear" && el.dataset.system && el.dataset.good) {
    ctx.send({type:"SetDepotRule",system_id:el.dataset.system,commodity:el.dataset.good as Commodity,target:0});
    v.feedback = "Depot rule cleared.";
  }
  else if (a === "pick-fleet" && el.dataset.fleet) { v.fleet = el.dataset.fleet; v.editing = false; v.picker = null; }
  else if (a === "new-route") { v.route = supplyDraft(st); v.editing = true; v.picker = null; }
  else if (a === "close-editor") { v.editing = false; v.picker = null; }
  else if (a === "advanced") v.advanced = !v.advanced;
  else if (a === "basket") {
    const home = homeSystem(st), good = el.dataset.good as Commodity, step = Number(el.dataset.step);
    if (!home || !fleet || !good || !step) return true;
    const basket = { ...(v.basket ?? openingImportPlan(st, home, "transport").manifest) };
    if (step > 0 && total(basket) >= fleetCargoCapacity(fleet)) return true;
    basket[good] = Math.max(0, (basket[good] ?? 0) + step);
    v.basket = basket;
  }
  else if (a === "send-shipment" || a === "customize" || a === "opening-plan") {
    // The charter's shipment: the plan card's basket, cap and escort. "opening-plan" is the older draft button's name.
    const home = homeSystem(st), f = fleet && fleetCargoCapacity(fleet) > 0 ? fleet : cargoFleets(st)[0];
    if (!home || !f) return true;
    v.fleet = f.id;
    const route = firstShipmentRoute(home, f, defaultEscort(st, f)?.id ?? null, v.basket ?? openingImportPlan(st, home, "transport").manifest, v.cap);
    if (a !== "send-shipment") { v.route = route; v.editing = true; v.picker = null; v.tab = "routes"; return true; }
    const reason = freightRouteReason(st, f, route);
    if (reason) { v.feedback = reason; return true; }
    stageRoute(ctx, v, f.id, route);
  }
  else if (a === "edit-route" || a === "run-again" || a === "copy") {
    const source = st.ghosts.find(g => g.own && g.id === (el.dataset.fleet ?? v.fleet))?.industry;
    if (source?.kind !== "route") return true;
    if (a === "run-again") stageRoute(ctx, v, v.fleet, source.run.route);
    else { v.route = structuredClone(source.run.route); v.editing = true; v.picker = null; }
  }
  else if (a === "stop" && el.dataset.fleet) ctx.intent.beginFleetCommand({type:"SetFreightRoute",fleet_id:el.dataset.fleet,route:null});
  else if (a === "add-stop" && v.route.stops.length < (st.galaxy?.industry_catalog?.max_stops ?? 8)) {
    const last = v.route.stops.at(-1)?.port;
    v.route.stops.push({port: last?.kind === "hub" ? (homeSystem(st) ? {kind:"system",id:homeSystem(st)!.id} : {kind:"hub"}) : {kind:"hub"},load:{},unload:{},sell:false});
  }
  else if (a === "remove-stop" && v.route.stops.length > 2) { v.route.stops.splice(Number(el.dataset.stop),1); v.picker = null; }
  else if (a === "open-pick" && (el.dataset.side === "load" || el.dataset.side === "unload")) {
    const stop = Number(el.dataset.stop), port = v.route.stops[stop]?.port;
    v.picker = { stop, side: el.dataset.side };
    // Loading at a colony suggests its largest free stock first.
    const system = port?.kind === "system" ? st.systems.find(s => s.id === port.id) : undefined;
    if (system && el.dataset.side === "load") {
      const best = [...COMMODITIES].sort((x, y) => availableStock(system, y) - availableStock(system, x))[0];
      if (best && availableStock(system, best) > 0) v.good = best;
    }
  }
  else if (a === "close-pick") v.picker = null;
  else if (a === "add-cargo") { const stop = v.route.stops[Number(el.dataset.stop)]; const side = el.dataset.side; if (stop && (side === "load" || side === "unload")) stop[side][v.good] = v.qty; v.picker = null; }
  else if (a === "drop-cargo") { const stop = v.route.stops[Number(el.dataset.stop)]; const side = el.dataset.side; if (stop && (side === "load" || side === "unload")) delete stop[side][el.dataset.good as Commodity]; }
  else if (a === "assign" && fleet) {
    const reason = freightRouteReason(st,fleet,v.route);
    if (reason) { v.feedback = reason; return true; }
    stageRoute(ctx, v, fleet.id, v.route);
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
