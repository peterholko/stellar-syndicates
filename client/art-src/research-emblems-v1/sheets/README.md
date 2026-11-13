# Delivered research emblem sheets

All 21 sheets from `docs/research-emblem-sheets.md` were generated with the
built-in image generator on 2026-11-03. They cover the 119 programme emblems
in `sheets.json`. The two Veil emblems remain single masters in the parent
directory, bringing the catalogue to 121 unique programme emblems.

Each PNG retains the generator's original bytes and alpha. Each accompanying
`.prompt.json` records the exact prompt, attached references and their hashes,
any revision prompts, and the validated badge bounds. Six-badge sheets use
1536 × 1024 canvases; smaller sheets retain the generator's native dimensions.
The slicer detects the clear gaps and frames every badge to the same 88% fill.

Life 01 and Computation 02 were revised to increase their badge size before
delivery. Flak Doctrine uses the single master
`../weap_cm_iii_flak_doctrine.png`, which corrects the direction of its incoming
missiles and takes precedence over its Weapons 02 cell.

`sheets.json` is unchanged from the approved plan. Keep its cell assignments
when replacing a sheet or adding a single-master correction.

From `client/`, run `npm run assets:research` to rebuild the 64, 128, 256 and
512 px programme derivatives, then `npm run test:research-workbench` to check
catalogue coverage, unlock art, fallbacks and slicing.
