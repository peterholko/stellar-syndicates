import { hubDockedFleets, shipKindLabel } from "../../core/derive/fleet";
import { agoLabel, fmt, fmtDur, rejectText, trend } from "../../core/derive/format";
import { systemName } from "../../core/derive/geo";
import { COMMODITIES, marketAverageQuote, marketReservations, moduleRecipeValue, pruneMarketReservations, recentMarketOrders, recordRecentMarketOrder, reservedMarketCredits, reserveMarketOrder, settleMarketReservation, spendableMarketCredits, warehouseUnits } from "../../core/derive/market";
import { projectedBand } from "../../core/derive/research";
import { icon, type IconKey, label } from "../../icons";
import { type Commodity, countClassLabel, fleetCargoManifest, fleetExactCount, type ModuleKind, type Side, type TradeEvent } from "../../protocol";
import { renderer } from "../../render";
import { state } from "../../state";
import { net } from "./index";
import { $, badge, commodityIcon, esc, readout, renderDeferred, setHtml, spark, stat, statStrip, svgIcon } from "./mapchrome";
import { fleetRosterRow } from "./rail";
import { selectShip, uxTabBar, type UxTabOption } from "./ship";
import { MODULE_ALL, MODULE_BUY_MULT, MODULE_LABEL, MODULE_SELL_MULT, MODULE_TIP, moduleIcon } from "./sysview";
import { activateWorkspacePage, deactivateWorkspacePage, workspacePageIsActive } from "./workspace";


// --- Hub Exchange overlay (top-navbar destination; independent of selection) ---
export function openMarket(): void {
  activateWorkspacePage("market");
  setMarketTab(marketTab); // §market-ux: reopen on the last tab (also updates)
}

export function closeMarket(): void {
  deactivateWorkspacePage("market");
}

export function toggleMarket(): void {
  if (workspacePageIsActive("market")) closeMarket();
  else openMarket();
}


// --- Wormhole Hub detail panel (§hub-art) --------------------------------------
// The hub is PUBLIC geography (nothing to fog-gate): selecting it shows its
// concept portrait, a role blurb, and the natural shortcut — Open Market
// (the hub IS the market). Its Fleets tab is different: it is built only from
// this player's SERVED dock reports, never from authoritative berth truth.
// Mirrors the planet-panel idiom (left dock).
export type HubPanelTab = "overview" | "fleets";

export let hubPanelTab: HubPanelTab = "overview";

export let lastHubPanelSig = "";

export let hubPanelBuilt = false;

export function buildHubPanel(): void {
  if (hubPanelBuilt) return;
  hubPanelBuilt = true;
  $("hub-panel").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-act],[data-hub-tab],[data-fleet],[data-center-fleet]") as HTMLElement | null;
    if (!el) return;
    const requestedTab = el.dataset.hubTab as HubPanelTab | undefined;
    if (requestedTab && (["overview", "fleets"] as HubPanelTab[]).includes(requestedTab)) {
      hubPanelTab = requestedTab;
      lastHubPanelSig = "";
      updateHubPanel();
    } else if (el.dataset.act === "close") {
      closeHubPanel();
    } else if (el.dataset.act === "market") {
      openMarket();
    } else if (el.dataset.centerFleet) {
      const fleet = state.ghosts.find((g) => g.id === el.dataset.centerFleet && g.own && g.docked === "hub");
      if (fleet) renderer.centerOnWorld(fleet.pos);
    } else if (el.dataset.fleet) {
      const fleet = el.dataset.fleet;
      if (!state.ghosts.some((g) => g.id === fleet && g.own && g.docked === "hub")) return;
      const ghost = state.ghosts.find((g) => g.id === fleet);
      if (ghost) renderer.centerOnWorld(ghost.pos);
      closeHubPanel();
      selectShip(fleet);
    }
  });
}


