# Trader portraits: paste sheet

Current portrait prompts for Ines Varga, Mags Halloran and Rafi Mensah. All three sets have been generated. Mags is now a 32-year-old salvage-and-spares trader; Rafi keeps both eyes fully open in every expression, with no blinking, winking or squinting.

1. Make each trader's **neutral** portrait first, in a new chat, with its two images attached in the order listed.
2. Make their **pleased** and **scoff** portraits as edits of the approved neutral one: attach that neutral PNG, then paste the prompt.
3. Download each as PNG (square, 1024 × 1024 if offered) and save it in this folder under the file name given.
4. Run `npm run assets:derive` in `client/`, or tell Claude, who will run it and check the face crop.

Framing matters: the eyes about 38% from the top, and the face, hair and hand inside the middle 80%. The game crops every trader's portrait to the same square around the face. If a result is framed too tight or too loose, ask ChatGPT to pull back or move the figure down until the eyes sit at 38% from the top, as Teo's needed.

## `ines-varga.png`

Attach, in this order: `client/public/art/captains/astrid_nystrom_senior.png` (style), then `client/art-src/free-trader-portrait-v1/teo-brannick.png` (series: framing and finish).

```text
Use case: character-portrait
Asset type: one square character portrait for the trading panel of Stellar Syndicates, a persistent science-fiction colony and trading game. It is shown small, 72 to 96 pixels, beside the trader's spoken lines.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: SERIES reference, the approved portrait of Teo Brannick, another of the game's free traders. Match its framing, head size, camera distance, eye height and painterly finish exactly, so the traders read as one set and share one crop. Do not copy his person, clothing, palette or background.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders plus one raised hand; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the face, hair, spectacles and raised hand inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: Ines Varga, an independent free trader in her fifties and a former Authority customs auditor: slim, upright and composed; olive skin with fine lines at the eyes; dark, steady eyes; a sharp silver-grey chin-length bob; thin round steel spectacles; small silver stud earrings. An immaculate high-collared charcoal spacer's coat with a single row of plain steel buttons and slim teal piping, a folded teal pocket square, a slim steel stylus clipped to the lapel. One hand is raised beside the face, holding up a small dark semiconductor module between two fingers and examining it with an auditor's cool interest. Expression: calm and exact, the faintest dry half-smile; she already knows the price.
Background: the Clean Slate's cargo hold, softly out of focus: sealed grey cargo cases in perfectly straight rows, clean white wall panels, a cool teal status light, one warm work lamp far off to one side.
Style/medium: high-end realistic digital painting matching the game's captain portraits and Teo Brannick's portrait; grounded materials (fine wool, brushed steel, glass), believable anatomy and hands. Not a cartoon, not a photograph, not a poster.
Lighting/mood: cool white key light from above and in front, a teal rim on the far side, a small warm accent from the distant lamp; composed, exacting, quietly formidable.
Constraints: absolutely no text, letters, numbers, logos, labels, watermark, borders, frames or interface. No weapons. Exactly one person. Deliver exactly one finished portrait.
Production clarifications: Generate a new, original person. The references are only for painting style, framing and finish. Keep every prop completely blank and unmarked, with no glyphs or pseudo-writing. The raised hand is anatomically correct with five fingers including the thumb. Output a single finished opaque 1024 x 1024 image.
```

## `ines-varga-pleased.png`

Attach: your approved `ines-varga.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: Ines Varga's neutral portrait. Keep the same identity, head size, framing, camera, clothing, silver bob, round steel spectacles, earrings, stylus and pocket square, lighting, colors and background. Change only the facial expression and the hand gesture, with the least movement they need. Do not zoom or reframe.
Expression: a small, satisfied smile, one eyebrow lifted in approval, eyes warmer behind the spectacles: the numbers close.
Gesture: remove the semiconductor; the raised hand makes a neat, precise 'agreed' sign at shoulder height, thumb and forefinger touching in a small ring and the other fingers straight. Reposition the forearm and sleeve naturally and restore whatever background or clothing the old pose covered. One visible hand with five anatomically correct digits, and no duplicate limbs. Keep the whole gesture inside the central 80% and readable at 72 to 96 pixels; do not cover the face.
Output: one opaque 1024 x 1024 square portrait. Realistic painterly digital finish. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
```

## `ines-varga-scoff.png`

