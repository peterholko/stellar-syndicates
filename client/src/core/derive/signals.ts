// §signals: the Signals pop-out's derivations (docs/signals.md). Signals are
// faint private contacts near home; a captained fleet answers one, sweeps it
// for 30 s and turns for home; the report brings what it found.
import type {
  CaptainRosterView, EntityId, GhostView, SignalFind, SignalKind, SignalView, SignalsView, Vec2,
} from "../../protocol";
import { label } from "../../icons";
import { fleetBaseSpeed, shipKindLabel, transitSpeedMultiplier, WARP_FACTOR } from "./fleet";
import { TRADERS, type Trader } from "./trader";

/** Mirrors sim::signals. */
export const SIGNAL_SWEEP_S = 30;
export const FIRST_TEN = 10;

export interface SignalInfo {
  name: string;
  slug: string;
  blurb: string;
  mayHold: string[];
  /** A 32×32 line drawing: the card until its painting loads. */
  icon: string;
}

export const SIGNAL_INFO: Record<SignalKind, SignalInfo> = {
  drifting_wreck: { name: "Drifting wreck", slug: "drifting-wreck", blurb: "A dead freighter, drifting in pieces.",
    mayHold: ["Alloys, Machinery", "Module"],
    icon: `<path d="M4 17 L11 11 H17 M20 11 H23 L28 16 L23 21 H11 Z"/><path d="M14 11 L16 7 M19 21 L21 25"/>` },
  cargo_pod: { name: "Cargo pod", slug: "cargo-pod", blurb: "A lost container, tumbling.",
    mayHold: ["Polymers, Provisions or Semiconductors"],
    icon: `<rect x="7" y="9" width="18" height="14" rx="2"/><path d="M7 14 H25 M13 9 V23 M19 9 V23"/>` },
  ore_fragment: { name: "Ore fragment", slug: "ore-fragment", blurb: "A broken-off rock rich in one ore.",
    mayHold: ["Ore for a Freighter"],
    icon: `<path d="M7 20 L11 9 L21 7 L27 15 L22 25 L12 25 Z"/><path d="M14 14 L19 18 M17 11 L20 13"/>` },
  silent_probe: { name: "Silent probe", slug: "silent-probe", blurb: "Someone's old survey probe.",
    mayHold: ["Survey Office credits"],
    icon: `<circle cx="16" cy="17" r="4"/><path d="M16 13 V5 M12 17 H5 M20 17 H27 M8 9 L13 14"/>` },
  distress_beacon: { name: "Distress beacon", slug: "distress-beacon", blurb: "A crew in trouble.",
    mayHold: ["A specialist", "Credits", "Rarely a captain"],
    icon: `<circle cx="16" cy="18" r="2.5"/><path d="M11 13 A7 7 0 0 1 21 13 M8 10 A11 11 0 0 1 24 10"/><path d="M16 21 V27"/>` },
  sensor_ghost: { name: "Sensor ghost", slug: "sensor-ghost", blurb: "A reading that may be nothing.",
    mayHold: ["Often nothing", "Sometimes a research dossier"],
    icon: `<circle cx="16" cy="16" r="9" stroke-dasharray="3 3"/><circle cx="16" cy="16" r="1.5"/>` },
  authority_courier: { name: "Authority courier", slug: "authority-courier", blurb: "A lost Authority mail drone.",
    mayHold: ["Credits and standing"],
    icon: `<rect x="7" y="10" width="18" height="12" rx="1"/><path d="M7 10 L16 17 L25 10"/>` },
  abandoned_outpost: { name: "Abandoned outpost", slug: "abandoned-outpost", blurb: "A mothballed station, its reactor failing.",
    mayHold: ["Research dossier", "Semiconductors, Machinery", "Rarely a blueprint"],
    icon: `<rect x="12" y="12" width="8" height="8"/><path d="M4 16 H12 M20 16 H28 M6 11 V21 M26 11 V21 M16 12 V6"/>` },
  pirate_stash: { name: "Pirate stash", slug: "pirate-stash", blurb: "Hidden loot, booby-trapped.",
    mayHold: ["Credits, Armaments", "Weapon crate"],
    icon: `<rect x="7" y="13" width="18" height="11" rx="1"/><path d="M7 17 H25 M14 13 V10 H18 V13"/>` },
  ghost_ship: { name: "Ghost ship", slug: "ghost-ship", blurb: "A crewless warship hulk, half-alive.",
    mayHold: ["Module", "Rarely a blueprint"],
    icon: `<path d="M4 17 L11 11 H23 L28 16 L23 21 H11 Z"/><path d="M14 11 L16 7 M19 21 L21 25"/><path d="M15 15 L18 18 M18 15 L15 18"/>` },
  precursor_fragment: { name: "Precursor fragment", slug: "precursor-fragment", blurb: "A shard of the old installations.",
    mayHold: ["Large research dossier", "Precision Components", "Rarely a blueprint"],
    icon: `<path d="M16 4 L22 16 L16 28 L10 16 Z"/><path d="M16 4 V28"/>` },
  exotic_flare: { name: "Exotic flare", slug: "exotic-flare", blurb: "Spill from an exotic node.",
    mayHold: ["Exotic Matter or a Void crystal", "Big experience"],
    icon: `<circle cx="16" cy="16" r="3"/><path d="M16 5 V11 M16 21 V27 M5 16 H11 M21 16 H27 M9 9 L12 12 M20 20 L23 23 M23 9 L20 12 M12 20 L9 23"/>` },
  teos_lost_crate: { name: "Teo's lost crate", slug: "teos-lost-crate", blurb: "A crate stencilled “Long Odds”.",
    mayHold: ["Teo's goods", "A finder's fee"],
    icon: `<rect x="7" y="9" width="18" height="14" rx="2"/><path d="M7 14 H25 M16 9 V14"/>` },
};

