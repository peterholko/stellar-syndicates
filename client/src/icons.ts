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
  | "deuterium" | "uranium" | "antimatter" | "fissionReactor" | "antimatterReactor" | "zephyrDrive" | "galeDrive" | "tempestDrive" | "maelstromDrive"
  | "fusionReactor" | "advancedFusionReactor" | "navComputer" | "cargoBay" | "pulseLaser" | "beamBattery" | "weaponMounts"
  | "torpedoes" | "slugs" | "beamCrystals"
  // economy / structures
  | "storage" | "slots" | "shipyard" | "sensor" | "defense" | "habitat" | "refinery" | "interdictor"
  | "extractor" | "orbital_warehouse" | "build" | "queue" | "warehouse" | "manifest" | "freightRoute" | "authorityFreighter"
  // dedicated structure-builder art
  | "structureMiningComplex" | "structureVolatileHarvester" | "structureBioharvester"
  | "structureSmelter" | "structureElectronicsFabricator" | "structureChemicalWorks" | "structureFuelRefinery"
  | "structureAgroplex" | "structureMachineWorks" | "structureArmamentsComplex"
  | "structureCompositeWorks" | "structureHullFabricator" | "structurePrecisionWorks" | "structureDriveWorks"
  | "structureShipyard" | "structureNavalDrydock" | "structureCapitalSlipway" | "structureOrdnanceFoundry"
  | "structureHabitat" | "structureWarehouse" | "structureOrbitalWarehouse" | "structureSensorArray" | "structureDefensePlatform" | "structureOrbitalDefense" | "structureCommandCenter"
  | "structureAcademy" | "structureGarrison" | "structureLivingTerraces" | "structurePowerPlant" | "structureAsteroidQuarry"
  | "structureUraniumMill" | "structureAntimatterPlant" | "structureVault" | "structureAssemblyHall" | "structureVeilProjector"
  // planetary profile / colony identity
  | "planetHabitable" | "planetHostile" | "planetUninhabitable"
  | "geologyPoor" | "geologyRich" | "geologyUltraRich"
  | "featureFertile" | "featureLowGravity" | "featurePrecursor"
  | "roleAgriculture" | "roleMining" | "roleFuel" | "roleElectronics" | "roleShipbuilding" | "rolePopulation" | "roleOutpost"
  // fleets / ship kinds
  | "fleet" | "scout" | "raider" | "corvette" | "convoy" | "colony" | "builder" | "transport"
  | "tiny_freighter" | "small_freighter" | "large_freighter" | "heavy_freighter" | "bulk_freighter" | "mining_barge" | "mining_belt_station"
  | "combat_freighter" | "free_trader" | "clean_slate" | "second_wind" | "small_fortune"
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
  | "moduleSurveyDrive" | "moduleNebulaSpectrometer" | "modulePrismaticLance" | "moduleLanceArray"
  | "moduleStasisWeb" | "moduleWarpStabilizer" | "moduleInterdictionField"
  // syndicates (§syndicates)
  | "syndicate" | "ally" | "garrison";

/** Wire slug → display label: "metallic_ore" → "Ferrite Ore". Display-only —
 * never feed the result back into commands; attributes that round-trip to
 * `net.send` (data-resource / data-hire / option values …) stay raw. */
