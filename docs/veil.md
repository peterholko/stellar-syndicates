# The Veil

Nov 1 2026. Peter: "the defender can turn on an ionised veil that destroys any
fast or small objects and only large hardened troop transports can land." The
in-world reason troops take ground when a fleet owns the orbit. Before this
the sim forbade orbit from taking a colony by law (suppression pins, never
kills; population never decreases) but gave no reason a silenced garrison
wasn't simply shelled flat.

## Rules as built

1. **Veil Projector** (`StructureKind::VeilProjector`, `veil_projector`),
   levels I–III, one per world, no slot, no crew. Only on a body with a sky
   (`Body::has_atmosphere`: not a belt or gas giant, environment not
   Uninhabitable). Recipe: 50 Alloys, 40 Semiconductors, 30 Machinery,
   20 Polymers; 30 s base build. Research: **Ionospheric Veil**
   (Countermeasures III) opens level I, **Deep Veil** (Countermeasures IV)
   opens II–III. Nothing shows before the research (research-locked stays
   hidden).
2. **Raising is automatic.** `World::resolve_veil` runs inside the blockade
   pass, after each system's stage is settled and before plunder. The Veil is
   up when: a *held* war blockade stands over the system (a Raid order or a
   pirate home raid never counts), the owner has not forbidden it, and the
   projector's world can feed it. One per-world switch, "Raise under siege",
   sets `StarSystem::veil_never` (`Command::SetVeilPolicy`).
3. **Fed by the Fusion Plant's reactor.** A raised Veil takes
   `power::veil_mw(level)` = 6 / 10 / 16 MW, charged to the plant's fuel at
   the plant's own burn rate on top of the grid: a Veil I on Deuterium burns
   0.03 a second (108 an hour), a Veil III 0.08 (288 an hour). A Solar Array
   (plant tier < 3) can't carry it, so it is mid-game by nature. The tick's
   burn comes out of the system's free stock; when the stock can't cover it
   the Veil drops (`VeilDown::Dry`) and stays down until fuel returns, which a
   held blockade prevents. A siege-anchor battleship overhead multiplies the
   burn by `SIEGE_ANCHOR_MULT` (1.25): heavy slow slugs partly get through.
   (Deviation from the spec: not a grid load, because solar fields would have
   carried a Veil I forever and a grid load flickered at the margin.)
4. **While up, the world is sealed.** `resolve_bombardment` reads zero guns
   over a veiled system, so suppression decays to nothing and the garrison
   fights whole. Plunder skips veiled systems. Hardpoints never repair under a
   held blockade anyway, so the blockade stays held and no battery rule was
   needed. Orbital stations (level 2+) are above the Veil and fight as before.
5. **Only Transports land.** `begin_assaults` counts `Fleet::transport_marines`
   under a Veil and passes them through `ground::through_veil`: descent losses
   `VEIL_DESCENT_LOSS` = 10 / 15 / 20% by level, rounded against the attacker.
   A capital's boarding party stays aboard (a landing of nothing is a hold,
   `AssaultHeld` with 0 marines). `GroundRecord::descent_losses` and
   `AssaultBegan::descent_losses` carry what the sky took.
6. **The 6 h siege gate stays** (offline protection, stretched by a fed
   garrison). After it ripens the attacker chooses: land Transports through
   the Veil at full garrison strength, or hold the blockade until the fuel
   runs out and the old siege (bombard, strip, land cheap) resumes.
7. **Untouched.** Homes already can't be captured; they need a projector like
   anyone else (the spec's "Founding-grade home Veil" was dropped: a free
   permanent home Veil would have blocked the home-plunder loop). Raids are
   unaffected. The charter is unchanged. Capture halves the projector like
   every structure and clears the sky (`veil_up = false`, policy reset).

## Events and view

- `EventPayload::VeilRaised { owner, by, system, pos, level }` and
  `VeilDropped { owner, by, system, pos, reason: Relieved | Dry | Forbidden }`.
  Owner light-delayed, besieger at once; journal kind Blockade, group
  `blockade:{system}`.
- `GroundStateView.veil: VeilStateView { level, up, descent_loss, burn_per_s,
  fuel, raise }` for the two siege parties (an aurora is not subtle).
- `LandingOddsView` under a Veil samples the viewer's own Transports through
  the descent instead of the history's marine count.

## Client

- Planet panel: the projector is listed like the Vault (no footprint); its
  row shows the burn, the descent loss, VEIL UP / Held down, and the owner's
  one switch. Siege banner adds `VEIL UP · landings −N%`; an aurora curtain
  (`.deck-planet__veil-sky`) hangs over the world while up.
- System View: a breathing aurora shell around the besieged world
  (`COL_VEIL`), headline detail `VEIL UP`.
- Blockade briefs (desktop + mobile) say what the Veil changes for each side.
- Ground theater caption names the descent losses.

## Bots

Defenders weight the projector like a Vault under threat (`strategy.rs`);
besiegers size troop waves for a Veil III descent (`warfare.rs`, ÷0.8) and
already wait for a ripe siege before landing, so under a Veil they land only
with 1.5× the unsuppressed break-even in Transports.

## Tests

`world::tests::a_veil_blanks_bombardment_and_plunder_until_the_reactor_runs_dry`,
`through_a_veil_only_transports_land_and_the_veil_takes_its_share`,
`a_veil_needs_a_reactor_a_siege_and_the_owners_leave`; the research catalog,
build-ladder and belt-suits tests were updated for the new kind.

## Open

- Scout reports don't carry the projector level yet (`SystemIntel`), so a
  besieger learns the Veil when it rises.
- Veil numbers (MW, descent losses, recipe) are first guesses; no bot balance
  run with sieges yet.
- Art owed, briefs in `docs/veil-art-prompt.md`: three Veil Projector sprites
  (I–III, the Power Plant route: surface sprite doubles as the icon; until then
  the Sensor Array icon stands in) and the two programme emblems (in the
  research emblem queue, 121 now). The aurora needs no art; the projector never
  draws on the planet stage (off-site like the Vault).
- The exotic node bonus is also called "Veil" (`node::NodeBonus::Veil`, a
  signature multiplier). Distinct code, same word.