/** §signal-report (Peter, Nov 11 2026: "when the player is visiting/
 * completing a signal operation, an event panel with some art is needed to
 * explain what happen in the operation"): what the sweep found, in a line or
 * two. `found` when it brought something home or left a cache, `empty` when
 * not, and `hurt` after either when the captain was hurt (named once, so
 * "the captain" there). {captain} is the captain, {ship} the hull, {trader}
 * the founder's free trader. */
export const SIGNAL_OUTCOME: Record<SignalKind, { found: string; empty: string; hurt: string }> = {
  drifting_wreck: { found: "{captain}'s {ship} cut into the wreck and hauled out what its dead crew left.",
    empty: "{captain}'s {ship} searched the wreck end to end. Scavengers got there first.",
    hurt: "A bulkhead gave way; the captain was hurt." },
  cargo_pod: { found: "{captain}'s {ship} matched the pod's tumble and cracked it open.",
    empty: "{captain}'s {ship} caught the pod. It was empty.",
    hurt: "Its clamps snapped back; the captain was hurt." },
  ore_fragment: { found: "{captain} sampled the fragment: rich ore, waiting to be mined.",
    empty: "{captain} sampled the fragment. Barren rock.",
    hurt: "The rock split under the drills; the captain was hurt." },
  silent_probe: { found: "{captain} pulled the probe's survey logs. The Survey Office pays for those.",
    empty: "{captain} found the probe dead, its memory wiped.",
    hurt: "Its reactor flared as they opened it; the captain was hurt." },
  distress_beacon: { found: "{captain}'s {ship} reached the crew in time and brought them aboard.",
    empty: "{captain} found the beacon still calling. No one was left to answer it.",
    hurt: "The wreck broke up around them; the captain was hurt getting them out." },
  sensor_ghost: { found: "The ghost was real: {captain} recovered a sealed data core.",
    empty: "{captain} swept the reading. Nothing there but static.",
    hurt: "Something out there fired once; the captain was hurt." },
  authority_courier: { found: "{captain} recovered the Authority's mail drone. It pays its finders, grudgingly.",
    empty: "{captain} found the drone gutted. Someone else read the mail.",
    hurt: "Its tamper charge went off; the captain was hurt." },
  abandoned_outpost: { found: "{captain}'s crew boarded the outpost before its reactor failed and stripped what they could.",
    empty: "{captain}'s crew boarded the outpost. Stripped bare long ago.",
    hurt: "The reactor scrammed as they left; the captain was hurt." },
  pirate_stash: { found: "{captain} found the stash behind its tripwires and took it.",
    empty: "{captain} found the stash already looted.",
    hurt: "The stash was rigged; the captain took the blast." },
  ghost_ship: { found: "{captain}'s crew boarded the hulk while its guns slept and salvaged what still worked.",
    empty: "{captain}'s crew boarded the hulk. Nothing aboard still worked.",
    hurt: "Its point defense woke; the captain was hurt getting out." },
  precursor_fragment: { found: "{captain} brought the shard home for study, its patterns intact.",
    empty: "{captain} found the shard inert. Whatever it held is gone.",
    hurt: "The shard pulsed at their touch; the captain was hurt." },
  exotic_flare: { found: "{captain}'s {ship} flew into the flare and skimmed the spill.",
    empty: "The flare guttered out as {captain} arrived.",
    hurt: "The flare surged; the captain was hurt." },
  teos_lost_crate: { found: "{captain} found {trader}'s crate, seal unbroken.",
    empty: "{captain} found {trader}'s crate, already pried open.",
    hurt: "It was rigged against thieves; the captain was hurt." },
};

