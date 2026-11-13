// §merchant persona (Oct 16 2026, Peter's pick): Teo Brannick of the Long
// Odds, a big, loud, theatrical haggler. §traders (Peter, Oct 27 2026:
// "shouldn't we have like 3 different merchant characters at least, that way
// not all the players see the same merchant"): two more, Ines Varga of the
// Clean Slate, a cool, exact former Authority customs auditor, and Mags
// Halloran of the Second Wind, a warm, unhurried old hauler shrewder than she
// looks. Peter, the same day: "can we have 4th male free trader": Rafi Mensah
// of the Small Fortune, a quick, cheeky young hustler. The server deals one
// per corporation (`FoundingView.trader`, sim merchant.rs). The prices are the sim's; only the talk is the trader's. Both
// shells' stalls show the portrait and the trader's line about the offer on
// the table.
import { traderShipArt, type FoundingView, type MerchantView, type TimelineEntry, type TraderSlug } from "../../protocol";
import type { MerchantGoods, MerchantQuote } from "./merchant";

/** How the trader takes the offer: the portrait's expression. */
export type TraderMood = "neutral" | "pleased" | "scoff";
export interface TraderLine { text: string; mood: TraderMood }

/** A trader's words, by moment; the refusals by the sim's reason (REFUSED). */
interface TraderVoice {
  arriving: string;
  leaving: string;
  again: string;
  greeting: string;
  both: string;
  /** On what you offer it (§sell-alloys, Nov 11 2026: Alloys). */
  sale: (price: string) => string;
  bigSpend: string;
  armaments: string;
  chips: string;
  goods: string;
  deal: string;
  credits: string;
  stock: string;
  cap: string;
  short: string;
  storage: string;
  charter: string;
  notMine: string;
  no: string;
}

export interface Trader {
  slug: TraderSlug;
  name: string;
  first: string;
  /** Its ship, without "the". */
  ship: string;
  /** Its portraits' stem under PORTRAITS. */
  portrait: string;
  /** One line on who they are: the fleet panel's (Oct 28 2026). */
  about: string;
  voice: TraderVoice;
}

