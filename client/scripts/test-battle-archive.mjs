// §battle-archive: the Log's Battles list (both shells) — verdicts from the
// viewer's side, names, newest-first rows, the Deck tab switch and the Open
// route a map marker uses. It never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { logFixture, logState } from "./log-harness.mjs";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const copy = (value) => JSON.parse(JSON.stringify(value));
function compile(text, globals = {}, deps = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(text, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {}, ...globals });
  return exports;
}

const names = { wadjet: "Wadjet", freya: "Freya" };
const archive = compile(src("core/derive/battlearchive.ts"), {}, {
  "./format": { fmtBuildDur: (secs) => secs < 120 ? `${Math.round(secs)}s` : `${Math.round(secs / 60)}m` },
  "./geo": { systemName: (id) => names[id] ?? id, locName: () => "Dabih" },
});

const side = (corp) => ({ corp, posture: null, platform_tiers: 0, initial: [] });
const record = (id, started_at, outcome, own_side, extra = {}) => ({
  id, pos: { x: 0, y: 0 }, system: "wadjet", started_at, raid: false, fidelity: "participant", own_side,
  sides: [side("pirates"), side("me")], rounds: [{ tick: 0, counts: [[], []], kills: [[], []], dealt: null, notes: [] }],
  light_frontier_tick: 0, outcome, ...extra,
});
const verdict = (rec) => copy(archive.battleVerdict(rec));

// Verdicts, from the side that fought, else neutral.
assert.deepEqual(verdict(record("a", 0, null, 1)), { label: "Ongoing", tone: "live" });
assert.deepEqual(verdict(record("a", 0, "attacker_destroyed", 1)), { label: "Won", tone: "good" });
assert.deepEqual(verdict(record("a", 0, "target_destroyed", 0)), { label: "Won", tone: "good" });
assert.deepEqual(verdict(record("a", 0, "target_destroyed", 1)), { label: "Lost", tone: "bad" });
assert.deepEqual(verdict(record("a", 0, "attacker_destroyed", 0)), { label: "Lost", tone: "bad" });
assert.deepEqual(verdict(record("a", 0, "both_destroyed", 1)), { label: "Both destroyed", tone: "bad" });
assert.deepEqual(verdict(record("a", 0, "escaped", 1)), { label: "Escaped", tone: "good" });
assert.deepEqual(verdict(record("a", 0, "escaped", 0)), { label: "Target escaped", tone: "quiet" });
assert.deepEqual(verdict(record("a", 0, "both_survive", 1)), { label: "Both survived", tone: "quiet" });
const withdrew = record("a", 0, "both_survive", 1, { rounds: [{ tick: 0, counts: [[], []], kills: [[], []], dealt: null,
  notes: [{ kind: "withdraw_ordered", side: 1 }] }] });
assert.deepEqual(verdict(withdrew), { label: "Withdrew", tone: "quiet" }, "your own withdrawal");
const theyLeft = record("a", 0, "both_survive", 1, { rounds: [{ tick: 0, counts: [[], []], kills: [[], []], dealt: null,
  notes: [{ kind: "retreat_tripped", side: 0 }] }] });
assert.deepEqual(verdict(theyLeft), { label: "Both survived", tone: "quiet" }, "their retreat is not yours");
assert.deepEqual(verdict(record("a", 0, "target_destroyed", null)), { label: "Attacker won", tone: "quiet" }, "watched, not fought");

// Names: you, pirates, known corporations, else a rival.
const st = {
  playerId: "me", galaxy: { pirate_id: "pirates" }, rankings: [{ player_id: "greeks", name: "Greeks" }],
  diplomacy: { relations: [{ other: "vega", name: "Vega Mining Combine" }], incoming: [] },
  syndicate: { members: [{ id: "ally", name: "Ally Corp", role: "member" }] }, battles: [],
  battleRecords: [],
};
assert.deepEqual(["me", "pirates", "greeks", "vega", "ally", "stranger"].map((id) => archive.corpLabel(st, id)),
  ["You", "Pirates", "Greeks", "Vega Mining Combine", "Ally Corp", "Rival"]);

