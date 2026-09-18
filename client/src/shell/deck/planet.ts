import { commodityIcon, label, structureImage } from "../../icons";
import type { AssignmentView, BodyView, BuildOption, ClientMsg, Commodity, SystemInfo, SystemStateView } from "../../protocol";
import { assignedRecipe, POOL_LABEL, recipeOutputs } from "../../core/derive/market";
import { PlanetSites } from "./planet-sites";
import { refiningComparison } from "../refining";
import type { RefiningContext } from "../../core/derive/refining";

const ART = "/art/derived/planet-workbench/v1/";
/** The rendered footprint width for srcset selection: 11.5% of the image plane,
 * which is the map area beside the standard system rail. Height-limited windows
 * render smaller and merely fetch a sharper sprite than needed. */
const FOOTPRINT_SIZES = "calc(11.5vw - 64px)";
type PlanetTab = "structures" | "population" | "survey";
const TABS: readonly PlanetTab[] = ["structures", "population", "survey"];
const EXTRACTION: Record<string, readonly Commodity[]> = {
  mining_complex: ["metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "silicates", "rare_elements"],
  volatile_harvester: ["volatiles"], bioharvester: ["biomass"],
};

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
  developmentHtml: string;
  /** This world's own construction jobs (owner only); empty when nothing applies. */
  queueHtml: string;
  economy?: RefiningContext;
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
  x: number;
  y: number;
}

/** Footprint addresses on the 16:9 artwork, in percent. Every surface painting shares
 * one composition: black sky above a planetary limb whose haze sits at ~19% height in
 * the middle and curves down to ~32% at both edges, terrain below. Columns are 12%
 * apart (a footprint is at most 11.5% wide), so only rows within a column need
 * vertical clearance, and rows sit at least 21% apart. Row 0 is lowered toward the
 * edges so a sprite never pokes into the haze. These are the initial kind addresses;
 * PlanetSites preserves each world's assigned addresses as capacity grows. */
const COLUMNS: readonly number[] = [7, 19, 31, 43, 55, 67, 79, 91];
const LAND_ROWS: readonly (readonly number[])[] = [
  [46, 42, 39.5, 38, 38, 39.5, 42, 46],
  [68, 64, 61.5, 60, 60, 61.5, 64, 68],
  [89, 86, 83.5, 82, 82, 83.5, 86, 89],
];
/** The ocean painting is mostly water: two island groups (left middle, right middle)
 * and two shorelines (bottom left, bottom right). Its slots reorder the columns so the
 * common surface kinds (mines, harvesters, agroplex, habitat) land on ground and the
 * offshore-plausible kinds (fuel rig, shipyard, dock) take open water. */
const OCEAN_COLUMNS: readonly number[] = [67, 19, 31, 79, 91, 55, 43, 7];
const OCEAN_ROWS: readonly (readonly number[])[] = [
  [46, 41, 41, 46, 46, 44, 43, 46],
  [68, 64, 65, 68, 68, 65, 64, 67],
  [89, 89, 89, 89, 89, 87, 86, 88],
];

export function footprintAddress(surface: string | null, slot: number): { x: number; y: number } {
  const column = slot % COLUMNS.length;
  // Unknown future kinds beyond the 24 authored slots share the last row.
  const row = Math.min(LAND_ROWS.length - 1, Math.floor(slot / COLUMNS.length));
  return surface === "ocean"
    ? { x: OCEAN_COLUMNS[column], y: OCEAN_ROWS[row][column] }
    : { x: COLUMNS[column], y: LAND_ROWS[row][column] };
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
  const surface = planetSurface(model.art);
  return keys.flatMap((key, index) => {
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
    const status = !staffable ? "Automatic"
      : !workers && !specialists ? "No workforce"
      : stopped ? ({ no_food: "Needs Provisions", no_inputs: "Needs inputs", storage_full: "Storage full", needs_crew: "No workforce" }[stopped] ?? label(stopped))
      : "Operating";
    return [{ key, title: assignment?.title ?? model.catalog.find(option => option.key === key)?.label ?? label(key),
      tier, workers, staffable, assignment, outputs, status, ...footprintAddress(surface, index) }];
  });
}

