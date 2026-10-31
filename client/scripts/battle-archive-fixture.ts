// Isolated preview of the Log's Battles tab (Deck): canned records, no socket.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import type { BattleRecordView } from "../src/protocol";
import { DeckLogRoutes } from "../src/shell/deck/log";
import { mountDeckMarkup } from "../src/shell/deck/markup";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

const NOW = 20_000;
const side = (corp: string) => ({ corp, posture: null, platform_tiers: 0, initial: [] });
const rec = (id: string, ago: number, outcome: BattleRecordView["outcome"], own: number | null, system: string | null, corps = ["pirates", "me"]): BattleRecordView => ({
  id, pos: { x: 100, y: 0 }, system, started_at: NOW - ago, raid: false, fidelity: "participant", own_side: own,
  sides: [side(corps[0]), side(corps[1])] as BattleRecordView["sides"], rounds: [], light_frontier_tick: 0, outcome,
});
syncRenderClock(NOW);
state.simTime = NOW;
state.playerId = "me";
state.galaxy = { pirate_id: "pirates", systems: [{ id: "wadjet", name: "Wadjet", pos: { x: 0, y: 0 } }, { id: "dabih", name: "Dabih", pos: { x: 900, y: 0 } }] } as unknown as typeof state.galaxy;
state.rankings = [{ player_id: "greeks", name: "Greeks" }] as typeof state.rankings;
state.battleRecords = [
  rec("b1", 40, null, 1, "wadjet"), rec("b2", 600, "attacker_destroyed", 1, "wadjet"), rec("b3", 3_700, "target_destroyed", 1, "wadjet"),
  rec("b4", 7_200, "both_survive", 0, null), rec("b5", 90_000, "target_destroyed", null, "dabih", ["greeks", "pirates"]),
];
state.battles = [{ id: "b1", pos: { x: 100, y: 0 }, own: true, participants: [] }] as unknown as typeof state.battles;
const host = document.createElement("div");
document.body.append(host);
mountDeckMarkup(host);
document.getElementById("deck-join")!.hidden = true;
const aside = document.getElementById("deck-workspace")!;
aside.dataset.route = "log";
aside.setAttribute("aria-hidden", "false");
const root = document.getElementById("deck-workspace-body")!;
const ctx = { state, send() {} } as unknown as CoreContext;
let route = { name: "log", query: { tab: "battles" } } as DeckRoute;
const log = new DeckLogRoutes(root, ctx, { go: (next) => console.log("go", JSON.stringify(next)), focusSystem() {}, focusFleet() {} });
root.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) log.handleAction(button, route);
});
try { log.render(route, true); } catch (error) { root.textContent = String(error); }
route = { name: "log" } as DeckRoute;
