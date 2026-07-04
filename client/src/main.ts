// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { countClassLabel, formatId, type Commodity, type Deposit, type FleetDoctrine, type GhostView, type PendingOrderView, type ShipKind, type Side, type StandingEndpoint, type StandingOrder, type StandingTrigger, type StockSlot, type SystemInfo, type SystemStateView, type TimelineEntry, type TradeEvent } from "./protocol";
import { starConceptUrl, starTypeFor } from "./stars";
import type { SystemBodyDetail } from "./systemview";

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
  provisions: 6, ore: 8, fuel: 10, volatiles: 18, alloys: 26,
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
const uiIcon = (slug: string, size = 16, cls = "") =>
  `<img class="cicon ${cls}" src="/art/ui_icons/svg/${slug}.svg" width="${size}" height="${size}" alt="" />`;

// Commodity → resource icon. Exact where the set has one (the SVG accent colors
// even match the map tints: metals=bronze=ore, industrials=purple=alloys,
// supplies=green=provisions, fuel=fuel). Volatiles has NO native icon → it reuses
// Fuel, hue-shifted cold, until it gets dedicated art (see README). Credits stay
// the text label "Cr" (no icon).
const RESOURCE_SLUG: Record<Commodity, string> = {
  fuel: "resource-fuel",
  ore: "resource-metals",
  provisions: "resource-supplies",
  alloys: "resource-industrials",
  volatiles: "resource-fuel", // STAND-IN (hue-shifted cold) — wants dedicated art
};
const commodityIcon = (c: Commodity, size = 18) =>
  `<img class="cicon${c === "volatiles" ? " cicon--cold" : ""}" src="/art/ui_icons/svg/${RESOURCE_SLUG[c]}.svg" width="${size}" height="${size}" alt="" title="${c}" />`;

// Status icon by timeline severity (the native Status set).
const STATUS_SLUG: Record<TimelineEntry["severity"], string> = {
  good: "status-success",
  bad: "status-warning-threat",
  warn: "status-warning-threat",
  info: "status-info",
};
const statusIcon = (sev: TimelineEntry["severity"], size = 13) => uiIcon(STATUS_SLUG[sev], size);

// --- Workspace rail: one right-docked column hosting System/Market/Logistics/
// Doctrine as a tab stack. Opening any tab opens the rail; one tab shows at a
// time; ✕ / Esc closes it → the map stays uncluttered. ----------------------
// The right rail hosts only the SELECTION/holdings-context tabs. The Market is a
// hub-wide institution → it lives in the TOP NAVBAR as its own overlay, not here.
type RailTab = "system" | "logistics" | "doctrine";
let railTab: RailTab = "system";
let railBuilt = false;

