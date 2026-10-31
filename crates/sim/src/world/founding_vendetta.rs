//! §vendetta (Oct 25 2026, `docs/pirate-vendetta.md`): the charter's finale.
//! The Rogue Privateer was a pirate family's nephew. His uncle swears revenge
//! once mining spreads to a second world (step 16). §vendetta-campaign (Oct 30
//! 2026): he then strikes the founder's expanded mining, a raider at the second
//! mine and two at the barge on the den's belt; when the founder's ore is home
//! (step 21) he comes for the colony with his flagship, four escorts and the
//! strike raiders that got away. Each countdown runs only while the founder is
//! online, with a deadline regardless. Over the colony he attacks its
//! strongest warship, and the founder's other warships at rest in the home's
//! well join the fight, as does the Gun Platform (§gun-platform); with no
//! warship left he assaults the platform itself. His flagship's death pays a
//! bounty and lands its salvage at home; a fight the colony's defense survives
//! sends him off (the charter completes either way). If nothing is left
//! fighting for the colony when he has loaded, it falls: game over until the
//! founder restarts (Peter, Oct 30 2026). His force never changes. Bots skip
//! it.
use super::*;
use crate::founding::{FoundingRaid, FoundingVendetta, RaidPhase, StrikeKind, VendettaBeat, VendettaStrike};

/// How the finale ends (§gun-platform).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// His flagship died.
    Won,
    /// A fight the colony's defense survived sent him off.
    Repelled,
    /// Nothing was left fighting for the colony: game over.
    Fallen,
}

impl World {
    /// The pirate family this corporation crossed (`founding::PirateFamily`).
    pub fn pirate_family(&self, owner: PlayerId) -> Option<crate::founding::PirateFamily> {
        let corp = self.players.get(&owner)?;
        Some(crate::founding::PirateFamily::of(self.config.seed, owner, corp.joined_tick))
    }

    /// The uncle's oath, once per charter. He broadcasts it, so it reaches
    /// command at once; the guide counts from it. §vendetta-campaign: it sets
    /// his first strike, on the second mine.
    pub(super) fn swear_vendetta(&mut self, owner: PlayerId, out: &mut Vec<Event>) {
        let now = self.time;
        let mine = self.vendetta_mine_target(owner);
        let Some(corp) = self.players.get_mut(&owner) else {
            return;
        };
        if owner.is_bot() || !corp.founding.local_opening || corp.founding.vendetta.is_some() {
            return;
        }
        let at = now + crate::founding::STRIKE_MINE_DELAY_S;
        corp.founding.vendetta = Some(FoundingVendetta {
            sworn_at: now,
            fleet: None,
            depart_at: None,
            raid: None,
            last_pos: None,
            fought: false,
            won: None,
            report_at: None,
            paid: false,
            deadline_at: None,
            strikes: mine.map(|(system, body)| VendettaStrike::new(StrikeKind::Mine, system, body, at)).into_iter().collect(),
            strike_survivors: 0,
            fallen: false,
            assaulted: false,
        });
        out.push(Event::new(now, EventPayload::Vendetta {
            owner, beat: VendettaBeat::Sworn, pos: corp.command_center, stolen: BTreeMap::new(), wreck: None }));
    }

    /// Step 19, once a tick: raise his pack, fly his raid and settle how it
    /// ends. True once the report of the outcome has reached command and a
    /// win is paid.
    pub(super) fn advance_vendetta(&mut self, owner: PlayerId, out: &mut Vec<Event>) -> bool {
        use crate::founding as F;
        self.swear_vendetta(owner, out);
        let Some(v) = self.players.get(&owner).and_then(|c| c.founding.vendetta.clone()) else {
            return true;
        };
        let pack = v.fleet.filter(|id| self.fleets.contains_key(id));
        let engaged = pack.is_some_and(|id| self.vendetta_engaged(id));
        let Some((home_id, home)) = self.players[&owner].home_system
            .and_then(|id| self.systems.iter().find(|s| s.id == id).map(|s| (id, s.pos)))
        else {
            return true;
        };
        if let Some(at) = v.report_at {
            // Settled: his survivors jump out once clear of the fight, and of
            // the colony's sensors until the report is in.
            if let Some(id) = pack
                && !engaged
                && (self.vendetta_clear(id, home) || self.time >= at)
            {
                self.fleets.remove(&id);
            }
            // §gun-platform: the colony fell; the charter waits for a restart.
            if v.fallen || at > self.time || pack.is_some_and(|id| self.fleets.contains_key(&id)) {
                return false;
            }
            if v.won == Some(true) && !v.paid {
                self.pay_vendetta(owner, home_id, out);
            }
            return true;
        }
        let Some(id) = v.fleet else {
            self.raise_vendetta_pack(owner, home);
            return false;
        };
        let pos = pack.map(|id| self.fleets[&id].pos).or(v.last_pos).unwrap_or(home);
        if !pack.is_some_and(|id| self.fleets[&id].count(F::VENDETTA_FLAGSHIP) > 0) {
            self.settle_vendetta(owner, Outcome::Won, pos, out);
            return false;
        }
        self.with_vendetta(owner, |v| v.last_pos = Some(pos));
        if engaged {
            self.with_vendetta(owner, |v| v.fought = true);
            return false;
        }
        // Waiting beyond the colony's sensors, then setting out (never at a
        // founder on leave).
        let Some(raid) = v.raid.clone() else {
            // A fight where he waits doesn't count against the raid to come.
            self.with_vendetta(owner, |v| v.fought = false);
            // §vendetta-campaign: his countdown runs only while the founder is
            // online, but he sets out by his deadline regardless.
            if !self.online.contains(&owner) && let Some(deadline) = v.deadline_at {
                self.with_vendetta(owner, |v| if let Some(at) = v.depart_at.as_mut() { *at = (*at + DT).min(deadline) });
            }
            let depart_at = self.players[&owner].founding.vendetta.as_ref().and_then(|v| v.depart_at);
            if depart_at.is_some_and(|at| self.time >= at) && !self.players[&owner].leave.active(self.time) {
                self.steer_raid_at_colony(id, home_id);
                self.with_vendetta(owner, |v| v.raid = Some(FoundingRaid { phase: RaidPhase::Inbound,
                    base: pos, lurk: pos, arrived_at: None, allowance: BTreeMap::new(),
                    stolen: BTreeMap::new(), loading_credit: 0.0 }));
            }
            return false;
        };
        // The founder's warships near home: his targets, and once a fight
        // ends with any of them alive he breaks off. One fight, never a loop.
        // §gun-platform: the station counts too, while a hardpoint fires.
        let reach = self.vendetta_reach();
        let mut defenders: Vec<(f64, EntityId)> = self.fleets.iter()
            .filter(|(_, f)| f.owner == owner && f.is_combatant() && f.pos.distance(home) <= reach)
            .map(|(fid, f)| (f.attack_power(), *fid))
            .collect();
        defenders.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let station = self.station_live_at(Some(home_id)) > 0;
        if v.fought {
            self.with_vendetta(owner, |v| v.fought = false);
            if (!defenders.is_empty() || station) && raid.phase != RaidPhase::Returning {
                self.break_off_vendetta(owner, id);
                return false;
            }
        }
        match raid.phase {
            RaidPhase::Inbound => {
                // §raid-course: for the colony's world, resuming the course a
                // fight on the way in took from him.
                if self.steer_raid_at_colony(id, home_id) {
                    // Over the colony: a raid's share of what it holds now.
                    let sys = self.systems.iter().find(|s| s.id == home_id).unwrap();
                    let reserve = raid_reserve(sys);
                    let allowance = sys.stockpile.iter()
                        .map(|(&c, &n)| (c, (n * pirate::HOME_RAID_STOCK_FRAC).min(raid_exposed(sys, &reserve, c)).max(0.0).floor() as u32))
                        .filter(|(_, n)| *n > 0)
                        .collect();
                    let now = self.time;
                    self.with_vendetta(owner, |v| {
                        let r = v.raid.as_mut().unwrap();
                        r.phase = RaidPhase::Looting;
                        r.arrived_at = Some(now);
                        r.allowance = allowance;
                    });
                    out.push(Event::new(now, EventPayload::Vendetta {
                        owner, beat: VendettaBeat::Arrival, pos, stolen: BTreeMap::new(), wreck: None }));
                }
            }
            RaidPhase::Looting => {
                let fleet = self.fleets.get_mut(&id).unwrap();
                if let Some(&(_, target)) = defenders.first() {
                    // The fight comes to the colony: its strongest warship.
                    fleet.order = FleetOrder::Attack { target };
                    return false;
                }
                if matches!(fleet.order, FleetOrder::Attack { .. }) {
                    fleet.order = FleetOrder::Idle;
                }
                // §gun-platform: no warship left, so the Gun Platform alone.
                if station && !v.assaulted {
                    self.open_vendetta_assault(id, owner, home_id);
                    self.with_vendetta(owner, |v| v.assaulted = true);
                    return false;
                }
                self.load_vendetta_loot(owner, id, home_id);
                let r = self.players[&owner].founding.vendetta.as_ref().unwrap().raid.as_ref().unwrap();
                let sys = self.systems.iter().find(|s| s.id == home_id).unwrap();
                let reserve = raid_reserve(sys);
                let left = r.allowance.iter().any(|(c, n)| *n > 0 && raid_exposed(sys, &reserve, *c) >= 1.0);
                let full = r.stolen.values().sum::<u32>() >= pirate::HOME_RAID_MAX_LOOT;
                let held = r.arrived_at.is_some_and(|at| self.time - at >= F::VENDETTA_HOLD_S);
                if !left || full || held {
                    if station {
                        self.break_off_vendetta(owner, id);
                    } else {
                        // Nothing fights for the colony: it falls.
                        self.settle_vendetta(owner, Outcome::Fallen, pos, out);
                    }
                }
            }
            RaidPhase::Returning => {
                // Clear of the colony's sensors: repelled, he got away.
                if self.vendetta_clear(id, home) {
                    self.settle_vendetta(owner, Outcome::Repelled, pos, out);
                } else if !matches!(self.fleets[&id].order, FleetOrder::MoveTo { .. }) {
                    self.fleets.get_mut(&id).unwrap().order = FleetOrder::MoveTo { dest: raid.base };
                }
            }
            RaidPhase::Lurking => {}
        }
        false
    }

