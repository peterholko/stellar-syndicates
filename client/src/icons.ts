// SEMANTIC ICON REGISTRY (§UX text diet). One meaning → one key → one glyph,
// used EVERYWHERE, so an icon reads the same in every panel and swapping in
// generated art later is a one-file change.
//
// Each entry is ART-BACKED (resource PNG, generated panel/hull PNG, or legacy SVG) or
// a PLACEHOLDER (`glyph` = a unicode/emoji stand-in until dedicated art exists).
// Every entry carries a default `tip` (hover text), because in the icon-first UI
// the words live in tooltips, not on screen.
//
// Render through `icon()` / `chip()` / `badgeChip()` — never hand-roll an <img>.

import type { Commodity } from "./protocol";

export type IconKey =
  // resources
  | "fuel" | "ore" | "alloys" | "provisions" | "volatiles" | "credits" | "biomass"
  // economy / structures
  | "storage" | "slots" | "shipyard" | "sensor" | "defense" | "habitat" | "refinery" | "interdictor"
  | "extractor" | "orbital_warehouse" | "build" | "queue" | "warehouse" | "manifest" | "freightRoute" | "authorityFreighter"
  // dedicated structure-builder art
  | "structureMiningComplex" | "structureVolatileHarvester" | "structureBioharvester"
  | "structureSmelter" | "structureElectronicsFabricator" | "structureChemicalWorks" | "structureFuelRefinery"
  | "structureAgroplex" | "structureMachineWorks" | "structureArmamentsComplex"
  | "structureCompositeWorks" | "structureHullFabricator" | "structurePrecisionWorks" | "structureDriveWorks"
  | "structureShipyard" | "structureNavalDrydock" | "structureCapitalSlipway" | "structureOrdnanceFoundry"
  | "structureHabitat" | "structureWarehouse" | "structureOrbitalWarehouse" | "structureSensorArray" | "structureDefensePlatform"
  | "structureAcademy" | "structureGarrison"
  // planetary profile / colony identity
  | "planetHabitable" | "planetHostile" | "planetUninhabitable"
  | "geologyPoor" | "geologyRich" | "geologyUltraRich"
  | "featureFertile" | "featureLowGravity" | "featurePrecursor"
  | "roleAgriculture" | "roleMining" | "roleFuel" | "roleElectronics" | "roleShipbuilding" | "rolePopulation" | "roleOutpost"
  // fleets / ship kinds
  | "fleet" | "scout" | "raider" | "corvette" | "convoy" | "colony" | "builder" | "transport"
  | "tiny_freighter" | "small_freighter" | "large_freighter" | "heavy_freighter" | "bulk_freighter"
  | "destroyer" | "cruiser" | "battleship" | "dreadnought" | "titan"
  // verbs / orders
  | "move" | "attack" | "raid" | "withdraw" | "reinforce" | "recall" | "blockade" | "siege"
  | "doctrine" | "posture" | "claim" | "cargo" | "market" | "jump" | "dock" | "undock" | "unload" | "escort"
  // transit / signature
  | "stealth" | "flank" | "sensorRange"
  // order lifecycle (light-delayed round trip)
  | "delay" | "echo" | "delivered" | "confirmed" | "inTransit"
  // status / intel
  | "unfed" | "fed" | "warning" | "unknown" | "intel" | "battle" | "aftermath" | "captured" | "lost"
  | "commandCenter" | "uncertainty" | "hub" | "success" | "info" | "home" | "mouse" | "shift" | "time"
  | "population" | "workforce" | "food" | "upkeep"
  // combat modules
  | "moduleMassDriver" | "moduleTorpedoRack" | "modulePointDefense" | "moduleReflectivePlating" | "moduleWhippleArmor" | "moduleExtendedTanks" | "moduleReconSuite" | "moduleCargoPods" | "moduleEscortDatalink" | "moduleFuelTransferRig"
  | "moduleSurveyDrive" | "moduleNebulaSpectrometer" | "modulePrismaticLance"
  // syndicates (§syndicates)
  | "syndicate" | "ally" | "garrison";

