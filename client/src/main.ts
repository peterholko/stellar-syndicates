// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { countClassLabel, formatId, type AcademyRow, type AssignmentView, type BattleRecordView, type BattleReportView, type BattleView, type BodyView, type BuildState, type Commodity, type CompCount, type CountClass, type Deposit, type EngagementPosture, type EntityId, type FleetDoctrine, type GhostView, type ModuleKind, type PendingOrderView, type ProgrammeView, type RaidOutcome, type RecordCount, type RoundNoteView, type RoundRecordView, type ShipKind, type Side, type SideRecordView, type StandingEndpoint, type StandingOrder, type StandingTrigger, type StockSlot, type SystemInfo, type SystemStateView, type TimelineEntry, type TradeEvent, type Vec2 } from "./protocol";
import { starConceptUrl, starTypeFor } from "./stars";
import { type SystemBodyDetail } from "./systemview";
import { badgeChip, chip, icon, type IconKey, type IconSize, label } from "./icons";

const state: ViewState = initialState();

// --- DOM handles -----------------------------------------------------------
// Wire protocol version this build speaks — kept in sync with the server's
// PROTOCOL_VERSION. (v6 = §research: the per-player view gained the Programme
// Boards research state; see crates/server/src/protocol.rs.)
const EXPECTED_PROTOCOL_VERSION = 6;
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
(window as unknown as { __ss: unknown }).__ss = { state, renderer, openBattleViewer };

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
const svgIcon = (slug: string, size: IconSize = "sm", cls = "") =>
  `<img class="icon icon--${size}${cls ? ` ${cls}` : ""}" src="/art/ui_icons/svg/${slug}.svg" alt="" />`;