    /// Where his pack waits and fights: just beyond the colony's sensors.
    fn vendetta_reach(&self) -> f64 {
        self.config.sensor_range * crate::founding::RAID_START_SENSOR_RANGES
    }

    /// Back where it waited, or at the edge of the colony's sensors.
    fn vendetta_clear(&self, fleet: EntityId, home: Vec2) -> bool {
        self.fleets.get(&fleet).is_none_or(|f| f.pos.distance(home) >= self.vendetta_reach() * 0.95)
    }

    fn vendetta_engaged(&self, fleet: EntityId) -> bool {
        self.engagements.values().any(|e| e.attackers.contains(&fleet) || e.defenders.contains(&fleet))
    }

    fn with_vendetta(&mut self, owner: PlayerId, edit: impl FnOnce(&mut FoundingVendetta)) {
        if let Some(v) = self.players.get_mut(&owner).and_then(|c| c.founding.vendetta.as_mut()) {
            edit(v);
        }
    }

    /// His flagship and escorts, beyond the colony's sensors on the side of
    /// his nephew's den (away from the Market Hub without one). They set out
    /// after [`crate::founding::VENDETTA_LEAD_S`].
    fn raise_vendetta_pack(&mut self, owner: PlayerId, home: Vec2) {
        use crate::founding as F;
        let den = self.players[&owner].founding.den
            .and_then(|d| self.systems.iter().find(|s| s.id == d))
            .map(|s| s.pos)
            .filter(|p| p.distance(home) > 1.0);
        let outward = if home.distance(self.hub) > 1.0 { (home - self.hub).normalized() } else { Vec2::new(1.0, 0.0) };
        let start = home + den.map_or(outward, |d| (d - home).normalized()) * self.vendetta_reach();
        let id = self.alloc_entity_id();
        // §vendetta-campaign: his strike raiders that got away ride with him.
        let survivors = self.players[&owner].founding.vendetta.as_ref().map_or(0, |v| v.strike_survivors);
        let mut pack = Fleet::single(id, PlayerId::PIRATE, F::VENDETTA_FLAGSHIP, start, FleetOrder::Idle, None);
        pack.add(ShipKind::Raider, F::VENDETTA_ESCORTS + survivors);
        pack.fill_stores();
        pack.vendetta_pirate = true;
        self.fleets.insert(id, pack);
        let depart_at = self.time + F::VENDETTA_LEAD_S;
        let deadline_at = self.time + F::VENDETTA_DEADLINE_S;
        self.with_vendetta(owner, |v| {
            v.fleet = Some(id);
            v.depart_at = Some(depart_at);
            v.deadline_at = Some(deadline_at);
            // A strike still waiting to set out is called off: he comes himself.
            for s in v.strikes.iter_mut().filter(|s| s.fleet.is_none()) {
                s.done = true;
            }
        });
    }

    // ─── §vendetta-campaign: his strikes on the expanded mining ──────────────

    /// The second mine's world: another world of the home system with a
    /// Mining Complex, or a belt's quarry.
    fn vendetta_mine_target(&self, owner: PlayerId) -> Option<(EntityId, u32)> {
        use crate::build::StructureKind as K;
        let home = self.players.get(&owner)?.home_system?;
        let sys = self.systems.iter().find(|s| s.id == home)?;
        sys.bodies.iter()
            .find(|b| b.tier(K::CommandCenter) == 0 && (b.tier(K::MiningComplex) > 0 || b.tier(K::AsteroidQuarry) > 0))
            .map(|b| (home, b.id))
    }

    /// The founder's barge at work on `system`'s belt.
    fn vendetta_barge(&self, owner: PlayerId, system: EntityId) -> Option<EntityId> {
        self.fleets.iter()
            .filter(|(_, f)| f.owner == owner && f.contains(ShipKind::MiningBarge))
            .find(|(_, f)| matches!(&f.industry, Some(crate::industry::FleetIndustry::Mine { system: s, status, .. })
                if *s == system && status == "mining"))
            .map(|(id, _)| *id)
    }

    /// §gun-platform: a freighter of the founder's in open space within
    /// [`crate::founding::STRIKE_FREIGHTER_SU`] of home, out of every well and
    /// not docked: the first strike's prey when there is one (the nearest).
    fn vendetta_freighter(&self, owner: PlayerId) -> Option<EntityId> {
        let home = self.players.get(&owner)?.home;
        self.fleets.iter()
            .filter(|(id, f)| f.owner == owner && f.flagship_kind().is_freighter_hull()
                && f.pos.distance(home) <= crate::founding::STRIKE_FREIGHTER_SU
                && self.hunted_freighter_exposed(**id))
            .min_by(|a, b| a.1.pos.distance(home).total_cmp(&b.1.pos.distance(home)).then(a.0.cmp(b.0)))
            .map(|(id, _)| *id)
    }

