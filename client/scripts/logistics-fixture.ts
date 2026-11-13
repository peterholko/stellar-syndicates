// Isolated preview of the Logistics route editor (Deck styles): a home world,
// a docked Tiny Freighter and a Hub price report; no socket, no game. Clicks
// and inputs go through the real industry handlers; orders go to the console.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import { handleIndustryAction, handleIndustryInput, industryHtml } from "../src/shell/industry";
import type { ViewState } from "../src/state";
import { state } from "../src/state";
import type { CoreContext } from "../src/shell/types";
import { setHtml } from "../src/shell/dom";

const home = {
  id: "home", owner: "1", storage_used: 420, storage_cap: 1000,
  stockpile: [{ commodity: "alloys", units: 64 }, { commodity: "metallic_ore", units: 120 }, { commodity: "machinery", units: 30 }, { commodity: "uranium", units: 40 }],
  bodies: [{ id: 0, name: "Freya II", habitable: true, deposits: [{ resource: "metallic_ore" }], structures: { shipyard: 1, smelter: 1 } }],
  assignments: [], workforce: { posted: 2, units: 1 }, industry: { reservations: [], outpost: null, projects: [] },
};
const prices = [["metallic_ore", 2.4], ["alloys", 26], ["machinery", 62], ["electronics", 34], ["uranium", 5.6], ["polymers", 30], ["provisions", 8], ["fission_reactor", 145]]
  .map(([commodity, price]) => ({ commodity, price, available_buy: 999, available_sell: 999, depth: 1600 }));
Object.assign(state, {
  playerId: "1", name: "Fixture", commandCenter: { x: 0, y: 0 },
  systems: [home],
  ghosts: [{ id: "f", kind: "tiny_freighter", own: true, pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 }, docked: "home", cargo_manifest: [], fuel: 50, fuel_capacity: 52.5, transit: "full" }],
  research: { programmes: [], active: null, queue: [] },
  galaxy: { c: 2000, hub: { x: 125000, y: 0 }, systems: [{ id: "home", name: "Freya", pos: { x: 0, y: 0 } }], build_options: [],
    industry_catalog: { projects: [], outpost_costs: [], outpost_build_secs: 60, skim_helium3_per_s: 0, max_stops: 8 } },
  market: { staleness: 0, prices },
});
const st = state as unknown as ViewState;
const root = document.getElementById("fixture")!;
const ctx = { state: st, send: (m: unknown) => console.log("send", m), intent: { beginFleetCommand: (c: unknown) => console.log("stage", c) } } as unknown as CoreContext;
// The game's own redraw: it morphs, so unchanged nodes (an open list, its scroll) stay.
const draw = () => setHtml(root, industryHtml(st));
const act = (industryAct: string, extra: Record<string, string> = {}) =>
  handleIndustryAction({ dataset: { industryAct, ...extra } } as unknown as HTMLElement, ctx);
root.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLElement>("[data-industry-act]");
  if (button && handleIndustryAction(button, ctx)) draw();
});
// Like the Deck: typing and toggles alike.
for (const kind of ["input", "change"]) root.addEventListener(kind, (event) => { if (handleIndustryInput(event.target as HTMLInputElement, st)) draw(); });
const params = new URLSearchParams(location.search);
act("pick-fleet", { fleet: "f" });
act("new-route");
act("open-pick", { stop: params.get("stop") ?? "1", side: "load" });
if (params.get("open")) act("goods-menu");
draw();
// The Hub report ticks: the open list must not move; only the price beside it.
setInterval(() => { for (const p of prices) p.price *= 1 + (Math.random() - 0.5) * 0.02; draw(); }, 1000);
