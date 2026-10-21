// §merchant: the traveling merchant's stall at your home
// (docs/traveling-merchant.md). It is a last resort: it sells the founding
// goods well above the Hub and buys a little Ferrite Ore well below it.
import type { Commodity, MerchantView, SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { availableStock } from "./industry";

/** The charter builds the guide can send to the merchant. §charter-v4: the
 * Solar Array and the Interceptor too (a Shipyard II costs a Shipyard).
 * §charter-v5: "ahead", the trader's last call: the semiconductors of the
 * builds after the den (`AHEAD`). */
export type MerchantTarget = "power_plant" | "shipyard" | "raider" | "academy" | "survey_probe" | "mining_barge" | "ahead";
export const MERCHANT_TARGETS: readonly MerchantTarget[] = ["power_plant", "shipyard", "raider", "academy", "survey_probe", "mining_barge", "ahead"];
/** §charter-v5: the charter's builds after the den: the Academy, the
 * expansion's Solar Array and Mining Complex, the Survey Probe and the Mining
 * Barge. The raid took the kit's semiconductors they need. The bots count
 * the same builds (`charter_goods_ahead`). */
const AHEAD = ["academy", "power_plant", "mining_complex", "survey_probe", "mining_barge"];
export const isMerchantTarget = (key: string): key is MerchantTarget => (MERCHANT_TARGETS as readonly string[]).includes(key);

export type MerchantGoods = Partial<Record<Commodity, number>>;

/** The merchant while it is docked at your home, trading. */
export const dockedMerchant = (st: ViewState): MerchantView | null =>
  st.merchant?.phase === "docked" ? st.merchant : null;

export const merchantLeavesIn = (m: MerchantView, now: number): number => Math.max(0, m.leave_by - now);

/** Ferrite Ore at home the trader could take: the stockpile less what queued
 * builds have set aside. */
export const oreAtHome = (home?: SystemStateView): number => home ? Math.floor(availableStock(home, "metallic_ore")) : 0;

export interface MerchantQuote { cost: number; revenue: number; netUnits: number; error: string | null }

/** Price a trade and name the first thing that won't fit. Mirrors the sim's
 * MerchantTrade checks: its stock and ore cap, your home ore, storage room and
 * credits (as last reported). */
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
    if (c !== "metallic_ore") { error ??= "The merchant doesn't buy that"; continue; }
    if (n > m.ore_room) error ??= "The merchant won't take that much ore";
    if (home && n > availableStock(home, c) + 1e-9) error ??= "Not enough ore at home";
    revenue += m.ore_price * n;
    netUnits -= n;
  }
  if (!cost && !revenue) error ??= "Nothing to trade";
  const room = home ? Math.max(0, (home.storage_cap ?? 0) - (home.storage_used ?? 0)) : Infinity;
  if (netUnits > 0 && netUnits > room + 1e-9) error ??= "Not enough storage room at home";
  if (credits !== undefined && credits + revenue + 1e-9 < cost) error ??= "Not enough credits";
  return { cost, revenue, netUnits, error };
}

/** §charter-v5: the guide's other prefills. `ore`: all the ore at home the
 * trader will take ("Sell ore to the trader"). `fuel`: Deuterium to fill a
 * hull's tank, bounded by its stock. */
export type MerchantErrand = "ore" | "fuel";
export const isMerchantErrand = (key: string): key is MerchantErrand => key === "ore" || key === "fuel";
export function merchantErrand(errand: MerchantErrand, home: SystemStateView | undefined, m: MerchantView,
  fuelRoom = 0): { buys: MerchantGoods; sells: MerchantGoods } {
  if (errand === "ore") {
    const units = Math.min(oreAtHome(home), m.ore_room);
    return { buys: {}, sells: units > 0 ? { metallic_ore: units } : {} };
  }
  const stock = m.goods.find((g) => g.commodity === "deuterium")?.stock ?? 0;
  const units = Math.min(Math.ceil(fuelRoom), stock);
  return { buys: units > 0 ? { deuterium: units } : {}, sells: {} };
}

/** What a charter build still lacks at home that the merchant sells, bounded
 * by its stock: the guide's prefill. */
export function merchantShortfall(st: ViewState, home: SystemStateView | undefined, target: MerchantTarget,
  m: MerchantView): MerchantGoods {
  const recipe = (key: string) => st.galaxy?.build_options.find((o) => o.key === key)?.costs ?? [];
  const costs = target !== "ahead" ? recipe(target) : [...AHEAD.flatMap(recipe).reduce((sum, c) =>
    sum.set(c.commodity, (sum.get(c.commodity) ?? 0) + c.units), new Map<string, number>())]
    .map(([commodity, units]) => ({ commodity, units }));
  const out: MerchantGoods = {};
  for (const cost of costs) {
    const c = cost.commodity as Commodity;
    const good = m.goods.find((g) => g.commodity === c);
    if (!good) continue;
    const short = Math.ceil(cost.units - (home ? availableStock(home, c) : 0) - 1e-9);
    const units = Math.min(Math.max(0, short), good.stock);
    if (units > 0) out[c] = units;
  }
  return out;
}
