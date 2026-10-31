STELLAR SYNDICATES — PROMO 09

Twelve scenes, 74 seconds, 16:9.

Slides run for 5–6 seconds, with 8 seconds for orbits and 10 for combat.

The opening and closing slides share the current Stellar Syndicates sign-in
logo: a single-S emblem with metallic lettering. Both use the existing
transparent game asset
client/public/art/derived/login/stellar-syndicates-banner-v1-1440.webp.

PLAY
Open index.html in a desktop browser with WebGL support. Every image,
3D model, renderer dependency, and both video clips are embedded.
It plays without a server or internet connection.

Space: play / pause. Left and right arrows: change chapter.
Full screen: fill the screen. Escape or Close: return to controls.
The timeline scrubs the entire sequence. Playback pauses in hidden tabs.

VIDEO
output/stellar-syndicates-promo-v09-1080p.mp4 is the complete
74-second promo at 1920 x 1080. Orbital motion runs from 0:11 to 0:19,
followed by the planet surface at 0:19. Fleet combat runs from 0:42 to 0:52.
output/stellar-syndicates-fleet-combat.mp4 is the separate clean 16-second
combat clip, without the promo titles.
output/stellar-syndicates-star-system-orbits.mp4 is the separate clean
12-second orbital time-lapse. All videos are silent.

The live player has Export > Record 1080p video. Keep the tab visible
until recording ends, then click Save video to download the result.

POWERPOINT
output/stellar-syndicates-promo-v09.pptx includes the orbital clip
as slide 3 and the developed planet as slide 4. Fleet combat is slide 8.
Both MP4s are embedded, with automatic playback triggers and poster frames.
The slides advance after 8 seconds of orbits and 10 seconds of combat; the
standalone clips keep their original full lengths and playback speeds.
Use Slide Show mode in a video-capable presentation app. A static preview
shows the poster. Captions remain editable, and both logos are separate images.
Other 3D scenes remain rendered stills in PowerPoint. The HTML and full MP4 carry all model animation.

SCENES
01 The frontier       Animated wormhole hub and freighter traffic (6s)
02 A new home         Colony landing (5s)
03 Star systems       Four planets and a moon orbiting Aster, orbital time-lapse (8s)
04 A growing colony   Planet surface with eight Tier II structures (5s)
05 The shipyard       Interceptor construction in Shipyard II (6s)
06 Trade routes       Medium and small freighters (6s)
07 Fleet command      Cruiser with Interceptor escorts (6s)
08 Fleet combat       Recorded mixed-fleet battle (10s)
09 Capital ambitions  Titan inside the capital slipway (6s)
10 The unknown        Precursor relics and pirates (5s)
11 Allies and rivals  Fleet standoff and syndicates (5s)
12 Among the stars    An unexplained ancient ruin and closing title (6s)

The closing scene hints at a larger mystery with existing game artwork and
the line: "Some discoveries change who holds the stars." Its meaning is left
for players to discover.

ORBITAL FOOTAGE
The third scene uses the production SystemViewScene, star effects, and orbital
poses from systemlayout.ts. The camera stays fixed while four planets and a
moon move at 40x orbital time. The promo uses the first 8 seconds
(320 simulation seconds) of the full 12-second source clip.
The scene is labelled TIME-LAPSE. The planet and moon periods keep their
relative speeds. Planet artwork keeps a fixed orientation; only the night-side
shadow turns with the direction of the star. Explicit simulation times make playback and seeking repeatable.
The clip uses the same isolated Aster demo as the original screenshot.
assets/star-system-orbits-poster.png is the 1920 x 1080 poster frame.

SCREENSHOTS
assets/star-system-screenshot.jpg and assets/planet-structures-screenshot.jpg
are browser screenshots at 1280 x 720. The original system still is retained
for reuse; the promo now uses orbital footage. The planet screenshot remains
in the promo, fitted into 1920 x 1080.
They use the production system and planet renderers with isolated demo data.
The planet has a command center, mining complex, smelter, shipyard, habitat,
warehouse, agroplex, and power plant, all at Tier II. This is a demonstration
colony, not a live player session. Surface art, structure sprites, and placement
rules come from the game. No generated replacement artwork is used.

COMBAT FOOTAGE
The clip replays client/public/dev/battles/mixed.json through the unchanged
production battle theater and 3D model renderer. It uses engine-recorded
positions, gunfire, torpedoes, and ship destruction, rounds 45–69 at 1.5
rounds per second. This is a recorded test battle, not a live player session.
The promo uses the first 10 seconds of the full 16-second source clip.
The angled view uses one Tighter framing step from the capture harness.

ASSETS AND EDITING
All artwork and models come from this Stellar Syndicates project.
asset-manifest.json records exact paths and hashes. Original artwork is
unchanged. The system renderer and approach preview keep planet artwork
upright as it orbits; orbital paths and periods are unchanged.

storyboard.mjs holds copy, timing, and composition. player.js holds animation
and playback. template.html holds the player UI. Rebuild from the repo root:
node client/art-src/promo-slideshow-v1/build.mjs

To recapture screenshots, run the client Vite server and open
/art-src/promo-slideshow-v1/world-capture.html?view=system or ?view=planet.
The capture page adapts to the current browser viewport.

To recapture orbits, open /art-src/promo-slideshow-v1/orbits-capture.html.
Wait for the planet artwork to appear, then click Record orbital clip and
Save orbital video. The output is 1920 x 1080. world-demo.js supplies the
shared isolated demo data.

To recapture combat, open /art-src/promo-slideshow-v1/combat-capture.html.
Wait for all seven hull types, click Tighter framing once, then Record combat
clip. The capture harnesses open no socket and send no game commands.

Three.js is included under its MIT license. See THREE-LICENSE.txt.
