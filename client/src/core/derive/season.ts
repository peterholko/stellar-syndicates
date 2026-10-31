// §season (docs/endgame-keystone.md): the Authority's grip, its decrees and
// the Galactic News Network, as this corporation has heard them.

import type { BearingView, CharterView, Commodity, CrackdownKind, CrackdownView, Demand, EntityId, Grip, NewsKind, NewsSource, NewsView, SeasonView, Vec2 } from "../../protocol";
import { ageLabel } from "./format";

export const GRIPS: readonly Grip[] = ["charter", "levies", "inspections", "martial"];

const NUMERALS = ["I", "II", "III", "IV"] as const;

export const gripNumeral = (grip: Grip): string => NUMERALS[GRIPS.indexOf(grip)] ?? "I";

export function gripTitle(grip: Grip): string {
  return { charter: "Charter", levies: "Levies", inspections: "Inspections", martial: "Martial Charter" }[grip];
}

/** What each decree imposes, for its pop-up. */
export const DECREE_TEXT: Record<Grip, string> = {
  charter: "The charters stand as granted.",
  levies: "A levy on every Hub sale and purchase. Requisitions begin.",
  inspections: "Colony licence fees. Cargo inspections at the Hub. A registry tax on warships.",
  martial: "Garrisons in the Hub district. Syndicate dues. Requisitions enforced by Authority fleets.",
};

/** The grip a decree's log line announces ("Authority decree · Grip II · Levies."). */
export function decreeOf(text: string): Grip | null {
  const match = /^Authority decree · Grip (I|II|III|IV) · .+\.$/.exec(text);
  if (!match) return null;
  return GRIPS[NUMERALS.indexOf(match[1] as typeof NUMERALS[number])] ?? null;
}

export function newsKindLabel(kind: NewsKind): string {
  return {
    decree: "Decree", crackdown: "Crackdown", wanted: "Wanted", war: "War", keystone: "Keystone", battle: "Battle",
    capture: "Capture", relic: "Relic", market: "Market", medal: "Medal", corporation: "Charter", retraction: "Retraction",
    resolve: "Resolve",
  }[kind];
}

/** The feed's names and emblems (art-src/gnn-v1): 48 px for the ticker and
 * News rows, 128 for a story pop-up's header. */
export const newsSourceName = (source: NewsSource): string => source === "free_signal" ? "Free Signal" : "GNN";
export const newsEmblem = (source: NewsSource, size: 48 | 128 = 48): string =>
  `/art/derived/gnn-v1/${source === "free_signal" ? "free-signal" : "gnn"}-emblem-${size}.webp`;

/** The ticker's headline: the newest item heard. */
export const tickerItem = (season: SeasonView | null): NewsView | null => season?.news[0] ?? null;

/** News heard since `since` (sim time), for the News tab's count. */
export const freshNews = (season: SeasonView | null, since: number): number =>
  season?.news.filter((item) => item.heard_at > since).length ?? 0;

/** One News tab row. */
export interface NewsRow {
  id: number;
  /** "GNN" or "Free Signal". */
  source: string;
  /** Its emblem, 48 px. */
  emblem: string;
  free: boolean;
  headline: string;
  /** Kind · age. */
  meta: string;
  tone: "warn" | "good" | "info";
  yours: boolean;
  unread: boolean;
  system?: EntityId;
  fleet?: EntityId;
}

const WARN_KINDS: ReadonlySet<NewsKind> = new Set(["decree", "crackdown", "wanted", "war"]);

export function newsRows(season: SeasonView | null, now: number, readThrough: number | null): NewsRow[] {
  return (season?.news ?? []).map((item) => {
    const free = item.source === "free_signal";
    const since = Math.max(0, now - item.heard_at);
    return {
      id: item.id,
      source: newsSourceName(item.source),
      emblem: newsEmblem(item.source),
      free,
      headline: item.headline,
      meta: `${newsKindLabel(item.kind)} · ${since < 60 ? "now" : `${ageLabel(since)} ago`}`,
      tone: free || item.kind === "retraction" ? "good" : WARN_KINDS.has(item.kind) ? "warn" : "info",
      yours: item.about_you,
      unread: readThrough !== null && item.heard_at > readThrough,
      system: item.system,
      fleet: item.fleet,
    };
  });
}