export const TRADERS: Record<TraderSlug, Trader> = {
  teo: { slug: "teo", name: "Teo Brannick", first: "Teo", ship: "Long Odds", portrait: "teo-brannick",
    about: "Loud, theatrical haggler.", voice: {
    arriving: "Hold your horses, I'm still docking.",
    leaving: "Too late, friend. I've a schedule.",
    again: "Back for more? Good, good.",
    greeting: "Raided! Terrible. Lucky I came by, eh?",
    both: "Now we're talking!",
    sale: (price) => `Alloys? Still warm! ${price} a unit, and I'm being kind.`,
    bigSpend: "A big spender! I like you already.",
    armaments: "The pointy end of trade. Not cheap.",
    chips: "Chips! The genuine article.",
    goods: "Fine goods, fair-ish prices.",
    deal: "Done! You'll tell your grandchildren.",
    credits: "Credits, friend. Or Alloys. I take both.",
    stock: "Ha! I'd sell you the ship if I could.",
    cap: "I've a hold, not a moon.",
    short: "You haven't smelted that much yet.",
    storage: "And where would you put it all?",
    charter: "Your charter's in the red. I don't touch that.",
    notMine: "Not my line of goods.",
    no: "No deal.",
  } },
  ines: { slug: "ines", name: "Ines Varga", first: "Ines", ship: "Clean Slate", portrait: "ines-varga",
    about: "Former Authority customs auditor. Exact.", voice: {
    arriving: "Docking. One moment.",
    leaving: "My manifest is closed.",
    again: "Back again. Efficient.",
    greeting: "Raided. Unfortunate. I carry what they took.",
    both: "A two-way ledger. Good.",
    sale: (price) => `Alloys. ${price} a unit. That is the price.`,
    bigSpend: "A substantial order. Noted.",
    armaments: "Armaments. Certified, and priced accordingly.",
    chips: "Semiconductors. Genuine. I checked every one.",
    goods: "Standard goods, standard terms.",
    deal: "Done. It is in your stores.",
    credits: "The numbers don't close.",
    stock: "My stock is finite.",
    cap: "My hold has a limit. That is past it.",
    short: "You don't have those Alloys.",
    storage: "Your stores are full.",
    charter: "Your charter is in the red. Regulations.",
    notMine: "Not on my manifest.",
    no: "No.",
  } },
  mags: { slug: "mags", name: "Mags Halloran", first: "Mags", ship: "Second Wind", portrait: "mags-halloran",
    about: "Old hauler, shrewder than she looks.", voice: {
    arriving: "Slow ships get there too, pet. Nearly tied up.",
    leaving: "That's me away, pet. Mind how you go.",
    again: "Back again? You'll have me rich.",
    greeting: "Raided, were you? Oh, pet. Let's see what we can do.",
    both: "Now that's proper trading.",
    sale: (price) => `Alloys, is it? ${price} a unit. Fair's fair.`,
    bigSpend: "Well! Someone's had a good week.",
    armaments: "Guns, is it? Mind you use them well.",
    chips: "Chips. Real ones. I don't sell rubbish.",
    goods: "Good honest goods, pet.",
    deal: "Done and dusted. Off you go, pet.",
    credits: "Credits first, pet. Or Alloys.",
    stock: "I've not got that much, pet.",
    cap: "My old hold won't take that much.",
    short: "You've not smelted that yet, pet.",
    storage: "And where would you put it, pet?",
    charter: "Your charter's in the red. I can't, pet.",
    notMine: "That's not my line.",
    no: "Not today, pet.",
  } },
  rafi: { slug: "rafi", name: "Rafi Mensah", first: "Rafi", ship: "Small Fortune", portrait: "rafi-mensah",
    about: "Quick, cheeky young hustler.", voice: {
    arriving: "Two minutes, chief. Just parking.",
    leaving: "Gotta run, chief. Places to be.",
    again: "You again! My favourite customer.",
    greeting: "Raided? Rough. Good news: I've got exactly what they took.",
    both: "Swap and shop. Love it.",
    sale: (price) => `Alloys? Sure. ${price} a unit. Practically charity.`,
    bigSpend: "Big order! You're making my week.",
    armaments: "Armaments. Fresh stock, all legal.",
    chips: "Chips! Real ones, I promise.",
    goods: "Quality goods, quick service.",
    deal: "Done! Pleasure doing business, chief.",
    credits: "Bit short there, chief. Credits or Alloys.",
    stock: "Whoa, I don't have that many.",
    cap: "My hold can't take that many, chief.",
    short: "You haven't got that many Alloys.",
    storage: "Where's it all going to go, chief?",
    charter: "Your charter's in the red. Can't touch it.",
    notMine: "Don't carry that, sorry.",
    no: "No can do.",
  } },
};

/** The founder's trader as the server dealt it; Teo until it says. */
export function traderOf(founding: FoundingView | null | undefined): Trader {
  return TRADERS[founding?.trader ?? "teo"] ?? TRADERS.teo;
}

const PORTRAITS = "/art/derived/free-trader/v1";
/** Until a trader's painting lands, their own ship stands in. */
const portraitInterim = (trader: Trader): string => `/art/derived/ships/models-v1/previews/${traderShipArt(trader.slug)}-768.webp`;
const MOOD_SUFFIX: Record<TraderMood, string> = { neutral: "", pleased: "-pleased", scoff: "-scoff" };

// Each painting is probed once; a missing one is never asked for again.
const probed = new Map<string, boolean>();
function painted(src: string, onLoad: () => void): boolean {
  if (probed.has(src)) return probed.get(src)!;
  if (typeof Image === "undefined") return false;
  probed.set(src, false);
  const image = new Image();
  image.onload = () => { probed.set(src, true); onLoad(); };
  image.src = src;
  return false;
}

