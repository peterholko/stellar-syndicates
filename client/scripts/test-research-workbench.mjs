// Exercise the actual desktop controller, not a parallel research UI. The
// optional loopback preview uses isolated served fixtures and never joins a game.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import ts from "typescript";

const src = path => readFileSync(new URL(`../src/${path}`, import.meta.url), "utf8");
const deps = {};
function compile(path, globals = {}) {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(src(path), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, require: name => deps[name] ?? {}, performance, ...globals });
  return exports;
}

// Use real catalogue names/descriptions/tiers so capital-school and long-label
// layouts stay represented as the game grows. Availability below is fixture data.
const catalog = readFileSync(new URL("../../crates/sim/src/research.rs", import.meta.url), "utf8").replace(/^\s*\/\/.*$/gm, "");
const snake = value => value.replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase();
const programmes = [...catalog.matchAll(/id: "([^"]+)",\s*field: Field::(\w+),\s*school: (None|Some\(School::\w+\)),\s*tier: (\d+),\s*name: "([^"]+)",\s*blurb: "([^"]*)",[\s\S]*?hidden: (false|true),/g)]
  .filter(match => match[7] === "false")
  .map(([, id, field, school, tier, name, blurb]) => ({ id, field: snake(field),
    school: school === "None" ? null : snake(school.slice(13, -1)), tier: Number(tier), name, blurb,
    state: tier === "1" ? "available" : "locked", cost: Number(tier) * 240,
    gate: tier === "1" ? null : { label: "Milestone progress", current: 2, threshold: 10 },
  }));
assert.ok(programmes.length > 100, "fixture includes the full authored catalogue, not a tiny best-case list");
assert.equal(programmes.length, [...catalog.matchAll(/hidden: false/g)].length, "every visible technology is represented");
const first = programmes.find(p => p.field === "propulsion" && p.tier === 1);
const choice = programmes.find(p => p.field === "propulsion" && p.tier === 1 && p !== first);
const materialOne = programmes.find(p => p.field === "materials" && p.tier === 1);
const materialTwo = programmes.find(p => p.field === "materials" && p.tier === 2);
const completed = programmes.find(p => p.field === "hulls" && p.tier === 1);
first.state = "active";
completed.state = "completed";
materialTwo.gate = { label: "Units through industry", current: 540, threshold: 10_000 };
materialTwo.recovered_data = materialTwo.cost * .3;
const baseResearch = {
  active: { id: first.id, name: first.name, progress: 60, cost: first.cost, eta_secs: 180 },
  queue: [], rate: 1, stalled: false,
  academies: [{ system: "Freya", body_id: 1, tier: 1, rate: 1, supplied: true }], programmes,
};
deps["./protocol"] = deps["../../protocol"] = compile("protocol.ts");
const state = deps["../../state"] = { state: { research: structuredClone(baseResearch), ghosts: [], selectedShipIds: new Set(),
  systems: [], battles: [], commandSignals: [], orders: {}, raids: {}, captains: [] }, liveSimTime: () => 100 };
const research = deps["../../core/derive/research"] = compile("core/derive/research.ts");
deps["../../icons"] = compile("icons.ts");
deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["../signature"] = { sheetFingerprint: JSON.stringify };
deps["../dom"] = { renderDeferred: () => false, setHtml: (root, html) => root.innerHTML = html };
const { DeckRosterRoutes } = compile("shell/deck/roster.ts", { document: { activeElement: null }, HTMLElement: class {} });
const st = state.state, sent = [], notices = [];
research.bindResearchNet(() => ({ send: command => sent.push(structuredClone(command)) }));
const root = { id: "research-fixture", innerHTML: "" };
const controller = new DeckRosterRoutes(root, { state: st }, { notice: html => notices.push(html), go() {}, openWorld() {} });
const route = { name: "research" };
const click = (action, data = {}) => controller.handleAction({ dataset: { deckAct: action, ...data } }, route);
const render = () => { controller.render(route); return root.innerHTML; };
const choices = () => [...root.innerHTML.matchAll(/data-deck-act="research-select" data-programme="([^"]+)"/g)].map(m => m[1]);
const addCount = () => [...root.innerHTML.matchAll(/data-deck-act="research-add"/g)].length;

