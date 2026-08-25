import { traitLine } from "../../core/derive/captains";
import { berthed, dockedAtSystem, fleetRosterDockName, sendCrew, shipKindLabel, systemFleetsAt } from "../../core/derive/fleet";
import { fmt, triggerLabel } from "../../core/derive/format";
import { allySystems, endpointLabel, foundingHomeSystemId, ownedSystems } from "../../core/derive/geo";
import { COMMODITIES, dispatchBuildKey, shippableStock, systemFlavor } from "../../core/derive/market";
import { latestGroundRecordFor, siegeProgress } from "../../core/derive/orders";
import { nodeBonusDesc, researchQueueIds, sendResearchQueue } from "../../core/derive/research";
import { badgeChip, chip, icon, label } from "../../icons";
import { type CaptainAttribute, type Commodity, countClassLabel, fleetCargoUnits, type FleetDoctrine, fleetExactCount, type GhostView, type LandingOddsView, type StandingEndpoint, type StandingOrder, type StandingTrigger, type SystemInfo, type SystemStateView } from "../../protocol";
import { starConceptUrl, starTypeFor } from "../../stars";
import { liveSimTime, state } from "../../state";
import { toggleCheckin } from "./checkin";
import { closeFaction, syncReinstateCost, toggleFaction } from "./faction";
import { net } from "./index";
import { $, badge, bar, commodityIcon, CONTACT_STALE_AGE_S, esc, readout, renderDeferred, setHtml, stat, statStrip, svgIcon } from "./mapchrome";
import { closeMarket, openMarket, toggleMarket } from "./market";
import { closeOperations, toggleOperations } from "./operations";
import { closeResearch, toggleResearch } from "./research";
import { deselectShip, fmtCountdown, ownActivity, selectShip, updateOfficersPanel, uxTabBar, type UxTabOption } from "./ship";
import { closeSyndicate, toggleSyndicate } from "./syndicate";
import { buildLabel, colonyOpportunityBlock, depositRow, enterSystem, romanTier } from "./sysview";


// --- Workspace rail: one right-docked column hosting System/Market/Logistics/
// Doctrine as a tab stack. Opening any tab opens the rail; one tab shows at a
// time; ✕ / Esc closes it → the map stays uncluttered. ----------------------
// The right rail hosts only the SELECTION/holdings-context tabs. The Market is a
// hub-wide institution → it lives in the TOP NAVBAR as its own overlay, not here.
export type RailTab = "system" | "fleets" | "logistics" | "doctrine" | "officers" | "rankings";

export let railTab: RailTab = "system";

export let railBuilt = false;


export function setRailTab(tab: RailTab): void {
  railTab = tab;
  const bodyId: Record<RailTab, string> = { system: "tab-system", fleets: "tab-fleets", logistics: "standing", doctrine: "doctrine", officers: "tab-officers", rankings: "tab-rankings" };
  for (const t of ["system", "fleets", "logistics", "doctrine", "officers", "rankings"] as RailTab[]) {
    $(bodyId[t]).classList.toggle("is-active", t === tab);
  }
  document.querySelectorAll<HTMLElement>("#rail-tabs button").forEach((b) => {
    b.classList.toggle("is-active", b.dataset.tab === tab);
  });
  // Render the shown tab once on switch (each tab then refreshes per-View only
  // while it's the visible one — see the View handler — so hidden tabs don't churn).
  if (tab === "system") updateSystemTab();
  else if (tab === "fleets") updateFleetsPanel();
  else if (tab === "logistics") updateStandingPanel();
  else if (tab === "doctrine") updateDoctrinePanel();
  else if (tab === "officers") updateOfficersPanel();
  else if (tab === "rankings") updateRankingsPanel();
  $("nav-fleets").classList.toggle("is-active", tab === "fleets");
  $("nav-officers").classList.toggle("is-active", tab === "officers");
}

export function openRail(tab: RailTab): void {
  deselectShip(); // the rail and the ship panel share the right-dock slot
  $("rail").classList.add("is-open");
  setRailTab(tab);
}

export function closeRail(): void {
  $("rail").classList.remove("is-open");
  $("nav-fleets").classList.remove("is-active");
  $("nav-officers").classList.remove("is-active");
}

export function toggleRail(tab: RailTab): void {
  const open = $("rail").classList.contains("is-open");
  if (open && railTab === tab) closeRail();
  else openRail(tab);
}

