// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { countClassLabel, formatId, type BattleView, type Commodity, type CompCount, type CountClass, type Deposit, type EngagementPosture, type EntityId, type FleetDoctrine, type GhostView, type PendingOrderView, type ShipKind, type Side, type StandingEndpoint, type StandingOrder, type StandingTrigger, type StockSlot, type SystemInfo, type SystemStateView, type TimelineEntry, type TradeEvent, type Vec2 } from "./protocol";
import { starConceptUrl, starTypeFor } from "./stars";
import type { DevTiers, SystemBodyDetail } from "./systemview";
import { badgeChip, chip, icon, type IconKey, type IconSize } from "./icons";

const state: ViewState = initialState();

// --- DOM handles -----------------------------------------------------------
// Wire protocol version this build speaks — kept in sync with the server's
// PROTOCOL_VERSION (§FLEETS = 2).
const EXPECTED_PROTOCOL_VERSION = 2;
const $ = (id: string) => document.getElementById(id)!;
const joinScreen = $("join");
const joinBtn = $("join-btn") as HTMLButtonElement;
const nameInput = $("name") as HTMLInputElement;
const joinErr = $("join-err");
const hud = $("hud");

function setHud(): void {
  $("hud-name").textContent = state.name || "—";
  $("hud-id").textContent = state.playerId !== null ? formatId(state.playerId) : "—";
  $("hud-tick").textContent = state.link === "online" ? state.tick.toLocaleString() : "—";
  $("hud-time").textContent =
    state.link === "online" ? `${state.simTime.toFixed(1)}s` : "—";
  $("hud-online").textContent = state.link === "online" ? String(state.corpsInView) : "—";
  $("hud-ships").textContent = state.link === "online" ? String(state.ghosts.length) : "—";
  $("hud-credits").textContent = state.wallet ? `${Math.round(state.wallet.credits).toLocaleString()}` : "—";
  $("hud-equity").textContent = state.wallet ? `${Math.round(state.wallet.valuation).toLocaleString()}` : "—";
  const link = $("hud-link");
  const labels: Record<LinkStatus, string> = {
    connecting: "connecting…",
    online: "● online",
    offline: "✕ disconnected",
  };
  link.textContent = labels[state.link];
  link.className = "v " + (state.link === "online" ? "accent" : "warn");
}

// --- §single-click: the PRESS GUARD ------------------------------------------
// Views stream every ~100ms (BROADCAST_EVERY 3 ticks @ 30Hz) and every open
// panel re-renders on each one. A re-render landing MID-PRESS — between
// pointerdown and pointerup, i.e. inside a normal ~100ms human click — destroys
// the pressed button; the browser then retargets the `click` event to the old
// and new targets' common ANCESTOR (the panel root), where the delegated
// `closest("[data-*]")` lookup finds nothing, so the action silently never
// fires. That was the "buttons need a double click" bug (and the unbuildable
// Scout). Structural fix, applied to EVERY per-View-rebuilt panel: while a
// press is down inside a panel, that panel's re-renders are DEFERRED — the
// pressed node survives to pointerup, the click lands normally — and the
// deferred render flushes right after the click dispatches (pointerup fires
// mouseup+click synchronously in the same task; setTimeout(0) runs after).
// Event delegation on the stable panel roots (already the codebase pattern)
// handles the "handler orphaned by innerHTML" half; this guard handles the
// "node destroyed mid-press" half. Presses elsewhere (map pans, other panels)
// defer nothing — each panel is guarded independently.
const pressGuard = { target: null as EventTarget | null, deferred: new Map<string, () => void>() };
window.addEventListener("pointerdown", (e) => { pressGuard.target = e.target; }, true);
function flushPressGuard(): void {
  pressGuard.target = null;
  const fns = [...pressGuard.deferred.values()];
  pressGuard.deferred.clear();
  for (const f of fns) f();
}
window.addEventListener("pointerup", () => setTimeout(flushPressGuard, 0), true);
window.addEventListener("pointercancel", () => setTimeout(flushPressGuard, 0), true);
/// True → a press is currently down inside `rootId`, so the caller must NOT
/// rebuild its DOM now; the render is queued and re-runs after the press.
function renderDeferred(rootId: string, render: () => void): boolean {
  const t = pressGuard.target;
  if (t instanceof Node && $(rootId).contains(t)) {
    pressGuard.deferred.set(rootId, render);
    return true;
  }
  return false;
}

// --- Renderer --------------------------------------------------------------
const renderer = new Renderer();
let rendererReady = false;

// Debug hook (harmless): lets tooling inspect the live view state and transform.
(window as unknown as { __ss: unknown }).__ss = { state, renderer };

async function startRenderer(): Promise<void> {
  if (rendererReady) return;
  await renderer.init($("app"));
  rendererReady = true;
  installInteraction();
  const frame = () => {
    updateSignals();
    renderer.update(state);
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

// Advance the OUTBOUND order signal each frame (the only traveling signal). This
// is the ONLY client-side timing computation: interpolating outbound progress
// between server-provided times. No delay is computed from truth or a client c.
// The signal is dropped once the order reaches the ship — from then on the ship's
// reaction is seen directly on the map (no inbound/response animation).
function updateSignals(): void {
  const estSimNow = state.simTime + (performance.now() - state.lastViewWallMs) / 1000;
  state.commandSignals = state.commandSignals.filter((s) => {
    if (estSimNow >= s.arrive) return false; // order has arrived — the comet is done
    const outSpan = s.arrive - s.depart;
    s.pOut = outSpan > 1e-3 ? (estSimNow - s.depart) / outSpan : 1;
    return true;
  });
}

const readout = () => $("readout");

// --- UI kit (Stellar-Charters-inspired) — string-template helpers every panel
// composes from. Each returns an HTML string; panels assign once via innerHTML and
// wire interaction through ONE delegated listener per root (handler-safe across
// re-renders). Tone is always a class → color-via-CSS-var, so the whole workspace
// themes from index.html's :root tokens. ------------------------------------
const fmt = (n: number) => Math.round(n).toLocaleString();
const esc = (s: string) => s.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]!));
const badge = (tone: string, txt: string) => `<span class="badge badge--${tone}">${esc(txt)}</span>`;
const bar = (pct: number, tone = "") =>
  `<div class="bar"><div class="bar__fill ${tone}" style="width:${Math.max(0, Math.min(100, pct))}%"></div></div>`;
const stat = (label: string, value: string, tone = "") =>
  `<div class="stat"><dt>${esc(label)}</dt><dd class="${tone}">${value}</dd></div>`;
const statStrip = (cells: string[]) => `<div class="stat-strip">${cells.join("")}</div>`;

// Sparkline as inline SVG — no deps. Stroke auto-colors by trend (first vs last).
function spark(data: number[], w = 60, h = 18): string {
  const pts = data.length >= 2 ? data : [data[0] ?? 0, data[0] ?? 0];
  const min = Math.min(...pts), max = Math.max(...pts), span = max - min || 1;
  const stroke = pts[pts.length - 1] >= pts[0] ? "var(--positive)" : "var(--negative)";
  const path = pts
    .map((v, i) => `${((i / (pts.length - 1)) * w).toFixed(1)},${(h - ((v - min) / span) * (h - 2) - 1).toFixed(1)}`)
    .join(" ");
  return `<svg class="spark" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true">` +
    `<polyline fill="none" stroke="${stroke}" stroke-width="1.5" vector-effect="non-scaling-stroke" points="${path}"/></svg>`;
}

// Observed price trend, derived ONLY from the client's own (light-delayed) price
// history — NOT a server "pressure" signal (the server exposes none; fabricating
// one would break the fog model). Dual color+glyph encoding reads without color.
function trend(h: number[]): { glyph: string; tone: string } {
  if (!h || h.length < 4) return { glyph: "▬", tone: "tone-flat" };
  const ref = h[h.length - 4] || 1;
  const pct = (h[h.length - 1] - h[h.length - 4]) / Math.abs(ref);
  if (pct > 0.04) return { glyph: "▲▲", tone: "tone-up" };
  if (pct > 0.004) return { glyph: "▲", tone: "tone-up" };
  if (pct < -0.04) return { glyph: "▼▼", tone: "tone-down" };
  if (pct < -0.004) return { glyph: "▼", tone: "tone-down" };
  return { glyph: "▬", tone: "tone-flat" };
}

// currentColor line icons (recolor for free via the parent's `color`).
// Mirror of the sim's commodity value-rank (also in render.ts) — for flavor text
// and dominant-resource selection. Client-only; no server data.
const COMMODITY_VALUE: Record<Commodity, number> = {
  biomass: 5, silicates: 6, metallic_ore: 8, volatiles: 9, rare_elements: 22,
  provisions: 9, fuel: 14, polymers: 16, alloys: 26, electronics: 34,
  machinery: 48, armaments: 56,
};

// Mirror of the sim's fuel-cost model (crates/sim/src/fuel.rs + ship.rs) — so the
// own-ship panel can show this ship's fuel burn rate honestly. Movement burns
// FUEL_PER_MASS_DISTANCE × distance × mass, mass = hull + cargoUnits·CARGO_MASS.
const FUEL_PER_MASS_DISTANCE = 1.0e-6;
const HULL_MASS: Record<ShipKind, number> = { convoy: 4500, raider: 200, corvette: 800, colony: 6000, scout: 80 };
const CARGO_MASS_PER_UNIT = 28;
const shipMass = (g: GhostView) =>
  HULL_MASS[g.kind] + (g.own && g.cargo ? g.cargo.units * CARGO_MASS_PER_UNIT : 0);

// The native Stellar Syndicates icon set (/art/ui_icons/svg) — full-color SVG,
// crisp at any size, used as <img>. Resources / Actions / Concepts / Status. This
// SUPERSEDES the earlier Stellar-Charters borrow. No loading="lazy" — these panels
// re-render ~10 Hz, recreating the <img>; lazy would replace them before the
// observer fires. Eager hits the browser cache instantly.
const uiIcon = (slug: string, size: IconSize = "sm", cls = "") =>
  `<img class="icon icon--${size}${cls ? ` ${cls}` : ""}" src="/art/ui_icons/svg/${slug}.svg" alt="" />`;

// A commodity icon is by definition a resource, so it always uses the dedicated
// `--icon-resource` token + the downscaled PNG art (each commodity now has its own,
// including Volatiles — no more hue-shifted Fuel stand-in). `size` kept for symmetry.
// §economy: the ORIGINAL five have dedicated PNG art (metallic_ore reuses the
// old ore art); the seven new industrial goods fall back to tinted glyphs until
// their art lands.
const COMMODITY_ART: Partial<Record<Commodity, string>> = {
  fuel: "fuel", metallic_ore: "ore", alloys: "alloys", provisions: "provisions", volatiles: "volatiles",
};
const COMMODITY_GLYPH: Record<Commodity, string> = {
  metallic_ore: "\u26cf", rare_elements: "\u2728", silicates: "\u25a6", volatiles: "\u2744", biomass: "\ud83c\udf3f",
  alloys: "\ud83d\udd29", electronics: "\ud83d\udda5", polymers: "\ud83e\uddea", fuel: "\u26fd", provisions: "\ud83c\udf5e",
  machinery: "\u2699", armaments: "\ud83d\udd2b",
};
const commodityIcon = (c: Commodity, _size: IconSize = "md") => {
  const art = COMMODITY_ART[c];
  return art
    ? `<img class="icon icon--resource" src="/art/ui_icons/resource/${art}.png" alt="" title="${c.replace("_", " ")}" />`
    : `<span class="icon icon--resource icon--glyph" title="${c.replace("_", " ")}">${COMMODITY_GLYPH[c]}</span>`;
};

// Status icon by timeline severity (the native Status set).
const STATUS_SLUG: Record<TimelineEntry["severity"], string> = {
  good: "status-success",
  bad: "status-warning-threat",
  warn: "status-warning-threat",
  info: "status-info",
};
const statusIcon = (sev: TimelineEntry["severity"], size: IconSize = "sm") => uiIcon(STATUS_SLUG[sev], size);

// --- Workspace rail: one right-docked column hosting System/Market/Logistics/
// Doctrine as a tab stack. Opening any tab opens the rail; one tab shows at a
// time; ✕ / Esc closes it → the map stays uncluttered. ----------------------
// The right rail hosts only the SELECTION/holdings-context tabs. The Market is a
// hub-wide institution → it lives in the TOP NAVBAR as its own overlay, not here.
type RailTab = "system" | "logistics" | "doctrine" | "rankings";
let railTab: RailTab = "system";
let railBuilt = false;

function setRailTab(tab: RailTab): void {
  railTab = tab;
  const bodyId: Record<RailTab, string> = { system: "tab-system", logistics: "standing", doctrine: "doctrine", rankings: "tab-rankings" };
  for (const t of ["system", "logistics", "doctrine", "rankings"] as RailTab[]) {
    $(bodyId[t]).classList.toggle("is-active", t === tab);
  }
  document.querySelectorAll<HTMLElement>("#rail-tabs button").forEach((b) => {
    b.classList.toggle("is-active", b.dataset.tab === tab);
  });
  // Render the shown tab once on switch (each tab then refreshes per-View only
  // while it's the visible one — see the View handler — so hidden tabs don't churn).
  if (tab === "system") updateSystemTab();
  else if (tab === "logistics") updateStandingPanel();
  else if (tab === "doctrine") updateDoctrinePanel();
  else if (tab === "rankings") updateRankingsPanel();
}
function openRail(tab: RailTab): void {
  deselectShip(); // the rail and the ship panel share the right-dock slot
  $("rail").classList.add("is-open");
  setRailTab(tab);
}
function closeRail(): void {
  $("rail").classList.remove("is-open");
}
function toggleRail(tab: RailTab): void {
  const open = $("rail").classList.contains("is-open");
  if (open && railTab === tab) closeRail();
  else openRail(tab);
}
function buildRail(): void {
  if (railBuilt) return;
  railBuilt = true;
  $("rail-close").addEventListener("click", closeRail);
  $("rail-tabs").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("button");
    if (b?.dataset.tab) setRailTab(b.dataset.tab as RailTab);
  });
  // §rankings: pick the sort category (chips live inside the re-rendered body, so
  // delegate off the STABLE tab container).
  $("tab-rankings").addEventListener("click", (e) => {
    const c = (e.target as HTMLElement).closest<HTMLElement>("[data-rankcat]");
    if (c?.dataset.rankcat) {
      rankingsSortCat = c.dataset.rankcat;
      updateRankingsPanel();
    }
  });
  // Top-navbar destinations (hub-wide, system-independent): Market + Syndicate + Log.
  $("nav-market").addEventListener("click", toggleMarket);
  $("nav-syndicate").addEventListener("click", toggleSyndicate);
  $("nav-log").addEventListener("click", toggleCheckin);
  $("market-close").addEventListener("click", closeMarket);
  // §syndicates: delegated actions inside the alliance panel.
  $("syndicate-panel").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("button");
    if (!b) return;
    const act = b.dataset.sy;
    if (act === "close") { closeSyndicate(); return; }
    if (!net) return;
    if (act === "create") {
      const name = ($("sy-create-name") as HTMLInputElement | null)?.value.trim() || "Syndicate";
      net.send({ type: "CreateSyndicate", name });
    } else if (act === "invite") {
      const name = ($("sy-invite-name") as HTMLInputElement | null)?.value.trim();
      if (name) { net.send({ type: "InviteToSyndicate", name }); ($("sy-invite-name") as HTMLInputElement).value = ""; }
    } else if (act === "accept") {
      const sid = b.dataset.sid;
      if (sid) net.send({ type: "AcceptSyndicateInvite", syndicate_id: sid });
    } else if (act === "leave") {
      net.send({ type: "LeaveSyndicate" });
    } else if (act === "dissolve") {
      net.send({ type: "DissolveSyndicate" });
    }
  });
}

// --- Ship details panel — a FOG-AWARE master→detail card for the SELECTED ship.
// It shares the right-dock slot with the rail (mutually exclusive: selecting a ship
// closes the rail and clears any system selection; opening the rail deselects the
// ship). Re-renders each View so the information AGE keeps ticking. Strictly a UI
// layer over GhostView — it shows ONLY what the per-player view already reveals, so
// a rival's cargo/route/internal state never leaks. ------------------------------
let shipPanelBuilt = false;
function buildShipPanel(): void {
  if (shipPanelBuilt) return;
  shipPanelBuilt = true;
  // One delegated listener survives the per-View innerHTML rewrites.
  $("ship-panel").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-act]");
    if (!b) return;
    const act = (b as HTMLElement).dataset.act;
    if (act === "close") {
      deselectShip();
    } else if (act === "recall" && state.selectedShipId && net) {
      net.send({ type: "RecallRaid", raider_id: state.selectedShipId });
      delete state.raids[state.selectedShipId]; // break off the intercept estimate
      updateShipPanel();
    } else if (act === "withdraw" && state.selectedShipId && net) {
      // §battles-take-time: light-delayed break-off; the echo lifecycle tracks it.
      net.send({ type: "Withdraw", fleet_id: state.selectedShipId });
      updateShipPanel();
    } else if (act === "split" && state.selectedShipId && net) {
      const kind = (b as HTMLElement).dataset.kind as ShipKind | undefined;
      if (kind) {
        net.send({ type: "SplitFleet", fleet_id: state.selectedShipId, counts: { [kind]: 1 } });
      }
    } else if (act === "merge" && state.selectedShipId && net) {
      const from = (b as HTMLElement).dataset.from;
      if (from) {
        net.send({ type: "MergeFleets", into: state.selectedShipId, from });
      }
    } else if (act === "transit" && state.selectedShipId && net) {
      const mode = (b as HTMLElement).dataset.mode as "full" | "stealth" | undefined;
      if (mode) {
        transitModes.set(state.selectedShipId, mode);
        net.send({ type: "SetFleetTransit", fleet_id: state.selectedShipId, mode });
        updateShipPanel();
      }
    } else if (act === "posture" && state.selectedShipId && net) {
      const posture = (b as HTMLElement).dataset.mode as EngagementPosture | undefined;
      if (posture) {
        postureModes.set(state.selectedShipId, posture);
        net.send({ type: "SetFleetPosture", fleet_id: state.selectedShipId, posture });
        updateShipPanel();
      }
    }
  });
}
function selectShip(id: string): void {
  state.selectedShipId = id;
  state.selectedSystemId = null; // a ship and a system are never both selected
  closeRail(); // the ship panel and rail share the right-dock slot
  $("ship-panel").classList.add("is-open");
  buildShipPanel();
  updateShipPanel();
}
function deselectShip(): void {
  state.selectedShipId = null;
  $("ship-panel").classList.remove("is-open");
}

const shipKindLabel = (k: ShipKind): string => (k === "convoy" ? "Convoy" : k === "raider" ? "Raider" : k === "corvette" ? "Corvette" : k === "colony" ? "Colony Ship" : k === "scout" ? "Scout" : k);

// --- §order-lifecycle: IN TRANSIT → AWAITING ECHO → CONFIRMED ----------------
// Below this, phases collapse to ~instant (a fleet near the command center) —
// suppress the noisy sub-second states.
const LIFECYCLE_MIN_S = 1.5;
// After a lifecycle drops (confirmed), flash "✓ confirmed" for a moment.
const confirmedFlashUntil = new Map<string, number>();

// Live sim-time, extrapolated from the last View's wall-clock stamp so the
// countdowns tick smoothly between messages.
function liveSimTime(): number {
  return state.simTime + (performance.now() - state.lastViewWallMs) / 1000;
}

// Replace the tracked lifecycles from the View; a lifecycle that DROPS while its
// fleet is still visible has just CONFIRMED — flash it briefly.
function syncOrderLifecycles(list: PendingOrderView[], _simTime: number): void {
  const next = new Map<string, PendingOrderView>();
  for (const p of list) next.set(p.fleet_id, p);
  for (const [fid] of state.pendingOrders) {
    if (!next.has(fid) && state.ghosts.some((g) => g.id === fid && g.own)) {
      confirmedFlashUntil.set(fid, performance.now() + 5000);
    }
  }
  state.pendingOrders = next;
}

