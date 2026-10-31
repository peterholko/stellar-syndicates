// Isolated preview of the Log (§log, Oct 28 2026): Inbox and History over a
// canned v71 journal, Deck and phone sheet. No socket, no game.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import "../src/styles/mobile.css";
import type { BattleRecordView, TimelineEntry } from "../src/protocol";
import { DeckLogRoutes } from "../src/shell/deck/log";
import { MobileLog } from "../src/shell/mobile/log";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

const NOW = 5 * 86_400 + 14 * 3_600;
syncRenderClock(NOW);
state.simTime = NOW;
state.playerId = "me" as typeof state.playerId;
state.awaySince = NOW - 4 * 3_600;
state.galaxy = {
  pirate_id: "pirates", siege_secs: 21_600, node_awakening_time: 0,
  systems: [{ id: "freya", name: "Freya", pos: { x: 0, y: 0 } }, { id: "nergal", name: "Nergal", pos: { x: 9_000, y: 0 } }, { id: "dabih", name: "Dabih", pos: { x: 40_000, y: 0 } }],
} as unknown as typeof state.galaxy;
state.systems = [
  { id: "freya", owner: "me", storage_cap: 1_000, storage_used: 930, stockpile: [{ commodity: "metallic_ore", units: 620 }], bodies: [], builds: [] },
  { id: "nergal", owner: "me", storage_cap: 800, storage_used: 200, stockpile: [], bodies: [], builds: [] },
] as unknown as typeof state.systems;
state.diplomacy = { relations: [], incoming: [{ id: 7, name: "Vega Mining Combine", treaty: "non_aggression", expires_at: NOW + 5_400 }] } as unknown as typeof state.diplomacy;
state.syndicateInvites = [{ id: "s1", name: "Long Haul" }] as typeof state.syndicateInvites;

let id = 0;
const line = (ago: number, severity: TimelineEntry["severity"], text: string, extra: Partial<TimelineEntry> = {}): TimelineEntry =>
  ({ id: ++id, at_time: NOW - ago, severity, text, ...extra });
const raid = (system: string) => ({ kind: "raid" as const, group: `raid:${system}`, refs: { system, fleet: "pack" } });
const blockade = { kind: "blockade" as const, group: "blockade:nergal", refs: { system: "nergal" } };
state.timeline = [
  line(30 * 3_600, "warn", "Nergal contested — pirates against your defenses.", { ...blockade, phase: "contested" }),
  line(29 * 3_600, "bad", "Nergal BLOCKADED — logistics cut.", { ...blockade, phase: "held" }),
  line(28 * 3_600, "good", "Blockade of Nergal LIFTED.", { ...blockade, phase: "lifted" }),
  line(3 * 3_600, "info", "Construction complete · Mining Complex II · Freya.", { kind: "build", refs: { system: "freya" } }),
  line(2 * 3_600 + 600, "bad", "Pirate raid inbound · Freya · 3 raiders · ETA ~4 min.", { ...raid("freya"), phase: "inbound", data: { ships: 3, eta: NOW - 2 * 3_600 - 360 } }),
  line(2 * 3_600 + 300, "bad", "Pirates raiding Freya — they take up to 10% of each good, 60 units in all.", { ...raid("freya"), phase: "raiding" }),
  line(2 * 3_600, "bad", "Raided Freya: pirates took 46 Ferrite Ore, 14 Provisions. Their haul can still be intercepted.",
    { ...raid("freya"), phase: "ended", data: { goods: { metallic_ore: -46, provisions: -14 }, ships: 3, duration: 300 } }),
  line(5_400, "good", "Sold 120 ferrite ore at the hub for 8.22 ea.", { kind: "trade", data: { goods: { metallic_ore: -120 }, credits: 986 } }),
  line(5_100, "good", "Sold 80 ferrite ore at the hub for 8.10 ea.", { kind: "trade", data: { goods: { metallic_ore: -80 }, credits: 648 } }),
  line(4_800, "good", "Sold 60 ferrite ore at the hub for 8.05 ea.", { kind: "trade", data: { goods: { metallic_ore: -60 }, credits: 483 } }),
  line(3_000, "info", "Research complete · Deep Core Drilling.", { kind: "research" }),
  line(2_400, "warn", "Order refused: that fleet can't do this work where it is (check its hold, fuel and the site). Its previous instructions remain in force.",
    { kind: "refusal", refs: { fleet: "barge" } }),
  line(1_800, "info", "Escort the Freight · available.", { kind: "operation", group: "operation:1", phase: "offered" }),
  line(90, "bad", "Pirate raid inbound · Nergal · 2 raiders · ETA ~4 min.", { ...raid("nergal"), phase: "inbound", data: { ships: 2, eta: NOW + 150 } }),
];
state.welcome = {
  away: 4 * 3_600, happened: [{ severity: "bad", text: "Raids ×1: lost 60 in all." }, { severity: "good", text: "Sales ×3: +2,117 Cr." }], waiting: [], next: [],
} as unknown as typeof state.welcome;
const side = (corp: string) => ({ corp, posture: null, platform_tiers: 0, initial: [] });
state.battleRecords = [
  { id: "b1", pos: { x: 0, y: 0 }, system: "freya", started_at: NOW - 2 * 3_600 - 280, raid: true, fidelity: "participant", own_side: 1,
    sides: [side("pirates"), side("me")], rounds: [], light_frontier_tick: 0, outcome: "attacker_destroyed" },
] as unknown as BattleRecordView[];

const ctx = { state, send: (msg: unknown) => console.log("send", JSON.stringify(msg)), intent: { beginFleetCommand: (c: unknown) => console.log("fleet", c) } } as unknown as CoreContext;
const root = document.getElementById("deck-workspace-body")!;
let route = { name: "log" } as DeckRoute;
const log = new DeckLogRoutes(root, ctx, {
  go: (next) => console.log("go", JSON.stringify(next)), focusSystem: (sys) => console.log("system", sys),
  focusFleet: (fleet) => console.log("fleet", fleet), notice: (text) => console.log("notice", text),
});
root.addEventListener("click", (event) => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) log.handleAction(button, route);
});
const tab = new URLSearchParams(location.search).get("tab");
if (tab) route = { name: "log", query: { tab } } as DeckRoute;
log.render(route, true);

if (location.search.includes("mobile")) {
  document.querySelector<HTMLElement>(".deck")!.style.display = "none";
  const shell = document.getElementById("m-preview")!;
  shell.style.display = "block";
  const body = shell.querySelector<HTMLElement>(".m-sheet__body")!;
  const entry = { id: "log" as const, props: tab ? { tab } : undefined };
  const mobile = new MobileLog(ctx, {
    openSheet: (next) => console.log("sheet", JSON.stringify(next)), focusSystem: (sys) => console.log("system", sys),
    focusFleet: (fleet) => console.log("fleet", fleet), notice: (html) => console.log("notice", html), refresh: () => draw(),
  });
  const draw = () => { const view = mobile.render(entry); body.innerHTML = `<h2 style="margin:0 0 8px">${view.title}</h2>${view.html}`; };
  body.addEventListener("click", (event) => {
    const button = (event.target as Element).closest<HTMLElement>("[data-mobile-act]");
    if (button) mobile.handleAction(button);
  });
  draw();
}
