# Pirate dens

Peter (Oct 24 2026), after production showed dozens of two-second battles at
one guest's home:
- "If a pirate retreats it should retreat to its pirate den, i.e. every
  pirate should have a den."
- "They shouldn't have been able to travel across the entire galaxy and
  further without refueling."

## What was wrong

- **The loop:** a damaged Rift raider (51 of 200 hull, set to withdraw at half
  hull) blockaded a guest's home. Each blockade battle made it retreat at once.
  - The retreat took it off the battle's side, so the raid never learned it had
    withdrawn.
  - The next tick re-issued the Blockade and opened the next battle.
  - About 37 battles per raid, twice. Fixed in `home_raid_battle_ended`.
- **Retreats into empty space:** a mission withdrawal sent any pirate without a
  linked den to a point 20,000 su to its right. "Sending a pirate home" did
  nothing, because pirates have no home system.
- **No ship crossed the galaxy.** The line of markers was the map fanning out
  70 markers drawn at one spot.

## Every pirate has a den

- **The link:** `Fleet::pirate_den` holds a den's system id, kept current each
  tick by `assign_pirate_dens` (`world/pirate_dens.rs`). It keeps a standing
  den; otherwise it takes:
  1. the den whose pack it is;
  2. the base whose patrol or support guard it is;
  3. the base of its home raid;
  4. the nearest standing den.

  Founders' privateer dens hold only their own garrisons.
- **Retreats go to the den:** a mission withdrawal, `send_ship_home` and a home
  raid's return all send the pirate there.
- **At the den** a pirate out of battle refills both stores and mends 1/1800 of
  its hull a second, so full repair takes half an hour. It doesn't mend while a
  player's warship is at the den, or at a fixed site (depot or stronghold): an
  expedition's damage stays.
- **Launches need repair:** a pack launches only with 90% of its hull.
- **New spawns are full:** every pirate spawn starts with full stores
  (`Fleet::fill_stores`).

## Pirate fuel: built, switched off

`PIRATES_BURN_FUEL` (`world/pirate_dens.rs`) is `false`, so pirates still fly
free.

- **When it is on:** pirates pay for movement exactly like players. One that
  has spent half its range and can't get home on the rest (×1.25, plus 10,000 su)
  turns for its den, and a home raid ends first.
- **Why it is off:**
  - Loot weighs 28 a unit. A raider carrying 60 units reaches about 90,000 su on
    both stores; empty, it reaches about 175,000 on fusion plus 700,000 on Slow.
  - Production raids run 280,000–330,000 su each way, so a loaded raider
    stranded on the way home in the replay of the live save.
- **Before turning it on, choose one:**
  1. Raids pick only homes within a loaded round trip.
  2. Pirate hulls carry bigger tanks.

## Checks

- **Tests:** `world::pirate_dens::tests` and
  `pirate_home_raids::a_pack_that_withdraws_ends_its_raid_instead_of_reopening_the_fight`.
- **Production replay:** two game hours of the production save under this code
  (Oct 24). All 29 pirates got a den; none stalled or left the map; raids kept
  cycling; no short battles. The damaged raider went home, mended, then
  raided again.