export function updateHubPanel(): void {
  const panel = $("hub-panel");
  if (!panel.classList.contains("is-open")) return;
  const fleets = hubDockedFleets();
  const sig = JSON.stringify([
    hubPanelTab,
    fleets.map((g) => [
      g.id, g.kind, g.count_class, g.composition, g.cargo, g.cargo_manifest,
      Math.floor(g.age),
    ]),
  ]);
  if (sig === lastHubPanelSig && panel.innerHTML) return;
  lastHubPanelSig = sig;

  const tabs: readonly UxTabOption<HubPanelTab>[] = [
    ["overview", "Overview", "market"],
    ["fleets", fleets.length ? `Fleets (${fleets.length})` : "Fleets", "fleet"],
  ];
  const overview =
    `<img class="hub-art" src="/art/wormhole_hub_concept.png" alt="" />` +
    `<div class="pp-body"><div class="pp-desc">The Authority's station at the wormhole to Sol — the body that issued your charter. Its Exchange sets the prices you read (light-delayed) across the galaxy, and your <b>warehouse</b> here is the only stock it will trade against.</div>` +
    `<div class="pp-desc dim">Getting goods to a colony is a separate act: book the Authority's scheduled <b>freight</b>, or load one of your own freighters and fly it yourself.</div>` +
    `<button class="act act--primary" data-act="market">${svgIcon("concept-market-exchange", "sm")} Open the Market</button>` +
    `<div class="pp-note">No engagement may open inside the Authority's sovereign space — fleeing into it is sanctuary. Public geography, ungated by fog.</div></div>`;
  const fleetRows = fleets.length
    ? `<div class="pp-body"><div class="deps-head">Reported Hub berths · ${fleets.length}</div>` +
      `<section class="sysfleet">${fleets.map(fleetRosterRow).join("")}</section></div>`
    : `<div class="pp-body"><div class="sp-empty">No fleets reported docked at the Hub.</div></div>`;

  setHtml(panel,
    `<div class="pp-head"><div class="panel-title"><div><div class="eyebrow">the Terran Charter Authority</div>` +
    `<h2>Wormhole Hub</h2></div></div>` +
    `<button class="pp-close" data-act="close" title="Close" aria-label="Close">✕</button></div>` +
    `<div class="hub-tabs">${uxTabBar(tabs, hubPanelTab, "hub-tab")}</div>` +
    (hubPanelTab === "overview" ? overview : fleetRows),
  );
}

export function openHubPanel(): void {
  buildHubPanel();
  activateWorkspacePage("hub-panel");
  lastHubPanelSig = "";
  updateHubPanel();
  readout().innerHTML = `<b>Wormhole Hub</b> selected — Exchange, warehouse, and freight desk. <span class="dim">Press <b>M</b> or use the panel.</span>`;
}

export function closeHubPanel(): void {
  deactivateWorkspacePage("hub-panel");
}


// --- Global Market (§9) — MARKET tab: a price board with observed-history
// sparklines + honest staleness, and a buy/sell composer that surfaces the
// integrated curve, finite external liquidity, and the separate physical
// freight decision. UI-only: same messages, same lagged-price model. ----------
// The composer's local selection (the board is the master list, this the detail).
export const composer: { side: Side; commodity: Commodity } = { side: "buy", commodity: "fuel" };


// Settlement is instant at the Market Hub, but the account report is not. Keep
// the player's own just-issued commitments as a pessimistic local overlay until
// the delayed receipt arrives; this prevents the stale wallet from offering the
// same credits or goods twice without pretending the estimate is server truth.

export let marketBuilt = false;

// §market-ux: which Market tab is showing — survives close/reopen within the
// session (M reopens on the last tab).
// §market-ux: owned freight is managed through fleet panels. The Warehouse pane
// is a readout for Hub stock, docked fleets, and shipments already in hand.
export type MarketTab = "exchange" | "warehouse" | "specialists" | "modules";

export let marketTab: MarketTab = "exchange";

export function setMarketTab(tab: MarketTab): void {
  marketTab = tab;
  ($("market-pane-exchange") as HTMLElement).hidden = tab !== "exchange";
  ($("market-pane-warehouse") as HTMLElement).hidden = tab !== "warehouse";
  ($("market-pane-specialists") as HTMLElement).hidden = tab !== "specialists";
  ($("market-pane-modules") as HTMLElement).hidden = tab !== "modules";
  document.querySelectorAll<HTMLButtonElement>("#market-tabs button").forEach((b) => {
    const selected = b.dataset.mtab === tab;
    b.classList.toggle("is-active", selected);
    b.setAttribute("aria-selected", String(selected));
    b.tabIndex = selected ? 0 : -1;
  });
  updateMarket();
}

