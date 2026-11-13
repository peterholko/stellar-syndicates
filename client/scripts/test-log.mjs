// §log (Oct 28 2026): the Log overhaul — History rows from the structured
// journal (v71), the shared Inbox's prefs and rules, and both shells' Log over
// the real modules. No DOM, no socket.
import assert from "node:assert/strict";
import { logFixture, logModules, logState, MemoryStorage, NAMES, plain, src, VALUES } from "./log-harness.mjs";

const { history, inbox } = logModules();
const nameOf = (id) => NAMES[id] ?? id;
const valueOf = (good) => VALUES[good] ?? 0;
const rows = (entries, options = {}) => plain(history.historyRows(entries, { now: 1_000, readThrough: 0, nameOf, valueOf, ...options }));

// ── History rows ──
const raid = { kind: "raid", group: "raid:freya", refs: { system: "freya", fleet: "pack" } };
const inbound = { ...raid, id: 1, phase: "inbound", at_time: 100, severity: "bad", text: "Pirate raid inbound · Freya · 2 raiders · ETA ~4 min.", data: { ships: 2, eta: 340 } };
const raiding = { ...raid, id: 2, phase: "raiding", at_time: 400, severity: "bad", text: "Pirates raiding Freya — they take up to 10% of each good, 60 units in all." };
const ended = { ...raid, id: 3, phase: "ended", at_time: 500, severity: "bad", text: "Raided Freya: pirates took 46 Ferrite Ore, 14 Provisions. Their haul can still be intercepted.",
  data: { goods: { metallic_ore: -46, provisions: -14 } } };
const episode = rows([inbound, raiding, ended]);
assert.equal(episode.length, 1, "a raid's lines are one row");
assert.equal(episode[0].title, "Raided · Freya");
assert.equal(episode[0].detail, "−46 Ferrite Ore · −14 Provisions", "the take, most valuable first");
assert.deepEqual(episode[0].goods, [["metallic_ore", -46], ["provisions", -14]]);
assert.equal(episode[0].entries.length, 3);
assert.equal(rows([inbound])[0].title, "Raid inbound · Freya");
assert.equal(rows([inbound])[0].detail, "2 raiders · arriving", "ETA from the served arrival, against now");
assert.equal(rows([inbound], { now: 100 })[0].detail, "2 raiders · ETA ~4m");
assert.equal(rows([inbound, raiding])[0].title, "Pirate raid · Freya");
assert.equal(rows([inbound, { ...ended, data: undefined, severity: "good", text: "Pirates left Freya empty-handed." }])[0].title, "Raid on Freya · nothing taken");
const again = { ...inbound, id: 4, at_time: 2_000 };
assert.equal(rows([inbound, raiding, ended, again]).length, 2, "an ended episode closes; the next raid is a new row");
assert.equal(rows([inbound, raiding, ended, again])[0].title, "Raid inbound · Freya", "newest first");

const sold = (id, at, n) => ({ id, at_time: at, severity: "good", kind: "trade", text: `Sold ${n} ferrite ore at the hub for 8.22 ea.` });
const trade = rows([sold(10, 100, 10), sold(11, 200, 20), sold(12, 300, 30), sold(13, 300 + 16 * 60, 5)]);
assert.equal(trade.length, 2, "like lines within 15 min collapse; later ones stand alone");
assert.equal(trade[1].detail, "×3");
assert.equal(trade[1].title, "Sold 30 ferrite ore at the hub for 8.22 ea.", "the latest line leads");

const legacy = rows([{ at_time: 5, severity: "info", text: "Construction complete." }, { at_time: 6, severity: "info", text: "Something else." }]);
assert.deepEqual(legacy.map((row) => row.kind), ["fleet", "build"], "pre-v71 lines get a kind from their prose");

