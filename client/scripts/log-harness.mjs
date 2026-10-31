// §log: the real Log modules (history, inbox prefs, both shells' Log) compiled
// over small stubs, for test-log and test-battle-archive. No DOM, no socket.
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

export const src = (path) => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
export const plain = (value) => JSON.parse(JSON.stringify(value));

export function compile(path, deps = {}, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: (name) => deps[name] ?? {}, ...globals });
  return exports;
}

export const NAMES = { wadjet: "Wadjet", freya: "Freya", nergal: "Nergal" };
export const LABELS = { metallic_ore: "Ferrite Ore", provisions: "Provisions", armaments: "Armaments" };
export const VALUES = { metallic_ore: 4, provisions: 10, armaments: 60 };

/** A browser storage double: what the Log keeps per corporation. */
export class MemoryStorage {
  data = new Map();
  getItem(key) { return this.data.has(key) ? this.data.get(key) : null; }
  setItem(key, value) { this.data.set(key, String(value)); }
}

export function logModules({ now = () => 1_000, inbox = () => [], storage = new MemoryStorage() } = {}) {
  const icons = { icon: (key) => `<i data-icon="${key}"></i>`, label: (key) => LABELS[key] ?? key };
  const format = {
    ageLabel: (s) => `${Math.round(s)}s`, fmtDur: (s) => `${Math.round(s / 60)}m`, gameClock: (t) => `T${t}`,
    fmtBuildDur: (s) => s < 120 ? `${Math.round(s)}s` : `${Math.round(s / 60)}m`,
  };
  const geo = { systemName: (id) => NAMES[id] ?? id, locName: () => "Dabih", commandDelayTo: () => 42, freshSurveyReports: new Map(), REPORT_RECENT_S: 600 };
  const state = { liveSimTime: now };
  const archive = compile("core/derive/battlearchive.ts", { "./format": format, "./geo": geo });
  const history = compile("core/derive/history.ts", { "../../icons": icons });
  const season = compile("core/derive/season.ts", { "./format": format });
  const realInbox = compile("core/derive/inbox.ts", { "../../icons": icons, "./format": format, "./geo": geo, "../../state": state,
    "./research": { nodeBonusDesc: () => "+10% ore." }, "./season": season },
    { localStorage: storage });
  const inboxModule = { ...realInbox, computeInbox: () => inbox() };
  const deps = {
    "../../core/derive/battlearchive": archive, "../../core/derive/fleet": { COMMODITY_VALUE: VALUES },
    "../../core/derive/format": format, "../../core/derive/geo": geo, "../../core/derive/history": history,
    "../../core/derive/inbox": inboxModule, "../../core/derive/orders": { nextDecisionLabel: () => "next in 5m" },
    "../../core/derive/season": season,
    "../../icons": icons, "../../state": state,
    "../dom": { renderDeferred: () => false, setHtml: (root, html) => { root.innerHTML = html; } },
    "../signature": { sheetFingerprint: JSON.stringify },
  };
  const deck = compile("shell/deck/log.ts", deps);
  const mobile = compile("shell/mobile/log.ts", deps);
  return { archive, history, inbox: realInbox, deck, mobile, storage };
}

/** A Log over a fixture state; every way out is recorded. */
export function logFixture(state, options = {}) {
  const modules = logModules(options);
  const calls = { routes: [], systems: [], fleets: [], sheets: [], sent: [], notices: [], fleetCommands: [] };
  const ctx = {
    state, send: (msg) => calls.sent.push(plain(msg)),
    intent: { beginFleetCommand: (command) => calls.fleetCommands.push(plain(command)) },
  };
  const root = { id: "deck-log", innerHTML: "" };
  const deckLog = new modules.deck.DeckLogRoutes(root, ctx, {
    go: (route) => calls.routes.push(plain(route)), focusSystem: (id) => calls.systems.push(id),
    focusFleet: (id) => calls.fleets.push(id), notice: (text) => calls.notices.push(text),
  });
  let refreshes = 0;
  const mobileLog = new modules.mobile.MobileLog(ctx, {
    openSheet: (entry) => calls.sheets.push(plain(entry)), focusSystem: (id) => calls.systems.push(id),
    focusFleet: (id) => calls.fleets.push(id), notice: (html) => calls.notices.push(html), refresh: () => { refreshes++; },
  });
  const deckClick = (dataset, route) => deckLog.handleAction({ dataset }, route);
  const mobileClick = (dataset) => mobileLog.handleAction({ dataset });
  return { ...modules, ctx, root, deckLog, mobileLog, calls, deckClick, mobileClick, refreshes: () => refreshes };
}

/** The served state the Log reads; `extra` overrides. */
export function logState(extra = {}) {
  return {
    playerId: "me", awaySince: 0, simTime: 1_000, galaxy: { pirate_id: "pirates", systems: [] }, systems: [{ id: "freya" }], ghosts: [],
    battles: [], battleReports: [], captureReports: [], battleRecords: [], standingOrders: [], operations: [], timeline: [],
    welcome: null, diplomacy: null, syndicateInvites: [], rankings: [], syndicate: null, ...extra,
  };
}