export function buildMarketPanel(): void {
  if (marketBuilt) return;
  marketBuilt = true;
  // §market-ux: Exchange / Specialists tabs.
  $("market-tabs").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-mtab]") as HTMLElement | null;
    if (b?.dataset.mtab) setMarketTab(b.dataset.mtab as MarketTab);
  });
  $("market-tabs").addEventListener("keydown", (e) => {
    if (!(e instanceof KeyboardEvent) || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
    const tabs = [...document.querySelectorAll<HTMLButtonElement>("#market-tabs button")];
    const current = tabs.indexOf(e.target as HTMLButtonElement);
    if (current < 0) return;
    e.preventDefault();
    const next = e.key === "Home" ? 0
      : e.key === "End" ? tabs.length - 1
      : (current + (e.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
    tabs[next].focus();
    setMarketTab(tabs[next].dataset.mtab as MarketTab);
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
      $("mod-feedback").textContent = `Buying a ${MODULE_LABEL[b.dataset.mbuy as ModuleKind]} from Sol — crate freighter inbound to your home (raidable).`;
    } else if (b.dataset.msell) {
      net.send({ type: "SellModule", module: b.dataset.msell as ModuleKind, n: 1, from_system: home });
      $("mod-feedback").textContent = `Selling a ${MODULE_LABEL[b.dataset.msell as ModuleKind]} to Sol — freighter away, clears on arrival.`;
    }
  });
  // Berthed hulls have no galaxy-map glyph. The hub needs the same explicit
  // access that system fleet rows provide, or a manually parked convoy becomes
  // impossible to recover after the renderer correctly hides it at its berth.
  $("wh-berths").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-wh-fleet],[data-wh-unload]");
    const fleet = el?.dataset.whFleet ?? el?.dataset.whUnload;
    if (!fleet || !state.ghosts.some((g) => g.id === fleet && g.own && g.docked === "hub")) return;
    if (el?.dataset.whUnload) {
      net?.send({ type: "HubUnload", fleet_id: fleet });
      return;
    }
    closeMarket();
    selectShip(fleet);
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
  $("market-orders").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest("[data-cancel-limit]") as HTMLElement | null;
    const orderId = Number(b?.dataset.cancelLimit);
    if (!b || !net || !Number.isFinite(orderId)) return;
    net.send({ type: "CancelLimitOrder", order_id: orderId });
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
      reserveMarketOrder({
        kind: "limit",
        side: composer.side,
        commodity: c,
        orderUnits: qty,
        units: composer.side === "sell" ? qty : 0,
        credits: composer.side === "buy" ? qty * limitPrice : 0,
        limitPrice,
      });
    } else {
      const observed = state.market?.prices.find((p) => p.commodity === c)?.price;
      if (observed === undefined) {
        return;
      }
      const quotedAverage = marketAverageQuote(observed, qty, composer.side);
      const protectionPrice = quotedAverage * (composer.side === "buy"
        ? 1 + MARKET_PROTECTION_FRAC
        : 1 - MARKET_PROTECTION_FRAC);
      net.send(
        composer.side === "buy"
          ? {
              type: "MarketBuy",
              commodity: c,
              units: qty,
              max_unit_price: protectionPrice,
            }
          : {
              type: "MarketSell",
              commodity: c,
              units: qty,
              min_unit_price: protectionPrice,
            },
      );
      reserveMarketOrder({
        kind: "market",
        side: composer.side,
        commodity: c,
        orderUnits: qty,
        units: composer.side === "sell" ? qty : 0,
        credits: composer.side === "buy"
          ? qty * protectionPrice * (1 + (state.charter?.market_penalty_frac ?? 0))
          : 0,
      });
    }
    renderIncomingMarketOrders();
  });
}