export function label(slug: string): string {
  const camp: Record<string, string> = { camp_prospect: "Home-world survey", camp_workings: "Surface workings", camp_storehouse: "Storehouse expansion" };
  if (camp[slug]) return camp[slug];
  if (slug === "metallic_ore" || slug === "ore") return "Ferrite Ore";
  if (slug === "rare_metal_ore") return "Rare-metal Ore";
  if (slug === "helium3_ice") return "Helium-3 Ice";
  if (slug === "helium3") return "Helium-3";
  if (slug === "naval_drydock") return "Orbital Shipyard";
  // §charter-v4: Electronics read as Semiconductors; the slug stays.
  if (slug === "electronics") return "Semiconductors";
  // §shipbuilding-overhaul: Armaments is the hardware good; it shows as
  // Weapon Mounts. The Armaments Complex keeps its name.
  if (slug === "armaments") return "Weapon Mounts";
  if (slug === "electronics_fabricator") return "Semiconductor Fab";
  // §modules-as-goods: the module goods by their modules' names.
  if (slug === "survey_drive") return "Survey Booster";
  if (slug === "point_defense_screen") return "Point-Defense Screen";
  if (slug === "electronics_center") return "Semiconductor center";
  // §founding R2: a camp's harvest pauses at its food reserve, keeping storage free.
  if (slug === "food_reserve") return "Food reserve full · paused";
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
  ore: R("goods-v1/ferrite_ore-128", "Ferrite Ore"),
  alloys: R("alloys", "Alloys"),
  provisions: R("provisions", "Provisions"),
  volatiles: R("volatiles", "Volatiles"),
  credits: R("credits", "Credits"),
  biomass: N("resource-biomass", "Biomass"),
  // §fuel-overhaul: Deuterium keeps the old Fuel art until its goods icon
  // arrives (docs/resource-icon-prompts.md).
  deuterium: R("fuel", "Deuterium"),
  uranium: R("goods-v1/uranium-128", "Uranium"),
  antimatter: R("goods-v1/antimatter-128", "Antimatter"),
  // §reactors-drives: the reactor family (the Nov 7 core masters, retinted by
  // rung), the drive placeholders, kit parts and munitions
  // (docs/shipbuilding-art-prompts.md).
  fissionReactor: R("goods-v1/fission_reactor-128", "Fission Reactor"),
  fusionReactor: R("goods-v1/fusion_reactor-128", "Fusion Reactor"),
  advancedFusionReactor: R("goods-v1/advanced_fusion_reactor-128", "Advanced Fusion Reactor"),
  antimatterReactor: R("goods-v1/antimatter_reactor-128", "Antimatter Reactor"),
  zephyrDrive: R("goods-v1/zephyr_drive-128", "Zephyr Drive"),
  galeDrive: R("goods-v1/gale_drive-128", "Gale Drive"),
  tempestDrive: R("goods-v1/tempest_drive-128", "Tempest Drive"),
  maelstromDrive: R("goods-v1/maelstrom_drive-128", "Maelstrom Drive"),
  navComputer: R("goods-v1/nav_computer-128", "Nav Computer"),
  cargoBay: R("goods-v1/cargo_bay-128", "Cargo Bay"),
  pulseLaser: R("goods-v1/pulse_laser-128", "Pulse Laser"),
  beamBattery: R("goods-v1/beam_battery-128", "Beam Battery"),
  weaponMounts: R("goods-v1/armaments-128", "Weapon Mounts"),
  torpedoes: R("goods-v1/torpedoes-128", "Torpedoes"),
  slugs: R("goods-v1/slugs-128", "Slugs"),
  beamCrystals: R("goods-v1/beam_crystals-128", "Beam Crystals"),
  // economy / structures
  storage: N("concept-stockpile", "Storage / stockpile capacity"),
  slots: N("status-development-slots", "Development slots (used / total)"),
  shipyard: S("shipyard", "Shipyard tier"),
  sensor: A("concept-sensor-range", "Sensor array"),
  defense: S("defense_platform", "Defense platform tier"),
  habitat: S("habitat", "Habitat tier (output boost)"),
  refinery: S("fuel_refinery", "Fuel refinery (Volatiles → Deuterium)"),
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
  structureElectronicsFabricator: S("electronics_fabricator", "Semiconductor Fab"),
  structureChemicalWorks: S("chemical_works", "Chemical Works"),
  structureFuelRefinery: S("fuel_refinery", "Fuel Refinery"),
  structureAgroplex: S("agroplex", "Agroplex"),
  structureLivingTerraces: S("living_terraces", "Living Terraces"),
  structureAsteroidQuarry: S("asteroid_quarry", "Asteroid Quarry"),
  structureMachineWorks: S("machine_works", "Machine Works"),
  structureArmamentsComplex: S("armaments_complex", "Armaments Complex"),
  structureCompositeWorks: S("composite_works", "Composite Works"),
  structureHullFabricator: S("hull_fabricator", "Hull Fabricator"),
  structurePrecisionWorks: S("precision_works", "Precision Works"),
  structureDriveWorks: S("drive_works", "Drive Works"),
  structureShipyard: S("shipyard", "Shipyard"),
  structureNavalDrydock: S("naval_drydock", "Orbital Shipyard"),
  structureCapitalSlipway: S("capital_slipway", "Capital Slipway"),
  structureOrdnanceFoundry: S("ordnance_foundry", "Ordnance Foundry"),
  structureHabitat: S("habitat", "Habitat"),
  structureOrbitalWarehouse: S("orbital_warehouse", "Orbital Warehouse"),
  structureWarehouse: S("warehouse", "Warehouse"),
  structureCommandCenter: S("command_center", "Command Center"),
  structureSensorArray: S("sensor_array", "Sensor Array"),
  structureVeilProjector: S("veil_projector", "Veil Projector"),
  structureDefensePlatform: S("defense_platform", "Defense Platform"),
  structureOrbitalDefense: S("orbital_defense", "Orbital defense station"),
  structureAcademy: S("academy", "Academy"),
  structureGarrison: S("garrison", "Garrison"),
  // §power: stand-in — the Fuel Refinery's art until the plant's own lands.
  structurePowerPlant: S("power_plant", "Power Plant"),
  // §fuel-overhaul: stand-ins until their own art lands.
  structureUraniumMill: S("fuel_refinery", "Uranium Mill"),
  structureAntimatterPlant: S("fuel_refinery", "Antimatter Plant"),
  // §vault: stand-in — the Garrison's dug-in art until the Vault's own lands.
  structureVault: S("garrison", "Vault"),
  // §season: the Habitat stands in until the Assembly Hall's own art lands.
  structureAssemblyHall: S("habitat", "Assembly Hall"),
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
  roleElectronics: N("role-electronics", "Semiconductor center"),
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
  mining_barge: H("mining_barge", "Mining Barge"),
  mining_belt_station: H("mining_belt_station", "Mining Belt Station"),
  // §holds: the Tiny Freighter's icon until the Combat Freighter's art lands.
  combat_freighter: H("combat_freighter", "Combat Freighter"),
  free_trader: H("free_trader", "Free Trader"),
  // §traders: the other three free traders' own ships.
  clean_slate: H("clean_slate", "Clean Slate"),
  second_wind: H("second_wind", "Second Wind"),
  small_fortune: H("small_fortune", "Small Fortune"),
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
  // modules. §modules-as-goods (Nov 8 2026): each wears its own goods-v1
  // master, one style with the parts (docs/shipbuilding-art-prompts.md).
  moduleMassDriver: R("goods-v1/mass_driver-128", "Mass Driver"),
  moduleTorpedoRack: R("goods-v1/torpedo_rack-128", "Torpedo Rack"),
  modulePointDefense: R("goods-v1/point_defense_screen-128", "Point-Defense Screen"),
  moduleReflectivePlating: R("goods-v1/reflective_plating-128", "Reflective Plating"),
  moduleWhippleArmor: R("goods-v1/whipple_armor-128", "Whipple Armor"),
  moduleExtendedTanks: R("goods-v1/extended_tanks-128", "Extended Tanks"),
  moduleReconSuite: R("goods-v1/recon_suite-128", "Recon Suite"),
  moduleCargoPods: R("goods-v1/cargo_pods-128", "Cargo Pods"),
  moduleEscortDatalink: R("goods-v1/escort_datalink-128", "Escort Datalink"),
  moduleFuelTransferRig: R("goods-v1/fuel_transfer_rig-128", "Fuel Transfer Rig"),
  moduleSurveyDrive: R("goods-v1/survey_drive-128", "Survey Booster"),
  moduleNebulaSpectrometer: R("goods-v1/nebula_spectrometer-128", "Nebula Spectrometer"),
  modulePrismaticLance: R("goods-v1/prismatic_lance-128", "Prismatic Lance"),
  moduleLanceArray: R("goods-v1/lance_array-128", "Lance Array"),
  moduleStasisWeb: R("goods-v1/stasis_web-128", "Stasis Web"),
  moduleWarpStabilizer: R("goods-v1/warp_stabilizer-128", "Warp Stabilizer"),
  moduleInterdictionField: R("goods-v1/interdiction_field-128", "Interdiction Field"),
  // syndicates
  syndicate: P("🤝", "Syndicate (alliance)"),
  ally: P("🟢", "Syndicate ally"),
  garrison: P("🛰", "Ally garrison"),
};

