// §stores: the stores bar under the top bar and its Stores pop-out
// (docs/stores.md). §stores-flows (Nov 5 2026): the server sends each good's
// flows, what the economy makes and uses, and a good's rate is their net. A
// system that reports none falls back to the rate measured from successive
// reports: the steady change over the last five minutes, leaving out one-off
// steps (a freighter unloading, a build starting, a sale).
import { goodKnown } from "./fleet";
import type { GoodsTab } from "./goods";
import type { Commodity, EntityId, SystemStateView } from "../../protocol";

/** The bar's goods until the player pins their own. */
export const DEFAULT_PINS: readonly Commodity[] = ["provisions", "metallic_ore", "alloys", "machinery", "electronics", "uranium"];

/** Every good, in the pop-out's groups and order, each on its tab (§goods-tabs:
 * Resources · Parts · Modules). */
export const STORE_GROUPS: readonly { title: string; tab: GoodsTab; goods: readonly Commodity[] }[] = [
  { title: "Raw", tab: "resources", goods: ["metallic_ore", "cuprite_ore", "crystalline_ore", "titanium_ore", "rare_metal_ore", "olivine_ore", "bornite_ore",
    "ilmenite_ore", "iridium_ore", "helium3_ice", "void_crystal", "volatiles", "biomass"] },
  { title: "Refined", tab: "resources", goods: ["provisions", "alloys", "electronics", "polymers", "silicates", "rare_elements", "conductive_metals", "titanium",
    "iridium", "exotic_matter"] },
  { title: "Advanced", tab: "resources", goods: ["machinery", "armaments", "composites", "precision_components"] },
  // §shipbuilding-overhaul Phase 5: what the guns fire.
  { title: "Munitions", tab: "resources", goods: ["torpedoes", "slugs", "beam_crystals"] },
  { title: "Fuel", tab: "resources", goods: ["uranium", "deuterium", "helium3", "antimatter"] },
  // §reactors-drives: the kit — reactors and drives by rung, and the hull parts.
  { title: "Parts", tab: "parts", goods: ["fission_reactor", "fusion_reactor", "advanced_fusion_reactor", "antimatter_reactor",
    "zephyr_drive", "gale_drive", "tempest_drive", "maelstrom_drive",
    "hull_sections", "nav_computer", "cargo_bay", "pulse_laser", "beam_battery"] },
  // §hull-ladder: the heavy and capital sizes (hidden until their lines).
  { title: "Heavy parts", tab: "parts", goods: ["heavy_fission_reactor", "heavy_fusion_reactor", "heavy_advanced_fusion_reactor", "heavy_antimatter_reactor", "heavy_zephyr_drive", "heavy_gale_drive", "heavy_tempest_drive", "heavy_maelstrom_drive", "heavy_hull_sections"] },
  { title: "Capital parts", tab: "parts", goods: ["capital_fission_reactor", "capital_fusion_reactor", "capital_advanced_fusion_reactor", "capital_antimatter_reactor", "capital_zephyr_drive", "capital_gale_drive", "capital_tempest_drive", "capital_maelstrom_drive", "capital_hull_sections"] },
  // §modules-as-goods (Nov 8 2026): the modules, by what they do.
  { title: "Weapons", tab: "modules", goods: ["mass_driver", "torpedo_rack", "point_defense_screen", "lance_array", "prismatic_lance"] },
  { title: "Armour", tab: "modules", goods: ["reflective_plating", "whipple_armor"] },
  { title: "Control", tab: "modules", goods: ["stasis_web", "warp_stabilizer", "interdiction_field"] },
  { title: "Utilities", tab: "modules", goods: ["extended_tanks", "recon_suite", "cargo_pods", "escort_datalink", "fuel_transfer_rig",
    "survey_drive", "nebula_spectrometer"] },
];

/** Sim seconds a rate is measured over. */
export const RATE_WINDOW_S = 300;
/** Steady history a rate needs before it shows. */
export const RATE_MIN_SPAN_S = 60;
/** At most one sample per this many sim seconds. */
const SAMPLE_EVERY_S = 2;
/** A step this size or larger that far outruns the steady flow is a transfer. */
const STEP_MIN_UNITS = 3;
const STEP_OUTRUNS = 4;
/** A good that empties sooner than this reads as running low. */
export const LOW_STOCK_S = 1_800;
/** Storage this full reads as nearly full. */
export const STORAGE_WARN = 0.9;