/** Wire slug → display label: "metallic_ore" → "Ferrite Ore". Display-only —
 * never feed the result back into commands; attributes that round-trip to
 * `net.send` (data-resource / data-hire / option values …) stay raw. */
export function label(slug: string): string {
  if (slug === "metallic_ore" || slug === "ore") return "Ferrite Ore";
  if (slug === "rare_metal_ore") return "Rare-metal Ore";
  return slug.replace(/_/g, " ").replace(/\b\w/g, (ch) => ch.toUpperCase());
}

interface IconDef {
  /** Downscaled RASTER (PNG) variant name under /art/ui_icons/resource/ —
   *  highest precedence (the resource icons). Small, retina-crisp. */
  png?: string;
  /** General 128px UI PNG under /art/ui_icons/png/128/. */
  png128?: string;
  /** Generated 64px transparent panel PNG under /art/ui_icons/panel/. */
  panel?: string;
  /** Approved hull master, downsampled to 128/256px for all fleet/build panels. */
  hull?: string;
  /** Tiered transparent structure art, shared by builders and the planet scene. */
  structure?: string;
  /** Bundled SVG slug (art-backed) — takes precedence over `glyph`. */
  art?: string;
  /** Unicode/emoji placeholder when there is no art yet. */
  glyph?: string;
  /** Default hover text — the words the icon replaced. */
  tip: string;
  /** True while the glyph is a stand-in awaiting generated art. */
  placeholder: boolean;
}

// R(name) = resource PNG; N(name) = generated panel PNG; H(name) = hull PNG;
// S(name) = structure PNG; A(slug) = legacy SVG; P(glyph) = emoji placeholder.
const R = (png: string, tip: string): IconDef => ({ png, tip, placeholder: false });
const R128 = (png128: string, tip: string): IconDef => ({ png128, tip, placeholder: false });
const N = (panel: string, tip: string): IconDef => ({ panel, tip, placeholder: false });
const H = (hull: string, tip: string): IconDef => ({ hull, tip, placeholder: false });
const S = (structure: string, tip: string): IconDef => ({ structure, tip, placeholder: false });
const A = (art: string, tip: string): IconDef => ({ art, tip, placeholder: false });
const P = (glyph: string, tip: string): IconDef => ({ glyph, tip, placeholder: true });

