// Shared orders derivations extracted from the desktop shell.

import {
  type BattleView,
  type GhostView,
  type GroundRecordView,
  type PendingOrderView,
  type SystemStateView,
  type Vec2,
} from "../../protocol";
import { label } from "../../icons";
import { renderer } from "../../render";
import { liveSimTime, state, type PendingIntent } from "../../state";
import { emplacementLabel, systemName } from "./geo";
import { projectedBand } from "./research";
import { estimatedFuelForLeg, fleetBaseSpeed, shipKindLabel, WARP_FACTOR } from "./fleet";
import { doneAtLocal, fmt, fmtDur } from "./format";

export const TCA_INCIDENT_LOSS_UI = 10;
export const SURVEY_SECS_UI = 20;
const ORDER_ETA_FUDGE_LO = 0.10;
const ORDER_ETA_FUDGE_HI = 0.25;
const BATTLE_LS_KEY = "ss_battle_marks";

export const orderPoint = (p: Vec2): string =>
  `(${Math.round(p.x).toLocaleString()} · ${Math.round(p.y).toLocaleString()})`;

export const jumpDepartureSelection: { key: string | null } = { key: null };

// Per-hull stats mirror crates/sim/src/ship.rs. Shared by the build surface and
// the armed-selection legality preview.
export const SHIP_STATS: Record<string, { role: string; speed: number; hull: number; atk: number; def: number; slots: number; cap: string }> = {
  scout: { role: "Eyes of the fleet — fastest hull, gathers intel; unarmed, dies if caught.", speed: 115, hull: 80, atk: 0, def: 0, slots: 1, cap: "No cargo · widest sensor bubble" },
  corvette: { role: "Armored escort/garrison — built to be shot at; too slow to chase raiders.", speed: 65, hull: 800, atk: 1, def: 4, slots: 2, cap: "No cargo · screens freighters" },
  raider: { role: "Fast corporate interceptor — patrols, responds to threats, and can seize hostile cargo.", speed: 100, hull: 200, atk: 3, def: 2, slots: 2, cap: "No cargo · jump capable" },
  convoy: { role: "Bulk freighter — carries goods to the hub; raidable, wants an escort.", speed: 40, hull: 4500, atk: 0, def: 1, slots: 0, cap: "Hauls cargo (raidable)" },
  colony: { role: "Settlement ship — carries colonists to physically claim a system.", speed: 33, hull: 6000, atk: 0, def: 1, slots: 0, cap: "Carries a colony (one claim)" },
  destroyer: { role: "The first ship of the line — heavy beam broadsides (beam ×1.20).", speed: 55, hull: 2000, atk: 2.4, def: 2.6, slots: 3, cap: "Line IV research · 8 fit pts" },
  cruiser: { role: "The season's prestige warship — armored core (protection ×1.20); the efficiency peak.", speed: 45, hull: 4000, atk: 4.5, def: 5.5, slots: 4, cap: "Line V research · 12 fit pts" },
  battleship: { role: "The siege anchor — driver broadsides (driver ×1.20); accelerates a siege clock on station.", speed: 36, hull: 8000, atk: 8, def: 12, slots: 4, cap: "Line VI research · 18 fit pts" },
  dreadnought: { role: "The fleet screen — a PD fit screens the whole side at platform grade (interception ×1.30).", speed: 29, hull: 16000, atk: 12, def: 26, slots: 5, cap: "Line VII research · 28 fit pts" },
  titan: { role: "The flagship — broadly good at every weapon (×1.10), best at nothing; one per syndicate.", speed: 23, hull: 32000, atk: 24, def: 44, slots: 6, cap: "Line VIII research · 45 fit pts · singleton" },
};


// Advance the OUTBOUND order signal each frame (the only traveling signal). This
// is the ONLY client-side timing computation: interpolating outbound progress
// between server-provided times. No delay is computed from truth or a client c.
// The signal is dropped once the order reaches the ship — from then on the ship's
// reaction is seen directly on the map (no inbound/response animation).
export function updateSignals(): void {
  const estSimNow = liveSimTime();
  state.commandSignals = state.commandSignals.filter((s) => {
    if (estSimNow >= s.arrive) return false; // order has arrived — the comet is done
    const outSpan = s.arrive - s.depart;
    s.pOut = outSpan > 1e-3 ? (estSimNow - s.depart) / outSpan : 1;
    return true;
  });
}