/** Match the public System View's deterministic visual variant. Gas giants
 * retain their globe: no land or invented surface colony on a gas giant. */
export function planetSurface(art: string): string | null {
  if (art.includes("gas_giant")) return null;
  return ["terrestrial", "desert", "ocean", "ice", "lava"].find(kind => art.includes(`${kind}-`) || art.includes(`${kind}.`)) ?? "barren";
}

type Posting = { workers: number; specialists: Record<string, number>; refining_ore?: Commodity };
type Review = Posting & { key: string; previous: string };

/** The planet is the deepest rung of the map's zoom ladder. `renderScene` draws the
 * world over the map area (art, footprints, selected badge, sibling-world strip) and
 * `render` is the management column for the `world` workspace route. Both read the
 * same served model through one selection/draft controller. Local selection and order
 * drafts only: sending a directive cannot advance the visible economy; a matching
 * arrived assignment is the only evidence that clears its pending marker. */
export class PlanetPanel {
  private context = "";
  private selected = "";
  private tab: PlanetTab = "structures";
  private drafts = new Map<string, number>();
  private oreDrafts = new Map<string, Commodity>();
  private pending = new Map<string, Posting>();
  private review: Review | null = null;
  private focusRequest = "";
  private buildSites = false;
  private sites = new PlanetSites();

  availableSite(model: PlanetPanelModel, slot: number) {
    return this.sites.snapshot(model).free.find(site => site.slot === slot);
  }

  preferSite(model: PlanetPanelModel, key: string, slot: number): void {
    this.sites.prefer(model, key, slot);
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
      this.buildSites = false;
    }
    if (!model.mine) {
      this.tab = "survey";
      this.drafts.clear();
      this.review = null;
      this.buildSites = false;
    }
    const structures = planetStructures(model);
    const layout = this.sites.snapshot(model);
    for (const item of structures) {
      const site = layout.occupied.find(site => site.key === item.key);
      if (site) Object.assign(item, footprintAddress(planetSurface(model.art), site.slot));
    }
    if (!structures.some(item => item.key === this.selected)) {
      this.selected = structures.find(item => item.outputs.length)?.key ?? structures[0]?.key ?? "";
      this.review = null;
    }
    if (this.focusRequest) {
      if (structures.some(item => item.key === this.focusRequest)) {
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
        && (!pending.refining_ore || pending.refining_ore === (item.assignment?.refining_ore ?? "metallic_ore"))) this.pending.delete(key);
      if (this.oreDrafts.get(key) === (item.assignment?.refining_ore ?? "metallic_ore")) this.oreDrafts.delete(key);
      const draft = this.drafts.get(key);
      if (draft !== undefined && (draft === item.workers || draft > item.tier)) this.drafts.delete(key);
      if (this.review?.key === key && this.review.previous !== postingFingerprint(item)) this.review = null;
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
    if (action === "planet-build-sites") {
      this.buildSites = !this.buildSites;
      this.tab = "structures";
      this.review = null;
      return true;
    }
    if (action === "planet-select") {
      if (structures.some(item => item.key === data.structure)) {
        this.selected = data.structure!;
        if (this.tab === "population" || this.tab === "survey") this.tab = "structures";
        this.review = null;
        this.buildSites = false;
      }
      return true;
    }
    const selected = structures.find(item => item.key === this.selected);
    if (!selected?.staffable || this.tab !== "structures") return true;
    const key = this.key(model, selected.key);
    const draft = this.drafts.get(key) ?? selected.workers;
    const ore = this.oreDrafts.get(key) ?? selected.assignment?.refining_ore ?? "metallic_ore";
    const oreChanged = selected.key === "smelter" && ore !== (selected.assignment?.refining_ore ?? "metallic_ore");
    if (action === "planet-workers") {
      const delta = Number(data.delta);
      if (delta === -1 || delta === 1) this.drafts.set(key, Math.max(0, Math.min(selected.tier, draft + delta)));
      this.review = null;
    } else if (action === "planet-ore" && selected.key === "smelter") {
      const recipes = model.catalog.find(o => o.key === selected.key)?.refining_recipes ?? [];
      if (recipes.some(r => r.inputs[0]?.[0] === data.ore)) this.oreDrafts.set(key, data.ore as Commodity);
      this.review = null;
    } else if (action === "planet-review" && (draft !== selected.workers || oreChanged)) {
      this.review = { key, workers: draft, specialists: { ...(selected.assignment?.specialists ?? {}) },
        ...(selected.key === "smelter" ? { refining_ore: ore } : {}), previous: postingFingerprint(selected) };
    } else if (action === "planet-cancel") {
      this.review = null;
      this.drafts.delete(key);
      this.oreDrafts.delete(key);
    } else if (action === "planet-confirm" && this.review?.key === key && this.review.previous === postingFingerprint(selected)) {
      const posting: Posting = { workers: this.review.workers, specialists: this.review.specialists,
        ...(this.review.refining_ore ? { refining_ore: this.review.refining_ore } : {}) };
      send({ type: "SetAssignment", system_id: model.system.id, body_id: model.body.id, structure: selected.key, ...posting });
      this.pending.set(key, posting);
      this.drafts.delete(key);
      this.oreDrafts.delete(key);
      this.review = null;
    }
    return true;
  }

