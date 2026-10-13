import { label } from "../../icons";
import type { BodyView, BuildState, Commodity, SystemStateView } from "../../protocol";
import { buildProgress } from "./construction";
import { POOL_OF, SHIP_YARD, structureBuildSeconds, structureLevel, YARD_PREREQ, YARD_TITLE, type BuildOpt } from "./market";

/** §construction: each world builds one structure at a time, in the order the
 * orders arrived; hulls, modules and courses use their own facilities. This is
 * that one work slot, read from the served report: who holds it, who waits,
 * and roughly when each starts and finishes. Estimates only; the served
 * report alone says a job has started or finished. */

/** Founding Camp projects share the slot (their base seconds, see SETTLEMENT_PROJECTS). */
const PROJECT_SECONDS: Record<string, number> = { camp_prospect: 60, camp_workings: 120, camp_storehouse: 90, camp_quarters: 120 };

export const isProjectKey = (key: string): boolean => key in PROJECT_SECONDS;
/** Structure tiers read as Roman numerals across the construction UI. */
export const romanTier = (tier: number): string => ["", "I", "II", "III", "IV", "V", "VI"][tier] ?? String(tier);
export const isStructureJob = (key: string): boolean => (!!POOL_OF[key] && !(key in SHIP_YARD)) || isProjectKey(key);
export const isHullJob = (key: string): boolean => key in SHIP_YARD;

/** Structures that do nothing until a workforce crew is posted to them. */
export const WORKFORCE_STRUCTURES: ReadonlySet<string> = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator",
  "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "living_terraces", "academy",
  "composite_works", "hull_fabricator", "precision_works", "drive_works", "uranium_mill", "antimatter_plant",
  "shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry", "service_berth",
]);

/** A finished building that still has no crew (from the served report). */
export function needsWorkforce(report: SystemStateView, bodyId: number, key: string): boolean {
  return WORKFORCE_STRUCTURES.has(key) && (report.bodies.find(body => body.id === bodyId)?.structures[key] ?? 0) > 0
    && !report.assignments.some(line => line.body_id === bodyId && line.structure === key && line.workers > 0);
}

export interface StructureJob {
  job: BuildState;
  key: string;
  /** The tier this order builds: the built tier plus earlier orders for it. */
  tier: number;
  project: boolean;
  /** Holds the world's work slot (legacy reports may show more than one). */
  active: boolean;
  /** 1 for the job building now, then its place in line. */
  position: number;
  /** Seconds until it starts (0 once active) and until it is ready; null when unknown. */
  startsIn: number | null;
  readyIn: number | null;
  /** 0–100 for an active job with a known clock. */
  progress: number | null;
  /** A waiting structure that no later order for the same structure builds on. */
  cancellable: boolean;
}

/** This world's structure line, active job first, then waiting jobs in the
 * served receipt order. `catalog` supplies recipe seconds for the estimates. */
export function structureQueue(report: SystemStateView, body: BodyView, catalog: readonly BuildOpt[], now: number): StructureJob[] {
  const jobs = report.builds.filter(job => job.body_id === body.id && isStructureJob(job.key));
  const ordered = [...jobs.filter(job => !job.queued), ...jobs.filter(job => job.queued)];
  const earlier = new Map<string, number>();
  const mult = body.structure_build_time_mult ?? body.construction_time_mult;
  let free: number | null = 0;
  const line = ordered.map((job, index): StructureJob => {
    const before = earlier.get(job.key) ?? 0;
    earlier.set(job.key, before + 1);
    const project = isProjectKey(job.key);
    const tier = project ? 1 : (body.structures[job.key] ?? 0) + before + 1;
    const active = !job.queued;
    let startsIn: number | null;
    let readyIn: number | null;
    if (active) {
      startsIn = 0;
      readyIn = job.complete_time == null ? null : Math.max(0, job.complete_time - now);
      free = free === null || readyIn === null ? null : Math.max(free, readyIn);
    } else {
      const option = catalog.find(entry => entry.key === job.key);
      const seconds = project ? PROJECT_SECONDS[job.key] * mult : option ? structureBuildSeconds(structureLevel(option, tier), body) : null;
      startsIn = free;
      readyIn = free === null || seconds === null ? null : free + seconds;
      free = readyIn;
    }
    return { job, key: job.key, tier, project, active, position: index + 1, startsIn, readyIn,
      progress: active ? buildProgress(job, now) : null, cancellable: false };
  });
  for (const entry of line) {
    entry.cancellable = !entry.active && !entry.project && entry.job.id !== undefined
      && !line.some(other => other.key === entry.key && other.position > entry.position);
  }
  return line;
}