// The per-commodity price board: icon | name | observed sparkline | (stale-aware)
// price + observed-trend glyph | held. Selection highlights the active row.
export function renderMarketBoard(): void {
  if (!state.market) return;
  const priceOf = new Map(state.market.prices.map((p) => [p.commodity, p.price]));
  // §TCA: the Exchange settles against the MARKET WAREHOUSE, so the held
  // column has to be the warehouse — showing a system stockpile here would offer a
  // player a sell the sim will soft-reject as InsufficientWarehouseStock.
  const stale = state.market.staleness > 0.5;
  setHtml($("market-board"), COMMODITIES.map((c) => {
    const quote = state.market!.prices.find((row) => row.commodity === c);
    const p = priceOf.get(c);
    const hist = state.priceHistory[c] ?? [];
    const tr = trend(hist);
    const active = composer.commodity === c ? "is-active" : "";
    const priceTxt = p === undefined ? `<span class="is-stale">—</span>` : `${stale ? "~" : ""}${p.toFixed(2)}`;
    const liquidity = quote
      ? `Observed immediate liquidity: buy up to ${quote.available_buy}, sell up to ${quote.available_sell}. This picture is ${state.market!.staleness.toFixed(1)}s old.`
      : "No market report has arrived yet.";
    return `<button class="board__row ${active}" data-resource="${c}" title="${esc(liquidity)}">` +
      `<span class="dep-ico">${commodityIcon(c, "md")}</span>` +
      `<span class="b-name">${label(c)}</span>` +
      spark(hist.length ? hist : (p !== undefined ? [p, p] : [0, 0])) +
      `<span class="b-price ${stale ? "is-stale" : ""}">${priceTxt} <span class="b-trend ${tr.tone}">${tr.glyph}</span></span>` +
      `<span class="b-held">${warehouseUnits(c)}</span></button>`;
  }).join(""));
}


// §economy Part 6 → §market-ux: SOL SPECIALIST CONTRACTS, now a Market TAB of
// their own — five professions at the posted contract price; the contractor ships to
// the player's HOME on a normal raidable personnel convoy (price-certain,
// delivery-risky). Wire slugs stay raw in data-hire; names come from label().
export const SPECIALISTS: { slug: string; icon: IconKey; blurb: string }[] = [
  { slug: "geologist", icon: "extractor", blurb: "mineral extraction" },
  { slug: "petrochemical_engineer", icon: "refinery", blurb: "volatiles, fuel, chemicals" },
  { slug: "xenobiologist", icon: "provisions", blurb: "biomass + agroplex" },
  { slug: "industrial_engineer", icon: "build", blurb: "heavy industry" },
  { slug: "naval_architect", icon: "shipyard", blurb: "shipyards + armaments" },
];

export function renderSpecialistsPane(): void {
  const cost = state.galaxy?.specialist_hire_cost ?? 800;
  const credits = spendableMarketCredits();
  const rows = SPECIALISTS.map((s) =>
    `<div class="board__row" title="A specialist multiplies affine production lines ×1.75 when posted. The personnel transport from Sol is sub-light and raidable.">` +
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
export function renderModulesPane(): void {
  const credits = spendableMarketCredits();
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
      `<span class="dep-ico">${moduleIcon(m, "md")}</span>` +
      `<span class="b-name">${esc(MODULE_LABEL[m])}${held ? ` <span class="dim">·held ${held}</span>` : ""}</span>` +
      `<button class="act" data-mbuy="${m}" ${canBuy ? "" : "disabled"} title="Buy one from Sol → ships a crate to your home (raidable).">Buy ${buyTxt}</button>` +
      `<button class="act" data-msell="${m}" ${held > 0 ? "" : "disabled"} title="Sell one from your home ledger → freighter to Sol, clears on arrival.">Sell ${sellTxt}</button>` +
      `</div>`;
  }).join("");
  $("mod-rows").innerHTML =
    `<div class="mhint" style="margin-bottom:6px">Sol's off-map foundry — buy modules at a premium (a crate ships to your <b>home</b>, raidable) or sell your home ledger back at a discount. Prices track the commodity market; local manufacture at an Armaments Complex is always cheaper.</div>` +
    rows;
}


// Mirrors sim::market's quantity integration so the stale quote includes the
// order's OWN impact. The server is authoritative; this only derives the default
// ±10% protection sent with the order and the preview shown to the player.
export const MARKET_PROTECTION_FRAC = 0.10;


