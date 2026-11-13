# Research panel plan

Plan, November 1 2026.

Make Research fill the game workspace, with illustrated programme cards and a
large detail pane. Follow the structure panel's card-and-detail layout and use
the six field emblems as the visual family. Every programme gets its own emblem;
until it arrives, its field emblem fills the same space.

This implements Peter's request for research to be “full screen like the
structure panel”, with “larger graphics for research”, recorded in
[the art brief](research-art-prompts.md). The existing Research, Queue and
Completed pages remain the navigation. This is a presentation change; research
costs, prerequisites, effects and completion stay with the simulation.

## Catalogue and art

The current [catalogue](../crates/sim/src/research.rs) has **119 visible
programmes**. All 119 have exactly one matching entry in
[prompts.json](../client/art-src/research-emblems-v1/prompts.json); ids, names,
fields and tiers agree, and all referenced image files exist.

| Field | Programmes |
|---|---:|
| Propulsion | 18 |
| Materials | 22 |
| Computation | 18 |
| Weapons | 21 |
| Hulls | 22 |
| Life | 18 |
| **Total** | **119** |

There are currently prompts but no programme PNGs in `research-emblems-v1`.
The art delivery sequence is **3 tests, 40 further Tier I–II emblems, then 76
Tier III–VIII emblems**. The three tests are part of the 43 early programmes.
Sixteen prompts also reference an existing hull or station render.

Use programme emblems consistently on cards, in the detail pane, in the queue
and beside active research. Pictures of unlocked objects belong in a separate
Unlocks section; they do not replace the programme emblem.

## Desktop layout

Open Research below the top bar and stores row, across the available viewport,
with a small outer margin and its bottom edge above the command and guide
stack. Remove the saved standard/wide width choice for this route. Back, Close,
Escape, the R shortcut and programme deep links keep their existing meanings.

Research should use the workspace's overlay camera behaviour so opening it
does not squeeze or pan the map. It needs its own full-width CSS rule: adding
it to the existing popout list alone would retain the other panels' width cap.
The structure sheet in
[planet-panel.css](../client/src/styles/planet-panel.css) supplies the visual
reference: illustrated cards, a bordered selection, an independently scrolling
detail column and actions close to the selected item.

| Region, from top to bottom | Contents and behaviour |
|---|---|
| Workspace header | Back, Research title, Close. |
| Page navigation | Research, Queue with its current count, Completed with its current count. |
| Active research bar | Emblem, name, progress, served ETA or paused state, research rate and the existing grant action. Visible on all three pages. |
| Field navigation | Six named field buttons, each with its field emblem and available count; Completed uses completed counts. |
| Tier navigation | Only tiers present in the selected field and page, through VIII where applicable. |
| Main area | Programme cards on the left and the selected programme on the right. Queue replaces this area with the ordered queue and Academy information. |

Keep the header and navigation out of the main area's scroll. Give the card
area and detail pane their own scroll on a wide screen, and keep the detail
action visible without covering its content. Use `min-height: 0` through the
layout so long descriptions and supplies can scroll inside the available
height.

### Programme cards

Browse one field and tier at a time, with school headings for the later tiers.
Use a wrapping grid of roughly 172–208 px cards, matching the scale of the
structure picker. Sparse tiers retain this card size instead of stretching one
card across the page.

Each card contains a 112–144 px emblem, the full programme name and a short
state label: Available, Locked, Queued, In progress or Completed. Allow names to
wrap. Selected cards have a clear border and `aria-pressed`; state remains
readable without colour. Locked cards can still be selected to inspect their
requirements.

Selecting a card only changes the detail pane. There is one Add to queue action
for the selected programme. Incoming reports update the card state and detail
figures without changing the selected field, tier or programme. Keep separate
selections for Research and Completed, as the current controller does.

### Selected programme

Use a detail column around 320–380 px wide on a roomy desktop, with a 192–224 px
programme emblem. Show the following in order:

1. Name, field, school where applicable, tier and current state.
2. The catalogue's benefit description.
3. Unlocks, when there are concrete objects to show.
4. Research work, expressed as research hours, and supplies per research hour
   at rate 1. These are work and consumption units; active ETA remains the
   server's supplied estimate.
5. Recovered data, predecessor requirements and the served milestone progress.
6. Academy or supply guidance when relevant, followed by the queue action.

Keep the explanation next to a disabled queue action. A met-looking milestone
does not permit the client to override a still-locked server state. Lack of a
staffed or supplied Academy prevents progress but does not prevent planning an
available programme into the queue.

Completed programmes retain their benefit and unlock information and have no
queue action. A missing report or an empty category gets an explicit empty
state with the navigation still usable.

### Unlock pictures

Support multiple unlocks per programme. For example, Bunkerage opens both the
Volatile Harvester and Fuel Refinery; Dreadnought includes a hull and building
unlocks. Show a small labelled image for each object instead of choosing one
and losing the rest.

