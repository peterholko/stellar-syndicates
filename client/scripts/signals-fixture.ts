// Isolated preview of the Signals pop-out (Deck): served fixtures only, no
// socket, no game.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import "../src/styles/mobile.css";
import type { CaptainRosterView, GhostView, SignalsView } from "../src/protocol";
import { DeckSignalsRoutes } from "../src/shell/deck/signals";
import { MobileSignals } from "../src/shell/mobile/signals";
import type { SheetStack } from "../src/shell/mobile/sheets";
import type { DeckRoute } from "../src/shell/deck/router";
import type { CoreContext } from "../src/shell/types";
import { state, syncRenderClock } from "../src/state";

const now = 2 * 86_400 + 3_600 * 5;
syncRenderClock(now);
state.simTime = now;
state.playerId = "12" as typeof state.playerId;
state.commandCenter = { x: 0, y: 0 };
const signals: SignalsView = {
  next_at: now + 5 * 3_600 + 40 * 60, open_max: 5, first_ten: 4, first_ten_next: "a research dossier",
  signals: [
    { id: "s9", number: 12, kind: "ghost_ship", hazardous: true, pos: { x: 15_600, y: 9_800 }, detected_at: now - 3 * 3_600, xp: 90 },
    { id: "s10", number: 13, kind: "cargo_pod", hazardous: false, pos: { x: -8_400, y: 3_700 }, detected_at: now - 3_700, xp: 45 },
    { id: "s11", number: 14, kind: "distress_beacon", hazardous: false, pos: { x: 2_000, y: -14_800 }, detected_at: now - 2_400, xp: 45 },
    { id: "s8", number: 11, kind: "drifting_wreck", hazardous: false, pos: { x: -12_000, y: -9_000 }, detected_at: now - 9_000, xp: 45,
      report: { at: now - 120, fleet: "f1", captain: "Mara Venn", xp: 45,
        finds: [{ kind: "module_aboard", module: "point_defense_screen" }, { kind: "cache", site: "c1", goods: [["alloys", 34], ["machinery", 8]] }] } },
  ],
};
state.signals = signals;
const captain = (level: number, fieldcraft: number, name: string, portrait: string) => ({
  id: 0, name, portrait, level, title: level >= 3 ? "commander" : "lieutenant_commander", portrait_age: "young", command_capacity: 8,
  xp: 120, next_level_xp: 300, unspent: 0, attributes: { command: 2, navigation: 1, fieldcraft, logistics: 1 },
});
const ghost = (id: string, kind: GhostView["kind"], pos: { x: number; y: number }, extra: Partial<GhostView>): GhostView => ({
  id, owner: "12", kind, pos, vel: { x: 0, y: 0 }, age: 0, own: true, jumped: false, route: null, passengers: {}, cargo: null,
  count_class: "one", composition: [{ kind, count: 1 }], signature: null, posture: null, supplied: true, ...extra,
} as unknown as GhostView);
state.ghosts = [
  ghost("f1", "raider", { x: 0, y: 0 }, { docked: "home", captain: captain(2, 1, "Mara Venn", "mara_venn") as GhostView["captain"] }),
  ghost("f2", "corvette", { x: 0, y: 0 }, { docked: "home", captain: captain(3, 1, "Ada Voss", "astrid_nystrom") as GhostView["captain"] }),
  ghost("f3", "raider", { x: 1_500, y: -9_000 }, { captain: captain(2, 2, "Jun Arclight", "jun_arclight") as GhostView["captain"],
    expedition: { site: "s11", task: "signal" } }),
];
state.captains = [
  { id: 0, name: "Mara Venn", portrait: "mara_venn", assigned_fleet: "f1", stationed_system: null, recovering_until: null, loss_fate: null, report: null },
  { id: 1, name: "Ada Voss", portrait: "astrid_nystrom", assigned_fleet: "f2", stationed_system: null, recovering_until: null, loss_fate: null, report: null,
    hurt_until: now + 3 * 3_600 + 12 * 60 },
  { id: 3, name: "Jun Arclight", portrait: "jun_arclight", assigned_fleet: "f3", stationed_system: null, recovering_until: null, loss_fate: null, report: null },
  { id: 2, name: "Daichi Mori", portrait: "daichi_mori", assigned_fleet: null, stationed_system: "home", recovering_until: null, loss_fate: null,
    report: captain(2, 2, "Daichi Mori", "daichi_mori") as CaptainRosterView["report"] },
] as CaptainRosterView[];
state.systems = [{ id: "home", bodies: [{ structures: { command_center: 1 } }] }] as unknown as typeof state.systems;
state.explorationSites = [];

const root = document.getElementById("deck-workspace-body")!;
const ctx = { state, send: (message: unknown) => console.log("send", message),
  renderer: { centerOnWorld: (pos: unknown) => console.log("center", pos) } } as unknown as CoreContext;
let route = { name: "signals", query: { signal: "s9" } } as DeckRoute;
const pages = new DeckSignalsRoutes(root, ctx, {
  go: (next: DeckRoute) => console.log("go", next), notice: (html: string) => console.log("notice", html),
});
root.addEventListener("click", event => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (button) pages.handleAction(button, route);
});
const pick: Record<string, string> = { ghost: "s9", beacon: "s11", wreck: "s8" };
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-fixture]")) {
  button.addEventListener("click", () => {
    route = { name: "signals", query: { signal: pick[button.dataset.fixture!] } } as DeckRoute;
    pages.render(route, true);
  });
}
pages.render(route, true);
setInterval(() => pages.render(route), 1_000);

// The mobile sheet: the same state in a phone-width sheet body.
if (location.search.includes("mobile")) {
  document.querySelector<HTMLElement>(".deck")!.style.display = "none";
  const body = document.getElementById("m-preview")!;
  body.style.display = "block";
  const entry = { id: "signals" as const };
  const sheets = { refresh: () => draw() } as unknown as SheetStack;
  const mobile = new MobileSignals(ctx, sheets, { notice: (html) => console.log("notice", html), focus: (pos) => console.log("focus", pos) });
  const draw = () => { const view = mobile.render(entry)!; body.innerHTML = `<h2 style="margin:0 0 8px">${view.title}</h2><small style="color:#8493a8">${view.eyebrow}</small>${view.html}`; };
  body.addEventListener("click", (event) => { if (mobile.handleClick(event)) draw(); });
  draw();
}