const mixed = [inbound, raiding, ended, sold(10, 600, 10), { id: 20, at_time: 700, severity: "info", kind: "build", text: "Construction complete · Nergal.", refs: { system: "nergal" } }];
assert.deepEqual(rows(mixed, { filter: "raids" }).map((row) => row.kind), ["raid"]);
assert.deepEqual(rows(mixed, { filter: "trade" }).map((row) => row.kind), ["trade"]);
assert.deepEqual(rows(mixed, { filter: "all", system: "nergal" }).map((row) => row.kind), ["build"], "a system's history");
assert.deepEqual(rows(mixed, { readThrough: 550 }).map((row) => row.unread), [true, true, false], "unread past the read mark");

const ways = (row) => plain(history.rowActions(row, nameOf)).map((action) => [action.label, action.primary ?? false, action.go]);
assert.deepEqual(ways(episode[0]), [
  ["Show Freya", true, { to: "system", id: "freya" }],
  ["Track the haul", false, { to: "fleet", id: "pack" }],
  ["History of Freya", false, { to: "history", system: "freya" }],
]);
assert.deepEqual(ways(rows([inbound])[0])[0], ["Defend Freya", true, { to: "system", id: "freya" }], "a live raid asks for defense");
const fought = rows([{ id: 30, at_time: 9, severity: "bad", kind: "battle", text: "Your raider was destroyed.", refs: { fleet: "f1", battle: "b1" } }])[0];
assert.deepEqual(ways(fought).slice(0, 2), [
  ["Replay", true, { to: "panel", panel: "battle", params: { id: "b1" } }],
  ["Open fleet", false, { to: "fleet", id: "f1" }],
]);

assert.equal(history.dayLabel(90_000, 100_000), "Today · D2");
assert.equal(history.dayLabel(1_000, 100_000), "Yesterday · D1");
assert.equal(history.dayLabel(1_000, 300_000), "D1");
assert.equal(history.goodsLine([["metallic_ore", -46], ["provisions", 14]]), "−46 Ferrite Ore · +14 Provisions");
assert.equal(history.goodsLine([["metallic_ore", -46]], false), "46 Ferrite Ore");
assert.equal(history.goodsValue([["metallic_ore", -46], ["provisions", 14]], valueOf), 46 * 4 + 14 * 10);
assert.equal(history.unreadNewsCount([inbound, raiding, ended, sold(10, 600, 10), { at_time: 700, severity: "warn", text: "Storage nearly full." }], 0), 2,
  "one per episode, warn or worse");
assert.equal(history.unreadNewsCount([inbound, raiding, ended], 450), 1, "only past the read mark");

// ── Inbox prefs and rules ──
const storage = new MemoryStorage();
const store = logModules({ storage }).inbox;
assert.deepEqual(plain(store.loadInboxPrefs("me")), { dismissed: [], snoozed: {}, readThrough: null, seen: [] });
store.saveInboxPrefs("me", { dismissed: ["a"], snoozed: { b: 50 }, readThrough: 9, seen: ["a"] });
assert.deepEqual(plain(store.loadInboxPrefs("me")), { dismissed: ["a"], snoozed: { b: 50 }, readThrough: 9, seen: ["a"] });
assert.deepEqual(plain(store.loadInboxPrefs("you")).dismissed, [], "per corporation");
assert.deepEqual(plain(store.loadInboxPrefs(null)).dismissed, [], "no player, no storage");
const item = (key) => ({ key, group: "threats", weight: 1, tone: "bad", icon: "raid", title: key, facts: "", hint: "", actions: [] });
const prefs = { dismissed: ["a", "gone"], snoozed: { b: 50, c: 5 }, readThrough: null, seen: ["gone"] };
assert.deepEqual(plain(inbox.visibleInbox([item("a"), item("b"), item("c")], prefs, 10, false)).map((entry) => entry.key), ["c"],
  "dismissed and snoozed rows hide; an expired snooze shows");
assert.deepEqual(prefs.dismissed, ["a", "gone"], "an unloaded view never forgets a dismissal");
inbox.visibleInbox([item("a"), item("b")], prefs, 10);
assert.deepEqual([...prefs.dismissed], ["a"], "a loaded view forgets keys that left");
assert.deepEqual([...prefs.seen], []);
assert.deepEqual(Object.keys(prefs.snoozed), ["b"]);

