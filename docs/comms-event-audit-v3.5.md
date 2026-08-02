# Communications event audit (v3.5)

This is the durable audit of every server-to-client message and every owner-facing
state family as of `§comms-v3.5`. Its rule is stricter than ordinary access control:
private truth is still a leak if a remote change reaches the command center before
its physical report.

## Legend

- **PHYSICS** — the surface changes only when a priced signal/picture arrives.
- **ESTIMATE** — a deadline computed from facts the player already has; it may expire
  without proving that the predicted event occurred.
- **EVENT** — evidence of an occurrence; it must be keyed to an arrival.
- **CONTROL** — protocol/session response, not an in-fiction observation.
- **Clean** — the current trigger satisfies the class.
- **Fixed** — corrected by v3.3/v3.5 and protected by a teeth test.
- **Policy conflict** — an older, explicit “own clock” ruling conflicts with the new
  universal-arrival ruling. These are listed rather than half-fixed: delaying only a
  toast while the same truth flips in `View` would remain a leak and make the client
  internally contradictory.

## Every `ServerMsg`

| Message | Class | Current trigger and audit result |
|---|---|---|
| `Welcome` | CONTROL | Join handshake and public constants are clean. The included initial galaxy is public geography. Fresh owner administration subsequently arrives through `View`; see the owner-state conflicts below. |
| `GalaxyUpdate` | CONTROL / EVENT | Public geometry refresh after a new home is minted. **Policy conflict:** it is broadcast at the join tick and therefore also announces that public change immediately. Recommendation: either rule public chart mutations control-plane truth, or give the new system a chart-bulletin emission point and per-viewer arrival. |
| `View` | mixed | Tick/time are CONTROL; market, ghosts, systems, infrastructure, orders, battles, and private administration are audited separately below. |
| `Sections` | mixed | Battle/capture reports are PHYSICS and clean. Standing-order definitions are local administration; runtime status is a policy conflict. Rankings are a public deterministic ledger-close publication (ESTIMATE) and clean under that ruling. |
| `BattleRecords` | PHYSICS | Header/round/outcome frontiers are emission-time gated in `visible_record_specs`; connection cursors ship only newly arrived material. Clean. |
| `GroundRecords` | PHYSICS | Same arrival-frontier rule as battle records in `visible_ground_specs`. Clean. |
| `Report` | EVENT | `DelayedReports::due_for` releases a participant report only at its priced arrival. The concluded-battle icon is retained to the same wavefront. Clean. |
| `Timeline` | mixed | Each payload is classified in the event table below. Promotion itself is arrival/deadline keyed and clean; some inputs deliberately use true-time “own clock.” |
| `Trade` | mixed EVENT / local CONTROL | Sent directly on the tick of every `TradeEvent`. Hub buys/sells/rejections/bookings can be local, but remote delivery/diversion/overflow/freight milestones are policy conflicts. Recommendation: split local Charterhouse actions from positioned remote outcomes, then serve the latter from one delayed owner-event ledger used by both this message and owner state. |
| `CommandSignal` | ESTIMATE | Immediate acknowledgement and animation derived from the served ghost and player-known network. It predicts the outbound meeting; it is not evidence that the order arrived. Clean. |
| `OrderConfirmed` | EVENT | Server-event-driven only. Re-entry now emits at the exact nominal boundary and arrives through the same priced wavefront as the boundary picture. **Fixed v3.5.** |
| `RoutePreview` | ESTIMATE | Immediate decision aid derived from the served sighting and public lanes. Clean. |
| `EngagementEstimate` | ESTIMATE | Immediate projection from served composition/intel with ages disclosed. Clean. |
| `Error` | CONTROL | Validation/protocol response to the caller. Clean. |

## `View` and other owner-facing state families

