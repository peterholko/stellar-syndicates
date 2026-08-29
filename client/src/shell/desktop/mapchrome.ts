import { theaterDebug, theaterHash, theaterSetTime, theaterStep } from "../../battletheater";
import { jumpCapable, shipKindLabel } from "../../core/derive/fleet";
import { fmt } from "../../core/derive/format";
import { nearestSystemName, systemUnderCursor } from "../../core/derive/geo";
import { reservedMarketCredits, spendableMarketCredits } from "../../core/derive/market";
import { saveBattleMarks } from "../../core/derive/orders";
import { armJumpAiming, beginPendingIntent, clearGuardAiming, clearJumpAiming, clearPendingIntent, confirmPendingIntent, intentAiming } from "../../core/intent";
import { type MapClickResult, resolveMapClick, resolveSystemClick } from "../../core/mapclick";
import { groundTheaterDebug } from "../../groundtheater";
import { badgeChip, icon, type IconKey, type IconSize, label } from "../../icons";
import { type BodyView, type Commodity, countClassLabel, formatId, type TimelineEntry } from "../../protocol";
import { renderer } from "../../render";
import { type LinkStatus, liveSimTime, state } from "../../state";
import type { Rect } from "../types";
import { bindLandingDelegate, buildBattlePanel, closeBattleViewer, enterBattleViewer, openBattlePanel, openBattleViewer, openGroundViewer, openOngoingBattlePanel, theaterDemo, theaterDemoLive } from "./battle";
import { toggleCheckin } from "./checkin";
import { toggleFaction } from "./faction";
import { net } from "./index";
import { openHubPanel, toggleMarket } from "./market";
import { toggleOperations } from "./operations";
import { openRail, toggleRail } from "./rail";
import { FIELD_TITLE, toggleResearch } from "./research";
import { addTransientReport, cycleOwnFleet, deselectShip, fmtCountdown, selectEmplacement, selectJumpDeparture, selectShip, toggleShipSelection, updateShipPanel } from "./ship";
import { toggleSyndicate } from "./syndicate";
import { closeBuildPanel, closePlanetPanel, enterSystem, exitSystem, hideSystemUi, openPlanetPanel, showSystemUi } from "./sysview";
import { activateWorkspacePage, activeWorkspacePage, workspaceBack } from "./workspace";


// --- DOM handles -----------------------------------------------------------
export const CONTACT_STALE_AGE_S = 8;

export const $ = (id: string) => document.getElementById(id)!;

export const joinScreen = $("join");

export const joinBtn = $("join-btn") as HTMLButtonElement;

export const nameInput = $("name") as HTMLInputElement;

export const joinErr = $("join-err");

export const hud = $("hud");

export const foundingGuide = $("founding-guide");


export function syncFoundingGuideClearance(): void {
  const guideBox = foundingGuide.getBoundingClientRect();
  const clearance = foundingGuide.classList.contains("is-open") && guideBox.height > 0
    ? guideBox.height + 22 // 14px viewport inset + 8px breathing room
    : 14;
  document.documentElement.style.setProperty("--founding-guide-clearance", `${Math.ceil(clearance)}px`);
}


// The navbar wraps as the viewport narrows, so fixed panels cannot safely use a
// guessed top offset. Publish its measured lower edge as the one CSS anchor for
// breadcrumbs, rails, reports, and detail panels.
export function syncHudSafeTop(): void {
  const box = hud.getBoundingClientRect();
  if (box.height <= 0) return;
  document.documentElement.style.setProperty("--hud-safe-top", `${Math.ceil(box.bottom) + 4}px`);
  syncFoundingGuideClearance();
}

export function __init_mapchrome_52(): void {
new ResizeObserver(syncHudSafeTop).observe(hud);
}

export function __init_mapchrome_53(): void {
new ResizeObserver(syncFoundingGuideClearance).observe(foundingGuide);
}

export function __init_mapchrome_54(): void {
window.addEventListener("resize", syncHudSafeTop);
}

export function __init_mapchrome_55(): void {
window.visualViewport?.addEventListener("resize", syncHudSafeTop);
}

export function __init_mapchrome_56(): void {
window.visualViewport?.addEventListener("scroll", syncHudSafeTop);
}


export const RIGHT_DOCK_IDS = ["desktop-workspace", "sysview-manage"] as const;

export const FOCUS_OVERLAY_IDS = [] as const;

export const LAYOUT_WATCH_IDS = [
  ...RIGHT_DOCK_IDS, ...FOCUS_OVERLAY_IDS,
  "planet-panel", "build-panel", "build-ship-panel",
] as const;

export const panelOpen = (id: string): boolean => {
  const el = $(id);
  // Most panels use .is-open; the legacy Check-in panel still toggles its
  // inline display. Treat both as the same occupancy signal.
  return el.classList.contains("is-open") || (!!el.style.display && el.style.display !== "none");
};

export function syncOverlayLayout(): void {
  document.body.classList.toggle("is-right-dock-open", RIGHT_DOCK_IDS.some(panelOpen));
  document.body.classList.toggle("is-focus-overlay-open", FOCUS_OVERLAY_IDS.some(panelOpen));
  document.body.classList.toggle("is-planet-panel-open", panelOpen("planet-panel"));
  document.body.classList.toggle("is-build-panel-open", panelOpen("build-panel") || panelOpen("build-ship-panel"));

  renderer.setCameraRect(desktopCameraRect());
}

export function desktopCameraRect(): Rect {
  let mapRight = window.innerWidth;
  for (const id of RIGHT_DOCK_IDS) {
    if (!panelOpen(id)) continue;
    const box = $(id).getBoundingClientRect();
    if (box.width > 0) mapRight = Math.min(mapRight, box.left);
  }
  return { x: 0, y: 0, w: Math.max(1, mapRight), h: Math.max(1, window.innerHeight) };
}

