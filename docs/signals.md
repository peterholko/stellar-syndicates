# Signals

Status: built Oct 20 2026, uncommitted. Protocol v55
(`stellar.msgpack.v55`): deploy the client and server together. No galaxy
reset needed; every new saved field is `#[serde(default)]`.

Travian's hero adventures, for Stellar Syndicates. The Command Center keeps
picking up faint, private contacts near home. Any captained fleet can answer
one: it flies out, sweeps the spot for 30 s and turns for home. The report
reaches command at light speed with what it found. Peter approved the design
and the v2 mockup (Oct 19 2026): a wide pop-out with one painting per signal
type. Auto-recall on a raid warning, a Send warning ("your last warship") and
the fuel cost on the Send rows are deferred at his request.

Code: `crates/sim/src/signals.rs` (types, curve, first ten, reward tables),
`crates/sim/src/world/signals.rs` (spawn, send, sweep, report),
`client/src/core/derive/signals.ts`, `client/src/shell/deck/signals.ts`,
`client/src/shell/mobile/signals.ts`, `client/src/signalmarkers.ts`.

## When they appear

- **Opening:** with "Export your ore" (charter step 12, the step after the
  den), or at once for a corporation outside the charter. The first two
  arrive together (`SIGNAL_OPENING`).
- **Curve:** Travian's x1 adventure rate, by days since a corporation's
  signals opened (`signal_interval_s`): every 8 h for 3 days, every 12 h to
  day 16, every 16 h to day 63, then daily.
- **Cap:** 5 open (`SIGNAL_OPEN_MAX`). At the cap the clock waits; the next
  appears as soon as one is answered. Signals never expire.
- **Where:** the first ten within 8–14k su of home, later ones within
  10–25k su; at least 3,000 su from any star, 5,000 from the Hub and 2,500
  from exploration sites and the corporation's other signals. Owner-only:
  rivals never see them, though they can see the fleet that answers.
- **Ids:** `SIGNAL_ID_BASE` + a galaxy-wide counter, apart from the entity
  counter, so spawning never shifts another entity's id.

## Answering one

- **Who:** any fleet with a captain aboard (`AnswerSignal`), except a Mining
  Belt Station. A captain hurt on a signal can't lead another until
  recovered. A captain in reserve needs a ship first ("Give a ship in
  Officers").
- **The flight:** an `Expedition` order with the `signal` task, light-delayed
  like any order (a fleet docked at home gets it at once). Expedition safety
  applies: a civilian hull breaks off from threats.
- **The sweep:** 30 s on station (`SIGNAL_SWEEP_S`), then the fleet turns for
  home on its own. If another fleet swept it first, it just goes home.
- **Aboard:** a module crate rides in any hull. Goods go aboard if the hold
  has room; the rest stay where the signal was, as a private, identified
  cache for a Freighter (an exploration site, `founding_owner`). Anything
  aboard lands at home with the fleet (`DeliverToSystem`).
- **The report:** reaches command at light speed from the signal. Credits,
  standing, dossiers, blueprints, captains, survivors, experience and injury
  are all paid when it arrives.

## Hazard

Each signal is Routine or Hazardous, shown before anyone is sent. A Hazardous
one pays double experience (an exotic flare four times) and may hurt the
captain: 45% less 15 points per Fieldcraft point, floor 5% (`hurt_chance`;
Fieldcraft 1 → 30%). A hurt captain still commands the fleet but sits signals
out for 4 h (`SIGNAL_HURT_S`, `Captain.hurt_until`). The roll is keyed by the
signal, so it's fixed when the signal appears.

## The first ten (they replaced the four first finds)

The same for every corporation, by number of appearance:

| # | Signal | Holds |
| --- | --- | --- |
| 1 | Silent probe | 1,500 Cr |
| 2 | Distress beacon | Captain Daichi Mori (Fieldcraft, level 2), in reserve at home |
| 3 | Drifting wreck | Recon Suite blueprint (else Extended Tanks, Cargo Pods; else 1,500 Cr) |
| 4 | Ore fragment | The prospector's cache: 30 Titanium Ore, 2 Rare-metal Ore |
| 5 | Sensor ghost | A research dossier (15%) |
| 6 | Drifting wreck | An Extended Tanks crate |
| 7 | Authority courier | 600 Cr and 2 standing |
| 8 | Distress beacon | A Geologist, flying home in the survivors' own craft |
| 9 | Cargo pod | 15 Semiconductors, 20 Polymers |
| 10 | Sensor ghost, Hazardous | Experience only: it teaches the hazard with nothing to lose |

Experience: 45 a sweep, 90 Hazardous. The strip above the cards shows "First
ten · 4/10 · next: a research dossier" until all ten reports are in.

## After the first ten

A random kind by the world's age (`draw_signal`), then what it holds by tier
(world day 7 → tier 2, day 21 → tier 3; amounts ×1, ×1.6, ×2.5):

| Signal | From | Hazard | May hold |
| --- | --- | --- | --- |
| Drifting wreck | day 0 | Routine | Alloys and Machinery, or a module crate |
| Cargo pod | day 0 | Routine | Polymers, Provisions or Semiconductors |
| Ore fragment | day 0 | Routine | Cuprite or Titanium Ore; Rare-metal Ore at tier 3 |
| Silent probe | day 0 | Routine | Survey Office credits |
| Distress beacon | day 0 | Routine, 25% Hazardous | A specialist, credits; rarely a captain when Hazardous |
| Sensor ghost | day 0 | Routine | Often nothing; sometimes a dossier |
| Authority courier | day 7 | Routine | Credits and 2 standing |
| Abandoned outpost | day 7 | Hazardous | A dossier, Semiconductors and Machinery; rarely a blueprint |
| Pirate stash | day 7 | Hazardous | Credits and Armaments, or a weapon crate |
| Ghost ship | day 7 | Hazardous | A Point Defense or Reflective Plating crate; rarely a blueprint |
| Precursor fragment | day 21 | Hazardous | A large dossier, Precision Components; rarely a Survey Drive, Nebula Spectrometer or Prismatic Lance blueprint |
| Exotic flare | day 21 | Hazardous | Exotic Matter or a Void crystal; four times the experience |
| Teo's lost crate (named for the founder's own trader since Oct 27 2026: "Ines's lost crate", "Mags's lost crate") | any | Routine | Semiconductors or Deuterium, and a finder's fee |