// The ONE raster-icon path helper: a downscaled PNG under /art/ui_icons/<category>/
// (transparent-background art — the commodity resource icons and the research field
// emblems), or the `glyph` fallback when `slug` is empty, so a missing/unknown icon
// degrades to a legible symbol instead of a broken <img>. Both commodity and
// research field icons go through here — no third copy of the path logic.
function uiIcon(category: "resource" | "research", slug: string | undefined, glyph: string, title = "", cls = ""): string {
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
// old ore art); the seven new industrial goods fall back to tinted glyphs until
// their art lands.
const COMMODITY_ART: Partial<Record<Commodity, string>> = {
  fuel: "fuel", metallic_ore: "ore", alloys: "alloys", provisions: "provisions", volatiles: "volatiles",
  // The six industrial goods — the framed-tile set sliced from the extended sheet
  // (file names match the wire slugs). Only BIOMASS is still on its glyph.
  rare_elements: "rare_elements", silicates: "silicates", electronics: "electronics",
  polymers: "polymers", machinery: "machinery", armaments: "armaments",
};
const COMMODITY_GLYPH: Record<Commodity, string> = {
  metallic_ore: "\u26cf", rare_elements: "\u2728", silicates: "\u25a6", volatiles: "\u2744", biomass: "\ud83c\udf3f",
  alloys: "\ud83d\udd29", electronics: "\ud83d\udda5", polymers: "\ud83e\uddea", fuel: "\u26fd", provisions: "\ud83c\udf5e",
  machinery: "\u2699", armaments: "\ud83d\udd2b",
};
// A commodity icon is by definition a resource — the shared helper with the
// `--icon-resource` token + a glyph fallback for goods whose art hasn't landed.
const commodityIcon = (c: Commodity, _size: IconSize = "md") =>
  uiIcon("resource", COMMODITY_ART[c], COMMODITY_GLYPH[c], label(c));

// §research R6: the six FIELD emblems (hexagonal art, 256px masters + 64px `-sm`
// variants under /art/ui_icons/research/), used wherever a research field is named.
// `size` picks the asset (sm → the light 64px variant) and the CSS token
// (rf-ic--{sm|md|lg|xl} ≈ 26 / 44 / 60 / 72 px); glyph fallback degrades gracefully.
const RESEARCH_FIELDS = new Set(["propulsion", "materials", "computation", "weapons", "hulls", "life"]);
const RESEARCH_GLYPH: Record<string, string> = {
  propulsion: "🚀", materials: "⚙️", computation: "📡", weapons: "🎯", hulls: "🛡️", life: "🌱",
};
function researchIcon(field: string, size: "sm" | "md" | "lg" | "xl" = "md"): string {
  const has = RESEARCH_FIELDS.has(field);
  const slug = has ? (size === "sm" ? `${field}-sm` : field) : undefined;
  return uiIcon("research", slug, RESEARCH_GLYPH[field] ?? "◆", FIELD_TITLE[field] ?? field, `rf-ic rf-ic--${size}`);
}

// Status icon by timeline severity (the native Status set).
const STATUS_SLUG: Record<TimelineEntry["severity"], string> = {
  good: "status-success",
  bad: "status-warning-threat",
  warn: "status-warning-threat",
  info: "status-info",
};
const statusIcon = (sev: TimelineEntry["severity"], size: IconSize = "sm") => svgIcon(STATUS_SLUG[sev], size);

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
  $("nav-research").addEventListener("click", toggleResearch);
  $("nav-syndicate").addEventListener("click", toggleSyndicate);
  $("nav-log").addEventListener("click", toggleCheckin);
  $("market-close").addEventListener("click", closeMarket);
  // §research R6: delegated actions inside the Programme Boards panel — close,
  // add an open node to the queue, and reorder/remove queued programmes.
  $("research-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    const closeBtn = t.closest("[data-rp='close']");
    if (closeBtn) { closeResearch(); return; }
    if (!net) return;
    // Reorder / remove a queued programme.
    const btn = t.closest("button") as HTMLButtonElement | null;
    if (btn && (btn.dataset.qup || btn.dataset.qdown || btn.dataset.qrm)) {
      const ids = researchQueueIds();
      if (btn.dataset.qrm !== undefined) {
        ids.splice(Number(btn.dataset.qrm), 1);
      } else if (btn.dataset.qup !== undefined) {
        const i = Number(btn.dataset.qup);
        if (i > 0) [ids[i - 1], ids[i]] = [ids[i], ids[i - 1]];
      } else if (btn.dataset.qdown !== undefined) {
        const i = Number(btn.dataset.qdown);
        if (i < ids.length - 1) [ids[i + 1], ids[i]] = [ids[i], ids[i + 1]];
      }
      sendResearchQueue(ids);
      return;
    }
    // Click an AVAILABLE node → append it to the queue.
    const node = t.closest("[data-rid]") as HTMLElement | null;
    if (node?.dataset.rid) {
      const ids = researchQueueIds();
      if (!ids.includes(node.dataset.rid)) ids.push(node.dataset.rid);
      sendResearchQueue(ids);
    }
  });
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
    } else if (act === "refitmod") {
      // §modules Part B4: toggle a module into the composed REFIT target (≤2).
      const m = (b as HTMLElement).dataset.mod as ModuleKind | undefined;
      if (m) {
        const i = pendingRefit.indexOf(m);
        if (i >= 0) pendingRefit.splice(i, 1);
        else if (pendingRefit.length < 2) pendingRefit.push(m);
        updateShipPanel();
      }
    } else if (act === "refit" && state.selectedShipId && net) {
      // §modules Part B4: refit the named (kind, from) stack to the composed
      // target (clamped to the hull's slots). The server enforces the docked-yard
      // + ledger-delta gate; a soft reject leaves the fleet unchanged.
      const el = b as HTMLElement;
      const ship = el.dataset.kind as ShipKind | undefined;
      const n = Number(el.dataset.n) || 0;
      if (ship && n > 0) {
        const from = el.dataset.from ? el.dataset.from.split(",").filter(Boolean) as ModuleKind[] : [];
        const to = pendingRefit.slice(0, MODULE_SLOTS[ship] ?? 0);
        net.send({ type: "RefitShips", fleet_id: state.selectedShipId, ship, from, to, n });
        $("readout").innerHTML = `Refit ordered: <b>${n}× ${esc(shipKindLabel(ship))}</b> → ${to.length ? to.map((m) => MODULE_GLYPH[m]).join(" ") : "stock"} <span class="dim">(at a docked Shipyard; needs the added modules in the system ledger).</span>`;
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
      ? `<div class="sp-cargo">${commodityIcon(g.cargo.commodity, "md")} <b>${fmt(g.cargo.units)}</b> ${esc(label(g.cargo.commodity))}</div>`
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
  parts.push(refitSection(g));
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
      ? `<div class="sp-line">${chip(g.cargo.commodity as IconKey, `${fmt(g.cargo.units)} ${esc(label(g.cargo.commodity))}`, "Cargo — visible because this convoy is inside your sensor coverage.")}</div>`
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
    `<h2>${svgIcon(g.kind === "convoy" ? "concept-convoy" : "concept-fleet", "md")} ${esc(shipKindLabel(g.kind))}</h2></div><div class="panel-title__right">${ownTag}</div></div>` +
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
  setMarketTab(marketTab); // §market-ux: reopen on the last tab (also updates)
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

// --- §research R6: the Programme Boards panel (top-navbar destination) ----------
// Owner-only (the View carries research only for the viewer's own syndicate). The
// whole 108-node tree as six Y-ladder boards; an active banner with the live rate
// + ETA + per-Academy contribution table (shown math); a queue strip you reorder
// (→ SetResearchQueue). Re-rendered only when something CHANGES (a coarse
// signature that includes the progress bucket, so the bar animates ~1 Hz).
const FIELD_ORDER = ["propulsion", "materials", "computation", "weapons", "hulls", "life"];
const FIELD_TITLE: Record<string, string> = {
  propulsion: "Propulsion", materials: "Materials", computation: "Computation",
  weapons: "Weapons", hulls: "Hulls", life: "Life",
};
const SCHOOL_TITLE: Record<string, string> = {
  line_haul: "Line Haul", expedition: "Expedition", deep_crust: "Deep Crust", foundry: "Foundry",
  watch: "Watch", shadow: "Shadow", strike: "Strike", countermeasures: "Countermeasures",
  line: "Line", corsair: "Corsair", growth: "Growth", talent: "Talent",
};
const ROMAN = ["", "I", "II", "III", "IV", "V"];
let lastResearchSig = "";

function fmtEta(secs: number): string {
  if (!isFinite(secs) || secs <= 0) return "—";
  const h = secs / 3600;
  if (h < 1) return `${Math.max(1, Math.round(secs / 60))}m`;
  if (h < 48) return `${h.toFixed(1)}h`;
  return `${(h / 24).toFixed(1)}d`;
}

function openResearch(): void {
  $("research-panel").classList.add("is-open");
  $("nav-research").classList.add("is-active");
  lastResearchSig = "";
  updateResearchPanel();
}
function closeResearch(): void {
  $("research-panel").classList.remove("is-open");
  $("nav-research").classList.remove("is-active");
}
function toggleResearch(): void {
  if ($("research-panel").classList.contains("is-open")) closeResearch();
  else openResearch();
}

// The full ordered queue the player controls = [active, ...queue-ahead]. Sending
// it back as SetResearchQueue re-promotes the front to active (the sim's rule).
function researchQueueIds(): string[] {
  const r = state.research;
  if (!r) return [];
  return r.active ? [r.active.id, ...r.queue] : [...r.queue];
}
function sendResearchQueue(ids: string[]): void {
  if (net) net.send({ type: "SetResearchQueue", queue: ids });
}

function researchNode(p: ProgrammeView, pos: number | null): string {
  const num = pos !== null ? `<span class="n">${pos + 1}</span> ` : "";
  const add = p.state === "available" ? ` data-rid="${esc(p.id)}"` : "";
  return `<div class="rp-node is-${p.state}"${add} title="${esc(p.blurb)}">` +
    `<div class="nm">${num}${esc(p.name)}</div><div class="bl">${esc(p.blurb)}</div></div>`;
}

function researchBoard(fieldSlug: string, progs: ProgrammeView[], queue: string[]): string {
  const qpos = (id: string): number | null => {
    const i = queue.indexOf(id);
    return i >= 0 ? i : null;
  };
  const at = (school: string | null, tier: number) =>
    progs.filter((p) => (p.school ?? null) === school && p.tier === tier);
  // A tier-group: its Roman label, a gate bar if any node is sealed, then nodes.
  const group = (school: string | null, tier: number): string => {
    const nodes = at(school, tier);
    if (!nodes.length) return "";
    const sealed = nodes.find((n) => n.gate);
    let gate = "";
    if (sealed?.gate) {
      const g = sealed.gate;
      const pct = Math.max(0, Math.min(100, (g.current / Math.max(1e-9, g.threshold)) * 100));
      gate = `<div class="rp-gate">${esc(g.label)} ${Math.floor(g.current)} / ${Math.round(g.threshold)}</div>` +
        `<div class="rp-gatebar"><i style="width:${pct}%"></i></div>`;
    }
    const cards = nodes.map((n) => researchNode(n, qpos(n.id))).join("");
    return `<div class="rp-tier"><div class="lbl">Tier ${ROMAN[tier]}</div>${gate}${cards}</div>`;
  };
  const schools = Array.from(new Set(progs.filter((p) => p.school).map((p) => p.school as string)));
  let inner = group(null, 1) + group(null, 2);
  for (const s of schools) {
    inner += `<div class="lbl" style="color:#8fd3dd;margin-top:2px">⑂ ${esc(SCHOOL_TITLE[s] ?? s)}</div>`;
    inner += group(s, 3) + group(s, 4) + group(s, 5);
  }
  return `<div class="rp-board"><h4>${researchIcon(fieldSlug, "lg")}<span>${esc(FIELD_TITLE[fieldSlug] ?? fieldSlug)}</span></h4>${inner}</div>`;
}

function updateResearchPanel(): void {
  const el = $("research-panel");
  if (!el.classList.contains("is-open")) return;
  const r = state.research;
  const sig = r
    ? JSON.stringify([
        r.programmes.map((p) => p.state),
        r.queue, r.active?.id, r.stalled,
        r.active ? Math.round((r.active.progress / Math.max(1, r.active.cost)) * 200) : 0,
        Math.round(r.rate * 100),
        r.academies.map((a) => [a.rate.toFixed(2), a.supplied]),
        r.programmes.filter((p) => p.state === "locked" && p.gate).map((p) => Math.round((p.gate!.current / Math.max(1e-9, p.gate!.threshold)) * 40)),
      ])
    : "none";
  if (sig === lastResearchSig && el.innerHTML) return;
  lastResearchSig = sig;

  let body = "";
  if (!r) {
    body = `<div class="rp-note">Research is a <b>syndicate</b> institution — found or join a syndicate (🤝 Syndicate) to open the Programme Boards. Every staffed <b>Academy</b> in the syndicate then powers one shared programme at a time.</div>`;
  } else {
    // Active banner.
    if (r.active) {
      const a = r.active;
      const pct = Math.max(0, Math.min(100, (a.progress / Math.max(1e-9, a.cost)) * 100));
      const eta = a.eta_secs != null ? `ETA ${fmtEta(a.eta_secs)}` : (r.stalled ? `<span class="rp-stalled">stalled</span>` : `no supply`);
      const acadRows = r.academies.length
        ? `<div class="rp-acad"><div class="hd">Academy</div><div class="hd">tier</div><div class="hd">rate/s</div>` +
          r.academies.map((x: AcademyRow) =>
            `<div class="${x.supplied ? "" : "amber"}">${esc(x.system)}${x.supplied ? "" : " ⚠"}</div>` +
            `<div>T${x.tier}</div><div>${x.rate.toFixed(2)}</div>`).join("") +
          `</div>`
        : `<div class="rp-acad"><div class="amber">No staffed Academy is contributing — post crew to an Academy.</div></div>`;
      const aField = r.programmes.find((p) => p.id === a.id)?.field ?? "";
      body += `<div class="rp-active">${researchIcon(aField, "xl")}<div class="rp-a-main"><div class="rp-a-top"><span class="rp-a-name">${esc(a.name)}</span>` +
        `<span class="rp-a-eta">${esc(String(Math.round(a.progress))) } / ${Math.round(a.cost)}·s · ${eta} · ${r.rate.toFixed(2)}/s</span></div>` +
        `<div class="rp-bar"><i style="width:${pct}%"></i></div>${acadRows}</div></div>`;
    } else {
      body += `<div class="rp-idle">No active programme. Pick any open node below to queue it — the front of the queue starts accruing.</div>`;
    }
    // Queue strip.
    const q = researchQueueIds();
    const chips = q.map((id, i) => {
      const p = r.programmes.find((x) => x.id === id);
      const nm = p ? p.name : id;
      return `<span class="rp-q-chip"><span class="n">${i + 1}</span>${researchIcon(p?.field ?? "", "sm")}${esc(nm)}` +
        `<button data-qup="${i}" title="Earlier">▲</button><button data-qdown="${i}" title="Later">▼</button>` +
        `<button data-qrm="${i}" title="Remove">✕</button></span>`;
    }).join("");
    body += `<div class="rp-queue"><span class="rp-q-label">Queue</span>${q.length ? chips : `<span class="rp-q-empty">empty — click an open programme to add it</span>`}</div>`;
    // Six boards.
    const boards = FIELD_ORDER.map((f) => researchBoard(f, r.programmes.filter((p) => p.field === f), q)).join("");
    body += `<div class="rp-boards">${boards}</div>`;
    body += `<div class="rp-note">Tech sheets are private — nothing here leaks to rivals. Completing a programme applies its effect instantly, galaxy-wide.</div>`;
  }
  // Refresh ONLY the scroll body, keeping the .rp-body element itself across
  // ticks so its scrollTop survives. The active programme's progress bar
  // advances the render signature almost every tick; replacing the whole panel
  // recreated .rp-body each time and snapped the list back to the top. Building
  // the head+body shell once and updating just the body's children keeps the
  // scroll position put.
  let bodyEl = el.querySelector<HTMLElement>(".rp-body");
  if (!bodyEl) {
    el.innerHTML = `<div class="rp-head"><b>🔬 RESEARCH — PROGRAMME BOARDS</b><button class="rp-close" data-rp="close" title="Close">✕</button></div><div class="rp-body"></div>`;
    bodyEl = el.querySelector<HTMLElement>(".rp-body")!;
  }
  bodyEl.innerHTML = body;
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
    `<button class="act act--primary" data-act="market">${svgIcon("concept-market-exchange", "sm")} Open Market</button>` +
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

// The star system under a screen point (for double-click / deep-zoom enter).
// Each star counts within its OWN rendered disk (so a deep-zoom giant's rim is
// enterable) or within `slack` of its center (so small stars stay easy to hit);
// the NEAREST CENTER wins among qualifiers — aiming at a small star always
// beats a visually larger neighbor whose disk merely blankets the same pixel.
function systemUnderCursor(sx: number, sy: number, slack = 22): SystemInfo | null {
  if (!state.galaxy) return null;
  let best: SystemInfo | null = null;
  let bestD = Infinity;
  for (const sys of state.galaxy.systems) {
    const s = renderer.worldToScreen(sys.pos);
    const d = Math.hypot(s.x - sx, s.y - sy);
    if (d < Math.max(slack, renderer.systemHitRadius(sys)) && d < bestD) { bestD = d; best = sys; }
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
  renderer.enterSystemView(sys, state.systems.find((s) => s.id === sys.id)?.bodies ?? []);
  state.selectedSystemId = sys.id; // keep the galaxy selection in sync (rail shows it)
  showBreadcrumb(sys.name);
  closePlanetPanel();
  closeRail(); // the management column takes the right dock inside the view
  // §management-home: feed the scene's structure markers + open the management
  // column (owned systems only — both no-op into scenery for rival/unclaimed).
  pushSystemDynamic(sys.id);
  updateSysviewManage();
  const mine = state.systems.find((s) => s.id === sys.id)?.owner === state.playerId;
  readout().innerHTML = mine
    ? `<b>${esc(sys.name)}</b> — your system. <span class="dim">The right column is the colony at a glance; CLICK A BODY (or a chip) to manage it — build, staff, ship. Esc closes panels, then returns to the galaxy. ` +
      `Deposits, structures &amp; crews live ON their bodies; the stockpile &amp; workforce pool system-wide.</span>`
    : `<b>${esc(sys.name)}</b> — schematic system view. <span class="dim">Click a planet for details · Esc / Back / zoom out returns to the galaxy. ` +
      `Geography is public — every corporation sees these worlds; a rival's development is not.</span>`;
}
function exitSystem(): void {
  if (renderer.viewMode.type !== "system") return;
  renderer.exitSystemView();
  $("breadcrumb").classList.remove("is-open");
  closePlanetPanel();
  closeSysviewManage();
  renderer.setSystemDynamic([], [], true);
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
/// §bodies: feed the scene its per-body dynamic layer straight from the wire —
/// the roster (public geography; a rival's bodies carry no structures, so fog
/// needs no client gate), the build queue (owner-only on the wire), the food.
function pushSystemDynamic(sid: string): void {
  const dyn = state.systems.find((s) => s.id === sid);
  renderer.setSystemDynamic(
    dyn?.bodies ?? [],
    (dyn?.builds ?? []).map((j) => ({ key: j.key, body_id: j.body_id })),
    dyn?.habitat_fed ?? true,
  );
}
/// Per-View refresh while inside the System View: feed the scene's markers (a
/// cached no-op unless a build completed) and re-render the management column.
function updateSysviewDynamic(): void {
  const sid = viewedSystemId();
  if (!sid) return;
  pushSystemDynamic(sid);
  updateSysviewManage();
  // §body-management: the open body panel is live — crew counts, queue bars,
  // and afford states track the Views (single-click-guarded like every panel).
  refreshOpenBodyPanel();
  // §build-panel: its rows/costs/queued-note track the same Views.
  refreshBuildPanel();
}
function buildSysviewManage(): void {
  if (sysviewManageBuilt) return;
  sysviewManageBuilt = true;
  $("svm-close").addEventListener("click", exitSystem);
  // ONE delegated listener on the static panel shell (only #svm-body's innerHTML
  // is ever rewritten), so build clicks can never lose their handler.
  // §body-management: the summary is PURE DATA — its only clickables are
  // NAVIGATION chips (data-body) that open a body's panel; every command verb
  // (build / crew / ship / auto-supply) lives on the body panels now.
  $("sysview-manage").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-body]") as HTMLElement | null;
    if (el?.dataset.body) openBodyPanelById(el.dataset.body);
  });
}
/// §body-management: chip → the body's panel, with a sprite pulse so the eye
/// lands on the right dot ("HERE is the thing you tapped").
function openBodyPanelById(bodyId: string): void {
  const d = renderer.systemBodyDetail(bodyId);
  if (!d) return;
  renderer.pulseSystemBody(bodyId);
  openPlanetPanel(d);
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
  $("svm-title").textContent = `${sys.name} System`;
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
    ? `<div class="deps-head">${icon("storage", "sm", "Stockpile")} Stockpile Capacity ${fmt(used)} / ${fmt(cap)}</div>` +
      `<div class="storage-row">${bar(Math.min(100, (used / cap) * 100), storageFull ? "is-warn" : "")}` +
      (storageFull ? ` ${badgeChip("storage", "full", "warn", "Storage full — production idles at the cap. Ship goods out or build a Depot to raise it (reserves aren't wasted; accrual resumes when goods ship).")}` : "") +
      `</div>`
    : "";
  // §body-management: COLONY VITALS — population, food rung, workforce, and the
  // Provisions upkeep (§system-reorg: the upkeep moved up here from the
  // production readout; the population eats provisions_per_million_per_s · pop).
  const wf = dyn.workforce;
  const foodState = label(dyn.food_state ?? "well_supplied");
  const popM = dyn.population ?? 0;
  const upkeepRate = (state.galaxy?.provisions_per_million_per_s ?? 0.06) * popM;
  const vitalCells = [
    stat("Population", `${popM.toFixed(1)}M`),
    stat("Food", foodState, dyn.habitat_fed ? "" : "is-warn"),
    stat("Workforce", wf ? `${Math.min(wf.posted, wf.units)}/${wf.posted}` : "—", wf && wf.posted > wf.units ? "is-warn" : ""),
  ];
  if (popM > 0)
    vitalCells.push(stat("Upkeep", `−${upkeepRate.toFixed(2)} ${commodityIcon("provisions", "sm")}/s`, dyn.habitat_fed ? "" : "is-warn"));
  const vitals = popM > 0 || wf ? statStrip(vitalCells) : "";
  // §body-management: the three SLOT POOLS — a system fact, so it reads here
  // (the per-pool gating itself lives with the build rows on the body panels).
  const pools = poolUsage(dyn);
  const poolStrip = `<div class="mhint" title="Slots are PER BODY — one per distinct structure on its body; deepening a built tier never needs a slot. Totals here sum the roster.">` +
    (["resource", "industrial", "infrastructure"] as const)
      .map((k) => `${k} ${pools[k].used}/${pools[k].total}`)
      .join(" · ") + `</div>`;
  // §system-reorg: the ROSTER — one row per body (public geography). The row is a
  // NAVIGATION button (opens that body's panel to build/staff/ship) followed by
  // the planet's CONTRIBUTION: the net output of its staffed lines, per commodity
  // (+x/s <icon>). Buildings no longer list here — they live on the body panel.
  const bodies = dyn.bodies ?? [];
  const outByBody = new Map<number, Map<Commodity, number>>();
  for (const a of dyn.assignments ?? []) {
    let m = outByBody.get(a.body_id);
    if (!m) { m = new Map(); outByBody.set(a.body_id, m); }
    for (const [c, r] of a.outputs) if (r > 0.001) m.set(c, (m.get(c) ?? 0) + r);
  }
  const contribFor = (b: BodyView): string => {
    const m = outByBody.get(b.id);
    if (!m || !m.size) return `<span class="dim">undeveloped</span>`;
    return [...m.entries()]
      .sort((x, y) => y[1] - x[1])
      .map(([c, r]) => `<span class="dev-contrib" title="${esc(b.name)} contributes +${r.toFixed(2)} ${esc(label(c))}/s to the colony">+${r.toFixed(2)}/s ${commodityIcon(c, "sm")}</span>`)
      .join(" ");
  };
  const devs = bodies.length
    ? bodies.map((b) => {
        const pop = b.population > 0 ? ` <span class="dim">${b.population.toFixed(1)}M</span>` : "";
        return `<div class="devs-row"><button class="dev act" data-body="${b.id}" title="Open ${esc(b.name)} — build, staff, ship from its panel">${esc(b.name)}</button>${pop} ${contribFor(b)}</div>`;
      }).join("")
    : `<div class="mhint">No bodies rostered yet.</div>`;
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
  // §body-management: NO action buttons here — shipping/auto-supply live on
  // the Depot's station panel, builds/crews on their anchor bodies.
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
  $("svm-eyebrow").textContent = blockaded ? "UNDER BLOCKADE" : "";
  const queue = buildQueueRows(sid, dyn, { nav: true });
  // §system-reorg: production + stockpile total up top, then the planet roster
  // (with per-body contribution), then colony vitals (pop/food/workforce/upkeep),
  // slot pools, garrison, and the build queue. Each is its own titled section,
  // separated by a divider (empty sections drop out, so no dangling rules).
  const sections = [
    blockadeBanner,
    storageBar + productionReadout(dyn), // Stockpile Capacity + bar, then the commodity rows
    `<div class="deps-head">Planets</div>` + devs, // the planet roster under its own header
    vitals + poolStrip, // colony vitals + slot pools
    garrisonHost,
    queue,
  ].filter((s) => s.trim() !== "");
  $("svm-body").innerHTML = sections.join(`<div class="svm-div"></div>`);
}

let planetPanelBuilt = false;
function buildPlanetPanel(): void {
  if (planetPanelBuilt) return;
  planetPanelBuilt = true;
  // §body-management: the body panel is THE action surface — every branch
  // sends the SAME system-level command the old summary buttons sent (the
  // anchor is a lens, not an address; nothing here is per-planet on the wire).
  $("planet-panel").addEventListener("click", (e) => {
    if ((e.target as HTMLElement).closest("[data-act='close']")) { closePlanetPanel(); return; }
    const el = (e.target as HTMLElement).closest("[data-build],[data-crew],[data-action],[data-fit]") as HTMLElement | null;
    const sid = viewedSystemId();
    if (!el || !sid || !net) return;
    if (el.dataset.fit) {
      // §modules Part B4: toggle a module into the composed fit (max 2 slots).
      const m = el.dataset.fit as ModuleKind;
      const i = pendingFit.indexOf(m);
      if (i >= 0) pendingFit.splice(i, 1);
      else if (pendingFit.length < 2) pendingFit.push(m);
      refreshOpenBodyPanel();
      return;
    }
    if (el.dataset.crew) {
      sendCrew(sid, el.dataset.crew);
      refreshOpenBodyPanel();
      updateSysviewManage();
      return;
    }
    if (el.dataset.build) {
      dispatchBuildKey(el.dataset.build, sid, openBodyDetail ? Number(openBodyDetail.id) : undefined);
      refreshOpenBodyPanel(); // the queue row appears on the next View push
      updateSysviewManage();
      return;
    }
    switch (el.dataset.action) {
      case "open-builder":
        // §build-panel: open the dedicated builder for THIS body (a sibling panel
        // to the right — nothing is sent until "Queue build").
        if (openBodyDetail) openBuildPanel(openBodyDetail.id);
        break;
      case "open-shipyard":
        // §build-ship-panel: open the ship builder for this shipyard body (its
        // sibling — opening it closes the structure builder, and vice-versa).
        if (openBodyDetail) openShipPanel(openBodyDetail.id);
        break;
      case "ship": {
        const manifest = shippableStock(state.systems.find((s) => s.id === sid));
        if (manifest.length) net.send({ type: "ShipProduction", system_id: sid });
        refreshOpenBodyPanel();
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
    }
  });
}
/// §body-management: the OPEN body panel (its visual detail), for live
/// re-renders as Views land — crew counts, queue bars, afford states.
let openBodyDetail: SystemBodyDetail | null = null;
function refreshOpenBodyPanel(): void {
  if (!openBodyDetail || !$("planet-panel").classList.contains("is-open")) return;
  if (renderDeferred("planet-panel", refreshOpenBodyPanel)) return; // §single-click
  openPlanetPanel(openBodyDetail);
}
function closePlanetPanel(): void {
  openBodyDetail = null;
  closeBuildPanel(); // the builder is a child of the planet context
  $("planet-panel").classList.remove("is-open");
}
/// §body-management: a section header for the body panel.
const ppSec = (title: string, tip = ""): string =>
  `<div class="sp-sec" style="color:var(--dim);text-transform:uppercase;font-size:9px;letter-spacing:0.6px;margin:12px 0 4px"${tip ? ` title="${esc(tip)}"` : ""}>${esc(title)}</div>`;

function openPlanetPanel(d: SystemBodyDetail): void {
  buildPlanetPanel();
  // §build-panel: retargeting to a DIFFERENT body closes a stale builder (it
  // pointed at the old body); a live refresh of the SAME body keeps it open.
  if (buildTargetBodyId && buildTargetBodyId !== d.id) closeBuildPanel();
  openBodyDetail = d;
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
  // §bodies: THE WIRE BODY — deposits/structures/population are ITS OWN now.
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === d.id);
  // 1. GEOLOGY — this body's deposits (survey-gated on the wire: null =
  // unsurveyed, [] = surveyed and barren).
  const deps = body
    ? body.deposits == null
      ? `<div class="pp-note" style="border:0;padding:0;margin-top:10px">Geology unsurveyed — a survey reveals what lies here, and on which body.</div>`
      : body.deposits.length
        ? ppSec("Geology") + body.deposits.map(depositRow).join("")
        : `<div class="pp-note" style="border:0;padding:0;margin-top:10px">No deposits on this body.</div>`
    : d.deposits.length
      ? ppSec("Geology") + d.deposits.map(depositRow).join("")
      : `<div class="pp-note" style="border:0;padding:0;margin-top:10px">No deposits on this body.</div>`;
  const note = `<div class="pp-note">The roster is public geography — every corporation sees these worlds. What happens ON a body is its own: deposits, structures, crews, population. The stockpile, workforce &amp; food supply pool at the <b>star system</b> — one colony economy across its worlds.</div>`;

  // §bodies: THE ACTION SURFACE — owner's own system only (fog law: rivals get
  // geology + flavor, nothing else, ever).
  let manage = "";
  const mine = !!dyn && dyn.owner !== null && dyn.owner === state.playerId;
  if (mine && sid && dyn && body) {
    const tiers = body.structures ?? {};
    const blockaded = !!dyn.blockade;
    const blockChip = blockaded
      ? `<div style="margin-top:8px">${badgeChip("blockade", "under blockade", "negative", "A rival fleet holds station — convoys are held in & out. Production and construction continue; shipping resumes when the blockade breaks.")}</div>`
      : "";

    // 2. BUILT HERE — structures ON this body, with status chips.
    const builtKeys = Object.keys(tiers).filter((k) => (tiers[k] ?? 0) > 0);
    const lineOf = (slug: string) => dyn.assignments?.find((a) => a.body_id === body.id && a.structure === slug);
    const built = builtKeys.length
      ? ppSec("Built here") + `<div class="devs-row">` + builtKeys.map((k) => {
          const line = lineOf(k);
          const status = line?.suspended
            ? ` ${badgeChip("unfed", esc(label(line.suspended)), "warn", SUSPEND_HINT[line.suspended] ?? "suspended — nothing is lost")}`
            : !line && PRODUCER_SLUGS.has(k) ? ` ${badge("warn", "unstaffed")}` : "";
          return `<span class="dev" title="${esc(label(k))} ×${tiers[k]}">${esc(label(k))} <b>×${tiers[k]}</b>${status}</span>`;
        }).join(`<span class="dev-sep">·</span>`) + `</div>`
      : "";

    // 3. PRODUCTION LINES — this body's lines, crew ± controls (SetAssignment
    // now carries the body).
    const lines = assignmentLines(dyn, true, body);
    const linesSec = lines ? ppSec("Production lines", "output = richness/rate × tier × staffing × skill × food — hover a row for its chain") + lines : "";

    // 4. BUILD — the per-body slot pools at a glance + ONE button into the
    // dedicated build panel (the per-structure grid moved there wholesale, so the
    // geology/built-here above stay readable while you choose what to build).
    const pools = bodyPoolUsage(body, dyn);
    const structOpts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => !SHIP_KEYS.has(o.key) && !!POOL_OF[o.key]);
    // Openable if there's anything to DO here — a foundable structure (free slot +
    // deposit) or an existing tier to deepen (goods aside; the panel shows afford).
    const anyOpenable = structOpts.some((o) => {
      const st = structOption(o, dyn, body, pools);
      return st.tierUp || (!st.poolFull && !st.noDeposit);
    });
    const poolStrip = `<div class="pp-pools">` + (["resource", "industrial", "infrastructure"] as const)
      .map((k) => `<span class="pp-pool${pools[k].used >= pools[k].total ? " is-full" : ""}" title="${POOL_LABEL[k]} slots used / total on this body — founding a new structure needs a free slot; deepening a tier never does.">${POOL_LABEL[k]} ${pools[k].used}/${pools[k].total}</span>`)
      .join("") + `</div>`;
    const buildSec = ppSec("Build", "The at-a-glance slot pools — the reason to open the builder. Founding a NEW structure claims one of this body's pool slots; tier-ups deepen in place.") +
      poolStrip +
      `<button class="act pp-build-open" data-action="open-builder" ${anyOpenable ? "" : "disabled"} title="${anyOpenable ? "Open the build panel — pick a structure, read its recipe & effect, then queue it." : "Nothing buildable here — every slot pool is full and there's nothing to deepen. Grow this body's population, or build on another body."}">${icon("build", "sm")} Build structure…</button>`;

    // Per-body construction queue (ship jobs render under the yard below).
    const bodyQueue = buildQueueRows(sid, dyn, { filter: (j) => j.body_id === body.id && !SHIP_KEYS.has(j.key), seenKey: `${sid}#b${body.id}` });

    // 5. SHIPYARD on this body: SHIP CONSTRUCTION — the orbital yard's menu + queue.
    // §modules Part B4: the FIT PICKER rides above the warship buttons (fits the
    // next warship built here from what's in the module ledger).
    let yardSec = "";
    if ((tiers["shipyard"] ?? 0) > 0) {
      const shipOpts = (state.galaxy?.build_options ?? []).filter((o) => SHIP_KEYS.has(o.key));
      const shipQueue = buildQueueRows(sid, dyn, { filter: (j) => SHIP_KEYS.has(j.key), seenKey: `${sid}#yard` });
      // The per-hull rows moved into the dedicated ship builder; the planet panel
      // keeps the fit composer + the yard's queue (watch it here, choose there).
      const yardTier = dyn.shipyard_tier ?? 0;
      yardSec = shipOpts.length
        ? ppSec("Orbital yard — ship construction", "Ships build at this body's Shipyard (tier-gated exactly as before) and spawn here.") +
          `<div class="pp-yardline"><span class="pp-pool" title="The shipyard tier gates what can be built (Convoy/Scout/Colony I, Raider/Corvette II).">${icon("shipyard", "sm")} Shipyard ${romanTier(yardTier)}</span>` +
          `<button class="act pp-build-open" data-action="open-shipyard" title="Open the ship builder — pick a hull, set a quantity, read its stats & recipe, then queue it.">${icon("shipyard", "sm")} Build ship…</button></div>` +
          fitPicker(dyn) +
          shipQueue
        : "";
    }

    // 5b. ARMAMENTS COMPLEX on this body: MODULE MANUFACTURE + the system ledger
    // (§modules Part B3). Modules pool in the ledger and fit ships at build/refit.
    let modulesSec = "";
    if ((tiers["armaments_complex"] ?? 0) > 0) {
      modulesSec = ppSec("Armaments — module manufacture", "Modules are manufactured here into the system's module ledger, then fitted to warships at build (the yard's fit picker) or by refitting a docked fleet.") +
        moduleForge(dyn);
    }

    // 6. DEPOT on this body: LOGISTICS — cargo leaves from the orbital warehouse.
    let depotSec = "";
    if ((tiers["depot"] ?? 0) > 0) {
      const canShip = !blockaded && shippableStock(dyn).length > 0;
      const shipTitle = blockaded ? "Held — this system is under blockade." : canShip ? "Ship one raidable convoy per commodity, selling on arrival (Fuel stays as this system's operating reserve)." : "Nothing shippable — Fuel is retained as the operating reserve; other goods ship in whole units.";
      depotSec = ppSec("Orbital warehouse — logistics", "The system stockpile ships out from this station.") +
        `<div><button class="act" data-action="ship" ${canShip ? "" : "disabled"} title="${esc(shipTitle)}">${icon("cargo", "sm")} Ship → hub</button>` +
        `<button class="act" data-action="standing" title="Set a standing logistics rule that auto-dispatches convoys from here (online or off).">${icon("doctrine", "sm")} Auto-supply</button></div>`;
    }

    // The "Under construction" progress bars sit directly under Production Lines
    // (a structure being built is production-in-progress), above the Build menu.
    manage = blockChip + built + linesSec + bodyQueue + buildSec + yardSec + modulesSec + depotSec;
    // §bodies edge state: a body with nothing built and nothing buildable
    // stays a quiet piece of scenery.
    if (!manage) manage = `<div class="mhint">Nothing built here yet.</div>`;
  }

  $("planet-panel").innerHTML = head + `<div class="pp-body">${kindLine}<div class="pp-desc" style="margin-top:8px">${esc(d.description)}</div>${deps}${manage}${note}</div>`;
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
    } else if (el.dataset.act === "viewbattle" && el.dataset.record) {
      openBattleViewer(el.dataset.record); // §battle-records: the light-cone replay
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
    // §battle-records: watch the round-by-round replay (if its record is retained).
    ((): string => {
      const rec = recordForReport(r);
      return rec ? `<button class="act" data-act="viewbattle" data-record="${rec.id}" title="Watch the round-by-round replay of this battle.">${svgIcon("concept-fleet", "sm")} View battle replay</button>` : "";
    })() +
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
    `${svgIcon(SHIP_ICON[kind], "md")}<span class="fs-n">${esc(num)}</span>${tail}</span>`;
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
          `↩ ${svgIcon(SHIP_ICON[g.kind], "sm")}<span class="fs-echo">${esc(compStr(g))}</span>${echo}</button>`;
      }).join("") + `</div>`
    : "";

  // §3 COMMAND DELAY, condensed to one line: one-way CC→anchor time + the local
  // wall-clock an order issued now would land at — plus a terse reach verdict.
  const delay = battleCommandDelay(b);
  const cmdDelayLine = delay !== null
    ? `<div class="sp-line dim">${svgIcon("action-standing-order", "sm")} Order lag <b style="color:var(--ink)">${fmtCountdown(delay)}</b> → lands ~${esc(arrivalLocal(delay))}` +
      (delay > 20 ? ` · <span style="color:#e88">too far to steer</span>` : ` · <span style="color:var(--accent)">still in reach</span>`) + `</div>`
    : "";

  const head =
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">${badge("negative", "battle raging")} · as of ${fmtCountdown(b.age)} ago</div>` +
    `<h2>Engagement ${esc(nearestSystemName(b.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>`;
  const ragingLine = `<div class="sp-line dim">Raging <b style="color:var(--ink)">${fmtCountdown(observed)}</b> · forces remaining by your light</div>`;
  // §battle-records: watch the round-by-round replay of this live fight (if a
  // record for it has reached us — participants always have one, an observer
  // only when their sensors cover the site).
  const viewBtn = state.battleRecords.some((r) => r.id === b.id)
    ? `<button class="act" data-act="viewbattle" data-record="${b.id}" title="Watch the round-by-round replay — it streams in as light arrives.">${svgIcon("concept-fleet", "sm")} View battle replay</button>`
    : "";
  const body =
    ragingLine +
    (b.own
      ? `<div class="force-strip">${forceSide("You", "you", ownChips)}${forceSide("Enemy", "foe", rivalChips)}</div>` +
        withdrawRow +
        cmdDelayLine +
        viewBtn +
        `<button class="act" data-act="doctrine" title="Change your corp fleet doctrine — the standing engage/retreat/escort policy your fleets follow.">${icon("doctrine", "sm")} Doctrine ▸</button>`
      : `<div class="force-strip">${forceSide("Forces", "foe", rivalChips)}</div>` +
        `<div class="mhint dim" title="You see this fight only by its weapons-fire light — you have no forces here.">no forces here</div>` +
        viewBtn);
  panel.innerHTML = head + `<div class="pp-body">${body}</div>`;
}

