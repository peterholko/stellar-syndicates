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
deps["../../icons"] = deps["./icons"] = compile("icons.ts");
deps["../../core/derive/format"] = compile("core/derive/format.ts");
deps["./research-art.generated"] = compile("research-art.generated.ts");
const art = deps["../../research-art"] = compile("research-art.ts");
deps["./core/derive/station"] = compile("core/derive/station.ts");
deps["./core/derive/equipment"] = compile("core/derive/equipment.ts");
deps["./ship-models.generated"] = compile("ship-models.generated.ts");
const presentation = deps["../../research-presentation"] = compile("research-presentation.ts");
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
// §research-panel: nothing more than one tier past the researchable frontier
// renders. Tier I is researchable, so Tier II shows as the next step; III is not.
const tierButtons = () => [...root.innerHTML.matchAll(/data-deck-act="research-tier" data-tier="(\d+)"/g)].map(m => Number(m[1]));
assert.deepEqual(tierButtons(), [1, 2], "only Tier I and the next tier are offered");
click("research-tier", { tier: "3" });
assert.ok(choices().every(id => programmes.find(p => p.id === id).tier === 1), "a hidden tier cannot be opened by a stale action");
for (const p of programmes.filter(p => p.tier >= 3)) assert.doesNotMatch(root.innerHTML, new RegExp(`data-programme="${p.id}"`), `${p.id} stays hidden`);
// Delivered programme emblems distinguish the cards; the selected detail
// loads eagerly while the card images remain lazy.
const cardArt = [...root.innerHTML.matchAll(/class="deck-research-card[^"]*"[^>]*data-programme="([^"]+)"[^>]*>(<img class="research-emblem"[^>]*>)/g)].map(([, id, img]) => ({ id, img }));
assert.equal(cardArt.length, choices().length, "every card carries an emblem");
for (const { id, img } of cardArt) {
  assert.ok(img.includes(`/art/derived/research/v1/programmes/${id}-128.webp`), `${id}: the card shows its own programme emblem`);
  assert.match(img, /loading="lazy"/, `${id}: card art loads lazily`);
  assert.match(img, /sizes="128px"/, `${id}: card art uses the 128 px size`);
}
assert.match(root.innerHTML, /deck-research-detail__hero"><img class="research-emblem"[^>]*sizes="208px"[^>]*loading="eager"/, "the selected emblem loads first");
assert.match(root.innerHTML, /data-deck-act="research-field" data-field="weapons"[^>]*><img class="research-emblem"[^>]*fields\/weapons-64\.webp/, "field tabs carry their own emblem");
assert.match(root.innerHTML, /deck-research-active"><img class="research-emblem"[^>]*sizes="48px"/, "the active bar shows the programme emblem");
assert.match(root.innerHTML, /Research work<\/dt><dd>0\.1 h<\/dd>/, "research work reads in research-hours");
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
// §rare-materials: the served basket reads per research-hour beside the work.
st.research.programmes.find(p => p.id === materialTwo.id).supplies = [["electronics", 0.02], ["machinery", 0.004]];
render();
assert.match(root.innerHTML, /Supplies \/h<\/dt><dd><span title="Semiconductors">.*?<b>72<\/b><\/span><span title="Machinery">.*?<b>14<\/b>/);
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
assert.equal([...root.innerHTML.matchAll(/deck-research-queue-row[^"]*"><span><img class="research-emblem"[^>]*sizes="48px"/g)].length, 2, "queue rows carry 48 px emblems");
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
assert.ok(choices().every(id => programmes.find(p => p.id === id).tier < 3), "late capital tiers stay hidden while far out of reach");
assert.ok(!tierButtons().includes(8));
// Once the Dreadnought is researchable, the Titan is the next step and shows.
st.research.programmes.find(p => p.id === "hull_line_vii_dreadnought").state = "available";
render();
assert.ok(tierButtons().includes(7) && tierButtons().includes(8) && !tierButtons().includes(6), "the frontier and one step beyond, nothing between");
click("research-tier", { tier: "8" });
assert.ok(choices().length && choices().every(id => programmes.find(p => p.id === id).tier === 8), "Tier VIII opens one step past an available Tier VII");
click("research-select", { programme: "hull_line_viii_titan" });
assert.match(root.innerHTML, /<h4>Unlocks<\/h4><ul><li><img class="research-unlock__img" src="\/art\/derived\/ships\/models-v1\/previews\/titan-768\.webp"[^>]*><span><b>Titan<\/b><small>Hull<\/small><\/span><\/li><li><img[^>]*\/shipyard\/tier-5-128\.webp[^>]*><span><b>Shipyard<\/b><small>Tier V–VI<\/small>/, "a programme shows every object it unlocks, each with its own picture");
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
// §smelt-from-landing (Nov 6 2026): the Smelter needs no research now.
assert.match(root.innerHTML, /<b>Chemical Works<\/b><small>Building<\/small>/, "Enrichment's building is pictured");
assert.doesNotMatch(root.innerHTML, /<b>Smelter<\/b><small>Building<\/small>/, "and the Smelter isn't");
st.research = { ...st.research, programmes: [] };
render();
assert.equal(addCount(), 1);
assert.match(root.innerHTML, /No technology reports/);
assert.match(root.innerHTML, /data-deck-act="research-add" disabled/);
st.research = null;
assert.match(render(), /Research report pending/);

// §research-panel: Research floats across the map like the Market. An old
// saved standard-width preference cannot shrink it, and opening or closing it
// never changes the map camera's horizontal allocation.
{
  deps["./router"] = compile("shell/deck/router.ts", { history: { state: null, pushState() {}, replaceState() {} }, window: { addEventListener() {} } });
  const widthButton = { hidden: false, textContent: "", setAttribute() {} };
  const attrs = {};
  const workspaceRoot = { dataset: {}, setAttribute: (k, v) => { attrs[k] = v; }, getAttribute: k => attrs[k] ?? null, removeAttribute: k => { delete attrs[k]; },
    getBoundingClientRect: () => ({ left: 1920 - 552 }),
    querySelector: selector => ({ "[data-deck-act=back]": { disabled: true, setAttribute() {} }, "[data-deck-act=width]": widthButton,
      "#deck-breadcrumb": { replaceChildren() {}, append() {} }, "#deck-workspace-title": { textContent: "" } })[selector] };
  const rects = [];
  const renderer = { cameraRect: { x: 0, y: 0, w: 1920, h: 1080 }, setCameraRect(rect) { this.cameraRect = rect; rects.push(rect); } };
  const stored = { "stellarSyndicates.deck.width.research": "standard" };
  const { DeckWorkspace } = compile("shell/deck/workspace.ts", {
    window: { innerWidth: 1920, innerHeight: 1080, addEventListener() {} },
    document: { getElementById: () => null, documentElement: { style: { setProperty() {}, removeProperty() {} } } },
    localStorage: { getItem: key => stored[key] ?? null, setItem: (key, value) => { stored[key] = value; } },
  });
  const workspace = new DeckWorkspace(workspaceRoot, renderer, new AbortController().signal);
  workspace.show({ name: "fleets" }, [], false);
  assert.equal(renderer.cameraRect.w, 1920 - 552, "a right-rail route reserves its column");
  workspace.show({ name: "research" }, [], false);
  assert.equal(workspaceRoot.dataset.route, "research");
  assert.equal(workspaceRoot.dataset.width, "wide", "an old standard-width preference no longer applies");
  assert.equal(widthButton.hidden, true, "no width toggle on the full panel");
  assert.equal(renderer.cameraRect.w, 1920, "the map keeps its whole width under Research");
  workspace.toggleWidth();
  assert.equal(workspaceRoot.dataset.width, "wide", "a stale width shortcut cannot shrink it");
  assert.equal(stored["stellarSyndicates.deck.width.research"], "standard", "nor rewrite the old preference");
  workspace.close();
  assert.equal(renderer.cameraRect.w, 1920, "closing Research leaves the camera where it was");
  assert.ok(rects.slice(1).every(rect => rect.w === 1920), "opening and closing Research never pans the map");
  const css = readFileSync(new URL("../src/styles/deck.css", import.meta.url), "utf8");
  assert.match(css, /\.deck-workspace\[data-route="research"\] \{ position: fixed;[^}]*left: 50%; width: min\(1600px, calc\(100vw - var\(--space-3\) \* 2\)\)/, "its own full-width frame, not the Market's 920 px");
  assert.doesNotMatch(css.match(/^\.deck-build-workbench, \.deck-workspace:is\([^)]*\)/m)[0], /research/, "not in the capped pop-out frame list");
}
console.log(`PASS: ${programmes.length}-technology catalogue, category/tier paging, selection-only inspection, one queue action, delayed unlocks, queues, history and report stability.`);

if (process.argv.includes("--serve")) {
  // Bundle a fixture entry with the REAL controller, workspace frame and DOM
  // reconciler. The fake transport only records commands; its report button
  // explicitly supplies fresh fixture data. No account, WebSocket or running
  // game is involved. ?scenario=late opens Hulls VIII and Materials VI with a
  // full queue, an unsupplied and a browned-out Academy, and a long name.
  const { build } = await import("vite");
  const clientRoot = fileURLToPath(new URL("../", import.meta.url));
  const result = await build({ root: clientRoot, configFile: false, publicDir: false, logLevel: "error",
    plugins: [{ name: "research-preview", resolveId: id => id.endsWith("virtual:research-preview") ? "\0research-preview" : undefined,
      load: id => id === "\0research-preview" ? `
        import { DeckRosterRoutes } from ${JSON.stringify(resolve(clientRoot, "src/shell/deck/roster.ts"))};
        import { DeckWorkspace } from ${JSON.stringify(resolve(clientRoot, "src/shell/deck/workspace.ts"))};
        import { state } from ${JSON.stringify(resolve(clientRoot, "src/state.ts"))};
        import { bindResearchNet } from ${JSON.stringify(resolve(clientRoot, "src/core/derive/research.ts"))};
        import { installPressGuard } from ${JSON.stringify(resolve(clientRoot, "src/shell/dom.ts"))};
        state.research = ${JSON.stringify(baseResearch)};
        state.research.programmes.find(p => p.id === ${JSON.stringify(first.id)}).state = 'active';
        if (new URLSearchParams(location.search).get('scenario') === 'late') {
          const r = state.research;
          for (const p of r.programmes) {
            const deep = p.field === 'hulls' ? 7 : p.field === 'materials' ? 6 : 3;
            if (p.field !== 'life' && p.tier < deep && (p.tier < 4 || p.school === 'line' || p.school === 'foundry')) p.state = 'completed';
            else if (p.field !== 'life' && p.tier === deep) { p.state = 'available'; p.gate = null; }
          }
          const dreadnought = r.programmes.find(p => p.id === 'hull_line_vii_dreadnought'); dreadnought.state = 'active';
          r.active = { id: dreadnought.id, name: dreadnought.name, progress: 400000, cost: dreadnought.cost * 4000, eta_secs: 2 * 86400 };
          const waiting = r.programmes.filter(p => p.state === 'available').slice(0, 4);
          waiting.forEach(p => { p.state = 'queued'; }); r.queue = waiting.map(p => p.id);
          r.programmes.find(p => p.id === 'mat_foundry_vi_antimatter_synthesis').name = 'Antimatter Synthesis and Containment Lattice Programme';
          r.academies = [{ system: 'Freya', body_id: 1, tier: 3, rate: 1.6, supplied: true, power: 0.62 }, { system: 'Tyr', body_id: 2, tier: 1, rate: 0, supplied: false }];
          r.grant = { price: 3000, secs: 3600, left: 2, per_day: 3 };
        }
        const workspaceRoot = document.getElementById('deck-workspace');
        const renderer = { cameraRect: { x: 0, y: 0, w: innerWidth, h: innerHeight }, setCameraRect(rect) { this.cameraRect = rect; } };
        const workspace = new DeckWorkspace(workspaceRoot, renderer, new AbortController().signal);
        workspace.show({ name: 'research' }, [{ label: 'Research', route: { name: 'research' } }], false);
        const root = document.getElementById('research-fixture');
        const controller = new DeckRosterRoutes(root, { state }, { go(){}, openWorld(){}, notice(){ document.getElementById('preview-notice').textContent = 'Queue change sent'; } });
        const route = { name: 'research' }; let sent = null;
        bindResearchNet(() => ({ send(command) { sent = command; } }));
        installPressGuard();
        root.addEventListener('click', event => { const b = event.target.closest('button[data-deck-act]'); if(b && !b.disabled) controller.handleAction(b, route); });
        document.getElementById('preview-arrive').onclick = () => {
          if(sent) { state.research.queue = sent.queue; state.research.programmes = state.research.programmes.map(p => ({...p, state: p.state === 'active' || p.state === 'completed' ? p.state : sent.queue.includes(p.id) ? 'queued' : p.state === 'queued' ? 'available' : p.state})); sent = null; }
          controller.render(route, true);
        };
        controller.render(route);
        setInterval(() => controller.render(route), 100);
      ` : undefined }], build: { write: false, minify: false, lib: { entry: "virtual:research-preview", formats: ["es"] } } });
  const output = (Array.isArray(result) ? result[0] : result).output;
  const bundle = output.find(file => file.type === "chunk" && file.isEntry).code;
  const chunks = new Map(output.filter(file => file.type === "chunk").map(file => [`/${file.fileName}`, file.code]));
  const publicRoot = resolve(clientRoot, "public");
  const portArg = process.argv.find(arg => arg.startsWith("--port="));
  const server = createServer((request, response) => {
    const url = new URL(request.url, "http://localhost");
    if (url.pathname === "/") {
      response.setHeader("Content-Type", "text/html; charset=utf-8");
      // The workspace markup mirrors shell/deck/markup.ts; DeckWorkspace sets its route and frame.
      response.end(`<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/tokens.css"><link rel="stylesheet" href="/deck.css"><link rel="stylesheet" href="/research.css"><div class="deck"><header id="deck-topbar" class="deck-topbar"><b>Research fixture</b><button id="preview-arrive">Deliver queue report</button><span id="preview-notice"></span></header><aside id="deck-workspace" class="deck-workspace" aria-hidden="true"><header class="deck-workspace__header"><button type="button" data-deck-act="back" aria-label="Back">←</button><nav id="deck-breadcrumb" class="deck-breadcrumb" aria-label="Breadcrumb"></nav><h1 id="deck-workspace-title"></h1><button type="button" data-deck-act="width" aria-label="Toggle workspace width">↔</button><button type="button" data-deck-act="close" aria-label="Close workspace">✕</button></header><div id="research-fixture" class="deck-workspace__body"></div></aside></div><script type="module" src="/fixture.js"></script>`);
      return;
    }
    if (url.pathname === "/fixture.js") { response.setHeader("Content-Type", "text/javascript"); response.end(bundle); return; }
    if (chunks.has(url.pathname)) { response.setHeader("Content-Type", "text/javascript"); response.end(chunks.get(url.pathname)); return; }
    let path;
    if (["/tokens.css", "/deck.css", "/research.css"].includes(url.pathname)) { path = resolve(clientRoot, `src/styles${url.pathname}`); response.setHeader("Content-Type", "text/css"); }
    else if (url.pathname.startsWith("/art/")) {
      path = resolve(publicRoot, `.${decodeURIComponent(url.pathname)}`);
      if (!path.startsWith(publicRoot + sep)) { response.writeHead(403).end(); return; }
      if (path.endsWith(".webp")) response.setHeader("Content-Type", "image/webp");
    } else { response.writeHead(404).end(); return; }
    try { response.end(readFileSync(path)); } catch { response.writeHead(404).end(); }
  });
  server.listen(portArg ? Number(portArg.slice(7)) : 0, "127.0.0.1", () => console.log(`Research fixture: http://127.0.0.1:${server.address().port}/`));
}
