# Planet-scene structure artwork, v2

Production sources for the approved shared-building / biome-ground art set.
These are copied byte-for-byte from the generated review collection at
`output/imagegen/surface-structures-v2`; production builds do not read that folder.
Original PNGs, sibling generation prompts, and the original source manifest are
retained. The builder verifies every promoted PNG against the approved hash.

## Coverage

- 142 building masters: 23 families with six tiers, Command Center with three,
  and the one-tier Service Berth.
- 13 natural ground layers, shared across building families and tiers.
- Living Terraces (six tiers) and the Marginal ground were added in Oct 2026,
  after the review set. They are listed in `additions-manifest.json`, which
  uses the same entry shape and hash checks as the original manifest.
- Agroplex tiers 4–6 were redone in Oct 2026 (`tier-N-v2.png`), because the
  originals barely changed between levels. `additions-manifest.json` lists
  them under `replacements`, and they ship under the original ids. The
  original masters stay beside them and in `source-manifest.json`.
- Ground Shipyard's six approved masters are already in `planet-shipyards-v1`.
- The review collection's ground-terminal concepts for `naval_drydock` and
  `capital_slipway` are superseded by the genuine orbital sprites in that same
  shipyard source directory. They deliberately are not copied here.

Together the two production scene sets cover all 28 structure kinds / 160 tier
images, plus six occupied Orbital Shipyard variants. `source-manifest.json` is
the untouched provenance manifest of the full original review set, including
the superseded concepts; the shipping manifest lists only promoted assets.

## Build and rendering contract

Run `npm run assets:surface-structures` in `client`, or the regular `npm run build`.
The pipeline creates 128/256/512 WebP derivatives, the shipping manifest, and
`src/planet-surface-structure-art.generated.ts`. Unchanged sources reuse their
derivatives. All resolutions preserve native square framing and genuine alpha;
there is no per-tier trim, repainting, flattening or upscaling.

The one exception is `FRAMING` in the builder, for the two Oct 2026 additions.
They were normalised to the 65–70% width the original prompts asked for, but
the approved set fills about 94%. So Living Terraces are enlarged 1.16× and the
Marginal ground 1.2×, each about one fixed canvas point. Every tier of a family
gets the same enlargement, so the tiers still share one framing. The masters
are untouched, and every output size is still a downscale from the master.

`planetStructureImage` is the planet-scene selector. It uses reported tiers only
and composites ordinary image layers in CSS; menus, builder cards and inventory
continue using the established icon set. Living Terraces have no menu icons of
their own, so their menus show these scene sprites (`sceneOnly` in
`src/icons.ts`). Ground is 1 / 0.82 times the building
canvas, offset to the approved gallery alignment. Buildings already contain
their contact/cast shadows, so the renderer does not add a second dark halo.

The Service Berth is drawn smaller alongside the Warehouse, sharing its ground
and address. Orbital yards have no terrain and remain above the horizon. Gas
giants also omit ground layers. Unknown future structure kinds retain a generic
fallback; Command Center and Service Berth image lookups cap at three and one
respectively without modifying a gameplay tier.

No roads are added. The existing Barren painting itself includes old baked-in
tracks/pads; this integration neither adds those nor edits the backgrounds.