/** Hulls under construction on this world's yards (they never wait for the slot). */
export function hullJobs(report: SystemStateView, body: BodyView): BuildState[] {
  return report.builds.filter(job => job.body_id === body.id && isHullJob(job.key));
}

// §build-sheet: the figures the build sheet quotes, mirrored from the sim.
/** production::TIER_THROUGHPUT: a level's output against level I, fully crewed. */
export const TIER_THROUGHPUT: readonly number[] = [0, 1, 2.2, 3.8, 6, 8.6, 11.5];
/** colony::POP_CAP_PER_HABITAT_TIER and TERRACE_POP_PER_TIER, in people per
 * level, before the world's habitat multiplier. */
const HOUSING_PER_LEVEL: Readonly<Record<string, number>> = { habitat: 25_000, living_terraces: 20_000 };
/** build::STORAGE_WAREHOUSE_INITIAL, STORAGE_PER_WAREHOUSE_TIER and
 * STORAGE_PER_ORBITAL_WAREHOUSE_TIER. A Founding Camp's Warehouse holds 250
 * until its Storehouse project is done. */
const WAREHOUSE_FIRST = 700, WAREHOUSE_PER_LEVEL = 400, ORBITAL_STORAGE_PER_LEVEL = 2_000, CAMP_STORAGE = 250;
const EXTRACTORS: ReadonlySet<string> = new Set(["mining_complex", "volatile_harvester", "bioharvester"]);

/** §power (sim::power): what draws power, each planet's free base, and a
 * plant's output. Food, housing, storage, defence and sensors never draw. */
export const POWERED_STRUCTURES: ReadonlySet<string> = new Set([
  "mining_complex", "volatile_harvester", "smelter", "electronics_fabricator", "chemical_works", "fuel_refinery",
  "machine_works", "armaments_complex", "composite_works", "hull_fabricator", "precision_works", "drive_works",
  "uranium_mill", "antimatter_plant",
  "shipyard", "naval_drydock", "capital_slipway", "ordnance_foundry", "academy",
]);
export const POWER_BASE_MW = 3;
/** §fuel-overhaul: I–II Solar Array (burns nothing), III–IV Fusion Plant on
 * Deuterium, V–VI on Helium-3. */
export const FUSION_CORE_TIER = 3;
export const FUSION_TIER = 5;
/** The heat-heavy works draw double; the Antimatter Plant ten times. */
const DRAW_FACTOR: Readonly<Record<string, number>> = { smelter: 2, machine_works: 2, drive_works: 2, antimatter_plant: 10 };
const throughputAt = (tier: number) => TIER_THROUGHPUT[Math.max(0, Math.min(tier, TIER_THROUGHPUT.length - 1))];
/** MW a fully crewed structure draws: its tier throughput × its heat factor. */
export const drawMW = (key: string, tier: number): number =>
  POWERED_STRUCTURES.has(key) && tier > 0 ? throughputAt(tier) * (DRAW_FACTOR[key] ?? 1) : 0;
/** What a plant at `tier` burns (null on solar). */
export const plantFuel = (tier: number): Commodity | null =>
  tier >= FUSION_TIER ? "helium3" : tier >= FUSION_CORE_TIER ? "deuterium" : null;
/** A Fusion Plant keeps its Solar Array II fields: their output stays free
 * (sim power::solar_mw). */