Attach: your approved `ines-varga.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: Ines Varga's neutral portrait. Keep the same identity, head size, framing, camera, clothing, silver bob, round steel spectacles, earrings, stylus and pocket square, lighting, colors and background. Change only the facial expression and the hand gesture, with the least movement they need. Do not zoom or reframe.
Expression: unimpressed: the spectacles slid a little down her nose as she looks over them, one brow arched, lips pressed thin.
Gesture: remove the semiconductor; the raised hand is held up flat at shoulder height, palm toward the viewer, a polite but final 'no'. Reposition the forearm and sleeve naturally and restore whatever background or clothing the old pose covered. One visible hand with five anatomically correct digits, and no duplicate limbs. Keep the whole gesture inside the central 80% and readable at 72 to 96 pixels; do not cover the face.
Output: one opaque 1024 x 1024 square portrait. Realistic painterly digital finish. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
```

## `mags-halloran.png`

Attach, in this order: `client/public/art/captains/astrid_nystrom_senior.png` (style), then `client/art-src/free-trader-portrait-v1/teo-brannick.png` (series: framing and finish).

```text
Use case: character-portrait
Asset type: a new original replacement portrait of Mags Halloran for the trading panel of Stellar Syndicates. One opaque 1024 x 1024 square portrait, readable at 72 to 96 pixels.
Input image 1: Astrid, STYLE reference only for the realistic painterly digital portrait finish, believable materials and restrained detail. Do not copy her identity, age, silver hair, uniform or bridge.
Input image 2: Teo, SERIES reference only for camera distance, head size, framing and painterly finish. Do not copy his identity, clothing, colors or background.
Character concept: Mags Halloran, age 32, owner-pilot and practical mechanic of the Second Wind, a small independent salvage-and-spares trading outfit. She buys surplus equipment, repairs good components and trades them between frontier colonies, keeping an aging freighter running through clever improvisation and bartered parts. Resourceful, warm, self-possessed and a little mischievous.
Subject: an attractive adult woman in her early thirties, lean and fit with a naturally pretty face, warm light-olive skin and a light scattering of freckles, expressive hazel-green eyes, softly defined cheekbones, chestnut-auburn hair pulled into a practical loose high bun with a few fine wisps, and small plain gold hoop earrings. Both eyes naturally open. A knowing, friendly half-smile. She looks capable, healthy and put together.
Clothing: a well-fitting moss-green canvas flight jacket in good repair, with plain copper fasteners and restrained brown leather shoulder reinforcement, worn open over a clean rust-orange cotton crew-neck shirt. A small unmarked steel mechanic's probe clipped to a chest pocket. Natural wear at the cuffs, otherwise clean, complete, practical clothing. Bare hands. No knitted cap, shawl, scarves or fingerless gloves.
Gesture: one hand raised beside her face, casually displaying a compact refurbished power coupler between thumb and fingers. It is a palm-sized plain dark cylinder with a brushed-steel collar and one copper contact ring, clearly a spacecraft spare part, completely unmarked. Five anatomically correct digits including the thumb; one visible hand.
Background: the Second Wind's compact working cargo bay, softly out of focus. Mismatched but securely strapped surplus cargo cases; a wall section neatly repaired with a different-colored panel; a small tidy rack of reclaimed ship components; one amber work lamp and a cool blue porthole glow at the far edge. The ship has been assembled and maintained from different generations of hardware. A scrappy, ingenious, functioning small business. Keep background forms broad, simple and quiet behind the face.
Composition: match Teo's pulled-back series framing. Head and shoulders with upper torso and complete raised hand. Three-quarter angle toward the viewer. Generous headroom, crown of the hair around 20% from the top, eyes around 38–40% from the top; head including hair about 35% of the square width. Keep face, hair, earrings, hand and coupler fully inside the central 80%. Strong clear silhouette and facial light-dark separation for the rounded-square UI crop. Do not make a tightly cropped beauty headshot.
Style: high-end realistic painterly digital game portrait, grounded cotton, canvas, metal and skin textures; soft directional warm-neutral key light with a subtle cool-blue rim. No glamour shoot, no heavy makeup or fashion posing. Not a photograph or cartoon.
Constraints: exactly one original adult woman and one visible anatomically correct hand. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons. All props blank. One finished opaque square image.
```

## `mags-halloran-pleased.png`

