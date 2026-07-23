//! The per-player check-in timeline (§16, async-automation Layer 3).
//!
//! Pillar 1 of the design: *presence gives awareness, not advantage*. A player
//! who logs in once a day must still be able to make good decisions — so when
//! they check in they need a clear digest of **what became observable while they
//! were away**, and a nudge toward the decisions waiting for them. (The decisions
//! themselves — "attention items" — are derived client-side from the player's own
//! View, since that already carries their owned systems, stockpiles, and standing
//! orders; this module owns only the historical *timeline*.)
//!
//! The timeline is a RETAINED, per-player journal that records discrete news at
//! the moment it BECAME OBSERVABLE to that player — own economy/automation on
//! their own clock (instant), distant battles and rival claims **light-delayed**
//! to their command center (the same retarded-time rule as [`crate::reports`] and
//! the view filter). Crucially it is populated for offline players too, so the
//! "since you were away" digest is real. It is ephemeral awareness state (not part
//! of the deterministic sim snapshot), like the report scheduler.

use std::collections::{BTreeMap, VecDeque};

use sim::{Commodity, Event, EventPayload, PlayerId, RaidOutcome, ShipKind, TradeEvent, World};

use crate::protocol::{TimelineEntry, TimelineSeverity};

/// Keep at most this many entries per player (a rolling check-in log).
const JOURNAL_CAP: usize = 40;
/// Drop a not-yet-observable entry whose light somehow never lands (bounded mem).
const MAX_PENDING_AGE: f64 = 1800.0;

struct Pending {
    player: PlayerId,
    /// Sim-time at which this becomes observable to `player` (light-arrival).
    observe_time: f64,
    severity: TimelineSeverity,
    text: String,
    promoted: bool,
}

#[derive(Default)]
pub struct Timeline {
    pending: Vec<Pending>,
    journal: BTreeMap<PlayerId, VecDeque<TimelineEntry>>,
    /// Last sim-time each player was online — the "while you were away" boundary.
    last_seen: BTreeMap<PlayerId, f64>,
}

impl Timeline {
    pub fn new() -> Self {
        Timeline::default()
    }