const fmtCountdown = (secs: number): string => {
  const s = Math.max(0, Math.round(secs));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

// §battles-take-time: notify ONCE when a battle first becomes visible (light-
// gated by the server). Keyed by a coarse location so it re-fires only for a
// genuinely new battle after an old one ends.
const seenBattles = new Set<string>();
function notifyNewBattles(battles: import("./protocol").BattleView[]): void {
  const nowKeys = new Set<string>();
  for (const b of battles) {
    const key = `${Math.round(b.pos.x / 200)},${Math.round(b.pos.y / 200)}`;
    nowKeys.add(key);
    if (!seenBattles.has(key)) {
      const log = $("reports-log");
      const el = document.createElement("div");
      el.className = "report bad";
      el.innerHTML = `<span class="ic">⚔</span> <b>Battle raging</b> near (${b.pos.x.toFixed(0)}, ${b.pos.y.toFixed(0)}) <span class="dim">— as of ${fmtCountdown(b.age)} ago${b.own ? " · your fleet is engaged" : ""}</span>`;
      log.prepend(el);
      while (log.children.length > 6) log.removeChild(log.lastChild!);
      setTimeout(() => el.classList.add("fade"), 12000);
    }
  }
  seenBattles.clear();
  for (const k of nowKeys) seenBattles.add(k);
}

// The order-lifecycle status line for the fleet panel (the star).
function orderLifecycleLine(g: GhostView): string {
  const p = state.pendingOrders.get(g.id);
  if (!p) {
    const exp = confirmedFlashUntil.get(g.id);
    if (exp && performance.now() < exp) {
      return `<div class="sp-sec">Order</div><div class="sp-line">${badgeChip("confirmed", "confirmed", "positive", "Confirmed — the echo light has returned; you can see the fleet complying.")}</div>`;
    }
    return "";
  }
  // Near-zero (fleet at the command center): don't flash noisy sub-second states.
  if (p.echo_at - p.delivered_at < LIFECYCLE_MIN_S) return "";
  const now = liveSimTime();
  const line = now < p.delivered_at
    ? chip("delivered", fmtCountdown(p.delivered_at - now), `IN TRANSIT — your ${p.kind} order is crossing space; it reaches the fleet in ${fmtCountdown(p.delivered_at - now)}.`)
    : chip("echo", fmtCountdown(p.echo_at - now), `DELIVERED — awaiting echo: the fleet has your ${p.kind} order, but the light showing it comply hasn't returned yet.`);
  return `<div class="sp-sec">Order</div><div class="sp-line">${line}</div>`;
}

// §Part 4: the player's chosen transit throttle per own fleet (optimistic —
// echoes the SetFleetTransit command; defaults to Full).
const transitModes = new Map<string, "full" | "stealth">();

// The Transit control (§Part 4) — Full/Stealth toggle for an own DARK fleet
// (only dark fleets benefit from running quiet; broadcasters are seen anyway).
function transitSection(g: GhostView): string {
  // Only meaningful for a fleet that can run dark (no broadcasting member) — i.e.
  // its flagship is a raider or scout.
  if (g.kind !== "raider" && g.kind !== "scout") return "";
  const mode = transitModes.get(g.id) ?? "full";
  const btn = (m: "full" | "stealth", label: string, hint: string) =>
    `<button class="act${mode === m ? " is-on" : ""}" data-act="transit" data-mode="${m}" title="${esc(hint)}">${label}</button>`;
  // Short labels; the trade-off lives in the button tooltips (§UX-diet).
  return `<div class="sp-sec">${icon(mode === "stealth" ? "stealth" : "flank", "sm")} Transit</div>` +
    `<div class="sp-line">${btn("full", "Full", "Full speed — fastest, but flank speed lights you up (high sensor signature).")} ${btn("stealth", "Stealth", "Stealth — creep at ~half speed (about 2× trip time) for a much smaller signature.")}</div>`;
}

// §offensive-orders Part 2: the player's chosen engagement POSTURE per own fleet
// (optimistic — echoes SetFleetPosture; falls back to the View's owner-only value).
const postureModes = new Map<string, EngagementPosture>();
const POSTURE_META: { key: EngagementPosture; label: string; hint: string }[] = [
  { key: "passive", label: "Passive", hint: "Fight only if engaged — take no autonomous offensive action (default)." },
  { key: "defensive", label: "Defensive", hint: "Defend a guarded asset / station (picket behaviour); no proactive hunting." },
  { key: "weapons_free", label: "Weapons-free", hint: "Auto-attack any rival that enters this fleet's OWN sensor bubble — on its own local detection, no command-center round trip. A lone convoy is raided, anything armed is destroyed; still gated by your corp doctrine's odds." },
];

// The POSTURE control — standing per-fleet aggression, for a strike-capable fleet
// (a raider aboard). Composes with the corp doctrine (which decides the odds).
function postureSection(g: GhostView): string {
  if (!g.composition?.some((c) => c.kind === "raider")) return ""; // needs strike capability
  const cur = postureModes.get(g.id) ?? g.posture ?? "passive";
  const btn = (m: EngagementPosture, label: string, hint: string) =>
    `<button class="act${cur === m ? " is-on" : ""}" data-act="posture" data-mode="${m}" title="${esc(hint)}">${esc(label)}</button>`;
  // Short labels; each posture's full description is its button tooltip (§UX-diet).
  return `<div class="sp-sec">${icon("posture", "sm")} Posture</div><div class="sp-line">${POSTURE_META.map((p) => btn(p.key, p.label, p.hint)).join(" ")}</div>`;
}

// Flagship precedence (drawn/named order) — also the composition display order.
const FLAGSHIP_ORDER: ShipKind[] = ["colony", "convoy", "corvette", "raider", "scout"];

// The COMPOSITION section of the fleet panel — mirrors the §13.1 intel ladder:
// full composition for own fleets and rivals inside sensor coverage; a bucket-only
// estimate ("est. 4–7 ships — composition unknown") outside coverage.
function compositionSection(g: GhostView): string {
  if (g.composition && g.composition.length) {
    const items = [...g.composition]
      .sort((a, b) => FLAGSHIP_ORDER.indexOf(a.kind) - FLAGSHIP_ORDER.indexOf(b.kind))
      .map((c) => `${esc(shipKindLabel(c.kind))} <b>×${c.count}</b>`)
      .join(" · ");
    const total = g.composition.reduce((a, c) => a + c.count, 0);
    return `<div class="sp-sec">Composition</div><div class="sp-line">${items} <span class="dim">(${total} ship${total > 1 ? "s" : ""})</span></div>`;
  }
  return `<div class="sp-sec">Composition</div><div class="sp-line dim">${icon("unknown", "sm", "Composition unknown — this fleet is out of your sensor range, so you have only the size estimate, never the exact makeup.")} est. <b>${countClassLabel(g.count_class)}</b> ships</div>`;
}

// Another of your OWN fleets co-located with `g` (within the claim radius) — the
// merge candidate. Composition/merge is done at a berth; the server enforces the
// "at an owned system, idle" rule and soft-rejects otherwise.
const MERGE_COLOCATE_RADIUS = 80; // matches COLONY_CLAIM_RADIUS on the server
function coLocatedOwnFleet(g: GhostView): GhostView | null {
  let best: GhostView | null = null;
  let bestD = MERGE_COLOCATE_RADIUS;
  for (const o of state.ghosts) {
    if (!o.own || o.id === g.id) continue;
    const d = Math.hypot(o.pos.x - g.pos.x, o.pos.y - g.pos.y);
    if (d <= bestD) {
      best = o;
      bestD = d;
    }
  }
  return best;
}

// Fleet-management controls (§FLEETS v1): split off a ship, or merge a co-located
// fleet. Only meaningful for your own fleet at an owned berth — offered by the
// client, enforced (idle + owned system) by the server.
function fleetManagementSection(g: GhostView): string {
  const parts: string[] = [];
  const comp = g.composition ?? [];
  const total = comp.reduce((a, c) => a + c.count, 0);
  const merge = coLocatedOwnFleet(g);
  if (total < 2 && !merge) return "";
  parts.push(`<div class="sp-sec">Fleet management</div>`);
  if (total >= 2) {
    const splitBtns = [...comp]
      .sort((a, b) => FLAGSHIP_ORDER.indexOf(a.kind) - FLAGSHIP_ORDER.indexOf(b.kind))
      .filter((c) => c.count >= 1)
      .map((c) => `<button class="act" data-act="split" data-kind="${c.kind}" title="Detach one ${esc(shipKindLabel(c.kind))} into a new fleet (at an owned system)">Split 1 ${esc(shipKindLabel(c.kind))}</button>`)
      .join("");
    parts.push(`<div class="sp-line">${splitBtns}</div>`);
  }
  if (merge) {
    parts.push(`<button class="act" data-act="merge" data-from="${merge.id}" title="Merge the co-located fleet into this one — works only at one of your owned systems (idle).">${icon("fleet", "sm")} Merge co-located fleet</button>`);
  }
  return parts.join("");
}

// Heading arrow + speed, computed in SCREEN space so it matches the map exactly.
function headingCell(g: GhostView): string {
  const sp = Math.hypot(g.vel.x, g.vel.y);
  if (sp < 0.5) return stat("Heading", `<span class="dim">stationary</span>`);
  const p0 = renderer.worldToScreen(g.pos);
  const p1 = renderer.worldToScreen({ x: g.pos.x + g.vel.x, y: g.pos.y + g.vel.y });
  const deg = (Math.atan2(p1.y - p0.y, p1.x - p0.x) * 180) / Math.PI;
  return stat("Heading", `<span class="sp-arrow" aria-hidden="true" style="transform:rotate(${deg.toFixed(0)}deg)">➤</span> ${sp.toFixed(0)} su/s`);
}

// Inferred activity for an OWN ship — there is NO server order field, so this reads
// purely from the client's own overlays (raids/orders/command signals/route/vel).
function ownActivity(g: GhostView): string {
  const a = (key: IconKey, label: string, tip: string) => `${icon(key, "sm", tip)} <b>${label}</b>`;
  if (state.commandSignals.some((s) => s.shipId === g.id)) return a("delivered", "order in transit", "Your command is still crossing space to this fleet.");
  if (state.raids[g.id]) return a("raid", "raiding", "Pursuing a rival contact. Press R to recall (break off).");
  if (state.orders[g.id]) return a("move", "en route", "Proceeding on your last move order.");
  if (g.route && g.route.length) return a("convoy", "hauling", "En route along its trade route.");
  if (Math.hypot(g.vel.x, g.vel.y) < 0.5) return `<span class="dim">holding station</span>`;
  return a("move", "under way", "Under way.");
}

// OWN ship: full knowledge — activity, cargo + route (you always know your own),
// the shared FLEET fuel reserve, and the relevant actions.
function ownBody(g: GhostView): string {
  const parts: string[] = [];
  parts.push(compositionSection(g));
  parts.push(orderLifecycleLine(g));
  parts.push(`<div class="sp-sec">Activity</div><div class="sp-line">${ownActivity(g)}</div>`);

  if (g.kind === "convoy") {
    const cargo = g.cargo
      ? `<div class="sp-cargo">${commodityIcon(g.cargo.commodity, "md")} <b>${fmt(g.cargo.units)}</b> ${esc(g.cargo.commodity)}</div>`
      : `<span class="dim">empty hold</span>`;
    parts.push(`<div class="sp-sec">Cargo</div>${cargo}`);
    if (g.route && g.route.length) {
      const d = g.route[g.route.length - 1];
      parts.push(`<div class="sp-sec">Route</div><div class="sp-line" title="The waypoints this convoy will fly; the last is its destination.">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → (${d.x.toFixed(0)}, ${d.y.toFixed(0)})</div>`);
    }
  }

  // Fleet fuel reserve (corp-wide, shared across ALL your ships) + this ship's burn
  // rate. Framed honestly: it's the operating reserve every ship spends, not a tank
  // on this one ship. (See the per-ship deepening note in the README.)
  const reserve = state.wallet ? state.wallet.fuel_total : 0;
  const rate = FUEL_PER_MASS_DISTANCE * 1000 * shipMass(g);
  const dest = state.orders[g.id];
  let burn = `~${rate.toFixed(1)}/1k su`;
  if (dest) {
    const cost = FUEL_PER_MASS_DISTANCE * Math.hypot(dest.x - g.pos.x, dest.y - g.pos.y) * shipMass(g);
    burn = `order ~${fmt(cost)} · ${rate.toFixed(1)}/1k su`;
  }
  parts.push(
    `<div class="sp-sec">${icon("fuel", "sm")} Fuel</div>` +
    `<div class="sp-line">${chip("fuel", fmt(reserve), "Fleet fuel reserve — one pool shared across ALL your systems; every ship draws on it to move (not a tank on this one ship).")} <span class="dim" title="This ship's fuel burn at its current mass (and the cost of its current order, if any).">${burn}</span></div>`,
  );

  // Role summaries: a one-liner on screen, the full doctrine in the tooltip.
  if (g.kind === "colony") {
    parts.push(`<div class="sp-sec">${icon("colony", "sm")} Settlement</div><div class="sp-line" title="Colonists + infrastructure. Send it to an unclaimed system: on arrival the system becomes yours and the ship is consumed (it becomes the colony). It broadcasts its voyage — slow, visible, raidable — so escort it. If someone claims the target first, it holds there intact; redirect it.">Send to an <b>unclaimed system</b> → it becomes yours (ship consumed). Slow &amp; visible — escort it.</div>`);
  }
  if (g.kind === "corvette") {
    parts.push(`<div class="sp-sec">${icon("corvette", "sm")} Escort · Garrison</div><div class="sp-line" title="A dedicated defender: any raid contact on one of your convoys within its protect radius must fight THROUGH this corvette first. Park it beside a convoy (escort) or at an owned system (garrison — stacks with a Defense Platform). It cannot raid.">Defends convoys (escort) &amp; systems (garrison). Can't raid.</div>`);
  }
  if (g.kind === "scout") {
    const mult = state.galaxy?.scout_sensor_mult ?? 1.5;
    parts.push(`<div class="sp-sec">${icon("sensor", "sm")} Sensors</div><div class="sp-line" title="Projects a ×${mult} sensor bubble — mobile vision. Sweep it through rival space to reveal dark contacts and convoy cargo; near a rival system it captures an intel snapshot of their defenses. No cargo, no weapons: if anything engages it, it dies — cheap on purpose.">${chip("sensorRange", `×${mult}`, "Mobile sensor bubble — sweep rival space for dark contacts, cargo &amp; defense intel.")} — dies if engaged (no weapons).</div>`);
    // §explore Part 2: the scout's SECOND job — click-to-survey (blockade idiom).
    parts.push(`<div class="sp-line dim" title="Click an UNSURVEYED system (its geology shows '?') to order a survey: the scout flies on-site and dwells ~${SURVEY_SECS_UI}s — active sensing is LOUD (detected farther) — then the exact geology travels home at light speed. Allies receive a relayed copy. Already-surveyed systems select normally.">${icon("intel", "sm")} <b>Survey:</b> click an unsurveyed system.</div>`);
  }
  parts.push(`<div class="sp-sec">${icon("build", "sm", "Actions")} Actions</div>`);
  // §battles-take-time: WITHDRAW when this fleet is in/near a visible battle.
  const inBattle = state.battles.some((b) => Math.hypot(b.pos.x - g.pos.x, b.pos.y - g.pos.y) <= 220);
  if (inBattle && (g.kind === "raider" || g.kind === "corvette")) {
    parts.push(`<button class="act" data-act="withdraw" title="Break off and flee home — light-delayed; your formation speed decides the escape (escorts cover you).">${icon("withdraw", "sm")} Withdraw</button>`);
  }
  if (g.kind === "raider") {
    parts.push(`<button class="act" data-act="recall" title="Recall to home (R) — travels at light speed; it may arrive too late.">${icon("recall", "sm")} Recall</button>`);
  }
  const strike = !!g.composition?.some((c) => c.kind === "raider");
  // Compact glyph legend replacing the how-to-click prose — words live in tooltips.
  const legend: string[] = [`${icon("mouse", "sm", "Click empty space on the map to MOVE this fleet.")} move`];
  if (g.kind === "raider") legend.push(`${icon("raid", "sm", "Click a rival contact on the map to RAID it — seize its cargo (brevity-capped skirmish).")} raid`);
  if (strike) legend.push(`${icon("shift", "sm", "Shift+click a rival to ATTACK — a full battle that destroys it (a convoy's cargo is lost with it).")} ${icon("attack", "sm")} attack`);
  if (g.kind === "raider") legend.push(`${icon("blockade", "sm", "Click a rival SYSTEM to blockade it — take station and strangle its logistics; its defenses fight you first.")} blockade`);
  parts.push(`<div class="sp-line dim sp-legend">${legend.join(" · ")}</div>`);
  parts.push(transitSection(g));
  parts.push(postureSection(g));
  parts.push(garrisonSection(g));
  parts.push(fleetManagementSection(g));
  return parts.join("");
}

// §syndicates Part 3: if this OWN fleet is stationed as an ally GARRISON, show its
// host + supply state (fed = the host is covering its Provisions upkeep; UNFED =
// its defense is suspended until fed — nothing destroyed).
function garrisonSection(g: GhostView): string {
  if (!g.own || !g.garrison_host) return "";
  const hostName = systemName(g.garrison_host);
  const fed = g.garrison_fed !== false;
  const tip = fed
    ? `Stationed at ally ${hostName}, joining its defense per your doctrine. The host is feeding this garrison its Provisions upkeep.`
    : `Stationed at ally ${hostName}, but the host is OUT of Provisions — this garrison's defense is SUSPENDED until fed (nothing is destroyed).`;
  return `<div class="sp-sec">${icon("garrison", "sm")} Garrison</div><div class="sp-line">` +
    `${badgeChip("garrison", `${hostName} · ${fed ? "fed" : "UNFED"}`, fed ? "positive" : "negative", tip)}</div>`;
}

// RIVAL ship: ONLY what's observable. A convoy broadcasts its route (light-delayed)
// and reveals cargo ONLY when inside your sensor coverage (cargo present). A raider
// runs dark. Never any order/intent/fuel/internal state.
function rivalBody(g: GhostView): string {
  const parts: string[] = [];
  parts.push(compositionSection(g));
  if (g.kind === "convoy") {
    if (g.route && g.route.length) {
      const d = g.route[g.route.length - 1];
      parts.push(`<div class="sp-sec">Route</div><div class="sp-line" title="A convoy broadcasts its route under the Convention — light-delayed, like everything you see.">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → (${d.x.toFixed(0)}, ${d.y.toFixed(0)}) <span class="dim">(broadcast)</span></div>`);
    }
    // Cargo ONLY when in sensor range (cargo present). NEVER shown otherwise.
    parts.push(`<div class="sp-sec">${icon("cargo", "sm")} Cargo</div>` + (g.cargo
      ? `<div class="sp-line">${chip(g.cargo.commodity as IconKey, `${fmt(g.cargo.units)} ${esc(g.cargo.commodity)}`, "Cargo — visible because this convoy is inside your sensor coverage.")}</div>`
      : `<div class="sp-line dim">${icon("unknown", "sm", "Cargo unknown — this convoy is out of your sensor range. It is revealed only inside your coverage.")} unknown</div>`));
  } else {
    const tip = g.kind === "scout"
      ? "A scout runs silent — someone is LOOKING at your space. No cargo, no weapons. You see it only because it is within your sensor range right now."
      : "A raider runs silent — no route or cargo is observable. You see it only because it is within your sensor range right now.";
    parts.push(`<div class="sp-sec">${icon("stealth", "sm")} Dark contact</div><div class="sp-line dim" title="${esc(tip)}">${g.kind === "scout" ? "scout" : "raider"} — in sensor range</div>`);
    // §Part 4: how LOUD it is (signature) — a big pack at flank speed flares far out.
    if (g.signature != null) {
      const loud = g.signature >= 1.6 ? "running LOUD — flank speed and/or a big pack (flares far out)"
        : g.signature <= 0.6 ? "running quiet — creeping or small (you caught it close)"
        : "a moderate signature";
      parts.push(`<div class="sp-line">${chip("delay", `${g.signature.toFixed(2)}×`, `Detection signature — how LOUD this contact is: ${loud}.`)}</div>`);
    }
  }
  parts.push(`<div class="sp-sec">${icon("build", "sm", "Actions")} Actions</div><div class="sp-line dim sp-legend">${icon("raid", "sm", "Click this contact on the map to commit a RAID with your selected raider.")} raid · ${icon("shift", "sm", "Shift+click to ATTACK — a full battle that destroys it (needs a raider in your selected fleet).")} ${icon("attack", "sm")} attack</div>`);
  return parts.join("");
}

function updateShipPanel(): void {
  if (renderDeferred("ship-panel", updateShipPanel)) return; // §single-click
  if (!state.selectedShipId) return;
  const root = $("ship-panel");
  const g = state.ghosts.find((x) => x.id === state.selectedShipId);
  if (!g) {
    // No longer observable (passed beyond your sensors/light, or — a rival —
    // destroyed). Honest: we can't show what we can't see.
    root.innerHTML =
      `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">contact</div><h2>Contact lost</h2></div></div>` +
      `<button class="sp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>` +
      `<div class="sp-body"><div class="sp-note" title="It has passed beyond your sensors and the last light to reach you — nothing more is observable.">Passed beyond your sensors.</div></div>`;
    return;
  }
  const own = g.own;
  const eyebrow = own ? "your fleet" : g.kind === "raider" ? "dark contact" : "rival contact";
  const ownTag = own ? badge("accent", "yours") : badge("negative", "rival");
  const stale = g.age >= 8;

  const head =
    `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h2>${uiIcon(g.kind === "convoy" ? "concept-convoy" : "concept-fleet", "md")} ${esc(shipKindLabel(g.kind))}</h2></div><div class="panel-title__right">${ownTag}</div></div>` +
    `<button class="sp-close" data-act="close" title="Deselect (Esc)" aria-label="Deselect">✕</button></div>`;

  // Information AGE is the headline stat (the game's identity: you always know HOW
  // OLD this sighting is).
  const ageCell = `<div class="stat sp-age ${stale ? "is-stale" : ""}"><dt>Seen</dt><dd>${g.age.toFixed(1)}s ago</dd></div>`;
  // Positional certainty follows the SAME light-delay model for own AND rival ships:
  // there is no FTL tether to your own fleet — uncertainty = age × max_speed for every
  // object (server view.rs / protocol GhostView). So read it HONESTLY off g.uncertainty
  // and never grant your own ships false certainty (a distant own ship is as uncertain
  // as a rival). A ship at your command center has ~0 lag → "confirmed".
  const certain = g.uncertainty < 1;
  // The uncertainty explanation rides the Position stat as a tooltip (§UX-diet).
  const uncTip = own ? "Delayed sighting — true position uncertain; see the uncertainty cone on the map." : "Last sighting — it could be anywhere within the cone on the map.";
  const posCell = certain
    ? `<div class="stat" title="At your command center (or nearly): ~zero light-lag, so the position is effectively certain."><dt>Position</dt><dd><span class="tone-up">confirmed</span></dd></div>`
    : `<div class="stat" title="${esc(uncTip)}"><dt>Position</dt><dd>±${fmt(g.uncertainty)} su</dd></div>`;
  const strip = statStrip([ageCell, headingCell(g), posCell]);

  root.innerHTML = head + `<div class="sp-body">${strip}${own ? ownBody(g) : rivalBody(g)}</div>`;
}

// --- Hub Exchange overlay (top-navbar destination; independent of selection) ---
function openMarket(): void {
  $("market").classList.add("is-open");
  $("nav-market").classList.add("is-active");
  updateMarket();
}
function closeMarket(): void {
  $("market").classList.remove("is-open");
  $("nav-market").classList.remove("is-active");
}
function toggleMarket(): void {
  if ($("market").classList.contains("is-open")) closeMarket();
  else openMarket();
}

// --- §syndicates: the alliance panel (top-navbar destination) ------------------
// Create / invite (by corp name) / accept / leave / dissolve. Strictly owner-only
// content — the View only carries YOUR roster + YOUR pending invites, never a
// rival's. Non-engagement itself is mechanical (server-side); this panel is how
// you form the pact. Re-rendered only when the roster/invites CHANGE (a signature
// guard), so a half-typed name is never wiped by a 10 Hz View.
let lastSyndicateSig = "";
function openSyndicate(): void {
  $("syndicate-panel").classList.add("is-open");
  $("nav-syndicate").classList.add("is-active");
  lastSyndicateSig = ""; // force a fresh render on open
  updateSyndicatePanel();
}
function closeSyndicate(): void {
  $("syndicate-panel").classList.remove("is-open");
  $("nav-syndicate").classList.remove("is-active");
}
function toggleSyndicate(): void {
  if ($("syndicate-panel").classList.contains("is-open")) closeSyndicate();
  else openSyndicate();
}
function updateSyndicatePanel(): void {
  const el = $("syndicate-panel");
  if (!el.classList.contains("is-open")) return;
  const s = state.syndicate;
  const invites = state.syndicateInvites;
  const sig = JSON.stringify([s, invites, state.playerId]);
  if (sig === lastSyndicateSig && el.innerHTML) return; // no roster change → keep DOM (+ any typing)
  lastSyndicateSig = sig;
  let body = "";
  if (s) {
    const roster = s.members
      .map((m) => {
        const tag =
          m.id === state.playerId ? `<span class="you">you</span>`
          : m.id === s.founder ? `<span class="fdr">founder</span>` : "";
        return `<div class="sy-row">🟢 <span>${esc(m.name)}</span>${tag}</div>`;
      })
      .join("");
    body += `<div><div class="sy-sub">Syndicate</div><div class="sy-name">🤝 ${esc(s.name)}</div></div>`;
    body += `<div><div class="sy-sub">Members (${s.members.length})</div>${roster}</div>`;
    if (s.is_founder) {
      const invited = s.invited.length ? `<div class="sy-note">Invited: ${s.invited.map(esc).join(", ")}</div>` : "";
      body += `<div><div class="sy-sub">Invite a corp (by name)</div><div class="sy-inv"><input id="sy-invite-name" type="text" placeholder="corp name" maxlength="32" />` +
        `<button class="act" data-sy="invite">Invite</button></div>${invited}</div>`;
    }
    const dissolve = s.is_founder ? `<button class="act act--danger" data-sy="dissolve">Dissolve</button>` : "";
    body += `<div class="sy-inv"><button class="act act--danger" data-sy="leave">Leave</button>${dissolve}</div>`;
    body += `<div class="sy-note">Members never auto-engage each other, and can't raid / attack / blockade one another. Ally ships & systems tint <b style="color:#9df0b3">green</b> as their membership light reaches you.</div>`;
  } else {
    body += `<div class="sy-note">A syndicate is a mutual non-engagement pact: members can't raid, attack, or blockade each other, and their pickets leave allies alone.</div>`;
    body += `<div><div class="sy-sub">Found a syndicate</div><div class="sy-inv"><input id="sy-create-name" type="text" placeholder="syndicate name" maxlength="32" />` +
      `<button class="act act--primary" data-sy="create">Create</button></div></div>`;
    if (invites.length) {
      const list = invites
        .map((i) => `<div class="sy-row">🤝 <span>${esc(i.name)}</span><button class="act" data-sy="accept" data-sid="${esc(i.id)}" style="margin-left:auto">Accept</button></div>`)
        .join("");
      body += `<div><div class="sy-sub">Invitations</div>${list}</div>`;
    } else {
      body += `<div class="sy-note">No pending invitations.</div>`;
    }
  }
  el.innerHTML = `<div class="pp-head"><b>SYNDICATE</b><button class="pp-close" data-sy="close" title="Close">✕</button></div><div class="pp-body">${body}</div>`;
}

// --- §rankings: the published leaderboard (rail tab) ---------------------------
// A public ledger snapshot (same for everyone), sortable by category. One category
// at a time keeps it legible in the narrow rail; the chips ARE the "sortable
// categories". Your row is highlighted; category leaders wear a title chip.
type RankCat = {
  slug: string;
  label: string;
  short: string;
  fmt: (r: import("./protocol").RankingRow) => string;
  sortVal: (r: import("./protocol").RankingRow) => number;
  tip: string;
};
const RANK_CATS: RankCat[] = [
  { slug: "valuation", label: "Valuation", short: "Val", fmt: (r) => fmt(r.valuation) + " Cr", sortVal: (r) => r.valuation, tip: "Net worth — credits + holdings at market (the classic ladder)." },
  { slug: "trade_throughput", label: "Trade Throughput", short: "Trade", fmt: (r) => fmt(r.trade_throughput), sortVal: (r) => r.trade_throughput, tip: "Cargo units your convoys delivered (home, ally, or sold at the hub)." },
  { slug: "market_profit", label: "Net Market Profit", short: "Profit", fmt: (r) => fmt(r.market_profit) + " Cr", sortVal: (r) => r.market_profit, tip: "Lifetime exchange P&L — sell proceeds minus buy spend." },
  { slug: "cargo_captured", label: "Cargo Captured", short: "Seized", fmt: (r) => fmt(r.cargo_captured), sortVal: (r) => r.cargo_captured, tip: "Units seized by raiding convoys + plunder taken on captures." },
  { slug: "cargo_protected", label: "Cargo Protected", short: "Guard", fmt: (r) => fmt(r.cargo_protected), sortVal: (r) => r.cargo_protected, tip: "Units delivered by convoys that survived a battle en route." },
  { slug: "battle_efficiency", label: "Battle Efficiency", short: "Kill/Loss", fmt: (r) => (r.battle_ranked ? "×" + r.battle_efficiency.toFixed(2) : "prov."), sortVal: (r) => (r.battle_ranked ? r.battle_efficiency : -Infinity), tip: "Enemy hull destroyed ÷ own hull lost. 'prov.' = too few battles to rank." },
  { slug: "systems_developed", label: "Systems Developed", short: "Built", fmt: (r) => fmt(r.systems_developed), sortVal: (r) => r.systems_developed, tip: "Total system-upgrade tiers built." },
  { slug: "intel_gathered", label: "Intel Gathered", short: "Intel", fmt: (r) => fmt(r.intel_gathered), sortVal: (r) => r.intel_gathered, tip: "Scout snapshots captured." },
  { slug: "recovery", label: "Recovery", short: "Comeback", fmt: (r) => fmt(r.recovery) + " Cr", sortVal: (r) => r.recovery, tip: "Valuation regained since your last major loss (a captured system)." },
];
let rankingsSortCat = "valuation";
let lastRankingsSig = "";

function updateRankingsPanel(): void {
  if (!$("tab-rankings").classList.contains("is-active")) return;
  if (renderDeferred("tab-rankings", updateRankingsPanel)) return; // §single-click guard
  const rows = state.rankings;
  const sig = JSON.stringify([rows, state.playerId, rankingsSortCat]);
  if (sig === lastRankingsSig && $("rankings-body").innerHTML) return;
  lastRankingsSig = sig;

  const el = $("rankings-body");
  if (!rows.length) {
    el.innerHTML = `<div class="dim">No ledger published yet — the first close lands within a minute of the campaign start.</div>`;
    return;
  }
  const cat = RANK_CATS.find((c) => c.slug === rankingsSortCat) ?? RANK_CATS[0];
  // Category selector chips (the "sortable categories").
  const chips = RANK_CATS.map((c) => {
    const on = c.slug === cat.slug;
    return `<button class="rk-chip${on ? " is-on" : ""}" data-rankcat="${c.slug}" title="${esc(c.tip)}">${esc(c.short)}</button>`;
  }).join("");
  // Rank by the chosen category, desc; provisional efficiency sinks to the bottom.
  const sorted = [...rows].sort((a, b) => cat.sortVal(b) - cat.sortVal(a));
  const body = sorted
    .map((r, i) => {
      const me = r.player_id === state.playerId;
      const titles = r.titles.map((t) => badge("accent", t)).join(" ");
      const engTip = cat.slug === "battle_efficiency" ? ` title="${r.battle_engagements} engagement(s)"` : "";
      return (
        `<div class="rk-row${me ? " is-me" : ""}">` +
        `<span class="rk-rank">${i + 1}</span>` +
        `<span class="rk-name">${esc(r.name)}${me ? ' <span class="you">you</span>' : ""}${titles ? " " + titles : ""}</span>` +
        `<span class="rk-val"${engTip}>${cat.fmt(r)}</span>` +
        `</div>`
      );
    })
    .join("");
  el.innerHTML =
    `<div class="rk-chips">${chips}</div>` +
    `<div class="rk-catname dim">Ranked by <b>${esc(cat.label)}</b></div>` +
    `<div class="rk-table">${body}</div>`;
}

// --- Wormhole Hub detail panel (§hub-art) --------------------------------------
// The hub is PUBLIC geography (nothing to fog-gate): selecting it shows its
// concept portrait, a role blurb, and the natural shortcut — Open Market
// (the hub IS the market). Mirrors the planet-panel idiom (left dock).
let hubPanelBuilt = false;
function buildHubPanel(): void {
  if (hubPanelBuilt) return;
  hubPanelBuilt = true;
  $("hub-panel").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.act === "close") closeHubPanel();
    else if (el.dataset.act === "market") openMarket();
  });
}
function openHubPanel(): void {
  buildHubPanel();
  $("hub-panel").innerHTML =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">the shared commons</div>` +
    `<h2>Wormhole Hub</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>` +
    `<img class="hub-art" src="/art/wormhole_hub_concept.png" alt="" />` +
    `<div class="pp-body">` +
    `<div class="pp-desc">The neutral trade station at the wormhole to Sol — every corporation's goods cross here, and its Exchange sets the prices you read (light-delayed) across the galaxy.</div>` +
    `<button class="act act--primary" data-act="market">${uiIcon("concept-market-exchange", "sm")} Open Market</button>` +
    `<div class="pp-note">Convoys within its safe radius escape raids; the hub itself is neutral ground — public geography, ungated by fog.</div>` +
    `</div>`;
  $("hub-panel").classList.add("is-open");
  readout().innerHTML = `<b>Wormhole Hub</b> selected — the market lives here. <span class="dim">Press <b>M</b> or use the panel to trade.</span>`;
}
function closeHubPanel(): void {
  $("hub-panel").classList.remove("is-open");
}