export function clearJumpDepartureSelection(): void {
  jumpDepartureSelection.key = null;
  renderer.selectedJumpDepartureKey = null;
}


export function intentTargetLabel(intent: PendingIntent): string {
  if (intent.verb === "move" && intent.targetId) {
    if (intent.targetId === "hub") return "Market Hub";
    return state.galaxy?.systems.find((system) => system.id === intent.targetId)?.name ?? "target system";
  }
  if (intent.verb === "raid" || intent.verb === "attack" || intent.verb === "guard") {
    const target = state.ghosts.find((g) => g.id === intent.targetId);
    if (intent.verb === "guard") {
      return target ? `your ${shipKindLabel(target.kind)} fleet` : "friendly fleet";
    }
    return target ? `rival ${shipKindLabel(target.kind)}` : "rival contact";
  }
  if (intent.verb === "blockade" || intent.verb === "survey") {
    return state.galaxy?.systems.find((s) => s.id === intent.targetId)?.name ?? "target system";
  }
  if (intent.verb === "demolish") {
    const target = state.emplacements.find((e) => e.id === intent.targetId);
    return target ? `rival ${emplacementLabel(target.kind)}` : "rival structure";
  }
  return "this destination";
}


export function intentSummary(intent: PendingIntent): string {
  const ship = state.ghosts.find((g) => g.id === intent.shipId && g.own);
  const shipName = ship ? shipKindLabel(ship.kind) : "fleet";
  const batchCount = intent.shipIds?.length ?? 1;
  const formation = batchCount > 1 ? `${batchCount} fleets` : shipName;
  const signal = ship ? `${ship.age.toFixed(0)}s` : "?";
  const target = intentTargetLabel(intent);
  const moveEstimate = ship && intent.dest ? (() => {
    const distance = Math.hypot(intent.dest!.x - ship.pos.x, intent.dest!.y - ship.pos.y);
    const flight = distance / Math.max(1, fleetBaseSpeed(ship) * WARP_FACTOR);
    const eta = ship.age + flight;
    const fuel = estimatedFuelForLeg(ship, intent.dest!);
    const tank = ship.fuel == null ? "" : ` / ${fmt(ship.fuel)} aboard`;
    const warning = ship.fuel != null && fuel > ship.fuel + 1e-6 ? " · FUEL SHORT" : "";
    return `${fmt(distance)} su · ETA ~${fmtDur(eta)} (signal ${fmtDur(ship.age)} + flight ${fmtDur(flight)}) · fuel ~${fmt(fuel)}${tank}${warning}`;
  })() : "";
  let summary = "";
  switch (intent.verb) {
    case "move": summary = `Move ${formation} → ${target}${moveEstimate ? ` · primary: ${moveEstimate}` : ` · signal ~${signal}`}`; break;
    case "jump": {
      const spool = state.galaxy?.jump_spool_s ?? 10;
      const point = intent.dest ? orderPoint(intent.dest) : "destination";
      summary = `Jump ${shipName} → ${point} — ~${spool.toFixed(0)} s spool, then instant relocation · signal ~${signal}`;
      break;
    }
    case "raid": summary = `RAID ${target} — intercept and steal cargo · signal ~${signal}`; break;
    case "attack": summary = `ATTACK ${target} — full battle, destroys it · signal ~${signal}`; break;
    case "guard": summary = `GUARD ${target} — shadow and defend · signal ~${signal}`; break;
    case "blockade": summary = `Blockade ${target} — disrupt its logistics · signal ~${signal}`; break;
    case "demolish": summary = `DEMOLISH ${target} — hold station until it falls · signal ~${signal}`; break;
    case "survey": summary = `SURVEY ${target} — active sensing, ~${SURVEY_SECS_UI}s on-site · signal ~${signal}`; break;
  }
  const authority = (intent.verb === "raid" || intent.verb === "attack")
    && state.ghosts.find((g) => g.id === intent.targetId)?.tca;
  return authority
    ? `${summary} · Authority citation projects ${projectedBand(TCA_INCIDENT_LOSS_UI)}`
    : summary;
}


