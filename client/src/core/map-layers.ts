// §map-layers: which system names the galaxy map shows. Your own systems'
// names always show (and the selected system's); allies', rivals', charted and
// uncharted systems' names are layers, remembered per browser. All start shown
// (Peter, Oct 21 2026: hidden uncharted names read as a darker map).

export type NameLayer = "ally" | "rival" | "charted" | "uncharted";
export type NameLayers = Record<NameLayer, boolean>;

export const NAME_LAYERS: { id: NameLayer; label: string }[] = [
  { id: "ally", label: "Allies" },
  { id: "rival", label: "Rivals" },
  { id: "charted", label: "Charted" },
  { id: "uncharted", label: "Uncharted" },
];

export const DEFAULT_NAME_LAYERS: NameLayers = { ally: true, rival: true, charted: true, uncharted: true };

// v2: v1 saves were made while uncharted names defaulted to hidden.
const KEY = "ss.mapNames.v2";

/** The saved layers, else the defaults (storage may be absent or blocked). */
export function loadNameLayers(): NameLayers {
  const layers = { ...DEFAULT_NAME_LAYERS };
  try {
    const saved = JSON.parse(globalThis.localStorage?.getItem(KEY) ?? "null") as Partial<NameLayers> | null;
    for (const { id } of NAME_LAYERS) if (typeof saved?.[id] === "boolean") layers[id] = saved[id];
  } catch { /* defaults */ }
  return layers;
}

export function saveNameLayers(layers: NameLayers): void {
  try { globalThis.localStorage?.setItem(KEY, JSON.stringify(layers)); } catch { /* a convenience only */ }
}

/** The layer a system's name belongs to; null for your own (always shown). */
export function nameLayerOf(owner: string | null, mine: boolean, ally: boolean, charted: boolean): NameLayer | null {
  if (mine) return null;
  if (owner !== null) return ally ? "ally" : "rival";
  return charted ? "charted" : "uncharted";
}
