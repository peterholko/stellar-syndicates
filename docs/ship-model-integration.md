# Ship model integration

The game uses the approved **LOD2** version of all 20 ship designs. Production
GLBs are byte-identical to the lowest-detail exports in
`client/art-src/ship-models/interceptor-lod-experiment-v1/models`: the geometry,
materials, textures and turret nodes are preserved. No Blender step is used.

## Where the models appear

- At ordinary zoom the galaxy map uses transparent, orthographic top views
  rendered from those LOD2 files: a two-triangle quad per hull or formation.
  Past 12× fit zoom, visible hulls qualify for live LOD2 geometry. A smooth fade
  completes at 22× (after the model loads). Live hulls lean 37° to show hull
  depth (`MAP_MODEL_TILT`, the battle viewer's angled view), seen from the
  screen's bottom edge like the battle viewer and System View. The lean grows
  with the fade from 0°, matching the straight top-down sprites, which never
  lean. The System View's selected fleet uses the same live hulls and lean.
  Zooming back out restores the sprites. Three.js and GLBs are lazy-loaded only
  when a visible close-up actually qualifies.
- Galaxy camera zoom extends to **384×**. Ship sizes retain their previous
  curve through 96×, then grow proportionally to four times the old maximum.
  The same sizing drives models, fallback artwork, click targets and ownership
  labels. Near a star, from 40× (complete by 64×), hulls ease down to their
  System View size so they stay in proportion with the star's orbit preview;
  they grow back as they clear the system. Past 96× they keep that proportion
  as the preview grows, so close inspection works only away from stars.
  Desktop wheel/buttons and mobile pinch share the camera clamp and
  cursor/midpoint anchoring. A visible fleet under the pointer takes priority
  over entering a star behind it; exposed stars still enter System View at
  96×. The hub, stars and battle doorway retain their previous size/zoom
  thresholds.
- A single Three.js context renders the live hulls into a shared atlas, which
  Pixi composites inside each existing fleet container. The 3D hulls therefore
  inherit the map's position, staleness alpha, labels, pips, selection and
  galaxy/system crossfade; no separate overlay intercepts map input. Hull yaw
  compensates for the viewing tilt to preserve the served heading. The existing
  size hierarchy is preserved through close-up magnification.
- The map admits at most eight markers and 120,000 mesh triangles (including
  formation escorts), prioritizing selection, visible size and proximity. Other
  markers retain their sprites. Models of the same kind share geometry and
  textures. Only two GLBs load concurrently; texture copies are capped at 1024px
  on desktop and 512px on narrow screens. Shipyard textures remain original.
  The atlas rerenders only when a model, heading or viewing tilt changes, capped
  at 30 updates/sec; straight flight/panning uses the existing Pixi transforms.
  Idle resources expire after 15 seconds; covered mobile maps release them.
- When a visible hull outgrows its normal atlas sample, inspection uses two
  slots at twice the tile resolution: 1536px on desktop, 768px on narrow
  screens. The atlas has the same total pixel area as the ordinary eight-slot
  layout. Close-up texture copies use 2048px/1024px respectively; selected
  fleets keep first priority and other markers retain their artwork. A lower
  return threshold avoids repeated quality swaps near the boundary, and the
  prior renderer and cached models are released when changing quality.
- Failed downloads or a lost WebGL context retain the sprite and retry after a
  cooldown. Offscreen ships, ordinary docked fleets, battle-suppressed fleets
  and presumed jump arrows do not load or display models. Pirate art retains
  its established separate silhouettes; migrant liners use the colony model.
- Fleet formations, hull icons and the shared battle artwork use the same
  designs. Pirate-specific artwork and migrant-liner selection remain separate.
- Participant battle viewers render these same LOD2 hulls in a shared Three
  scene, composited between the existing ship artwork and weapon/UI layers.
  Static sections sharing a material are merged without changing triangles or
  UVs, then instanced across all representatives of that type. Team glows,
  hull meters, ×N counts, flagship names, hover and camera controls are retained.
  Labels remain above the 3D surface. Ship headings, sizes, movement, damage,
  arrivals and withdrawals still come from the existing replay presentation.
- Battles admit up to 80 representatives within a 360,000-triangle hull budget
  (130,000 on narrow screens), with at most eight cached types (five on narrow
  screens). Very small, offscreen or over-budget hulls keep their artwork;
  privateers and platforms keep their distinct designs. The existing performance
  ladder lowers the model/effect budgets under load. Texture copies use 1024px
  or 512px, two files load concurrently, and all 3D resources are released on
  close. A failed load, render or context restores ordinary artwork and FX.
- Recorded hits illuminate nearby hulls. Destruction produces mesh fireballs,
  shaded smoke, spinning hull fragments, soft glow and tilted shock rings in
  bounded particle pools. Laser/driver paths, torpedo trails, point defense and
  armor mitigation feedback keep their existing visual grammar. No impact is
  added to a miss; no effect precedes its arrived evidence. All mesh particles
  are seeded functions of replay time, so pause and seeking are deterministic.
  Reduced-motion preferences disable shake and soften flashes/fragment motion.
  The entire theater runs at up to 30fps to keep hulls and overlays in sync;
  an unchanged paused 3D scene requires no rerender or texture upload.
- The flat effects are pooled soft sprites painted at startup
  (`battlefxsprites.ts`, helpers in `battlefx.ts`): light (beams, flames,
  sparks, flashes) blends additively so overlapping effects brighten, and
  smoke is drawn normally beneath it. Guns and engines sit where the 3D hulls
  have them: `npm run assets:hardpoints` bakes each shipped hull's named engine
  and barrel sockets into `ship-hardpoints.generated.ts`, in the model space
  the viewer draws, and `test:battle-fx` fails when that table is stale.
  Engines burn in their side's colour from every nozzle, longer with the hull's
  recorded speed and change of speed, and flat out while its side withdraws.
  Lasers and drivers leave the barrels nearest their target; capital lances
  draw light into the barrel first; beam hits leave a brief glowing scorch and
  driver hits spray sparks back along the line of fire. Torpedoes trail smoke
  behind a motor in their side's colour, and point defense fires tracer dashes
  from its guns. Hulls vent smoke below 60% and 40% hull and burn with sparks
  below 30%; every evidenced hit lights the hull for an instant. Everything is
  seeded on replay time, drawn only from arrived evidence, and thinned by the
  performance ladder (fewer smoke puffs, no nozzle glows or sparks).
- Ships move along curved tracks through their recorded positions (exact at
  every round, with velocity carried across rounds), so fast hulls sweep and
  slow ones crawl instead of every ship easing to a stop at each keyframe.
  A track uses the next round only once its light has arrived.
- Facing is presentation (the engine records positions only): at each round a
  hull turns toward its direction of travel by at most its class's turn rate
  (`TURN_RATE` in `battlefx.ts`: an Interceptor 2.2 rad a round; hulls
  heavier than a destroyer use the engine's own v9 limits, a Battleship 0.20,
  a Titan 0.085, about 37 rounds to come fully about, so their noses follow
  their recorded courses exactly), spread evenly through the round, and a
  hull moving under 1.5 units a round keeps its facing. The chain runs from
  the battle's first keyframe, so scrubbing and replays agree.
- Torpedoes: rounds record every torpedo in flight (stable id, target,
  position) and how each ended (hit with its damage, miss, shot down and by
  which hull, or ran dry). The viewer (`torpTracks`/`torpHead` in
  `battletheater.ts`) flies each one in a straight line between its recorded
  positions at its real speed (140 units a round), so a torpedo carries on
  across rounds instead of restarting. A launch kindles on the hull it sits
  on and leaves with the next round; a hit explodes on the hull the record
  names, at the moment it arrives at that speed, and is that hull's evidenced
  impact; a shot-down torpedo pops where it died while the hull that killed it
  hoses it; a miss coasts on past and gutters out; a torpedo whose target died
  runs dry and fades. Point-defense screens (PD-fitted hulls and platforms)
  fire light dashes at enemy torpedoes inside their bubble. Older frames that
  only summarize salvos keep the cosmetic arcs.
- Test fights: `__ss.theaterDemo()` opens a real engine fight (`"mixed"`,
  `"swarm"`, `"torpedo"`, `"titan"`, `"collision"`, `"formations"`; `"scripted"` keeps the old hand-made
  demo), and the battle preview page lists the same fights. See
  `docs/battle-rules-v10.md` for how they are recorded and refreshed.
- Deaths (`battlewrecks.ts` rules, drawn from a per-record death timeline in
  `battletheater.ts`): each recorded death becomes an event at the moment its
  last evidenced hit lands, on the hull's on-screen track, with the velocity
  and facing it had then; everything after is a function of replay time.
  Light hulls shatter or break at once. Capitals burn and cook off along their
  length first (0.8 rounds; a titan 1.8), then breach: a fireball, a shock
  front that pushes nearby smoke and debris, and the hull breaks into sections
  (2–5 by type, cut at the same places in 3D and in the flat art) that drift
  apart with the hull's momentum, tumble, char dark and keep burning; capital
  wrecks stay for the battle (past eight, the oldest fade). Debris is cut
  from the dead hull's own flat art once that art has loaded: six torn plates
  and two long spars per hull type, sized so the plating shows at the hull's
  own scale, with charred rims and one lit edge (`debrisCuts` picks solid
  metal; `paintDebrisAtlas` paints them once). The pieces glow hot, then
  tumble — foreshortening, darkening edge-on and glinting face-on — and lie
  in the battlefield plane beneath every hull and wreck. Plain painted plates
  stand in only until the art loads. In 3D, blast shrapnel glows and goes out
  as it cools; the cold metal is always the cut debris. Embers cool from white
  to dark, and a dust haze and a smoke cloud lit from inside linger. A titan's breach lights
  the whole battlefield and raises the "IS DOWN" banner. Concluded battles
  play a three-round epilogue past their last record so final deaths play
  out. 3D wreck sections share the intact hull's vertices (a per-instance char
  darkens them and dims their lights); the flat art is sliced when 3D is off.
  Under rules v9 a large hull's wreck (8000+ mass) is the engine's: the hull
  stays whole until the engine breaks it, then each section sits where the
  round recorded it (`wreckTracks`/`wreckPiecePos` in `battletheater.ts`,
  interpolated between rounds and held after its last record), with tumble,
  char and fires drawn on top. A hull ramming wreckage takes a visible impact
  (a flash and sparks where it struck, and its hull meter drops then), and a
  torpedo that strikes wreckage detonates on it. See `docs/battle-rules-v9.md`.
