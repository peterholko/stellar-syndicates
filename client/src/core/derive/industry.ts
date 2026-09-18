import type { ColonyProjectKind, Commodity, FreightRoute, GhostView, ProjectTarget, SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { fleetBaseSpeed, fleetCargoCapacity, fleetFuelCapacity, WARP_FACTOR } from "./fleet";
import { assignedRecipe } from "./market";

export const projectName = (target: ProjectTarget): string => target.kind === "development"
  ? ({ orbital_assembly: "Orbital assembly complex", agricultural_export: "Agricultural export center", deep_extraction: "Deep extraction facility" })[target.project]
  : ({ cruiser: "Cruiser", academy: "Academy", colony: "Colony expedition" })[target.kind];
export const reservedStock = (s: SystemStateView, c: Commodity): number =>
  (s.industry?.reservations ?? []).reduce((n, r) => n + (r.goods[c] ?? 0), 0);
export const availableStock = (s: SystemStateView, c: Commodity): number =>
  Math.max(0, (s.stockpile?.find(x => x.commodity === c)?.units ?? 0) - reservedStock(s, c));
export const routePortId = (port: FreightRoute["stops"][number]["port"]): string => port.kind === "hub" ? "hub" : port.id;
export const routePortPosition = (st: ViewState, port: FreightRoute["stops"][number]["port"]) =>
  port.kind === "hub" ? st.galaxy?.hub : st.galaxy?.systems.find(s => s.id === port.id)?.pos;
export function freightRouteReason(st: ViewState, g: GhostView, r: FreightRoute): string | null {
  if (!g.own || g.tca || fleetCargoCapacity(g) <= 0) return "Choose an owned Freighter.";
  if (!r.name.trim() || r.name.length > 48) return "Name the route (up to 48 characters).";
  if (r.stops.length < 2 || r.stops.length > 8) return "Use 2–8 stops.";
  if (r.stops.length > 2 && !st.research?.programmes.some(p => p.id === "prop_line_autonomous_freight" && p.state === "completed")) return "More than two stops requires Autonomous Freight research.";
  if (!Number.isFinite(r.fuel_reserve) || r.fuel_reserve < 0 || r.fuel_reserve > fleetFuelCapacity(g)) return "Fuel reserve exceeds the tank.";
  for (const [i, stop] of r.stops.entries()) {
    if (!routePortPosition(st, stop.port)) return `Choose stop ${i + 1}.`;
    if (i && routePortId(stop.port) === routePortId(r.stops[i - 1].port)) return "Adjacent stops must differ.";
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
  if (kind === "agricultural_export" && (!b.habitable || !b.deposits?.some(d => d.resource === "biomass"))) return "Requires a habitable world with Biomass.";
  if (kind === "deep_extraction" && !b.deposits?.some(d => d.resource === commodity && ["metallic_ore","cuprite_ore","titanium_ore","crystalline_ore","rare_metal_ore","rare_elements","silicates"].includes(commodity))) return "Choose a matching mineral deposit.";
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
    : lines.reduce((sum, a) => sum + recipe.conversion!.rate * a.throughput * a.skill * a.food * a.site * (a.recovery ?? 1) * 60, 0)) : staffed;
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
      if (distance > 0) freight += Math.min(delivered, loaded) * 60 / (distance / (fleetBaseSpeed(g) * WARP_FACTOR));
    }
    return { commodity, needed, local, shortfall, stock: availableStock(s, commodity),
      runway: shortfall > 0 ? availableStock(s, commodity) / shortfall : null, suppliers, freight };
  });
  return { recipe, rated, staffed, inputs, capacityLimited: rated + .001 < perMinute,
    workforceLimited: staffed + .001 < Math.min(rated, perMinute),
    feedstockLimited: inputs.some(i => i.shortfall > .001), freightLimited: inputs.some(i => i.shortfall > i.freight + .001) };
}