const warnings = (timeline, now) => plain(inbox.latestRaidWarnings(timeline, now)).map(([system, entry]) => [system, entry.id]);
assert.deepEqual(warnings([inbound], 200), [["freya", 1]]);
assert.deepEqual(warnings([inbound, raiding], 450), [], "the raid landed");
assert.deepEqual(warnings([inbound], 100 + 21 * 60), [], "a stale warning");
assert.deepEqual(warnings([inbound], 340 + 301), [], "well past its ETA");

const ruled = { ...logState({ timeline: [inbound], systems: [] }), galaxy: { pirate_id: "pirates", systems: [], siege_secs: 21_600 },
  diplomacy: { incoming: [{ id: 7, name: "Vega", treaty: "non_aggression", expires_at: 1_600 }] },
  syndicateInvites: [{ id: "s1", name: "Long Haul" }], founding: null, battleViewed: new Set(), pendingOrders: new Map(), orders: {} };
const live = logModules({ now: () => 200 }).inbox;
const computed = plain(live.computeInbox(ruled));
assert.deepEqual(computed.map((entry) => [entry.key, entry.group]), [["raid:freya:1", "threats"], ["treaty:7", "opportunities"], ["invites:s1", "opportunities"]],
  "threats first, then by weight");
assert.equal(computed[0].title, "Raid inbound · Freya");
assert.equal(computed[0].facts, "2 raiders · ETA ~3m", "140 s, rounded up");
assert.deepEqual(computed[1].actions.map((action) => [action.label, action.go.command]), [
  ["Accept", { type: "RespondTreaty", proposal_id: 7, accept: true }], ["Decline", { type: "RespondTreaty", proposal_id: 7, accept: false }]]);
for (const entry of computed) assert.ok(entry.hint && !entry.facts.includes(". "), `${entry.key}: facts are terse; the why is a tooltip`);
// Unclaimed nodes near a system you own are one row, nearest first; a far one
// never counts (not the galaxy's dozens).
const node = (id, x) => ({ info: { id, name: id, pos: { x, y: 0 } }, system: { id, owner: null, node: { awakened: true, title: "Veil", bonus: {} } } });
const nodes = [node("tithe", 9_000), node("seginus", 4_000), node("denebola", 30_000), node("faraway", 90_000)];
const galaxyNodes = { ...ruled, timeline: [], diplomacy: null, syndicateInvites: [],
  galaxy: { ...ruled.galaxy, systems: [{ id: "home", pos: { x: 0, y: 0 } }, ...nodes.map((entry) => entry.info)] },
  systems: [{ id: "home", owner: "me", stockpile: [], bodies: [] }, ...nodes.map((entry) => entry.system)] };
const folded = plain(live.computeInbox(galaxyNodes)).filter((entry) => entry.key.startsWith("nodeopen:"));
assert.deepEqual(folded.map((entry) => [entry.key, entry.title, entry.facts, entry.actions[0].label]),
  [["nodeopen:seginus", "3 exotic nodes nearby", "nearest seginus · 4,000 su", "Show nearest"]]);
const lone = { ...galaxyNodes, systems: galaxyNodes.systems.filter((system) => ["home", "faraway"].includes(system.id)) };
assert.deepEqual(plain(live.computeInbox(lone)).map((entry) => entry.key), ["noauto"], "a node 90,000 su out is not news");
// §founding R8: the charter keeps side business out; nodes wait for its end.
const charter = (stage, bounty_received) => ({ ...galaxyNodes, diplomacy: ruled.diplomacy, syndicateInvites: ruled.syndicateInvites,
  founding: { stage, bounty_received } });
const kinds = (st) => plain(live.computeInbox(st)).map((entry) => entry.key.split(":")[0]);
assert.deepEqual(kinds(charter("sell_ore", false)), ["treaty"], "a new corporation sees no nodes, invites or side rows");
assert.deepEqual(kinds(charter("export_production", true)), ["treaty", "invites"], "after the bounty the side rows return");
assert.deepEqual(kinds(charter("complete", true)), ["treaty", "nodeopen", "invites"]);

