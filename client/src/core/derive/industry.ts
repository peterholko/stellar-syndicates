import type { ColonyProjectKind, Commodity, EntityId, FreightRoute, GhostView, Manifest, ProjectTarget, SystemStateView, TransitMode } from "../../protocol";
import type { ViewState } from "../../state";
import { slowCapable, fleetBaseSpeed, fleetCargoCapacity, fleetFuelCapacity, transitSpeedMultiplier, WARP_FACTOR } from "./fleet";
import { assignedRecipe, marketAverageQuote } from "./market";

export const projectName = (target: ProjectTarget): string => target.kind === "development"
  ? ({ orbital_assembly: "Orbital assembly complex", agricultural_export: "Agricultural export center", deep_extraction: "Deep extraction facility" })[target.project]
  : ({ cruiser: "Cruiser", academy: "Academy", colony: "Colony expedition" })[target.kind];
export const reservedStock = (s: SystemStateView, c: Commodity): number =>
  (s.industry?.reservations ?? []).reduce((n, r) => n + (r.goods[c] ?? 0), 0);
export const availableStock = (s: SystemStateView, c: Commodity): number =>
  Math.max(0, (s.stockpile?.find(x => x.commodity === c)?.units ?? 0) - reservedStock(s, c));
export const routePortId = (port: FreightRoute["stops"][number]["port"]): string =>
  port.kind === "hub" ? "hub" : port.kind === "station" ? `station:${port.id}` : port.id;
export type OpeningInvestment = "transport" | "refining" | "exploration";
export function openingImportPlan(st: ViewState, home: SystemStateView, goal: OpeningInvestment) {
  const key = goal === "transport" ? (home.structures.shipyard ? "tiny_freighter" : "shipyard")
    : home.structures.academy ? "smelter" : "academy";
  const costs: [Commodity, number][] = goal === "exploration" ? [["machinery",12],["electronics",8]]
    : (st.galaxy?.build_options.find(o => o.key === key)?.costs ?? []).map(c => [c.commodity as Commodity,c.units]);
  const missing = new Map(costs.map(([c,n]) => [c, Math.max(0, Math.ceil(n - availableStock(home,c)))]));
  // What home lacks before any basket: the goods a player's own basket covers.
  const deficit: Manifest = Object.fromEntries([...missing].filter(([,n]) => n > 0));
  // §fuel-overhaul: no fuel in the basket — the freighter flies Slow on
  // Uranium and fills at the Hub; the whole hold carries goods.
  const manifest: Manifest = {};
  let room = 50;
  // Fill a bounded mixed basket, not an implicit promise that every project kit
  // fits one hold. Stock and prerequisites come only from arrived reports.
  const priority: Commodity[] = ["electronics", "machinery", "alloys", "provisions", "polymers"];
  while (room > 0 && priority.some(c => (missing.get(c) ?? 0) > 0)) {
    for (const c of priority) if (room && (missing.get(c) ?? 0) > 0) {
      manifest[c] = (manifest[c] ?? 0) + 1; missing.set(c, missing.get(c)! - 1); room--;
    }
  }
  return { manifest, remainder: Object.fromEntries([...missing].filter(([,n]) => n > 0)), key, deficit };
}
export const routePortPosition = (st: ViewState, port: FreightRoute["stops"][number]["port"]) =>
  port.kind === "hub" ? st.galaxy?.hub
    : port.kind === "station" ? st.ghosts.find(g => g.own && g.id === port.id)?.pos
    : st.galaxy?.systems.find(s => s.id === port.id)?.pos;

/** §logistics: the charter's first shipment as one route. It loads home ore,
 * sells it at the Hub and buys the return basket within the cap, then unloads
 * the basket at home. Stop 1 also unloads whatever is already aboard, to make
 * room for the ore. */
export function firstShipmentRoute(home: SystemStateView, freighter: GhostView, escort: EntityId | null, basket: Manifest, cap: number): FreightRoute {
  const ore = Math.min(50, fleetCargoCapacity(freighter));
  const goods: Manifest = Object.fromEntries(Object.entries(basket).filter(([,n]) => (n ?? 0) > 0));
  const aboard = freighter.cargo_manifest?.length ? freighter.cargo_manifest : freighter.cargo ? [freighter.cargo] : [];
  return { name: "First shipment", transit: "slow", repeat: false, shopping_budget: cap, escort,
    fuel_reserve: Math.min(2, fleetFuelCapacity(freighter) * .1),
    stops: [
      { port: { kind: "system", id: home.id }, load: { metallic_ore: ore }, unload: Object.fromEntries(aboard.map(c => [c.commodity, c.units])), sell: false },
      { port: { kind: "hub" }, load: goods, unload: { metallic_ore: ore }, sell: true },
      { port: { kind: "system", id: home.id }, load: {}, unload: { ...goods }, sell: false },
    ] };
}

