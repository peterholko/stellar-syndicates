# Pirate family portraits

Peter (Oct 25 2026): the charter's first raider gets a personality. At the end
of the tutorial, a vengeful relative comes for the player. Each new player gets
a random pirate from about five portraits, with random names.

- **The cast:** five pirate families, each with two portraits:
  - **nephew:** the step-5 raider, killed at step 10;
  - **uncle:** the finale.
  - A brother can come later, made the same way as the uncles.
- **Names** are drawn by the game, never painted. A family surname is shared by
  the nephew, uncle and brother.
- **Order:** generate the five nephews first. Each uncle prompt uses its
  nephew's finished portrait for family resemblance.

| Family | Accent | Nephew | Uncle | Family trait |
| --- | --- | --- | --- | --- |
| Showboat | rust orange | vain performer | theatrical warlord | gold canine, slicked hair |
| Hothead | mustard yellow | reckless brawler | silent enforcer | burn scars, pale blue eyes |
| Collector | oxblood red | polite, cold | the family's accountant | round wire spectacles, oxblood gloves |
| Scrapper | verdigris green | grinning scavenger | the junk king | amber cybernetic eye, green copper |
| Fatalist | bone white | calm, unafraid | the elder | line tattoos, bolt beads |

## Delivery

- **Save as:** `client/art-src/pirate-portraits-v1/<slug>.png`.
  - Square, 1024 × 1024 asked, at least 512.
  - Slugs: `showboat`, `showboat-uncle`, `hothead`, `hothead-uncle`,
    `collector`, `collector-uncle`, `scrapper`, `scrapper-uncle`, `fatalist`,
    `fatalist-uncle`.
- **Attach for every portrait:** the style reference,
  `client/public/art/captains/astrid_nystrom_senior.png`.
- **Also attach for the nephews:** the pirate livery reference,
  `client/art-src/story-events-v1/pirate-raid.png`.
- **Also attach for the uncles:** the matching nephew's portrait.
- **Framing** matches Teo Brannick's portraits, so the same face crop works:
  `TRADER_PORTRAIT_CROP` is 75% of the frame, from 14.3% left and 11.3% down.

## Nephews

