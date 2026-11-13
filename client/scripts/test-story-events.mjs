// §charter-v5: the charter's story turns pop up as illustrated events; the
// founding guide keeps to the next action. No game socket.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
function compile(text, globals = {}, deps = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...globals });
  return exports;
}
// §vendetta: the session reads the pirate family's lines through it.
const vendetta = compile(source("core/derive/vendetta.ts"));

// A fresh raid report becomes a story event, not a toast, and never replays.
const season = compile(source("core/derive/season.ts"));
const { applyServerMessage } = compile(source("core/session.ts"), {}, { "./derive/vendetta": vendetta, "./derive/season": season });
const state = { timeline: [], awaySet: false };
const receipt = entries => applyServerMessage({ type: "Timeline", entries, away_since: 0 }, state);
const line = { at_time: 120, severity: "bad", text: "Pirates raided Freya and took 40 semiconductors and 15 armaments." };
assert.doesNotMatch(line.text, /trader/i, "the raid's line says nothing of the trader");
receipt([]);
const events = receipt([line]);
assert.deepEqual(JSON.parse(JSON.stringify(events.find(e => e.kind === "StoryEvent"))),
  { kind: "StoryEvent", story: "pirates", goods: { electronics: 40, armaments: 15 } }, "a line without goods: the two it names");
// §raid-losses: the entry's goods (negative is lost) carry every good taken, engines too.
const withGoods = { at_time: 121, severity: "bad", text: "Pirates raided Freya and took 1 fission reactor, 1 zephyr drive and 20 semiconductors.",
  data: { goods: { fission_reactor: -1, zephyr_drive: -1, electronics: -20 } } };
assert.deepEqual(JSON.parse(JSON.stringify(receipt([line, withGoods]).find(e => e.kind === "StoryEvent"))),
  { kind: "StoryEvent", story: "pirates", goods: { fission_reactor: 1, zephyr_drive: 1, electronics: 20 } });
assert.ok(!events.some(e => e.kind === "CharterMilestone"), "the pop-up replaces the toast");
assert.ok(!receipt([line]).some(e => e.kind === "StoryEvent"), "an old line never pops up again");
// The trader is news only when the server says it docked.
const docking = { at_time: 150, severity: "good", text: "A free trader docked at Freya." };
assert.deepEqual({ ...receipt([line, docking]).find(e => e.kind === "StoryEvent") }, { kind: "StoryEvent", story: "free_trader" });
assert.ok(!receipt([line, docking]).some(e => e.kind === "StoryEvent"), "and only once");
// §den-contract (Nov 5 2026): the Authority's contract on the den, with the kill's report.
const contractLine = { at_time: 160, severity: "info", text: "Authority contract · Eliminate the Corvane den · 3500 Cr." };
assert.deepEqual({ ...receipt([line, docking, contractLine]).find(e => e.kind === "StoryEvent") },
  { kind: "StoryEvent", story: "den_contract", title: "Eliminate the Corvane den", reward: "3500 Cr" });
// §privateer-contract (Nov 9 2026): the Authority's contract on the privateer, offered as the hunt begins.
const offerLine = { at_time: 170, severity: "info", text: "Authority contract offered · Eliminate Milo Quill · 8000 Cr." };
assert.deepEqual({ ...receipt([line, docking, contractLine, offerLine]).find(e => e.kind === "StoryEvent") },
  { kind: "StoryEvent", story: "privateer_contract", title: "Eliminate Milo Quill", reward: "8000 Cr" }, "its offer, not the den's");
// §log (v71): a home raid's start and its take toast from the entry's kind
// and phase, each once; the shell words them (core/derive/history logToast).
const homeRaid = { kind: "raid", group: "raid:nergal", refs: { system: "nergal" } };
const raiding = { ...homeRaid, phase: "raiding", at_time: 160, severity: "bad", text: "Pirates raiding Nergal — they take up to 10% of each good, 60 units in all." };
const took = { ...homeRaid, phase: "ended", at_time: 200, severity: "bad", text: "Raided Nergal: pirates took 46 Ferrite Ore, 14 Provisions. Their haul can still be intercepted.",
  data: { goods: { metallic_ore: -46, provisions: -14 } } };
const empty = { ...homeRaid, phase: "ended", at_time: 210, severity: "good", text: "Pirates left Nergal empty-handed." };
const toastOf = entries => [...receipt(entries).filter(e => e.kind === "LogToast").map(e => e.entry.text)];
assert.deepEqual(toastOf([line, docking, raiding]), [raiding.text]);
assert.deepEqual(toastOf([line, docking, raiding, took]), [took.text]);
assert.deepEqual(toastOf([line, docking, raiding, took, empty]), [empty.text]);
assert.deepEqual(toastOf([line, docking, raiding, took, empty]), [], "each line toasts once");
assert.deepEqual(toastOf([line, docking, raiding, took, empty, { ...docking, at_time: 220 }]), [], "only raid lines toast");
// A session that opens on old lines was told them then.
const later = { timeline: [], awaySet: false, storyTold: new Set() };
const resumed = applyServerMessage({ type: "Timeline", entries: [line, docking], away_since: 0 }, later);
assert.ok(!resumed.some(e => e.kind === "StoryEvent"), "no pop-up for old news");
assert.ok(later.storyTold.has("free_trader") && later.storyTold.has("pirates"), "nothing withheld either");
// §season: the GNN's "The relics stir" pops up when it is first heard; news
// heard before this session was told then.
const stir = { id: 7, heard_at: 500, source: "gnn", kind: "relic", headline: "The relics stir: 3 resonators wake", about_you: false };
const seasonOf = news => ({ maturity: 0.4, grip: "levies", census_at: 1, news, ending: null });
const storiesOf = (st, news) => [...applyServerMessage({ type: "Sections", season: seasonOf(news) }, st)]
  .filter(e => e.kind === "StoryEvent").map(e => e.story);
assert.deepEqual(storiesOf({ season: null }, [stir]), [], "heard before this session");
const listening = { season: seasonOf([]) };
assert.deepEqual(storiesOf(listening, [{ ...stir, id: 6, headline: "2 more relics wake" }]), [], "only the first waking");
assert.deepEqual(storiesOf(listening, [stir]), ["relics"], "heard now: it pops up");
assert.deepEqual(storiesOf(listening, [stir]), [], "once");
// §chapters (Oct 29 2026): the Pirates and Reach chapters open painted.
// §three-stipends (Nov 6 2026): as the served stage reaches their steps (11
// and 24 since the Smelter's step), not on the "Shipyard staffed" and
// "Nearby system charted" stipends, which are gone.
const sessionSource = source("core/session.ts");
const step = name => Number(new RegExp(`\\b${name}: (\\d+)`).exec(source("core/derive/founding.ts"))[1]);
assert.deepEqual([step("build_interceptor"), step("build_barge")], [11, 24]);
const viewing = compile(sessionSource, {}, { "./derive/vendetta": vendetta, "./derive/season": season,
  "../state": { syncRenderClock() {}, JUMP_DEPARTURE_TTL_S: 60 }, "./derive/stores": { noteStockSamples() {} },
  "./derive/geo": { noteSurveyReports() {} }, "./derive/orders": { syncOrderLifecycles() {} },
  "./derive/market": { marketReservations: [], recentMarketOrders: [], recordPriceHistory() {} },
  "./derive/founding": { FOUNDING_STEP: Object.fromEntries(["build_shipyard", "build_interceptor", "defeat_privateer",
    "eliminate_den", "survey_candidates", "build_barge", "prospect_belt"].map(name => [name, step(name)])) } });