/** The trader's portrait for `mood`: its own expression when painted, else
 * the neutral one, else the ship. `onLoad` hears when a painting arrives. */
export function traderPortrait(trader: Trader, mood: TraderMood, onLoad: () => void = () => {}): { src: string; alt: string; painted: boolean } {
  const alt = `${trader.name}, master of the ${trader.ship}.`;
  for (const suffix of new Set([MOOD_SUFFIX[mood], ""])) {
    const src = `${PORTRAITS}/${trader.portrait}${suffix}-320.webp`;
    if (painted(src, onLoad)) return { src, alt, painted: true };
  }
  return { src: portraitInterim(trader), alt: `The ${trader.ship}.`, painted: false };
}

type VoiceLine = Exclude<keyof TraderVoice, "sale">;

/** The trader's word on a refusal, the sim's reason or the stall's own. */
const REFUSED: [RegExp, VoiceLine][] = [
  [/credits/i, "credits"],
  [/hasn't that much/i, "stock"],
  [/won't take that many/i, "cap"],
  // Before "at home": "Not enough storage room at home" is the storage's.
  [/storage/i, "storage"],
  [/^Not enough .+ at home/i, "short"],
  [/charter/i, "charter"],
  [/doesn't (sell|buy)/i, "notMine"],
  [/isn't docked/i, "arriving"],
  [/No merchant/i, "leaving"],
];
export function refusalLine(trader: Trader, reason: string): TraderLine {
  return { text: trader.voice[REFUSED.find(([match]) => match.test(reason))?.[1] ?? "no"], mood: "scoff" };
}

/** What became of the last trade sent, from the server's own timeline line:
 * "Free trader: …" after `since`. */
export function traderResult(timeline: readonly TimelineEntry[], since: number): { ok: true } | { ok: false; reason: string } | null {
  const line = [...timeline].reverse().find(e => e.at_time + 1e-6 >= since && e.text.startsWith("Free trader: "));
  if (!line) return null;
  const refused = /^Free trader: trade refused — (.+)\.$/.exec(line.text);
  return refused ? { ok: false, reason: refused[1] } : { ok: true };
}

/** What the trader says about the offer on the table. Nothing entered: a
 * greeting; an offer that won't go: why, in the trader's words; an offer that
 * will: their take on it. `dealt`: a trade already went through this visit.
 * §den-contract (Nov 5 2026): it stays for the charter, so it has no last
 * call. */
export function traderLine(trader: Trader, m: MerchantView, buys: MerchantGoods, sells: MerchantGoods,
  quote: MerchantQuote, dealt: boolean): TraderLine {
  const v = trader.voice;
  if (m.phase === "arriving") return { text: v.arriving, mood: "neutral" };
  if (m.phase === "leaving") return { text: v.leaving, mood: "scoff" };
  const entered = (book: MerchantGoods) => Object.entries(book).filter(([, n]) => (n ?? 0) > 0).map(([c]) => c);
  const buying = entered(buys);
  const selling = entered(sells);
  if (!buying.length && !selling.length) {
    return dealt ? { text: v.again, mood: "pleased" } : { text: v.greeting, mood: "neutral" };
  }
  if (quote.error) return refusalLine(trader, quote.error);
  if (buying.length && selling.length) return { text: v.both, mood: "pleased" };
  if (selling.length) return { text: v.sale(price(m.buy_price)), mood: "neutral" };
  if (quote.cost >= 1_500) return { text: v.bigSpend, mood: "pleased" };
  if (buying.includes("armaments")) return { text: v.armaments, mood: "neutral" };
  if (buying.includes("electronics")) return { text: v.chips, mood: "neutral" };
  return { text: v.goods, mood: "neutral" };
}

/** After a trade went through: the trader's parting shot. */
export function dealLine(trader: Trader): TraderLine {
  return { text: trader.voice.deal, mood: "pleased" };
}

const price = (n: number) => `${Number.isInteger(n) ? n : n.toFixed(1)} Cr`;