function setRailTab(tab: RailTab): void {
  railTab = tab;
  const bodyId: Record<RailTab, string> = { system: "tab-system", logistics: "standing", doctrine: "doctrine" };
  for (const t of ["system", "logistics", "doctrine"] as RailTab[]) {
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
  // Top-navbar destinations (hub-wide, system-independent): Market + check-in Log.
  $("nav-market").addEventListener("click", toggleMarket);
  $("nav-log").addEventListener("click", toggleCheckin);
  $("market-close").addEventListener("click", closeMarket);
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
      return `<div class="sp-sec">Order</div><div class="sp-line"><span class="tone-up">✓ confirmed</span> — you can see the fleet complying.</div>`;
    }
    return "";
  }
  // Near-zero (fleet at the command center): don't flash noisy sub-second states.
  if (p.echo_at - p.delivered_at < LIFECYCLE_MIN_S) return "";
  const now = liveSimTime();
  const line = now < p.delivered_at
    ? `<span class="dim">◈</span> <b>IN TRANSIT</b> — arrives in ${fmtCountdown(p.delivered_at - now)}`
    : `<span style="color:var(--accent)">◔</span> <b>DELIVERED — awaiting echo</b> ~${fmtCountdown(p.echo_at - now)}`;
  return `<div class="sp-sec">Order</div><div class="sp-line">${line}</div><div class="sp-line dim" style="margin-top:2px">The fleet has your ${esc(p.kind)} order; the light showing it hasn't returned yet.</div>`;
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
    `<button class="act${mode === m ? " is-on" : ""}" data-act="transit" data-mode="${m}" title="${hint}">${label}</button>`;
  const state = mode === "stealth"
    ? `<span class="tone-up">running quiet</span> — ~2× trip time, a much smaller sensor signature`
    : `<span class="dim">full speed</span> — fastest, but flank speed lights you up (high signature)`;
  return `<div class="sp-sec">Transit</div><div class="sp-line">${btn("full", "Full", "Formation speed — loud")} ${btn("stealth", "Stealth", "Creep at half speed — quiet")}</div><div class="sp-line dim" style="margin-top:4px">${state}.</div>`;
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
  return `<div class="sp-sec">Composition</div><div class="sp-line dim">est. <b>${countClassLabel(g.count_class)}</b> ships — composition unknown (out of sensor range)</div>`;
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
    parts.push(`<button class="act" data-act="merge" data-from="${merge.id}" title="Merge the co-located fleet into this one (at an owned system)">${uiIcon("concept-fleet", 13)} Merge co-located fleet</button>`);
  }
  parts.push(`<div class="sp-line dim" style="margin-top:4px">Composing fleets works only at one of your owned systems.</div>`);
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
  if (state.commandSignals.some((s) => s.shipId === g.id)) return "Order in transit — your command is still crossing space to it.";
  if (state.raids[g.id]) return "Raiding — pursuing a rival contact (recall to break off).";
  if (state.orders[g.id]) return "En route — proceeding on your last move order.";
  if (g.route && g.route.length) return "Hauling — en route along its trade route.";
  if (Math.hypot(g.vel.x, g.vel.y) < 0.5) return "Holding station — idle.";
  return "Under way.";
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
      ? `<div class="sp-cargo">${commodityIcon(g.cargo.commodity, 16)} <b>${fmt(g.cargo.units)}</b> ${esc(g.cargo.commodity)}</div>`
      : `<span class="dim">empty hold</span>`;
    parts.push(`<div class="sp-sec">Cargo</div>${cargo}`);
    if (g.route && g.route.length) {
      const d = g.route[g.route.length - 1];
      parts.push(`<div class="sp-sec">Route</div><div class="sp-line">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → final waypoint near (${d.x.toFixed(0)}, ${d.y.toFixed(0)}).</div>`);
    }
  }

  // Fleet fuel reserve (corp-wide, shared across ALL your ships) + this ship's burn
  // rate. Framed honestly: it's the operating reserve every ship spends, not a tank
  // on this one ship. (See the per-ship deepening note in the README.)
  const reserve = state.wallet ? state.wallet.fuel_total : 0;
  const rate = FUEL_PER_MASS_DISTANCE * 1000 * shipMass(g);
  let sub = `<span class="dim">~${rate.toFixed(1)} fuel / 1,000 su at this ship's mass</span>`;
  const dest = state.orders[g.id];
  if (dest) {
    const cost = FUEL_PER_MASS_DISTANCE * Math.hypot(dest.x - g.pos.x, dest.y - g.pos.y) * shipMass(g);
    sub = `<span class="dim">~${fmt(cost)} fuel for its current order · ${rate.toFixed(1)}/1,000 su</span>`;
  }
  parts.push(
    `<div class="sp-sec">Fuel</div>` +
    `<div class="sp-fuel">${commodityIcon("fuel", 16)}<div><div>Fleet reserve: <span class="sp-fuel-v">${fmt(reserve)}</span></div>${sub}</div></div>` +
    `<div class="sp-line dim" style="margin-top:6px">Shared reserve across all your systems — what every ship draws on to move, not a tank on this one ship.</div>`,
  );

  if (g.kind === "colony") {
    parts.push(`<div class="sp-sec">Role — Settlement</div><div class="sp-line">Colonists + infrastructure. Send it to an <b>unclaimed system</b>: on arrival the system becomes yours and the ship is consumed (it becomes the colony). It broadcasts its voyage — slow, visible, raidable: <b>escort it</b>. If someone claims the target first, it holds there intact — redirect it.</div>`);
  }
  if (g.kind === "corvette") {
    parts.push(`<div class="sp-sec">Role — Escort · Garrison</div><div class="sp-line">A dedicated <b>defender</b>: any raid contact on one of your convoys within its protect radius must fight THROUGH this ship first. Park it beside a convoy (escort) or at an owned system (garrison — stacks with a Defense Platform). It cannot raid.</div>`);
  }
  if (g.kind === "scout") {
    const mult = state.galaxy?.scout_sensor_mult ?? 1.5;
    parts.push(`<div class="sp-sec">Sensors</div><div class="sp-line">Projects a <b>×${mult}</b> sensor bubble — mobile vision. Sweep it through rival space to reveal dark contacts and convoy cargo; near a rival system it captures an intel snapshot of their defenses.</div>` +
      `<div class="sp-line dim" style="margin-top:4px">No cargo, no weapons: if anything engages it, it dies. Cheap on purpose.</div>`);
  }
  parts.push(`<div class="sp-sec">Actions</div>`);
  // §battles-take-time: WITHDRAW when this fleet is in/near a visible battle.
  const inBattle = state.battles.some((b) => Math.hypot(b.pos.x - g.pos.x, b.pos.y - g.pos.y) <= 220);
  if (inBattle && (g.kind === "raider" || g.kind === "corvette")) {
    parts.push(`<button class="act" data-act="withdraw" title="Break off and flee home — light-delayed; your formation speed decides the escape (escorts cover you)">↩ Withdraw from battle</button>`);
  }
  if (g.kind === "raider") {
    parts.push(`<button class="act" data-act="recall" title="Recall to home (R) — travels at light speed">${uiIcon("action-recall", 14)} Recall raider</button>`);
  }
  parts.push(`<div class="sp-line dim" style="margin-top:6px">${uiIcon("action-move-travel", 12)} Click empty space on the map to <b>move</b> this fleet${g.kind === "raider" ? ` · ${uiIcon("action-attack-raid", 12)} click a rival contact to <b>raid</b>` : ""}.</div>`);
  parts.push(transitSection(g));
  parts.push(fleetManagementSection(g));
  return parts.join("");
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
      parts.push(`<div class="sp-sec">Route (broadcast)</div><div class="sp-line">${g.route.length} leg${g.route.length > 1 ? "s" : ""} → heading near (${d.x.toFixed(0)}, ${d.y.toFixed(0)}). <span class="dim">Light-delayed.</span></div>`);
    }
    // Cargo ONLY when in sensor range (cargo present). NEVER shown otherwise.
    parts.push(`<div class="sp-sec">Cargo</div>` + (g.cargo
      ? `<div class="sp-cargo">${commodityIcon(g.cargo.commodity, 16)} <b>${fmt(g.cargo.units)}</b> ${esc(g.cargo.commodity)} <span class="dim">— in sensor range</span></div>`
      : `<span class="dim">unknown — out of sensor range</span>`));
  } else {
    const what = g.kind === "scout" ? "scout" : "raider";
    const hint = g.kind === "scout"
      ? "A scout runs silent — someone is LOOKING at your space. It carries no cargo and no weapons."
      : "A raider runs silent — no route or cargo is observable.";
    parts.push(`<div class="sp-sec">Dark contact</div><div class="sp-line dim">${hint} You see this ${what} only because it is within your sensor range right now.</div>`);
    // §Part 4: how LOUD it is (signature) — a big pack at flank speed flares far out.
    if (g.signature != null) {
      const loud = g.signature >= 1.6 ? "running LOUD — flank speed and/or a big pack (flares far out)"
        : g.signature <= 0.6 ? "running quiet — creeping or small (you caught it close)"
        : "a moderate signature";
      parts.push(`<div class="sp-line dim">Signature: <b>${g.signature.toFixed(2)}×</b> — ${loud}.</div>`);
    }
  }
  parts.push(`<div class="sp-sec">Action</div><div class="sp-line dim">${uiIcon("action-attack-raid", 12)} Click this contact on the map to commit a <b>raid</b> with your selected raider.</div>`);
  return parts.join("");
}

