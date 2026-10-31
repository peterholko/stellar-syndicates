# Battle asteroids — version 1

The rocks behind a pirate den's battles (`buildBeltBackdrop` in
`client/src/battletheater.ts`), briefed in
[docs/battle-asteroid-prompts.md](../../../docs/battle-asteroid-prompts.md).
One sheet per belt type, generated 2026-10-29 with the built-in image_gen tool
at 1536 × 1024 on a transparent background and kept as delivered: six single
asteroids in a 3 × 2 grid of 512 px cells (long, round, wedge, two-lobed, slab,
compact), seen from straight above so any rotation reads the same.

| Belt | Sheet |
| --- | --- |
| Stony | `stony-sheet.png` |
| Metallic | `metallic-sheet.png` |
| Icy | `icy-sheet.png` |
| Shattered | `shattered-sheet.png` |

`prompts.json` holds each sheet's prompt and references;
`image-validation.json` its SHA-256, alpha range and each rock's area and
bounds.

`buildBattleAsteroids` (in `client/scripts/build-art-derivatives.mjs`) cuts
each cell to its largest silhouette at alpha ≥ 8, which drops the generator's
faint matte haze and a few stray pixels, pads it 4 px, centres it in a square
it fills and writes a 256 px WebP to
`client/public/art/derived/battle-asteroids/v1/<belt>-<1–6>-256.webp`.