export function buildRail(): void {
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
  $("tab-officers").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-officer-act]");
    if (!el) return;
    const act = el.dataset.officerAct;
    const captainId = Number(el.dataset.captainId);
    if (act === "recruit" && net) {
      const home = foundingHomeSystemId();
      if (home) net.send({ type: "RecruitCaptain", system_id: home });
    } else if (act === "assign" && net && Number.isFinite(captainId)) {
      const select = $(`officer-assign-${captainId}`) as HTMLSelectElement;
      if (select.value) net.send({ type: "AssignCaptain", captain_id: captainId, fleet_id: select.value });
    } else if (act === "reserve" && net && Number.isFinite(captainId)) {
      net.send({ type: "ReserveCaptain", captain_id: captainId });
    } else if (act === "select-fleet") {
      const fleet = el.dataset.fleet;
      if (fleet) selectShip(fleet);
    } else if (act === "train" && net && Number.isFinite(captainId)) {
      const attribute = el.dataset.attribute as CaptainAttribute | undefined;
      if (attribute) net.send({ type: "TrainCaptain", captain_id: captainId, attribute });
    }
  });
  $("tab-fleets").addEventListener("click", (e) => {
    const row = (e.target as HTMLElement).closest<HTMLElement>("[data-fleet]");
    if (row?.dataset.fleet) selectShip(row.dataset.fleet);
  });
  // Top-navbar destinations (hub-wide, system-independent): Market + Syndicate +
  // Faction + Log.
  $("nav-market").addEventListener("click", toggleMarket);
  $("nav-fleets").addEventListener("click", () => toggleRail("fleets"));
  $("nav-research").addEventListener("click", toggleResearch);
  $("nav-officers").addEventListener("click", () => toggleRail("officers"));
  $("nav-operations").addEventListener("click", toggleOperations);
  $("nav-syndicate").addEventListener("click", toggleSyndicate);
  $("nav-faction").addEventListener("click", toggleFaction);
  $("nav-log").addEventListener("click", toggleCheckin);
  // §TCA: delegated actions inside the Faction panel — close, and the
  // reinstatement desk (the body rebuilds, so both listeners live on the root).
  $("faction-panel").addEventListener("click", (e) => {
    const t = e.target as HTMLElement;
    if (t.closest("[data-fp='close']")) { closeFaction(); return; }
    if (t.id !== "ch-pay" || !net) return;
    const points = Math.max(1, Math.floor(Number(($("ch-points") as HTMLInputElement).value) || 0));
    net.send({ type: "PayReinstatement", points });
  });
  $("faction-panel").addEventListener("input", (e) => {
    if ((e.target as HTMLElement).id === "ch-points") syncReinstateCost();
  });
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
    } else if (act === "role") {
      const member = b.dataset.member;
      const role = b.dataset.role as import("../../protocol").SyndicateRole | undefined;
      if (member && role) net.send({ type: "SetSyndicateRole", member, role });
    } else if (act === "project") {
      if (state.selectedSystemId) net.send({ type: "CreateSyndicateOperation", system_id: state.selectedSystemId });
    } else if (act === "propose-nap" || act === "propose-ceasefire" || act === "declare-war") {
      const name = ($("sy-dip-name") as HTMLInputElement | null)?.value.trim();
      if (!name) return;
      if (act === "declare-war") net.send({ type: "DeclareWar", target_name: name });
      else net.send({ type: "ProposeTreaty", target_name: name, treaty: act === "propose-nap" ? "non_aggression" : "ceasefire" });
    } else if (act === "treaty-response") {
      const proposal = Number(b.dataset.proposal);
      if (Number.isFinite(proposal)) net.send({ type: "RespondTreaty", proposal_id: proposal, accept: b.dataset.accept === "1" });
    } else if (act === "cancel-treaty") {
      const target = b.dataset.target;
      if (target) net.send({ type: "CancelTreaty", target });
    }
  });
  $("operations-panel").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-op]");
    if (!b) return;
    const act = b.dataset.op;
    const operation_id = b.dataset.id;
    if (act === "close") { closeOperations(); return; }
    if (!net || !operation_id) return;
    if (act === "accept") net.send({ type: "AcceptOperation", operation_id });
    else if (act === "abandon") net.send({ type: "AbandonOperation", operation_id });
    else if (act === "assign" && state.selectedShipId) net.send({ type: "AssignOperationFleet", operation_id, fleet_id: state.selectedShipId });
    else if (act === "recover" && state.selectedShipId) net.send({ type: "RecoverOperation", operation_id, fleet_id: state.selectedShipId });
    else if (act === "contribute") {
      const operation = state.operations.find((o) => o.id === operation_id);
      if (operation?.kind.kind !== "syndicate_megaproject") return;
      const commodity: Commodity = operation.kind.stage === 0 ? "alloys" : operation.kind.stage === 1 ? "electronics" : "machinery";
      const units = Math.max(1, Math.floor(Number(($(`op-units-${operation_id}`) as HTMLInputElement | null)?.value) || 0));
      net.send({ type: "ContributeOperationCargo", operation_id, commodity, units });
    }
  });
}


// --- §rankings: the published leaderboard (rail tab) ---------------------------
// A public ledger snapshot (same for everyone), sortable by category. One category
// at a time keeps it legible in the narrow rail; the chips ARE the "sortable
// categories". Your row is highlighted; category leaders wear a title chip.
export type RankCat = {
  slug: string;
  label: string;
  short: string;
  fmt: (r: import("../../protocol").RankingRow) => string;
  sortVal: (r: import("../../protocol").RankingRow) => number;
  tip: string;
};