  /** The map-area scene: artwork, footprints and optional build-site shortcuts.
   * Orders are still reviewed and sent through the existing builder. */
  renderScene(model: PlanetPanelModel): string {
    const structures = this.sync(model);
    const selected = structures.find(item => item.key === this.selected);
    const surface = planetSurface(model.art);
    const background = surface
      ? `<img class="deck-planet__surface" src="${ART}${surface}-768.webp" srcset="${ART}${surface}-768.webp 768w, ${ART}${surface}-1536.webp 1536w" sizes="(min-width: 1125px) calc(100vw - 552px), 100vw" alt="${esc(model.body.name)} landscape" draggable="false">`
      : `<img class="deck-planet__globe" src="${esc(model.art)}" alt="${esc(model.body.name)}" draggable="false">`;
    const position = (item: { x: number; y: number }) => `--planet-x:${item.x}%;--planet-y:${item.y}%`;
    const footprints = structures.map(item => `<button id="planet-spot-${esc(item.key)}" type="button" class="deck-planet__building" style="${position(item)}" data-deck-act="planet-select" data-structure="${esc(item.key)}" aria-label="Inspect ${esc(item.title)}" aria-pressed="${item.key === this.selected}" aria-controls="planet-structure-detail">${structureImage(item.key, item.tier, "lg", undefined, "", FOOTPRINT_SIZES)}</button>`).join("");
    const badge = selected && !this.buildSites ? `<div class="deck-planet__selection" style="${position(selected)};--planet-output-height:${selected.outputs.length > 2 ? 84 : 44}px">
      <button type="button" class="deck-planet__workforce-badge" data-deck-act="planet-select" data-structure="${esc(selected.key)}" aria-label="${esc(selected.title)}${selected.staffable ? `: ${selected.workers} workforce assigned` : ""}"><span>${esc(selected.title)}</span>${selected.staffable ? `<b>${selected.workers}</b>${workforceIcon()}` : ""}</button>
      ${selected.outputs.length ? `<div class="deck-planet__output-badge" aria-label="Reported output">${outputHtml(selected.outputs)}</div>` : ""}
    </div>` : "";
    const sites = this.sites.snapshot(model);
    const emptySites = this.buildSites ? sites.free.map(site => `<button id="planet-site-${site.slot}" type="button" class="deck-planet__site is-${site.pool}" style="${position(footprintAddress(surface, site.slot))}" data-deck-act="world-build-site" data-slot="${site.slot}" data-pool="${site.pool}" aria-label="Build in available ${POOL_LABEL[site.pool]} slot" title="${POOL_LABEL[site.pool]} slot"><b>+</b><small>${site.pool === "infrastructure" ? "Infra" : site.pool === "industrial" ? "Industry" : "Resource"}</small></button>`).join("") : "";
    const construction = sites.occupied.filter(site => !model.body.structures[site.key!]).map(site => {
      const job = model.report.builds.find(job => job.body_id === model.body.id && job.key === site.key)!;
      const name = model.catalog.find(option => option.key === site.key)?.label ?? label(site.key!);
      const status = job.queued ? "Queued" : job.complete_time === null ? "Paused" : "Building";
      return `<div id="planet-construction-${esc(site.key!)}" class="deck-planet__site is-construction" style="${position(footprintAddress(surface, site.slot))}" role="img" aria-label="${status}: ${esc(name)}" title="${esc(name)} · ${status}">${structureImage(site.key!, 1, "lg")}<small>${status}</small></div>`;
    }).join("");
    // Sibling worlds come from the same arrived system report as the scene.
    const worlds = model.report.bodies.map(body => `<button id="planet-world-${body.id}" type="button" data-deck-act="world-switch" data-body="${body.id}"${body.id === model.body.id ? ' aria-current="page"' : ""}>${esc(body.name)}</button>`).join("");
    return `<div class="deck-planet-scene" aria-label="${esc(model.body.name)} planet view">
      <nav id="planet-worlds" class="deck-planet__worlds" aria-label="Worlds in ${esc(model.system.name)}"><span>${esc(model.system.name)} · worlds</span>${worlds}${model.mine ? `<button type="button" class="deck-planet__sites-toggle" data-deck-act="planet-build-sites" aria-pressed="${this.buildSites}">${this.buildSites ? "Done" : "Build sites"}</button>` : ""}</nav>
      <div class="deck-planet__scene${surface ? "" : " is-orbital"}" aria-label="${surface ? "Planet surface" : "Orbital infrastructure"}"><div class="deck-planet__canvas">${background}${footprints}${construction}${emptySites}${badge}${this.buildSites && !sites.free.length ? '<span class="deck-planet__sites-empty">No free slots</span>' : ""}</div></div>
    </div>`;
  }