// --- Check-in modal (top-navbar destination; the welcome-back digest) ----------
function openCheckin(): void {
  $("checkin").style.display = "block";
  $("nav-log").classList.add("is-active");
  updateCheckinPanel();
}
function closeCheckin(): void {
  $("checkin").style.display = "none";
  $("nav-log").classList.remove("is-active");
}
function toggleCheckin(): void {
  if ($("checkin").style.display === "none") openCheckin();
  else closeCheckin();
}

// --- System View (semantic-zoom LOD) — ENTER/EXIT + planet details -----------
// A PRESENTATION-ONLY level-of-detail: the schematic star-system view. It shows
// public geography + the SAME light-gated ownership as the galaxy map, and adds
// NO gameplay (no per-planet claim/build/defend, no intra-system ships/combat).
// All state lives in the renderer (viewMode); this layer only wires the UX.
const hex6 = (n: number) => "#" + (n >>> 0).toString(16).padStart(6, "0").slice(-6);

// The nearest star system within a screen-space radius of a point (for
// double-click / deep-zoom enter). Mirrors handleMapClick's system hit-test.
function systemUnderCursor(sx: number, sy: number, radius = 22): SystemInfo | null {
  if (!state.galaxy) return null;
  let best: SystemInfo | null = null;
  let bestD = radius;
  for (const sys of state.galaxy.systems) {
    const s = renderer.worldToScreen(sys.pos);
    const d = Math.hypot(s.x - sx, s.y - sy);
    if (d < bestD) { bestD = d; best = sys; }
  }
  return best;
}

/// §explore R2: OUR known geology for a system — the exact deposit table iff we
/// surveyed it or own it (from the light-gated view), else null (band only).
function knownDeposits(sysId: string): Deposit[] | null {
  return state.systems.find((s) => s.id === sysId)?.deposits ?? null;
}

function showBreadcrumb(name: string): void {
  $("bc-system").textContent = name;
  $("breadcrumb").classList.add("is-open");
}
function enterSystem(sys: SystemInfo): void {
  renderer.enterSystemView(sys, knownDeposits(sys.id) ?? []);
  state.selectedSystemId = sys.id; // keep the galaxy selection in sync (rail shows it)
  showBreadcrumb(sys.name);
  closePlanetPanel();
  closeRail(); // the management column takes the right dock inside the view
  // §management-home: feed the scene's structure markers + open the management
  // column (owned systems only — both no-op into scenery for rival/unclaimed).
  renderer.setSystemDevelopments(devTiersFor(sys.id));
  updateSysviewManage();
  const mine = state.systems.find((s) => s.id === sys.id)?.owner === state.playerId;
  readout().innerHTML = mine
    ? `<b>${esc(sys.name)}</b> — your system, managed from here. <span class="dim">Build/develop in the right column · click a body to see what anchors there · Esc returns to the galaxy. ` +
      `Buildings stay SYSTEM-level (slots, stockpile, defense — exactly as before).</span>`
    : `<b>${esc(sys.name)}</b> — schematic system view. <span class="dim">Click a planet for details · Esc / Back / zoom out returns to the galaxy. ` +
      `This is a VIEW: claims, production &amp; defense stay at the system level.</span>`;
}
function exitSystem(): void {
  if (renderer.viewMode.type !== "system") return;
  renderer.exitSystemView();
  $("breadcrumb").classList.remove("is-open");
  closePlanetPanel();
  closeSysviewManage();
  renderer.setSystemDevelopments(null);
}

// --- §management-home: the System View is where an OWNED system is RUN --------
// (the city-screen pattern). The management column + the structure markers are a
// RELOCATION of the rail's system-level management, not new gameplay scale:
// every command here is the same system-level BuildShip/DevelopSystem/… the rail
// sent, buildings consume SYSTEM dev slots, and the markers are decorative
// anchors. Rival/unclaimed system views stay pure scenery (tiers are owner-only
// in the View — a rival's dyn carries 0s — and we ALSO gate on `mine` here).
let sysviewManageBuilt = false;
/// The currently-viewed system id, or null when not in the System View.
function viewedSystemId(): string | null {
  const m = renderer.viewMode;
  return m.type === "system" ? m.systemId : null;
}
function devTiersFor(sid: string): DevTiers | null {
  const dyn = state.systems.find((s) => s.id === sid);
  if (!dyn || dyn.owner === null || dyn.owner !== state.playerId) return null; // fog gate: markers only on YOUR systems
  return {
    extractor: dyn.extractor_tier ?? 0,
    depot: dyn.depot_tier ?? 0,
    shipyard: dyn.shipyard_tier ?? 0,
    sensor_array: dyn.sensor_tier ?? 0,
    defense_platform: dyn.defense_tier ?? 0,
    habitat: dyn.habitat_tier ?? 0,
    refinery: dyn.refinery_tier ?? 0,
    habitat_fed: dyn.habitat_fed ?? false,
    inProgress: (dyn.builds ?? []).map((b) => b.key), // §build-progress site glyphs
  };
}
/// Per-View refresh while inside the System View: feed the scene's markers (a
/// cached no-op unless a build completed) and re-render the management column.
function updateSysviewDynamic(): void {
  const sid = viewedSystemId();
  if (!sid) return;
  renderer.setSystemDevelopments(devTiersFor(sid));
  updateSysviewManage();
}
function buildSysviewManage(): void {
  if (sysviewManageBuilt) return;
  sysviewManageBuilt = true;
  $("svm-close").addEventListener("click", exitSystem);
  // ONE delegated listener on the static panel shell (only #svm-body's innerHTML
  // is ever rewritten), so build clicks can never lose their handler.
  $("sysview-manage").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-build],[data-action]") as HTMLElement | null;
    if (!el) return;
    const sid = viewedSystemId();
    if (!sid || !net) return;
    if (el.dataset.build) {
      dispatchBuildKey(el.dataset.build, sid);
      return;
    }
    switch (el.dataset.action) {
      case "ship": {
        const manifest = shippableStock(state.systems.find((s) => s.id === sid));
        if (manifest.length) net.send({ type: "ShipProduction", system_id: sid });
        updateSysviewManage();
        break;
      }
      case "standing":
        // Logistics is a corp-wide galaxy concern — a deliberate context switch.
        exitSystem();
        openRail("logistics");
        updateStandingPanel();
        {
          const sel = $("so-source") as HTMLSelectElement;
          if ([...sel.options].some((o) => o.value === sid)) sel.value = sid;
        }
        break;
      case "market": openMarket(); break;
    }
  });
}
function closeSysviewManage(): void {
  $("sysview-manage").classList.remove("is-open");
}
function updateSysviewManage(): void {
  if (renderDeferred("sysview-manage", updateSysviewManage)) return; // §single-click
  const sid = viewedSystemId();
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const mine = !!dyn && dyn.owner !== null && dyn.owner === state.playerId;
  const panel = $("sysview-manage");
  if (!sid || !sys || !mine) {
    // Rival/unclaimed system view: pure scenery — no management column at all.
    panel.classList.remove("is-open");
    return;
  }
  buildSysviewManage();
  panel.classList.add("is-open");
  $("svm-title").textContent = sys.name;
  // SLOTS — the system's defining constraint, promoted to the header.
  const sUsed = dyn.slots_used ?? 0;
  const sTotal = dyn.slots_total ?? 0;
  const slotsEl = $("svm-slots");
  slotsEl.textContent = `SLOTS ${sUsed}/${sTotal}`;
  slotsEl.classList.toggle("is-warn", sTotal > 0 && sUsed >= sTotal);

  // Stockpile + depot cap (the "ship it or it idles" pressure).
  const cap = dyn.storage_cap ?? 0;
  const used = dyn.storage_used ?? 0;
  const storageFull = cap > 0 && used >= cap;
  const storageBar = cap > 0
    ? `<div class="deps-head">${icon("storage", "sm", "Stockpile")} Stockpile ${fmt(used)} / ${fmt(cap)}</div>` +
      `<div class="storage-row">${bar(Math.min(100, (used / cap) * 100), storageFull ? "is-warn" : "")}` +
      (storageFull ? ` ${badgeChip("storage", "full", "warn", "Storage full — production idles at the cap. Ship goods out or build a Depot to raise it (reserves aren't wasted; accrual resumes when goods ship).")}` : "") +
      `</div>`
    : "";
  // Developments at a glance — building ICONS ×tier (names live in tooltips).
  const habTag = (dyn.habitat_tier ?? 0) > 0
    ? (dyn.habitat_fed ? ` ${badgeChip("fed", "fed", "positive", "The colony is well supplied — full workforce, population growing.")}`
                       : ` ${badgeChip("unfed", (dyn.food_state ?? "rationing").replace("_", " "), "warn", "Provisions short — workforce slowed, growth paused. Nothing is destroyed; it recovers when food arrives.")}`)
    : "";
  const DEV_ROW: [string, IconKey, number, string][] = [
    ["Mining Complex", "extractor", dyn.extractor_tier ?? 0, ""],
    ["Depot", "depot", dyn.depot_tier ?? 0, ""],
    ["Shipyard", "shipyard", dyn.shipyard_tier ?? 0, ""],
    ["Sensor array", "sensor", dyn.sensor_tier ?? 0, ""],
    ["Defense platform", "defense", dyn.defense_tier ?? 0, ""],
    ["Habitat", "habitat", dyn.habitat_tier ?? 0, habTag],
    ["Fuel Refinery", "refinery", dyn.refinery_tier ?? 0, ""],
  ];
  const devs = `<div class="devs-row" title="System developments — the map markers show where each one anchors (not separate colonies). Click a body to see what would anchor there.">` +
    DEV_ROW.map(([name, key, t, tag]) => `<span class="dev ${t ? "" : "dev--none"}" title="${esc(name)} ×${t}">${icon(key, "sm", `${name} ×${t}`)}<b>×${t}</b>${tag}</span>`).join(`<span class="dev-sep">·</span>`) +
    `</div>`;
  // §contestable-territory Part 1: a blockade STRANGLES logistics — outbound
  // dispatches hold at origin, so the ship button is disabled while blockaded
  // (production still accrues into the stockpile). A prominent banner explains it.
  const blockaded = !!dyn.blockade;
  const siege = siegeProgress(dyn);
  const siegeTip = "Defenses suppressed, the siege clock is running. Break the blockade or rebuild a Defense Platform to reset it — a rival colony ship landing at full siege CAPTURES this system. (Your home can be blockaded but never falls.)";
  const siegeLine = siege
    ? `<div class="deps-head" style="margin-top:6px">${badgeChip("siege", siege.ripe ? "SIEGE CRITICAL — capture imminent" : `siege — falls in ${fmtCountdown(siege.left)}`, "negative", siegeTip)}</div>` +
      `<div class="storage-row">${bar(siege.pct, "is-warn")}</div>`
    : "";
  const blockadeBanner = blockaded
    ? `<div style="margin:6px 0">${badgeChip("blockade", "under blockade", "negative", "A rival fleet holds station — convoys are held in & out (production still accrues). Break the blockade (relief, or a new Defense Platform tier) to resume shipping.")}</div>${siegeLine}`
    : "";
  const canShip = !blockaded && shippableStock(dyn).length > 0;
  const shipTitle = blockaded ? "Held — this system is under blockade." : canShip ? "Ship one raidable convoy per commodity, selling on arrival (Fuel stays as this system's operating reserve)." : "Nothing shippable — Fuel is retained as the operating reserve; other goods ship in whole units.";
  const actions =
    `<div style="margin-top:10px">` +
    `<button class="act" data-action="ship" ${canShip ? "" : "disabled"} title="${esc(shipTitle)}">${icon("cargo", "sm")} Ship → hub</button>` +
    `<button class="act" data-action="standing" title="Set a standing logistics rule that auto-dispatches convoys from here (online or off).">${icon("doctrine", "sm")} Auto-supply</button>` +
    `<button class="act" data-action="market" title="Open the hub Exchange.">${icon("market", "sm")} Market</button></div>`;
  const guard = "";
  // §syndicates Part 3: the ally GARRISON you're hosting here — the coalition
  // shield you feed (its Provisions upkeep draws from THIS system).
  const gShips = dyn.ally_garrison_ships ?? 0;
  const garrisonHost = gShips > 0
    ? `<div class="deps-head" style="margin-top:6px">${icon("garrison", "sm")} Ally garrison: <b>${gShips}</b> ship${gShips > 1 ? "s" : ""} ` +
      (dyn.ally_garrison_fed
        ? badgeChip("garrison", "fed", "positive", "You're hosting an allied coalition shield here — its Provisions upkeep is covered from this system, and it joins your defense.")
        : badgeChip("garrison", "UNFED", "warn", "The allied garrison here is UNFED — this system is out of Provisions to cover its upkeep, so its defense is suspended. Ship Provisions here to restore it.")) +
      `</div>`
    : "";
  $("svm-eyebrow").textContent = blockaded ? "system management · UNDER BLOCKADE" : "system management · yours";
  $("svm-body").innerHTML = blockadeBanner + storageBar + devs + garrisonHost + productionReadout(dyn) + buildPanel(sid, dyn) + actions + guard;
}

let planetPanelBuilt = false;
function buildPlanetPanel(): void {
  if (planetPanelBuilt) return;
  planetPanelBuilt = true;
  $("planet-panel").addEventListener("click", (e) => {
    if ((e.target as HTMLElement).closest("[data-act='close']")) { closePlanetPanel(); return; }
    // §management-home contextual build sugar: the body panel offers the
    // developments that would ANCHOR here — the SAME system-level DevelopSystem
    // the full menu sends (a friendlier entry point, not a per-planet build).
    const b = (e.target as HTMLElement).closest("[data-build]") as HTMLElement | null;
    const sid = viewedSystemId();
    if (b?.dataset.build && sid && net) {
      dispatchBuildKey(b.dataset.build, sid);
      updateSysviewManage(); // the queue/slots readout reflects it on the next View
    }
  });
}
function closePlanetPanel(): void {
  $("planet-panel").classList.remove("is-open");
}
function openPlanetPanel(d: SystemBodyDetail): void {
  buildPlanetPanel();
  const eyebrow = d.isMoon ? "natural satellite" : d.habitable ? "habitable world" : "planet";
  const habitable = d.habitable ? " " + badge("positive", "habitable") : "";
  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h2>${esc(d.name)}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  // The body's art as the panel thumbnail (mirrors the star concept banner in
  // the System tab); the color swatch stays as the no-art fallback.
  const thumb = d.icon
    ? `<img class="pp-thumb" src="${d.icon}" alt="" />`
    : "";
  const kindLine = `<div class="pp-kindrow">${thumb}<div><span class="pp-swatch" style="background:${hex6(d.kindColor)}"></span>${esc(d.kindLabel)}${habitable}</div></div>`;
  // The SYSTEM's deposits, shown here as a VISUAL ASSOCIATION with this body — the
  // deposit still belongs to the system (claim/produce/ship it at the system level).
  const deps = d.deposits.length
    ? `<div class="sp-sec" style="color:var(--dim);text-transform:uppercase;font-size:9px;letter-spacing:0.6px;margin:12px 0 4px">Associated deposit</div>` +
      d.deposits.map(depositRow).join("")
    : `<div class="pp-note" style="border:0;padding:0;margin-top:10px">No deposit associated with this body.</div>`;
  const note = `<div class="pp-note">Public geography — the same for every corporation. Any deposit here belongs to the <b>star system</b>; claim it, develop it, and ship its output from the system panel, exactly as on the galaxy map.</div>`;
  // §management-home: contextual DEVELOP offers — the developments whose marker
  // ANCHORS at this body (icy moon → Refinery, agri world → Habitat, …). Owner's
  // own system only; the buttons issue the ordinary system-level DevelopSystem
  // with the full menu's exact gating (shared row renderer), so this is sugar,
  // never a new capability. The full build menu stays in the management column.
  let develop = "";
  const sid = viewedSystemId();
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  if (sys && dyn && dyn.owner !== null && dyn.owner === state.playerId) {
    const keys = renderer.systemAnchorsAtBody(sys, dyn.deposits ?? [], d.id);
    const opts = (state.galaxy?.build_options ?? []).filter((o) => (keys as string[]).includes(o.key));
    if (opts.length) {
      const slotsTotal = dyn.slots_total ?? 0;
      const slotsFull = slotsTotal > 0 && (dyn.slots_used ?? 0) >= slotsTotal;
      const anchored = keys.map((k) => opts.find((o) => o.key === k)?.label ?? k).join(", ");
      develop = `<div class="sp-sec" style="color:var(--dim);text-transform:uppercase;font-size:9px;letter-spacing:0.6px;margin:12px 0 4px" title="${esc(anchored)} would site its structure at this body — still a SYSTEM development (one system slot, the system stockpile).">Would anchor here</div>` +
        `<div class="build-grid">${opts.map((o) => buildOptionRow(o, dyn, slotsFull)).join("")}</div>`;
    }
  }
  $("planet-panel").innerHTML = head + `<div class="pp-body">${kindLine}<div class="pp-desc" style="margin-top:8px">${esc(d.description)}</div>${deps}${develop}${note}</div>`;
  $("planet-panel").classList.add("is-open");
}

// --- §battle-aftermath: the battle-results panel + marker interaction --------
// Markers/reports come strictly from the owner-only `View.battle_reports`; the
// client only adds presentation state: VIEWED (marker dims) and DISMISSED
// (marker hidden — the report stays in the retained list). Both persist to
// localStorage so a reload keeps the read/dismissed status.
const BATTLE_LS_KEY = "ss_battle_marks";
function loadBattleMarks(): void {
  try {
    const raw = localStorage.getItem(BATTLE_LS_KEY);
    if (!raw) return;
    const m = JSON.parse(raw) as { viewed?: number[]; dismissed?: number[] };
    state.battleViewed = new Set(m.viewed ?? []);
    state.battleDismissed = new Set(m.dismissed ?? []);
  } catch { /* corrupt marks → start clean */ }
}
function saveBattleMarks(): void {
  // Prune to ids the server still retains (the list is capped, so this stays tiny).
  const live = new Set(state.battleReports.map((r) => r.id));
  const keep = (s: Set<number>) => [...s].filter((id) => live.has(id));
  localStorage.setItem(BATTLE_LS_KEY, JSON.stringify({ viewed: keep(state.battleViewed), dismissed: keep(state.battleDismissed) }));
}
loadBattleMarks();
// Clicking a top-center notification DISMISSES it (quick fade → remove). A
// battle report also opens its full results panel (the map marker + reports
// history persist — only the transient toast goes away). Delegated once on the
// persistent log root (§single-click pattern).
$("reports-log").addEventListener("click", (e) => {
  const row = (e.target as HTMLElement).closest(".report") as HTMLElement | null;
  if (!row) return;
  if (row.dataset.reportId) openBattlePanel(Number(row.dataset.reportId));
  if (row.classList.contains("dismissing")) return; // already on its way out
  row.classList.add("dismissing");
  setTimeout(() => row.remove(), 200);
});

