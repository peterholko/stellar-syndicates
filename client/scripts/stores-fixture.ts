// Isolated preview of the stores bar and the Stores workbench (Deck): canned
// systems and five minutes of synthetic stock reports, no game socket.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import { clearStockSamples, noteStockSamples } from "../src/core/derive/stores";
import type { Commodity, SystemStateView } from "../src/protocol";
import { mountDeckMarkup } from "../src/shell/deck/markup";
import { DeckStores } from "../src/shell/deck/stores";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

/** Units at t=0 and the steady change per minute. */
const FLOWS: [Commodity, number, number][] = [
  ["provisions", 1_150, 18], ["metallic_ore", 660, 40], ["alloys", 282, 6], ["machinery", 113, 3],
  ["electronics", 54, -2], ["uranium", 120, 0], ["deuterium", 116, -4], ["polymers", 40, 0], ["crystalline_ore", 210, 12],
  // §goods-tabs: parts and a module, for the Resources · Parts · Modules strip.
  ["fission_reactor", 4, 0], ["zephyr_drive", 3, 0], ["torpedo_rack", 2, 0], ["whipple_armor", 1, 0],
  // The Nov 8 2026 goods art: Hull Sections, a heavy and a capital part, more modules.
  ["hull_sections", 6, 0], ["heavy_fission_reactor", 1, 0], ["capital_zephyr_drive", 1, 0],
  ["lance_array", 1, 0], ["stasis_web", 1, 0], ["survey_drive", 1, 0], ["cargo_pods", 1, 0],
];
const NOW = 2_000;
/** §stores-flows: what the economy makes and uses a minute (net = the FLOWS
 * rate); `?measured` leaves them out to show the measured fallback. */
const SPLIT: Partial<Record<Commodity, [number, number]>> = {
  provisions: [30, 12], metallic_ore: [52, 12], alloys: [6, 0], machinery: [3, 0], electronics: [0, 2], deuterium: [0, 4], crystalline_ore: [12, 0],
};
const MEASURED = new URLSearchParams(location.search).has("measured");

function system(id: string, cap: number, scale: number, t: number): SystemStateView {
  const stockpile = FLOWS.map(([commodity, start, perMinute]) => {
    // A freighter unloads 40 Alloys a minute before now; the rate ignores it.
    const unload = commodity === "alloys" && t >= NOW - 60 ? 40 : 0;
    return { commodity, units: Math.max(0, Math.floor((start + perMinute * (t - (NOW - 300)) / 60) * scale) + unload) };
  });
  const used = stockpile.reduce((sum, slot) => sum + slot.units, 0);
  const flows = MEASURED ? undefined : Object.entries(SPLIT).map(([commodity, [made, usedUp]]) => ({ commodity, made: made * scale, used: usedUp * scale }));
  return {
    id, owner: "me", stockpile, flows, storage_used: used, storage_cap: cap, builds: [], bodies: [], assignments: [],
    industry: { reservations: [{ target: { kind: "orbital_assembly" }, goods: { alloys: 40 } }], outpost: null, projects: [] },
  } as unknown as SystemStateView;
}

function load(which: string): void {
  clearStockSamples();
  state.playerId = "me";
  state.commandCenter = { x: 0, y: 0 };
  state.galaxy = { systems: [{ id: "freya", name: "Freya", pos: { x: 0, y: 0 } }, { id: "dabih", name: "Dabih", pos: { x: 900, y: 0 } }] } as unknown as typeof state.galaxy;
  const cap = which === "full" ? 3_000 : 5_000;
  for (let t = NOW - 300; t <= NOW; t += 2) {
    state.systems = [system("freya", cap, 1, t), ...(which === "two" ? [system("dabih", 2_000, 0.4, t)] : [])];
    noteStockSamples(state, t);
  }
  syncRenderClock(NOW);
  state.simTime = NOW;
}

// ?deck: the real Deck markup (top bar, nav, workspace) instead of the bare
// fixture, to check the taller chrome and the workbench's place.
if (new URLSearchParams(location.search).has("deck")) {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  mountDeckMarkup(host);
  document.getElementById("deck-join")!.hidden = true;
}
const bar = document.getElementById("deck-stores")!;
const shell = document.getElementById("deck-stores-workbench")!;
const status = (text: string) => { const line = document.getElementById("fixture-status"); if (line) line.textContent = text; };
const ctx = { state, send: (message: unknown) => status(`sent ${JSON.stringify(message)}`) } as unknown as CoreContext;
// Stores is no route: the system's Production tab stays open beneath it.
const route = { name: "system", params: { id: "freya" }, query: { tab: "production" } } as DeckRoute;
const stores = new DeckStores(bar, shell, ctx, {
  notice: (html) => status(html.replace(/<[^>]+>/g, "")),
  resized: () => {},
}, new AbortController().signal);
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-fixture]")) {
  button.addEventListener("click", () => {
    const which = button.dataset.fixture!;
    if (which === "pop") stores.toggle(route);
    else load(which);
    stores.invalidate();
    stores.renderBar(route, true);
    stores.render(route, true);
  });
}
load("one");
stores.renderBar(route, true);
stores.open(route);