interface Sample { t: number; used: number; stock: Map<Commodity, number> }

const history = new Map<EntityId, Sample[]>();
let historyPlayer: string | null = null;

/** Called with each served view: one sample per owned system. */
export function noteStockSamples(st: { playerId: string | null; systems: readonly SystemStateView[] }, time: number): void {
  if (st.playerId !== historyPlayer) {
    history.clear();
    historyPlayer = st.playerId;
  }
  for (const system of st.systems) {
    if (system.owner !== st.playerId || !system.stockpile) {
      history.delete(system.id);
      continue;
    }
    const samples = history.get(system.id) ?? [];
    const last = samples[samples.length - 1];
    if (last && time < last.t) samples.length = 0;
    else if (last && time - last.t < SAMPLE_EVERY_S) continue;
    samples.push({ t: time, used: system.storage_used, stock: new Map(system.stockpile.map((slot) => [slot.commodity, slot.units])) });
    // Keep one sample from before the window, so the window is full.
    while (samples.length > 2 && samples[1].t <= time - RATE_WINDOW_S) samples.shift();
    history.set(system.id, samples);
  }
}

/** Forget every sample (tests, and a new corporation). */
export function clearStockSamples(): void {
  history.clear();
  historyPlayer = null;
}

/** Net change per sim second over the window, or null until there is enough. */
function measured(samples: readonly Sample[], value: (sample: Sample) => number): number | null {
  const steps: { d: number; dt: number }[] = [];
  for (let i = 1; i < samples.length; i++) {
    const dt = samples[i].t - samples[i - 1].t;
    if (dt > 0) steps.push({ d: value(samples[i]) - value(samples[i - 1]), dt });
  }
  if (!steps.length) return null;
  const speeds = steps.map((step) => Math.abs(step.d) / step.dt).sort((a, b) => a - b);
  const typical = speeds[Math.floor(speeds.length / 2)];
  let change = 0;
  let span = 0;
  for (const { d, dt } of steps) {
    if (Math.abs(d) >= STEP_MIN_UNITS && Math.abs(d) > STEP_OUTRUNS * typical * dt) continue;
    change += d;
    span += dt;
  }
  return span >= RATE_MIN_SPAN_S ? change / span : null;
}

/** A good's measured rate in this system, units per sim second. */
export function stockRate(systemId: EntityId, commodity: Commodity): number | null {
  const samples = history.get(systemId);
  return samples ? measured(samples, (sample) => sample.stock.get(commodity) ?? 0) : null;
}

/** The system's measured storage fill rate, units per sim second. */
export function storageRate(systemId: EntityId): number | null {
  const samples = history.get(systemId);
  return samples ? measured(samples, (sample) => sample.used) : null;
}

export interface StoreLine {
  commodity: Commodity;
  units: number;
  /** Set aside for the system's colony projects. */
  held: number;
  /** §vault: out of raiders' reach (the settlers' rations, the Vault's goods). */
  safe: number;
  /** §stores-flows: made and used, units per sim second; null when the
   * system reports no flows. */
  made: number | null;
  used: number | null;
  /** Units per sim second: made less used, else measured; null until known. */
  rate: number | null;
  /** Sim seconds until empty at the measured rate; null when not draining. */
  emptyIn: number | null;
  low: boolean;
}

export function storeLine(system: SystemStateView, commodity: Commodity): StoreLine {
  const units = system.stockpile?.find((slot) => slot.commodity === commodity)?.units ?? 0;
  const held = (system.industry?.reservations ?? []).reduce((sum, reservation) => sum + (reservation.goods[commodity] ?? 0), 0);
  const safe = Math.min(units, system.vault?.find((slot) => slot.commodity === commodity)?.units ?? 0);
  const flow = system.flows ? system.flows.find((f) => f.commodity === commodity) ?? { made: 0, used: 0 } : null;
  const made = flow ? flow.made / 60 : null, used = flow ? flow.used / 60 : null;
  const rate = made !== null && used !== null ? made - used : stockRate(system.id, commodity);
  const emptyIn = rate !== null && rate < 0 && units > 0 ? units / -rate : null;
  return { commodity, units, held, safe, made, used, rate, emptyIn, low: emptyIn !== null && emptyIn < LOW_STOCK_S };
}

