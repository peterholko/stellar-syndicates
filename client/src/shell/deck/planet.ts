import { commodityIcon, label, structureImage } from "../../icons";
import type { AssignmentView, BodyView, BuildOption, BuildState, ClientMsg, Commodity, ShipKind, SystemInfo, SystemStateView, VeilStateView } from "../../protocol";
import type { ShipArtKind } from "../../shipart";
import { assignedRecipe, defaultRecipeKey, POOL_LABEL, recipeKey, recipeOutputs, recipeSelectable, SHIP_YARD, smelterOres, structureCeiling, structureLevel, type Pool } from "../../core/derive/market";
import { goodKnown } from "../../core/derive/fleet";
import { fmtBuildDur } from "../../core/derive/format";
import { liveBuildAttr, liveProgressAttr, progressAt, progressLine } from "../../core/derive/construction";
import { STATION_LEVELS } from "../../core/derive/station";
import { baseSpecialist, COURSE, drawMW, freeSpecialists, gridLine, isExpertKind, isHullJob, jobTag, jobTitle, maxLevel, mw, plantMW, POWERED_STRUCTURES, romanTier, SENDING, SPECIALIST_KINDS, specialistAffine, yardStatus, type StructureJob } from "../../core/derive/worldbuild";
import { PlanetSites } from "./planet-sites";
import { refiningComparison } from "../refining";
import type { RefiningContext } from "../../core/derive/refining";
import { isBeltSurface, planetSurface, planetSurfaceArt } from "../../planet-surfaces";
import { PLANET_SITES } from "../../planet-sites.generated";
import { hasOrbitalShipConstruction, isOrbitalYard, orbitalYardAddress, planetStructureImage } from "../../planet-structure-art";
import { SHIP_MODELS } from "../../ship-models.generated";
import { SETTLEMENT_PROJECTS } from "../../settlement";
export { planetSurface } from "../../planet-surfaces";

const ART = "/art/derived/planet-workbench/v1/";
/** The largest sprite/ground layer is 1.22 × the 130px footprint. The actual
 * image plane can be height-limited; a conservative responsive hint still avoids
 * fetching full masters or undersized art on high-DPI screens. */
const FOOTPRINT_SIZES = "(min-width: 2300px) 160px, (min-width: 1125px) calc(9.15vw - 50px), 9.15vw";
type PlanetTab = "structures" | "population" | "survey";
const TABS: readonly PlanetTab[] = ["structures", "population", "survey"];
/** An order's travel time is shown only from this many seconds. */
const TRAVEL_SHOWN_S = 10;
const EXTRACTION: Record<string, readonly Commodity[]> = {
  mining_complex: ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "olivine_ore", "bornite_ore", "ilmenite_ore", "iridium_ore", "void_crystal", "silicates", "rare_elements"],
  volatile_harvester: ["volatiles", "helium3_ice"], bioharvester: ["biomass"],
};
// §asteroid-quarry: every deposit on its belt.
EXTRACTION.asteroid_quarry = [...EXTRACTION.mining_complex, ...EXTRACTION.volatile_harvester];

/** §asteroid-quarry: the quarry's own rock fills the belt's near foreground,
 * the same place at every tier (planet-panel.css sizes it). */
export const QUARRY_ADDRESS: FootprintAddress = { x: 28, y: 71, scale: 1 };
/** Where `key` stands in the scene: orbit, the quarry's rock, or its site. */
function sceneAddress(key: string, tier: number, surface: string | null, slot: number): FootprintAddress {
  return orbitalYardAddress(key, tier, surface !== null) ?? (key === "asteroid_quarry" ? QUARRY_ADDRESS : footprintAddress(surface, slot));
}

export interface PlanetPanelModel {
  system: SystemInfo;
  report: SystemStateView;
  body: BodyView;
  mine: boolean;
  art: string;
  delay: number | null;
  catalog: readonly BuildOption[];
  workforceStructures: ReadonlySet<string>;
  descriptions: Readonly<Record<string, string>>;
  populationHtml: string;
  surveyHtml: string;
  /** §construction: the bar pinned under the header (owner only; empty otherwise). */
  constructionHtml: string;
  /** While the world builder is open it replaces the management column. */
  builderHtml: string;
  /** The work slot, orders in flight and the builder's pick, for the scene and rows. */
  construction: PlanetConstruction | null;
  economy?: RefiningContext;
  /** A siege of THIS world from the served, participant-only fields: drawn on
   * the painting (bombardment, landing craft) with its status banner. */
  siege?: PlanetSiege | null;
  /** §veil: the system's Veil Projector state, for its structure row. */
  veil?: VeilStateView | null;
  /** §planet-orbit: fleets berthed at a facility on this world (the System
   * View's berth rule), drawn in its sky. */
  docked?: readonly PlanetDockedFleet[];
  /** §research-wiring Talent: expert courses researched, courses at once. */
  talent?: { expert: boolean; slots: number };
  /** §designations: the researched crown designations and whether they point here. */
  crowns?: readonly { cap: "crown_vein" | "crown_project"; title: string; here: boolean; lockedUntil: number | null }[];
  /** Sim seconds now, for course clocks. */
  now?: number;
  /** §camp: the Founding Camp's card waits for the charter's end. */
  campLocked?: boolean;
}

export interface PlanetConstruction {
  line: readonly StructureJob[];
  /** Orders this client sent that no report has shown yet; new buildings carry their site. */
  pending: readonly { key: string; tier: number; slot: number | null }[];
  /** The structure picked in the open build sheet. `fresh`: a new building that
   * needs a site; `placing`: the sheet is folded away while one is chosen. */
  build: { key: string; tier: number; fresh: boolean; pool: Pool | null; site: number | null; placing: boolean } | null;
}

export interface PlanetDockedFleet {
  id: string;
  kind: ShipKind;
  /** The hull's art when it differs from its kind (§merchant: the free trader). */
  art?: ShipArtKind;
  own: boolean;
  /** Hulls in the fleet when known (own fleets, sensor-covered rivals). */
  count: number | null;
  name: string;
  selected: boolean;
  /** §yard-repair: the hull it has back while its world's yard mends it;
   * null otherwise. It stands on the yard, not in the sky. */
  repairing?: number | null;
}

export interface PlanetSiege {
  caption: string;
  detail: string;
  /** 0..1 of the garrison pinned: sets how many impacts flash. */
  suppression: number;
  /** The landing record to follow, when marines are down. */
  landingId: string | null;
  /** §veil: the ionised sky is up — drawn as an aurora over the world. */
  veil?: boolean;
}

export interface PlanetStructure {
  key: string;
  title: string;
  tier: number;
  workers: number;
  staffable: boolean;
  assignment?: AssignmentView;
  outputs: [Commodity, number][];
  status: string;
  hasShipConstruction: boolean;
  x: number;
  y: number;
  /** Depth recession for the sprite; see footprintAddress. */
  scale: number;
}

/** Footprint addresses on the 16:9 artwork, in percent. Each surface painting has
 * its own 24 addresses, generated by scripts/build-planet-sites.mjs from the
 * painting itself so every building stands on open land, never on forest, water,
 * lava, cliffs or rock; slots 0, 1, 2, 3, 7 and 18 hold the opening colony at the
 * heart of the most open ground. These are fixed art coordinates, never
 * recalculated from a viewport or report; PlanetSites keeps each world's assigned
 * slots as capacity grows. */
function siteAddress(surface: string, slot: number): readonly [number, number] | undefined {
  const sites = PLANET_SITES[surface]?.sites;
  // Unknown future kinds beyond the 24 generated slots share the last address.
  return sites?.[Math.min(sites.length - 1, Math.max(0, slot))];
}
/** A gas giant shows its globe, not a horizon: orbital infrastructure keeps an
 * even spread around it at one size. */
const COLUMNS: readonly number[] = [7, 19, 31, 43, 55, 67, 79, 91];
const ORBIT_ROWS: readonly (readonly number[])[] = [
  [46, 42, 39.5, 38, 38, 39.5, 42, 46],
  [68, 64, 61.5, 60, 60, 61.5, 64, 68],
  [89, 86, 83.5, 82, 82, 83.5, 86, 89],
];

/** Sprite scale for an address's height on the painting. Every painting is an
 * oblique near-orbital view, so equal ground looks smaller toward the limb;
 * footprints recede the same way instead of sitting on the terrain at one size.
 * Mirrored in scripts/build-planet-sites.mjs, which spaces the addresses with it.
 * The CSS derives each footprint's haze from the same scale. */
export function siteScale(y: number): number {
  return Math.round(100 * Math.min(1, Math.max(0.6, 0.64 + (y / 100 - 0.5) * 1.15))) / 100;
}

export interface FootprintAddress { x: number; y: number; scale: number }

export function footprintAddress(surface: string | null, slot: number): FootprintAddress {
  const site = surface === null ? undefined : siteAddress(surface, slot);
  if (site) return { x: site[0], y: site[1], scale: siteScale(site[1]) };
  const column = slot % COLUMNS.length;
  const row = Math.min(ORBIT_ROWS.length - 1, Math.floor(slot / COLUMNS.length));
  return { x: COLUMNS[column], y: ORBIT_ROWS[row][column], scale: 1 };
}

/** Fixed addresses on the 16:9 artwork, not viewport coordinates or slot promises.
 * The image and footprints share one canvas; resizing never rearranges buildings. Only
 * built structures in the arrived body report get a footprint. Upgrades and new
 * construction never move other buildings. */