const ART_BASE = "/art/ui_icons/svg/";
const PNG_BASE = "/art/ui_icons/resource/"; // downscaled 64px resource PNGs
const PNG_128_BASE = "/art/ui_icons/png/128/"; // high-DPI general UI PNGs
const PANEL_BASE = "/art/ui_icons/panel/"; // generated transparent 64px panel PNGs
const HULL_BASE = "/art/derived/ships/models-v1/panels/"; // LOD2 model renders; CSS sizes stay fixed
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
const RESOURCE_KEYS = new Set<IconKey>(["fuel", "ore", "alloys", "provisions", "volatiles", "credits", "biomass",
  "deuterium", "uranium", "antimatter", "fissionReactor", "antimatterReactor", "zephyrDrive", "galeDrive", "tempestDrive", "maelstromDrive", "fusionReactor",
  "advancedFusionReactor", "navComputer", "cargoBay", "pulseLaser", "beamBattery", "weaponMounts",
  "torpedoes", "slugs", "beamCrystals"]);

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
    const command = def.structure === "command_center";
    const station = def.structure === "orbital_defense";
    // §gaia: Living Terraces have no separate menu set; menus show their scene sprites.
    // These families have no separate menu icon set: menus use the scene sprites.
    const sceneOnly = def.structure === "living_terraces" || def.structure === "power_plant" || def.structure === "veil_projector";
    const maxTier = command || def.structure === "veil_projector" ? 3 : 6;
    const tier = Number.isFinite(structureTier) ? Math.max(1, Math.min(maxTier, Math.floor(structureTier))) : 1;
    // §laser-battery: the station's level 1 is a ground turret.
    const base = command ? `/art/derived/command-center/v1/tier-${tier}`
      : station ? tier === 1 ? "/art/derived/planet-surface-structures/v2/buildings/defense_platform/tier-1" : `/art/derived/stations/v2/level-${tier}`
      // §asteroid-quarry: its rock and machinery, framed square for menus.
      : def.structure === "asteroid_quarry" ? `/art/derived/asteroid-quarry/v2/icon-tier-${tier}`
      : sceneOnly ? `/art/derived/planet-surface-structures/v2/buildings/${def.structure}/tier-${tier}` : `${STRUCTURE_BASE}${def.structure}/tier-${tier}`;
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

