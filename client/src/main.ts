// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { formatId, type Commodity, type Deposit, type FleetDoctrine, type Side, type StandingEndpoint, type StandingOrder, type StandingTrigger, type SystemInfo, type SystemStateView, type TimelineEntry, type TradeEvent } from "./protocol";

const state: ViewState = initialState();

// --- DOM handles -----------------------------------------------------------
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
const ICONS: Record<string, string> = {
  trending: "M3 17l6-6 4 4 8-8 M21 7v4h-4",
  ship: "M3 13l9-9 9 9 M5 13v6a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2v-6",
  send: "M4 12l16-7-7 16-2-7-7-2Z",
  gavel: "M14 4l6 6-4 4-6-6 4-4Z M8 10l-5 5 4 4 5-5 M14 18h7",
  spark: "M12 3v4 M12 17v4 M3 12h4 M17 12h4",
};
const icon = (name: string, size = 14) =>
  `<svg width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" ` +
  `stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${ICONS[name] ?? ""}"/></svg>`;

// Mirror of the sim's commodity value-rank (also in render.ts) — for flavor text
// and dominant-resource selection. Client-only; no server data.
const COMMODITY_VALUE: Record<Commodity, number> = {
  provisions: 6, ore: 8, fuel: 10, volatiles: 18, alloys: 26,
};

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

