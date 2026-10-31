# Scouting and counter-scouting

Peter (Oct 28 2026), after asking how Travian handles counter-scouting:
"Yes do it". Travian's rule: only scouts fight scouts. Too few at home and
you are spied on silently; enough and you see it coming and blind the spy.

## The rule (`World::gather_intel`, `World::counter_scouting`)

- Any fleet carrying a Scout within 1,300 su of a rival system probes it.
- **Counter-scouting** = the Scouts the system's owner and its allies keep in
  range of it, plus 2 a Sensor Array level there (`COUNTER_SCOUTING_PER_ARRAY`).
- The probe needs **more** Scouts than that. A tie goes to the defender.
- **Through:** the probe reads the defences (as before) and the stores: every
  good in raiders' reach, dearest first, and the Vault's room
  (`Corporation.scouted_stores`, Travian's "resources" report). The defender
  is told who it was ("Freya scouted by Vesper") only if it had any
  counter-scouting. With none, nobody is told, as in Travian.
- **Blocked:** the probe reads nothing. The defender hears "Scout probe
  blocked at Freya: Vesper"; the prober hears "Probe blocked at Freya: its
  counter-scouting held. Send more Scouts."
- Nothing is destroyed: it's a count, not a fight. Notices fire once per
  approach (`World.scout_contacts`), light-delayed from where the scout stood.
- Pirate bases are read as before; pirates don't counter-scout.

## Screens

- A rival system's overview (desktop) and sheet (mobile) carry a **Scout
  report**: its age, the defences, the goods in reach with icons and their
  value at the arrived Hub quotes, and the Vault. Without one: "None · send
  Scouts", the rule in its tooltip.
- The served field is `SystemStateView.scouted_stores` (v74), injected by the
  game loop once the look's light reaches command.

## Tests

`world::tests::counter_scouting_blocks_a_weak_probe_and_a_strong_one_reads_the_stores`,
`world::tests::an_undefended_system_is_scouted_unseen`, `client/scripts/test-scouting.mjs`.
