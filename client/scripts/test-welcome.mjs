// §welcome (R9): the server's report on return as the desktop card draws it,
// from a fixture. It never connects to a live game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {
  "../../core/derive/founding": { fmtHours: (s) => `${Math.floor(s / 3600)}h ${Math.floor(s / 60) % 60}m`, foundingPlan: () => null },
};
function compile(path) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {} });
  return exports;
}
const { serverWelcomeHtml } = compile("shell/deck/welcome.ts");

const report = {
  away: 3 * 3600 + 20 * 60,
  happened: [
    { severity: "bad", text: "Blockade at Vega <b>", go: { kind: "system", id: "41" } },
    { severity: "good", text: "Sold 40 ferrite ore · +400 Cr", go: { kind: "market" } },
    { severity: "info", text: "Charter milestone" },
  ],
  waiting: [{ severity: "info", text: "2 Signals waiting", go: { kind: "signals" } }],
  next: [
    { label: "Defend Vega", go: { kind: "system", id: "41" } },
    { label: "Answer a Signal", go: { kind: "signals" } },
    { label: "Queue a hull at Home", go: { kind: "shipyard", system: "7", body: 2 } },
  ],
};

const html = serverWelcomeHtml(report, [], null);
assert.match(html, /Welcome back · away 3h 20m/);
assert.match(html, /<li class="is-bad"><button type="button" class="deck-welcome__line" data-deck-act="welcome-go" data-go="system" data-id="41">Blockade at Vega &lt;b&gt;<\/button><\/li>/, "lines escape and lead somewhere");
assert.match(html, /<li class="is-info">Charter milestone<\/li>/, "a line without a destination is plain text");
assert.match(html, /<small>Waiting<\/small><ul><li class="is-info"><button[^>]*data-go="signals"/);
assert.match(html, /<button type="button" class="is-primary" data-deck-act="welcome-go" data-go="system" data-id="41">Defend Vega<\/button>/, "the most urgent action leads");
assert.match(html, /data-go="shipyard" data-id="7">Queue a hull at Home/);
assert.doesNotMatch(html, /data-deck-act="welcome-next"/, "no guide step after the charter");

// While the charter runs, its step leads and two server actions follow.
const step = { title: "Export your ore", label: "Open the Market", copy: "Sell at the hub.", reason: null, alternatives: [] };
const charter = serverWelcomeHtml(report, ["Barge mining · 40%"], step);
assert.match(charter, /<small>Under way<\/small><ul><li>Barge mining · 40%<\/li><\/ul>/);
assert.match(charter, /class="is-primary" data-deck-act="welcome-next">Open the Market<\/button>/);
assert.equal((charter.match(/data-deck-act="welcome-go" data-go="[a-z]+"[^>]*>(Defend|Answer|Queue)/g) ?? []).length, 2);
assert.doesNotMatch(charter, /Queue a hull at Home<\/button>/, "the third action yields to the guide's step");

console.log("welcome: server report card ok");