export const overlayLayoutObserver = new MutationObserver(syncOverlayLayout);

export function __init_mapchrome_88(): void {
for (const id of LAYOUT_WATCH_IDS) {
  overlayLayoutObserver.observe($(id), { attributes: true, attributeFilter: ["class", "style"] });
}
}

export function __init_mapchrome_91(): void {
syncOverlayLayout();
}

export function __init_mapchrome_92(): void {
window.addEventListener("resize", syncOverlayLayout);
}


export function setHud(): void {
  $("hud-name").textContent = state.name || "—";
  $("hud-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
  $("hud-tick").textContent = state.link === "online" ? state.tick.toLocaleString() : "—";
  $("hud-time").textContent =
    state.link === "online"
      ? `${state.simTime.toFixed(1)}s${state.pacingScale !== 1 ? ` · FAST ${state.pacingScale}×` : ""}`
      : "—";
  $("hud-online").textContent = state.link === "online" ? String(state.corpsInView) : "—";
  $("hud-ships").textContent = state.link === "online" ? String(state.ghosts.length) : "—";
  const reserved = reservedMarketCredits();
  const hudCredits = $("hud-credits");
  hudCredits.textContent = state.wallet
    ? `${reserved > 0 ? "~" : ""}${Math.round(spendableMarketCredits()).toLocaleString()}`
    : "—";
  hudCredits.title = reserved > 0
    ? `${fmt(state.wallet!.credits)} last reported · ${fmt(reserved)} reserved by orders awaiting the Market Hub report`
    : "Last-arrived Market Hub account report";
  $("hud-equity").textContent = state.wallet ? `${Math.round(state.wallet.valuation).toLocaleString()}` : "—";
  const link = $("hud-link");
  const labels: Record<LinkStatus, string> = {
    connecting: "connecting…",
    reconnecting: "reconnecting…",
    online: "● online",
    offline: "✕ disconnected",
  };
  link.textContent = labels[state.link];
  link.className = "v " + (state.link === "online" ? "accent" : "warn");
}


export {
  __init_mapchrome_142,
  __init_mapchrome_149,
  __init_mapchrome_150,
  flushPressGuard,
  lastHtmlWritten,
  morphChildren,
  morphElement,
  nodeKey,
  NODE_KEY_ATTRS,
  pressGuard,
  renderDeferred,
  setHtml,
} from "../dom";


// Debug hook (harmless): lets tooling inspect the live view state and transform.
export function __init_mapchrome_314(): void {
bindLandingDelegate();
}

export function __init_mapchrome_315(): void {
(window as unknown as { __ss: unknown }).__ss = { state, renderer, openBattleViewer, openGroundViewer, groundTheaterDebug, theaterHash, theaterDebug, theaterSetTime, theaterStep, theaterDemo: (titanDown = false, big = false) => theaterDemo(titanDown, big), theaterDemoLive: (ms = 1800) => theaterDemoLive(ms) };
}


export function onDesktopRenderFrame(): void {
  updateZoomLevel();
  const scrubEndpoint = renderer.consumeSystemScrubEndpoint();
  if (scrubEndpoint?.type === "system") {
    const sys = state.galaxy?.systems.find((s) => s.id === scrubEndpoint.systemId);
    if (sys) showSystemUi(sys);
  } else if (scrubEndpoint?.type === "galaxy") {
    hideSystemUi();
  }
}


export let lastZoomLevel = "";

export function updateZoomLevel(): void {
  const mode = renderer.viewMode.type;
  const zoom = renderer.zoomFactor();
  const text = mode === "system"
    ? "SYSTEM"
    : mode === "battle"
      ? "BATTLE"
      : zoom < 10
        ? `${zoom.toFixed(1)}×`
        : `${Math.round(zoom)}×`;
  if (text === lastZoomLevel) return;
  lastZoomLevel = text;
  const level = $("zoom-level");
  level.textContent = text;
  level.title = mode === "galaxy"
    ? `${zoom.toFixed(2)}× galaxy magnification relative to fit-to-map`
    : `${text.toLowerCase()} semantic view`;
}


export const readout = () => $("readout");


function updateMapHover(clientX: number, clientY: number): void {
  const tip = $("map-hover");
  if (renderer.viewMode.type !== "galaxy") {
    tip.classList.remove("is-open");
    renderer.canvas.style.cursor = "default";
    return;
  }

  let copy = "";
  let best = Infinity;
  const engaged = new Map<string, { x: number; y: number }>();
  for (const battle of state.battles) for (const id of battle.participants) engaged.set(id, battle.pos);
  for (const ghost of state.ghosts) {
    const battlePos = engaged.get(ghost.id);
    const p = battlePos ? renderer.worldToScreen(battlePos) : renderer.fleetScreenPosition(ghost);
    const d = Math.hypot(p.x - clientX, p.y - clientY);
    const radius = ghost.docked ? 11 : Math.max(18, renderer.fleetHitRadius(ghost));
    if (d >= radius || d >= best) continue;
    best = d;
    copy = ghost.own
      ? `${shipKindLabel(ghost.kind)} · ${ghost.docked ? "select berth" : battlePos ? "select engaged fleet" : "select fleet"}`
      : `${shipKindLabel(ghost.kind)} contact · select for delayed intelligence`;
  }
  if (state.galaxy) {
    for (const system of state.galaxy.systems) {
      const p = renderer.worldToScreen(system.pos);
      const d = Math.hypot(p.x - clientX, p.y - clientY);
      if (d >= Math.max(15, renderer.systemHitRadius(system)) || d >= best) continue;
      best = d;
      const selected = state.selectedShipId
        ? state.ghosts.find((ghost) => ghost.id === state.selectedShipId && ghost.own)
        : undefined;
      copy = selected ? `Move ${shipKindLabel(selected.kind)} to ${system.name}` : `Inspect ${system.name}`;
    }
    const hub = renderer.worldToScreen(state.galaxy.hub);
    const d = Math.hypot(hub.x - clientX, hub.y - clientY);
    if (d < Math.max(24, renderer.hubHitRadius()) && d < best) copy = "Open Wormhole Hub";
  }
  const battle = renderer.battlePick(clientX, clientY);
  if (battle !== null && !copy) copy = "Open ongoing battle";

  if (!copy) {
    tip.classList.remove("is-open");
    renderer.canvas.style.cursor = state.selectedShipId ? "crosshair" : "grab";
    return;
  }
  tip.textContent = copy;
  tip.classList.add("is-open");
  const left = Math.min(window.innerWidth - Math.max(180, tip.offsetWidth) - 8, clientX + 14);
  const top = Math.min(window.innerHeight - tip.offsetHeight - 8, clientY + 16);
  tip.style.left = `${Math.max(8, left)}px`;
  tip.style.top = `${Math.max(8, top)}px`;
  renderer.canvas.style.cursor = "pointer";
}


// --- UI kit (Stellar-Charters-inspired) — string-template helpers every panel
// composes from. Each returns an HTML string; panels assign once via innerHTML and
// wire interaction through ONE delegated listener per root (handler-safe across
// re-renders). Tone is always a class → color-via-CSS-var, so the whole workspace
// themes from index.html's :root tokens. ------------------------------------
export const fmtPopulation = (millions: number): string => {
  const people = Math.max(0, Math.round(millions * 1_000_000));
  return people >= 1_000_000
    ? `${(people / 1_000_000).toFixed(people >= 10_000_000 ? 0 : 1)}M`
    : people.toLocaleString();
};

export const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));

