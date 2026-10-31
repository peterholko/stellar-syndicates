# Resource icons: one set

Peter (Oct 24 2026): "I want all the resource icons to look the same."

Every good's icon now comes from one style of master, the ore set's
(`client/art-src/ores-2026-09-16/`, its `prompts.json`). The build trims each
master to its artwork and re-centres it at one fill, so a stockpile reads as one
set (`buildGoodsIcons` in `client/scripts/build-art-derivatives.mjs`).

| Masters | Goods | State |
|---|---|---|
| `ores-2026-09-16/` | 16 ores and materials | in use |
| `fuel-goods-2026-10-12/` | Uranium, Antimatter, Fusion Core | made Oct 12, wired Oct 24 |
| `goods-2026-10-24/` | the 15 below | **to generate** |

Until a good's new master exists, it keeps its old icon: the square tiles and
the older, smaller cut-outs.

## What to generate

Fifteen images, one per good, with the prompt below. Replace `[subject]` with
the line from the table.

- **Save as:** `client/art-src/goods-2026-10-24/<file>.png`, with a
  `prompts.json` beside them in the ore set's format (`mode`, `assets` with
  `key` and `prompt`).
- **Size:** 1254 × 1254, genuinely transparent (real alpha, no checkerboard).
- **File names are the game's slugs**: they become the icon names.

```text
Use case: stylized-concept. Asset type: sci-fi strategy game inventory resource icon for Stellar Syndicates. A single centered resource object, polished hard-surface 3D game icon, crisp broad forms readable at 32–64 px, 3/4 view, restrained texture, strong silhouette, studio light from upper left. Object occupies 80% of square canvas. Genuine transparent RGBA background; no scene, ground, border, frame, text, labels, watermark, glow haze, UI or extra props. [subject]
```

| File | Good | `[subject]` |
|---|---|---|
| `alloys.png` | Alloys | A neat stack of three refined alloy ingots, brushed blue-grey steel with bevelled edges catching a faint warm sheen; clean industrial bars, not raw ore. |
| `provisions.png` | Provisions | A sturdy sealed ration crate: a rugged olive-green polymer container with reinforced dark-grey corners, two heavy latches and a small blank label plate. |
| `volatiles.png` | Volatiles | A tall reinforced pressure canister of volatile compounds: a steel cylinder banded with grey ribs, a narrow window showing swirling violet gas, and a heavy valve cap. |
| `deuterium.png` | Deuterium | A compact armoured fuel cell of heavy water: a squat rounded steel canister with a cyan translucent core window and a short pressure valve; shorter and wider than the Volatiles canister. |
| `biomass.png` | Biomass | A freshly harvested bundle of alien biomass: dense deep-green leafy fronds wrapped around one bulbous seed pod; organic and alive, no pot, soil or crate. |
| `electronics.png` | Semiconductors | A polished silicon wafer with a fine grid of iridescent chip dies, tilted toward the viewer, with one packaged black microchip with gold pins resting against its edge. |
| `polymers.png` | Polymers | Coiled lengths of pale green-white polymer tubing beside a small heap of polymer pellets. |
| `machinery.png` | Machinery | A compact machinery assembly: two interlocking brass gears on a steel gearbox with a short drive shaft. |
| `armaments.png` | Armaments | Two dark-grey missiles with red bands lying on a small steel launch cradle. |
| `composites.png` | Composites | A stack of three layered composite panels: carbon-black and pale steel laminate sheets with visible layer edges and a thin blue edge trim. |
| `silicates.png` | Silicates | A cluster of pale translucent silicate crystals, whitish quartz-like prisms rising from a rough grey stone base. |
| `rare_elements.png` | Rare Elements | Three sealed glass ampoules held upright in a small steel rack, holding fine metallic powders in silver, gold and violet. |
| `hull_sections.png` | Hull Sections | A curved armoured hull section: a riveted grey steel plate segment with internal ribbing visible along its open edge. |
| `precision_components.png` | Precision Components | A small arrangement of precision-machined parts: a polished bearing, a fine gear and a threaded steel fitting. |
| `warp_core.png` | Warp Core | An elongated warp-drive core: a sleek steel cylinder wrapped in copper conduits with a crisp pale-blue emitter ring at one end; lighter and sleeker than a fusion core. |

Keep each subject distinct at 32 px: Volatiles (tall, violet) and Deuterium
(squat, cyan) are both canisters, so their shapes differ on purpose.

Semiconductors keep their internal name, `electronics`, so their file is
`electronics.png`. The wafer replaces the old circuit board, which read as
"electronics".

## Checking a result

- 1254 × 1254 with real transparency; the object fills about 80–90% of the
  square, centred.
- The same 3/4 view, upper-left studio light and finish as the ore icons (lay
  them side by side with `ores-2026-09-16/preview.webp`).
- No text, frame, glow haze or background.
