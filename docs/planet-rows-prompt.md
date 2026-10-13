# Planet stage: tighter rows, per-row tilt, front-row sprite set

## Status, 2026-09-28 (later): superseded. Do not hand this to an agent.

The shared row grid is gone. Every painting now gets its own 24 footprint
addresses on open land, generated from the painting by
`client/scripts/build-planet-sites.mjs` into `client/src/planet-sites.generated.ts`.
The generator classifies water, lava, forest and dark rock, rough ground (cliffs,
pits, ridges) and, on vegetated worlds, anything that is not grassland. It then
grows one colony outward from the roomiest open spot, spacing buildings as
generously as each painting allows. Sprite scale and haze follow each address's
depth; the tilt experiment was removed. `test-planet-panel.mjs` fails if a master
changes without regenerating.

After replacing or adding a painting master:

    npm --prefix client run assets:derive
    npm --prefix client run assets:sites -- --review <dir>

then check the overlays in `<dir>` and run `test:planet-panel`.

### Art still needed (user's other image model)

Four paintings have too little open land for a fully built world, which holds
up to 17 buildings (a typical developed world holds 12 or 13). Their remaining
addresses fall back to the least-bad ground and are listed as `rough`.

| Painting | Addresses on clean open land |
|---|---|
| Boreal World | 8 |
| Volcanic Highlands | 10 |
| Archipelago World | 12 |
| Ocean (hostile) | 4 |

Edit prompt, one painting at a time, same 1672 × 941 size:

    Edit this planet-surface painting. Keep the composition, camera, horizon,
    lighting, colours and painterly style exactly. Widen the open ground in the
    middle distance: in the band between about 55% and 80% of the image height,
    across the central two-thirds of the width, turn forest, rock outcrops and
    small ponds into continuous open grassland with only a few scattered trees,
    enough for a settlement of about 17 buildings. Keep the large lakes, rivers,
    cliffs and forests elsewhere. No buildings, roads, fields, text or interface.

For the ocean painting, ask instead for its middle-band islands to become one
or two broad, low, grassy islands with the same total of about 17 buildings'
room. Judge the result with the generator's overlays, not by eye alone.

The earlier phases below are kept as history only.

## Context

The planet stage composites fixed-angle structure sprites onto the habitable
surface paintings. The paintings are oblique near-orbital views, so the ground
pitch changes with height on the plane, while every sprite was generated with a
35-degree isometric orthographic camera (`client/art-src/structure-tiers-2026-09-16/common.prompt.txt`).
Estimated from the paintings:

| Row    | Height on plane (today) | Terrain pitch, roughly | Sprite camera            |
|--------|-------------------------|------------------------|--------------------------|
| back   | 38–46 %                 | 20–25°                 | 35°, too steep           |
| middle | 60–68 %                 | 35–40°                 | 35°, matches             |
| front  | 82–89 %                 | 50–55°                 | 35°, too flat            |

The middle row is the primary row: on most habitable paintings the first eight
slots land there (`HABITABLE_ADDRESSES` in `planet.ts`). The mismatch shows only
as a colony grows into the front and back rows. On the non-habitable land
paintings the opening slots land on the back row first, the worst match.

What already exists in the working tree (keep all of it):

- `footprintAddress(surface, slot)` returns `{ x, y, scale }`; `ROW_SCALE = [0.55, 0.78, 1]`
  back to front; `position()` emits `--planet-x/--planet-y/--planet-scale`.
- `planet-panel.css`: `--planet-building: clamp(36px, 7.5cqw, 130px)`;
  `--planet-footprint = building × scale`; the sprite, its `::before` contact
  shadow, its `::after` ground ring and the badges all use `--planet-footprint`;
  the hit target stays at `--planet-building`.
- Fixtures: `.claude/launch.json` has `planet-fixture` (port 4174, all six
  habitable paintings with five structures each) and `planet-fixture-system`
  (port 4175: lava, barren, gas giant). Both run
  `client/scripts/test-planet-panel.mjs --serve [--habitable] --port=N`.
  The fixture's "All buildings" button fills all 24 addresses on the first
  world; "Build sites" shows empty markers. CSS is served live from
  `src/styles`; TS changes need a restart.