- The theater looks at the battlefield at a slight angle by default
  (`THEATER_TILT` 0.8 in `battletheater.ts`, about 37°; the System View uses
  0.6). Only the camera changes: positions, timing, hits and `theaterHash` are
  the same in either view. Vertical screen distances shrink to 0.8, the arena
  rings and shock rings become ellipses, flat ship art and glows are squashed
  onto the plane, and lower (nearer) hulls draw over higher ones. 3D hulls,
  shock rings and particles lie in the same plane, seen from its lower edge,
  and sit nearer the camera the lower they are, so the depth buffer gives the
  same overlap order. Labels and hull meters stay upright and readable. The
  fit view zooms in a little to use the stage's width.
- Hull meters: only the ship under the mouse pointer (a held finger on touch
  screens) shows its hull meter, just below the hull, with the tooltip above
  it. Every ship's pointable area is its hull, never smaller than 12 px on
  screen; a destroyed or not-yet-arrived ship cannot be pointed at.
  A "Top-down" button in the stage's top-right corner switches to the
  straight-down view and back, easing over about a third of a second
  (instantly with reduced motion). The choice is remembered per browser
  (`ss_theater_view` in localStorage). Top-down, hulls still lean 18° and
  shock rings 0.18 rad the same way as the angled view, as seen from the
  stage's bottom edge: a hull flying east shows its near (starboard) side.
  Switching views eases one steady lean, from −18° to the angled plane's −37°
  (`BATTLE_MODEL_TILT`, mirrored in battletheater.ts).
