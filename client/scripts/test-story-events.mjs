// §charter-v5: the charter's story turns pop up as illustrated events; the
// founding guide keeps to the next action. No game socket.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
function compile(text, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: () => ({}), ...globals });
  return exports;
}

// A fresh raid report becomes a story event, not a toast, and never replays.
const { applyServerMessage } = compile(source("core/session.ts"));
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
// A session that opens on old lines was told them then.
const later = { timeline: [], awaySet: false, storyTold: new Set() };
const resumed = applyServerMessage({ type: "Timeline", entries: [line, docking], away_since: 0 }, later);
assert.ok(!resumed.some(e => e.kind === "StoryEvent"), "no pop-up for old news");
assert.ok(later.storyTold.has("free_trader") && later.storyTold.has("pirates"), "nothing withheld either");

// One native modal: escaped, queued, followed up, and closed with the session.
class Dialog {
  open = false; innerHTML = ""; className = ""; handlers = {};
  setAttribute() {} addEventListener(type, fn) { this.handlers[type] = fn; }
  showModal() { this.open = true; }
  close() { if (!this.open) return; this.open = false; this.handlers.close?.(); }
  querySelector(selector) {
    if (selector !== "img") return { focus() {} };
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
const { StoryEventUI, freeTraderEvent, piratesEvent, FOLLOW_UP_MS } = compile(source("story-events.ts"), { document, setTimeout, clearTimeout, Date });
const shown = [];
const ui = new StoryEventUI(event => shown.push(event.key));
const raid = piratesEvent({ semiconductors: 40, armaments: 15 });
assert.equal(raid.text, "A privateer swept down on the colony and made off with all 40 semiconductors and 15 armaments.");
assert.equal(piratesEvent({ semiconductors: 0, armaments: 15 }).text, "A privateer swept down on the colony and made off with 15 armaments.");
assert.doesNotMatch(raid.text, /trader|Interceptor|sell/i, "the story, not the next action");
assert.equal(freeTraderEvent().text, "Teo Brannick's Long Odds has docked at your colony. Word is its hold carries semiconductors.");

// Each painting is prompted; until it lands, its interim stands in.
const prompts = JSON.parse(readFileSync(new URL("../art-src/story-events-v1/prompts.json", import.meta.url), "utf8"));
for (const event of [raid, freeTraderEvent()]) {
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
dialog.image.listeners.error();
assert.equal(dialog.image.src, raid.art.interim.src, "a missing painting falls back to the interim");
assert.ok(dialog.image.classes.has("is-icon"), "the privateer's sprite stands in at 2×, its frame kept 16:9");
press();
assert.equal(timers.length, 0, "closing the raid announces no trader by itself");
// The server's docking line arrives while a pop-up is open: the trader waits
// for it to close, then FOLLOW_UP_MS.
ui.show(piratesEvent({ semiconductors: 40, armaments: 15 }));
ui.showAfterGap(freeTraderEvent(), FOLLOW_UP_MS);
assert.equal(timers.length, 0);
press();
assert.equal(timers.length, 1);
assert.equal(timers[0].ms, 15_000, "fifteen seconds after it closes");
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
ui.closedAt = Date.now() - 20_000;
ui.showAfterGap(freeTraderEvent(), FOLLOW_UP_MS);
assert.ok(dialog.open, "shown at once");
press();

// Queued events wait their turn and are text, not markup.
ui.show(freeTraderEvent(false));
ui.show({ title: "<b>Next</b>", text: "&", art: { src: "/x.webp", alt: "x" } });
press();
assert.match(dialog.innerHTML, /&lt;b&gt;Next&lt;\/b&gt;/);
press();
assert.ok(!dialog.open);

// A replaced session drops the queue and the pending trader.
ui.show(piratesEvent({ semiconductors: 40, armaments: 15 }));
ui.showAfterGap(freeTraderEvent(), FOLLOW_UP_MS);
press();
ui.endSession();
assert.equal(timers.at(-1).fn, null, "the pending trader is cancelled");
assert.ok(!dialog.open);

// The guide never tells the story itself.
for (const file of ["core/derive/founding.ts", "shell/deck/command.ts", "shell/mobile/surfaces.ts", "shell/deck/log.ts"]) {
  const code = source(file).replace(/^\s*\/\/.*$/gm, "");
  assert.doesNotMatch(code, /made off with|swept down/, `${file} keeps to what happens and what to do`);
}
console.log("story events: the raid on its report, then the free trader on the server's docking line (15 s after a pop-up closes); queue, escaping, interims and a guide without the story");