// The submit button carries the ordinary estimate and protection rule. The line
// beneath it is warnings-only: if nothing needs attention, it disappears.
export function renderComposer(): void {
  if (!state.market) return;
  const c = composer.commodity;
  const price = state.market.prices.find((p) => p.commodity === c)?.price;
  const liquidity = state.market.prices.find((p) => p.commodity === c);
  $("mk-sel").textContent = label(c);
  document.querySelectorAll<HTMLElement>("#mk-side button").forEach((b) => b.classList.toggle("is-active", b.dataset.side === composer.side));
  const qty = Math.max(1, Math.floor(Number(($("mk-qty") as HTMLInputElement).value) || 0));
  const limitOn = ($("mk-limit-on") as HTMLInputElement).checked;
  const submit = $("mk-submit");
  const preview = $("mk-preview");
  preview.innerHTML = "";
  if (limitOn) {
    submit.textContent = `Place limit ${composer.side}`;
    submit.title = "Rests on the book and clears in the periodic uniform-price batch. Reacting fastest confers no edge; partial fills carry to the next batch.";
  } else if (composer.side === "buy") {
    const average = price === undefined ? undefined : marketAverageQuote(price, qty, "buy");
    const penFrac = state.charter?.market_penalty_frac ?? 0;
    const pen = average !== undefined ? qty * average * penFrac : 0;
    const warnings: string[] = [];
    if (average === undefined) {
      warnings.push(`<span class="warn">No observed quote yet.</span>`);
      submit.textContent = `Buy ${qty} ${label(c)}`;
      submit.title = "Waiting for a light-delayed market quote.";
    } else {
      const cost = fmt(qty * average * (1 + penFrac));
      const bound = (average * (1 + MARKET_PROTECTION_FRAC)).toFixed(2);
      submit.textContent = `Buy ${qty} ${label(c)} · ~${cost} Cr`;
      submit.title = `Cancels if the true average exceeds ~${bound}/u (+${Math.round(MARKET_PROTECTION_FRAC * 100)}%). Quantity impact included; prices are light-delayed.`;
    }
    if (liquidity && qty > liquidity.available_buy) {
      warnings.push(`<span class="warn" title="The displayed pool is light-delayed; the server soft-rejects an order beyond true available liquidity.">observed supply ${liquidity.available_buy}</span>`);
    }
    if (pen > 0.005) warnings.push(`<span class="warn">includes ${fmt(pen)} Cr charter penalty</span>`);
    preview.innerHTML = warnings.join(" · ");
  } else {
    const average = price === undefined ? undefined : marketAverageQuote(price, qty, "sell");
    const held = warehouseUnits(c);
    const penFrac = state.charter?.market_penalty_frac ?? 0;
    const pen = average !== undefined ? qty * average * penFrac : 0;
    const warnings: string[] = [];
    if (average === undefined) {
      warnings.push(`<span class="warn">No observed quote yet.</span>`);
      submit.textContent = `Sell ${qty} ${label(c)}`;
      submit.title = "Waiting for a light-delayed market quote.";
    } else {
      const gain = fmt(qty * average * (1 - penFrac));
      const bound = (average * (1 - MARKET_PROTECTION_FRAC)).toFixed(2);
      submit.textContent = `Sell ${qty} ${label(c)} · ~${gain} Cr`;
      submit.title = `Cancels if the true average falls below ~${bound}/u (−${Math.round(MARKET_PROTECTION_FRAC * 100)}%). Quantity impact included; prices are light-delayed.`;
    }
    if (held < qty) {
      warnings.push(`<span class="warn" title="Selling draws only from your Market Warehouse. Move goods there first through the Warehouse tab.">Warehouse holds ${held} — ${qty - held} short</span>`);
    }
    if (liquidity && qty > liquidity.available_sell) {
      warnings.push(`<span class="warn" title="The displayed pool is light-delayed; the server soft-rejects an order beyond true available liquidity.">observed demand ${liquidity.available_sell}</span>`);
    }
    if (pen > 0.005) warnings.push(`<span class="warn">${fmt(pen)} Cr charter penalty deducted</span>`);
    preview.innerHTML = warnings.join(" · ");
  }
}


// --- §TCA: the Market Warehouse and shipment queue ----------------------


/// The warehouse table — commodity × units, the Exchange's only stock.
export function renderWarehouse(): void {
  const rows = (state.wallet?.warehouse ?? [])
    .map((holding) => ({ ...holding, units: warehouseUnits(holding.commodity) }))
    .filter((holding) => holding.units > 0);
  $("wh-table").innerHTML = rows.length
    ? rows.map((w) =>
        `<div class="ord">` +
        `${commodityIcon(w.commodity, "sm")} <b>${w.units}</b> ${esc(label(w.commodity))}</div>`).join("")
    : `<div class="mhint dim">Empty. Buy on the Exchange or unload a docked freighter here.</div>`;
}