### showboat

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the pirate who raids a new player's colony, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: PIRATE LIVERY reference, the game's painting of this pirate band raiding a colony: charcoal armored boarding suits with muted rust-orange and mustard-yellow accents, and a raider hull of rugged dark gunmetal with rust-orange and mustard-yellow patch plates. Use these materials and colors; the ship, the colonists and the scene do not appear.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: a pirate raider in his mid-twenties, his family's showboat: handsome, vain, always performing for an audience. Olive skin, a sharp jaw, glossy black hair slicked back with shaved sides, a thin moustache, one gold-capped canine tooth. Over a charcoal armored boarding suit he wears a stolen colonial officer's jacket with its insignia torn off, and a rust-orange silk scarf at the throat. Expression: a wide, cocky grin that shows the gold tooth, one eyebrow raised, chin lifted, as if the raided colony is his stage.
Background: the cramped cockpit of his raider, softly out of focus: dark canopy glass with a sliver of stars, scuffed gunmetal panels with rust-orange patch plates, a few red and amber instrument glows.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (scuffed armor, worn leather, silk, grime), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: warm amber key light from the instruments, a cool blue starlight rim; charming, smug and dangerous.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### hothead

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the pirate who raids a new player's colony, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: PIRATE LIVERY reference, the game's painting of this pirate band raiding a colony: charcoal armored boarding suits with muted rust-orange and mustard-yellow accents, and a raider hull of rugged dark gunmetal with rust-orange and mustard-yellow patch plates. Use these materials and colors; the ship, the colonists and the scene do not appear.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: a pirate raider in his early twenties, his family's hothead: wiry, restless and reckless. Pale freckled skin, a shaved head, pale blue eyes, a fresh burn scar across the left cheek and ear. A battered charcoal boarding suit with mustard-yellow hazard stripes; a scuffed helmet with a cracked visor pushed up onto the forehead. Expression: a snarl with teeth bared, leaning toward the viewer as if shouting over the comms.
Background: the cramped cockpit of his raider, softly out of focus: dark canopy glass with a sliver of stars, scuffed gunmetal panels with mustard-yellow patch plates, a few red and amber instrument glows.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (scuffed armor, cracked visor glass, scar tissue, grime), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: harsh red alarm light from one side, mustard-yellow instrument glow from below; volatile and aggressive.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### collector

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the pirate who raids a new player's colony, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: PIRATE LIVERY reference, the game's painting of this pirate band raiding a colony: charcoal armored boarding suits with muted rust-orange and mustard-yellow accents, and a raider hull of rugged dark gunmetal with rust-orange and mustard-yellow patch plates. Use these materials and colors; the ship, the colonists and the scene do not appear.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders plus one raised hand; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head and the hand inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: a pirate raider in his early thirties, his family's collector: calm, courteous and cold, cataloguing everything he takes. Light brown skin, close-cropped black hair, a neatly trimmed short beard, thin round wire spectacles. An immaculate high-collared charcoal flight jacket over light armor, and oxblood-red leather gloves; one gloved hand raised near the chin, fingertips pressed together. Expression: a polite, chilling half-smile, eyes weighing the viewer like an item in a ledger.
Background: the cramped cockpit of his raider, softly out of focus: dark canopy glass with a sliver of stars, neat gunmetal panels with oxblood-red trim, a few cool blue screen glows.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (pressed fabric, fine leather, wire, polished armor), believable anatomy and hands. Not a cartoon, not a photograph, not a poster.
Lighting/mood: cool blue-white screen light on the face, a soft red rim; quiet, precise and menacing.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. One visible hand with five anatomically correct fingers. Exactly one person. Deliver exactly one finished portrait.
```

### scrapper

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the pirate who raids a new player's colony, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: PIRATE LIVERY reference, the game's painting of this pirate band raiding a colony: charcoal armored boarding suits with muted rust-orange and mustard-yellow accents, and a raider hull of rugged dark gunmetal with rust-orange and mustard-yellow patch plates. Use these materials and colors; the ship, the colonists and the scene do not appear.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: a pirate raider in his late twenties, his family's scrapper: a scavenger who strips wrecks and colonies alike, grinning and a little unhinged. Dark brown skin, a lean face smeared with grease, shaggy sun-bleached locs tied up on top, a crude cybernetic implant in place of the left eye with a glowing amber lens. Mismatched charcoal armor patched with welded salvage plates, and a verdigris-green copper collar. Expression: a manic, gap-toothed grin.
Background: the cramped cockpit of his raider, softly out of focus: dark canopy glass with a sliver of stars, gunmetal panels patched with salvage and green-patinated copper, loose cables, a few amber instrument glows.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (welded steel, oxidized copper, grease, worn fabric), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: amber glow from the implant and the instruments, a cold blue rim; gleeful and unpredictable.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### fatalist

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the pirate who raids a new player's colony, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: PIRATE LIVERY reference, the game's painting of this pirate band raiding a colony: charcoal armored boarding suits with muted rust-orange and mustard-yellow accents, and a raider hull of rugged dark gunmetal with rust-orange and mustard-yellow patch plates. Use these materials and colors; the ship, the colonists and the scene do not appear.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: a pirate raider in his mid-thirties, his family's fatalist: gaunt, quiet and afraid of nothing, sure the void decides who lives. Warm tan skin, hollow cheeks, long black hair tied back, deep-set dark eyes; thin geometric line tattoos across the brow and one cheek, abstract parallel lines and dots only, no symbols or letters. Bone-white armor plates over a charcoal suit; a string of polished machine bolts worn like beads around the neck. Expression: solemn, unblinking, unsettlingly calm.
Background: the cramped cockpit of his raider, softly out of focus: dark canopy glass with a sliver of stars, gunmetal panels with bone-white patch plates, one dim red instrument glow.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (chipped ceramic armor, polished steel bolts, weathered skin), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: a single cold white key light from above, deep shadows; eerie, fatal calm.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, religious symbols, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

## Uncles

### showboat-uncle

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the head of a pirate family who comes for revenge after the player kills his nephew, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: FAMILY reference, the portrait of his nephew. The uncle is a different, older man of the same family: keep the olive skin, the sharp jaw, the slicked-back hair with shaved sides and the gold-capped canine, and the family's rust-orange accent. Do not copy the nephew's face, age, clothing or pose.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: the family's patriarch, in his sixties: tall, heavy-set and theatrical, a warlord who stages his raids like performances. Silver hair slicked back with shaved sides, a trimmed silver moustache, the family's gold-capped canine. A long charcoal greatcoat over heavy armor, lined in rust-orange silk, with a crooked row of looted military medals on the chest, plain metal with no writing. Expression: a slow, cold smile that does not reach the eyes; amused and merciless.
Background: the bridge of his warship, larger and darker than a raider's cockpit, softly out of focus: heavy gunmetal bulkheads with rust-orange trim, trophies of past raids (a dented hull plate, a cracked colony beacon), dim red battle lighting.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (heavy wool, silk lining, tarnished medals, scarred armor), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: a warm amber key light, deep red shadows from the battle lighting, a thin blue rim; grand, cold and lethal.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### hothead-uncle

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the head of a pirate family who comes for revenge after the player kills his nephew, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: FAMILY reference, the portrait of his nephew. The uncle is a different, older man of the same family: keep the pale freckled skin, the pale blue eyes, the shaved head and the burn scars, and the family's mustard-yellow accent. Do not copy the nephew's face, age, clothing or pose.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: the family's enforcer and chief, in his fifties: massive, slab-shouldered, a brawler who has never lost a boarding fight. A shaved head with old burn scars covering half the scalp, one ear missing, a heavy brow, a grey stubble beard. Heavy charcoal assault armor, dented and scorched, with worn mustard-yellow hazard stripes. Expression: a fixed, contained glare of fury, jaw clenched, utterly still.
Background: the bridge of his warship, larger and darker than a raider's cockpit, softly out of focus: heavy gunmetal bulkheads with mustard-yellow hazard trim, trophies of past raids (a dented hull plate, a cracked colony beacon), dim red battle lighting.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (scorched armor plate, scar tissue, stubble), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: hard red light from below, a mustard-yellow side glow, deep shadow over one eye; a silent threat.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### collector-uncle

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the head of a pirate family who comes for revenge after the player kills his nephew, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: FAMILY reference, the portrait of his nephew. The uncle is a different, older man of the same family: keep the light brown skin, the round wire spectacles and the oxblood-red gloves, and the family's oxblood-red accent. Do not copy the nephew's face, age, clothing or pose.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders plus one raised hand; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head and the hand inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: the family's head, in his sixties: lean, immaculate and patient, who runs the family like a ledger. Close-cropped white hair and a neat white beard; the family's round wire spectacles, one lens with a hairline crack. A long, perfectly pressed high-collared charcoal coat; one oxblood-gloved hand raised, slowly removing the spectacles. Expression: patient, disappointed and utterly ruthless; a man settling an account.
Background: the bridge of his warship, larger and darker than a raider's cockpit, softly out of focus: orderly gunmetal bulkheads with oxblood-red trim, trophies of past raids arranged in neat rows, dim red battle lighting.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (pressed wool, fine leather, wire, polished armor), believable anatomy and hands. Not a cartoon, not a photograph, not a poster.
Lighting/mood: cool white screen light on the face, a deep red rim; quiet, exact and lethal.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. One visible hand with five anatomically correct fingers. Exactly one person. Deliver exactly one finished portrait.
```

