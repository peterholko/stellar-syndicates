// §merchant persona (Oct 16 2026, Peter's pick): Teo Brannick of the Long
// Odds, a big, loud, theatrical haggler. The prices are the sim's; only the
// talk is Teo's. Both shells' stalls show the portrait and Teo's line about
// the offer on the table.
import type { MerchantView, TimelineEntry } from "../../protocol";
import type { MerchantGoods, MerchantQuote } from "./merchant";

export const TRADER = { name: "Teo Brannick", ship: "the Long Odds" } as const;

/** How Teo takes the offer: the portrait's expression. */
export type TraderMood = "neutral" | "pleased" | "scoff";
export interface TraderLine { text: string; mood: TraderMood }

const PORTRAIT = "/art/derived/free-trader/v1/teo-brannick";
/** Until the painting lands, the Long Odds stands in. */
const PORTRAIT_INTERIM = "/art/derived/ships/models-v1/previews/free_trader-768.webp";
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

/** The portrait for `mood`: its own expression when painted, else the
 * neutral one, else the ship. `onLoad` hears when a painting arrives. */
export function traderPortrait(mood: TraderMood, onLoad: () => void = () => {}): { src: string; alt: string; painted: boolean } {
  const alt = `${TRADER.name}, master of ${TRADER.ship}.`;
  for (const suffix of new Set([MOOD_SUFFIX[mood], ""])) {
    const src = `${PORTRAIT}${suffix}-256.webp`;
    if (painted(src, onLoad)) return { src, alt, painted: true };
  }
  return { src: PORTRAIT_INTERIM, alt: `${TRADER.ship}.`, painted: false };
}

/** Teo's word on a refusal, the sim's reason or the stall's own. */
const REFUSED: [RegExp, string][] = [
  [/credits/i, "Credits, friend. Or ore. I take both."],
  [/hasn't that much/i, "Ha! I'd sell you the ship if I could."],
  [/won't take that much ore/i, "I've a hold, not a moon."],
  [/ore at home/i, "You haven't dug that much yet."],
  [/storage/i, "And where would you put it all?"],
  [/charter/i, "Your charter's in the red. I don't touch that."],
  [/doesn't (sell|buy)/i, "Not my line of goods."],
  [/isn't docked/i, "Hold your horses, I'm still docking."],
  [/No merchant/i, "Too late, friend. I've a schedule."],
];
export function refusalLine(reason: string): TraderLine {
  return { text: REFUSED.find(([match]) => match.test(reason))?.[1] ?? "No deal.", mood: "scoff" };
}

/** What became of the last trade sent, from the server's own timeline line:
 * "Free trader: …" after `since`. */
export function traderResult(timeline: readonly TimelineEntry[], since: number): { ok: true } | { ok: false; reason: string } | null {
  const line = [...timeline].reverse().find(e => e.at_time + 1e-6 >= since && e.text.startsWith("Free trader: "));
  if (!line) return null;
  const refused = /^Free trader: trade refused — (.+)\.$/.exec(line.text);
  return refused ? { ok: false, reason: refused[1] } : { ok: true };
}

/** What Teo says about the offer on the table. Nothing entered: a greeting;
 * an offer that won't go: why, in Teo's words; an offer that will: Teo's take
 * on it. `dealt`: a trade already went through this visit. `lastCall`
 * (§charter-v5): the privateer's bounty is paid and Teo leaves soon. */
export function traderLine(m: MerchantView, buys: MerchantGoods, sells: MerchantGoods, quote: MerchantQuote,
  dealt: boolean, lastCall = false): TraderLine {
  if (m.phase === "arriving") return { text: "Hold your horses, I'm still docking.", mood: "neutral" };
  if (m.phase === "leaving") return { text: "Too late, friend. I've a schedule.", mood: "scoff" };
  const entered = (book: MerchantGoods) => Object.entries(book).filter(([, n]) => (n ?? 0) > 0).map(([c]) => c);
  const buying = entered(buys);
  const selling = entered(sells);
  if (!buying.length && !selling.length) {
    if (lastCall) {
      return dealt ? { text: "Anything else? The clock's ticking.", mood: "neutral" }
        : { text: "Last call, friend! I'm off before the shooting starts.", mood: "neutral" };
    }
    return dealt ? { text: "Back for more? Good, good.", mood: "pleased" }
      : { text: "Raided! Terrible. Lucky I came by, eh?", mood: "neutral" };
  }
  if (quote.error) return refusalLine(quote.error);
  if (buying.length && selling.length) return { text: "Now we're talking!", mood: "pleased" };
  if (selling.length) {
    return { text: `Ore? Heavy, dirty stuff. ${price(m.ore_price)} a unit, and I'm being kind.`, mood: "neutral" };
  }
  if (quote.cost >= 1_500) return { text: "A big spender! I like you already.", mood: "pleased" };
  if (buying.includes("armaments")) return { text: "The pointy end of trade. Not cheap.", mood: "neutral" };
  if (buying.includes("electronics")) return { text: "Chips! The genuine article.", mood: "neutral" };
  return { text: "Fine goods, fair-ish prices.", mood: "neutral" };
}

/** After a trade went through: Teo's parting shot. */
export const DEAL_LINE: TraderLine = { text: "Done! You'll tell your grandchildren.", mood: "pleased" };

const price = (n: number) => `${Number.isInteger(n) ? n : n.toFixed(1)} Cr`;
