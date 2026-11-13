# Ship parts and modules — plan

Agreed with Peter on Nov 8 2026. One rule for everything a ship is made of:

- **Parts** are what a hull is built from: reactors and warp drives, the
  frame (Hull Sections), the Nav Computer, the Cargo Bays. Fixed at build;
  the reactor and drive stay with the airframe, like a fighter's engine.
- **Modules** are what you fit: weapons, armour, control and utilities, in
  slots and fitting points. Weapons swap easily, like a fighter's stores
  (Peter: "weapons should be able to be swapped out easily vs the reactor /
  warp drive. Kinda like a modern fighter").
- **Both are ordinary goods**: one stockpile, the Hub order book, holds and
  freight, and pirates raid them like any good.

## Phase 1 — modules become goods (BUILT Nov 8 2026, `--reset-galaxy`)

- The seventeen modules are `Commodity` goods with their module's slug
  (`ModuleKind::good`, `Commodity::module`). They weigh as parts in a hold.
- **Stockpile, not a ledger.** `StarSystem.modules` is gone;
  `module_stock` / `take_modules` / `put_modules` read and write the
  stockpile. Building a fitted ship, refitting and fitting a station
  hardpoint take modules from it; stripped ones go back.
- **Made as before:** one-off `BuildModule` orders at an Armaments Complex
  (weapons, armour, control) or a staffed Shipyard (utilities), under the same
  research rules; the finished module lands in the stockpile.
- **The Hub book.** A module's base price is its recipe at base prices × 1.2
  (`market::MODULE_VALUE_ADDED`, the kit parts' margin): Mass Driver 701,
  Torpedo Rack 917, Point-Defense Screen 730, Reflective Plating 386, Whipple
  Armor 638, Extended Tanks 826, Recon Suite 874, Cargo Pods 1,238, Escort
  Datalink 1,474, Fuel Transfer Rig 1,646, Survey Booster 1,934, Nebula
  Spectrometer 931, Prismatic Lance 1,332, Lance Array 1,584, Stasis Web 758,
  Warp Stabilizer 797, Interdiction Field 1,270. Modules trade on the thin
  book (a bottomless one would buy every module a corporation can make at
  its margin). The 2× buy / 0.5× buy-back module shop is gone.
- **Found-only modules** (Survey Booster, Nebula Spectrometer, Prismatic
  Lance): the Authority never sells them (`market::hub_sells`); the Hub buys
  them, and corporations can trade them through limit orders.
- **Hidden until researched** (`World::good_known`, client `goodKnown`): a
  module is known once research opens it (an arrived blueprint counts);
  military modules need none. A module in stock always shows in Stores.
- **Hauling:** module crates, their 12 berths a freighter, and the
  `TransferModules`, `BuyModule` and `SellModule` orders are gone. Prize
  caches, exploration finds, signal finds and wreck salvage load into a hold
  as cargo; a hull with no room leaves them (a signal's become a cache for a
  freighter). Docking no longer lands them on its own: unload like any cargo.
- **Raids:** a home raid takes the dearest goods first, so a Torpedo Rack
  goes before Semiconductors (`a_home_raid_takes_a_torpedo_rack_first`).
- **Survey Drive → Survey Booster** (display; slug `survey_drive` stays): its
  recipe takes 13 Precision Components and 16 Alloys in place of a Fusion
  Reactor and 12 Alloys (the same 1,612 basket), so "drive" only means the
  warp drive.
- **Tabs: Resources · Parts · Modules** (`core/derive/goods.ts`): one split
  in the Hub market (Exchange board and Warehouse), the free trader's goods
  and the Stores pop-out, on desktop and mobile. A strip shows only when a
  second tab holds something. Resources: ores, refined goods, materials
  (Machinery, Weapon Mounts, Composites, Precision Components), Provisions,
  fuel and munitions. Parts: reactors, drives and Hull Sections of every size,
  Nav Computer, Cargo Bay, Pulse Laser, Beam Battery (Hull Sections moved
  out of Stores' "Advanced"). Modules: Weapons, Armour, Control, Utilities.
- Protocol v85.

## Phase 2 — guns become modules (design for review)

[guns-as-modules.md](guns-as-modules.md): no built-in guns; every warship has
mount points (Interceptor 4 … Titan 18) that are its Weapon Mounts; every gun
is a module on them. Needs a new battle rules version. Not built.

## On hold

- **Refit.** Module refits work as before (now from the stockpile); engines
  stay as built (the sim's `RefitEngines` has no client button). The refit
  analysis comes later, on the fighter rule above.
- Making modules on continuous production lines like parts.
- Credits and upkeep.