/** The News tab's head: the grip in force and the galaxy's maturity. */
export const seasonLine = (season: SeasonView | null): string | null => season
  ? `Grip ${gripNumeral(season.grip)} · ${gripTitle(season.grip)} · Galaxy maturity ${Math.round(season.maturity * 100)}%`
  : null;

/** The newest item's arrival, for marking the tab read. */
export const latestNews = (season: SeasonView | null): number => season?.news[0]?.heard_at ?? 0;

/** §season: the Hub's cut of a trade (a fraction): the Authority's levy and
 * any charter penalty. */
export const hubCut = (charter: CharterView | null | undefined): number =>
  (charter?.market_penalty_frac ?? 0) + (charter?.levy_frac ?? 0);

/** §season: the good is embargoed for you at the Hub. */
export const isEmbargoed = (charter: CharterView | null | undefined, commodity: Commodity): boolean =>
  !!charter?.squeeze?.embargoes.some((e) => e.commodity === commodity);

/** A requisition's demand, e.g. "300 Alloys" or "4,000 Cr" (`label` names goods). */
export function demandText(demand: Demand, label: (c: Commodity) => string): string {
  return demand.kind === "goods" ? `${demand.units.toLocaleString()} ${label(demand.commodity)}` : `${Math.round(demand.amount).toLocaleString()} Cr`;
}

export function crackdownTitle(kind: CrackdownKind): string {
  return { sabotage: "Sabotage at the Hub", massacre: "Convoy massacre", quarantine: "Quarantine", relic_seizure: "Relic seizure",
    keystone: "Keystone crackdown" }[kind];
}

/** What a crackdown's pop-up says. */
export const CRACKDOWN_TEXT: Record<CrackdownKind, string> = {
  sabotage: "Insurgents blamed. Inspections and doubled Hub fees for 12h. A Scout at the Hub may find who did it.",
  massacre: "The Authority names you for a destroyed convoy: a bounty and an enforcement squadron. A Scout at the wreck may prove it false.",
  quarantine: "Authority freight to your system is suspended for 12h.",
  relic_seizure: "The Authority demands your relics.",
  keystone: "The Keystone draws the Authority: enforcement squadrons move on your syndicate for 12h.",
};

/** The crackdown a log line announces ("Crackdown · Sabotage at the Hub · …"). */
export function crackdownOf(text: string): CrackdownKind | null {
  const match = /^Crackdown · (Sabotage at the Hub|Convoy massacre|Quarantine|Relic seizure|Keystone crackdown)\b/.exec(text);
  if (!match) return null;
  return ({ "Sabotage at the Hub": "sabotage", "Convoy massacre": "massacre", "Quarantine": "quarantine", "Relic seizure": "relic_seizure",
    "Keystone crackdown": "keystone" } as const)[match[1] as "Quarantine"];
}

/** Evidence a Scout can still read: open, unexposed and not yet read by you. */
export const openEvidence = (season: SeasonView | null, now: number): CrackdownView[] =>
  (season?.crackdowns ?? []).filter((c) => c.evidence_until > now && !c.exposed && !c.read);

/** Resolve's band: loyalist, uncommitted, or set on freedom (relics answer). */
export function resolveBand(resolve: number): "loyal" | "restless" | "rebel" {
  return resolve < 0.2 ? "loyal" : resolve < 0.5 ? "restless" : "rebel";
}

export const RESOLVE_BAND = { loyal: "loyal", restless: "restless", rebel: "set on freedom" } as const;
export const RESOLVE_TIP = "Rises with every levy, requisition and crackdown, and in Assembly Halls; fades 3% a day. "
  + "Below 20%: loyal, a lighter levy. 50%: relics answer you, +5% production. 100%: +10%.";

/** A bearing's wedge holds `pos` (the sim's `Bearing::holds`). */
export function bearingHolds(b: BearingView, pos: Vec2): boolean {
  const dx = pos.x - b.from.x, dy = pos.y - b.from.y;
  if (Math.hypot(dx, dy) < 1) return true;
  let off = Math.abs(Math.atan2(dy, dx) - b.angle) % (2 * Math.PI);
  if (off > Math.PI) off = 2 * Math.PI - off;
  return off <= b.spread;
}