const viewer = { systems: [], ghosts: [], orders: {}, research: null, selectedShipIds: new Set(), selectedShipId: null,
  jumpDepartures: [], timeline: [], awaySet: false };
const viewAt = stage => [...viewing.applyServerMessage({ type: "View", tick: 1, sim_time: 1, systems: [{ id: 1, owner: "a rival" }], ghosts: [],
  anchors: [], wallet: { credits: 0 }, founding: { stage } }, viewer)];
const openedBy = events => events.filter(e => e.story === "chapter").map(e => e.chapter);
assert.deepEqual(openedBy(viewAt("build_shipyard")), [], "the first View opens nothing: old news");
assert.deepEqual(openedBy(viewAt("build_shipyard")), []);
assert.deepEqual(openedBy(viewAt("build_interceptor")), ["pirates"], "step 11 opens Pirates");
assert.deepEqual(openedBy(viewAt("defeat_privateer")), [], "once");
assert.deepEqual(openedBy(viewAt("survey_candidates")), []);
assert.deepEqual(openedBy(viewAt("prospect_belt")), ["reach"], "step 24 opens Reach, even passed in one View");
assert.doesNotMatch(sessionSource, /CHAPTER_OPENINGS|chapterOpening\(/, "no milestone line opens a chapter");
// §signal-report (Peter, Nov 11 2026): a signal's report reaching command
// opens its panel, once; reports already in at the first View stay old news.
{
  const signals = list => ({ next_at: null, open_max: 5, first_ten: 3, signals: list.map(([id, reported]) => ({ id, number: 1,
    kind: "drifting_wreck", hazardous: false, pos: { x: 0, y: 0 }, detected_at: 0, xp: 45,
    report: reported ? { at: 1, fleet: "f", captain: "Mara Vosk", xp: 45, finds: [] } : undefined })) });
  const watcher = { systems: [], ghosts: [], orders: {}, research: null, selectedShipIds: new Set(), selectedShipId: null,
    jumpDepartures: [], timeline: [], awaySet: false, signals: null };
  const view = list => [...viewing.applyServerMessage({ type: "View", tick: 1, sim_time: 1, systems: [{ id: 1, owner: "a rival" }],
    ghosts: [], anchors: [], wallet: { credits: 0 }, founding: { stage: "build_shipyard" }, signals: signals(list) }, watcher)];
  const reported = events => events.filter(e => e.story === "signal").map(e => e.signalId);
  const plainIds = events => JSON.parse(JSON.stringify(reported(events)));
  assert.deepEqual(plainIds(view([["a", true], ["b", false]])), [], "the first View: reports already in stay old news");
  assert.deepEqual(plainIds(view([["a", true], ["b", false]])), []);
  assert.deepEqual(plainIds(view([["a", true], ["b", true]])), ["b"], "b's report reached command");
  assert.deepEqual(plainIds(view([["a", true], ["b", true]])), [], "once");
  // Its line raises no notice of its own: the panel tells it (Nov 12 2026).
  const lines = [{ at_time: 1, severity: "info", text: "Bootstrap." }];
  const later = { timeline: [], awaySet: false };
  applyServerMessage({ type: "Timeline", entries: lines, away_since: 0 }, later);
  const after = [...applyServerMessage({ type: "Timeline", entries: [...lines,
    { at_time: 2, severity: "good", text: "First find 3/10 · Drifting wreck · 40 Alloys aboard · +45 XP." },
    { at_time: 3, severity: "good", text: "Report in · Cargo pod · +20 XP." },
    { at_time: 4, severity: "info", text: "New signal · Ghost ship · Hazardous." }], away_since: 0 }, later)];
  assert.deepEqual(JSON.parse(JSON.stringify(after.filter(e => e.kind === "SignalNews").map(e => e.title))), ["New signal"],
    "a new signal is still news; a report is its panel's");
}
// §den-loot (Peter, Nov 5 2026): the den contract's payout ("{Family} den
// eliminated · contract") raises the loot's panel; "Lanes open" waits for later.
const chapterState = { timeline: [], awaySet: false };
const chaptersOf = entries => [...applyServerMessage({ type: "Timeline", entries, away_since: 0 }, chapterState)];
chaptersOf([]);
const milestone = (at, title) => ({ at_time: at, severity: "good", text: `Charter milestone · ${title} · 3,500 Cr · +2 standing.` });
const denFall = chaptersOf([milestone(30, "Corvane den eliminated · contract")]);
assert.equal(denFall.filter(e => e.story === "den_loot").length, 1, "the den's loot");
assert.equal(denFall.filter(e => e.kind === "CharterMilestone").length, 1, "its toast still shows");
assert.ok(!chaptersOf([milestone(30, "Corvane den eliminated · contract")]).some(e => e.story === "den_loot"), "once");

// One native modal: escaped, queued, followed up, and closed with the session.
class Dialog {
  open = false; innerHTML = ""; className = ""; handlers = {};
  setAttribute() {} addEventListener(type, fn) { this.handlers[type] = fn; }
  showModal() { this.open = true; }
  close() { if (!this.open) return; this.open = false; this.handlers.close?.(); }
  querySelector(selector) {
    if (selector !== "img" && selector !== ".settlement-story-image") return { focus() {} };
    const classes = new Set();
    return (this.image = { listeners: {}, alt: "", src: "", classes, parentElement: { classList: { add: c => classes.add(c) } },
      removeAttribute() {}, addEventListener(type, fn) { this.listeners[type] = fn; } });
  }
}
let dialog = null;
const timers = [];
const document = { createElement: () => (dialog = new Dialog()), body: { append() {} } };
const setTimeout = (fn, ms) => { timers.push({ fn, ms }); return timers.length; };
const clearTimeout = id => { if (timers[id - 1]) timers[id - 1].fn = null; };
const iconsModule = compile(source("icons.ts"));
const protocolModule = compile(source("protocol.ts"));
const signalsDerive = compile(source("core/derive/signals.ts"), {}, { "../../icons": iconsModule,
  "./fleet": { fleetBaseSpeed: () => 1, shipKindLabel: k => k, transitSpeedMultiplier: () => 1, WARP_FACTOR: 5 },
  "./trader": compile(source("core/derive/trader.ts"), {}, { "../../protocol": protocolModule }) });
const { StoryEventUI, freeTraderEvent, piratesEvent, vendettaEvent, denContractEvent, privateerContractEvent, creditsShort, signalReportEvent, laserBatteryEvent, denLootEvent, FOLLOW_UP_MS, decreeEvent, crackdownEvent, exposedEvent, journeyEvent,
  endingEvent, relicsEvent, chapterEvent, crateEvent, CrateOffers, CRATE_DELAY_MS, CRATE_RETRY_MS, CRATE_TEXT } = compile(source("story-events.ts"),
  { document, setTimeout, clearTimeout, Date },
  { "./protocol": compile(source("protocol.ts")), "./core/derive/season": season, "./icons": compile(source("icons.ts")),
    "./core/derive/vendetta": vendetta, "./core/derive/signals": signalsDerive });
const shown = [];
const ui = new StoryEventUI(event => shown.push(event.key));
const raid = piratesEvent({ electronics: 40, armaments: 15 });
// §raid-losses (Peter, Nov 8 2026): the take as the crate's grid in reverse, no sentence.
assert.equal(raid.text, "");
assert.deepEqual(JSON.parse(JSON.stringify(raid.losses)), [["electronics", 40], ["armaments", 15]]);
assert.deepEqual(JSON.parse(JSON.stringify(piratesEvent({ electronics: 0, armaments: 15 }).losses)), [["armaments", 15]]);
assert.equal(piratesEvent({}).text, "It took nothing.");
assert.doesNotMatch(raid.text, /trader|Interceptor|sell/i, "the story, not the next action");
// §den-contract: his log gives the den's coordinates; the Authority's seal speaks the contract.
const contract = denContractEvent("Eliminate the Corvane den", "3500 Cr", "Jax Corvane", "Tarn");
assert.deepEqual([contract.key, contract.title, contract.speaker.speaker, contract.speaker.line],
  ["den_contract", "Authority contract", "Terran Charter Authority", "Eliminate the Corvane den."]);
// §den-retreat (Peter, Nov 12 2026): he went home to his den after his raid,
// so the panel comes as the hunt begins; his log names it only if he died first.
assert.equal(contract.text, "Jax Corvane went home to his den at Tarn. The Authority has a contract out on it.");
assert.equal(denContractEvent("Eliminate the Corvane den", "3500 Cr", "Jax Corvane", "Tarn", true).text,
  "Jax Corvane's log gives the coordinates of his den at Tarn. The Authority has a contract out on it.");
assert.match(source("boot.ts"), /denContractEvent\(event\.title, event\.reward, pirateNames\(state\.founding\)\.privateer, foundingDen\(state\)\?\.name \?\? null,\n\s*state\.founding\?\.stage !== "defeat_privateer"\)\)/);
// §contract-reward (Peter, Nov 11 2026): the reward as a crate tile, short.
assert.equal(contract.credits, 3500);
assert.deepEqual([0, 950, 3500, 8000, 12400, 1_250_000].map(creditsShort), ["0", "950", "3.5k", "8k", "12.4k", "1.3M"]);
assert.match(denContractEvent("Eliminate the den", "a favour", "Jax", null).text, /contract out on it: a favour\.$/, "a reward in another form stays in words");
assert.match(source("story-events.ts"), /function rewardHtml\(credits: number\): string \{\n\s*return `<ul class="crate-goods is-reward" style="--cols: 1" aria-label="Reward"><li style="--i: 0">\$\{icon\("credits"\)\}`\n\s*\+ `<b>\$\{creditsShort\(credits\)\}<\/b><small>Credits<\/small><\/li><\/ul>`;/);
assert.match(source("story-events.ts"), /\(event\.text \? `<p>\$\{esc\(event\.text\)\}<\/p>` : ""\)\n\s*\+ \(event\.credits \? rewardHtml\(event\.credits\) : ""\)/, "under the sentence");
assert.match(denContractEvent("Eliminate the privateer den", "3500 Cr", "the privateer", null).text, /^The privateer went home to his den\. /);
// §privateer-contract (Peter, Nov 9 2026: "The Authority Advisor gets a
// transmission from the Authority that there just happens to be a contract
// out to eliminate the Pirate. Can we introduce the player to the Contracts
// at this point?"): the advisor says it; its one button opens Contracts.
{
  let opened = 0;
  const okafor = { speaker: "Commander Imara Okafor", line: "Authority contract on Milo Quill.", portrait: "/okafor.webp", alt: "Okafor", tone: "authority" };
  const offer = privateerContractEvent("Eliminate Milo Quill", "8000 Cr", okafor, () => opened++);
  assert.deepEqual([offer.key, offer.title, offer.speaker.speaker, offer.text], ["privateer_contract", "Authority contract", "Commander Imara Okafor", ""]);
  assert.deepEqual([[...offer.facts], offer.credits], [["Eliminate Milo Quill"], 8000], "the terms under the advisor's word, the reward a tile");
  assert.equal(offer.accept.label, "Open Contracts");
  offer.accept.run();
  assert.equal(opened, 1);
  const sealed = privateerContractEvent("Eliminate Milo Quill", "8000 Cr", null, () => {});
  assert.deepEqual([sealed.speaker.speaker, sealed.speaker.line], ["Terran Charter Authority", "Eliminate Milo Quill."], "no advisor: the seal says it");
  assert.match(sealed.text, /Contracts/);
  const boot = source("boot.ts");
  assert.match(boot, /event\.story === "privateer_contract"\) showPrivateerContract\(event\.title, event\.reward\);/);
  assert.match(boot, /function showPrivateerContract[\s\S]*?state\.storyTold\.has\("privateer_contract"\) \|\| storyEvents\.has\("privateer_contract"\)[\s\S]*?cardLine\(advisor, "privateer_contract", \{ privateer \}\)[\s\S]*?activeShell\?\.openContract\?\.\(\)\), due: untaken \}/,
    "in the advisor's voice, once; skipped once taken on");
  assert.match(boot, /offerLaserBattery\(\);\n\s*offerPrivateerContract\(\);/, "a page loaded after its news still hears it");
  for (const shell of ["shell/deck/index.ts", "shell/mobile/index.ts"]) {
    assert.match(source(shell), /openContract\(\): void \{[\s\S]*?coach\("contract-accept", CONTRACT_COACH\);/, `${shell}: Contracts, its Accept ringed`);
  }
  assert.match(source("shell/deck/strategic.ts"), /charter\.length \? group\("Authority contract", charter\)/, "pinned on top (desktop)");
  assert.match(source("shell/deck/strategic.ts"), /isFounderContract\(operation\) \? accept\.replace\("<button ", `<button data-coach="contract-accept" `\)/);
  assert.match(source("shell/mobile/parity.ts"), /<h3>Authority contract<\/h3>/, "pinned on top (phone)");
  assert.match(source("shell/mobile/parity.ts"), /isFounderContract\(operation\) \? ` data-coach="contract-accept"` : ""/);
  assert.match(source("shell/deck/founding-guide.ts"), /action === "contract"\) coach\("contract-accept", CONTRACT_COACH\)/);
  assert.match(source("shell/mobile/surfaces.ts"), /action === "contract"\) \{[\s\S]*?coach\("contract-accept", CONTRACT_COACH\)/);
}
// §laser-battery-card (Nov 5 2026): the step the raid calls for, painted;
// since Nov 6 its title and line only, no reason or cost under the title.
const battery = laserBatteryEvent();
assert.deepEqual([battery.key, battery.title, battery.facts], ["laser_battery", "Build a Laser Battery", undefined]);
assert.equal(battery.text, "It would be wise to build some defenses after that pirate raid…", "with no advisor on duty");
assert.match(battery.art.src, /laser-battery-1280\.webp$/);
// Peter (Nov 9 2026): "The recommendation to build a Laser Battery should
// come from the Authority Advisor": their word, under the Authority's seal.
const okafor = { speaker: "Commander Imara Okafor", line: "The Authority protects its investments. You aren't one yet. Shoot back next time.",
  portrait: "/art/derived/advisors/v1/veteran-320.webp", alt: "Commander Imara Okafor, Charter Advisor.", tone: "authority" };