function updateShipPanel(): void {
  if (!state.selectedShipId) return;
  const root = $("ship-panel");
  const g = state.ghosts.find((x) => x.id === state.selectedShipId);
  if (!g) {
    // No longer observable (passed beyond your sensors/light, or — a rival —
    // destroyed). Honest: we can't show what we can't see.
    root.innerHTML =
      `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">contact</div><h2>Contact lost</h2></div></div>` +
      `<button class="sp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>` +
      `<div class="sp-body"><div class="sp-note">This ship has passed beyond your sensors and the last light to reach you. Nothing more is observable.</div></div>`;
    return;
  }
  const own = g.own;
  const eyebrow = own ? "your fleet" : g.kind === "raider" ? "dark contact" : "rival contact";
  const ownTag = own ? badge("accent", "yours") : badge("negative", "rival");
  const stale = g.age >= 8;

  const head =
    `<div class="sp-head"><div class="panel-title"><div><div class="eyebrow">${esc(eyebrow)}</div>` +
    `<h2>${uiIcon(g.kind === "convoy" ? "concept-convoy" : "concept-fleet", 15)} ${esc(shipKindLabel(g.kind))}</h2></div><div class="panel-title__right">${ownTag}</div></div>` +
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
  const posCell = certain
    ? stat("Position", `<span class="tone-up">confirmed</span>`)
    : stat("Position", `±${fmt(g.uncertainty)} su`);
  const strip = statStrip([ageCell, headingCell(g), posCell]);

  // Terse note — the stat strip already carries age / heading / ±uncertainty, so
  // this only adds a glance of context (no numbers restated, no physics lecture).
  const note = certain
    ? "" // the "confirmed" Position stat already says it
    : own
      ? `<div class="sp-note">Delayed sighting — true position uncertain (see cone).</div>`
      : `<div class="sp-note">Last sighting — could be anywhere in the cone.</div>`;

  root.innerHTML = head + `<div class="sp-body">${strip}${note}${own ? ownBody(g) : rivalBody(g)}</div>`;
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
    `<button class="act act--primary" data-act="market">${uiIcon("concept-market-exchange", 14)} Open Market</button>` +
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

function showBreadcrumb(name: string): void {
  $("bc-system").textContent = name;
  $("breadcrumb").classList.add("is-open");
}
function enterSystem(sys: SystemInfo): void {
  renderer.enterSystemView(sys);
  state.selectedSystemId = sys.id; // keep the galaxy selection in sync (rail shows it)
  showBreadcrumb(sys.name);
  closePlanetPanel();
  readout().innerHTML =
    `<b>${esc(sys.name)}</b> — schematic system view. <span class="dim">Click a planet for details · Esc / Back / zoom out returns to the galaxy. ` +
    `This is a VIEW: claims, production &amp; defense stay at the system level.</span>`;
}
function exitSystem(): void {
  if (renderer.viewMode.type !== "system") return;
  renderer.exitSystemView();
  $("breadcrumb").classList.remove("is-open");
  closePlanetPanel();
}

let planetPanelBuilt = false;
function buildPlanetPanel(): void {
  if (planetPanelBuilt) return;
  planetPanelBuilt = true;
  $("planet-panel").addEventListener("click", (e) => {
    if ((e.target as HTMLElement).closest("[data-act='close']")) closePlanetPanel();
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
  $("planet-panel").innerHTML = head + `<div class="pp-body">${kindLine}<div class="pp-desc" style="margin-top:8px">${esc(d.description)}</div>${deps}${note}</div>`;
  $("planet-panel").classList.add("is-open");
}

// Click INSIDE the System View: a planet/moon opens its details; empty space
// clears the selection/panel. No move orders, no raids — those are galaxy-only.
function handleSystemClick(sx: number, sy: number): void {
  const d = renderer.systemPick(sx, sy);
  if (d) openPlanetPanel(d);
  else closePlanetPanel();
}

// The map CLICK action (select own ship · select a star system incl. home ·
// inspect a command anchor · raid a rival ghost · move order to empty space). All
// hit-testing goes through screenToWorld, so it's correct at any zoom/pan. Run
// ONLY on a tap (see installInteraction's click-vs-drag gate) — never on a pan.
function handleMapClick(sx: number, sy: number): void {
    // Selection priority: a star SYSTEM and an own SHIP are hit-tested together,
    // because your starting fleet sits right on your home system — letting a parked
    // ship always swallow the click made the home system unselectable. Nearest wins,
    // with a small bias toward the SYSTEM so a body with ships on it (the home case)
    // still opens its System view; ships out in open space are still picked normally.
    const SYSTEM_BIAS = 5; // px the system may be "farther" and still win the tie

    let shipPick: string | null = null;
    let bestShip = Infinity; // nearest own-ship hit distance (px)
    for (const g of state.ghosts) {
      if (!g.own) continue;
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      // Hit radius tracks the MARKER's current on-screen size (formation sprite
      // included), so it grows with the sprite in the deep-zoom native-size band;
      // floored at 24px so normal-zoom clicking feels exactly as before.
      const rad = Math.max(24, renderer.fleetHitRadius(g));
      if (d < rad && d < bestShip) {
        bestShip = d;
        shipPick = g.id;
      }
    }

    let sysPick: string | null = null;
    let bestSys = Infinity;
    if (state.galaxy) {
      for (const sys of state.galaxy.systems) {
        const s = renderer.worldToScreen(sys.pos);
        const d = Math.hypot(s.x - sx, s.y - sy);
        // Hit radius follows the star's rendered disk in the deep-zoom band —
        // capped (~90px) so a max-zoom giant never blankets the map — with the
        // old 15px floor so normal-zoom clicking is unchanged.
        const rad = Math.max(15, renderer.systemHitRadius(sys));
        if (d < rad && d < bestSys) {
          bestSys = d;
          sysPick = sys.id;
        }
      }
    }

    // Prefer the system when it's hit and either no ship was hit, or the system is
    // within SYSTEM_BIAS of being as close (i.e. they're essentially co-located).
    if (sysPick && (!shipPick || bestSys <= bestShip + SYSTEM_BIAS)) {
      state.selectedSystemId = sysPick;
      openRail("system"); // → setRailTab("system") renders the detail
      return;
    }

    if (shipPick) {
      const g = state.ghosts.find((x) => x.id === shipPick)!;
      selectShip(shipPick); // opens the fog-aware ship panel; clears any system selection
      readout().innerHTML =
        `<b>${esc(g.own ? shipKindLabel(g.kind) : "rival " + g.kind)}</b> selected — details in the panel. ` +
        `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`;
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
    // Raiding is the raider's verb (mirrors the sim's CommitRaid gate).
    const haveRaider = haveOwn && sel!.kind === "raider";

    if (enemy) {
      const tgt = state.ghosts.find((x) => x.id === enemy)!;
      if (haveRaider && net) {
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
          `<span class="dim">Press R to recall — it may arrive too late.</span>`;
      } else {
        // Nothing of yours selected to attack with → INSPECT the rival (panel).
        selectShip(enemy);
        readout().innerHTML = `Rival <b>${esc(tgt.kind)}</b> selected — its light-delayed details are in the panel.`;
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

    // Empty space → move order for the selected OWN ship (a rival can't be moved).
    if (haveOwn && net) {
      const dest = renderer.screenToWorld(sx, sy);
      net.send({ type: "MoveShip", ship_id: sel!.id, dest });
      state.orders[sel!.id] = dest;
      updateShipPanel();
      const out = sel!.age; // ≈ light delay command-center → ship
      readout().innerHTML =
        `Order away to <b>${esc(shipKindLabel(sel!.kind))}</b>. ` +
        `Reaches it in <b>~${out.toFixed(0)}s</b> (your light), ` +
        `you'll see it respond <b>~${(out * 2).toFixed(0)}s</b> from now. ` +
        `<span class="dim">Estimated from a ${out.toFixed(0)}s-old sighting.</span>`;
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
    if (!panning) {
      if (renderer.viewMode.type === "system") handleSystemClick(e.clientX, e.clientY);
      else handleMapClick(e.clientX, e.clientY);
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
    } else if (e.key === "l" || e.key === "L") {
      toggleCheckin();
    } else if (e.key === "Escape") {
      // In the System View, Escape steps out one level: planet panel → system → galaxy.
      if ($("planet-panel").classList.contains("is-open")) {
        closePlanetPanel();
      } else if (renderer.viewMode.type === "system") {
        exitSystem();
      } else {
        closeMarket();
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

// Eyebrow flavor, derived purely client-side from public geology + position.
function systemFlavor(sys: SystemInfo): string {
  if (!sys.deposits.length) return "barren system";
  const dom = sys.deposits.reduce((a, b) =>
    a.richness * COMMODITY_VALUE[a.resource] >= b.richness * COMMODITY_VALUE[b.resource] ? a : b);
  const frac = state.galaxy ? Math.hypot(sys.pos.x, sys.pos.y) / state.galaxy.radius : 0;
  const tier = frac > 0.6 ? "frontier" : frac > 0.33 ? "mid-rim" : "core";
  return `${dom.resource}-rich ${tier}`;
}

function depositRow(d: Deposit): string {
  const pct = Math.min(100, d.richness * 40);
  const reserves = d.reserves === null
    ? `<span class="tone-up">renewable</span>`
    : d.reserves < 50 ? `<span class="is-warn">${fmt(d.reserves)} left</span>`
      : `${fmt(d.reserves)} left`;
  return `<div class="dep-row"><span class="dep-ico">${commodityIcon(d.resource, 18)}</span>` +
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

function productionReadout(sys: SystemInfo, dyn: SystemStateView | undefined): string {
  const stockOf = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const tier = dyn?.extractor_tier ?? 0;
  let mult = Math.pow(EXTRACTOR_RICHNESS_MULT, tier);
  // A FED Habitat multiplies the whole system's output on top of the Extractor
  // (§buildings step 3a); unfed = suspended (shown, not applied). Owner-only.
  const habTier = dyn?.habitat_tier ?? 0;
  const habMult = state.galaxy?.habitat_output_mult ?? 1.25;
  const habFed = !!dyn?.habitat_fed;
  if (habTier > 0 && habFed) mult *= Math.pow(habMult, habTier);
  const rateOf = new Map<Commodity, number>();
  for (const d of sys.deposits) rateOf.set(d.resource, (rateOf.get(d.resource) ?? 0) + d.richness * mult);
  const all = new Set<Commodity>([...stockOf.keys(), ...rateOf.keys()] as Commodity[]);
  const rows = [...all].filter((c) => (stockOf.get(c) ?? 0) >= 1 || (rateOf.get(c) ?? 0) > 0.01);
  if (!rows.length) return "";
  const tierTag = tier > 0 ? ` <span class="sp-tier" title="Extractor upgrades boost output ×${EXTRACTOR_RICHNESS_MULT} per tier">· Extractor ×${tier}</span>` : "";
  const habTag = habTier > 0
    ? (habFed
      ? ` <span class="sp-tier" title="a fed Habitat boosts ALL output ×${habMult} per tier">· Habitat ×${habTier}</span>`
      : ` <span class="sp-tier" style="color:var(--warn)" title="unfed — boost suspended until Provisions arrive (nothing is lost)">· Habitat UNFED</span>`)
    : "";
  // Standing upkeep line (the game's first continuous consumption).
  const upkeep = habTier > 0
    ? `<div class="mhint" style="margin-top:4px">Habitat upkeep: −${((state.galaxy?.habitat_upkeep_per_tier ?? 0.15) * habTier).toFixed(2)} provisions/s from this stockpile${habFed ? "" : ` — <span style="color:var(--warn)">UNFED: output boost suspended (nothing lost; resupply to restore)</span>`}.</div>`
    : "";
  // Refinery line (§buildings step 3b): converting Volatiles → Fuel, or idle dry.
  const refTier = dyn?.refinery_tier ?? 0;
  let refinery = "";
  if (refTier > 0) {
    const rate = (state.galaxy?.refinery_rate_per_tier ?? 0.5) * refTier;
    const yieldK = state.galaxy?.refinery_yield ?? 0.8;
    const vol = stockOf.get("volatiles") ?? 0;
    refinery = vol > 0
      ? `<div class="mhint" style="margin-top:4px">Refinery: converting ${rate.toFixed(1)} volatiles/s → +${(rate * yieldK).toFixed(1)} fuel/s.</div>`
      : `<div class="mhint" style="margin-top:4px">Refinery: <span style="color:var(--warn)">idle — no Volatiles</span>. Haul some in (${yieldK} fuel per volatile).</div>`;
  }
  return `<div class="deps-head" style="margin-top:8px">Stockpile · production${tierTag}${habTag}</div>` +
    rows.map((c) => {
      const rt = rateOf.get(c) ?? 0;
      const rate = rt > 0.01 ? `<span class="sp-rate">+${rt.toFixed(2)}/s</span>` : `<span class="sp-none">—</span>`;
      return `<div class="sys-prod"><span class="dep-ico">${commodityIcon(c, 16)}</span>` +
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

function buildPanel(dyn: SystemStateView | undefined): string {
  const opts = state.galaxy?.build_options ?? [];
  if (!opts.length) return "";
  // Development slots (§buildings step 1) — the scarcity that forces the
  // Extractor-vs-Depot-vs-Shipyard choice. Owner-only fields (rivals see 0/0);
  // this panel renders only for owned systems, so the readout is always real.
  const slotsUsed = dyn?.slots_used ?? 0;
  const slotsTotal = dyn?.slots_total ?? 0;
  const slotsFull = slotsTotal > 0 && slotsUsed >= slotsTotal;
  const slotsTag = slotsTotal > 0
    ? ` <span class="sp-tier" title="each development (Extractor/Depot/Shipyard tier) uses one slot — ships don't">· slots ${slotsUsed}/${slotsTotal}</span>`
    : "";
  const head = `<div class="deps-head" style="margin-top:8px">${uiIcon("action-build", 12)} Build · develop${slotsTag}</div>`;
  const building = dyn?.build ?? null;
  if (building) {
    const eta = Math.max(0, building.complete_time - state.simTime);
    const label = building.key.charAt(0).toUpperCase() + building.key.slice(1);
    return head + `<div class="mhint">${uiIcon("action-build", 13)} Building <b>${label}</b> — ETA <b>${eta.toFixed(0)}s</b>. <span class="dim">One job at a time.</span></div>`;
  }
  const have = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const yard = dyn?.shipyard_tier ?? 0;
  const rows = opts.map((o) => {
    const isDev = !SHIP_KEYS.has(o.key);
    const afford = o.costs.every((c) => (have.get(c.commodity as Commodity) ?? 0) >= c.units);
    // Two gates, mirroring the sim's soft-rejects: a DEVELOPMENT needs a free
    // slot; a SHIP needs this system's Shipyard at the required tier.
    const needYard = SHIP_KEYS.has(o.key) ? SHIP_REQ[o.key] ?? 1 : 0;
    const yardShort = needYard > 0 && yard < needYard;
    const blocked = (isDev && slotsFull) || yardShort;
    const enabled = afford && !blocked;
    const title = isDev && slotsFull ? "no free development slot — systems must specialize"
      : yardShort ? `ships build only at a Shipyard system (this needs tier ${needYard})`
        : afford ? "costs draw from this system's stockpile"
          : "not enough resources stockpiled here";
    const cost = o.costs.map((c) => `${commodityIcon(c.commodity as Commodity, 13)}${c.units}`).join(" ");
    const gate = isDev && slotsFull ? `<span class="bo-gate">slots full</span>`
      : yardShort ? `<span class="bo-gate">requires Shipyard ${needYard}</span>` : "";
    return `<button class="act build-opt" data-build="${o.key}" ${enabled ? "" : "disabled"} title="${title}">` +
      `<span class="bo-name">${esc(o.label)}${gate}</span><span class="bo-cost">${cost} · ${o.build_secs}s</span></button>`;
  }).join("");
  const full = slotsFull
    ? `<div class="mhint">${badge("warn", "slots full")} every development slot here is used — develop another system (specialize!).</div>`
    : "";
  return head + `<div class="build-grid">${rows}</div>` + full;
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
      // §step1 build sink: convoy/raider → BuildShip; developments → DevelopSystem.
      const k = el.dataset.build;
      if (k === "convoy" || k === "raider" || k === "corvette" || k === "colony" || k === "scout") net.send({ type: "BuildShip", system_id: sid, ship_kind: k });
      else if (k === "extractor" || k === "depot" || k === "shipyard" || k === "sensor_array" || k === "defense_platform" || k === "habitat" || k === "refinery") net.send({ type: "DevelopSystem", system_id: sid, upgrade: k });
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
  const root = $("tab-system");
  const rail = ownedSystemsRail();
  const sid = state.selectedSystemId;
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  if (!sys) {
    root.innerHTML = rail +
      `<div class="mhint">Click a star system on the map to inspect its geology, claim it, or ship its output.` +
      (rail ? " Or pick one of your holdings above." : "") + `</div>`;
    return;
  }
  const dyn = state.systems.find((s) => s.id === sid);
  const owner = dyn?.owner ?? null;
  const mine = owner !== null && owner === state.playerId;
  const rival = owner !== null && !mine;
  const unclaimed = owner === null;
  const stockTotal = (dyn?.stockpile ?? []).reduce((n, k) => n + k.units, 0);
  const yieldRate = sys.deposits.reduce((n, d) => n + d.richness, 0);

  // A system co-located with a home anchor is a starting HOME site; the one at
  // your command center is YOUR home (granted, not claimable). Detected by
  // position (the client already knows anchor + command-center positions).
  const coincides = (p: { x: number; y: number }) => Math.abs(p.x - sys.pos.x) < 1 && Math.abs(p.y - sys.pos.y) < 1;
  const atHomeSite = state.anchors.some((a) => coincides(a.pos));
  const isMyHome = mine && !!state.commandCenter && coincides(state.commandCenter);

  const ownTag = isMyHome ? badge("accent", "home base")
    : mine ? badge("accent", "yours")
      : rival ? badge("negative", "rival") : badge("neutral", "unclaimed");
  const header = `<div class="panel-title"><div><div class="eyebrow">${esc(isMyHome ? "your command seat" : systemFlavor(sys))}</div>` +
    `<h2>${esc(sys.name)}</h2></div><div class="panel-title__right">${ownTag}</div></div>`;

  // The system's STAR — concept art + type name. Flavor only; observable for ANY
  // system (a star is visible from afar) and leaks no economy/holdings (those stay
  // light-gated). Assigned deterministically by system id (stars.ts), so it's
  // stable and matches the map icon.
  const st = starTypeFor(sys.id);
  const starFeature = `<div class="sysview__star">` +
    `<img class="star-art" src="${starConceptUrl(st.slug)}" alt="" />` +
    `<div class="star-cap"><span class="star-type">${esc(st.title)}</span>` +
    `${st.exotic ? badge("accent", "exotic") : badge("neutral", "star")}</div></div>`;

  // Storage (§buildings step 2): the owner sees fill vs cap — the "ship it or
  // production idles" pressure made visible. Owner-only fields; rivals see —.
  const cap = dyn?.storage_cap ?? 0;
  const used = dyn?.storage_used ?? 0;
  const storageFull = mine && cap > 0 && used >= cap;
  const strip = statStrip([
    stat("Deposits", String(sys.deposits.length)),
    stat("Yield/s", yieldRate.toFixed(1)),
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
  // The system's DEVELOPMENTS at a glance (owner-only): what it has specialized
  // into, inside its slot budget.
  // Habitat FED/UNFED tag rides next to its tier (owner-only; §buildings 3a).
  const habTier = dyn?.habitat_tier ?? 0;
  const habTag = habTier > 0
    ? (dyn?.habitat_fed ? ` ${badge("positive", "fed")}` : ` ${badge("warn", "unfed")}`)
    : "";
  const devs = mine
    ? `<div class="devs-row">` +
      ([["Extractor", dyn?.extractor_tier ?? 0, ""], ["Depot", dyn?.depot_tier ?? 0, ""], ["Shipyard", dyn?.shipyard_tier ?? 0, ""], ["Sensor", dyn?.sensor_tier ?? 0, ""], ["Defense", dyn?.defense_tier ?? 0, ""], ["Habitat", habTier, habTag], ["Refinery", dyn?.refinery_tier ?? 0, ""]] as [string, number, string][])
        .map(([n, t, tag]) => `<span class="dev ${t ? "" : "dev--none"}">${n} ×${t}${tag}</span>`)
        .join(`<span class="dev-sep">·</span>`) +
      `</div>`
    : "";

  const deps = `<div class="sysview__deps"><div class="deps-head">Geology — richer toward the frontier</div>` +
    sys.deposits.map(depositRow).join("") + `</div>`;
  const prod = mine ? productionReadout(sys, dyn) : "";
  const build = mine ? buildPanel(dyn) : "";

  let actions: string;
  if (unclaimed && atHomeSite) {
    actions = `<div class="mhint" style="margin-top:8px">${badge("neutral", "reserved")} A starting home site — a future corporation will begin here owning it, so it can't be claimed.</div>`;
  } else if (unclaimed) {
    // Claiming is PHYSICAL now (§ships part 3): build a Colony Ship at a
    // shipyard and SEND it here — it claims on arrival (and is raidable en
    // route). The old instant credit purchase is gone.
    actions = `<div class="mhint" style="margin-top:8px">${uiIcon("action-claim-system", 14)} <b>To claim:</b> build a <b>Colony Ship</b> at a shipyard system and send it here — the system becomes yours when it ARRIVES (slow, visible, raidable: escort it). First arrival wins.</div>`;
  } else if (mine) {
    // Gate "Ship → hub" on what the sim will ACTUALLY dispatch (non-fuel whole
    // units), not the raw stock total — the home's Fuel reserve used to keep
    // this button lit while a click shipped nothing.
    const canShip = shippableStock(dyn).length > 0;
    const shipTitle = canShip
      ? "one raidable convoy per commodity, selling on arrival (Fuel stays as this system's operating reserve)"
      : "nothing shippable — Fuel is retained as the operating reserve (sell it via the Market); other goods ship in whole units";
    actions = `<button class="act" data-action="ship" ${canShip ? "" : "disabled"} title="${shipTitle}">${uiIcon("action-load-cargo", 14)} Ship production → hub</button>` +
      `<button class="act" data-action="standing">${uiIcon("action-standing-order", 14)} Auto-supply from here</button>` +
      `<button class="act" data-action="market">${uiIcon("concept-market-exchange", 14)} Open hub market</button>`;
  } else {
    actions = `<div class="mhint" style="margin-top:8px">${badge("negative", "held by rival")} ownership is light-delayed — what you see may already be stale.</div>`;
  }
  // OUR scout intel about a system we don't own (§scout part 2): a timestamped
  // SNAPSHOT of its fortifications — never live, aging until re-scouted. Shown
  // only to us (the View carries only our own snapshots, light-delayed).
  let intelBlock = "";
  if (!mine && dyn?.intel) {
    const age = Math.max(0, state.simTime - dyn.intel.observed_at);
    const ageTxt = age < 90 ? `${age.toFixed(0)}s ago` : `${(age / 60).toFixed(0)}m ago`;
    intelBlock = `<div class="deps-head" style="margin-top:8px">${uiIcon("concept-sensor-range", 12)} Scout intel — snapshot</div>` +
      `<div class="sp-line">Defense ×${dyn.intel.defense_tier} · Shipyard ×${dyn.intel.shipyard_tier} <span class="dim">· scouted ${ageTxt}</span></div>` +
      `<div class="mhint">A snapshot, not a feed — they may have built since. Re-scout to refresh.</div>`;
  }
  // Inspect → the presentation-only schematic System View. Offered for ANY system
  // (its geography is public); it is a VIEW, never a gameplay action. Also reachable
  // by double-click or deep-zoom on the map.
  actions += `<button class="act" data-action="inspect">◎ Inspect system ▸</button>`;

  const hint = isMyHome
    ? `<div class="mhint">Your command center sits here — your vantage on the galaxy, and a developed base producing from turn one. Ship its output to the hub or automate supply (Logistics).</div>`
    : mine
      ? `<div class="mhint">Production ships across fogged space to the hub — raidable in transit. Automate it from the Logistics tab.</div>`
      : unclaimed && !atHomeSite
        ? `<div class="mhint">Settlement starts production at once; rivals learn you hold it only when the claim's light reaches them.</div>`
        : "";

  root.innerHTML = rail + header + starFeature + strip + storageBar + devs + deps + intelBlock + prod + build + actions + hint;
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
const COMMODITIES: Commodity[] = ["fuel", "ore", "alloys", "provisions", "volatiles"];

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
      `<span class="dep-ico">${commodityIcon(c, 18)}</span>` +
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
    $("mk-preview").innerHTML = `<b>Limit ${composer.side} ${qty} ${c}</b> rests on the book and clears in the periodic <span class="accent">uniform-price batch</span> — reacting fastest confers no edge. Partial fills carry to the next batch.`;
    submit.textContent = `Place limit ${composer.side}`;
  } else if (composer.side === "buy") {
    const cost = price !== undefined ? fmt(qty * price) : "?";
    $("mk-preview").innerHTML = `Settles <b>now</b> at ${px}/u (~<span class="accent">${cost} Cr</span>). The goods then cross fogged space to your home anchor — that delivery convoy is <b>raidable</b> in transit.`;
    submit.textContent = `Buy ${qty} ${c}`;
  } else {
    $("mk-preview").innerHTML = `Convoy <b>dispatched now</b>; it clears at the price <b>on arrival</b> (not today's ${px}) and is <b>raidable</b> until it reaches the hub — double uncertainty: price + delivery.`;
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
  // Rebuild source/dest selects only when the owned-systems set changes (so a
  // mid-edit selection isn't clobbered every tick).
  const owned = ownedSystems();
  const ownedKey = owned.map((s) => s.id).join(",");
  const srcSel = $("so-source") as HTMLSelectElement;
  if (srcSel.dataset.key !== ownedKey) {
    srcSel.dataset.key = ownedKey;
    const destSel = $("so-dest") as HTMLSelectElement;
    const prevSrc = srcSel.value, prevDest = destSel.value;
    srcSel.innerHTML = owned.length
      ? owned.map((s) => `<option value="${s.id}">${s.name}</option>`).join("")
      : `<option value="">(claim a system first)</option>`;
    if (owned.some((s) => s.id === prevSrc)) srcSel.value = prevSrc;
    destSel.innerHTML = `<option value="hub">hub (sell)</option><option value="home">home (store)</option>` +
      owned.map((s) => `<option value="${s.id}">${s.name} (depot)</option>`).join("");
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
        `<b>#${o.id}</b> ${commodityIcon(o.commodity, 14)} ${o.commodity}: ${endpointLabel(o.source)} → ${endpointLabel(o.dest)}${paused}<br>` +
        `<span class="meta">${triggerLabel(o.trigger)} · ${flight}</span></div>`;
    })
    .join("");
  list.querySelectorAll<HTMLElement>("[data-clear]").forEach((el) => {
    el.addEventListener("click", () => {
      if (net) net.send({ type: "ClearStandingOrder", order_id: Number(el.dataset.clear) });
    });
  });
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

type Attn = { severity: TimelineEntry["severity"]; text: string };
function computeAttention(): Attn[] {
  if (state.playerId === null) return [];
  const items: Attn[] = [];
  const owned = state.systems.filter((s) => s.owner === state.playerId);
  const ownedIds = new Set(owned.map((s) => s.id));
  const active = state.standingOrders.filter((o) => o.status === "active");
  const IDLE = 30;
  // 0. STORAGE FULL (§buildings step 2) — production is idling right now; the
  //    most urgent economy cue there is. Owner-only fields, so this never fires
  //    for systems the player doesn't hold.
  for (const s of owned) {
    if (s.storage_cap > 0 && s.storage_used >= s.storage_cap) {
      items.push({ severity: "warn", text: `${systemName(s.id)}: storage FULL (${s.storage_used}/${s.storage_cap}) — production idling. Ship goods out or build a Depot.` });
    }
  }
  // 0b. UNFED HABITAT (§buildings step 3a) — the boost is suspended right now;
  //     a Provisions haul (or standing order) restores it. Owner-only fields.
  for (const s of owned) {
    if (s.habitat_tier >= 1 && !s.habitat_fed) {
      items.push({ severity: "warn", text: `${systemName(s.id)}: Habitat UNFED — output boost suspended. Ship Provisions there (nothing is lost).` });
    }
  }
  // 0c. DRY REFINERY (§buildings step 3b) — built industry sitting idle.
  for (const s of owned) {
    const vol = (s.stockpile ?? []).find((k) => k.commodity === "volatiles")?.units ?? 0;
    if (s.refinery_tier >= 1 && vol === 0) {
      items.push({ severity: "info", text: `${systemName(s.id)}: Refinery idle — no Volatiles. Haul some in to produce Fuel locally.` });
    }
  }
  // 1. Idle stockpile not covered by a standing order sourced there → automate it.
  for (const s of owned) {
    const total = (s.stockpile ?? []).reduce((n, k) => n + k.units, 0);
    const covered = active.some((o) => o.source.kind === "system" && o.source.id === s.id);
    if (total >= IDLE && !covered) {
      items.push({ severity: "warn", text: `${systemName(s.id)}: ${total} units sitting idle — set a standing order (O) or ship it.` });
    }
  }
  // 2. A rule that points at a system you no longer hold → fix it.
  for (const o of active) {
    const refs: string[] = [];
    if (o.source.kind === "system" && !ownedIds.has(o.source.id)) refs.push(systemName(o.source.id));
    if (o.dest.kind === "system" && !ownedIds.has(o.dest.id)) refs.push(systemName(o.dest.id));
    if (refs.length) items.push({ severity: "warn", text: `Standing order #${o.id} targets ${refs.join(" & ")} — you no longer hold it; update it (O).` });
  }
  // 3. General nudge toward automation if you hold producers but run nothing.
  if (owned.length > 0 && active.length === 0 && items.length === 0) {
    items.push({ severity: "info", text: `You hold ${owned.length} system${owned.length > 1 ? "s" : ""} but run no standing orders — automate supply so it works while you're away (O).` });
  }
  return items;
}

let checkinBuilt = false;
function buildCheckinPanel(): void {
  if (checkinBuilt) return;
  checkinBuilt = true;
  $("checkin-toggle").addEventListener("click", closeCheckin);
}

function updateCheckinPanel(): void {
  if (!checkinBuilt) return;
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
  $("checkin-timeline").innerHTML =
    `<div class="ci-sub">Since you were away${away.length ? ` (${away.length})` : ""}</div>${awayHtml}${earlierHtml}`;

  const att = computeAttention();
  $("checkin-attention").innerHTML = att.length
    ? att.map((a) => `<div class="ci ${a.severity}">${statusIcon(a.severity)} ${a.text}</div>`).join("")
    : `<span class="dim">Nothing needs your attention.</span>`;
  $("checkin-att-head").textContent = `Needs attention${att.length ? ` (${att.length})` : ""}`;
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
          }
          // The selected-ship panel keeps the information AGE ticking (and handles a
          // contact passing out of view) while it's open.
          if ($("ship-panel").classList.contains("is-open")) updateShipPanel();
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