export const ICONS: Record<IconKey, IconDef> = {
  // resources — dedicated downscaled PNG art (source-of-truth 1254px; UI loads 64px)
  fuel: R("fuel", "Fuel"),
  ore: R("ores-2026-09-16/ferrite_ore-128", "Ferrite Ore"),
  alloys: R("alloys", "Alloys"),
  provisions: R("provisions", "Provisions"),
  volatiles: R("volatiles", "Volatiles"),
  credits: R("credits", "Credits"),
  biomass: N("resource-biomass", "Biomass"),
  // economy / structures
  storage: N("concept-stockpile", "Storage / stockpile capacity"),
  slots: N("status-development-slots", "Development slots (used / total)"),
  shipyard: S("shipyard", "Shipyard tier"),
  sensor: A("concept-sensor-range", "Sensor array"),
  defense: S("defense_platform", "Defense platform tier"),
  habitat: S("habitat", "Habitat tier (output boost)"),
  refinery: S("fuel_refinery", "Fuel refinery (Volatiles → Fuel)"),
  interdictor: P("⛓", "Interdictor"),
  extractor: S("mining_complex", "Extractor tier (output ×1.5)"),
  orbital_warehouse: S("orbital_warehouse", "Orbital Warehouse tier (storage cap)"),
  build: A("action-build", "Build"),
  queue: N("status-construction-queue", "Under construction"),
  warehouse: N("concept-warehouse", "Market Warehouse"),
  manifest: N("concept-manifest", "Cargo manifest"),
  freightRoute: N("concept-freight-route", "Freight route"),
  authorityFreighter: H("freighter", "Authority freighter"),
  // structure builder — one purpose-built installation per buildable structure
  structureMiningComplex: S("mining_complex", "Mining Complex"),
  structureVolatileHarvester: S("volatile_harvester", "Volatile Harvester"),
  structureBioharvester: S("bioharvester", "Bioharvester"),
  structureSmelter: S("smelter", "Smelter"),
  structureElectronicsFabricator: S("electronics_fabricator", "Electronics Fabricator"),
  structureChemicalWorks: S("chemical_works", "Chemical Works"),
  structureFuelRefinery: S("fuel_refinery", "Fuel Refinery"),
  structureAgroplex: S("agroplex", "Agroplex"),
  structureMachineWorks: S("machine_works", "Machine Works"),
  structureArmamentsComplex: S("armaments_complex", "Armaments Complex"),
  structureCompositeWorks: S("composite_works", "Composite Works"),
  structureHullFabricator: S("hull_fabricator", "Hull Fabricator"),
  structurePrecisionWorks: S("precision_works", "Precision Works"),
  structureDriveWorks: S("drive_works", "Drive Works"),
  structureShipyard: S("shipyard", "Shipyard"),
  structureNavalDrydock: S("naval_drydock", "Naval Drydock"),
  structureCapitalSlipway: S("capital_slipway", "Capital Slipway"),
  structureOrdnanceFoundry: S("ordnance_foundry", "Ordnance Foundry"),
  structureHabitat: S("habitat", "Habitat"),
  structureOrbitalWarehouse: S("orbital_warehouse", "Orbital Warehouse"),
  structureWarehouse: S("warehouse", "Warehouse"),
  structureSensorArray: S("sensor_array", "Sensor Array"),
  structureDefensePlatform: S("defense_platform", "Defense Platform"),
  structureAcademy: S("academy", "Academy"),
  structureGarrison: S("garrison", "Garrison"),
  // planetary profile / colony identity
  planetHabitable: N("planet-habitable", "Habitable world"),
  planetHostile: N("planet-hostile", "Hostile world"),
  planetUninhabitable: N("planet-uninhabitable", "Uninhabitable world"),
  geologyPoor: N("geology-poor", "Poor mineral geology"),
  geologyRich: N("geology-rich", "Rich mineral geology"),
  geologyUltraRich: N("geology-ultra-rich", "Ultra-rich mineral geology"),
  featureFertile: N("feature-fertile", "Fertile biosphere"),
  featureLowGravity: N("feature-low-gravity", "Low gravity"),
  featurePrecursor: N("feature-precursor", "Precursor ruins"),
  roleAgriculture: N("role-agriculture", "Agricultural exporter"),
  roleMining: N("role-mining", "Mining center"),
  roleFuel: N("role-fuel-production", "Fuel complex"),
  roleElectronics: N("role-electronics", "Electronics center"),
  roleShipbuilding: N("role-shipbuilding", "Shipbuilding center"),
  rolePopulation: N("role-population-center", "Population center"),
  roleOutpost: N("role-strategic-outpost", "Strategic outpost"),
  // fleets / ship kinds
  fleet: R128("concept-fleet-v2", "Fleet"),
  scout: H("scout", "Scout"),
  raider: H("raider", "Interceptor"),
  corvette: H("corvette", "Corvette"),
  convoy: H("convoy", "Medium Freighter"),
  tiny_freighter: H("tiny_freighter", "Tiny Freighter"),
  small_freighter: H("small_freighter", "Small Freighter"),
  large_freighter: H("large_freighter", "Large Freighter"),
  heavy_freighter: H("heavy_freighter", "Heavy Freighter"),
  bulk_freighter: H("bulk_freighter", "Bulk Freighter"),
  colony: H("colony", "Colony ship"),
  builder: H("builder", "Construction Ship"),
  transport: H("transport", "Troop Transport"),
  destroyer: H("destroyer", "Destroyer"),
  cruiser: H("cruiser", "Cruiser"),
  battleship: H("battleship", "Battleship"),
  dreadnought: H("dreadnought", "Dreadnought"),
  titan: H("titan", "Titan"),
  // verbs / orders
  move: A("action-move-travel", "Move"),
  attack: P("⚔", "Attack (destroy)"),
  raid: A("action-attack-raid", "Raid (seize cargo)"),
  withdraw: P("↩", "Withdraw from battle"),
  reinforce: P("➕", "Reinforce"),
  recall: A("action-recall", "Recall"),
  blockade: P("⛔", "Blockade"),
  siege: P("⏳", "Siege"),
  doctrine: A("action-standing-order", "Fleet doctrine"),
  posture: P("🎯", "Engagement posture"),
  claim: A("action-claim-system", "Claim"),
  cargo: N("concept-manifest", "Cargo"),
  market: A("concept-market-exchange", "Hub market"),
  jump: N("action-jump", "Jump"),
  dock: N("action-dock", "Dock"),
  undock: N("action-undock", "Undock"),
  unload: N("action-unload-cargo", "Unload cargo"),
  escort: N("action-escort", "Escort"),
  // transit / signature
  stealth: P("🌑", "Stealth transit (quiet, ~2× trip)"),
  flank: P("💨", "Full speed (loud — high signature)"),
  sensorRange: A("concept-sensor-range", "Sensor range"),
  // order lifecycle
  delay: R128("concept-communication-delay", "Command / light delay"),
  echo: P("◔", "Response in transit (received, presumed complying)"),
  delivered: P("◈", "Signal outbound (order en route)"),
  confirmed: P("✓", "Confirmed"),
  inTransit: A("status-in-transit", "In transit"),
  // status / intel
  unfed: P("🍽", "Unfed — upkeep not met (boost suspended)"),
  fed: P("🍽", "Fed — upkeep met"),
  warning: A("status-warning-threat", "Warning"),
  unknown: P("❓", "Unknown — out of sensor range"),
  intel: P("🔭", "Scout intel (snapshot)"),
  battle: P("💥", "Battle in progress"),
  aftermath: P("☄", "Concluded battle"),
  captured: P("🚩", "System captured"),
  lost: P("🏴", "System lost"),
  commandCenter: A("concept-command-center-hq", "Command center"),
  uncertainty: A("concept-uncertainty-fog", "Position uncertainty"),
  hub: P("✷", "Wormhole hub"),
  success: A("status-success", "Success"),
  info: A("status-info", "Info"),
  home: P("★", "Home system"),
  mouse: P("🖱", "Click"),
  shift: P("⇧🖱", "Shift+click"),
  time: P("🕘", "Time"),
  population: N("status-population", "Population"),
  workforce: N("status-workforce", "Workforce"),
  food: N("status-food-supply", "Food supply"),
  upkeep: N("status-upkeep", "Upkeep"),
  // combat modules
  moduleMassDriver: N("module-mass-driver", "Mass Driver"),
  moduleTorpedoRack: N("module-torpedo-rack", "Torpedo Rack"),
  modulePointDefense: N("module-point-defense", "Point-Defense Screen"),
  moduleReflectivePlating: N("module-reflective-plating", "Reflective Plating"),
  moduleWhippleArmor: N("module-whipple-armor", "Whipple Armor"),
  moduleExtendedTanks: R("fuel", "Extended Tanks"),
  moduleReconSuite: N("concept-sensor-range", "Recon Suite"),
  moduleCargoPods: N("concept-manifest", "Cargo Pods"),
  moduleEscortDatalink: N("action-escort", "Escort Datalink"),
  moduleFuelTransferRig: N("role-fuel-production", "Fuel Transfer Rig"),
  moduleSurveyDrive: N("action-jump", "Survey Drive"),
  moduleNebulaSpectrometer: N("concept-sensor-range", "Nebula Spectrometer"),
  modulePrismaticLance: A("action-attack-raid", "Prismatic Lance"),
  // syndicates
  syndicate: P("🤝", "Syndicate (alliance)"),
  ally: P("🟢", "Syndicate ally"),
  garrison: P("🛰", "Ally garrison"),
};