export function planetStructures(model: PlanetPanelModel): PlanetStructure[] {
  if (!model.mine) return [];
  const order = Object.keys(model.descriptions);
  const extra = Object.keys(model.body.structures).filter(key => !order.includes(key)).sort();
  const keys = [...order, ...extra];
  const surface = planetSurface(model.art, model.system.id, model.body);
  const structures = keys.flatMap((key, index) => {
    const tier = model.body.structures[key] ?? 0;
    if (tier <= 0) return [];
    const assignment = model.report.assignments.find(line => line.body_id === model.body.id && line.structure === key);
    const workers = assignment?.workers ?? 0;
    const staffable = model.workforceStructures.has(key);
    const specialists = Object.values(assignment?.specialists ?? {}).reduce((sum, count) => sum + count, 0);
    const converter = model.report.converters?.find(line => line.body_id === model.body.id && line.structure === key);
    const stopped = assignment?.suspended ?? (converter?.status !== "running" ? converter?.status : null);
    const recipe = assignedRecipe(model.catalog.find(option => option.key === key), assignment?.refining_ore);
    const commodities = recipe ? recipeOutputs(recipe).map(([c]) => c) : (model.body.deposits ?? [])
      .filter(deposit => EXTRACTION[key]?.includes(deposit.resource)).map(deposit => deposit.resource);
    // Rates are the received factor-chain output, converted /s → /min. Never
    // derive from a draft, a timer, an optimistic assignment or other world.
    // Known outages suppress the rated converter output in that SAME report.
    const outputs: [Commodity, number][] = assignment?.outputs.length
      ? assignment.outputs.map(([good, rate]) => [good, stopped ? 0 : Math.max(0, rate * 60)])
      : [...new Set(commodities)].map(good => [good, 0]);
    const stoppedLabel = (reason: string) => ({ no_food: "Needs Provisions", no_inputs: "Needs inputs", storage_full: "Storage full",
      needs_crew: "No workforce", no_supplies: "Needs Provisions and Machinery" }[reason] ?? label(reason));
    const status = !staffable ? stopped ? stoppedLabel(stopped) : "Automatic"
      : !workers && !specialists ? "No workforce"
      : stopped ? stoppedLabel(stopped)
      : "Operating";
    // §habitat-rebase (Nov 1 2026): the camp's Habitat and Warehouse carry
    // their own names; no Founding Quarters.
    const option = model.catalog.find(option => option.key === key);
    return [{ key, title: assignment?.title ?? (option ? structureLevel(option, tier).label : label(key)),
      tier, workers, staffable, assignment, outputs, status,
      hasShipConstruction: key === "naval_drydock" && hasOrbitalShipConstruction(model.report.builds, model.body.id),
      ...sceneAddress(key, tier, surface, index) }];
  });
  positionAttachments(structures);
  return structures;
}

/** §orbital-defense: a station climbs levels of a named ladder, not tiers. */
/** §workforce cap: the most a line may hold — its tier, and never more than
 * its own crew plus the system's free workforce (the sim clamps the same). */
const staffCap = (model: PlanetPanelModel, item: { tier: number; workers: number }): number => {
  const workforce = model.report.workforce;
  const free = workforce ? Math.max(0, workforce.units - workforce.posted) : Infinity;
  return Math.min(item.tier, item.workers + free);
};
const tierText = (item: { key: string; tier: number }): string =>
  item.key === "orbital_defense" ? `Level ${item.tier} of ${STATION_LEVELS}` : `Tier ${item.tier}`;

/** An attachment uses its parent's address, not a 25th ground plot or a second
 * storage building. Its small clickable sprite is offset within that site by
 * CSS, and remains selectable in the management list as before. */
function positionAttachments(structures: PlanetStructure[]): void {
  const warehouse = structures.find(item => item.key === "warehouse");
  const berth = structures.find(item => item.key === "service_berth");
  if (warehouse && berth) Object.assign(berth, { x: warehouse.x, y: warehouse.y, scale: warehouse.scale });
}

type Posting = { workers: number; specialists: Record<string, number>; refining_ore?: Commodity };
type Review = Posting & { key: string; previous: string };

/** The planet is the deepest rung of the map's zoom ladder. `renderScene` draws the
 * world over the map area (art, footprints, selected badge, sibling-world strip) and
 * `render` is the management column for the `world` workspace route. Both read the
 * same served model through one selection/draft controller. Local selection and order
 * drafts only: sending a directive cannot advance the visible economy; a matching
 * arrived assignment is the only evidence that clears its pending marker. */
/** §power: the planet's grid line for the world header. */
function gridHtml(body: BodyView): string {
  const grid = gridLine(body);
  return grid ? `<span class="deck-planet__grid${grid.short ? " is-short" : ""}" title="${esc(grid.tip)}">${esc(grid.text)}</span>` : "";
}

export class PlanetPanel {
  private context = "";
  private selected = "";
  private tab: PlanetTab = "structures";
  private drafts = new Map<string, number>();
  private oreDrafts = new Map<string, Commodity>();
  private specDrafts = new Map<string, Record<string, number>>();
  private pending = new Map<string, Posting>();
  private review: Review | null = null;
  private focusRequest = "";
  private picked: { world: string; key: string } | null = null;
  private sites = new PlanetSites();

  availableSite(model: PlanetPanelModel, slot: number) {
    return this.sites.snapshot(model).free.find(site => site.slot === slot);
  }

  /** Where a new building of `pool` goes when no site is chosen. */
  firstFreeSite(model: PlanetPanelModel, pool: Pool): number | null {
    return this.sites.snapshot(model).free.find(site => site.pool === pool)?.slot ?? null;
  }

  preferSite(model: PlanetPanelModel, key: string, slot: number): void {
    this.sites.prefer(model, key, slot);
  }

  /** The building the player clicked on this world (not the default
   * selection): the Construction card marks its order. */
  pickedOn(systemId: string, bodyId: number): string {
    return this.picked?.world === `${systemId}:${bodyId}` ? this.picked.key : "";
  }

  /** Deep links (a production line in the system rail) name the structure to
   * select once the world's served report renders. */
  focus(structure: string): void {
    this.focusRequest = structure;
  }

  private key(model: PlanetPanelModel, structure: string): string {
    return `${model.report.owner}:${model.system.id}:${model.body.id}:${structure}`;
  }

  private sync(model: PlanetPanelModel): PlanetStructure[] {
    const context = this.key(model, "");
    if (context !== this.context) {
      this.context = context;
      this.selected = "";
      this.tab = model.mine ? "structures" : "survey";
      this.drafts.clear();
      this.oreDrafts.clear();
      this.review = null;
    }
    if (!model.mine) {
      this.tab = "survey";
      this.drafts.clear();
      this.review = null;
    }
    const structures = planetStructures(model);
    const layout = this.sites.snapshot(model);
    for (const item of structures) {
      const site = layout.occupied.find(site => site.key === item.key);
      const surface = planetSurface(model.art, model.system.id, model.body);
      if (site) Object.assign(item, sceneAddress(item.key, item.tier, surface, site.slot));
    }
    positionAttachments(structures);
    if (!canSelect(model, structures, this.selected)) {
      this.selected = structures.find(item => item.outputs.length)?.key ?? structures[0]?.key ?? "";
      this.review = null;
    }
    if (this.focusRequest) {
      if (canSelect(model, structures, this.focusRequest)) {
        this.selected = this.focusRequest;
        if (this.tab === "population" || this.tab === "survey") this.tab = "structures";
        this.review = null;
      }
      this.focusRequest = "";
    }
    for (const item of structures) {
      const key = this.key(model, item.key);
      const pending = this.pending.get(key);
      if (pending && item.workers === pending.workers && sameSpecialists(item.assignment?.specialists ?? {}, pending.specialists)
        && (!pending.refining_ore || pending.refining_ore === (item.assignment?.refining_ore ?? defaultRecipeKey(item.key)))) this.pending.delete(key);
      if (this.oreDrafts.get(key) === (item.assignment?.refining_ore ?? defaultRecipeKey(item.key))) this.oreDrafts.delete(key);
      const specDraft = this.specDrafts.get(key);
      if (specDraft && sameSpecialists(specDraft, item.assignment?.specialists ?? {})) this.specDrafts.delete(key);
      const draft = this.drafts.get(key);
      if (draft !== undefined && (draft === item.workers || draft > item.tier)) this.drafts.delete(key);
      // A report newer than the review keeps it, flagged stale: the panel says
      // so and blocks Confirm until it is updated. Once nothing differs, drop it.
      if (this.review?.key === key && this.review.previous !== postingFingerprint(item) && this.review.workers === item.workers
        && (this.review.refining_ore ?? defaultRecipeKey(item.key)) === (recipeSelectable(item.key) ? item.assignment?.refining_ore ?? defaultRecipeKey(item.key) : defaultRecipeKey(item.key))) {
        this.review = null;
      }
    }
    return structures;
  }

  handleAction(data: DOMStringMap, model: PlanetPanelModel, send: (command: ClientMsg) => void): boolean {
    const action = data.deckAct;
    if (!action?.startsWith("planet-")) return false;
    const structures = this.sync(model);
    if (action === "planet-tab") {
      if (TABS.includes(data.tab as PlanetTab) && (model.mine || data.tab === "survey")) {
        this.tab = data.tab as PlanetTab;
        this.review = null;
      }
      return true;
    }
    if (!model.mine) return true;
    // §designations: point a crown at this world.
    if (action === "planet-crown" && (data.cap === "crown_vein" || data.cap === "crown_project")) {
      send({ type: "SetDesignation", cap: data.cap, target: { body: { system: model.system.id, body: model.body.id } } });
      return true;
    }
    // §research-wiring Talent: an Academy course, at this world's system.
    if (action === "planet-train" && data.kind) {
      send({ type: "TrainSpecialist", system_id: model.system.id, specialist: data.kind });
      return true;
    }
    // §veil: the owner's standing order for this world's sky.
    if (action === "planet-veil") {
      send({ type: "SetVeilPolicy", system_id: model.system.id, raise: data.raise === "1" });
      return true;
    }
    if (action === "planet-select") {
      if (canSelect(model, structures, data.structure)) {
        this.selected = data.structure!;
        this.picked = { world: `${model.system.id}:${model.body.id}`, key: this.selected };
        if (this.tab === "population" || this.tab === "survey") this.tab = "structures";
        this.review = null;
      }
      return true;
    }
    const selected = structures.find(item => item.key === this.selected);
    if (!selected?.staffable || this.tab !== "structures") return true;
    const key = this.key(model, selected.key);
    const draft = this.drafts.get(key) ?? selected.workers;
    const ore = this.oreDrafts.get(key) ?? selected.assignment?.refining_ore ?? defaultRecipeKey(selected.key);
    const selectable = recipeSelectable(selected.key);
    const oreChanged = selectable && ore !== (selected.assignment?.refining_ore ?? defaultRecipeKey(selected.key));
    const posted = selected.assignment?.specialists ?? {};
    const spec = this.specDrafts.get(key) ?? posted;
    // Any change to the plan opens its review right under the stepper; the
    // review is what Confirm sends. A report that arrives in between makes it
    // stale: the review then refreshes (visibly) instead of sending.
    const reviewPlan = (workers: number, refining: Commodity, specialists: Record<string, number>): Review | null =>
      workers !== selected.workers || (selectable && refining !== (selected.assignment?.refining_ore ?? defaultRecipeKey(selected.key)))
        || !sameSpecialists(specialists, posted)
        ? { key, workers, specialists: { ...specialists },
          ...(selectable ? { refining_ore: refining } : {}), previous: postingFingerprint(selected) }
        : null;
    if (action === "planet-workers") {
      const delta = Number(data.delta);
      const next = delta === -1 || delta === 1 ? Math.max(0, Math.min(staffCap(model, selected), draft + delta)) : draft;
      this.drafts.set(key, next);
      this.review = reviewPlan(next, ore, spec);
    } else if (action === "planet-specialist" && data.kind) {
      // §research-wiring Talent: post or recall a specialist on this line.
      const delta = Number(data.delta);
      const total = Object.values(spec).reduce((sum, n) => sum + n, 0);
      const have = spec[data.kind] ?? 0;
      const free = freeSpecialists(model.report, data.kind, model.body.id, selected.key);
      const next = delta === 1 && have < free && total < selected.tier ? have + 1 : delta === -1 ? Math.max(0, have - 1) : have;
      const specialists = { ...spec, [data.kind]: next };
      if (!next) delete specialists[data.kind];
      this.specDrafts.set(key, specialists);
      this.review = reviewPlan(draft, ore, specialists);
    } else if (action === "planet-ore" && selectable) {
      const recipes = model.catalog.find(o => o.key === selected.key)?.refining_recipes ?? [];
      const next = recipes.some(r => recipeKey(selected.key, r) === data.ore) ? data.ore as Commodity : ore;
      this.oreDrafts.set(key, next);
      this.review = reviewPlan(draft, next, spec);
    } else if (action === "planet-review") {
      this.review = draft !== selected.workers || oreChanged || !sameSpecialists(spec, posted) ? reviewPlan(draft, ore, spec) : null;
    } else if (action === "planet-cancel") {
      this.review = null;
      this.drafts.delete(key);
      this.oreDrafts.delete(key);
      this.specDrafts.delete(key);
    } else if (action === "planet-confirm" && this.review?.key === key && this.review.previous === postingFingerprint(selected)) {
      const posting: Posting = { workers: this.review.workers, specialists: this.review.specialists,
        ...(this.review.refining_ore ? { refining_ore: this.review.refining_ore } : {}) };
      send({ type: "SetAssignment", system_id: model.system.id, body_id: model.body.id, structure: selected.key, ...posting });
      this.pending.set(key, posting);
      this.drafts.delete(key);
      this.oreDrafts.delete(key);
      this.specDrafts.delete(key);
      this.review = null;
    }
    return true;
  }

