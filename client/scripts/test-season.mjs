// §season (Oct 29 2026): decrees, the GNN ticker and the Log's News tab
// (core/derive/season.ts), and their wiring on both shells.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (file) => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
const ageLabel = (s) => s < 90 ? `${s.toFixed(0)}s` : s < 5400 ? `${(s / 60).toFixed(0)}m` : `${(s / 3600).toFixed(0)}h`;
const season = {};
vm.runInNewContext(ts.transpileModule(src("core/derive/season.ts"), { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText,
{ exports: season, require: (m) => (m === "./format" ? { ageLabel } : {}) });

// The decree's log line names its grip; nothing else does.
assert.equal(season.decreeOf("Authority decree · Grip II · Levies."), "levies");
assert.equal(season.decreeOf("Authority decree · Grip IV · Martial Charter."), "martial");
assert.equal(season.decreeOf("Authority decree · Grip V · Nonsense."), null);
assert.equal(season.decreeOf("Charter milestone · Landfall · 500 Cr."), null);
assert.equal(season.gripNumeral("inspections"), "III");
assert.match(season.DECREE_TEXT.levies, /levy on every Hub sale and purchase/);

const view = {
  maturity: 0.314, grip: "levies", census_at: 100,
  news: [
    { id: 3, heard_at: 900, source: "free_signal", kind: "resolve", headline: "Liberators rise", about_you: false },
    { id: 2, heard_at: 600, source: "gnn", kind: "decree", headline: "Authority decree: a levy on every Hub trade", about_you: false },
    { id: 1, heard_at: 300, source: "gnn", kind: "capture", headline: "Alpha takes Vega from Beta", about_you: true, system: "S1" },
  ],
};
assert.equal(season.tickerItem(view).id, 3, "the ticker shows the newest item heard");
assert.equal(season.tickerItem(null), null);
assert.equal(season.seasonLine(view), "Grip II · Levies · Galaxy maturity 31%");
const rows = season.newsRows(view, 930, 500);
assert.deepEqual(rows.map((r) => r.tone), ["good", "warn", "info"], "Free Signal good, decrees warn, the rest info");
assert.deepEqual(rows.map((r) => r.unread), [true, true, false], "heard after the read mark is unread");
assert.equal(rows[0].source, "Free Signal");
assert.equal(rows[0].meta, "Resolve · now");
assert.equal(rows[2].meta, "Capture · 11m ago");
assert.equal(rows[2].yours, true);
assert.equal(rows[2].system, "S1");
assert.deepEqual(rows.map((r) => r.emblem), ["/art/derived/gnn-v1/free-signal-emblem-48.webp", "/art/derived/gnn-v1/gnn-emblem-48.webp",
  "/art/derived/gnn-v1/gnn-emblem-48.webp"], "each row by its source's emblem");
for (const file of ["gnn-emblem-48", "gnn-emblem-128", "free-signal-emblem-48", "free-signal-emblem-128"]) {
  assert.ok(existsSync(new URL(`../public/art/derived/gnn-v1/${file}.webp`, import.meta.url)), `${file} derived`);
}
assert.equal(season.freshNews(view, 500), 2);
assert.equal(season.latestNews(view), 900);

// Wiring: the pop-up, the ticker, both News tabs.
assert.match(src("core/session.ts"), /const grip = decreeOf\(entry\.text\);\n\s+if \(grip\) return \{ kind: "StoryEvent", story: "decree", grip \};/);
assert.match(src("boot.ts"), /event\.story === "decree" && !state\.storyTold\.has\(`decree_\$\{event\.grip\}`\)\) storyEvents\.show\(decreeEvent\(event\.grip\)\)/);
assert.match(src("shell/deck/markup.ts"), /<div id="deck-stores-main" class="deck-stores__main"><\/div><button type="button" id="deck-news" class="deck-news" hidden><\/button>/);
assert.match(src("shell/deck/index.ts"), /this\.news\?\.render\(\);/);
assert.match(src("shell/deck/log.ts"), /data-tab="news"/);
assert.match(src("shell/mobile/log.ts"), /data-tab="news"/);
assert.match(src("shell/deck/news.ts"), /<img src="\$\{newsEmblem\(item\.source\)\}" alt="\$\{source\}" decoding="async"><span>/, "the ticker leads with the emblem");
assert.match(src("shell/deck/log.ts"), /<img class="deck-newsrow__source" src="\$\{esc\(row\.emblem\)\}"/, "desk News rows");
assert.match(src("shell/mobile/log.ts"), /<img class="m-newsrow__source" src="\$\{esc\(row\.emblem\)\}"/, "phone News rows");
assert.match(src("core/session.ts"), /if \(msg\.season\) st\.season = msg\.season;/);
// §season phase 2: the squeeze — the Hub's cut, embargoes, demand text and its wiring.
assert.ok(Math.abs(season.hubCut({ market_penalty_frac: 0.02, levy_frac: 0.07 }) - 0.09) < 1e-12);
assert.equal(season.hubCut(null), 0);
const charter = { squeeze: { embargoes: [{ commodity: "alloys", arrears: 5000, since: 1 }] } };
assert.equal(season.isEmbargoed(charter, "alloys"), true);
assert.equal(season.isEmbargoed(charter, "polymers"), false);
const name = (c) => ({ alloys: "Alloys" })[c] ?? c;
assert.equal(season.demandText({ kind: "goods", commodity: "alloys", units: 300 }, name), "300 Alloys");
assert.equal(season.demandText({ kind: "credits", amount: 4000.4 }, name), "4,000 Cr");
const inbox = src("core/derive/inbox.ts");
assert.match(inbox, /command: \{ type: "AnswerLevy", accept: true \}/, "Inbox: accept a levy rise");
assert.match(inbox, /command: \{ type: "AnswerRequisition", id: req\.id, pay: true \}/, "Inbox: pay a requisition");
assert.match(inbox, /command: \{ type: "PayArrears", commodity: embargo\.commodity \}/, "Inbox: pay arrears");
assert.match(src("shell/deck/market.ts"), /const canSubmit = !!account && !!quote && limitReady && stockReady && !embargoed;/, "desk: no ticket for an embargoed good");
assert.match(src("shell/deck/strategic.ts"), /\$\{this\.gripHtml\(charter\)\}/, "desk Faction: the grip");
assert.match(src("shell/mobile/parity.ts"), /\$\{this\.gripHtml\(charter\)\}/, "phone Faction: the grip");

// §season phase 3: Resolve bands, crackdown lines and evidence.
assert.equal(season.resolveBand(0.1), "loyal");
assert.equal(season.resolveBand(0.3), "restless");
assert.equal(season.resolveBand(0.5), "rebel");
assert.equal(season.crackdownOf("Crackdown · Sabotage at the Hub · Hub fees doubled until 14:00 · evidence at the scene."), "sabotage");
assert.equal(season.crackdownOf("Crackdown · Quarantine at Vega · Alpha named · Authority freight suspended until 14:00."), "quarantine");
assert.equal(season.crackdownOf("Crackdown · Convoy massacre · Alpha named · bounty and enforcement until 14:00."), "massacre");
assert.equal(season.crackdownOf("Crackdown · Keystone crackdown at Vega · Alpha named · enforcement at its builders until 14:00."), "keystone");
assert.equal(season.crackdownTitle("keystone"), "Keystone crackdown");
assert.equal(season.crackdownOf("Charter milestone · Landfall · 500 Cr."), null);
const scene = { maturity: 0.5, grip: "inspections", census_at: 1, news: [], galaxy_resolve: 0.3, war_fleet: 19, crackdowns: [
  { id: 1, kind: "sabotage", at: 0, until: 500, pos: { x: 0, y: 0 }, evidence_until: 900, exposed: false, read: false },
  { id: 2, kind: "massacre", at: 0, until: 500, pos: { x: 9, y: 9 }, evidence_until: 900, exposed: true, read: false },
  { id: 3, kind: "massacre", at: 0, until: 500, pos: { x: 9, y: 9 }, evidence_until: 900, exposed: false, read: true },
  { id: 4, kind: "quarantine", at: 0, until: 500, pos: { x: 9, y: 9 }, evidence_until: 0, exposed: false, read: false },
] };
assert.deepEqual(season.openEvidence(scene, 100).map((c) => c.id), [1], "only open, unexposed, unread evidence");
assert.deepEqual(season.openEvidence(scene, 950), [], "evidence goes cold");
assert.match(src("core/session.ts"), /if \(crackdown && \(crackdown === "sabotage" \|\| entry\.severity === "bad"\)\) return \{ kind: "StoryEvent", story: "crackdown"/,
  "a crackdown pops up when it strikes you or the Hub");
assert.match(src("boot.ts"), /storyEvents\.show\(exposedEvent\(event\.text, event\.at\)\)/);
assert.match(src("core/derive/inbox.ts"), /for \(const c of openEvidence\(st\.season, now\)\)/, "evidence is an Inbox opportunity");
assert.match(src("core/derive/market.ts"), /o\.key === "assembly_hall" && \(state\?\.founding\?\.charter_gates\?\.assembly_hall \|\| \(state\?\.season\?\.grip \?\? "charter"\) === "charter"\)/,
  "no Assembly Hall before the levies");

// §season phase 4: bearings, the search zone and the journey.
const east = { relic: "R1", from: { x: 0, y: 0 }, angle: 0, spread: 0.1, by: "me", at: 1 };
const north = { relic: "R2", from: { x: 100, y: -100 }, angle: Math.PI / 2, spread: 0.1, by: "me", at: 1 };
assert.equal(season.bearingHolds(east, { x: 100, y: 5 }), true);
assert.equal(season.bearingHolds(east, { x: 100, y: 50 }), false);
assert.equal(season.bearingHolds(east, { x: -100, y: 0 }), false, "a wedge points one way");
const stars = [{ id: "A", pos: { x: 100, y: 0 } }, { id: "B", pos: { x: 100, y: 200 } }, { id: "C", pos: { x: 300, y: 0 } }];
assert.deepEqual([...season.searchZone([east], stars)], [], "one bearing makes no zone");
assert.deepEqual([...season.searchZone([east, north], stars)], ["A"], "two bearings cross over the world");
assert.equal(season.attuneSecs(0.3), 24 * 3600);
assert.equal(season.attuneSecs(1), 4 * 3600);
const journeyView = { ...scene, relics: [{ system: "R1", awake_at: 0, resonance: 0.5, drawn: false }], bearings: [east], precursors: [], war_at: 4000, rebel_since: null, rebels: [], race_since: null };
const steps = season.journeySteps(journeyView, 0.6, stars, (id) => `Star ${id}`, 1000, 4 * 3600);
assert.deepEqual([...steps].map((step) => step.title), ["Attune Star R1", "One more bearing crosses it", "An Authority spy"]);
assert.equal(steps[0].facts, "50% · ~2h left");
assert.equal(season.journeySteps(journeyView, 0.3, stars, (id) => id, 1000, 3600)[0].facts, "attuning needs Resolve 50%");
assert.match(src("render.ts"), /this\.drawSeason\(state\);/, "the map draws bearings and sealed worlds");
assert.match(src("core/session.ts"), /"Sealed world found · "\)\) return \{ kind: "StoryEvent", story: "journey", beat: "found"/);

// §season phase 5: the Keystone's feed, the ending, its wiring.
const keystoneView = { ...journeyView, keystone: { system: "K", level: 3, basket: [["alloys", 1200, 400], ["machinery", 480, 480]], relics_needed: 0, relics_held: 1, powers: false, gates: [] } };
assert.deepEqual([...season.keystoneFeed(keystoneView, { alloys: 950.7 })].map((row) => [...row]), [["alloys", 1200, 400, 950], ["machinery", 480, 480, 0]]);
assert.equal(season.endingText({ kind: "sovereign", at: 1, holder: "Alpha", system: "K", winners: ["Alpha"], won: true }).title, "The Seal fires");
assert.match(season.endingText({ kind: "charter_holds", at: 1, holder: null, system: null, winners: ["Beta"], won: false }).text, /Honoured: Beta\./);
assert.match(src("shell/deck/strategic.ts"), /type: "FeedKeystone", system_id: button\.dataset\.system/, "desk: feed the Keystone");
assert.match(src("shell/deck/strategic.ts"), /type: "GateStep", fleet_id: button\.dataset\.fleet/, "desk: Gate Step");
assert.match(src("shell/mobile/parity.ts"), /data-mobile-act="faction-feed"/, "phone: feed the Keystone");
assert.match(src("core/session.ts"), /const endedNow = !!msg\.season\?\.ending && !!st\.season && !st\.season\.ending;/, "the ending pops up once");

console.log("PASS: decrees (log line → pop-up), GNN ticker item, News rows, the squeeze (Hub cut, embargo, demands, Inbox, Faction), Resolve bands, crackdowns and evidence, bearings, the search zone and the journey, the Keystone and the ending, and wiring on both shells.");