| Unlock | Existing art to reuse |
|---|---|
| Building or building tier | `structureImage(key, tier, …)` and the existing structure art registry. |
| Orbital defense level | `stations/v2`, through the existing structure helper; Fire Control corresponds to level 2, Star Fortress to level 6. |
| Hull, including Mining Belt Station | The `poster` in `SHIP_MODELS`, using the actual `models-v1/previews` render. |
| Equipment module | The existing equipment registry and its icon; no extra module illustration batch. |

In the current catalogue, 34 programmes have a hull or structure unlock, six
more have only module unlocks, and 79 have modifiers or capabilities without a
direct object unlock. Those 79 still get their programme emblem and full
benefit text. An empty Unlocks section is omitted.

The Antimatter Plant currently resolves to the Fuel Refinery's art. Its own
art remains the separate task in
[fuel-overhaul-art-prompt.md](fuel-overhaul-art-prompt.md).

## Queue and Academy behaviour

The Queue page keeps the active programme pinned first, followed by the ordered
waiting programmes. Add 48 px emblems, keep the named inspect action, and retain
Up, Down and Remove for waiting items. The count includes the pinned active
row, matching the existing page. Academy rows follow the queue, with supplied
state, power warnings, contribution rate and Open Academy.

Preserve these command rules:

- Only the selected, server-reported available programme can be added; an id
  already in the arrived queue cannot be added again.
- The active row cannot be moved or removed, including through a stale action.
- `SetResearchQueue` sends the waiting ids only when research is active. When
  idle, the first submitted item becomes active in the simulation.
- Sending a command does not mark a programme queued or completed locally.
  The existing notice acknowledges dispatch; an incoming report supplies state.
- Research grants use the served price, amount and daily limit. Progress and
  credits update from reports.

Desktop already filters the active id before sending. Mobile currently sends
the displayed queue including the active id and allows controls on that row.
Share the queue derivation and action guards in this pass so both surfaces
follow the simulation's `ResearchState::set_queue` contract. Correct the stale
comment in `core/derive/research.ts` that suggests sending the active row back.

## Narrow screens and mobile

Use the panel's available width to choose the layout. Around 900 px, reduce the
detail column toward 280 px and cards toward 144 px. Below roughly 700 px,
switch to a card browser and a selected-programme view with a Back to programmes
control. Keep the browser's selection and scroll when returning. On short
screens, let navigation join the page scroll rather than squeezing the main
area to an unusable height.

The native mobile shell is a separate implementation in
[mobile/parity.ts](../client/src/shell/mobile/parity.ts). Give it the same field
art, programme art, tier selection, detail facts and queue rules within its
existing sheet. Use 32–48 px queue emblems and a 144–192 px detail emblem. Share
presentation data and pure derivations across the shells; use each shell's
own navigation and action plumbing.

Use native buttons with visible keyboard focus, labelled queue controls and
touch targets of at least 44 px. Artwork adjacent to a programme name uses an
empty alt attribute. Card selection, report refreshes and closing the panel
must preserve or restore focus appropriately. Honour reduced motion and avoid
animating emblem replacements.

## Data and asset implementation

`ProgrammeInfo` supplies names, fields, schools, tiers, costs and descriptions,
but no structured list of unlocks. Add a small typed presentation map keyed by
programme id for concrete unlock illustrations. Use it only for labels and
art. Availability, gates, cost and benefits continue to come from the served
research view; this version needs no protocol change.

Populate that map from the catalogue's effects and check it against the sim's
building rules. `UnlockStructureTier(kind, 4)` is not universally a literal
“Tier IV unlocked”: ordinary buildings use that marker to open tiers V–VI,
while stations and Command Centers have their own rules in
[`max_buildable_tier`](../crates/sim/src/build.rs). Keep captions and depicted
tiers consistent with those rules. Multiple effects must survive the mapping.

For the art pipeline:

1. Extract the six field badges from the approved sheet with explicit crop
   rectangles and masks around their hexagonal rims. The sheet has a painted
   background, so rectangular crops alone are insufficient. Preserve the rim
   and navy face, and check all six against the source before using them.
2. Add `build-research-art.mjs`, following the existing derivative scripts.
   Read approved `<programme-id>.png` masters from `research-emblems-v1` and
   emit WebP files under `public/art/derived/research/v1`.
3. Derive field images at 64, 128 and 256 px, and programme images at 64, 128,
   256 and 512 px. Keep the shared framing and alpha; validate actual transparent
   pixels, not merely the existence of an alpha channel. Keep the 1254 px
   programme masters out of the runtime request path.
4. Generate `research-art.generated.ts` from the files actually delivered.
   Missing programme masters are valid during rollout; malformed delivered
   images and unknown ids are errors. Cache generation using source and recipe
   fingerprints, following the station pipeline.
5. Resolve art as programme emblem, then field emblem, then the existing field
   icon as a final fallback. Use the manifest to choose URLs instead of
   requesting nonexistent programme files. Handle a failed asset request with
   the same fallback without an error loop.
6. Supply image dimensions and responsive sources. Load the selected emblem
   promptly and other cards lazily. Opening Research should request art for its
   visible UI, without preloading all 119 masters.