  /** The workspace column for the `world` route: identity, workforce, tabs,
   * structure list and detail, then this world's slots, build actions and queue. */
  render(model: PlanetPanelModel): string {
    const structures = this.sync(model);
    const selected = structures.find(item => item.key === this.selected);
    const assignedHere = structures.reduce((sum, item) => sum + item.workers, 0);
    const free = model.report.workforce ? Math.max(0, model.report.workforce.units - model.report.workforce.posted) : null;
    const tabs = TABS.filter(tab => model.mine || tab === "survey").map(tab => `<button type="button" data-deck-act="planet-tab" data-tab="${tab}" aria-selected="${this.tab === tab}" aria-controls="planet-page">${label(tab)}${tab === "structures" ? ` <small>${structures.length}</small>` : ""}</button>`).join("");
    const list = structures.length ? structures.map(item => {
      const pending = this.pending.has(this.key(model, item.key));
      return `<button id="planet-row-${esc(item.key)}" type="button" class="deck-planet__row" data-deck-act="planet-select" data-structure="${esc(item.key)}" aria-pressed="${item.key === this.selected}" aria-controls="planet-structure-detail">
        ${structureImage(item.key, item.tier, "md")}<span class="deck-planet__row-name"><b>${esc(item.title)}</b><small>${pending ? "Order sent · awaiting report" : `Tier ${item.tier} · ${esc(item.status)}`}</small></span>
        <span class="deck-planet__row-values">${outputHtml(item.outputs)}${item.staffable ? `<span class="deck-planet__staffed-value">${item.workers} ${workforceIcon()}</span>` : item.outputs.length ? "" : "—"}</span></button>`;
    }).join("") : `<div class="deck-empty-inline">No structures built.</div>`;
    const page = this.tab === "survey" ? model.surveyHtml
      : this.tab === "population" ? model.populationHtml
      : `<div class="deck-planet__management"><section class="deck-planet__list" aria-label="Planet structures">${list}</section>${selected ? this.detailHtml(model, selected) : ""}</div>`;
    const development = model.mine && this.tab === "structures"
      ? `<div class="deck-planet__development">${model.developmentHtml}${model.queueHtml}</div>` : "";
    const delay = model.delay === null ? "Unknown" : model.delay < 1 ? "<1s" : `~${Math.ceil(model.delay)}s`;
    return `<section class="deck-page deck-planet" aria-label="${esc(model.body.name)} planet management">
      <header class="deck-page__lead deck-planet__lead"><img class="deck-planet__art" src="${esc(model.art)}" alt="" draggable="false"><span>${esc(model.system.name)} · World</span><h2>${esc(model.body.name)}</h2><p><span>${esc(label(model.body.environment))} · ${esc(label(model.body.size))}${model.body.geology ? ` · ${esc(label(model.body.geology))}` : ""}</span><span class="deck-planet__delay">Information delay <b>${delay}</b></span></p></header>
      ${model.mine ? `<div class="deck-planet__workforce-summary"><span>${workforceIcon()} Workforce <b>${assignedHere}</b> assigned here</span><span class="deck-planet__available">${free === null ? "—" : free} available in system</span></div>` : ""}
      <nav class="deck-tabs deck-planet__tabs" aria-label="Planet management">${tabs}</nav>
      <div id="planet-page">${page}</div>
      ${development}
    </section>`;
  }