// Replace the tracked lifecycles from the View. The wire is a flat owner-only
// list; group it per fleet for panel/map lookup and preserve oldest-first order.
export function syncOrderLifecycles(list: PendingOrderView[], _simTime: number, st = state): void {
  const next = new Map<string, PendingOrderView[]>();
  for (const p of list) {
    const queue = next.get(p.fleet_id) ?? [];
    queue.push(p);
    next.set(p.fleet_id, queue);
  }
  for (const queue of next.values()) {
    queue.sort((a, b) => a.issued_at - b.issued_at || a.id - b.id);
  }
  st.pendingOrders = next;
  const lost = new Set(list.filter((order) => order.lost).map((order) => order.id));
  if (lost.size) {
    st.commandSignals = st.commandSignals.filter((signal) => !lost.has(signal.orderId));
  }
  if (st.selectedOrderId !== null && !list.some((p) => p.id === st.selectedOrderId)) {
    st.selectedOrderId = null;
  }
}


export function latestPendingOrder(fleetId: string): PendingOrderView | undefined {
  const queue = state.pendingOrders.get(fleetId);
  return queue?.[queue.length - 1];
}
 // Tunable: covers drive-drop and route variance.
export function orderEtaRange(responseAt: number, now: number): string {
  const delta = responseAt - now;
  if (delta <= 0) return "overdue · unconfirmed";
  const lo = Math.max(0, Math.ceil(delta * (1 - ORDER_ETA_FUDGE_LO)));
  const hi = Math.max(lo, Math.ceil(delta * (1 + ORDER_ETA_FUDGE_HI)));
  return `ETA range ~${lo}–${hi}s`;
}


export function orderObject(p: PendingOrderView): string {
  const target = p.target_id ? state.ghosts.find((g) => g.id === p.target_id) : undefined;
  const emplacement = p.target_id ? state.emplacements.find((e) => e.id === p.target_id) : undefined;
  switch (p.kind) {
    case "move": return `Move → ${p.dest ? orderPoint(p.dest) : "destination"}`;
    case "hold": return "Cancel course → hold position";
    case "jump": return `Jump → ${p.dest ? orderPoint(p.dest) : "destination"}`;
    case "raid": return `Raid → ${target ? `rival ${shipKindLabel(target.kind)}` : "rival contact"}`;
    case "attack": return "Attack → rival contact";
    case "construct": {
      const what = p.emplacement === "deep_space_sensor" ? "Deep Space Sensor" : "structure";
      return `Construct → ${what}`;
    }
    case "demolish": return `Demolish → ${emplacement ? label(emplacement.kind) : "rival structure"}`;
    case "blockade": return `Blockade → ${p.target_id ? systemName(p.target_id) : "rival system"}`;
    case "survey": return `Survey → ${p.target_id ? systemName(p.target_id) : "system"}`;
    case "guard": return `Guard → ${target ? `your ${shipKindLabel(target.kind)} fleet` : "friendly fleet"}`;
    case "recall": return "Recall → home";
    case "withdraw": return "Withdraw → home";
    case "load": return "Load cargo at dock";
    case "unload": return "Unload cargo at dock";
    case "haul": return p.target_id ? `Haul → ${systemName(p.target_id)}` : "Haul → Market Hub";
    case "configure": {
      const configuration = p.configuration;
      if (configuration?.kind === "transit") {
        return `Transit → ${configuration.mode === "full" ? "Full speed" : "Stealth"}`;
      }
      if (configuration?.kind === "posture") {
        return `Posture → ${label(configuration.posture)}`;
      }
      if (configuration?.kind === "engage_freight") {
        return `Authority freight → ${configuration.on ? "Engage" : "Ignore"}`;
      }
      return "Update fleet configuration";
    }
    case "refit": return "Refit formation";
    case "reorganize": return p.target_id ? "Merge formations" : "Split formation";
    case "assign": return "Assign fleet duty";
  }
}