- Frame rate: the viewer draws its 2D and 3D layers together, at up to 60 fps
  on desktop and 30 on phones (the mobile shell asks for 30). At 30 a fast
  interceptor, zoomed in, jumped about 12 px a frame at 1×; at 60 it moves
  about 6. A stage that can't hold 60 drops to 30 before any effect is cut,
  and tries 60 again 20 s later, waiting twice as long after each failed
  try. The first 1.5 s after opening a battle (model loading) don't count.
  `theaterDebug().fps` shows the cap, the preferred cap and the measured rate.
- Camera: a battle opens on the arena overview, following one of the
  player's hulls: their heaviest still in the fight (lowest id on a tie). The
  zoom is always the player's. The camera's centre locks on the followed hull
  but is held inside the overview's frame, so at the opening zoom the whole
  battle shows and zooming in closes on that hull. When it falls or leaves,
  the camera follows the next heaviest and glides there in 0.6 s. A click or
  tap picks the nearest hull within 22 px (or its own drawn size), selecting
  and following it; a press only becomes a drag past 5 px, so a click's wobble
  never cancels it. A drag frees the camera (the selection stays);
  double-click (or Reset camera) returns to the overview and resumes
  following. A battle the player only watches follows no one. The hint in the
  stage's corner names the followed hull. `theaterDebug().follow` shows the
  state.