    /// Still in open space: out of every gravity well and not docked.
    fn hunted_freighter_exposed(&self, fleet: EntityId) -> bool {
        self.fleets.get(&fleet).is_some_and(|f| self.well_system(f.pos).is_none() && self.dock_of(fleet).is_none())
    }

    fn with_strike(&mut self, owner: PlayerId, i: usize, edit: impl FnOnce(&mut VendettaStrike)) {
        self.with_vendetta(owner, |v| if let Some(s) = v.strikes.get_mut(i) { edit(s) });
    }

    /// Once a tick from step 16: set the barge strike once the barge cuts the
    /// charter's belt, and fly each strike.
    pub(super) fn advance_vendetta_strikes(&mut self, owner: PlayerId, out: &mut Vec<Event>) {
        use crate::founding::FoundingStage as S;
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        let Some(v) = corp.founding.vendetta.as_ref() else {
            return;
        };
        if matches!(corp.founding.stage, S::MineBelt | S::HaulOre)
            && !v.strikes.iter().any(|s| s.kind == StrikeKind::Barge)
            && let Some((system, body)) = corp.founding.belt_target
            && self.vendetta_barge(owner, system).is_some()
        {
            let at = self.time + crate::founding::STRIKE_BARGE_DELAY_S;
            self.with_vendetta(owner, |v| v.strikes.push(VendettaStrike::new(StrikeKind::Barge, system, body, at)));
        }
        let n = self.players[&owner].founding.vendetta.as_ref().map_or(0, |v| v.strikes.len());
        for i in 0..n {
            self.advance_strike(owner, i, out);
        }
    }

    fn advance_strike(&mut self, owner: PlayerId, i: usize, out: &mut Vec<Event>) {
        use crate::founding as F;
        let Some(s) = self.players[&owner].founding.vendetta.as_ref().and_then(|v| v.strikes.get(i)).cloned() else {
            return;
        };
        if s.done {
            return;
        }
        let now = self.time;
        let Some(fleet) = s.fleet else {
            // Waiting to set out: like his, its countdown runs only while the
            // founder is online, and never past its deadline.
            let at = if self.online.contains(&owner) { s.at } else { (s.at + DT).min(s.deadline) };
            self.with_strike(owner, i, |s| s.at = at);
            if now >= at {
                self.launch_strike(owner, i, out);
            }
            return;
        };
        if let Some(at) = s.killed_report_at {
            if now >= at {
                self.pay_strike(owner, i, out);
            }
            return;
        }
        let Some(raiders) = self.fleets.get(&fleet).map(|f| (f.pos, f.count(ShipKind::Raider))) else {
            // All dead: the bounty rides the report home.
            let cc = self.players[&owner].command_center;
            let at = now + crate::transit::delay(s.last_pos.unwrap_or(cc), cc, self.config.c);
            self.with_strike(owner, i, |s| s.killed_report_at = Some(at));
            return;
        };
        let (pos, count) = raiders;
        self.with_strike(owner, i, |s| s.last_pos = Some(pos));
        if self.vendetta_engaged(fleet) {
            self.with_strike(owner, i, |s| s.fought = true);
            return;
        }
        let base = s.base.unwrap_or(pos);
        let Some(sys) = self.systems.iter().find(|x| x.id == s.system) else {
            return;
        };
        let target_star = sys.pos;
        if s.leaving {
            let clear = match s.kind {
                StrikeKind::Mine => self.vendetta_clear(fleet, target_star),
                StrikeKind::Barge | StrikeKind::Freighter => pos.distance(base) <= 100.0
                    || pos.distance(target_star) >= F::STRIKE_BARGE_START_SU * 0.95
                        && (s.kind == StrikeKind::Barge || self.vendetta_clear(fleet, target_star)),
            };
            if clear || pos.distance(base) < 100.0 {
                // Home with what they took: they ride with him at the end.
                self.fleets.remove(&fleet);
                self.with_vendetta(owner, |v| {
                    v.strike_survivors += count;
                    v.strikes[i].done = true;
                });
            } else if !matches!(self.fleets[&fleet].order, FleetOrder::MoveTo { .. }) {
                self.fleets.get_mut(&fleet).unwrap().order = FleetOrder::MoveTo { dest: base };
            }
            return;
        }
        if s.fought {
            // A fight ended with them alive: they break off.
            self.break_off_strike(owner, i, fleet, base);
            return;
        }
        match s.kind {
            StrikeKind::Mine => {
                let Some(world) = sys.bodies.iter().any(|b| b.id == s.body).then(|| sys.body_pos(s.body, now)) else {
                    self.break_off_strike(owner, i, fleet, base);
                    return;
                };
                // For the world, then riding it while over it.
                self.fleets.get_mut(&fleet).unwrap().order = FleetOrder::MoveTo { dest: world };
                match s.over_since {
                    None if pos.distance(world) <= F::RAID_OVER_COLONY_SU => {
                        self.with_strike(owner, i, |s| {
                            s.over_since = Some(now);
                            s.next_wreck_at = Some(now + F::STRIKE_WRECK_S);
                        });
                        out.push(Event::new(now, EventPayload::Vendetta {
                            owner, beat: VendettaBeat::MineStrike, pos, stolen: BTreeMap::new(), wreck: None }));
                    }
                    None => {}
                    // §gun-platform: unopposed, one structure every
                    // STRIKE_WRECK_S; none left, it leaves.
                    Some(_) if s.next_wreck_at.is_some_and(|at| now >= at) => {
                        if self.wreck_strike_world(owner, i, pos, out) {
                            self.with_strike(owner, i, |s| s.next_wreck_at = Some(now + F::STRIKE_WRECK_S));
                        } else {
                            self.break_off_strike(owner, i, fleet, base);
                        }
                    }
                    Some(_) => {}
                }
            }
            StrikeKind::Freighter => {
                // §gun-platform: after the freighter while it is in open space;
                // docked or in a well, it is safe.
                match s.target.filter(|t| self.hunted_freighter_exposed(*t)) {
                    Some(target) => self.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Attack { target },
                    None => self.break_off_strike(owner, i, fleet, base),
                }
            }
            StrikeKind::Barge => {
                // At the barge while it works the belt (or flees within reach).
                let barge = self.fleets.iter()
                    .filter(|(_, f)| f.owner == owner && f.contains(ShipKind::MiningBarge)
                        && f.pos.distance(target_star) <= F::STRIKE_BARGE_START_SU * 5.0)
                    .min_by(|a, b| a.1.pos.distance(pos).total_cmp(&b.1.pos.distance(pos)).then(a.0.cmp(b.0)))
                    .map(|(id, _)| *id);
                match barge {
                    Some(target) => self.fleets.get_mut(&fleet).unwrap().order = FleetOrder::Attack { target },
                    None => self.break_off_strike(owner, i, fleet, base),
                }
            }
        }
    }