/// Own fleets in the player's SERVED picture that are berthed at the Market
/// Hub. Docked hulls deliberately disappear from the galaxy map, so this is
/// their durable selection surface and the explicit unload control for a convoy
/// that was moved to the hub without first being assigned a haul mission.
export function renderHubBerths(): void {
  const fleets = state.ghosts
    .filter((g) => g.own && g.docked === "hub")
    .sort((a, b) => a.id.localeCompare(b.id));
  $("wh-berths").innerHTML = fleets.length
    ? fleets.map((g) => {
        const manifest = fleetCargoManifest(g);
        const cargo = manifest.length
          ? manifest.map((stack) => `${fmt(stack.units)} ${label(stack.commodity)}`).join(" · ")
          : "hold empty";
        const exact = fleetExactCount(g);
        const count = exact === null
          ? `est. ${countClassLabel(g.count_class)} ships`
          : `${exact} ship${exact === 1 ? "" : "s"}`;
        const name = `${shipKindLabel(g.kind)} fleet`;
        return `<div class="hubberth">` +
          `<button class="hubberth__fleet" data-wh-fleet="${esc(g.id)}" title="Open this fleet's panel">` +
          `${icon("manifest", "md")}<span><b>${esc(name)}</b><small>${esc(count)} · ${esc(cargo)}</small></span><span>›</span></button>` +
          (manifest.length
            ? `<button class="act act--mini" data-wh-unload="${esc(g.id)}" title="Unload every cargo stack into your Market Warehouse.">${icon("unload", "sm")} Unload all</button>`
            : "") +
          `</div>`;
      }).join("")
    : `<div class="mhint dim">No fleets berthed at the hub.</div>`;
}


/// The shipment queue — your lots, waiting at the Market Hub or aboard a hull.
export function renderShipmentQueue(): void {
  const ships = state.freight?.shipments ?? [];
  $("fr-queue").innerHTML = ships.length
    ? ships
        .map((s) => {
          const where = s.direction === "outbound" ? `→ ${esc(systemName(s.system))}` : `← ${esc(systemName(s.system))}`;
          const st = s.aboard
            ? badge("neutral", "aboard")
            : badge("warn", "awaiting departure");
          const sell = s.direction === "inbound" && s.sell_on_arrival ? ` <span class="dim">· sells on arrival</span>` : "";
          return `<div class="ord">${icon("authorityFreighter", "sm")} ${st} ${commodityIcon(s.commodity, "sm")} <b>${s.units}</b> ${esc(label(s.commodity))} ${where}${sell}</div>`;
        })
        .join("")
    : `<div class="mhint dim">No freight booked.</div>`;
}


export function renderRestingOrders(): void {
  const orders = state.wallet?.orders ?? [];
  setHtml($("market-orders"), orders.length
    ? `<div class="deps-head">Resting limit orders</div>` +
      orders.map((o) => `<div class="ord">${badge(o.side === "buy" ? "positive" : "warn", `${o.side} ${o.units} ${label(o.commodity)} @ ${o.limit_price.toFixed(1)}`)}` +
        `<button class="o-rm" data-cancel-limit="${o.id}" title="Cancel and return remaining escrow">Cancel</button></div>`).join("")
    : "");
}


// The player knows an instruction was sent, but the Market Hub's delayed
// account report has not returned yet. Keep that honest local acknowledgement
// here—not under the trade button and not mixed into the observed order book.
export function renderIncomingMarketOrders(): void {
  pruneMarketReservations();
  setHtml($("market-incoming-orders"), marketReservations.length
    ? marketReservations.map((order) => {
        const action = order.side === "buy" ? "Buy" : "Sell";
        const limit = order.limitPrice === undefined ? "" : ` @ ${order.limitPrice.toFixed(1)}`;
        return `<div class="ord">${icon("inTransit", "sm")} <b>${action} ${order.orderUnits} ${esc(label(order.commodity))}${limit}</b>` +
          ` <span class="dim">· sent · awaiting Market Hub</span></div>`;
      }).join("")
    : `<span class="dim">No incoming orders.</span>`);
}


export function renderRecentMarketOrders(): void {
  setHtml($("market-recent-orders"), recentMarketOrders.length
    ? recentMarketOrders.map((order) => {
        const action = order.side === "buy" ? "Bought" : "Sold";
        const source = order.limitFill ? "limit fill · " : "";
        return `<div class="ord">${icon("confirmed", "sm")} <b>${action} ${order.units} ${esc(label(order.commodity))} @ ${order.unitPrice.toFixed(1)}</b>` +
          ` <span class="dim">· ${source}${esc(agoLabel(order.observedAt))}</span></div>`;
      }).join("")
    : `<span class="dim">No recent executions.</span>`);
}