- Ship card (desktop): the selected hull's card leads the info column: art
  and name (a Titan's name too), whose it is and what it is doing (in the
  fight, arriving, withdrawing, leaving, destroyed), its hull at the moment
  shown (amber under 60%, red under 30%), its fit and a Follow / Stop
  following button. The record keeps fits per hull type, so a type flown in
  two fits shows "one of" them. The theater exposes it as `theaterSelection()`
  and `theaterFollow(on)`.
- Replay speed: a finished battle opens at 1×, the battle's own pace, with
  1× and 4× to choose from. Live battles always play at 1×.
- Size: in the game the viewer's canvas is drawn at its panel's size (the
  layout sets the panel; a `ResizeObserver` re-attaches the theater when it
  changes). On a window at least 1200 px wide, the battle card fills the
  window height and up to 1680 px of width. The viewer takes all the height
  under the header, with replay controls beneath it, and the status, forces,
  notes and actions sit in a 320 px column beside it: about 1192 × 863 px at a
  1600 × 1000 window, against the old fixed 1068 × 672. Narrower windows stack
  that column below the viewer, which takes the window height less 170 px
  (360–1200 px).
- Live battles: rounds arrive on the server's update beat (10 Hz desktop, 5 Hz
  phone), so each lands up to a beat late. Live playback trails the live edge
  by one round plus 0.25 s, easing its speed between 0.8× and 1.25× to hold
  that, so a late round never stops the hulls. Both shells share this
  (`core/derive/liveplay.ts`). A burst of late rounds jumps back near the edge
  instead of racing through it. A finished battle plays out to its end at its
  own pace.
- The desktop construction workbench and mobile shipyard display an interactive
  preview of the selected, researched hull. The construction catalogue and
  authoritative recipes determine which ships can be built; the art catalogue
  does not grant construction access to Authority or other special hulls.
- The desktop fleet details workbench shows the selected fleet's hull as its
  hero (`docs/fleet-panel.md`).