// --- §battle-records Part A3: the BATTLE VIEWER (the light-cone replay) --------
// A centered overlay (#battle-viewer) that plays a battle round-by-round from
// `state.battleRecords`. Because nothing outruns light, the replay IS the battle
// as far as the viewer is concerned: only the ARRIVED round prefix exists (up to
// `light_frontier_tick`); rounds beyond it draw as a hatched "beyond your light
// cone" zone, and a still-running fight pins playback LIGHT-LIVE to the frontier,
// flipping to the outcome chip when the end light lands. Participant fidelity
// shows exact bars + damage-dealt salvo arrows + shown-math; a bucket-fidelity
// third party sees CountClass labels only (no dealt, no tooltip) — the fog law.
let openBattleViewerId: string | null = null;
let bvRound = 0; // the round index currently shown
let bvPlaying = false;
let bvSpeed = 4; // 1× | 4× | 16×
let bvLive = false; // pinned to the arriving light frontier (a running battle)
let bvAccum = 0; // fractional-round playback accumulator
let bvLastTs = 0;
let bvLoopRunning = false;
const BV_ROUND_SECS = 0.55; // wall-seconds per round at 1× playback

const bvRecordFor = (id: string): BattleRecordView | undefined => state.battleRecords.find((r) => r.id === id);
/// §battle-records: a concluded aftermath report has a DIFFERENT id space than
/// the record (its id is a report counter, the record's is the engagement id),
/// so join by the shared engagement-anchor position.
function recordForReport(r: BattleReportView): BattleRecordView | undefined {
  return state.battleRecords.find((rec) => rec.pos.x === r.pos.x && rec.pos.y === r.pos.y);
}

let battleViewerBuilt = false;
function buildBattleViewer(): void {
  if (battleViewerBuilt) return;
  battleViewerBuilt = true;
  $("battle-viewer").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act]") as HTMLElement | null;
    if (!el) return;
    const rec = openBattleViewerId ? bvRecordFor(openBattleViewerId) : undefined;
    const frontier = rec ? rec.rounds.length - 1 : -1;
    switch (el.dataset.act) {
      case "close":
        closeBattleViewer();
        break;
      case "play":
        // Replaying a finished battle from its end → restart from the top.
        if (!bvPlaying && rec && rec.outcome !== null && bvRound >= frontier) { bvRound = 0; bvLive = false; }
        bvPlaying = !bvPlaying;
        bvAccum = 0;
        renderBattleViewer();
        break;
      case "speed":
        bvSpeed = Number(el.dataset.speed) || 1;
        renderBattleViewer();
        break;
      case "round": {
        bvRound = Number(el.dataset.round) || 0;
        bvLive = rec !== undefined && rec.outcome === null && bvRound >= frontier;
        bvPlaying = false;
        renderBattleViewer();
        break;
      }
    }
  });
}

function openBattleViewer(id: string): void {
  const rec = bvRecordFor(id);
  if (!rec) return; // no access → no viewer (fog); the affordance is guarded too
  buildBattleViewer();
  openBattleViewerId = id;
  const running = rec.outcome === null;
  const frontier = rec.rounds.length - 1;
  bvLive = running;
  bvRound = running ? Math.max(0, frontier) : 0;
  bvPlaying = !running && frontier > 0; // auto-play a concluded replay from the top
  bvAccum = 0;
  bvLastTs = 0;
  renderBattleViewer();
  if (!bvLoopRunning) {
    bvLoopRunning = true;
    requestAnimationFrame(bvTick);
  }
}

function closeBattleViewer(): void {
  openBattleViewerId = null;
  bvPlaying = false;
  $("battle-viewer").classList.remove("is-open");
}

/// The playback clock — advances the shown round while playing, clamped to the
/// arrived light frontier. Self-stops when the viewer closes.
function bvTick(ts: number): void {
  if (openBattleViewerId === null) { bvLoopRunning = false; return; }
  const rec = bvRecordFor(openBattleViewerId);
  if (!rec) { closeBattleViewer(); bvLoopRunning = false; return; }
  const frontier = rec.rounds.length - 1;
  if (bvPlaying && frontier >= 0) {
    const dt = bvLastTs ? Math.min(0.25, (ts - bvLastTs) / 1000) : 0;
    bvAccum += (dt * bvSpeed) / BV_ROUND_SECS;
    let changed = false;
    while (bvAccum >= 1 && bvRound < frontier) { bvRound++; bvAccum -= 1; changed = true; }
    if (bvRound >= frontier) {
      bvAccum = 0;
      if (rec.outcome !== null) { bvPlaying = false; changed = true; } // end of a finished replay
      else { bvLive = true; } // caught up to a running fight's light frontier
    }
    if (changed) renderBattleViewer();
  }
  bvLastTs = ts;
  requestAnimationFrame(bvTick);
}

/// Keep an open viewer live as new light arrives (called from the View handler).
function refreshOpenBattleViewer(): void {
  if (openBattleViewerId === null || !$("battle-viewer").classList.contains("is-open")) return;
  renderBattleViewer();
}

const bvRC = (arr: RecordCount[], k: ShipKind): RecordCount | undefined => arr.find((rc) => rc.kind === k);

// §modules B5: the salvo FAMILY typing. A side's dominant weapon = the hardest
// hitter it brought (torpedo > driver > beam), derived from its participant-only
// initial loadouts; drives the replay's salvo arrow color + label. `beam` is the
// stock default (unfitted brawlers / no weapon modules).
type SalvoFamily = "beam" | "driver" | "torpedo";
const FAMILY_COLOR: Record<SalvoFamily, string> = { beam: "var(--accent)", driver: "#e8a13a", torpedo: "#e0574b" };
const FAMILY_LABEL: Record<SalvoFamily, string> = { beam: "beam", driver: "drivers", torpedo: "torpedoes" };
function sideFamily(sv: SideRecordView): SalvoFamily {
  const mods = (sv.loadouts ?? []).flatMap((st) => st.modules);
  if (mods.includes("torpedo_rack")) return "torpedo";
  if (mods.includes("mass_driver")) return "driver";
  return "beam";
}
// A compact per-stack fit summary for a side header (participant only).
function bvFitLine(sv: SideRecordView): string {
  const fits = sv.loadouts ?? [];
  if (!fits.length) return "";
  const parts = fits.map((st) => `${st.n}× ${st.modules.map((m) => MODULE_GLYPH[m as ModuleKind]).join("")} ${esc(shipKindLabel(st.kind))}`);
  return `<div class="bv-fits" title="What this side was fitted with — participant intel.">${parts.join(" · ")}</div>`;
}