    /// Record a tick's events as future timeline entries, each at its
    /// player-specific observable time. Runs for ALL players (online or off).
    pub fn ingest(&mut self, events: &[Event], world: &World) {
        let c = world.config.c;
        for e in events {
            match &e.payload {
                // Own economy / automation — observable on your own clock now.
                EventPayload::Trade(te) => {
                    if let Some((sev, text)) = trade_entry(te, world) {
                        self.push(te.player(), e.time, sev, text);
                    }
                }
                // A battle — each side learns the SAME outcome when its light
                // reaches their command center (so generally at different times).
                EventPayload::RaidResolved {
                    attacker,
                    defender,
                    attacker_kind,
                    target_kind,
                    outcome,
                    pos,
                    ..
                } => {
                    for &p in &[*attacker, *defender] {
                        let Some(cc) = world.players.get(&p).map(|corp| corp.command_center) else {
                            continue;
                        };
                        let observe = e.time + pos.distance(cc) / c;
                        let (sev, text) = raid_entry(p, *attacker, *defender, *attacker_kind, *target_kind, *outcome);
                        self.push(p, observe, sev, text);
                    }
                }
                // §TCA Phase 2: a CITATION is a PUBLIC bulletin from the
                // Charterhouse. Everyone learns it light-delayed from the hub — the
                // reputational hit rides the same wavefront as the legal one. The
                // culprit reads it as an indictment; everyone else reads it as
                // intelligence about who is worth avoiding (or hiring).
                EventPayload::Citation { culprit, offense, pos, occurred_at } => {
                    let who = world
                        .players
                        .get(culprit)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("{culprit}"));
                    let lag = (e.time - *occurred_at).max(0.0);
                    let when = if lag >= 1.0 {
                        format!(" (the offense was {} ago)", fmt_wait(lag))
                    } else {
                        String::new()
                    };
                    for (&p, corp) in &world.players {
                        let observe = e.time + pos.distance(corp.command_center) / c;
                        let (sev, text) = if p == *culprit {
                            (
                                TimelineSeverity::Bad,
                                format!(
                                    "The Terran Charter Authority has CITED your corporation for {}{when}.",
                                    offense.title()
                                ),
                            )
                        } else {
                            (
                                TimelineSeverity::Info,
                                format!("Authority bulletin: {who} cited for {}{when}.", offense.title()),
                            )
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // §TCA Phase 2: ENFORCEMENT bulletins, public from the Charterhouse
                // on the same light-gating as a citation. The announcement's light
                // outruns the squadron — that IS the target's lead time.
                EventPayload::EnforcementDispatched { target, system, pos } => {
                    let who = world.players.get(target).map(|c| c.name.clone()).unwrap_or_else(|| format!("{target}"));
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        let observe = e.time + pos.distance(corp.command_center) / c;
                        let (sev, text) = if p == *target {
                            (
                                TimelineSeverity::Bad,
                                format!(
                                    "AUTHORITY ENFORCEMENT DISPATCHED against your corporation — a squadron is \
                                     under way to blockade {name}. Pay reinstatement to call it off, fight it, or wait it out."
                                ),
                            )
                        } else {
                            (TimelineSeverity::Info, format!("Authority bulletin: an enforcement squadron sails against {who} at {name}."))
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                EventPayload::EnforcementWithdrawn { target, recalled, pos } => {
                    let who = world.players.get(target).map(|c| c.name.clone()).unwrap_or_else(|| format!("{target}"));
                    for (&p, corp) in &world.players {
                        let observe = e.time + pos.distance(corp.command_center) / c;
                        let (sev, text) = if p == *target {
                            if *recalled {
                                (TimelineSeverity::Good, "Authority enforcement RECALLED — your charter is back above the proscription line.".to_string())
                            } else {
                                (TimelineSeverity::Info, "The Authority's enforcement squadron has served its time and is standing down.".to_string())
                            }
                        } else {
                            (TimelineSeverity::Info, format!("Authority bulletin: the enforcement squadron against {who} has stood down."))
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // The galaxy changed: your own claim is instant; a rival's claim is
                // awareness that arrives light-delayed (same gate as the map).
                EventPayload::SystemClaimed { system, owner, pos } => {
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        if p == *owner {
                            self.push(p, e.time, TimelineSeverity::Good, format!("You claimed {name}."));
                        } else {
                            let observe = e.time + pos.distance(corp.command_center) / c;
                            self.push(p, observe, TimelineSeverity::Warn, format!("A rival claimed {name}."));
                        }
                    }
                }
                // §ladder B4: a Titan dying is HEADLINE news — every corp hears
                // it (light-delayed from the wreck; the owner instantly).
                EventPayload::FlagshipDestroyed { owner, name, pos, .. } => {
                    let title = match name {
                        Some(n) => format!("The *{n}* is destroyed."),
                        None => "A Titan is destroyed.".to_string(),
                    };
                    let wreck = *pos;
                    for (&p, corp) in &world.players {
                        if p == *owner {
                            self.push(p, e.time, TimelineSeverity::Bad, format!("{title} Your syndicate's flagship is gone — the yards may lay a new keel."));
                        } else {
                            let observe = e.time + wreck.distance(corp.command_center) / c;
                            self.push(p, observe, TimelineSeverity::Warn, title.clone());
                        }
                    }
                }
                // Construction is your own private administration (§step1) — owner-only,
                // observable instantly; the finished ship reveals as a light-gated ghost.
                EventPayload::BuildStarted { owner, system, what, .. } => {
                    let name = system_name(world, *system);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("Construction started at {name}: {}.", build_label(*what)));
                }
                EventPayload::SystemUpgraded { owner, system, upgrade, tier } => {
                    let name = system_name(world, *system);
                    // §economy: one title-driven line for all 16 structure kinds.
                    let what = format!("{} tier {tier}", upgrade.title());
                    self.push(*owner, e.time, TimelineSeverity::Good, format!("{name} developed — {what}."));
                }
                // A soft-rejected build (§buildings step 1) — owner-only, instant
                // (your own administration): nothing was spent, the request just
                // couldn't be hosted. Tells the player WHY so the fix is obvious.
                EventPayload::BuildRejected { owner, system, what, reason } => {
                    let name = system_name(world, *system);
                    let text = match reason {
                        sim::BuildRejectReason::NoSlot => format!(
                            "Can't build {} at {name}: every development slot is used — systems must specialize.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NeedsShipyard { required } => format!(
                            "Can't build {} at {name}: needs Shipyard tier {required} there.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NotBuildable => format!(
                            "Can't build {} at {name}: it isn't a corporation-buildable hull.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::NeedsResearch => format!(
                            "Can't build {} at {name}: its hull hasn't been researched — complete the Line programme on the Hulls board.",
                            build_label(*what)
                        ),
                        sim::BuildRejectReason::TitanFielded => format!(
                            "Can't build {} at {name}: your syndicate already fields its Titan — one flagship per syndicate (rebuild only after it is lost).",
                            build_label(*what)
                        ),
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, text);
                }
                // A colony ship arrived at an already-claimed system (§ships
                // part 3) — you lost the race (or it flipped en route). OWNER-
                // ONLY, light-delayed from the hold position; the ship is intact.
                EventPayload::ColonyHeld { owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*owner, observe, TimelineSeverity::Warn, format!(
                            "Your colony ship arrived at {name} — already claimed. It is holding position, intact; redirect it to another system."
                        ));
                    }
                }
                // A rival BLOCKADE was established at one of your systems
                // (§contestable-territory). The OWNER learns it light-delayed
                // (a rival arrived at the system — news travels home at c); the
                // BESIEGER learns instantly (their fleet is there).
                EventPayload::BlockadeEstablished { by, owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*owner, observe, TimelineSeverity::Bad, format!(
                            "{name} is under BLOCKADE — a rival fleet holds station; convoys in and out are cut off. Break the blockade (relief, a new defense tier) to restore your supply lines."
                        ));
                    }
                    self.push(*by, e.time, TimelineSeverity::Good, format!(
                        "Your blockade of {name} is established — its logistics are strangled while you hold station."
                    ));
                }
                // A besieged system was CAPTURED (§contestable-territory Part 2).
                // Both participants learn it light-delayed from the flip site: the
                // OLD owner ("you lost X"), the CAPTOR ("you captured X"). Third
                // parties see the ownership change via the light-gated map.
                EventPayload::SystemCaptured { old_owner, new_owner, system, pos, .. } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(old_owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*old_owner, observe, TimelineSeverity::Bad, format!(
                            "You LOST {name} — a besieging colony ship captured it. Its stockpile was plundered and its developments damaged; your fleets survive."
                        ));
                    }
                    if let Some(cc) = world.players.get(new_owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*new_owner, observe, TimelineSeverity::Good, format!(
                            "You CAPTURED {name} — the siege paid off. You inherit its (damaged) developments and plundered stockpile."
                        ));
                    }
                }
                // A blockade at one of your systems lifted (§contestable-territory).
                EventPayload::BlockadeLifted { owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "The blockade of {name} has LIFTED — logistics resume."
                        ));
                    }
                }
                // A scout captured intel (§scout part 2) — OWNER-ONLY, delivered
                // when the capture's light (from the scout's position) reaches
                // the owner's command center: knowledge travels home at c.
                EventPayload::IntelGathered { owner, system, defense_tier, shipyard_tier, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*owner, observe, TimelineSeverity::Info, format!(
                            "Scout report: {name} — Defense ×{defense_tier} · Shipyard ×{shipyard_tier}."
                        ));
                    }
                }
                // §economy Part 4: specialist news — hire/training/delivery on
                // the owner's own clock; a LOSS light-delayed from the wreck
                // (battle-news precedent). All owner-only.
                EventPayload::SpecialistHired { owner, kind, dest } => {
                    let name = system_name(world, *dest);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Contract signed: a {} ships out from Sol for {name} (sub-light — protect the run).",
                        kind.title()
                    ));
                }
                EventPayload::SpecialistTrained { owner, system, kind } => {
                    let name = system_name(world, *system);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "The Academy at {name} graduated a {}.",
                        kind.title()
                    ));
                }
                EventPayload::SpecialistsDelivered { owner, system, manifest } => {
                    let name = system_name(world, *system);
                    let who: Vec<String> = manifest.iter().map(|(k, n)| format!("{}× {}", n, k.title())).collect();
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "Personnel landed at {name}: {}.",
                        who.join(", ")
                    ));
                }
                EventPayload::SpecialistsLost { owner, manifest, pos } => {
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        let who: Vec<String> = manifest.iter().map(|(k, n)| format!("{}× {}", n, k.title())).collect();
                        self.push(*owner, observe, TimelineSeverity::Bad, format!(
                            "Lost with the ship: {}.",
                            who.join(", ")
                        ));
                    }
                }
                // §economy Part 3: a production line STOPPED or RECOVERED —
                // OWNER-ONLY, own clock, transitions only (latched in the sim, so
                // it can never spam). The named cause is the fix-first pointer.
                EventPayload::ProductionSuspended { owner, system, structure, reason } => {
                    let name = system_name(world, *system);
                    use sim::SuspendReason as R;
                    let cause = match reason {
                        R::NoFood => "the colony is out of Provisions — ship food",
                        R::NoInputs => "its input basket ran dry — ship raws in or staff extraction",
                        R::StorageFull => "storage is FULL — ship goods out or build a Depot",
                    };
                    self.push(*owner, e.time, TimelineSeverity::Warn, format!(
                        "{} at {name} SUSPENDED — {cause} (nothing is lost).",
                        structure.title()
                    ));
                }
                EventPayload::ProductionResumed { owner, system, structure } => {
                    let name = system_name(world, *system);
                    self.push(*owner, e.time, TimelineSeverity::Good, format!(
                        "{} at {name} is producing again.",
                        structure.title()
                    ));
                }
                // §economy Part 2: a colony moved on the FOOD LADDER — OWNER-ONLY,
                // on the owner's own clock (own-economy precedent, like stockpiles
                // and FuelShortfall). Transitions only, so it never spams.
                EventPayload::FoodStateChanged { owner, system, state } => {
                    let name = system_name(world, *system);
                    use sim::FoodState as F;
                    let (sev, text) = match state {
                        F::WellSupplied => (TimelineSeverity::Good, format!("{name} is WELL SUPPLIED again — full workforce, growth resumed.")),
                        F::Rationing => (TimelineSeverity::Warn, format!("{name} is RATIONING — workforce slowed, growth paused. Ship Provisions there (nothing is lost).")),
                        F::Critical => (TimelineSeverity::Warn, format!("Food CRITICAL at {name} — workforce at half strength. Ship Provisions there (nothing is lost).")),
                        F::NoProvisions => (TimelineSeverity::Bad, format!("{name} is OUT OF PROVISIONS — industry stalled; the colony endures (nobody dies). Ship Provisions there.")),
                    };
                    self.push(*owner, e.time, sev, text);
                }
                // §syndicates Part 3: an ally GARRISON's supply flipped at a HOST
                // system — OWNER-ONLY (the sender, whose fleet it is), light-delayed
                // from the distant host to their command center (NOT own-economy —
                // the garrison is far away). Transitions only, so no spam.
                EventPayload::GarrisonSupplyChanged { owner, host, fed } => {
                    if let (Some(cc), Some(hpos)) = (
                        world.players.get(owner).map(|c| c.command_center),
                        world.systems.iter().find(|s| s.id == *host).map(|s| s.pos),
                    ) {
                        let name = system_name(world, *host);
                        let observe = e.time + hpos.distance(cc) / c;
                        let (sev, text) = if *fed {
                            (TimelineSeverity::Good, format!("Your garrison at ally {name} is fed again — back on defense."))
                        } else {
                            (TimelineSeverity::Warn, format!("Your garrison at ally {name} is UNFED — its defense is suspended until the host has Provisions (nothing is lost)."))
                        };
                        self.push(*owner, observe, sev, text);
                    }
                }
                // §pirates: a player DESTROYED a pirate enclave — OWNER-ONLY (the
                // victor), light-delayed from the base to their command center.
                EventPayload::PirateEnclaveCleared { owner, system, pos, plunder } => {
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let name = system_name(world, *system);
                        let observe = e.time + pos.distance(cc) / c;
                        let loot: u32 = plunder.values().sum();
                        let tail = if loot > 0 { format!(" — {loot} units of plunder seized") } else { String::new() };
                        self.push(*owner, observe, TimelineSeverity::Good, format!("Pirate enclave at {name} CLEARED{tail}. It will lie dormant, then respawn weaker."));
                    }
                }
                // §node: an EXOTIC system AWAKENED — announced GALAXY-WIDE,
                // light-delayed from the node to each observer's command center (the
                // awakening TIME is public, but news of the event still travels at
                // c). Same delivery as a rival claim.
                EventPayload::NodeAwakened { system, pos, bonus } => {
                    let name = system_name(world, *system);
                    for (&p, corp) in &world.players {
                        let observe = e.time + pos.distance(corp.command_center) / c;
                        self.push(
                            p,
                            observe,
                            TimelineSeverity::Info,
                            format!("EXOTIC NODE AWAKENED at {name} — a {} node is now capturable.", bonus.title()),
                        );
                    }
                }
                // §node EXPOSURE: a node's HOLDER changed — announced GALAXY-WIDE,
                // light-delayed, so every corp learns who now commands it. The holder
                // hears "you now command…"; everyone else hears who took it.
                EventPayload::NodeCaptured { owner, system, pos, bonus } => {
                    let name = system_name(world, *system);
                    let holder = world.players.get(owner).map(|c| c.name.clone()).unwrap_or_else(|| "A rival".into());
                    for (&p, corp) in &world.players {
                        let observe = e.time + pos.distance(corp.command_center) / c;
                        let (sev, text) = if p == *owner {
                            (TimelineSeverity::Good, format!("You now command the {} node at {name}.", bonus.title()))
                        } else {
                            (TimelineSeverity::Warn, format!("{holder} now commands the {} node at {name}.", bonus.title()))
                        };
                        self.push(p, observe, sev, text);
                    }
                }
                // §explore Part 2: a SURVEY completed — OWNER-ONLY, light-delayed
                // from the fleet's position (knowledge travels home at c; the sim
                // inserts into `surveyed` on the same clock, so the notice and the
                // map's new geology land together).
                EventPayload::SurveyCompleted { owner, system, pos } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        self.push(*owner, observe, TimelineSeverity::Good, format!(
                            "Survey of {name} complete — exact geology charted (permanent; allies receive a relayed copy)."
                        ));
                    }
                }
                // §explore Part 3: the system's HIDDEN TRAIT revealed to its (new)
                // owner — the blind claimer's gamble resolving (or capture spoils).
                // OWNER-ONLY, light-delayed from the system.
                EventPayload::TraitRevealed { owner, system, pos, trait_ } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c2| c2.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        let what = match trait_ {
                            sim::explore::SystemTrait::BonusVein { commodity } => {
                                format!("Bonus Vein — its {} deposit runs ×{} richer", commodity.slug(), sim::explore::BONUS_VEIN_MULT)
                            }
                            sim::explore::SystemTrait::DeepDeposits => format!("Deep Deposits — base ×{} richer, but the FIRST Extractor tier is wasted breaking through", sim::explore::DEEP_DEPOSITS_BASE_MULT),
                            sim::explore::SystemTrait::UnstableGeology => format!("Unstable Geology — development costs ×{} here", sim::explore::UNSTABLE_COST_MULT),
                            sim::explore::SystemTrait::VolatilePockets => format!("Volatile Pockets — Refinery output ×{} here", sim::explore::VOLATILE_REFINERY_MULT),
                            sim::explore::SystemTrait::PrecursorCache => format!("Precursor Cache — a one-time {} Alloys deposited to the stockpile", sim::explore::PRECURSOR_ALLOYS),
                        };
                        let sev = if matches!(trait_, sim::explore::SystemTrait::UnstableGeology) {
                            TimelineSeverity::Warn
                        } else {
                            TimelineSeverity::Good
                        };
                        self.push(*owner, observe, sev, format!("TRAIT REVEALED at {name}: {what}."));
                    }
                }
                // §node: a held node's upkeep flipped — OWNER-ONLY, on the owner's own
                // clock (own-economy precedent, like HabitatSupplyChanged). Transitions
                // only, so it never spams.
                EventPayload::NodeSupplyChanged { owner, system, fed } => {
                    let name = system_name(world, *system);
                    let (sev, text) = if *fed {
                        (TimelineSeverity::Good, format!("Node at {name} is fed again — its bonus is live."))
                    } else {
                        (TimelineSeverity::Warn, format!("Node at {name} is UNFED — its bonus is SUSPENDED. Ship its upkeep there (nothing is lost)."))
                    };
                    self.push(*owner, e.time, sev, text);
                }
                // A Defense Platform fought (§buildings step 2c) — OWNER-ONLY
                // detail (tiers lost / result), light-delayed from the battle
                // like any combat news. The attacker's side of the story arrives
                // separately via the ordinary RaidResolved report.
                EventPayload::PlatformEngaged { owner, system, pos, raider_destroyed, driven_off, tiers_lost } => {
                    let name = system_name(world, *system);
                    if let Some(cc) = world.players.get(owner).map(|c| c.command_center) {
                        let observe = e.time + pos.distance(cc) / c;
                        let result = if *raider_destroyed {
                            "destroyed the raider"
                        } else if *driven_off {
                            "drove the raider off"
                        } else {
                            "was fought through"
                        };
                        let damage = if *tiers_lost > 0 {
                            format!(" — {tiers_lost} platform tier(s) lost")
                        } else {
                            String::new()
                        };
                        let sev = if *raider_destroyed || *driven_off { TimelineSeverity::Good } else { TimelineSeverity::Bad };
                        self.push(*owner, observe, sev, format!("Defense Platform at {name} engaged a hostile raider and {result}{damage}."));
                    }
                }
                EventPayload::FuelShortfall { owner, needed, kind } => {
                    self.push(*owner, e.time, TimelineSeverity::Warn,
                        format!("A {} was held — out of fuel (needed ~{:.0}). Stockpile fuel near your fleet.", kind.label(), needed));
                }
                // §order-lifecycle (OWNER-ONLY). "Delivered" is the player's own
                // command data (they computed delivery at issue), shown on their
                // own clock at delivery, with the exact echo countdown. "Confirmed"
                // is genuinely observed — it fires when the echo light arrives, so
                // its time IS the owner's observation time.
                EventPayload::OrderDelivered { owner, fleet, kind, echo_at } => {
                    let name = fleet_label(world, *fleet);
                    let wait = fmt_wait(echo_at - e.time);
                    self.push(*owner, e.time, TimelineSeverity::Info,
                        format!("Order delivered to {name} — {} underway (echo ~{wait}).", kind.label()));
                }
                EventPayload::OrderConfirmed { owner, fleet, kind } => {
                    let name = fleet_label(world, *fleet);
                    self.push(*owner, e.time, TimelineSeverity::Good,
                        format!("{name} confirmed its {} — you can see it complying now.", kind.label()));
                }
                // §research: syndicate-wide institution news — pushed to every
                // member at once (their own private research, like the roster; no
                // light delay). A completed programme's effect is already live.
                EventPayload::ResearchCompleted { syndicate, programme } => {
                    let name = sim::research::programme(programme).map(|p| p.name).unwrap_or("a programme");
                    for &p in members_of(world, *syndicate).iter() {
                        self.push(p, e.time, TimelineSeverity::Good, format!("Research complete: {name} — its effect is live galaxy-wide."));
                    }
                }
                EventPayload::TierUnlocked { syndicate, field, school, tier } => {
                    let where_ = match school {
                        Some(s) => format!("{} · {}", field.title(), s.title()),
                        None => field.title().to_string(),
                    };
                    for &p in members_of(world, *syndicate).iter() {
                        self.push(p, e.time, TimelineSeverity::Info, format!("Tier {tier} unlocked on {where_}."));
                    }
                }
                EventPayload::ResearchStalled { syndicate } => {
                    for &p in members_of(world, *syndicate).iter() {
                        self.push(p, e.time, TimelineSeverity::Warn, "Research stalled — no staffed Academy is contributing. Post crew to an Academy to resume.".to_string());
                    }
                }
                EventPayload::ResearchResumed { syndicate } => {
                    for &p in members_of(world, *syndicate).iter() {
                        self.push(p, e.time, TimelineSeverity::Good, "Research resumed — a staffed Academy is contributing again.".to_string());
                    }
                }
                _ => {}
            }
        }
    }

    fn push(&mut self, player: PlayerId, observe_time: f64, severity: TimelineSeverity, text: String) {
        self.pending.push(Pending { player, observe_time, severity, text, promoted: false });
    }

    /// Promote every entry whose light has now arrived into its player's journal.
    /// Idempotent per entry; bounds each journal to the most recent [`JOURNAL_CAP`].
    pub fn promote(&mut self, now: f64) {
        for p in &mut self.pending {
            if !p.promoted && p.observe_time <= now {
                p.promoted = true;
                let j = self.journal.entry(p.player).or_default();
                j.push_back(TimelineEntry {
                    at_time: p.observe_time,
                    severity: p.severity,
                    text: p.text.clone(),
                });
                while j.len() > JOURNAL_CAP {
                    j.pop_front();
                }
            }
        }
        self.pending
            .retain(|p| !p.promoted && (now - p.observe_time) < MAX_PENDING_AGE);
    }

    /// Note that `player` is currently online at `now` — moves their
    /// "while you were away" boundary forward.
    pub fn mark_seen(&mut self, player: PlayerId, now: f64) {
        self.last_seen.insert(player, now);
    }

    /// The player's current journal plus the sim-time they were last online (the
    /// boundary the client uses to split "while you were away" from earlier).
    pub fn digest(&self, player: PlayerId) -> (Vec<TimelineEntry>, f64) {
        let entries = self
            .journal
            .get(&player)
            .map(|j| j.iter().cloned().collect())
            .unwrap_or_default();
        let away_since = self.last_seen.get(&player).copied().unwrap_or(0.0);
        (entries, away_since)
    }

    /// How many entries the player's journal holds (for cheap change-detection).
    pub fn journal_len(&self, player: PlayerId) -> usize {
        self.journal.get(&player).map(|j| j.len()).unwrap_or(0)
    }
}