  private detailHtml(model: PlanetPanelModel, item: PlanetStructure): string {
    const key = this.key(model, item.key);
    const draft = this.drafts.get(key) ?? item.workers;
    const pending = this.pending.get(key);
    const option = model.catalog.find(option => option.key === item.key);
    const ore = this.oreDrafts.get(key) ?? item.assignment?.refining_ore ?? "metallic_ore";
    const recipe = assignedRecipe(option, ore);
    const compareRecipe = item.key === "smelter" ? recipe : item.key === "mining_complex"
      ? model.catalog.find(o => o.key === "smelter")?.refining_recipes?.find(r => model.body.deposits?.some(d => d.resource === r.inputs[0][0])) : undefined;
    const comparison = model.economy && model.mine ? refiningComparison(model.economy, model.report, model.body.id, compareRecipe) : "";
    const changed = draft !== item.workers || (item.key === "smelter" && ore !== (item.assignment?.refining_ore ?? "metallic_ore"));
    const refining = option?.refining_recipes?.length ? `<div class="deck-planet__refining"><small>Standing refining recipe</small><div class="deck-planet__ore-options">${option.refining_recipes.map(r => `<button type="button" data-deck-act="planet-ore" data-ore="${r.inputs[0][0]}" aria-pressed="${r.inputs[0][0] === ore}">${resourceIcon(r.inputs[0][0])}${esc(label(r.inputs[0][0]))}</button>`).join("")}</div><small>${recipe ? `${recipe.inputs.map(([c,n]) => `${n} ${label(c)}`).join(" + ")} → ${recipeOutputs(recipe).map(([c,n]) => `${n} ${label(c)}`).join(" + ")}` : ""}</small></div>` : "";
    const specialists = Object.entries(item.assignment?.specialists ?? {}).filter(([, count]) => count > 0);
    const review = this.review?.key === key;
    const overposted = !!model.report.workforce && model.report.workforce.posted - item.workers + draft > model.report.workforce.units;
    const action = review
      ? `<div class="deck-planet__confirm" role="group" aria-label="Review workforce assignment"><b>${item.workers} → ${draft} workforce</b><span>${model.delay === null ? "Order delay unknown" : `Order travel ~${Math.ceil(model.delay)}s`} · updates after response</span>${overposted ? `<span class="deck-planet__warning">Not enough free workforce; assignments will share capacity.</span>` : ""}<div><button type="button" class="is-primary" data-deck-act="planet-confirm">Confirm assignment</button><button type="button" data-deck-act="planet-cancel">Cancel</button></div></div>`
      : `<button type="button" class="is-primary" data-deck-act="planet-review" ${!changed ? "disabled" : ""}>${!changed ? "No changes" : "Review assignment"}</button>`;
    return `<aside id="planet-structure-detail" class="deck-planet__detail" aria-label="Selected structure">
      <header>${structureImage(item.key, item.tier, "lg")}<div><small>Tier ${item.tier}</small><h3>${esc(item.title)}</h3><span class="${item.status === "Operating" || item.status === "Automatic" ? "deck-planet__good" : "deck-planet__warning"}">${esc(item.status)}</span></div></header>
      <p>${esc(model.descriptions[item.key] ?? "Planetary infrastructure.")}</p>
      ${refining}${comparison}
      ${item.staffable ? `<div class="deck-planet__assignment"><div><b>Assigned workforce</b><small>Capacity ${item.tier}</small></div><div class="deck-stepper"><button type="button" data-deck-act="planet-workers" data-delta="-1" aria-label="Decrease assigned workforce" ${draft <= 0 ? "disabled" : ""}>−</button><b aria-label="Planned workforce">${draft}</b><button type="button" data-deck-act="planet-workers" data-delta="1" aria-label="Increase assigned workforce" ${draft >= item.tier ? "disabled" : ""}>+</button></div><small>1 workforce = 1,000 populace</small>${specialists.length ? `<small>Specialists: ${specialists.map(([kind, count]) => `${count} ${esc(label(kind))}`).join(" · ")}</small>` : ""}</div>` : ""}
      ${item.outputs.length ? `<div class="deck-planet__reported"><small>Reported output</small><div>${outputHtml(item.outputs)}</div></div>` : ""}
      ${recipe ? `<div class="deck-planet__inputs"><small>Inputs</small><div>${recipe.inputs.map(([good]) => resourceIcon(good)).join("")}</div></div>` : ""}
      ${review && item.key === "smelter" ? `<small>Refine ${esc(label(this.review!.refining_ore ?? "metallic_ore"))} · takes effect when the order arrives</small>` : ""}
      ${pending ? `<div class="deck-planet__pending" role="status">${pending.workers} workforce${pending.refining_ore ? ` · ${esc(label(pending.refining_ore))}` : ""} requested · awaiting report</div>` : ""}
      ${item.staffable ? action : ""}
    </aside>`;
  }
}

