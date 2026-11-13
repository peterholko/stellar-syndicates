// §charter-advisor (docs/charter-advisor.md): the Authority officer dealt to
// guide a charter: the cast and their art, the step lines and their limits,
// which line fits the guide's action, what outranks it, the moments they
// speak and the stall nudge. No game socket.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const repo = path => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
function compile(text, globals = {}, deps = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, ...globals });
  return exports;
}
const vendetta = compile(source("core/derive/vendetta.ts"));
const founding = compile(source("core/derive/founding.ts"), {}, { "./vendetta": vendetta });
const advisor = compile(source("core/derive/advisor.ts"), {}, { "./founding": founding });
const { ADVISORS, STEP_LINES, ACTION_LINES } = advisor;
const words = line => line.trim().split(/\s+/).length;
// The modules run in their own realm: compare plain copies.
const copy = value => JSON.parse(JSON.stringify(value));
const eq = (actual, expected, message) => assert.deepEqual(copy(actual), copy(expected), message);

// ─── The cast: three officers, dealt by the server (FoundingView.advisor) ───
eq(ADVISORS.map(a => [a.slug, a.name]), [["veteran", "Commander Imara Okafor"],
  ["lieutenant", "Lieutenant Daniel Seo"], ["frontier", "Major Rurik Halvorsen"]]);
for (const a of ADVISORS) {
  assert.equal(a.portrait, `/art/derived/advisors/v1/${a.slug}-320.webp`);
  assert.equal(a.face, `/art/derived/advisors/v1/${a.slug}-face-128.webp`);
  for (const art of [a.portrait, a.face]) assert.ok(existsSync(new URL(`../public${art}`, import.meta.url)), `${art} is built`);
  assert.ok(existsSync(new URL(`../art-src/charter-advisors-v1/${a.slug}.png`, import.meta.url)), `${a.slug}'s master`);
  for (const [moment, line] of Object.entries(a.moments)) assert.ok(words(line) <= 16, `${a.slug} ${moment}: ${words(line)} words`);
  eq(Object.keys(a.moments), ["intro", "raid", "left", "blocked", "goodbye", "freighter_lost"]);
  // §advisor-cards (Peter, Nov 9 2026): the cards they voice in their own
  // words, "snarky, cynical and quasi insulting", to "instill some hate for
  // the Authority": at most 18 words with the longest names filled in.
  eq(Object.keys(a.cards), ["laser_battery", "free_trader", "chapter_pirates", "privateer_contract"]);
  for (const card of Object.keys(a.cards)) {
    const line = advisor.cardLine(a, card, { trader: "Mags Halloran's Second Wind", privateer: "Lazlo Marrow" });
    assert.ok(words(line) <= 18 && !/[{}]/.test(line), `${a.slug} ${card}: "${line}" is ${words(line)} words`);
  }
  assert.match(a.cards.laser_battery, /Authority|regulation/, `${a.slug}: the Authority's indifference, in their voice`);
  assert.match(a.cards.free_trader, /^\{trader\} [^.]*docked[\s\S]*semiconductors/, `${a.slug}: who docked, and what it carries`);
  assert.match(a.cards.chapter_pirates, /\{privateer\}/, `${a.slug}: the pirate by name`);
  // §privateer-contract (Peter, Nov 9 2026): "there just happens to be a
  // contract out to eliminate the Pirate": the Authority's, on him by name.
  assert.match(a.cards.privateer_contract, /Authority[\s\S]*\{privateer\}|\{privateer\}[\s\S]*Authority/, `${a.slug}: the Authority's contract on him`);
}
// §privateer-contract: the stall nudge on the contract's step has its own line.
assert.equal(advisor.adviceKey({ title: "Take the Authority's contract", action: "contract", label: "Open Contracts" }, "defeat_privateer"), "contract");
assert.ok(words(ACTION_LINES.contract) <= 12 && /contract/i.test(ACTION_LINES.contract));
// Names filled in; one opening a sentence takes a capital.
assert.equal(advisor.cardLine(ADVISORS[0], "chapter_pirates"),
  "A Shipyard. Finally. The privateer is still out there, laughing at you. Build something that shoots.");
assert.equal(advisor.cardLine(ADVISORS[1], "chapter_pirates", { privateer: "Lazlo Marrow" }),
  "Shipyard operational, behind schedule. Regulations permit you to hunt Lazlo Marrow yourself. The Authority certainly won't.");