/** The report's account: what happened, and the injury if there was one. */
export function signalStory(signal: SignalView, ship: string | null, trader: Trader = TRADERS.teo): string {
  const report = signal.report;
  if (!report) return "";
  const lines = SIGNAL_OUTCOME[signal.kind];
  const fill = (line: string) => line.replaceAll("{captain}", report.captain)
    .replaceAll("{ship}", ship ?? "ship").replaceAll("{trader}", trader.first);
  return [fill(report.finds.length ? lines.found : lines.empty), report.hurt_until != null ? fill(lines.hurt) : ""]
    .filter(Boolean).join(" ");
}

/** §traders: a signal's card text. The lost crate is the founder's own free
 * trader's, as the server names it. */
export function signalInfo(kind: SignalKind, trader: Trader = TRADERS.teo): SignalInfo {
  const info = SIGNAL_INFO[kind];
  return kind === "teos_lost_crate" ? { ...info, name: `${trader.first}'s lost crate`,
    blurb: `A crate stencilled “${trader.ship}”.`, mayHold: [`${trader.first}'s goods`, "A finder's fee"] } : info;
}

const ART = "/art/derived/signals/v1";
const AFTERMATH = "/art/derived/signals/aftermath-v1";

/** The card's painting: the zoomed card crop. */
export function signalCardArt(kind: SignalKind): { src: string; srcset: string } {
  const slug = SIGNAL_INFO[kind].slug;
  return { src: `${ART}/${slug}-card-384.webp`, srcset: `${ART}/${slug}-card-384.webp 384w, ${ART}/${slug}-card-640.webp 640w` };
}

/** The detail's and the report's painting: the full frame. */
export function signalFrameArt(kind: SignalKind): { src: string; srcset: string } {
  const slug = SIGNAL_INFO[kind].slug;
  return { src: `${ART}/${slug}-768.webp`, srcset: `${ART}/${slug}-768.webp 768w, ${ART}/${slug}-1280.webp 1280w` };
}

/** §signal-report: the report panel's painting: the aftermath
 * (art-src/signals-aftermath-v1), the card painting standing in until its
 * master lands. */
export function signalAftermathArt(kind: SignalKind): { src: string; srcset: string } {
  const slug = SIGNAL_INFO[kind].slug;
  return { src: `${AFTERMATH}/${slug}-768.webp`, srcset: `${AFTERMATH}/${slug}-768.webp 768w, ${AFTERMATH}/${slug}-1280.webp 1280w` };
}

/** Mirrors sim::signals::hurt_chance: 45% less 15 points per Fieldcraft
 * point, floor 5%. */
export function hurtChance(fieldcraft: number): number {
  return Math.max(0.05, 0.45 - 0.15 * fieldcraft);
}

/** "3h 12m", "42m", "0:42". */
export function signalDur(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  if (s < 60 * 10) return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
  return h ? `${h}h ${m}m` : `${m}m`;
}

