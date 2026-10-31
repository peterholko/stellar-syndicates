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
assert.deepEqual({ ...events.find(e => e.kind === "StoryEvent") },
  { kind: "StoryEvent", story: "pirates", semiconductors: 40, armaments: 15 });
assert.ok(!events.some(e => e.kind === "CharterMilestone"), "the pop-up replaces the toast");
assert.ok(!receipt([line]).some(e => e.kind === "StoryEvent"), "an old line never pops up again");
// The trader is news only when the server says it docked.
const docking = { at_time: 150, severity: "good", text: "A free trader docked at Freya." };
assert.deepEqual({ ...receipt([line, docking]).find(e => e.kind === "StoryEvent") }, { kind: "StoryEvent", story: "free_trader" });
assert.ok(!receipt([line, docking]).some(e => e.kind === "StoryEvent"), "and only once");
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
// §chapters (Oct 29 2026): the Pirates, Expansion and Reach chapters open
// painted, on the milestone line that starts them; its toast still shows.
const chapterState = { timeline: [], awaySet: false };
const chaptersOf = entries => [...applyServerMessage({ type: "Timeline", entries, away_since: 0 }, chapterState)];
chaptersOf([]);
const milestone = (at, title) => ({ at_time: at, severity: "good", text: `Charter milestone · ${title} · 400 Cr · +2 standing.` });
const yard = [milestone(10, "Shipyard staffed"), milestone(20, "Interceptor launched")];
const opened = chaptersOf(yard);
assert.deepEqual(opened.filter(e => e.kind === "StoryEvent").map(e => e.chapter), ["pirates"], "the Shipyard opens Pirates");
assert.equal(opened.filter(e => e.kind === "CharterMilestone").length, 2, "the toasts still show");
assert.deepEqual(chaptersOf([...yard, milestone(30, "Privateer den eliminated"), milestone(40, "Nearby system charted")])
  .filter(e => e.story === "chapter").map(e => e.chapter), ["expansion", "reach"]);
assert.ok(!chaptersOf([...yard, milestone(30, "Privateer den eliminated"), milestone(40, "Nearby system charted")]).some(e => e.story === "chapter"), "once");

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
const { StoryEventUI, freeTraderEvent, piratesEvent, vendettaEvent, FOLLOW_UP_MS, decreeEvent, crackdownEvent, exposedEvent, journeyEvent,
  endingEvent, relicsEvent, chapterEvent } = compile(source("story-events.ts"), { document, setTimeout, clearTimeout, Date },
  { "./protocol": compile(source("protocol.ts")), "./core/derive/season": season });
const shown = [];
const ui = new StoryEventUI(event => shown.push(event.key));
const raid = piratesEvent({ semiconductors: 40, armaments: 15 });
assert.equal(raid.text, "A privateer swept down on the colony and made off with all 40 semiconductors and 15 armaments.");
assert.equal(piratesEvent({ semiconductors: 0, armaments: 15 }).text, "A privateer swept down on the colony and made off with 15 armaments.");
assert.doesNotMatch(raid.text, /trader|Interceptor|sell/i, "the story, not the next action");
// §traders: the founder's own trader docks.
const teo = { name: "Teo Brannick", ship: "Long Odds" };
assert.equal(freeTraderEvent(teo).text, "Teo Brannick's Long Odds has docked at your colony. Word is its hold carries semiconductors.");
assert.equal(freeTraderEvent({ name: "Ines Varga", ship: "Clean Slate" }).text,
  "Ines Varga's Clean Slate has docked at your colony. Word is its hold carries semiconductors.");
// §traders: until the painting lands, the trader's own ship stands in.
assert.equal(freeTraderEvent({ slug: "ines", name: "Ines Varga", ship: "Clean Slate" }).art.interim.src,
  "/art/derived/ships/models-v1/previews/clean_slate-768.webp");
assert.match(source("boot.ts"), /freeTraderEvent\(traderOf\(state\.founding\)\)/, "the one the server dealt");

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
ui.show(piratesEvent({ semiconductors: 40, armaments: 15 }));
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
ui.show(piratesEvent({ semiconductors: 40, armaments: 15 }));
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
assert.deepEqual(chapters.map(e => e.title), ["Strike back", "Lanes open", "Into the belt"]);
assert.equal(chapters[0].text, "The Shipyard stands. Jace Marrow is still out there.", "the nephew by name");
assert.equal(chapterEvent("pirates").text, "The Shipyard stands. The privateer is still out there.");
for (const event of chapters) {
  const slug = /story-events\/v1\/(.+)-1280\.webp/.exec(event.art.src)[1];
  assert.match(build, new RegExp(`"${slug}"`), `${slug} is built`);
  assert.ok(event.art.alt && event.art.interim.alt, `${slug}: alternative text for both images`);
  assert.ok(existsSync(new URL(`../public${event.art.interim.src}`, import.meta.url)), `${slug}: its hull stands in`);
}
assert.match(source("boot.ts"), /storyEvents\.show\(chapterEvent\(event\.chapter, pirateNames\(state\.founding\)\.privateer\)\)/);
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

// The guide never tells the story itself.
for (const file of ["core/derive/founding.ts", "shell/deck/command.ts", "shell/mobile/surfaces.ts", "shell/deck/log.ts"]) {
  const code = source(file).replace(/^\s*\/\/.*$/gm, "");
  assert.doesNotMatch(code, /made off with|swept down/, `${file} keeps to what happens and what to do`);
}
console.log("story events: the raid on its report, then the free trader on the server's docking line (3 s after a pop-up closes); queue, escaping, interims and a guide without the story; the GNN stories' paintings and broadcast headers; the relics stir once; the chapters open painted");
