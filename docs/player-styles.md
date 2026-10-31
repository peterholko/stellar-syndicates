# Player styles

Status: built Oct 31 2026 (protocol v80, `--reset-galaxy`), uncommitted. The
alpha's two styles of play, as Peter agreed them (Oct 31 2026: "player styles
definitely not game modes").

## One ruleset, two builds

Every corporation can raid and be raided once founder protection ends. A
style is what a corporation builds and flies, never a flag:

- **Raider:** Interceptors to beat a defense, a Combat Freighter to carry the
  loot, the Raid order (repeating if wanted). Targets: personal pirate
  hideouts, players who've stopped playing, and players with weak defenses.
- **Defender:** the Vault, the station ladder, warships at home, Sensor
  Arrays, counter-scouting. It never has to attack a player; it fights
  pirates and can strike back for 12 hours when raided.

Balance rests on three stances, not two: **raider beats builder** (an
undefended builder's stores pay for raids), **builder beats turtle** (goods
spent on defense don't produce), **turtle beats raider** (raiding a defended
home costs more than it takes). Leave and founder protection stay the only
shelters that cover everyone.

## The daily loot cap

`world/raid_limits.rs`.

- A corporation loses at most **a quarter of its storage per rolling 24
  hours** (`RAID_LOSS_CAP_FRAC`) to other corporations' raids and wartime
  blockades, all raiders together, against its storage across every system it
  owns. Hourly buckets (`RaidLosses`) on the corporation.
- It stops applying once the owner has given no order for **three days**
  (`RAID_CAP_INACTIVE_S`, `Corporation.last_active`): an abandoned colony is a
  farm.
- Not counted: pirate raids (their own limit, `pirate::HOME_RAID_MAX_LOOT`),
  freight taken in space, a captured system's stockpile. The Vault and the
  settlers' 40 Provisions stay out of reach as before.
- A raid whose victim has lost the day's limit turns home; the raider hears
  why (`RaidCapped`: "Raid on Nergal: … has lost a day's limit.").
- The victim's Stores header shows "Raided today 80 / 425" once something
  has been taken (`View.raid_losses`).

## The raid warning

`world/rival_raids.rs`, `warn_of_rival_raids`.

- A rival fleet on a Raid or Blockade order against one of your systems is
  reported once a trip, when your Command Center or Sensor Arrays first see
  it (the shared detection rule, as the charter's raid). It travels home on
  the light from where it was seen (`RaidSighted`).
- "Raid inbound · Vesper · 3 ships → Nergal · ETA ~1 min." in the raid
  episode (`raid:{system}`, phase inbound), so the Inbox row, its Defend
  button, the toast and the Raid alert work as they do for pirates.
- Interceptors and freighters run dark, so the warning is only as early as
  your sensors: a Sensor Array I sees them more than three times farther than
  the Command Center alone.

## Repeating raids

`Fleet.raid_repeat`, `restart_repeat_raids`.

- "Raid ↻" beside Raid in the order preview, both shells
  (`BlockadeSystem.repeat`). The fleet line reads "Raiding Vidar · ↻ 3 · held
  · 46/80"; its recall is "Stop raids".
- After unloading at home the fleet sets out again once refuelled (docked
  fleets refuel themselves) and repaired, or after 30 minutes
  (`REPEAT_REPAIR_WAIT_S`) if it has at least half a tank: home's power and
  refining reserves aren't drawn for raids.
- It stops by itself, and says why (`RaidRepeatStopped`), when a trip comes
  home empty, a trip loses a hull, the victim's day is capped, the target
  can't be raided (protection, leave, a pact), home has no fuel, or home's
  storage couldn't take the loot. Any order the fleet receives ends it.

## Pirate hideouts near home

`world/personal_hideouts.rs`. Humans only.

- When the charter ends, a pirate crew sets up at one of the founder's
  doorstep stars (within 25,000 su), revealed on its map ("Pirate hideout ·
  Thuban."). One at a time, moving round the stars as each falls.
- Cleared like the den: an idle warship within reach opens the assault. Its
  loot becomes private salvage contracts, as any cleared hideout's does.
- The next crew comes 2 hours after a clear for the first three, then every
  6 hours, a little stronger:

| Hideout | Tier | Guards | Loot (base value) |
|---|---|---|---|
| 1st–2nd | 1 | 3 raiders at 75% hull | 30 Machinery, 35 Semiconductors (≈3,000 Cr) |
| 3rd–4th | 2 | 4 raiders | 40 Machinery, 40 Semiconductors, 40 Armaments (≈6,000 Cr) |
| 5th on | 3 | 6 raiders | 40 Machinery, 40 Armaments, 30 Titanium Ore, 20 Rare Elements (≈10,000 Cr) |

- The guards' guns are scaled to 0.1 (`HIDEOUT_DAMAGE_MULT`): each tier falls
  to one, two and three Interceptors on every seed tried.
- Left alone for 24 hours, its raiders hunt their founder's freighters and
  barges outside Gun Platform cover (within 30,000 su), never anyone else's,
  and never the home itself.

## The balance checks (Oct 31 2026)

Sim tests in `world.rs` (`§player-styles`):

1. **Undefended home** (Gun Platform, Vault, Warehouse, full stores, no
   warships) raided by 2 Interceptors and a Combat Freighter from 60,000 su:
   the Gun Platform kills a raider on 1 trip in 5. The day's haul under the
   cap is 350 units worth 6,660 Cr (the Vault keeps the Titanium and
   Machinery). Slow warp nets about 2,300 Cr a day after fuel and losses, so
   the fleet repays itself in about **2.5 days**; full warp burns more than it
   takes. The target was one day. It wasn't forced: the levers that meet it
   (a smaller Vault, a larger cap) cost the victims more, and Peter's rule for
   the alpha is that weak raids beat farmed testers.
2. **Defended home** (Defense Platform and Vault): the same fleet takes
   nothing and loses hulls. Passes as built.
3. **The cap**: however many holds come, the victim loses its day's limit and
   no more; it comes back a day later and lifts after three days without an
   order. Passes.

## Dev sandbox

For playtesting raids between new players (never in production):
`DEV_PVP_SANDBOX=1` founds new corporations past the charter, unshielded, with
the charter's basic buildings staffed; with `DEV_START_SHIPS=raider,raider,combat_freighter`
each starts with a raid fleet. Both are read on each start and never saved.

## Not built yet

The defender medal, bots using any of this, a balance probe, art, and any
style tutorial beyond the hideouts.