const advised = laserBatteryEvent(okafor);
assert.deepEqual([advised.key, advised.title, advised.text, advised.source, advised.speaker.line],
  ["laser_battery", "Build a Laser Battery", "", undefined, okafor.line], "the advisor's line in place of the plain one");
assert.equal(advised.art.src, battery.art.src, "the painting stays");
// §den-loot (Peter, Nov 9 2026: "list the actual loot from the Pirate Den
// with the resource icons" in place of its units and trips; "Keep the 'Crates
// of stolen goods fill the den...'"): what waits there, most first.
const stash = { machinery: 37, armaments: 35, electronics: 15, zephyr_drive: 7, fission_reactor: 6, polymers: 0 };
const loot = denLootEvent(stash);
assert.deepEqual([loot.key, loot.title, loot.facts], ["den_loot", "The den's loot", undefined], "no units or trips");
assert.deepEqual(JSON.parse(JSON.stringify(loot.goods)), [["machinery", 37], ["armaments", 35], ["electronics", 15],
  ["zephyr_drive", 7], ["fission_reactor", 6]], "the goods, most first, none of what isn't there");
assert.equal(loot.text, "Crates of stolen goods fill the den. It will take a freighter to haul them home.", "its line stays");
assert.match(loot.art.src, /den-loot-1280\.webp$/);
assert.match(source("boot.ts"), /storyEvents\.show\(denLootEvent\(stash!\.goods\)\)/, "from the den's own stash");
// §traders: the founder's own trader docks.
const teo = { name: "Teo Brannick", ship: "Long Odds" };
assert.equal(freeTraderEvent(teo).text, "Teo Brannick's Long Odds has docked at your colony. Word is its hold carries semiconductors.");
assert.equal(freeTraderEvent({ name: "Ines Varga", ship: "Clean Slate" }).text,
  "Ines Varga's Clean Slate has docked at your colony. Word is its hold carries semiconductors.");