function postingFingerprint(item: PlanetStructure): string {
  return JSON.stringify([item.workers, item.tier, item.assignment?.refining_ore ?? "metallic_ore", Object.entries(item.assignment?.specialists ?? {}).sort()]);
}
function sameSpecialists(a: Record<string, number>, b: Record<string, number>): boolean {
  return JSON.stringify(Object.entries(a).filter(([, n]) => n > 0).sort()) === JSON.stringify(Object.entries(b).filter(([, n]) => n > 0).sort());
}
function workforceIcon(): string {
  return `<img class="deck-planet__workforce-icon" src="${ART}workforce-128.png" srcset="${ART}workforce-128.png 1x, ${ART}workforce-256.png 2x" alt="Workforce" title="Workforce">`;
}
function resourceIcon(good: Commodity): string {
  // A title on the wrapper covers every registry variant, including resources
  // whose shared icon currently supplies alt text but no native hover label.
  return `<span class="deck-planet__resource" title="${esc(label(good))}" aria-label="${esc(label(good))}">${commodityIcon(good)}</span>`;
}
function outputHtml(outputs: readonly [Commodity, number][]): string {
  return outputs.map(([good, rate]) => `<span class="deck-planet__rate" aria-label="${rateText(rate)} ${esc(label(good))} per minute"><b>${rateText(rate)}</b>${resourceIcon(good)}<small>/ min</small></span>`).join("");
}
function rateText(rate: number): string {
  return `+${Number(rate.toFixed(2)).toLocaleString("en-US", { maximumFractionDigits: 2 })}`;
}
function esc(text: string): string {
  return text.replace(/[&<>"']/g, ch => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[ch]!));
}
