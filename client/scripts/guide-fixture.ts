// Isolated preview of the founding guide card and its Charter Advisor
// (§charter-advisor): the real DeckFoundingGuide over fixture state, one card
// per situation, and the stall nudge's bubble. No socket, no game.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import "../src/styles/coach.css";
import { state } from "../src/state";
import { DeckFoundingGuide } from "../src/shell/deck/founding-guide";
import { coach } from "../src/shell/coach";
import { actionAdvice, ADVISORS, advisorTitle } from "../src/core/derive/advisor";
import { foundingPlan } from "../src/core/derive/founding";
import type { CoreContext } from "../src/shell/types";
import type { FoundingStage, FoundingView, SystemStateView } from "../src/protocol";

const params = new URLSearchParams(location.search);
const officer = Math.max(0, Math.min(2, Number(params.get("advisor") ?? 1)));
const recipe = (key: string, costs: [string, number][]) => ({ key, costs: costs.map(([commodity, units]) => ({ commodity, units })) });

Object.assign(state, {
  playerId: "1", simTime: 600, commandCenter: { x: 0, y: 0 }, ghosts: [], captains: [], pendingOrders: new Map(),
  storyTold: new Set(["free_trader", "pirates"]), homeOpened: true, homeWorldOpened: true, merchant: null,
  research: { programmes: [], active: null },
  galaxy: { instance_id: "fixture", hub: { x: 70_000, y: 0 }, systems: [{ id: "home", name: "Freya", pos: { x: 0, y: 0 } }],
    build_options: [recipe("power_plant", [["alloys", 30], ["machinery", 12], ["electronics", 8]]),
      recipe("bioharvester", [["alloys", 25], ["machinery", 12]]), recipe("mining_complex", [["alloys", 25], ["machinery", 12]])] },
});

const founding = (stage: FoundingStage): FoundingView => ({
  stage, local_opening: true, protected: true, protection_min_until: 0, protection_max_until: 90_000, bounty_received: false,
  expansion_unlocked: stage === "expand_mining", land_grant_open: stage === "expand_mining", opening_exports: [], survey_candidates: [],
  advisor: officer,
} as unknown as FoundingView);
const landing = { id: 1, name: "Freya I", kind: "terrestrial", resource_slots: 3,
  deposits: [{ resource: "biomass", richness: 1 }, { resource: "metallic_ore", richness: 0.8 }], structures: { command_center: 1, habitat: 1 } };
const rock = { id: 3, name: "Freya III", kind: "barren", resource_slots: 1, deposits: [{ resource: "metallic_ore", richness: 0.9 }], structures: {} };
const home = (bodies: unknown[], extra: Partial<SystemStateView> = {}) => ({ id: "home", owner: "1", builds: [], assignments: [], bodies,
  storage_used: 320, storage_cap: 1000, stockpile: [], workforce: { units: 6000, posted: 2000 }, converters: [], ...extra }) as unknown as SystemStateView;

const ctx = { state, renderer: {}, intent: {}, send() {}, net: {} } as unknown as CoreContext;
const hooks = { go() {}, openWorld() {}, focusFleet() {}, flyIntoSystem() {} };
const cases: [string, FoundingStage, SystemStateView, boolean?][] = [
  ["The advisor speaks in the step's why", "power_shipyard", home([landing])],
  ["A substep: staffing", "build_bioharvester", home([{ ...landing, structures: { ...landing.structures, power_plant: 1, bioharvester: 1 } }])],
  ["A live reason outranks the advisor", "expand_mining", home([{ ...landing, structures: { ...landing.structures, smelter: 1 } }, rock],
    { converters: [{ structure: "smelter", status: "no_inputs" }] } as unknown as Partial<SystemStateView>)],
  ["A blocker keeps its warning", "expand_mining", home([landing])],
  ["Minimized", "power_shipyard", home([landing]), true],
  ["Watch the pirate", "build_smelter", home([{ ...landing, structures: { ...landing.structures, smelter: 1 } }])],
];
const main = document.getElementById("fixture")!;
for (const [title, stage, system, minimized] of cases) {
  state.founding = founding(stage);
  // §watch-the-pirate: the raider over the colony, the Smelter's crew waiting.
  if (title === "Watch the pirate") Object.assign(state.founding, { raid_seen: true, raid_over: true });
  state.systems = [system];
  const cell = document.createElement("section");
  cell.innerHTML = `<h2>${title}</h2><div class="slot"><aside class="deck-founding"></aside></div>`;
  main.append(cell);
  const root = cell.querySelector<HTMLElement>(".deck-founding")!;
  try { localStorage.setItem("stellar-syndicates:founding-guide-minimized", minimized ? "1" : "0"); } catch { /* fixture */ }
  new DeckFoundingGuide(root, ctx, hooks).render(true);
}
// The stall nudge on the first card: the advisor's face beside the advice.
state.founding = founding("power_shipyard");
state.systems = [cases[0][2]];
const advisor = ADVISORS[officer];
const line = actionAdvice(foundingPlan(state, cases[0][2])?.primary, state.founding);
if (params.get("nudge") !== "0" && line) {
  // The coach looks for its target each frame; re-aim it until it shows.
  setTimeout(() => coach("founding-next", line, "right", { src: advisor.face, alt: advisorTitle(advisor) }), 300);
}
try { localStorage.removeItem("stellar-syndicates:founding-guide-minimized"); } catch { /* fixture */ }