// ── Deck Log ──
const items = [
  { ...item("raid:freya:1"), title: "Raid inbound · Freya", facts: "2 raiders", hint: "Pirates take up to 10%.", actions: [{ label: "Defend", go: { to: "system", id: "freya" } }] },
  { ...item("treaty:7"), group: "opportunities", tone: "info", title: "Vega proposes non_aggression", hint: "Answer it.",
    actions: [{ label: "Accept", go: { to: "send", command: { type: "RespondTreaty", proposal_id: 7, accept: true }, done: "Treaty accepted." } },
      { label: "Decline", go: { to: "send", command: { type: "RespondTreaty", proposal_id: 7, accept: false } } }] },
];
let shown = items, now = 1_000;
const deckStorage = new MemoryStorage();
const state = logState({ timeline: [inbound, raiding, ended, sold(10, 600, 10)] });
const desk = logFixture(state, { inbox: () => shown, now: () => now, storage: deckStorage });
const logRoute = { name: "log" };
assert.equal(desk.deckLog.badgeCount(null), 2 + 1, "two new inbox rows and the raid's news");
desk.deckLog.render(logRoute);
let html = desk.root.innerHTML;
assert.match(html, /data-tab="inbox" aria-selected="true">Inbox<em>2<\/em>/, "a fresh open with work shows the Inbox");
assert.match(html, /<h4 class="deck-log__group"><span>Threats<\/span><b>1<\/b><\/h4><article class="deck-logrow is-bad is-unread" title="Pirates take up to 10%.">/);
assert.match(html, /data-key="raid:freya:1" data-index="0">Defend<\/button>/, "one button a row");
assert.match(html, /<summary aria-label="More actions">⋯<\/summary><div><button[^>]*data-key="treaty:7" data-index="1">Decline<\/button><button[^>]*data-deck-act="log-snooze"/);
assert.doesNotMatch(html, /Needs a decision|Arrived reports|Light-delayed/, "the old sections are gone");
assert.equal(desk.deckLog.badgeCount(logRoute), 1, "the open Inbox is seen; History's news still waits");
desk.deckClick({ deckAct: "log-go", source: "inbox", key: "raid:freya:1", index: "0" }, logRoute);
assert.deepEqual(desk.calls.systems, ["freya"]);
desk.deckClick({ deckAct: "log-go", source: "inbox", key: "treaty:7", index: "0" }, logRoute);
assert.deepEqual(desk.calls.sent, [{ type: "RespondTreaty", proposal_id: 7, accept: true }]);
assert.deepEqual(desk.calls.notices, ["Treaty accepted."]);
desk.deckClick({ deckAct: "log-dismiss", key: "treaty:7" }, logRoute);
assert.deepEqual(plain(desk.deckLog.items()).map((entry) => entry.key), ["raid:freya:1"]);
desk.deckClick({ deckAct: "log-snooze", key: "raid:freya:1" }, logRoute);
assert.deepEqual(plain(desk.deckLog.items()), [], "snoozed for an hour");
assert.match(desk.root.innerHTML, /All clear · next in 5m/);
now += 3_601;
assert.deepEqual(plain(desk.deckLog.items()).map((entry) => entry.key), ["raid:freya:1"], "the snooze ends");
const reloaded = logFixture(state, { inbox: () => shown, now: () => now, storage: deckStorage });
assert.deepEqual(plain(reloaded.deckLog.items()).map((entry) => entry.key), ["raid:freya:1"], "a dismissal survives a reload");
assert.equal(reloaded.deckLog.badgeCount(null), 1, "seen rows stay seen; the news is still unread");

