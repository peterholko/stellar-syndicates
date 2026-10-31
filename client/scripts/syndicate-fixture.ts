// Isolated preview of the R5–R9 pages (Deck): the syndicate's projects and
// invites, Comms, the syndicate rankings and the welcome-back card. Served
// fixtures only, no socket, no game.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import type { RankingRow, SocialView, SyndicateView, SystemStateView, WelcomeBackView } from "../src/protocol";
import { DeckStrategicRoutes } from "../src/shell/deck/strategic";
import { DeckWelcome } from "../src/shell/deck/welcome";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

const DAY = 86_400;
const now = 3 * DAY + 3_600 * 5;
syncRenderClock(now);
state.simTime = now;
state.playerId = "12" as typeof state.playerId;
state.name = "Kepler Freight";
const wall = Math.floor(Date.now() / 1000);
state.galaxy = { hub: { x: 0, y: 0 }, systems: [{ id: "home", name: "Kepler", pos: { x: 0, y: 0 } },
  { id: "vega", name: "Vega", pos: { x: 9_000, y: 0 } }] } as unknown as typeof state.galaxy;
state.systems = [
  { id: "home", owner: "12", stockpile: [{ commodity: "electronics", units: 260 }, { commodity: "alloys", units: 90 }] },
  { id: "vega", owner: "12", stockpile: [{ commodity: "electronics", units: 75 }] },
] as unknown as SystemStateView[];
state.syndicate = {
  id: "4", name: "Long Haul Pact", founder: "12", is_founder: true, invited: [], my_role: "founder",
  members: [{ id: "12", name: "Kepler Freight", role: "founder" }, { id: "7", name: "Vega Mining Combine", role: "officer" },
    { id: "3", name: "Orion & Sons", role: "member" }],
  projects: {
    week: 1, week_ends_in: 3 * DAY + 3_600 * 19, rung: 2, progress: 340,
    rungs: [{ goods: "alloys", goal: 150, points: 1 }, { goods: "machinery", goal: 400, points: 2 },
      { goods: "electronics", goal: 900, points: 3 }, { goods: "polymers", goal: 1_800, points: 4 }, { goods: "provisions", goal: 3_200, points: 5 }],
    points: 4, week_points: 3,
    bonuses: [
      { bonus: "freight", level: 2, mine: 2, per_level: 0.06, next_cost: 10, next_in: null },
      { bonus: "research", level: 1, mine: 1, per_level: 0.04, next_cost: 6, next_in: null },
      { bonus: "migration", level: 0, mine: 0, per_level: 0.08, next_cost: 3, next_in: null },
    ],
    given: [["Kepler Freight", 640], ["Vega Mining Combine", 250]], medals: [1, 0, 1], may_raise: true,
  },
} as unknown as SyndicateView;
state.social = {
  channel: [
    { id: 1, author: "7", author_name: "Vega Mining Combine", text: "Semiconductor run leaves Vega at dawn.", at: wall - 5_400 },
    { id: 2, author: "12", author_name: "Kepler Freight", text: "I'll cover the rest of rung three.", at: wall - 1_200 },
    { id: 3, author: "3", author_name: "Orion & Sons", text: "Pirates near the hub lane, escort your freighters.", at: wall - 120 },
  ],
  inbox: [{ id: 9, from: "7", from_name: "Vega Mining Combine", to: "12", to_name: "Kepler Freight", subject: "Ore swap",
    body: "200 ferrite ore for 80 alloys?", at: wall - 3_000, read: false }],
  sent: [], unread: 1, muted: [],
  members: [{ player: "12", name: "Kepler Freight", online: true, last_seen: null },
    { player: "7", name: "Vega Mining Combine", online: true, last_seen: null },
    { player: "3", name: "Orion & Sons", online: false, last_seen: wall - 3 * 3_600 }],
  invites: [{ code: "K7QX2M9P", until: now + 2 * DAY + 3_600 * 11, near: "Arden" }],
} as unknown as SocialView;
const row = (id: string, name: string, valuation: number): RankingRow => ({ player_id: id, name, valuation, trade_throughput: 0,
  market_profit: 0, cargo_captured: 0, cargo_protected: 0, battle_efficiency: 0, battle_engagements: 0, battle_ranked: false,
  systems_developed: 0, intel_gathered: 0, recovery: 0, titles: [] } as RankingRow);
state.rankings = [row("7", "Vega Mining Combine", 48_200), row("12", "Kepler Freight", 21_300), row("3", "Orion & Sons", 19_900)];
state.syndicateRankings = [
  { id: "4", name: "Long Haul Pact", members: 3, valuation: 89_400, week_points: 3, levels: 3, medals: [1, 0, 1] },
  { id: "6", name: "Rim Accord", members: 2, valuation: 51_000, week_points: 6, levels: 5, medals: [0, 1, 0] },
];
const welcome: WelcomeBackView = {
  away: 9 * 3_600 + 20 * 60,
  happened: [
    { severity: "bad", text: "Blockade at Vega: a rival holds the lanes.", go: { kind: "system", id: "vega" } },
    { severity: "good", text: "Sold 1,240 ferrite ore, 300 alloys · +18,600 Cr", go: { kind: "market" } },
    { severity: "good", text: "Research reports ×2: Research complete: Hull Engineering I — its effect is live corporation-wide.", go: { kind: "research" } },
    { severity: "good", text: "Long Haul Pact: week 1 rung 2 done · +2 points.", go: { kind: "syndicate" } },
  ],
  waiting: [
    { severity: "info", text: "2 Signals waiting", go: { kind: "signals" } },
    { severity: "warn", text: "Academy idle: no programme", go: { kind: "research" } },
    { severity: "info", text: "1 unread mail", go: { kind: "comms" } },
  ],
  next: [
    { label: "Defend Vega", go: { kind: "system", id: "vega" } },
    { label: "Answer a Signal", go: { kind: "signals" } },
    { label: "Pick a programme", go: { kind: "research" } },
  ],
};
state.awaySet = true;
state.awaySince = now - welcome.away;
state.welcome = welcome;

const root = document.getElementById("deck-workspace-body")!;
const ctx = { state, send: (message: unknown) => console.log("send", message) } as unknown as CoreContext;
let route = { name: "syndicate" } as DeckRoute;
const pages = new DeckStrategicRoutes(root, ctx, {
  go: (next: DeckRoute) => { route = next; pages.render(route, true); },
  back() {}, notice: (html: string) => console.log("notice", html), openBattleViewer() {}, selectFleets() {}, openWorld() {},
} as unknown as ConstructorParameters<typeof DeckStrategicRoutes>[2]);
root.addEventListener("click", event => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) pages.handleAction(button, route);
});
pages.render(route, true);
const card = document.getElementById("deck-welcome")!;
const welcomeCard = new DeckWelcome(card, ctx, { runFounding() {}, openLog() {}, go: (next) => console.log("go", next) }, new AbortController().signal);
document.querySelectorAll<HTMLButtonElement>("[data-fixture]").forEach(button => button.addEventListener("click", () => {
  const name = button.dataset.fixture!;
  if (name === "welcome") { card.hidden = false; welcomeCard.render(); return; }
  route = { name } as DeckRoute;
  pages.render(route, true);
}));