export let lastMarketSig = "";

export function updateMarket(): void {
  if (renderDeferred("market", updateMarket)) return; // §single-click
  if (!state.market || !state.wallet) return;
  // §perf: the whole 11-pane cascade (incl. the hidden panes + ~25 <img>s) ran on
  // every View at 10 Hz. Skip it when nothing it renders has changed. Two guards:
  //  (1) never rebuild while the player is editing a field/dropdown in the panel
  //      (wipes a half-typed qty / wedges a <select>);
  //  (2) a content signature over the discrete inputs, plus a 1 s simTime heartbeat
  //      so the freshness badge / equity / sparklines / freight countdown still
  //      tick at their (whole-second) display cadence. Continuously-varying fields
  //      (staleness, equity, ticker drift) are deliberately excluded from the sig
  //      and refreshed by that heartbeat, never per-100 ms.
  const mp = $("market");
  const ae = document.activeElement;
  if (ae && mp.contains(ae) && (ae.tagName === "INPUT" || ae.tagName === "SELECT")) return;
  const sig = JSON.stringify([
    state.wallet.credits, state.wallet.warehouse, state.wallet.fuel_total,
    marketReservations.map((reservation) => [reservation.kind, reservation.side, reservation.commodity, reservation.orderUnits, reservation.units, reservation.credits, reservation.limitPrice]),
    recentMarketOrders.map((order) => [order.side, order.commodity, order.units, order.unitPrice, order.limitFill, order.observedAt]),
    state.charter, state.freight,
    state.systems.map((s) => [s.id, s.owner]),
    state.ghosts.filter((g) => g.own && g.docked === "hub").map((g) => [g.id, g.kind, g.composition, g.cargo, g.cargo_manifest]),
    marketTab, Math.floor(state.simTime),
  ]);
  if (sig === lastMarketSig && mp.querySelector("#market-board")?.childElementCount) return;
  lastMarketSig = sig;
  const stale = state.market.staleness;
  const fresh = $("market-fresh");
  fresh.className = "badge " + (stale > 0.5 ? "badge--warn" : "badge--positive");
  fresh.textContent = stale > 0.5 ? `~${stale.toFixed(0)}s stale` : "live";
  fresh.title = "Last-synced, light-delayed prices. History is observed, not forecast.";
  $("market-wallet").innerHTML = statStrip([
    stat("Credits", `${reservedMarketCredits() > 0 ? "~" : ""}${fmt(spendableMarketCredits())} Cr`, "is-accent"),
    stat("Equity", `${fmt(state.wallet.valuation)} Cr`),
  ]);
  renderMarketBoard();
  renderComposer();
  renderRestingOrders();
  renderIncomingMarketOrders();
  renderRecentMarketOrders();
  renderSpecialistsPane();
  renderModulesPane();
  renderWarehouse();
  renderHubBerths();
  renderShipmentQueue();
}


