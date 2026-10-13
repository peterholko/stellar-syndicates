# Living Terraces

Status: built Oct 9 2026, uncommitted. It needs no galaxy reset and no
protocol bump.

A Gaia-only building that houses people and grows its own food. Gaia worlds are
rare: 6 in the Oct 8 galaxy, against 23 Terran. The terraces make one a
breadbasket that feeds its own people and exports the surplus.

## Rules

| | |
|---|---|
| Where | Gaia worlds only (refused elsewhere with "Living Terraces grow only on Gaia worlds") |
| Slot | Infrastructure, beside the Habitat, Agroplex, Warehouse and the rest |
| Housing | 20,000 people per level × world size × environment (Gaia 1.5×), in addition to any Habitat |
| Food | 1.2 Provisions/s at level I with a full crew; higher levels follow the standard curve (2.2×, 3.8×, 6×, 8.6×, 11.5×). No Biomass or other input |
| Crew | One per level, like every producer ("nothing produces unstaffed") |
| Cost per level | 30 Alloys, 25 Polymers, 20 Biomass, 10 Machinery; 30 s |
| Levels | I–IV, V–VI with research, the same as other structures |
| Research | None. A Gaia world is the gate |
| Bonuses | Xenobiologists count as skilled crew; Fertile Biosphere ×1.2 output, as for the Agroplex |
| Famine | It keeps working at half rate even when food runs out, like the Agroplex, so a Gaia world can always feed itself back up |

**What a full crew does.** At a medium Gaia world, level I houses 30,000 people
and grows enough for about 37,500. Each level adds 30,000 people, while the food
output grows faster than that, so the surplus widens as it rises. A full
warehouse idles it, as it would any producer.

## Code

**Sim:**
- `StructureKind::LivingTerraces` is added last to `ALL`, because the client
  seeds a planet's first building addresses from that order.
- Recipe: `LIVING_TERRACES_RECIPE`.
- Housing: `colony::TERRACE_POP_PER_TIER` in `Body::population_capacity`.
- Food: a no-input entry in `production::CONVERTERS`.
- The Gaia check is in `World::apply_build`.
- Siting is in `StarSystem::site_for`.
- Tests are in `world/living_terraces.rs`.

**Server:**
- `BuildOptionView.requires_environment` is `"gaia"` for the terraces.
- Bots value them at zero unless their home has a Gaia world.

**Client:**
- `structOption` (`core/derive/market.ts`) shows "Grows only on Gaia worlds."
- Lists that include the terraces: `POOL_OF`, `WORKFORCE_STRUCTURES`,
  `PRODUCTION_INPUTS`, the mobile workforce list, and the description at the end
  of `STRUCTURE_DESCRIPTION`.
- Recipes with no inputs read "No inputs → …".

## Art

The six sprites arrived on Oct 9, together with the Marginal ground patch. They
are in `client/art-src/planet-surface-structures-v2/` and are listed in its
`additions-manifest.json`. The brief that produced them is in
[living-terraces-art-prompt.md](living-terraces-art-prompt.md).

- The planet scene draws them like every other building: a ground layer, then
  the tier's sprite.
- The terraces have no menu icon set, so menus, builder cards and the inventory
  show the same scene sprites (`sceneOnly` in `client/src/icons.ts`).
- **They are enlarged when the game-sized images are made.** The brief asked
  for the 65–70% canvas width named in the original v2 prompts. The approved v2
  set actually fills about 94% of its canvas, with the front foot at about 88%.
  The terraces came out at 69%, with the front foot at 82%, and the Marginal
  ground at x 10–90% against 2–98% for the other grounds.
  - `FRAMING` in `client/scripts/build-planet-surface-structure-art.mjs`
    enlarges the terraces 1.16× about (50%, 38%). That is about 80% wide, with
    the front foot at 89%; level VI's roof stays 3% inside the top edge.
  - It enlarges the Marginal ground 1.2× about (50%, 75.5%), to x 2–98% and
    y 28–93%, the same patch as the others.
  - The masters are untouched, and `test-planet-surface-structure-art.mjs`
    checks both results.