export const RANK_CATS: RankCat[] = [
  { slug: "valuation", label: "Valuation", short: "Val", fmt: (r) => fmt(r.valuation) + " Cr", sortVal: (r) => r.valuation, tip: "Net worth — credits + holdings at market (the classic ladder)." },
  { slug: "trade_throughput", label: "Trade Throughput", short: "Trade", fmt: (r) => fmt(r.trade_throughput), sortVal: (r) => r.trade_throughput, tip: "Cargo units your freighters delivered (home, ally, or sold at the hub)." },
  { slug: "market_profit", label: "Net Market Profit", short: "Profit", fmt: (r) => fmt(r.market_profit) + " Cr", sortVal: (r) => r.market_profit, tip: "Lifetime exchange P&L — sell proceeds minus buy spend." },
  { slug: "cargo_captured", label: "Cargo Captured", short: "Seized", fmt: (r) => fmt(r.cargo_captured), sortVal: (r) => r.cargo_captured, tip: "Units seized by raiding freighters + plunder taken on captures." },
  { slug: "cargo_protected", label: "Cargo Protected", short: "Guard", fmt: (r) => fmt(r.cargo_protected), sortVal: (r) => r.cargo_protected, tip: "Units delivered by freighters that survived a battle en route." },
  { slug: "battle_efficiency", label: "Battle Efficiency", short: "Kill/Loss", fmt: (r) => (r.battle_ranked ? "×" + r.battle_efficiency.toFixed(2) : "prov."), sortVal: (r) => (r.battle_ranked ? r.battle_efficiency : -Infinity), tip: "Enemy hull destroyed ÷ own hull lost. 'prov.' = too few battles to rank." },
  { slug: "systems_developed", label: "Systems Developed", short: "Built", fmt: (r) => fmt(r.systems_developed), sortVal: (r) => r.systems_developed, tip: "Total system-upgrade tiers built." },
  { slug: "intel_gathered", label: "Intel Gathered", short: "Intel", fmt: (r) => fmt(r.intel_gathered), sortVal: (r) => r.intel_gathered, tip: "Scout snapshots captured." },
  { slug: "recovery", label: "Recovery", short: "Comeback", fmt: (r) => fmt(r.recovery) + " Cr", sortVal: (r) => r.recovery, tip: "Valuation regained since your last major loss (a captured system)." },
];

export let rankingsSortCat = "valuation";

export let lastRankingsSig = "";


