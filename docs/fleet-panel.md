# Fleet panel

The desktop deck's fleet route has two layers: an everyday panel in the
workspace column, and a details workbench over the map. Rival fleets keep the
single read-only panel. The phone shell keeps its own fleet sheet.

## Everyday panel

What a player reaches for on most visits, top to bottom:

- Header: hull art and title; Report, Drive, Speed and Position (or Docked at).
- The assignment line, then any dispatch warning, hull damage or out-of-supply
  alert. Industry and job readouts follow when the fleet has them.
- Commands. Move is always offered. Jump appears for jump-capable hulls,
  Initiate docking when undocked, and Guard for guard-capable hulls.
  Situational verbs appear only while they apply: Hold position (a course or a
  pending hold), Lift blockade or Recall raid, Withdraw from battle, and the
  rescue call when stalled. Then Transit (Full speed, Economy or Stealth where
  offered).
- Orders in flight, Fuel, and Cargo with hauling for hulls with a hold.
- The Fleet details button, pinned to the foot of the column.

## Details workbench

Fleet details opens `#deck-fleet-workbench`. It takes the construction
workbench's place and size (`.deck-build-workbench`), centred over the map area.

- **Hero.** The hull in 3D (`<ship-model-preview turntable>`), swaying slowly
  about a three-quarter view until the player takes the camera. A mixed fleet
  shows one hull at a time, the flagship first; chips switch it.
- **Spec card.** Role, Impulse (the hull's base speed; warp multiplies it),
  Hull, Attack and Defense from `SHIP_STATS`.
- **Record.** Two columns, one when the workbench is under 640 px:
  - Readiness and Officer side by side.
  - Fleet at full width: composition, refit, split and merge.
  - More commands (interdiction field, builder emplacement, fuel transfer) and
    Standing policy side by side.
  - Mission profile at full width.

Esc, ✕ or the button closes it. It also closes when the route leaves the fleet,
or when the fleet resolves to an emplacement, a jump departure or a lost
contact. The founding guide's Recon fit step opens it on the scout, since
refits live there.

## Code

- `client/src/shell/deck/fleet.ts`: `ownHtml` (everyday) and `detailsHtml`
  (workbench).
- `shell/deck/markup.ts`: the workbench shell.
- `styles/deck.css`: `.deck-fleet__*` and `.deck-fleet-details*`.

Tests: `npm run test:fleet-panel`. `node client/scripts/test-fleet-panel.mjs
--serve` (launch.json `fleet-fixture`, port 4178) serves the real panel, deck
CSS and 3D models over canned fleets, without a game server. `?fleet=103&details`
opens the mixed squadron's details.