/// One side's column: a per-kind survivor bar (participant: exact; bucket:
/// CountClass label), a kill flash, and the defender's platform block.
function bvSideHtml(rec: BattleRecordView, rd: RoundRecordView, s: 0 | 1, participant: boolean, platGone: boolean): string {
  const mine = rec.own_side === s;
  const cls = `bv-side ${s === 1 ? "right " : ""}${mine ? "mine" : ""}`;
  const role = s === 0 ? "Attackers" : "Defenders";
  // §modules B5: a weapon-family pip (participant only) + the per-stack fit line.
  const fam = participant ? sideFamily(rec.sides[s]) : null;
  const famPip = fam
    ? ` <span class="bv-fampip" style="color:${FAMILY_COLOR[fam]}" title="This side's dominant weapon — its salvos are typed ${FAMILY_LABEL[fam]}.">● ${esc(FAMILY_LABEL[fam])}</span>`
    : "";
  const hd = `<div class="bv-side__hd">${mine ? badge("neutral", "you") : ""}${esc(role)}${famPip}</div>` +
    (participant ? bvFitLine(rec.sides[s]) : "");
  const rows = rec.sides[s].initial.map((op) => {
    const k = op.kind;
    const surv = bvRC(rd.counts[s], k);
    const kill = bvRC(rd.kills[s], k);
    const gone = surv === undefined;
    let pct: number;
    let nlabel: string;
    if (participant) {
      const openN = op.exact ?? 0;
      const survN = surv?.exact ?? 0;
      pct = openN > 0 ? (survN / openN) * 100 : 0;
      nlabel = `×${survN}`;
    } else {
      const ord = surv ? COUNT_CLASS_ORD[surv.class] : -1;
      pct = ord >= 0 ? ((ord + 1) / 6) * 100 : 0;
      nlabel = surv ? countClassLabel(surv.class) : "—";
    }
    const killTag = participant
      ? (kill?.exact ? ` <span class="bv-krow__kill">−${kill.exact}</span>` : "")
      : (kill ? ` <span class="bv-krow__kill">▾</span>` : "");
    const nStyle = gone ? ' style="text-decoration:line-through;color:var(--dim)"' : "";
    return `<div class="bv-krow" title="${esc(shipKindLabel(k))}">${svgIcon(SHIP_ICON[k], "sm")}` +
      `<div class="bv-krow__bar"><div class="bv-krow__fill${gone ? " gone" : ""}" style="width:${gone ? 100 : Math.max(5, pct)}%"></div></div>` +
      `<span class="bv-krow__n"${nStyle}>${esc(nlabel)}${killTag}</span></div>`;
  }).join("");
  const plat = s === 1 && rec.sides[1].platform_tiers > 0
    ? `<div class="bv-plat${platGone ? " gone" : ""}">${icon("defense", "sm")} Platform ×${rec.sides[1].platform_tiers}</div>`
    : "";
  return `<div class="${cls}">${hd}${rows}${plat}</div>`;
}

const BV_NOTE_META: Record<string, { cls: string; icon: IconKey; text: (side: string) => string }> = {
  joined: { cls: "join", icon: "reinforce", text: (s) => `Reinforcements join the ${s.toLowerCase()}` },
  retreat_tripped: { cls: "retreat", icon: "withdraw", text: (s) => `The ${s.toLowerCase()} trip their retreat threshold — withdrawing` },
  withdraw_ordered: { cls: "retreat", icon: "withdraw", text: (s) => `A withdraw order reaches the ${s.toLowerCase()}` },
  disengage_exposure: { cls: "retreat", icon: "withdraw", text: (s) => `The ${s.toLowerCase()} break off — parting-shot exposure` },
  platform_destroyed: { cls: "", icon: "defense", text: () => `The Defense Platform is destroyed` },
  mutual_disengage: { cls: "", icon: "withdraw", text: () => `Mutual disengage — the grind breaks off` },
};
function bvNoteBanner(n: RoundNoteView): string {
  const meta = BV_NOTE_META[n.kind] ?? { cls: "", icon: "battle" as IconKey, text: () => n.kind };
  const side = n.side === 0 ? "Attackers" : n.side === 1 ? "Defenders" : "";
  return `<div class="bv-note ${meta.cls}">${icon(meta.icon, "sm")} ${esc(meta.text(side))}</div>`;
}

/// The battle's outcome as a verdict chip. From the viewer's own side when a
/// participant; a neutral factual label for a bucket-fidelity third party.
function bvOutcomeChip(rec: BattleRecordView, outcome: RaidOutcome): string {
  const atkDied = outcome === "attacker_destroyed" || outcome === "both_destroyed";
  const defDied = outcome === "target_destroyed" || outcome === "both_destroyed";
  if (rec.own_side === null) {
    const label = outcome === "both_destroyed" ? "mutual destruction"
      : atkDied ? "attackers destroyed"
        : defDied ? "defenders destroyed"
          : "both withdrew";
    return badge("neutral", label);
  }
  const youDied = rec.own_side === 0 ? atkDied : defDied;
  const themDied = rec.own_side === 0 ? defDied : atkDied;
  if (youDied && themDied) return badge("negative", "mutual destruction");
  if (youDied) return badge("negative", "defeat — your force destroyed");
  if (themDied) return badge("positive", "victory — their force destroyed");
  return badge("neutral", "both withdrew");
}