Plan: three phases, cheapest first, each gated by fixture captures.

Rules: work top to bottom; one commit per phase; after every commit
`npm --prefix client run build` must be clean and these must pass:
`npm --prefix client run test:planet-panel`, `test:planet-surfaces`,
`test:settlement`, `test:structure-art`. Never move the front row. Never touch
the 2026-09-16 icon set, its prompts or its derivatives. Never change the ocean
painting's addresses (hand-placed on islands). Do not add pads or roads here;
that is a separate prompt. Mobile is untouched (it has no footprint scene).
Captures are 1920×1080 of the fixture; `chrome --headless --screenshot` hangs on
the fixture's re-render timer, so capture over the remote-debugging protocol
(`Page.captureScreenshot`) or with Playwright. STOP AND REPORT after Phase B
with the captures. Phase C starts with a one-structure pilot (C0) and runs
only on an explicit go; C1–C4 need a second go after the pilot captures.

## Phase A — tighten the rows (no art)

A1. **Move the back and middle rows down**, out of the foreshortened zone, in
    `planet.ts`. Front row unchanged.

    ```ts
    const LAND_ROWS = [
      [58, 56.5, 55.5, 55, 55, 55.5, 56.5, 58],   // back: was 38–46
      [72, 70.5, 69.5, 69, 69, 69.5, 70.5, 72],   // middle: was 60–68
      [89, 86, 83.5, 82, 82, 83.5, 86, 89],       // front: unchanged
    ];
    const ROW_SCALE = [0.7, 0.85, 1];             // was [0.55, 0.78, 1]
    ```

    Keep the gentle edge curve; it follows the limb. `OCEAN_ROWS` stay as they
    are. Update the comment block above `COLUMNS` (it still says rows sit at
    least 21 % apart and describes the old heights).
    ✔ every back-row anchor is at least 20 % below the limb haze at its column.

A2. **Opening colony on the middle row for every land painting.** The
    non-habitable land surfaces (`terrestrial`, `desert`, `ice`, `lava`,
    `barren`) currently use the raw slot order, so slots 0–7 land on the back
    row. Give them the same middle-first permutation as `temperate-basin`
    (`[10, 11, 12, 13, 14, 15, 8, 9, 16, 17, 19, 20, 21, 22, 23, 0, 1, 2, 18, 3, 4, 5, 6, 7]`).
    `ocean` keeps the raw order: its row 0 is islands by design, and the test's
    `oceanSpots` assertion must not change.
    ✔ on the lava world in `planet-fixture-system`, the Mining Complex sits in
    the middle row.

A3. **Verify each painting against the new heights.** Run `planet-fixture`,
    press "All buildings", capture each of the six paintings, and check that
    none of the first 16 slots (the middle and front rows) sits on open water,
    a lake, or a cliff face. Where one does, re-permute that painting's
    `HABITABLE_ADDRESSES` so the poor addresses come last, as the existing
    permutations already do. Do not solve this with per-painting row heights.
    ✔ six captures attached to the report, with any permutation changes listed.

A4. **Rewrite the spacing test around scaled heights** in
    `test-planet-panel.mjs`. The 21 % same-column rule assumed one sprite size.
    A sprite's height as a share of plane height is `7.5 × 16/9 × scale`
    (13.3 % front, 11.3 % middle, 9.3 % back). New rules for the 24 anchors of
    every surface, including `null`:

    - same column: `|dy| ≥ (height(a) + height(b)) / 2 + 1`;
    - different columns: `|dx| ≥ 12` unchanged;
    - limb and edge clearance: keep the existing conservative reserved-footprint
      bounds (5.75 / 10.2) unchanged; they still pass.

    Update the scale expectations (`y < 50 ? .55 : y < 75 ? .78 : 1` becomes
    thresholds that separate the new rows, with 0.7 / 0.85 / 1) and the literal
    `--planet-scale:0.55` on the Mining Complex footprint, which after A2 is a
    middle-row structure on the fixture's default surface.
    ✔ `test:planet-panel` passes and would fail if any two same-column anchors
    were closer than their scaled sprites allow.

Commit: `feat(planet): tighten footprint rows toward the camera`.