render();
assert.equal(addCount(), 1, "one shared queue action, not a button per technology");
assert.ok(choices().length < 10, "no whole-catalogue render");
assert.ok(choices().every(id => programmes.find(p => p.id === id).field === "propulsion" && programmes.find(p => p.id === id).tier === 1));
assert.doesNotMatch(root.innerHTML, /deck-research-boards|Programme queue|Private corporation programme boards/);
click("research-select", { programme: choice.id });
assert.equal(sent.length, 0, "selecting a technology is never an order");
assert.equal(controller.handleAction({ dataset: { deckAct: "research-add" } }, { name: "fleets" }), false, "a hidden/stale research action cannot fire on another page");
assert.match(root.innerHTML, new RegExp(`data-programme="${choice.id}" aria-pressed="true"`));
assert.doesNotMatch(root.innerHTML, /data-deck-act="research-add" disabled/);
click("research-add", { programme: "forged-unselected-id" });
assert.deepEqual(sent.at(-1), { type: "SetResearchQueue", queue: [choice.id] }, "the selected tech is sent; the active row is not echoed onto the wire");
assert.equal(st.research.programmes.find(p => p.id === choice.id).state, "available", "dispatch does not invent arrived research state");
st.research = { ...st.research, queue: [choice.id], programmes: st.research.programmes.map(p => p.id === choice.id ? { ...p, state: "queued" } : p) };
render();
assert.match(root.innerHTML, /data-deck-act="research-add" disabled/);
const sentOnce = sent.length;
click("research-add");
assert.equal(sent.length, sentOnce, "queued work cannot be duplicated");

click("research-field", { field: "materials" });
click("research-tier", { tier: "2" });
click("research-select", { programme: materialTwo.id });
assert.ok(choices().every(id => programmes.find(p => p.id === id).field === "materials" && programmes.find(p => p.id === id).tier === 2));
assert.match(root.innerHTML, /Complete one Materials Tier I technology/);
assert.match(root.innerHTML, /540 \/ 10,000/);
assert.match(root.innerHTML, /30% research work banked/);
click("research-add");
assert.equal(sent.length, sentOnce, "locked research is inspectable but never dispatchable");
const beforeChoices = choices();
st.research = { ...st.research, active: { ...st.research.active, progress: 90 }, programmes: st.research.programmes.map(p => p.id === materialTwo.id ? { ...p, gate: { ...p.gate, current: 9_999 } } : p) };
render();
assert.deepEqual(choices(), beforeChoices, "incoming reports never reset the field or tier");
assert.match(root.innerHTML, /9,999 \/ 10,000/);
assert.match(root.innerHTML, new RegExp(`data-programme="${materialTwo.id}" aria-pressed="true"`));
st.research.programmes.find(p => p.id === materialOne.id).state = "completed";
st.research.programmes.find(p => p.id === materialTwo.id).gate.current = 10_000;
render();
assert.match(root.innerHTML, /data-deck-act="research-add" disabled/, "matching client-visible requirements cannot override a still-locked server state");
st.research.programmes.find(p => p.id === materialTwo.id).state = "available";
render();
assert.doesNotMatch(root.innerHTML, /data-deck-act="research-add" disabled/);
st.research.academies = [];
render();
assert.match(root.innerHTML, /Staff and supply an Academy to progress/);
assert.doesNotMatch(root.innerHTML, /data-deck-act="research-add" disabled/, "lack of funding stalls work, not queue planning");

click("research-page", { page: "queue" });
assert.equal(addCount(), 0);
assert.equal(choices().length, 0, "queue management is its own page");
assert.match(root.innerHTML, /Research queue/);
assert.match(root.innerHTML, /No staffed Academy/);
click("research-remove", { index: "0" });
assert.equal(sent.length, sentOnce, "an active programme remains pinned even through the action handler");
click("research-remove", { index: "1" });
assert.deepEqual(sent.at(-1), { type: "SetResearchQueue", queue: [] });
st.research.queue.push(materialOne.id);
click("research-down", { index: "1" });
assert.deepEqual(sent.at(-1).queue, [materialOne.id, choice.id], "queue reordering stays intact");

click("research-page", { page: "completed" });
assert.equal(addCount(), 0, "reviewing completed research cannot queue it again");
assert.ok(choices().length > 0);
assert.ok(choices().every(id => st.research.programmes.find(p => p.id === id).state === "completed"));
click("research-field", { field: "life" });
assert.match(root.innerHTML, /No completed technologies in this category/);
click("research-page", { page: "catalog" });
assert.match(root.innerHTML, new RegExp(`data-programme="${materialTwo.id}" aria-pressed="true"`), "catalogue selection survives visits to queue and history");
click("research-field", { field: "hulls" });
click("research-tier", { tier: "8" });
assert.ok(choices().length && choices().every(id => programmes.find(p => p.id === id).tier === 8), "late capital tiers remain browsable without cluttering the first page");
const last = st.research.programmes.find(p => p.id === choices()[0]);
last.name = '<img src=x onerror="bad">'; last.blurb = "<script>bad</script>";
render();
assert.match(root.innerHTML, /&lt;img/);
assert.doesNotMatch(root.innerHTML, /<img src=x|<script>bad/);
const beforePreset=sent.length;
controller.render({name:"research",query:{programme:"mat_enrichment"}});
assert.match(root.innerHTML,/data-programme="mat_enrichment" aria-pressed="true"/);
assert.equal(sent.length,beforePreset,"the refining tutorial link selects Enrichment without queueing it");
assert.ok(choices().includes("mat_ore_recovery"),"recovery is an independent research option");
st.research = { ...st.research, programmes: [] };
render();
assert.equal(addCount(), 1);
assert.match(root.innerHTML, /No technology reports/);
assert.match(root.innerHTML, /data-deck-act="research-add" disabled/);
st.research = null;
assert.match(render(), /Research report pending/);
console.log(`PASS: ${programmes.length}-technology catalogue, category/tier paging, selection-only inspection, one queue action, delayed unlocks, queues, history and report stability.`);