// History: rows, a day header, filters, a report with its goods priced.
desk.deckClick({ deckAct: "log-tab", tab: "history" }, logRoute);
html = desk.root.innerHTML;
assert.match(html, /data-tab="history" aria-selected="true">History<em>2 new<\/em>/, "new rows stay marked while you look");
assert.equal(desk.deckLog.badgeCount(logRoute), 0, "opening History reads it");
assert.match(html, /<h4 class="deck-log__day">Today · D1<\/h4><article class="deck-logrow is-good/);
assert.match(html, /<b>Raided · Freya<\/b><small>−46 Ferrite Ore · −14 Provisions<\/small>/);
assert.match(html, /data-filter="raids" aria-pressed="false">Raids<\/button>/);
const raidKey = desk.history.historyRows(state.timeline, { now, readThrough: 0, nameOf, valueOf }).find((row) => row.kind === "raid").key;
desk.deckClick({ deckAct: "log-row", key: raidKey }, logRoute);
html = desk.root.innerHTML;
assert.match(html, /aria-expanded="true"/);
assert.match(html, /<td>Ferrite Ore<\/td><td class="is-num is-neg">−46<\/td><td class="is-num">184 Cr<\/td>/);
assert.match(html, /<tfoot><tr><td>Total<\/td><td class="is-num">60<\/td><td class="is-num">324 Cr<\/td>/);
assert.match(html, /<li><time>T100<\/time><span>Pirate raid inbound · Freya/, "every line of the episode");
assert.match(html, /class="is-primary" data-deck-act="log-go" data-source="row" data-key="[^"]+" data-index="0">Show Freya/);
desk.deckClick({ deckAct: "log-go", source: "row", key: raidKey, index: "1" }, logRoute);
assert.deepEqual(desk.calls.fleets, ["pack"], "track the haul");
desk.deckClick({ deckAct: "log-go", source: "row", key: raidKey, index: "2" }, logRoute);
assert.match(desk.root.innerHTML, /aria-label="Clear the system filter">Freya ✕<\/button>/, "a system's history");
desk.deckClick({ deckAct: "log-system-clear" }, logRoute);
desk.deckClick({ deckAct: "log-filter", filter: "trade" }, logRoute);
assert.doesNotMatch(desk.root.innerHTML, /Raided · Freya/);
assert.match(desk.root.innerHTML, /Sold 10 ferrite ore/);

// A route's query applies once; a new navigation applies its own.
const toastRoute = { name: "log", query: { tab: "history", filter: "raids" } };
desk.deckLog.render(toastRoute);
assert.match(desk.root.innerHTML, /data-filter="raids" aria-pressed="true"/);
desk.deckClick({ deckAct: "log-filter", filter: "all" }, toastRoute);
desk.deckLog.render(toastRoute, true);
assert.match(desk.root.innerHTML, /data-filter="all" aria-pressed="true"/, "a click outlives the route's query");
desk.deckLog.render({ name: "fleets" });
shown = [];
desk.deckLog.render(logRoute);
assert.match(desk.root.innerHTML, /data-tab="history" aria-selected="true"/, "a fresh open with nothing to do shows History");

// §welcome: the server's away report rides over History until closed.
state.welcome = { away: 7_200, happened: [{ severity: "bad", text: "Raids ×3: lost 120 Ferrite Ore in all." }], waiting: [], next: [] };
desk.deckLog.render(logRoute, true);
assert.match(desk.root.innerHTML, /While you were away · 120m[\s\S]*class="deck-chip is-bad" title="Raids ×3: lost 120 Ferrite Ore in all.">/);
desk.deckClick({ deckAct: "log-away-close" }, logRoute);
assert.doesNotMatch(desk.root.innerHTML, /While you were away/);

// ── Mobile Log ──
shown = items;
const phone = logFixture(logState({ timeline: [inbound, raiding, ended], battleReports: [{ id: 5, battle_id: "b5" }],
  welcome: { away: 600, happened: [{ severity: "warn", text: "Raids ×1." }], waiting: [], next: [{ label: "Open Freya", go: { kind: "system", id: "freya" } }] } }),
{ inbox: () => shown, now: () => 1_000 });
let sheet = phone.mobileLog.render({ id: "log" });
assert.equal(sheet.title, "Log");
assert.match(sheet.html, /aria-selected="true">Inbox · 2<\/button>/);
assert.match(sheet.html, /class="m-logrow is-bad is-unread"[^>]*><span class="m-logrow__text"><b>Raid inbound · Freya<\/b><small>2 raiders<\/small><\/span><button type="button" class="m-logrow__act"/);
phone.mobileClick({ mobileAct: "log-go", source: "inbox", key: "treaty:7", index: "1" });
assert.deepEqual(phone.calls.sent, [{ type: "RespondTreaty", proposal_id: 7, accept: false }]);
assert.equal(phone.mobileClick({ mobileAct: "market-tab" }), false, "only log actions");
const props = { tab: "history", filter: "raids" };
sheet = phone.mobileLog.render({ id: "log", props });
assert.match(sheet.html, /aria-selected="true">History/);
assert.match(sheet.html, /data-filter="raids" aria-pressed="true">Raids/);
assert.match(sheet.html, /Away · 10m[\s\S]*data-mobile-act="welcome-go" data-go="system" data-id="freya">Open Freya/, "the phone keeps the next steps");
phone.mobileClick({ mobileAct: "log-filter", filter: "all" });
assert.match(phone.mobileLog.render({ id: "log", props }).html, /data-filter="all" aria-pressed="true"/, "props apply once");
const phoneKey = phone.history.historyRows(phone.ctx.state.timeline, { now: 1_000, readThrough: 0, nameOf, valueOf })[0].key;
phone.mobileClick({ mobileAct: "log-row", key: phoneKey });
assert.match(phone.mobileLog.render({ id: "log", props }).html, /<td>Provisions<\/td><td class="is-neg">−14<\/td><td>140 Cr<\/td>/);
assert.ok(phone.refreshes() >= 3, "each action redraws the sheet");
// Every Go lands on a sheet or the map.
const run = (go) => phone.mobileLog.run(go);
run({ to: "panel", panel: "battle", params: { id: "5", report: "battle" } });
run({ to: "panel", panel: "battle", params: { id: "b9" } });
run({ to: "panel", panel: "world", params: { systemId: "freya", bodyId: "3" } });
run({ to: "panel", panel: "build", params: { id: "freya" } });
run({ to: "panel", panel: "comms" });
run({ to: "panel", panel: "operations" });
run({ to: "withdraw", fleet: "f1", pos: { x: 0, y: 0 } });
assert.deepEqual(phone.calls.sheets, [
  { id: "battle", props: { id: "b5" } }, { id: "battle", props: { id: "b9" } }, { id: "planet", props: { systemId: "freya", bodyId: 3 } },
  { id: "build", props: { systemId: "freya" } }, { id: "comms", props: { tab: "inbox" } }, { id: "operations" },
]);
assert.deepEqual(phone.calls.fleetCommands, [{ type: "Withdraw", fleet_id: "f1" }], "withdraw rides the confirm flow");
run({ to: "panel", panel: "battle", params: { id: "2", report: "capture" } });
assert.match(phone.mobileLog.render({ id: "log", props }).html, /data-filter="combat" aria-pressed="true"/, "a capture report falls back to Combat");

// ── Shell wiring ──
const deckIndex = src("shell/deck/index.ts"), surfaces = src("shell/mobile/surfaces.ts");
// §command-retired (Peter, Oct 31 2026): no Command page repeats the Inbox.
assert.doesNotMatch(deckIndex + src("shell/deck/router.ts") + src("shell/deck/markup.ts"), /name: "command"|"command": \{|navButton\("command"/);
assert.match(surfaces, /case "log": view = this\.log\.render\(entry\); break;/);
assert.match(surfaces, /if \(action\.startsWith\("log-"\)\) return this\.log\.handleAction\(button\);/);
assert.doesNotMatch(surfaces, /renderCheckin|arrivedReports|Decision inbox/, "the Check-in sheet is retired");
assert.match(deckIndex, /event\.kind === "LogToast"/);
console.log("PASS log: history episodes, collapse, filters, unread, reports, row actions; inbox prefs, rules and snooze; Deck and phone Logs, routes, welcome strip and shell wiring");
