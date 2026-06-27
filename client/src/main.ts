// Bootstrap: wire the join screen → WebSocket → view state → HUD + Pixi render.

import { Net } from "./net";
import { Renderer } from "./render";
import { initialState, type LinkStatus, type ViewState } from "./state";
import { formatId, type Commodity, type TradeEvent } from "./protocol";

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

// Advance the traveling-signal visualizations each frame. This is the ONLY
// client-side timing computation: interpolating progress between server-provided
// endpoints/times (and revealing a report's verdict when its ring arrives). No
// delay is computed from truth or a client-side c.
function updateSignals(): void {
  const estSimNow = state.simTime + (performance.now() - state.lastViewWallMs) / 1000;
  // Order round trip: comet OUT to the ship, then the response light coming
  // BACK; dropped once that return light reaches the command center (which is
  // when the ghost's new course becomes visible — so the gap is never dead).
  state.commandSignals = state.commandSignals.filter((s) => {
    const outSpan = s.arrive - s.depart;
    const backSpan = s.observe - s.arrive;
    if (estSimNow < s.arrive) {
      s.phase = "out";
      s.pOut = outSpan > 1e-3 ? (estSimNow - s.depart) / outSpan : 1;
      s.pBack = 0;
    } else if (estSimNow < s.observe) {
      s.phase = "back";
      s.pOut = 1;
      s.pBack = backSpan > 1e-3 ? (estSimNow - s.arrive) / backSpan : 1;
    } else {
      return false; // response light has arrived; the ghost now shows the change
    }
    s.remainingS = Math.max(0, s.observe - estSimNow);
    return true;
  });
  // Inbound rings: progress over the server-provided light delay; reveal the
  // verdict on arrival at the command center.
  const nowMs = performance.now();
  state.reportSignals = state.reportSignals.filter((s) => {
    s.progress = (nowMs - s.startWallMs) / (s.durationS * 1000);
    if (s.progress >= 1) {
      addReport(s.report);
      return false;
    }
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
        `<b>${g.kind}</b> selected — last seen <b>${g.age.toFixed(1)}s</b> ago.<br>` +
        `Click empty space to move it · click a <span style="color:#ff7a6b">rival</span> to raid · press <b>R</b> to recall.`;
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
      readout().innerHTML =
        `Recall away to your raider — travels at light speed. ` +
        `<span class="dim">If it has already made contact, you're commanding into the past.</span>`;
    } else if (e.key === "m" || e.key === "M") {
      const m = $("market");
      m.style.display = m.style.display === "none" ? "block" : "none";
    }
  });
}

// --- Delayed reports log -----------------------------------------------------
function addReport(r: import("./protocol").RaidReport): void {
  const log = $("reports-log");
  const intercepted = r.outcome === "intercepted";
  let icon: string, cls: string, text: string;
  if (r.you === "attacker") {
    if (intercepted) { icon = "✓"; cls = "good"; text = "Your raider intercepted a rival convoy."; }
    else { icon = "✗"; cls = "bad"; text = "Your target reached the hub — raid failed."; }
  } else {
    if (intercepted) { icon = "‼"; cls = "bad"; text = "Your convoy was lost to a raider."; }
    else { icon = "✓"; cls = "good"; text = "Your convoy reached safety despite a raider."; }
  }
  const el = document.createElement("div");
  el.className = "report " + cls;
  el.innerHTML = `<span class="ic">${icon}</span> ${text} <span class="dim">— delayed news, ${r.age.toFixed(0)}s old</span>`;
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
    const qty = Math.max(1, Math.floor(Number((($("market-qty") as HTMLInputElement).value) || 0)));
    net.send(btn.getAttribute("data-side") === "buy"
      ? { type: "MarketBuy", commodity: c, units: qty }
      : { type: "MarketSell", commodity: c, units: qty });
  });
}

function updateMarket(): void {
  if (!state.market || !state.wallet) return;
  $("market-credits").textContent = `${Math.round(state.wallet.credits).toLocaleString()} cr`;
  const stale = state.market.staleness;
  $("market-stale").textContent = stale > 0.5 ? `ticker ~${stale.toFixed(0)}s stale` : "ticker live";
  const priceOf = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  const heldOf = new Map(state.wallet.inventory.map((i) => [i.commodity, i.units]));
  for (const c of COMMODITIES) {
    const pe = document.getElementById(`mp-price-${c}`);
    const he = document.getElementById(`mp-held-${c}`);
    if (pe) pe.textContent = priceOf.has(c) ? priceOf.get(c)!.toFixed(2) : "—";
    if (he) he.textContent = String(heldOf.get(c) ?? 0);
  }
}

function addTradeNews(t: TradeEvent): void {
  const log = $("reports-log");
  let text = "";
  switch (t.event) {
    case "Bought": text = `Bought ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} — delivery convoy inbound (raidable).`; break;
    case "Delivered": text = `Delivery arrived: +${t.units} ${t.commodity} in stores.`; break;
    case "SellDispatched": text = `Sell convoy away: ${t.units} ${t.commodity} crossing to the hub.`; break;
    case "Sold": text = `Sold ${t.units} ${t.commodity} @ ${t.unit_price.toFixed(2)} on arrival.`; break;
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
          state.ghosts = msg.ghosts;
          state.market = msg.market;
          state.wallet = msg.wallet;
          // Light-respecting "corps in view": distinct owners we can actually
          // see (self + rivals whose light has arrived). Never a raw count.
          state.corpsInView = new Set(msg.ghosts.map((g) => g.owner)).size;
          state.lastViewWallMs = performance.now();
          state.link = "online";
          updateMarket();
          break;
        case "CommandSignal": {
          // Your order is crossing space to your ship, and you'll see its
          // response a round trip later. Replace any in-flight signal for the
          // same ship (a newer order supersedes).
          state.commandSignals = state.commandSignals.filter((s) => s.shipId !== msg.ship_id);
          state.commandSignals.push({
            shipId: msg.ship_id,
            depart: msg.depart_time,
            arrive: msg.arrive_time,
            observe: msg.observe_time,
            phase: "out",
            pOut: 0,
            pBack: 0,
            remainingS: 0,
          });
          break;
        }
        case "Report": {
          // The news has become observable. Visualize it crossing home from the
          // resolution point; the verdict is revealed when the ring arrives at
          // the command center (in updateSignals).
          const rep = msg.report;
          state.reportSignals.push({
            from: rep.pos,
            startWallMs: performance.now(),
            durationS: Math.max(rep.age, 0.6),
            report: rep,
            progress: 0,
          });
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