export function updateRankingsPanel(): void {
  if (!$("tab-rankings").classList.contains("is-active")) return;
  if (renderDeferred("tab-rankings", updateRankingsPanel)) return; // §single-click guard
  const rows = state.rankings;
  const sig = JSON.stringify([rows, state.playerId, rankingsSortCat]);
  if (sig === lastRankingsSig && $("rankings-body").innerHTML) return;
  lastRankingsSig = sig;

  const el = $("rankings-body");
  if (!rows.length) {
    setHtml(el, `<div class="dim">No ledger published yet — the first close lands within a minute of the campaign start.</div>`);
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
  setHtml(el,
    `<div class="rk-chips">${chips}</div>` +
    `<div class="rk-catname dim">Ranked by <b>${esc(cat.label)}</b></div>` +
    `<div class="rk-table">${body}</div>`);
}


// Master rail of your holdings (only when you own ≥2 — otherwise it's clutter).
export function ownedSystemsRail(): string {
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


export function systemFleetsSection(sys: SystemInfo, fleets: GhostView[]): string {
  if (fleets.length === 0) return "";
  const rows = fleets.map((g) => {
    const exact = fleetExactCount(g);
    const count = exact === null
      ? `est. ${countClassLabel(g.count_class)} ships`
      : `${exact} ship${exact === 1 ? "" : "s"}`;
    const composition = (g.composition ?? [])
      .filter((c) => c.count > 0)
      .map((c) => `${c.count}× ${shipKindLabel(c.kind)}`)
      .join(" · ");
    const summary = composition ? `${count} · ${composition}` : count;
    const speed = Math.hypot(g.vel.x, g.vel.y);
    const status = dockedAtSystem(g, sys.id) ? "docked" : speed > 1 ? "under way" : "holding";
    const tone = status === "docked" ? "accent" : status === "holding" ? "positive" : "neutral";
    const stale = g.age >= CONTACT_STALE_AGE_S
      ? `<span class="sysfleet__seen is-stale">Information delay ${g.age.toFixed(1)}s</span>`
      : "";
    const flagship = g.kind === "titan" ? state.syndicate?.flagship_name?.trim() : null;
    const name = flagship || `${shipKindLabel(g.kind)} fleet`;
    return `<button class="sysfleet__row" data-act="select-fleet" data-fleet="${esc(g.id)}" title="Select ${esc(name)}">` +
      `<span class="sysfleet__main"><b class="sysfleet__name">${esc(name)}</b>` +
      `<span class="sysfleet__summary">${esc(summary)}</span></span>` +
      `<span class="sysfleet__meta">${badge(tone, status)}${stale}</span></button>`;
  }).join("");
  return `<section class="sysfleet"><div class="deps-head">Fleets</div>${rows}</section>`;
}


export let lastFleetRosterSig = "";


export function fleetRosterRow(g: GhostView): string {
  const exact = fleetExactCount(g);
  const count = exact === null
    ? `est. ${countClassLabel(g.count_class)} ships`
    : `${exact} ship${exact === 1 ? "" : "s"}`;
  const composition = (g.composition ?? [])
    .filter((entry) => entry.count > 0)
    .map((entry) => `${entry.count}× ${shipKindLabel(entry.kind)}`)
    .join(" · ");
  const cargo = fleetCargoUnits(g);
  const summary = [count, composition, cargo > 0 ? `${fmt(cargo)} cargo` : ""]
    .filter(Boolean)
    .join(" · ");
  const dock = fleetRosterDockName(g);
  const inBattle = state.battles.some((battle) => battle.participants.includes(g.id));
  const guard = g.guard_target
    ? state.ghosts.find((candidate) => candidate.id === g.guard_target && candidate.own)
    : undefined;
  const activity = dock
    ? `${icon("dock", "sm")} Docked at <b>${esc(dock)}</b>`
    : inBattle
      ? `${icon("battle", "sm")} <b>In battle</b>`
      : guard
        ? `${icon("fleet", "sm")} Guarding <b>${esc(shipKindLabel(guard.kind))} fleet</b>`
        : ownActivity(g);
  const flagship = g.kind === "titan" ? state.syndicate?.flagship_name?.trim() : null;
  const name = flagship || `${shipKindLabel(g.kind)} fleet`;
  return `<button class="sysfleet__row" data-fleet="${esc(g.id)}" title="Select ${esc(name)}">` +
    `<span class="sysfleet__main"><b class="sysfleet__name">${esc(name)}</b>` +
    `<span class="sysfleet__summary">${esc(summary)}</span>` +
    `<span class="fleet-roster__activity">${activity}</span></span>` +
    `<span class="sysfleet__meta">${badge(dock ? "accent" : "neutral", dock ? "docked" : "undocked")}` +
    `<span class="sysfleet__seen${g.age >= CONTACT_STALE_AGE_S ? " is-stale" : ""}">${g.age.toFixed(1)}s delay</span></span></button>`;
}


export function updateFleetsPanel(): void {
  const root = $("tab-fleets");
  if (!root.classList.contains("is-active")) return;
  if (renderDeferred("tab-fleets", updateFleetsPanel)) return;
  const fleets = state.ghosts
    .filter((ghost) => ghost.own)
    .sort((a, b) => shipKindLabel(a.kind).localeCompare(shipKindLabel(b.kind)) || a.id.localeCompare(b.id));
  const sig = JSON.stringify([fleets, state.battles, state.commandSignals, state.orders, state.raids]);
  if (sig === lastFleetRosterSig && root.innerHTML) return;
  lastFleetRosterSig = sig;
  const undocked = fleets.filter((fleet) => !fleet.docked);
  const docked = fleets.filter((fleet) => !!fleet.docked);
  const group = (labelText: string, rows: GhostView[]) => rows.length
    ? `<section class="fleet-roster__group"><div class="deps-head">${esc(labelText)} · ${rows.length}</div>${rows.map(fleetRosterRow).join("")}</section>`
    : "";
  setHtml(root,
    `<div class="panel-title"><div><div class="eyebrow">corporation-wide roster</div><h2>${svgIcon("concept-fleet", "md")} Fleets</h2></div></div>` +
    `<div class="fleet-roster__summary"><span class="dim">Every owned formation in your served picture.</span><b>${fleets.length}</b></div>` +
    (fleets.length
      ? group("Undocked", undocked) + group("Docked", docked)
      : `<div class="sp-empty">No fleet reports available.</div>`),
  );
}


export type SystemSummaryTab = "overview" | "fleets" | "intelligence";

export let systemSummaryTab: SystemSummaryTab = "overview";

export let systemTabBuilt = false;

export function buildSystemTab(): void {
  if (systemTabBuilt) return;
  systemTabBuilt = true;
  $("tab-system").addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest("[data-action],[data-act],[data-sys],[data-build],[data-crew],[data-sys-tab]") as HTMLElement | null;
    if (!el) return;
    const requestedTab = el.dataset.sysTab as SystemSummaryTab | undefined;
    if (requestedTab && (["overview", "fleets", "intelligence"] as SystemSummaryTab[]).includes(requestedTab)) {
      systemSummaryTab = requestedTab;
      lastSystemTabSig = "";
      updateSystemTab();
      return;
    }
    if (el.dataset.act === "select-fleet") {
      const id = el.dataset.fleet;
      if (id && state.ghosts.some((g) => g.id === id && g.own)) selectShip(id);
      return;
    }
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
          `Booking Authority pickup for <b>${manifest.map((s) => `${s.units} ${esc(label(s.commodity))}`).join(", ")}</b> → Market Warehouse — ` +
          `ordinary fees and departure queues apply; each lot sells on arrival. ` +
          `<span class="dim">Fuel stays as the reserve; this creates no corporate hull.</span>`;
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


export let lastSystemTabSig = "";

export function updateSystemTab(): void {
  if (!systemTabBuilt) return;
  if (renderDeferred("tab-system", updateSystemTab)) return; // §single-click
  const root = $("tab-system");
  const sid = state.selectedSystemId;
  const sys = sid && state.galaxy ? state.galaxy.systems.find((s) => s.id === sid) : undefined;
  const fleets = sys ? systemFleetsAt(sys) : [];
  // §perf: the rail panel (incl. its star concept <img>) rebuilt on every View at
  // 10 Hz. Skip when nothing it shows changed — it reads the selected system's
  // dynamic slice, served fleets in its well, the owned-holdings rail, and
  // syndicate/research affordances; a 1 s heartbeat keeps ages and ETAs ticking.
  const stSig = JSON.stringify([
    state.selectedSystemId,
    systemSummaryTab,
    state.systems,
    fleets.map((g) => [
      g.id, g.kind, g.docked, Math.hypot(g.vel.x, g.vel.y) > 1,
      Math.floor(g.age), g.count_class, g.composition,
    ]),
    state.syndicate,
    state.research?.programmes.map((p) => p.state) ?? null,
    state.anchors.length,
    Math.floor(state.simTime),
  ]);
  if (stSig === lastSystemTabSig && root.innerHTML) return;
  lastSystemTabSig = stSig;
  const rail = ownedSystemsRail();
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
  const bandIcon = sys.band === "poor" ? icon("geologyPoor", "sm") : sys.band === "rich" ? icon("geologyRich", "sm") : "";
  const strip = statStrip([
    // §explore: exact deposit count/yield are survey knowledge — unsurveyed
    // shows the public band instead.
    stat("Band", `${bandIcon}${sys.band.toUpperCase()}`, sys.band === "rich" ? "is-accent" : ""),
    stat("Deposits", deps ? String(deps.length) : "?"),
    stat("Yield/s", deps ? yieldRate.toFixed(1) : "?"),
    stat("Stock", `${icon("storage", "sm")}${mine && cap > 0 ? `${fmt(used)} / ${fmt(cap)}` : mine ? fmt(stockTotal) : "—"}`, storageFull ? "is-warn" : ""),
    // Development slots (owner-only; §buildings step 1) — the specialization budget.
    stat("Slots", `${icon("slots", "sm")}${mine ? `${dyn?.slots_used ?? 0}/${dyn?.slots_total ?? 0}` : "—"}`,
      mine && (dyn?.slots_total ?? 0) > 0 && (dyn?.slots_used ?? 0) >= (dyn?.slots_total ?? 0) ? "is-warn" : ""),
  ]);
  // Storage fill bar + full warning, under the strip (owner-only).
  const storageBar = mine && cap > 0
    ? `<div class="storage-row">${bar(Math.min(100, (used / cap) * 100), storageFull ? "is-warn" : "")}` +
      (storageFull ? `<span class="storage-warn">${badge("warn", "storage full")} production idling — ship goods out or build an Orbital Warehouse</span>` : "") +
      `</div>`
    : "";
  // §management-home: the rail is a SUMMARY now — the build menu, production
  // readout, and developments detail moved INTO the System View's management
  // column (one management UI to maintain, not two). The rail keeps the header,
  // stats strip, stockpile summary, ATTENTION CUES, and a prominent way in.
  const cues: string[] = [];
  if (mine) {
    if (dyn?.blockade) cues.push(`${badge("negative", "blockaded")} logistics cut — freighters held in &amp; out`);
    if (storageFull) cues.push(`${icon("storage", "sm")}${badge("warn", "storage full")} production idling`);
    if ((dyn?.population ?? 0) > 0 && !dyn?.habitat_fed) cues.push(`${icon("food", "sm")}${badge("warn", label(dyn?.food_state ?? "rationing"))} workforce slowed — ship provisions`);
    if (dyn?.node?.awakened && !dyn.node.fed) cues.push(`${icon("upkeep", "sm")}${badge("warn", "node unfed")} bonus suspended — ship its upkeep`);
    // §build-progress: the compact construction line — a glance from the map
    // says work is running (and when the next job lands) without opening the view.
    const jobs = dyn?.builds ?? [];
    if (jobs.length === 1) {
      cues.push(`${icon("queue", "sm")} building: <b>${esc(buildLabel(jobs[0].key))}</b> — ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
    } else if (jobs.length > 1) {
      cues.push(`${icon("queue", "sm")} building ×${jobs.length} — next ${fmtCountdown(Math.max(0, jobs[0].complete_time - liveSimTime()))}`);
    }
  }
  const attention = cues.length ? `<div class="mhint" style="margin-top:6px">${cues.join(" · ")}</div>` : "";

  // §explore R2: surveyed-or-owner → the full geology table; unsurveyed → the
  // band + "composition unsurveyed" (survey it — or claim blind and find out).
  // Economic traits arrive with survey data (or ownership), so the player can
  // choose a colony rather than discovering the entire reason only after claim.
  const tr = dyn?.trait ? traitLine(dyn.trait) : null;
  const traitRow = tr
    ? `<div class="mhint" style="margin-top:4px${tr.warn ? ";color:var(--warn)" : ""}" title="${esc(tr.desc)} Economic trait — revealed by survey or ownership.">` +
      `${badge(tr.warn ? "warn" : "accent", tr.title)} ${esc(tr.desc)}</div>`
    : "";
  const opportunityRows = colonyOpportunityBlock(dyn);
  const geology = deps
    ? `<div class="sysview__deps"><div class="deps-head">${bandIcon} Geology — richer toward the frontier</div>` +
      deps.map(depositRow).join("") + traitRow + opportunityRows + `</div>`
    : `<div class="sysview__deps"><div class="deps-head">${bandIcon} Geology</div>` +
      `<div class="mhint" title="The spectral read gives only the richness band. Send a scout to SURVEY the exact deposits, planetary mineral grades and rare features — or claim blind.">` +
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
      ? ` <span style="color:var(--warn)" title="You know only the band — exact deposits, mineral grades and rare features are unknown. Survey first with a scout, or claim blind.">unsurveyed — claiming blind</span>`
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
    ? `<button class="act act--primary" data-action="inspect" style="margin-top:8px" title="${isMyHome ? "Your command center sits here. " : ""}Run this system from its System View — build/develop, production, and shipping. Freighters cross fogged space to the hub, raidable in transit.">${icon("build", "sm")} Open System View ▸</button>`
    : `<button class="act" data-action="inspect" title="Inspect this system (public geography — its holdings stay fogged unless you own it).">◎ Inspect ▸</button>`;

  // §ground: the landing readout sits with the siege badge — this is the panel a
  // BESIEGER stares at while their guns work, so the marine requirement has to
  // be here and not only on the owner's own colony sheet.
  const tabs: readonly UxTabOption<SystemSummaryTab>[] = [
    ["overview", "Overview", "planetHabitable"],
    ["fleets", "Fleets", "fleet"],
    ["intelligence", "Intel", "intel"],
  ];
  const overview = starFeature + strip + storageBar + nodeBlock;
  const activity = groundLine(dyn) + berthLine(sys.id) + systemFleetsSection(sys, fleets);
  const active = systemSummaryTab === "overview"
    ? overview
    : systemSummaryTab === "fleets"
      ? activity || `<div class="sp-empty">No fleet activity reported.</div>`
      : geology + intelBlock;
  // The one primary route into the system sits directly under identity. Alerts
  // follow it, before the reference cards; duplicated claim guidance is gone.
  setHtml(root, rail + header + `<div class="ux-primary-actions">${actions}</div>` +
    (attention ? `<div class="ux-alert">${attention}</div>` : "") +
    uxTabBar(tabs, systemSummaryTab, "sys-tab") + `<div class="ux-tab-body">${active}</div>`);
}


export let standingBuilt = false;

export function buildStandingPanel(): void {
  if (standingBuilt) return;
  standingBuilt = true;
  const trig = $("so-trigger") as HTMLSelectElement;
  const syncForm = () => {
    const amt = $("so-amount") as HTMLInputElement;
    ($("so-floor-row") as HTMLElement).style.display = trig.value === "percent_surplus" ? "flex" : "none";
    // §TCA: the sell-on-arrival choice only means anything for a Market Hub rule.
    const destIsHub = (($("so-dest") as HTMLSelectElement).value || "") === "hub";
    ($("so-sell-row") as HTMLElement).style.display = destIsHub ? "flex" : "none";
    amt.title = trig.value === "above_threshold" ? "threshold (units)"
      : trig.value === "percent_surplus" ? "percent (1–100)"
      : "target level (units)";
  };
  trig.addEventListener("change", syncForm);
  ($("so-dest") as HTMLSelectElement).addEventListener("change", syncForm);
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
      // §TCA: a Hub rule sells on arrival by default (today's behaviour); the
      // Market Hub panel exposes the stockpile-instead option.
      sell_on_arrival: ($("so-sell") as HTMLInputElement).checked,
    };
    net.send({ type: "SetStandingOrder", order });
  });
}


export let lastStandingListSig = "";

export function updateStandingPanel(): void {
  if (!standingBuilt) return;
  if (renderDeferred("standing", updateStandingPanel)) return; // §single-click
  // Rebuild source/dest selects only when the owned-systems set changes (so a
  // mid-edit selection isn't clobbered every tick).
  const owned = ownedSystems();
  const allies = allySystems();
  // Key includes the ally set so the dest list rebuilds when an alliance forms/ends.
  const ownedKey = owned.map((s) => s.id).join(",") + "|" + allies.map((s) => s.id).join(",");
  const srcSel = $("so-source") as HTMLSelectElement;
  const destSel = $("so-dest") as HTMLSelectElement;
  // Skip the rebuild while EITHER native dropdown is open, or Chrome's popup
  // wedges the tab (the fixed Deliver-dropdown bug); it retries next update.
  if (srcSel.dataset.key !== ownedKey && document.activeElement !== srcSel && document.activeElement !== destSel) {
    srcSel.dataset.key = ownedKey;
    const prevSrc = srcSel.value, prevDest = destSel.value;
    // Source is always your OWN system; destinations add hub/home + your colonies +
    // ally systems (§syndicates Part 3 AID).
    srcSel.innerHTML = owned.length
      ? owned.map((s) => `<option value="${s.id}">${s.name}</option>`).join("")
      : `<option value="">(claim a system first)</option>`;
    if (owned.some((s) => s.id === prevSrc)) srcSel.value = prevSrc;
    destSel.innerHTML = `<option value="hub">hub (sell)</option><option value="home">home (store)</option>` +
      owned.map((s) => `<option value="${s.id}">${s.name} (colony)</option>`).join("") +
      allies.map((s) => `<option value="${s.id}">${s.name} (ally aid)</option>`).join("");
    if (prevDest) destSel.value = prevDest;
  }
  const comSel = $("so-commodity") as HTMLSelectElement;
  if (!comSel.options.length) comSel.innerHTML = COMMODITIES.map((c) => `<option value="${c}">${label(c)}</option>`).join("");

  const list = $("standing-list");
  const orders = state.standingOrders;
  // §perf: the list is effectively static between edits — rebuild it only when an
  // order's shown fields change (id/status/in-flight/route/trigger), not at 10 Hz
  // (which churned garbage and risked resetting #standing-list's scroll).
  const listSig = JSON.stringify(orders.map((o) => [o.id, o.status, o.in_flight, o.commodity, o.source, o.dest, o.trigger]));
  if (listSig === lastStandingListSig && list.innerHTML) return;
  lastStandingListSig = listSig;
  if (!orders.length) {
    setHtml(list, `<span class="dim">No standing orders yet — set one below. They run on the server while you're away.</span>`);
    return;
  }
  setHtml(list, orders
    .map((o) => {
      const flight = o.in_flight ? `<span class="run">● freighter en route</span>` : `<span class="dim">idle</span>`;
      const paused = o.status === "paused" ? " · paused" : "";
      return `<div class="so"><span class="x" data-clear="${o.id}" title="remove">✕</span>` +
        `<b>#${o.id}</b> ${commodityIcon(o.commodity, "sm")} ${label(o.commodity)}: ${endpointLabel(o.source)} → ${endpointLabel(o.dest)}${paused}<br>` +
        `<span class="meta">${triggerLabel(o.trigger)} · ${flight}</span></div>`;
    })
    .join(""));
  // ✕ handling is DELEGATED on the persistent list root (see buildStanding…) —
  // per-render listeners on the rebuilt rows was the old pattern; §single-click
  // standardizes on delegation everywhere.
}


// --- Fleet doctrine panel (§16) — constrained combat & logistics policy -------
// Four dropdowns, each a closed menu mirroring the sim enums; any change sends
// the whole doctrine (instant local admin — the convoys/pickets it commands stay
// raidable & light-revealed). Every field defaults to today's behaviour.
export const DOCTRINE_FIELDS: { key: keyof FleetDoctrine; id: string; opts: [string, string][] }[] = [
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
    ["guard_nearest", "Guard nearest freighter (default)"],
    ["guard_richest", "Guard richest freighter"],
    ["hold_station", "Hold station — picket your route"],
  ] },
  { key: "destination_invalid", id: "fd-dest", opts: [
    ["drop", "Lost supply: drop cargo (default)"],
    ["return_home", "Lost supply: re-route home"],
    ["sell_at_hub", "Lost supply: sell at hub"],
  ] },
];


export let doctrineBuilt = false;

export function buildDoctrinePanel(): void {
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


export function updateDoctrinePanel(): void {
  if (!doctrineBuilt) return;
  for (const f of DOCTRINE_FIELDS) {
    const sel = $(f.id) as HTMLSelectElement;
    // Don't clobber a dropdown the player is actively changing.
    if (document.activeElement === sel) continue;
    sel.value = String(state.doctrine[f.key]);
  }
}


// §contestable-territory Part 2: siege progress for a system's blockade view
// field. Returns null unless the (defense-suppressed) siege clock is running.
// `pct` fills a bar; `left` is the capture countdown; `ripe` = a landing force
// delivered now would capture.
// §ground: the LANDING readout — the one number that makes orbital bombardment
// legible. Both participants read the SAME figure from the same fog-safe view
// field (owner + besieger only; see `SystemStateView.ground`), so a besieger can
// watch `marines_needed` fall as their guns pin the garrison, and the owner can
// watch it fall too and know exactly how exposed they are. No ground, no line.
export function groundLine(dyn: SystemStateView | undefined): string {
  const g = dyn?.ground;
  if (!g || g.garrison_tier === 0) return "";
  const supp = Math.round(g.suppression * 100);
  const mine = dyn!.owner === state.playerId;
  const tip = mine
    ? `Your Garrison ${romanTier(g.garrison_tier)} holds this ground. A landing of about ${g.marines_needed} marines would be an even fight — fewer and their commanders won't come down at all. Bombardment from a blockading fleet pins the garrison and lowers that number, but only while their guns keep firing: break the blockade and your pinned troops rejoin the fight, even mid-landing.`
    : `A Garrison ${romanTier(g.garrison_tier)} holds this ground. About ${g.marines_needed} marines makes it an even fight, and below that your commanders will hold off rather than throw the men away. Keep a fleet on station bombarding to pin the garrison and cut the number — the suppression bleeds off the moment you leave, and a landing already on the ground can lose because of it.`;
  const need = g.marines_needed === 0
    ? badgeChip("garrison", "ground open — no defenders", mine ? "negative" : "positive", tip)
    : badgeChip("garrison", `even odds at ${g.marines_needed} marines`, mine ? "positive" : "warn", tip);
  const fed = g.garrison_fed
    ? ""
    : ` ${badgeChip("garrison", "UNFED — not defending", mine ? "negative" : "positive", "This garrison has no Provisions, so it counts for nothing until the colony feeds it. Nothing is lost — it stands back up the moment supply returns.")}`;
  const suppTag = supp > 0
    ? ` <span class="dim" title="Orbital bombardment currently pinning the garrison. It decays once the guns stop.">· ${supp}% suppressed</span>`
    : "";
  // §ground G3: if a landing has been fought here and its light has reached us,
  // offer the replay right where the ground is described.
  const landing = latestGroundRecordFor(dyn!.id);
  const watch = landing
    ? ` <button class="pp-btn pp-btn--sm" data-act="watch-landing" data-landing="${esc(landing.id)}" title="${esc(
        landing.outcome === null
          ? "A landing is being fought here right now. Watch it as its rounds reach you."
          : "Replay this landing — both sides' strength round by round, and the moment it turned.",
      )}">${landing.outcome === null ? "◉ Landing in progress" : "▶ Replay the landing"}</button>`
    : "";
  return `<div class="deps-head" style="margin-top:6px">${icon("garrison", "sm")} Garrison ${romanTier(g.garrison_tier)} ${need}${fed}${suppTag}${watch}</div>`
    + landingOddsLine(g.landing);
}


/// §ground G4: the PRE-COMMIT ESTIMATE, shown to the besieger who has the men.
/// A landing is rolled now, so this is what stands between "my gamble" and "the
/// game robbed me" — it prices the decision BEFORE the transports go down, and
/// it is sampled from the same engine that will fight it.
export function landingOddsLine(o: LandingOddsView | null | undefined): string {
  if (!o) return "";
  const pct = (v: number) => `${Math.round(v * 100)}%`;
  const tone = o.win >= 0.85 ? "positive" : o.win >= 0.5 ? "warn" : "negative";
  // THE WARNING: a landing that only works while the guns fire is the blockade's,
  // not the men's — and it lasts exactly as long as you hold the orbit.
  const fragile = o.win - o.win_if_guns_leave >= 0.25;
  const chip = badgeChip(
    "garrison",
    `landing: ${pct(o.win)}`,
    tone,
    `Sampled from the real ground engine using the ${o.marines} marines you have in orbit. ` +
      `Expect to lose about ${o.expected_losses} of them over roughly ${Math.round(o.expected_secs)}s of fighting. ` +
      `A landing is fought out and rolled — margin above the break-even strength buys confidence, never certainty.`,
  );
  const warn = fragile
    ? ` ${badgeChip(
        "garrison",
        `${pct(o.win_if_guns_leave)} if the guns leave`,
        "negative",
        "This landing belongs to your BOMBARDMENT, not to your marines. If the blockade breaks while they are on the ground, the pinned garrison comes out of cover and these odds collapse. Hold the orbit for the whole landing, or bring more men.",
      )}`
    : "";
  return `<div class="deps-head" style="margin-top:4px"><span class="dim">Your ${o.marines} marines —</span> ${chip}${warn}</div>`;
}


/// The berth readout for a system panel: how many hulls are parked here, and
/// whose. Rival hulls are counted separately — you can see that someone else's
/// ships are sitting on this ground, which is exactly what their sprites used
/// to tell you.
export function berthLine(systemId: string): string {
  const all = berthed(systemId);
  if (all.length === 0) return "";
  const mine = all.filter((g) => g.own);
  const others = all.length - mine.length;
  const chip = (n: number, singular: string, plural: string, tone: "positive" | "warn") =>
    badgeChip("garrison", `${n} ${n === 1 ? singular : plural}`, tone,
      `Hulls berthed here — at rest, and available for loading, repair and refit. They are not drawn on the galaxy map; a docked ship belongs to the system view.`);
  return `<div class="deps-head" style="margin-top:6px">⚓ Berths ` +
    (mine.length > 0 ? chip(mine.length, "of yours", "of yours", "positive") : "") +
    (others > 0 ? ` ${chip(others, "other hull", "other hulls", "warn")}` : "") +
    `</div>`;
}

