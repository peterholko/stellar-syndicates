# Complete planet-scene structure integration

The regenerated collection is now used by the real planet scene, not just its
art-review gallery. All 27 live structure kinds are covered: 25 six-tier families,
three Command Center levels and one Service Berth. Ground Shipyard uses its
already promoted art; Orbital Shipyard and Capital Slipway use their newer
floating station designs instead of the earlier ground-terminal concepts.

## Visible behavior

- Every received structure tier selects its corresponding new scene sprite.
  Command Center and Service Berth have their own bounded tier counts.
- Each of the 12 painted planet surfaces supplies its matching shared natural
  ground layer. Moon art selects Barren. Gas giants have no terrain layer.
- The building and ground remain separate alpha images. Source resolution is
  selected through `srcset`; 1254px masters never ship to the game scene.
- Service Berth is a small selectable attachment at the Warehouse's address,
  not another full-sized Warehouse or an extra development plot.
- The two orbital yards remain above the painted horizon. Their industrial
  capacity cost is unchanged, but their cosmetic addresses no longer consume
  the 24 terrain positions. A full art stress fixture can display all 27 kinds:
  24 ground buildings, two orbital facilities, and the Warehouse attachment.
- Orbital Shipyard's six occupied variants still follow this world's received
  ship queue, including queued/paused builds, and clear only with the report.
- New-construction silhouettes use the new art. A queued upgrade, expired ETA
  or unconfirmed order does not advance a built sprite. Rival structures and
  queues remain hidden.
- Existing menu/inventory icons, gameplay rules, prices, research, workforce,
  save formats, information delay, backgrounds and live galaxies are unchanged.
  No connecting roads are introduced.

## Files and pipeline

- `client/art-src/planet-surface-structures-v2/`: 136 promoted building masters,
  12 biome layers, original prompts and source hashes.
- `client/art-src/planet-shipyards-v1/`: existing ground and orbital shipyard
  masters, including occupied variants.
- `client/scripts/build-planet-surface-structure-art.mjs`: reproducible,
  alpha-preserving 128/256/512 WebPs, shipping manifest and typed lookup.
- `client/src/planet-structure-art.ts`: scene-only selection and layering.
- `client/src/shell/deck/planet.ts` and `planet-sites.ts`: received-report
  rendering, parent/attachment anchoring and stable surface/orbital locations.
- `client/src/styles/planet-panel.css`: shared layer alignment, depth haze,
  hit targets and attachment size.

Production does not depend on the `output/` review package or generator temp
files. The normal client build runs the derivative pipeline automatically.

## Verification and preview

From `client`:

```sh
npm run test:surface-structures
npm run test:orbital-shipyards
npm run test:structure-art
npm run test:command-centers
npm run test:planet-surfaces
npm run build
node scripts/test-planet-panel.mjs --serve --surface-structures --port=4331
```

The last command is an isolated UI fixture, with no game-server connection or
simulation. It offers all 12 surfaces plus a gas giant, all six visual tiers,
the full structure collection, queue toggles and real selection controls.
The automated checks cover catalogue completeness, all asset URLs and tiers,
alpha, source/derivative hashes, unchanged icons, caching, report privacy,
attachment placement, every surface and terrain-space separation.

This is a local implementation/build. It does not deploy or restart the server.
