// Isolated preview of the charter's story pop-ups: no socket, no game state.
import "../src/styles/tokens.css";
import "../src/styles/settlement.css";
import { chapterEvent, crackdownEvent, decreeEvent, endingEvent, exposedEvent, FOLLOW_UP_MS, freeTraderEvent, journeyEvent, piratesEvent,
  relicsEvent, StoryEventUI, vendettaEvent, type StoryEvent } from "../src/story-events";
import { transmission, type VendettaBeat } from "../src/core/derive/vendetta";
import type { PirateFamilySlug } from "../src/protocol";
import type { ViewState } from "../src/state";

const ui = new StoryEventUI();
// §vendetta: a dealt family over the colony of Alphard (`?family=` or the picker).
const family = () => (document.getElementById("fixture-family") as HTMLSelectElement | null)?.value as PirateFamilySlug ?? "showboat";
const st = () => ({ playerId: "1", commandCenter: { x: 0, y: 0 }, systems: [{ id: "home", owner: "1" }],
  galaxy: { systems: [{ id: "home", name: "Alphard", pos: { x: 0, y: 0 } }] },
  founding: { pirates: { family: family(), surname: "Marrow", nephew: "Jace Marrow", uncle: "Mordecai Marrow", brother: "Rook Marrow" } } }) as unknown as ViewState;
const lines: Record<Exclude<VendettaBeat, "raid">, string> = {
  kill: "Jace Marrow's last words named his uncle, Mordecai Marrow.",
  den: "Among the den's stores: a message from Mordecai Marrow to Jace Marrow.",
  sworn: "Mordecai Marrow swore revenge for Jace Marrow.",
  arrival: "Mordecai Marrow is over Alphard.",
  won: "Mordecai Marrow is dead.",
  lost: "Mordecai Marrow got away with 12 Alloys and 4 Machinery.",
};
const raid = () => piratesEvent({ semiconductors: 40, armaments: 15 }, transmission("raid", st()));
// In the game the trader shows on the server's docking line, no sooner than
// FOLLOW_UP_MS after the last pop-up closed; here it docks at once.
const show = () => {
  ui.show(raid());
  ui.showAfterGap(freeTraderEvent(), FOLLOW_UP_MS);
};
const picked = new URLSearchParams(location.search).get("family");
if (picked) (document.getElementById("fixture-family") as HTMLSelectElement).value = picked;
document.getElementById("fixture-pirates")?.addEventListener("click", show);
document.getElementById("fixture-trader")?.addEventListener("click", () => ui.show(freeTraderEvent()));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-beat]")) {
  const beat = button.dataset.beat as Exclude<VendettaBeat, "raid">;
  button.addEventListener("click", () => ui.show(vendettaEvent(beat, lines[beat], transmission(beat, st()))));
}
// §season: the GNN stories by painting (`?story=` or the buttons).
const season: Record<string, () => StoryEvent> = {
  // §chapters: the charter's painted chapter openings.
  "chapter-pirates": () => chapterEvent("pirates", "Jace Marrow"),
  "chapter-expansion": () => chapterEvent("expansion"),
  "chapter-reach": () => chapterEvent("reach"),
  "decree-levies": () => decreeEvent("levies"),
  "decree-inspections": () => decreeEvent("inspections"),
  "decree-martial": () => decreeEvent("martial"),
  "sabotage-at-the-hub": () => crackdownEvent("sabotage", 1),
  "convoy-massacre": () => crackdownEvent("massacre", 2),
  "quarantine": () => crackdownEvent("quarantine", 3),
  "relic-seizure": () => crackdownEvent("relic_seizure", 4),
  "keystone-crackdown": () => crackdownEvent("keystone", 5),
  "false-flag-exposed": () => exposedEvent("False flag exposed · Convoy massacre · by Vega Freight · crackdown lifted.", 6),
  "relics-awaken": () => relicsEvent(),
  "sealed-world-found": () => journeyEvent("found", "Sealed world found · Tau Ceti · its Keystone can close every gate.", 7),
  "authority-spy": () => journeyEvent("spy", "An Authority spy · the Authority will know at 14:20 · then war.", 8),
  "authority-war": () => journeyEvent("war", "War with the Authority · no Hub, no Authority freight · bounties on your hulls.", 9),
  "seal-fires": () => endingEvent({ kind: "sovereign", at: 1, holder: "Vega Freight", system: "K", winners: ["Vega Freight"], won: true }),
  "charter-holds": () => endingEvent({ kind: "charter_holds", at: 1, holder: null, system: null, winners: ["Vega Freight", "Orion Mining"], won: false }),
};
const bar = document.getElementById("fixture-season");
for (const [slug, story] of Object.entries(season)) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = slug;
  button.addEventListener("click", () => ui.show(story()));
  bar?.append(" ", button);
}
const params = new URLSearchParams(location.search);
const beat = params.get("beat") as Exclude<VendettaBeat, "raid"> | null;
const story = params.get("story");
if (story && season[story]) ui.show(season[story]());
else if (beat && lines[beat]) ui.show(vendettaEvent(beat, lines[beat], transmission(beat, st())));
else show();
