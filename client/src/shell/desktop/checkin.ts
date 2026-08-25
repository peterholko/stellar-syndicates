import { traitLine } from "../../core/derive/captains";
import { agoLabel, arrivalLocal } from "../../core/derive/format";
import { commandDelayTo, freshSurveyReports, locName, REPORT_RECENT_S, systemName } from "../../core/derive/geo";
import { nextDecisionLabel, siegeProgress } from "../../core/derive/orders";
import { nodeBonusDesc } from "../../core/derive/research";
import { icon, type IconKey, label } from "../../icons";
import { countClassLabel, type TimelineEntry, type Vec2 } from "../../protocol";
import { liveSimTime, state } from "../../state";
import { openBattlePanel, openOngoingBattlePanel } from "./battle";
import { net } from "./index";
import { $, esc, openCapturePanel, renderDeferred, setHtml, statusIcon } from "./mapchrome";
import { openRail } from "./rail";
import { fmtCountdown, selectShip } from "./ship";


// --- Check-in modal (top-navbar destination; the welcome-back digest) ----------
export function openCheckin(): void {
  $("checkin").style.display = "block";
  $("nav-log").classList.add("is-active");
  updateCheckinPanel();
}

export function closeCheckin(): void {
  $("checkin").style.display = "none";
  $("nav-log").classList.remove("is-active");
}

export function toggleCheckin(): void {
  if ($("checkin").style.display === "none") openCheckin();
  else closeCheckin();
}


// ================= DECISION INBOX (§decision-inbox) =========================
// The digest's PRIMARY surface: not "what happened" but "what deserves a
// decision". Every item is a PURE FUNCTION of already-delivered, OWNER-GATED View
// state — blockade is participant-only, stockpile/tiers/garrison are owner-only,
// battle/capture reports are per-participant, ghosts are the fog-safe delayed
// feed — so the inbox carries NO new information and there is nothing to leak
// beyond what the View already (leak-tested) reveals. Priority is encoded in the
// weights: threats > strangulation > idle capacity > information (tunable here).
export const INBOX_W = {
  siege: 100, battle: 92, hostile: 85, captureLost: 82, blockade: 80,
  garrisonUnfed: 70, nodeUnfed: 68, enclave: 58, storageFull: 55, unfedHabitat: 50, idleStockpile: 48,
  brokenOrder: 46, surveyReport: 45, nodeAwakening: 44, dryRefinery: 42, nodeOpportunity: 41, myGarrisonUnfed: 40,
  surveyOpportunity: 36, emptyQueue: 34,
  captureWon: 28, battleReport: 26, noAutomation: 20,
};

export const HOSTILE_CONCERN_MULT = 1.6;
 // a raider within this × sensor_range of an asset
export const IDLE_UNITS = 30;
 // idle-stockpile threshold
export const MAX_HOSTILE_ITEMS = 4;


export type InboxTone = "negative" | "warn" | "info" | "neutral";

export type InboxAction = { label: string; icon?: IconKey; run: () => void; deliveryPos?: Vec2; primary?: boolean; danger?: boolean };

export type InboxItem = { key: string; weight: number; tone: InboxTone; icon: IconKey; headline: string; stakes?: string; age?: number; confidence?: string; actions: InboxAction[] };


export const dismissedInbox = new Set<string>();

export let currentInbox: InboxItem[] = [];


// §explore Part 4: SURVEY REPORT detection — geology APPEARING for a system we
// didn't previously know (our survey landing, or an ally's relayed copy; the
// view field is the single source, so this is fog-safe by construction). Seeded
// silently on the first View (the join payload isn't news); systems WE own are
// suppressed (claiming reveals by holding, not by a report).
// Deep-link actions (close the inbox, focus the relevant panel/target).
export function inboxFocusSystem(id: string): void { state.selectedShipId = null; state.selectedOrderId = null; state.selectedSystemId = id; closeCheckin(); openRail("system"); }

export function inboxFocusFleet(id: string): void { closeCheckin(); selectShip(id); }

export function inboxOpenLogistics(): void { closeCheckin(); openRail("logistics"); }

export const dismissAct = (key: string): InboxAction => ({ label: "Dismiss", run: () => { dismissedInbox.add(key); renderInbox(); } });