let battlePanelBuilt = false;
function buildBattlePanel(): void {
  if (battlePanelBuilt) return;
  battlePanelBuilt = true;
  $("battle-panel").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.act === "close") {
      openOngoingBattleId = null;
      renderer.selectedBattleMarkerId = null; // §aftermath-select: drop the ring
      $("battle-panel").classList.remove("is-open");
    } else if (el.dataset.act === "dismiss") {
      const id = Number(el.dataset.id);
      state.battleDismissed.add(id);
      if (renderer.selectedBattleMarkerId === id) renderer.selectedBattleMarkerId = null;
      saveBattleMarks();
      $("battle-panel").classList.remove("is-open");
    } else if (el.dataset.act === "withdraw" && net) {
      // §one-battle-one-icon: Withdraw an OWN engaged fleet straight from the
      // battle panel (its map marker is suppressed — no hidden sprite to hunt).
      const fleet = el.dataset.fleet;
      if (fleet) { net.send({ type: "Withdraw", fleet_id: fleet }); if (openOngoingBattleId) updateOngoingBattlePanel(); }
    } else if (el.dataset.act === "doctrine") {
      openOngoingBattleId = null;
      $("battle-panel").classList.remove("is-open");
      openRail("doctrine");
    }
  });
}
// §one-battle-one-icon: the ongoing battle whose panel is open (client-local),
// so the View handler can keep its elapsed / echo countdowns / losses live.
let openOngoingBattleId: string | null = null;
/// The nearest system's name, as a human-readable "where" for a battle site.
function nearestSystemName(p: Vec2): string {
  let best = "";
  let bestD = Infinity;
  for (const s of state.galaxy?.systems ?? []) {
    const d = Math.hypot(s.pos.x - p.x, s.pos.y - p.y);
    if (d < bestD) { bestD = d; best = s.name; }
  }
  return bestD < 200 ? `at ${best}` : best ? `near ${best} (${bestD.toFixed(0)} su out)` : `at (${p.x.toFixed(0)}, ${p.y.toFixed(0)})`;
}
function openBattlePanel(id: number): void {
  const r = state.battleReports.find((x) => x.id === id);
  if (!r) return; // rotated out of the retained list
  buildBattlePanel();
  state.battleViewed.add(id); // opening = viewed → the marker goes static/dim
  saveBattleMarks();
  const now = liveSimTime();
  const ago = (t: number) => fmtCountdown(Math.max(0, now - t));
  const youAtk = r.you === "attacker";
  const yourKind = youAtk ? r.attacker_kind : r.target_kind;
  const theirKind = youAtk ? r.target_kind : r.attacker_kind;
  const yourLoss = youAtk ? r.attacker_losses : r.target_losses;
  const theirLoss = youAtk ? r.target_losses : r.attacker_losses;
  const lossStr = (l: CompCount[]) => l.length ? l.map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") : "nothing";
  // Outcome in the recipient's terms (victory / withdrawal / mutual disengage).
  const yourSideDied = r.outcome === "both_destroyed" || (youAtk ? r.outcome === "attacker_destroyed" : r.outcome === "target_destroyed");
  const theirSideDied = r.outcome === "both_destroyed" || (youAtk ? r.outcome === "target_destroyed" : r.outcome === "attacker_destroyed");
  const verdict = yourSideDied && theirSideDied ? badge("negative", "mutual destruction")
    : yourSideDied ? badge("negative", "defeat — your force destroyed")
      : theirSideDied ? badge("positive", "victory — their force destroyed")
        : badge("neutral", "withdrawal — both sides survive");
  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">battle result · delayed report</div>` +
    `<h2>Engagement ${esc(nearestSystemName(r.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const body =
    `<div class="sp-line">${verdict}</div>` +
    `<div class="sp-sec">When</div>` +
    `<div class="sp-line">Concluded <b>${ago(r.at_time)}</b> ago · you learned <b>${ago(r.learned_at)}</b> ago <span class="dim">(light delay ${fmtCountdown(Math.max(0, r.learned_at - r.at_time))})</span></div>` +
    `<div class="sp-sec">Sides — as you learned them</div>` +
    `<div class="sp-line"><b>You</b> (${esc(youAtk ? "attacker" : "defender")}): ${esc(shipKindLabel(yourKind))}-led force</div>` +
    `<div class="sp-line"><b>Rival</b> (${esc(youAtk ? "defender" : "attacker")}): ${esc(shipKindLabel(theirKind))}-led force</div>` +
    `<div class="sp-sec" title="Outcomes are as of the light that reached your command center — the site may look different by now.">Losses</div>` +
    `<div class="sp-line">You lost: <b>${esc(lossStr(yourLoss))}</b></div>` +
    `<div class="sp-line">They lost: <b>${esc(lossStr(theirLoss))}</b></div>` +
    `<button class="act" data-act="dismiss" data-id="${r.id}" title="Remove the map marker — the report stays in your log.">${icon("aftermath", "sm")} Dismiss marker</button>`;
  $("battle-panel").innerHTML = head + `<div class="pp-body">${body}</div>`;
  $("battle-panel").classList.add("is-open");
}

// §one-battle-one-icon: open (and keep live) the ONGOING battle panel — clicking
// the single battle icon. Participants are shown AS KNOWN TO THE VIEWER (own
// fleets: full composition + the three verbs with echo countdowns; rivals:
// whatever the site-reveal already granted). Own engaged fleets are reachable
// here even though their map markers are suppressed.
function openOngoingBattlePanel(id: string): void {
  buildBattlePanel();
  openOngoingBattleId = id;
  updateOngoingBattlePanel();
  $("battle-panel").classList.add("is-open");
}
// §live-battle-panel running-loss tracking. Purely a HIGH-WATER of the viewer's
// ALREADY-DELIVERED light — never anything the ghosts didn't carry, so it can't
// leak: own fleets are tracked at EXACT counts (own light); rivals only at the
// site-revealed SIZE BUCKET (the fog never grants exact rival counts). Keyed by
// battle id; a distant viewer's staler ghosts naturally yield a laggier tally.
type BattleForceHW = { own: Map<ShipKind, number>; rivalPeak: Map<EntityId, CountClass> };
const battleForceHW = new Map<string, BattleForceHW>();
const COUNT_CLASS_ORD: Record<CountClass, number> = {
  one: 0, two_to_three: 1, four_to_seven: 2, eight_to_fifteen: 3, sixteen_to_thirty: 4, thirty_one_plus: 5,
};
// Sum a set of own ghosts' EXACT compositions into a per-kind tally.
function sumOwnComposition(ghosts: GhostView[]): Map<ShipKind, number> {
  const m = new Map<ShipKind, number>();
  for (const g of ghosts) {
    const comp = g.composition ?? [{ kind: g.kind, count: 1 }];
    for (const c of comp) m.set(c.kind, (m.get(c.kind) ?? 0) + c.count);
  }
  return m;
}
// One-way COMMAND delay (§3): command-center → battle anchor, at light speed.
// The same math the order echo-lifecycle uses; null before the galaxy/CC arrive.
function battleCommandDelay(b: BattleView): number | null {
  if (!state.commandCenter || !state.galaxy) return null;
  return Math.hypot(b.pos.x - state.commandCenter.x, b.pos.y - state.commandCenter.y) / state.galaxy.c;
}
// A one-way delay mapped onto the player's wall-clock, to the second ("~14:32:10").
function arrivalLocal(delaySecs: number): string {
  return new Date(Date.now() + delaySecs * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

// Per-class ship glyph for the live force strip (reuses the shared UI icon set).
const SHIP_ICON: Record<ShipKind, string> = {
  convoy: "concept-convoy", raider: "action-attack-raid", corvette: "concept-fleet",
  colony: "action-claim-system", scout: "action-survey-scout",
};
// One force-strip chip: a ship-class icon + the count still standing. `lost` (own,
// exact) draws a red "−k"; a fully-wiped class dims + strikes its count. `est`
// replaces the number with a fog bucket (rivals), and `shrunk` marks a bucket that
// visibly fell. The count IS the progress signal — it falls as the battle grinds.
function shipChip(kind: ShipKind, count: number, opts: { lost?: number; est?: string; shrunk?: boolean } = {}): string {
  const wiped = opts.est === undefined && count <= 0;
  const num = opts.est ?? String(count);
  const tail = opts.lost && opts.lost > 0
    ? `<span class="fs-fallen">−${opts.lost}</span>`
    : opts.shrunk ? `<span class="fs-fallen">▾</span>` : "";
  return `<span class="fs-chip${wiped ? " lost" : ""}" title="${esc(shipKindLabel(kind))}">` +
    `${uiIcon(SHIP_ICON[kind], "md")}<span class="fs-n">${esc(num)}</span>${tail}</span>`;
}
// A labelled side of the force strip. `chips` empty → a dim placeholder.
function forceSide(label: string, cls: string, chips: string): string {
  return `<div class="fs-row"><span class="fs-side ${cls}">${esc(label)}</span>` +
    `<span class="fs-chips">${chips || `<span class="fs-empty">—</span>`}</span></div>`;
}

function updateOngoingBattlePanel(): void {
  const id = openOngoingBattleId;
  if (id === null) return;
  const b = state.battles.find((x) => x.id === id);
  const panel = $("battle-panel");
  if (!b) {
    // The battle's light now shows it CONCLUDED (it left the live set). Close;
    // the aftermath marker + report carry the outcome. Drop its loss tracking.
    battleForceHW.delete(id);
    openOngoingBattleId = null;
    panel.classList.remove("is-open");
    return;
  }
  const now = liveSimTime();
  // Observed elapsed: the viewer sees the battle as of (now − age); it began at
  // started_at. Light-honest — never ahead of their light.
  const observed = Math.max(0, now - b.age - b.started_at);
  const parts = new Set(b.participants);
  const involved = state.ghosts.filter((g) => parts.has(g.id));
  const ownFleets = involved.filter((g) => g.own);
  const rivalFleets = involved.filter((g) => !g.own);
  const compStr = (g: GhostView): string => {
    const comp = g.composition ?? [];
    return comp.length ? comp.map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") : shipKindLabel(g.kind);
  };

  // Advance the high-water tally from THIS view's already-delivered light.
  const hw: BattleForceHW = battleForceHW.get(id) ?? { own: new Map<ShipKind, number>(), rivalPeak: new Map<EntityId, CountClass>() };
  const ownNow = sumOwnComposition(ownFleets);
  for (const [k, n] of ownNow) hw.own.set(k, Math.max(hw.own.get(k) ?? 0, n));
  for (const g of rivalFleets) {
    const prev = hw.rivalPeak.get(g.id);
    if (prev === undefined || COUNT_CLASS_ORD[g.count_class] > COUNT_CLASS_ORD[prev]) hw.rivalPeak.set(g.id, g.count_class);
  }
  battleForceHW.set(id, hw);

  // OWN force strip: one chip per ship CLASS still standing (exact, own light),
  // each carrying its running losses (peak − now) as a red "−k". Kinds sorted for
  // a stable order. A wiped class stays visible (dim, struck) so the toll shows.
  const ownChips = [...hw.own.keys()].sort().map((k) => {
    const cur = ownNow.get(k) ?? 0;
    return shipChip(k, cur, { lost: (hw.own.get(k) ?? 0) - cur });
  }).join("");
  // RIVAL force strip: one chip per site-revealed fleet — flagship glyph + fog
  // SIZE BUCKET (never an exact count), with a ▾ when the bucket has shrunk.
  const rivalChips = rivalFleets.map((g) => {
    const peak = hw.rivalPeak.get(g.id);
    const shrunk = peak !== undefined && COUNT_CLASS_ORD[peak] > COUNT_CLASS_ORD[g.count_class];
    return shipChip(g.kind, 0, { est: `~${countClassLabel(g.count_class)}`, shrunk });
  }).join("");

  // Compact per-fleet Withdraw (own engaged fleets) — the map markers are
  // suppressed, so these are the only handle. A tiny echo tag if an order is pending.
  const withdrawRow = ownFleets.length
    ? `<div class="wd-row">` + ownFleets.map((g) => {
        const pend = state.pendingOrders.get(g.id);
        let echo = "";
        if (pend && pend.echo_at - pend.delivered_at >= 1.5) {
          const inTransit = now < pend.delivered_at;
          echo = ` <span class="fs-echo">${inTransit ? "▸" : "◂"}${fmtCountdown((inTransit ? pend.delivered_at : pend.echo_at) - now)}</span>`;
        }
        return `<button class="wd-btn" data-act="withdraw" data-fleet="${g.id}" title="Break off ${esc(compStr(g))} and flee home — light-delayed">` +
          `↩ ${uiIcon(SHIP_ICON[g.kind], "sm")}<span class="fs-echo">${esc(compStr(g))}</span>${echo}</button>`;
      }).join("") + `</div>`
    : "";

  // §3 COMMAND DELAY, condensed to one line: one-way CC→anchor time + the local
  // wall-clock an order issued now would land at — plus a terse reach verdict.
  const delay = battleCommandDelay(b);
  const cmdDelayLine = delay !== null
    ? `<div class="sp-line dim">${uiIcon("action-standing-order", "sm")} Order lag <b style="color:var(--ink)">${fmtCountdown(delay)}</b> → lands ~${esc(arrivalLocal(delay))}` +
      (delay > 20 ? ` · <span style="color:#e88">too far to steer</span>` : ` · <span style="color:var(--accent)">still in reach</span>`) + `</div>`
    : "";

  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">${badge("negative", "battle raging")} · as of ${fmtCountdown(b.age)} ago</div>` +
    `<h2>Engagement ${esc(nearestSystemName(b.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const ragingLine = `<div class="sp-line dim">Raging <b style="color:var(--ink)">${fmtCountdown(observed)}</b> · forces remaining by your light</div>`;
  const body =
    ragingLine +
    (b.own
      ? `<div class="force-strip">${forceSide("You", "you", ownChips)}${forceSide("Enemy", "foe", rivalChips)}</div>` +
        withdrawRow +
        cmdDelayLine +
        `<button class="act" data-act="doctrine" title="Change your corp fleet doctrine — the standing engage/retreat/escort policy your fleets follow.">${icon("doctrine", "sm")} Doctrine ▸</button>`
      : `<div class="force-strip">${forceSide("Forces", "foe", rivalChips)}</div>` +
        `<div class="mhint dim" title="You see this fight only by its weapons-fire light — you have no forces here.">no forces here</div>`);
  panel.innerHTML = head + `<div class="pp-body">${body}</div>`;
}

// §contestable-territory Part 2: the CAPTURE results panel — a system changed
// hands. Reuses the ember-striped battle-panel element + the shared viewed/
// dismissed sets (capture ids are globally unique). Shows the flip in the
// recipient's terms (you captured / you lost), the light delay, and the plunder.
function openCapturePanel(id: number): void {
  const r = state.captureReports.find((x) => x.id === id);
  if (!r) return;
  buildBattlePanel();
  state.battleViewed.add(id);
  saveBattleMarks();
  const now = liveSimTime();
  const ago = (t: number) => fmtCountdown(Math.max(0, now - t));
  const plunderStr = r.plunder.length ? r.plunder.map((s) => `${s.units} ${esc(s.commodity)}`).join(", ") : "an empty stockpile";
  const verdict = r.captor
    ? badgeChip("captured", "captured — yours", "positive", "A colony ship became the occupation government (one consumed). The old owner keeps their fleets — no elimination.")
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
  $("battle-panel").classList.add("is-open");
}

// Click INSIDE the System View: a planet/moon opens its details; empty space
// clears the selection/panel. No move orders, no raids — those are galaxy-only.
function handleSystemClick(sx: number, sy: number): void {
  const d = renderer.systemPick(sx, sy);
  if (d) openPlanetPanel(d);
  else closePlanetPanel();
}

// §co-location cycling: the last selection click's spot + the stack it hit, so a
// repeat click at the same spot advances through co-located selectables instead
// of re-picking the same one. Reset implicitly whenever the spot or stack changes.
let clickCycle: { sx: number; sy: number; keys: string; index: number } | null = null;

// The map CLICK action (select own ship · select a star system incl. home ·
// inspect a command anchor · raid a rival ghost · move order to empty space). All
// hit-testing goes through screenToWorld, so it's correct at any zoom/pan. Run
// ONLY on a tap (see installInteraction's click-vs-drag gate) — never on a pan.
function handleMapClick(sx: number, sy: number, shift = false): void {
    // §aftermath-select: any fresh map click drops the concluded-battle marker
    // ring; the aftermath/capture branches below re-set it if they hit a marker.
    renderer.selectedBattleMarkerId = null;
    // §contestable-territory Part 1: BLOCKADE-ON-CLICK. With one of your RAIDER
    // fleets selected, clicking a rival-owned system orders a blockade there —
    // the raider's second verb, mirroring "click a rival contact to raid." Runs
    // BEFORE ordinary selection so the click commits the order rather than just
    // selecting the system. (A raider is required; the sim re-checks.)
    {
      const selF = state.selectedShipId ? state.ghosts.find((x) => x.id === state.selectedShipId) : undefined;
      if (selF && selF.own && selF.kind === "raider" && net && state.galaxy) {
        let hitSys: SystemInfo | null = null;
        let bestD = Infinity;
        for (const sys of state.galaxy.systems) {
          const s = renderer.worldToScreen(sys.pos);
          const d = Math.hypot(s.x - sx, s.y - sy);
          if (d < Math.max(15, renderer.systemHitRadius(sys)) && d < bestD) { bestD = d; hitSys = sys; }
        }
        const dyn = hitSys ? state.systems.find((s) => s.id === hitSys!.id) : undefined;
        const rival = dyn && dyn.owner !== null && dyn.owner !== state.playerId;
        if (hitSys && rival) {
          net.send({ type: "BlockadeSystem", fleet_id: selF.id, system_id: hitSys.id });
          delete state.orders[selF.id];
          updateShipPanel();
          readout().innerHTML =
            `Blockade ordered: your <b>raider fleet</b> → <b>${esc(hitSys.name)}</b>. ` +
            `It sets off at light speed to take station and strangle the system's logistics; ` +
            `standing defense will contest it. <span class="dim">Recall (R) to break off.</span>`;
          return;
        }
      }
      // §explore Part 2 — SURVEY-ON-CLICK (the blockade idiom for the scout's
      // second job): a SCOUT-carrying own fleet selected + click an UNSURVEYED
      // system → order a survey. Surveyed systems click-select normally (no
      // intercept — you already know their geology).
      if (selF && selF.own && net && state.galaxy && (selF.composition ?? []).some((c) => c.kind === "scout" && c.count > 0)) {
        let hitSys: SystemInfo | null = null;
        let bestD = Infinity;
        for (const sys of state.galaxy.systems) {
          const s = renderer.worldToScreen(sys.pos);
          const d = Math.hypot(s.x - sx, s.y - sy);
          if (d < Math.max(15, renderer.systemHitRadius(sys)) && d < bestD) { bestD = d; hitSys = sys; }
        }
        if (hitSys && knownDeposits(hitSys.id) === null) {
          net.send({ type: "SurveySystem", fleet_id: selF.id, system_id: hitSys.id });
          delete state.orders[selF.id];
          updateShipPanel();
          readout().innerHTML =
            `Survey ordered: your <b>scout fleet</b> → <b>${esc(hitSys.name)}</b> (${esc(hitSys.band.toUpperCase())} band). ` +
            `It flies on-site and dwells ~${SURVEY_SECS_UI}s — active sensing is LOUD (detectable farther). ` +
            `<span class="dim">The exact geology travels home at light speed; allies receive a relayed copy.</span>`;
          return;
        }
      }
    }

    // Selection priority + CO-LOCATION CYCLING. A star SYSTEM and your own SHIPS
    // are hit-tested TOGETHER, because things stack at one spot all the time:
    // your starting fleet parks on your home system, a freshly-built ship spawns
    // right on its shipyard, several fleets sit at one berth. A fixed priority
    // can only ever surface ONE of them — whatever loses is then permanently
    // unclickable (the parked-ship-vs-home-system tug-of-war). So instead,
    // REPEATED clicks at the same spot CYCLE through everything hit there. The
    // system sorts FIRST on a near-tie (SYSTEM_BIAS), so the home body still
    // opens on the first click — but one more click reaches the ship on top of
    // it. Ships out in open space still select on the first click as before.
    const SYSTEM_BIAS = 5; // px the system may be "farther" and still sort ahead on a tie
    const CLICK_CYCLE_PX = 10; // a click within this of the last cycles the stack

    // Each candidate carries its selection side-effect (`pick`), a short `label`
    // for the cycle hint, and its base `readout` message — the readout is set
    // FRESH per pick (never appended), so cycling to the system clears stale
    // ship text and the hint can't accumulate across clicks.
    type Candidate = { key: string; sortD: number; label: string; pick: () => void; readout: string };
    const cands: Candidate[] = [];

    // §one-battle-one-icon: fleets ENGAGED in a battle are represented by the
    // single battle icon (their own markers are suppressed), so exclude them from
    // ship/rival hit-testing — otherwise a participant ghost sitting under the
    // icon would swallow the click meant to OPEN the battle panel. A withdrawn
    // fleet leaves the participant set, so its marker becomes clickable again.
    const engagedIds = new Set<string>();
    for (const bt of state.battles) for (const p of bt.participants) engagedIds.add(p);

    for (const g of state.ghosts) {
      if (!g.own) continue;
      if (engagedIds.has(g.id)) continue;
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      // Hit radius tracks the MARKER's current on-screen size (formation sprite
      // included), so it grows with the sprite in the deep-zoom native-size band;
      // floored at 24px so normal-zoom clicking feels exactly as before.
      const rad = Math.max(24, renderer.fleetHitRadius(g));
      if (d < rad) {
        cands.push({
          key: `ship:${g.id}`, sortD: d, label: shipKindLabel(g.kind),
          pick: () => selectShip(g.id), // opens the fog-aware ship panel; clears any system selection
          readout: `<b>${esc(shipKindLabel(g.kind))}</b> selected — details in the panel. ` +
            `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`,
        });
      }
    }

    if (state.galaxy) {
      for (const sys of state.galaxy.systems) {
        const s = renderer.worldToScreen(sys.pos);
        const d = Math.hypot(s.x - sx, s.y - sy);
        // Hit radius follows the star's rendered disk in the deep-zoom band —
        // capped (~90px) so a max-zoom giant never blankets the map — with the
        // old 15px floor so normal-zoom clicking is unchanged.
        const rad = Math.max(15, renderer.systemHitRadius(sys));
        if (d < rad) {
          cands.push({
            key: `sys:${sys.id}`, sortD: d - SYSTEM_BIAS, label: sys.name,
            pick: () => { state.selectedSystemId = sys.id; openRail("system"); }, // → setRailTab renders the detail
            readout: `<b>${esc(sys.name)}</b> selected — details in the rail.`,
          });
        }
      }
    }

    if (cands.length) {
      cands.sort((a, b) => a.sortD - b.sortD);
      const keys = cands.map((c) => c.key).join(",");
      // Same spot + same stack as the previous click → advance to the next
      // candidate; otherwise start at the front (the system, by the bias sort).
      const prev = clickCycle;
      const same = prev !== null
        && Math.hypot(prev.sx - sx, prev.sy - sy) <= CLICK_CYCLE_PX
        && prev.keys === keys;
      const index = same ? (prev!.index + 1) % cands.length : 0;
      clickCycle = { sx, sy, keys, index };
      const chosen = cands[index];
      chosen.pick();
      // Fresh readout = the chosen thing's message, plus a stack hint naming what
      // one more click reaches (so co-located things never read as unselectable).
      let msg = chosen.readout;
      if (cands.length > 1) {
        const next = cands[(index + 1) % cands.length];
        msg += ` <span class="dim">· ${cands.length} here — click again for <b>${esc(next.label)}</b>.</span>`;
      }
      readout().innerHTML = msg;
      return;
    }

    // A home ANCHOR (command base) — the prominent "small circle" that marks where
    // a corporation commands from. It is NOT a star system (no deposits to claim),
    // so it has no System view; but clicking it should explain what it is instead
    // of feeling dead. Light-gated: a rival's base reveals nothing beyond "they're
    // here" (their systems/stockpiles/orders never leak). Own ships are picked
    // above, so the parked starting fleet still selects normally.
    let anchorPick: import("./protocol").AnchorView | null = null;
    let bestA = 14;
    for (const a of state.anchors) {
      const s = renderer.worldToScreen(a.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      if (d < bestA) {
        bestA = d;
        anchorPick = a;
      }
    }
    if (anchorPick) {
      const ownA = anchorPick.owner !== null && anchorPick.owner === state.playerId;
      readout().innerHTML = ownA
        ? `<b>Your command center</b> — your vantage on the galaxy. Everything you see is light-delayed from here; nothing reaches you faster than its light.`
        : anchorPick.owner !== null
          ? `<b>Rival command base</b> — a rival corporation commands from here. <span class="dim">You can see the base, but its systems, stockpiles &amp; orders never leak. To contest a rival, <b>claim and hold the star systems</b> around it.</span>`
          : `<b>Empty command site</b> — no corporation is based here.`;
      return;
    }

    // Rival ghost hit-test — either RAID it (when you have an own ship selected to
    // direct) or INSPECT it (open the fog-aware rival panel when you don't). Own
    // ghosts are picked earlier, so here we only ever match rivals.
    let enemy: string | null = null;
    let bestE = Infinity; // nearest rival-ghost hit distance (px)
    for (const g of state.ghosts) {
      if (g.own) continue;
      if (engagedIds.has(g.id)) continue; // engaged → reachable via the battle icon, not here
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      // Hit radius tracks the marker's current on-screen size (formation sprite
      // included, grows in deep zoom), floored at 24px so raid-targeting is unchanged.
      const rad = Math.max(24, renderer.fleetHitRadius(g));
      if (d < rad && d < bestE) {
        bestE = d;
        enemy = g.id;
      }
    }

    const sel = state.selectedShipId ? state.ghosts.find((x) => x.id === state.selectedShipId) : undefined;
    const haveOwn = !!sel && sel.own;
    // Raiding is the RAIDER-FLAGSHIP's verb (mirrors the sim's CommitRaid gate).
    const haveRaider = haveOwn && sel!.kind === "raider";
    // ATTACK needs only ≥1 raider ABOARD (the sim's contains-raider gate) — a
    // corvette-flagship fleet with a raider can attack though it can't raid.
    const haveStrike = haveOwn && !!sel!.composition?.some((c) => c.kind === "raider");

    if (enemy) {
      const tgt = state.ghosts.find((x) => x.id === enemy)!;
      if (shift && haveStrike && net) {
        // §offensive-orders Part 1: ATTACK to DESTROY (shift+click) — a full battle,
        // a convoy's cargo is lost with it (RAID steals, ATTACK kills).
        net.send({ type: "AttackFleet", fleet_id: sel!.id, target_id: tgt.id });
        net.send({ type: "EstimateEngagement", attacker: sel!.id, target: tgt.id });
        state.raids[sel!.id] = tgt.id; // drive the soft intercept-estimate overlay
        delete state.orders[sel!.id];
        updateShipPanel();
        readout().innerHTML =
          `Attack committed: your <b>${esc(shipKindLabel(sel!.kind))}</b> → rival <b>${esc(tgt.kind)}</b> to <b>destroy</b> it. ` +
          `A FULL battle (a raid steals cargo; an attack kills — cargo is lost with the fleet). ` +
          `Light-delayed pursuit of its <i>true</i> position. <span class="dim">Press R to recall — it may arrive too late.</span>`;
      } else if (haveRaider && net) {
        // Direct your selected ship to raid the rival's TRUE position.
        net.send({ type: "CommitRaid", raider_id: sel!.id, target_id: tgt.id });
        // §FLEETS Part 3: ask for a projected engagement estimate to show at
        // commit time (computed server-side from your own view data).
        net.send({ type: "EstimateEngagement", attacker: sel!.id, target: tgt.id });
        state.raids[sel!.id] = tgt.id; // drive the soft intercept-estimate overlay
        delete state.orders[sel!.id];
        updateShipPanel();
        readout().innerHTML =
          `Raid committed: your <b>${esc(shipKindLabel(sel!.kind))}</b> → rival <b>${esc(tgt.kind)}</b>. ` +
          `The order sets off at light speed; your raider will pursue the rival's <i>true</i> position, ` +
          `not the <b>${tgt.age.toFixed(0)}s</b>-old ghost you see. ` +
          (haveStrike ? `<span class="dim">Shift+click to ATTACK (destroy) instead · Press R to recall.</span>` : `<span class="dim">Press R to recall — it may arrive too late.</span>`);
      } else {
        // Nothing of yours selected to attack with → INSPECT the rival (panel).
        selectShip(enemy);
        const hint = haveStrike ? ` <span class="dim">Shift+click it to ATTACK with your selected fleet.</span>` : "";
        readout().innerHTML = `Rival <b>${esc(tgt.kind)}</b> selected — its light-delayed details are in the panel.${hint}`;
      }
      return;
    }

    // The WORMHOLE HUB landmark (public geography): show its detail panel.
    // Checked AFTER ships/rivals so fleets parked at the hub stay individually
    // selectable/raid-targetable; before the empty-space move order.
    if (state.galaxy) {
      const hs = renderer.worldToScreen(state.galaxy.hub);
      if (Math.hypot(hs.x - sx, hs.y - sy) < Math.max(24, renderer.hubHitRadius())) {
        openHubPanel();
        return;
      }
    }

    // §one-battle-one-icon: the ongoing BATTLE icon → the live battle panel
    // (its participants' own markers are suppressed, so this is how you reach
    // your engaged fleets to Withdraw). Checked here among the map chrome.
    {
      const hit = renderer.battlePick(sx, sy);
      if (hit !== null) {
        openOngoingBattlePanel(hit);
        return;
      }
    }
    // §battle-aftermath: a concluded-battle marker (owner-only UI) → its full
    // results. After ships/systems/hub — the marker is small screen-space
    // chrome and must never steal a gameplay click — before the move order.
    {
      const hit = renderer.aftermathPick(sx, sy);
      if (hit !== null) {
        // §aftermath-select: select it like any map object — standard ring + panel.
        deselectShip();
        state.selectedSystemId = null;
        renderer.selectedBattleMarkerId = hit;
        openBattlePanel(hit);
        return;
      }
    }
    // §contestable-territory Part 2: a capture marker → the capture results.
    {
      const hit = renderer.capturePick(sx, sy);
      if (hit !== null) {
        deselectShip();
        state.selectedSystemId = null;
        renderer.selectedBattleMarkerId = hit;
        openCapturePanel(hit);
        return;
      }
    }

    // Empty space → move order for the selected OWN ship (a rival can't be moved).
    if (haveOwn && net) {
      const dest = renderer.screenToWorld(sx, sy);
      net.send({ type: "MoveShip", ship_id: sel!.id, dest });
      state.orders[sel!.id] = dest;
      updateShipPanel();
      const out = sel!.age; // ≈ light delay command-center → ship
      // §explore Part 4: a COLONY ship sent toward an UNSURVEYED system is a
      // blind claim — informational friction only (never blocks the order).
      let blind = "";
      if (sel!.kind === "colony" && state.galaxy) {
        const near = state.galaxy.systems.find((sys) => Math.hypot(sys.pos.x - dest.x, sys.pos.y - dest.y) <= 150);
        if (near && knownDeposits(near.id) === null) {
          blind = ` <span style="color:var(--warn)">Heading to <b>${esc(near.name)}</b> (${esc(near.band.toUpperCase())} band) — unsurveyed, claiming blind: the composition and any hidden trait are a gamble.</span>`;
        }
      }
      readout().innerHTML =
        `Order away to <b>${esc(shipKindLabel(sel!.kind))}</b>. ` +
        `Reaches it in <b>~${out.toFixed(0)}s</b> (your light), ` +
        `you'll see it respond <b>~${(out * 2).toFixed(0)}s</b> from now. ` +
        `<span class="dim">Estimated from a ${out.toFixed(0)}s-old sighting.</span>` + blind;
    }
}