// Click an own ship to select it; click elsewhere to order the selected ship
// there. The order travels at light speed — the readout makes the three clocks
// (send / arrive / observe) explicit, estimated from the stale sighting.
function installInteraction(): void {
  renderer.canvas.addEventListener("pointerdown", (e: PointerEvent) => {
    const sx = e.clientX;
    const sy = e.clientY;

    // Selection priority: a star SYSTEM and an own SHIP are hit-tested together,
    // because your starting fleet sits right on your home system — letting a parked
    // ship always swallow the click made the home system unselectable. Nearest wins,
    // with a small bias toward the SYSTEM so a body with ships on it (the home case)
    // still opens its System view; ships out in open space are still picked normally.
    const SYSTEM_BIAS = 5; // px the system may be "farther" and still win the tie

    let shipPick: string | null = null;
    let bestShip = 16;
    for (const g of state.ghosts) {
      if (!g.own) continue;
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      if (d < bestShip) {
        bestShip = d;
        shipPick = g.id;
      }
    }

    let sysPick: string | null = null;
    let bestSys = 15;
    if (state.galaxy) {
      for (const sys of state.galaxy.systems) {
        const s = renderer.worldToScreen(sys.pos);
        const d = Math.hypot(s.x - sx, s.y - sy);
        if (d < bestSys) {
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
      state.selectedShipId = shipPick;
      const g = state.ghosts.find((x) => x.id === shipPick)!;
      readout().innerHTML =
        `<b>${g.kind}</b> selected — last seen <b>${g.age.toFixed(1)}s</b> ago.<br>` +
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

    if (!state.selectedShipId || !net) return;
    const sel = state.ghosts.find((x) => x.id === state.selectedShipId);
    if (!sel) return;

    // Did we click a RIVAL ghost? → commit a raid against it.
    let enemy: string | null = null;
    let bestE = 16;
    for (const g of state.ghosts) {
      if (g.own) continue;
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      if (d < bestE) {
        bestE = d;
        enemy = g.id;
      }
    }
    if (enemy) {
      const tgt = state.ghosts.find((x) => x.id === enemy)!;
      net.send({ type: "CommitRaid", raider_id: sel.id, target_id: tgt.id });
      state.raids[sel.id] = tgt.id; // drive the soft intercept-estimate overlay
      delete state.orders[sel.id];
      readout().innerHTML =
        `Raid committed: your <b>${sel.kind}</b> → rival <b>${tgt.kind}</b>. ` +
        `The order sets off at light speed; your raider will pursue the rival's <i>true</i> position, ` +
        `not the <b>${tgt.age.toFixed(0)}s</b>-old ghost you see. ` +
        `<span class="dim">Press R to recall — it may arrive too late.</span>`;
      return;
    }

    // Otherwise → move order to the clicked point.
    const dest = renderer.screenToWorld(sx, sy);
    net.send({ type: "MoveShip", ship_id: sel.id, dest });
    state.orders[sel.id] = dest;
    const out = sel.age; // ≈ light delay command-center → ship
    readout().innerHTML =
      `Order away to <b>${sel.kind}</b>. ` +
      `Reaches it in <b>~${out.toFixed(0)}s</b> (your light), ` +
      `you'll see it respond <b>~${(out * 2).toFixed(0)}s</b> from now. ` +
      `<span class="dim">Estimated from a ${out.toFixed(0)}s-old sighting.</span>`;
  });

  // Keyboard: R = recall selected raider; M = toggle the Hub Exchange panel.
  window.addEventListener("keydown", (e) => {
    if (e.target instanceof HTMLInputElement) return; // don't hijack the qty field
    if ((e.key === "r" || e.key === "R") && state.selectedShipId && net) {
      net.send({ type: "RecallRaid", raider_id: state.selectedShipId });
      delete state.raids[state.selectedShipId]; // break off the intercept estimate
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
      closeMarket();
      closeRail();
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
  return `<div class="dep-row"><span class="dep-ico">${icon("trending", 13)}</span>` +
    `<span class="dep-name">${d.resource}</span>${bar(pct)}` +
    `<span class="dep-r">~${d.richness.toFixed(2)}/s · ${reserves}</span></div>`;
}

// Owner-only production readout: per-resource stockpile + the deposit yield as its
// flow (the protocol carries no separate per-tick flow). Gated behind ownership.
function productionReadout(sys: SystemInfo, dyn: SystemStateView | undefined): string {
  const stockOf = new Map((dyn?.stockpile ?? []).map((s) => [s.commodity, s.units]));
  const rateOf = new Map<Commodity, number>();
  for (const d of sys.deposits) rateOf.set(d.resource, (rateOf.get(d.resource) ?? 0) + d.richness);
  const all = new Set<Commodity>([...stockOf.keys(), ...rateOf.keys()] as Commodity[]);
  const rows = [...all].filter((c) => (stockOf.get(c) ?? 0) >= 1 || (rateOf.get(c) ?? 0) > 0.01);
  if (!rows.length) return "";
  return `<div class="deps-head" style="margin-top:8px">Stockpile · production</div>` +
    rows.map((c) => {
      const rt = rateOf.get(c) ?? 0;
      const rate = rt > 0.01 ? `<span class="sp-rate">+${rt.toFixed(2)}/s</span>` : `<span class="sp-none">—</span>`;
      return `<div class="sys-prod"><span class="dep-ico">${icon("spark", 12)}</span>` +
        `<span>${c}</span><span class="sp-stock">${fmt(stockOf.get(c) ?? 0)}</span>${rate}</div>`;
    }).join("");
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
    const el = (e.target as HTMLElement).closest("[data-action],[data-sys]") as HTMLElement | null;
    if (!el) return;
    if (el.dataset.sys) {
      state.selectedSystemId = el.dataset.sys; // re-selects; map highlights it too
      updateSystemTab();
      return;
    }
    const sid = state.selectedSystemId;
    if (!sid || !net) return;
    switch (el.dataset.action) {
      case "claim": net.send({ type: "ClaimSystem", system_id: sid }); break;
      case "ship": net.send({ type: "ShipProduction", system_id: sid }); break;
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
  const afford = (state.wallet?.credits ?? 0) >= sys.claim_cost;
  const stockTotal = (dyn?.stockpile ?? []).reduce((n, k) => n + k.units, 0);
  const yieldRate = sys.deposits.reduce((n, d) => n + d.richness, 0);

  const ownTag = mine ? badge("accent", "yours") : rival ? badge("negative", "rival") : badge("neutral", "unclaimed");
  const header = `<div class="panel-title"><div><div class="eyebrow">${esc(systemFlavor(sys))}</div>` +
    `<h2>${esc(sys.name)}</h2></div><div class="panel-title__right">${ownTag}</div></div>`;

  const strip = statStrip([
    stat("Deposits", String(sys.deposits.length)),
    stat("Yield/s", yieldRate.toFixed(1)),
    stat("Stock", mine ? fmt(stockTotal) : "—"),
    stat("Claim", unclaimed ? fmt(sys.claim_cost) : "—", unclaimed && !afford ? "is-negative" : ""),
  ]);

  const deps = `<div class="sysview__deps"><div class="deps-head">Geology — richer toward the frontier</div>` +
    sys.deposits.map(depositRow).join("") + `</div>`;
  const prod = mine ? productionReadout(sys, dyn) : "";

  let actions: string;
  if (unclaimed) {
    actions = `<button class="act act--primary" data-action="claim" ${afford ? "" : "disabled"}>` +
      `${icon("gavel")} ${afford ? "Claim system" : "Can't afford claim"}</button>`;
  } else if (mine) {
    actions = `<button class="act" data-action="ship" ${stockTotal > 0 ? "" : "disabled"}>${icon("ship")} Ship production → hub</button>` +
      `<button class="act" data-action="standing">${icon("send")} Auto-supply from here</button>` +
      `<button class="act" data-action="market">${icon("trending")} Open hub market</button>`;
  } else {
    actions = `<div class="mhint" style="margin-top:8px">${badge("negative", "held by rival")} ownership is light-delayed — what you see may already be stale.</div>`;
  }

  const hint = mine
    ? `<div class="mhint">Production ships across fogged space to the hub — raidable in transit. Automate it from the Logistics tab.</div>`
    : unclaimed
      ? `<div class="mhint">Claiming starts production at once; rivals learn you hold it only when the claim's light reaches them.</div>`
      : "";

  root.innerHTML = rail + header + strip + deps + prod + actions + hint;
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
  const el = document.createElement("div");
  el.className = "report " + cls;
  el.innerHTML = `<span class="ic">${icon}</span> ${text} <span class="dim">— delayed news, ${r.age.toFixed(0)}s old</span>`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
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
      `<span class="dep-ico">${icon("trending", 12)}</span>` +
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
    $("mk-preview").innerHTML = `Settles <b>now</b> at ${px}/u (~<span class="accent">${cost} cr</span>). The goods then cross fogged space to your home anchor — that delivery convoy is <b>raidable</b> in transit.`;
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
    stat("Credits", `${fmt(state.wallet.credits)} cr`, "is-accent"),
    stat("Equity", `${fmt(state.wallet.valuation)} cr`),
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
        `<b>#${o.id}</b> ${o.commodity}: ${endpointLabel(o.source)} → ${endpointLabel(o.dest)}${paused}<br>` +
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
  const row = (e: TimelineEntry) => `<div class="ci ${e.severity}">${e.text} <span class="t">${agoLabel(e.at_time)}</span></div>`;
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
    ? att.map((a) => `<div class="ci ${a.severity}">⚑ ${a.text}</div>`).join("")
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
    },
    onMessage: (msg) => {
      switch (msg.type) {
        case "Welcome":
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