Attach: your approved `mags-halloran.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: the redesigned Mags Halloran's finished neutral portrait. She is the same attractive 32-year-old mechanic and owner-pilot of the Second Wind. Preserve her exact identity, head size, position, head angle, framing and camera distance, auburn bun, freckles, hazel-green eyes, gold hoops, moss flight jacket, rust shirt, clipped steel probe, lighting, colors, background and realistic painterly digital finish. No zooming, reframing or character redesign. Change only the facial expression and hand gesture, with minimal natural movement.
Expression: A broad, genuine pleased smile, both eyes visibly open and warm, eyebrows relaxed. She has just made a good trade and looks delighted, confident and youthful.
Gesture: Remove the power coupler entirely. Move the same raised hand naturally to rest lightly on her upper chest over the heart in an appreciative 'we have a deal' gesture. Keep five anatomically correct bare digits including the thumb, one visible hand, and no duplicate limbs. Reposition the forearm and sleeve naturally, restoring any exposed clothing and background. Keep the whole hand within the central 80% of the canvas and readable at 72 to 96 pixels. Do not cover the face.
Output exactly one opaque 1024 x 1024 square portrait. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
```

## `mags-halloran-scoff.png`

Attach: your approved `mags-halloran.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: the redesigned Mags Halloran's finished neutral portrait. She is the same attractive 32-year-old mechanic and owner-pilot of the Second Wind. Preserve her exact identity, head size, position, head angle, framing and camera distance, auburn bun, freckles, hazel-green eyes, gold hoops, moss flight jacket, rust shirt, clipped steel probe, lighting, colors, background and realistic painterly digital finish. No zooming, reframing or character redesign. Change only the facial expression and hand gesture, with minimal natural movement.
Expression: A skeptical, slightly incredulous look, one eyebrow raised and lips pressed into a dry crooked line: 'You cannot be serious about that price.' BOTH eyes visibly open, no wink. Keep her attractive youthful identity, restrained humor and self-possession.
Gesture: Remove the power coupler entirely. The same raised hand makes a small palm-up questioning gesture at shoulder height, fingers relaxed and gently separated. One visible anatomically correct bare hand with five digits including the thumb, no duplicate limbs. It reads as a small incredulous shrug. Reposition the forearm and sleeve naturally, restoring any exposed clothing and background. Keep the whole hand within the central 80% of the canvas and readable at 72 to 96 pixels. Do not cover the face.
Output exactly one opaque 1024 x 1024 square portrait. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
Crop-safe placement: keep the entire palm-up gesture between 20% and 42% of the canvas width, with the leftmost fingertip at least 20% from the left edge.
```

## `rafi-mensah.png`

Attach, in this order: `client/public/art/captains/astrid_nystrom_senior.png` (style), then `client/art-src/free-trader-portrait-v1/teo-brannick.png` (series: framing and finish).