/** Cruise speed for a route's legs: its own transit mode, not the fleet's current one. */
export const routeCruiseSpeed = (g: GhostView, transit: TransitMode = "full"): number =>
  Math.max(1, fleetBaseSpeed(g) * WARP_FACTOR * transitSpeedMultiplier({ ...g, transit }));

/** Flight seconds for each leg, stop i to stop i+1 (a repeating route also
 * closes the loop). Straight-line cruise estimates; null for an unknown port. */
export function routeLegSeconds(st: ViewState, g: GhostView, route: FreightRoute): (number | null)[] {
  const speed = routeCruiseSpeed(g, route.transit), n = route.stops.length;
  return Array.from({ length: Math.max(0, route.repeat ? n : n - 1) }, (_, i) => {
    const a = routePortPosition(st, route.stops[i].port), b = routePortPosition(st, route.stops[(i + 1) % n].port);
    return a && b ? Math.hypot(a.x - b.x, a.y - b.y) / speed : null;
  });
}

/** Walking the received Hub book: what `units` would fetch or cost. Null
 * before a price report, or for a good the Hub does not list. */
export function hubQuote(st: ViewState, commodity: Commodity, units: number, side: "buy" | "sell"): number | null {
  const price = st.market?.prices.find(p => p.commodity === commodity);
  if (!price) return null;
  return units > 0 ? units * marketAverageQuote(price.price, units, side, price.depth) : 0;
}

/** What a route's Hub visits should earn and spend at the latest received
 * prices, or null before the first price report. Goods already in the Hub
 * warehouse load before anything is bought; without a cap nothing is bought. */
export function routeHubEstimate(st: ViewState, route: FreightRoute): { sold: number; bought: number } | null {
  if (!st.market?.prices.length) return null;
  const stocked = new Map((st.wallet?.warehouse ?? []).map(slot => [slot.commodity, slot.units]));
  let sold = 0, bought = 0;
  for (const stop of route.stops) if (stop.port.kind === "hub") {
    if (stop.sell) for (const [c, n] of Object.entries(stop.unload)) sold += hubQuote(st, c as Commodity, n ?? 0, "sell") ?? 0;
    if (route.shopping_budget != null) for (const [c, n] of Object.entries(stop.load)) {
      bought += hubQuote(st, c as Commodity, Math.max(0, (n ?? 0) - (stocked.get(c as Commodity) ?? 0)), "buy") ?? 0;
    }
  }
  return { sold, bought };
}

/** Units aboard as the Freighter leaves each stop, from `start`: unloads take
 * what is aboard, loads take their whole manifest. */
export function routeAboard(route: FreightRoute, start: Manifest = {}): number[] {
  const hold: Manifest = { ...start };
  return route.stops.map(stop => {
    for (const [c, n] of Object.entries(stop.unload)) hold[c as Commodity] = Math.max(0, (hold[c as Commodity] ?? 0) - (n ?? 0));
    for (const [c, n] of Object.entries(stop.load)) hold[c as Commodity] = (hold[c as Commodity] ?? 0) + (n ?? 0);
    return Object.values(hold).reduce((total: number, n) => total + (n ?? 0), 0);
  });
}
export function freightRouteReason(st: ViewState, g: GhostView, r: FreightRoute): string | null {
  if (!g.own || g.tca || fleetCargoCapacity(g) <= 0) return "Choose an owned Freighter.";
  if (r.transit === "slow" && !slowCapable(g)) return "Ships of the line cannot fly Slow.";
  if (!r.name.trim() || r.name.length > 48) return "Name the route (up to 48 characters).";
  if (r.stops.length < 2 || r.stops.length > 8) return "Use 2–8 stops.";
  const returnTrip = !r.repeat && r.stops.length === 3 && routePortId(r.stops[0].port) === routePortId(r.stops[2].port);
  if (r.stops.length > 2 && !returnTrip && !st.research?.programmes.some(p => p.id === "prop_line_autonomous_freight" && p.state === "completed")) return "More than two stops requires Autonomous Freight research (a one-off return to the first port is allowed).";
  if (r.shopping_budget != null && (!Number.isFinite(r.shopping_budget) || r.shopping_budget < 0 || r.shopping_budget > 1e9)) return "Enter a valid Hub shopping cap.";
  if (!Number.isFinite(r.fuel_reserve) || r.fuel_reserve < 0 || r.fuel_reserve > fleetFuelCapacity(g)) return "Fuel reserve exceeds the tank.";
  for (const [i, stop] of r.stops.entries()) {
    if (!routePortPosition(st, stop.port)) return `Choose stop ${i + 1}.`;
    if (i && routePortId(stop.port) === routePortId(r.stops[i - 1].port)) return "Adjacent stops must differ.";
    if (stop.port.kind === "station" && Object.values(stop.unload).some(n => (n ?? 0) > 0)) return `Stop ${i + 1}: a station stop only loads.`;
    for (const manifest of [stop.load, stop.unload]) {
      if (Object.values(manifest).some(n => !Number.isInteger(n) || n! < 0)) return "Cargo quantities must be whole units.";
      if (Object.values(manifest).reduce((sum, n) => sum + (n ?? 0), 0) > fleetCargoCapacity(g)) return `Stop ${i + 1} exceeds cargo capacity.`;
    }
  }
  return null;
}

