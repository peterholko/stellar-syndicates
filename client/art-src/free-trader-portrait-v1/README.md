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

## Four traders (§traders, Oct 27 2026)

Peter: "shouldn't we have like 3 different merchant characters at least, that
way not all the players see the same merchant", then "can we have 4th male
free trader". The server deals one of four per corporation; each needs the
same three portraits. The new prompts are in
[prompts.json](prompts.json), with all twelve masters now present; a paste-ready copy with
what to attach is in [trader-portraits-paste-sheet.md](trader-portraits-paste-sheet.md). They take Teo's portrait as
the series reference and keep his framing numbers, so the one
`TRADER_PORTRAIT_CROP` fits all four traders. Until a trader's neutral
portrait lands, their stall shows the ship.

| Trader | Neutral | Pleased | Scoff |
| --- | --- | --- | --- |
| Ines Varga, the Clean Slate: a cool, exact former customs auditor | `ines-varga.png`: examines a semiconductor | `ines-varga-pleased.png`: a neat "agreed" ring sign | `ines-varga-scoff.png`: looks over her spectacles, palm up |
| Mags Halloran, the Second Wind: a resourceful 32-year-old mechanic and salvage trader | `mags-halloran.png`: displays a rebuilt power coupler | `mags-halloran-pleased.png`: smiles, hand on heart | `mags-halloran-scoff.png`: raises a brow, palm up |
| Rafi Mensah, the Small Fortune: a quick, cheeky young hustler (Peter: "can we have 4th male free trader") | `rafi-mensah.png`: grins with both eyes fully open, rolls a semiconductor across his knuckles | `rafi-mensah-pleased.png`: grins, thumbs-up | `rafi-mensah-scoff.png`: grimaces with eyes fully open, rubs the back of his neck |

Mags runs a small salvage-and-spares operation: she buys surplus ship equipment,
rebuilds useful components and trades them between frontier colonies. The Second
Wind stays flying through ingenious repairs and bartered, mismatched hardware.
Her redesigned masters and prompts are recorded in
[mags-halloran-generation-v2.json](mags-halloran-generation-v2.json); the earlier
senior portraits were retired on Oct 27 2026 (Peter: "make sure the old ones
are removed").
Rafi has both eyes fully open in all three states, with no blinking, winking or
squinting; his generation and corrections are
in [rafi-mensah-generation.json](rafi-mensah-generation.json).

The prompts are in [prompts.json](prompts.json). The neutral portrait uses an
approved captain portrait (`client/public/art/captains/astrid_nystrom_senior.png`)
for style and the free-trader painting
(`../story-events-v1/free-trader-inbound.png`) for the livery. The two
expressions share the neutral portrait's identity, framing, clothing and
setting, with distinct facial expressions and hand gestures. Only the neutral
portraits hold a trade item. Teo's exact generation and revision prompts are in
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