// Wire map interaction: zoom (wheel toward cursor + buttons), pan (left-drag on
// empty space), and the click action — gated so a drag PANS and never fires a
// click (no accidental move orders / raids / selections when panning).
function installInteraction(): void {
  const canvas = renderer.canvas;
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
      if (renderer.viewMode.type === "galaxy") { renderer.panBy(e.clientX - lastX, e.clientY - lastY); }
      lastX = e.clientX; lastY = e.clientY;
    }
  });
  const endPress = (e: PointerEvent) => {
    if (!down) return;
    down = false;
    try { canvas.releasePointerCapture(e.pointerId); } catch { /* not captured */ }
    // A tap (no pan) runs the click action for the ACTIVE scene; a pan suppresses it.
    // Shift+tap on the galaxy map is the ATTACK modifier (destroy vs raid).
    if (!panning) {
      if (renderer.viewMode.type === "system") handleSystemClick(e.clientX, e.clientY);
      else handleMapClick(e.clientX, e.clientY, e.shiftKey);
    }
    panning = false;
  };
  canvas.addEventListener("pointerup", endPress);
  canvas.addEventListener("pointercancel", () => { down = false; panning = false; });

  // Mouse wheel zooms toward the cursor. preventDefault stops the page scrolling;
  // over a panel the wheel hits the panel (not the canvas), so panels still scroll.
  // Wheel also drives the semantic-zoom LOD change: zooming IN past the galaxy's
  // max zoom (with a system under the cursor) ENTERS the System View; zooming OUT
  // in the System View EXITS back to the galaxy. Both are explicit LOD changes
  // (a crossfade), not a literal zoom through space.
  let sysZoomOutAccum = 0;
  canvas.addEventListener("wheel", (e: WheelEvent) => {
    e.preventDefault();
    if (renderer.viewMode.type === "system") {
      if (e.deltaY > 0) { // scrolling out
        sysZoomOutAccum += e.deltaY;
        if (sysZoomOutAccum > 60) { exitSystem(); sysZoomOutAccum = 0; }
      } else {
        sysZoomOutAccum = 0; // scrolling in — reset (no deeper level to zoom into)
      }
      return;
    }
    // Galaxy mode: if already at max zoom and the user keeps zooming IN, dive into
    // the system under the cursor (or the selected one).
    const wasMax = renderer.atMaxZoom();
    renderer.zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY * 0.0016));
    if (e.deltaY < 0 && wasMax) {
      const sys = systemUnderCursor(e.clientX, e.clientY)
        ?? (state.selectedSystemId ? state.galaxy?.systems.find((s) => s.id === state.selectedSystemId) ?? null : null);
      if (sys) enterSystem(sys);
    }
  }, { passive: false });

  // Double-click a star system → enter its System View (the primary explicit
  // enter gesture; single-click still just selects it, see handleMapClick).
  canvas.addEventListener("dblclick", (e: MouseEvent) => {
    if (renderer.viewMode.type !== "galaxy") return;
    const sys = systemUnderCursor(e.clientX, e.clientY, 16);
    if (sys) enterSystem(sys);
  });

  // Breadcrumb: GALAXY / Back both return to the galaxy map.
  $("bc-galaxy").addEventListener("click", exitSystem);
  $("bc-back").addEventListener("click", exitSystem);

  // On-screen zoom controls.
  $("zoom-in").addEventListener("click", () => renderer.zoomByFactor(1.3));
  $("zoom-out").addEventListener("click", () => renderer.zoomByFactor(1 / 1.3));
  $("zoom-reset").addEventListener("click", () => renderer.resetView());

  // Keyboard: R = recall selected raider; M = toggle the Hub Exchange panel.
  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement) return; // don't hijack the qty field
    const selShip = state.selectedShipId ? state.ghosts.find((x) => x.id === state.selectedShipId) : undefined;
    if ((e.key === "r" || e.key === "R") && selShip?.own && net) {
      net.send({ type: "RecallRaid", raider_id: selShip.id });
      delete state.raids[selShip.id]; // break off the intercept estimate
      updateShipPanel();
      readout().innerHTML =
        `Recall away to your raider — travels at light speed. ` +
        `<span class="dim">If it has already made contact, you're commanding into the past.</span>`;
    } else if (e.key === "s" || e.key === "S") {
      toggleRail("system");
    } else if (e.key === "m" || e.key === "M") {
      toggleMarket(); // hub-wide overlay, not a rail tab
    } else if (e.key === "o" || e.key === "O") {
      toggleRail("logistics");
    } else if (e.key === "f" || e.key === "F") {
      toggleRail("doctrine");
    } else if (e.key === "g" || e.key === "G") {
      toggleRail("rankings"); // §rankings: the published leaderboard
    } else if (e.key === "l" || e.key === "L") {
      toggleCheckin();
    } else if (e.key === "y" || e.key === "Y") {
      toggleSyndicate(); // §syndicates: alliance panel
    } else if (e.key === "Escape") {
      // In the System View, Escape steps out one level: planet panel → system → galaxy.
      if ($("planet-panel").classList.contains("is-open")) {
        closePlanetPanel();
      } else if (renderer.viewMode.type === "system") {
        exitSystem();
      } else {
        closeMarket();
        closeSyndicate();
        closeRail();
        closeHubPanel();
        deselectShip();
      }
    } else if (e.key === "+" || e.key === "=") {
      renderer.zoomByFactor(1.3);
    } else if (e.key === "-" || e.key === "_") {
      renderer.zoomByFactor(1 / 1.3);
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

// --- Star System view (SYSTEM tab) — a master→detail workspace (§4, §9) -------
// The galaxy map is the master list (click a system); this tab is the detail:
// header + light-gated ownership + stat strip + geology readout + production
// readout (owner-only) + valid context actions, plus an owned-systems rail when
// you hold several. Fog-safe: ownership/stockpile use exactly the light-gated
// fields the View already provides; a rival's system shows only that it's held.
// One delegated listener (set once) survives the per-render innerHTML rewrites.

// Eyebrow flavor, derived client-side from position + OUR known geology
// (§explore: the exact composition is survey knowledge — unsurveyed systems
// show only the public band).
function systemFlavor(sys: SystemInfo, deps: Deposit[] | null): string {
  const frac = state.galaxy ? Math.hypot(sys.pos.x, sys.pos.y) / state.galaxy.radius : 0;
  const tier = frac > 0.6 ? "frontier" : frac > 0.33 ? "mid-rim" : "core";
  if (deps === null) return `unsurveyed ${tier}`;
  if (!deps.length) return "barren system";
  const dom = deps.reduce((a, b) =>
    a.richness * COMMODITY_VALUE[a.resource] >= b.richness * COMMODITY_VALUE[b.resource] ? a : b);
  return `${dom.resource}-rich ${tier}`;
}

function depositRow(d: Deposit): string {
  const pct = Math.min(100, d.richness * 40);
  const reserves = d.reserves === null
    ? `<span class="tone-up">renewable</span>`
    : d.reserves < 50 ? `<span class="is-warn">${fmt(d.reserves)} left</span>`
      : `${fmt(d.reserves)} left`;
  return `<div class="dep-row"><span class="dep-ico">${commodityIcon(d.resource, "md")}</span>` +
    `<span class="dep-name">${d.resource}</span>${bar(pct)}` +
    `<span class="dep-r">~${d.richness.toFixed(2)}/s · ${reserves}</span></div>`;
}

// Owner-only production readout: per-resource stockpile + the deposit yield as its
// flow (the protocol carries no separate per-tick flow). Gated behind ownership.
// Per-Extractor-tier output multiplier — MIRRORS the sim's `EXTRACTOR_RICHNESS_MULT`
// (crates/sim/src/build.rs). Production compounds as `richness · MULT^tier`, so the
// readout shows the ACTUAL current output, not the intrinsic geology (which the
// Geology section above shows unmodified).
const EXTRACTOR_RICHNESS_MULT = 1.5;

// §explore Part 2 — UI mirror of the sim's SURVEY_SECS (the dwell duration), for
// the order readout + the progress-ring tooltip. Display only, never authoritative.
const SURVEY_SECS_UI = 20;

function productionReadout(dyn: SystemStateView | undefined): string {
  const stockOf = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const tier = dyn?.extractor_tier ?? 0;
  const mult = Math.pow(EXTRACTOR_RICHNESS_MULT, tier);
  // §economy Part 2: the colony EATS (Provisions ∝ population) and the old
  // fed-Habitat output boost is retired — supply trouble shows as the food
  // rung, not a multiplier. Owner-only, like every colony readout.
  const habTier = dyn?.habitat_tier ?? 0;
  const habFed = !!dyn?.habitat_fed; // legacy wire alias for "well supplied"
  const popM = dyn?.population ?? 0;
  const rateOf = new Map<Commodity, number>();
  // §explore: the readout is owner-only, and an owner always knows their own
  // geology (dyn.deposits present) — read from the light-gated view.
  for (const d of dyn?.deposits ?? []) rateOf.set(d.resource, (rateOf.get(d.resource) ?? 0) + d.richness * mult);
  const all = new Set<Commodity>([...stockOf.keys(), ...rateOf.keys()] as Commodity[]);
  const rows = [...all].filter((c) => (stockOf.get(c) ?? 0) >= 1 || (rateOf.get(c) ?? 0) > 0.01);
  if (!rows.length) return "";
  const tierTag = tier > 0 ? ` <span class="sp-tier" title="Extractor upgrades boost output ×${EXTRACTOR_RICHNESS_MULT} per tier">· Extractor ×${tier}</span>` : "";
  const habTag = habTier > 0 && !habFed
    ? ` <span class="sp-tier" style="color:var(--warn)" title="the colony is short on Provisions — workforce slowed, growth paused (nothing is lost)">· ${(dyn?.food_state ?? "rationing").replace("_", " ").toUpperCase()}</span>`
    : "";
  // Standing upkeep line (the game's first continuous consumption): the
  // POPULATION eats, `provisions_per_million_per_s · millions` (§economy Part 2).
  const upkeep = popM > 0
    ? `<div class="mhint" style="margin-top:4px" title="The colony's population draws Provisions from the system stockpile each second; shortages slow the workforce and pause growth — nothing is lost, nobody dies.">${icon("habitat", "sm")} pop ${popM.toFixed(1)}M eats −${((state.galaxy?.provisions_per_million_per_s ?? 0.06) * popM).toFixed(2)} ${icon("provisions", "sm")}/s${habFed ? "" : ` ${badgeChip("unfed", "short", "warn", "Provisions running low — workforce slowed, growth paused (nothing is lost).")}`}</div>`
    : "";
  // Refinery line (§buildings step 3b): converting Volatiles → Fuel, or idle dry.
  const refTier = dyn?.refinery_tier ?? 0;
  let refinery = "";
  if (refTier > 0) {
    const rate = (state.galaxy?.refinery_rate_per_tier ?? 0.5) * refTier;
    const yieldK = state.galaxy?.refinery_yield ?? 0.8;
    const vol = stockOf.get("volatiles") ?? 0;
    refinery = vol > 0
      ? `<div class="mhint" style="margin-top:4px" title="Fuel refinery: converting ${rate.toFixed(1)} Volatiles/s into ${(rate * yieldK).toFixed(1)} Fuel/s (slightly lossy).">${icon("refinery", "sm")} ${rate.toFixed(1)} ${icon("volatiles", "sm")}/s → +${(rate * yieldK).toFixed(1)} ${icon("fuel", "sm")}/s</div>`
      : `<div class="mhint" style="margin-top:4px" title="Fuel refinery idle — no Volatiles to convert. Haul some in (${yieldK} Fuel per Volatile).">${icon("refinery", "sm")} ${badgeChip("warning", "idle — no Volatiles", "warn", "Haul Volatiles in to convert.")}</div>`;
  }
  return `<div class="deps-head" style="margin-top:8px">${icon("storage", "sm")} Stockpile · production${tierTag}${habTag}</div>` +
    rows.map((c) => {
      const rt = rateOf.get(c) ?? 0;
      const rate = rt > 0.01 ? `<span class="sp-rate">+${rt.toFixed(2)}/s</span>` : `<span class="sp-none">—</span>`;
      return `<div class="sys-prod"><span class="dep-ico">${commodityIcon(c, "md")}</span>` +
        `<span>${c}</span><span class="sp-stock">${fmt(stockOf.get(c) ?? 0)}</span>${rate}</div>`;
    }).join("") + upkeep + refinery;
}

// Build / develop panel (§step1 growth + structure sinks) for an OWNED system:
// each buildable option with its recipe cost + afford state (costs draw from THIS
// system's stockpile), plus any in-progress build with an ETA. Fog-safe — only
// rendered for systems you own (the View only sends build state to the owner).
// Ship build keys — units, not developments: they never consume a development
// slot (mirrors the sim's slot rule in world.rs apply_build).
const SHIP_KEYS = new Set(["convoy", "raider", "corvette", "colony", "scout"]);
// Shipyard tier each ship kind requires — MIRRORS the sim's
// `required_shipyard_tier` (crates/sim/src/build.rs): Convoy 1, Raider 2.
// Homes bootstrap at tier 1, so convoys build turn one; raiders are earned.
const SHIP_REQ: Record<string, number> = { convoy: 1, raider: 2, corvette: 2, colony: 1, scout: 1 };

// --- §build-progress: the construction QUEUE (Travian-style) -----------------
// Rows derive ENTIRELY from the job timestamps the view already carries:
// `complete_time` (sim-time) from the server + the recipe's `build_secs` from
// the public build options give start = complete − total, so the bar fill and
// the countdown recompute from scratch every render — correct across reconnects
// and offline gaps by construction (no client-accumulated time, same pattern as
// the order-echo countdowns; no per-second traffic).
const BUILD_ICON: Record<string, string> = {
  convoy: "concept-convoy", raider: "action-attack-raid", corvette: "concept-fleet",
  colony: "action-claim-system", scout: "action-survey-scout",
  extractor: "resource-metals", depot: "action-load-cargo", shipyard: "action-build",
  sensor_array: "concept-sensor-range", defense_platform: "status-warning-threat",
  habitat: "resource-supplies", refinery: "resource-fuel",
};
const buildOption = (key: string) => state.galaxy?.build_options.find((o) => o.key === key);
const buildLabel = (key: string): string => buildOption(key)?.label ?? key;
/// The absolute wall-clock completion ("done 14:32" local) — the async-planning
/// detail: sim-time delta mapped onto the player's clock.
function doneAtLocal(completeTime: number): string {
  const ms = Date.now() + Math.max(0, completeTime - liveSimTime()) * 1000;
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}
// Brief ✓ resolve when a watched job leaves the queue (the completion notice /
// digest entry are unchanged — this is just the row's exit animation). The
// last-seen stamp keeps a long-closed panel from "flashing" stale history.
const buildQueueSeen = new Map<string, { keys: string[]; at: number }>();
const buildDoneFlash = new Map<string, { label: string; until: number }[]>();
function buildQueueRows(sid: string, dyn: SystemStateView | undefined): string {
  const jobs = dyn?.builds ?? [];
  const now = liveSimTime();
  // Diff vs the previous render to catch completions (only if seen recently).
  const prev = buildQueueSeen.get(sid);
  const keys = jobs.map((j) => j.key);
  if (prev && performance.now() - prev.at < 2000) {
    const remaining = [...keys];
    for (const k of prev.keys) {
      const i = remaining.indexOf(k);
      if (i >= 0) remaining.splice(i, 1);
      else {
        const flashes = buildDoneFlash.get(sid) ?? [];
        flashes.push({ label: buildLabel(k), until: performance.now() + 4000 });
        buildDoneFlash.set(sid, flashes);
      }
    }
  }
  buildQueueSeen.set(sid, { keys, at: performance.now() });
  const flashes = (buildDoneFlash.get(sid) ?? []).filter((f) => f.until > performance.now());
  buildDoneFlash.set(sid, flashes);
  if (!jobs.length && !flashes.length) return "";

  // Resulting tier per development job: current tier + 1 + same-key jobs ahead.
  const tierOf: Record<string, number> = {
    extractor: dyn?.extractor_tier ?? 0, depot: dyn?.depot_tier ?? 0, shipyard: dyn?.shipyard_tier ?? 0,
    sensor_array: dyn?.sensor_tier ?? 0, defense_platform: dyn?.defense_tier ?? 0,
    habitat: dyn?.habitat_tier ?? 0, refinery: dyn?.refinery_tier ?? 0,
  };
  const aheadCount: Record<string, number> = {};
  const rows = jobs.map((j) => {
    const total = buildOption(j.key)?.build_secs ?? 0;
    const start = j.complete_time - total;
    const pct = total > 0 ? Math.max(0, Math.min(100, ((now - start) / total) * 100)) : 0;
    const left = Math.max(0, j.complete_time - now);
    const isDev = !SHIP_KEYS.has(j.key);
    const ahead = aheadCount[j.key] ?? 0;
    aheadCount[j.key] = ahead + 1;
    const name = isDev ? `${buildLabel(j.key)} ×${(tierOf[j.key] ?? 0) + 1 + ahead}` : buildLabel(j.key);
    return `<div class="bq-row"><span class="bq-ic">${uiIcon(BUILD_ICON[j.key] ?? "action-build", "sm")}</span>` +
      `<div class="bq-main"><div class="bq-head"><b>${esc(name)}</b>` +
      `<span class="bq-eta">${fmtCountdown(left)} · done ${doneAtLocal(j.complete_time)}</span></div>` +
      `${bar(pct)}</div></div>`;
  }).join("");
  const doneRows = flashes.map((f) =>
    `<div class="bq-row bq-done"><span class="bq-ic tone-up">✓</span><div class="bq-main"><b>${esc(f.label)}</b> <span class="dim">complete</span></div></div>`).join("");
  return `<div class="deps-head" style="margin-top:8px">${uiIcon("action-build", "sm")} Under construction</div>` +
    `<div class="bq-list">${rows}${doneRows}</div>`;
}

// One build/develop option row — cost, afford state, and the two sim-mirroring
// gates (dev slot / shipyard tier). Shared by the full build menu and the
// System View's contextual per-body offers, so gating can never diverge.
function buildOptionRow(o: { key: string; label: string; costs: { commodity: string; units: number }[]; build_secs: number }, dyn: SystemStateView | undefined, slotsFull: boolean): string {
  const have = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const yard = dyn?.shipyard_tier ?? 0;
  const isDev = !SHIP_KEYS.has(o.key);
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const needYard = SHIP_KEYS.has(o.key) ? SHIP_REQ[o.key] ?? 1 : 0;
  const yardShort = needYard > 0 && yard < needYard;
  const blocked = (isDev && slotsFull) || yardShort;
  const enabled = afford && !blocked;
  const title = isDev && slotsFull ? "No free development slot — systems must specialize."
    : yardShort ? `Ships build only at a Shipyard system — this needs Shipyard tier ${needYard}.`
      : afford ? "Costs draw from this system's stockpile."
        : "Not enough resources stockpiled here.";
  const cost = o.costs.map((c) => `${commodityIcon(c.commodity as Commodity, "sm")}${c.units}`).join(" ");
  const gate = isDev && slotsFull ? `<span class="bo-gate" title="No free development slot.">${icon("slots", "sm")}full</span>`
    : yardShort ? `<span class="bo-gate" title="Requires Shipyard tier ${needYard}.">${icon("shipyard", "sm")}${needYard}</span>` : "";
  return `<button class="act build-opt" data-build="${o.key}" ${enabled ? "" : "disabled"} title="${esc(title)}">` +
    `<span class="bo-name">${esc(o.label)}${gate}</span><span class="bo-cost">${cost} · ${icon("time", "sm")}${o.build_secs}s</span></button>`;
}

// §explore Part 3: the trait line (name + one-line effect) for the OWNER's
// system panel. Warn-tinted for the lemon. Slug "bonus_vein:<commodity>" carries
// the vein's commodity.
function traitLine(slug: string): { title: string; desc: string; warn: boolean } {
  if (slug.startsWith("bonus_vein:")) {
    const c = slug.split(":")[1];
    return { title: "Bonus Vein", desc: `Its ${c} deposit runs ×1.5 richer — always on.`, warn: false };
  }
  switch (slug) {
    case "deep_deposits":
      return { title: "Deep Deposits", desc: "Base output ×1.5 — but the FIRST Extractor tier is wasted breaking through.", warn: false };
    case "unstable_geology":
      return { title: "Unstable Geology", desc: "Development costs ×1.25 here — the lemon a survey can't see.", warn: true };
    case "volatile_pockets":
      return { title: "Volatile Pockets", desc: "Refinery output ×1.3 here.", warn: false };
    case "precursor_cache":
      return { title: "Precursor Cache", desc: "A one-time 40 Alloys was deposited to the stockpile at claim.", warn: false };
    default:
      return { title: slug, desc: "", warn: false };
  }
}

// §node: one-line description of what a node's bonus does (by slug). Used in the
// system panel + inbox so the tactical payoff is always legible.
function nodeBonusDesc(slug: string): string {
  switch (slug) {
    case "relay_anchor":
      return "Halves your command delay to targets in its region — orders and their echoes land twice as fast nearby.";
    case "veil":
      return "Your dark fleets in its region run quieter — detected only at half the usual range.";
    case "deep_scan":
      return "Your sensors resolve EXACT composition on anything already visible in its region (bucket → exact).";
    default:
      return "A tactical edge to whoever holds it.";
  }
}

function buildPanel(sid: string, dyn: SystemStateView | undefined): string {
  const opts = state.galaxy?.build_options ?? [];
  if (!opts.length) return "";
  // Development slots (§buildings step 1) — the scarcity that forces the
  // Extractor-vs-Depot-vs-Shipyard choice. Owner-only fields (rivals see 0/0);
  // this panel renders only for owned systems, so the readout is always real.
  const slotsUsed = dyn?.slots_used ?? 0;
  const slotsTotal = dyn?.slots_total ?? 0;
  const slotsFull = slotsTotal > 0 && slotsUsed >= slotsTotal;
  const slotsTag = slotsTotal > 0
    ? ` <span class="sp-tier" title="Development slots used / total. Each development (Extractor/Depot/Shipyard tier…) uses one slot; ships don't.">· ${icon("slots", "sm")} ${slotsUsed}/${slotsTotal}</span>`
    : "";
  const head = `<div class="deps-head" style="margin-top:8px">${icon("build", "sm")} Build · develop${slotsTag}</div>`;
  // §build-progress: the construction QUEUE renders above a menu that STAYS
  // open — concurrent jobs were always legal in the sim (costs debit up front;
  // pending upgrades already count against slots); the old "one job at a time"
  // was only this panel hiding itself.
  const queue = buildQueueRows(sid, dyn);
  const rows = opts.map((o) => buildOptionRow(o, dyn, slotsFull)).join("");
  const full = slotsFull
    ? `<div class="mhint">${badgeChip("slots", "slots full", "warn", "Every development slot here is used — develop another system (specialize!).")}</div>`
    : "";
  return queue + head + `<div class="build-grid">${rows}</div>` + full;
}

// Master rail of your holdings (only when you own ≥2 — otherwise it's clutter).
function ownedSystemsRail(): string {
  if (!state.galaxy) return "";
  const owned = state.galaxy.systems.filter((s) =>
    state.systems.find((d) => d.id === s.id)?.owner === state.playerId);
  if (owned.length < 2) return "";
  return `<div class="sysrail">` + owned.map((s) => {
    const dyn = state.systems.find((d) => d.id === s.id);
    const stock = (dyn?.stockpile ?? []).reduce((n, k) => n + k.units, 0);
    const active = s.id === state.selectedSystemId ? "is-active" : "";
    return `<button class="sysrail__row ${active}" data-sys="${s.id}">` +
      `<span>${esc(s.name)} <span class="sysrail__sub">· ${stock > 0 ? fmt(stock) + " stock" : "idle"}</span></span>` +
      `<span class="sysrail__chev">›</span></button>`;
  }).join("") + `</div>`;
}

let systemTabBuilt = false;
function buildSystemTab(): void {
  if (systemTabBuilt) return;
  systemTabBuilt = true;
  $("tab-system").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-action],[data-sys],[data-build]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.sys) {
      state.selectedSystemId = el.dataset.sys; // re-selects; map highlights it too
      updateSystemTab();
      return;
    }
    const sid = state.selectedSystemId;
    if (!sid || !net) return;
    if (el.dataset.build) {
      dispatchBuildKey(el.dataset.build, sid);
      return;
    }
    switch (el.dataset.action) {
      case "inspect": {
        const s = state.galaxy?.systems.find((x) => x.id === sid);
        if (s) enterSystem(s);
        break;
      }
      case "ship": {
        // Immediate, honest feedback: list what THIS click dispatches (the same
        // non-fuel whole-units rule the sim applies), instead of silence.
        const manifest = shippableStock(state.systems.find((s) => s.id === sid));
        if (!manifest.length) {
          readout().innerHTML =
            `<b>Nothing to ship</b> — Fuel is retained as this system's operating reserve ` +
            `(sell it via the <b>Market</b>); other goods ship in whole units once produced.`;
          break; // save the round-trip: the sim would dispatch nothing anyway
        }
        net.send({ type: "ShipProduction", system_id: sid });
        readout().innerHTML =
          `Shipping <b>${manifest.map((s) => `${s.units} ${esc(s.commodity)}`).join(", ")}</b> → hub — ` +
          `one raidable convoy per commodity, selling on arrival. ` +
          `<span class="dim">Fuel stays as the reserve; a fuel-short convoy is held (see the Log).</span>`;
        break;
      }
      case "standing": {
        openRail("logistics");
        updateStandingPanel();
        const sel = $("so-source") as HTMLSelectElement;
        if ([...sel.options].some((o) => o.value === sid)) sel.value = sid;
        break;
      }
      case "market": openMarket(); break;
    }
  });
}