- The Market Hub is the Authority Nexus station: a baked sprite on the galaxy
  map that becomes a live 3D model with a flowing vortex up close
  (`docs/hub-model.md`).

`ship-model-preview` loads its Three.js module only when visible. It fetches only
the selected GLB, draws on interaction/resize rather than running an idle
animation loop (`turntable`, below, is the one exception), and frees textures,
geometry, controls and its WebGL context when removed. Superseded loads are aborted and late results are disposed.
The shadow root keeps its canvas and camera intact through live panel morphs.
Failed loads show the matching ship image with a retry control; build controls
continue to work. Keyboard arrows, zoom buttons and six view presets accompany
mouse/touch orbit controls.

Every view frames the hull's own vertices to 84% of the stage (`FRAME_FILL`),
not its bounding box, so long, flat hulls fill wide panels. With `turntable`,
the camera sways 0.5 rad either side of the framed angle over 24 s. That view
is framed for every bearing the sway passes through, so it never swings nose-on.
The sway runs only while the stage is visible. A drag, wheel zoom, arrow key or
view preset ends it.

## Asset pipeline

The explicit save-game-kind mapping is in
`client/scripts/ship-model-catalog.mjs`. In particular, `raider` is Interceptor,
`convoy` is Medium Freighter, and `freighter` is Authority Freighter.

1. Run `npm run assets:ships:bake` from `client` after changing approved models.
2. Open the loopback URL printed by the baker. It renders every missing or stale
   view with the same lighting used in the shipyard and records source/render
   hashes. Verified views of unchanged models are retained byte-for-byte. The
   catalog has 20 designs and 60 views. Top masters are 1254 pixels; underside
   and perspective checks are 768 pixels. The Survey Probe remains a Scout alias.
3. The baker generates map tiers, 40 panel images, 12 fleet formations and
   perspective fallback images. `npm run build` subsequently rebuilds only
   changed derivatives and checks that masters still match their model sources.

Editable baked masters and their provenance are in
`client/art-src/ship-models/game-ready-v1`. Production images live in
`client/public/art/derived/ships/models-v1`; GLBs live in
`client/public/art/derived/ship-models/v1`. GLBs total about 45.8 MiB across all
20 ships; these are individual lazy requests, not an initial-page bundle.
Neither the experimental viewer nor its embedded full-detail assets ship.

The Mining Barge replaces its Small Freighter stand-in with its own 4,970-triangle,
two-material LOD2. Its original textures, UVs and four socket nodes are preserved.
Two engine sockets drive battle thrusters; the two `socket.mining.*` nodes remain
for later cutter beams and are not weapons. Battle display size still uses its
3,500 mass. The lab adds it at a 2.902× Interceptor display length, between the
Small and Medium Freighters, with a 40-direction atlas. The approved source
model and reference art are unchanged.

Mining Barge acceptance captures are in
`client/art-src/ship-models/game-ready-v1/mining-barge-map-comparison.jpg` and
`mining-barge-fleet-panel.jpg`. The map comparison uses the actual galaxy
renderer at ordinary sprite zoom; these are map indicators, not physical scale.

The Mining Belt Station replaces its Construction Ship stand-in with its own
8,420-triangle, two-material LOD2. The octagonal core, six silo surfaces, open
barge cradles, forward loading gantry and four anchor frames retain their visible
source triangles. Reduction removes wholly buried faces and simplifies small
fittings; the approved source and original texture bytes/UV attributes remain
unchanged. All seven docking/anchor nodes survive. Its engine and weapon
hardpoint lists are empty. Battle size uses mass 12,000; the lab uses its 4.751×
length ratio between Large and Heavy Freighters. Map sizing retains freight base
86 and the 180-pixel class cap at 96×, with the existing zoom behavior beyond it.