/** §goods-icons: goods whose goods-v1 icon exists, all from one style of
 * master trimmed to one fill (docs/resource-icon-prompts.md). The ore-economy
 * test keeps this in step with the masters in art-src. */
export const GOODS_ICONS: ReadonlySet<string> = new Set([
  "advanced_fusion_reactor", "antimatter", "armaments", "beam_battery", "beam_crystals", "bornite_ore",
  "antimatter_reactor", "cargo_bay", "conductive_metals", "crystalline_ore", "cuprite_ore", "exotic_matter", "ferrite_ore",
  "fission_reactor", "fusion_reactor", "gale_drive", "helium3", "helium3_ice",
  "ilmenite_ore", "iridium", "iridium_ore", "maelstrom_drive", "nav_computer", "olivine_ore", "pulse_laser", "rare_metal_ore",
  "slugs", "tempest_drive", "titanium", "titanium_ore", "torpedoes", "uranium", "void_crystal", "zephyr_drive",
  "hull_sections",
  // §modules-as-goods: the seventeen modules (goods-2026-11-08).
  "cargo_pods", "escort_datalink", "extended_tanks", "fuel_transfer_rig", "interdiction_field", "lance_array", "mass_driver",
  "nebula_spectrometer", "point_defense_screen", "prismatic_lance", "recon_suite", "reflective_plating", "stasis_web",
  "survey_drive", "torpedo_rack", "warp_stabilizer", "whipple_armor",
  // §hull-ladder: every heavy (a cradle) and capital (a gantry) part, an edit
  // of its light master (goods-2026-11-08).
  ...["fission_reactor", "fusion_reactor", "advanced_fusion_reactor", "antimatter_reactor",
    "zephyr_drive", "gale_drive", "tempest_drive", "maelstrom_drive", "hull_sections"]
    .flatMap((part) => [`heavy_${part}`, `capital_${part}`]),
]);