## Files and delivery order

| Step | Main files | Reviewable result |
|---|---|---|
| 1. Field art and shared presentation | New `client/scripts/build-research-art.mjs`, `client/src/research-art.ts`, generated art manifest and `client/src/research-presentation.ts`; integrate with `build-art-derivatives.mjs`. | Six field badges, fallback resolution and a typed unlock map. Missing programme art is supported. |
| 2. Full desktop panel | `client/src/shell/deck/workspace.ts`, `roster.ts` and `client/src/styles/deck.css`. | Full-width layout, emblem grid, large detail, existing Research/Queue/Completed flow and programme deep links. |
| 3. Shared queue rules and mobile | `client/src/core/derive/research.ts`, `client/src/shell/mobile/parity.ts` and `client/src/styles/mobile.css`. | Mobile art and selection flow, with both shells protecting the active row and sending waiting ids only. |
| 4. Programme art rollout | `client/art-src/research-emblems-v1` and generated derivatives. | Three test emblems checked at card and queue sizes; 40 early and 76 later emblems follow. The panel can ship after step 3 using field fallbacks. |

Keep `DeckRosterRoutes` as the desktop entry point and preserve the existing
route and action names. If shared markup helpers are extracted, the controller
still owns selection and dispatch. Changes stay within the research surface
and its shared helpers.

## Acceptance checks

Extend the existing
[research workbench fixture](../client/scripts/test-research-workbench.mjs),
which exercises the real desktop controller. Preserve its existing checks for
selection-only clicks, server-authoritative availability, pinned active work,
queue payloads, recovered data, supplies, empty states, selection persistence
and the Enrichment deep link.

Add focused checks for:

- Complete catalogue-to-prompt coverage and valid unlock-art mappings,
  including multiple unlocks and the structure-tier exceptions.
- Programme art, missing-art fallback and image-load failure; generated URLs
  resolve, and the derivative build works with zero programme masters.
- Queue action parity on desktop and mobile, including idle queues, stale
  actions, active-row protection and an already queued selection.
- Programme deep links, inspecting active or queued work, and returning from
  Completed without losing the Research selection.
- Workspace sizing with an old saved standard-width preference; opening and
  closing Research must not alter the map's horizontal camera allocation.

Update the fixture's preview to mount the real `DeckWorkspace`; it currently
hardcodes a wide sidebar and a width-toggle button, which cannot verify the
new overlay behaviour. Keep its fake transport and explicit report delivery.

Visually check 1920×1080, 1366×768, 1024×768 and a 390 px mobile viewport, plus
200% zoom. Include long names, Hulls VIII, Materials VI, an empty Completed
field, an unsupplied or low-power Academy, a populated queue and missing art.
Verify readable 32–48 px emblems, keyboard access, stable focus and scroll
during reports, and reachable actions above the bottom UI. No horizontal page
overflow or broken-image indicators should remain.

After implementation, run `npm run test:research-workbench`, the focused mobile,
workspace and art checks added for this change, and `npm run build` from
`client`. A plan-only change does not require a game build.

## As built (November 1 2026)

Built to this plan, client-only, with no protocol change and no galaxy reset.
Where the build differs:

- **One tier ahead, nothing deeper.** Peter also asked to "hide what research
  is available past 1 level deep". `visibleProgrammes` in
  `core/derive/research.ts` renders everything researched or researchable,
  plus the next step after each of those, using the sim's ladder rule
  (`isPredecessor`). Both shells use it. The tier navigation lists only the
  visible tiers, and a stale action cannot open a hidden tier.
- **Width.** The frame spans the map area but stops at 1600 px on very wide
  screens. The cards grow to 208 px with 144 px emblems once the panel is
  1280 px or wider.
- **Narrow desktop.** The desktop shell gives way to the phone shell below
  768 px, so the desktop panel is never narrower than about 744 px. Below
  700 px it only stacks the detail under the cards. The card browser with a
  "← Programmes" back control lives in the phone sheet.
- **Field navigation** stacks each emblem over its name below 1100 px, so the
  six fields stay on one row.
- **Shared rules.** The queue guard is `editResearchQueue`, the sender is
  `sendResearchQueue` (waiting ids only) and the label is `researchStatus`,
  all in `core/derive/research.ts`. The mobile shell now binds the research
  sender as the deck does.
- **Art.** `scripts/build-research-art.mjs` cuts the six field badges and
  derives any programme masters it finds. It runs as part of
  `npm run assets:derive`, or on its own as `npm run assets:research`.
  Emblems resolve programme → field → field icon (`research-art.ts`). A
  failed request is remembered and never retried.
- **Fixture.** The `research-fixture` launch entry serves the real controller
  and workspace on :4180. `?scenario=late` opens Hulls VIII and Materials VI,
  with a full queue, an unsupplied Academy, a browned-out Academy and a long
  name.
- **Tests.** `npm run test:research-workbench` now also runs
  `test-research-art.mjs`. The phone sheet's checks live in
  `test-progression.mjs`, beside its other mobile fixtures.