Station acceptance captures in `client/art-src/ship-models/game-ready-v1/`:
`mining-belt-station-map-comparison.jpg`, `mining-belt-station-fleet-panel.jpg`,
and `mining-belt-station-system-view.jpg`. They use the actual game renderers
and fleet controller in isolated fixtures; the System View shows the anchored
station and a Mining Barge working the same belt. No game orders are sent.

## Verification

- `npm run test:map-zoom`: 384× camera limits, cursor/button anchors, panning,
  exact camera restoration, desktop wheel/mobile pinch priority, 96× system
  entry, close-up hull/pick ratios, and unchanged hub/star sizing.
- `npm run test:ship-models`: coverage of the actual `ShipKind` union, exact LOD2
  bytes and triangle counts, embedded textures, all 60 baked views, derivative
  provenance, and repeatable builds. It also runs ship-art and map-size checks.
- `npm run test:map-ship-models`: real admission, fade, cache, texture-resizing
  and transform code with network/GPU boundaries substituted. Covers a 500-fleet
  budget, selected priority, viewport edges, sprite fallback, stale alpha,
  context failure, idle cleanup, shared-resource ownership, aborted loads,
  projected headings, atlas coordinates and unchanged size/art regressions.
  This is an automated logic check, not a GPU performance benchmark.
- `npm run test:battle-fx` (part of `test:battle-models`): the baked hardpoint
  table matches all 20 shipped hulls, the speed table mirrors the sim, and the
  plume, gun, thrust, flicker and damage-vent geometry is checked.
- `npm run test:battle-wrecks` (part of `test:battle-models`): break-up tables,
  staging, drift, char, section motion, debris, art-cut debris layout (solid
  metal, spread apart, plates and spars, hull-scaled), embers and shock
  fronts. The theater tests also cut every real hull into its sections (every
  triangle in exactly one; at rest they match the intact hull exactly) and
  check blast placement, staging, the titan flash, the epilogue, the wreck
  cap, and that debris uses the hull's cut art beneath the hulls, hot then
  cooled and shaded by its tumble.
- `npm run test:battle-models`: all 20 actual GLBs through the production static
  consolidation and instance transforms; checks exact triangle totals, UVs,
  dimensions, headings, opacity, 500-candidate admission limits, instanced draw
  cost, particle determinism, model loading/cleanup, fallback and context loss.
  The real theater is also exercised with synthetic arrived records to verify
  damage/death timing, missed shots, frontier/no-frame behavior and unchanged
  record data. Network and GPU boundaries are substituted, not visual output.
- `npm run test:industry`, `test:progression`, `test:planet-panel`,
  `test:fleet-lists`, and `test:live-battle`: construction, existing UI and shared
  artwork regressions. The progression freight fixture supplies both ground and
  orbital yards so it tests research independently of the existing yard gates.
- `npm run preview:ship-models`: an isolated browser fixture using the real
  galaxy renderer, desktop/mobile construction controllers, DOM reconciler and
  preview component. Its build orders are recorded locally and never sent.
  It includes all 20 designs, a 500-fleet map, report refresh and stock controls.
  Close-up controls select each map hull, change heading, show formations,
  move ships and exercise 500 overlapping close-ups and visibility exclusions.
  Its Battle model preview link opens `scripts/battle-model-fixture.html`, with
  mixed combat, all 20 designs, an 80-ship fleet, individual hulls, replay seeking,
  destruction timing, phone layout, zoom and close/reopen controls. It uses
  synthetic records and the production theater, without a game connection.

Browser checks covered all 18 triangle counts, underside views, preserved
camera state after a report refresh, disabled build controls without stock,
phone-width layout, failed model requests and successful retry. The map resource
tree was also checked: no GLB or 3D-runtime requests at startup. Those browser
checks predate the live map transition; its browser verification is pending
local-preview access. The new automated transition tests run without browser
access and do not substitute for checking the final GPU composite visually.
Battle model and effect browser verification is pending the same saved local
browser permission; the automated geometry and replay checks are complete.