const ART_BASE = "/art/ui_icons/svg/";
const PNG_BASE = "/art/ui_icons/resource/"; // downscaled 64px resource PNGs
const PNG_128_BASE = "/art/ui_icons/png/128/"; // high-DPI general UI PNGs
const PANEL_BASE = "/art/ui_icons/panel/"; // generated transparent 64px panel PNGs
const HULL_BASE = "/art/derived/ships/2026-09-08/panels/"; // same hulls as map/battle; CSS sizes stay fixed
const STRUCTURE_BASE = "/art/derived/structures/2026-09-16/";
const escAttr = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));

/** ICON SIZE TOKENS — the ONE source of truth for icon dimensions (mapped to the
 *  `--icon-sm/md/lg` CSS variables in index.html). No panel ever hardcodes a pixel
 *  size; it picks a token by role:
 *    · `sm` — inline with text (legends, prose, small badges)
 *    · `md` — stat/value chips, list rows, buttons (the default for values)
 *    · `lg` — panel/section headers, emphasis
 *  "One notch bigger everywhere" = editing the three CSS vars. */
export type IconSize = "sm" | "md" | "lg";

// The commodity/credit icons render one notch LARGER than the size tier they're
// asked for — they're the game's currency and must read at a glance in every
// context. They get their own `--icon-resource` token (see index.html), applied
// here regardless of the caller's size, so it stays consistent everywhere.
const RESOURCE_KEYS = new Set<IconKey>(["fuel", "ore", "alloys", "provisions", "volatiles", "credits", "biomass"]);