  /** The map-area scene: artwork, footprints and optional build-site shortcuts.
   * Orders are still reviewed and sent through the existing builder. */
  renderScene(model: PlanetPanelModel): string {
    const structures = this.sync(model);
    const selected = structures.find(item => item.key === this.selected);
    const surface = planetSurface(model.art, model.system.id, model.body);
    const surfaceArt = surface ? planetSurfaceArt(surface, true) : null;
    const background = surfaceArt
      ? `<img class="deck-planet__surface" src="${surfaceArt.src}" srcset="${surfaceArt.srcset}" sizes="(min-width: 1125px) calc(100vw - 552px), 100vw" alt="${esc(model.body.name)} ${isBeltSurface(surface) ? "asteroid field" : "landscape"}" draggable="false">`
      : `<img class="deck-planet__globe" src="${esc(model.art)}" alt="${esc(model.body.name)}" draggable="false">`;
    const position = (item: FootprintAddress) => `--planet-x:${item.x}%;--planet-y:${item.y}%;--planet-scale:${item.scale}`;
    // §orbital-defense: the station's square art grows with its level.
    // §laser-battery: level 1 is a ground building, sized like one.
    const station = (key: string, level: number) => key === "orbital_defense" && level >= 2 ? [" is-station", `;--station-level:${level}`] : ["", ""];
    // §construction: the scene shows the work slot where it happens.
    const line = model.construction?.line ?? [];
    const orders = model.construction?.pending ?? [];
    const build = model.construction?.build ?? null;
    // §build-sheet: free sites always show for the owner, as quiet + marks that
    // name their kind on hover; placing lights the pick's kind.
    const placing = !!build?.placing;
    const nameOf = (key: string) => { const option = model.catalog.find(entry => entry.key === key); return option ? structureLevel(option, 1).label : label(key); };
    // §construction: the scene says where the work is, never how far it has
    // got. An order's bar and time live in the Construction card alone.
    // §construction (Peter, Oct 21 2026): a small bar over the Building tag
    // says how far along it is; the Construction card keeps the time.
    const pill = (job: StructureJob) => `<span class="deck-planet__pill ${job.active ? "is-active" : "is-waiting"}">${job.active && job.progress !== null
      ? `<i class="deck-planet__pill-bar" role="progressbar"${liveProgressAttr(job.line)} aria-label="${esc(jobTitle(job))} build progression" aria-valuemin="0" aria-valuemax="100" aria-valuenow="${job.progress.toFixed(0)}"><i style="width:${job.progress.toFixed(1)}%"></i></i>`
      : ""}${jobTag(job)}</span>`;
    const sentPill = `<span class="deck-planet__pill is-sent">${SENDING}</span>`;
    const upgradePill = (key: string) => {
      const job = line.find(entry => entry.key === key);
      return job ? pill(job) : orders.some(entry => entry.key === key) ? sentPill : "";
    };
    const orderLabel = (key: string) => {
      const job = line.find(entry => entry.key === key);
      const order = orders.find(entry => entry.key === key);
      return job ? ` · ${jobTitle(job)}: ${jobTag(job)}` : order ? ` · ${label(key)} ${romanTier(order.tier)}: ${SENDING}` : "";
    };
    const quarry = (key: string) => key === "asteroid_quarry" ? " is-quarry" : "";
    // §vault: underground — listed in the column, never drawn on the ground.
    const footprints = structures.filter(item => item.key !== "vault" && item.key !== "assembly_hall" && item.key !== "veil_projector").map(item => `<button id="planet-spot-${esc(item.key)}" type="button" class="deck-planet__building${quarry(item.key)}${isOrbitalYard(item.key, item.tier) ? " is-space" : item.key === "service_berth" ? " is-attachment" : ""}${station(item.key, item.tier)[0]}${build && !build.fresh && build.key === item.key ? " is-build-pick" : ""}" style="${position(item)}${station(item.key, item.tier)[1]}" data-deck-act="planet-select" data-structure="${esc(item.key)}" aria-label="Inspect ${esc(item.title)}${isOrbitalYard(item.key, item.tier) ? " · in orbit" : item.key === "service_berth" ? " · Warehouse attachment" : ""}${item.hasShipConstruction ? " · ship under construction" : ""}${esc(orderLabel(item.key))}" aria-pressed="${item.key === this.selected}" aria-controls="planet-structure-detail">${planetStructureImage(item.key, item.tier, isOrbitalYard(item.key, item.tier) ? "18vw" : FOOTPRINT_SIZES, item.hasShipConstruction, surface)}${upgradePill(item.key)}</button>`).join("");
    const badge = selected && selected.key !== "vault" && selected.key !== "assembly_hall" && selected.key !== "veil_projector" && !build ? `<div class="deck-planet__selection${isOrbitalYard(selected.key, selected.tier) ? " is-space" : ""}" style="${position(selected)};--planet-output-height:${selected.outputs.length > 2 ? 84 : 44}px">
      <button type="button" class="deck-planet__workforce-badge" data-deck-act="planet-select" data-structure="${esc(selected.key)}" aria-label="${esc(selected.title)}${selected.staffable ? `: ${selected.workers} workforce assigned` : ""}"><span>${esc(selected.title)}</span>${selected.staffable ? `<b>${selected.workers}</b>${workforceIcon()}` : ""}</button>
      ${selected.outputs.length ? `<div class="deck-planet__output-badge" aria-label="Reported output">${outputHtml(selected.outputs)}</div>` : ""}
    </div>` : "";
    const sites = this.sites.snapshot(model);
    // Orders in flight hold their site; the builder's pick previews on the
    // chosen site, or the one it would take when none is chosen.
    const held = new Set(orders.flatMap(order => order.slot === null || model.body.structures[order.key] ? [] : [order.slot]));
    const ghostSlot = placing && build?.fresh && build.pool && !isOrbitalYard(build.key, 1)
      ? build.site ?? sites.free.find(site => site.pool === build.pool && !held.has(site.slot))?.slot ?? null : null;
    // §asteroid-quarry: a belt takes only its quarry, so it offers one site,
    // the quarry's rock, until the quarry is built or under way.
    const belt = model.body.kind === "asteroid_belt";
    const beltOpen = belt && !model.body.structures.asteroid_quarry && !sites.occupied.length && !orders.length;
    const freeSites = belt ? sites.free.slice(0, beltOpen ? 1 : 0) : sites.free;
    const emptySites = model.mine ? freeSites.filter(site => !held.has(site.slot) && site.slot !== ghostSlot).map(site => {
      const match = placing && !!build?.fresh && site.pool === build.pool;
      const text = match ? "Place here" : belt ? "Asteroid Quarry site" : `Free ${POOL_LABEL[site.pool]} site`;
      return `<button id="planet-site-${site.slot}" type="button" class="deck-planet__site is-free is-${site.pool}${belt ? " is-quarry" : ""}${placing ? match ? " is-match" : " is-dim" : ""}" style="${position(belt ? QUARRY_ADDRESS : footprintAddress(surface, site.slot))}" data-deck-act="world-build-site" data-slot="${site.slot}" data-pool="${site.pool}" aria-label="${match ? `Place ${esc(nameOf(build!.key))} here` : `Build on this free ${POOL_LABEL[site.pool]} site`}"><b>+</b><small>${text}</small></button>`;
    }).join("") : "";
    const construction = sites.occupied.filter(site => !model.body.structures[site.key!]).map(site => {
      const job = model.report.builds.find(job => job.body_id === model.body.id && job.key === site.key)!;
      const entry = line.find(candidate => candidate.key === site.key);
      const name = model.catalog.find(option => option.key === site.key)?.label ?? label(site.key!);
      const status = entry ? jobTag(entry) : job.queued ? "Queued" : job.complete_time === null ? "Paused" : "Building";
      return `<div id="planet-construction-${esc(site.key!)}" class="deck-planet__site is-construction${quarry(site.key!)}${entry && !entry.active ? " is-waiting" : ""}${isOrbitalYard(site.key!, 1) ? " is-space" : ""}${station(site.key!, 1)[0]}" style="${position(sceneAddress(site.key!, 1, surface, site.slot))}${station(site.key!, 1)[1]}" role="img" aria-label="${status}: ${esc(name)}" title="${esc(name)} · ${status}">${planetStructureImage(site.key!, 1, isOrbitalYard(site.key!, 1) ? "18vw" : FOOTPRINT_SIZES, false, surface)}${entry ? pill(entry) : `<small>${status}</small>`}</div>`;
    }).join("");
    const sent = orders.filter(order => order.slot !== null && !model.body.structures[order.key] && !line.some(job => job.key === order.key))
      .map(order => `<div class="deck-planet__site is-construction is-sent${quarry(order.key)}" style="${position(sceneAddress(order.key, 1, surface, order.slot!))}" role="img" aria-label="${SENDING}: ${esc(nameOf(order.key))}" title="${esc(nameOf(order.key))} · ${SENDING}">${planetStructureImage(order.key, 1, FOOTPRINT_SIZES, false, surface)}${sentPill}</div>`).join("");
    const ghost = build && ghostSlot !== null ? `<div class="deck-planet__site is-construction is-ghost${quarry(build.key)}" style="${position(sceneAddress(build.key, 1, surface, ghostSlot))}" role="img" aria-label="${esc(nameOf(build.key))} would be built here">${planetStructureImage(build.key, 1, FOOTPRINT_SIZES, false, surface)}<span class="deck-planet__pill is-ghost"><b>${esc(nameOf(build.key))}</b>${build.site === null ? " · suggested site" : " · this site"}</span></div>` : "";
    const siege = model.siege ? siegeOverlay(model.siege, structures, model.body.id) : "";
    const sky = (model.docked ?? []).filter((fleet) => fleet.repairing == null);
    // §yard-repair: hulls in for repairs stand on the yard.
    const yardItem = structures.find((item) => (item.key === "shipyard" || item.key === "ordnance_foundry") && !isOrbitalYard(item.key, item.tier));
    const repairing = (model.docked ?? []).filter((fleet) => fleet.repairing != null);
    const landed = yardItem ? yardShipsHtml(repairing, position(yardItem)) : "";
    // §yard-build (Peter, Nov 9 2026): a hull the ground Shipyard is building
    // stands on its pad as a skeleton, filling in stern to nose.
    const shipyardItem = structures.find((item) => item.key === "shipyard" && !isOrbitalYard(item.key, item.tier));
    const slip = shipyardItem ? model.report.builds.filter((job) => job.body_id === model.body.id && isHullJob(job.key)
      && (job.yard ?? SHIP_YARD[job.key]?.yard) === "shipyard" && !job.queued && job.complete_time != null) : [];
    const raising = shipyardItem ? yardBuildsHtml(slip, position(shipyardItem), shipyardItem.tier, model.now ?? 0, nameOf) : "";
    // §yard-launch: a hull that leaves the slip lifts off for its sky slot.
    if (model.mine) noteLaunches(model.body.id, slip, sky, shipyardItem ? launchFrom(shipyardItem) : null, model.now ?? 0);
    // The sky first: a launch's course is fixed by its slot (§takeoff).
    const orbit = orbitFleetsHtml(sky, surface !== null);
    const smoke = launchSmokeHtml(model.body.id, new Set(sky.map((fleet) => fleet.id)));
    // Sibling worlds come from the same arrived system report as the scene.
    const worlds = model.report.bodies.map(body => `<button id="planet-world-${body.id}" type="button" data-deck-act="world-switch" data-body="${body.id}"${body.id === model.body.id ? ' aria-current="page"' : ""}>${esc(body.name)}</button>`).join("");
    return `<div class="deck-planet-scene${build && !placing ? " is-building" : ""}" aria-label="${esc(model.body.name)} planet view">
      <nav id="planet-worlds" class="deck-planet__worlds" aria-label="Worlds in ${esc(model.system.name)}"><span>${esc(model.system.name)} · worlds</span>${worlds}${model.mine && !build ? `<button type="button" class="deck-planet__sites-toggle" data-deck-act="world-build" data-mode="structures">Build</button>` : ""}</nav>
      <div class="deck-planet__scene${surface ? "" : " is-orbital"}${isBeltSurface(surface) ? " is-belt" : ""}" aria-label="${surface ? isBeltSurface(surface) ? "Asteroid field" : "Planet surface" : "Orbital infrastructure"}"><div class="deck-planet__canvas">${background}${footprints}${construction}${sent}${ghost}${emptySites}${landed}${raising}${smoke}${orbit}${siege}${badge}${placing && !sites.free.length ? '<span class="deck-planet__sites-empty">No free slots</span>' : ""}</div></div>
      ${model.builderHtml}
    </div>`;
  }

