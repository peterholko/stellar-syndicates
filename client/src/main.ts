// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer, fmtDelay } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { formatId, type Commodity, type Side, type TradeEvent } from "./protocol";

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
    state.link === "online" ? fmtDelay(state.simTime) : "—";
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

// Click an own ship to select it; click elsewhere to order the selected ship
// there. The order travels at light speed — the readout makes the three clocks
// (send / arrive / observe) explicit, estimated from the stale sighting.
function installInteraction(): void {
  renderer.canvas.addEventListener("pointerdown", (e: PointerEvent) => {
    const sx = e.clientX;
    const sy = e.clientY;

    // Pick the nearest OWN ghost within a tolerance.
    let picked: string | null = null;
    let bestD = 16;
    for (const g of state.ghosts) {
      if (!g.own) continue;
      const s = renderer.worldToScreen(g.pos);
      const d = Math.hypot(s.x - sx, s.y - sy);
      if (d < bestD) {
        bestD = d;
        picked = g.id;
      }
    }

    if (picked) {
      state.selectedShipId = picked;
      const g = state.ghosts.find((x) => x.id === picked)!;
      readout().innerHTML =
        `<b>${g.kind}</b> selected — last seen <b>${fmtDelay(g.age)}</b> ago.<br>` +
        `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`;
      return;
    }

    // Otherwise, did we click a star system? → open its claim / production panel.
    let sysPick: string | null = null;
    let bestS = 13;
    if (state.galaxy) {
      for (const sys of state.galaxy.systems) {
        const s = renderer.worldToScreen(sys.pos);
        const d = Math.hypot(s.x - sx, s.y - sy);
        if (d < bestS) {
          bestS = d;
          sysPick = sys.id;
        }
      }
    }
    if (sysPick) {
      state.selectedSystemId = sysPick;
      updateSystemPanel();
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
        `not the <b>${fmtDelay(tgt.age)}</b>-old ghost you see. ` +
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
      `Reaches it in <b>~${fmtDelay(out)}</b> (your light), ` +
      `you'll see it respond <b>~${fmtDelay(out * 2)}</b> from now. ` +
      `<span class="dim">Estimated from a ${fmtDelay(out)}-old sighting.</span>`;
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
    } else if (e.key === "m" || e.key === "M") {
      const m = $("market");
      m.style.display = m.style.display === "none" ? "block" : "none";
    }
  });
}

// --- System panel: claim a system / ship its production (§4, §9) -------------
// Shows the selected system's deposits (the frontier-richer geology) and, by
// ownership: a Claim button if unclaimed, or stockpile + a Ship-to-hub button if
// it's yours. A rival's system shows only that it's owned (their holdings never
// leak). Refreshed each View so stockpile/credits stay live.
function updateSystemPanel(): void {
  const panel = $("system-panel");
  const sid = state.selectedSystemId;
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  if (!sid || !sys) {
    panel.style.display = "none";
    return;
  }
  // The habitable planet is the MARKET world — not a claimable mining body.
  if (sys.body === "planet") {
    panel.innerHTML =
      `<div class="title">${sys.name} · <span style="color:#7fd4ff">MARKET</span> <span class="x" id="sys-close">✕</span></div>` +
      `<div class="srow">The habitable planet — home of the Exchange.</div>` +
      `<div class="srow">~${sys.semi_major_au.toFixed(1)} AU from the sun. Convoys sell here; prices reach you light-delayed.</div>` +
      `<div class="hint">mine the belts, haul your ore here to sell — raidable in transit</div>`;
    panel.style.display = "block";
    document.getElementById("sys-close")?.addEventListener("click", () => {
      state.selectedSystemId = null;
      updateSystemPanel();
    });
    return;
  }

  const dyn = state.systems.find((s) => s.id === sid);
  const owner = dyn?.owner ?? null;
  const mine = owner !== null && owner === state.playerId;
  const rival = owner !== null && !mine;
  const credits = state.wallet?.credits ?? 0;

  const deps = sys.deposits
    .map((d) => `<div class="dep">${d.resource} · <b>${d.richness.toFixed(2)}</b>/s${d.reserves === null ? " · renewable" : ` · ${Math.round(d.reserves)} left`}</div>`)
    .join("");

  let action: string;
  if (mine) {
    const slots = dyn?.stockpile ?? [];
    const stock = slots.length ? slots.map((s) => `${s.units} ${s.commodity}`).join(", ") : "—";
    action =
      `<div class="srow">Mined ore: <b>${stock}</b></div>` +
      `<button id="ship-btn" ${slots.length ? "" : "disabled"}>Haul ore → market</button>`;
  } else if (rival) {
    action = `<div class="srow warn">Operated by a rival corporation.</div>`;
  } else {
    const afford = credits >= sys.claim_cost;
    action =
      `<div class="srow">~${sys.semi_major_au.toFixed(1)} AU from the sun · operate for <b>${Math.round(sys.claim_cost).toLocaleString()}</b> cr</div>` +
      `<button id="claim-btn" ${afford ? "" : "disabled"}>${afford ? "Operate asteroid" : "Can't afford"}</button>`;
  }

  const tag = mine ? ' · <span style="color:#4fc3ff">YOURS</span>' : rival ? ' · <span style="color:#ff7a6b">rival</span>' : "";
  const hint = rival
    ? "ownership is light-delayed — what you see may already be stale"
    : mine
      ? "ore hauls across fogged space to the market — raidable in transit"
      : "richer, more valuable deposits lie out toward the dangerous Kuiper frontier";
  panel.innerHTML =
    `<div class="title">${sys.name}${tag} <span class="x" id="sys-close">✕</span></div>` +
    `<div class="deps">${deps}</div>${action}<div class="hint">${hint}</div>`;
  panel.style.display = "block";

  document.getElementById("claim-btn")?.addEventListener("click", () => {
    if (net) net.send({ type: "ClaimSystem", system_id: sid });
  });
  document.getElementById("ship-btn")?.addEventListener("click", () => {
    if (net) net.send({ type: "ShipProduction", system_id: sid });
  });
  document.getElementById("sys-close")?.addEventListener("click", () => {
    state.selectedSystemId = null;
    updateSystemPanel();
  });
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
  el.innerHTML = `<span class="ic">${icon}</span> ${text} <span class="dim">— delayed news, ${fmtDelay(r.age)} old</span>`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
}