function renderBattleViewer(): void {
  if (openBattleViewerId === null) return;
  if (renderDeferred("battle-viewer", renderBattleViewer)) return; // §single-click guard
  const rec = bvRecordFor(openBattleViewerId);
  if (!rec) { closeBattleViewer(); return; }
  const participant = rec.fidelity === "participant";
  const running = rec.outcome === null;
  const frontier = rec.rounds.length - 1;
  if (bvLive && frontier >= 0) bvRound = frontier;
  bvRound = Math.max(0, Math.min(bvRound, Math.max(0, frontier)));

  const head =
    `<div class="pp-head"><div class="panel-title"><div>` +
    `<div class="eyebrow">${svgIcon("concept-fleet", "sm")} battle replay${rec.raid ? " · raid" : ""}${participant ? "" : " · sensor estimate"}</div>` +
    `<h2>Engagement ${esc(nearestSystemName(rec.pos))}</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close (Esc)" aria-label="Close">✕</button></div>`;

  const label0 = rec.own_side === 0 ? "You" : "Attackers";
  const label1 = rec.own_side === 1 ? "You" : "Defenders";
  const statusChip = rec.outcome ? bvOutcomeChip(rec, rec.outcome) : badge("warn", "◉ LIGHT-LIVE");
  const counter = frontier < 0 ? "no rounds yet" : `round ${bvRound + 1} / ${rec.rounds.length}${running ? " +" : ""}`;
  const sub = `<div class="bv-sub"><span class="bv-vs"><span class="${rec.own_side === 0 ? "you" : "foe"}">${esc(label0)}</span> vs <span class="${rec.own_side === 1 ? "you" : "foe"}">${esc(label1)}</span></span> ${statusChip}<span class="bv-count">${esc(counter)}</span></div>`;

  let arena = `<div class="bv-empty">Awaiting the first round's light…</div>`;
  let notes = "";
  let agoline = "";
  if (frontier >= 0) {
    const rd = rec.rounds[bvRound];
    const platGone = rec.rounds.slice(0, bvRound + 1).some((r) => r.notes.some((n) => n.kind === "platform_destroyed"));
    // Salvo gutter: arrows scaled by damage dealt (participant); a glyph for bucket.
    let salvos: string;
    if (participant && rd.dealt) {
      const maxDealt = Math.max(1e-6, ...rec.rounds.flatMap((r) => (r.dealt ? [r.dealt[0], r.dealt[1]] : [0])));
      const w = (d: number) => Math.max(8, (d / maxDealt) * 88);
      const mute = (d: number) => (d < maxDealt * 0.03 ? " mute" : "");
      // §modules B5: TYPE each salvo arrow by its firing side's weapon family.
      const famA = sideFamily(rec.sides[0]), famD = sideFamily(rec.sides[1]);
      salvos = `<div class="bv-salvos">` +
        `<div class="bv-arrow r${mute(rd.dealt[0])}" style="width:${w(rd.dealt[0])}%; background:${FAMILY_COLOR[famA]}" title="attackers' ${FAMILY_LABEL[famA]} dealt ${rd.dealt[0].toFixed(2)} this round"></div>` +
        `<div class="bv-arrow l${mute(rd.dealt[1])}" style="width:${w(rd.dealt[1])}%; margin-left:auto; background:${FAMILY_COLOR[famD]}" title="defenders' ${FAMILY_LABEL[famD]} dealt ${rd.dealt[1].toFixed(2)} this round"></div>` +
        `</div>`;
    } else {
      salvos = `<div class="bv-salvos" style="align-items:center;color:var(--dim)" title="exact fire strength is fogged — you see only the size buckets">⚔</div>`;
    }
    arena = `<div class="bv-arena">${bvSideHtml(rec, rd, 0, participant, false)}${salvos}${bvSideHtml(rec, rd, 1, participant, platGone)}</div>`;
    notes = rd.notes.length ? `<div class="bv-notes">${rd.notes.map(bvNoteBanner).join("")}</div>` : "";
    const intoFight = Math.max(0, rd.tick / state.tickHz - rec.started_at);
    const delay = state.commandCenter && state.galaxy
      ? Math.hypot(rec.pos.x - state.commandCenter.x, rec.pos.y - state.commandCenter.y) / state.galaxy.c
      : 0;
    const seenAgo = Math.max(0, liveSimTime() - (rd.tick / state.tickHz + delay));
    agoline = `<div class="bv-agoline">at +${fmtCountdown(intoFight)} into the fight · this round's light reached you ${fmtCountdown(seenAgo)} ago${participant ? "" : " · size estimates only"}</div>`;
  }

  const playIcon = bvPlaying ? "❚❚ Pause" : "▶ Play";
  const speeds = [1, 4, 16].map((sp) => `<button class="bv-btn${bvSpeed === sp ? " on" : ""}" data-act="speed" data-speed="${sp}">${sp}×</button>`).join("");
  const ticks = rec.rounds.map((_r, i) => `<div class="bv-tick${i < bvRound ? " seen" : ""}${i === bvRound ? " cur" : ""}" data-act="round" data-round="${i}" title="round ${i + 1}"></div>`).join("");
  const hatch = running ? `<div class="bv-hatch" title="beyond your light cone — later rounds haven't reached you yet"></div>` : "";
  const transport = frontier < 0 ? "" :
    `<div class="bv-transport">` +
    `<button class="bv-btn" data-act="play">${playIcon}</button>` +
    `<span class="bv-speeds">${speeds}</span>` +
    `<div class="bv-scrub">${ticks || `<div class="bv-tick cur"></div>`}${hatch}</div></div>${agoline}`;

  $("battle-viewer").innerHTML = head + sub + arena + notes + transport;
  $("battle-viewer").classList.add("is-open");
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
  const plunderStr = r.plunder.length ? r.plunder.map((s) => `${s.units} ${esc(label(s.commodity))}`).join(", ") : "an empty stockpile";
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
    } else if (e.key === "r" || e.key === "R") {
      toggleResearch(); // §research: the Programme Boards (no own ship selected)
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
      // §battle-records: the replay overlay is topmost — Escape closes it first.
      if ($("battle-viewer").classList.contains("is-open")) {
        closeBattleViewer();
      } else if ($("build-panel").classList.contains("is-open") || $("build-ship-panel").classList.contains("is-open")) {
        closeBuildPanel(); // back out of either builder before the planet panel
      } else if ($("planet-panel").classList.contains("is-open")) {
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
  return `${label(dom.resource)}-rich ${tier}`;
}

function depositRow(d: Deposit): string {
  const pct = Math.min(100, d.richness * 40);
  const reserves = d.reserves === null
    ? `<span class="tone-up">renewable</span>`
    : d.reserves < 50 ? `<span class="is-warn">${fmt(d.reserves)} left</span>`
      : `${fmt(d.reserves)} left`;
  return `<div class="dep-row"><span class="dep-ico">${commodityIcon(d.resource, "md")}</span>` +
    `<span class="dep-name">${label(d.resource)}</span>${bar(pct)}` +
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
  const rateOf = new Map<Commodity, number>();
  // §explore: the readout is owner-only, and an owner always knows their own
  // geology (dyn.deposits present) — read from the light-gated view.
  for (const d of dyn?.deposits ?? []) rateOf.set(d.resource, (rateOf.get(d.resource) ?? 0) + d.richness * mult);
  const all = new Set<Commodity>([...stockOf.keys(), ...rateOf.keys()] as Commodity[]);
  const rows = [...all].filter((c) => (stockOf.get(c) ?? 0) >= 1 || (rateOf.get(c) ?? 0) > 0.01);
  if (!rows.length) return "";
  // Refinery line (§buildings step 3b): converting Volatiles → Fuel, or idle dry.
  const refTier = dyn?.refinery_tier ?? 0;
  let refinery = "";
  if (refTier > 0) {
    // §economy: the refinery is a STAFFED converter line now — this hint shows
    // its base rate; the Part-7 colony panel carries the live factor chain.
    const rate = state.galaxy?.fuel_refinery_rate ?? 0.8;
    const vol = stockOf.get("volatiles") ?? 0;
    refinery = vol > 0
      ? `<div class="mhint" style="margin-top:4px" title="Fuel Refinery ×${refTier}: converts Volatiles → Fuel (1:1) up to ${rate.toFixed(1)}/s per throughput tier when staffed.">${icon("refinery", "sm")} ${icon("volatiles", "sm")} → ${icon("fuel", "sm")} up to ${rate.toFixed(1)}/s · staffed line</div>`
      : `<div class="mhint" style="margin-top:4px" title="Fuel Refinery idle — no Volatiles to convert. Haul some in (1 Fuel per Volatile).">${icon("refinery", "sm")} ${badgeChip("warning", "idle — no Volatiles", "warn", "Haul Volatiles in to convert.")}</div>`;
  }
  return rows.map((c) => {
      const rt = rateOf.get(c) ?? 0;
      const rate = rt > 0.01 ? `<span class="sp-rate">+${rt.toFixed(2)}/s</span>` : `<span class="sp-none">—</span>`;
      return `<div class="sys-prod"><span class="dep-ico">${commodityIcon(c, "md")}</span>` +
        `<span class="sp-name">${label(c)}</span><span class="sp-stock">${fmt(stockOf.get(c) ?? 0)}</span>${rate}</div>`;
    }).join("") + refinery;
}

// §economy Part 6: the per-line PRODUCTION ROWS — one row per line with the
// server-resolved factor chain (shown math: throughput × staffing × skill ×
// food), crew ± controls (data-crew), suspension causes. Rendered on the BODY
// panels now (§system-reorg dropped the system-screen workforce block).
const PRODUCER_SLUGS = new Set([
  "mining_complex", "volatile_harvester", "bioharvester", "smelter",
  "electronics_fabricator", "chemical_works", "fuel_refinery", "agroplex",
  "machine_works", "armaments_complex", "shipyard",
]);
const SUSPEND_HINT: Record<string, string> = {
  no_food: "out of Provisions — ship food",
  no_inputs: "input basket dry — ship raws in or staff extraction",
  storage_full: "storage full — ship goods out or build a Depot",
};
// §body-management: the production-line rows, shared between the read-only
// summary/rail digest (withControls=false — pure data) and the BODY PANELS
// (withControls=true — the ONLY place crew ± controls render; the SetAssignment
// sends are byte-identical, just relocated). `slugFilter` scopes a body panel
// to the structures anchored there.
function assignmentLines(dyn: SystemStateView | undefined, withControls: boolean, forBody?: BodyView): string {
  const nameOf = new Map((dyn?.bodies ?? []).map((b) => [b.id, b.name] as const));
  const lines = (dyn?.assignments ?? []).filter((a) => !forBody || a.body_id === forBody.id);
  // §bodies: a line is keyed (body, structure) — idle detection must match.
  const postedAt = new Set((dyn?.assignments ?? []).map((a) => `${a.body_id}:${a.structure}`));
  const rowFor = (a: AssignmentView): string => {
    const chain = `×${a.throughput.toFixed(1)} tier · ×${a.staffing.toFixed(2)} staffing · ×${a.skill.toFixed(2)} skill · ×${a.food.toFixed(2)} food`;
    const out = a.outputs.filter(([, r]) => r > 0.001).map(([c, r]) => `+${r.toFixed(2)} ${esc(label(c))}/s`).join(" ");
    const spec = Object.entries(a.specialists).map(([k, n]) => `${n as number}× ${esc(label(k))}`).join(", ");
    const susp = a.suspended
      ? ` ${badgeChip("unfed", esc(label(a.suspended)), "warn", SUSPEND_HINT[a.suspended] ?? "suspended — nothing is lost")}`
      : "";
    const controls = withControls
      ? `<button class="act" data-crew="${a.body_id}:${a.structure}:${a.workers + 1}" title="post another crew">+</button>` +
        `<button class="act" data-crew="${a.body_id}:${a.structure}:${Math.max(0, a.workers - 1)}" title="withdraw a crew">−</button>`
      : "";
    // The body lives in the hover title — the roster table already maps
    // what's where, and the row grid is tuned for short names.
    return `<div class="sys-prod sys-prod--flow" title="${esc(a.title)} ×${a.tier} on ${esc(nameOf.get(a.body_id) ?? "—")} — ${chain}${spec ? ` · specialists: ${spec}` : ""}">` +
      `<span class="sp-name">${esc(a.title)} ×${a.tier}</span>` +
      `<span class="sp-stock">${a.workers}👷${spec ? ` +${Object.values(a.specialists).reduce((s: number, n) => s + (n as number), 0)}🎓` : ""}</span>` +
      `<span class="sp-rate">${out || "—"}</span>${susp}${controls}` +
      `</div>`;
  };
  const idle = (forBody ? [forBody] : dyn?.bodies ?? [])
    .flatMap((b) => Object.entries(b.structures ?? {})
      .filter(([slug, t]) => t > 0 && PRODUCER_SLUGS.has(slug) && !postedAt.has(`${b.id}:${slug}`))
      .map(([slug, t]) =>
        `<div class="sys-prod sys-prod--flow dev--none" title="built but UNSTAFFED — it produces nothing until a crew is posted${withControls ? "" : " (staff it from its body\u2019s panel)"}">` +
        `<span class="sp-name">${esc(label(slug))} ×${t}</span><span class="sp-none">unstaffed</span>` +
        (withControls ? `<button class="act" data-crew="${b.id}:${slug}:1" title="post a crew">+ crew</button>` : "") +
        `</div>`))
    .join("");
  return lines.map(rowFor).join("") + idle;
}

// Build / develop panel (§step1 growth + structure sinks) for an OWNED system:
// each buildable option with its recipe cost + afford state (costs draw from THIS
// system's stockpile), plus any in-progress build with an ETA. Fog-safe — only
// rendered for systems you own (the View only sends build state to the owner).
// Ship build keys — units, not developments: they never consume a development
// slot (mirrors the sim's slot rule in world.rs apply_build).
const SHIP_KEYS = new Set(["convoy", "raider", "corvette", "colony", "scout"]);

// §economy Part 6 / §bodies: crew ± control → SetAssignment. `spec` is
// "bodyId:slug:workers" — the line lives ON a body now; posted specialists are
// preserved server-side only if re-sent, so we send the current line's along.
function sendCrew(systemId: EntityId, spec: string): void {
  if (!net) return;
  const [bid, slug, n] = spec.split(":");
  const body_id = Number(bid);
  const dyn = state.systems.find((s) => s.id === systemId);
  const line = dyn?.assignments?.find((a) => a.body_id === body_id && a.structure === slug);
  net.send({ type: "SetAssignment", system_id: systemId, structure: slug, workers: Math.max(0, Number(n) || 0), specialists: line?.specialists ?? {}, body_id });
}
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
function buildQueueRows(
  sid: string,
  dyn: SystemStateView | undefined,
  opts?: {
    /// §bodies: rows NAVIGATE to their build site (data-body from the job's
    /// own body_id — never a command).
    nav?: boolean;
    /// Scope the rows (e.g. the shipyard panel shows only ship jobs).
    filter?: (j: BuildState) => boolean;
    /// Independent completion-flash bookkeeping per surface (the summary and a
    /// body panel may both render queues for one system).
    seenKey?: string;
  },
): string {
  const jobs = (dyn?.builds ?? []).filter((j) => !opts?.filter || opts.filter(j));
  const now = liveSimTime();
  const seenAs = opts?.seenKey ?? sid;
  // Diff vs the previous render to catch completions (only if seen recently).
  const prev = buildQueueSeen.get(seenAs);
  const keys = jobs.map((j) => j.key);
  if (prev && performance.now() - prev.at < 2000) {
    const remaining = [...keys];
    for (const k of prev.keys) {
      const i = remaining.indexOf(k);
      if (i >= 0) remaining.splice(i, 1);
      else {
        const flashes = buildDoneFlash.get(seenAs) ?? [];
        flashes.push({ label: buildLabel(k), until: performance.now() + 4000 });
        buildDoneFlash.set(seenAs, flashes);
      }
    }
  }
  buildQueueSeen.set(seenAs, { keys, at: performance.now() });
  const flashes = (buildDoneFlash.get(seenAs) ?? []).filter((f) => f.until > performance.now());
  buildDoneFlash.set(seenAs, flashes);
  if (!jobs.length && !flashes.length) return "";

  // Resulting tier per development job: the SITE BODY's current tier + 1 +
  // same-site jobs ahead (§bodies: tiers live on bodies).
  const bodyTier = (bid: number, slug: string): number =>
    (dyn?.bodies?.find((b) => b.id === bid)?.structures ?? {})[slug] ?? 0;
  const aheadCount: Record<string, number> = {};
  const rows = jobs.map((j) => {
    const total = buildOption(j.key)?.build_secs ?? 0;
    const start = j.complete_time - total;
    const pct = total > 0 ? Math.max(0, Math.min(100, ((now - start) / total) * 100)) : 0;
    const left = Math.max(0, j.complete_time - now);
    const isDev = !SHIP_KEYS.has(j.key);
    const site = `${j.body_id}:${j.key}`;
    const ahead = aheadCount[site] ?? 0;
    aheadCount[site] = ahead + 1;
    const name = isDev ? `${buildLabel(j.key)} ×${bodyTier(j.body_id, j.key) + 1 + ahead}` : buildLabel(j.key);
    // §bodies: a queue row NAVIGATES to its site body when the caller asks.
    const bodyId = opts?.nav ? String(j.body_id) : null;
    const nav = bodyId ? ` data-body="${bodyId}" style="cursor:pointer" title="under construction — click to open its body panel"` : "";
    return `<div class="bq-row"${nav}><span class="bq-ic">${svgIcon(BUILD_ICON[j.key] ?? "action-build", "sm")}</span>` +
      `<div class="bq-main"><div class="bq-head"><b>${esc(name)}</b>` +
      `<span class="bq-eta">${fmtCountdown(left)} · done ${doneAtLocal(j.complete_time)}</span></div>` +
      `${bar(pct)}</div></div>`;
  }).join("");
  const doneRows = flashes.map((f) =>
    `<div class="bq-row bq-done"><span class="bq-ic tone-up">✓</span><div class="bq-main"><b>${esc(f.label)}</b> <span class="dim">complete</span></div></div>`).join("");
  return `<div class="deps-head" style="margin-top:8px">${svgIcon("action-build", "sm")} Under construction</div>` +
    `<div class="bq-list">${rows}${doneRows}</div>`;
}

// --- §modules Part B: the module catalog + client UI state -------------------
// The 5 modules in a fixed order (mirrors sim MODULE_KINDS); labels + a compact
// glyph for chips/ledger; and per-hull slot counts (mirrors ShipKind::module_slots).
const MODULE_ALL: ModuleKind[] = ["mass_driver", "torpedo_rack", "point_defense_screen", "reflective_plating", "whipple_armor"];
const MODULE_LABEL: Record<ModuleKind, string> = {
  mass_driver: "Mass Driver", torpedo_rack: "Torpedo Rack", point_defense_screen: "Point-Defense",
  reflective_plating: "Reflective Plating", whipple_armor: "Whipple Armor",
};
const MODULE_GLYPH: Record<ModuleKind, string> = {
  mass_driver: "◎", torpedo_rack: "➹", point_defense_screen: "◈", reflective_plating: "◇", whipple_armor: "▤",
};
// What each module DOES, one line (for button/chip titles).
const MODULE_TIP: Record<ModuleKind, string> = {
  mass_driver: "Weapon: fires DRIVERS (harder hit) — countered by Whipple Armor.",
  torpedo_rack: "Weapon: fires TORPEDOES (hardest hit, ignores armor) — countered by Point-Defense.",
  point_defense_screen: "Weapon+defense: weak beam, but adds torpedo INTERCEPTION for the side.",
  reflective_plating: "Armor: blunts incoming BEAM into this ship.",
  whipple_armor: "Armor: blunts incoming DRIVER into this ship.",
};
const MODULE_SLOTS: Record<string, number> = { corvette: 2, raider: 2, scout: 1, convoy: 0, colony: 0 };
// Sol's module spread (mirrors sim MODULE_BUY_MULT / MODULE_SELL_MULT) — DISPLAY
// only; the server prices the real charge on execution (shown "~").
const MODULE_BUY_MULT = 2.0, MODULE_SELL_MULT = 0.5;
// A module's goods VALUE = its recipe commodities priced at the observed hub
// market (the same basis the sim uses), or null if the price board isn't in yet.
function moduleRecipeValue(m: ModuleKind): number | null {
  const o = buildOption(`module:${m}`);
  if (!o || !state.market) return null;
  const price = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  let v = 0;
  for (const c of o.costs) {
    const p = price.get(c.commodity as Commodity);
    if (p === undefined) return null;
    v += c.units * p;
  }
  return v;
}
// The FIT the player is composing for the next warship build (module slugs, ≤2).
let pendingFit: ModuleKind[] = [];
// The target FIT the player is composing for a REFIT (own-fleet panel, ≤2).
let pendingRefit: ModuleKind[] = [];
// The module ledger at a system (owner-only; {} if unseen).
function moduleLedgerAt(sid: string): Record<string, number> {
  return state.systems.find((s) => s.id === sid)?.modules ?? {};
}
// §modules Part B3: the module FORGE for a body with an Armaments Complex —
// the system ledger line + one manufacture button per module (BuildModule),
// costs/afford drawn from the shared build_options channel ("module:<slug>").
function moduleForge(dyn: SystemStateView | undefined): string {
  const ledger = dyn?.modules ?? {};
  const onHand = MODULE_ALL.filter((m) => (ledger[m] ?? 0) > 0);
  const ledgerLine = `<div class="mhint" style="margin-top:2px">${icon("cargo", "sm")} ledger: ${onHand.length ? onHand.map((m) => `${MODULE_GLYPH[m]} ${esc(MODULE_LABEL[m])} ×${ledger[m]}`).join(" · ") : "empty"}</div>`;
  const have = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const btns = MODULE_ALL.map((m) => {
    const o = buildOption(`module:${m}`);
    if (!o) return "";
    const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
    const cost = o.costs.map((c) => `${commodityIcon(c.commodity as Commodity, "sm")}${c.units}`).join(" ");
    return `<button class="act build-opt" data-build="module:${m}" ${afford ? "" : "disabled"} title="${esc(MODULE_TIP[m])} — costs draw from this system's stockpile.">` +
      `<span class="bo-name">${MODULE_GLYPH[m]} ${esc(MODULE_LABEL[m])}</span><span class="bo-cost">${cost} · ${icon("time", "sm")}${o.build_secs}s</span></button>`;
  }).join("");
  return ledgerLine + `<div class="build-grid" style="margin-top:4px">${btns}</div>`;
}
// §modules Part B4: the FIT PICKER above a yard's warship builds — toggle chips
// for modules IN THE LEDGER (only what you have can be fitted); the composed fit
// (≤2) is clamped per-hull at dispatch. Empty ledger → no picker (nothing to fit).
function fitPicker(dyn: SystemStateView | undefined): string {
  const ledger = dyn?.modules ?? {};
  const avail = MODULE_ALL.filter((m) => (ledger[m] ?? 0) > 0);
  if (!avail.length) return "";
  pendingFit = pendingFit.filter((m) => (ledger[m] ?? 0) > 0); // drop now-absent picks
  const chips = avail.map((m) => {
    const on = pendingFit.includes(m);
    return `<button class="act fit-chip${on ? " is-on" : ""}" data-fit="${m}" title="${esc(MODULE_TIP[m])}">${MODULE_GLYPH[m]} ${esc(MODULE_LABEL[m])}${on ? " ✓" : ""}</button>`;
  }).join("");
  const cur = pendingFit.length ? pendingFit.map((m) => MODULE_GLYPH[m]).join(" ") : "stock (unfitted)";
  return `<div class="mhint" style="margin:4px 0 2px" title="Pick up to 2 modules to fit the next warship built here; a ship takes as many as its hull has slots (Raider/Corvette 2, Scout 1).">${svgIcon("action-build", "sm")} fit next build: <b>${cur}</b></div>` +
    `<div class="fit-row">${chips}</div>`;
}
// §modules Part B4: the REFIT section on an OWN fleet — its warship STACKS
// (kind × loadout, fitted + the unfitted remainder) each with a "Refit →"
// button to the composed target fit. Offered whenever the fleet has warships;
// the server enforces the docked-Shipyard + ledger-delta gate (a soft reject
// leaves the fleet untouched). Empty for logistics-only fleets.
function refitSection(g: GhostView): string {
  if (!g.own) return "";
  const comp = g.composition ?? [];
  const warKinds = comp.filter((c) => (MODULE_SLOTS[c.kind] ?? 0) > 0 && c.count > 0);
  if (!warKinds.length) return "";
  const loadouts = g.loadouts ?? [];
  // Build (kind, from, n) stacks: each fitted stack + the unfitted remainder.
  const stacks: { kind: ShipKind; from: ModuleKind[]; n: number }[] = [];
  for (const c of warKinds) {
    let fittedTotal = 0;
    for (const l of loadouts.filter((l) => l.kind === c.kind)) {
      stacks.push({ kind: c.kind, from: l.modules, n: l.n });
      fittedTotal += l.n;
    }
    const unfit = c.count - fittedTotal;
    if (unfit > 0) stacks.push({ kind: c.kind, from: [], n: unfit });
  }
  const chips = MODULE_ALL.map((m) => {
    const on = pendingRefit.includes(m);
    return `<button class="act fit-chip${on ? " is-on" : ""}" data-act="refitmod" data-mod="${m}" title="${esc(MODULE_TIP[m])}">${MODULE_GLYPH[m]} ${esc(MODULE_LABEL[m])}${on ? " ✓" : ""}</button>`;
  }).join("");
  const targetTxt = pendingRefit.length ? pendingRefit.map((m) => `${MODULE_GLYPH[m]} ${esc(MODULE_LABEL[m])}`).join(" · ") : "stock (strip all fits)";
  const rows = stacks.map((s) => {
    const fromTxt = s.from.length ? s.from.map((m) => MODULE_GLYPH[m]).join("") : "stock";
    const to = pendingRefit.slice(0, MODULE_SLOTS[s.kind] ?? 0);
    const same = [...s.from].sort().join(",") === [...to].sort().join(",");
    return `<div class="sp-line" style="justify-content:space-between;gap:6px">` +
      `<span title="${s.n} ${esc(shipKindLabel(s.kind))} currently fitted: ${fromTxt}">${s.n}× ${esc(shipKindLabel(s.kind))} · ${fromTxt}</span>` +
      `<button class="act" data-act="refit" data-kind="${s.kind}" data-from="${s.from.join(",")}" data-n="${s.n}" ${same ? "disabled" : ""} title="Refit these ${s.n} ship(s) to the target fit — done at a docked Shipyard you own/ally; the added modules come from that system's ledger, removed ones return to it.">Refit →</button>` +
      `</div>`;
  }).join("");
  return `<div class="sp-sec">${svgIcon("action-build", "sm")} Refit</div>` +
    `<div class="mhint" style="margin:2px 0" title="Pick the target fit (≤2), then Refit a stack. The ships enter the yard and rejoin fitted; requires a docked Shipyard and the added modules in that system's ledger.">target: <b>${targetTxt}</b> — at a docked Shipyard</div>` +
    `<div class="fit-row">${chips}</div>${rows}`;
}

// (buildOptionRow removed — the inline structure/ship rows it drew are gone; the
//  dedicated build panels now own that gating via structOption / shipOption.)

// §explore Part 3: the trait line (name + one-line effect) for the OWNER's
// system panel. Warn-tinted for the lemon. Slug "bonus_vein:<commodity>" carries
// the vein's commodity.
function traitLine(slug: string): { title: string; desc: string; warn: boolean } {
  if (slug.startsWith("bonus_vein:")) {
    const c = slug.split(":")[1];
    return { title: "Bonus Vein", desc: `Its ${label(c)} deposit runs ×1.5 richer — always on.`, warn: false };
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
      return { title: label(slug), desc: "", warn: false };
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

// §body-management: the monolithic buildPanel is gone — its pool readout
// lives in the summary, its rows on the body panels (openPlanetPanel).

// Slug → pool + derived PER-BODY pool budgets — MIRRORS the sim (build.rs
// slot_pool / body.rs *_slots): Resource = deposits.min(4); Industrial =
// (gas giant ? 0 : 1) + body pop tier; Infrastructure = 1 + habitable +
// (tier ≥ 1); body pop tiers at 1.5M / 4.0M.
const POOL_OF: Record<string, "resource" | "industrial" | "infrastructure"> = {
  mining_complex: "resource", volatile_harvester: "resource", bioharvester: "resource",
  smelter: "industrial", electronics_fabricator: "industrial", chemical_works: "industrial",
  fuel_refinery: "industrial", machine_works: "industrial", armaments_complex: "industrial", shipyard: "industrial",
  agroplex: "infrastructure", habitat: "infrastructure", depot: "infrastructure",
  sensor_array: "infrastructure", defense_platform: "infrastructure", academy: "infrastructure",
};
// Extraction structure → the deposit commodities it works — MIRRORS the sim's
// production.rs extraction_structure (a mine only works its own rock).
const EXTRACTION_OF: Record<string, Commodity[]> = {
  mining_complex: ["metallic_ore", "silicates", "rare_elements"],
  volatile_harvester: ["volatiles"],
  bioharvester: ["biomass"],
};
type PoolUse = Record<"resource" | "industrial" | "infrastructure", { used: number; total: number }>;
function bodyPoolTotals(b: BodyView): Record<"resource" | "industrial" | "infrastructure", number> {
  const popTier = b.population >= 4.0 ? 2 : b.population >= 1.5 ? 1 : 0;
  return {
    resource: Math.min(4, (b.deposits ?? []).length),
    industrial: (b.kind === "gas_giant" ? 0 : 1) + popTier,
    infrastructure: 1 + (b.habitable ? 1 : 0) + (popTier >= 1 ? 1 : 0),
  };
}
/// THIS body's pools, counting built structures AND pending NEW-structure jobs
/// (mirrors pool_slots_built + pool_slots_pending — tier-ups are exempt).
function bodyPoolUsage(b: BodyView, dyn: SystemStateView | undefined): PoolUse {
  const totals = bodyPoolTotals(b);
  const used = { resource: 0, industrial: 0, infrastructure: 0 };
  for (const [slug, t] of Object.entries(b.structures ?? {})) {
    if (t > 0 && POOL_OF[slug]) used[POOL_OF[slug]] += 1;
  }
  const pending = new Set((dyn?.builds ?? [])
    .filter((j) => j.body_id === b.id && POOL_OF[j.key] && ((b.structures ?? {})[j.key] ?? 0) === 0)
    .map((j) => j.key));
  for (const slug of pending) used[POOL_OF[slug]] += 1;
  return {
    resource: { used: used.resource, total: totals.resource },
    industrial: { used: used.industrial, total: totals.industrial },
    infrastructure: { used: used.infrastructure, total: totals.infrastructure },
  };
}
/// The summary strip: pools SUMMED across the roster (a system fact).
function poolUsage(dyn: SystemStateView | undefined): PoolUse {
  const sum = { resource: { used: 0, total: 0 }, industrial: { used: 0, total: 0 }, infrastructure: { used: 0, total: 0 } };
  for (const b of dyn?.bodies ?? []) {
    const totals = bodyPoolTotals(b);
    for (const k of ["resource", "industrial", "infrastructure"] as const) sum[k].total += totals[k];
    for (const [slug, t] of Object.entries(b.structures ?? {})) {
      if (t > 0 && POOL_OF[slug]) sum[POOL_OF[slug]].used += 1;
    }
  }
  return sum;
}

// ---- §build-panel: the dedicated structure builder ---------------------------
// A client-only UI over the SAME DevelopSystem command — the per-structure grid
// that used to live inline on the planet panel moved here wholesale, so the
// planet panel stays a lens on the body while you choose what to build. All the
// slot / afford / tier / deposit gating below is the ONE source of truth shared
// by the row list, the detail, and the Queue button.
type Pool = "resource" | "industrial" | "infrastructure";
const POOL_LABEL: Record<Pool, string> = { resource: "Resource", industrial: "Industrial", infrastructure: "Infrastructure" };
// Structure → the closest registry icon (art only; mirrors systemview's family).
const STRUCT_ICON: Record<string, IconKey> = {
  mining_complex: "extractor", volatile_harvester: "extractor", bioharvester: "extractor",
  smelter: "refinery", electronics_fabricator: "refinery", chemical_works: "refinery",
  fuel_refinery: "refinery", machine_works: "build", armaments_complex: "build", shipyard: "shipyard",
  agroplex: "habitat", habitat: "habitat", depot: "depot",
  sensor_array: "sensor", defense_platform: "defense", academy: "habitat",
};
// Producers scale output by the tier-throughput curve (mirrors sim TIER_THROUGHPUT).
const TIER_THROUGHPUT = [0, 1.0, 2.2, 3.8, 6.0];
const THROUGHPUT_STRUCTS = new Set(["mining_complex", "volatile_harvester", "bioharvester", "smelter", "electronics_fabricator", "chemical_works", "fuel_refinery", "machine_works", "armaments_complex", "agroplex", "academy"]);
// One-line "what it does" + "what it enables" per structure (client flavor, kept
// consistent with the sim's recipes — production.rs CONVERTERS / build.rs).
const STRUCT_INFO: Record<string, { desc: string; effect: string }> = {
  mining_complex: { desc: "Mines the body's Metallic Ore, Silicates, or Rare-Element deposit.", effect: "Feeds raw ore into the system stockpile." },
  volatile_harvester: { desc: "Draws Volatiles from the body's gas/ice deposit.", effect: "Feeds Volatiles — fuel & polymer feedstock." },
  bioharvester: { desc: "Harvests Biomass from the body's living deposit.", effect: "Feeds Biomass — food & polymer feedstock." },
  smelter: { desc: "Smelts Metallic Ore (+Fuel) into Alloys.", effect: "Unlocks Alloys production." },
  electronics_fabricator: { desc: "Fabricates Electronics from Rare Elements + Silicates.", effect: "Unlocks Electronics production." },
  chemical_works: { desc: "Processes Volatiles + Biomass into Polymers.", effect: "Unlocks Polymers production." },
  fuel_refinery: { desc: "Refines Volatiles into Fuel.", effect: "Unlocks Fuel — powers movement + smelting." },
  machine_works: { desc: "Builds Machinery from Alloys + Electronics + Fuel.", effect: "Unlocks Machinery — the build-cost backbone." },
  armaments_complex: { desc: "Assembles Armaments from Alloys + Electronics + Polymers.", effect: "Unlocks Armaments + on-site module manufacture." },
  shipyard: { desc: "An orbital yard that builds and fits warships here.", effect: "Gates ship construction (Convoy I, Raider/Corvette II)." },
  agroplex: { desc: "Grows Provisions from Biomass.", effect: "Feeds the colony — keeps it Well Supplied." },
  habitat: { desc: "Housing that lifts this body's population ceiling.", effect: "+population cap & workforce; boosts output when fed." },
  depot: { desc: "An orbital warehouse that raises storage capacity.", effect: "+400 storage cap; ships cargo to the hub." },
  sensor_array: { desc: "A standing sensor array over the system.", effect: "Projects a sensor bubble — see rivals sooner." },
  defense_platform: { desc: "Static defenses that fight raiders at the system.", effect: "+1 defense tier vs. attackers (can be worn down)." },
  academy: { desc: "Trains specialists and powers syndicate research.", effect: "Enables specialist training + a research contribution." },
};

type BuildOpt = { key: string; label: string; costs: { commodity: string; units: number }[]; build_secs: number };
interface StructOpt {
  o: BuildOpt; pool: Pool; currentTier: number; targetTier: number;
  foundsNew: boolean; tierUp: boolean;
  afford: boolean; poolFull: boolean; noDeposit: boolean;
  buildable: boolean; reason: string;
}
/// The sim-mirroring state for building `o` on `body` — current/target tier,
/// whether it founds a NEW slot vs. deepens in place, and every precondition
/// (afford / pool-full / matching-deposit). `buildable` = all pass (Queue is
/// live); `reason` is the first failing gate. Lifted out of the old inline build
/// rows so the list, the detail, and the button can never disagree.
function structOption(o: BuildOpt, dyn: SystemStateView, body: BodyView, pools: PoolUse): StructOpt {
  const pool = POOL_OF[o.key];
  const currentTier = (body.structures ?? {})[o.key] ?? 0;
  const pendingAhead = (dyn.builds ?? []).filter((j) => j.body_id === body.id && j.key === o.key).length;
  const foundsNew = currentTier === 0 && pendingAhead === 0;
  const targetTier = currentTier + pendingAhead + 1;
  const have = new Map((dyn.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const poolFull = foundsNew && !!pool && pools[pool].used >= pools[pool].total;
  const extractsFrom = EXTRACTION_OF[o.key];
  const noDeposit = foundsNew && !!extractsFrom && !(body.deposits ?? []).some((d) => extractsFrom.includes(d.resource as Commodity));
  const buildable = !poolFull && !noDeposit && afford;
  const reason = noDeposit ? "No matching deposit on this body — a mine only works its own rock."
    : poolFull ? `This body's ${POOL_LABEL[pool]} slots are full (${pools[pool].used}/${pools[pool].total}).`
      : !afford ? "Not enough goods stockpiled at this system." : "";
  return { o, pool, currentTier, targetTier, foundsNew, tierUp: !foundsNew, afford, poolFull, noDeposit, buildable, reason };
}
const romanTier = (n: number): string => ROMAN[n] ?? String(n);

// Build-panel state: which body it targets + the currently-selected structure.
let buildPanelBuilt = false;
let buildTargetBodyId: string | null = null; // the body BOTH builders target (shared)
let buildSelectedKey: string | null = null; // struct builder selection
let shipSelectedKind: string | null = null; // ship builder selection
let shipQty = 1; // ship builder quantity

// §build-panel: the shared SHELL for both builders (structures + ships) — same
// chrome, dock, and dimensions (`.build-shell` in the CSS). The two panels are
// siblings, never open together (opening one closes the other via closeBuildPanel).
function panelShellHtml(el: HTMLElement, eyebrow: string, title: string, chips: string, listHtml: string, detailHtml: string, footHtml: string): void {
  el.innerHTML =
    `<div class="bp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div><h2>${esc(title)}</h2></div></div>` +
    `<button class="pp-close" data-bp="close" title="Close" aria-label="Close">✕</button></div>` +
    `<div class="bp-pools">${chips}</div>` +
    `<div class="bp-body"><div class="bp-list">${listHtml}</div><div class="bp-detail">${detailHtml}</div></div>` +
    footHtml;
  el.classList.add("is-open");
}
function buildBuildPanel(): void {
  if (buildPanelBuilt) return;
  buildPanelBuilt = true;
  $("build-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-bp='close']")) { closeBuildPanel(); return; }
    const row = t.closest("[data-bp-row]") as HTMLElement | null;
    if (row) { buildSelectedKey = row.dataset.bpRow ?? null; renderBuildPanel(); return; }
    if (t.closest("[data-bp='queue']")) { queueSelectedBuild(); return; }
  });
  $("build-ship-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-bp='close']")) { closeBuildPanel(); return; }
    const qbtn = t.closest("[data-bp-qty]") as HTMLElement | null;
    if (qbtn) { shipQty = Math.max(1, Number(qbtn.dataset.bpQty) || 1); renderShipPanel(); return; }
    const row = t.closest("[data-bp-row]") as HTMLElement | null;
    if (row) { shipSelectedKind = row.dataset.bpRow ?? null; renderShipPanel(); return; }
    if (t.closest("[data-bp='queue']")) { queueSelectedShips(); return; }
  });
}
function openBuildPanel(bodyId: string): void {
  buildBuildPanel();
  // Toggle: the same body's builder re-clicked closes it (the button is a switch).
  const toggleOff = buildTargetBodyId === bodyId && $("build-panel").classList.contains("is-open");
  closeBuildPanel(); // also closes the ship builder — the two never coexist
  if (toggleOff) return;
  buildTargetBodyId = bodyId;
  renderBuildPanel();
}
function openShipPanel(bodyId: string): void {
  buildBuildPanel();
  const toggleOff = buildTargetBodyId === bodyId && $("build-ship-panel").classList.contains("is-open");
  closeBuildPanel(); // also closes the structure builder
  if (toggleOff) return;
  buildTargetBodyId = bodyId;
  shipQty = 1;
  renderShipPanel();
}
function closeBuildPanel(): void {
  buildTargetBodyId = null;
  buildSelectedKey = null;
  shipSelectedKind = null;
  shipQty = 1;
  $("build-panel").classList.remove("is-open");
  $("build-ship-panel").classList.remove("is-open");
}
function refreshBuildPanel(): void {
  if (!buildTargetBodyId) return;
  if ($("build-panel").classList.contains("is-open")) {
    if (renderDeferred("build-panel", refreshBuildPanel)) return; // §single-click
    renderBuildPanel();
  } else if ($("build-ship-panel").classList.contains("is-open")) {
    if (renderDeferred("build-ship-panel", refreshBuildPanel)) return;
    renderShipPanel();
  }
}
function queueSelectedBuild(): void {
  const sid = viewedSystemId();
  if (!sid || !net || !buildTargetBodyId || !buildSelectedKey) return;
  const dyn = state.systems.find((s) => s.id === sid);
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  const o = body ? buildOption(buildSelectedKey) : undefined;
  if (!dyn || !body || !o) return;
  const st = structOption(o, dyn, body, bodyPoolUsage(body, dyn));
  if (!st.buildable) return; // the button is disabled, but never trust the DOM
  // §byte-identical: exactly the DevelopSystem the inline buttons sent — the body
  // panel names its body, the sim soft-rejects on arrival as always.
  dispatchBuildKey(buildSelectedKey, sid, Number(buildTargetBodyId));
  readout().innerHTML =
    `Queued <b>${esc(o.label)}${st.tierUp ? ` ×${st.targetTier}` : ""}</b> on ${esc(body.name)} — ` +
    `it appears under construction. <span class="dim">A soft-reject (no slot / short on goods) shows in the Log.</span>`;
  buildSelectedKey = null; // clear so several can be queued back-to-back
  renderBuildPanel();
  refreshOpenBodyPanel(); // the queue row lands on the next View push
  updateSysviewManage();
}
function buildRowHtml(st: StructOpt): string {
  const sel = st.o.key === buildSelectedKey ? " is-sel" : "";
  const off = st.buildable ? "" : " is-off";
  const badge = st.foundsNew ? `<span class="bp-row-tier">new</span>` : `<span class="bp-row-tier">▲ ×${st.targetTier}</span>`;
  const short = st.noDeposit ? "no deposit" : st.poolFull ? "pool full" : !st.afford ? "short on goods" : "";
  const reason = short ? `<span class="bp-row-reason">${short}</span>` : "";
  return `<button class="bp-row${sel}${off}" data-bp-row="${st.o.key}" title="${esc(st.buildable ? st.o.label : st.reason)}">` +
    `<span class="bp-row-ic">${icon(STRUCT_ICON[st.o.key] ?? "build", "sm")}</span>` +
    `<span class="bp-row-name">${esc(st.o.label)}</span>${badge}${reason}</button>`;
}
function buildDetailHtml(o: BuildOpt, dyn: SystemStateView, body: BodyView, pools: PoolUse): string {
  const st = structOption(o, dyn, body, pools);
  const info = STRUCT_INFO[o.key] ?? { desc: "", effect: "" };
  const have = new Map((dyn.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const costRows = o.costs.map((c) => {
    const has = have.get(c.commodity as Commodity) ?? 0;
    const shortC = has < c.units;
    return `<div class="bp-cost-row${shortC ? " is-short" : ""}">` +
      `<span class="bp-cost-c">${commodityIcon(c.commodity as Commodity, "sm")} ${esc(label(c.commodity))}</span>` +
      `<span class="bp-cost-n">${c.units} <span class="bp-cost-have">have ${has}</span></span></div>`;
  }).join("");
  const slotLine = st.foundsNew
    ? `Claims a <b>${POOL_LABEL[st.pool]}</b> slot — ${pools[st.pool].used} → ${pools[st.pool].used + 1} / ${pools[st.pool].total}.`
    : `Deepens in place — no new slot consumed.`;
  const from = Math.min(4, st.targetTier - 1), to = Math.min(4, st.targetTier);
  let framing: string;
  if (st.tierUp) {
    let delta = "";
    if (THROUGHPUT_STRUCTS.has(o.key) && TIER_THROUGHPUT[from] && TIER_THROUGHPUT[to]) {
      const pct = Math.round((TIER_THROUGHPUT[to] / TIER_THROUGHPUT[from] - 1) * 100);
      delta = ` Throughput ×${TIER_THROUGHPUT[from]} → ×${TIER_THROUGHPUT[to]} <span class="tone-up">(+${pct}%)</span>.`;
    }
    framing = `<div class="bp-upgrade"><b>Upgrade</b> — Tier ${romanTier(st.targetTier - 1)} → ${romanTier(st.targetTier)}.${delta}</div>`;
  } else {
    framing = `<div class="bp-upgrade"><b>New structure</b> — founds Tier I.</div>`;
  }
  return `<div class="bp-d-head">${icon(STRUCT_ICON[o.key] ?? "build", "md")} <b>${esc(o.label)}</b>` +
    `<span class="bp-d-tier">${st.foundsNew ? "new" : `→ ×${st.targetTier}`}</span></div>` +
    `<div class="bp-d-desc">${esc(info.desc)}</div>${framing}` +
    `<div class="bp-d-sec">Recipe — required vs. this system's stock</div><div class="bp-costs">${costRows}</div>` +
    (st.afford ? "" : `<div class="bp-d-warn">Short on goods — it waits (or soft-rejects) until the stockpile covers it.</div>`) +
    (st.noDeposit ? `<div class="bp-d-warn">No matching deposit on this body — found it on a body that has one.</div>` : "") +
    `<div class="bp-d-sec">Build time</div><div class="bp-d-line">${icon("time", "sm")} ${Math.round(o.build_secs)}s at this system.</div>` +
    `<div class="bp-d-sec">Slot</div><div class="bp-d-line">${slotLine}</div>` +
    `<div class="bp-d-sec">Enables</div><div class="bp-d-line">${esc(info.effect)}</div>`;
}
function renderBuildPanel(): void {
  const el = $("build-panel");
  if (!buildTargetBodyId) { el.classList.remove("is-open"); return; }
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  if (!dyn || !body) { closeBuildPanel(); return; }
  const pools = bodyPoolUsage(body, dyn);
  const poolChips = (["resource", "industrial", "infrastructure"] as Pool[]).map((k) => {
    const p = pools[k];
    return `<span class="bp-pool${p.used >= p.total ? " is-full" : ""}" title="${POOL_LABEL[k]} slots used / total on this body">${POOL_LABEL[k]} ${p.used}/${p.total}</span>`;
  }).join("");
  // LEFT: every non-ship structure buildable here, grouped by slot pool. Rows that
  // fail a gate still render, greyed, with the reason inline.
  const opts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => !SHIP_KEYS.has(o.key) && !!POOL_OF[o.key]);
  const byPool: Record<Pool, StructOpt[]> = { resource: [], industrial: [], infrastructure: [] };
  for (const o of opts) byPool[POOL_OF[o.key]].push(structOption(o, dyn, body, pools));
  const groups = (["resource", "industrial", "infrastructure"] as Pool[]).map((k) => {
    if (!byPool[k].length) return "";
    return `<div class="bp-group">${POOL_LABEL[k]} <span class="bp-group-n">${pools[k].used}/${pools[k].total}</span></div>` +
      byPool[k].map(buildRowHtml).join("");
  }).join("");
  const selOpt = buildSelectedKey ? buildOption(buildSelectedKey) : undefined;
  const detail = selOpt
    ? buildDetailHtml(selOpt as BuildOpt, dyn, body, pools)
    : `<div class="bp-detail-empty">Select a structure to see its recipe, build time, slot, and effect.</div>`;
  // FOOTER: Queue + a live note of what's already queued on THIS body.
  const selSt = selOpt ? structOption(selOpt as BuildOpt, dyn, body, pools) : null;
  const canQueue = !!selSt && selSt.buildable;
  const qTip = !selSt ? "Select a structure first." : selSt.buildable ? "Queue this build — draws from the system stockpile." : selSt.reason;
  const queued = (dyn.builds ?? []).filter((j) => j.body_id === body.id && !SHIP_KEYS.has(j.key));
  const queuedNote = queued.length
    ? `Already queued here: <b>${queued.map((j) => esc(buildLabel(j.key))).join(", ")}</b>.`
    : "Nothing queued on this body yet.";
  const foot = `<div class="bp-foot"><button class="act bp-queue" data-bp="queue" ${canQueue ? "" : "disabled"} title="${esc(qTip)}">${icon("build", "sm")} Queue build</button>` +
    `<div class="bp-queued">${queuedNote}</div></div>`;
  panelShellHtml(el, "build", `Build on ${body.name}`, poolChips, groups, detail, foot);
}

// ---- §build-ship-panel: the dedicated SHIP builder (mirrors the structure
// builder; shares its shell/dock/breakpoint via `.build-shell`). Opened from the
// shipyard body's "Build ship" button; a sibling of the structure builder — the
// two never render together. Client-only over the SAME BuildShip command. ----
const SHIP_ORDER = ["scout", "corvette", "raider", "convoy", "colony"];
const SHIP_HULL_ICON: Record<string, IconKey> = { convoy: "convoy", raider: "raider", corvette: "corvette", colony: "colony", scout: "scout" };
// Per-hull stats + one-line role, mirroring crates/sim/src/ship.rs (speed / hull
// mass / attack+defense weights / module slots). Display only — the COSTS + gates
// that matter for the command come from build_options + SHIP_REQ.
const SHIP_STATS: Record<string, { role: string; speed: number; hull: number; atk: number; def: number; slots: number; cap: string }> = {
  scout: { role: "Eyes of the fleet — fastest hull, gathers intel; unarmed, dies if caught.", speed: 115, hull: 80, atk: 0, def: 0, slots: 1, cap: "No cargo · widest sensor bubble" },
  corvette: { role: "Armored escort/garrison — built to be shot at; too slow to chase raiders.", speed: 65, hull: 800, atk: 1, def: 4, slots: 2, cap: "No cargo · screens convoys" },
  raider: { role: "The hunter — fast and hard-hitting; seizes a convoy's cargo on a won raid.", speed: 100, hull: 200, atk: 3, def: 2, slots: 2, cap: "No cargo · takes prizes" },
  convoy: { role: "Bulk hauler — carries goods to the hub; raidable, wants an escort.", speed: 40, hull: 4500, atk: 0, def: 1, slots: 0, cap: "Hauls cargo (raidable)" },
  colony: { role: "Settlement ship — carries colonists to physically claim a system.", speed: 33, hull: 6000, atk: 0, def: 1, slots: 0, cap: "Carries a colony (one claim)" },
};
interface ShipOpt { o: BuildOpt; needTier: number; yardTier: number; yardShort: boolean; afford: boolean; maxAff: number; buildable: boolean; reason: string; }
/// Ship gating mirrored from the sim: shipyard-tier gate (SHIP_REQ vs the system's
/// shipyard tier — the same field the old inline rows read) + afford, plus the
/// max affordable count for the quantity stepper. `buildable` = tier ok + affords 1.
function shipOption(o: BuildOpt, dyn: SystemStateView): ShipOpt {
  const have = new Map((dyn.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const yardTier = dyn.shipyard_tier ?? 0;
  const needTier = SHIP_REQ[o.key] ?? 1;
  const yardShort = yardTier < needTier;
  const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
  const maxAff = o.costs.length
    ? Math.max(0, Math.min(...o.costs.map((c) => Math.floor((have.get(c.commodity as Commodity) ?? 0) / c.units))))
    : 0;
  const buildable = !yardShort && afford;
  const reason = yardShort ? `Needs Shipyard tier ${needTier} (have ${yardTier}).` : !afford ? "Not enough goods stockpiled at this system." : "";
  return { o, needTier, yardTier, yardShort, afford, maxAff, buildable, reason };
}
/// The staffed-shipyard build-time multiplier (mirrors the sim: build_ticks /
/// (1 + SHIPYARD_BOOST·staffing·skill), SHIPYARD_BOOST = 0.25). 1.0 when the yard
/// has no crew posted here (the AssignmentView carries the resolved factors).
function shipyardBoost(dyn: SystemStateView, body: BodyView): number {
  const a = (dyn.assignments ?? []).find((x) => x.body_id === body.id && x.structure === "shipyard");
  return a ? 1 + 0.25 * a.staffing * a.skill : 1;
}
function shipRowHtml(st: ShipOpt): string {
  const sel = st.o.key === shipSelectedKind ? " is-sel" : "";
  const off = st.buildable ? "" : " is-off";
  const info = SHIP_STATS[st.o.key];
  const short = st.yardShort ? `needs yard ${romanTier(st.needTier)}` : !st.afford ? "short on goods" : "";
  const reason = short ? `<span class="bp-row-reason">${short}</span>` : "";
  return `<button class="bp-row bp-ship-row${sel}${off}" data-bp-row="${st.o.key}" title="${esc(info?.role ?? st.o.label)}">` +
    `<span class="bp-row-ic">${icon(SHIP_HULL_ICON[st.o.key] ?? "fleet", "sm")}</span>` +
    `<span class="bp-ship-main"><span class="bp-ship-top"><span class="bp-row-name">${esc(st.o.label)}</span>` +
    `<span class="bp-row-tier">${Math.round(st.o.build_secs)}s</span>${reason}</span>` +
    `<span class="bp-ship-role">${esc(info?.role ?? "")}</span></span></button>`;
}
function shipDetailHtml(o: BuildOpt, dyn: SystemStateView, body: BodyView): string {
  const st = shipOption(o, dyn);
  const info = SHIP_STATS[o.key];
  const q = Math.max(1, shipQty);
  const have = new Map((dyn.stockpile ?? []).map((s) => [s.commodity, s.units]));
  // QUANTITY stepper — 1 / 5 / 10 / max-affordable; the total cost + time track it.
  const qbtn = (n: number, lbl: string) => `<button class="bp-qty-btn${q === n ? " is-on" : ""}" data-bp-qty="${n}" ${n < 1 ? "disabled" : ""}>${lbl}</button>`;
  const stepper = `<div class="bp-qty"><span class="bp-qty-lbl">Quantity</span>${qbtn(1, "1")}${qbtn(5, "5")}${qbtn(10, "10")}${qbtn(st.maxAff, `Max ${st.maxAff}`)}<span class="bp-qty-cur">building <b>${q}</b></span></div>`;
  // Cost table — the TOTAL (unit × q) reads largest; unit shown small alongside.
  const costRows = o.costs.map((c) => {
    const need = c.units * q;
    const has = have.get(c.commodity as Commodity) ?? 0;
    const short = has < need;
    return `<div class="bp-cost-row${short ? " is-short" : ""}">` +
      `<span class="bp-cost-c">${commodityIcon(c.commodity as Commodity, "sm")} ${esc(label(c.commodity))}</span>` +
      `<span class="bp-cost-n"><b class="bp-cost-total">${need}</b>${q > 1 ? ` <span class="bp-cost-mul">${c.units}×${q}</span>` : ""} <span class="bp-cost-have">have ${has}</span></span></div>`;
  }).join("");
  const affordsQ = q <= st.maxAff;
  // Build time — per-ship at this yard's current throughput (staffed-yard bonus
  // shown; the shown-math law). N hulls build in PARALLEL, so the batch time == 1.
  const boost = shipyardBoost(dyn, body);
  const per = Math.max(1, Math.round(o.build_secs / boost));
  const timeLine = boost > 1.001
    ? `${icon("time", "sm")} <b>${per}s</b> each — staffed-yard bonus ×${boost.toFixed(2)} (base ${Math.round(o.build_secs)}s).${q > 1 ? ` The ${q} build in parallel.` : ""}`
    : `${icon("time", "sm")} <b>${per}s</b> each${q > 1 ? ` · the ${q} build in parallel` : ""}. <span class="dim">Post crew to the Shipyard to build faster.</span>`;
  const stat = (lbl: string, val: string) => `<div class="bp-stat"><span class="bp-stat-l">${lbl}</span><span class="bp-stat-v">${val}</span></div>`;
  const stats = info ? `<div class="bp-stats">${stat("Speed", `${info.speed}`)}${stat("Hull mass", `${info.hull}`)}${stat("Attack", `${info.atk}`)}${stat("Defense", `${info.def}`)}${stat("Module slots", `${info.slots}`)}${stat("Fuel", info.hull >= 1000 ? "heavy (∝ mass)" : "light (∝ mass)")}</div>` : "";
  return `<div class="bp-d-head">${icon(SHIP_HULL_ICON[o.key] ?? "fleet", "md")} <b>${esc(o.label)}</b><span class="bp-d-tier">${st.yardShort ? `needs yard ${romanTier(st.needTier)}` : `${info?.slots ?? 0} slots`}</span></div>` +
    `<div class="bp-d-desc">${esc(info?.role ?? "")}</div>` +
    stepper +
    `<div class="bp-d-sec">Recipe — total for ${q}, vs. this system's stock</div><div class="bp-costs">${costRows}</div>` +
    (st.yardShort ? `<div class="bp-d-warn">Requires Shipyard tier ${romanTier(st.needTier)} here — this system's yard is tier ${romanTier(st.yardTier)}.</div>` : "") +
    (!affordsQ ? `<div class="bp-d-warn">The stockpile covers ${st.maxAff} right now — queue that many, or wait for production.</div>` : "") +
    `<div class="bp-d-sec">Build time</div><div class="bp-d-line">${timeLine}</div>` +
    `<div class="bp-d-sec">Stats</div>${stats}<div class="bp-d-line" style="margin-top:4px">${esc(info?.cap ?? "")}</div>`;
}
function queueSelectedShips(): void {
  const sid = viewedSystemId();
  if (!sid || !net || !buildTargetBodyId || !shipSelectedKind) return;
  const dyn = state.systems.find((s) => s.id === sid);
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  const o = body ? buildOption(shipSelectedKind) : undefined;
  if (!dyn || !body || !o) return;
  const st = shipOption(o, dyn);
  const q = Math.min(Math.max(1, shipQty), st.maxAff);
  if (st.yardShort || q < 1) return; // the button is disabled, but never trust the DOM
  // §byte-identical: N × the exact BuildShip the inline row sent (the loadout comes
  // from the yard's fit picker via dispatchBuildKey, clamped per hull as before).
  for (let i = 0; i < q; i++) dispatchBuildKey(shipSelectedKind, sid);
  readout().innerHTML =
    `Queued <b>${q}× ${esc(o.label)}</b> at ${esc(body.name)} — building at the orbital yard. ` +
    `<span class="dim">Spawns here; a fuel-short or over-queued build shows in the Log.</span>`;
  shipQty = 1; // reset for the next batch (mix a Corvette + two Convoys without reopening)
  renderShipPanel();
  refreshOpenBodyPanel();
  updateSysviewManage();
}
function renderShipPanel(): void {
  const el = $("build-ship-panel");
  if (!buildTargetBodyId) { el.classList.remove("is-open"); return; }
  const sid = viewedSystemId();
  const dyn = sid ? state.systems.find((s) => s.id === sid) : undefined;
  const body = dyn?.bodies?.find((b) => String(b.id) === buildTargetBodyId);
  if (!dyn || !body) { closeBuildPanel(); return; }
  const yardTier = dyn.shipyard_tier ?? 0;
  const chip = `<span class="bp-pool">${icon("shipyard", "sm")} Shipyard ${romanTier(yardTier)}</span>`;
  const opts = ((state.galaxy?.build_options ?? []) as BuildOpt[]).filter((o) => SHIP_KEYS.has(o.key));
  const ordered = SHIP_ORDER.map((k) => opts.find((o) => o.key === k)).filter((o): o is BuildOpt => !!o);
  const list = ordered.map((o) => shipRowHtml(shipOption(o, dyn))).join("");
  const selOpt = shipSelectedKind ? buildOption(shipSelectedKind) : undefined;
  const detail = selOpt
    ? shipDetailHtml(selOpt as BuildOpt, dyn, body)
    : `<div class="bp-detail-empty">Select a hull to see its recipe, stats, and build time. Set a quantity, then queue the batch.</div>`;
  const selSt = selOpt ? shipOption(selOpt as BuildOpt, dyn) : null;
  const q = Math.max(1, shipQty);
  const canQueue = !!selSt && !selSt.yardShort && q >= 1 && q <= selSt.maxAff;
  const qTip = !selSt ? "Select a hull first." : selSt.yardShort ? selSt.reason : q > selSt.maxAff ? `The stockpile covers ${selSt.maxAff} right now.` : "Queue this batch — draws from the system stockpile.";
  // The yard's line: every ship job in the SYSTEM (ships build at the best yard).
  const queued = (dyn.builds ?? []).filter((j) => SHIP_KEYS.has(j.key));
  const queuedNote = queued.length ? `At the yard: <b>${queued.map((j) => esc(buildLabel(j.key))).join(", ")}</b>.` : "Nothing at the yard yet.";
  const foot = `<div class="bp-foot"><button class="act bp-queue" data-bp="queue" ${canQueue ? "" : "disabled"} title="${esc(qTip)}">${icon("build", "sm")} Queue build${selSt && q > 1 ? ` ×${q}` : ""}</button>` +
    `<div class="bp-queued">${queuedNote}</div></div>`;
  panelShellHtml(el, "build ship", `Build at ${body.name}`, chip, list, detail, foot);
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
    const el = (e.target as HTMLElement).closest("[data-action],[data-sys],[data-build],[data-crew]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.sys) {
      state.selectedSystemId = el.dataset.sys; // re-selects; map highlights it too
      updateSystemTab();
      return;
    }
    const sid = state.selectedSystemId;
    if (!sid || !net) return;
    if (el.dataset.crew) {
      sendCrew(sid, el.dataset.crew);
      return;
    }
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
          `Shipping <b>${manifest.map((s) => `${s.units} ${esc(label(s.commodity))}`).join(", ")}</b> → hub — ` +
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
function dispatchBuildKey(k: string, sid: string, bodyId?: number): void {
  if (!net) return;
  if (k === "convoy" || k === "raider" || k === "corvette" || k === "colony" || k === "scout") {
    // §modules Part B4: a warship build carries the composed FIT, clamped to this
    // hull's module slots (so a 2-module fit on a 1-slot scout sends just 1, not a
    // silent server reject). The ledger is debited server-side.
    const fit = pendingFit.filter((m) => (moduleLedgerAt(sid)[m] ?? 0) > 0).slice(0, MODULE_SLOTS[k] ?? 0);
    net.send({ type: "BuildShip", system_id: sid, ship_kind: k, loadout: fit.length ? fit : undefined });
  }
  // §modules Part B3: "module:<slug>" → manufacture into the system ledger.
  else if (k.startsWith("module:")) net.send({ type: "BuildModule", system_id: sid, module: k.slice(7) as ModuleKind });
  // §bodies: the body panel names its body; omitted → the sim auto-sites.
  else net.send({ type: "DevelopSystem", system_id: sid, upgrade: k, body_id: bodyId }); // §economy: any structure slug
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
    if ((dyn?.population ?? 0) > 0 && !dyn?.habitat_fed) cues.push(`${badge("warn", label(dyn?.food_state ?? "rationing"))} workforce slowed — ship provisions`);
    if (dyn?.node?.awakened && !dyn.node.fed) cues.push(`${badge("warn", "node unfed")} bonus suspended — ship its upkeep`);
    // §build-progress: the compact construction line — a glance from the map
    // says work is running (and when the next job lands) without opening the view.
    const jobs = dyn?.builds ?? [];
    if (jobs.length === 1) {
      cues.push(`${svgIcon("action-build", "sm")} building: <b>${esc(buildLabel(jobs[0].key))}</b> — ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
    } else if (jobs.length > 1) {
      cues.push(`${svgIcon("action-build", "sm")} building ×${jobs.length} — next ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
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
// §market-ux: which Market tab is showing — survives close/reopen within the
// session (M reopens on the last tab).
type MarketTab = "exchange" | "specialists" | "modules";
let marketTab: MarketTab = "exchange";
function setMarketTab(tab: MarketTab): void {
  marketTab = tab;
  ($("market-pane-exchange") as HTMLElement).hidden = tab !== "exchange";
  ($("market-pane-specialists") as HTMLElement).hidden = tab !== "specialists";
  ($("market-pane-modules") as HTMLElement).hidden = tab !== "modules";
  document.querySelectorAll<HTMLElement>("#market-tabs button").forEach((b) =>
    b.classList.toggle("is-active", b.dataset.mtab === tab));
  updateMarket();
}
function buildMarketPanel(): void {
  if (marketBuilt) return;
  marketBuilt = true;
  // §market-ux: Exchange / Specialists tabs.
  $("market-tabs").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-mtab]") as HTMLElement | null;
    if (b?.dataset.mtab) setMarketTab(b.dataset.mtab as MarketTab);
  });
  // §modules Part B3: the Sol MODULE market — buy ships a crate to your home
  // (price-certain, delivery-risky); sell dispatches from home, clears on arrival.
  $("market-pane-modules").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-mbuy],[data-msell]") as HTMLElement | null;
    if (!b || !net) return;
    const home = state.systems.find((s) => s.owner === state.playerId)?.id;
    if (!home) return;
    if (b.dataset.mbuy) {
      net.send({ type: "BuyModule", module: b.dataset.mbuy as ModuleKind, n: 1, dest_system: home });
      $("mod-feedback").textContent = `Buying a ${MODULE_LABEL[b.dataset.mbuy as ModuleKind]} from Sol — crate convoy inbound to your home (raidable).`;
    } else if (b.dataset.msell) {
      net.send({ type: "SellModule", module: b.dataset.msell as ModuleKind, n: 1, from_system: home });
      $("mod-feedback").textContent = `Selling a ${MODULE_LABEL[b.dataset.msell as ModuleKind]} to Sol — convoy away, clears on arrival.`;
    }
  });
  // §economy Part 6: a Sol specialist contract → HireSpecialist to the home.
  // Lives on the Specialists pane; feedback lands where the player is looking.
  $("market-pane-specialists").addEventListener("click", (e) => {
    const h = (e.target as HTMLElement).closest("[data-hire]") as HTMLElement | null;
    if (!h || !net) return;
    // Ships to the first owned system (the home — always held).
    const dest = state.systems.find((s) => s.owner === state.playerId)?.id;
    if (!dest) return;
    net.send({ type: "HireSpecialist", specialist: h.dataset.hire!, dest_system: dest });
    $("sp-feedback").textContent = `Contract signed — a ${label(h.dataset.hire!)} ships out from Sol.`;
  });
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
    $("mk-feedback").textContent = `Order sent: ${composer.side} ${qty} ${label(c)}${limitOn && limitPrice > 0 ? ` @ ${limitPrice}` : ""}.`;
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
    return `<button class="board__row ${active}" data-resource="${c}" title="your observed price history">` +
      `<span class="dep-ico">${commodityIcon(c, "md")}</span>` +
      `<span class="b-name">${label(c)}</span>` +
      spark(hist.length ? hist : (p !== undefined ? [p, p] : [0, 0])) +
      `<span class="b-price ${stale ? "is-stale" : ""}">${priceTxt} <span class="b-trend ${tr.tone}">${tr.glyph}</span></span>` +
      `<span class="b-held">${heldOf.get(c) ?? 0}</span></button>`;
  }).join("");
}