export const solarMW = (tier: number): number => plantMW(Math.min(tier, FUSION_CORE_TIER - 1));
/** A Power Plant's output at `tier`. */
export const plantMW = (tier: number): number => tier > 0 ? 8 * throughputAt(tier) : 0;
/** MW for display: one decimal below 100. */
export const mw = (value: number): string => (value >= 100 ? Math.round(value) : Math.round(value * 10) / 10).toLocaleString("en-US");
/** A planet's grid as its report shows it: "⚡ 4.4 / 8 MW", and the power
 * share when it is short. Null when nothing there draws or supplies power. */
export function gridLine(body: BodyView): { text: string; short: boolean; tip: string } | null {
  const grid = body.power;
  if (!grid || grid.demand <= 1e-9 && !(body.structures.power_plant > 0)) return null;
  const short = grid.power < 0.999;
  const fuel = grid.fuel ? label(grid.fuel) : "fuel";
  const text = `⚡ ${mw(grid.demand)} / ${mw(grid.supply)} MW${short ? ` · Power ${Math.round(grid.power * 100)}%` : ""}`;
  const tip = `Demand / supply. ${POWER_BASE_MW} MW base${body.structures.power_plant > 0 ? ` + plant` : ""}.`
    + (grid.burn > 0 ? ` Burning ${mw(grid.burn * 3600)} ${fuel}/h.` : "")
    + (grid.no_fuel ? ` Plant out of ${fuel}.` : "");
  return { text, short, tip };
}

/** The highest level the catalogue offers: Command Center and station levels
 * are listed; the Service Berth has one; everything else climbs to VI (V and VI
 * need research, which only the sim checks). */
export function maxLevel(option: BuildOpt): number {
  if (option.tiers?.length) return Math.max(...option.tiers.map(level => level.tier));
  return option.key === "service_berth" ? 1 : 6;
}

export interface StructureFact { label: string; from: string | null; to: string }

/** What raising `option` from level `from` (0: a new building) to `to` changes
 * on `body`, in the sim's own numbers. Only figures the client can state
 * exactly: extraction rates depend on the deposit, so a new mine shows none. */