    /// The strike's raiders set out: one for the second mine's world from
    /// beyond the colony's sensors, two for the barge from beyond the belt.
    fn launch_strike(&mut self, owner: PlayerId, i: usize, out: &mut Vec<Event>) {
        use crate::founding as F;
        let s = self.players[&owner].founding.vendetta.as_ref().unwrap().strikes[i].clone();
        let now = self.time;
        let Some(sys) = self.systems.iter().find(|x| x.id == s.system) else {
            self.with_strike(owner, i, |s| s.done = true);
            return;
        };
        let home = self.players[&owner].home;
        // §gun-platform: a freighter in open space near home takes the first
        // strike instead of the second mine's world.
        if s.kind == StrikeKind::Mine && let Some(target) = self.vendetta_freighter(owner) {
            self.with_strike(owner, i, |s| {
                s.kind = StrikeKind::Freighter;
                s.target = Some(target);
            });
            return self.launch_strike(owner, i, out);
        }
        let (start, raiders, order) = match s.kind {
            StrikeKind::Freighter => {
                // From beyond its sensors, ahead of it (or on the side away
                // from home while it sits still).
                let Some((at, vel)) = s.target.and_then(|t| self.fleets.get(&t)).map(|f| (f.pos, f.vel)) else {
                    self.with_strike(owner, i, |s| s.done = true);
                    return;
                };
                let ahead = if vel.length() > 1e-6 { vel.normalized() }
                    else if at.distance(home) > 1.0 { (at - home).normalized() } else { Vec2::new(1.0, 0.0) };
                (at + ahead * F::STRIKE_BARGE_START_SU, F::STRIKE_MINE_RAIDERS, FleetOrder::Attack { target: s.target.unwrap() })
            }
            StrikeKind::Mine => {
                let Some(world) = sys.bodies.iter().any(|b| b.id == s.body).then(|| sys.body_pos(s.body, now)) else {
                    self.with_strike(owner, i, |s| s.done = true);
                    return;
                };
                let den = self.players[&owner].founding.den
                    .and_then(|d| self.systems.iter().find(|x| x.id == d))
                    .map(|x| x.pos)
                    .filter(|p| p.distance(home) > 1.0);
                let outward = if home.distance(self.hub) > 1.0 { (home - self.hub).normalized() } else { Vec2::new(1.0, 0.0) };
                let start = sys.pos + den.map_or(outward, |d| (d - home).normalized()) * self.vendetta_reach();
                (start, F::STRIKE_MINE_RAIDERS, FleetOrder::MoveTo { dest: world })
            }
            StrikeKind::Barge => {
                let Some(barge) = self.vendetta_barge(owner, s.system) else {
                    // Not at work there now: it waits, and is called off at its deadline.
                    if now >= s.deadline {
                        self.with_strike(owner, i, |s| s.done = true);
                    }
                    return;
                };
                let away = if sys.pos.distance(home) > 1.0 { (sys.pos - home).normalized() } else { Vec2::new(1.0, 0.0) };
                (sys.pos + away * F::STRIKE_BARGE_START_SU, F::STRIKE_BARGE_RAIDERS, FleetOrder::Attack { target: barge })
            }
        };
        let id = self.alloc_entity_id();
        let mut pack = Fleet::single(id, PlayerId::PIRATE, ShipKind::Raider, start, order, None);
        if raiders > 1 {
            pack.add(ShipKind::Raider, raiders - 1);
        }
        pack.fill_stores();
        pack.vendetta_pirate = true;
        self.fleets.insert(id, pack);
        self.with_strike(owner, i, |s| {
            s.fleet = Some(id);
            s.base = Some(start);
            s.last_pos = Some(start);
        });
        let beat = match self.players[&owner].founding.vendetta.as_ref().unwrap().strikes[i].kind {
            StrikeKind::Barge => Some(VendettaBeat::BargeStrike),
            StrikeKind::Freighter => Some(VendettaBeat::FreighterStrike),
            StrikeKind::Mine => None,
        };
        if let Some(beat) = beat {
            out.push(Event::new(now, EventPayload::Vendetta { owner, beat, pos: start, stolen: BTreeMap::new(), wreck: None }));
        }
    }

    /// §gun-platform: over the second mine's world, wreck its next structure
    /// in [`crate::founding::STRIKE_WRECKS`] order. False when none is left.
    fn wreck_strike_world(&mut self, owner: PlayerId, i: usize, pos: Vec2, out: &mut Vec<Event>) -> bool {
        let s = self.players[&owner].founding.vendetta.as_ref().unwrap().strikes[i].clone();
        let Some(sys) = self.systems.iter_mut().find(|x| x.id == s.system) else {
            return false;
        };
        let Some(body) = sys.bodies.iter_mut().find(|b| b.id == s.body) else {
            return false;
        };
        let Some(structure) = crate::founding::STRIKE_WRECKS.into_iter().find(|k| body.tier(*k) > 0) else {
            return false;
        };
        body.set_tier(structure, 0);
        body.assignments.remove(&structure);
        // Work under way on it goes with it.
        let upgrade = crate::build::BuildKind::Upgrade { upgrade: structure };
        self.build_queue.retain(|j| !(j.owner == owner && j.system == s.system && j.body_id == s.body && j.what == upgrade));
        self.with_strike(owner, i, |st| st.wrecked.push(structure));
        let wreck = crate::founding::Wreck { system: s.system, body: s.body, structure };
        out.push(Event::new(self.time, EventPayload::Vendetta {
            owner, beat: VendettaBeat::Wrecked, pos, stolen: BTreeMap::new(), wreck: Some(wreck) }));
        true
    }

    fn break_off_strike(&mut self, owner: PlayerId, i: usize, fleet: EntityId, base: Vec2) {
        if let Some(pack) = self.fleets.get_mut(&fleet) {
            pack.order = FleetOrder::MoveTo { dest: base };
        }
        self.with_strike(owner, i, |s| {
            s.leaving = true;
            s.fought = false;
        });
    }

    /// Every raider of a strike destroyed: the bounty, on the report.
    fn pay_strike(&mut self, owner: PlayerId, i: usize, out: &mut Vec<Event>) {
        use crate::founding as F;
        let kind = self.players[&owner].founding.vendetta.as_ref().unwrap().strikes[i].kind;
        let who = self.pirate_family(owner).map_or_else(|| "The warlord".to_string(), |p| p.uncle_name());
        let title = match kind {
            StrikeKind::Mine | StrikeKind::Freighter => format!("{who}'s raider destroyed · bounty"),
            StrikeKind::Barge => format!("{who}'s raiders destroyed · bounty"),
        };
        let corp = self.players.get_mut(&owner).unwrap();
        corp.credits += F::STRIKE_BOUNTY;
        self.with_strike(owner, i, |s| s.done = true);
        out.push(Event::new(self.time, EventPayload::CharterStipend {
            owner, title, credits: F::STRIKE_BOUNTY, standing: 0.0, dossier: None }));
    }

    /// Load a tick's worth of his allowance, the dearest good first, never
    /// what the Vault keeps out of reach (§vault).
    fn load_vendetta_loot(&mut self, owner: PlayerId, fleet: EntityId, home: EntityId) {
        let Some(mut exposed) = self.systems.iter().find(|s| s.id == home).map(|sys| {
            let reserve = raid_reserve(sys);
            sys.stockpile.keys().map(|c| (*c, raid_exposed(sys, &reserve, *c))).collect::<BTreeMap<_, _>>()
        }) else {
            return;
        };
        let Some(raid) = self.players.get_mut(&owner)
            .and_then(|c| c.founding.vendetta.as_mut())
            .and_then(|v| v.raid.as_mut())
        else {
            return;
        };
        let Some(sys) = self.systems.iter_mut().find(|s| s.id == home) else {
            return;
        };
        raid.loading_credit += crate::founding::RAID_LOOT_PER_SEC * DT;
        while raid.loading_credit >= 1.0 && raid.stolen.values().sum::<u32>() < pirate::HOME_RAID_MAX_LOOT {
            let good = raid.allowance.iter()
                .filter(|(c, n)| **n > 0 && exposed.get(c).copied().unwrap_or(0.0) >= 1.0
                    && sys.stockpile.get(c).copied().unwrap_or(0.0) >= 1.0)
                .max_by(|a, b| crate::market::base_price(*a.0).total_cmp(&crate::market::base_price(*b.0))
                    .then(b.0.cmp(a.0)))
                .map(|(c, _)| *c);
            let Some(good) = good else {
                break;
            };
            *sys.stockpile.get_mut(&good).unwrap() -= 1.0;
            *exposed.get_mut(&good).unwrap() -= 1.0;
            *raid.allowance.get_mut(&good).unwrap() -= 1;
            *raid.stolen.entry(good).or_default() += 1;
            raid.loading_credit -= 1.0;
            if let Some(pack) = self.fleets.get_mut(&fleet) {
                pack.add_cargo(good, 1);
            }
        }
        raid.loading_credit = raid.loading_credit.min(1.0);
    }