// §economy Part 6 → §market-ux: SOL SPECIALIST CONTRACTS, now a Market TAB of
// their own — five professions at the standing price; the contractor ships to
// the player's HOME on a normal raidable personnel convoy (price-certain,
// delivery-risky). Wire slugs stay raw in data-hire; names come from label().
const SPECIALISTS: { slug: string; icon: IconKey; blurb: string }[] = [
  { slug: "geologist", icon: "extractor", blurb: "mineral extraction" },
  { slug: "petrochemical_engineer", icon: "refinery", blurb: "volatiles, fuel, chemicals" },
  { slug: "xenobiologist", icon: "provisions", blurb: "biomass + agroplex" },
  { slug: "industrial_engineer", icon: "build", blurb: "heavy industry" },
  { slug: "naval_architect", icon: "shipyard", blurb: "shipyards + armaments" },
];
function renderSpecialistsPane(): void {
  const cost = state.galaxy?.specialist_hire_cost ?? 800;
  const credits = state.wallet?.credits ?? 0;
  const rows = SPECIALISTS.map((s) =>
    `<div class="board__row" title="A specialist multiplies affine production lines ×1.75 when posted. The personnel convoy from Sol is sub-light and raidable.">` +
    `<span class="dep-ico">${icon(s.icon, "sm")}</span>` +
    `<span class="b-name">${esc(label(s.slug))}</span>` +
    `<span class="dim">${esc(s.blurb)}</span>` +
    `<span class="b-price">${cost.toFixed(0)} cr</span>` +
    `<button class="act" data-hire="${s.slug}" ${credits >= cost ? "" : "disabled"}>Hire</button>` +
    `</div>`).join("");
  // Rows only — #sp-feedback is a STATIC sibling so the 10 Hz view refresh
  // never wipes a just-shown hire confirmation.
  $("sp-rows").innerHTML =
    `<div class="mhint" style="margin-bottom:6px">Standing Sol contracts — the specialist ships to your <b>home system</b>; post them to a matching line from the colony panel.</div>` +
    rows;
}

