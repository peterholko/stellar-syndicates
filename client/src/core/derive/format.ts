// Shared format derivations extracted from the desktop shell.

import { formatId, type StandingTrigger, type TradeEvent } from "../../protocol";
import { liveSimTime, state } from "../../state";
import { icon, label } from "../../icons";
import { hashId } from "../../prng";
import { operationSystemName, systemName } from "./geo";

export const fmt = (n: number): string => Math.round(n).toLocaleString();
const NPC_HULL_ROOT = "/art/ship_sprites/npc-contractors";


// Observed price trend, derived ONLY from the client's own (light-delayed) price
// history — NOT a server "pressure" signal (the server exposes none; fabricating
// one would break the fog model). Dual color+glyph encoding reads without color.
export function trend(h: number[]): { glyph: string; tone: string } {
  if (!h || h.length < 4) return { glyph: "▬", tone: "tone-flat" };
  const ref = h[h.length - 4] || 1;
  const pct = (h[h.length - 1] - h[h.length - 4]) / Math.abs(ref);
  if (pct > 0.04) return { glyph: "▲▲", tone: "tone-up" };
  if (pct > 0.004) return { glyph: "▲", tone: "tone-up" };
  if (pct < -0.04) return { glyph: "▼▼", tone: "tone-down" };
  if (pct < -0.004) return { glyph: "▼", tone: "tone-down" };
  return { glyph: "▬", tone: "tone-flat" };
}


export function operationTitle(o: import("../../protocol").OperationView): string {
  if (o.briefing) return o.briefing.title;
  const k = o.kind;
  switch (k.kind) {
    case "pirate_bounty": return `Suppress ${operationSystemName(k.system)} enclave`;
    case "survey_expedition": return `Survey ${operationSystemName(k.system)}`;
    case "market_delivery": return `Deliver ${k.units} ${label(k.commodity)}`;
    case "rescue_salvage": return "Recover a distress site";
    case "convoy_escort": return "Escort an Authority freighter";
    case "freight_escort": return "Guard a market run";
    case "authority_enforcement": return `Authority enforcement · ${formatId(k.target)}`;
    case "strategic_control": return `Hold ${operationSystemName(k.system)} strategic node`;
    case "regional_mandate": return "Regional Authority mandate";
    case "syndicate_megaproject": return `Syndicate project · ${operationSystemName(k.system)}`;
  }
}


export function operationIcon(o: import("../../protocol").OperationView): string {
  switch (o.kind.kind) {
    case "freight_escort":
    case "convoy_escort": return icon("escort", "md", "Freighter escort");
    case "market_delivery": return icon("freightRoute", "md", "Market delivery");
    case "survey_expedition": return icon("planetUninhabitable", "md", "Survey expedition");
    case "strategic_control": return icon("roleOutpost", "md", "Strategic control");
    default: return icon("manifest", "md", "Operation contract");
  }
}

export function operationHullArt(o: import("../../protocol").OperationView): string | null {
  const pick = (names: string[]): string => names[hashId(o.id) % names.length];
  switch (o.kind.kind) {
    case "pirate_bounty":
      return `${NPC_HULL_ROOT}/${pick(["pirate_corsair.png", "pirate_boarding_raider.png"])}`;
    case "survey_expedition":
      return `${NPC_HULL_ROOT}/survey_vessel.png`;
    case "rescue_salvage":
      return `${NPC_HULL_ROOT}/${pick(["rescue_cutter.png", "salvage_tug.png", "salvage_carrier.png"])}`;
    case "market_delivery":
      return `${NPC_HULL_ROOT}/${o.kind.units >= 80 ? "salvage_carrier.png" : "contract_courier.png"}`;
    case "convoy_escort":
    case "freight_escort":
    case "authority_enforcement":
      return `${NPC_HULL_ROOT}/contract_escort.png`;
    default:
      return null;
  }
}