export function ago(secs: number): string {
  const s = Math.max(0, secs);
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86_400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86_400)}d ago`;
}

const dist = (a: Vec2, b: Vec2): number => Math.hypot(a.x - b.x, a.y - b.y);

/** Own fleets answering a signal: sweeping, or on their way. */
export function answering(ghosts: readonly GhostView[], signal: EntityId): GhostView | undefined {
  return ghosts.find((g) => g.own && g.expedition?.task === "signal" && g.expedition.site === signal);
}

function warpSpeed(g: GhostView): number {
  return Math.max(1, fleetBaseSpeed(g) * WARP_FACTOR * transitSpeedMultiplier(g));
}

export type SignalStatus = "open" | "answering" | "reported";

export interface SignalCard {
  signal: SignalView;
  info: SignalInfo;
  status: SignalStatus;
  tag: string;
  line: string;
  /** 0..1 while answering. */
  progress: number | null;
}

/** The cards, in order: reports in, then those being answered, then open by
 * age (newest first). */
export function signalCards(view: SignalsView, ghosts: readonly GhostView[], home: Vec2 | null, now: number,
  trader: Trader = TRADERS.teo): SignalCard[] {
  const cards = view.signals.map((signal): SignalCard => {
    const info = signalInfo(signal.kind, trader);
    if (signal.report) {
      return { signal, info, status: "reported", tag: "Report in", line: `Explored · ${ago(now - signal.report.at)}`, progress: null };
    }
    const fleet = answering(ghosts, signal.id);
    const hazard = signal.hazardous ? "Hazardous" : "Routine";
    if (fleet) {
      const who = `${shipKindLabel(fleet.kind)}${fleet.captain ? ` · ${fleet.captain.name}` : ""}`;
      if (fleet.survey_progress != null) {
        return { signal, info, status: "answering", tag: hazard, line: `${who} · sweeping`, progress: fleet.survey_progress };
      }
      const eta = dist(fleet.pos, signal.pos) / warpSpeed(fleet);
      return { signal, info, status: "answering", tag: hazard, line: `${who} · ${signalDur(eta)}`, progress: null };
    }
    const away = home ? `${Math.round(dist(home, signal.pos)).toLocaleString()} su · ` : "";
    return { signal, info, status: "open", tag: hazard, line: `${away}${ago(now - signal.detected_at)}`, progress: null };
  });
  const rank: Record<SignalStatus, number> = { reported: 0, answering: 1, open: 2 };
  return cards.sort((a, b) => rank[a.status] - rank[b.status]
    || (a.status === "reported" ? b.signal.report!.at - a.signal.report!.at : b.signal.detected_at - a.signal.detected_at));
}

/** "3 open · next ~5h 40m", "5 open · full". */
export function signalsSummary(view: SignalsView, now: number): string {
  const open = view.signals.filter((s) => !s.report).length;
  const next = view.next_at == null ? "full" : view.next_at <= now ? "next soon" : `next ~${signalDur(view.next_at - now)}`;
  return `${open} open · ${next}`;
}

export function openSignals(view: SignalsView | null): number {
  return view ? view.signals.filter((s) => !s.report).length : 0;
}

/** The card picked by default: a fresh report, then the newest open one. */
export function defaultSignal(cards: readonly SignalCard[]): EntityId | null {
  return (cards.find((c) => c.status === "reported") ?? cards.find((c) => c.status === "open") ?? cards[0])?.signal.id ?? null;
}

export interface SendRow {
  fleet: GhostView | null;
  captain: CaptainRosterView;
  title: string;
  sub: string;
  /** "Injury 30%" on a Hazardous signal. */
  risk: string | null;
  trip: string | null;
  /** Why Send is off, or null. */
  blocked: string | null;
}

/** One row per captain: a captained fleet can go; a captain in reserve or
 * hurt can't. Ready rows first, nearest first. */
export function sendRows(
  signal: SignalView, captains: readonly CaptainRosterView[], ghosts: readonly GhostView[],
  home: Vec2 | null, now: number, placeName: (berth: string) => string,
): SendRow[] {
  const rows: (SendRow & { order: number })[] = [];
  for (const captain of captains) {
    if (captain.loss_fate === "killed" || captain.recovering_until != null && captain.recovering_until > now) continue;
    const fleet = captain.assigned_fleet ? ghosts.find((g) => g.own && g.id === captain.assigned_fleet) ?? null : null;
    const report = fleet?.captain ?? captain.report;
    const level = report?.level ?? 1;
    const fieldcraft = report?.attributes.fieldcraft ?? 1;
    const hurt = captain.hurt_until != null && captain.hurt_until > now ? captain.hurt_until : null;
    const risk = signal.hazardous ? `Injury ${Math.round(hurtChance(fieldcraft) * 100)}%` : null;
    if (!fleet) {
      rows.push({ fleet: null, captain, title: `No ship · ${captain.name}`, sub: `Lv ${level} · Fieldcraft ${fieldcraft} · in reserve`,
        risk: null, trip: null, blocked: hurt ? `Recovering · ${signalDur(hurt - now)}` : "Give a ship in Officers", order: 3 });
      continue;
    }
    if (fleet.kind === "mining_belt_station") continue;
    const where = fleet.docked ? `docked at ${placeName(fleet.docked)}` : "under way";
    const back = home ?? fleet.pos;
    const trip = (dist(fleet.pos, signal.pos) + dist(signal.pos, back)) / warpSpeed(fleet) + SIGNAL_SWEEP_S;
    const here = answering(ghosts, signal.id)?.id === fleet.id;
    const busy = fleet.expedition?.task === "signal" && !here;
    const blocked = hurt ? `Recovering · ${signalDur(hurt - now)}`
      : here ? "On its way"
      : busy ? "On another signal"
      : fleet.stalled ? "Out of fuel"
      : fleet.supplied === false ? "Unsupplied"
      : null;
    rows.push({
      fleet, captain, title: `${shipKindLabel(fleet.kind)} · ${captain.name}`,
      sub: `Lv ${level} · Fieldcraft ${fieldcraft} · ${where}`, risk, trip: `~${signalDur(trip)} trip`,
      blocked, order: blocked ? 2 : trip,
    });
  }
  // Ready rows by trip time; blocked ones after them.
  return rows.sort((a, b) => (a.blocked ? 1 : 0) - (b.blocked ? 1 : 0) || a.order - b.order)
    .map(({ order: _order, ...row }) => row);
}

export interface FindRow {
  label: string;
  value: string;
  tone?: "good" | "warn";
}

/** What a report found, row by row, then experience and any injury. */
export function findRows(signal: SignalView, ship: string | null, now: number, programmeName: (id: string) => string): FindRow[] {
  const report = signal.report;
  if (!report) return [];
  const goods = (list: [string, number][]) => list.map(([c, n]) => `${n} ${label(c)}`).join(", ");
  const aboard = ship ? `aboard the ${ship}` : "aboard";
  const rows: FindRow[] = report.finds.map((find: SignalFind): FindRow => {
    switch (find.kind) {
      case "credits": return { label: "Credits", value: `+${Math.round(find.credits).toLocaleString()} Cr`, tone: "good" };
      case "standing": return { label: "Authority fee · standing", value: `+${Math.round(find.credits).toLocaleString()} Cr · +${find.standing}`, tone: "good" };
      case "cargo_aboard": return { label: goods(find.goods), value: aboard };
      case "cache": return { label: goods(find.goods), value: "left for a Freighter" };
      case "module_aboard": return { label: label(find.module), value: aboard };
      case "dossier": return { label: "Research dossier", value: programmeName(find.programme) };
      case "blueprint": return { label: `${label(find.module)} blueprint`, value: "licensed", tone: "good" };
      case "captain": return { label: `Captain ${find.name}`, value: "in reserve at home", tone: "good" };
      case "specialist": return { label: label(find.specialist), value: "flying home" };
    }
  });
  if (!report.finds.length) rows.push({ label: "Nothing of value", value: "—" });
  rows.push({ label: `Experience · ${report.captain}`, value: `+${report.xp}`, tone: "good" });
  if (report.hurt_until != null) {
    rows.push({ label: `${report.captain} hurt`, value: report.hurt_until > now ? `back in ${signalDur(report.hurt_until - now)}` : "recovered", tone: "warn" });
  }
  return rows;
}

/** The first ten's strip: reports in, and what the next holds. */
export function firstTenStrip(view: SignalsView): { done: number; next: string | null } | null {
  if (view.first_ten >= FIRST_TEN) return null;
  return { done: view.first_ten, next: view.first_ten_next ?? null };
}
