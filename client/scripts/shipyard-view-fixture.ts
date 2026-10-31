// Isolated preview of the hull-under-construction viewer (§shipyard-view):
// each yard's plate with hulls at a progress. `window.__yards` holds the live
// poses; `__refresh()` re-frames every viewer after a tweak.
import "../src/ship-construction-view";
import { HULL_YARD, YARD_SCENES, type YardKind } from "../src/shipyard-scene";

const params = new URLSearchParams(location.search);
const progress = params.get("p") ?? "0.45";
const only = params.get("only");
const grid = document.getElementById("grid")!;
grid.style.gridTemplateColumns = `repeat(${Number(params.get("cols") ?? 2)}, minmax(0, 1fr))`;
const sample: [YardKind, string][] = [["shipyard", "raider"], ["shipyard", "tiny_freighter"], ["naval_drydock", "destroyer"],
  ["naval_drydock", "bulk_freighter"], ["capital_slipway", "dreadnought"], ["capital_slipway", "titan"]];
const cells = params.get("all") ? Object.entries(HULL_YARD).map(([kind, yard]) => [yard!, kind] as [YardKind, string]) : sample;
for (const [yard, kind] of cells.filter(([yard]) => !only || yard === only)) {
  const cell = document.createElement("figure");
  cell.style.margin = "0";
  const view = document.createElement("ship-construction-view");
  view.setAttribute("kind", kind);
  view.setAttribute("yard", yard);
  if (progress !== "wait") view.setAttribute("progress", progress);
  const caption = document.createElement("figcaption");
  caption.textContent = `${yard} · ${kind}`;
  cell.append(view, caption);
  grid.append(cell);
}
Object.assign(window, {
  __yards: YARD_SCENES,
  __refresh: () => { for (const view of document.querySelectorAll("ship-construction-view")) view.setAttribute("yard", view.getAttribute("yard")!); },
});