## Phase B — per-row tilt (one table, five lines of CSS)

A vertical squash approximates a steeper camera on the front row and a
shallower one on the back row. It shortens the walls correctly but also
flattens the roof, which a steeper camera would deepen, so it is half right;
at 70–130 px it may read fine. It must be trivially removable.

B1. **Tilt table beside the scale table** in `planet.ts`:
    `const ROW_TILT = [1.1, 1, 0.85];` back to front. `FootprintAddress` gains
    `tilt: number`, `footprintAddress` returns it (1 for `surface === null`, the
    globe), `position()` emits `--planet-tilt:${tilt}`.

B2. **CSS** in `planet-panel.css`, applied only to sprites and the ground
    ellipses, never to site markers, badges or text:

    ```css
    .deck-planet__building > .icon { transform: translate(-50%, -50%) scaleY(var(--planet-tilt, 1)); }
    .deck-planet__site.is-construction > .icon { transform: scaleY(var(--planet-tilt, 1)); }
    /* steeper camera = rounder ground ellipse: divide by the tilt */
    .deck-planet__building::before, .deck-planet__building::after { height: calc(var(--planet-footprint) * .46 / var(--planet-tilt, 1)); }
    .deck-planet__building::after { height: calc(var(--planet-footprint) * .5 / var(--planet-tilt, 1)); }
    ```

    The workforce and output badges offset by `--planet-footprint × tilt / 2`
    so they keep hugging the visible sprite.

B3. **Test:** assert tilt per row from `footprintAddress`, that the footprint
    style carries `--planet-tilt`, that only the `.icon` rules and the
    ellipses read it, and that `ROW_TILT` is one table (so `[1, 1, 1]` turns
    the effect off).

B4. **Captures**, Temperate Basin and Highland Plateau at 1920×1080 with
    "All buildings": once with the tilt table as above and once with
    `[1, 1, 1]`. Same viewport, same world, same selection.

Commit: `feat(planet): per-row sprite tilt`. Then STOP AND REPORT with the
Phase A and B captures and a one-paragraph verdict on whether the front row
still reads as the wrong camera. The decision to run Phase C is the user's.

## Phase C — a steeper sprite set for the front row (150 images; only on go)

Decision recorded here so it is not re-litigated: the middle and back rows keep
the 2026-09-16 set, so the front set must match that set's crisp hard-surface
style exactly. Only the camera changes. No soft shading, no baked pad, no
baked shadow: the CSS grounds every row the same way. When a front-row footprint
uses the front set, its tilt is 1.

C0. **Pilot: one structure, one tier, before any of C1–C4.** Generate only
    the Mining Complex tier I with the shared prompt below. Image 1 is its
    2026-09-16 master; image 2 is that same master, since no front-set
    standard exists yet. Save it as
    `client/art-src/structure-tiers-front-<date>/mining-complex/tier-1.png`
    with its prompt file beside it. Leave the derivative build alone for now:
    make 128/256/512 WebPs with the same trim/contain/pad recipe and
    temporarily overwrite the derived files of a structure that sits on the
    FRONT row of the habitable fixture worlds (the Habitat does), open
    `planet-fixture`, and capture Temperate Basin and Highland Plateau at
    1920×1080 with the pilot on the front row beside the Phase B tilt-only
    capture of the same painting. Restore with `npm run assets:derive`. If the
    generator does not keep the design, try the Warehouse tier I as a simpler
    control before concluding anything.
    ✔ the pilot reads as the same Mining Complex from a steeper camera, and on
    the front row its roof-to-wall ratio fits the terrain better than the
    tilt-only sprite. User go before C1.