// §step1 build sink, shared by every build UI (the System View management
// column, its contextual body offers, and the rail's remaining paths):
// ships → BuildShip; developments → DevelopSystem. Same system-level commands
// as always — no UI adds a new gameplay verb.
function dispatchBuildKey(k: string, sid: string): void {
  if (!net) return;
  if (k === "convoy" || k === "raider" || k === "corvette" || k === "colony" || k === "scout") net.send({ type: "BuildShip", system_id: sid, ship_kind: k });
  else net.send({ type: "DevelopSystem", system_id: sid, upgrade: k }); // §economy: any structure slug
}

// What "Ship production → hub" will ACTUALLY dispatch: the system's NON-FUEL
// stock in whole units. MIRRORS the sim's apply_ship_production rule — Fuel is
// retained as the system's operating reserve (it powers movement; sell it via
// the Market), so it must neither light the button nor be promised in feedback.
// The View's stockpile is already owner-only whole units, so this is exact.
function shippableStock(dyn: SystemStateView | undefined): StockSlot[] {
  return (dyn?.stockpile ?? []).filter((s) => s.commodity !== "fuel" && s.units >= 1);
}

function updateSystemTab(): void {
  if (!systemTabBuilt) return;
  if (renderDeferred("tab-system", updateSystemTab)) return; // §single-click
  const root = $("tab-system");
  const rail = ownedSystemsRail();
  const sid = state.selectedSystemId;
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  if (!sys) {
    root.innerHTML = rail +
      `<div class="mhint" title="Click a star system on the map to inspect its geology, claim it, or ship its output${rail ? " — or pick one of your holdings above" : ""}.">${icon("mouse", "sm")} Select a star system${rail ? ", or a holding above" : ""}.</div>`;
    return;
  }
  const dyn = state.systems.find((s) => s.id === sid);
  const owner = dyn?.owner ?? null;
  const mine = owner !== null && owner === state.playerId;
  const rival = owner !== null && !mine;
  const unclaimed = owner === null;
  const stockTotal = (dyn?.stockpile ?? []).reduce((n, k) => n + k.units, 0);
  // §explore: exact geology is survey knowledge — null = unsurveyed (band only).
  const deps = dyn?.deposits ?? null;
  const yieldRate = (deps ?? []).reduce((n, d) => n + d.richness, 0);

  // A system co-located with a home anchor is a starting HOME site; the one at
  // your command center is YOUR home (granted, not claimable). Detected by
  // position (the client already knows anchor + command-center positions).
  const coincides = (p: { x: number; y: number }) => Math.abs(p.x - sys.pos.x) < 1 && Math.abs(p.y - sys.pos.y) < 1;
  const atHomeSite = state.anchors.some((a) => coincides(a.pos));
  const isMyHome = mine && !!state.commandCenter && coincides(state.commandCenter);

  const ownTag = isMyHome ? badge("accent", "home base")
    : mine ? badge("accent", "yours")
      : rival ? badge("negative", "rival") : badge("neutral", "unclaimed");
  // §contestable-territory: a blockade badge (participant-only, from the fog-safe
  // view field) — "UNDER BLOCKADE" for the owner, "BLOCKADING" for the besieger —
  // plus a SIEGE badge with a live capture countdown once defenses are suppressed.
  const siege = siegeProgress(dyn);
  let blkTag = dyn?.blockade ? ` ${badge("negative", dyn.blockade.by_me ? "blockading" : "under blockade")}` : "";
  if (siege) {
    blkTag += ` ${badge("negative", siege.ripe ? (dyn!.blockade!.by_me ? "READY TO CAPTURE" : "SIEGE — CRITICAL") : `siege ${fmtCountdown(siege.left)}`)}`;
  }
  const header = `<div class="panel-title"><div><div class="eyebrow">${esc(isMyHome ? "your command seat" : systemFlavor(sys, deps))}</div>` +
    `<h2>${esc(sys.name)}</h2></div><div class="panel-title__right">${ownTag}${blkTag}</div></div>`;

  // The system's STAR — concept art + type name. Flavor only; observable for ANY
  // system (a star is visible from afar) and leaks no economy/holdings (those stay
  // light-gated). Assigned deterministically by system id (stars.ts), so it's
  // stable and matches the map icon.
  const st = starTypeFor(sys.id);
  const starFeature = `<div class="sysview__star">` +
    `<img class="star-art" src="${starConceptUrl(st.slug)}" alt="" />` +
    `<div class="star-cap"><span class="star-type">${esc(st.title)}</span>` +
    `${st.exotic ? badge("accent", "exotic") : badge("neutral", "star")}</div></div>`;

  // §node: EXOTIC NODE — the midgame catalyst. Dormant systems telegraph a
  // countdown from t=0; awakened ones show the bonus, the holder (as our light
  // knows it), and — for the holder — the fed state + region. bonus/awakened are
  // public; fed/region are owner-only (a rival sees only the landmark + holder).
  let nodeBlock = "";
  if (dyn?.node) {
    const n = dyn.node;
    const desc = nodeBonusDesc(n.bonus);
    if (!n.awakened) {
      const awakenAt = state.galaxy?.node_awakening_time ?? 0;
      const left = Math.max(0, awakenAt - liveSimTime());
      nodeBlock =
        `<div class="deps-head" style="margin-top:8px" title="An exotic system. At the awakening time it becomes a capturable NODE granting a tactical bonus — claim it if unowned, or blockade→siege→capture if held.">◈ Exotic node — ${esc(n.title)}</div>` +
        `<div class="sp-line">${badge("accent", `awakens in ${fmtCountdown(left)}`)} <span class="dim">${esc(desc)}</span></div>`;
    } else {
      const holderTag = mine
        ? badge("accent", "you hold it")
        : rival
          ? badge("negative", "held by a rival")
          : badge("neutral", "unclaimed — capturable");
      let fedLine = "";
      if (mine) {
        fedLine = n.fed
          ? ` ${badgeChip("fed", "bonus live", "positive", "Upkeep met — the node's bonus is active.")}`
          : ` ${badgeChip("unfed", "UNFED — suspended", "negative", "The node's upkeep isn't covered — its bonus is suspended until you ship supplies here (nothing is lost).")}`;
      }
      nodeBlock =
        `<div class="deps-head" style="margin-top:8px" title="${esc(desc)}">◈ ${esc(n.title)} node</div>` +
        `<div class="sp-line">${holderTag}${fedLine}</div>` +
        `<div class="sp-line dim">${esc(desc)}</div>`;
    }
  }

  // Storage (§buildings step 2): the owner sees fill vs cap — the "ship it or
  // production idles" pressure made visible. Owner-only fields; rivals see —.
  const cap = dyn?.storage_cap ?? 0;
  const used = dyn?.storage_used ?? 0;
  const storageFull = mine && cap > 0 && used >= cap;
  const strip = statStrip([
    // §explore: exact deposit count/yield are survey knowledge — unsurveyed
    // shows the public band instead.
    stat("Band", sys.band.toUpperCase(), sys.band === "rich" ? "is-accent" : ""),
    stat("Deposits", deps ? String(deps.length) : "?"),
    stat("Yield/s", deps ? yieldRate.toFixed(1) : "?"),
    stat("Stock", mine && cap > 0 ? `${fmt(used)} / ${fmt(cap)}` : mine ? fmt(stockTotal) : "—", storageFull ? "is-warn" : ""),
    // Development slots (owner-only; §buildings step 1) — the specialization budget.
    stat("Slots", mine ? `${dyn?.slots_used ?? 0}/${dyn?.slots_total ?? 0}` : "—",
      mine && (dyn?.slots_total ?? 0) > 0 && (dyn?.slots_used ?? 0) >= (dyn?.slots_total ?? 0) ? "is-warn" : ""),
  ]);
  // Storage fill bar + full warning, under the strip (owner-only).
  const storageBar = mine && cap > 0
    ? `<div class="storage-row">${bar(Math.min(100, (used / cap) * 100), storageFull ? "is-warn" : "")}` +
      (storageFull ? `<span class="storage-warn">${badge("warn", "storage full")} production idling — ship goods out or build a Depot</span>` : "") +
      `</div>`
    : "";
  // §management-home: the rail is a SUMMARY now — the build menu, production
  // readout, and developments detail moved INTO the System View's management
  // column (one management UI to maintain, not two). The rail keeps the header,
  // stats strip, stockpile summary, ATTENTION CUES, and a prominent way in.
  const cues: string[] = [];
  if (mine) {
    if (dyn?.blockade) cues.push(`${badge("negative", "blockaded")} logistics cut — convoys held in &amp; out`);
    if (storageFull) cues.push(`${badge("warn", "storage full")} production idling`);
    if ((dyn?.population ?? 0) > 0 && !dyn?.habitat_fed) cues.push(`${badge("warn", (dyn?.food_state ?? "rationing").replace("_", " "))} workforce slowed — ship provisions`);
    if (dyn?.node?.awakened && !dyn.node.fed) cues.push(`${badge("warn", "node unfed")} bonus suspended — ship its upkeep`);
    // §build-progress: the compact construction line — a glance from the map
    // says work is running (and when the next job lands) without opening the view.
    const jobs = dyn?.builds ?? [];
    if (jobs.length === 1) {
      cues.push(`${uiIcon("action-build", "sm")} building: <b>${esc(buildLabel(jobs[0].key))}</b> — ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
    } else if (jobs.length > 1) {
      cues.push(`${uiIcon("action-build", "sm")} building ×${jobs.length} — next ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
    }
  }
  const attention = cues.length ? `<div class="mhint" style="margin-top:6px">${cues.join(" · ")}</div>` : "";

  // §explore R2: surveyed-or-owner → the full geology table; unsurveyed → the
  // band + "composition unsurveyed" (survey it — or claim blind and find out).
  // §explore R3: the OWNER's hidden-trait line (never shown to anyone else —
  // the field only ever arrives for your own systems).
  const tr = dyn?.trait ? traitLine(dyn.trait) : null;
  const traitRow = tr
    ? `<div class="mhint" style="margin-top:4px${tr.warn ? ";color:var(--warn)" : ""}" title="${esc(tr.desc)} Hidden trait — revealed by ownership; a survey can't see it.">` +
      `${badge(tr.warn ? "warn" : "accent", tr.title)} ${esc(tr.desc)}</div>`
    : "";
  const geology = deps
    ? `<div class="sysview__deps"><div class="deps-head">Geology — richer toward the frontier</div>` +
      deps.map(depositRow).join("") + traitRow + `</div>`
    : `<div class="sysview__deps"><div class="deps-head">Geology</div>` +
      `<div class="mhint" title="The spectral read gives only the richness band. Send a scout to SURVEY the exact composition — or claim blind and find out the hard way. Some systems also hide a TRAIT only ownership reveals.">` +
      `${badge(sys.band === "rich" ? "accent" : "neutral", `${sys.band.toUpperCase()} band`)} composition unsurveyed</div></div>`;

  let actions: string;
  if (unclaimed && atHomeSite) {
    actions = `<div class="mhint" style="margin-top:8px">${badgeChip("home", "reserved", "neutral", "A starting home site — a future corporation will begin here owning it, so it can't be claimed.")}</div>`;
  } else if (unclaimed) {
    // Claiming is PHYSICAL (§ships part 3): build + send a Colony Ship; it claims
    // on arrival (the how lives in the tooltip). No management view for an
    // unclaimed system, so this guidance stays on the rail.
    // §explore Part 4: informational blind-claim friction — never blocks.
    const blind = deps === null
      ? ` <span style="color:var(--warn)" title="You know only the band — the exact composition (and any hidden trait) is a gamble. Survey first with a scout, or claim blind and find out.">unsurveyed — claiming blind</span>`
      : "";
    actions = `<div class="mhint" style="margin-top:8px" title="Build a Colony Ship at a shipyard system and send it here — the system becomes yours when it ARRIVES (slow, visible, raidable: escort it). First arrival wins.">${icon("claim", "sm")} <b>To claim:</b> send a ${icon("colony", "sm")} colony ship here.${blind}</div>`;
  } else if (mine) {
    // Management lives in the System View now; the rail's job is to take you there.
    actions = "";
  } else {
    actions = `<div class="mhint" style="margin-top:8px">${badgeChip("lost", "held by rival", "negative", "Ownership is light-delayed — what you see may already be stale.")}</div>`;
  }
  // OUR scout intel about a system we don't own (§scout part 2): a timestamped
  // SNAPSHOT of its fortifications — never live, aging until re-scouted. Shown
  // only to us (the View carries only our own snapshots, light-delayed).
  let intelBlock = "";
  if (!mine && dyn?.intel) {
    const iv = dyn.intel;
    const age = Math.max(0, state.simTime - iv.observed_at);
    const ageTxt = age < 90 ? `${age.toFixed(0)}s ago` : `${(age / 60).toFixed(0)}m ago`;
    // §syndicates Part 2: RELAYED intel is ally-sourced — name the reporter and
    // show the honest chain (observed T₁ → relayed T₂ → received T₃). It ages from
    // the ORIGINAL observation and never upgrades to live truth.
    let prov = "";
    let head = "Scout intel";
    let headTip = "A scout SNAPSHOT of this rival system's fortifications — never a live feed; it ages until you re-scout (they may have built since).";
    if (iv.relayed_by) {
      const allyName = state.syndicate?.members.find((m) => m.id === iv.relayed_by)?.name ?? "an ally";
      const relayTip =
        `Relayed by ${allyName}. Observed T=${iv.observed_at.toFixed(0)}s (their scout) → reached them ${(iv.relayed_at ?? 0).toFixed(0)}s → reached you ${(iv.received_at ?? 0).toFixed(0)}s. ` +
        `Ages from the original observation; honestly staler than their own picture, and never a live feed.`;
      head = "Ally intel";
      headTip = relayTip;
      prov = ` <span class="dim" title="${esc(relayTip)}">${icon("ally", "sm")} via ${esc(allyName)}</span>`;
    }
    // §pirates: a scouted ENCLAVE reads as a pirate base (its `defense_tier` is the
    // base defense an assault must grind down), distinct from a rival fortress.
    const et = iv.enclave_tier ?? 0;
    if (et > 0) {
      head = "Pirate enclave";
      headTip = "A scouted PIRATE BASE — it raids trade nearby and grows if ignored. Station a raider fleet on it to destroy the base (it drops its plunder). A snapshot; re-scout to refresh.";
      intelBlock = `<div class="deps-head" style="margin-top:8px" title="${esc(headTip)}">${icon("raider", "sm")} ${head}</div>` +
        `<div class="sp-line">${chip("raider", `tier ×${et}`, "Enclave escalation tier — bigger, bolder packs.")} ${chip("defense", `×${iv.defense_tier}`, "Base defense (what an assault must grind down).")} <span class="dim" title="Age of this snapshot — re-scout to refresh.">${icon("time", "sm")} ${ageTxt}</span></div>`;
    } else {
      intelBlock = `<div class="deps-head" style="margin-top:8px" title="${esc(headTip)}">${icon("intel", "sm")} ${head}</div>` +
        `<div class="sp-line">${chip("defense", `×${iv.defense_tier}`, "Defense platform tier (scouted).")} ${chip("shipyard", `×${iv.shipyard_tier}`, "Shipyard tier (scouted).")} <span class="dim" title="Age of this snapshot — re-scout to refresh.">${icon("time", "sm")} ${ageTxt}</span>${prov}</div>`;
    }
  }
  // Open System View — for YOUR systems this is now THE way in to management
  // (city-screen pattern), so it's the rail's PRIMARY action; for any other
  // system it stays the presentation-only inspect. Also reachable by
  // double-click or deep-zoom on the map.
  actions += mine
    ? `<button class="act act--primary" data-action="inspect" style="margin-top:8px" title="${isMyHome ? "Your command center sits here. " : ""}Run this system from its System View — build/develop, production, and shipping. Convoys cross fogged space to the hub, raidable in transit.">${icon("build", "sm")} Open System View ▸</button>`
    : `<button class="act" data-action="inspect" title="Inspect this system (public geography — its holdings stay fogged unless you own it).">◎ Inspect ▸</button>`;

  // Only the unclaimed case keeps a short one-liner; the rest live in tooltips.
  const hint = !mine && unclaimed && !atHomeSite
    ? `<div class="mhint" title="Send a colony ship: on arrival the system becomes yours (the ship is consumed). Rivals learn you hold it only when the claim's light reaches them.">Claim by sending a ${icon("colony", "sm")} colony ship here.${deps === null ? ` <span style="color:var(--warn)">unsurveyed — claiming blind</span>` : ""}</div>`
    : "";

  root.innerHTML = rail + header + starFeature + nodeBlock + strip + storageBar + attention + geology + intelBlock + actions + hint;
}

// --- Delayed reports log -----------------------------------------------------
function addReport(r: import("./protocol").RaidReport): void {
  const log = $("reports-log");
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
  const fmtLosses = (l: import("./protocol").CompCount[]): string =>
    l.filter((c) => c.count > 0).map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ");
  const yours = r.you === "attacker" ? r.attacker_losses : r.target_losses;
  const rivals = r.you === "attacker" ? r.target_losses : r.attacker_losses;
  const yoursStr = fmtLosses(yours ?? []);
  const rivalsStr = fmtLosses(rivals ?? []);
  let lossLine = "";
  if (yoursStr || rivalsStr) {
    lossLine = `<div class="sp-line dim" style="margin-top:2px">You lost: ${yoursStr || "nothing"} · They lost: ${rivalsStr || "nothing"}</div>`;
  }
  const el = document.createElement("div");
  el.className = "report " + cls;
  // §battle-aftermath: the news toast and the retained report share an id —
  // clicking the log entry opens the same results panel as the map marker.
  el.dataset.reportId = String(r.report_id);
  el.title = "Open the full battle results";
  el.innerHTML = `<span class="ic">${icon}</span> ${text} <span class="dim">— delayed news, ${r.age.toFixed(0)}s old</span>${lossLine}`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
}

// §FLEETS Part 3: the commit-time STALE-INTEL battle calculator panel. Renders
// the server's projection (computed from YOUR view data) into the report stream —
// projected per-kind losses on both sides, honest about the age of every input
// and about whether the target's makeup was known or a typical-hull estimate.
function showEngagementEstimate(e: import("./protocol").EngagementEstimate): void {
  const log = $("reports-log");
  const fmt = (l: import("./protocol").CompCount[]): string =>
    l.filter((c) => c.count > 0).map((c) => `${c.count} ${shipKindLabel(c.kind)}`).join(", ") || "none";
  const targetDesc = e.target_known
    ? "their exact composition"
    : `est. ${countClassLabel(e.target_count_class)} ships — <b>assuming typical hulls</b>`;
  const ages: string[] = [`their composition: ${e.composition_age.toFixed(0)}s old`];
  ages.push(e.defenses_age != null ? `defenses: scouted ${e.defenses_age.toFixed(0)}s ago` : `defenses: unknown`);
  const el = document.createElement("div");
  el.className = "report good";
  el.innerHTML =
    `<span class="ic">⟿</span> <b>Projected raid</b> — ${targetDesc}` +
    `<div class="sp-line dim" style="margin-top:2px">You'd lose: ${esc(fmt(e.own_losses))} · They'd lose: ${esc(fmt(e.target_losses))}${e.platform_tiers != null ? ` · through a ${e.platform_tiers}-tier platform` : ""}</div>` +
    `<div class="sp-line dim">${esc(ages.join(" · "))} — exact arithmetic on stale inputs</div>`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 15000);
}

// --- Hub Exchange (§9) — MARKET tab: a price board with observed-history
// sparklines + honest staleness, and a buy/sell composer that surfaces the
// buy(instant)/sell(raidable convoy, clears on arrival) asymmetry. Inspired by
// Stellar Charters' Exchange. UI-only: same messages, same lagged-price model. --
const COMMODITIES: Commodity[] = [
  "metallic_ore", "rare_elements", "silicates", "volatiles", "biomass",
  "alloys", "electronics", "polymers", "fuel", "provisions",
  "machinery", "armaments",
];

// The composer's local selection (the board is the master list, this the detail).
const composer: { side: Side; commodity: Commodity } = { side: "buy", commodity: "fuel" };

// Accumulate the OBSERVED hub prices into a per-commodity rolling history (the
// sparkline data source). Fog-safe: it only ever stores the lagged prices the
// player has already been shown. Throttled to ~1 Hz of sim-time, capped.
const PRICE_HISTORY_CAP = 60; // ~1 minute at 1 Hz sampling
function recordPriceHistory(): void {
  if (!state.market) return;
  if (state.simTime - state.lastPriceSampleAt < 0.9) return; // throttle
  state.lastPriceSampleAt = state.simTime;
  for (const p of state.market.prices) {
    const series = (state.priceHistory[p.commodity] ??= []);
    series.push(p.price);
    if (series.length > PRICE_HISTORY_CAP) series.shift();
  }
}

let marketBuilt = false;
function buildMarketPanel(): void {
  if (marketBuilt) return;
  marketBuilt = true;
  // Board row click = select commodity (master→detail drives the composer).
  $("market-board").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-resource]") as HTMLElement | null;
    if (!b?.dataset.resource) return;
    composer.commodity = b.dataset.resource as Commodity;
    renderMarketBoard();
    renderComposer();
  });
  $("mk-side").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("button") as HTMLElement | null;
    if (!b?.dataset.side) return;
    composer.side = b.dataset.side as Side;
    renderComposer();
  });
  $("mk-limit-on").addEventListener("change", () => {
    ($("mk-limit") as HTMLInputElement).disabled = !($("mk-limit-on") as HTMLInputElement).checked;
    renderComposer();
  });
  $("mk-qty").addEventListener("input", renderComposer);
  $("mk-limit").addEventListener("input", renderComposer);
  $("mk-submit").addEventListener("click", () => {
    if (!net) return;
    const c = composer.commodity;
    const qty = Math.max(1, Math.floor(Number(($("mk-qty") as HTMLInputElement).value) || 0));
    const limitOn = ($("mk-limit-on") as HTMLInputElement).checked;
    const limitPrice = Number(($("mk-limit") as HTMLInputElement).value);
    if (limitOn && limitPrice > 0) {
      net.send({ type: "PlaceLimitOrder", side: composer.side, commodity: c, units: qty, limit_price: limitPrice });
    } else {
      net.send(composer.side === "buy" ? { type: "MarketBuy", commodity: c, units: qty } : { type: "MarketSell", commodity: c, units: qty });
    }
    $("mk-feedback").textContent = `Order sent: ${composer.side} ${qty} ${c}${limitOn && limitPrice > 0 ? ` @ ${limitPrice}` : ""}.`;
  });
}

