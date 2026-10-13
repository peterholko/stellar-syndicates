// Blockades as players read them (§contestable-territory). One source for every
// surface: the galaxy map, the System View, the system panel, the inbox and the
// mobile sheets all say the same thing about the same blockade.

import type { DiplomacyView, GhostView, SystemInfo, SystemStateView, Vec2 } from "../../protocol";
import { fmtDur } from "./format";

export type BlockadeState = NonNullable<SystemStateView["blockade"]>;

/** CONTESTED while the system's defense still fights (only a battle: shipping
 * runs), HELD once it is beaten (logistics cut, stores stripped), a SIEGE while
 * a held blockade's capture clock runs (never at a home). */
export type BlockadeStage = "contested" | "held" | "siege";

export function blockadeStage(b: BlockadeState): BlockadeStage {
  if (b.held_since == null) return "contested";
  return b.siege_since != null ? "siege" : "held";
}

/** The capture clock this blockade needs (its garrison and a battleship on
 * station included), else the galaxy's base. */
export function siegeSecs(b: BlockadeState, baseSecs: number): number {
  return b.siege_secs ?? baseSecs;
}

export interface SiegeClock {
  /** 0..100. */
  pct: number;
  left: number;
  ripe: boolean;
  /** Sim-time it ripens. */
  eta: number;
}

export function siegeClock(b: BlockadeState | null | undefined, now: number, baseSecs: number): SiegeClock | null {
  if (!b || b.siege_since == null) return null;
  const total = Math.max(1, siegeSecs(b, baseSecs));
  const elapsed = Math.max(0, now - b.siege_since);
  return { pct: Math.min(100, (elapsed / total) * 100), left: Math.max(0, total - elapsed), ripe: elapsed >= total, eta: b.siege_since + total };
}

/** A short badge for the map and headlines. */
export function blockadeBadge(b: BlockadeState, now: number, baseSecs: number): string {
  const stage = blockadeStage(b);
  if (stage === "contested") return b.by_me ? "CONTESTING" : "CONTESTED";
  if (stage === "held") return b.by_me ? "BLOCKADING" : "BLOCKADED";
  return `SIEGE ${Math.floor(siegeClock(b, now, baseSecs)?.pct ?? 0)}%`;
}

const pct = (p: number): string => `${Math.round(p * 100)}%`;

/** How often a held blockade runs down a freighter trying to slip past:
 * "caught 66% · 51% with a Warp Stabilizer". Empty while contested (nothing is
 * cut off yet, so there is nothing to run). */
export function blockadeOdds(b: BlockadeState): string {
  if (blockadeStage(b) === "contested" || b.catch == null) return "";
  const stabilized = b.catch_stabilized ?? b.catch;
  return stabilized < b.catch - 0.005
    ? `caught ${pct(b.catch)} · ${pct(stabilized)} with a Warp Stabilizer`
    : `caught ${pct(b.catch)}`;
}

export interface BlockadeBrief {
  tone: "warn" | "bad" | "good";
  title: string;
  /** What the stage means for this viewer and what to do about it. */
  detail: string;
}

/** `marinesNeeded`: the ground readout's break-even landing, when served. */
export function blockadeBrief(b: BlockadeState, now: number, baseSecs: number, marinesNeeded?: number | null): BlockadeBrief {
  const who = b.by_name || "A rival";
  const stage = blockadeStage(b);
  const clock = siegeClock(b, now, baseSecs);
  const marines = marinesNeeded ? ` (about ${marinesNeeded} marines for an even chance)` : "";
  const odds = blockadeOdds(b);
  if (b.by_me) {
    if (stage === "contested") {
      return { tone: "warn", title: "Contesting the defenses",
        detail: "Your fleet is fighting the system's defenses. Beat them to blockade it; if your fleet retreats, the blockade is off." };
    }
    if (stage === "held") {
      return { tone: "good", title: "Blockade holds",
        detail: "Its logistics are cut and its stores are yours while you hold station."
          + (odds ? ` Freighters running it: ${odds}; interceptors on station tighten it.` : "")
          + (b.capturable ? "" : " It is a home system: it can be blockaded, never captured.") };
    }
    return clock?.ripe
      ? { tone: "good", title: "Siege ripe",
        detail: `Land marines now with a separate, idle troop-transport fleet at the star${marines}.` }
      : { tone: "good", title: `Siege ${Math.floor(clock?.pct ?? 0)}%`,
        detail: `Ripe in ${fmtDur(clock?.left ?? 0)} while the blockade holds. Then land marines with a separate, idle troop-transport fleet at the star${marines}.` };
  }
  if (stage === "contested") {
    return { tone: "warn", title: `${who} is attacking`,
      detail: "Your defenses are fighting a rival fleet. Shipping runs while they hold; if they fall, the system is blockaded." };
  }
  if (stage === "held") {
    return { tone: "bad", title: `Blockaded by ${who}`,
      detail: "Its logistics are cut off: deliveries wait outside and its stores are being stripped. Send warships, or raise the system's station a level: its new hardpoints contest the blockade."
        + (odds ? ` A freighter set to run blockades tries to slip through: ${odds}.` : "")
        + (b.capturable ? "" : " Your home can't be captured.") };
  }
  return clock?.ripe
    ? { tone: "bad", title: `Besieged by ${who} · ripe`,
      detail: "Rival marines can land and take the system now. Send warships, or raise the system's station a level, to contest the blockade." }
    : { tone: "bad", title: `Besieged by ${who}`,
      detail: `Falls to a marine landing in ${fmtDur(clock?.left ?? 0)} unless you contest the blockade: send warships, or raise the system's station a level.` };
}

