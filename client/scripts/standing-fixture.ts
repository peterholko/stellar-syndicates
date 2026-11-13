// Isolated preview of weekly medals, the corporation card and the contract
// board (Deck): served fixtures only, no socket, no game.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import type { OperationView, RankingRow } from "../src/protocol";
import { DeckStrategicRoutes } from "../src/shell/deck/strategic";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";
import { coach, CONTRACT_COACH } from "../src/shell/coach";

const DAY = 86_400;
const now = 8 * DAY + 3_600 * 4;
syncRenderClock(now);
state.simTime = now;
state.playerId = "12" as typeof state.playerId;
state.midgameStage = "home_development" as typeof state.midgameStage;
const row = (id: string, name: string, extra: Partial<RankingRow>): RankingRow => ({ player_id: id, name, valuation: 0,
  trade_throughput: 0, market_profit: 0, cargo_captured: 0, cargo_protected: 0, battle_efficiency: 0, battle_engagements: 0,
  battle_ranked: false, systems_developed: 0, intel_gathered: 0, recovery: 0, titles: [], ...extra } as RankingRow);
state.rankings = [
  row("7", "Vega Mining Combine", { valuation: 48_200, trade_throughput: 1_840, titles: ["Magnate"], systems_developed: 31,
    medals: [{ category: "valuation", gold: 2, silver: 0, bronze: 1 }, { category: "intel_gathered", gold: 0, silver: 1, bronze: 0 }] }),
  row("3", "Orion & Sons", { valuation: 39_900, trade_throughput: 2_610, titles: ["Master Merchant"], battle_ranked: true, battle_efficiency: 2.4,
    battle_engagements: 7, medals: [{ category: "trade_throughput", gold: 1, silver: 1, bronze: 0 }] }),
  row("12", "Kepler Freight", { valuation: 21_300, trade_throughput: 620, systems_developed: 18,
    medals: [{ category: "systems_developed", gold: 0, silver: 0, bronze: 1 }] }),
  row("15", "Halcyon Drift [Bot 2]", { valuation: 12_050, trade_throughput: 310 }),
];
const op = (id: string, kind: OperationView["kind"], state_: OperationView["state"], progress: number, goal: number,
  credits: number, standing: number, joined = false): OperationView => ({ id, issuer: kind.kind === "survey_expedition" ? "survey_office" : "market",
  scope: { scope: "private", player: "12" }, kind, state: state_, progress, goal, stage: 0, reported_at: now - 90, offered_at: 8 * DAY + 62,
  starts_at: 8 * DAY + 62, expires_at: joined ? now + DAY - 600 : 9 * DAY, target_pos: { x: 0, y: 0 },
  reward: { credits, authority_standing: standing, captain_xp: 0, research_insight: 0 }, joined, board_day: 8 } as unknown as OperationView);
state.operations = [
  op("41", { kind: "survey_expedition", system: "s1" } as OperationView["kind"], "offered", 0, 1, 400, 2),
  op("42", { kind: "market_delivery", commodity: "provisions", units: 140 } as OperationView["kind"], "active", 60, 140, 1_120, 1, true),
  op("43", { kind: "market_delivery", commodity: "metallic_ore", units: 30 } as OperationView["kind"], "completed", 30, 30, 240, 1, true),
  op("44", { kind: "survey_expedition", system: "s2" } as OperationView["kind"], "completed", 1, 1, 400, 2, true),
];
state.board = { day: 8, next_at: 9 * DAY, done: 2, needed: 3, bonus_standing: 3, bonus_dossier: 0.05, bonus_paid: false };
// §standing-ladder (R3): a Trusted corporation part-way through its day.
state.charter = { standing: 131, max_standing: 200, full_standing: 100, status: "good_standing", title: "Good Standing",
  tariff_mult: 0.85, market_penalty_frac: 0, reinstate_cost_per_point: 2_000, grade: "trusted", grade_title: "Trusted",
  next_grade: { title: "Preferred", at: 150 }, earned_today: 5, earn_per_day: 8, shipment_cap: 500 } as typeof state.charter;
state.charterLadder = [["Good Standing", 100], ["Sanctioned", 100], ["Suspended", 60], ["Revoked", 20], ["Proscribed", -20]];
state.gradeLadder = [["Chartered", 100], ["Trusted", 125], ["Preferred", 150], ["Commissioned", 175]];
state.galaxy = { hub: { x: 0, y: 0 }, systems: [{ id: "s1", name: "Tarn", pos: { x: 9_000, y: 0 } },
  { id: "s2", name: "Sif", pos: { x: 0, y: 12_000 } }] } as unknown as typeof state.galaxy;
state.explorationSites = [];

const root = document.getElementById("deck-workspace-body")!;
const ctx = { state, send: (message: unknown) => console.log("send", message) } as unknown as CoreContext;
let route = { name: "rankings" } as DeckRoute;
// §privateer-contract: ?charter=1 pins the Authority's contract on the
// privateer on top of Operations, its Accept ringed as the guide does.
const charter = new URLSearchParams(location.search).has("charter");
if (charter) {
  state.operations.unshift({ ...op("50", { kind: "privateer_patrol", pos: { x: 6_600, y: 0 } } as unknown as OperationView["kind"],
    "offered", 0, 1, 8_000, 0), issuer: "authority", board_day: null, expires_at: Number.MAX_VALUE,
    briefing: { follow_up: "founder_privateer", title: "Eliminate Milo Quill", difficulty: "One raider", suitable_fleets: "One Interceptor",
      summary: "Milo Quill raided your colony. Last seen lying in wait nearby.", variant: 0 } } as unknown as OperationView);
  route = { name: "operations" } as DeckRoute;
}
const pages = new DeckStrategicRoutes(root, ctx, {
  go: (next: DeckRoute) => { route = next; pages.render(route, true); },
  back() {}, notice: (html: string) => console.log("notice", html), openBattleViewer() {}, selectFleets() {}, openWorld() {},
});
root.addEventListener("click", event => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) pages.handleAction(button, route);
});
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-fixture]")) {
  button.addEventListener("click", () => {
    const which = button.dataset.fixture!;
    route = (which === "card" ? { name: "rankings", query: { corp: "12" } } : { name: which }) as DeckRoute;
    pages.render(route, true);
  });
}
pages.render(route, true);
if (charter) coach("contract-accept", CONTRACT_COACH);
