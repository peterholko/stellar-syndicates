// Isolated, development-only acceptance page. Uses the real Pixi renderer,
// construction controllers, DOM reconciler and model component. No sockets.
import "../src/styles/tokens.css";
import "../src/styles/deck.css";
import "../src/styles/mobile.css";
import "../src/ship-model-preview";
import { renderer } from "../src/render";
import { state } from "../src/state";
import { SHIP_MODELS } from "../src/ship-models.generated";
import { DeckEmpireRoutes } from "../src/shell/deck/empire";
import { MobileParitySurfaces } from "../src/shell/mobile/parity";
import { installPressGuard, setHtml } from "../src/shell/dom";
import { bindMarketDerive } from "../src/core/derive/market";
import * as intent from "../src/core/intent";
import type { CoreContext } from "../src/shell/types";
import type { DeckRoute } from "../src/shell/deck/router";
import type { ShipKind } from "../src/protocol";

const style = document.createElement("style");
style.textContent = `
 .fixture-toolbar{position:fixed;z-index:200;top:0;left:0;right:0;min-height:56px;display:flex;align-items:center;gap:10px;padding:10px 18px;background:#0b1320ed;border-bottom:1px solid #34465a;pointer-events:auto;font:12px system-ui}
 .fixture-toolbar b{letter-spacing:.1em;color:#a1d3e4}.fixture-toolbar>span{color:#95acbf;margin-right:auto}.fixture-toolbar a{color:#9fc9db}
 #fixture-status{position:fixed;z-index:50;bottom:12px;left:18px;margin:0;padding:8px 12px;background:#101b2be8;border:1px solid #34485c;border-radius:4px;color:#afc7d9;font:12px system-ui;max-width:90vw}
 #fixture-inspector{position:fixed;z-index:100;top:85px;left:50%;transform:translateX(-50%);width:min(620px,90vw);padding:20px;background:#0b1421;border:1px solid #38526a;border-radius:10px;pointer-events:auto}
 #fixture-inspector[hidden]{display:none}#fixture-inspector select{margin-left:16px;padding:6px;background:#162538;color:#d4e8f8}
 #fixture-model ship-model-preview{margin-top:20px}#fixture-zoom{position:fixed;right:20px;bottom:24px;display:flex;gap:6px;pointer-events:auto}
 #fixture-map-tools{position:fixed;z-index:50;left:18px;bottom:65px;display:flex;gap:6px;flex-wrap:wrap;max-width:calc(100vw - 40px);align-items:center;pointer-events:auto;padding:8px;background:#0b1320ed;border:1px solid #34465a;font:12px system-ui}
 #fixture-map-tools select{background:#142234;color:#c8e0ef;padding:5px}
 .deck{--deck-workspace-inset:0px;--deck-topbar-height:60px;--deck-build-workbench-max:1000px}
 #deck-build-workbench{bottom:60px}.fixture-mobile{position:fixed;inset:66px 0 62px;overflow:auto;padding:16px;background:#08121f;pointer-events:auto}
 @media(max-width:600px){.fixture-toolbar{flex-wrap:wrap;gap:5px;padding:5px}.fixture-toolbar b{font-size:10px}.fixture-toolbar>span{display:none}.fixture-toolbar button{font-size:11px;min-height:26px}.fixture-toolbar a{font-size:11px}#fixture-status{font-size:10px;bottom:6px}.fixture-mobile{top:95px}}
`;
document.head.append(style);
const $ = (id: string) => document.getElementById(id)!;
const kinds = Object.keys(SHIP_MODELS) as ShipKind[];
const mobile = new URLSearchParams(location.search).has("mobile");
const body = { id: 0, name: "Aster Prime", kind: "rocky", environment: "habitable", size: "large", geology: "metal_rich",
  structures: { shipyard: 6, naval_drydock: 6, capital_slipway: 6, garrison: 1, warehouse: 4 },
  construction_time_mult: 1, ship_build_time_mult: .8, population: 1, habitat_capacity_mult: 1, population_growth_mult: 1,
  resource_slots: 4, industrial_slots: 4, infrastructure_slots: 8, deposits: [] };
