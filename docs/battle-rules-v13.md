# Battle rules v13: tracking and fortune

Superseded for new battles by v14 (`docs/battle-rules-v14.md`), which keeps
this tracking and fortune and fights battles inside a system at its world.
Battles opened on rules v13 (`crates/sim/src/tactical/v13.rs`) keep it. It
adds two things to v12: big guns track small hulls poorly, tacklers worst of
all, and every battle has its fortunes. Everything else (v12's one shot per weapon,
line width, dreadnought and armour numbers, v11's tackle, webs, light
batteries and light guns, v10's formations and older) is kept.

## In battle

- **Big guns track small hulls poorly.** A battleship's, dreadnought's or
  titan's guns hit hulls lighter than a corvette (interceptors and scouts) at
  three quarters of the usual chance. Its torpedoes lock onto them three times
  in four; a torpedo that can't lock isn't launched, and the rack reloads as
  usual.
- **Tacklers are hardest of all.** An interceptor holding one of the side's
  hulls is weaving inside its big guns. Holding means within tackle range of
  a hull trying to leave, while the tacklers outnumber its Warp Stabilizers.
  Battleships and bigger hit a tackler at 15% of the usual chance and lock
  torpedoes onto it as rarely. Since v12 a battleship's four drivers rolled
  four times a volley and cleared its tacklers; now the hold lasts. Destroyer
  light batteries and cruiser guns track interceptors as before: escorts are
  the answer to tackle.
- **Fortune.** Each side rolls its fortune once per battle, from the
  battle's private seed: its weapons deal between 60% and 140% of their
  damage for the whole fight, every value equally likely. Fortune spends no
  other roll. The battle estimate rolls it afresh for each fight it projects,
  so the win chance a player sees now carries the uncertainty: a matchup
  that used to project as a sure thing may show 80%.

## Why these rules

Each alternative was measured at fleet-action size (40 fights per pairing in
each role), against v12, where beam interceptors score 26% against line
ships and every runner escapes beam interceptors:

| Capital tracking | Beam interceptors vs line ships | Driver battleships escaping beam / web tacklers | Titans escaping |
|---|---|---|---|
| ×0.25 everywhere | 78% | 0% / 32% | 0% |
| ×0.25 within 280 of the shooter | 54% | 71% / 100% | 0% |
| ×0.25 within 280 of any capital on the side | 78% | 0% / 36% | 0% |
| ×0.25 against tacklers only | 26% | 0% / 36% | 0% |
| ×0.75 everywhere, ×0.15 against tacklers (v13) | 36% | 0% / 0% | 0% |

Tracking everywhere makes interceptors immune to capitals in a stand-up
fight, so they beat every capital fleet. Tracking by range doesn't separate
tacklers from skirmishers: battleships in a group cover each other's
tacklers from outside 280, and in a stand-up fight capitals close to their
own band, inside 280 of the interceptors. Tying the hard penalty to tackling
holds every runner, and the mild general one lifts beam interceptors into
the 30–40% target against line ships.

Fortune's size trades blowouts against how reliably counters work (fleet
action, with v13 tracking):

| Fortune | Pairings 90/10 or worse | Among types scoring ≥ 45% | Pairings within 35–65% |
|---|---|---|---|
| none | 86% | 77% | 6% |
| ±35% | 66% | 48% | 13% |
| ±45% | 61% | 46% | 15% |

The target of at most 40% blowouts is out of reach at any sensible size:
most blowouts are lopsided (the winner takes 85% or more of the value
traded), and extrapolating the trend, flipping enough of them would take
swings of about ±80%. v13 uses ±40%.

## Measured effect

The full balance matrix against v12, at equal cost, 100 fights per pairing in
each role (report: `docs/balance/combat-balance-v13.html`). Targets met:
skirmish 10 of 15 (v12: 11), fleet action 11 of 16 (11), major battle 10 of
16 (9). The count barely moves, but every miss is closer except one:

- **Tackle holds again, and sinks.** Beam and web interceptors hold driver
  battleships, stabilized battleships and titans (0–8% get away; v12 100%),
  and the held fleets are lost (87–100%): tacklers that survive keep
  chipping at the edge of the fight. Battleships with destroyer escorts and
  beam cruisers still get away (84–100%), because escorts track interceptors.
  Running from a swarm without escorts is now worse than fighting it.
- **Interceptor beams are useful but not dominant:** 30–37% against line
  ships (v12 19–26%; target 30–40%) and 48–50% against the field (v12
  38–43%).
- **Fewer fights are settled in advance.** Pairings decided 90/10 or worse fall
  from 86–87% to 60–63% (among types scoring at least 45% against the field,
  from 78–79% to 44–49%). Pairings within 35–65% rise from 5–6% to 11–14%.
  Still short of the 40% target.
- **Broader mix and a flatter top.** The equilibrium mix has 6, 5 and 5 types
  (v12 5, 5, 3). The strongest type scores 75–78% (v12 76–81%).
- **Capital ladder closer, mostly still out of band.** Titans over
  dreadnoughts is in band in a fleet action (69%) and 75% in a major battle.
  Dreadnoughts over battleships is 75–79% in the two smaller sizes and 34% in
  a major battle. Battleships over cruisers is 23–38%.
- **One counter lost.** Fortune erodes a modest counter: Whipple brawler
  corvettes now beat driver corvettes only 58–60% of the time (v12 84–96%). The
  other 24 counter checks pass.

## Tests re-earned

- `world::battle_report_carries_per_kind_losses`: seven interceptors against
  six corvettes. With this test world's seed, fortune makes four against five
  a one-sided rout, and the test needs both sides to lose hulls.
- `world::platform_pool_attrits_tiers_then_reports`: fourteen interceptors
  (twelve fall short of a platform tier with this seed's fortune).
- v12's tests open on the frozen `open_v12`. Its `volley` gained the tracking
  and fortune inputs at 1, so v12 fights are unchanged to the bit.

## Test fights

`client/public/dev/battles/lockdown` is back to v11's eight webbing and ten
torpedo interceptors (v12 needed sixteen torpedo boats). The battleships
can't track the tacklers, so the line is held and sunk at the edge. Regenerate
after any combat change:

```bash
BATTLE_FIXTURES_WRITE=1 cargo test -p server battle_fixtures
```