export const badge = (tone: string, txt: string) => `<span class="badge badge--${tone}">${esc(txt)}</span>`;

export const bar = (pct: number, tone = "") =>
  `<div class="bar"><div class="bar__fill ${tone}" style="width:${Math.max(0, Math.min(100, pct))}%"></div></div>`;

export const stat = (label: string, value: string, tone = "") =>
  `<div class="stat"><dt>${esc(label)}</dt><dd class="${tone}">${value}</dd></div>`;

export const statStrip = (cells: string[], cls = "") => `<div class="stat-strip${cls ? ` ${cls}` : ""}">${cells.join("")}</div>`;


// Sparkline as inline SVG — no deps. Stroke auto-colors by trend (first vs last).
export function spark(data: number[], w = 60, h = 18): string {
  const pts = data.length >= 2 ? data : [data[0] ?? 0, data[0] ?? 0];
  const min = Math.min(...pts), max = Math.max(...pts), span = max - min || 1;
  const stroke = pts[pts.length - 1] >= pts[0] ? "var(--positive)" : "var(--negative)";
  const path = pts
    .map((v, i) => `${((i / (pts.length - 1)) * w).toFixed(1)},${(h - ((v - min) / span) * (h - 2) - 1).toFixed(1)}`)
    .join(" ");
  return `<svg class="spark" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true">` +
    `<polyline fill="none" stroke="${stroke}" stroke-width="1.5" vector-effect="non-scaling-stroke" points="${path}"/></svg>`;
}


// The native Stellar Syndicates icon set (/art/ui_icons/svg) — full-color SVG,
// crisp at any size, used as <img>. Resources / Actions / Concepts / Status. This
// SUPERSEDES the earlier Stellar-Charters borrow. No loading="lazy" — these panels
// re-render ~10 Hz, recreating the <img>; lazy would replace them before the
// observer fires. Eager hits the browser cache instantly.
export const svgIcon = (slug: string, size: IconSize = "sm", cls = "") =>
  `<img class="icon icon--${size}${cls ? ` ${cls}` : ""}" src="/art/ui_icons/svg/${slug}.svg" alt="" />`;


// The ONE raster-icon path helper: a downscaled PNG under /art/ui_icons/<category>/
// (transparent-background art — the commodity resource icons and the research field
// emblems), or the `glyph` fallback when `slug` is empty, so a missing/unknown icon
// degrades to a legible symbol instead of a broken <img>. Both commodity and
// research field icons go through here — no third copy of the path logic.
export function uiIcon(category: "resource" | "research", slug: string | undefined, glyph: string, title = "", cls = ""): string {
  const klass = cls || `icon icon--${category}`;
  const t = title ? ` title="${esc(title)}"` : "";
  return slug
    ? `<img class="${klass}" src="/art/ui_icons/${category}/${slug}.png" alt=""${t} />`
    : `<span class="${klass} icon--glyph"${t}>${glyph}</span>`;
}


// A commodity icon is by definition a resource, so it always uses the dedicated
// `--icon-resource` token + the downscaled PNG art (each commodity now has its own,
// including Volatiles — no more hue-shifted Fuel stand-in). `size` kept for symmetry.
// §economy: the ORIGINAL five have dedicated PNG art (metallic_ore reuses the
// old ore art). Biomass uses the generated panel set because it was authored in
// the same hard-surface family as the new economy and colony icons.
export const COMMODITY_ART: Partial<Record<Commodity, string>> = {
  fuel: "fuel", metallic_ore: "ore", alloys: "alloys", provisions: "provisions", volatiles: "volatiles",
  // The six industrial goods — the framed-tile set sliced from the extended sheet
  // (file names match the wire slugs).
  rare_elements: "rare_elements", silicates: "silicates", electronics: "electronics",
  polymers: "polymers", machinery: "machinery", armaments: "armaments",
};