const system: any = { id: "preview-home", owner: "preview", name: "Aster", bodies: [body], structures: { ...body.structures },
  stockpile: ["alloys", "electronics", "machinery", "fuel", "provisions"].map(commodity => ({ commodity, units: 4000 })),
  builds: [], deposits: [], assignments: ["shipyard", "naval_drydock", "capital_slipway"].map(structure =>
    ({ body_id: 0, structure, tier: 6, workers: 6, specialists: {}, staffing: 1, skill: 1, outputs: [], throughput: 1, food: 1, title: structure })),
  blockade: null, module_stock: [], population: 1, habitat_fed: true, age: 0 };
state.playerId = "preview"; state.link = "online"; state.systems = [system];
state.galaxy = { hub: { x: 5000, y: 5000 }, radius: 10000, nebulas: [], c: 1000, sensor_range: 0,
  jump_range: 2000, jump_spool_s: 10, hyperlimit: 10, scout_sensor_mult: 1, convoy_sensor_mult: 1,
  sensor_array_base: 0, sensor_array_per_tier: 0, defense_platform_radius: 0,
  systems: [{ id: system.id, name: "Aster", pos: { x: 2000, y: 3000 }, band: "rich", body_count: 1 }],
  build_options: kinds.map((kind, i) => ({ key: kind, label: SHIP_MODELS[kind].title, build_secs: 60 + i * 15,
    costs: [{ commodity: "alloys", units: 40 + i * 10 }, { commodity: "electronics", units: 20 }] })),
} as any;
state.research = { programmes: ["prop_freight_frames", "prop_heavy_lifters", "prop_line_express_charters", "prop_line_bulk_charters", "prop_line_autonomous_freight", "hull_line_iv_destroyer", "hull_line_v_cruiser", "hull_line_vi_battleship", "hull_line_vii_dreadnought", "hull_line_viii_titan"].map(id => ({ id, state: "completed" })) } as any;
const commands: unknown[] = [];
const send = (command: unknown) => { commands.push(command); $("fixture-status").textContent = `Preview command (not sent): ${JSON.stringify(command)}`; };
const ctx = { state, renderer, intent, send, net: { send } } as unknown as CoreContext;
let route: DeckRoute | null = null;
const go = (next: DeckRoute) => { route = next; empire.render(route, true); };
const empire = new DeckEmpireRoutes($("deck-workspace-body"), $("deck-build-workbench-body"), $("deck-planet-stage-body"), ctx,
  { go, replace: go, openGroundViewer() {}, notice() {}, toast() {} });