/** Render one icon at a SIZE TOKEN (never a pixel size). `tip` overrides the
 *  registry default; `cls` adds classes. Art → crisp <img>; placeholder → an
 *  emoji <span>. Both carry `.icon.icon--<size>`, so CSS drives the dimensions
 *  and the surrounding flex row centers them. Resource keys use `.icon--resource`. */
export function icon(key: IconKey, size: IconSize = "sm", tip?: string, cls = "", sizes?: string, structureTier = 1): string {
  const def = ICONS[key];
  const t = escAttr(tip ?? def.tip);
  const sizeCls = RESOURCE_KEYS.has(key) ? "icon--resource" : `icon--${size}`;
  const c = `icon ${sizeCls}${cls ? ` ${cls}` : ""}`;
  if (def.png) {
    return `<img class="${c}" src="${PNG_BASE}${def.png}.png" alt="" title="${t}" />`;
  }
  if (def.png128) {
    return `<img class="${c}" src="${PNG_128_BASE}${def.png128}.png" alt="" title="${t}" />`;
  }
  if (def.panel) {
    return `<img class="${c}" src="${PANEL_BASE}${def.panel}.png" alt="" title="${t}" />`;
  }
  if (def.hull) {
    return `<img class="${c}" src="${HULL_BASE}${def.hull}-128.png" srcset="${HULL_BASE}${def.hull}-128.png 1x, ${HULL_BASE}${def.hull}-256.png 2x" alt="" title="${t}" />`;
  }
  if (def.structure) {
    // Only the asset lookup is bounded: this never changes a gameplay tier.
    // 512px keeps the planet footprints sharp on high-DPI screens without
    // enlarging their CSS footprint or loading the full-resolution masters.
    const tier = Number.isFinite(structureTier) ? Math.max(1, Math.min(6, Math.floor(structureTier))) : 1;
    const base = `${STRUCTURE_BASE}${def.structure}/tier-${tier}`;
    return `<img class="${c}" src="${base}-128.webp" srcset="${base}-128.webp 128w, ${base}-256.webp 256w, ${base}-512.webp 512w" sizes="${escAttr(sizes ?? "48px")}" alt="" title="${t}" />`;
  }
  if (def.art) {
    return `<img class="${c}" src="${ART_BASE}${def.art}.svg" alt="" title="${t}" />`;
  }
  return `<span class="${c}" title="${t}" role="img" aria-label="${t}">${def.glyph}</span>`;
}

/** Built structures pass their ARRIVED tier; builders pass the proposed target
 * tier for the upgrade preview. A queued upgrade must not change the built art. */
