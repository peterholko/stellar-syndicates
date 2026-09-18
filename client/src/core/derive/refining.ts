import type { Commodity, ConversionRecipe, SystemStateView } from "../../protocol";
import type { ViewState } from "../../state";
import { marketAverageQuote, recipeOutputs } from "./market";

/** Compare the SAME 50-unit ore lot, not one raw unit against one refined unit.
 * Only arrived prices/capabilities are accepted. Never substitute reference
 * prices, current corporate research or a pending assignment for missing light.
 * Rated time assumes full staffing and supplied inputs, not a completion clock.
 */
export type RefiningContext = Pick<ViewState, "playerId" | "market" | "research">;
export function refiningEstimate(st: RefiningContext, system: SystemStateView, body: number, recipe?: ConversionRecipe) {
  if (system.owner !== st.playerId || !recipe) return null;
  const site = system.refining_sites?.find(s => s.body_id === body);
  const ore = recipe.inputs[0]?.[0];
  if (!site || !ore || !(site.recovery > 0) || !st.market) return null;
  const units = 50;
  const work = units / recipe.inputs[0][1];
  const quote = (c: Commodity, n: number, side: "buy" | "sell") => {
    if (n === 0) return 0;
    const p = st.market!.prices.find(p => p.commodity === c);
    if (!p || !Number.isFinite(p.price) || p.price <= 0) return null;
    // Use reported executable depth, not the entire ticker at its top price.
    if ((side === "buy" ? p.available_buy : p.available_sell) < n) return null;
    return marketAverageQuote(p.price, n, side, p.depth) * n;
  };
  const inputs = recipe.inputs.slice(1).map(([c,n]) => [c, Math.ceil(work * n - 1e-9)] as const);
  const outputs = recipeOutputs(recipe).map(([c,n]) => [c, Math.floor(work * site.recovery * n + 1e-9)] as const);
  const raw = quote(ore, units, "sell");
  const costs = inputs.map(([c,n]) => quote(c,n,"buy"));
  const sales = outputs.map(([c,n]) => quote(c,n,"sell"));
  if (raw === null || [...costs,...sales].some(n => n === null)) return null;
  const fuelCost = costs.reduce<number>((n,x) => n + x!,0);
  const refined = sales.reduce<number>((n,x) => n + x!,0) - fuelCost;
  return { ore, units, raw, refined, difference: refined - raw, fuelCost,
    fuel: inputs.find(([c]) => c === "fuel")?.[1] ?? 0, outputs,
    seconds: site.work_rate > 0 ? work / (recipe.rate * site.work_rate) : null,
    workforce: site.tier, built: site.built, recovery: site.recovery,
    priceAge: st.market.staleness,
    unlocked: !!st.research?.programmes.some(p => p.id === "mat_enrichment" && p.state === "completed") };
}