    /// He breaks off for the edge of the colony's sensors with what he holds.
    fn break_off_vendetta(&mut self, owner: PlayerId, fleet: EntityId) {
        let base = self.players[&owner].founding.vendetta.as_ref()
            .and_then(|v| v.raid.as_ref()).map(|r| r.base);
        let Some(base) = base else {
            return;
        };
        if let Some(pack) = self.fleets.get_mut(&fleet) {
            pack.order = FleetOrder::MoveTo { dest: base };
        }
        self.with_vendetta(owner, |v| v.raid.as_mut().unwrap().phase = RaidPhase::Returning);
    }

    /// §gun-platform: with no warship near home, his pack assaults the
    /// colony's station: a battle against its standing hardpoints alone, which
    /// the founder's warships at rest in the well join as usual.
    fn open_vendetta_assault(&mut self, pack: EntityId, owner: PlayerId, home: EntityId) {
        let Some(pos) = self.fleets.get(&pack).map(|f| f.pos) else {
            return;
        };
        let id = self.alloc_engagement_id();
        let a_start = self.side_comp(&[pack]);
        let a_start_strength = crate::combat::Forces::from_fleet(&a_start, &BTreeMap::new()).strength();
        let a_start_loadouts = self.side_loadouts(&[pack]);
        self.engagements.insert(id, Engagement {
            id,
            pos,
            started_at: self.time,
            raid: false,
            a_owner: PlayerId::PIRATE,
            d_owner: owner,
            attackers: vec![pack],
            defenders: Vec::new(),
            platform_system: Some(home),
            attacker_platform_system: None,
            a_start,
            d_start: BTreeMap::new(),
            a_start_loadouts,
            d_start_loadouts: BTreeMap::new(),
            a_start_strength,
            d_start_strength: 0.0,
            platform_start_tiers: 0,
            attacker_platform_start_tiers: 0,
            a_lead: pack,
            d_lead: pack,
            disengaging: BTreeMap::new(),
            a_fled: false,
            d_fled: false,
            departed: Default::default(),
            touched: true,
            platform_live: [None; 2],
            blockade_run: None,
            station_mounts: Default::default(),
            wrecked_modules: Default::default(),
            tactical: None,
        });
    }

    /// How it ended, on its light to command.
    fn settle_vendetta(&mut self, owner: PlayerId, outcome: Outcome, pos: Vec2, out: &mut Vec<Event>) {
        let cc = self.players[&owner].command_center;
        let report_at = self.time + crate::transit::delay(pos, cc, self.config.c);
        let Some(v) = self.players.get_mut(&owner).and_then(|c| c.founding.vendetta.as_mut()) else {
            return;
        };
        let won = outcome == Outcome::Won;
        v.won = Some(won);
        v.fallen = outcome == Outcome::Fallen;
        v.report_at = Some(report_at);
        let stolen = if won { BTreeMap::new() } else { v.raid.as_ref().map(|r| r.stolen.clone()).unwrap_or_default() };
        let base = v.raid.as_ref().map(|r| r.base);
        let fleet = v.fleet;
        if let Some(pack) = fleet.and_then(|id| self.fleets.get_mut(&id))
            && let Some(dest) = base
        {
            pack.order = FleetOrder::MoveTo { dest };
        }
        let beat = match outcome {
            Outcome::Won => VendettaBeat::Won,
            Outcome::Repelled => VendettaBeat::Lost,
            Outcome::Fallen => VendettaBeat::Fallen,
        };
        out.push(Event::new(self.time, EventPayload::Vendetta { owner, beat, pos, stolen, wreck: None }));
    }

