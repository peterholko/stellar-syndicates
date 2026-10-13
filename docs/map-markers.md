# Galaxy map points of interest

The galaxy map shows two families of points of interest, drawn so they can't be
confused.

## Exploration contacts: cyan sensor circles

`explorationlayer.ts` draws them.

- **Unknown contact.** A signal glyph in a dashed ring. It never reveals the
  hidden kind.
- **Identified site.** A solid ring with the kind's glyph (derelict, station,
  asteroids, anomaly, precursor). A pip shows the site's state:

  | Pip colour | State |
  | --- | --- |
  | Green | Recovery available |
  | Red | Guarded |
  | Amber | New coordinates recovered |
  | Violet | Deeper opportunity |
  | Teal | Restored outpost |

  Depleted sites grey out.
- **Size and labels.** Badges grow from 18 px at the overview to 28 px from 24×.
  Names show from 12×, or whenever a contact is selected or pinned.
- **Panel art.** The painted site illustrations remain in the exploration
  panels.

## Operation contracts: tags in the issuer's colour

`operationlayer.ts` draws them.

| Issuer | Colour |
| --- | --- |
| Authority | Gold |
| Market | Green |
| Survey Office | Teal |
| Salvage Office | Orange |
| Regional Council | Blue |
| Syndicate | Violet |

- **Glyph.** Each operation kind has its own glyph, such as bounty, patrol or
  salvage.
- **Offered or joined.** An offered contract is a hollow tag. A joined one is
  filled, with a progress bar while it runs.
- **Labels.** Tags spell out their kind from 6×, or always once joined.
- **Placement.** A contract that targets a star stands up and to the right of
  it instead of under it, and contracts at one place stack. Only offered and
  running contracts are drawn.

## Interaction

- **Hover.** Both families explain themselves in the hover tip:
  - A contact: name, state, and "click to send" with a fleet selected.
  - A contract: title, issuer, reward, progress, time left.
- **Click.** A contract tag opens its contract card on the Operations route. On
  mobile it opens the Operations sheet. It is never a movement order. Contacts
  keep their behaviour: a click sends the selected fleet, and right-click opens
  the catalogue.

The rules live in `core/derive/map-markers.ts`: sizes, styles, colours, anchors
and hover copy.

## Glyph art

The source is `client/art-src/galaxy-map-glyphs-v1/`: four generated white-on-black
sheets of 3 × 2 cells. `npm run assets:glyphs` (also part of `assets:derive`):

- slices them;
- keys black to transparency and snaps grey fringes;
- trims and fits each glyph to 84% of its square;
- writes `public/art/derived/map-glyphs/v1/{name}-{64,128}.png` and
  `src/map-glyphs.generated.ts`.

The build checks each sheet against its recorded hash. The glyphs are pure
white, and the map tints them. The six issuer emblems are built but not yet
used.

Glyphs that may deserve a regeneration:

- `site-station`: its spokes cross the ring like a reticle.
- `contact-unknown`: reads as a Wi-Fi icon.
- `issuer-regional-council`: reads a little like a hazard sign.

## Not done yet

The first pass covers distinct families, glyphs, state, hover and clickable
contracts. Still to do:

- Clustering when zoomed out.
- Expiry warnings on tags.
- Tethers from assigned fleets.
- A map filter.
- A ping when a new contact's light arrives.
- Light-age tags.

## Tests

`npm run test:map-markers` covers glyph provenance and shape, the rules, the
contract layer, and the wiring; the exploration test covers the contact layer.

`node client/scripts/test-map-markers.mjs --serve` (launch.json
`markers-fixture`, port 4181) draws the real renderer over a canned region with
every kind of contact and contract. `?zoom=N&x=&y=` frames it.