/** Every good the pop-out lists: held, pinned or moving. */
export function storeGroups(system: SystemStateView, pins: readonly Commodity[]): { title: string; tab: GoodsTab; lines: StoreLine[] }[] {
  const listed = new Set<Commodity>(STORE_GROUPS.flatMap((group) => group.goods));
  const other = (system.stockpile ?? []).map((slot) => slot.commodity).filter((commodity) => !listed.has(commodity));
  return [...STORE_GROUPS, { title: "Other", tab: "resources" as GoodsTab, goods: other }]
    .map(({ title, tab, goods }) => ({
      title,
      tab,
      lines: goods.map((commodity) => storeLine(system, commodity))
        // §reactors-drives: a good research has not opened is not listed,
        // unless it is in stock (§modules-as-goods: a found module).
        .filter((line) => goodKnown(line.commodity) || line.units > 0)
        .filter((line) => line.units > 0 || pins.includes(line.commodity) || (line.rate !== null && Math.abs(line.rate) > 1e-6)
          || (line.made ?? 0) > 0 || (line.used ?? 0) > 0),
    }))
    .filter((group) => group.lines.length > 0);
}

/** §vault: units the system's Vaults hold out of raiders' reach (sim
 * `VAULT_UNITS_PER_TIER` a tier, every world's Vault together). */
export const VAULT_UNITS_PER_TIER = 200;
export function vaultCapacity(system: SystemStateView): number {
  return VAULT_UNITS_PER_TIER * (system.bodies ?? []).reduce((sum, body) => sum + (body.structures?.vault ?? 0), 0);
}

export interface StorageState { used: number; cap: number; fill: number; fullIn: number | null; warn: boolean; full: boolean }

export function storageState(system: SystemStateView): StorageState {
  const used = system.storage_used;
  const cap = system.storage_cap;
  const fill = cap > 0 ? used / cap : 0;
  const rate = storageRate(system.id);
  // §storage-full (Nov 3 2026): within a unit of the cap is full, as the
  // guide counts it: food eaten keeps a full store a fraction under its cap.
  const full = cap > 0 && used >= cap - 1;
  const fullIn = !full && rate !== null && rate > 0 && cap > used ? (cap - used) / rate : null;
  return { used, cap, fill, fullIn, warn: fill >= STORAGE_WARN, full };
}

/** The system the stores bar shows: your system that is open, else the one
 * selected on the map, else home. Null when you hold none. */
export function storesFocus(owned: ReadonlySet<EntityId>, open: EntityId | null, selected: EntityId | null, home: EntityId | null): EntityId | null {
  for (const id of [open, selected, home]) if (id && owned.has(id)) return id;
  return owned.values().next().value ?? null;
}

const PINS_KEY = "stellar.stores.pins.v1";

/** The bar's goods: the player's pins, or the defaults. */
export function storePins(): Commodity[] {
  try {
    const saved = JSON.parse(localStorage.getItem(PINS_KEY) ?? "null") as unknown;
    if (Array.isArray(saved)) {
      const known = new Set<string>(STORE_GROUPS.flatMap((group) => group.goods));
      return saved.filter((good): good is Commodity => typeof good === "string" && known.has(good));
    }
  } catch { /* storage is optional */ }
  return [...DEFAULT_PINS];
}

/** Pin a good to the bar, or unpin it. Returns the new pins. */
export function togglePin(commodity: Commodity): Commodity[] {
  const pins = storePins();
  const next = pins.includes(commodity) ? pins.filter((good) => good !== commodity) : [...pins, commodity];
  try { localStorage.setItem(PINS_KEY, JSON.stringify(next)); } catch { /* storage is optional */ }
  return next;
}

/** A per-minute rate for the bar: "+18", "−2", "+0.4"; empty when flat or unknown. */
export function rateText(perSecond: number | null): string {
  if (perSecond === null) return "";
  const perMinute = perSecond * 60;
  if (Math.abs(perMinute) < 0.05) return "";
  const size = Math.abs(perMinute);
  const text = size < 10 ? size.toFixed(1).replace(/\.0$/, "") : Math.round(size).toLocaleString();
  return `${perMinute > 0 ? "+" : "−"}${text}`;
}

/** A compact amount: 860, 1,240, 12.4k. */
export function stockText(units: number): string {
  const n = Math.max(0, Math.floor(units));
  if (n < 10_000) return n.toLocaleString();
  if (n < 100_000) return `${(n / 1_000).toFixed(1).replace(/\.0$/, "")}k`;
  return `${Math.round(n / 1_000).toLocaleString()}k`;
}
