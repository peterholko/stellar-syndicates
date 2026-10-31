// Delayed journal receipts, not hidden NPC orders, drive both shells' warning.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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
const season = compile(source("core/derive/season.ts"));
const { applyServerMessage } = compile(source("core/session.ts"), {}, { "./derive/vendetta": vendetta, "./derive/season": season });
const state = { timeline: [], awaySet: false };
// §log (v71): the server tags a home raid's lines; the toast reads the tags.
const raid = { kind: "raid", group: "raid:freya", refs: { system: "freya", fleet: "pack" } };
const warning = { ...raid, phase: "inbound", at_time: 300, severity: "bad", text: "Pirate raid inbound · Freya · 2 raiders · ETA ~4 min.",
  data: { ships: 2, eta: 540 } };
const receipt = entries => applyServerMessage({ type: "Timeline", entries, away_since: 100 }, state);
assert.equal(receipt([warning]).length, 1, "reconnect retains the log without replaying a stale alert");
assert.equal(receipt([warning]).length, 1, "the same digest does not repeat a warning");
const next = { ...warning, at_time: 1300, data: { ships: 2, eta: 1540 } };
const events = receipt([warning, next]);
const alert = events.find(event => event.kind === "LogToast");
assert.equal(alert.entry.text, next.text);
assert.equal(receipt([warning, next]).length, 1);
// The raid's end is news too, once; untagged prose never toasts.
const beaten = receipt([next, { ...raid, phase: "ended", at_time: 1400, severity: "good", text: "Pirates left Freya empty-handed." },
  { at_time: 1401, severity: "bad", text: "Pirate raid inbound · Freya · 2 raiders · ETA ~4 min." }]);
assert.deepEqual([...beaten.filter(event => event.kind === "LogToast").map(event => event.entry.phase)], ["ended"]);

// The toast's words: raiders and ETA, the start, the take or none.
const icons = { label: good => ({ metallic_ore: "Ferrite Ore", provisions: "Provisions" })[good] ?? good };
const history = compile(source("core/derive/history.ts"), {}, { "../../icons": icons });
const name = id => ({ freya: "Freya" })[id] ?? id;
const toast = (entry, now = 1300) => ({ ...history.logToast(entry, name, now) });
assert.deepEqual(toast(next), { title: "Raid inbound · Freya", message: "2 raiders · ETA ~4m", tone: "bad" });
assert.deepEqual(toast(next, 1530), { title: "Raid inbound · Freya", message: "2 raiders · arriving", tone: "bad" });
assert.deepEqual(toast({ ...raid, phase: "attacking" }), { title: "Raid · Freya", message: "Your defenses are fighting them.", tone: "warn" });
assert.deepEqual(toast({ ...raid, phase: "raiding" }), { title: "Raid · Freya", message: "Pirates are stripping its stores.", tone: "bad" });
assert.deepEqual(toast({ ...raid, phase: "ended", data: { goods: { metallic_ore: -46, provisions: -14 } } }),
  { title: "Raided · Freya", message: "Took 46 Ferrite Ore · 14 Provisions", tone: "bad" });
assert.deepEqual(toast({ ...raid, phase: "ended" }), { title: "Raid · Freya", message: "Nothing taken.", tone: "good" });
assert.equal(history.logToast({ ...raid, kind: "blockade", phase: "held" }, name, 0), null, "only raids toast");

// Execute the desktop handler's real method without mounting or changing a game.
const deckSource = source("shell/deck/index.ts");
const file = ts.createSourceFile("deck.ts", deckSource, ts.ScriptTarget.Latest, true);
const owner = file.statements.find(node => ts.isClassDeclaration(node)
  && node.members.some(member => member.name?.getText(file) === "toastFor"));
const method = owner.members.find(member => member.name?.getText(file) === "toastFor").getText(file);
const { NoticeFixture } = compile(`export class NoticeFixture { ${method} }`, {
  logToast: history.logToast, systemName: name, liveSimTime: () => 1300,
});
const fixture = new NoticeFixture(), notices = [];
fixture.toasts = { push: notice => notices.push(notice) };
fixture.toastFor(alert);
const plain = notice => JSON.parse(JSON.stringify(notice));
assert.deepEqual(plain(notices[0]), { title: "Raid inbound · Freya", message: "2 raiders · ETA ~4m", tone: "bad", durationMs: 20_000,
  destination: { name: "log", query: { tab: "inbox" } } }, "a warning lands in the Inbox");
fixture.toastFor({ kind: "LogToast", entry: { ...raid, phase: "ended", data: { goods: { metallic_ore: -46 } } } });
assert.deepEqual(plain(notices[1]), { title: "Raided · Freya", message: "Took 46 Ferrite Ore", tone: "bad", durationMs: 20_000,
  destination: { name: "log", query: { tab: "history", filter: "raids" } } }, "its outcome in History");
const mobile = source("shell/mobile/index.ts");
assert.match(mobile, /event\.kind === "LogToast"[\s\S]*?logToast\(event\.entry, systemName, liveSimTime\(\)\)[\s\S]*?destination: \{ id: "log", props: event\.entry\.phase === "inbound" \? \{ tab: "inbox" \} : \{ tab: "history", filter: "raids" \} \}/);
assert.doesNotMatch(mobile + deckSource, /PirateRaidWarning|PirateRaidNews/, "the prose matchers are gone");
console.log("PASS pirate raid warnings: tagged receipts, deduplication, reconnect, toast words, desktop Inbox/History routes, mobile Log destination");