bindMarketDerive(() => ctx.net, () => empire.composedFit);
installPressGuard();
const entry = { id: "shipyard", props: { systemId: system.id, bodyId: body.id } } as const;
const mobileRoot = document.createElement("div"); mobileRoot.className = "fixture-mobile"; mobileRoot.hidden = true;
$("fixture-deck").append(mobileRoot);
const refreshMobile = () => { const view = parity.render(entry); if (view) setHtml(mobileRoot, `<h2>${view.title}</h2>${view.html}`); };
const parity = new MobileParitySurfaces(ctx, { refresh: refreshMobile, current: entry } as any, { notice() {}, openSheet() {} } as any);
mobileRoot.addEventListener("click", event => parity.handleClick(event));
$("deck-build-workbench").addEventListener("click", event => {
  const button = (event.target as Element).closest<HTMLButtonElement>("button[data-deck-act]");
  if (!button || button.disabled) return;
  empire.handleAction(button, route); empire.render(route, true);
});
await renderer.init($("app"));
renderer.setGalaxy(state.galaxy!); renderer.zoomByFactor(4);
function arrange(count = kinds.length, shownKinds = kinds): void {
  state.battles = [];
  const columns = count === 3 ? 3 : count > kinds.length ? 25 : 6, rows = Math.ceil(count / columns);
  const top = mobile ? 130 : 130, width = window.innerWidth, height = window.innerHeight - top - 80;
  state.ghosts = Array.from({ length: count }, (_, i) => ({
    id: `preview-${i}`, owner: "preview", own: true, kind: shownKinds[i % shownKinds.length],
    pos: renderer.screenToWorld(width * ((i % columns) + .5) / columns, top + height * (Math.floor(i / columns) + .5) / rows),
    vel: { x: 0, y: -.001 }, age: 0, jumped: false, route: null, count_class: "one",
    composition: [{ kind: shownKinds[i % shownKinds.length], count: 1 }], cargo: null, docked: null,
  })) as any;
  renderer.stateVersion++;
}
arrange();
const labels = document.createElement("div");
labels.style.cssText = "position:fixed;inset:0;pointer-events:none;color:#a5c1d7;font:11px system-ui";
$("fixture-deck").append(labels);
function drawLabels(): void {
  if (renderer.viewMode.type === "system" || state.ghosts.length > kinds.length) { labels.innerHTML = ""; return; }
  setHtml(labels, state.ghosts.map(ghost => {
    const p = renderer.fleetScreenPosition(ghost);
    return `<span style="position:absolute;left:${p.x}px;top:${p.y + 58}px;transform:translateX(-50%);white-space:nowrap">${SHIP_MODELS[ghost.kind].title}</span>`;
  }).join(""));
}
function closePanels(): void {
  if (renderer.viewMode.type === "system") renderer.exitSystemView();
  empire.render(null, true); route = null; mobileRoot.hidden = true;
  $("fixture-inspector").hidden = true; setHtml($("fixture-model"), "");
}
let moving = false;
$("fixture-map").onclick = () => { moving = false; closePanels(); renderer.zoomByFactor(4 / renderer.zoomFactor()); arrange(); };
function miningComparison(): void {
  moving = false; closePanels();
  renderer.zoomByFactor(10 / renderer.zoomFactor());
  arrange(3, ["small_freighter", "mining_barge", "convoy"]);
}
$("fixture-mining-comparison").onclick = miningComparison;
function stationComparison(): void {
  moving = false; closePanels();
  renderer.zoomByFactor(10 / renderer.zoomFactor());
  arrange(3, ["large_freighter", "mining_belt_station", "heavy_freighter"]);
}
$("fixture-station-comparison").onclick = stationComparison;
if (new URLSearchParams(location.search).get("comparison") === "mining-belt-station") stationComparison();
function miningSystem(): void {
  moving = false; closePanels();
  const star = state.galaxy!.systems[0];
  const belt: any = { ...body, id: 20, name: "Aster Mining Belt", kind: "asteroid_belt", parent: null, habitable: false, environment: "uninhabitable", size: "large",
    belt: { kind: "metallic", radius: .48, width: .065 }, structures: {},
    deposits: [{ resource: "bornite_ore", richness: .65, reserves: null }] };
  system.bodies = [{ ...body, parent: null, habitable: true, kind: "terrestrial", environment: "terran" }, belt,
    { ...body, id: 2, name: "Aster II", parent: null, habitable: false, environment: "uninhabitable", structures: {} }];
  star.body_count = system.bodies.length;
  state.ghosts = ["mining_barge", "mining_belt_station"].map((kind, i) => ({
    id: `belt-${i}`, owner: "preview", own: true, kind, pos: { x: star.pos.x + 300, y: star.pos.y },
    vel: { x: 0, y: 0 }, age: 0, jumped: false, route: null, count_class: "one",
    composition: [{ kind, count: 1 }], cargo: null, docked: null,
    ...(i ? { anchor: { system: star.id, body: 20, phase: "anchored", since: 0 } }
      : { industry: { kind: "mine", system: star.id, body: 20, ore: "bornite_ore", mined: 4, fraction: 0, status: "mining" } }),
  })) as any;
  state.selectedShipId = "belt-1";
  renderer.enterSystemView(star, system.bodies);
  renderer.stateVersion++;
}
$("fixture-mining-system").onclick = miningSystem;
if (new URLSearchParams(location.search).get("system") === "mining-belt") miningSystem();
if (new URLSearchParams(location.search).get("comparison") === "mining-barge") miningComparison();
$("fixture-map-kind").innerHTML = kinds.map(kind => `<option value="${kind}">${SHIP_MODELS[kind].title}</option>`).join("");
($("fixture-map-kind") as HTMLSelectElement).value = "cruiser";
function closeup(): void {
  closePanels(); moving = false;
  state.battles = [];
  renderer.zoomByFactor(24 / renderer.zoomFactor());
  const kind = ($("fixture-map-kind") as HTMLSelectElement).value as ShipKind;
  state.ghosts = [{ id: `closeup-${kind}`, owner: "preview", own: true, kind,
    pos: renderer.screenToWorld(window.innerWidth / 2, window.innerHeight / 2 - 20),
    vel: { x: .6, y: -.8 }, age: 0, jumped: false, route: null, count_class: "one",
    composition: [{ kind, count: 1 }], cargo: null, docked: null }] as any;
  state.selectedShipId = state.ghosts[0].id;
  renderer.stateVersion++;
}
$("fixture-closeup").onclick = closeup;
$("fixture-map-kind").onchange = closeup;
$("fixture-turn").onclick = () => { for (const g of state.ghosts) g.vel = { x: -g.vel.y, y: g.vel.x }; renderer.stateVersion++; };
$("fixture-formation").onclick = () => { for (const g of state.ghosts) g.composition = [{ kind: g.kind, count: (g.composition?.[0]?.count ?? 1) === 1 ? 8 : 1 }]; renderer.stateVersion++; };
$("fixture-motion").onclick = () => { moving = !moving; };
$("fixture-dense").onclick = () => { closePanels(); renderer.zoomByFactor(24 / renderer.zoomFactor()); arrange(500); state.selectedShipId = "preview-8"; renderer.stateVersion++; };
$("fixture-visibility").onclick = () => {
  closePanels(); renderer.zoomByFactor(24 / renderer.zoomFactor()); arrange(6);
  // Only the ordinary and stale contact qualify. Docked, presumed, battle and
  // pirate markers retain their established visibility/art semantics.
  state.ghosts[0].docked = system.id;
  (state.ghosts[1] as any).jump_presumed = { heading: { x: 0, y: -1 }, information_delay: 60, report_in: 30 };
  state.ghosts[2].pirate = true;
  state.ghosts[3].own = false; state.ghosts[3].age = 45;
  state.battles = [{ id: "preview-battle", pos: state.ghosts[4].pos, participants: [state.ghosts[4].id], age: 0 }] as any;
  renderer.stateVersion++;
};
$("fixture-yard").onclick = () => {
  closePanels();
  if (mobile) { mobileRoot.hidden = false; refreshMobile(); }
  else go({ name: "build", params: { id: system.id }, query: { body: "0", mode: "ships", select: "cruiser" } });
};
$("fixture-kind").innerHTML = kinds.map(kind => `<option value="${kind}">${SHIP_MODELS[kind].title}</option>`).join("");
function inspect(): void { setHtml($("fixture-model"), `<ship-model-preview kind="${($("fixture-kind") as HTMLSelectElement).value}"></ship-model-preview>`); }
$("fixture-models").onclick = () => { closePanels(); $("fixture-inspector").hidden = false; inspect(); };
$("fixture-kind").onchange = inspect;
$("fixture-stress").onclick = () => { closePanels(); renderer.zoomByFactor(1 / renderer.zoomFactor()); arrange(500); $("fixture-status").textContent = "500 fleets · model-derived map sprites · no 3D downloads"; };
$("fixture-refresh").onclick = () => { state.tick++; system.stockpile[0].units--; empire.render(route, true); if (!mobileRoot.hidden) refreshMobile(); };
$("fixture-stock").onclick = () => { for (const stock of system.stockpile) stock.units = stock.units ? 0 : 4000; empire.render(route, true); if (!mobileRoot.hidden) refreshMobile(); };
$("fixture-zoom-in").onclick = () => renderer.zoomByFactor(1.5);
$("fixture-zoom-out").onclick = () => renderer.zoomByFactor(1 / 1.5);
renderer.canvas.addEventListener("wheel", event => { event.preventDefault(); (renderer.viewMode.type === "system" ? renderer.systemZoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * .001)) : renderer.zoomAt(event.clientX, event.clientY, Math.exp(-event.deltaY * .001))); }, { passive: false });
let last: { x: number; y: number } | null = null, moved = false;
renderer.canvas.addEventListener("pointerdown", e => { last = { x: e.clientX, y: e.clientY }; moved = false; renderer.canvas.setPointerCapture(e.pointerId); });
renderer.canvas.addEventListener("pointermove", e => { if (!last) return; const dx = e.clientX - last.x, dy = e.clientY - last.y; if (dx || dy) { moved = true; (renderer.viewMode.type === "system" ? renderer.systemPanBy(dx, dy) : renderer.panBy(dx, dy)); last = { x: e.clientX, y: e.clientY }; } });
renderer.canvas.addEventListener("pointerup", e => {
  if (!moved && renderer.viewMode.type === "system") {
    const pick = renderer.systemScenePick(e.clientX, e.clientY);
    state.selectedShipId = pick?.kind === "fleet" ? pick.id : null; renderer.stateVersion++;
  } else if (!moved) { const selected = state.ghosts.find(g => { const p = renderer.fleetScreenPosition(g); return Math.hypot(e.clientX - p.x, e.clientY - p.y) < renderer.fleetHitRadius(g); });
    state.selectedShipId = selected?.id ?? null; renderer.stateVersion++; }
  last = null;
});
let samples: number[] = [], previous = performance.now();
function frame(now: number): void {
  const elapsed = Math.min((now - previous) / 1000, .1);
  samples.push(now - previous); previous = now; if (samples.length > 120) samples.shift();
  if (moving) for (const ghost of state.ghosts) { ghost.pos.x += ghost.vel.x * elapsed * 5; ghost.pos.y += ghost.vel.y * elapsed * 5; }
  renderer.update(state); drawLabels();
  const canvas = renderer.canvas;
  canvas.dataset.fleets = String(state.ghosts.length);
  canvas.dataset.frameMedian = [...samples].sort((a, b) => a - b)[Math.floor(samples.length / 2)].toFixed(1);
  canvas.dataset.selected = state.selectedShipId ?? "";
  canvas.dataset.assetVersion = "models-v1";
  const models = renderer.shipModelDebug();
  canvas.dataset.shipModels = JSON.stringify(models);
  canvas.dataset.zoom = renderer.zoomFactor().toFixed(2);
  canvas.dataset.positions = JSON.stringify(state.ghosts.length <= kinds.length ? state.ghosts.map(g => ({ id: g.id, ...renderer.fleetScreenPosition(g) })) : []);
  $("fixture-status").textContent = `${state.ghosts.length} fleets · ${renderer.viewMode.type === "system" ? `System ${renderer.systemZoomFactor().toFixed(1)}×` : `${renderer.zoomFactor().toFixed(1)}× zoom`} · ${models.active.length} live LOD2 models · ${(models.runtime?.triangles ?? 0).toLocaleString()} mesh triangles`;
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
setInterval(() => { if (route) empire.render(route); if (!mobileRoot.hidden) refreshMobile(); }, 200);
if (mobile) $("fixture-yard").click();
document.documentElement.dataset.ready = "true";