export const COMMODITY_GLYPH: Record<Commodity, string> = {
  metallic_ore: "\u26cf", rare_elements: "\u2728", silicates: "\u25a6", volatiles: "\u2744", biomass: "\ud83c\udf3f",
  alloys: "\ud83d\udd29", electronics: "\ud83d\udda5", polymers: "\ud83e\uddea", fuel: "\u26fd", provisions: "\ud83c\udf5e",
  machinery: "\u2699", armaments: "\ud83d\udd2b",
};

// A commodity icon is by definition a resource — the shared helper with the
// `--icon-resource` token + a glyph fallback for goods whose art hasn't landed.
export const commodityIcon = (c: Commodity, size: IconSize = "md") =>
  c === "biomass"
    ? icon("biomass", size, label(c))
    : uiIcon("resource", COMMODITY_ART[c], COMMODITY_GLYPH[c], label(c));


// §research R6: the six FIELD emblems (hexagonal art, 256px masters + 64px `-sm`
// variants under /art/ui_icons/research/), used wherever a research field is named.
// `size` picks the asset (sm → the light 64px variant) and the CSS token
// (rf-ic--{sm|md|lg|xl} ≈ 26 / 44 / 60 / 72 px); glyph fallback degrades gracefully.
export const RESEARCH_FIELDS = new Set(["propulsion", "materials", "computation", "weapons", "hulls", "life"]);

export const RESEARCH_GLYPH: Record<string, string> = {
  propulsion: "🚀", materials: "⚙️", computation: "📡", weapons: "🎯", hulls: "🛡️", life: "🌱",
};

export function researchIcon(field: string, size: "sm" | "md" | "lg" | "xl" = "md"): string {
  const has = RESEARCH_FIELDS.has(field);
  const slug = has ? (size === "sm" ? `${field}-sm` : field) : undefined;
  return uiIcon("research", slug, RESEARCH_GLYPH[field] ?? "◆", FIELD_TITLE[field] ?? field, `rf-ic rf-ic--${size}`);
}


// Status icon by timeline severity (the native Status set).
export const STATUS_SLUG: Record<TimelineEntry["severity"], string> = {
  good: "status-success",
  bad: "status-warning-threat",
  warn: "status-warning-threat",
  info: "status-info",
};

export const statusIcon = (sev: TimelineEntry["severity"], size: IconSize = "sm") => svgIcon(STATUS_SLUG[sev], size);


// The generated panel set is routed through the semantic registry, then chosen
// from the exact served body facts. These helpers are presentation only: they do
// not infer survey-gated geology or a colony role the server did not send.
export const ENVIRONMENT_ICON: Record<BodyView["environment"], IconKey> = {
  gaia: "planetHabitable",
  terran: "planetHabitable",
  marginal: "planetHostile",
  hostile: "planetHostile",
  uninhabitable: "planetUninhabitable",
};

export const GEOLOGY_ICON: Partial<Record<Exclude<BodyView["geology"], null>, IconKey>> = {
  ultra_poor: "geologyPoor",
  poor: "geologyPoor",
  rich: "geologyRich",
  ultra_rich: "geologyUltraRich",
};

export const FEATURE_ICON: Partial<Record<Exclude<BodyView["special"], null>, IconKey>> = {
  fertile_biosphere: "featureFertile",
  low_gravity: "featureLowGravity",
  precursor_ruins: "featurePrecursor",
};

export const COLONY_ROLE_ICON: Record<import("../../protocol").ColonyOpportunityView["role"], IconKey> = {
  agricultural_exporter: "roleAgriculture",
  mining_world: "roleMining",
  fuel_complex: "roleFuel",
  electronics_center: "roleElectronics",
  shipbuilding_center: "roleShipbuilding",
  population_world: "rolePopulation",
  strategic_outpost: "roleOutpost",
};


