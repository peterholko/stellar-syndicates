// §medals: weekly medals and the corporation card, from a published rankings
// table fixture. It never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {} });
  return exports;
}
const copy = (value) => JSON.parse(JSON.stringify(value));

deps["./format"] = compile("core/derive/format.ts");
const medals = deps["../core/derive/medals"] = compile("core/derive/medals.ts");
const card = compile("shell/corpcard.ts");

const row = (id, name, extra = {}) => ({ player_id: id, name, valuation: 0, trade_throughput: 0, market_profit: 0,
  cargo_captured: 0, cargo_protected: 0, battle_efficiency: 0, battle_engagements: 0, battle_ranked: false,
  systems_developed: 0, intel_gathered: 0, recovery: 0, titles: [], ...extra });
const rows = [
  row("7", "Vega Mining", { valuation: 9_000, trade_throughput: 400, titles: ["Magnate"],
    medals: [{ category: "valuation", gold: 2, silver: 0, bronze: 1 }, { category: "intel_gathered", gold: 0, silver: 1, bronze: 0 }] }),
  row("3", "Orion & Sons", { valuation: 5_000, trade_throughput: 900, battle_ranked: true, battle_efficiency: 2.5 }),
  row("12", "Kepler Freight", { valuation: 5_000, trade_throughput: 100 }),
];

assert.deepEqual(copy(medals.medalTotals(rows[0])), [2, 1, 1]);
assert.deepEqual(copy(medals.medalTotals(rows[1])), [0, 0, 0], "an older server sends no medals");
// Week 1 runs to 7 days; a week and a day in is week 2 with six days left.
assert.deepEqual(copy(medals.medalWeek(0)), { week: 1, left: 604_800 });
assert.deepEqual(copy(medals.medalWeek(8 * 86_400)), { week: 2, left: 6 * 86_400 });
assert.equal(medals.fmtDays(6 * 86_400 + 3 * 3_600), "6d 3h");
assert.equal(medals.fmtDays(3 * 3_600 + 20 * 60), "3h 20m");

const vega = medals.corpCard(rows, "7", "12", 8 * 86_400);
assert.equal(vega.me, false);
assert.equal(vega.week, 2);
assert.deepEqual(copy(vega.medals.map((m) => m.slug)), ["valuation", "intel_gathered"], "only the categories it has medals in");
assert.deepEqual(copy(vega.medals[0]), { slug: "valuation", label: "Valuation", gold: 2, silver: 0, bronze: 1 });
const standing = Object.fromEntries(vega.standing.map((s) => [s.label, [s.rank, s.of]]));
assert.deepEqual(standing["Valuation"], [1, 3]);
assert.deepEqual(standing["Trade throughput"], [2, 3]);
assert.deepEqual(standing["Battle efficiency"], [null, 3], "provisional: no rank");
assert.deepEqual(standing["Cargo captured"], [null, 3], "nothing to rank at zero");
assert.equal(medals.corpCard(rows, "12", "12", 0).me, true);
assert.equal(medals.corpCard(rows, "99", "12", 0), null, "not in the published table");
// Ties keep the table's lowest id first.
assert.deepEqual(Object.fromEntries(medals.corpCard(rows, "12", null, 0).standing.map((s) => [s.label, s.rank]))["Valuation"], 3);

const html = card.corpCardHtml(vega, "<button>All rankings</button>");
assert.match(html, /<h2>Vega Mining<\/h2>/);
assert.match(html, /<em>Magnate<\/em>/);
assert.match(html, /Week 2 · awards in 6d 0h/);
assert.match(card.medalChipsHtml(rows[0]), /is-gold">2<\/i><i class="medal is-silver">1<\/i><i class="medal is-bronze">1/);
assert.equal(card.medalChipsHtml(rows[1]), "", "no medals, no chips");
assert.match(card.corpCardHtml(medals.corpCard([row("1", "<b>")], "1", null, 0), ""), /<h2>&lt;b&gt;<\/h2>/, "names are escaped");
assert.match(card.corpCardHtml(medals.corpCard([row("1", "New")], "1", null, 0), ""), /None yet\./, "no medals yet");
// §contract-board: today's contracts, open ones first, and the bonus line.
deps["./medals"] = medals;
const board = compile("core/derive/board.ts");
const op = (id, state, day) => ({ id, state, board_day: day, progress: 0, goal: 1 });
const st = { simTime: 100_800, board: { day: 1, next_at: 172_800, done: 2, needed: 3, bonus_credits: 1_000, bonus_paid: false },
  operations: [op("9", "completed", 1), op("4", "offered", 1), op("7", "active", 1), op("2", "offered", 0), { id: "5", state: "offered" }] };
const summary = board.boardSummary(st);
assert.deepEqual(copy(summary.contracts.map((o) => o.id)), ["7", "4", "9"], "today's only: active, offered, then done");
assert.equal(board.boardLine(summary), "2/3 for +1,000 Cr · new in 20h 0m");
st.board.bonus_paid = true;
assert.equal(board.boardLine(board.boardSummary(st)), "Bonus paid · new in 20h 0m");
st.board = null;
assert.equal(board.boardSummary(st), null, "no board before the charter is fulfilled");
console.log("PASS: weekly medals, week clock, the corporation card and the contract board.");
