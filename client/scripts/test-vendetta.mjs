// §vendetta (docs/pirate-vendetta.md): the pirate family's lines and faces,
// the server's "Vendetta · …" lines as story turns, the pop-ups, and the
// guide's last chapter. No game socket.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = name => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (name, deps = {}, globals = {}) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source(name), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: id => deps[id] ?? {}, structuredClone, ...globals });
  return exports;
};
const plain = value => JSON.parse(JSON.stringify(value));
const vendetta = load("core/derive/vendetta.ts");

const cast = { family: "showboat", surname: "Marrow", nephew: "Jace Marrow", uncle: "Mordecai Marrow", brother: "Rook Marrow" };
const st = { playerId: "1", simTime: 100, commandCenter: { x: 0, y: 0 },
  galaxy: { c: 400, systems: [{ id: "home", name: "Alphard", pos: { x: 0, y: 0 } }], hyperlimit: 900 },
  systems: [{ id: "home", owner: "1" }], ghosts: [], battles: [], pendingOrders: new Map(), storyTold: new Set(), captains: [], explorationSites: [],
  founding: { stage: "build_barge", local_opening: true, pirates: cast, vendetta: null, stipends: [], survey_candidates: [] } };

// Every family speaks at every turn; names and the colony are filled in, and
// each face is a derived portrait.
const families = ["showboat", "hothead", "collector", "scrapper", "fatalist"];
const beats = ["raid", "kill", "den", "sworn", "arrival", "won", "lost", "freighter_strike", "fallen"];
for (const family of families) {
  for (const beat of beats) {
    const t = vendetta.transmission(beat, { ...st, founding: { ...st.founding, pirates: { ...cast, family } } });
    assert.ok(t.line.length > 3 && !/[{}]/.test(t.line), `${family} ${beat}: ${t.line}`);
    const [role, mood] = vendetta.SPEAKER[beat];
    assert.equal(t.speaker, role === "uncle" ? "Mordecai Marrow" : "Jace Marrow");
    assert.equal(t.portrait, `/art/derived/pirates/v1/${family}${role === "uncle" ? "-uncle" : ""}-${mood}-320.webp`);
    assert.ok(existsSync(new URL(`../public${t.portrait}`, import.meta.url)), `${t.portrait} is derived`);
  }
}
assert.equal(vendetta.transmission("raid", st).line, "Smile for the feeds, Alphard! Jace Marrow thanks you for your donation.");
assert.equal(vendetta.transmission("won", st).line, "Booed off my own stage… Rook will want a rematch.");
assert.equal(vendetta.transmission("raid", { ...st, founding: { ...st.founding, pirates: null } }), null, "no family, no line");
assert.equal(vendetta.colonyName(st), "Alphard");