export function addTradeNews(t: TradeEvent): void {
  settleMarketReservation(t);
  recordRecentMarketOrder(t);
  if ($("market").classList.contains("is-open")) {
    renderIncomingMarketOrders();
    renderRecentMarketOrders();
  }
  const log = $("reports-log");
  let text = "";
  switch (t.event) {
    case "Bought":
      text = `Bought ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)} — held in your Market Warehouse.`
        + (t.penalty ? ` (charter penalty ${fmt(t.penalty)} Cr)` : "");
      break;
    case "Delivered": text = t.system
      ? `Delivery arrived: +${t.units} ${label(t.commodity)} — stocked at ${systemName(t.system)}.`
      : `Delivery arrived: +${t.units} ${label(t.commodity)} — into your Market Warehouse.`;
      break;
    case "StockDispatched": text = `Supply freighter away: ${t.units} ${label(t.commodity)} → ${systemName(t.system)} (raidable).`; break;
    case "SellDispatched": text = `Sell freighter away: ${t.units} ${label(t.commodity)} crossing to the hub.`; break;
    case "Sold":
      text = `Sold ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)} on arrival.`
        + (t.penalty ? ` (charter penalty ${fmt(t.penalty)} Cr)` : "");
      break;
    case "LimitPlaced": text = `Limit ${t.side} ${t.units} ${label(t.commodity)} @ ${t.limit_price.toFixed(2)} resting on the book.`; break;
    case "LimitFilled":
      text = `Limit ${t.side} filled in batch: ${t.units} ${label(t.commodity)} @ ${t.unit_price.toFixed(2)}.`
        + (t.penalty ? ` (charter penalty ${fmt(t.penalty)} Cr)` : "");
      break;
    case "LimitCancelled":
      text = `Cancelled limit ${t.side}: ${t.units} ${label(t.commodity)} @ ${t.limit_price.toFixed(2)} — escrow returned.`;
      break;
    case "AutoDispatched": text = `⚙ Standing order #${t.rule_id} shipped ${t.units} ${label(t.commodity)} (auto, raidable).`; break;
    case "SupplyDiverted": {
      const what = t.action === "lost" ? "lost (cargo dropped)"
        : t.action === "returned_home" ? "re-routed home (raidable)"
        : "re-routed to sell at the hub (raidable)";
      text = `⚠ Supply to ${systemName(t.system)} undeliverable — you no longer hold it: ${t.units} ${label(t.commodity)} ${what}.`;
      break;
    }
    case "StorageOverflow":
      text = `⚠ Storage full at ${systemName(t.system)}: ${t.units} ${label(t.commodity)} couldn't be stored — carried on to sell at the hub.`;
      break;
    // --- §TCA: freight + dockside logistics -------------------------------
    case "Rejected": text = `⚠ ${rejectText(t)}`; break;
    case "FreightBooked":
      text = `Booked ${t.units} ${label(t.commodity)} ${t.direction === "outbound" ? "→" : "←"} ${systemName(t.system)} — fee ${fmt(t.fee)} Cr, departs in ${fmtDur(Math.max(0, t.depart_at - state.simTime))}.`;
      break;
    case "FreightMoved": {
      const what = `${t.units} ${label(t.commodity)}`;
      const where = systemName(t.system);
      const remaining = t.remaining ?? 0;
      text =
        t.stage === "departed" ? `Authority freighter away with ${what} (${where}).`
        : t.stage === "collected_for_pickup" ? `Authority freighter collected ${what} at ${where}.`
        : t.stage === "delivered_to_system" && remaining > 0
          ? `⚠ Freight delivered ${what} to ${where}; storage is full, so ${remaining} ${label(t.commodity)} remains aboard for return to your Market Warehouse.`
        : t.stage === "delivered_to_system" ? `Freight delivered: ${what} → ${where}.`
        : t.stage === "arrived_at_warehouse" ? `Freight landed: ${what} from ${where} → your warehouse.`
        : t.stage === "returned_undeliverable" ? `⚠ ${what} couldn't unload at ${where} — returned to your warehouse.`
        : t.stage === "forfeited_on_capture" ? `✖ Lost ${what} awaiting pickup at ${where} — the system fell first.`
        : `✖ ${what} destroyed with the Authority freighter carrying it (${where}).`;
      break;
    }
    case "CharterReinstated": {
      // Name the band the payment actually bought back into, when it moved one.
      const from = projectedBand(state.charter ? state.charter.standing - t.before : 0);
      const to = projectedBand(state.charter ? state.charter.standing - t.after : 0);
      const band = to !== from ? ` Restored to <b>${esc(to)}</b>.` : "";
      text = `Paid the Authority ${fmt(t.cost)} Cr for ${t.points.toFixed(0)} standing (${t.before.toFixed(0)} → ${t.after.toFixed(0)}).${band}`;
      break;
    }
    case "Loaded": text = `Loaded ${t.units} ${label(t.commodity)} at ${t.system ? systemName(t.system) : "the hub"}.`; break;
    case "Unloaded": text = `Unloaded ${t.units} ${label(t.commodity)} at ${t.system ? systemName(t.system) : "the hub"}.`; break;
  }
  const el = document.createElement("div");
  el.className = t.event === "SupplyDiverted" && t.action === "lost" ? "report bad" : "report good";
  el.innerHTML = `<span class="ic" style="color:#7fd4ff">◈</span> ${text}`;
  log.prepend(el);
  while (log.children.length > 6) log.removeChild(log.lastChild!);
  setTimeout(() => el.classList.add("fade"), 12000);
}