export function structureFacts(option: BuildOpt, from: number, to: number, body: BodyView): StructureFact[] {
  const key = option.key;
  const facts: StructureFact[] = [];
  const num = (value: number) => Math.round(value).toLocaleString("en-US");
  const was = (value: string) => from > 0 ? value : null;
  const producer = EXTRACTORS.has(key) || !!option.conversion || !!option.refining_recipes?.length;
  if (producer && WORKFORCE_STRUCTURES.has(key)) {
    if (from > 0) facts.push({ label: "Output", from: `×${TIER_THROUGHPUT[from]}`, to: `×${TIER_THROUGHPUT[Math.min(to, TIER_THROUGHPUT.length - 1)]}` });
    else if (option.conversion) facts.push({ label: "Output", from: null, to: `${num(option.conversion.rate * 60)} ${label(option.conversion.output)}/min` });
  }
  if (WORKFORCE_STRUCTURES.has(key) && key !== "service_berth") facts.push({ label: "Crew", from: was(String(from)), to: String(to) });
  // §power: what it draws or supplies, and this planet's grid after it.
  const grid = body.power;
  if (POWERED_STRUCTURES.has(key)) {
    facts.push({ label: "Power", from: was(`${mw(drawMW(key, from))} MW`), to: `${mw(drawMW(key, to))} MW` });
  }
  if (key === "power_plant") {
    facts.push({ label: "Supplies", from: was(`${mw(plantMW(from))} MW`), to: `${mw(plantMW(to))} MW` });
    const burns = (tier: number) => { const fuel = plantFuel(tier);
      return fuel ? `${label(fuel)} past ${mw(POWER_BASE_MW + solarMW(tier))} MW` : "Nothing (solar)"; };
    facts.push({ label: "Burns", from: from > 0 && burns(from) !== burns(to) ? burns(from) : null, to: burns(to) });
  }
  if (grid && (POWERED_STRUCTURES.has(key) || key === "power_plant")) {
    const demand = grid.demand + drawMW(key, to) - drawMW(key, from);
    const supply = key === "power_plant" ? POWER_BASE_MW + plantMW(to)
      : grid.no_fuel ? grid.supply : POWER_BASE_MW + plantMW(body.structures.power_plant ?? 0);
    const share = demand > 1e-9 ? Math.min(1, supply / demand) : 1;
    facts.push({ label: "Grid", from: was(`${mw(grid.demand)} / ${mw(grid.supply)} MW`),
      to: `${mw(demand)} / ${mw(supply)} MW${share < 0.999 ? ` · ${Math.floor(share * 100)}%` : ""}` });
  }
  const housing = HOUSING_PER_LEVEL[key];
  if (housing) {
    const mult = body.habitat_capacity_mult ?? 1;
    const terraces = HOUSING_PER_LEVEL.living_terraces * mult * (body.structures.living_terraces ?? 0);
    // A Founding Camp's first Habitat houses the camp, not a full level.
    const camp = key === "habitat" && from === 1 && body.founding_camp ? Math.max(0, (body.population_capacity ?? 0) * 1e6 - terraces) : null;
    facts.push({ label: "Houses", from: was(num(camp ?? housing * mult * from)), to: num(housing * mult * to) });
  }
  if (key === "warehouse") {
    const camp = !!body.founding_camp && !body.founding_camp.completed.includes("storehouse");
    const cap = (tier: number) => tier === 1 && camp ? CAMP_STORAGE : WAREHOUSE_FIRST + WAREHOUSE_PER_LEVEL * (tier - 1);
    facts.push({ label: "Storage", from: was(num(cap(from))), to: num(cap(to)) });
  }
  if (key === "orbital_warehouse") facts.push({ label: "Storage", from: was(num(ORBITAL_STORAGE_PER_LEVEL * from)), to: num(ORBITAL_STORAGE_PER_LEVEL * to) });
  // build::command_infrastructure_bonus and command_construction_mult.
  if (key === "command_center" && to >= 2) facts.push({ label: "Gives", from: null, to: to === 2 ? "+1 Infrastructure slot, builds 10% faster" : "+2 Infrastructure slots, builds 20% faster" });
  const unlocks = Object.entries(YARD_PREREQ).filter(([, need]) => need.yard === key && need.tier === to).map(([yard]) => YARD_TITLE[yard] ?? label(yard));
  if (unlocks.length) facts.push({ label: "Unlocks", from: null, to: unlocks.join(", ") });
  return facts;
}

/** When a new order placed now would start and finish, behind the current line. */
export function nextOrderTiming(line: readonly StructureJob[], seconds: number, travel: number): { startsIn: number | null; readyIn: number | null } {
  if (!line.length) return { startsIn: travel, readyIn: travel + seconds };
  const last = line[line.length - 1].readyIn;
  return last === null ? { startsIn: null, readyIn: null } : { startsIn: Math.max(last, travel), readyIn: Math.max(last, travel) + seconds };
}

/** Developed worlds with an idle work slot or a structure under construction,
 * across every owned system: the top bar's construction summary. */
export interface WorldSlot { systemId: string; body: BodyView; job: StructureJob | null; waiting: number }
export function worldSlots(systems: readonly SystemStateView[], playerId: string | null, catalog: readonly BuildOpt[], now: number): WorldSlot[] {
  return systems.filter(system => system.owner === playerId && playerId !== null).flatMap(system => system.bodies
    .filter(body => Object.values(body.structures).some(tier => tier > 0) || system.builds.some(job => job.body_id === body.id && isStructureJob(job.key)))
    .map(body => {
      const line = structureQueue(system, body, catalog, now);
      return { systemId: system.id, body, job: line[0] ?? null, waiting: Math.max(0, line.length - 1) };
    }));
}