// The server's lines (timeline.rs), each read as its turn.
const rust = readFileSync(new URL("../../crates/server/src/timeline.rs", import.meta.url), "utf8");
const arms = [...rust.matchAll(/B::(\w+) => \(TimelineSeverity::\w+, format!\("([^"]+)"\)\)/g)];
assert.equal(arms.length, 8, "eight one-line turns from the server (the mine strike names its world)");
const fill = text => text.replace("{nephew}", "Jace Marrow").replace("{uncle}", "Mordecai Marrow")
  .replace("{colony}", "Alphard").replace("{took}", "12 Alloys and 4 Machinery");
for (const [, variant, format] of arms) {
  assert.equal(vendetta.vendettaBeatOf(`Vendetta · ${fill(format)}`), variant.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase(), format);
}
// §vendetta-campaign: the mine strike's line, not his arrival's.
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow's raider is over Freya III."), "mine_strike");
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow is over Alphard."), "arrival");
// §gun-platform: the freighter hunt is a turn; a wreck is a log line; repelled, he got away.
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow's raider is hunting your tiny freighter."), "freighter_strike");
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow's raider wrecked the Mining Complex on Freya III."), null);
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow got away."), "lost");
assert.equal(vendetta.vendettaBeatOf("Vendetta · Mordecai Marrow sacked Alphard."), "fallen");
for (const family of ["showboat", "hothead", "collector", "scrapper", "fatalist"]) {
  for (const beat of ["mine_strike", "barge_strike", "freighter_strike", "fallen"]) assert.ok(vendetta.LINES[family][beat], `${family} ${beat}`);
}
assert.equal(vendetta.vendettaBeatOf("Mordecai Marrow is dead."), null, "only the prefixed lines");

// The anonymous guide copy takes the family's names.
const founding = load("core/derive/founding.ts", { "./vendetta": vendetta });
assert.equal(founding.withPirateNames("Destroy the privateer", st.founding), "Destroy Jace Marrow");
assert.equal(founding.withPirateNames("The privateer's log names its base.", st.founding), "Jace Marrow's log names its base.");
assert.equal(founding.withPirateNames("Eliminate the privateer den", st.founding), "Eliminate the Marrow den");
assert.equal(founding.withPirateNames("Defeat the privateer's uncle", st.founding), "Defeat Mordecai Marrow");
assert.equal(founding.withPirateNames("Destroy the privateer", { ...st.founding, pirates: null }), "Destroy the privateer");

// The pop-ups: the raid carries the nephew's transmission; the arrival is
// painted; the other turns are transmissions alone.
class Dialog {
  open = false; innerHTML = ""; className = ""; handlers = {};
  setAttribute() {} addEventListener(type, fn) { this.handlers[type] = fn; }
  showModal() { this.open = true; }
  close() { if (!this.open) return; this.open = false; this.handlers.close?.(); }
  querySelector() { return { focus() {}, addEventListener() {}, parentElement: { classList: { add() {} } } }; }
}
let dialog = null;
const document = { createElement: () => (dialog = new Dialog()), body: { append() {} } };
const story = load("story-events.ts", {}, { document, setTimeout, clearTimeout, Date });
const raid = story.piratesEvent({ electronics: 40, armaments: 15 }, vendetta.transmission("raid", st));
assert.equal(raid.speaker.speaker, "Jace Marrow", "the raider speaks; the take is the grid");
assert.deepEqual(JSON.parse(JSON.stringify(raid.losses)), [["electronics", 40], ["armaments", 15]]);
const arrival = story.vendettaEvent("arrival", "Mordecai Marrow is over Alphard.", vendetta.transmission("arrival", st));
assert.equal(arrival.key, "vendetta_arrival");
assert.equal(arrival.title, "Mordecai Marrow is here");
assert.match(arrival.art.src, /\/story-events\/v1\/pirate-vendetta-1280\.webp$/);
assert.ok(existsSync(new URL("../art-src/story-events-v1/pirate-vendetta.png", import.meta.url)), "the painting is in");
for (const beat of ["kill", "den", "sworn", "won", "lost"]) {
  assert.equal(story.vendettaEvent(beat, "…", vendetta.transmission(beat, st)).art, undefined, `${beat}: a transmission alone`);
}
const ui = new story.StoryEventUI();
ui.show({ ...story.vendettaEvent("sworn", "Mordecai <Marrow> swore revenge for Jace Marrow.", vendetta.transmission("sworn", st)) });
assert.match(dialog.innerHTML, /class="story-transmission is-alone"/);
assert.match(dialog.innerHTML, /showboat-uncle-angry-320\.webp/);
assert.match(dialog.innerHTML, /<strong>Mordecai Marrow<\/strong><q>You cut my nephew&#39;s final act short\. I&#39;m bringing the encore\.<\/q>/);
assert.match(dialog.innerHTML, /Mordecai &lt;Marrow&gt;/, "the server's line is escaped");
assert.doesNotMatch(dialog.innerHTML, /settlement-story-frame/, "no painting frame");

// Step 19 in the guide: preparation, the countdown, the attack, the end.
const protocol = load("protocol.ts");
const equipment = load("core/derive/equipment.ts", { "../../protocol": protocol });
const clock = { state: st, liveSimTime: () => st.simTime };
const fleets = load("core/derive/fleet.ts", { "../../state": clock, "../../protocol": protocol,
  "./geo": load("core/derive/geo.ts"), "./equipment": equipment });
const icons = load("icons.ts");
const merchants = load("core/derive/merchant.ts", { "./industry": { availableStock: () => 0 } });
const guide = load("core/derive/founding.ts", { "../../protocol": protocol, "./fleet": fleets,
  "./format": { fmtDur: n => `${Math.round(n)}s` }, "../../icons": icons, "./merchant": merchants, "./vendetta": vendetta });
const home = { id: "home", owner: "1", bodies: [{ id: 0, name: "Alphard I", structures: { shipyard: 2 } }],
  assignments: [], builds: [], storage_used: 10, storage_cap: 500 };
st.systems = [home];
const warship = id => ({ id, kind: "raider", own: true, pos: { x: 0, y: 0 }, vel: { x: 0, y: 0 },
  composition: [{ kind: "raider", count: 1 }], cargo_manifest: [] });
st.ghosts = [warship("int1")];

// From step 16: his oath stands, and a lone Interceptor is told it isn't enough.
st.founding.vendetta = { depart_at: null, fleet: null, arrived: false, won: null };
let plan = guide.foundingPlan(st, home);
assert.equal(plan.progress[0], "Mordecai Marrow swore revenge", "the oath, never when he comes");
assert.ok(plan.waiting.some(card => card.title === "Build a second Interceptor" && card.action === "build-interceptor"));

// Step 19, before he sets out.
st.founding.stage = "vendetta";
st.founding.vendetta = { depart_at: 400, fleet: null, arrived: false, won: null };
plan = guide.foundingPlan(st, home);
assert.equal(plan.chapter.name, "Vendetta");
assert.deepEqual(plain(plan.checklist), [{ title: "Secure your stores", done: true }, { title: "Defeat Mordecai Marrow", done: false }]);
assert.ok(plan.progress.includes("Mordecai Marrow sets out in 300s"));
assert.equal(plan.primary.title, "Build a second Interceptor");
assert.match(plan.primary.hint, /within a day regardless/, "Peter: online if possible, never avoidable");
// §vendetta-campaign: three Interceptors for his flagship and four raiders.
st.ghosts.push(warship("int2"));
plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.title, "Build a third Interceptor");
assert.equal(plan.primary.reason, "Mordecai Marrow brings his flagship and four raiders.");
st.ghosts.push(warship("int3"));
plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.title, "Hold your warships at home");
assert.match(plan.primary.copy, /Every warship at rest in Alphard and the Laser Battery join the fight\./);

// Over the colony: one click sends every warship at home at him.
st.ghosts.push({ id: "pack", kind: "corvette", own: false, pos: { x: 500, y: 0 }, vel: { x: 0, y: 0 },
  composition: [{ kind: "corvette", count: 1 }, { kind: "raider", count: 2 }] });
st.founding.vendetta = { depart_at: 400, fleet: "pack", arrived: true, won: null };
plan = guide.foundingPlan(st, home);
assert.equal(plan.primary.action, "attack-uncle");
assert.equal(plan.primary.title, "Attack Mordecai Marrow");
assert.match(plan.primary.reason, /Bounty: 6,000 Cr/);
assert.deepEqual(plain(guide.foundingCommands("attack-uncle", st, home)), [
  { type: "AttackFleet", fleet_id: "int1", target_id: "pack" }, { type: "AttackFleet", fleet_id: "int2", target_id: "pack" },
  { type: "AttackFleet", fleet_id: "int3", target_id: "pack" }]);

// §vendetta-campaign (Peter: "the uncle should harass the player's expanded
// mining ops"): a strike in sight comes before the step's own card.
st.ghosts = st.ghosts.filter(g => g.id !== "pack");
st.founding.stage = "survey_candidates";
st.ghosts.push({ id: "r1", kind: "raider", own: false, pos: { x: 300, y: 0 }, vel: { x: 0, y: 0 }, composition: [{ kind: "raider", count: 1 }] });
st.founding.vendetta = { depart_at: null, fleet: null, arrived: false, won: null, deadline_at: null,
  strikes: [{ kind: "mine", system: "home", body: 0, fleet: "r1", over: false, done: false }] };
plan = guide.foundingPlan(st, home);
assert.deepEqual([plan.primary.title, plan.primary.action, plan.blocked], ["Raider closing on Alphard I", "attack-strike", true]);
st.founding.vendetta.strikes[0].over = true;
plan = guide.foundingPlan(st, home);
assert.deepEqual([plan.primary.title, plan.primary.copy, plan.primary.reason], ["Raider over Alphard I", "It's wrecking the mine.", "Bounty: 500 Cr."]);
// §gun-platform (Peter, Oct 30 2026): it wrecks the world, mine first; nothing is stolen.
st.founding.vendetta.strikes[0].wrecked = ["mining_complex", "power_plant"];
assert.equal(guide.foundingPlan(st, home).primary.copy, "Mining Complex, Solar Array wrecked");
delete st.founding.vendetta.strikes[0].wrecked;
assert.deepEqual(plain(guide.foundingCommands("attack-strike", st, home)).map(c => [c.fleet_id, c.target_id]),
  [["int1", "r1"], ["int2", "r1"], ["int3", "r1"]]);
st.founding.vendetta.strikes = [{ kind: "barge", system: "den", body: 0, fleet: "r1", over: false, done: false }];
plan = guide.foundingPlan(st, home);
assert.deepEqual([plan.primary.title, plan.primary.action], ["Raiders on the barge", "defend-barge"]);
assert.equal(plain(guide.foundingCommands("defend-barge", st, home)).length, 3, "every warship");
// §gun-platform: or a freighter of yours out in open space near home.
const bargeStrike = st.founding.vendetta.strikes;
st.founding.vendetta.strikes = [{ kind: "freighter", system: "home", body: 0, fleet: "r1", over: false, done: false, target: "f1" }];
plan = guide.foundingPlan(st, home);
assert.deepEqual([plan.primary.title, plan.primary.action, plan.blocked], ["Raider hunting your freighter", "attack-strike", true]);
st.founding.vendetta.strikes = bargeStrike;
st.founding.vendetta.strikes[0].done = true;
assert.notEqual(guide.foundingPlan(st, home).primary.action, "defend-barge", "over: back to the step");
st.ghosts = st.ghosts.filter(g => g.id !== "r1");
st.founding.stage = "vendetta";

// Settled: the outcome, until the charter completes on its report.
st.founding.vendetta = { depart_at: 400, fleet: null, arrived: true, won: true };
assert.equal(guide.foundingPlan(st, home).primary.title, "Mordecai Marrow is dead");
st.founding.vendetta.won = false;
assert.equal(guide.foundingPlan(st, home).primary.title, "Mordecai Marrow got away");
// §gun-platform (Peter: "game over and they can restart").
st.founding.vendetta.fallen = true;
plan = guide.foundingPlan(st, home);
assert.deepEqual([plan.primary.title, plan.primary.action, plan.primary.label, plan.blocked], ["Alphard has fallen", "restart-corporation", "Restart", true]);

console.log("Vendetta: 45 lines with derived faces, the server's turns, names in the guide, the transmission pop-ups, the strikes and step 22 pass.");