// Rows: newest first, your side first, deep space by its nearest star.
st.battleRecords = [
  record("old", 100, "attacker_destroyed", 1),
  record("new", 900, null, 1),
  record("deep", 500, "both_survive", null, { system: null, sides: [side("greeks"), side("vega")] }),
];
st.battles = [{ id: "new" }];
const rows = copy(archive.archiveRows(st, 1_000));
assert.deepEqual(rows.map((row) => row.id), ["new", "deep", "old"]);
assert.deepEqual(rows[0], { id: "new", ago: "100s ago", where: "Wadjet", sides: "You vs Pirates", outcome: "Ongoing", tone: "live", running: true });
assert.equal(rows[1].where, "Near Dabih");
assert.equal(rows[1].sides, "Greeks vs Vega Mining Combine");
assert.equal(rows[2].outcome, "Won");
assert.equal(rows[2].ago, "15m ago");

// The Deck's Log: Battles live under History's Combat filter (§log); the old
// "battles" tab route lands there. The real class, over the harness.
const desk = logFixture(logState({ ...st, battleRecords: st.battleRecords, battles: st.battles }));
const battlesRoute = { name: "log", query: { tab: "battles" } };
desk.deckLog.render(battlesRoute);
let html = desk.root.innerHTML;
assert.match(html, /data-tab="history" aria-selected="true"/);
assert.match(html, /data-filter="combat" aria-pressed="true">Combat/);
assert.ok(html.indexOf('data-battle="new"') < html.indexOf('data-battle="deep"'), "newest first");
assert.match(html, /<b>Wadjet<\/b><small>100s ago · You vs Pirates<\/small><\/span><em>Ongoing<\/em>/);
assert.match(html, /<span>Engagements<\/span><b>3<\/b><\/h4>/, "the count");
assert.equal(desk.deckClick({ deckAct: "log-battle-open", battle: "new" }, battlesRoute), true);
assert.equal(desk.deckClick({ deckAct: "log-battle-open", battle: "old" }, battlesRoute), true);
assert.deepEqual(desk.calls.routes, [
  { name: "battle", params: { id: "new", label: "Ongoing battle" } },
  { name: "battle", params: { id: "old", label: "Battle replay" } },
], "the same route a map marker opens");
assert.equal(desk.deckClick({ deckAct: "log-battle-open", battle: "x" }, { name: "fleets" }), false, "only on the Log");
desk.deckClick({ deckAct: "log-filter", filter: "all" }, battlesRoute);
assert.doesNotMatch(desk.root.innerHTML, /Engagements/, "the list rides the Combat filter");
desk.ctx.state.battleRecords = [];
desk.deckClick({ deckAct: "log-filter", filter: "combat" }, battlesRoute);
assert.doesNotMatch(desk.root.innerHTML, /Engagements/, "no records, no list");

// Mobile: the Log sheet's Combat filter lists the same rows and opens the battle sheet.
const phone = logFixture(logState({ ...st, battleRecords: [record("old", 100, "attacker_destroyed", 1), record("new", 900, null, 1)], battles: st.battles }));
const sheet = phone.mobileLog.render({ id: "log", props: { tab: "history", filter: "combat" } }).html;
assert.match(sheet, /class="m-battle-row is-live"><span><b>Wadjet<\/b><small>100s ago · You vs Pirates<\/small><\/span><em>Ongoing<\/em><button type="button" data-mobile-act="battle-open" data-id="new">Open/);
assert.match(src("shell/mobile/log.ts"), /state\.battleRecords\.map\(\(record\) => \[record\.id, record\.outcome\]\)/, "new records redraw the sheet");
assert.match(src("shell/mobile/surfaces.ts"), /case "battle-open":\s*if \(button\.dataset\.id\) this\.hooks\.openSheet\(\{ id: "battle", props: \{ id: button\.dataset\.id \} \}\);/);

console.log("PASS: battle archive verdicts, names, newest-first rows, the Log's Combat filter and Open route (Deck), the phone's Combat list.");