/** Commodity wire slugs are not always asset filenames: metallic_ore uses ore,
 * and biomass has panel art. Keep every cargo/stockpile panel on the same map.
 * §goods-icons: a good with a goods-v1 icon (one style, one fill) uses it; the
 * rest keep their older art until their masters arrive. */
export function commodityIcon(commodity: Commodity): string {
  // §hull-ladder: a heavy or capital part wears its own master (Nov 8 2026);
  // a size without one falls back to its light part's icon.
  const base = commodity.replace(/^(heavy|capital)_/, "") as Commodity;
  const slug = commodity === "metallic_ore" ? "ferrite_ore" : GOODS_ICONS.has(commodity) ? commodity : base;
  if (GOODS_ICONS.has(slug)) {
    const src = `${PNG_BASE}goods-v1/${slug}`;
    return `<img class="icon icon--resource" src="${src}-64.png" srcset="${src}-128.png 2x" alt="${escAttr(label(commodity))}" title="${escAttr(label(commodity))}">`;
  }
  const keys: Partial<Record<Commodity, IconKey>> = {
    metallic_ore: "ore", alloys: "alloys", deuterium: "deuterium", provisions: "provisions",
    volatiles: "volatiles", biomass: "biomass", uranium: "uranium", antimatter: "antimatter",
    // §reactors-drives: the reactor and drive ladders and the kit parts.
    fission_reactor: "fissionReactor", fusion_reactor: "fusionReactor", advanced_fusion_reactor: "advancedFusionReactor",
    antimatter_reactor: "antimatterReactor",
    zephyr_drive: "zephyrDrive", gale_drive: "galeDrive", tempest_drive: "tempestDrive", maelstrom_drive: "maelstromDrive",
    nav_computer: "navComputer", cargo_bay: "cargoBay", pulse_laser: "pulseLaser", beam_battery: "beamBattery",
    armaments: "weaponMounts",
    torpedoes: "torpedoes", slugs: "slugs", beam_crystals: "beamCrystals",
    // §modules-as-goods: a module good wears its module's icon.
    mass_driver: "moduleMassDriver", torpedo_rack: "moduleTorpedoRack", point_defense_screen: "modulePointDefense",
    reflective_plating: "moduleReflectivePlating", whipple_armor: "moduleWhippleArmor", extended_tanks: "moduleExtendedTanks",
    recon_suite: "moduleReconSuite", cargo_pods: "moduleCargoPods", escort_datalink: "moduleEscortDatalink",
    fuel_transfer_rig: "moduleFuelTransferRig", survey_drive: "moduleSurveyDrive", nebula_spectrometer: "moduleNebulaSpectrometer",
    prismatic_lance: "modulePrismaticLance", lance_array: "moduleLanceArray", stasis_web: "moduleStasisWeb",
    warp_stabilizer: "moduleWarpStabilizer", interdiction_field: "moduleInterdictionField",
  };
  if (keys[base]) return icon(keys[base]!, "sm", label(commodity));
  return `<img class="icon icon--resource" src="${PNG_BASE}${base}.png" alt="${escAttr(label(commodity))}">`;
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
    living_terraces: "structureLivingTerraces",
    asteroid_quarry: "structureAsteroidQuarry",
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
    command_center: "structureCommandCenter",
    service_berth: "structureWarehouse",
    sensor_array: "structureSensorArray",
    defense_platform: "structureDefensePlatform",
    orbital_defense: "structureOrbitalDefense",
    academy: "structureAcademy",
    garrison: "structureGarrison",
    power_plant: "structurePowerPlant",
    uranium_mill: "structureUraniumMill",
    antimatter_plant: "structureAntimatterPlant",
    vault: "structureVault",
    assembly_hall: "structureAssemblyHall",
    veil_projector: "structureVeilProjector",
  };
  return icons[key] ?? "build";
}