C1. **Sources.** `client/art-src/structure-tiers-front-<date>/`, mirroring the
    2026-09-16 layout: `<family>/tier-N.png` masters (square, transparent,
    1024 px preferred, never below 512), `<family>/tier-N.prompt.txt`,
    `common.prompt.txt` (below), `catalog.json` copied unchanged, `review.md`
    and comparison sheets made with a copy of `prepare-sheets.mjs`. Use the
    2026-09-16 `alpha.prompt.txt` pass when a result ships a baked background.
    Generate in this order and review each group before the next:
    `mining-complex` first (it becomes the camera standard), then `warehouse`,
    `bioharvester`, `agroplex`, `habitat` (the opening colony), then the rest.

    Shared prompt (`common.prompt.txt`). Image 1 is always the 2026-09-16 master
    of the same family and tier. Image 2 is the front set's Mining Complex
    tier I once it exists; for that first image, attach the 2026-09-16 Mining
    Complex tier I instead and say so in its prompt file.

    ```text
    Use case: stylized-concept. Asset type: one standalone sci-fi strategy-game building sprite, transparent PNG, for the FRONT ROW of the planet-surface scene. STYLE REFERENCES: image 1 is this structure's existing design identity at the old 35-degree camera; image 2 is the front-row camera and style standard. Re-create EXACTLY the building in image 1, with the same design, silhouette parts, roof layout, equipment, colours and materials, in the same crisp hard-surface 3D style, but from a STEEPER CAMERA: an elevated orthographic view about 50 degrees above the ground plane, same front-left azimuth, front and right walls still visible. Compared with image 1 the roof occupies clearly more of the silhouette and the walls are shorter; nothing else changes. Gunmetal and pale steel, restrained coloured service lights. Large readable geometry for a 96–150px game sprite, not tiny noisy greebles. Square canvas, fully centred structure, about 80% of canvas width/height with clear margins on ALL sides. No text, tier numbers, logos, UI, borders, floor, pedestal, slab, ground patch, terrain, people or backdrop. Real transparent alpha with zero opacity around and through the gaps in the structure; DO NOT draw a checkerboard pattern. Neutral top-left light, shading contained on the object; no background glow or exterior cast shadow. Keep the same compact site footprint as image 1.

    Subject: <the Subject block of client/art-src/structure-tiers-2026-09-16/<family>/tier-N.prompt.txt, verbatim, including the family colour line>
    ```

    ✔ every family's six tiers sit on one review sheet beside the 2026-09-16
    tiers, and at 150 px the two rows read as the same building from two
    cameras, not as two designs.

C2. **Derivatives.** Generalise `client/scripts/build-structure-tier-art.mjs`
    so the same recipe (`transparent-tier-contain-padding-v1`, sizes 128/256/512,
    the alpha assertions, the manifest cache) builds a named set from a source
    root into `client/public/art/derived/structures/front-<date>/` with its own
    `manifest.json`. The front set may be partial while it is generated: the
    manifest lists only families with all six masters present. Call it from
    `build-art-derivatives.mjs`. Extend `test-structure-art.mjs` to validate
    the front manifest with the same checks and to print its family count.
    ✔ `npm run assets:derive` is idempotent for both sets.

C3. **Runtime selection.** Emit a generated registry from the derivative build
    (for example `client/src/structure-art.generated.ts`: the front set's
    version and the list of families it holds). In `icons.ts`, give `icon()`
    and `structureImage()` an optional `view: "icon" | "front"`; `"front"`
    resolves to the front set's base only when the registry has the family,
    otherwise it falls back to the 2026-09-16 set. In `planet.ts`, footprints
    and construction ghosts on the front row request `"front"`; the workspace
    list and detail never do. When a footprint actually resolves to the front
    set, emit `--planet-tilt:1` for it.
    ✔ `test:planet-panel` asserts: front-row footprints reference
    `structures/front-<date>/`, middle and back rows reference `2026-09-16`,
    the workspace column never references the front set, a family missing from
    the registry falls back, and a front-set footprint carries tilt 1.

C4. **Captures.** All six paintings with "All buildings", front row with the
    front set beside the Phase B tilt-only capture of the same painting.

Commit per family group as the art lands, then
`feat(planet): front-row sprite set`. STOP AND REPORT with the sheets and
captures.

## Do not

- Move the front row, change `COLUMNS`, or add per-painting row heights.
- Change anything under `client/art-src/structure-tiers-2026-09-16/` or
  `client/public/art/derived/structures/2026-09-16/`.
- Give the front set soft shading, pads or shadows; that is a different, later
  decision that would have to include all three rows.
- Add pads, roads or haze; separate prompt.
- Reintroduce the pale sprite halo or a box around footprints.
