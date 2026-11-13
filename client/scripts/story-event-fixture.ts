// Isolated preview of the charter's story pop-ups: no socket, no game state.
import "../src/styles/tokens.css";
import "../src/styles/settlement.css";
import { chapterEvent, crackdownEvent, crateEvent, decreeEvent, denContractEvent, denLootEvent, privateerContractEvent, signalReportEvent, endingEvent, exposedEvent, FOLLOW_UP_MS, freeTraderEvent, journeyEvent, laserBatteryEvent, piratesEvent,
  relicsEvent, StoryEventUI, vendettaEvent, type StoryEvent } from "../src/story-events";
import { ADVISORS, advisorEvent, advisorSpeaker, cardLine } from "../src/core/derive/advisor";
import { TRADERS } from "../src/core/derive/trader";
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
  ui.showAfterGap(freeTraderEvent(TRADERS.teo), FOLLOW_UP_MS);
};
const picked = new URLSearchParams(location.search).get("family");
if (picked) (document.getElementById("fixture-family") as HTMLSelectElement).value = picked;
document.getElementById("fixture-pirates")?.addEventListener("click", show);
document.getElementById("fixture-trader")?.addEventListener("click", () => ui.show(freeTraderEvent(TRADERS.teo)));
for (const button of document.querySelectorAll<HTMLButtonElement>("[data-beat]")) {
  const beat = button.dataset.beat as Exclude<VendettaBeat, "raid">;
  button.addEventListener("click", () => ui.show(vendettaEvent(beat, lines[beat], transmission(beat, st()))));
}
// §season: the GNN stories by painting (`?story=` or the buttons).
const season: Record<string, () => StoryEvent> = {
  "laser-battery": () => laserBatteryEvent(),
  "den-loot": () => denLootEvent({ machinery: 37, armaments: 35, electronics: 15, zephyr_drive: 7, fission_reactor: 6 }),
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
// §charter-advisor: the officer's moments, and a chapter and a crate in their voice (`?advisor=0|1|2`).
const officer = ADVISORS[Math.max(0, Math.min(2, Number(new URLSearchParams(location.search).get("advisor") ?? 0)))];
const voice = (line: string) => advisorSpeaker(officer, line);
const names = { privateer: "Jace Marrow", den: "the Marrow den", uncle: "Mordecai Marrow" };
Object.assign(season, {
  "advisor-intro": () => advisorEvent("intro", officer),
  "advisor-raid": () => advisorEvent("raid", officer),
  "advisor-blocked": () => advisorEvent("blocked", officer),
  "advisor-goodbye": () => advisorEvent("goodbye", officer),
  "advisor-left": () => advisorEvent("left", officer),
  // §freighter-lost: a freighter destroyed, haul and all.
  "advisor-freighter-lost": () => advisorEvent("freighter_lost", officer),
  "advisor-laser-battery": () => laserBatteryEvent(voice(cardLine(officer, "laser_battery"))),
  "advisor-free-trader": () => freeTraderEvent(TRADERS.ines, voice(cardLine(officer, "free_trader", { trader: "Ines Varga's Clean Slate" }))),
  "advisor-chapter-pirates": () => chapterEvent("pirates", "Jace Marrow", null, voice, cardLine(officer, "chapter_pirates", { privateer: "Jace Marrow" })),
  // §privateer-contract: the advisor relays the Authority's contract on him.
  "advisor-privateer-contract": () => privateerContractEvent("Eliminate Milo Quill", "8000 Cr",
    voice(cardLine(officer, "privateer_contract", { privateer: "Milo Quill" })), () => {}),
  "privateer-contract-sealed": () => privateerContractEvent("Eliminate Milo Quill", "8000 Cr", null, () => {}),
  // §signal-report: a sweep's report, as it reaches command.
  ...Object.fromEntries(([
    ["signal-wreck", "drifting_wreck", false, [{ kind: "cargo_aboard", goods: [["alloys", 40], ["machinery", 12]] }, { kind: "module_aboard", module: "recon_suite" }], 45, null],
    ["signal-ore", "ore_fragment", false, [{ kind: "cache", site: "x", goods: [["titanium_ore", 120]] }], 30, null],
    ["signal-stash-hurt", "pirate_stash", true, [{ kind: "credits", credits: 2400 }, { kind: "cargo_aboard", goods: [["armaments", 15]] }], 90, 14_400],
    ["signal-beacon", "distress_beacon", true, [{ kind: "specialist", specialist: "geologist" }, { kind: "captain", name: "Ilya Brand" }], 80, null],
    ["signal-courier", "authority_courier", false, [{ kind: "standing", credits: 1500, standing: 3 }], 30, null],
    ["signal-ghost-empty", "sensor_ghost", false, [], 20, null],
  ] as const).map(([key, kind, hazardous, finds, xp, hurt]) => [key, () => signalReportEvent({ id: key, number: 4, kind, hazardous,
    pos: { x: 0, y: 0 }, detected_at: 0, xp, report: { at: 0, fleet: "f", captain: "Mara Vosk", xp, finds: finds as never, hurt_until: hurt ?? undefined } },
    { ship: "Interceptor", now: 0, programmeName: () => "Precursor Mechanics", showCache: () => console.log("show the cache") })])),
  // §contract-reward: the den's contract, its reward a crate tile.
  "den-contract": () => denContractEvent("Eliminate the Varga den", "8000 Cr", "Tomas Varga", "Thuban"),
  "advisor-crate-vendetta": () => crateEvent({ id: "vendetta", released: true, goods: { alloys: 120, machinery: 40, polymers: 30 } },
    names, () => ({ used: 640, cap: 1000 }), () => {}, () => true, voice),
});
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