export function colonyProjectReason(st: ViewState, s: SystemStateView, body: number, kind: ColonyProjectKind, commodity: Commodity): string | null {
  const b = s.bodies.find(b => b.id === body), spec = st.galaxy?.industry_catalog?.projects.find(p => p.kind === kind);
  if (!b || !spec || s.owner !== st.playerId || s.industry?.outpost) return "Choose an owned colony world.";
  if (s.blockade) return "Construction access blocked.";
  if (s.industry?.projects.some(p => p.kind === kind)) return "This colony already has that project.";
  if (s.industry?.projects.some(p => p.work < (st.galaxy?.industry_catalog?.projects.find(k => k.kind === p.kind)?.build_secs ?? Infinity))) return "Finish the colony’s active project first.";
  if (kind === "orbital_assembly" && (b.structures.shipyard ?? 0) < 2) return "Requires Shipyard II on this world.";
  // §fuel-overhaul: its Warp Cores wait for Fusion Reactors, like the Drive Works.
  if (kind === "orbital_assembly" && !st.research?.programmes.some(p => p.id === "mat_foundry_iv_fusion_reactors" && p.state === "completed")) return "Requires Fusion Reactors research.";
  if (kind === "agricultural_export" && (!b.habitable || !b.deposits?.some(d => d.resource === "biomass"))) return "Requires a habitable world with Biomass.";
  if (kind === "deep_extraction" && !b.deposits?.some(d => d.resource === commodity && ["metallic_ore","cuprite_ore","titanium_ore","crystalline_ore","rare_metal_ore","olivine_ore","bornite_ore","ilmenite_ore","iridium_ore","void_crystal","rare_elements","silicates"].includes(commodity))) return "Choose a matching mineral deposit.";
  const ownReserve = s.industry?.reservations.find(r => r.target.kind === "development" && r.target.project === kind);
  if (!spec.costs.every(([c,n]) => {
    const others = reservedStock(s,c) - (ownReserve?.goods[c] ?? 0);
    return Math.max(0,(s.stockpile?.find(x => x.commodity === c)?.units ?? 0)-others) >= n;
  })) return "Deliver the construction materials to the colony.";
  return null;
}

/** Rated planning, not a second simulation or ETA clock. Inputs are exclusively
 * arrived assignment/stock reports and the public recipe catalog. Suspended
 * suppliers never promise output; unsurveyed deposits are never inferred. */