export function operationCopy(o: import("../../protocol").OperationView): string {
  if (o.briefing) return o.briefing.summary;
  const k = o.kind;
  switch (k.kind) {
    case "pirate_bounty": return `Tier ${k.tier} enclave. Destroy its base; confirmation follows the battle report.`;
    case "survey_expedition": return "Send a Scout, complete the on-site dwell, and wait for the survey report.";
    case "market_delivery": return "Physically deliver or sell this commodity at the Market Hub.";
    case "rescue_salvage": return `${k.units} ${label(k.commodity)} remain at the reported wreck position. A cargo fleet must recover them.`;
    case "convoy_escort": return "Assign a fleet and remain close when the protected freighter reaches its destination.";
    case "freight_escort": return "Guard your Freighter from home to the Market Hub.";
    case "authority_enforcement": return "Join the public response against a proscribed corporation.";
    case "strategic_control": return "Capture and continuously supply the node through the published hold interval.";
    case "regional_mandate": return "Survey, trade, and suppress piracy inside the region. Highest contribution wins at close.";
    case "syndicate_megaproject": {
      const good = k.stage === 0 ? "Alloys" : k.stage === 1 ? "Electronics" : "Machinery";
      return `Stage ${k.stage + 1}/3 · freight ${good} to the project system; arrivals are consumed and credited to their sender. The host may also commit its local stock.`;
    }
  }
}


export function operationReward(o: import("../../protocol").OperationView): string {
  const bits: string[] = [];
  if (o.reward.credits) bits.push(`${Math.round(o.reward.credits).toLocaleString()} cr`);
  if (o.reward.authority_standing) bits.push(`+${o.reward.authority_standing} standing`);
  if (o.reward.captain_xp) bits.push(`${o.reward.captain_xp} Captain XP`);
  if (o.reward.research_insight) bits.push(`${Math.round(o.reward.research_insight)} research`);
  return bits.join(" · ");
}


export function fmtEta(secs: number): string {
  if (!isFinite(secs) || secs <= 0) return "—";
  const h = secs / 3600;
  if (h < 1) return `${Math.max(1, Math.round(secs / 60))}m`;
  if (h < 48) return `${h.toFixed(1)}h`;
  return `${(h / 24).toFixed(1)}d`;
}

/** The campaign clock is simulation time. Keeping it next to the pacing
 * multiplier makes every ETA legible without pretending a 4x playtest minute
 * is a wall-clock minute. */
export function gameClock(simTime: number): string {
  if (!Number.isFinite(simTime)) return "—";
  const total = Math.max(0, Math.floor(simTime));
  const day = Math.floor(total / 86_400) + 1;
  const hh = String(Math.floor(total / 3_600) % 24).padStart(2, "0");
  const mm = String(Math.floor(total / 60) % 60).padStart(2, "0");
  const ss = String(total % 60).padStart(2, "0");
  return `D${day} ${hh}:${mm}:${ss}`;
}

/** Canonical served-age vocabulary for command surfaces. */
export function informationDelay(secs: number): string {
  return secs > 0.5 ? `Information delay ${Math.max(1, Math.round(secs))}s` : "Live report";
}