    /// A win's bounty, and his flagship's salvage landed at home (as storage
    /// allows).
    fn pay_vendetta(&mut self, owner: PlayerId, home: EntityId, out: &mut Vec<Event>) {
        use crate::founding as F;
        let title = self.pirate_family(owner)
            .map_or_else(|| "Pirate warlord destroyed · bounty".to_string(), |p| format!("{} destroyed · bounty", p.uncle_name()));
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == home) {
            for (good, units) in F::VENDETTA_SALVAGE {
                let landed = (units as f64).min(sys.storage_headroom()).max(0.0).floor();
                *sys.stockpile.entry(good).or_insert(0.0) += landed;
            }
        }
        let corp = self.players.get_mut(&owner).unwrap();
        corp.credits += F::VENDETTA_BOUNTY;
        corp.founding.vendetta.as_mut().unwrap().paid = true;
        out.push(Event::new(self.time, EventPayload::CharterStipend {
            owner, title, credits: F::VENDETTA_BOUNTY, standing: 0.0, dossier: None }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::founding::FoundingStage as S;

    fn founder(seed: u64, owner: PlayerId) -> (World, EntityId, Vec2) {
        let mut w = World::new(SimConfig::for_players(seed, 4));
        w.step(&[Command::AddPlayer { id: owner, name: "Avenged".into() }]);
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.enabled = true;
        f.local_opening = true;
        let home = w.players[&owner].home_system.unwrap();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        (w, home, pos)
    }

    fn interceptors(w: &mut World, owner: PlayerId, at: Vec2, n: usize) -> Vec<EntityId> {
        (0..n).map(|_| {
            let id = w.alloc_entity_id();
            let mut f = Fleet::single(id, owner, ShipKind::Raider, at, FleetOrder::Idle, None);
            f.fill_stores();
            w.fleets.insert(id, f);
            id
        }).collect()
    }

    /// §gun-platform: the home's Gun Platform (station level 1) over the
    /// Command Center's world, its hardpoint whole.
    fn gun_platform(w: &mut World, home: EntityId) {
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let cc = sys.bodies.iter().find(|b| b.tier(crate::build::StructureKind::CommandCenter) > 0).unwrap().id;
        sys.bodies.iter_mut().find(|b| b.id == cc).unwrap().set_tier(crate::build::StructureKind::OrbitalDefense, 1);
        sys.sync_mounts();
        assert_eq!(sys.live_mounts(), 1);
    }

    /// Step 22 to its end (at most `secs`), skipping his wait beyond the
    /// colony's sensors. The founder is online.
    fn finale(w: &mut World, owner: PlayerId, secs: f64) -> Vec<Event> {
        w.online.insert(owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Vendetta, w.time);
        let mut events = Vec::new();
        for _ in 0..(secs * crate::config::TICK_HZ as f64) as usize {
            events.extend(w.step(&[]));
            let now = w.time;
            if let Some(v) = w.players.get_mut(&owner).unwrap().founding.vendetta.as_mut()
                && v.depart_at.is_some_and(|at| at > now)
            {
                v.depart_at = Some(now);
            }
            if w.players[&owner].founding.stage == S::Complete || w.players[&owner].founding.fallen(w.time) {
                break;
            }
        }
        events
    }

    fn beats(events: &[Event], owner: PlayerId) -> Vec<VendettaBeat> {
        events.iter().filter_map(|e| match &e.payload {
            EventPayload::Vendetta { owner: o, beat, .. } if *o == owner => Some(*beat),
            _ => None,
        }).collect()
    }

    #[test]
    fn the_uncle_swears_as_the_reach_chapter_opens() {
        let owner = PlayerId(91_001);
        let (mut w, _, _) = founder(12648430, owner);
        let f = &mut w.players.get_mut(&owner).unwrap().founding;
        f.set_stage(S::MineBelt, 0.0);
        f.belt_report_at = Some(0.0);
        let events = w.step(&[]);
        let f = &w.players[&owner].founding;
        assert_eq!(f.stage, S::HaulOre);
        assert!(f.vendetta.as_ref().is_some_and(|v| v.fleet.is_none()), "sworn, not yet raised");
        assert_eq!(beats(&events, owner), vec![VendettaBeat::Sworn]);
        let events = w.step(&[]);
        assert!(beats(&events, owner).is_empty(), "once");
    }

    #[test]
    fn with_the_ore_home_and_the_stores_secured_players_face_him_and_bots_skip_him() {
        for (owner, finale) in [(PlayerId(91_002), S::Vendetta), (PlayerId(PlayerId::BOT_BASE | 7), S::Complete)] {
            let (mut w, home, _) = founder(12648430, owner);
            let f = &mut w.players.get_mut(&owner).unwrap().founding;
            f.set_stage(S::HaulOre, 0.0);
            f.haul_report_at = Some(0.0);
            w.step(&[]);
            assert_eq!(w.players[&owner].founding.stage, S::SecureStores, "{owner:?}: §charter-crates, then the stores");
            let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            let land = sys.bodies.iter().find(|b| b.tier(crate::build::StructureKind::CommandCenter) > 0).unwrap().id;
            let body = sys.bodies.iter_mut().find(|b| b.id == land).unwrap();
            body.set_tier(crate::build::StructureKind::Warehouse, 1);
            body.set_tier(crate::build::StructureKind::Vault, 1);
            w.step(&[]);
            assert_eq!(w.players[&owner].founding.stage, finale, "{owner:?}");
        }
    }

    /// §gun-platform (Peter, Oct 30 2026: "if the player hasn't built enough
    /// defenses, game over and they can restart"): with nothing fighting for
    /// it, he takes a raid's share and the colony falls. The charter stops.
    #[test]
    fn with_nothing_fighting_for_it_the_colony_falls_and_the_charter_stops() {
        let owner = PlayerId(91_003);
        let (mut w, home, home_pos) = founder(12648430, owner);
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(crate::cargo::Commodity::Alloys, 300.0);
        let before = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.clone();
        let events = finale(&mut w, owner, 900.0);
        let f = &w.players[&owner].founding;
        assert!(f.fallen(w.time), "game over");
        assert_eq!(f.stage, S::Vendetta, "the charter stops");
        let v = f.vendetta.as_ref().unwrap();
        assert_eq!((v.won, v.fallen), (Some(false), true));
        assert_eq!(beats(&events, owner), vec![VendettaBeat::Sworn, VendettaBeat::Arrival, VendettaBeat::Fallen]);
        let start = v.raid.as_ref().unwrap().base;
        assert!(start.distance(home_pos) > w.config.sensor_range, "he set out unseen");
        let stolen = &v.raid.as_ref().unwrap().stolen;
        assert!(stolen.values().sum::<u32>() > 0 && stolen.values().sum::<u32>() <= pirate::HOME_RAID_MAX_LOOT);
        let after = &w.systems.iter().find(|s| s.id == home).unwrap().stockpile;
        for (good, units) in stolen {
            assert!(*units as f64 <= before[good] * pirate::HOME_RAID_STOCK_FRAC + 1e-9, "{good:?}: a tenth at most");
            if *good == crate::cargo::Commodity::Provisions {
                assert!(after[good] >= RATIONS_UNITS.min(before[good]) - 1e-9, "the settlers keep their rations");
            }
        }
        assert!(!v.paid && w.players[&owner].credits < crate::founding::VENDETTA_BOUNTY, "no bounty");
        // A fallen colony gives no orders but its restart.
        w.step(&[Command::BuildShip { player_id: owner, system_id: home, ship_kind: ShipKind::Raider,
            join: None, loadout: Default::default() }]);
        assert!(w.build_queue.iter().all(|j| j.owner != owner), "refused");
    }

    /// §gun-platform: with no warship near home he assaults the Gun Platform
    /// itself; alone it can't hold, and the colony falls.
    #[test]
    fn with_only_the_gun_platform_he_assaults_it_and_the_colony_falls() {
        let owner = PlayerId(91_010);
        let (mut w, home, _) = founder(12648430, owner);
        gun_platform(&mut w, home);
        let events = finale(&mut w, owner, 1_200.0);
        let v = w.players[&owner].founding.vendetta.clone().unwrap();
        assert!(v.assaulted, "he went for the platform");
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::RaidResolved { defender, .. } if *defender == owner)),
            "a battle under the gun");
        assert!(v.fallen && w.players[&owner].founding.fallen(w.time), "one salvaged gun isn't enough");
    }

    /// The sizing (`VENDETTA_DAMAGE_MULT` 0.28, no captains aboard) for his
    /// flagship and four escorts (§vendetta-campaign), fought at home under
    /// the Gun Platform (§gun-platform; his force never changes): three
    /// Interceptors kill him in at least five galaxies of six and the colony
    /// never falls; with one, it falls in at least two.
    #[test]
    fn under_the_gun_platform_three_interceptors_kill_him_and_one_often_falls() {
        let mut wins = [0usize; 4];
        let mut falls = [0usize; 4];
        for seed in [12648430, 7, 41, 99, 123, 5150] {
            for n in [1usize, 3] {
                let owner = PlayerId(91_100 + n as u64);
                let (mut w, home, home_pos) = founder(seed, owner);
                gun_platform(&mut w, home);
                interceptors(&mut w, owner, home_pos, n);
                let credits = w.players[&owner].credits;
                let alloys = w.systems.iter().find(|s| s.id == home).unwrap().stockpile.get(&crate::cargo::Commodity::Alloys).copied().unwrap_or(0.0);
                let events = finale(&mut w, owner, 1_800.0);
                let f = &w.players[&owner].founding;
                let v = f.vendetta.as_ref().unwrap();
                let won = v.won.expect("settled");
                wins[n] += won as usize;
                falls[n] += v.fallen as usize;
                let last = if won { VendettaBeat::Won } else if v.fallen { VendettaBeat::Fallen } else { VendettaBeat::Lost };
                assert_eq!(beats(&events, owner).last(), Some(&last), "seed {seed}, {n} Interceptors");
                if v.fallen {
                    assert_eq!(f.stage, S::Vendetta, "seed {seed}: game over stops the charter");
                } else {
                    assert_eq!(f.stage, S::Complete, "seed {seed}, {n} Interceptors: it ends");
                }
                if won {
                    let fulfilled = crate::founding::charter_stipend(S::Complete).unwrap().1;
                    assert_eq!(w.players[&owner].credits, credits + crate::founding::VENDETTA_BOUNTY + fulfilled, "the bounty");
                    let now = w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&crate::cargo::Commodity::Alloys];
                    assert!(now > alloys, "the salvage lands at home");
                    assert!(!w.fleets.values().any(|f| f.vendetta_pirate), "seed {seed}: no pirate of his lingers");
                }
            }
        }
        assert!(wins[3] >= 5 && falls[3] == 0, "three Interceptors: {} wins, {} falls of 6", wins[3], falls[3]);
        assert!(falls[1] >= 2, "one Interceptor: the colony falls in {} of 6", falls[1]);
    }

    /// §vendetta-campaign (Peter: online if possible, never "strictly for
    /// online players"): his countdown waits while the founder is away, but
    /// he sets out by his deadline regardless.
    #[test]
    fn his_countdown_runs_while_the_founder_is_online_and_his_deadline_regardless() {
        let owner = PlayerId(91_004);
        let (mut w, _, _) = founder(12648430, owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Vendetta, w.time);
        w.step(&[]);
        let v = w.players[&owner].founding.vendetta.clone().unwrap();
        let (depart, deadline) = (v.depart_at.unwrap(), v.deadline_at.unwrap());
        assert!((deadline - depart - (crate::founding::VENDETTA_DEADLINE_S - crate::founding::VENDETTA_LEAD_S)).abs() < 1.0);
        let at = |w: &World| w.players[&owner].founding.vendetta.as_ref().unwrap().depart_at.unwrap();
        for _ in 0..60 * crate::config::TICK_HZ { w.step(&[]); }
        assert!(at(&w) >= depart + 59.0, "away, his countdown waits");
        w.online.insert(owner);
        w.step(&[]);
        let online_at = at(&w);
        for _ in 0..30 * crate::config::TICK_HZ { w.step(&[]); }
        assert_eq!(at(&w), online_at, "online, it runs");
        w.online.remove(&owner);
        w.time = deadline;
        w.step(&[]);
        assert!(w.players[&owner].founding.vendetta.as_ref().unwrap().raid.is_some(), "his deadline: he sets out regardless");
    }

    /// A home whose second world runs a staffed, powered mine, holding ore.
    fn expanded(w: &mut World, home: EntityId) -> u32 {
        use crate::build::StructureKind as K;
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        sys.stockpile.insert(crate::cargo::Commodity::MetallicOre, 200.0);
        let b = sys.bodies.iter_mut()
            .find(|b| b.tier(K::CommandCenter) == 0 && b.deposits.iter().any(|d| d.resource == crate::cargo::Commodity::MetallicOre))
            .unwrap();
        b.structures.insert(K::MiningComplex, 1);
        b.structures.insert(K::PowerPlant, 1);
        b.assignments.insert(K::MiningComplex, crate::production::Assignment::crew(1));
        b.id
    }

    fn strike(w: &World, owner: PlayerId, i: usize) -> VendettaStrike {
        w.players[&owner].founding.vendetta.as_ref().unwrap().strikes[i].clone()
    }

    /// §vendetta-campaign (Peter: "the uncle should harass the player's
    /// expanded mining ops"): sworn as mining spreads, his raider strikes the
    /// second mine. §gun-platform (Peter: stolen ore is no revenge for a
    /// nephew): unopposed it wrecks the mine, then the Solar Array, steals
    /// nothing, and gets away to ride with him.
    #[test]
    fn a_raider_wrecks_the_second_mine_then_its_solar_array_and_rides_with_him_after() {
        use crate::build::StructureKind as K;
        let owner = PlayerId(91_005);
        let (mut w, home, _) = founder(12648430, owner);
        let body = expanded(&mut w, home);
        w.online.insert(owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::ExpandMining, w.time);
        let events = w.step(&[]);
        assert_eq!(w.players[&owner].founding.stage, S::SurveyCandidates);
        assert_eq!(beats(&events, owner), vec![VendettaBeat::Sworn], "sworn as mining spreads (step 17)");
        let s = strike(&w, owner, 0);
        assert_eq!((s.kind, s.system, s.body), (StrikeKind::Mine, home, body));
        assert!(s.at > w.time + 170.0, "a few minutes after his oath");
        w.with_strike(owner, 0, |s| s.at = 0.0);
        w.step(&[]);
        let s = strike(&w, owner, 0);
        assert_eq!(s.kind, StrikeKind::Mine, "no freighter out: the second world");
        let fleet = s.fleet.expect("set out");
        assert_eq!(w.fleets[&fleet].count(ShipKind::Raider), crate::founding::STRIKE_MINE_RAIDERS);
        assert!(w.fleets[&fleet].pos.distance(w.players[&owner].home) > w.config.sensor_range, "from beyond the colony's sensors");
        // Over the world (put there; the flight is the founding raid's).
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        w.fleets.get_mut(&fleet).unwrap().pos = sys.body_pos(body, w.time);
        let events = w.step(&[]);
        assert_eq!(beats(&events, owner), vec![VendettaBeat::MineStrike]);
        let tier = |w: &World, k: K| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().find(|b| b.id == body).unwrap().tier(k);
        let ore = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&crate::cargo::Commodity::MetallicOre];
        let before = ore(&w);
        let mut wrecks = Vec::new();
        for _ in 0..(crate::founding::STRIKE_WRECK_S * 2.0 + 2.0) as usize * crate::config::TICK_HZ as usize {
            for e in w.step(&[]) {
                if let EventPayload::Vendetta { owner: o, beat: VendettaBeat::Wrecked, wreck: Some(wreck), .. } = e.payload
                    && o == owner { wrecks.push(wreck.structure); }
            }
        }
        assert_eq!(wrecks, vec![K::MiningComplex, K::PowerPlant], "the mine first, then the Solar Array");
        assert_eq!((tier(&w, K::MiningComplex), tier(&w, K::PowerPlant)), (0, 0), "both gone");
        assert!(ore(&w) >= before - 1e-9, "nothing stolen");
        assert!(w.fleets[&fleet].cargo.is_none(), "his hold stays empty");
        for _ in 0..(crate::founding::STRIKE_WRECK_S + 1.0) as usize * crate::config::TICK_HZ as usize { w.step(&[]); }
        assert!(strike(&w, owner, 0).leaving, "nothing left to wreck: it leaves");
        let base = strike(&w, owner, 0).base.unwrap();
        w.fleets.get_mut(&fleet).unwrap().pos = base;
        w.step(&[]);
        assert!(!w.fleets.contains_key(&fleet) && strike(&w, owner, 0).done);
        assert_eq!(w.players[&owner].founding.vendetta.as_ref().unwrap().strike_survivors, 1, "it rides with him at the end");
    }

    /// §gun-platform (Peter: "the first strike be either on the player's
    /// freighter if it is nearby or ..."): a freighter in open space near home
    /// takes the strike instead; reaching a gravity well saves it.
    #[test]
    fn a_freighter_in_open_space_near_home_takes_the_first_strike_until_it_reaches_a_well() {
        let owner = PlayerId(91_009);
        let (mut w, home, home_pos) = founder(12648430, owner);
        let body = expanded(&mut w, home);
        let away = |w: &mut World, at: Vec2| {
            let id = w.alloc_entity_id();
            w.fleets.insert(id, Fleet::single(id, owner, ShipKind::TinyFreighter, at, FleetOrder::Idle, None));
            id
        };
        let far = away(&mut w, home_pos + Vec2::new(crate::founding::STRIKE_FREIGHTER_SU + 5_000.0, 0.0));
        let near = away(&mut w, home_pos + Vec2::new(0.0, 12_000.0));
        assert_eq!(w.vendetta_freighter(owner), Some(near), "the one in reach");
        w.online.insert(owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::ExpandMining, w.time);
        w.step(&[]);
        w.with_strike(owner, 0, |s| s.at = 0.0);
        let events = w.step(&[]);
        assert_eq!(beats(&events, owner), vec![VendettaBeat::FreighterStrike]);
        let s = strike(&w, owner, 0);
        assert_eq!((s.kind, s.target, s.body), (StrikeKind::Freighter, Some(near), body));
        let fleet = s.fleet.unwrap();
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::Attack { target } if target == near));
        assert!(w.fleets[&fleet].pos.distance(w.fleets[&near].pos) >= crate::founding::STRIKE_BARGE_START_SU - 1.0);
        // Home: in the well, it is safe.
        w.fleets.get_mut(&near).unwrap().pos = home_pos;
        w.step(&[]);
        assert!(strike(&w, owner, 0).leaving, "it breaks off");
        assert!(w.fleets.contains_key(&far));
    }

    #[test]
    fn destroying_a_strike_pays_its_bounty_on_the_report() {
        let owner = PlayerId(91_006);
        let (mut w, home, _) = founder(12648430, owner);
        expanded(&mut w, home);
        w.online.insert(owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::ExpandMining, w.time);
        w.step(&[]);
        w.with_strike(owner, 0, |s| s.at = 0.0);
        w.step(&[]);
        let fleet = strike(&w, owner, 0).fleet.unwrap();
        let credits = w.players[&owner].credits;
        w.fleets.remove(&fleet); // destroyed
        w.step(&[]);
        let report = strike(&w, owner, 0).killed_report_at.expect("its loss travels home");
        assert!(report > w.time && w.players[&owner].credits == credits, "not before the report");
        w.time = report;
        let events = w.step(&[]);
        assert_eq!(w.players[&owner].credits, credits + crate::founding::STRIKE_BOUNTY);
        assert!(strike(&w, owner, 0).done);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::CharterStipend { title, .. } if title.ends_with("raider destroyed · bounty"))));
    }

    #[test]
    fn a_strike_waits_while_the_founder_is_away_but_never_past_its_hour() {
        let owner = PlayerId(91_007);
        let (mut w, home, _) = founder(12648430, owner);
        expanded(&mut w, home);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::ExpandMining, w.time);
        w.step(&[]);
        let s = strike(&w, owner, 0);
        for _ in 0..60 * crate::config::TICK_HZ { w.step(&[]); }
        assert!(strike(&w, owner, 0).at >= s.at + 59.0, "away, it waits");
        w.time = s.deadline;
        w.step(&[]);
        assert!(strike(&w, owner, 0).fleet.is_some(), "its hour: it strikes regardless");
    }

    /// §vendetta-campaign: his raiders jump the barge while it cuts the den's
    /// belt; strike raiders that got away ride with him at the end.
    #[test]
    fn two_raiders_jump_the_barge_at_work_and_survivors_join_his_pack() {
        let owner = PlayerId(91_008);
        let (mut w, _, _) = founder(12648430, owner);
        let den = w.founding_den_system(owner).unwrap();
        let belt = w.systems.iter().find(|s| s.id == den).unwrap().bodies[0].id;
        let belt_pos = w.systems.iter().find(|s| s.id == den).unwrap().pos;
        w.online.insert(owner);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::MineBelt, w.time);
        w.players.get_mut(&owner).unwrap().founding.belt_target = Some((den, belt));
        w.swear_vendetta(owner, &mut Vec::new());
        // The pocket found, as step 18 does.
        w.systems.iter_mut().find(|s| s.id == den).unwrap().bodies[0].pockets[0].found_by.insert(owner);
        let barge = w.alloc_entity_id();
        let mut f = Fleet::single(barge, owner, ShipKind::MiningBarge, belt_pos, FleetOrder::Idle, None);
        f.industry = Some(crate::industry::FleetIndustry::Mine { system: den, body: belt, ore: crate::founding::DEN_POCKET_ORE,
            pocket: true, to_station: false, mined: 0.0, fraction: 0.0, status: "mining".into() });
        w.fleets.insert(barge, f);
        w.park_at(barge, den, belt); // §real-coordinates: at work on the belt
        w.step(&[]);
        let i = w.players[&owner].founding.vendetta.as_ref().unwrap().strikes.iter()
            .position(|s| s.kind == StrikeKind::Barge).expect("set as the barge cuts the belt");
        w.with_strike(owner, i, |s| s.at = 0.0);
        let events = w.step(&[]);
        assert!(beats(&events, owner).contains(&VendettaBeat::BargeStrike));
        let fleet = strike(&w, owner, i).fleet.expect("they set out");
        assert_eq!(w.fleets[&fleet].count(ShipKind::Raider), crate::founding::STRIKE_BARGE_RAIDERS);
        assert!(matches!(w.fleets[&fleet].order, FleetOrder::Attack { target } if target == barge));
        // Their survivors ride with him.
        w.with_vendetta(owner, |v| v.strike_survivors = 2);
        w.players.get_mut(&owner).unwrap().founding.set_stage(S::Vendetta, w.time);
        w.step(&[]);
        let pack = w.players[&owner].founding.vendetta.as_ref().unwrap().fleet.unwrap();
        assert_eq!(w.fleets[&pack].count(ShipKind::Raider), crate::founding::VENDETTA_ESCORTS + 2);
    }

    /// Sizing sweep (not a check): `cargo test -p sim --lib vendetta_sizing_sweep -- --ignored --nocapture`.
    /// §gun-platform: the fight at home, under the Gun Platform.
    #[test]
    #[ignore]
    fn vendetta_sizing_sweep() {
        // §charter-crates: step 22 puts a Vault under the stores he loots.
        for (n, vault) in [0usize, 1, 2, 3, 4].into_iter().flat_map(|n| [(n, false), (n, true)]) {
            let mut line = format!("{n} Interceptors + Gun Platform{}:", if vault { " + Vault" } else { "" });
            for seed in [12648430u64, 7, 41, 99, 123, 5150] {
                let owner = PlayerId(91_200 + n as u64);
                let (mut w, home, home_pos) = founder(seed, owner);
                gun_platform(&mut w, home);
                let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
                sys.stockpile.insert(crate::cargo::Commodity::TitaniumOre, 120.0);
                if vault {
                    let cc = sys.bodies.iter().find(|b| b.tier(crate::build::StructureKind::CommandCenter) > 0).unwrap().id;
                    sys.bodies.iter_mut().find(|b| b.id == cc).unwrap().set_tier(crate::build::StructureKind::Vault, 1);
                }
                let ours = interceptors(&mut w, owner, home_pos, n);
                let events = finale(&mut w, owner, 1_800.0);
                let v = w.players[&owner].founding.vendetta.clone().unwrap();
                let left = ours.iter().filter(|id| w.fleets.contains_key(id)).count();
                let battles = events.iter().filter(|e| matches!(&e.payload, EventPayload::RaidResolved { attacker, defender, .. }
                    if *attacker == owner || *defender == owner)).count();
                let gun = w.systems.iter().find(|s| s.id == home).unwrap().live_mounts();
                line += &format!(" [{} left {left}/{n} gun {gun} battles {battles}]",
                    match (v.won, v.fallen) { (Some(true), _) => "WON", (_, true) => "FELL", (Some(false), _) => "repelled", _ => "open" });
            }
            println!("{line}");
        }
    }

    #[test]
    fn the_family_is_dealt_from_the_galaxy_the_corporation_and_its_join() {
        let a = crate::founding::PirateFamily::of(1, PlayerId(5), 100);
        assert_eq!(a, crate::founding::PirateFamily::of(1, PlayerId(5), 100), "stable");
        assert_ne!(a.nephew, a.brother, "the brother is someone else");
        let families: std::collections::BTreeSet<_> = (0..200u64)
            .map(|i| crate::founding::PirateFamily::of(1, PlayerId(i), 100).family).collect();
        assert_eq!(families.len(), crate::founding::PIRATE_FAMILIES.len(), "every family is dealt");
        assert!(a.nephew_name().ends_with(a.surname()) && a.uncle_name().ends_with(a.surname()));
    }
}