// §contestable-territory Part 2: the CAPTURE results panel — a system changed
// hands. Reuses the ember-striped battle-panel element + the shared viewed/
// dismissed sets (capture ids are globally unique). Shows the flip in the
// recipient's terms (you captured / you lost), the light delay, and the plunder.
export function openCapturePanel(id: number): void {
  const r = state.captureReports.find((x) => x.id === id);
  if (!r) return;
  buildBattlePanel();
  state.battleViewed.add(id);
  saveBattleMarks();
  const now = liveSimTime();
  const ago = (t: number) => fmtCountdown(Math.max(0, now - t));
  const plunderStr = r.plunder.length ? r.plunder.map((s) => `${s.units} ${esc(label(s.commodity))}`).join(", ") : "an empty stockpile";
  const verdict = r.captor
    ? badgeChip("captured", "captured — yours", "positive", "Your marines took the ground (one Troop Transport consumed). The old owner keeps their fleets — no elimination.")
    : badgeChip("lost", "lost — taken", "negative", "Your fleets survive; only the territory changed hands. Retake it the same way — blockade, suppress, and land a colony ship.");
  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">capture · delayed report</div>` +
    `<h2>${r.captor ? "Captured" : "Lost"} ${esc(nearestSystemName(r.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const body =
    `<div class="sp-line">${verdict}</div>` +
    `<div class="sp-sec">When</div>` +
    `<div class="sp-line">Fell <b>${ago(r.at_time)}</b> ago · you learned <b>${ago(r.learned_at)}</b> ago <span class="dim">(light delay ${fmtCountdown(Math.max(0, r.learned_at - r.at_time))})</span></div>` +
    `<div class="sp-sec" title="The besieged stockpile; developments transferred at HALF tiers.">${r.captor ? "Plunder seized" : "Plunder lost"}</div>` +
    `<div class="sp-line" title="The besieged stockpile changed hands; developments transferred at half tiers."><b>${plunderStr}</b></div>` +
    `<button class="act" data-act="dismiss" data-id="${r.id}" title="Remove the map marker — the report stays in your log.">${icon("aftermath", "sm")} Dismiss marker</button>`;
  $("battle-panel").innerHTML = head + `<div class="pp-body">${body}</div>`;
  activateWorkspacePage("battle-panel");
}


// Apply a shell-neutral click decision through the existing desktop affordances.
// The resolver owns legality and guard order; this wrapper owns DOM presentation.
export function applyMapClickResult(result: MapClickResult): void {
  if (result.kind === "reject") {
    if (result.clearAiming === "jump") clearJumpAiming(true);
    else if (result.clearAiming === "guard") clearGuardAiming(true);
    if (result.clearAiming) updateShipPanel();
    readout().innerHTML = result.reason;
    return;
  }
  if (result.kind === "intent") {
    if (result.clearAiming === "jump") clearJumpAiming(true);
    else if (result.clearAiming === "guard") clearGuardAiming(true);
    if (result.clearAiming) updateShipPanel();
    beginPendingIntent(result.intent);
    if (result.readout) readout().innerHTML = result.readout;
    return;
  }
  if (result.kind !== "select") return;

  const target = result.target;
  switch (target.type) {
    case "fleet":
      selectShip(target.id);
      break;
    case "jumpDeparture":
      selectJumpDeparture(target.key);
      break;
    case "emplacement":
      selectEmplacement(target.id);
      break;
    case "system":
      state.selectedSystemId = target.id;
      openRail("system");
      break;
    case "anchor":
      break;
    case "hub":
      openHubPanel();
      break;
    case "ongoingBattle":
      openOngoingBattlePanel(target.id);
      break;
    case "aftermath":
      deselectShip();
      state.selectedSystemId = null;
      renderer.selectedBattleMarkerId = target.id;
      openBattlePanel(target.id);
      break;
    case "capture":
      deselectShip();
      state.selectedSystemId = null;
      renderer.selectedBattleMarkerId = target.id;
      openCapturePanel(target.id);
      break;
    case "systemBody":
      openPlanetPanel(target.detail);
      break;
    case "clearSystemBody":
      closePlanetPanel();
      break;
  }
  if ("readout" in target && target.readout) readout().innerHTML = target.readout;
}


// Click INSIDE the System View: a planet/moon opens its details; empty space
// clears the selection/panel. No move orders, no raids — those are galaxy-only.
export function handleSystemClick(sx: number, sy: number): void {
  applyMapClickResult(resolveSystemClick(sx, sy, {
    state, renderer, jumpAiming: intentAiming.jump, guardAiming: intentAiming.guard, emplaceArmed: null,
  }));
}


// The map CLICK action runs only on a tap (see installInteraction's click-vs-
// drag gate). Shift and mobile long-press share the explicit ATTACK modifier.
export function handleMapClick(sx: number, sy: number, shift = false, long = false, multi = false): void {
  renderer.selectedBattleMarkerId = null;
  const result = resolveMapClick(sx, sy, { shift, long }, {
    state, renderer, jumpAiming: intentAiming.jump, guardAiming: intentAiming.guard, emplaceArmed: null,
  });
  if (multi && result.kind === "select" && result.target.type === "fleet") {
    const fleetId = result.target.id;
    const fleet = state.ghosts.find((ghost) => ghost.id === fleetId && ghost.own);
    if (fleet) {
      toggleShipSelection(fleet.id);
      return;
    }
  }
  if (result.kind === "intent" && result.intent.verb === "move" && state.selectedShipIds.size > 1) {
    result.intent.shipIds = [...state.selectedShipIds].filter((id) => state.ghosts.some((ghost) => ghost.id === id && ghost.own));
  }
  applyMapClickResult(result);
}


// Wire map interaction: zoom (wheel toward cursor + buttons), pan (left-drag on
// empty space), and the click action — gated so a drag PANS and never fires a
// click (no accidental move orders / raids / selections when panning).
export function installInteraction(): void {
  const canvas = renderer.canvas;
  // The map tracks the pointer for hover affordances (hit outlines etc.).
  canvas.addEventListener("pointermove", (e) => {
    const r = canvas.getBoundingClientRect();
    renderer.cursorWorld = renderer.screenToWorld(e.clientX - r.left, e.clientY - r.top);
    updateMapHover(e.clientX, e.clientY);
  });
  canvas.addEventListener("pointerleave", () => {
    renderer.cursorWorld = null;
    $("map-hover").classList.remove("is-open");
  });
  const DRAG_THRESHOLD = 5; // px of motion that turns a press into a pan
  let down = false, panning = false;
  let startX = 0, startY = 0, lastX = 0, lastY = 0;

  canvas.addEventListener("pointerdown", (e: PointerEvent) => {
    if (e.button !== 0) return; // left button only starts a click/drag
    down = true; panning = false;
    startX = e.clientX; startY = e.clientY; lastX = e.clientX; lastY = e.clientY;
    try { canvas.setPointerCapture(e.pointerId); } catch { /* capture optional */ }
  });
  canvas.addEventListener("pointermove", (e: PointerEvent) => {
    if (!down) return;
    if (!panning && Math.hypot(e.clientX - startX, e.clientY - startY) > DRAG_THRESHOLD) {
      panning = true; // crossed the threshold → this is a pan, not a click
    }
    if (panning) {
      // Pan only the galaxy camera. The System View has a fixed fit camera (no
      // intra-system pan/zoom — zoom-out is an EXIT gesture), so a drag there just
      // suppresses the click.
      if (renderer.viewMode.type === "galaxy" && !renderer.isSystemScrubbing()) {
        renderer.panBy(e.clientX - lastX, e.clientY - lastY);
      }
      lastX = e.clientX; lastY = e.clientY;
    }
  });
  const endPress = (e: PointerEvent) => {
    if (e.pointerType !== "mouse") renderer.cursorWorld = null;
    if (!down) return;
    down = false;
    try { canvas.releasePointerCapture(e.pointerId); } catch { /* not captured */ }
    // A tap (no pan) runs the click action for the ACTIVE scene; a pan suppresses it.
    // Shift+tap on the galaxy map is the ATTACK modifier (destroy vs raid).
    if (!panning && !renderer.isSystemScrubbing()) {
      if (renderer.viewMode.type === "system") handleSystemClick(e.clientX, e.clientY);
      else if (renderer.viewMode.type === "galaxy") handleMapClick(e.clientX, e.clientY, e.shiftKey, false, e.ctrlKey || e.metaKey);
    }
    panning = false;
  };
  canvas.addEventListener("pointerup", endPress);
  canvas.addEventListener("pointercancel", (e: PointerEvent) => {
    if (e.pointerType !== "mouse") renderer.cursorWorld = null;
    down = false;
    panning = false;
  });

  // Mouse wheel zooms toward the cursor. preventDefault stops the page scrolling;
  // over a panel the wheel hits the panel (not the canvas), so panels still scroll.
  // Past max zoom, the wheel directly scrubs the existing semantic transition.
  // Its accumulated target advances in deliberate chunks while the renderer
  // smooths the visible progress, giving mouse wheels and trackpads one gesture.
  const SYSTEM_SCRUB_STEP = 0.18;
  canvas.addEventListener("wheel", (e: WheelEvent) => {
    e.preventDefault();
    if (renderer.viewMode.type === "battle") return; // the overlay owns its zoom-out gesture

    if (renderer.isSystemScrubbing()) {
      renderer.adjustSystemScrub(e.deltaY < 0 ? SYSTEM_SCRUB_STEP : -SYSTEM_SCRUB_STEP);
      return;
    }

    if (renderer.viewMode.type === "system") {
      if (e.deltaY > 0) {
        const mode = renderer.viewMode;
        const sys = mode.type === "system" ? state.galaxy?.systems.find((s) => s.id === mode.systemId) : undefined;
        if (sys && renderer.beginSystemScrubOut(sys)) renderer.adjustSystemScrub(-SYSTEM_SCRUB_STEP);
      }
      return;
    }

    // Galaxy mode: battle keeps its old semantic threshold. At the system max,
    // only a star actually under the zoom anchor can begin the scrubbed handoff.
    const battleHit = renderer.battlePick(e.clientX, e.clientY);
    const wasBattleZoom = renderer.atBattleZoomThreshold();
    const wasMax = renderer.atMaxZoom();
    if (e.deltaY < 0 && battleHit !== null && wasBattleZoom) {
      enterBattleViewer(battleHit);
    } else if (e.deltaY < 0 && wasMax) {
      const sys = systemUnderCursor(e.clientX, e.clientY);
      if (sys && renderer.beginSystemScrubIn(sys, state.systems.find((s) => s.id === sys.id)?.bodies ?? [])) {
        renderer.adjustSystemScrub(SYSTEM_SCRUB_STEP);
      }
    } else {
      renderer.zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY * 0.0016));
    }
  }, { passive: false });

  // Double-click an ongoing battle or star → enter its semantic view. Battle
  // gets first refusal because its fixed marker may sit directly over a system;
  // concluded aftermath markers deliberately have no semantic doorway.
  canvas.addEventListener("dblclick", (e: MouseEvent) => {
    if (renderer.viewMode.type !== "galaxy" || renderer.isSystemScrubbing()) return;
    const battle = renderer.battlePick(e.clientX, e.clientY);
    if (battle !== null) {
      enterBattleViewer(battle);
      return;
    }
    const sys = systemUnderCursor(e.clientX, e.clientY, 16);
    if (sys) enterSystem(sys);
  });

  // Breadcrumb: GALAXY / Back both return from whichever semantic level is open.
  const exitSemantic = () => {
    if (renderer.isSystemScrubbing()) renderer.cancelSystemScrub();
    else if (renderer.viewMode.type === "battle") closeBattleViewer();
    else exitSystem();
  };
  $("bc-galaxy").addEventListener("click", exitSemantic);
  $("bc-back").addEventListener("click", exitSemantic);

  // On-screen zoom controls.
  $("zoom-in").addEventListener("click", () => renderer.zoomByFactor(1.3));
  $("zoom-out").addEventListener("click", () => renderer.viewMode.type === "battle" ? closeBattleViewer() : renderer.zoomByFactor(1 / 1.3));
  $("zoom-reset").addEventListener("click", () => renderer.viewMode.type === "battle" ? closeBattleViewer() : renderer.resetView());

  // Keyboard: Enter/Esc commit or cancel a map-order preview; J arms a selected
  // jump-capable fleet; R recalls the selected raider; M opens the market.
  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement || e.target instanceof HTMLTextAreaElement) return;
    const selShip = state.selectedShipId ? state.ghosts.find((x) => x.id === state.selectedShipId) : undefined;
    if (e.key === "Enter" && state.pendingIntent) {
      e.preventDefault();
      confirmPendingIntent();
    } else if ((e.key === "j" || e.key === "J") && selShip?.own && jumpCapable(selShip)) {
      e.preventDefault();
      armJumpAiming(selShip);
    } else if ((e.key === "r" || e.key === "R") && selShip?.own && net) {
      net.send({ type: "RecallRaid", raider_id: selShip.id });
      delete state.raids[selShip.id]; // break off the intercept estimate
      updateShipPanel();
      readout().innerHTML =
        `Recall away to your raider — travels at light speed. ` +
        `<span class="dim">If it has already made contact, you're commanding into the past.</span>`;
    } else if (e.key === "r" || e.key === "R") {
      toggleResearch(); // §research: the Programme Boards (no own ship selected)
    } else if (e.key === "s" || e.key === "S") {
      toggleRail("system");
    } else if (e.key === "v" || e.key === "V") {
      toggleRail("fleets");
    } else if (e.key === "m" || e.key === "M") {
      toggleMarket(); // hub-wide overlay, not a rail tab
    } else if (e.key === "p" || e.key === "P") {
      toggleRail("officers");
    } else if (e.key === "o" || e.key === "O") {
      toggleRail("logistics");
    } else if (e.key === "u" || e.key === "U") {
      toggleOperations();
    } else if (e.key === "f" || e.key === "F") {
      toggleRail("doctrine");
    } else if (e.key === "g" || e.key === "G") {
      toggleRail("rankings"); // §rankings: the published leaderboard
    } else if (e.key === "l" || e.key === "L") {
      toggleCheckin();
    } else if (e.key === "y" || e.key === "Y") {
      toggleSyndicate(); // §syndicates: alliance panel
    } else if (e.key === "c" || e.key === "C") {
      toggleFaction(); // §TCA: your charter with the Authority
    } else if (e.key === "[") {
      e.preventDefault();
      cycleOwnFleet(-1);
    } else if (e.key === "]") {
      e.preventDefault();
      cycleOwnFleet(1);
    } else if (e.key === "Escape") {
      // A prospective order is the topmost map interaction: cancel it without
      // also closing the selection/panels beneath it.
      if (intentAiming.jump) {
        clearJumpAiming();
        updateShipPanel();
      } else if (intentAiming.guard) {
        clearGuardAiming();
        updateShipPanel();
      } else if (state.pendingIntent) {
        clearPendingIntent();
      } else if (renderer.isSystemScrubbing()) {
        renderer.cancelSystemScrub();
      // §battle-records: the replay overlay is topmost — Escape closes it first.
      } else if ($("battle-viewer").classList.contains("is-open") || renderer.viewMode.type === "battle") {
        closeBattleViewer();
      } else if ($("build-panel").classList.contains("is-open") || $("build-ship-panel").classList.contains("is-open")) {
        closeBuildPanel(); // back out of either builder before the planet panel
      } else if ($("planet-panel").classList.contains("is-open")) {
        closePlanetPanel();
      } else if (renderer.viewMode.type === "system") {
        exitSystem();
      } else if (activeWorkspacePage()) {
        // One Escape = one workspace step. It never collapses the whole deck or
        // clears the map selection underneath the page being dismissed.
        workspaceBack();
      } else {
        // Nothing open: Escape is intentionally a no-op. It never clears an
        // unrelated selection as collateral damage.
      }
    } else if (e.key === "+" || e.key === "=") {
      renderer.zoomByFactor(1.3);
    } else if (e.key === "-" || e.key === "_") {
      if (renderer.viewMode.type === "battle") closeBattleViewer();
      else renderer.zoomByFactor(1 / 1.3);
    } else if (e.key === "ArrowLeft") {
      renderer.panBy(60, 0);
    } else if (e.key === "ArrowRight") {
      renderer.panBy(-60, 0);
    } else if (e.key === "ArrowUp") {
      renderer.panBy(0, 60);
    } else if (e.key === "ArrowDown") {
      renderer.panBy(0, -60);
    }
  });
}