// The per-commodity price board: icon | name | observed sparkline | (stale-aware)
// price + observed-trend glyph | held. Selection highlights the active row.
function renderMarketBoard(): void {
  if (!state.market) return;
  const priceOf = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  const heldOf = new Map((state.wallet?.inventory ?? []).map((i) => [i.commodity, i.units]));
  const stale = state.market.staleness > 0.5;
  $("market-board").innerHTML = COMMODITIES.map((c) => {
    const p = priceOf.get(c);
    const hist = state.priceHistory[c] ?? [];
    const tr = trend(hist);
    const active = composer.commodity === c ? "is-active" : "";
    const priceTxt = p === undefined ? `<span class="is-stale">—</span>` : `${stale ? "~" : ""}${p.toFixed(2)}`;
    return `<button class="board__row ${active}" data-resource="${c}" title="observed from your own price history — not a market forecast">` +
      `<span class="dep-ico">${commodityIcon(c, "md")}</span>` +
      `<span class="b-name">${c}</span>` +
      spark(hist.length ? hist : (p !== undefined ? [p, p] : [0, 0])) +
      `<span class="b-price ${stale ? "is-stale" : ""}">${priceTxt} <span class="b-trend ${tr.tone}">${tr.glyph}</span></span>` +
      `<span class="b-held">${heldOf.get(c) ?? 0}</span></button>`;
  }).join("");
}

// The composer preview surfaces the buy/sell asymmetry in plain language — the
// honest-fog centerpiece (teaches the lightspeed economy, not shipping fees).
function renderComposer(): void {
  if (!state.market) return;
  const c = composer.commodity;
  const price = state.market.prices.find((p) => p.commodity === c)?.price;
  const stale = state.market.staleness > 0.5;
  const px = price !== undefined ? `${stale ? "~" : ""}${price.toFixed(2)}` : "?";
  $("mk-sel").textContent = c;
  document.querySelectorAll<HTMLElement>("#mk-side button").forEach((b) => b.classList.toggle("is-active", b.dataset.side === composer.side));
  const qty = Math.max(1, Math.floor(Number(($("mk-qty") as HTMLInputElement).value) || 0));
  const limitOn = ($("mk-limit-on") as HTMLInputElement).checked;
  const submit = $("mk-submit");
  if (limitOn) {
    $("mk-preview").innerHTML = `<span title="It rests on the book and clears in the periodic uniform-price batch — reacting fastest confers no edge; partial fills carry to the next batch."><b>Limit ${composer.side} ${qty} ${c}</b> → rests, clears in the <span class="accent">batch</span></span>`;
    submit.textContent = `Place limit ${composer.side}`;
  } else if (composer.side === "buy") {
    const cost = price !== undefined ? fmt(qty * price) : "?";
    $("mk-preview").innerHTML = `<span title="Settles instantly; the goods then cross fogged space to your home anchor as a delivery convoy — raidable in transit.">Settles <b>now</b> ~<span class="accent">${cost} Cr</span> → ${icon("convoy", "sm")} <b>raidable</b> delivery</span>`;
    submit.textContent = `Buy ${qty} ${c}`;
  } else {
    $("mk-preview").innerHTML = `<span title="A convoy is dispatched now; it clears at the price ON ARRIVAL (not today's ${px}) and is raidable until it reaches the hub — double uncertainty: price + delivery.">${icon("convoy", "sm")} <b>dispatched now</b> → clears at price <b>on arrival</b> · <b>raidable</b></span>`;
    submit.textContent = `Sell ${qty} ${c}`;
  }
}

function renderRestingOrders(): void {
  const orders = state.wallet?.orders ?? [];
  $("market-orders").innerHTML = orders.length
    ? `<div class="deps-head">Resting limit orders</div>` +
      orders.map((o) => `<div class="ord">${badge(o.side === "buy" ? "positive" : "warn", `${o.side} ${o.units} ${o.commodity} @ ${o.limit_price.toFixed(1)}`)}</div>`).join("")
    : "";
}

function updateMarket(): void {
  if (renderDeferred("market", updateMarket)) return; // §single-click
  if (!state.market || !state.wallet) return;
  const stale = state.market.staleness;
  const fresh = $("market-fresh");
  fresh.className = "badge " + (stale > 0.5 ? "badge--warn" : "badge--positive");
  fresh.textContent = stale > 0.5 ? `~${stale.toFixed(0)}s stale` : "live";
  $("market-wallet").innerHTML = statStrip([
    stat("Credits", `${fmt(state.wallet.credits)} Cr`, "is-accent"),
    stat("Equity", `${fmt(state.wallet.valuation)} Cr`),
  ]);
  renderMarketBoard();
  renderComposer();
  renderRestingOrders();
}

function addTradeNews(t: TradeEvent): void {
  const log = $("reports-log");
  let text = "";
  switch (t.event) {
    case "Bought": text = `Bought ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} — delivery convoy inbound (raidable).`; break;
    case "Delivered": text = `Delivery arrived: +${t.units} ${t.commodity} (stored at destination).`; break;
    case "SellDispatched": text = `Sell convoy away: ${t.units} ${t.commodity} crossing to the hub.`; break;
    case "Sold": text = `Sold ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} on arrival.`; break;
    case "LimitPlaced": text = `Limit ${t.side} ${t.units} ${t.commodity} @ ${t.limit_price.toFixed(2)} resting on the book.`; break;
    case "LimitFilled": text = `Limit ${t.side} filled in batch: ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)}.`; break;
    case "AutoDispatched": text = `⚙ Standing order #${t.rule_id} shipped ${t.units} ${t.commodity} (auto, raidable).`; break;
    case "SupplyDiverted": {
      const what = t.action === "lost" ? "lost (cargo dropped)"
        : t.action === "returned_home" ? "re-routed home (raidable)"
        : "re-routed to sell at the hub (raidable)";
      text = `⚠ Supply to ${systemName(t.system)} undeliverable — you no longer hold it: ${t.units} ${t.commodity} ${what}.`;
      break;
    }
    case "StorageOverflow":
      text = `⚠ Depot full at ${systemName(t.system)}: ${t.units} ${t.commodity} couldn't be stored — convoy carries it on to sell at the hub (raidable).`;
      break;
  }
  const el = document.createElement("div");
  el.className = t.event === "SupplyDiverted" && t.action === "lost" ? "report bad" : "report good";
  el.innerHTML = `<span class="ic" style="color:#7fd4ff">◈</span> ${text}`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
}

// --- Standing orders panel (§15) — constrained logistics automation ----------
function systemName(id: string): string {
  return state.galaxy?.systems.find((x) => x.id === id)?.name ?? id;
}
function ownedSystems(): { id: string; name: string }[] {
  if (state.playerId === null) return [];
  return state.systems
    .filter((s) => s.owner === state.playerId)
    .map((s) => ({ id: s.id, name: systemName(s.id) }));
}
// §syndicates Part 3: SYNDICATE-ally systems (per the viewer's known membership)
// are valid AID destinations for standing orders / convoys — deliveries credit the
// ally's stockpile (blockades still interdict the run).
function allySystems(): { id: string; name: string }[] {
  return state.systems
    .filter((s) => s.ally)
    .map((s) => ({ id: s.id, name: systemName(s.id) }));
}
function endpointLabel(e: StandingEndpoint): string {
  return e.kind === "hub" ? "hub" : e.kind === "home" ? "home" : systemName(e.id);
}
function triggerLabel(t: StandingTrigger): string {
  if (t.kind === "above_threshold") return `when stock ≥ ${t.threshold}`;
  if (t.kind === "percent_surplus") return `${t.percent}% of surplus over ${t.floor}`;
  return `keep dest ≥ ${t.target}`;
}

let standingBuilt = false;
function buildStandingPanel(): void {
  if (standingBuilt) return;
  standingBuilt = true;
  const trig = $("so-trigger") as HTMLSelectElement;
  const syncForm = () => {
    const amt = $("so-amount") as HTMLInputElement;
    ($("so-floor-row") as HTMLElement).style.display = trig.value === "percent_surplus" ? "flex" : "none";
    amt.title = trig.value === "above_threshold" ? "threshold (units)"
      : trig.value === "percent_surplus" ? "percent (1–100)"
      : "target level (units)";
  };
  trig.addEventListener("change", syncForm);
  syncForm();
  // Remove-✕ is delegated on the PERSISTENT list root — the rows rebuild every
  // View, the listener never does (§single-click).
  $("standing-list").addEventListener("click", (e) => {
    const x = (e.target as HTMLElement).closest("[data-clear]") as HTMLElement | null;
    if (x && net) net.send({ type: "ClearStandingOrder", order_id: Number(x.dataset.clear) });
  });
  $("so-add").addEventListener("click", () => {
    if (!net) return;
    const source = ($("so-source") as HTMLSelectElement).value;
    if (!source) return; // need an owned source system first
    const commodity = ($("so-commodity") as HTMLSelectElement).value as Commodity;
    const tkind = ($("so-trigger") as HTMLSelectElement).value;
    const amount = Number(($("so-amount") as HTMLInputElement).value) || 0;
    const floor = Number(($("so-floor") as HTMLInputElement).value) || 0;
    const destVal = ($("so-dest") as HTMLSelectElement).value;
    const dest: StandingEndpoint = destVal === "hub" ? { kind: "hub" }
      : destVal === "home" ? { kind: "home" }
      : { kind: "system", id: destVal };
    let trigger: StandingTrigger;
    if (tkind === "percent_surplus") trigger = { kind: "percent_surplus", percent: Math.max(1, Math.min(100, Math.round(amount))), floor };
    else if (tkind === "maintain_at_dest") trigger = { kind: "maintain_at_dest", target: amount };
    else trigger = { kind: "above_threshold", threshold: amount };
    const order: StandingOrder = {
      id: 0, source: { kind: "system", id: source }, dest, commodity, trigger,
      status: "active", next_eval_tick: 0, in_flight: null,
    };
    net.send({ type: "SetStandingOrder", order });
  });
}

function updateStandingPanel(): void {
  if (!standingBuilt) return;
  if (renderDeferred("standing", updateStandingPanel)) return; // §single-click
  // Rebuild source/dest selects only when the owned-systems set changes (so a
  // mid-edit selection isn't clobbered every tick).
  const owned = ownedSystems();
  const allies = allySystems();
  // Key includes the ally set so the dest list rebuilds when an alliance forms/ends.
  const ownedKey = owned.map((s) => s.id).join(",") + "|" + allies.map((s) => s.id).join(",");
  const srcSel = $("so-source") as HTMLSelectElement;
  if (srcSel.dataset.key !== ownedKey) {
    srcSel.dataset.key = ownedKey;
    const destSel = $("so-dest") as HTMLSelectElement;
    const prevSrc = srcSel.value, prevDest = destSel.value;
    // Source is always your OWN system; destinations add hub/home + your depots +
    // ally systems (§syndicates Part 3 AID).
    srcSel.innerHTML = owned.length
      ? owned.map((s) => `<option value="${s.id}">${s.name}</option>`).join("")
      : `<option value="">(claim a system first)</option>`;
    if (owned.some((s) => s.id === prevSrc)) srcSel.value = prevSrc;
    destSel.innerHTML = `<option value="hub">hub (sell)</option><option value="home">home (store)</option>` +
      owned.map((s) => `<option value="${s.id}">${s.name} (depot)</option>`).join("") +
      allies.map((s) => `<option value="${s.id}">${s.name} (ally aid)</option>`).join("");
    if (prevDest) destSel.value = prevDest;
  }
  const comSel = $("so-commodity") as HTMLSelectElement;
  if (!comSel.options.length) comSel.innerHTML = COMMODITIES.map((c) => `<option value="${c}">${c}</option>`).join("");

  const list = $("standing-list");
  const orders = state.standingOrders;
  if (!orders.length) {
    list.innerHTML = `<span class="dim">No standing orders yet — set one below. They run on the server while you're away.</span>`;
    return;
  }
  list.innerHTML = orders
    .map((o) => {
      const flight = o.in_flight ? `<span class="run">● convoy en route</span>` : `<span class="dim">idle</span>`;
      const paused = o.status === "paused" ? " · paused" : "";
      return `<div class="so"><span class="x" data-clear="${o.id}" title="remove">✕</span>` +
        `<b>#${o.id}</b> ${commodityIcon(o.commodity, "sm")} ${o.commodity}: ${endpointLabel(o.source)} → ${endpointLabel(o.dest)}${paused}<br>` +
        `<span class="meta">${triggerLabel(o.trigger)} · ${flight}</span></div>`;
    })
    .join("");
  // ✕ handling is DELEGATED on the persistent list root (see buildStanding…) —
  // per-render listeners on the rebuilt rows was the old pattern; §single-click
  // standardizes on delegation everywhere.
}

// --- Fleet doctrine panel (§16) — constrained combat & logistics policy -------
// Four dropdowns, each a closed menu mirroring the sim enums; any change sends
// the whole doctrine (instant local admin — the convoys/pickets it commands stay
// raidable & light-revealed). Every field defaults to today's behaviour.
const DOCTRINE_FIELDS: { key: keyof FleetDoctrine; id: string; opts: [string, string][] }[] = [
  { key: "engagement", id: "fd-engage", opts: [
    ["avoid", "Avoid — never engage"],
    ["defensive_only", "Defensive only (default)"],
    ["engage_weaker", "Engage weaker — hunt when you outnumber"],
    ["engage_any", "Engage any — hunt all sensed hostiles"],
  ] },
  { key: "retreat", id: "fd-retreat", opts: [
    ["quarter", "Retreat if outnumbered ~3:1 (25%)"],
    ["half", "Retreat if outnumbered (50%)"],
    ["three_quarter", "Hold only with a clear edge (75%)"],
    ["never", "Never retreat (default)"],
  ] },
  { key: "escort", id: "fd-escort", opts: [
    ["guard_nearest", "Guard nearest convoy (default)"],
    ["guard_richest", "Guard richest convoy"],
    ["hold_station", "Hold station — picket your route"],
  ] },
  { key: "destination_invalid", id: "fd-dest", opts: [
    ["drop", "Lost supply: drop cargo (default)"],
    ["return_home", "Lost supply: re-route home"],
    ["sell_at_hub", "Lost supply: sell at hub"],
  ] },
];

let doctrineBuilt = false;
function buildDoctrinePanel(): void {
  if (doctrineBuilt) return;
  doctrineBuilt = true;
  const sendDoctrine = () => {
    if (!net) return;
    const d = { ...state.doctrine };
    for (const f of DOCTRINE_FIELDS) {
      (d as Record<string, string>)[f.key] = ($(f.id) as HTMLSelectElement).value;
    }
    net.send({ type: "SetFleetDoctrine", doctrine: d });
  };
  for (const f of DOCTRINE_FIELDS) {
    const sel = $(f.id) as HTMLSelectElement;
    sel.innerHTML = f.opts.map(([v, label]) => `<option value="${v}">${label}</option>`).join("");
    sel.addEventListener("change", sendDoctrine);
  }
}

function updateDoctrinePanel(): void {
  if (!doctrineBuilt) return;
  for (const f of DOCTRINE_FIELDS) {
    const sel = $(f.id) as HTMLSelectElement;
    // Don't clobber a dropdown the player is actively changing.
    if (document.activeElement === sel) continue;
    sel.value = String(state.doctrine[f.key]);
  }
}

// --- Check-in loop (§16, Layer 3) — timeline digest + attention surfacing ----
// Presence buys AWARENESS, not advantage: when you check in, here's what became
// observable while you were away, and the decisions waiting for you. The timeline
// is server-composed (light-correct, buffered offline); the attention items are
// derived right here from the player's own View — no extra information, just a
// summary of what they can already see.
function agoLabel(at: number): string {
  const d = Math.max(0, state.simTime - at);
  return d < 90 ? `${d.toFixed(0)}s ago` : `${(d / 60).toFixed(0)}m ago`;
}

// §contestable-territory Part 2: siege progress for a system's blockade view
// field. Returns null unless the (defense-suppressed) siege clock is running.
// `pct` fills a bar; `left` is the capture countdown; `ripe` = a colony ship
// delivered now would capture.
function siegeProgress(dyn: SystemStateView | undefined): { pct: number; left: number; ripe: boolean } | null {
  if (!dyn?.blockade || dyn.blockade.siege_since == null || !state.galaxy) return null;
  const total = state.galaxy.siege_secs || 1;
  const elapsed = Math.max(0, liveSimTime() - dyn.blockade.siege_since);
  return { pct: Math.min(100, (elapsed / total) * 100), left: Math.max(0, total - elapsed), ripe: elapsed >= total };
}

// ================= DECISION INBOX (§decision-inbox) =========================
// The digest's PRIMARY surface: not "what happened" but "what deserves a
// decision". Every item is a PURE FUNCTION of already-delivered, OWNER-GATED View
// state — blockade is participant-only, stockpile/tiers/garrison are owner-only,
// battle/capture reports are per-participant, ghosts are the fog-safe delayed
// feed — so the inbox carries NO new information and there is nothing to leak
// beyond what the View already (leak-tested) reveals. Priority is encoded in the
// weights: threats > strangulation > idle capacity > information (tunable here).
const INBOX_W = {
  siege: 100, battle: 92, hostile: 85, captureLost: 82, blockade: 80,
  garrisonUnfed: 70, nodeUnfed: 68, enclave: 58, storageFull: 55, unfedHabitat: 50, idleStockpile: 48,
  brokenOrder: 46, surveyReport: 45, nodeAwakening: 44, dryRefinery: 42, nodeOpportunity: 41, myGarrisonUnfed: 40,
  surveyOpportunity: 36, emptyQueue: 34,
  captureWon: 28, battleReport: 26, noAutomation: 20,
};
const HOSTILE_CONCERN_MULT = 1.6; // a raider within this × sensor_range of an asset
const REPORT_RECENT_S = 300; // capture/battle reports surface only while this fresh
const IDLE_UNITS = 30; // idle-stockpile threshold
const MAX_HOSTILE_ITEMS = 4;

type InboxTone = "negative" | "warn" | "info" | "neutral";
type InboxAction = { label: string; icon?: IconKey; run: () => void; deliveryPos?: Vec2; primary?: boolean; danger?: boolean };
type InboxItem = { key: string; weight: number; tone: InboxTone; icon: IconKey; headline: string; stakes?: string; age?: number; confidence?: string; actions: InboxAction[] };

const dismissedInbox = new Set<string>();
let currentInbox: InboxItem[] = [];

// §explore Part 4: SURVEY REPORT detection — geology APPEARING for a system we
// didn't previously know (our survey landing, or an ally's relayed copy; the
// view field is the single source, so this is fog-safe by construction). Seeded
// silently on the first View (the join payload isn't news); systems WE own are
// suppressed (claiming reveals by holding, not by a report).
let knownGeologyIds: Set<string> | null = null;
const freshSurveyReports = new Map<string, number>(); // system id → sim-time noticed

function noteSurveyReports(simTime: number): void {
  const cur = new Set(state.systems.filter((x) => x.deposits != null).map((x) => x.id));
  if (knownGeologyIds === null) {
    knownGeologyIds = cur; // first View: seed silently
    return;
  }
  for (const id of cur) {
    if (!knownGeologyIds.has(id)) {
      knownGeologyIds.add(id);
      const dyn = state.systems.find((x) => x.id === id);
      if (dyn?.owner !== state.playerId) freshSurveyReports.set(id, simTime);
    }
  }
  // Age out stale reports (they remain in the log/panel; the CARD is for the
  // decision window).
  for (const [id, t] of freshSurveyReports) {
    if (simTime - t > REPORT_RECENT_S) freshSurveyReports.delete(id);
  }
}