// Derive the prioritized inbox from owner-gated View state. Deterministic order
// (weight desc, then key) so it rebuilds identically on reconnect.
export function computeInbox(): InboxItem[] {
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
        stakes: sg.ripe ? "CRITICAL — rival marines landing now TAKE it." : `Falls in ${fmtCountdown(sg.left)} unless you break the blockade or rebuild a Defense Platform.`,
        age: s.blockade.since,
        actions: [{ label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(`siege:${s.id}`)] });
    } else {
      push({ key: `blockade:${s.id}`, weight: INBOX_W.blockade, tone: "negative", icon: "blockade",
        headline: `${systemName(s.id)} — under BLOCKADE`,
        stakes: "Freighters held in & out; production idles. Break it with relief, or build a Defense Platform tier.",
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
      stakes: r.captor ? "Territory taken — plunder seized." : "Rival marines landed at full siege and took the system.",
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
        stakes: "Production idles at the cap. Ship goods out, automate it, or build an Orbital Warehouse (nothing is lost).",
        actions: [{ label: "Book pickup", icon: "cargo", run: () => { if (net) net.send({ type: "ShipProduction", system_id: s.id }); } }, { label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics }, { label: "Focus", run: () => inboxFocusSystem(s.id), primary: true }, dismissAct(key)] });
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
        actions: [{ label: "Auto-supply", icon: "doctrine", run: inboxOpenLogistics, primary: true }, { label: "Book pickup", icon: "cargo", run: () => { if (net) net.send({ type: "ShipProduction", system_id: s.id }); } }, dismissAct(key)] });
    }
    // A DEVELOPED-but-idle system (a claimed frontier with nothing built/building).
    if ((s.slots_total ?? 0) > 0 && (s.slots_used ?? 0) === 0 && (s.builds?.length ?? 0) === 0) {
      const key = `queue:${s.id}`;
      push({ key, weight: INBOX_W.emptyQueue, tone: "info", icon: "build",
        headline: `${systemName(s.id)} — nothing built yet`,
        stakes: `${s.slots_total} development slot(s) free and idle — develop it (Mining Complex, Orbital Warehouse, Sensor…).`,
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
    const roles = (dyn.opportunities ?? []).slice(0, 3)
      .map((o) => `${o.tier === "jackpot" ? "JACKPOT " : ""}${o.title}${o.body_name ? ` — ${o.body_name}` : ""} ×${o.score.toFixed(2)}`);
    const garden = [...(dyn.bodies ?? [])].sort((a, b) => b.habitat_capacity_mult - a.habitat_capacity_mult)[0];
    const minerals = [...(dyn.bodies ?? [])]
      .filter((b) => b.geology != null)
      .sort((a, b) => (b.mineral_extraction_mult ?? 1) - (a.mineral_extraction_mult ?? 1))[0];
    const discovered = roles.length ? roles.join(" · ") : [
      garden ? `Best settlement: ${garden.name} (${label(garden.size)} ${label(garden.environment)})` : "",
      minerals ? `Best minerals: ${minerals.name} (${label(minerals.geology!)})` : "",
      dyn.trait ? traitLine(dyn.trait).title : "",
    ].filter(Boolean).join(" · ");
    const unowned = dyn.owner === null;
    push({ key, weight: INBOX_W.surveyReport, tone: "info", icon: "intel",
      headline: `Survey report: ${systemName(sid)} (${info.band.toUpperCase()} band)`,
      age: t,
      stakes: `${summary || "barren"}.${discovered ? ` ${discovered}.` : ""}` +
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
        stakes: "The spectral read says rich, but its deposits, mineral grades and rare features are unknown. Survey before committing a colony ship — or claim blind.",
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


// Render one inbox card (headline+icon, age chip, stakes, confidence, action row
// with per-button delivery times for order-issuing verbs).
export function inboxCardHtml(it: InboxItem, i: number): string {
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
export function renderInbox(): void {
  const items = computeInbox();
  currentInbox = items;
  const el = $("checkin-attention");
  $("checkin-att-head").textContent = `Decision inbox${items.length ? ` (${items.length})` : ""}`;
  setHtml(el, items.length
    ? items.map((it, i) => inboxCardHtml(it, i)).join("")
    : `<div class="inbox-clear">${icon("success", "sm")} ${esc(nextDecisionLabel())}</div>`);
}


export let checkinBuilt = false;

export function buildCheckinPanel(): void {
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


export function updateCheckinPanel(): void {
  if (!checkinBuilt) return;
  // §perf: the modal is closed most of the session — skip the inbox recompute +
  // innerHTML rebuild entirely while hidden. openCheckin() re-renders on open and
  // the Timeline handler sets state.timeline before calling us, so nothing is lost.
  if ($("checkin").style.display === "none") return;
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
  setHtml($("checkin-timeline"), awayHtml + earlierHtml);
}