assert.equal(advisor.cardLine(ADVISORS[2], "free_trader", { trader: "Ines Varga's Clean Slate" }),
  "Ines Varga's Clean Slate docked. Free traders would sell their own mothers. Lucky you: they also sell semiconductors.");
assert.match(readFileSync(new URL("./build-founding-story-art.mjs", import.meta.url), "utf8"),
  /ADVISOR_PORTRAIT_ART = \["veteran", "lieutenant", "frontier"\]/, "the art build derives all three");
assert.match(readFileSync(new URL("./build-art-derivatives.mjs", import.meta.url), "utf8"), /buildAdvisorPortraitArt\(\),/);
const f0 = { stage: "build_habitat", local_opening: true, advisor: 2, survey_candidates: [] };
assert.equal(advisor.charterAdvisor(f0).slug, "frontier");
assert.equal(advisor.charterAdvisor({ ...f0, advisor: undefined }), null, "none before the server deals one");
assert.equal(advisor.charterAdvisor({ ...f0, advisor: 7 }), null, "an unknown officer is no officer");
assert.equal(advisor.charterAdvisor({ ...f0, local_opening: false }), null, "the charter's guide only");
assert.equal(advisor.advisorOnDuty({ ...f0, stage: "complete" }), null, "they leave when the charter is fulfilled");
assert.equal(advisor.charterAdvisor({ ...f0, stage: "complete" }).slug, "frontier", "but say goodbye");
assert.equal(advisor.advisorTitle(ADVISORS[0]), "Commander Imara Okafor, Charter Advisor");
// Peter (Nov 9 2026): at the raid, "something cynical & insulting to the player".
assert.equal(ADVISORS[0].moments.raid, "Typical pirates, preying on the weak and disorganized.");

// ─── Step lines: 29 charter steps and the two opening client steps ──────────
const checklist = founding.FOUNDING_CHAPTERS.flatMap(([, steps]) => steps);
assert.equal(checklist.length, 29);
eq(Object.keys(STEP_LINES).sort(), ["open_home", "open_world", ...checklist.map(([s]) => s)].sort(), "31 entries, one per step");
const family = { family: "collector", surname: "Marrow", nephew: "Lazlo Marrow", uncle: "Mordecai Marrow", brother: "Silas Marrow" };
const named = { ...f0, pirates: family };
for (const [key, line] of [...Object.entries(STEP_LINES), ...Object.entries(ACTION_LINES)]) {
  for (const text of [line, founding.withPirateNames(line, named)]) assert.ok(words(text) <= 12, `${key}: "${text}" is ${words(text)} words`);
  assert.doesNotMatch(line, /\d/, `${key}: no numbers`);
}
for (const [stage, title] of checklist) {
  assert.ok(!STEP_LINES[stage].toLowerCase().includes(title.toLowerCase()), `${stage}'s line doesn't repeat its title`);
}
// §no-spoilers: nothing before the raid names a pirate or the trader.
const preRaid = ["open_home", "open_world", "build_habitat", "power_shipyard", "build_bioharvester", "build_agroplex", "build_mine", "build_smelter"];
for (const key of preRaid) assert.doesNotMatch(STEP_LINES[key], /pirat|privateer|raid|trader|den\b/i, key);
assert.doesNotMatch(ACTION_LINES.staff, /pirat|privateer|raid|trader/i, "staffing comes before the raid too");
assert.doesNotMatch(ACTION_LINES.find_semiconductors, /trader/i, "the Shipyard's need, before the trader is told");