// One-way command delay (cc → pos) — the SAME echo math the order lifecycle uses;
// null before the galaxy/CC arrive.
function commandDelayTo(pos: Vec2): number | null {
  if (!state.commandCenter || !state.galaxy) return null;
  return Math.hypot(pos.x - state.commandCenter.x, pos.y - state.commandCenter.y) / state.galaxy.c;
}
// Nearest KNOWN system name to a point (for naming a battle/report location).
function locName(pos: Vec2): string {
  if (!state.galaxy) return `(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
  let best: { name: string; d: number } | null = null;
  for (const s of state.galaxy.systems) {
    const d = Math.hypot(s.pos.x - pos.x, s.pos.y - pos.y);
    if (!best || d < best.d) best = { name: s.name, d };
  }
  return best ? best.name : `(${Math.round(pos.x)}, ${Math.round(pos.y)})`;
}
// Deep-link actions (close the inbox, focus the relevant panel/target).
function inboxFocusSystem(id: string): void { state.selectedShipId = null; state.selectedSystemId = id; closeCheckin(); openRail("system"); }
function inboxFocusFleet(id: string): void { closeCheckin(); selectShip(id); }
function inboxOpenLogistics(): void { closeCheckin(); openRail("logistics"); }
const dismissAct = (key: string): InboxAction => ({ label: "Dismiss", run: () => { dismissedInbox.add(key); renderInbox(); } });

// Derive the prioritized inbox from owner-gated View state. Deterministic order
// (weight desc, then key) so it rebuilds identically on reconnect.
function computeInbox(): InboxItem[] {
  const out: InboxItem[] = [];
  if (state.playerId === null || !state.galaxy) return out;
  const galaxy = state.galaxy;
  const owned = state.systems.filter((s) => s.owner === state.playerId);
  const ownedIds = new Set(owned.map((s) => s.id));
  const active = state.standingOrders.filter((o) => o.status === "active");
  const now = liveSimTime();
  const push = (it: InboxItem) => { if (!dismissedInbox.has(it.key)) out.push(it); };
  const sysPos = (id: string) => galaxy.systems.find((x) => x.id === id)?.pos ?? null;

  // --- SIEGE / BLOCKADE (threat / strangulation; owner-only blockade field) ---
  for (const s of owned) {
    if (!s.blockade) continue;
    const sg = siegeProgress(s);
    if (sg) {
      push({ key: `siege:${s.id}`, weight: INBOX_W.siege, tone: "negative", icon: "siege",
        headline: `${systemName(s.id)} — SIEGE in progress`,
        stakes: sg.ripe ? "CRITICAL — a rival colony ship landing now CAPTURES it." : `Falls in ${fmtCountdown(sg.left)} unless you break the blockade or rebuild a Defense Platform.`,
        age: s.blockade.since,
        actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(`siege:${s.id}`)] });
    } else {
      push({ key: `blockade:${s.id}`, weight: INBOX_W.blockade, tone: "negative", icon: "blockade",
        headline: `${systemName(s.id)} — under BLOCKADE`,
        stakes: "Convoys held in & out; production idles. Break it with relief, or build a Defense Platform tier.",
        age: s.blockade.since,
        actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(`blockade:${s.id}`)] });
    }
  }

  // --- ONGOING BATTLE you're in (threat; per-participant BattleView) ---
  for (const b of state.battles) {
    if (!b.own) continue;
    const ownFleet = state.ghosts.find((g) => g.own && b.participants.includes(g.id));
    const acts: InboxAction[] = [{ label: "Open battle", run: () => { closeCheckin(); openOngoingBattlePanel(b.id); }, primary: true }];
    if (ownFleet && net) acts.push({ label: "Withdraw", danger: true, deliveryPos: b.pos, run: () => net!.send({ type: "Withdraw", fleet_id: ownFleet.id }) });
    acts.push(dismissAct(`battle:${b.id}`));
    push({ key: `battle:${b.id}`, weight: INBOX_W.battle, tone: "negative", icon: "battle",
      headline: `Your fleet is ENGAGED near ${locName(b.pos)}`,
      stakes: "A battle is underway — reinforce, or Withdraw to break off (light-delayed).",
      age: b.started_at, actions: acts });
  }

  // --- HOSTILE CONTACTS near an owned asset (threat; the fog-safe ghost feed) ---
  const threatR = galaxy.sensor_range * HOSTILE_CONCERN_MULT;
  const hostiles: InboxItem[] = [];
  for (const g of state.ghosts) {
    if (g.own || g.ally || g.kind !== "raider") continue; // rival strike craft only
    let near: { id: string; d: number; pos: Vec2 } | null = null;
    for (const s of owned) {
      const p = sysPos(s.id);
      if (!p) continue;
      const d = Math.hypot(g.pos.x - p.x, g.pos.y - p.y);
      if (d <= threatR && (!near || d < near.d)) near = { id: s.id, d, pos: p };
    }
    if (!near) continue;
    const speed = Math.hypot(g.vel.x, g.vel.y);
    const closing = speed > 1 && (g.vel.x * (near.pos.x - g.pos.x) + g.vel.y * (near.pos.y - g.pos.y)) > 0;
    const size = g.composition ? `${g.composition.reduce((n, c) => n + c.count, 0)}-ship` : `~${countClassLabel(g.count_class)}`;
    // §pirates: name a neutral-faction pack distinctly (and weight it higher).
    const foe = g.pirate ? "PIRATE" : "Hostile";
    hostiles.push({ key: `hostile:${g.id}:${near.id}`, weight: INBOX_W.hostile + (g.pirate ? 3 : 0), tone: "warn", icon: "warning",
      headline: `${foe} ${size} raider near ${systemName(near.id)}`,
      stakes: closing ? `Closing on ${systemName(near.id)} — ~${fmtCountdown(near.d / speed)} out at its shown speed (a delayed sighting).` : `${Math.round(near.d)} su out, holding — watch it (delayed sighting).`,
      age: g.age,
      confidence: g.composition ? undefined : "size estimate only — the contact is outside your sensor coverage",
      actions: [{ label: "Focus", run: () => inboxFocusSystem(near!.id), primary: true }, dismissAct(`hostile:${g.id}:${near.id}`)] });
  }
  hostiles.sort((a, b) => (b.age ?? 0) - (a.age ?? 0)).slice(0, MAX_HOSTILE_ITEMS).forEach(push);

  // --- SCOUTED PIRATE ENCLAVE (§pirates): an objective you found — clear it. ---
  for (const s of state.systems) {
    const et = s.intel?.enclave_tier ?? 0;
    if (et <= 0) continue;
    const key = `enclave:${s.id}`;
    push({ key, weight: INBOX_W.enclave, tone: "warn", icon: "raider",
      headline: `Pirate enclave at ${systemName(s.id)} — tier ${et}`,
      stakes: "It raids careless trade nearby and grows if ignored. Station a raider fleet on it to destroy the base (yields its plunder).",
      age: s.intel?.observed_at,
      actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(key)] });
  }

  // --- CAPTURE reports (territory flip; per-participant, recent only) ---
  for (const r of state.captureReports) {
    if (now - r.learned_at > REPORT_RECENT_S) continue;
    const key = `capture:${r.id}`;
    push({ key, weight: r.captor ? INBOX_W.captureWon : INBOX_W.captureLost, tone: r.captor ? "info" : "negative", icon: r.captor ? "captured" : "lost",
      headline: r.captor ? `You CAPTURED ${locName(r.pos)}` : `You LOST ${locName(r.pos)}`,
      stakes: r.captor ? "Territory taken — plunder seized." : "A rival colony ship landed at full siege and took the system.",
      age: r.learned_at,
      actions: [{ label: "Open report", run: () => { closeCheckin(); openCapturePanel(r.id); }, primary: true }, dismissAct(key)] });
  }

  // --- GARRISON UNFED — an ally shield YOU host is starving (owner-only) ---
  for (const s of owned) {
    if ((s.ally_garrison_ships ?? 0) > 0 && s.ally_garrison_fed === false) {
      const key = `garrison:${s.id}`;
      push({ key, weight: INBOX_W.garrisonUnfed, tone: "warn", icon: "garrison",
        headline: `Ally garrison at ${systemName(s.id)} is UNFED`,
        stakes: `${s.ally_garrison_ships} allied ship(s) here — their defense is SUSPENDED until you supply Provisions.`,
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Focus", run: () => inboxFocusSystem(s.id) }, dismissAct(key)] });
    }
  }
  // --- your OWN garrison, stationed at an ally, going unfed (owner-only ghost) ---
  for (const g of state.ghosts) {
    if (g.own && g.garrison_host && g.garrison_fed === false) {
      const key = `mygarr:${g.id}`;
      push({ key, weight: INBOX_W.myGarrisonUnfed, tone: "warn", icon: "garrison",
        headline: `Your garrison at ${systemName(g.garrison_host)} is UNFED`,
        stakes: "The host is out of Provisions — this garrison isn't defending. Recall it, or wait for the host to resupply.",
        actions: [{ label: "Inspect", run: () => inboxFocusFleet(g.id), primary: true }, dismissAct(key)] });
    }
  }

  // --- IDLE CAPACITY (owner-only economy fields) ---
  for (const s of owned) {
    if (s.storage_cap > 0 && s.storage_used >= s.storage_cap) {
      const key = `storage:${s.id}`;
      push({ key, weight: INBOX_W.storageFull, tone: "warn", icon: "storage",
        headline: `${systemName(s.id)} — storage FULL (${s.storage_used}/${s.storage_cap})`,
        stakes: "Production idles at the cap. Ship goods out, automate it, or build a Depot (nothing is lost).",
        actions: [{ label: "Ship → hub", icon: "cargo", run: () => { if (net) net.send({ type: "ShipProduction", system_id: s.id }); } }, { label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics }, { label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(key)] });
    }
    if (s.population > 0 && !s.habitat_fed) {
      const key = `habitat:${s.id}`;
      push({ key, weight: INBOX_W.unfedHabitat, tone: "warn", icon: "habitat",
        headline: `${systemName(s.id)} — food ${(s.food_state ?? "rationing").replace("_", " ").toUpperCase()}`,
        stakes: "Workforce slowed, growth paused. Ship Provisions here or set a standing order (nothing is lost, nobody dies).",
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Focus", run: () => inboxFocusSystem(s.id) }, dismissAct(key)] });
    }
    // §node: a held node whose upkeep lapsed — its TACTICAL BONUS is suspended.
    if (s.node?.awakened && !s.node.fed) {
      const key = `node:${s.id}`;
      push({ key, weight: INBOX_W.nodeUnfed, tone: "warn", icon: "unfed",
        headline: `${systemName(s.id)} — ${s.node.title} node UNFED`,
        stakes: `Its bonus is SUSPENDED. ${nodeBonusDesc(s.node.bonus)} Ship its upkeep here or automate it (nothing is lost).`,
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Focus", run: () => inboxFocusSystem(s.id) }, dismissAct(key)] });
    }
    const vol = (s.stockpile ?? []).find((k) => k.commodity === "volatiles")?.units ?? 0;
    if (s.refinery_tier >= 1 && vol === 0) {
      const key = `refinery:${s.id}`;
      push({ key, weight: INBOX_W.dryRefinery, tone: "info", icon: "refinery",
        headline: `${systemName(s.id)} — Refinery idle`,
        stakes: "No Volatiles — Fuel production stopped. Haul some in or automate it.",
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Focus", run: () => inboxFocusSystem(s.id) }, dismissAct(key)] });
    }
    const total = (s.stockpile ?? []).reduce((n, k) => n + k.units, 0);
    const covered = active.some((o) => o.source.kind === "system" && o.source.id === s.id);
    if (total >= IDLE_UNITS && !covered && !(s.storage_cap > 0 && s.storage_used >= s.storage_cap)) {
      const key = `idle:${s.id}`;
      push({ key, weight: INBOX_W.idleStockpile, tone: "info", icon: "market",
        headline: `${systemName(s.id)} — ${total} units idle`,
        stakes: "No standing order ships from here — automate it so it works while you're away.",
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Ship → hub", icon: "cargo", run: () => { if (net) net.send({ type: "ShipProduction", system_id: s.id }); } }, dismissAct(key)] });
    }
    // A DEVELOPED-but-idle system (a claimed frontier with nothing built/building).
    if ((s.slots_total ?? 0) > 0 && (s.slots_used ?? 0) === 0 && (s.builds?.length ?? 0) === 0) {
      const key = `queue:${s.id}`;
      push({ key, weight: INBOX_W.emptyQueue, tone: "info", icon: "build",
        headline: `${systemName(s.id)} — nothing built yet`,
        stakes: `${s.slots_total} development slot(s) free and idle — develop it (Extractor, Depot, Sensor…).`,
        actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(key)] });
    }
  }

  // --- §node: EXOTIC NODES — awakening telegraph + capturable opportunities ---
  {
    const nodeSystems = state.systems.filter((s) => s.node);
    const awakenAt = galaxy.node_awakening_time ?? 0;
    const secsLeft = awakenAt - now;
    // TELEGRAPH: while any node is still dormant, one low-priority countdown card
    // (the "nodes awaken at T" notice, from campaign start through the run-up).
    if (nodeSystems.length && nodeSystems.some((s) => !s.node!.awakened) && secsLeft > 0) {
      const key = "nodes:awakening";
      push({ key, weight: INBOX_W.nodeAwakening, tone: "info", icon: "intel",
        headline: `Exotic nodes awaken in ${fmtCountdown(secsLeft)}`,
        stakes: `${nodeSystems.length} exotic system(s) become capturable tactical prizes. Stage colony ships + fleets now — first arrival claims an unowned node.`,
        actions: [dismissAct(key)] });
    }
    // OPPORTUNITY: an AWAKENED, UNCLAIMED node — claim it before a rival does.
    for (const s of nodeSystems) {
      if (!s.node!.awakened || s.owner) continue; // held (mine/rival) → not an open claim
      const key = `nodeopen:${s.id}`;
      push({ key, weight: INBOX_W.nodeOpportunity, tone: "info", icon: "claim",
        headline: `${systemName(s.id)} — ${s.node!.title} node UNCLAIMED`,
        stakes: `A capturable tactical prize. ${nodeBonusDesc(s.node!.bonus)} Send a colony ship — first arrival claims it.`,
        actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(key)] });
    }
  }

  // --- §explore Part 4: SURVEY REPORTS + the survey-first opportunity ---
  // A fresh survey report (our scout's, or an ally's relayed copy): the geology
  // just ARRIVED for a system we didn't know — the "claim it or skip it?" moment.
  for (const [sid, t] of freshSurveyReports) {
    const key = `surveyrep:${sid}`;
    const dyn = state.systems.find((x) => x.id === sid);
    const info = galaxy.systems.find((x) => x.id === sid);
    if (!dyn?.deposits || !info) continue;
    const summary = dyn.deposits
      .map((d) => `${d.resource} ~${d.richness.toFixed(1)}/s`)
      .join(" · ");
    const unowned = dyn.owner === null;
    push({ key, weight: INBOX_W.surveyReport, tone: "info", icon: "intel",
      headline: `Survey report: ${systemName(sid)} (${info.band.toUpperCase()} band)`,
      age: t,
      stakes: `${summary || "barren"}. Trait UNKNOWN — only a claim reveals it.` +
        (unowned ? " Unclaimed: send a colony ship if it's worth holding." : ""),
      actions: [{ label: "Focus", run: () => inboxFocusSystem(sid), primary: true }, dismissAct(key)] });
  }
  // OPPORTUNITY: Rich-band systems still unsurveyed near your holdings — the
  // survey-first nudge (pure function of the public band + own knowledge).
  {
    const NEAR_SU = 3000;
    const ownedPos = owned
      .map((s) => galaxy.systems.find((x) => x.id === s.id)?.pos)
      .filter((p): p is Vec2 => !!p);
    const richUnsurveyed = state.systems.filter((x) => {
      if (x.deposits != null) return false; // known
      const info = galaxy.systems.find((z) => z.id === x.id);
      if (!info || info.band !== "rich") return false;
      return ownedPos.some((p) => Math.hypot(p.x - info.pos.x, p.y - info.pos.y) <= NEAR_SU);
    });
    if (richUnsurveyed.length) {
      const key = "surveyops";
      const nearest = richUnsurveyed[0];
      push({ key, weight: INBOX_W.surveyOpportunity, tone: "info", icon: "sensor",
        headline: `${richUnsurveyed.length} RICH-band system(s) unsurveyed within ${NEAR_SU} su`,
        stakes: "The spectral read says rich; the composition (and any hidden trait) is a gamble. Send a scout to survey before committing a colony ship — or claim blind and find out.",
        actions: [{ label: "Focus nearest", run: () => inboxFocusSystem(nearest.id), primary: true }, dismissAct(key)] });
    }
  }

  // --- BROKEN standing order (points at a system you no longer hold; an ALLY-aid
  //     destination is valid, so it doesn't count as broken) ---
  const allyIds = new Set(state.systems.filter((x) => x.ally).map((x) => x.id));
  for (const o of active) {
    const refs: string[] = [];
    if (o.source.kind === "system" && !ownedIds.has(o.source.id)) refs.push(systemName(o.source.id));
    if (o.dest.kind === "system" && !ownedIds.has(o.dest.id) && !allyIds.has(o.dest.id)) refs.push(systemName(o.dest.id));
    if (refs.length) {
      const key = `order:${o.id}`;
      push({ key, weight: INBOX_W.brokenOrder, tone: "warn", icon: "doctrine",
        headline: `Standing order #${o.id} targets a system you don't hold`,
        stakes: `Points at ${refs.join(" & ")} — update or clear it.`,
        actions: [{ label: "Open logistics", run: inboxOpenLogistics, primary: true }, dismissAct(key)] });
    }
  }

  // --- CONCLUDED battle you learned of (information; unviewed + recent) ---
  for (const r of state.battleReports) {
    if (state.battleViewed.has(r.id) || now - r.learned_at > REPORT_RECENT_S) continue;
    const key = `report:${r.id}`;
    push({ key, weight: INBOX_W.battleReport, tone: "info", icon: "aftermath",
      headline: `A battle you were in concluded near ${locName(r.pos)}`,
      stakes: "Open the report for losses and the outcome.",
      age: r.learned_at,
      actions: [{ label: "Open results", run: () => { closeCheckin(); openBattlePanel(r.id); }, primary: true }, dismissAct(key)] });
  }

  // --- NO AUTOMATION nudge (only when nothing else needs a decision) ---
  if (owned.length > 0 && active.length === 0 && out.length === 0) {
    push({ key: "noauto", weight: INBOX_W.noAutomation, tone: "info", icon: "doctrine",
      headline: "No standing orders running",
      stakes: `You hold ${owned.length} system${owned.length > 1 ? "s" : ""} — automate supply so it works while you're away.`,
      actions: [{ label: "Open logistics", run: inboxOpenLogistics, primary: true }] });
  }

  out.sort((a, b) => b.weight - a.weight || a.key.localeCompare(b.key));
  return out;
}

// The "all clear" line — the single most check-in-respecting sentence in the
// game: when nothing needs a decision, show the NEXT known timestamp that will.
function nextDecisionLabel(): string {
  const now = liveSimTime();
  let at = Infinity, label = "";
  const consider = (t: number, l: string) => { if (t > now && t < at) { at = t; label = l; } };
  const owned = state.systems.filter((s) => s.owner === state.playerId);
  for (const s of owned) {
    for (const b of s.builds ?? []) consider(b.complete_time, `a build completes at ${systemName(s.id)}`);
    if (s.blockade?.siege_since != null && state.galaxy) consider(s.blockade.siege_since + state.galaxy.siege_secs, `the siege at ${systemName(s.id)} completes`);
  }
  for (const p of state.pendingOrders.values()) consider(p.echo_at, "an order confirms");
  // §explore Part 4: an in-flight survey DWELL — its completion is often the
  // soonest thing worth waiting for (owner-only live progress, honest estimate).
  for (const g of state.ghosts) {
    if (g.own && g.survey_progress != null) {
      consider(now + (1 - g.survey_progress) * SURVEY_SECS_UI, "a survey completes");
    }
  }
  if (!isFinite(at)) return "All quiet — nothing scheduled needs you.";
  return `Nothing needs you until ${doneAtLocal(at)} (${label}).`;
}

// Render one inbox card (headline+icon, age chip, stakes, confidence, action row
// with per-button delivery times for order-issuing verbs).
function inboxCardHtml(it: InboxItem, i: number): string {
  const age = it.age != null ? `<span class="ic-age" title="Information age — how stale this is on your clock (light-delayed).">${agoLabel(it.age)}</span>` : "";
  const stakes = it.stakes ? `<div class="ic-stakes">${it.stakes}</div>` : "";
  const conf = it.confidence ? `<div class="ic-conf">${icon("uncertainty", "sm")} ${esc(it.confidence)}</div>` : "";
  const btns = it.actions.map((a, j) => {
    const d = a.deliveryPos ? commandDelayTo(a.deliveryPos) : null;
    const eta = d != null ? ` <span class="ic-eta" title="When your order's light reaches the target — the echo lifecycle.">arrives ~${arrivalLocal(d)}</span>` : "";
    const cls = `ic-btn${a.primary ? " ic-btn--primary" : ""}${a.danger ? " ic-btn--danger" : ""}`;
    return `<button class="${cls}" data-i="${i}" data-j="${j}">${a.icon ? icon(a.icon, "sm") + " " : ""}${esc(a.label)}${eta}</button>`;
  }).join("");
  return `<div class="inbox-card tone-${it.tone}"><div class="ic-head">${icon(it.icon, "sm")} <b>${it.headline}</b> ${age}</div>${stakes}${conf}<div class="ic-actions">${btns}</div></div>`;
}

// Render the inbox (or the all-clear line) into the check-in panel's primary slot.
function renderInbox(): void {
  const items = computeInbox();
  currentInbox = items;
  const el = $("checkin-attention");
  $("checkin-att-head").textContent = `Decision inbox${items.length ? ` (${items.length})` : ""}`;
  el.innerHTML = items.length
    ? items.map((it, i) => inboxCardHtml(it, i)).join("")
    : `<div class="inbox-clear">${icon("success", "sm")} ${esc(nextDecisionLabel())}</div>`;
}

let checkinBuilt = false;
function buildCheckinPanel(): void {
  if (checkinBuilt) return;
  checkinBuilt = true;
  $("checkin-toggle").addEventListener("click", closeCheckin);
  // Delegated inbox actions — the buttons are rebuilt each render; the live
  // closures live in `currentInbox` (kept fresh by renderInbox each View).
  $("checkin-attention").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("button.ic-btn") as HTMLElement | null;
    if (!b) return;
    const i = Number(b.dataset.i), j = Number(b.dataset.j);
    currentInbox[i]?.actions[j]?.run();
  });
}

function updateCheckinPanel(): void {
  if (!checkinBuilt) return;
  if (renderDeferred("checkin", updateCheckinPanel)) return; // §single-click (the ✕ toggle sits inside)
  // DECISION INBOX first (the primary surface — "what deserves a decision").
  renderInbox();
  // The LOG below (what happened) — the light-correct, offline-buffered timeline.
  const tl = state.timeline;
  const away = tl.filter((e) => e.at_time > state.awaySince);
  const earlier = tl.filter((e) => e.at_time <= state.awaySince);
  const row = (e: TimelineEntry) => `<div class="ci ${e.severity}">${statusIcon(e.severity)} ${e.text} <span class="t">${agoLabel(e.at_time)}</span></div>`;
  const awayHtml = away.length
    ? away.slice().reverse().map(row).join("")
    : `<span class="dim">Nothing new since you were last here.</span>`;
  const earlierHtml = earlier.length
    ? `<div class="ci-sub">Earlier</div>` + earlier.slice().reverse().map(row).join("")
    : "";
  $("checkin-log-head").textContent = `Log${away.length ? ` (${away.length} new)` : ""}`;
  $("checkin-timeline").innerHTML = awayHtml + earlierHtml;
}

// --- Networking ------------------------------------------------------------
let net: Net | null = null;

function join(): void {
  const name = nameInput.value.trim();
  if (!name) {
    joinErr.textContent = "Enter a corporation name.";
    return;
  }
  joinErr.textContent = "";
  joinBtn.disabled = true;
  state.name = name;
  state.link = "connecting";

  net = new Net({
    onOpen: () => {
      net!.send({ type: "Join", name });
      (window as unknown as { __ss: { net?: unknown } }).__ss.net = net; // debug hook
    },
    onMessage: (msg) => {
      switch (msg.type) {
        case "Welcome":
          // Wire protocol check (§FLEETS bumped to 2): warn if the server speaks a
          // newer dialect than this build — the View shape may have drifted.
          if (typeof msg.protocol_version === "number" && msg.protocol_version !== EXPECTED_PROTOCOL_VERSION) {
            console.warn(`protocol mismatch: server v${msg.protocol_version}, client expects v${EXPECTED_PROTOCOL_VERSION} — a refresh may be needed`);
          }
          state.playerId = msg.player_id;
          state.name = msg.name;
          state.tickHz = msg.tick_hz;
          state.tick = msg.tick;
          state.simTime = msg.sim_time;
          state.galaxy = msg.galaxy;
          state.link = "online";
          // Swap from the join screen to the galaxy view.
          joinScreen.style.display = "none";
          hud.style.display = "flex";
          $("readout").style.display = "block";
          $("legend").style.display = "block";
          $("zoom-controls").style.display = "flex";
          // Wire the rail (System/Logistics/Doctrine), the navbar Market overlay,
          // and the navbar Log. The rail + Market stay CLOSED on join so the map is
          // uncluttered — opened by clicking a system, S/O/F, or the navbar/M.
          buildRail();
          buildSystemTab();
          buildMarketPanel();
          buildStandingPanel();
          buildDoctrinePanel();
          updateDoctrinePanel();
          setRailTab("system");
          // Fresh session: re-latch the "while you were away" boundary from the
          // next Timeline digest, and open the check-in panel for the welcome-back.
          state.awaySet = false;
          buildCheckinPanel();
          openCheckin();
          void startRenderer();
          break;
        case "View":
          state.tick = msg.tick;
          state.simTime = msg.sim_time;
          state.commandCenter = msg.command_center;
          state.anchors = msg.anchors;
          state.systems = msg.systems;
          state.ghosts = msg.ghosts;
          state.market = msg.market;
          state.wallet = msg.wallet;
          state.standingOrders = msg.standing_orders;
          state.doctrine = msg.doctrine;
          state.battles = msg.battles;
          state.battleReports = msg.battle_reports;
          state.captureReports = msg.capture_reports;
          state.syndicate = msg.syndicate ?? null;
          state.syndicateInvites = msg.syndicate_invites ?? [];
          state.rankings = msg.rankings ?? [];
          noteSurveyReports(msg.sim_time); // §explore Part 4: survey-report cards
          notifyNewBattles(msg.battles);
          syncOrderLifecycles(msg.pending_orders, msg.sim_time);
          // Accumulate observed prices every View (fog-safe history for the
          // sparklines), even when the Market tab is closed.
          recordPriceHistory();
          // Refresh only the currently-visible rail tab — hidden tabs don't churn
          // (they re-render on show via setRailTab). Each updater also guards itself.
          if ($("rail").classList.contains("is-open")) {
            if (railTab === "system") updateSystemTab();
            else if (railTab === "logistics") updateStandingPanel();
            else if (railTab === "doctrine") updateDoctrinePanel();
            else if (railTab === "rankings") updateRankingsPanel();
          }
          // The selected-ship panel keeps the information AGE ticking (and handles a
          // contact passing out of view) while it's open.
          if ($("ship-panel").classList.contains("is-open")) updateShipPanel();
          // §syndicates: refresh the alliance roster/invites if the panel is open
          // (guarded by a signature so a half-typed name survives).
          if ($("syndicate-panel").classList.contains("is-open")) updateSyndicatePanel();
          // §management-home: inside the System View, refresh the management
          // column + the structure markers (a cached no-op unless tiers changed).
          updateSysviewDynamic();
          // §one-battle-one-icon: keep an open ongoing-battle panel live (elapsed,
          // echo countdowns, running composition; auto-closes when it concludes).
          if (openOngoingBattleId !== null && $("battle-panel").classList.contains("is-open")) updateOngoingBattlePanel();
          // The Market is a navbar overlay now — refresh it when open.
          if ($("market").classList.contains("is-open")) updateMarket();
          updateCheckinPanel(); // the check-in modal; guards itself, refreshes ages
          // Light-respecting "corps in view": distinct owners we can actually
          // see (self + rivals whose light has arrived). Never a raw count.
          state.corpsInView = new Set(msg.ghosts.map((g) => g.owner)).size;
          state.lastViewWallMs = performance.now();
          state.link = "online";
          break;
        case "CommandSignal": {
          // Your order is crossing space to your ship (the violet comet); you'll
          // see the ship react on the map when its light arrives. Replace any
          // in-flight signal for the same ship (a newer order supersedes).
          state.commandSignals = state.commandSignals.filter((s) => s.shipId !== msg.ship_id);
          state.commandSignals.push({
            shipId: msg.ship_id,
            depart: msg.depart_time,
            arrive: msg.arrive_time,
            pOut: 0,
          });
          break;
        }
        case "Report": {
          // The server delivers this exactly when the destruction's light reaches
          // THIS player's command center — the same moment the doomed ghost
          // vanishes on their map at the kill site. So we just NOTIFY now: no
          // travelling ring (the map already IS the inbound feed). Two players at
          // different distances are notified at different times, each synced to
          // when they see it (§6).
          addReport(msg.report);
          // The raid is over — drop its intercept estimate.
          delete state.raids[msg.report.attacker_ship];
          break;
        }
        case "EngagementEstimate":
          showEngagementEstimate(msg);
          break;
        case "Timeline":
          state.timeline = msg.entries;
          // Latch the "while you were away" boundary from the FIRST digest of the
          // session (the connect message); live re-sends keep that boundary so the
          // away-section doesn't empty out mid-session.
          if (!state.awaySet) {
            state.awaySince = msg.away_since;
            state.awaySet = true;
          }
          updateCheckinPanel();
          break;
        case "Trade":
          addTradeNews(msg.trade);
          break;
        case "Error":
          joinErr.textContent = msg.message;
          break;
      }
      setHud();
    },
    onClose: () => {
      state.link = "offline";
      joinBtn.disabled = false;
      setHud();
    },
    onError: () => {
      state.link = "offline";
      joinErr.textContent = `Could not reach server at ${net?.url ?? ""}.`;
      joinBtn.disabled = false;
      setHud();
    },
  });
  net.connect();
}

joinBtn.addEventListener("click", join);
nameInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") join();
});
nameInput.focus();
setHud();
