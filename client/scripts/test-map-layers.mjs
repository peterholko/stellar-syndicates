// §map-layers: which systems' names the galaxy map shows, and how the choice
// is remembered. Your own names always show; every layer starts shown.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const source = (name) => readFileSync(new URL(`../src/${name}`, import.meta.url), "utf8");
const load = (storage) => {
  const exports = {};
  vm.runInNewContext(ts.transpileModule(source("core/map-layers.ts"), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText, { exports, globalThis: { localStorage: storage }, JSON });
  return exports;
};
const memory = () => {
  const items = new Map();
  return { getItem: (k) => items.get(k) ?? null, setItem: (k, v) => items.set(k, String(v)) };
};
const plain = (value) => JSON.parse(JSON.stringify(value));

const store = memory();
const layers = load(store);
assert.deepEqual(plain(layers.loadNameLayers()), { ally: true, rival: true, charted: true, uncharted: true },
  "defaults: every name shown");
store.setItem("ss.mapNames.v1", JSON.stringify({ ally: true, rival: true, charted: true, uncharted: false }));
assert.equal(load(store).loadNameLayers().uncharted, true, "v1 saves (uncharted hidden by default) are dropped");
layers.saveNameLayers({ ally: false, rival: true, charted: false, uncharted: true });
assert.deepEqual(plain(load(store).loadNameLayers()), { ally: false, rival: true, charted: false, uncharted: true }, "remembered");
store.setItem("ss.mapNames.v2", "{not json");
assert.equal(load(store).loadNameLayers().uncharted, true, "a damaged save falls back to the defaults");
store.setItem("ss.mapNames.v2", JSON.stringify({ rival: false, extra: true, charted: "yes" }));
assert.deepEqual(plain(load(store).loadNameLayers()), { ally: true, rival: false, charted: true, uncharted: true },
  "unknown keys and non-booleans are ignored");
const blocked = load({ getItem() { throw new Error("blocked"); }, setItem() { throw new Error("blocked"); } });
assert.equal(blocked.loadNameLayers().charted, true, "blocked storage still loads the defaults");
blocked.saveNameLayers({ ally: true, rival: true, charted: true, uncharted: true });
assert.equal(load(undefined).loadNameLayers().ally, true, "no storage at all");

const { nameLayerOf } = layers;
assert.equal(nameLayerOf("me", true, false, true), null, "your own names always show");
assert.equal(nameLayerOf("pal", false, true, false), "ally");
assert.equal(nameLayerOf("foe", false, false, false), "rival", "a rival's system whether charted or not");
assert.equal(nameLayerOf(null, false, false, true), "charted");
assert.equal(nameLayerOf(null, false, false, false), "uncharted");
assert.deepEqual(plain(layers.NAME_LAYERS.map((l) => l.label)), ["Allies", "Rivals", "Charted", "Uncharted"]);

// The renderer filters by layer, keeps the selected system named, and both
// shells offer the same toggles.
const render = source("render.ts");
assert.match(render, /t\.visible = layer === null \|\| this\.nameLayers\[layer\] \|\| selected;/);
assert.match(source("shell/deck/markup.ts"), /data-deck-act="layers"/);
assert.match(source("shell/deck/index.ts"), /ctx\.renderer\.setNameLayers\(next\)/);
assert.match(source("shell/mobile/markup.ts"), /id="m-name-layers"/);
assert.match(source("shell/mobile/index.ts"), /ctx\.renderer\.setNameLayers\(next\)/);
console.log("Map layers: defaults (every name shown, v1 saves dropped), remembered choices, damaged or blocked storage, layer per system, selection kept named, and the toggles in both shells passed.");
