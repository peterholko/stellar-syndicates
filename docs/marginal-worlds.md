# Marginal worlds

Status: built Oct 9 2026, uncommitted. Client only, with no reset or protocol
change.

## The rule

Only a world colonists can settle (`habitable`) wears the lush terrestrial or
ocean art. Any other terrestrial or ocean world reads as what it is:

| Wire kind | Settleable | Environment | Look | System View label |
| --- | --- | --- | --- | --- |
| terrestrial | yes | Gaia / Terran | terrestrial | Terrestrial world |
| terrestrial | no | Marginal / Hostile | **marginal** | Marginal world · can't be settled |
| terrestrial | no | Uninhabitable | barren | Barren world |
| ocean | yes | Gaia / Terran | ocean | Ocean world |
| ocean | no | any | **marginal_ocean** | Marginal ocean · can't be settled |

Why it matters: the one-garden rule (`clear_habitability_except`) demotes a
home's extra Terran worlds to Marginal. It keeps their kind and geology. Before
this change those worlds still looked like Earth. In the Oct 8 galaxy, 23 of 52
terrestrial and ocean planets were in that state, including Freya III. The
result was 11 systems that looked like they had two or more habitable worlds;
only 2 really do.

`visualKindFor` in `client/src/systemview.ts` applies the rule. It feeds the
System View, the world panel header, battle backdrops and galaxy close-ups.

## Art

- **Marginal world, sprite:** `client/art-src/planets-2026-10-08/marginal-soft-edges-v1.png`.
  - It is a cleanup edit of `drafts/marginal.png`.
  - The prompts are in the `*.prompt.json` files beside them.
- **Marginal world, ground painting:**
  `client/art-src/planet-view/surfaces/marginal-surface-v1.png`, plus the
  `-orbit` version with 240 rows of space added above.
  - It has 24 building sites, all on clean open land.
- **Ground patch under each building:**
  `client/art-src/planet-surface-structures-v2/grounds/marginal.png`, added
  Oct 9 2026. Before then, buildings on Marginal worlds stood without one.
- **Marginal ocean:** this is a stand-in. Its sprite is the ice master, and its
  ground is the ice painting.
  - To give it its own art, change `PLANET_ART_SOURCES.marginal_ocean` in
    `client/scripts/build-planet-derivatives.mjs`.
  - Then add a `marginal_ocean` painting to `build-planet-workbench-art.mjs` and
    `build-planet-sites.mjs`.
  - Finally, drop the ice fallback in `planetSurface`
    (`client/src/planet-surfaces.ts`).

## Rebuilding

```bash
npm --prefix client run assets:planets
npm --prefix client run assets:sites -- --review <dir>
npm --prefix client run assets:derive
```

`assets:derive` regenerates the painting sizes. `npm run test:planet-panel -- --serve --marginal`
opens a Marginal world in the planet fixture.

## Tests

- **Looks and labels:** `test-system-view.mjs`.
- **Grounds:** `test-planet-surfaces.mjs`.
- **Masters and the ice stand-in:** `test-planet-art.mjs`.
- **Sites and paintings:** `test-planet-panel.mjs`.
- **Texture preload:** `test-battle-backdrop.mjs`.
