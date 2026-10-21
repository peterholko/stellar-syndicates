# Free trader portrait — version 1

Teo Brannick, master of the Long Odds: the free trader's face in the trading
panel (`client/src/core/derive/trader.ts`), chosen by Peter on 2026-10-16. A
big, loud, theatrical haggler; his lines live in `trader.ts`, his prices are
the sim's.

| Image | Master | Shown |
| --- | --- | --- |
| Neutral | `teo-brannick.png` | Greetings and ordinary offers; holds up a semiconductor. |
| Pleased | `teo-brannick-pleased.png` | A deal struck; laughing with a welcoming open palm. |
| Scoff | `teo-brannick-scoff.png` | A refusal; skeptically strokes a beard braid. |

The prompts are in [prompts.json](prompts.json). The neutral portrait uses an
approved captain portrait (`client/public/art/captains/astrid_nystrom_senior.png`)
for style and the free-trader painting
(`../story-events-v1/free-trader-inbound.png`) for the livery. The two
expressions share the neutral portrait's identity, framing, clothing and
setting, with distinct facial expressions and hand gestures. Only the neutral
portrait holds a semiconductor. Exact generation and revision prompts are in
[generation.json](generation.json).

## Delivery

Save each master here under its slug, square (1024 × 1024 asked; at least
512), then run `npm run assets:derive`. `buildTraderPortraitArt` (in
`client/scripts/build-founding-story-art.mjs`) writes a 256-pixel WebP to
`client/public/art/derived/free-trader/v1/`, cropped to the same square
around the face from all three masters (`TRADER_PORTRAIT_CROP`: 75% of the
frame, from 14.3% left and 11.3% down) so the face reads at 72–112 px and a
change of mood never jumps. Until the neutral portrait lands
the panel shows the Long Odds; a missing expression falls back to the neutral
portrait.
