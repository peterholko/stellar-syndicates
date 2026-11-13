// §merchant: the traveling merchant's stall at your home
// (docs/traveling-merchant.md). It is a last resort: it sells the founding
// goods well above the Hub and buys a few Alloys well below it (§sell-alloys,
// Nov 11 2026; `MerchantView.buys`).
import type { Commodity, MerchantView, SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { label } from "../../icons";
import { HULL_MASS } from "./fleet";
import { availableStock } from "./industry";

/** The charter builds the guide can send to the merchant. §charter-v4: the
 * Solar Array and the Interceptor too. §den-stores (Nov 5 2026): the Tiny
 * Freighter, with its fuel. §den-contract (Peter, Nov 5 2026: "The free
 * trader doesn't leave after building your own freighter that way the player
 * can buy the necessary parts from free trader"): it stays for the charter,
 * so every build after the den buys its parts as it comes up, and "repair"
 * the den-mauled Interceptor's repair parts. */
export type MerchantTarget = "power_plant" | "shipyard" | "raider" | "tiny_freighter" | "academy" | "smelter" | "mining_complex"
  | "asteroid_quarry" | "survey_probe" | "mining_barge" | "small_freighter" | "repair";
export const MERCHANT_TARGETS: readonly MerchantTarget[] = ["power_plant", "shipyard", "raider", "tiny_freighter", "academy", "smelter",
  "mining_complex", "asteroid_quarry", "survey_probe", "mining_barge", "small_freighter", "repair"];
/** §den-stores: the fuel the Tiny Freighter fills at the dock, with its
 * parts. §reactors-drives (Nov 8 2026): its Fission Reactors burn Uranium at
 * every speed, so one tank, 52.5 (sim hull mass × `fuel::FUEL_PER_HULL_MASS`),
 * and a refill at home for a third den load at Full speed (~19 a round trip).
 * It has no Deuterium tank, and the trader stocks none. */
export const TINY_FREIGHTER_FUEL = { uranium: 60 } as const;
/** §den-contract: what a yard spends per hull point it restores (sim
 * `build::REPAIR_COST_PER_HP`); a hull's points are its mass. */
const REPAIR_COST_PER_HP: Partial<Record<Commodity, number>> = { alloys: 0.01, machinery: 0.002 };
export const isMerchantTarget = (key: string): key is MerchantTarget => (MERCHANT_TARGETS as readonly string[]).includes(key);

export type MerchantGoods = Partial<Record<Commodity, number>>;

/** The merchant while it is docked at your home, trading. */
export const dockedMerchant = (st: ViewState): MerchantView | null =>
  st.merchant?.phase === "docked" ? st.merchant : null;

/** What the trader buys, held at home: the stockpile less what queued builds
 * have set aside. */
export const sellableAtHome = (m: MerchantView, home?: SystemStateView): number =>
  home ? Math.floor(availableStock(home, m.buys)) : 0;

export interface MerchantQuote { cost: number; revenue: number; netUnits: number; error: string | null }

const NOTHING = "Nothing to trade";
const NO_ROOM = "Not enough storage room at home";
const NO_CREDITS = "Not enough credits";

/** Price a trade and name the first thing that won't fit. Mirrors the sim's
 * MerchantTrade checks: its stock and what it will take, your stock at home,
 * storage room and credits (as last reported). */
export function merchantQuote(m: MerchantView, buys: MerchantGoods, sells: MerchantGoods,
  home?: SystemStateView, credits?: number): MerchantQuote {
  let cost = 0, revenue = 0, netUnits = 0;
  let error: string | null = null;
  for (const [c, n] of Object.entries(buys) as [Commodity, number][]) {
    if (!n) continue;
    const good = m.goods.find((g) => g.commodity === c);
    if (!good) { error ??= "The merchant doesn't sell that"; continue; }
    if (n > good.stock) error ??= "The merchant hasn't that much";
    cost += good.price * n;
    netUnits += n;
  }
  for (const [c, n] of Object.entries(sells) as [Commodity, number][]) {
    if (!n) continue;
    if (c !== m.buys) { error ??= "The merchant doesn't buy that"; continue; }
    if (n > m.buy_room) error ??= "The merchant won't take that many";
    if (home && n > availableStock(home, c) + 1e-9) error ??= `Not enough ${label(c)} at home`;
    revenue += m.buy_price * n;
    netUnits -= n;
  }
  if (!cost && !revenue) error ??= NOTHING;
  const room = home ? Math.max(0, (home.storage_cap ?? 0) - (home.storage_used ?? 0)) : Infinity;
  if (netUnits > 0 && netUnits > room + 1e-9) error ??= NO_ROOM;
  if (credits !== undefined && credits + revenue + 1e-9 < cost) error ??= NO_CREDITS;
  return { cost, revenue, netUnits, error };
}

/** Why a deal is blocked, in plain words. `storage`: it doesn't fit at home;
 * `full` within a unit of the cap (the guide's own "full"); `sell`: the sale
 * of what the trader buys, in all, that makes room in the same deal (or pays
 * for it), if it can. */
export interface MerchantBlock { text: string; storage: boolean; full: boolean; sell: number | null }

const whole = (n: number) => Math.floor(Math.max(0, n)).toLocaleString();

/** §storage-full (Peter, Nov 3 2026: "make it more clear to the player that
 * their storage is full"): the trader's word on a blocked deal is in
 * character; this says it plainly. Null when the deal goes or is empty.
 * §three-stipends (Nov 6 2026): a deal short of credits offers the sale that
 * pays for it, inside the trader's cap. */
export function merchantBlock(m: MerchantView, sells: MerchantGoods, quote: MerchantQuote,
  home: SystemStateView | undefined, trader: string, credits?: number): MerchantBlock | null {
  if (!quote.error || quote.error === NOTHING) return null;
  const most = Math.min(m.buy_room, sellableAtHome(m, home));
  if (quote.error === NO_CREDITS && credits !== undefined && m.buy_price > 0) {
    const units = (sells[m.buys] ?? 0) + Math.ceil((quote.cost - quote.revenue - credits) / m.buy_price - 1e-9);
    return { text: NO_CREDITS, storage: false, full: false, sell: units <= most ? units : null };
  }
  if (quote.error !== NO_ROOM || !home) {
    return { text: quote.error.replace(/^The merchant\b/, trader), storage: false, full: false, sell: null };
  }
  const room = Math.max(0, home.storage_cap - home.storage_used);
  const full = room <= 1;
  const units = (sells[m.buys] ?? 0) + Math.ceil(quote.netUnits - room - 1e-9);
  const stored = `${whole(home.storage_used)}/${whole(home.storage_cap)}`;
  return { text: full ? `Storage full · ${stored}` : `Storage ${stored} · room for ${whole(room)}`, storage: true, full,
    sell: units <= most ? units : null };
}

/** §charter-v5: the guide's other prefills. `sell`: all the Alloys at home the
 * trader will take ("Sell Alloys to the trader"). `fuel`: the fuel a hull's
 * reactor burns, to fill its tank (founding `fuelNeed`), bounded by stock. */
export type MerchantErrand = "sell" | "fuel";
export const isMerchantErrand = (key: string): key is MerchantErrand => key === "sell" || key === "fuel";
export function merchantErrand(errand: MerchantErrand, home: SystemStateView | undefined, m: MerchantView,
  need: { fuel: Commodity; room: number } | null = null): { buys: MerchantGoods; sells: MerchantGoods } {
  if (errand === "sell") {
    const units = Math.min(sellableAtHome(m, home), m.buy_room);
    return { buys: {}, sells: units > 0 ? { [m.buys]: units } : {} };
  }
  if (!need) return { buys: {}, sells: {} };
  const stock = m.goods.find((g) => g.commodity === need.fuel)?.stock ?? 0;
  const units = Math.min(Math.ceil(need.room), stock);
  return { buys: units > 0 ? { [need.fuel]: units } : {}, sells: {} };
}

/** The den-mauled hulls' repair parts: their lost hull points at the yard's
 * rate. */
function repairCosts(st: ViewState): { commodity: string; units: number }[] {
  const hurt = st.ghosts.filter(g => g.own && st.founding?.den_wounded?.includes(g.id));
  const points = hurt.reduce((sum, g) => sum + (g.damage ?? 0) * (g.composition
    ? g.composition.reduce((n, c) => n + c.count * (HULL_MASS[c.kind] ?? 0), 0) : HULL_MASS[g.kind] ?? 0), 0);
  return Object.entries(REPAIR_COST_PER_HP).map(([commodity, per]) => ({ commodity, units: points * (per ?? 0) }));
}

/** What a charter build (or "repair") still lacks at home that the merchant
 * sells, bounded by its stock: the guide's prefill. */
export function merchantShortfall(st: ViewState, home: SystemStateView | undefined, target: MerchantTarget,
  m: MerchantView): MerchantGoods {
  const recipe = (key: string) => st.galaxy?.build_options.find((o) => o.key === key)?.costs ?? [];
  const fuel = target === "tiny_freighter" ? Object.entries(TINY_FREIGHTER_FUEL).map(([commodity, units]) => ({ commodity, units })) : [];
  const costs = target === "repair" ? repairCosts(st) : [...recipe(target), ...fuel];
  const out: MerchantGoods = {};
  // §charter-crates: a build counts only a crate already released (Peter,
  // Nov 5 2026: no Pirates crate, so the first Interceptor's Alloys are the
  // trader's, not a sealed crate's).
  const released = (c: Commodity) => (st.founding?.crates ?? []).filter((k) => k.released)
    .reduce((n, k) => n + (k.goods[c] ?? 0), 0);
  for (const cost of costs) {
    const c = cost.commodity as Commodity;
    const good = m.goods.find((g) => g.commodity === c);
    if (!good) continue;
    const short = Math.ceil(cost.units - (home ? availableStock(home, c) : 0) - released(c) - 1e-9);
    const units = Math.min(Math.max(0, short), good.stock);
    if (units > 0) out[c] = units;
  }
  return out;
}