// --- Delayed reports log -----------------------------------------------------
export function addReport(r: import("../../protocol").RaidReport): void {
  const mine = r.you === "attacker" ? r.attacker_kind : r.target_kind; // your ship in this fight
  const theirs = r.you === "attacker" ? r.target_kind : r.attacker_kind;
  let icon = "◦", cls = "good", text = "";
  // Win = your side came out ahead; loss = your ship died.
  const yourShipDied =
    r.outcome === "both_destroyed" ||
    (r.you === "attacker" && r.outcome === "attacker_destroyed") ||
    (r.you === "defender" && r.outcome === "target_destroyed");
  const theirShipDied =
    r.outcome === "both_destroyed" ||
    (r.you === "attacker" && r.outcome === "target_destroyed") ||
    (r.you === "defender" && r.outcome === "attacker_destroyed");
  switch (r.outcome) {
    case "both_destroyed":
      icon = "✺"; cls = "bad"; text = `Your ${mine} and a rival ${theirs} destroyed each other.`; break;
    case "both_survive":
      icon = "≈"; cls = "good";
      text = r.you === "attacker" ? `Your raid on a rival ${theirs} was driven off — both survived.` : `A raider attacked your ${mine} but was driven off.`; break;
    case "escaped":
      icon = "✗"; cls = "good";
      text = r.you === "attacker" ? `Your target ${theirs} reached the hub — raid failed.` : `Your ${mine} reached the hub safely.`; break;
    default:
      if (yourShipDied && theirShipDied) { icon = "✺"; cls = "bad"; text = `Your ${mine} and a rival ${theirs} destroyed each other.`; }
      else if (yourShipDied) { icon = "‼"; cls = "bad"; text = `Your ${mine} was destroyed by a rival ${theirs}.`; }
      else { icon = "✓"; cls = "good"; text = `Your ${mine} destroyed a rival ${theirs}.`; }
  }
  // §pirates: name the neutral faction distinctly — "a pirate ..." instead of
  // "a rival ..." when the aggressor is the pirate faction (the first raid report
  // is how a player DISCOVERS pirates exist).
  const pid = state.galaxy?.pirate_id;
  if (pid && (r.attacker === pid || r.defender === pid)) {
    text = text.split("rival").join("pirate");
  }
  // Per-kind losses (§FLEETS Part 2) — a composition-vs-composition tally.
  const fmtLosses = (l: import("../../protocol").CompCount[]): string =>
    l.filter((c) => c.count > 0).map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ");
  const yours = r.you === "attacker" ? r.attacker_losses : r.target_losses;
  const rivals = r.you === "attacker" ? r.target_losses : r.attacker_losses;
  const yoursStr = fmtLosses(yours ?? []);
  const rivalsStr = fmtLosses(rivals ?? []);
  let lossLine = "";
  if (yoursStr || rivalsStr) {
    lossLine = `<div class="sp-line dim" style="margin-top:2px">You lost: ${yoursStr || "nothing"} · They lost: ${rivalsStr || "nothing"}</div>`;
  }
  const el = addTransientReport(
    icon,
    cls as "good" | "bad",
    `${text} <span class="dim">— delayed news, ${r.age.toFixed(0)}s old</span>${lossLine}`,
  );
  // §battle-aftermath: the news toast and the retained report share an id —
  // clicking the log entry opens the same results panel as the map marker.
  el.dataset.reportId = String(r.report_id);
  el.title = "Open the full battle results";
}


