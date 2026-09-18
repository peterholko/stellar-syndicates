import type { ConversionRecipe, SystemStateView } from "../protocol";
import { refiningEstimate, type RefiningContext } from "../core/derive/refining";
import { fmtDur } from "../core/derive/format";
import { label } from "../icons";

/** Shared by desktop and mobile. Estimates never mutate production or orders. */
export function refiningComparison(st: RefiningContext, system: SystemStateView, body: number, recipe?: ConversionRecipe): string {
  if (system.owner !== st.playerId || !recipe) return "";
  const e = refiningEstimate(st,system,body,recipe);
  if (!e) return `<div class="refining-comparison"><small>Refining estimate unavailable · waiting for reports or market liquidity.</small></div>`;
  const money = (n: number) => `~${Math.round(n).toLocaleString()} Cr`;
  return `<section class="refining-comparison" aria-label="Raw versus refined estimate"><small>Estimate · ${e.units} ${label(e.ore)}</small>
    <dl><div><dt>Sell raw</dt><dd>${money(e.raw)}</dd></div><div><dt>Refine &amp; sell <small>after Fuel</small></dt><dd>${money(e.refined)}</dd></div></dl>
    <small>${e.fuel} Fuel · ${e.workforce} workforce · ${e.seconds === null ? "paused · needs Provisions" : `${fmtDur(e.seconds)} processing`} · recovery ×${e.recovery.toFixed(2)}</small>
    ${!e.built ? `<small>Planned Smelter I${!e.unlocked ? " · requires Enrichment" : ""}</small>` : ""}
    <small>Prices ${fmtDur(e.priceAge)} old. Fully staffed &amp; supplied; excludes freight, building and workforce costs.</small></section>`;
}