```text
Use case: character-portrait
Asset type: one square character portrait for the trading panel of Stellar Syndicates, a persistent science-fiction colony and trading game. It is shown small, 72 to 96 pixels, beside the trader's spoken lines.
Input image 1: STYLE reference only, one of the game's approved captain portraits. Match its realistic painterly digital portrait style, head-and-shoulders framing, soft directional light and restrained detail. Do not copy that person, the navy uniform or the bridge.
Input image 2: SERIES reference, the approved portrait of Teo Brannick, another of the game's free traders. Match its framing, head size, camera distance, eye height and painterly finish exactly, so the traders read as one set and share one crop. Do not copy his person, clothing, palette or background.
Composition/framing: one opaque square portrait, exactly 1024 x 1024 pixels. Head and shoulders plus one raised hand; the face a little above center, turned three-quarters toward the viewer, the eyes about 38% from the top. Keep the face, hair, ear cuff and raised hand inside the central 80% so the portrait survives a rounded-square crop. It must read at 72 pixels: a strong silhouette and clear light-dark separation between the face and an uncluttered background.
Subject: Rafi Mensah, an independent free trader in his early thirties: lean, quick and easy-going; warm dark-brown skin; a bright, gap-toothed grin; close-cropped dark curls with a sharp fade and a neatly trimmed short beard; a single small brass ear cuff. A battered but stylish rust-red flight jacket with a cream shearling collar over a mustard-yellow shirt, a loosened patterned neckerchief, a blank cargo-tag lanyard. One hand is raised beside the face, rolling a small dark semiconductor module across his knuckles like a coin. Expression: a cheeky wide grin with both eyes fully open; a likeable hustler who could sell you your own boots.
Background: the Small Fortune's cramped cargo bay, softly out of focus: mismatched crates stacked to the ceiling, a hanging cargo net, a warm amber work light, a small magenta indicator glow at one edge.
Style/medium: high-end realistic digital painting matching the game's captain portraits and Teo Brannick's portrait; grounded materials (scuffed leather, shearling, cotton, brass), believable anatomy and hands. Not a cartoon, not a photograph, not a poster.
Lighting/mood: warm amber key light from the work lamp, a magenta rim on the far side; quick, playful and full of energy.
Constraints: absolutely no text, letters, numbers, logos, labels, watermark, borders, frames or interface. No weapons. Exactly one person. Deliver exactly one finished portrait.
Production clarifications: Generate a new, original person. The references are only for painting style, framing and finish. Keep every prop completely blank and unmarked, with no glyphs or pseudo-writing. The raised hand is anatomically correct with five fingers including the thumb. Output a single finished opaque 1024 x 1024 image.
Framing priority: use image 2's pulled-back camera distance and generous headroom. Leave approximately 20% of the square as headroom above the crown of his curls; keep the eye line about 38% down the canvas and the head including hair around 35% of the canvas width. Show enough upper torso to match Teo's framing, with the complete raised hand comfortably inside the crop-safe central 80%.
Both eyes must be fully and unmistakably open in every portrait, with both irises clearly visible and relaxed upper and lower eyelids. No blinking, winking, squinting, narrowed eyes or partially closed eyelids. Express the mood through the mouth, eyebrows and hand gesture; keep the eyes fully open.
```

## `rafi-mensah-pleased.png`

Attach: your approved `rafi-mensah.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: Rafi Mensah's neutral portrait. Keep the same identity, head size, framing, camera, clothing, dark curls, short beard, brass ear cuff, neckerchief and cargo tag, lighting, colors and background. Change only the facial expression and the hand gesture, with the least movement they need. Do not zoom or reframe.
Expression: a delighted, open grin, both eyes fully open and bright: a deal.
Gesture: remove the semiconductor; the raised hand gives a confident thumbs-up at shoulder height. Reposition the forearm and sleeve naturally and restore whatever background or clothing the old pose covered. One visible hand with five anatomically correct digits, and no duplicate limbs. Keep the whole gesture inside the central 80% and readable at 72 to 96 pixels; do not cover the face.
Output: one opaque 1024 x 1024 square portrait. Realistic painterly digital finish. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
Both eyes must be fully and unmistakably open in every portrait, with both irises clearly visible and relaxed upper and lower eyelids. No blinking, winking, squinting, narrowed eyes or partially closed eyelids. Express the mood through the mouth, eyebrows and hand gesture; keep the eyes fully open.
```

## `rafi-mensah-scoff.png`

Attach: your approved `rafi-mensah.png`.

```text
Use case: precise-object-edit
Input image 1 is the EDIT TARGET: Rafi Mensah's neutral portrait. Keep the same identity, head size, framing, camera, clothing, dark curls, short beard, brass ear cuff, neckerchief and cargo tag, lighting, colors and background. Change only the facial expression and the hand gesture, with the least movement they need. Do not zoom or reframe.
Expression: a reluctant teeth-baring grimace, skeptical eyebrows and both eyes fully open with relaxed eyelids: no can do. Convey the refusal through his mouth, eyebrows and hand gesture only.
Gesture: remove the semiconductor; the raised hand rubs the back of his neck, elbow up, the hand still visible beside his head. Reposition the forearm and sleeve naturally and restore whatever background or clothing the old pose covered. One visible hand with five anatomically correct digits, and no duplicate limbs. Keep the whole gesture inside the central 80% and readable at 72 to 96 pixels; do not cover the face.
Output: one opaque 1024 x 1024 square portrait. Realistic painterly digital finish. No text, letters, numbers, glyphs, logos, labels, watermark, borders, frames, interface or weapons.
Both eyes must be fully and unmistakably open in every portrait, with both irises clearly visible and relaxed upper and lower eyelids. No blinking, winking, squinting, narrowed eyes or partially closed eyelids. Express the mood through the mouth, eyebrows and hand gesture; keep the eyes fully open.
```