/** The search zone: systems inside two or more bearings' wedges, unless one
 * bearing is all there is (then its wedge). */
export function searchZone(bearings: BearingView[], systems: { id: EntityId; pos: Vec2 }[]): EntityId[] {
  if (bearings.length < 2) return [];
  return systems.filter((sys) => bearings.filter((b) => bearingHolds(b, sys.pos)).length >= 2).map((sys) => sys.id);
}

/** One step of the relic journey, for the Inbox. */
export interface JourneyStep {
  key: string;
  title: string;
  facts: string;
  system?: EntityId;
}

/** The relic journey's next steps (docs/endgame-keystone.md): attune, cross
 * bearings, survey the zone, clear and land. `name` names systems. */
export function journeySteps(season: SeasonView | null, resolve: number, systems: { id: EntityId; pos: Vec2 }[],
  name: (id: EntityId) => string, now: number, attuneSecs: number): JourneyStep[] {
  if (!season) return [];
  const steps: JourneyStep[] = [];
  for (const relic of season.relics) {
    if (relic.resonance === undefined || relic.drawn) continue;
    if (resolve < 0.5) {
      steps.push({ key: `relic:${relic.system}`, title: `Relic · ${name(relic.system)}`, facts: "attuning needs Resolve 50%", system: relic.system });
      continue;
    }
    const left = Math.max(0, (1 - relic.resonance) * attuneSecs);
    steps.push({ key: `relic:${relic.system}`, title: `Attune ${name(relic.system)}`, facts: `${Math.round(relic.resonance * 100)}% · ~${Math.ceil(left / 3600)}h left`, system: relic.system });
  }
  const zone = searchZone(season.bearings, systems);
  const found = new Set(season.precursors.map((p) => p.system));
  if (season.bearings.length === 1) steps.push({ key: "bearings:1", title: "One more bearing crosses it", facts: "attune a second relic" });
  else if (zone.length) steps.push({ key: `zone:${zone.length}`, title: `Survey the zone · ${zone.length} systems`, facts: "arrive with a bearing through it", system: zone.find((id) => !found.has(id)) ?? zone[0] });
  for (const p of season.precursors) {
    if (p.landed_by) continue;
    steps.push({ key: `precursor:${p.system}`, title: `Sealed world · ${name(p.system)}`, facts: p.guarded ? "clear the guardians, then land a colony ship" : "land a colony ship", system: p.system });
  }
  if (season.war_at && season.war_at > now) steps.push({ key: "leak", title: "An Authority spy", facts: `war in ${Math.ceil((season.war_at - now) / 60)}m · bring freight home` });
  return steps;
}

/** Attuning's length at `maturity` (the sim's `season::attune_secs`). */
export function attuneSecs(maturity: number): number {
  const t = Math.min(1, Math.max(0, (maturity - 0.3) / 0.7));
  return 24 * 3600 + (4 * 3600 - 24 * 3600) * t;
}

/** §season phase 5: what the Keystone world can feed now, per good:
 * [good, needed, paid, in its stockpile]. */
export function keystoneFeed(season: SeasonView | null, stock: Partial<Record<Commodity, number>>): [Commodity, number, number, number][] {
  return (season?.keystone?.basket ?? []).map(([good, need, paid]) => [good, need, paid, Math.floor(stock[good] ?? 0)]);
}

/** The ending's headline for its pop-up. */
export function endingText(ending: NonNullable<SeasonView["ending"]>): { title: string; text: string } {
  if (ending.kind === "sovereign") {
    return { title: "The Seal fires", text: `Every gate closes and the Authority is cut off. ${ending.holder ?? "A syndicate"} rules the galaxy as Sovereign.${ending.won ? " You stand with it." : " Your corporation is recorded as subjugated."}` };
  }
  return { title: "The Charter holds", text: `No Seal fired. The Authority keeps the galaxy.${ending.winners.length ? ` Honoured: ${ending.winners.slice(0, 6).join(", ")}.` : ""}${ending.won ? " You are among them." : ""}` };
}
