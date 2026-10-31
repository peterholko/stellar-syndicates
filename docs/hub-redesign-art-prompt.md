# Market Hub redesign: concept prompt

Peter (Oct 21 2026): the current Authority Nexus reads as too much built ring.
It's a broad terraced disc, and the wormhole fills only about a third of its
width (`client/public/art/derived/hub-model/v1/authority-nexus-top-1254.webp`).
This prompt asks ChatGPT for six 2D concept references with a slim frame and a
dominant wormhole. Each is shown top-down (the map below 12×) and at the map's
37° lean seen from the bottom edge (12× and closer).

Paste the prompt into ChatGPT as one message. If it stops after one image, say
"next design". To compare all six first, ask instead for "one 3×2 sheet with
only the top views of all six designs".

```text
Use case: stylized-concept.
Asset type: 2D concept reference art for a redesign of the Market Hub in Stellar Syndicates, a premium space strategy game. Six separate images, one per design below, generated one at a time. Keep canvas, views, framing, lighting and background identical across all six so they can be compared side by side.

Subject: the Authority Nexus, the galaxy's central market hub and customs gate, run by the Terran Charter Authority. It is a station built around a stable wormhole: a large swirling violet and indigo vortex with a dark core and fine cyan highlights, held open by stabilizer emitters, with docking berths where freighters arrive and leave. The wormhole is the star of the image; the station is the frame that holds it.

The problem to fix (takes priority): the current station is a broad flat disc, a wide terraced platform around a small portal, so the wormhole fills only about a third of its width. Every new design must be the opposite:
- The wormhole aperture spans at least 65% of the station's total diameter in the top view.
- The built ring, collar or frame around it is slim: its radial width is no more than about 12% of the station's radius.
- Anything reaching further out (pylons, spokes, docking arms) is slender and covers little area.
- No wide plaza, deck, terraces or solid disc around the aperture.

Composition: one landscape 3:2 canvas (1536 x 1024) per design, two views of the same station side by side, each centered in its half with generous clear space:
- Left: strict orthographic top-down plan view, camera directly overhead, its full outer diameter spanning about 80% of the half's height.
- Right: the same station in a three-quarter view at the same scale, tilted back about 37 degrees from top-down (camera about 53 degrees above the station's plane), seen from the near (bottom) edge, showing its depth and what stands above the ring.
Both views show the same design, with the same numbers of pylons, spokes, segments and berths.

Readability: the top view must still read at 48 pixels wide as a bright violet eye in a thin frame: big clear shapes, restrained small detail, strong value separation against a dark map.

Visual style: clean, high-end rendered hard-surface game concept art; crisp machined geometry, soft even light from the upper left, glow kept tight to the vortex and small running lights. Materials and finish vary per design as described below.

Background: flat, very dark navy-black (#05070D); no stars, nebula, planets or spotlight gradient.

Constraints: exactly one station per view; no ships, text, numbers, labels, logos, watermark, border, grid, frame or UI; no recognizable franchise design (no Stargate ring, no Mass Effect relay, no Death Star); no painterly brush marks.

Designs, one image each:
1. SLIM HALO: one slender polished ring framing the aperture like a lens bezel, four slim stabilizer pylons on the diagonals rising above it, small docking ports along the ring's outer edge. Ivory ceramic armor, machined brass inlay, cyan running lights.
2. FLOATING CROWN: no continuous ring. Six free-floating stabilizer spires evenly spaced around the aperture, linked by thin glowing cyan field tethers; a small separate market and customs module hovering just outside the circle. White ceramic with fine gold filigree.
3. ARMILLARY GATE: two or three thin rings set at different angles around the vortex, like an armillary sphere or gyroscope, with docking berths on the outermost ring. Dark bronze and blackened steel, art-deco detailing, warm amber lights.
4. COMPASS CROSS: a narrow ring with four long, thin docking spokes reaching out on the cardinal directions, berths along each spoke, a compass-rose silhouette. Industrial graphite with white and hazard-yellow markings, cyan lights.
5. BROKEN COLLAR: the ring split into eight separate floating segments with clear gaps between them where the vortex light shines through; each segment is a docking module. Sleek minimal matte white with glowing cyan seams.
6. PRECURSOR GATE: an ancient, narrow ring of dark stone with violet crystal veins, the gate the Authority found, with compact human scaffolding, cranes and docking clamps bolted onto it in clean ivory and brass.
```

When a design is chosen, the next step follows the hull pipeline: a single
top-down sprite, then a reference sheet, then the GLB, rebaked with
`npm run assets:hub:bake` (see [hub-model.md](hub-model.md)).