// --- Hub Exchange (§9) -------------------------------------------------------
const COMMODITIES: Commodity[] = ["fuel", "ore", "alloys", "provisions", "volatiles"];

function buildMarketPanel(): void {
  const rows = $("market-rows");
  rows.innerHTML = "";
  for (const c of COMMODITIES) {
    const tr = document.createElement("tr");
    tr.innerHTML =
      `<td class="name">${c}</td>` +
      `<td id="mp-price-${c}">—</td>` +
      `<td id="mp-held-${c}">—</td>` +
      `<td><button class="buy" data-c="${c}" data-side="buy">Buy</button> ` +
      `<button class="sell" data-c="${c}" data-side="sell">Sell</button></td>`;
    rows.appendChild(tr);
  }
  rows.addEventListener("click", (e) => {
    const btn = (e.target as HTMLElement).closest("button");
    if (!btn || !net) return;
    const c = btn.getAttribute("data-c") as Commodity;
    const side = btn.getAttribute("data-side") as Side;
    const qty = Math.max(1, Math.floor(Number((($("market-qty") as HTMLInputElement).value) || 0)));
    const limitOn = ($("market-limit-on") as HTMLInputElement).checked;
    const limitPrice = Number(($("market-limit-price") as HTMLInputElement).value);
    if (limitOn && limitPrice > 0) {
      net.send({ type: "PlaceLimitOrder", side, commodity: c, units: qty, limit_price: limitPrice });
    } else {
      net.send(side === "buy"
        ? { type: "MarketBuy", commodity: c, units: qty }
        : { type: "MarketSell", commodity: c, units: qty });
    }
  });
}

function updateMarket(): void {
  if (!state.market || !state.wallet) return;
  $("market-credits").textContent =
    `${Math.round(state.wallet.credits).toLocaleString()} cr · equity ${Math.round(state.wallet.valuation).toLocaleString()}`;
  const stale = state.market.staleness;
  $("market-stale").textContent = stale > 0.5 ? `ticker ~${fmtDelay(stale)} stale` : "ticker live";
  const priceOf = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  const heldOf = new Map(state.wallet.inventory.map((i) => [i.commodity, i.units]));
  for (const c of COMMODITIES) {
    const pe = document.getElementById(`mp-price-${c}`);
    const he = document.getElementById(`mp-held-${c}`);
    if (pe) pe.textContent = priceOf.has(c) ? priceOf.get(c)!.toFixed(2) : "—";
    if (he) he.textContent = String(heldOf.get(c) ?? 0);
  }
  const ordersEl = $("market-orders");
  const orders = state.wallet.orders;
  ordersEl.innerHTML = orders.length
    ? "<b>resting:</b> " + orders.map((o) => `<span class="o ${o.side}">${o.side} ${o.units} ${o.commodity} @ ${o.limit_price.toFixed(1)}</span>`).join(" · ")
    : "";
}

function addTradeNews(t: TradeEvent): void {
  const log = $("reports-log");
  let text = "";
  switch (t.event) {
    case "Bought": text = `Bought ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} — delivery convoy inbound (raidable).`; break;
    case "Delivered": text = `Delivery arrived: +${t.units} ${t.commodity} in stores.`; break;
    case "SellDispatched": text = `Sell convoy away: ${t.units} ${t.commodity} crossing to the hub.`; break;
    case "Sold": text = `Sold ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} on arrival.`; break;
    case "LimitPlaced": text = `Limit ${t.side} ${t.units} ${t.commodity} @ ${t.limit_price.toFixed(2)} resting on the book.`; break;
    case "LimitFilled": text = `Limit ${t.side} filled in batch: ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)}.`; break;
  }
  const el = document.createElement("div");
  el.className = "report good";
  el.innerHTML = `<span class="ic" style="color:#7fd4ff">◈</span> ${text}`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
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
          buildMarketPanel();
          $("market").style.display = "block";
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
          updateSystemPanel();
          // Light-respecting "corps in view": distinct owners we can actually
          // see (self + rivals whose light has arrived). Never a raw count.
          state.corpsInView = new Set(msg.ghosts.map((g) => g.owner)).size;
          state.lastViewWallMs = performance.now();
          state.link = "online";
          updateMarket();
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