  /** The workspace column for the `world` route: identity, workforce, tabs,
   * structure list and detail, then this world's slots, build actions and queue. */
  render(model: PlanetPanelModel): string {
    const structures = this.sync(model);
    const selected = structures.find(item => item.key === this.selected);
    const rising = selected ? undefined : risingStructures(model, structures).find(entry => entry.key === this.selected);
    const assignedHere = structures.reduce((sum, item) => sum + item.workers, 0);
    const free = model.report.workforce ? Math.max(0, model.report.workforce.units - model.report.workforce.posted) : null;
    const tabs = TABS.filter(tab => model.mine || tab === "survey").map(tab => `<button type="button" data-deck-act="planet-tab" data-tab="${tab}" aria-selected="${this.tab === tab}" aria-controls="planet-page">${label(tab)}${tab === "structures" ? ` <small>${structures.length}</small>` : ""}</button>`).join("");
    const line = model.construction?.line ?? [];
    const orders = model.construction?.pending ?? [];
    const list = structures.length ? structures.map(item => {
      const pending = this.pending.has(this.key(model, item.key));
      // §construction: an upgrade on order reads on the building's own row,
      // tagged as on the planet; its bar and time are the Construction card's.
      const job = line.find(entry => entry.key === item.key);
      const order = orders.find(entry => entry.key === item.key);
      const work = job ? ` · ↑ ${romanTier(job.tier)} ${rowTag(jobTag(job), job.active ? "is-active" : "")}`
        : order ? ` · ↑ ${romanTier(order.tier)} ${rowTag(SENDING, "is-sent")}` : "";
      return `<button id="planet-row-${esc(item.key)}" type="button" class="deck-planet__row" data-deck-act="planet-select" data-structure="${esc(item.key)}" aria-pressed="${item.key === this.selected}" aria-controls="planet-structure-detail">
        ${structureImage(item.key, item.tier, "md")}<span class="deck-planet__row-name"><b>${esc(item.title)}</b><small>${pending ? "Order sent · awaiting report" : `${tierText(item)} · ${esc(item.status)}${work}`}</small></span>
        <span class="deck-planet__row-values">${outputHtml(item.outputs)}${item.staffable ? `<span class="deck-planet__staffed-value">${item.workers} ${workforceIcon()}</span>` : ""}${powerHtml(item.key, item.tier, model.body)
        || (item.staffable || item.outputs.length ? "" : "—")}</span></button>`;
    }).join("") + this.risingRows(model, structures) : this.risingRows(model, structures) || `<div class="deck-empty-inline">No structures built.</div>`;
    const page = this.tab === "survey" ? model.surveyHtml
      : this.tab === "population" ? model.populationHtml
      : `<div class="deck-planet__management"><section class="deck-planet__list" aria-label="Planet structures">${list}</section>${selected ? this.detailHtml(model, selected) : rising ? risingDetailHtml(model, rising) : ""}</div>`;
    const delay = model.delay === null ? "Unknown" : model.delay < 1 ? "<1s" : `~${Math.ceil(model.delay)}s`;
    return `<section class="deck-page deck-planet" aria-label="${esc(model.body.name)} planet management">
      <header class="deck-page__lead deck-planet__lead"><img class="deck-planet__art" src="${esc(model.art)}" alt="" draggable="false"><span>${esc(model.system.name)} · World</span><h2>${esc(model.body.name)}</h2><p><span>${model.body.belt ? esc(model.body.belt.kind === "shattered" ? "Shattered world" : `${label(model.body.belt.kind)} belt`) : `${esc(label(model.body.environment))} · ${esc(label(model.body.size))}`}${model.body.geology ? ` · ${esc(label(model.body.geology))}` : ""}</span><span class="deck-planet__delay">Information delay <b>${delay}</b></span></p></header>
      ${model.mine ? model.constructionHtml : ""}
      ${model.mine && model.crowns?.length ? crownsHtml(model) : ""}
      ${model.mine && !model.body.belt ? `<div class="deck-planet__workforce-summary"><span>${workforceIcon()} Workforce <b>${assignedHere}</b> assigned here</span><span class="deck-planet__available">${free === null ? "—" : free} available in system</span>${gridHtml(model.body)}</div>` : ""}
      <nav class="deck-tabs deck-planet__tabs" aria-label="Planet management">${tabs}</nav>
      <div id="planet-page">${page}</div>
      ${model.mine && model.body.founding_camp && !model.campLocked ? `<section class="camp-summary"><b>Founding Camp · ${model.body.founding_camp.completed.length}/${SETTLEMENT_PROJECTS.length} developments</b><p>Housing ${Math.round((model.body.population_capacity ?? 0) * 1e6).toLocaleString()} · ${model.report.storage_cap} goods of system storage. Prospect, improve the mine or expand storage.</p><button type="button" data-settlement-open data-coach="settlement-projects" data-system="${esc(model.system.id)}" data-body="${model.body.id}">Settlement projects</button></section>` : ""}
    </section>`;
  }

  /** §construction: new buildings still rising (or ordered) list beneath the
   * built ones, so the list is the whole picture. Selecting one shows what it
   * is; its controls wait until it is built (Peter, Nov 4 2026). */
  private risingRows(model: PlanetPanelModel, structures: readonly PlanetStructure[]): string {
    if (this.tab !== "structures") return "";
    const listed = new Set<string>();
    return risingStructures(model, structures).map(entry => {
      const option = model.catalog.find(o => o.key === entry.key);
      const title = `${option ? structureLevel(option, 1).label : label(entry.key)} ${romanTier(entry.tier)}`;
      // A later order for the same building shares its selection, not its id.
      const id = listed.has(entry.key) ? "" : ` id="planet-row-${esc(entry.key)}"`;
      listed.add(entry.key);
      return `<button${id} type="button" class="deck-planet__row is-rising" data-deck-act="planet-select" data-structure="${esc(entry.key)}" aria-pressed="${entry.key === this.selected}" aria-controls="planet-structure-detail">
        ${structureImage(entry.key, entry.tier, "md")}<span class="deck-planet__row-name"><b>${esc(title)}</b><small>${rowTag(entry.tag, entry.tone)}</small></span><span class="deck-planet__row-values"></span></button>`;
    }).join("");
  }

