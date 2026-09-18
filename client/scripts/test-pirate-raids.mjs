// Delayed journal receipts, not hidden NPC orders, drive both shells' warning.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
const { applyServerMessage } = compile(source("core/session.ts"));
const state = { timeline: [], awaySet: false };
const warning = { at_time: 300, severity: "bad", text: "Pirate raid incoming: Freya — 2 raiders. Assemble defenses or intercept the approach." };
const receipt = entries => applyServerMessage({ type: "Timeline", entries, away_since: 100 }, state);
assert.equal(receipt([warning]).length, 1, "reconnect retains the log without replaying a stale alert");
assert.equal(receipt([warning]).length, 1, "the same digest does not repeat a warning");
const next = { ...warning, at_time: 1300 };
const events = receipt([warning, next]);
const alert = events.find(event => event.kind === "PirateRaidWarning");
assert.equal(alert.message, next.text);
assert.equal(receipt([warning, next]).length, 1);
assert.equal(receipt([next, { ...next, text: "Pirates breaking off from Freya. No stockpile goods taken." }]).length, 1);

// Execute the desktop handler's real method without mounting or changing a game.
const deckSource = source("shell/deck/index.ts");
const file = ts.createSourceFile("deck.ts", deckSource, ts.ScriptTarget.Latest, true);
const owner = file.statements.find(node => ts.isClassDeclaration(node)
  && node.members.some(member => member.name?.getText(file) === "toastFor"));
const method = owner.members.find(member => member.name?.getText(file) === "toastFor").getText(file);
const { NoticeFixture } = compile(`export class NoticeFixture { ${method} }`);
const fixture = new NoticeFixture(), notices = [];
fixture.toasts = { push: notice => notices.push(notice) };
fixture.toastFor(alert);
assert.equal(notices[0].tone, "bad");
assert.equal(notices[0].destination.name, "log");
assert.equal(notices[0].message, next.text);
assert.match(source("shell/mobile/index.ts"), /event.kind === "PirateRaidWarning"[\s\S]*?destination: \{ id: "log" \}/);
console.log("PASS pirate raid warnings: arrived receipts, deduplication, reconnect, desktop notice, mobile log destination");