| Surface / flip | Class | Current trigger and audit result |
|---|---|---|
| Render clock, tick | CONTROL | Server cadence only; not world evidence. Clean. |
| Ghost position, velocity, path, drive, composition | PHYSICS | Retarded samples, wake channel, frontier cursors, and destruction ledger. Clean. |
| Ghost `in_comms`; full/blip/arrow presentation | PHYSICS | Sampled physical membership; exact boundary report inserted and presentation may flip only when that report arrives. Clean (v3.1). |
| Ghost death/disappearance | EVENT | Retained until the destruction sample/report arrival. Clean. |
| Own-ghost posture, engage-freight, garrison host/fed, supplied, survey progress | EVENT / state | Injected from current `World` after the delayed ghost is built. **Policy conflict:** explicit older “fresh private policy/quartermastery” rulings. Recommendation: put mutable remote hull state in the sampled track and remove all truth overlays; keep only command-center policy definitions local. |
| Own manifest/cargo on a remote hull | EVENT / state | Current run/manifest is overlaid onto the served ghost. **Policy conflict.** Recommendation: sample the owner-readable manifest with the hull picture; keep only hub-held cargo local. |
| Pending order: scheduled / signal outbound | CONTROL + ESTIMATE | The command-center action is immediate; `arrives_at` is computed from the served sighting. Clean. |
| Pending order: presumed delivered / awaiting response | ESTIMATE | Client crosses the quoted deadline, but this is explicitly labelled presumed and is not confirmation. Clean. |
| Pending order: confirmed | EVENT | Only `OrderConfirmed`; exact boundary/report arrival for re-entry, echo arrival otherwise. **Fixed v3.3/v3.5.** |
| Pending order: lost / relay casualty | EVENT | Hidden loss is disclosed only when the frozen surviving-network death news arrives. Clean (v3.3). |
| Order comet and route | ESTIMATE | Player-known inputs only; no true-fleet position. Clean. |
| Relay circles and wire after destruction | EVENT | `relay_network_known` retains the dead site until `news_at`; casualty and map change share that wavefront. Clean (v3.3). |
| Newly constructed own emplacement / relay circle / wire | EVENT | Current emplacement truth appears immediately and a new relay enters `relay_network_known` immediately. **Policy conflict / uncovered counterpart to relay death.** Recommendation: add a positioned `EmplacementBuilt` report with a frozen route; gate the emplacement, known network, and circle on its arrival as one transaction. |
| Rival emplacement discovery | PHYSICS | Requires served sensor coverage; stationary position then remains known. Clean under the established “detection is the report” rule. |
| Emplacement demolition/destruction | EVENT | `GoneEmplacement` holds the structure until per-viewer news; relay owner uses the frozen casualty wavefront. Clean (v3.3). |
| Battle start icon and participant suppression | EVENT | `started_at + distance/c`; no icon for a battle whose start light never arrived. Clean. |
| Battle conclusion / aftermath | EVENT | Concluded icon retained until the same arrival that releases the report and retained aftermath. Clean. |
| Battle and ground replay rounds | PHYSICS | Round emission frontier plus participant/bucket fidelity gates. Clean. |
| Rival system ownership / claim | EVENT | `claimed_at + routed delay` before the rival owner appears. Clean. |
| Own claim/capture/loss and `mine` management handoff | EVENT | Current owner is special-cased as immediately known; old-owner state is not retained. Capture reports themselves are delayed. **Policy conflict.** Recommendation: retain ownership epochs `(owner, from, to)` and select the last epoch whose transition light arrived; gate owner-only management off served ownership, not truth. |
| Blockade badge | EVENT | Defender is delayed; besieger sees current truth immediately. **Policy conflict:** older “fleet is there” ruling bypasses the command center. Recommendation: gate both participants from the served on-station fleet/boundary report, while the sim continues using truth. |
| Ground/siege readout and odds | PHYSICS / ESTIMATE | Defender/besieger read current garrison truth; odds are derived from it. **Policy conflict.** Recommendation: store a positioned fortification observation for each participant and compute odds only from that served snapshot. |
| Capture report / blockade timeline notice | EVENT | Participant capture reports and defender blockade notices are delayed. Besieger’s immediate notice is covered by the same policy conflict as the badge. |
| Stockpile, structures, modules, specialists, population, food, assignments, converters, storage, build queue | EVENT / state | Owner receives current remote system truth. **Policy conflict:** broad older “own economy / own clock” contract. Recommendation: introduce a per-system owner snapshot stream and serve its arrival frontier; all system panels and completion notices must read that same snapshot. |
| Build countdown | ESTIMATE | Once a build-start snapshot is known, `complete_tick` is a player-computable deadline. It must remain an estimate until a completion report/snapshot arrives if completion can fail or stall. Current live queue makes it a policy conflict with the row above. |
| Repair/build/refit/training/module completion | EVENT | Timeline and owner ledgers currently flip at true completion. **Policy conflict.** Recommendation: positioned system completion events feeding the same per-system owner snapshot stream; never delay only the toast. |
| Survey completion, exact deposits, trait reveal, scout intel | EVENT | Survey/trait/intel knowledge is inserted or exposed only after its report chain arrives; ally intel adds both legs. Clean. |
| Node awakening | ESTIMATE | Awakening time is public config and the badge is a deterministic countdown. Clean. |
| Node holder | EVENT | Inherits the ownership handoff conflict above even though `NodeCaptured` timeline news is delayed. Recommendation: holder tint reads served ownership epoch. |
| Node fed/bonus state | EVENT / state | Current owner truth. **Policy conflict:** older own-upkeep ruling. Recommendation: include it in the per-system owner snapshot stream. |
| Market prices | PHYSICS | Price history is sampled at `now - hub distance/c`. Clean. |
| Wallet, charter, freight desk and shipments | mixed | Hub-local cash/book actions can be CONTROL/local PHYSICS; credits, standing, fuel total, and shipment milestones can change from remote events and currently flip immediately. **Policy conflict.** Recommendation: split hub ledger from remote receivables; apply remote deltas only when their positioned reports arrive. |
| Doctrine and standing-order definitions | CONTROL | Command-center policy authored by the player. Clean. Runtime `in_flight`/status changes are remote EVENT state and belong in the delayed owner-event ledger. |
| Syndicate roster/invites/fits | CONTROL / EVENT | Direct invitations and authored policy are local; remote membership acceptance is exposed immediately to every member. **Policy conflict.** Recommendation: retain membership epochs and relay acceptance from the accepting member’s command center. |
| Research state/completions/stall | EVENT / state | Distributed Academy truth is exposed immediately syndicate-wide. **Policy conflict.** Recommendation: define a syndicate research reporting node (or aggregate arrived Academy reports) and key state/toasts to that wavefront. |
| Public rankings | ESTIMATE | Published only at deterministic ledger closes and identical to all viewers. Clean under the public-publication ruling. |