// §modules Part B3: the SOL MODULE MARKET tab — buy each module at a premium
// (crate ships to your home, raidable) or sell it back low (convoy → hub, clears
// on arrival). Prices are computed client-side from the recipe × observed hub
// prices (the sim's own basis), shown "~" because the server prices on execution.
// The home ledger count gates Sell (you can only sell what you hold at home).
function renderModulesPane(): void {
  const credits = state.wallet?.credits ?? 0;
  const home = state.systems.find((s) => s.owner === state.playerId);
  const ledger = home?.modules ?? {};
  const rows = MODULE_ALL.map((m) => {
    const v = moduleRecipeValue(m);
    const buy = v === null ? null : v * MODULE_BUY_MULT;
    const sell = v === null ? null : v * MODULE_SELL_MULT;
    const held = ledger[m] ?? 0;
    const buyTxt = buy === null ? "—" : `~${buy.toFixed(0)} cr`;
    const sellTxt = sell === null ? "—" : `~${sell.toFixed(0)} cr`;
    const canBuy = buy !== null && credits >= buy && !!home;
    return `<div class="board__row" title="${esc(MODULE_TIP[m])}">` +
      `<span class="dep-ico">${MODULE_GLYPH[m]}</span>` +
      `<span class="b-name">${esc(MODULE_LABEL[m])}${held ? ` <span class="dim">·held ${held}</span>` : ""}</span>` +
      `<button class="act" data-mbuy="${m}" ${canBuy ? "" : "disabled"} title="Buy one from Sol → ships a crate to your home (raidable).">Buy ${buyTxt}</button>` +
      `<button class="act" data-msell="${m}" ${held > 0 ? "" : "disabled"} title="Sell one from your home ledger → convoy to Sol, clears on arrival.">Sell ${sellTxt}</button>` +
      `</div>`;
  }).join("");
  $("mod-rows").innerHTML =
    `<div class="mhint" style="margin-bottom:6px">Sol's off-map foundry — buy modules at a premium (a crate ships to your <b>home</b>, raidable) or sell your home ledger back at a discount. Prices track the commodity market; local manufacture at an Armaments Complex is always cheaper.</div>` +
    rows;
}