/** An own fleet that can blockade: it carries an interceptor. */
export function blockadeCapable(g: GhostView): boolean {
  return g.own && (g.composition ?? []).some((stack) => stack.kind === "raider" && stack.count > 0);
}

/** Why `fleet` can't blockade a system held by `owner` (empty: it can). Mirrors
 * the sim's checks from the viewer's arrived diplomacy; the sim checks again
 * when the order arrives, and every tick after. */
export function blockadeOrderBlock(fleet: GhostView, owner: string | null | undefined, me: string,
  diplomacy: DiplomacyView | null, now: number): string {
  if (!fleet.own) return "Choose one of your fleets.";
  if (!blockadeCapable(fleet)) return "A blockade needs an interceptor in the fleet to run down what slips past it.";
  if (!owner || owner === me) return "Only a system another corporation owns can be blockaded.";
  const rel = diplomacy?.relations.find((relation) => relation.other === owner);
  if (!rel) return "";
  if (rel.separation_until > now || rel.state === "non_aggression" || rel.state === "ceasefire") {
    const pact = rel.state === "non_aggression" ? "non-aggression pact" : rel.state === "ceasefire" ? "ceasefire" : "separation window";
    return `Your ${pact} with ${rel.name} protects their systems.`;
  }
  if (rel.state !== "war" && !((rel.reprisal_until ?? 0) > now)) {
    return `Blockading ${rel.name}'s systems needs a declared war or your right of reprisal.`;
  }
  return "";
}

/** Where a fleet on a Blockade order is: "Blockading Vidar · contested", or
 * "Sailing to blockade" before it takes station (the served report places it
 * within platform cover of a system it blockades). */
export function blockadeStatus(g: GhostView, view: {
  systems: SystemStateView[];
  galaxy: { systems: SystemInfo[]; siege_secs: number } | null;
  simTime: number;
}): string {
  const near = (pos: Vec2) => Math.hypot(pos.x - g.pos.x, pos.y - g.pos.y) <= 1_500;
  for (const system of view.systems) {
    const blockade = system.blockade;
    const info = view.galaxy?.systems.find((entry) => entry.id === system.id);
    if (!blockade?.by_me || !info || !near(info.pos)) continue;
    const stage = blockadeStage(blockade);
    const tail = stage === "contested" ? "contested" : stage === "held" ? "held"
      : `siege ${Math.floor(siegeClock(blockade, view.simTime, view.galaxy?.siege_secs ?? 0)?.pct ?? 0)}%`;
    return `Blockading ${info.name} · ${tail}`;
  }
  return "Sailing to blockade";
}

/** A delivery waiting outside a held blockade: "Waiting outside the blockade
 * of Vidar · running it: caught 66%". Empty when it isn't waiting. */
export function heldOutsideStatus(g: GhostView, view: {
  systems: SystemStateView[];
  galaxy: { systems: SystemInfo[] } | null;
}): string {
  if (!g.held_outside) return "";
  const name = view.galaxy?.systems.find((entry) => entry.id === g.held_outside)?.name ?? "its destination";
  const blockade = view.systems.find((system) => system.id === g.held_outside)?.blockade;
  const odds = blockade ? blockadeOdds(blockade) : "";
  return `Waiting outside the blockade of ${name}${odds ? ` · running it: ${odds}` : ""}`;
}

/** The run-blockades policy as an own freighter fleet's control shows it. */
export function runBlockadesCopy(on: boolean): { label: string; hint: string } {
  return on
    ? { label: "Running held blockades",
      hint: "Deliveries sail through a held blockade and may be caught and raided. Turn off to wait outside until it lifts." }
    : { label: "Waiting held blockades out",
      hint: "Deliveries wait outside a held blockade until it lifts. Turn on to try to slip through: interceptors on station catch runners; a Warp Stabilizer and Blockade Runners research help." };
}