export function loadBattleMarks(): void {
  try {
    const raw = localStorage.getItem(BATTLE_LS_KEY);
    if (!raw) return;
    const m = JSON.parse(raw) as { viewed?: number[]; dismissed?: number[] };
    state.battleViewed = new Set(m.viewed ?? []);
    state.battleDismissed = new Set(m.dismissed ?? []);
  } catch { /* corrupt marks → start clean */ }
}

export function saveBattleMarks(): void {
  // Prune to ids the server still retains (the list is capped, so this stays tiny).
  const live = new Set(state.battleReports.map((r) => r.id));
  const keep = (s: Set<number>) => [...s].filter((id) => live.has(id));
  localStorage.setItem(BATTLE_LS_KEY, JSON.stringify({ viewed: keep(state.battleViewed), dismissed: keep(state.battleDismissed) }));
}

// One-way COMMAND delay (§3): command-center → battle anchor, at light speed.
// The same math the order echo-lifecycle uses; null before the galaxy/CC arrive.
export function battleCommandDelay(b: BattleView): number | null {
  if (!state.commandCenter || !state.galaxy) return null;
  return Math.hypot(b.pos.x - state.commandCenter.x, b.pos.y - state.commandCenter.y) / state.galaxy.c;
}


// §emplacements: the player's currently selected fleet IF it has teeth — the
// client mirror of the sim's `is_combatant()` gate on demolition, read off the
// same per-hull attack weights the panel shows. A crane or a convoy is not a
// wrecking crew, so selecting one leaves a rival structure merely inspectable.
export function armedSelection(st = state): GhostView | undefined {
  const g = st.selectedShipId
    ? st.ghosts.find((x) => x.id === st.selectedShipId && x.own)
    : undefined;
  if (!g) return undefined;
  const armed = g.composition?.length
    ? g.composition.some((c) => (SHIP_STATS[c.kind]?.atk ?? 0) > 0)
    : (SHIP_STATS[g.kind]?.atk ?? 0) > 0;
  return armed ? g : undefined;
}


/// The most recent landing at a system that this viewer can see. Fog-safe by
/// construction: `state.groundRecords` only ever holds what the server sent us.
export function latestGroundRecordFor(system: string): GroundRecordView | undefined {
  let best: GroundRecordView | undefined;
  for (const r of state.groundRecords) {
    if (r.system !== system) continue;
    // A running landing always wins — it is the live thing.
    if (r.outcome === null) return r;
    if (!best || r.started_at > best.started_at) best = r;
  }
  return best;
}


export function siegeProgress(dyn: SystemStateView | undefined): { pct: number; left: number; ripe: boolean } | null {
  if (!dyn?.blockade || dyn.blockade.siege_since == null || !state.galaxy) return null;
  const total = state.galaxy.siege_secs || 1;
  const elapsed = Math.max(0, liveSimTime() - dyn.blockade.siege_since);
  return { pct: Math.min(100, (elapsed / total) * 100), left: Math.max(0, total - elapsed), ripe: elapsed >= total };
}


// The "all clear" line — the single most check-in-respecting sentence in the
// game: when nothing needs a decision, show the NEXT known timestamp that will.
export function nextDecisionLabel(): string {
  const now = liveSimTime();
  let at = Infinity, label = "";
  const consider = (t: number, l: string) => { if (t > now && t < at) { at = t; label = l; } };
  const owned = state.systems.filter((s) => s.owner === state.playerId);
  for (const s of owned) {
    for (const b of s.builds ?? []) consider(b.complete_time, `a build completes at ${systemName(s.id)}`);
    if (s.blockade?.siege_since != null && state.galaxy) consider(s.blockade.siege_since + state.galaxy.siege_secs, `the siege at ${systemName(s.id)} completes`);
  }
  for (const queue of state.pendingOrders.values()) {
    for (const p of queue) if (!p.lost) consider(p.response_at, "an order response is expected");
  }
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