// The composer preview surfaces the buy/sell asymmetry in plain language — the
// honest-fog centerpiece (teaches the lightspeed economy, not shipping fees).
function renderComposer(): void {
  if (!state.market) return;
  const c = composer.commodity;
  const price = state.market.prices.find((p) => p.commodity === c)?.price;
  const stale = state.market.staleness > 0.5;
  const px = price !== undefined ? `${stale ? "~" : ""}${price.toFixed(2)}` : "?";
  $("mk-sel").textContent = label(c);
  document.querySelectorAll<HTMLElement>("#mk-side button").forEach((b) => b.classList.toggle("is-active", b.dataset.side === composer.side));
  const qty = Math.max(1, Math.floor(Number(($("mk-qty") as HTMLInputElement).value) || 0));
  const limitOn = ($("mk-limit-on") as HTMLInputElement).checked;
  const submit = $("mk-submit");
  if (limitOn) {
    $("mk-preview").innerHTML = `<span title="It rests on the book and clears in the periodic uniform-price batch — reacting fastest confers no edge; partial fills carry to the next batch."><b>Limit ${composer.side} ${qty} ${label(c)}</b> → rests, clears in the <span class="accent">batch</span></span>`;
    submit.textContent = `Place limit ${composer.side}`;
  } else if (composer.side === "buy") {
    const cost = price !== undefined ? fmt(qty * price) : "?";
    $("mk-preview").innerHTML = `<span title="Settles instantly; the goods then cross fogged space to your home anchor as a delivery convoy — raidable in transit.">Settles <b>now</b> ~<span class="accent">${cost} Cr</span> → ${icon("convoy", "sm")} <b>raidable</b> delivery</span>`;
    submit.textContent = `Buy ${qty} ${label(c)}`;
  } else {
    $("mk-preview").innerHTML = `<span title="A convoy is dispatched now; it clears at the price ON ARRIVAL (not today's ${px}) and is raidable until it reaches the hub — double uncertainty: price + delivery.">${icon("convoy", "sm")} <b>dispatched now</b> → clears at price <b>on arrival</b> · <b>raidable</b></span>`;
    submit.textContent = `Sell ${qty} ${label(c)}`;
  }
}

function renderRestingOrders(): void {
  const orders = state.wallet?.orders ?? [];
  $("market-orders").innerHTML = orders.length
    ? `<div class="deps-head">Resting limit orders</div>` +
      orders.map((o) => `<div class="ord">${badge(o.side === "buy" ? "positive" : "warn", `${o.side} ${o.units} ${label(o.commodity)} @ ${o.limit_price.toFixed(1)}`)}</div>`).join("")
    : "";
}

function updateMarket(): void {
  if (renderDeferred("market", updateMarket)) return; // §single-click
  if (!state.market || !state.wallet) return;
  const stale = state.market.staleness;
  const fresh = $("market-fresh");
  fresh.className = "badge " + (stale > 0.5 ? "badge--warn" : "badge--positive");
  fresh.textContent = stale > 0.5 ? `~${stale.toFixed(0)}s stale` : "live";
  fresh.title = "Last-synced ticker — light-delayed";
  $("market-wallet").innerHTML = statStrip([
    stat("Credits", `${fmt(state.wallet.credits)} Cr`, "is-accent"),
    stat("Equity", `${fmt(state.wallet.valuation)} Cr`),
  ]);
  renderMarketBoard();
  renderComposer();
  renderRestingOrders();
  renderSpecialistsPane();
  renderModulesPane();
}

function addTradeNews(t: TradeEvent): void {
  const log = $("reports-log");
  let text = "";
  switch (t.event) {
    case "Bought": text = `Bought ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)} — delivery convoy inbound (raidable).`; break;
    case "Delivered": text = `Delivery arrived: +${t.units} ${label(t.commodity)} (stored at destination).`; break;
    case "SellDispatched": text = `Sell convoy away: ${t.units} ${label(t.commodity)} crossing to the hub.`; break;
    case "Sold": text = `Sold ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)} on arrival.`; break;
    case "LimitPlaced": text = `Limit ${t.side} ${t.units} ${label(t.commodity)} @ ${t.limit_price.toFixed(2)} resting on the book.`; break;
    case "LimitFilled": text = `Limit ${t.side} filled in batch: ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)}.`; break;
    case "AutoDispatched": text = `⚙ Standing order #${t.rule_id} shipped ${t.units} ${label(t.commodity)} (auto, raidable).`; break;
    case "SupplyDiverted": {
      const what = t.action === "lost" ? "lost (cargo dropped)"
        : t.action === "returned_home" ? "re-routed home (raidable)"
        : "re-routed to sell at the hub (raidable)";
      text = `⚠ Supply to ${systemName(t.system)} undeliverable — you no longer hold it: ${t.units} ${label(t.commodity)} ${what}.`;
      break;
    }
    case "StorageOverflow":
      text = `⚠ Depot full at ${systemName(t.system)}: ${t.units} ${label(t.commodity)} couldn't be stored — convoy carries it on to sell at the hub (raidable).`;
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
  if (!comSel.options.length) comSel.innerHTML = COMMODITIES.map((c) => `<option value="${c}">${label(c)}</option>`).join("");

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
        `<b>#${o.id}</b> ${commodityIcon(o.commodity, "sm")} ${label(o.commodity)}: ${endpointLabel(o.source)} → ${endpointLabel(o.dest)}${paused}<br>` +
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
        headline: `${systemName(s.id)} — food ${label(s.food_state ?? "rationing").toUpperCase()}`,
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
      .map((d) => `${label(d.resource)} ~${d.richness.toFixed(1)}/s`)
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
        case "GalaxyUpdate":
          // §over-capacity homes: the star chart grew after our Welcome (a new
          // corp's freshly-minted home). Fresh OBJECT identity on purpose — the
          // renderer re-ingests the galaxy on the next frame by identity check.
          if (state.galaxy) state.galaxy = { ...state.galaxy, systems: msg.systems };
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
          state.battleRecords = msg.battle_records ?? []; // §battle-records: replay timelines
          state.syndicate = msg.syndicate ?? null;
          state.syndicateInvites = msg.syndicate_invites ?? [];
          state.rankings = msg.rankings ?? [];
          state.research = msg.research ?? null;
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
          // §research R6: refresh the Programme Boards if open (coarse signature —
          // the progress bar animates ~1 Hz, node states update as they change).
          if ($("research-panel").classList.contains("is-open")) updateResearchPanel();
          // §management-home: inside the System View, refresh the management
          // column + the structure markers (a cached no-op unless tiers changed).
          updateSysviewDynamic();
          // §one-battle-one-icon: keep an open ongoing-battle panel live (elapsed,
          // echo countdowns, running composition; auto-closes when it concludes).
          if (openOngoingBattleId !== null && $("battle-panel").classList.contains("is-open")) updateOngoingBattlePanel();
          // §battle-records: keep an open replay viewer live — rounds grow, the
          // light frontier advances, the outcome may arrive (guards itself).
          refreshOpenBattleViewer();
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
