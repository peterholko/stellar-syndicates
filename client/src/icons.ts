// SEMANTIC ICON REGISTRY (§UX text diet). One meaning → one key → one glyph,
// used EVERYWHERE, so an icon reads the same in every panel and swapping in
// generated art later is a one-file change.
//
// Each entry is either ART-BACKED (`art` = a bundled SVG slug under
// /art/ui_icons/svg/, rendered crisp) or a PLACEHOLDER (`glyph` = a unicode/emoji
// stand-in until dedicated art is generated — `placeholder: true`, surfaced in
// the icon-generation batch). Every entry carries a default `tip` (hover text),
// because in the icon-first UI the words live in tooltips, not on screen.
//
// Render through `icon()` / `chip()` / `badgeChip()` — never hand-roll an <img>.

export type IconKey =
  // resources
  | "fuel" | "ore" | "alloys" | "provisions" | "volatiles" | "credits"
  // economy / structures
  | "storage" | "slots" | "shipyard" | "sensor" | "defense" | "habitat" | "refinery" | "interdictor"
  | "extractor" | "depot" | "build" | "queue"
  // fleets / ship kinds
  | "fleet" | "scout" | "raider" | "corvette" | "convoy" | "colony"
  // verbs / orders
  | "move" | "attack" | "raid" | "withdraw" | "reinforce" | "recall" | "blockade" | "siege"
  | "doctrine" | "posture" | "claim" | "cargo" | "market"
  // transit / signature
  | "stealth" | "flank" | "sensorRange"
  // order lifecycle (light-delayed round trip)
  | "delay" | "echo" | "delivered" | "confirmed" | "inTransit"
  // status / intel
  | "unfed" | "fed" | "warning" | "unknown" | "intel" | "battle" | "aftermath" | "captured" | "lost"
  | "commandCenter" | "uncertainty" | "hub" | "success" | "info" | "home" | "mouse" | "shift" | "time"
  // syndicates (§syndicates)
  | "syndicate" | "ally" | "garrison";

interface IconDef {
  /** Downscaled RASTER (PNG) variant name under /art/ui_icons/resource/ —
   *  highest precedence (the resource icons). Small, retina-crisp. */
  png?: string;
  /** Bundled SVG slug (art-backed) — takes precedence over `glyph`. */
  art?: string;
  /** Unicode/emoji placeholder when there is no art yet. */
  glyph?: string;
  /** Default hover text — the words the icon replaced. */
  tip: string;
  /** True while the glyph is a stand-in awaiting generated art. */
  placeholder: boolean;
}

// R(name) = downscaled PNG; A(slug) = art-backed SVG; P(glyph) = emoji placeholder.
const R = (png: string, tip: string): IconDef => ({ png, tip, placeholder: false });
const A = (art: string, tip: string): IconDef => ({ art, tip, placeholder: false });
const P = (glyph: string, tip: string): IconDef => ({ glyph, tip, placeholder: true });

export const ICONS: Record<IconKey, IconDef> = {
  // resources — dedicated downscaled PNG art (source-of-truth 1254px; UI loads 64px)
  fuel: R("fuel", "Fuel"),
  ore: R("ore", "Ore"),
  alloys: R("alloys", "Alloys"),
  provisions: R("provisions", "Provisions"),
  volatiles: R("volatiles", "Volatiles"),
  credits: R("credits", "Credits"),
  // economy / structures
  storage: P("📦", "Storage / stockpile capacity"),
  slots: P("▦", "Development slots (used / total)"),
  shipyard: P("🛠", "Shipyard tier"),
  sensor: A("concept-sensor-range", "Sensor array"),
  defense: P("🛡", "Defense platform tier"),
  habitat: P("🏠", "Habitat tier (output boost)"),
  refinery: P("⚗", "Fuel refinery (Volatiles → Fuel)"),
  interdictor: P("⛓", "Interdictor"),
  extractor: P("⛏", "Extractor tier (output ×1.5)"),
  depot: P("🏬", "Depot tier (storage cap)"),
  build: A("action-build", "Build"),
  queue: P("🔨", "Under construction"),
  // fleets / ship kinds
  fleet: A("concept-fleet", "Fleet"),
  scout: P("🛰", "Scout"),
  raider: P("🗡", "Raider"),
  corvette: P("🛡", "Corvette"),
  convoy: A("concept-convoy", "Convoy"),
  colony: P("🏗", "Colony ship"),
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
  cargo: A("action-load-cargo", "Cargo"),
  market: A("concept-market-exchange", "Hub market"),
  // transit / signature
  stealth: P("🌑", "Stealth transit (quiet, ~2× trip)"),
  flank: P("💨", "Full speed (loud — high signature)"),
  sensorRange: A("concept-sensor-range", "Sensor range"),
  // order lifecycle
  delay: A("concept-lightspeed-signal", "Command / light delay"),
  echo: P("◔", "Awaiting echo (executing, unconfirmed)"),
  delivered: P("◈", "In transit (order en route)"),
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
  // syndicates
  syndicate: P("🤝", "Syndicate (alliance)"),
  ally: P("🟢", "Syndicate ally"),
  garrison: P("🛰", "Ally garrison"),
};

const ART_BASE = "/art/ui_icons/svg/";
const PNG_BASE = "/art/ui_icons/resource/"; // downscaled 64px resource PNGs
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
const RESOURCE_KEYS = new Set<IconKey>(["fuel", "ore", "alloys", "provisions", "volatiles", "credits"]);

/** Render one icon at a SIZE TOKEN (never a pixel size). `tip` overrides the
 *  registry default; `cls` adds classes. Art → crisp <img>; placeholder → an
 *  emoji <span>. Both carry `.icon.icon--<size>`, so CSS drives the dimensions
 *  and the surrounding flex row centers them. Resource keys use `.icon--resource`. */
export function icon(key: IconKey, size: IconSize = "sm", tip?: string, cls = ""): string {
  const def = ICONS[key];
  const t = escAttr(tip ?? def.tip);
  const sizeCls = RESOURCE_KEYS.has(key) ? "icon--resource" : `icon--${size}`;
  const c = `icon ${sizeCls}${cls ? ` ${cls}` : ""}`;
  if (def.png) {
    return `<img class="${c}" src="${PNG_BASE}${def.png}.png" alt="" title="${t}" />`;
  }
  if (def.art) {
    return `<img class="${c}" src="${ART_BASE}${def.art}.svg" alt="" title="${t}" />`;
  }
  return `<span class="${c}" title="${t}" role="img" aria-label="${t}">${def.glyph}</span>`;
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