## Every simulation event that can reach a player-facing surface

| `EventPayload` | Class | Delivery / result |
|---|---|---|
| `PlayerJoined` | CONTROL | Session/persistence event; the `GalaxyUpdate` publication question is listed above. |
| `ShipSpawned` | PHYSICS | Reaches rivals and owners as a track sample; remote owner roster truth remains part of the owner-state policy conflict. |
| `OrderApplied` | internal | No direct server message; reaction arrives through the ghost picture. Clean. |
| `OrderScheduled` | CONTROL / ESTIMATE | Allocates lifecycle/comet at the command center. Clean. |
| `OrderDelivered` | ESTIMATE | The lifecycle may cross the already-quoted delivery deadline; no phase-3 evidence. Clean. |
| `OrderConfirmed` | EVENT | Arrived echo/re-entry evidence only. Fixed. |
| `Trade` | mixed | Local Exchange actions are local; remote variants are the `Trade`/owner-ledger policy conflict above. |
| `RaidResolved` | EVENT | `DelayedReports`, timeline, icon handoff, and record frontiers. Clean. |
| `SystemClaimed` | EVENT | Rival timeline/view delayed; owner timeline/view immediate. Ownership policy conflict. |
| `ShipDestroyed` | EVENT | History death ledger + delayed report/timeline. Clean, except owner flagship headline below. |
| `EmplacementDestroyed` | EVENT | Frozen owner-news wavefront and per-viewer retained wreck. Clean. |
| `FlagshipDestroyed` | EVENT | Rival timeline delayed; owner headline and syndicate flagship state immediate. Policy conflict; recommend wreck-to-owner/syndicate arrival. |
| `BuildStarted`, `BuildRejected`, `SystemUpgraded` | mixed CONTROL / EVENT | Request acceptance/rejection can be local only if validation is command-center knowledge; remote start/completion and live system state are policy conflicts. |
| `ColonyHeld`, `IntelGathered` | EVENT | Positioned owner timeline/report gates. Clean. |
| `SpecialistHired`, `ModulesPurchased`, `ModulesSold` | CONTROL | Charterhouse contract/settlement; clean if defined at the hub. |
| `SpecialistTrained`, `ModuleBuilt`, `ShipsRefitted`, `FleetRepaired`, `ModulesDelivered`, `SpecialistsDelivered` | EVENT | Remote completion/delivery currently own-clock. Policy conflict; recommend positioned system report + served system/hull state. |
| `AssaultHeld`, `AssaultBegan`, `AssaultRepulsed` | EVENT | Positioned timeline/ground records are delayed where mapped; authoritative ground state remains a policy conflict. |
| `GarrisonSupplyStateChanged`, `FleetSupplyChanged` | EVENT | Current owner state (and some notices) is immediate. Policy conflict; recommend system/hull sample channel. |
| `SystemPlundered` | EVENT | Victim delayed; besieger immediate. Besieger policy conflict; recommend on-station fleet report wavefront. |
| `ModulesLost`, `SpecialistsLost` | EVENT | Positioned loss notices delayed. Clean. |
| `ResearchCompleted`, `TierUnlocked`, `ResearchStalled`, `ResearchResumed` | EVENT | Immediate to every member. Distributed-research policy conflict. |
| `AssignmentSet` | CONTROL | Player-authored local policy; remote applied-state acknowledgement needs the per-system stream if application can differ. |
| `ProductionSuspended`, `ProductionResumed`, `FoodStateChanged` | EVENT | Immediate owner timeline and live system state. Policy conflict. |
| `PirateEnclaveCleared` | EVENT | Positioned victor notice delayed. Clean; any plunder ledger delta must remain coherent with its arrival. |
| `NodeAwakened` | ESTIMATE / EVENT | Badge is deterministic ESTIMATE; public timeline is position-delayed. Clean. |
| `NodeCaptured` | EVENT | Timeline delayed; holder/ownership state inherits ownership policy conflict. |
| `NodeSupplyChanged` | EVENT | Immediate owner state/timeline. Policy conflict. |
| `SurveyCompleted`, `TraitRevealed` | EVENT | Positioned knowledge legs and notices. Clean. |
| `GarrisonSupplyChanged` | EVENT | Positioned at host and delayed to sender. Clean; raw ghost/system overlay remains a policy conflict. |
| `PlatformEngaged` | EVENT | Positioned owner notice delayed, attacker learns through normal battle report. Clean. |
| `FuelShortfall` | mixed | If refusal is decided at command issuance it is CONTROL; if caused by remote fleet/system truth it is EVENT. Current immediate owner notice is a policy conflict; recommendation: distinguish validation-site causes in the payload. |
| `BlockadeEstablished`, `BlockadeLifted` | EVENT | Defender delayed; besieger/current state immediate. Participant policy conflict above. |
| `Citation`, `EnforcementDispatched`, `EnforcementWithdrawn` | EVENT | Public Charterhouse bulletin, delayed from the hub to every command center. Clean. |
| `OrderRejected` | CONTROL / EVENT | Sovereign-zone validation can be local; remote unsupplied refusal depends on fleet truth. Policy conflict; recommendation: local reject for command-known facts, otherwise fleet response light. |
| `SystemCaptured` | EVENT | Participant reports/timeline delayed; ownership/management state conflict above. |

## Regression anchors

- `confirmation_never_precedes_the_crossings_light` proves that re-entry phase 3
  equals the exact bisected boundary report arrival. Re-keying it to the first true
  inside tick fails the epsilon assertion.
- `confirmation_falls_back_with_boundary_light_after_midflight_relay_death`
  proves that both picture and confirmation abandon a broken fast copy for the
  same frozen passive copy. Leaving confirmation on the fast clock fails it.
- Existing v3.1/v3.3 tests cover picture-frame bubble morphing, relay-casualty
  disclosure, mortal orders, delayed demolition, battle icon/aftermath handoff,
  capture reports, surveys, and record round frontiers.

The policy-conflict rows should be solved by coherent served-state streams, not by
adding arbitrary UI timers. Until each subsystem has that representation, the audit
keeps the conflict visible and prevents a cosmetic delay from being mistaken for an
epistemic fix.