fn commodity_name(c: Commodity) -> &'static str {
    // §economy: human names for the timeline prose (the wire uses `slug()`).
    match c {
        Commodity::MetallicOre => "metallic ore",
        Commodity::RareElements => "rare elements",
        Commodity::Silicates => "silicates",
        Commodity::Volatiles => "volatiles",
        Commodity::Biomass => "biomass",
        Commodity::Alloys => "alloys",
        Commodity::Electronics => "electronics",
        Commodity::Polymers => "polymers",
        Commodity::Fuel => "fuel",
        Commodity::Provisions => "provisions",
        Commodity::Machinery => "machinery",
        Commodity::Armaments => "armaments",
    }
}

fn system_name(world: &World, id: sim::EntityId) -> String {
    world
        .systems
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("{id}"))
}

/// §research: the members of a syndicate (for fanning institution news out to the
/// whole roster). Empty if the syndicate is gone.
fn members_of(world: &World, sid: sim::SyndicateId) -> Vec<PlayerId> {
    world
        .syndicates
        .get(&sid)
        .map(|s| s.members.iter().copied().collect())
        .unwrap_or_default()
}

/// A short label for a fleet in the timeline — "your <flagship> fleet".
fn fleet_label(world: &World, id: sim::EntityId) -> String {
    match world.fleets.get(&id) {
        Some(f) => {
            let k = match f.flagship_kind() {
                sim::ShipKind::Convoy => "convoy",
                sim::ShipKind::Raider => "raider",
                sim::ShipKind::Corvette => "corvette",
                sim::ShipKind::Colony => "colony",
                sim::ShipKind::Scout => "scout",
                sim::ShipKind::Freighter => "freighter",
                sim::ShipKind::Destroyer => "destroyer",
                sim::ShipKind::Cruiser => "cruiser",
                sim::ShipKind::Battleship => "battleship",
                sim::ShipKind::Dreadnought => "dreadnought",
                sim::ShipKind::Titan => "titan",
            };
            format!("your {k} fleet")
        }
        None => "your fleet".to_string(),
    }
}

