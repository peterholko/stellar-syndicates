# Planetary Command Centers

Implemented as one structure family (`command_center`) with three levels on each inhabited world. It belongs to the player's corporation, not the Terran Charter Authority. The approved cream/graphite/amber/cyan art has no Authority branding.

| Level | Name | Cost to order this level | Base construction | Local bonus |
| --- | --- | --- | --- | --- |
| I | Landing Command | Free on initial settlement or a Colony Ship landing; rebuilding costs 15 Alloys + 5 Machinery | 3 minutes if constructed | None |
| II | Colony Command Center | 40 Alloys + 12 Machinery + 8 Electronics | 5 minutes | +1 Infrastructure slot; 10% shorter structure construction |
| III | Syndicate Headquarters | 120 Alloys + 40 Machinery + 30 Electronics + 20 Polymers | 15 minutes | +2 Infrastructure slots; 20% shorter structure construction, total rather than additive |

Headquarters III requires **Prefab Construction** (`mat_prefab_construction`). Levels I–II do not require research. A solid-surface world with at least 1,000 residents is required to construct or upgrade the building. It takes no ordinary development slot and requires no posted workforce. The three levels upgrade the same footprint, not three separate buildings. Multiple populated worlds may each have one; the name Headquarters does not impose a corporation-wide singleton.

The Level I kit is automatic, not an extra compulsory tutorial task. The settlement confirmation names it. A planet's structure panel offers an upgrade-review shortcut; desktop and mobile builders use the exact next-level costs and research gate, including already queued upgrades. Level III is the maximum. The built sprite changes only when a report containing the completed upgrade arrives.

## Timing and information

Local construction duration is `recipe ticks × planet environment × local command multiplier × research multiplier`, with the existing tick-rounding rules. Prefab Construction's existing 0.75 multiplier stacks with the local multiplier. The table gives base times, not a promise about a specific planet or its queue. For example, Headquarters III on a neutral planet with Level II and Prefab Construction takes approximately 10 minutes 8 seconds of active construction.

Bonuses apply to newly received ordinary structure orders. Work already queued keeps its quoted duration. Ships, module manufacture, training, research and the short Founding Camp projects are unchanged. There is still one FIFO structure-construction queue per planet. The building never creates or refills materials, fuel, population or workforce.

The existing `Corporation.command_center` remains the communications-origin coordinate. This building does not move it, accelerate signals, bypass command travel, or expose current remote truth. Environment/local/research construction modifiers are presented together from the arrived site report. Rival views omit administration, slots and modified build-time data.

## Persistence and bots

- New settlement and Colony Ship landing paths deploy Level I automatically.
- Old saves receive a one-time Level I grant on populated solid worlds; higher levels and resources are preserved.
- Historical reports receive only the same baseline grant appropriate to their own recorded population, with no timestamp change or import of current-world data.
- Subsequent reloads do not rebuild a missing or lost Command Center.
- Bots can select upgrades through their continuing development goals and use the correct level-specific material basket and research cap. No full balance simulation was run for this change.

## Art and checks

Masters and built-in image-generation prompts: `client/art-src/planet-surface-structures-v1/command-center/`.
Generated shipping variants: `client/public/art/derived/command-center/v1/`.

Targeted checks: `cargo test --workspace command_centers`, `cargo test -p server bots_price_command`, and `cd client && npm run test:command-centers`.
The planet-panel fixture includes a **Next Command level** control for isolated visual QA: `cd client && node scripts/test-planet-panel.mjs --serve --habitable`.

No live galaxy reset, server restart, deployment or commit is part of this implementation.