### scrapper-uncle

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the head of a pirate family who comes for revenge after the player kills his nephew, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: FAMILY reference, the portrait of his nephew. The uncle is a different, older man of the same family: keep the dark brown skin, the locs, the amber cybernetic eye and the green-patinated copper, and the family's verdigris-green accent. Do not copy the nephew's face, age, clothing or pose.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: the family's junk king, in his sixties: heavyset and heavily augmented, half man, half salvage. Wild grey locs; both eyes replaced with mismatched amber cybernetic lenses; a riveted copper plate along the jaw. Armor built from salvaged hull plates gone verdigris green, with cables and old components wired into it. Expression: a wolfish, delighted grin, a scavenger who has found something worth taking apart.
Background: the bridge of his warship, larger and darker than a raider's cockpit, softly out of focus: bulkheads patched with salvage and green-patinated copper, hanging cables, trophies of past raids (a dented hull plate, a cracked colony beacon), dim red battle lighting.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (oxidized copper, rivets, welded steel, worn skin), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: amber glow from the lenses, red battle light, a cold blue rim; greedy and gleeful.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```

### fatalist-uncle

```
Use case: character-portrait
Asset type: one square character portrait for Stellar Syndicates, a persistent science-fiction colony and trading game. He is the head of a pirate family who comes for revenge after the player kills his nephew, shown small (72 to 112 pixels) beside his transmissions.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: FAMILY reference, the portrait of his nephew. The uncle is a different, older man of the same family: keep the warm tan skin, the deep-set eyes, the line tattoos and the bolt beads, and the family's bone-white accent. Do not copy the nephew's face, age, clothing or pose.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the whole head inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: the family's elder, in his sixties: tall and gaunt, the calm voice the family obeys. Long white hair tied back; the family's line tattoos, faded with age, across the brow and cheek, abstract lines and dots only; deep-set dark eyes. Bone-white armor plates scored by old impacts over a long charcoal coat; several strands of polished machine bolts around the neck. Expression: serene and merciless, as if the viewer's end has already been decided.
Background: the bridge of his warship, larger and darker than a raider's cockpit, softly out of focus: bare gunmetal bulkheads with bone-white plates, trophies of past raids (a dented hull plate, a cracked colony beacon), one dim red light.
Style/medium: high-end realistic digital painting matching the captain portraits; grounded materials (chipped ceramic armor, polished bolts, weathered skin), believable anatomy. Not a cartoon, not a photograph, not a poster.
Lighting/mood: a single cold white key light from above, deep shadows, a faint red rim; still and final.
Constraints: absolutely no text, letters, numbers, logos, insignia with writing, religious symbols, labels, watermark, borders, frames or interface. No weapons in hand, no gore. Exactly one person. Deliver exactly one finished portrait.
```