A dossier goes to the active programme, else the queue, else the first Tier
I left; a blueprint to the first of its list not yet licensed.

## Client

- **Deck:** a "Signals · N" chip in the top bar (accent; "· report in" when
  one waits) opens a wide pop-out (`signals` route). Cards on the left with
  their paintings, tag (Routine, Hazardous, Report in), distance and age, the
  one being answered with its ETA or sweep progress, and a dashed "Next
  signal" slot. The chosen signal on the right: the full painting, "May hold"
  chips, "If hurt: captain out 4h", and a Send row per captain (portrait,
  ship · captain, level, Fieldcraft, where; injury odds and trip time, or why
  not). A report lists what was found, with Select fleet, Show the cache and
  Dismiss. Also reachable from map markers, toasts and a line on Operations.
- **Mobile:** a Signals sheet: the cards stacked with their paintings as
  banners; a tap opens the signal in place. Map taps, notices and a line on
  Operations lead there.
- **Map:** open signals are small diamonds (amber when Hazardous), not
  another sensor circle; named from the contact label zoom.
- **Officers:** "· Hurt · 3h 58m" after a hurt captain's status.
- **Art:** `client/art-src/signals-v1/` (13 masters) → `npm run
  assets:derive` (`build-signal-art.mjs`) → full frames at 768/1280 and
  card crops (the central 62%, 16:9) at 384/640 in
  `client/public/art/derived/signals/v1/`. Cards show the type's line icon
  until a painting loads.
- **Preview:** `/scripts/signals-fixture.html` on the vite dev server.
- **News:** "New signal · Ghost ship · Hazardous." and "First find 3/10 ·
  Drifting wreck · Recon Suite blueprint · +45 XP." or "Report in · …",
  toasted.

## Not built

- Auto-recall on a raid warning, the Send warning and the fuel display
  (deferred by Peter).
- Bots don't answer signals; theirs sit at the cap.
- A Silent probe that charts a nearby system (it pays credits instead), a
  pirate stash's hideout intel, and Teo's lost crate's link to Teo.
- Contract-board "Answer 2 signals" contracts.

## Tests

- Sim (`world::signals`): opening after the den, two at once, the curve and
  the cap; a sweep paid on the report's arrival with the fleet turning home;
  a crate landing at home; goods left as a cache; a Hazardous injury and the
  refusal while hurt; random signals fixed at appearance. `signals.rs`: the
  curve, the odds, the age gates, the first ten.
- Client: `test-signals.mjs` (cards, summary, Send rows, report rows, the
  strip, art paths).