export function productionPlan(st: ViewState, s: SystemStateView, output: Commodity, perMinute: number) {
  const recipes = st.galaxy?.build_options ?? [];
  // Alternatives share a Smelter, not its throughput: only the assigned recipe
  // contributes capacity to the plan. All identities come from Welcome.
  const alternatives = recipes.flatMap(r => (r.refining_recipes?.length ? r.refining_recipes : r.conversion ? [r.conversion] : [])
    .map(conversion => ({ ...r, conversion })));
  const recipe = alternatives.find(r => r.conversion.output === output);
  const lines = s.assignments.filter(a => assignedRecipe(recipes.find(r => r.key === a.structure), a.refining_ore)?.output === output);
  // A secondary yield is useful supply, not another fully rated Smelter.
  const secondary = s.assignments.filter(a => !a.suspended && !lines.includes(a))
    .reduce((n,a) => n + (a.outputs.find(([c]) => c === output)?.[1] ?? 0) * 60, 0);
  const staffed = secondary + lines.reduce((sum, a) => sum + (a.outputs.find(([c]) => c === output)?.[1] ?? 0) * 60, 0);
  const built = (s.converters ?? []).filter(a => a.structure === recipe?.key &&
    assignedRecipe(recipes.find(r => r.key === a.structure), s.assignments.find(line => line.structure === a.structure && line.body_id === a.body_id)?.refining_ore)?.output === output);
  const rated = recipe?.conversion ? secondary + (built.length && built.every(a => a.rated_output != null)
    ? built.reduce((sum,a) => sum+a.rated_output!*60,0)
    : lines.reduce((sum, a) => sum + recipe.conversion!.rate * a.throughput * a.skill * a.food * (a.power ?? 1) * a.site * (a.recovery ?? 1) * 60, 0)) : staffed;
  const net = (site: SystemStateView, c: Commodity) => {
    let n = site.assignments.filter(a => !a.suspended).reduce((v, a) => v + (a.outputs.find(([g]) => c === g)?.[1] ?? 0), 0);
    for (const a of site.assignments.filter(a => !a.suspended)) {
      const conv = assignedRecipe(recipes.find(r => r.key === a.structure), a.refining_ore);
      // Replace this colony's existing target line with the plan, rather than
      // charging both its old consumption and the planned output's inputs.
      if (conv && !(site.id === s.id && conv.output === output)) n -= (a.outputs.find(([g]) => g === conv.output)?.[1] ?? 0) / Math.max(.001, a.site * (a.recovery ?? 1)) * (conv.inputs.find(([g]) => g === c)?.[1] ?? 0);
    }
    for (const p of site.industry?.projects ?? []) if (p.supplied) {
      n += p.outputs.find(([g]) => g === c)?.[1] ?? 0;
      n -= st.galaxy?.industry_catalog?.projects.find(x => x.kind === p.kind)?.inputs.find(([g]) => g === c)?.[1] ?? 0;
    }
    if (site.industry?.outpost?.supplied && site.industry.outpost.commodity === c && site.industry.outpost.kind === "extraction") n += site.industry.outpost.rate;
    return n * 60;
  };
  // Use the least efficient active line for an intentionally cautious shopping
  // list. The UI says rated: research, food changes, combat and travel can differ.
  const siteYield = built.length ? Math.min(...built.map(a => (a.site ?? 1) * (a.recovery ?? 1))) : lines.length ? Math.min(...lines.map(a => a.site * (a.recovery ?? 1))) : 1;
  const inputs = (recipe?.conversion?.inputs ?? []).map(([commodity, units]) => {
    const needed = Math.max(0, perMinute - secondary) / Math.max(.001, siteYield) * units;
    const local = Math.max(0, net(s, commodity));
    const shortfall = Math.max(0, needed - local);
    const suppliers = st.systems.filter(x => x.id !== s.id && x.owner === st.playerId).map(x => ({
      id: x.id, name: st.galaxy?.systems.find(g => g.id === x.id)?.name ?? x.id,
      surplus: Math.max(0, net(x, commodity)), stock: availableStock(x, commodity),
      deposit: x.bodies.some(b => b.deposits?.some(d => d.resource === commodity)),
    })).filter(x => x.surplus > 0 || x.stock > 0 || x.deposit);
    let freight = 0;
    for (const g of st.ghosts.filter(g => g.own && g.industry?.kind === "route")) {
      const run = g.industry!.kind === "route" ? g.industry!.run : null;
      if (!run || !run.route.repeat || run.hold || run.phase === "complete") continue;
      const stops = run.route.stops;
      let distance = 0;
      for (let i = 0; i < stops.length; i++) {
        const a = routePortPosition(st, stops[i].port), b = routePortPosition(st, stops[(i + 1) % stops.length].port);
        if (a && b) distance += Math.hypot(a.x - b.x, a.y - b.y);
      }
      const delivered = stops.filter(x => x.port.kind === "system" && x.port.id === s.id).reduce((n, x) => n + (x.unload[commodity] ?? 0), 0);
      const loaded = stops.filter(x => x.port.kind !== "system" || x.port.id !== s.id).reduce((n, x) => n + (x.load[commodity] ?? 0), 0);
      // Ceiling at cruise speed, ignoring berths/wells/waiting. Not a promise
      // that a not-yet-arrived load, refuel or ship exists at another colony.
      if (distance > 0) freight += Math.min(delivered, loaded) * 60 / (distance / (fleetBaseSpeed(g) * WARP_FACTOR * transitSpeedMultiplier(g)));
    }
    const refiningFloor = recipe?.key === "smelter" ? s.industry?.refining_reserve?.[commodity] ?? 0 : 0;
    const stock = Math.max(0, availableStock(s, commodity) - refiningFloor);
    return { commodity, needed, local, shortfall, stock, refiningFloor,
      runway: shortfall > 0 ? stock / shortfall : null, suppliers, freight };
  });
  return { recipe, rated, staffed, inputs, capacityLimited: rated + .001 < perMinute,
    workforceLimited: staffed + .001 < Math.min(rated, perMinute),
    feedstockLimited: inputs.some(i => i.shortfall > .001), freightLimited: inputs.some(i => i.shortfall > i.freight + .001) };
}