  private detailHtml(model: PlanetPanelModel, item: PlanetStructure): string {
    const key = this.key(model, item.key);
    const draft = this.drafts.get(key) ?? item.workers;
    const pending = this.pending.get(key);
    const option = model.catalog.find(option => option.key === item.key);
    const description = option ? structureLevel(option, item.tier).description : undefined;
    const ore = this.oreDrafts.get(key) ?? item.assignment?.refining_ore ?? defaultRecipeKey(item.key);
    const recipe = assignedRecipe(option, ore);
    const product = item.key === "drive_works";
    // Peter (Nov 9 2026): the refine-or-sell estimate is the mine's, not the
    // Smelter's ("Remove this from the smelter").
    const compareRecipe = item.key === "mining_complex"
      ? model.catalog.find(o => o.key === "smelter")?.refining_recipes?.find(r => model.body.deposits?.some(d => d.resource === r.inputs[0][0])) : undefined;
    const comparison = model.economy && model.mine ? refiningComparison(model.economy, model.report, model.body.id, compareRecipe) : "";
    const spec = this.specDrafts.get(key) ?? item.assignment?.specialists ?? {};
    const changed = draft !== item.workers || (recipeSelectable(item.key) && ore !== (item.assignment?.refining_ore ?? defaultRecipeKey(item.key)))
      || !sameSpecialists(spec, item.assignment?.specialists ?? {});
    // The ores this system can feed a Smelter (§smelter-ores), and only the
    // products research has opened (§research-locked-hidden).
    const feedable = item.key === "smelter" ? smelterOres(model.report, ore) : null;
    const choices = (option?.refining_recipes ?? []).filter(r => feedable ? feedable.has(recipeKey(item.key, r)) : goodKnown(r.output));
    const refining = choices.length ? `<div class="deck-planet__refining"><small>${product ? "Product" : "Standing refining recipe"}</small><div class="deck-planet__ore-options">${choices.map(r => recipeKey(item.key, r)).map(c => `<button type="button" data-deck-act="planet-ore" data-ore="${c}" aria-pressed="${c === ore}">${resourceIcon(c)}${esc(label(c))}</button>`).join("")}</div><small>${recipe ? `${recipe.inputs.map(([c,n]) => `${n} ${label(c)}`).join(" + ")} → ${recipeOutputs(recipe).map(([c,n]) => `${n} ${label(c)}`).join(" + ")}` : ""}</small></div>` : "";
    const specialists = model.mine && item.staffable ? specialistsHtml(model, item, spec) : "";
    const review = this.review?.key === key ? this.review : null;
    const stale = !!review && review.previous !== postingFingerprint(item);
    const short = model.report.workforce ? model.report.workforce.posted - item.workers + draft - model.report.workforce.units : 0;
    // Only a delay worth waiting for is worth reading (§short-ui-copy).
    const travel = model.delay !== null && model.delay >= TRAVEL_SHOWN_S
      ? `<small title="Order travel; the panel updates when the report returns">Travel ~${Math.ceil(model.delay)}s</small>` : "";
    const confirm = review && changed
      ? `<div class="deck-planet__confirm" role="group" aria-label="Review workforce assignment"><div><b>Sending order…</b>${recipeSelectable(item.key) ? `<small title="Takes effect when the order arrives">${product ? "Make" : "Refine"} ${esc(label(review.refining_ore ?? defaultRecipeKey(item.key)))}</small>` : ""}${travel}${stale ? `<small class="deck-planet__warning">Newer report arrived</small>` : ""}${short > 0 ? `<small class="deck-planet__warning" title="Not enough free workforce: assignments share it">Short ${short} workforce</small>` : ""}</div><div class="deck-planet__confirm-actions">${stale ? `<button type="button" class="is-primary" data-deck-act="planet-review">Update</button>` : `<button type="button" class="is-primary" data-deck-act="planet-confirm">Confirm</button>`}<button type="button" data-deck-act="planet-cancel">Cancel</button></div></div>`
      : "";
    return `<aside id="planet-structure-detail" class="deck-planet__detail" aria-label="Selected structure">
      <header>${structureImage(item.key, item.tier, "lg")}<div><small>${tierText(item)}</small><h3>${esc(item.title)}</h3><span class="${item.status === "Operating" || item.status === "Automatic" ? "deck-planet__good" : "deck-planet__warning"}">${esc(item.status)}</span></div></header>
      <p>${esc(description ?? model.descriptions[item.key] ?? "Planetary infrastructure.")}</p>
      ${item.hasShipConstruction ? `<small>Ship under construction · reported build queue. The hull remains in its berth while work is paused.</small>` : ""}
      ${item.key === "shipyard" || item.key === "ordnance_foundry" ? yardRepairRows(model.docked ?? []) : ""}
      ${item.key === "veil_projector" ? veilDetail(model, item.tier) : item.key === "vault" ? `<small>Under the Command Center · no slot · no crew</small>` : item.key === "assembly_hall" ? `<small>At the Command Center · no slot · no crew</small>` : item.key === "power_plant" ? `<small>Own slot · one per planet · no crew</small>` : POWERED_STRUCTURES.has(item.key) && (model.body.power?.power ?? 1) < 0.999 ? `<small class="deck-planet__warning" title="This planet's grid can't meet its draw">Power ${Math.round((model.body.power?.power ?? 1) * 100)}%</small>` : ""}
      ${item.key === "warehouse" && (model.body.structures.service_berth ?? 0) > 0 ? `<small>Attached Service Berth · utility fittings · no extra slot</small>` : ""}
      ${refining}${comparison}
      ${item.staffable ? `<div class="deck-planet__assignment"><div class="deck-planet__assignment-row"><b title="Capacity ${item.tier} · 1 workforce = 1,000 populace">Workforce</b><div class="deck-stepper"><button type="button" data-deck-act="planet-workers" data-delta="-1" aria-label="Decrease assigned workforce" ${draft <= 0 ? "disabled" : ""}>−</button><span><b aria-label="Planned workforce">${draft}</b><small aria-label="Capacity">/${item.tier}</small></span><button type="button" data-deck-act="planet-workers" data-delta="1" data-coach="staff-plus" aria-label="Increase assigned workforce" ${draft >= staffCap(model, item) ? `disabled${draft < item.tier ? ' title="No free workforce"' : ""}` : ""}>+</button></div></div>${confirm}${specialists}</div>` : ""}
      ${item.key === "academy" && model.mine ? coursesHtml(model) : ""}
      ${item.outputs.length ? `<div class="deck-planet__reported"><small>Reported output</small><div>${outputHtml(item.outputs)}</div></div>` : ""}
      ${recipe?.inputs.length ? `<div class="deck-planet__inputs"><small>Inputs</small><div>${recipe.inputs.map(([good]) => resourceIcon(good)).join("")}</div></div>` : ""}
      ${option?.upkeep?.length ? `<div class="deck-planet__inputs"><small>Upkeep</small><div>${upkeepHtml(option.upkeep, item.tier)}</div></div>` : ""}
      ${pending ? `<div class="deck-planet__pending" role="status">${pending.workers} workforce${pending.refining_ore ? ` · ${esc(label(pending.refining_ore))}` : ""} requested · awaiting report</div>` : ""}
      ${model.mine ? this.upgradeHtml(model, item, option) : ""}
    </aside>`;
  }

  /** §build-sheet: Upgrade opens the build sheet on the building's next level,
   * which shows its costs and time (Peter, Oct 28: not repeated here). */
  private upgradeHtml(model: PlanetPanelModel, item: PlanetStructure, option: BuildOption | undefined): string {
    if (!option) return "";
    const ordered = (model.construction?.line ?? []).filter(job => job.key === item.key).length
      + (model.construction?.pending ?? []).filter(order => order.key === item.key).length;
    const next = item.tier + ordered + 1;
    if (next > maxLevel(option)) return `<div class="deck-planet__upgrade"><span>Top level</span></div>`;
    if (next > structureCeiling(item.key)) return "";
    return `<div class="deck-planet__upgrade"><button type="button" class="is-primary" data-deck-act="world-build" data-mode="structures" data-select="${esc(item.key)}">Upgrade to ${romanTier(next)}</button></div>`;
  }
}

/** §designations: the crowns this world wears, or may take. */
function crownsHtml(model: PlanetPanelModel): string {
  return `<div class="deck-planet__crowns">${(model.crowns ?? []).map(crown => crown.here
    ? `<span class="is-on" title="Designated here">♛ ${esc(crown.title)}</span>`
    : `<button type="button" data-deck-act="planet-crown" data-cap="${crown.cap}" ${crown.lockedUntil !== null && crown.lockedUntil > (model.now ?? 0) ? `disabled title="Moves in ${esc(fmtBuildDur(crown.lockedUntil - (model.now ?? 0)))}"` : `title="Designate this world"`}>♛ ${esc(crown.title)}</button>`).join("")}</div>`;
}

/** §research-wiring Talent: the line's specialists, one stepper per
 * profession the system holds (★ where its skill serves this line), and the
 * system's housing. */
function specialistsHtml(model: PlanetPanelModel, item: PlanetStructure, spec: Record<string, number>): string {
  const pool = model.report.specialists ?? {};
  const kinds = [...new Set([...Object.keys(pool).filter(kind => (pool[kind] ?? 0) > 0), ...Object.keys(spec)])]
    .sort((a, b) => Number(specialistAffine(b, item.key)) - Number(specialistAffine(a, item.key)) || a.localeCompare(b));
  if (!kinds.length) return "";
  const total = Object.values(spec).reduce((sum, n) => sum + n, 0);
  const housed = model.report.specialist_housing ?? 0;
  const working = Object.values(pool).reduce((sum, n) => sum + n, 0);
  const rows = kinds.map(kind => {
    const n = spec[kind] ?? 0;
    const free = freeSpecialists(model.report, kind, model.body.id, item.key);
    const star = specialistAffine(kind, item.key) ? ` <span title="Skilled here: ×${isExpertKind(kind) ? "2.25" : "1.75"} at a full line">★</span>` : "";
    return `<div class="deck-planet__assignment-row"><b>${esc(label(kind))}${star}</b><div class="deck-stepper"><button type="button" data-deck-act="planet-specialist" data-kind="${esc(kind)}" data-delta="-1" aria-label="Recall ${esc(label(kind))}" ${n <= 0 ? "disabled" : ""}>−</button><span><b>${n}</b><small>/${free}</small></span><button type="button" data-deck-act="planet-specialist" data-kind="${esc(kind)}" data-delta="1" aria-label="Post ${esc(label(kind))}" ${n >= free || total >= item.tier ? "disabled" : ""}>+</button></div></div>`;
  }).join("");
  return `<div class="deck-planet__specialists"><small title="Habitat and Living Terraces tiers house 2 specialists each; the rest wait unhoused">Specialists · housed ${Math.min(working, housed)}/${housed}</small>${rows}</div>`;
}

/** §research-wiring Talent: the Academy's courses — train a profession, raise
 * one to expert once researched, and the system's course line. */
