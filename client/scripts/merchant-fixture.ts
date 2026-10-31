// Isolated preview of the free trader's negotiation table (Deck): served
// fixtures only, no socket, no game. Trades go to the console.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import type { MerchantView, SystemStateView } from "../src/protocol";
import { DeckMerchantRoutes } from "../src/shell/deck/merchant";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

const docked: MerchantView = { fleet: "m", system: "home", phase: "docked", leave_by: 1_000 + 9_731, markup: 0.5, ore_price: 4, ore_room: 300,
  goods: [["alloys", 120, 39], ["machinery", 50, 93], ["electronics", 100, 51], ["armaments", 15, 84], ["polymers", 30, 45],
    ["provisions", 40, 15], ["deuterium", 30, 38]].map(([commodity, stock, price]) =>
    ({ commodity, stock, price, hub_mid: (price as number) / 1.5 })) as MerchantView["goods"] };
syncRenderClock(1_000);
state.simTime = 1_000;
state.merchant = docked;
// §traders: ?trader=teo|ines|mags picks who flies it.
state.founding = { trader: new URLSearchParams(location.search).get("trader") ?? "teo" } as unknown as typeof state.founding;
state.wallet = { credits: 3_000, report_pending: false } as typeof state.wallet;
state.timeline = [];
state.systems = [{ id: "home", storage_used: 620, storage_cap: 1_000, stockpile: [
  { commodity: "metallic_ore", units: 212 }, { commodity: "alloys", units: 330 }, { commodity: "machinery", units: 120 }] } as unknown as SystemStateView];
state.galaxy = { build_options: [{ key: "raider", costs: [{ commodity: "alloys", units: 20 }, { commodity: "electronics", units: 12 },
  { commodity: "armaments", units: 15 }, { commodity: "warp_core", units: 1 }] }] } as unknown as typeof state.galaxy;

const root = document.getElementById("deck-workspace-body")!;
const ctx = { state, send: (message: unknown) => console.log("send", message) } as unknown as CoreContext;
const stall = new DeckMerchantRoutes(root, ctx, { go() {}, notice: (html: string) => console.log("notice", html) });
let route = { name: "merchant", query: { for: "ore" } } as unknown as DeckRoute;
root.addEventListener("input", event => stall.handleInput(event.target as HTMLInputElement, route));
root.addEventListener("click", event => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) stall.handleAction(button, route);
});
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-fixture]")) {
  button.addEventListener("click", () => {
    const which = button.dataset.fixture!;
    state.merchant = which === "arriving" ? { ...docked, phase: "arriving" } : docked;
    state.wallet = { credits: which === "short" ? 400 : 3_000, report_pending: false } as typeof state.wallet;
    route = { name: "merchant", query: { for: which === "short" ? "raider" : which } } as unknown as DeckRoute;
    stall.invalidate();
    stall.render(route, true);
  });
}
setInterval(() => stall.render(route), 500);
stall.render(route, true);