// §traders: until the painting lands, the trader's own ship stands in.
assert.equal(freeTraderEvent({ slug: "ines", name: "Ines Varga", ship: "Clean Slate" }).art.interim.src,
  "/art/derived/ships/models-v1/previews/clean_slate-768.webp");
assert.match(source("boot.ts"), /const trader = traderOf\(state\.founding\);[\s\S]{0,120}freeTraderEvent\(trader,/, "the one the server dealt");
// §advisor-cards (Peter, Nov 9 2026: "let's have the Authority Advisor
// announce the arrival of the Free Trader"): their announcement in place of
// the line, the painting kept.
const announced = freeTraderEvent({ slug: "ines", name: "Ines Varga", ship: "Clean Slate" },
  { ...okafor, line: "Ines Varga's Clean Slate has docked with semiconductors. Free traders are thieves with permits. Today, you need one." });
assert.deepEqual([announced.key, announced.title, announced.text, announced.speaker.speaker], ["free_trader", "A free trader", "", "Commander Imara Okafor"]);
assert.match(announced.art.src, /free-trader-inbound-1280\.webp$/);
assert.match(source("boot.ts"), /advisorSpeaker\(advisor, cardLine\(advisor, "free_trader", \{ trader: `\$\{trader\.name\}'s \$\{trader\.ship\}` \}\)\)/);

// Each painting is prompted; until it lands, its interim stands in.
const prompts = JSON.parse(readFileSync(new URL("../art-src/story-events-v1/prompts.json", import.meta.url), "utf8"));
for (const event of [raid, freeTraderEvent(teo)]) {
  const slug = /story-events\/v1\/(.+)-1280\.webp/.exec(event.art.src)[1];
  assert.ok(prompts.images.some(image => image.slug === slug && image.prompt), `${slug} is prompted`);
  assert.ok(event.art.alt && event.art.interim.alt, "alternative text for both images");
  const painted = existsSync(new URL(`../public${event.art.src}`, import.meta.url));
  assert.ok(painted || existsSync(new URL(`../public${event.art.interim.src}`, import.meta.url)), "an image is always served");
}
assert.match(prompts.images.find(i => i.slug === "pirate-raid").prompt, /no free trader/i,
  "the raid scene leaves the trader to its own");

const press = () => dialog.handlers.click({ target: { closest: selector => selector.includes("close") } });
ui.show(raid);
assert.ok(dialog.open);
assert.match(dialog.innerHTML, /<h1 id="story-event-title"[^>]*>Pirate raid<\/h1>/);
assert.match(dialog.innerHTML, /data-story-act="close">Continue</);
assert.doesNotMatch(dialog.innerHTML, /Charter event/, "no kicker over the story (Peter, Oct 21 2026)");
dialog.image.listeners.error();
assert.equal(dialog.image.src, raid.art.interim.src, "a missing painting falls back to the interim");
assert.ok(dialog.image.classes.has("is-icon"), "the privateer's sprite stands in at 2×, its frame kept 16:9");
press();
assert.equal(timers.length, 0, "closing the raid announces no trader by itself");
// The server's docking line arrives while a pop-up is open: the trader waits
// for it to close, then FOLLOW_UP_MS.
ui.show(piratesEvent({ electronics: 40, armaments: 15 }));
assert.match(dialog.innerHTML, /class="crate-goods is-loss"[\s\S]*<img class="icon icon--resource"[^>]*><b>−40<\/b><small>Semiconductors<\/small>[\s\S]*<b>−15<\/b>/,
  "§raid-losses: the take shows as icons and −N under the caption");
assert.match(dialog.innerHTML, /settlement-story-image/, "the painting stays");
press();
ui.show(denLootEvent(stash));
assert.match(dialog.innerHTML, /settlement-story-image[\s\S]*class="crate-goods is-loot" style="--cols: 3" aria-label="Loot">[\s\S]*<img class="icon icon--resource"[^>]*><b>37<\/b><small>Machinery<\/small>[\s\S]*<b>6<\/b>[\s\S]*<p>Crates of stolen goods fill the den\./,
  "§den-loot: icons and counts under the painting, then its line");
assert.doesNotMatch(dialog.innerHTML, /story-facts|units at|Tiny Freighter/, "no units or trips");
press();
ui.show(piratesEvent({ electronics: 40, armaments: 15 }));
ui.showAfterGap(freeTraderEvent(teo), FOLLOW_UP_MS);
assert.equal(timers.length, 0);
press();
assert.equal(timers.length, 1);
assert.equal(timers[0].ms, 3_000, "three seconds after it closes");
assert.ok(!shown.includes("free_trader"), "waiting is not telling: the guide still keeps the trader back");
timers[0].fn();
assert.ok(dialog.open, "then the trader pops up");
assert.equal(shown.at(-1), "free_trader", "and only now is it told");
assert.match(dialog.innerHTML, /A free trader/);
assert.match(dialog.innerHTML, /has docked at your colony/);
dialog.image.listeners.error();
assert.ok(!dialog.image.classes.has("is-icon"), "the ship model fills the frame");
press();
assert.equal(timers.length, 1, "the trader leads nowhere");
// Docking long after the last pop-up closed: no wait.
ui.closedAt = Date.now() - 4_000;
ui.showAfterGap(freeTraderEvent(teo), FOLLOW_UP_MS);
assert.ok(dialog.open, "shown at once");
press();

// Queued events wait their turn and are text, not markup.
ui.show(freeTraderEvent(teo));
ui.show({ title: "<b>Next</b>", text: "&", art: { src: "/x.webp", alt: "x" } });
press();
assert.match(dialog.innerHTML, /&lt;b&gt;Next&lt;\/b&gt;/);
press();
assert.ok(!dialog.open);

// A replaced session drops the queue and the pending trader.
ui.show(piratesEvent({ electronics: 40, armaments: 15 }));
ui.showAfterGap(freeTraderEvent(teo), FOLLOW_UP_MS);
press();
ui.endSession();
assert.equal(timers.at(-1).fn, null, "the pending trader is cancelled");
assert.ok(!dialog.open);

// §season (Oct 29 2026): each GNN story has its own painting, prompted and
// derived (docs/gnn-art-prompts.md), and names the broadcast telling it.
const told = [
  ...["levies", "inspections", "martial"].map(grip => decreeEvent(grip)),
  ...["sabotage", "massacre", "quarantine", "relic_seizure", "keystone"].map((kind, i) => crackdownEvent(kind, i)),
  exposedEvent("False flag exposed · Convoy massacre · by Alpha · crackdown lifted.", 9),
  ...["found", "spy", "war"].map((beat, i) => journeyEvent(beat, "Sealed world found · Vega · its Keystone can close every gate.", 20 + i)),
  endingEvent({ kind: "sovereign", at: 1, holder: "Alpha", system: "K", winners: ["Alpha"], won: true }),
  endingEvent({ kind: "charter_holds", at: 1, holder: null, system: null, winners: [], won: false }),
  relicsEvent(),
];
const painted = told.map(event => /story-events\/v1\/(.+)-1280\.webp/.exec(event.art.src)[1]);
assert.deepEqual(painted, ["decree-levies", "decree-inspections", "decree-martial", "sabotage-at-the-hub", "convoy-massacre", "quarantine",
  "relic-seizure", "keystone-crackdown", "false-flag-exposed", "sealed-world-found", "authority-spy", "authority-war", "seal-fires",
  "charter-holds", "relics-awaken"]);
const build = readFileSync(new URL("./build-founding-story-art.mjs", import.meta.url), "utf8");
for (const [i, slug] of painted.entries()) {
  assert.ok(prompts.images.some(image => image.slug === slug && image.prompt), `${slug} is prompted`);
  assert.ok(told[i].art.alt, `${slug}: alternative text`);
  assert.match(build, new RegExp(`"${slug}"`), `${slug} is built`);
  for (const width of [768, 1280, 1672]) {
    assert.ok(existsSync(new URL(`../public/art/derived/story-events/v1/${slug}-${width}.webp`, import.meta.url)), `${slug}-${width} derived`);
  }
}
assert.deepEqual(told.map(event => event.source ?? "-"), ["gnn", "gnn", "gnn", "gnn", "gnn", "gnn", "gnn", "gnn", "free_signal",
  "-", "-", "gnn", "free_signal", "gnn", "gnn"], "the GNN tells the Authority's stories; the Free Signal its defeats");
assert.equal(crackdownEvent("keystone", 1).title, "Keystone crackdown");
// §chapters: each opening's painting is built; its hull stands in until it lands.
const chapters = ["pirates", "expansion", "reach"].map(c => chapterEvent(c, "Jace Marrow"));
assert.deepEqual(chapters.map(e => e.key), ["chapter_pirates", "chapter_expansion", "chapter_reach"]);
assert.deepEqual(chapters.map(e => e.title), ["Time to Strike Back", "Lanes open", "Into the belt"]);
assert.equal(chapters[0].text, "The Shipyard stands. Jace Marrow is still out there.", "the nephew by name");
assert.equal(chapterEvent("pirates").text, "The Shipyard stands. The privateer is still out there.");
for (const event of chapters) {
  const slug = /story-events\/v1\/(.+)-1280\.webp/.exec(event.art.src)[1];
  assert.match(build, new RegExp(`"${slug}"`), `${slug} is built`);
  assert.ok(event.art.alt && event.art.interim.alt, `${slug}: alternative text for both images`);
  assert.ok(existsSync(new URL(`../public${event.art.interim.src}`, import.meta.url)), `${slug}: its hull stands in`);
}
// §event cards (Nov 6 2026): the Laser Battery's and the den loot's paintings
// landed; each is indexed with its prompt and built at every width.
for (const event of [laserBatteryEvent(), denLootEvent({ machinery: 37 })]) {
  const slug = /story-events\/v1\/(.+)-1280\.webp/.exec(event.art.src)[1];
  assert.match(build, new RegExp(`"${slug}"`), `${slug} is built`);
  assert.ok(prompts.images.some(image => image.slug === slug && image.prompt), `${slug} is prompted`);
  for (const width of [768, 1280, 1672]) {
    assert.ok(existsSync(new URL(`../public/art/derived/story-events/v1/${slug}-${width}.webp`, import.meta.url)), `${slug}-${width} derived`);
  }
}
// §smelter-raid (Nov 9 2026): a raid can end before the Smelter is staffed;
// the Laser Battery's card waits for its own step, and lapses past it.
assert.match(source("boot.ts"), /laserBatteryDue = !state\.storyTold\.has\("laser_battery"\);\n\s*offerLaserBattery\(\);/);
assert.match(source("boot.ts"), /FOUNDING_STEP\[stage\] > FOUNDING_STEP\.build_gun_platform\) \{\n\s*laserBatteryDue = false;/);
assert.match(source("boot.ts"), /crates\.offer\(\);\n\s*offerLaserBattery\(\);/, "on every server message");
// §watch-the-pirate: not while the guide still watches the raider, and after
// the advisor's raider-gone panel; then in the advisor's own words.
assert.match(source("boot.ts"), /if \(\(f\.raid_seen && !f\.raid_lost\) \|\| advisorDesk\.leftOwed\(\)\) return;/);
assert.match(source("boot.ts"), /storyEvents\.showAfterGap\(\{ \.\.\.laserBatteryEvent\(advisor && advisorSpeaker\(advisor, cardLine\(advisor, "laser_battery"\)\)\),/);
assert.match(source("boot.ts"), /storyEvents\.show\(chapterEvent\(event\.chapter, privateer,\n\s*belt\?\.ore && belt\.richness != null \? \{ ore: belt\.ore, richness: belt\.richness \} : null, voiceOf\(state\.founding\),/,
  "the Reach chapter carries the den belt's best ore");
// §advisor-cards (Peter, Nov 9 2026: "Add more snarkiness, cynicism and
// insulting remarks to this panel"): the Pirates chapter opens on the
// advisor's own line; the others speak the chapter's.
assert.match(source("boot.ts"), /const advisor = event\.chapter === "pirates" \? advisorOnDuty\(state\.founding\) : null;/);
const voiced = line => ({ ...okafor, line });
const strike = chapterEvent("pirates", "Jace Marrow", null, voiced, "Took you long enough.");
assert.deepEqual([strike.title, strike.text, strike.speaker.line], ["Time to Strike Back", "", "Took you long enough."]);
assert.equal(chapterEvent("expansion", "Jace Marrow", null, voiced).speaker.line, "The den is gone. The lanes to the Market Hub are clear.");
ui.show(advised);
assert.match(dialog.innerHTML, /<header class="settlement-story-header"><div><small>STELLAR SYNDICATES<\/small><\/div><\/header>[\s\S]*settlement-story-image[\s\S]*<div class="story-transmission is-authority"><img src="\/art\/derived\/advisors\/v1\/veteran-320\.webp"/,
  "the painting, then the advisor beside their line");
assert.doesNotMatch(dialog.className, /is-advisor/, "painted: not the large advisor-only panel");
assert.doesNotMatch(dialog.innerHTML, /It would be wise/);
press();
ui.show(decreeEvent("levies"));
assert.match(dialog.innerHTML, /<div class="story-source"><img src="\/art\/derived\/gnn-v1\/gnn-emblem-128\.webp" alt="" decoding="async"><small>GALACTIC NEWS NETWORK<\/small><\/div>/);
assert.match(dialog.innerHTML, /srcset="[^"]*decree-levies-1672\.webp 1672w"/);
press();
ui.show(told[8]);
assert.match(dialog.innerHTML, /free-signal-emblem-128\.webp" alt="" decoding="async"><small>FREE SIGNAL</);
press();
ui.show(told[9]);
assert.match(dialog.innerHTML, /<header class="settlement-story-header"><div><small>STELLAR SYNDICATES<\/small><\/div><\/header>/, "your own find is no broadcast");
press();

// §crate-panel (Peter, Nov 3 2026): a released charter crate pops up in the
// middle of the screen, its goods on show, and only Accept closes it.
const accept = () => dialog.handlers.click({ target: { closest: selector => selector.includes("accept") } });
const escape = () => {
  const key = { key: "Escape", defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
  const cancel = { defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
  dialog.handlers.keydown(key);
  dialog.handlers.cancel(cancel);
  assert.equal(key.defaultPrevented, cancel.defaultPrevented, "the key and the cancel alike");
  return cancel.defaultPrevented;
};
const storesCrate = { id: "vendetta", released: true, step: 25, goods: { alloys: 70, machinery: 18 } };
let fill = { used: 412, cap: 1_000 };
const collected = [];
const marrow = { privateer: "Jace Marrow", den: "the Marrow den", uncle: "Mordecai Marrow" };
const crate = crateEvent(storesCrate, marrow, () => fill, () => collected.push("vendetta"));
assert.equal(crate.key, "crate_vendetta");
ui.show(crate);
// Peter (Nov 9 2026): "remove the Terran Charter Authority title at the top
// of these panels, doesn't make sense": the crates and the advisor's panels
// take the plain header.
assert.match(dialog.innerHTML, /<header class="settlement-story-header"><div><small>STELLAR SYNDICATES<\/small><\/div><\/header>/, "no Authority title");
assert.doesNotMatch(source("story-events.ts"), /<small>TERRAN CHARTER AUTHORITY<\/small>/, "on no panel");
assert.match(dialog.innerHTML, /<h1 id="story-event-title"[^>]*>Charter crate<\/h1><p>Mordecai Marrow swore revenge\. Secure your stores\.<\/p><footer>/,
  "what released it, under the title");
assert.match(dialog.innerHTML, new RegExp(/<ul class="crate-goods" style="--cols: 2"><li style="--i: 0"><img [^>]*resource\/alloys\.png[^>]*><b>\+70<\/b><small>Alloys<\/small><\/li>/
  .source + /<li style="--i: 1"><img [^>]*resource\/machinery\.png[^>]*><b>\+18<\/b><small>Machinery<\/small><\/li><\/ul>/.source));
assert.match(dialog.innerHTML, /<p class="crate-storage">Storage 412 → 500\/1,000<\/p>/, "what it does to the stores");
assert.match(dialog.innerHTML, /<footer><button type="button" class="settlement-primary" data-story-act="accept">Accept<\/button><\/footer>/);
assert.doesNotMatch(dialog.innerHTML, /data-story-act="close"|<p><\/p>/, "no other way out, no empty line");
assert.match(dialog.className, /\bis-crate\b/);
assert.ok(escape(), "Escape does nothing");
assert.ok(dialog.open);
accept();
assert.deepEqual(collected, ["vendetta"], "Accept collects it");
assert.ok(!dialog.open, "and closes it");
ui.show(raid);
assert.ok(!escape(), "a story still escapes");
assert.doesNotMatch(dialog.className, /is-crate/);
press();
// Past the cap it still opens (sim collect_charter_crate); production waits.
fill = { used: 980, cap: 1_000 };
ui.show(crateEvent(storesCrate, marrow, () => fill, () => {}));
assert.match(dialog.innerHTML, /<p class="crate-storage is-over" title="Production waits for room\.">Storage 980 → 1,068\/1,000 · over capacity<\/p>/);
accept();
// Six goods three to a row, four two by two; every good its icon.
const sixGoods = { alloys: 60, machinery: 28, polymers: 12, provisions: 20, uranium: 50, warp_core: 1 };
ui.show(crateEvent({ id: "reach", released: true, step: 23, goods: sixGoods }, marrow, () => null, () => {}));
assert.match(dialog.innerHTML, /--cols: 3/);
assert.equal(dialog.innerHTML.match(/<li style/g).length, 6);
assert.match(dialog.innerHTML, /goods-v1\/uranium-64\.png/);
assert.match(dialog.innerHTML, /<b>\+1<\/b><small>Warp Core<\/small>/);
assert.doesNotMatch(dialog.innerHTML, /crate-storage/, "no home report, no storage line");
accept();
ui.show(crateEvent({ id: "reach", released: true, step: 21, goods: { alloys: 60, machinery: 28, polymers: 12, warp_core: 1 } }, marrow, () => null, () => {}));
assert.match(dialog.innerHTML, /--cols: 2/);
accept();
// Each says what released it (Peter, Nov 3 2026: "ties it to the chapter or
// event ... that triggered it"), the pirates by name once known.
const blurbs = Object.fromEntries(Object.entries(CRATE_TEXT).map(([id, text]) => [id, text(marrow)]));
assert.deepEqual(blurbs, {
  landfall: "From the colony ship's holds: your first builds.",
  // §progression (Nov 11 2026): step 19 releases the Expansion chapter's builds.
  expansion: "For your Academy and a second world's mine.",
  reach: "For the charted belt: a Mining Barge to work it.",
  vendetta: "Mordecai Marrow swore revenge. Secure your stores.",
});
// Peter (Nov 4 2026): the Reach chapter names the survey's best ore and its richness.
assert.equal(chapterEvent("reach", "Jace Marrow", { ore: "bornite_ore", richness: 1.62 }).text,
  "The survey is in: the den's old belt is rich in Bornite Ore, ×1.62.");
assert.equal(chapterEvent("reach", "Jace Marrow").text, "The survey is in: the den's old belt is rich in ore.", "before the survey's figures arrive");
const unnamed = vendetta.pirateNames(null);
assert.equal(CRATE_TEXT.vendetta(unnamed), "The privateer's uncle swore revenge. Secure your stores.");
// One collected while it waited its turn is skipped.
ui.show(raid);
ui.show(crateEvent(storesCrate, marrow, () => fill, () => {}, () => false));
assert.ok(ui.has("pirates") && ui.has("crate_vendetta"), "up, or waiting its turn");
press();
assert.ok(!dialog.open, "no longer due: skipped");
assert.ok(!ui.has("crate_vendetta"));

// CrateOffers: each released crate after its step's story, once; held after
// Accept for the server's word, and up again if the word never comes.
const founding = { pirates: { nephew: "Jace Marrow", surname: "Marrow", uncle: "Mordecai Marrow" },
  crates: [storesCrate, { id: "reach", released: false, step: 23, goods: { alloys: 30 } }] };
const sent = [];
const offers = new CrateOffers(ui, { founding: () => founding, storage: () => fill, collect: id => sent.push(id) });
const t0 = Date.now();
offers.offer(t0);
assert.ok(!dialog.open, "not at once");
ui.show(chapterEvent("pirates", "Jace Marrow"));
offers.offer(t0 + CRATE_DELAY_MS);
offers.offer(t0 + CRATE_DELAY_MS + 100);
assert.match(dialog.innerHTML, /Time to Strike Back/, "the chapter's story first");
press();
assert.match(dialog.innerHTML, /Charter crate/, "then the crate");
assert.doesNotMatch(dialog.innerHTML, /Charter crate[\s\S]*Charter crate/);
accept();
assert.deepEqual(sent, ["vendetta"]);
assert.ok(!dialog.open, "queued once");
offers.offer(Date.now() + 1_000);
assert.ok(!dialog.open, "accepted: waiting for the server");
offers.offer(Date.now() + CRATE_RETRY_MS + 10);
assert.ok(dialog.open, "no word: up again");
assert.ok(escape() && dialog.open);
accept();
founding.crates = [{ ...founding.crates[1], released: true }];
offers.offer(Date.now() + 60_000);
offers.offer(Date.now() + 60_000 + CRATE_DELAY_MS);
assert.match(dialog.innerHTML, /<b>\+30<\/b><small>Alloys<\/small>/, "the next chapter's crate in its turn");
assert.match(dialog.innerHTML, /<p>For the charted belt: a Mining Barge to work it\.<\/p>/, "its own line");
accept();
assert.deepEqual(sent, ["vendetta", "vendetta", "reach"]);
// A restarted charter's crate waits for its story again.
founding.crates = [];
offers.offer(Date.now() + 120_000);
founding.crates = [{ ...storesCrate }];
offers.offer(Date.now() + 130_000);
assert.ok(!dialog.open, "a fresh release waits its 2 s");
offers.offer(Date.now() + 130_000 + CRATE_DELAY_MS);
assert.ok(dialog.open);
accept();
assert.match(source("boot.ts"), /present\(applyServerMessage\(message, state\)\);\s*crates\.offer\(\);/, "on every server message, after its stories");
assert.match(source("boot.ts"), /type: "CollectCharterCrate", charter_crate: crate/);

// The guide never tells the story itself.
for (const file of ["core/derive/founding.ts", "shell/deck/founding-guide.ts", "shell/mobile/surfaces.ts", "shell/deck/log.ts"]) {
  const code = source(file).replace(/^\s*\/\/.*$/gm, "");
  assert.doesNotMatch(code, /made off with|swept down/, `${file} keeps to what happens and what to do`);
}
// §signal-report (Peter, Nov 11 2026: "when the player is visiting/completing
// a signal operation, an event panel with some art is needed to explain what
// happen in the operation"): its painting, what happened, what came home.
{
  const sig = (kind, finds, extra = {}) => ({ id: "s9", number: 4, kind, hazardous: false, pos: { x: 0, y: 0 }, detected_at: 0, xp: 45,
    report: { at: 0, fleet: "f", captain: "Mara Vosk", xp: 45, finds, ...extra } });
  const plain = value => JSON.parse(JSON.stringify(value));
  let opened = 0;
  const opts = { ship: "Interceptor", now: 0, programmeName: id => ({ prop_drive_tuning: "Drive Tuning" })[id] ?? id, showCache: () => opened++ };
  const wreck = signalReportEvent(sig("drifting_wreck", [{ kind: "cargo_aboard", goods: [["alloys", 40], ["machinery", 12]] },
    { kind: "module_aboard", module: "recon_suite" }, { kind: "credits", credits: 8000 }, { kind: "dossier", programme: "prop_drive_tuning" },
    { kind: "blueprint", module: "cargo_pods" }]), opts);
  assert.deepEqual([wreck.key, wreck.title, wreck.text], ["signal_s9", "Drifting wreck",
    "Mara Vosk's Interceptor cut into the wreck and hauled out what its dead crew left."]);
  // The aftermath (Peter, Nov 11 2026); its card painting until the master lands.
  assert.equal(wreck.art.src, "/art/derived/signals/aftermath-v1/drifting-wreck-768.webp");
  assert.equal(wreck.art.interim.src, "/art/derived/signals/v1/drifting-wreck-768.webp", "the card painting stands in");
  const aftermath = JSON.parse(readFileSync(new URL("../art-src/signals-aftermath-v1/prompts.json", import.meta.url), "utf8"));
  assert.deepEqual(aftermath.images.map(i => i.slug), Object.values(signalsDerive.SIGNAL_INFO).map(i => i.slug),
    "an aftermath prompt for every signal type, in order");
  for (const image of aftermath.images) {
    assert.ok(image.references[0] === `client/art-src/signals-v1/${image.slug}.png` && existsSync(new URL(`../art-src/signals-v1/${image.slug}.png`, import.meta.url)),
      `${image.slug}: generated from its own card painting`);
    assert.match(image.prompt, /exactly 1672 x 941 pixels[\s\S]*show the work done, not the loot/, image.slug);
  }
  assert.deepEqual(plain(wreck.tiles.map(t => [t.amount ?? null, t.label])), [["40", "Alloys"], ["12", "Machinery"], ["1", "Recon Suite"],
    ["8k", "Credits"], [null, "Drive Tuning dossier"], [null, "Cargo Pods blueprint"]], "what came home, as crate tiles");
  assert.deepEqual(plain(wreck.notes.map(n => n.text)), ["First find 4/10", "+45 XP · Mara Vosk"], "the first ten's tally");
  assert.deepEqual(plain(signalReportEvent({ ...sig("cargo_pod", []), number: 11 }, opts).notes.map(n => n.text)), ["+45 XP · Mara Vosk"], "after the first ten, none");
  assert.equal(wreck.also, undefined, "no cache, no second button");
  const ore = signalReportEvent(sig("ore_fragment", [{ kind: "cache", site: "c", goods: [["titanium_ore", 120]] }]), { ...opts, ship: null });
  assert.equal(ore.text, "Mara Vosk sampled the fragment: rich ore, waiting to be mined.", "a cache counts as found");
  assert.deepEqual(plain(ore.tiles), [], "nothing came home");
  assert.equal(ore.notes.at(-1).text, "120 Titanium Ore wait at the site for a Freighter");
  assert.equal(ore.also.label, "Show the cache");
  const hurt = signalReportEvent({ ...sig("pirate_stash", [{ kind: "credits", credits: 2400 }], { hurt_until: 14_400 }), hazardous: true }, opts);
  assert.equal(hurt.text, "Mara Vosk found the stash behind its tripwires and took it. The stash was rigged; the captain took the blast.");
  assert.deepEqual(plain(hurt.notes.at(-1)), { text: "Mara Vosk is hurt · out 4h", tone: "warn" });
  const courier = signalReportEvent(sig("authority_courier", [{ kind: "standing", credits: 1500, standing: 3 }]), opts);
  assert.deepEqual(plain([courier.tiles.map(t => t.amount), courier.notes.map(n => n.text)]), [["1.5k"], ["First find 4/10", "+45 XP · Mara Vosk", "+3 standing with the Authority"]]);
  const empty = signalReportEvent(sig("sensor_ghost", []), opts);
  assert.equal(empty.text, "Mara Vosk swept the reading. Nothing there but static.");
  const crate = signalReportEvent(sig("teos_lost_crate", [{ kind: "credits", credits: 600 }]), { ...opts, trader: { first: "Ines" } });
  assert.equal(crate.text, "Mara Vosk found Ines's crate, seal unbroken.", "the founder's own trader");
  // Every type: three lines, the captain named once, no placeholder left.
  for (const [kind, lines] of Object.entries(signalsDerive.SIGNAL_OUTCOME)) {
    for (const [moment, line] of Object.entries(lines)) {
      assert.ok(moment === "hurt" ? !line.includes("{captain}") : line.includes("{captain}"), `${kind} ${moment}`);
      assert.ok(line.split(/\s+/).length <= 18, `${kind} ${moment}: short`);
    }
    for (const finds of [[], [{ kind: "credits", credits: 1 }]]) {
      const text = signalReportEvent(sig(kind, finds, { hurt_until: 99 }), opts).text;
      assert.doesNotMatch(text, /[{}]/, `${kind}: every placeholder filled`);
    }
  }
  // On screen: the tiles, the notes, and Show the cache beside Continue; it
  // runs, then the panel closes.
  ui.show(ore);
  assert.match(dialog.innerHTML, /<p class="story-notes"><span>First find 4\/10<\/span><span>\+45 XP · Mara Vosk<\/span><span>120 Titanium Ore wait at the site for a Freighter<\/span><\/p>/);
  assert.match(dialog.innerHTML, /<footer><button type="button" data-story-act="also">Show the cache<\/button><button type="button" class="settlement-primary" data-story-act="close">Continue<\/button><\/footer>/);
  dialog.handlers.click({ target: { closest: selector => selector.includes("also") } });
  assert.deepEqual([opened, dialog.open], [1, false], "it framed the cache and closed");
  ui.show(hurt);
  assert.match(dialog.innerHTML, /<ul class="crate-goods is-reward" style="--cols: 1" aria-label="Found"><li style="--i: 0"><img[^>]*credits[^>]*><b>2\.4k<\/b><small>Credits<\/small><\/li><\/ul>/);
  assert.match(dialog.innerHTML, /<span class="is-warn">Mara Vosk is hurt · out 4h<\/span>/);
  dialog.handlers.click({ target: { closest: selector => selector.includes("close") } });
  // boot: a report raises it once, and Show the cache frames the site.
  const boot = source("boot.ts");
  assert.match(boot, /event\.story === "signal" && !state\.storyTold\.has\(`signal_\$\{event\.signalId\}`\)\) showSignalReport\(event\.signalId\);/);
  assert.match(boot, /state\.selectedExplorationSiteId = site\.id;\n\s*renderer\.centerOnWorld\(site\.pos\);/);
}
console.log("story events: the raid on its report, then the free trader on the server's docking line (3 s after a pop-up closes); queue, escaping, interims and a guide without the story; the GNN stories' paintings and broadcast headers; the relics stir once; the chapters open painted; charter crates pop up to be accepted");
