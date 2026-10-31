# Pirate family portraits

Ten opaque, square PNG masters generated on October 25, 2026 with the built-in
image generator from [the portrait prompts](../../../docs/pirate-portraits.md).
Each master is 1254 × 1254 pixels, above the brief's 512-pixel minimum.

Open [preview.html](preview.html) to compare each nephew with his uncle and
inspect the shared face crop at 72 or 112 pixels.

| Family | Nephew | Uncle |
| --- | --- | --- |
| Showboat | [showboat.png](showboat.png) | [showboat-uncle.png](showboat-uncle.png) |
| Hothead | [hothead.png](hothead.png) | [hothead-uncle.png](hothead-uncle.png) |
| Collector | [collector.png](collector.png) | [collector-uncle.png](collector-uncle.png) |
| Scrapper | [scrapper.png](scrapper.png) | [scrapper-uncle.png](scrapper-uncle.png) |
| Fatalist | [fatalist.png](fatalist.png) | [fatalist-uncle.png](fatalist-uncle.png) |

All portraits use `client/public/art/captains/astrid_nystrom_senior.png` as
the house style reference. The nephews also use
`client/art-src/story-events-v1/pirate-raid.png` for pirate livery; each uncle
uses his family's finished nephew for resemblance. Names are supplied by the
game and are absent from the artwork.

The Collector's initial heavy armor was corrected to his clean charcoal
jacket before generating his uncle. The initial version is preserved at
`revisions/collector-v1-armored.png`.

[generation.json](generation.json) records the original prompts, exact
generation and correction prompts, reference hashes, output filenames, and
master hashes. The preview applies `TRADER_PORTRAIT_CROP` in CSS without
modifying the masters: `{ left: 0.143, top: 0.113, size: 0.75 }`.

## Expression variants

Each nephew and uncle has three additional portraits in `expressions/`:

| Suffix | Dialogue use |
| --- | --- |
| `-smug.png` | Confident first encounter or gloating |
| `-angry.png` | Revenge or an escalating confrontation |
| `-shocked.png` | A battle reversal or loss of control |

For example, the Showboat uncle's revenge portrait is
`expressions/showboat-uncle-angry.png`. All 30 edits start from their own
original master and retain the character's clothing, setting, age, and framing.
The Collector and Fatalist show restrained anger; the Hothead and Showboat are
more overt. Cybernetic eyes, spectacle gestures, scars, and tattoos stay with
their characters.

[Compare expressions side by side](expressions/preview.html), or use the
expression buttons in the [family gallery](preview.html).
[The expression generation record](expressions/generation.json) includes every
exact prompt, reference hash, and final file hash. The original portraits above
are preserved.

These are source art assets. Runtime portrait derivatives and game integration
are not included in this delivery.