/// An ABSOLUTE sim-time as a mission-clock stamp ("T+7:20") — used for the §TCA
/// freight timetable, whose departure and arrival instants are exact.
fn fmt_clock(t: f64) -> String {
    format!("T+{}", fmt_wait(t))
}

/// Format a wait in seconds as `M:SS` for the echo countdown label.
fn fmt_wait(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Human label for a build job, for the check-in timeline (§step1).
fn build_label(what: sim::BuildKind) -> &'static str {
    match what {
        sim::BuildKind::Ship { ship: sim::ShipKind::Convoy } => "a Convoy",
        sim::BuildKind::Ship { ship: sim::ShipKind::Raider } => "a Raider",
        sim::BuildKind::Ship { ship: sim::ShipKind::Corvette } => "a Corvette",
        sim::BuildKind::Ship { ship: sim::ShipKind::Colony } => "a Colony Ship",
        // §TCA: never appears in a real build event (the Freighter is TCA-only),
        // but the match must be total — a defensive label.
        sim::BuildKind::Ship { ship: sim::ShipKind::Freighter } => "an Authority Freighter",
        sim::BuildKind::Ship { ship: sim::ShipKind::Scout } => "a Scout",
        sim::BuildKind::Ship { ship: sim::ShipKind::Destroyer } => "a Destroyer",
        sim::BuildKind::Ship { ship: sim::ShipKind::Cruiser } => "a Cruiser",
        sim::BuildKind::Ship { ship: sim::ShipKind::Battleship } => "a Battleship",
        sim::BuildKind::Ship { ship: sim::ShipKind::Dreadnought } => "a Dreadnought",
        sim::BuildKind::Ship { ship: sim::ShipKind::Titan } => "a Titan",
        sim::BuildKind::Upgrade { upgrade } => upgrade.title(),
        // §economy Part 4: an Academy course — label by profession.
        sim::BuildKind::Train { specialist } => match specialist {
            sim::SpecialistKind::Geologist => "a Geologist (training)",
            sim::SpecialistKind::PetrochemicalEngineer => "a Petrochemical Engineer (training)",
            sim::SpecialistKind::Xenobiologist => "a Xenobiologist (training)",
            sim::SpecialistKind::IndustrialEngineer => "an Industrial Engineer (training)",
            sim::SpecialistKind::NavalArchitect => "a Naval Architect (training)",
        },
        // §modules Part B3: a module in manufacture.
        sim::BuildKind::Module { module } => match module {
            sim::ModuleKind::MassDriver => "a Mass Driver",
            sim::ModuleKind::TorpedoRack => "a Torpedo Rack",
            sim::ModuleKind::PointDefenseScreen => "a Point-Defense Screen",
            sim::ModuleKind::ReflectivePlating => "Reflective Plating",
            sim::ModuleKind::WhippleArmor => "Whipple Armor",
        },
    }
}