// A one-way delay mapped onto the player's wall-clock, to the second ("~14:32:10").
export function arrivalLocal(delaySecs: number): string {
  return new Date(Date.now() + delaySecs / Math.max(0.01, state.pacingScale) * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

/// The absolute wall-clock completion ("done 14:32" local) — the async-planning
/// detail: sim-time delta mapped onto the player's clock.
export function doneAtLocal(completeTime: number): string {
  const ms = Date.now() + Math.max(0, completeTime - liveSimTime()) / Math.max(0.01, state.pacingScale) * 1000;
  return new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

// A build duration for humans: seconds under 2 min, then minutes / hours / days
// (a capital keel is a season event — "8d" reads, "691200s" doesn't).
export function fmtBuildDur(secs: number): string {
  if (secs < 120) return `${Math.round(secs)}s`;
  if (secs < 7200) return `${Math.round(secs / 60)}m`;
  if (secs < 172800) return `${(secs / 3600).toFixed(secs < 36000 ? 1 : 0).replace(/\.0$/, "")}h`;
  return `${(secs / 86400).toFixed(1).replace(/\.0$/, "")}d`;
}


/// A short duration ("2m 10s") for the freight timetable.
export function fmtDur(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  return s >= 60 ? `${Math.floor(s / 60)}m ${s % 60}s` : `${s}s`;
}

export function triggerLabel(t: StandingTrigger): string {
  if (t.kind === "above_threshold") return `when stock ≥ ${t.threshold}`;
  if (t.kind === "percent_surplus") return `${t.percent}% of surplus over ${t.floor}`;
  return `keep dest ≥ ${t.target}`;
}


// --- Check-in loop (§16, Layer 3) — timeline digest + attention surfacing ----
// Presence buys AWARENESS, not advantage: when you check in, here's what became
// observable while you were away, and the decisions waiting for you. The timeline
// is server-composed (light-correct, buffered offline); the attention items are
// derived right here from the player's own View — no extra information, just a
// summary of what they can already see.
export function agoLabel(at: number): string {
  const d = Math.max(0, state.simTime - at);
  return d < 90 ? `${d.toFixed(0)}s ago` : `${(d / 60).toFixed(0)}m ago`;
}



/// §TCA: plain-language text for a soft-rejected order or booking. Every one of
/// these costs the player NOTHING — the wording says what to do instead.
export function rejectText(t: Extract<TradeEvent, { event: "Rejected" }>): string {
  const com = t.commodity;
  const where = t.system ? systemName(t.system) : null;
  switch (t.reason.reason) {
    case "insufficient_warehouse_stock":
      return `Your Market Warehouse holds ${t.reason.have} ${com} — ship goods in first (Authority freight, or one of your freighters).`;
    case "not_your_system":
      return `The Authority serves your own colonies only — ${where ?? "that system"} isn't yours.`;
    case "insufficient_system_stock":
      return `${where ?? "That system"} holds ${t.reason.have} ${com} — not enough to collect ${t.units}.`;
    case "cannot_afford_fee":
      return `Freight fee is ${fmt(t.reason.fee)} Cr — more than your treasury.`;
    case "destination_blockaded":
      return `The Authority reports ${where ?? "that system"} BLOCKADED and won't book freight there.`;
    case "fleet_unavailable":
      return `That fleet must be yours, idle, and out of a fight to handle cargo.`;
    case "out_of_logistics_range":
      return `That fleet is too far from the dock — bring it alongside first.`;
    case "no_cargo_room":
      return t.reason.capacity === 0
        ? `That fleet has no cargo hold — only freighters haul goods.`
        : `Not enough hold for ${t.units} ${com}: this fleet lifts ${t.reason.capacity} units.`;
    case "cargo_mismatch":
      return `This older server refused a mixed load. Unload before loading ${com}.`;
    case "charter_suspended":
      return `Your charter is SUSPENDED — the Authority takes no new freight. Freight already booked still completes; pay reinstatement, or haul it yourself.`;
    case "charter_revoked":
      return `Your charter is REVOKED — the Exchange is closed to you. Your warehouse is still yours to fetch from; pay reinstatement to trade again.`;
    case "cant_afford":
      return t.units === 0
        ? `Reinstatement costs ${fmt(t.reason.cost)} credits — more than your treasury.`
        : `This purchase needs ${fmt(t.reason.cost)} credits including penalties — more than your treasury.`;
    case "price_protection":
      return `Price protection stopped the order: your bound was ${t.reason.bound.toFixed(2)}, but the true average price was ${t.reason.actual.toFixed(2)}. Nothing traded.`;
    case "market_liquidity":
      return `The Global Market can clear only ${t.reason.available} units immediately. Nothing traded — reduce the lot or place a limit order.`;
  }
}