function coursesHtml(model: PlanetPanelModel): string {
  const talent = model.talent ?? { expert: false, slots: 1 };
  const pool = model.report.specialists ?? {};
  const train = SPECIALIST_KINDS.map(kind => `<button type="button" data-deck-act="planet-train" data-kind="${kind}" title="${COURSE.base.costs} · ${COURSE.base.seconds} s">${esc(label(kind))}</button>`).join("");
  const raise = talent.expert ? SPECIALIST_KINDS.filter(kind => (pool[kind] ?? 0) > 0)
    .map(kind => `<button type="button" data-deck-act="planet-train" data-kind="expert_${kind}" title="Raises one ${esc(label(kind))} · ${COURSE.expert.costs} · ${COURSE.expert.seconds} s">${esc(label(`expert_${kind}`))}</button>`).join("") : "";
  const courses = model.report.builds.filter(job => SPECIALIST_KINDS.includes(baseSpecialist(job.key) as typeof SPECIALIST_KINDS[number]));
  const line = courses.map(job => `<li>${esc(label(job.key))}<small>${esc(job.queued ? "waiting" : yardStatus(job, model.now ?? 0))}</small></li>`).join("");
  return `<div class="deck-planet__courses"><small>Courses · ${talent.slots} at a time</small><div class="deck-planet__course-options">${train}</div>${raise ? `<small>Raise to expert</small><div class="deck-planet__course-options">${raise}</div>` : ""}${line ? `<ul>${line}</ul>` : ""}</div>`;
}

function postingFingerprint(item: PlanetStructure): string {
  return JSON.stringify([item.workers, item.tier, item.assignment?.refining_ore ?? defaultRecipeKey(item.key), Object.entries(item.assignment?.specialists ?? {}).sort()]);
}
function sameSpecialists(a: Record<string, number>, b: Record<string, number>): boolean {
  return JSON.stringify(Object.entries(a).filter(([, n]) => n > 0).sort()) === JSON.stringify(Object.entries(b).filter(([, n]) => n > 0).sort());
}
/** An order's tag in the structure list, as on the planet. */
function rowTag(text: string, tone: string): string {
  return `<span class="deck-planet__tag${tone ? ` ${tone}` : ""}">${text}</span>`;
}

interface RisingStructure { key: string; tier: number; tag: string; tone: string }

/** §construction: new buildings on this world still rising, then orders no
 * report has shown yet. Built structures (upgrades) aren't among them. */
function risingStructures(model: PlanetPanelModel, structures: readonly PlanetStructure[]): RisingStructure[] {
  const built = new Set(structures.map(item => item.key));
  const line = (model.construction?.line ?? []).filter(job => !job.project && !built.has(job.key));
  const sent = (model.construction?.pending ?? []).filter(order => !built.has(order.key) && !line.some(job => job.key === order.key));
  return [...line.map(job => ({ key: job.key, tier: job.tier, tag: jobTag(job), tone: job.active ? "is-active" : "" })),
    ...sent.map(order => ({ key: order.key, tier: order.tier, tag: SENDING, tone: "is-sent" }))];
}

/** A built structure, or one still rising, can be selected. */
function canSelect(model: PlanetPanelModel, structures: readonly PlanetStructure[], key: string | undefined): boolean {
  return structures.some(item => item.key === key) || risingStructures(model, structures).some(entry => entry.key === key);
}

/** §construction: a building not yet complete shows its tier, name and
 * description only; no workforce until it stands (Peter, Nov 4 2026). */
function risingDetailHtml(model: PlanetPanelModel, entry: RisingStructure): string {
  const option = model.catalog.find(o => o.key === entry.key);
  const level = option ? structureLevel(option, entry.tier) : undefined;
  return `<aside id="planet-structure-detail" class="deck-planet__detail is-rising" aria-label="Selected structure">
      <header>${structureImage(entry.key, entry.tier, "lg")}<div><small>${tierText(entry)}</small><h3>${esc(level?.label ?? label(entry.key))}</h3></div></header>
      <p>${esc(level?.description ?? model.descriptions[entry.key] ?? "Planetary infrastructure.")}</p>
    </aside>`;
}

export function workforceIcon(): string {
  return `<img class="deck-planet__workforce-icon" src="${ART}workforce-128.png" srcset="${ART}workforce-128.png 1x, ${ART}workforce-256.png 2x" alt="Workforce" title="Workforce">`;
}
/** §power: the grid's bolt, at the workforce icon's size (no raster art yet). */
export function powerIcon(): string {
  return `<svg class="deck-planet__power-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M13.6 2 4.5 13.4h6.2L9.4 22l10.1-12.2h-6.3L13.6 2z"/></svg>`;
}
/** §power (Peter, Nov 8 2026): on the collapsed row, beside its output and
 * crew, what a structure draws from this planet's grid at full crew, or what
 * the Power Plant supplies. Amber when the grid is short. */