fn kind_word(k: ShipKind) -> &'static str {
    match k {
        ShipKind::Convoy => "convoy",
        ShipKind::Raider => "raider",
        ShipKind::Corvette => "corvette",
        ShipKind::Colony => "colony ship",
        ShipKind::Scout => "scout",
        ShipKind::Freighter => "freighter",
        ShipKind::Destroyer => "destroyer",
        ShipKind::Cruiser => "cruiser",
        ShipKind::Battleship => "battleship",
        ShipKind::Dreadnought => "dreadnought",
        ShipKind::Titan => "titan",
    }
}

/// The check-in line for an economy event the recipient cares about while away,
/// or `None` for the noisy/online-only ones we deliberately skip.
fn trade_entry(te: &TradeEvent, world: &World) -> Option<(TimelineSeverity, String)> {
    use TimelineSeverity::*;
    Some(match *te {
        TradeEvent::AutoDispatched { commodity, units, source, rule_id, .. } => (
            Good,
            format!(
                "Standing order #{rule_id} auto-shipped {units} {} from {} (raidable).",
                commodity_name(commodity),
                system_name(world, source)
            ),
        ),
        TradeEvent::Sold { commodity, units, unit_price, .. } => (
            Good,
            format!("Sold {units} {} at the hub for {unit_price:.2} ea.", commodity_name(commodity)),
        ),
        TradeEvent::Delivered { commodity, units, .. } => (
            Good,
            format!("Delivery arrived: +{units} {}.", commodity_name(commodity)),
        ),
        TradeEvent::LimitFilled { commodity, units, unit_price, side, .. } => {
            let s = format!("{side:?}").to_lowercase();
            (Good, format!("Limit {s} filled: {units} {} @ {unit_price:.2}.", commodity_name(commodity)))
        }
        TradeEvent::SupplyDiverted { commodity, units, system, action, .. } => {
            let name = system_name(world, system);
            let com = commodity_name(commodity);
            match action {
                sim::DivertAction::Lost => (
                    Bad,
                    format!("Supply to {name} lost — you no longer hold it: {units} {com} dropped."),
                ),
                sim::DivertAction::ReturnedHome => (
                    Warn,
                    format!("Supply to {name} re-routed home ({units} {com}) — system lost."),
                ),
                sim::DivertAction::SoldAtHub => (
                    Warn,
                    format!("Supply to {name} re-routed to sell at the hub ({units} {com}) — system lost."),
                ),
            }
        }
        // A full depot bounced part of a delivery onward to the hub (§buildings
        // step 2) — an attention item: the player should ship out or build a Depot.
        TradeEvent::StorageOverflow { commodity, units, system, .. } => {
            let name = system_name(world, system);
            (
                Warn,
                format!(
                    "Depot full at {name}: {units} {} couldn't be stored — re-routed to sell at the hub. Ship goods out or build a Depot.",
                    commodity_name(commodity)
                ),
            )
        }
        // A SOFT-REJECTED Exchange order or freight booking (§9, §TCA) — owner-only,
        // instant, and free: nothing was spent. Names the reason so the fix is obvious.
        TradeEvent::Rejected { commodity, units, system, reason, .. } => {
            let com = commodity_name(commodity);
            let where_ = system.map(|s| system_name(world, s));
            match reason {
                sim::TradeRejectReason::InsufficientWarehouseStock { have } => (
                    Warn,
                    match &where_ {
                        Some(name) => format!(
                            "Can't ship {units} {com} to {name}: your hub warehouse holds {have}."
                        ),
                        None => format!(
                            "Can't sell {units} {com}: your hub warehouse holds {have}. \
                             Ship goods to the hub first (Authority freight or a convoy)."
                        ),
                    },
                ),
                sim::TradeRejectReason::NotYourSystem => (
                    Warn,
                    format!(
                        "Authority freight refused {units} {com}: {} isn't yours. The Charter \
                         Authority serves your own colonies only.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
                sim::TradeRejectReason::InsufficientSystemStock { have } => (
                    Warn,
                    format!(
                        "Can't collect {units} {com} from {}: its stockpile holds {have}.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
                sim::TradeRejectReason::CannotAffordFee { fee } => (
                    Warn,
                    format!("Can't book {units} {com}: the Authority's freight fee is {fee:.0} credits."),
                ),
                sim::TradeRejectReason::FleetUnavailable => (
                    Warn,
                    "That fleet can't handle cargo right now — it must be YOURS, idle, and not in a fight.".to_string(),
                ),
                sim::TradeRejectReason::OutOfLogisticsRange => (
                    Warn,
                    "That fleet is too far from the dock to move cargo — bring it alongside first.".to_string(),
                ),
                sim::TradeRejectReason::NoCargoRoom { capacity } if capacity == 0 => (
                    Warn,
                    format!("That fleet has no cargo hold — only convoys haul goods (tried {units} {com})."),
                ),
                sim::TradeRejectReason::NoCargoRoom { capacity } => (
                    Warn,
                    format!("Not enough hold for {units} {com}: this fleet lifts {capacity} units."),
                ),
                sim::TradeRejectReason::CantAfford { cost } => (
                    Warn,
                    format!("Reinstatement costs {cost:.0} credits — more than your treasury holds."),
                ),
                sim::TradeRejectReason::CharterSuspended => (
                    Bad,
                    format!(
                        "The Authority won't book {units} {com}: your charter is SUSPENDED. \
                         Freight already queued or aboard still completes — pay down your \
                         citations, or haul it yourself."
                    ),
                ),
                sim::TradeRejectReason::CharterRevoked => (
                    Bad,
                    format!(
                        "The Exchange is closed to you: your charter is REVOKED ({units} {com} not traded). \
                         Your warehouse is still yours to fetch from — pay reinstatement to trade again."
                    ),
                ),
                sim::TradeRejectReason::CargoMismatch => (
                    Warn,
                    format!("That fleet is already carrying something else — unload before loading {com}."),
                ),
                sim::TradeRejectReason::DestinationBlockaded => (
                    Warn,
                    format!(
                        "The Authority won't book {units} {com} to {} — it reports the system BLOCKADED. \
                         Break the blockade, or move the goods yourself.",
                        where_.unwrap_or_else(|| "that system".into())
                    ),
                ),
            }
        }
        // §TCA Phase 2: the reinstatement receipt — what it cost, and the band it
        // bought you back into.
        TradeEvent::CharterReinstated { points, cost, before, after, .. } => {
            let from = sim::charter_status(before);
            let to = sim::charter_status(after);
            let crossed = if from != to {
                format!(" — charter reinstated to {}", to.title())
            } else {
                String::new()
            };
            (
                Good,
                format!("Paid the Authority {cost:.0} credits for {points:.0} standing ({before:.0} → {after:.0}){crossed}."),
            )
        }
        // §TCA: the booking receipt — what it cost and when the Authority sails.
        TradeEvent::FreightBooked { commodity, units, system, direction, fee, depart_at, eta, .. } => {
            let name = system_name(world, system);
            let (verb, dest) = match direction {
                sim::ShipmentDir::Outbound => ("Booked", format!("to {name}")),
                sim::ShipmentDir::Inbound => ("Booked pickup of", format!("from {name}")),
            };
            (
                Good,
                format!(
                    "{verb} {units} {} {dest} — fee {fee:.0}cr, departs {}, arrives {}.",
                    commodity_name(commodity),
                    fmt_clock(depart_at),
                    fmt_clock(eta)
                ),
            )
        }
        // §TCA: freight progress. Only the outcomes a player would want in an
        // away-digest; the routine "departed" tick stays out of it.
        TradeEvent::FreightMoved { commodity, units, system, stage, .. } => {
            let name = system_name(world, system);
            let com = commodity_name(commodity);
            match stage {
                sim::FreightStage::Departed => return None,
                sim::FreightStage::CollectedForPickup => return None,
                sim::FreightStage::DeliveredToSystem => {
                    (Good, format!("Authority freight delivered {units} {com} to {name}."))
                }
                sim::FreightStage::ArrivedAtWarehouse => (
                    Good,
                    format!("Authority freight landed {units} {com} from {name} in your hub warehouse."),
                ),
                sim::FreightStage::ReturnedUndeliverable => (
                    Warn,
                    format!(
                        "Authority freight couldn't unload {units} {com} at {name} — it's no longer yours, \
                         or its depot is full. The lot is back in your hub warehouse."
                    ),
                ),
                sim::FreightStage::ForfeitedOnCapture => (
                    Bad,
                    format!("Lost {units} {com} awaiting pickup at {name} — the system fell before the Authority collected it."),
                ),
                sim::FreightStage::LostWithFreighter => (
                    Bad,
                    format!("{units} {com} destroyed with the Authority freighter carrying it (to/from {name})."),
                ),
            }
        }
        // Online-only / low-signal news (manual buys, dispatch-started, resting
        // placements) stays out of the away-digest.
        _ => return None,
    })
}

/// The check-in line for a battle, framed for `me`'s role and the outcome.
fn raid_entry(
    me: PlayerId,
    attacker: PlayerId,
    defender: PlayerId,
    attacker_kind: ShipKind,
    target_kind: ShipKind,
    outcome: RaidOutcome,
) -> (TimelineSeverity, String) {
    use TimelineSeverity::*;
    let i_attack = me == attacker;
    let _ = defender;
    match outcome {
        RaidOutcome::TargetDestroyed => {
            if i_attack {
                (Good, format!("Your raider destroyed an enemy {}.", kind_word(target_kind)))
            } else {
                (Bad, format!("You lost a {} to a hostile raider.", kind_word(target_kind)))
            }
        }
        RaidOutcome::AttackerDestroyed => {
            if i_attack {
                (Bad, format!("Your {} was destroyed in the attack.", kind_word(attacker_kind)))
            } else {
                (Good, "You destroyed an attacking raider.".to_string())
            }
        }
        RaidOutcome::BothDestroyed => (Bad, "Both ships were lost in a clash.".to_string()),
        RaidOutcome::BothSurvive => {
            if i_attack {
                (Info, "Your attack was driven off — no losses.".to_string())
            } else {
                (Good, "You drove off an attacking raider — no losses.".to_string())
            }
        }
        RaidOutcome::Escaped => {
            if i_attack {
                (Info, "Your quarry escaped to the hub.".to_string())
            } else {
                (Good, "Your convoy reached the hub safely.".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{Command, EntityId, SimConfig, Vec2};

    fn world_with_two() -> (World, PlayerId, PlayerId) {
        let mut w = World::new(SimConfig::for_players(7, 4));
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: a, name: "A".into() },
            Command::AddPlayer { id: b, name: "B".into() },
        ]);
        (w, a, b)
    }

    /// §TCA Phase 2: a CITATION is a PUBLIC bulletin from the Charterhouse, and
    /// every player — culprit and bystander alike — learns it LIGHT-DELAYED from
    /// the hub. A distant third party hears about it later than a near one, and
    /// the culprit's own copy reads as an indictment rather than gossip.
    #[test]
    fn citations_are_public_and_reach_each_player_at_lightspeed() {
        let mut w = World::new(SimConfig::for_players(11, 4));
        let (culprit, near, far) = (PlayerId(1), PlayerId(2), PlayerId(3));
        w.step(&[
            Command::AddPlayer { id: culprit, name: "Outlaw".into() },
            Command::AddPlayer { id: near, name: "Near".into() },
            Command::AddPlayer { id: far, name: "Far".into() },
        ]);
        // Put the two bystanders at KNOWN, different distances from the hub.
        let c = w.config.c;
        let hub = w.hub;
        w.players.get_mut(&near).unwrap().command_center = hub + Vec2::new(600.0, 0.0);
        w.players.get_mut(&far).unwrap().command_center = hub + Vec2::new(6000.0, 0.0);

        let mut tl = Timeline::new();
        let issued = w.time;
        tl.ingest(
            &[sim::Event::new(
                issued,
                sim::EventPayload::Citation {
                    culprit,
                    offense: sim::tca::CitationOffense::FreightDestroyed,
                    pos: hub,
                    occurred_at: issued,
                },
            )],
            &w,
        );
        let seen = |tl: &Timeline, who: PlayerId| tl.digest(who).0.len();

        // Nobody has it before its light arrives.
        tl.promote(issued);
        for who in [culprit, near, far] {
            assert_eq!(seen(&tl, who), 0, "no bulletin before its light arrives");
        }
        let near_at = issued + 600.0 / c;
        let far_at = issued + 6000.0 / c;
        assert!(near_at < far_at, "the test geometry must actually differ");

        // The NEAR bystander is informed first, and the bulletin NAMES the culprit.
        tl.promote(near_at + 1e-6);
        assert_eq!(seen(&tl, near), 1, "the near bystander is informed on schedule");
        assert_eq!(seen(&tl, far), 0, "the far bystander is still in the dark");
        let near_text = tl.digest(near).0[0].text.clone();
        assert!(near_text.contains("Outlaw"), "a bystander's bulletin names the culprit: {near_text}");

        // …and the FAR one only once its own light lands.
        tl.promote(far_at + 1e-6);
        assert_eq!(seen(&tl, far), 1, "…until its light arrives");

        // The culprit reads it as an indictment of THEIR corporation.
        let mine = tl.digest(culprit).0;
        assert_eq!(mine.len(), 1);
        assert!(mine[0].text.contains("your corporation"), "the culprit is told plainly: {}", mine[0].text);
    }

    #[test]
    fn own_economy_news_is_observable_immediately() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        let ev = Event::new(
            w.time,
            EventPayload::Trade(TradeEvent::Sold {
                player: a,
                commodity: Commodity::MetallicOre,
                units: 12,
                unit_price: 8.0,
                penalty: 0.0,
            }),
        );
        tl.ingest(&[ev], &w);
        tl.promote(w.time);
        let (entries, _away) = tl.digest(a);
        assert_eq!(entries.len(), 1, "own sale should journal at once");
        assert!(entries[0].text.contains("Sold 12 metallic ore"));
    }

    #[test]
    fn a_rival_claim_arrives_light_delayed() {
        let (w, a, b) = world_with_two();
        let mut tl = Timeline::new();
        // A claim far from b's command center: its light takes time to arrive.
        let cc = w.players[&b].command_center;
        let pos = cc + Vec2::new(3000.0, 0.0); // 10 s of light away at c=300
        let ev = Event::new(w.time, EventPayload::SystemClaimed { system: EntityId(999), owner: a, pos });
        tl.ingest(&[ev], &w);

        // Immediately: the owner knows; the rival does not yet.
        tl.promote(w.time);
        assert_eq!(tl.journal_len(a), 1, "owner learns instantly");
        assert_eq!(tl.journal_len(b), 0, "rival's light hasn't arrived");

        // After the light delay: the rival now sees it (light-respecting awareness).
        tl.promote(w.time + 11.0);
        assert_eq!(tl.journal_len(b), 1, "rival learns after the light arrives");
        let (entries, _) = tl.digest(b);
        assert!(entries[0].text.contains("rival claimed"));
    }

    #[test]
    fn offline_buffering_and_away_boundary() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        // Player A was last online at t = 5.
        tl.mark_seen(a, 5.0);
        // While "away", two automation events occur and become observable.
        for (t, units) in [(10.0_f64, 3u32), (20.0, 4)] {
            let ev = Event::new(
                t,
                EventPayload::Trade(TradeEvent::AutoDispatched {
                    player: a,
                    commodity: Commodity::Fuel,
                    units,
                    source: EntityId(1),
                    rule_id: 1,
                }),
            );
            tl.ingest(&[ev], &w);
        }
        tl.promote(25.0); // time has marched on while A was gone
        let (entries, away_since) = tl.digest(a);
        assert_eq!(entries.len(), 2, "events buffered while offline");
        assert_eq!(away_since, 5.0, "away boundary is the last-online time");
        assert!(entries.iter().all(|e| e.at_time > away_since), "all are 'while you were away'");
    }

    #[test]
    fn journal_is_bounded() {
        let (w, a, _b) = world_with_two();
        let mut tl = Timeline::new();
        for i in 0..(JOURNAL_CAP as u32 + 15) {
            let ev = Event::new(
                i as f64,
                EventPayload::Trade(TradeEvent::Delivered {
                    player: a,
                    commodity: Commodity::MetallicOre,
                    units: i + 1,
                    system: None,
                }),
            );
            tl.ingest(&[ev], &w);
        }
        tl.promote(1000.0);
        assert_eq!(tl.journal_len(a), JOURNAL_CAP, "journal keeps only the most recent cap");
    }
}