export function structureImage(key: string, tier: number, size: IconSize = "sm", tip?: string, cls = "", sizes?: string): string {
  return icon(structureIcon(key), size, tip, cls, sizes, tier);
}

/** Commodity wire slugs are not always asset filenames: metallic_ore uses ore,
 * and biomass has panel art. Keep every cargo/stockpile panel on the same map. */
export function commodityIcon(commodity: Commodity): string {
  const oreArt = commodity === "metallic_ore" ? "ferrite_ore" : commodity;
  if (["ferrite_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "conductive_metals", "titanium"].includes(oreArt)) {
    const src = `${PNG_BASE}ores-2026-09-16/${oreArt}`;
    return `<img class="icon icon--resource" src="${src}-64.png" srcset="${src}-128.png 2x" alt="${escAttr(label(commodity))}" title="${escAttr(label(commodity))}">`;
  }
  const keys: Partial<Record<Commodity, IconKey>> = {
    metallic_ore: "ore", alloys: "alloys", fuel: "fuel", provisions: "provisions",
    volatiles: "volatiles", biomass: "biomass",
  };
  if (keys[commodity]) return icon(keys[commodity]!, "sm", label(commodity));
  return `<img class="icon icon--resource" src="${PNG_BASE}${commodity}.png" alt="${escAttr(label(commodity))}">`;
}

/** An icon-VALUE chip: `⛽ 120`. The whole chip carries the tooltip, so the
 *  number stays bare and the words live on hover. `value` may contain markup.
 *  Value chips default to the `md` token. */
export function chip(key: IconKey, value: string, tip?: string, size: IconSize = "md"): string {
  const t = escAttr(tip ?? ICONS[key].tip);
  return `<span class="ichip" title="${t}">${icon(key, size, tip)}<b>${value}</b></span>`;
}

/** A status BADGE chip with an icon: `⛔ blockaded`. `tone` = the badge palette
 *  (negative / positive / neutral / warn). Tooltip carries the full explanation. */
export function badgeChip(key: IconKey, label: string, tone = "neutral", tip?: string): string {
  const t = escAttr(tip ?? ICONS[key].tip);
  return `<span class="badge badge--${tone} ichip" title="${t}">${icon(key, "sm", tip)}${escAttr(label)}</span>`;
}

/** Whether `key` is still an art PLACEHOLDER (for the generation batch / audits). */
export function isPlaceholder(key: IconKey): boolean {
  return ICONS[key].placeholder;
}

/** Build wire slug → its dedicated installation art. This covers every current
 * `StructureKind`; the generic build icon remains only a forward-compatible
 * fallback for a newly added server structure whose art has not landed yet. */
export function structureIcon(key: string): IconKey {
  const icons: Record<string, IconKey> = {
    mining_complex: "structureMiningComplex",
    volatile_harvester: "structureVolatileHarvester",
    bioharvester: "structureBioharvester",
    smelter: "structureSmelter",
    electronics_fabricator: "structureElectronicsFabricator",
    chemical_works: "structureChemicalWorks",
    fuel_refinery: "structureFuelRefinery",
    agroplex: "structureAgroplex",
    machine_works: "structureMachineWorks",
    armaments_complex: "structureArmamentsComplex",
    composite_works: "structureCompositeWorks",
    hull_fabricator: "structureHullFabricator",
    precision_works: "structurePrecisionWorks",
    drive_works: "structureDriveWorks",
    shipyard: "structureShipyard",
    naval_drydock: "structureNavalDrydock",
    capital_slipway: "structureCapitalSlipway",
    ordnance_foundry: "structureOrdnanceFoundry",
    habitat: "structureHabitat",
    orbital_warehouse: "structureOrbitalWarehouse",
    warehouse: "structureWarehouse",
    sensor_array: "structureSensorArray",
    defense_platform: "structureDefensePlatform",
    academy: "structureAcademy",
    garrison: "structureGarrison",
  };
  return icons[key] ?? "build";
}