// §FLEETS Part 3: the commit-time STALE-INTEL battle calculator panel. Renders
// the server's projection (computed from YOUR view data) into the report stream —
// projected per-kind losses on both sides, honest about the age of every input
// and about whether the target's makeup was known or a typical-hull estimate.
export function showEngagementEstimate(e: import("../../protocol").EngagementEstimate): void {
  const log = $("reports-log");
  const fmt = (l: import("../../protocol").CompCount[]): string =>
    l.filter((c) => c.count > 0).map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") || "none";
  // §tactical T4: an interquartile band reads "4–7 Corvettes" (or just "5" when tight).
  const fmtBands = (b: import("../../protocol").LossRange[] | null | undefined): string | null => {
    if (!b?.length) return null;
    const parts = b.filter((x) => x.hi > 0)
      .map((x) => `${x.lo === x.hi ? x.lo : `${x.lo}–${x.hi}`} ${shipKindLabel(x.kind)}`);
    return parts.length ? parts.join(", ") : "none";
  };
  const targetDesc = e.target_known
    ? "their exact composition"
    : `est. ${countClassLabel(e.target_count_class)} ships — <b>assuming typical hulls</b>`;
  const ages: string[] = [`their composition: ${e.composition_age.toFixed(0)}s old`];
  ages.push(e.defenses_age != null ? `defenses: scouted ${e.defenses_age.toFixed(0)}s ago` : `defenses: unknown`);
  const el = document.createElement("div");
  el.className = "report good";
  // §tactical T4: lead with the distribution — "68% favorable · expected losses
  // 4–7 Corvettes". Predictive Plots research widens the DISPLAY (their bands,
  // rollout count) — the math underneath is identical either way.
  const plots = state.research?.programmes.find((p) => p.id === "comp_predictive_plots")?.state === "completed";
  const ownBand = fmtBands(e.own_loss_bands);
  let lines: string;
  if (e.win_pct != null && ownBand != null) {
    const pct = Math.round(e.win_pct);
    const tone = pct >= 55 ? "favorable" : pct >= 45 ? "even" : "unfavorable";
    const verdict = `<b>${pct}% ${tone}</b> · expected losses ${esc(ownBand)}`;
    const detail: string[] = [];
    if (plots) {
      const theirBand = fmtBands(e.target_loss_bands);
      if (theirBand) detail.push(`their losses: ${theirBand}`);
      if (e.runs != null) detail.push(`${e.runs} rollouts of the live engine`);
    } else {
      detail.push(`they'd lose: ${fmt(e.target_losses)} (median)`);
    }
    if (e.platform_tiers != null) detail.push(`through a ${e.platform_tiers}-tier platform`);
    lines = `<div class="sp-line dim" style="margin-top:2px">${verdict}</div>` +
      `<div class="sp-line dim">${esc(detail.join(" · "))}</div>`;
  } else {
    // Pre-distribution server: the old median-only readout.
    lines = `<div class="sp-line dim" style="margin-top:2px">You'd lose: ${esc(fmt(e.own_losses))} · They'd lose: ${esc(fmt(e.target_losses))}${e.platform_tiers != null ? ` · through a ${e.platform_tiers}-tier platform` : ""}</div>`;
  }
  el.innerHTML =
    `<span class="ic">⟿</span> <b>Projected raid</b> — ${targetDesc}` +
    lines +
    `<div class="sp-line dim">${esc(ages.join(" · "))} — the real engine, sampled, on stale inputs</div>`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 15000);
}