// ─── Which line: the action the card actually shows ──────────────────────────
const key = (step, stage) => advisor.adviceKey({ title: "", label: "Build", ...step }, stage);
assert.equal(key({ title: founding.HOME_STEP_TITLE, action: "open-home" }, "build_habitat"), "open_home");
assert.equal(key({ title: founding.HOME_WORLD_STEP_TITLE, action: "home-world" }, "build_habitat"), "open_world");
assert.equal(key({ title: "Build a Habitat", action: "build-habitat" }, "build_habitat"), "build_habitat");
assert.equal(key({ title: "Staff the Bioharvester", action: "bioharvester", label: "Assign workforce" }, "build_bioharvester"), "staff");
assert.equal(key({ title: "Staff Freya III's mine", action: "expansion-mine", label: "Assign workforce" }, "expand_mining"), "staff");
assert.equal(key({ title: "Assign a captain", action: "assign-captain" }, "build_interceptor"), "captain");
assert.equal(key({ title: "Bring the Interceptor home", action: "interceptor-captain" }, "build_interceptor"), "captain_dock");
assert.equal(key({ title: "Buy the Shipyard parts", action: "merchant-shipyard" }, "build_shipyard"), "trader_parts");
assert.equal(key({ title: "Buy the repair parts", action: "merchant-repair" }, "repair_interceptor"), "trader_parts");
assert.equal(key({ title: "Sell Alloys to the trader", action: "merchant-sell" }, "sell_ore"), "sell_ore");
assert.equal(key({ title: "Find semiconductors", action: "market" }, "sell_ore"), "find_semiconductors");
// Aliases: every served stage, retired ones too, lands on a step with a line.
for (const stage of Object.keys(founding.FOUNDING_STEP)) {
  const at = advisor.stepStage(stage);
  assert.equal(founding.FOUNDING_STEP[at], founding.FOUNDING_STEP[stage], `${stage} → ${at}`);
  assert.ok(STEP_LINES[at], `${stage} has a line`);
}
assert.equal(advisor.stepStage("buy_semiconductors"), "build_shipyard");
assert.equal(advisor.stepStage("build_academy"), "first_research");
assert.equal(advisor.stepStage("export_production"), "prepare_export");
assert.equal(advisor.stepStage("complete"), "vendetta");

// ─── What outranks the advisor's line ────────────────────────────────────────
const card = (primary, blocked = false) => ({ primary: { copy: "", label: "Build", ...primary }, blocked, progress: [], waiting: [],
  checklist: [], chapter: { name: "Landfall", done: 0, total: 8 } });
const solar = { title: "Build a Solar Array", action: "build-power", why: "Light the colony." };
const at = stage => ({ ...f0, stage });
assert.equal(advisor.stepAdvice(card(solar), at("power_shipyard")), STEP_LINES.power_shipyard, "it speaks in the why's place");
assert.equal(advisor.stepAdvice(card({ ...solar, reason: "Bounty: 8,000 Cr." }), at("power_shipyard")), null, "a live reason outranks it");
assert.equal(advisor.stepAdvice(card(solar, true), at("power_shipyard")), null, "a blocker keeps its warning");
assert.equal(advisor.stepAdvice(card({ ...solar, status: "Solar Array under construction · 0:20" }), at("power_shipyard")), null, "a wait keeps its line");
assert.equal(advisor.stepAdvice(card({ ...solar, quiet: true }), at("power_shipyard")), null, "an interruption keeps its line");
assert.equal(advisor.stepAdvice(card(solar), { ...at("power_shipyard"), advisor: undefined }), null, "no officer, no line");
assert.equal(advisor.stepAdvice(card(solar), at("complete")), null, "gone after the charter");
assert.equal(advisor.stepAdvice(null, at("power_shipyard")), null);
// The nudge's caption ignores what outranks the card's line.
assert.equal(advisor.actionAdvice({ ...solar, reason: "Bounty: 8,000 Cr." }, at("power_shipyard")), STEP_LINES.power_shipyard);
// Moments: in the Authority's gold. Peter (Nov 9 2026): no "Terran Charter
// Authority" header over them ("doesn't make sense").
const intro = advisor.advisorEvent("intro", ADVISORS[1], () => true);
eq([intro.key, intro.title, intro.text, intro.speaker.speaker, intro.speaker.tone, intro.speaker.portrait],
  ["advisor_intro", "Assigned to your charter", "", "Lieutenant Daniel Seo", "authority", ADVISORS[1].portrait]);
assert.equal(intro.source, undefined, "the plain header");
assert.equal(intro.speaker.line, ADVISORS[1].moments.intro);
assert.equal(advisor.advisorMemoryKey("galaxy-1", "7"), 'ss_advisor_v1:["galaxy-1","7"]');
assert.equal(advisor.advisorMemoryKey(undefined, "7"), null, "no instance, no memory");
assert.equal(advisor.advisorMemoryKey("galaxy-1", null), null);