function powerHtml(key: string, tier: number, body: BodyView): string {
  if (tier <= 0) return "";
  if (key === "power_plant") {
    const supply = mw(plantMW(tier));
    return `<span class="deck-planet__power-value is-supply" aria-label="Supplies ${supply} MW" title="Supplies ${supply} MW to this planet's grid"><b>+${supply}</b>${powerIcon()}<small>MW</small></span>`;
  }
  if (!POWERED_STRUCTURES.has(key)) return "";
  const draw = mw(drawMW(key, tier));
  const short = (body.power?.power ?? 1) < 0.999;
  return `<span class="deck-planet__power-value${short ? " is-short" : ""}" aria-label="Draws ${draw} MW" title="Draws ${draw} MW from this planet's grid at full crew${short ? ` · the grid runs at ${Math.round((body.power?.power ?? 1) * 100)}%` : ""}"><b>${draw}</b>${powerIcon()}<small>MW</small></span>`;
}
function resourceIcon(good: Commodity): string {
  // A title on the wrapper covers every registry variant, including resources
  // whose shared icon currently supplies alt text but no native hover label.
  return `<span class="deck-planet__resource" title="${esc(label(good))}" aria-label="${esc(label(good))}">${commodityIcon(good)}</span>`;
}
function outputHtml(outputs: readonly [Commodity, number][]): string {
  return outputs.map(([good, rate]) => `<span class="deck-planet__rate" aria-label="${rateText(rate)} ${esc(label(good))} per minute"><b>${rateText(rate)}</b>${resourceIcon(good)}<small>/ min</small></span>`).join("");
}
/** §asteroid-quarry: what a structure eats at its tier, a minute. */
function upkeepHtml(upkeep: readonly [Commodity, number][], tier: number): string {
  return upkeep.map(([good, rate]) => {
    const n = Number((rate * tier * 60).toFixed(2)).toLocaleString("en-US", { maximumFractionDigits: 2 });
    return `<span class="deck-planet__rate is-upkeep" aria-label="${n} ${esc(label(good))} per minute"><b>−${n}</b>${resourceIcon(good)}<small>/ min</small></span>`;
  }).join("");
}
function rateText(rate: number): string {
  return `+${Number(rate.toFixed(2)).toLocaleString("en-US", { maximumFractionDigits: 2 })}`;
}
function esc(text: string): string {
  return text.replace(/[&<>"']/g, ch => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]!));
}

/** Bombardment flashes on this world's own buildings (else seeded open
 * ground), troop craft dropping while a landing is fought, and the siege
 * banner. Markup is stable between reports so the morph keeps animations
 * running; only the banner text changes as the arrived numbers move. */
/** §planet-orbit: up to this many docked fleets are drawn in the world's sky;
 * the rest fold into "+N in orbit". */
export const ORBIT_FLEETS_SHOWN = 3;
/** A docked hull's visible length, in percent of the scene's width: the class
 * ladder, compressed so a titan never rivals the yards it berths at. */
const ORBIT_HULL: Record<ShipKind, number> = {
  scout: 3.6, survey_probe: 2.8, mining_barge: 4.8, raider: 4.2, corvette: 4.6, tiny_freighter: 4, small_freighter: 4.6, convoy: 5.4,
  large_freighter: 6, heavy_freighter: 6.6, bulk_freighter: 7.2, freighter: 6, colony: 6.4, transport: 5.6,
  builder: 5.2, destroyer: 5.6, cruiser: 6.8, battleship: 8, dreadnought: 9, titan: 10.5, mining_belt_station: 7.6,
  combat_freighter: 4.3,
};
/** Sky slots, in the ground frame's percent: warships by the Orbital Shipyard,
 * capital ships by the Capital Slipway, everything else between them. A gas
 * giant's globe fills the middle, so its slots sit in the corners. */
const ORBIT_SLOTS = {
  painted: [{ x: 38, y: -4 }, { x: 62, y: -4 }, { x: 50, y: 3 }],
  globe: [{ x: 12, y: 16 }, { x: 88, y: 16 }, { x: 88, y: 36 }],
};
const ORBIT_MORE = { painted: { x: 50, y: 9 }, globe: { x: 88, y: 48 } };
const CAPITAL_HULLS: ReadonlySet<ShipKind> = new Set(["battleship", "dreadnought", "titan"]);
const WARSHIP_HULLS: ReadonlySet<ShipKind> = new Set(["raider", "corvette", "destroyer", "cruiser"]);

/** Which docked fleets are drawn, in which sky slot, and how many fold away:
 * the selected fleet first, then your own, then the larger hulls. */
export function orbitFleets(docked: readonly PlanetDockedFleet[]): { shown: { fleet: PlanetDockedFleet; slot: number }[]; more: number } {
  const order = [...docked].sort((a, b) => Number(b.selected) - Number(a.selected) || Number(b.own) - Number(a.own)
    || ORBIT_HULL[b.kind] - ORBIT_HULL[a.kind] || a.id.localeCompare(b.id));
  const taken = new Set<number>();
  const shown = order.slice(0, ORBIT_FLEETS_SHOWN).map(fleet => {
    const preferred = CAPITAL_HULLS.has(fleet.kind) ? 1 : WARSHIP_HULLS.has(fleet.kind) ? 0 : 2;
    const slot = taken.has(preferred) ? [0, 1, 2].find(i => !taken.has(i))! : preferred;
    taken.add(slot);
    return { fleet, slot };
  });
  return { shown, more: docked.length - shown.length };
}

/** §yard-repair: when each hull touched down on its yard (wall clock), so the
 * fly-down plays once across re-renders: a redraw mid-flight resumes it. */
const landedAt = new Map<string, number>();
const LANDING_MS = 2_400;

/** §yard-repair (Peter, Nov 5 2026: "the ship being repaired in the ground
 * based shipyard fly down to the ground in the planetary view"): each hull
 * mending stands on the yard's pad, its hull back on a tag; it opens the yard. */
function yardShipsHtml(ships: readonly PlanetDockedFleet[], at: string): string {
  const now = Date.now();
  for (const id of [...landedAt.keys()]) if (!ships.some((ship) => ship.id === id)) landedAt.delete(id);
  return ships.map((ship, index) => {
    const since = now - (landedAt.get(ship.id) ?? (landedAt.set(ship.id, now), now));
    const landing = since < LANDING_MS ? ` is-landing" style="${at};--yard-ship-index:${index};animation-delay:-${since}ms` : `" style="${at};--yard-ship-index:${index}`;
    const pct = Math.round((ship.repairing ?? 1) * 100);
    return `<button type="button" class="deck-planet__yard-ship${landing}" data-deck-act="planet-yard-repair" data-fleet="${esc(ship.id)}" aria-label="${esc(ship.name)} · repairing · ${pct}% hull · view the Shipyard" title="${esc(ship.name)} · repairing · ${pct}% hull"><img src="${esc(SHIP_MODELS[ship.art ?? ship.kind].poster)}" alt="" draggable="false" decoding="async"><small>${pct}%</small></button>`;
  }).join("");
}

/** §yard-launch (Peter, Nov 9 2026: "an animation to the interceptor (or
 * any ship) completed in the shipyard that shows it launching into orbit and
 * taking up its regular orbit spot ... some quasi 'launch animation' instead
 * of just tweening it over there"): where a launch starts (the bay's cradle,
 * as CSS lengths in the stage), how long it runs, and the launches under way
 * by fleet (wall clock), so a redraw mid-flight resumes it. */
interface LaunchFrom { x: string; y: string; w: string; yardX: number; scale: number }
/** §takeoff (Peter, Nov 9 2026: "launch more like an airplane which has the
 * capabilities to fly up to space vs a rocket launching straight up ... have
 * the interceptor travel horizontally out of the shipyard and then up to
 * orbit"): 6 s. Engines spool up in the bay (0-0.5 s), it taxis out and turns
 * level (to 1.3 s), rolls along the ground gathering speed (to 2.6 s), pitches
 * up and lifts off, climbs in a curve to its slot and levels out there. */
const LAUNCH_MS = 6_000;
const SMOKE_MS = 6_000;
/** The takeoff roll, in % of the stage's width: half the way across to the
 * slot, within these bounds. */
const RUN_MIN = 7;
const RUN_MAX = 18;
/** The slip's hulls at each world's last draw: kind, when due (sim s), when
 * seen (wall ms). */
const slipSeen = new Map<string, { body: number; kind: string; due: number; seen: number; from: LaunchFrom | null }>();
/** A hull that leaves the slip this close to its due time (sim s) finished;
 * one leaving earlier was cancelled. Its report can land a moment before the
 * local clock gets there. */
const LAUNCH_EARLY_S = 5;
/** Each world's docked fleets at its last draw, with their hull counts. */
const dockedSeen = new Map<number, Map<string, number>>();
/** Launches in flight. `fleet`: the docked fleet it becomes in this world's
 * sky, or null while it climbs away (its berth is another world, or its
 * fleet hasn't shown yet). `skip`: how far in (ms) its elements start, fixed
 * when they first draw: a redraw that rewrote it would make a running
 * animation jump (setHtml morphs nodes in place). */
interface Launch { body: number; kind: string; at: number; skip: number; from: LaunchFrom; fleet: string | null; course?: LaunchCourse }
const launches: Launch[] = [];
/** §takeoff: how far it rolls before lifting off (% of the stage's width),
 * and whether its slot lies behind where it lifts off. The runway always
 * points right, the way the hulls' art faces: Peter (Nov 9 2026), "Can we
 * avoid that flip of the image somehow?" (a mirrored hull flipped to head
 * left; rotated instead, the art's three-quarter view reads as upside down).
 * A slot behind: it climbs away off the top and the parked hull fades in at
 * its slot. Fixed at its first draw, like `skip`: rewriting it would make a
 * running flight jump. */
interface LaunchCourse { run: number; behind: boolean }

/** The bay's cradle on a ground Shipyard (§yard-bay), as stage lengths. */
function launchFrom(yard: { x: number; y: number; scale: number; tier: number }): LaunchFrom {
  const [bayX, bayY] = SHIPYARD_BAY[Math.max(1, Math.min(6, Math.floor(yard.tier) || 1))];
  const foot = `var(--planet-building) * ${yard.scale}`;
  return { x: `calc(${yard.x}% + ${(bayX - 0.5).toFixed(3)} * ${foot})`, y: `calc(${yard.y}% + ${(bayY - 0.5).toFixed(3)} * ${foot})`,
    w: `calc(${foot} * .5)`, yardX: yard.x, scale: yard.scale };
}

/** §yard-launch: a hull that leaves the slip at its due time (the build's
 * served `complete_time`) while this world is on screen lifts off at once: to
 * the slot of the new fleet of its kind in this sky (or the docked fleet it
 * joined), else up and away (it berths at another world), taking the slot if
 * its fleet shows in this sky mid-climb. A hull cancelled before its time, a
 * world drawn for the first time, or one not drawn for a while (the view was
 * elsewhere) launches nothing. */
function noteLaunches(body: number, slip: readonly BuildState[], docked: readonly PlanetDockedFleet[], from: LaunchFrom | null, now: number): void {
  const wall = Date.now();
  const keys = new Set<string>();
  slip.forEach((job, index) => {
    const key = `${body}:${job.id ?? `${job.key}@${index}`}`;
    keys.add(key);
    slipSeen.set(key, { body, kind: job.key, due: job.complete_time ?? Infinity, seen: wall, from });
  });
  const finished: { kind: string; from: LaunchFrom }[] = [];
  for (const [key, seen] of slipSeen) {
    if (seen.body !== body || keys.has(key)) continue;
    slipSeen.delete(key);
    if (now >= seen.due - LAUNCH_EARLY_S && seen.from && wall - seen.seen < 6_000) finished.push({ kind: seen.kind, from: seen.from });
  }
  const before = dockedSeen.get(body);
  const fresh = before ? docked.filter((fleet) => fleet.own && (!before.has(fleet.id) || (fleet.count ?? 1) > before.get(fleet.id)!)) : [];
  dockedSeen.set(body, new Map(docked.map((fleet) => [fleet.id, fleet.count ?? 1])));
  for (let i = launches.length - 1; i >= 0; i--) if (wall - launches[i].at > LAUNCH_MS) launches.splice(i, 1);
  for (const hull of finished) {
    const index = fresh.findIndex((fleet) => fleet.kind === hull.kind);
    const fleet = index >= 0 ? fresh.splice(index, 1)[0].id : null;
    launches.push({ body, kind: hull.kind, at: wall, skip: 0, from: hull.from, fleet });
  }
  // Its fleet shows in this sky mid-climb: it heads for the slot from here on.
  for (const fleet of fresh) {
    const away = launches.find((launch) => launch.body === body && launch.fleet === null && launch.kind === fleet.kind);
    if (away) Object.assign(away, { fleet: fleet.id, skip: wall - away.at });
  }
}

/** The taxi is 1.1 hull lengths, a hull half a building's footprint, a
 * footprint about 7.5% of the stage. */
const taxiLength = (from: LaunchFrom): number => 1.1 * 7.5 * from.scale * 0.5;
/** Where it lifts off, roughly (% of the stage). */
const liftOff = (from: LaunchFrom, run: number): number => from.yardX + taxiLength(from) + run;

/** §takeoff: its course toward a slot at `slotX` (% of the stage): a roll
 * half the way across to it (or a usual one when it lies behind), short of
 * the stage's right edge. */
function launchCourse(launch: Launch, slotX: number): LaunchCourse {
  if (!launch.course) {
    const span = slotX - launch.from.yardX;
    const wanted = span > 0 ? Math.max(RUN_MIN, Math.min(RUN_MAX, span / 2)) : 12;
    const room = 96 - launch.from.yardX - taxiLength(launch.from);
    const run = Math.round(Math.max(3, Math.min(wanted, room)) * 10) / 10;
    launch.course = { run, behind: slotX - liftOff(launch.from, run) < 3 };
  }
  return launch.course;
}

/** A launch's stage lengths, course and how far in its elements start. */
function launchStyle(launch: Launch, slotX: number): string {
  const { run } = launchCourse(launch, slotX);
  return `--launch-x:${launch.from.x};--launch-y:${launch.from.y};--launch-w:${launch.from.w};--launch-run:${run}cqw;--launch-elapsed:-${Math.round(launch.skip)}ms`;
}

/** §takeoff: a hull climbing away (its berth is another world, or its slot
 * lies behind): out over the top, ahead of where it lifts off. */
function awaySlotX(launch: Launch): number {
  const { run } = launchCourse(launch, launch.from.yardX + 28);
  return Math.min(99, Math.max(liftOff(launch.from, run) + 6, launch.from.yardX + 28));
}

/** §takeoff: it climbs away rather than to its slot in this sky. */
const climbsAway = (launch: Launch): boolean => launch.fleet === null || !!launch.course?.behind;

/** §takeoff: the exhaust's dust, behind the hull as it rolls (fractions of
 * the roll, and when the hull passes each: it rolls 1.32-2.58 s, gathering
 * speed, so x grows with time squared), plus two wafts of wash in the bay. */
const DUST: readonly [number, number, number][] = [[0, 0, 320], [.6, 0, 820],
  ...[0, .12, .26, .42, .6, .8, 1].map((g): [number, number, number] => [1.1, g, Math.round(1_320 + 1_260 * Math.sqrt(g))])];

const flying = (launch: Launch): boolean => Date.now() - launch.at < LAUNCH_MS;

/** §yard-launch: the engines' glow in the bay and §takeoff the dust the
 * exhaust kicks up along the roll, while they last, and a hull climbing away
 * (to a berth at another world, or round to a slot behind it). After the
 * sky, so each course is known. */
function launchSmokeHtml(body: number, sky: ReadonlySet<string>): string {
  const here = launches.filter((launch) => launch.body === body);
  // Round to a slot behind: only while its fleet is still in this sky.
  const away = here.filter((launch) => climbsAway(launch) && flying(launch) && (launch.fleet === null || sky.has(launch.fleet))).map((launch) => {
    const poster = (SHIP_MODELS as Partial<Record<string, { poster: string }>>)[launch.kind]?.poster;
    const x = awaySlotX(launch);
    return poster ? `<span id="planet-launch-${launch.at}" class="deck-planet__orbit-ship is-launching is-folding" style="--planet-x:${x.toFixed(1)}%;--planet-y:-14%;--orbit-hull:${ORBIT_HULL[launch.kind as ShipKind] ?? 4};${launchStyle(launch, x)}" aria-hidden="true"><span class="deck-planet__launch-body"><i class="deck-planet__plume"></i><img src="${esc(poster)}" alt="" draggable="false" decoding="async"></span></span>` : "";
  }).join("");
  const smoke = here.filter((launch) => Date.now() - launch.at < SMOKE_MS).map((launch) =>
    `<span class="deck-planet__launch-smoke" style="${launchStyle(launch, awaySlotX(launch))}" aria-hidden="true"><b></b>${DUST.map(([taxi, roll, delay]) =>
      `<i style="--at-w:${taxi};--at-run:${roll};--puff:${delay}ms"></i>`).join("")}</span>`).join("");
  return smoke + away;
}

/** §yard-bay (Peter, Nov 9 2026: "is it possible to have the ship in the
 * 'garage part' of the shipyard, i.e. rotate the interceptor image 90 degrees
 * to the left"): where the open bay's cradle sits in each tier's sprite, as
 * fractions of its square (art-src/planet-shipyards-v1/shipyard/tier-N.png;
 * the building's footprint is that square, centred on its spot). */
const SHIPYARD_BAY: Readonly<Record<number, readonly [number, number]>> = {
  1: [0.38, 0.61], 2: [0.38, 0.64], 3: [0.37, 0.64], 4: [0.375, 0.68], 5: [0.34, 0.72], 6: [0.33, 0.72],
};

/** §yard-build (Peter, Nov 9 2026: "when the Interceptor (or any ship is
 * being built in the Shipyard) can you render a similar ship
 * silhouette/skeleton in the actual Shipyard on the ground"): each hull on
 * the slip stands in the yard's open bay (§yard-bay), turned 50° left: its
 * skeleton (blueprint ribs in the hull's outline), the built hull filling in
 * stern to nose behind a weld line as the frame tick moves `--build`. It
 * opens the yard's close-up on it. A hull in for repairs keeps the pad. */
function yardBuildsHtml(jobs: readonly BuildState[], at: string, tier: number, now: number, nameOf: (key: string) => string): string {
  const [bayX, bayY] = SHIPYARD_BAY[Math.max(1, Math.min(6, Math.floor(tier) || 1))];
  return jobs.map((job, index) => {
    const poster = (SHIP_MODELS as Partial<Record<string, { poster: string }>>)[job.key]?.poster;
    if (!poster) return "";
    const line = progressLine(job);
    const share = line ? progressAt(line, now) / 100 : 0;
    const pct = Math.floor(share * 100);
    const name = nameOf(job.key);
    return `<button type="button" class="deck-planet__yard-ship is-building" style="${at};--bay-x:${bayX};--bay-y:${bayY};--yard-ship-index:${index};--build:${share.toFixed(4)}"${liveBuildAttr(line)} data-deck-act="planet-yard-build" data-kind="${esc(job.key)}" aria-label="${esc(name)} · under construction · view the Shipyard" title="${esc(name)} · under construction · ${esc(yardStatus(job, now))}">`
      + `<span class="deck-planet__yard-hull" aria-hidden="true"><i class="deck-planet__yard-frame" style="--poster:url('${esc(poster)}')"></i><img class="is-hull" src="${esc(poster)}" alt="" draggable="false" decoding="async">`
      + `<i class="deck-planet__weld"></i></span><small>${pct}%</small></button>`;
  }).join("");
}

/** §yard-repair: the yard's detail names what it is mending, each with the
 * close-up. */
function yardRepairRows(docked: readonly PlanetDockedFleet[]): string {
  const ships = docked.filter((ship) => ship.repairing != null);
  return ships.map((ship) => `<div class="deck-planet__repair"><span><b>${esc(ship.name)}</b><small>Repairing · ${Math.round((ship.repairing ?? 1) * 100)}% hull</small></span>`
    + `<button type="button" data-deck-act="planet-yard-repair" data-fleet="${esc(ship.id)}">View shipyard</button></div>`).join("");
}

function orbitFleetsHtml(docked: readonly PlanetDockedFleet[], painted: boolean): string {
  if (!docked.length) return "";
  const { shown, more } = orbitFleets(docked);
  const slots = painted ? ORBIT_SLOTS.painted : ORBIT_SLOTS.globe;
  const ships = shown.map(({ fleet, slot }) => {
    const at = slots[slot];
    const many = fleet.count !== null && fleet.count > 1;
    const name = `${fleet.name}${many ? ` ×${fleet.count}` : ""} · docked in orbit`;
    // §yard-launch: freshly built, it flies up from the yard's bay to here;
    // §takeoff a slot behind its lift-off fades in as it climbs away.
    const launch = launches.find((entry) => entry.fleet === fleet.id && flying(entry));
    const arriving = !!launch && launchCourse(launch, at.x).behind;
    const flight = launch && !arriving;
    const poster = `<img src="${esc(SHIP_MODELS[fleet.art ?? fleet.kind].poster)}" alt="" draggable="false" decoding="async">`;
    return `<button id="planet-orbit-${esc(fleet.id)}" type="button" class="deck-planet__orbit-ship${fleet.own ? " is-own" : ""}${flight ? " is-launching" : arriving ? " is-arriving" : ""}" style="--planet-x:${at.x}%;--planet-y:${at.y}%;--orbit-hull:${ORBIT_HULL[fleet.kind]}${launch ? `;${launchStyle(launch, at.x)}` : ""}" data-deck-act="world-fleet-open" data-fleet="${esc(fleet.id)}" aria-pressed="${fleet.selected}" aria-label="${esc(name)}" title="${esc(name)}">${flight ? `<span class="deck-planet__launch-body"><i class="deck-planet__plume"></i>${poster}</span>` : poster}${many ? `<small>×${fleet.count}</small>` : ""}</button>`;
  }).join("");
  const at = painted ? ORBIT_MORE.painted : ORBIT_MORE.globe;
  const rest = more > 0 ? `<span class="deck-planet__orbit-more" style="--planet-x:${at.x}%;--planet-y:${at.y}%">+${more} in orbit</span>` : "";
  // A launch whose fleet folds into "+N in orbit" flies there and fades.
  const folded = docked.filter((fleet) => !shown.some((entry) => entry.fleet.id === fleet.id)).flatMap((fleet) => {
    const launch = launches.find((entry) => entry.fleet === fleet.id && flying(entry));
    return launch && !launchCourse(launch, at.x).behind ? [`<span id="planet-launch-${launch.at}" class="deck-planet__orbit-ship is-launching is-folding" style="--planet-x:${at.x}%;--planet-y:${at.y}%;--orbit-hull:${ORBIT_HULL[fleet.kind]};${launchStyle(launch, at.x)}" aria-hidden="true"><span class="deck-planet__launch-body"><i class="deck-planet__plume"></i><img src="${esc(SHIP_MODELS[fleet.art ?? fleet.kind].poster)}" alt="" draggable="false" decoding="async"></span></span>`] : [];
  }).join("");
  return `<div class="deck-planet__orbit-fleets" aria-label="Fleets docked in orbit">${ships}${rest}${folded}</div>`;
}

/** §veil: the projector's row — what it burns, what it costs a landing, and
 * the owner's standing order (one switch, no input creep). */
function veilDetail(model: PlanetPanelModel, tier: number): string {
  const v = model.veil;
  const loss = Math.round((v?.descent_loss ?? [0.10, 0.15, 0.20][Math.max(0, Math.min(2, tier - 1))]) * 100);
  const burn = v?.fuel ? `burns ${v.burn_per_s.toFixed(2)} ${label(v.fuel)}/s raised` : "needs a Fusion Plant";
  const status = v?.up ? `<b class="deck-planet__good">VEIL UP</b>` : v && !v.raise ? `<b class="deck-planet__warning">Held down</b>` : `<span>Rises under a held blockade</span>`;
  const policy = model.mine && v
    ? `<button type="button" data-deck-act="planet-veil" data-raise="${v.raise ? "0" : "1"}" aria-pressed="${v.raise}">${v.raise ? "Raise under siege: on" : "Raise under siege: off"}</button>`
    : "";
  return `<small>Off the Fusion Plant · no slot · no crew · ${burn} · landings −${loss}%</small><div class="deck-planet__veil">${status}${policy}</div>`;
}

function siegeOverlay(siege: PlanetSiege, structures: readonly PlanetStructure[], bodyId: number): string {
  const targets = structures.filter((item) => !isOrbitalYard(item.key, item.tier) && Number.isFinite(item.x) && Number.isFinite(item.y))
    .sort((a, b) => a.key.localeCompare(b.key));
  const count = siege.suppression > 0.02 ? Math.min(8, 2 + Math.round(siege.suppression * 6)) : 0;
  let seed = (bodyId * 2654435761) >>> 0;
  const rnd = () => { seed = (Math.imul(seed ^ (seed >>> 15), 0x2c1b3c6d) + 0x6d2b79f5) >>> 0; return seed / 4294967296; };
  const impacts = Array.from({ length: count }, (_, i) => {
    const at = targets.length ? targets[i % targets.length] : null;
    const x = at ? at.x + (rnd() - 0.5) * 5 : 18 + rnd() * 64;
    const y = at ? at.y - 2 + (rnd() - 0.5) * 4 : 48 + rnd() * 36;
    return `<span class="deck-planet__impact" style="--planet-x:${x.toFixed(1)}%;--planet-y:${y.toFixed(1)}%;--impact-delay:${(i * 0.37 + rnd() * 0.5).toFixed(2)}s"></span>`;
  }).join("");
  const landers = siege.landingId ? [0, 1, 2].map((i) => {
    const at = targets[i % Math.max(1, targets.length)];
    const x = at ? at.x + (i - 1) * 4 : 40 + i * 10;
    const y = at ? at.y : 62;
    return `<span class="deck-planet__lander" style="--planet-x:${x.toFixed(1)}%;--lander-y:${y.toFixed(1)}%;--lander-delay:${(i * 0.9).toFixed(2)}s"></span>`;
  }).join("") : "";
  const follow = siege.landingId
    ? `<button type="button" data-deck-act="watch-ground" data-ground="${esc(siege.landingId)}">Follow landing · delayed</button>` : "";
  // §veil: the ionised sky, an aurora curtain over the world.
  const veil = siege.veil ? `<span class="deck-planet__veil-sky" aria-hidden="true"></span>` : "";
  return `<div class="deck-planet__siege${siege.veil ? " is-veiled" : ""}" aria-label="${esc(siege.caption)}">${veil}${impacts}${landers}<div class="deck-planet__siege-banner" role="status"><b>⛔ ${esc(siege.caption)}</b>${siege.detail ? `<span>${esc(siege.detail)}</span>` : ""}${follow}</div></div>`;
}