export function __init_mapchrome_7576(): void {
setHud();
}


// Map help is useful on first contact but should stay out of the way once the
// player knows it. The compact toggle remains on-map, and the preference is a
// purely local presentation choice — it never enters simulation state.
export const LEGEND_COLLAPSED_KEY = "stellar-syndicates.legend-collapsed";

export function setLegendCollapsed(collapsed: boolean): void {
  const legend = $("legend");
  const toggle = $("legend-toggle") as HTMLButtonElement;
  legend.classList.toggle("is-collapsed", collapsed);
  toggle.setAttribute("aria-expanded", String(!collapsed));
  $("legend-toggle-label").textContent = collapsed ? "Show map help" : "Hide map help";
  const chev = toggle.querySelector(".legend-toggle__chev");
  if (chev) chev.textContent = collapsed ? "▸" : "▾";
}

export let legendCollapsed = false;

export function __init_mapchrome_7592(): void {
try {
  legendCollapsed = localStorage.getItem(LEGEND_COLLAPSED_KEY) === "1";
} catch {
  // Storage can be unavailable in a locked-down/private browser; the toggle
  // still works for this page load.
}
}

export function __init_mapchrome_7598(): void {
setLegendCollapsed(legendCollapsed);
}

export function __init_mapchrome_7599(): void {
$("legend-toggle").addEventListener("click", () => {
  legendCollapsed = !legendCollapsed;
  setLegendCollapsed(legendCollapsed);
  try {
    localStorage.setItem(LEGEND_COLLAPSED_KEY, legendCollapsed ? "1" : "0");
  } catch {
    // Presentation preference only; failure to persist is harmless.
  }
});
}