// ─── Chapters and crates: the advisor speaks the existing text, once ─────────
class Dialog {
  open = false; innerHTML = ""; className = ""; handlers = {};
  setAttribute() {} addEventListener(type, fn) { this.handlers[type] = fn; }
  showModal() { this.open = true; }
  close() { if (!this.open) return; this.open = false; this.handlers.close?.(); }
  querySelector() { return { focus() {}, addEventListener() {}, removeAttribute() {}, parentElement: { classList: { add() {} } } }; }
}
let dialog = null;
const document = { createElement: () => (dialog = new Dialog()), body: { append() {} } };
const story = compile(source("story-events.ts"), { document, setTimeout: () => 0, clearTimeout() {}, Date },
  { "./protocol": compile(source("protocol.ts")), "./core/derive/season": compile(source("core/derive/season.ts")),
    "./icons": compile(source("icons.ts")), "./core/derive/vendetta": vendetta });
const voice = line => advisor.advisorSpeaker(ADVISORS[0], line);
const plain = story.chapterEvent("pirates", "Lazlo Marrow");
const spoken = story.chapterEvent("pirates", "Lazlo Marrow", null, voice);
assert.equal(spoken.text, "", "not repeated below");
assert.equal(spoken.speaker.line, plain.text, "the chapter's own text, as the advisor's line");
assert.equal(spoken.speaker.speaker, "Commander Imara Okafor");
eq(spoken.art, plain.art, "the painting stays");
const crate = { id: "reach", released: true, goods: { mining_barge_kit: 1 } };
const names = { privateer: "Lazlo Marrow", den: "the Marrow den", uncle: "Mordecai Marrow" };
let accepted = 0;
const crateSpoken = story.crateEvent(crate, names, () => null, () => accepted++, () => true, voice);
assert.equal(crateSpoken.speaker.line, story.CRATE_TEXT.reach(names));
assert.equal(crateSpoken.text, "");
assert.equal(crateSpoken.accept.label, "Accept", "the crate is still accepted");
assert.equal(typeof crateSpoken.panel, "function", "its goods still on show");
assert.equal(story.crateEvent(crate, names, () => null, () => {}).speaker, undefined, "no officer on duty: as before");
for (const text of [plain.text, story.chapterEvent("reach", "x", { ore: "metallic_ore", richness: 1.25 }).text,
  ...Object.values(story.CRATE_TEXT).map(t => t({ ...names, uncle: "Mordecai Marrow" }))]) {
  assert.ok(words(text) <= 16, `spoken as a moment line: "${text}"`);
}
// The pop-up frames the advisor in gold; `then` runs once it closes.
const ui = new story.StoryEventUI();
let closed = 0;
ui.show({ ...advisor.advisorEvent("blocked", ADVISORS[2], () => true, () => closed++) });
assert.match(dialog.innerHTML, /class="story-transmission is-alone is-authority"/);
assert.match(dialog.innerHTML, /<strong>Major Rurik Halvorsen<\/strong><q>Start with the warning\. That&#39;s what&#39;s holding us up\.<\/q>/);
assert.equal(ui.open, true);
dialog.close();
assert.equal(closed, 1, "then runs when it closes");
assert.equal(ui.open, false);
// Peter (Nov 9 2026: "a large Charter Advisor panel"): the advisor alone
// speaks large; with a painting or a crate, as before.
ui.show(advisor.advisorEvent("left", ADVISORS[2], () => true));
assert.match(dialog.className, /\bis-advisor\b/);
dialog.close();
ui.show(spoken);
assert.doesNotMatch(dialog.className, /is-advisor/, "a chapter's painting keeps its frame");
dialog.close();
ui.show(crateSpoken);
assert.match(dialog.className, /\bis-crate\b/);
assert.doesNotMatch(dialog.className, /is-advisor/, "a crate keeps its panel");
dialog.close();
assert.match(source("styles/settlement.css"), /\.story-event\.is-advisor \.story-transmission img \{ width: 220px; height: 220px;/);

// ─── The desk: moments once per account and galaxy, and the stall nudge ──────
const desk = compile(source("advisor-desk.ts"), {}, { "./core/derive/advisor": advisor, "./core/derive/founding": founding });
function rig(over = {}) {
  const env = {
    founding: { ...f0, advisor: 0, stage: "power_shipyard" }, settling: false, foreground: true, control: "enabled",
    told: new Set(), plan: card(solar), orders: 0, coached: [], timers: [],
    storage: over.storage ?? new Map(), scope: { instance: "galaxy-1", playerId: "7" },
  };
  const queue = { open: false, events: [], gaps: [],
    has: k => queue.events.some(e => e.key === k) || queue.gaps.some(e => e.key === k),
    show(e) { queue.events.push(e); }, showAfterGap(e, gap) { queue.gaps.push({ ...e, gap }); } };
  const d = new desk.AdvisorDesk(queue, {
    founding: () => env.founding, scope: () => env.scope, storyTold: k => env.told.has(k), plan: () => env.plan,
    settling: () => env.settling, foreground: () => env.foreground, nextControl: () => env.control,
    coachNext: (caption, a) => env.coached.push([caption, a.slug]), orders: () => env.orders,
    storage: { getItem: k => env.storage.get(k) ?? null, setItem: (k, v) => env.storage.set(k, v) },
    timer: (run, ms) => env.timers.push({ run, ms }),
    raidVoiced: () => !!env.voiced,
    freighters: () => env.freighters ?? 0,
  });
  // The pop-up queue's onShow: what goes on screen is remembered.
  const showAll = () => { for (const e of queue.events.splice(0)) if (!e.due || e.due()) d.shown(e.key); };
  return { env, queue, d, showAll };
}
{
  // §freighter-lost (Peter, Nov 11 2026: "When the player loses their Tiny
  // Freighter, the Authority Advisor should point out something like 'It is
  // odd that a pirate would destroy a freighter and its valuable haul. I
  // suspect you have upset a criminal lowlife...'"): once, after the pop-up
  // on screen, while the officer is on duty.
  const { env, queue, d, showAll } = rig();
  env.freighters = 0;
  d.update(); showAll();
  env.freighters = 1;
  d.update();
  assert.equal(queue.gaps.length, 0, "a freighter built is no loss");
  env.freighters = 1;
  d.update();
  assert.equal(queue.gaps.length, 0, "nor one still afloat");
  env.freighters = 0;
  d.update();
  eq(queue.gaps.map(e => [e.key, e.gap, e.title]), [["advisor_freighter_lost", desk.FREIGHTER_LOST_GAP_MS, "Freighter lost"]]);
  assert.equal(queue.gaps[0].speaker.line, ADVISORS[0].moments.freighter_lost);
  assert.match(ADVISORS[2].moments.freighter_lost, /destroying a freighter and its valuable haul\? I suspect you've upset a criminal lowlife/,
    "Peter's line, in Halvorsen's mouth");
  d.shown(queue.gaps[0].key);
  queue.gaps.length = 0;
  env.freighters = 1; d.update();
  env.freighters = 0; d.update();
  assert.equal(queue.gaps.length, 0, "said once per account and galaxy");
  // Lost after the charter: the officer has gone.
  const late = rig();
  late.env.founding = { ...late.env.founding, stage: "complete" };
  late.env.freighters = 1; late.d.update();
  late.env.freighters = 0; late.d.update();
  assert.ok(!late.queue.gaps.some(e => e.key === "advisor_freighter_lost"), "no officer, no word");
  // A new galaxy starts its own count: its first view is no loss.
  const moved = rig();
  moved.env.freighters = 3; moved.d.update();
  moved.env.scope = { instance: "galaxy-2", playerId: "7" };
  moved.env.freighters = 0; moved.d.update();
  assert.ok(!moved.queue.gaps.some(e => e.key === "advisor_freighter_lost"), "another galaxy's fleet is not a loss");
}
{
  // The introduction waits for the landing card, then comes once.
  const { env, queue, d, showAll } = rig();
  env.settling = true;
  d.update();
  assert.equal(queue.events.length, 0, "not over the landing card");
  env.settling = false;
  d.update();
  d.update();
  eq(queue.events.map(e => e.key), ["advisor_intro"], "once, not twice");
  assert.equal(queue.events[0].speaker.speaker, "Commander Imara Okafor");
  showAll();
  assert.ok(d.told("intro"));
  d.update();
  assert.equal(queue.events.length, 0, "told");
  // A reload in this browser: remembered for this account and galaxy.
  const again = rig({ storage: env.storage });
  again.d.update();
  assert.equal(again.queue.events.length, 0, "not after a reload");
  const elsewhere = rig({ storage: env.storage });
  elsewhere.env.scope = { instance: "galaxy-2", playerId: "7" };
  elsewhere.d.update();
  eq(elsewhere.queue.events.map(e => e.key), ["advisor_intro"], "a new galaxy meets its officer");
}
{
  // The raid: when command sees the raider, never before; not after its report.
  const { env, queue, d, showAll } = rig();
  d.update(); showAll();
  env.founding = { ...env.founding, stage: "pirate_raid", raid_seen: false };
  d.update();
  assert.equal(env.timers.length, 0, "§no-spoilers: nothing before the sighting");
  env.founding = { ...env.founding, raid_seen: true };
  d.update(); d.update();
  assert.equal(env.timers.length, 1, "once, after the camera's turn");
  assert.equal(env.timers[0].ms, desk.RAID_MOMENT_DELAY_MS);
  env.timers[0].run();
  eq(queue.events.map(e => e.key), ["advisor_raid"]);
  assert.equal(queue.events[0].speaker.line, ADVISORS[0].moments.raid);
  env.told.add("pirates");
  assert.equal(queue.events[0].due(), false, "dropped once the raid's report is told");
  // §smelter-raid (Peter, Nov 9 2026): the privateer sets out on the
  // Smelter's order, so the sighting can come while the Smelter is the step.
  const early = rig();
  early.d.update(); early.showAll();
  early.env.founding = { ...early.env.founding, stage: "build_smelter", raid_seen: true };
  early.d.update();
  early.env.timers[0].run();
  eq(early.queue.events.map(e => e.key), ["advisor_raid"], "at the Smelter's step too");
  early.env.founding = { ...early.env.founding, raided: true };
  assert.equal(early.queue.events[0].due(), false, "not once its report is in");
  // Peter (Nov 9 2026): on the desktop the advisor speaks inside the raid's
  // warning (the tracking window); no pop-up then, and it counts as said.
  const desk2 = rig();
  desk2.d.update(); desk2.showAll();
  desk2.env.founding = { ...desk2.env.founding, stage: "pirate_raid", raid_seen: true };
  desk2.env.voiced = true;
  desk2.d.update();
  desk2.env.timers[0].run();
  eq(desk2.queue.events.map(e => e.key), [], "no pop-up over the warning");
  assert.ok(desk2.d.told("raid"), "said in the warning");
  const late = rig();
  late.d.update(); late.showAll();
  late.env.founding = { ...late.env.founding, stage: "pirate_raid", raid_seen: true };
  late.env.told.add("pirates");
  late.d.update();
  assert.equal(late.env.timers.length, 0, "a page loaded after the report stays quiet");
}
{
  // The first actionable blocker: said once, then its action is pointed at.
  const { env, queue, d, showAll } = rig();
  d.update(); showAll();
  const room = { title: "Make room at home", action: "build-warehouse", label: "Build Warehouse", reason: "Build a Warehouse, or ship goods out." };
  env.founding = { ...env.founding, stage: "pirate_raid", raid_seen: true };
  env.plan = card({ title: "Raider closing on Freya", status: "Raider closing on Freya · ETA 0:20", action: "privateer" }, true);
  d.tick(1000);
  assert.equal(queue.events.length, 0, "not the scripted raid");
  env.founding = { ...env.founding, stage: "prepare_export" };
  env.plan = card({ title: "Shipment underway", status: "Shipment heading to the Market Hub · ETA 2:00", action: "shipment-trip" }, true);
  d.tick(1000);
  assert.equal(queue.events.length, 0, "not a wait");
  env.plan = card({ title: "Raider over Freya III", action: "attack-strike", reason: "Bounty: 500 Cr." }, true);
  d.tick(1000);
  assert.equal(queue.events.length, 0, "not a strike: the uncle's transmission tells it");
  env.plan = card(room, true);
  d.tick(1000); d.tick(1000);
  eq(queue.events.map(e => e.key), ["advisor_blocked"], "once");
  assert.equal(queue.events[0].due(), true);
  queue.events[0].then();
  eq(env.coached, [["Build a Warehouse, or ship goods out.", "veteran"]], "then the action, with its warning");
  showAll();
  env.plan = card({ ...room, title: "Fuel the Interceptor", reason: undefined }, true);
  d.tick(1000);
  assert.equal(queue.events.length, 0, "only the first");
  const solved = rig();
  solved.d.update(); solved.showAll();
  solved.env.plan = card(room, true);
  solved.d.tick(1000);
  solved.env.plan = card(solar);
  assert.equal(solved.queue.events[0].due(), false, "dropped when it no longer stands");
}
{
  // The goodbye: to a charter they guided, after its last pop-ups.
  const { env, queue, d, showAll } = rig();
  env.founding = { ...env.founding, stage: "complete" };
  d.update();
  assert.equal(queue.gaps.length, 0, "a charter they never guided");
  const met = rig();
  met.d.update(); met.showAll();
  met.env.founding = { ...met.env.founding, stage: "complete" };
  met.d.update(); met.d.update();
  eq(met.queue.gaps.map(e => [e.key, e.gap]), [["advisor_goodbye", desk.GOODBYE_GAP_MS]]);
  assert.equal(met.queue.gaps[0].speaker.line, ADVISORS[0].moments.goodbye);
  void queue; void showAll;
}
{
  // §watch-the-pirate (Peter, Nov 9 2026): as the raider drops off the
  // colony's sensors after its raid, a large panel of the advisor's scorn.
  const { env, queue, d, showAll } = rig();
  d.update(); showAll();
  env.founding = { ...env.founding, stage: "build_gun_platform", raid_seen: true, raided: true, raid_lost: false };
  d.update();
  assert.equal(queue.gaps.length, 0, "not while it is still in sight");
  assert.equal(d.leftOwed(), false);
  env.founding = { ...env.founding, raid_lost: true };
  // §laser-battery-card: the Laser Battery's card waits behind it, from the
  // raider's going until the panel is on screen.
  assert.equal(d.leftOwed(), true, "owed before it is even queued");
  d.update(); d.update();
  eq(queue.gaps.map(e => [e.key, e.title, e.gap]), [["advisor_left", "The raider is gone", desk.LEFT_GAP_MS]], "once, after the report's pop-up");
  assert.equal(queue.gaps[0].speaker.line, ADVISORS[0].moments.left);
  assert.equal(d.leftOwed(), true, "queued, not yet shown");
  d.shown("advisor_left");
  assert.equal(d.leftOwed(), false, "on screen: the card queues behind it");
  const none = rig();
  none.d.update(); none.showAll();
  none.env.founding = { ...none.env.founding, stage: "build_gun_platform", raid_seen: true, raided: true, raid_lost: true, advisor: undefined };
  assert.equal(none.d.leftOwed(), false, "no advisor, no panel to wait for");
  const old = rig();
  old.d.update(); old.showAll();
  old.env.founding = { ...old.env.founding, stage: "build_shipyard", raid_seen: true, raided: true, raid_lost: true };
  old.d.update();
  assert.equal(old.queue.gaps.length, 0, "past the trader's step it is old news");
}
assert.equal(ADVISORS[2].moments.left, "Better start smelting that ore to buy back what you lost. You call yourself an entrepreneur?!",
  "Peter's line, to the limit");
{
  // The stall nudge: three foreground minutes on an unstarted action, once a step.
  const { env, queue, d, showAll } = rig();
  d.update(); showAll();
  const minutes = n => { for (let i = 0; i < n * 60; i++) d.tick(1000); };
  minutes(2);
  env.foreground = false;
  minutes(2);
  env.foreground = true;
  queue.open = true;
  minutes(1);
  queue.open = false;
  env.control = "none";
  minutes(1);
  env.control = "disabled";
  minutes(1);
  env.control = "enabled";
  assert.equal(env.coached.length, 0, "hidden, behind a pop-up, or out of reach: the clock waits");
  minutes(1);
  eq(env.coached, [[STEP_LINES.power_shipyard, "veteran"]], "the advice for the action, with the face");
  minutes(5);
  assert.equal(env.coached.length, 1, "once per step");
  assert.ok(d.nudged("2:power_shipyard"));
  const reload = rig({ storage: env.storage });
  for (let i = 0; i < 240; i++) reload.d.tick(1000);
  assert.equal(reload.env.coached.length, 0, "remembered across a reload");
  // The clock restarts on a new action, a fresh order, or the guide's button.
  const busy = rig();
  for (let i = 0; i < 170; i++) busy.d.tick(1000);
  busy.env.plan = card({ ...solar, title: "Buy the Solar Array parts", action: "merchant-power_plant" });
  for (let i = 0; i < 170; i++) busy.d.tick(1000);
  busy.env.orders = 1;
  for (let i = 0; i < 170; i++) busy.d.tick(1000);
  busy.d.acted();
  for (let i = 0; i < 170; i++) busy.d.tick(1000);
  assert.equal(busy.env.coached.length, 0, "never three idle minutes");
  for (let i = 0; i < 10; i++) busy.d.tick(1000);
  eq(busy.env.coached, [[ACTION_LINES.trader_parts, "veteran"]], "the substep's own line");
  // Never a blocked, waiting or finished action.
  for (const plan of [card(solar, true), card({ ...solar, status: "Solar Array under construction · 1:00" }), card({ ...solar, quiet: true })]) {
    const idle = rig();
    idle.env.plan = plan;
    for (let i = 0; i < 400; i++) idle.d.tick(1000);
    eq(idle.env.coached.filter(([, slug]) => slug).length, 0);
  }
  const done = rig();
  done.env.founding = { ...done.env.founding, stage: "complete" };
  for (let i = 0; i < 400; i++) done.d.tick(1000);
  assert.equal(done.env.coached.length, 0, "the advisor has left");
}

// ─── Wiring: server, protocol, both shells, boot ─────────────────────────────
assert.match(repo("crates/server/src/protocol.rs"), /pub const PROTOCOL_VERSION: u32 = 91;/);
assert.match(repo("crates/server/src/wire.rs"), /"stellar\.msgpack\.v91"/);
assert.match(source("wire.mjs"), /export const PROTOCOL_VERSION = 91;/);
// §watch-the-pirate (v88): the colony lost the privateer after its raid, as heard.
assert.match(repo("crates/server/src/game_loop.rs"), /raid_lost: corp\.founding\.raid_lost_at\.is_some_and\(\|at\| at <= now\),/);
assert.match(source("protocol.ts"), /raid_lost\?: boolean;/);
assert.match(repo("crates/server/src/protocol.rs"), /pub advisor: u8,/);
assert.match(repo("crates/server/src/game_loop.rs"), /advisor: charter_advisor\(&self\.galaxy_instance_id, self\.world\.config\.seed, player_id\),/);
assert.match(source("protocol.ts"), /advisor\?: number;/);
// Peter (Nov 9 2026): the face right beside the line, no quotes, the step's
// title above it; nothing of the advisor in the header.
for (const [shell, line] of [["shell/deck/founding-guide.ts", /<div class="deck-founding__advisor"><img src="\$\{esc\(advisor\.face\)\}"[^`]*<p>\$\{esc\(advice\)\}<\/p><\/div>/],
  ["shell/mobile/surfaces.ts", /<div class="m-founding__advisor"><img src="\$\{esc\(advisor\.face\)\}"[^`]*<p>\$\{esc\(advice\)\}<\/p><\/div>/]]) {
  const text = source(shell);
  assert.match(text, line, `${shell}: the advisor's face beside their line`);
  assert.doesNotMatch(text, /has-advisor|founding__advice"|<q /, `${shell}: no face in the header, no quotes`);
  assert.match(text, /alt="\$\{esc\(advisorTitle\(advisor\)\)\}"/, `${shell}: the face, named`);
  assert.match(text, /data-coach="founding-next"/, `${shell}: the nudge's target`);
  assert.match(text, /stepAdvice\(plan, founding\)/);
}
const boot = source("boot.ts");
assert.match(boot, /new AdvisorDesk\(storyEvents, \{/);
assert.match(boot, /settlement\.onMessage\(message\);\n\s*advisorDesk\.update\(\);/, "after the landing card's own update");
assert.match(boot, /setInterval\(\(\) => advisorDesk\.tick\(1_000\), 1_000\);/);
assert.match(boot, /raidVoiced: \(\) => renderer\.raidVoiced\(\),/, "the raid's warning voices the advisor on the desktop");
assert.match(boot, /advisorDesk\.shown\(event\.key\);/);
assert.match(boot, /voice: \(\) => voiceOf\(state\.founding\),/, "the crates speak through the advisor");
assert.match(boot, /coach\("founding-next", caption, "right", \{ src: advisor\.face, alt: advisorTitle\(advisor\) \}\)/, "beside the guide card");
console.log("charter advisor: three officers dealt and drawn, 31 step lines within 12 words, the action's line under live reasons, "
  + "no spoilers, moments once per account and galaxy, the first blocker pointed at, and the stall nudge once a step");