if (process.argv.includes("--serve")) {
  // Bundle a tiny fixture entry with the REAL controller + DOM reconciler. The
  // fake transport only records commands; its report button explicitly supplies
  // fresh fixture data. No account, WebSocket or running game is involved.
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(new URL("../", import.meta.url));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "research-preview", resolveId: id => id.endsWith("virtual:research-preview") ? "\0research-preview" : undefined,
      load: id => id === "\0research-preview" ? `
        import { DeckRosterRoutes } from ${JSON.stringify(resolve(clientRoot, "src/shell/deck/roster.ts"))};
        import { state } from ${JSON.stringify(resolve(clientRoot, "src/state.ts"))};
        import { bindResearchNet } from ${JSON.stringify(resolve(clientRoot, "src/core/derive/research.ts"))};
        import { installPressGuard } from ${JSON.stringify(resolve(clientRoot, "src/shell/dom.ts"))};
        state.research = ${JSON.stringify(baseResearch)};
        state.research.programmes.find(p => p.id === ${JSON.stringify(first.id)}).state = 'active';
        const root = document.getElementById('research-fixture');
        const controller = new DeckRosterRoutes(root, { state }, { go(){}, openWorld(){}, notice(){ document.getElementById('preview-notice').textContent = 'Queue change sent'; } });
        const route = { name: 'research' }; let sent = null;
        bindResearchNet(() => ({ send(command) { sent = command; } }));
        installPressGuard();
        root.addEventListener('click', event => { const b = event.target.closest('button[data-deck-act]'); if(b && !b.disabled) controller.handleAction(b, route); });
        document.getElementById('preview-arrive').onclick = () => {
          if(sent) { state.research.queue = sent.queue; state.research.programmes = state.research.programmes.map(p => ({...p, state: p.state === 'active' || p.state === 'completed' ? p.state : sent.queue.includes(p.id) ? 'queued' : p.tier === 1 ? 'available' : 'locked'})); sent = null; }
          controller.render(route, true);
        };
        document.getElementById('preview-width').onclick = () => { const w = document.getElementById('deck-workspace'); w.dataset.width = w.dataset.width === 'wide' ? 'standard' : 'wide'; };
        controller.render(route);
        setInterval(() => controller.render(route), 100);
      ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:research-preview", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const chunks = new Map(output.filter(file => file.type === "chunk").map(file => [`/${file.fileName}`, file.code]));
  const publicRoot = resolve(clientRoot, "public");
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      response.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"><div class="deck"><header class="deck-topbar"><b>Research fixture</b><button id="preview-arrive">Deliver queue report</button><span id="preview-notice"></span></header><aside id="deck-workspace" class="deck-workspace" data-route="research" data-width="wide" aria-hidden="false"><header class="deck-workspace__header"><button data-deck-act="back" aria-label="Back" disabled>←</button><nav id="deck-breadcrumb" class="deck-breadcrumb">Command</nav><h1 id="deck-workspace-title">Research</h1><button id="preview-width" data-deck-act="width" aria-label="Toggle width">↔</button><button data-deck-act="close" aria-label="Close" disabled>✕</button></header><div id="research-fixture" class="deck-workspace__body"></div></aside></div><script type="module" src="/fixture.js"></script>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (chunks.has(url.pathname)) { response.setHeader("Content-Type", "text/javascript"); response.end(chunks.get(url.pathname)); return; }
    let path;
    if (["/tokens.css", "/deck.css"].includes(url.pathname)) { path = resolve(clientRoot, `src/styles${url.pathname}`); response.setHeader("Content-Type", "text/css"); }
    else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  server.listen(0, "127.0.0.1", () => console.log(`Research fixture: http://127.0.0.1:${server.address().port}/`));
}
