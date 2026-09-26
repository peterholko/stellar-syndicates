//! Deliberate campaigns from reported intelligence, with a home/escort reserve.
//! A strength estimate is not a guaranteed win. Unknown contacts are scouted,
//! losses are learned through ghosts, and withdrawals are ordinary delayed orders.
use super::*;
use sim::diplomacy::RelationState;

#[derive(Clone)]
pub(super) struct MilitarySite {
    pub id: EntityId, pub pos: Vec2, pub owner: Option<PlayerId>,
    pub intel: sim::IntelSnapshot, pub blockade: Option<sim::galaxy::Blockade>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) enum Target { Fleet(EntityId), System(EntityId) }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Campaign {
    pub target: Target, pub leader: EntityId, pub since: f64, pub deadline: f64,
}

pub(super) fn composition(f: &GhostView) -> BTreeMap<H, u32> {
    f.composition.iter().flatten().map(|s| (s.kind, s.count)).collect()
}

pub(super) fn strength(f: &GhostView) -> Option<f64> {
    f.composition.as_ref()?;
    Some(sim::combat::Forces::from_fleet(&composition(f), &BTreeMap::new()).strength()
        * (1.0 - f.damage.unwrap_or(0.0)).clamp(0.0, 1.0))
}

impl Bot {
    pub(super) fn campaign_fleet(&self, id: EntityId) -> bool {
        self.campaign.as_ref().is_some_and(|c| c.leader == id)
    }

    pub(super) fn home_guard(&self, p: &Observation) -> Option<EntityId> {
        // Never merge or send every surviving warship away on an optional raid.
        p.own().filter(|f| f.kind.is_combatant() && f.guard_target.is_none()
            && !self.campaign_fleet(f.id)).min_by(|a, b|
                a.pos.distance(p.cc).total_cmp(&b.pos.distance(p.cc)).then(a.id.cmp(&b.id))).map(|f| f.id)
    }

    fn ready_warship(&self, p: &Observation, f: &GhostView) -> bool {
        f.kind.is_combatant() && f.supplied && !f.stalled && f.guard_target.is_none()
            && !p.fighting.contains(&f.id) && f.damage.unwrap_or(1.0) <= 0.15
            && !self.returns.contains_key(&f.id) && self.ready(&format!("fleet:{}", f.id), p.now)
            && f.jump_spool.is_none() && f.jump_presumed.is_none()
    }

    pub(super) fn recover_military(&mut self, p: &Observation) -> Option<Command> {
        for f in p.own().filter(|f| f.kind.is_combatant()) {
            let key = format!("fleet:{}", f.id);
            if !self.ready(&key, p.now) { continue; }
            if p.fighting.contains(&f.id) {
                if f.damage.unwrap_or(0.0) > 0.5 {
                    self.wait(key, p, f.pos);
                    self.status = "withdrawing a damaged formation on received battle evidence".into();
                    return Some(Command::Withdraw { player_id: p.owner, fleet_id: f.id });
                }
                continue;
            }
            if f.damage.unwrap_or(0.0) > 0.2 {
                let yard = p.sites.iter().filter(|s| s.system.tier(K::OrdnanceFoundry) > 0
                    && p.enough_fuel(f, s.system.pos)).min_by(|a, b|
                        f.pos.distance(a.system.pos).total_cmp(&f.pos.distance(b.system.pos)));
                if let Some(yard) = yard {
                    if f.docked.as_deref() == Some(yard.system.id.to_string().as_str()) { continue; }
                    if self.returns.contains_key(&f.id) { continue; }
                    self.wait(key, p, f.pos);
                    self.returns.insert(f.id, ReturnTrip { destination: yard.system.pos, issued_at: p.now });
                    self.status = "returning for paid foundry repairs".into();
                    return Some(Command::MoveShip { player_id: p.owner, ship_id: f.id, dest: yard.system.pos });
                }
            }
            if f.damage.unwrap_or(0.0) > 0.2 || f.fuel.unwrap_or(0.0) < p.tank(f) * 0.3 {
                if let Some(cmd) = self.return_for_fuel(p, f) { return Some(cmd); }
            }
        }
        None
    }

    pub(super) fn officers(&mut self, p: &Observation) -> Option<Command> {
        for captain in &p.captains {
            if captain.missing() || captain.recover_at.is_some() { continue; }
            let key = format!("officer:{}", captain.id);
            if !self.ready(&key, p.now) { continue; }
            if captain.unspent > 0 && (captain.stationed_system == Some(p.capital)
                || captain.assigned_fleet.is_some_and(|id| p.own().any(|f| f.id == id && f.docked.as_deref() == Some(p.capital.to_string().as_str())))) {
                use sim::captain::CaptainAttribute as A;
                let options = if self.priorities().military > 1.2 {
                    [(A::Command, captain.attributes.command), (A::Navigation, captain.attributes.navigation),
                        (A::Logistics, captain.attributes.logistics), (A::Fieldcraft, captain.attributes.fieldcraft)]
                } else { [(A::Logistics, captain.attributes.logistics), (A::Fieldcraft, captain.attributes.fieldcraft),
                    (A::Navigation, captain.attributes.navigation), (A::Command, captain.attributes.command)] };
                if let Some((attribute, _)) = options.into_iter().find(|(_, n)| *n < sim::captain::CAPTAIN_ATTRIBUTE_MAX) {
                    self.wait(key, p, p.cc);
                    return Some(Command::TrainCaptain { player_id: p.owner, captain_id: captain.id, attribute });
                }
            }
            if captain.available_in_reserve() {
                if let Some(f) = p.own().filter(|f| f.captain.is_none() && f.kind != H::Transport && f.vel.length_sq() < 1.0
                    && (!f.kind.is_combatant() || self.campaign_fleet(f.id) || f.guard_target.is_some()
                        || f.defend_system.is_some() || composition(f).values().sum::<u32>() > 1)
                    && captain.stationed_system.is_some_and(|id| f.docked.as_deref() == Some(id.to_string().as_str()))
                    && captain.can_command(&composition(f)))
                    .max_by_key(|f| (f.kind.is_combatant(), f.kind == H::Scout)) {
                    self.wait(key, p, f.pos); self.wait(format!("fleet:{}", f.id), p, f.pos);
                    return Some(Command::AssignCaptain { player_id: p.owner, captain_id: captain.id, fleet_id: f.id });
                }
            }
        }
        let free = p.captains.iter().filter(|c| c.available_in_reserve() && !c.missing()).count();
        let uncommanded = p.own().filter(|f| f.captain.is_none() && f.kind != H::Transport).count();
        let costs = sim::build::recipe_for(BuildKind::RecruitCaptain { serial: 0 });
        if self.ready("recruit", p.now) && free < uncommanded && p.home.system.tier(K::Academy) > 0
            && p.home.builds.is_empty() && !self.recovering(p)
            && costs.costs.iter().all(|(c, n)| p.stock(*c) >= *n) {
            self.wait("recruit".into(), p, p.cc);
            return Some(Command::RecruitCaptain { player_id: p.owner, system_id: p.capital });
        }
        None
    }

    fn site_strength(&self, p: &Observation, site: &MilitarySite) -> Option<f64> {
        // Old intel is a reason to scout again, not permission to attack an
        // imaginary empty base. No exact composition means no strength bargain.
        if p.now - site.intel.observed_at > 3_600.0 + p.round_trip(site.pos) { return None; }
        let mut defenders = 0.0;
        for f in p.ghosts.iter().filter(|f| !f.own && !f.ally && f.kind.is_combatant()
            && f.pos.distance(site.pos) < 25_000.0) { defenders += strength(f)?; }
        if site.intel.enclave_tier > 0 {
            // Public pirate templates are a conservative floor, never a read of
            // the hidden surviving pack. Don't count a travelling pack twice.
            let template: BTreeMap<_, _> = sim::pirate::formation(site.intel.enclave_tier).into_iter()
                .fold(BTreeMap::new(), |mut c, (kind, _, n)| { *c.entry(kind).or_default() += n; c });
            defenders = defenders.max(sim::combat::Forces::from_fleet(&template, &BTreeMap::new()).strength());
        }
        Some(defenders + sim::combat::Forces::from_fleet(&BTreeMap::new(), &BTreeMap::new())
            .with_platform(site.intel.defense_tier, 0.0).strength())
    }

    fn can_sortie(&self, p: &Observation, f: &GhostView, dest: Vec2, enemy: f64) -> bool {
        let margin = 1.8 - self.priorities().military * 0.15;
        strength(f).is_some_and(|power| power >= enemy.max(H::Raider.combat_weight() * 0.5) * margin)
            && f.fuel.unwrap_or(0.0) >= p.leg(f, f.pos, dest, 0) + p.sites.iter()
                .map(|s| p.leg(f, dest, s.system.pos, 0)).min_by(f64::total_cmp).unwrap_or(f64::INFINITY)
                + p.arrival_reserve(f)
    }

    fn contact_strength(&self, p: &Observation, at: Vec2) -> Option<f64> {
        // A weak picket beside a strong base is not an isolated easy battle.
        // Price the whole reported neighborhood, including unknown contacts
        // (which veto the sortie), without peeking at hidden reinforcements.
        let mut total = 0.0;
        for f in p.ghosts.iter().filter(|f| !f.own && !f.ally && !f.owner.is_tca()
            && f.kind.is_combatant() && f.pos.distance(at) < 25_000.0) { total += strength(f)?; }
        for site in p.military_sites.iter().filter(|s| s.pos.distance(at) < 25_000.0) {
            total = total.max(self.site_strength(p, site)?);
        }
        Some(total)
    }

    pub(super) fn military(&mut self, p: &Observation) -> Option<Command> {
        if self.campaign.as_ref().is_some_and(|c| !p.own().any(|f| f.id == c.leader)) {
            // Only disappearance from the arrived owned picture releases it.
            self.campaign = None;
        }
        if let Some(c) = self.campaign.clone() {
            let leader = p.own().find(|f| f.id == c.leader)?;
            if p.fighting.contains(&leader.id) { return None; }
            let target = match c.target {
                Target::Fleet(id) => p.ghosts.iter().find(|f| f.id == id).map(|f| (f.pos, false)),
                Target::System(id) => p.military_sites.iter().find(|s| s.id == id).map(|s| (s.pos, s.owner.is_some())),
            };
            let peace = matches!(c.target, Target::System(id) if p.military_sites.iter().any(|s|
                s.id == id && s.owner.is_some_and(|o| p.relations.get(&o) != Some(&RelationState::War))));
            if p.now > c.deadline || target.is_none() || peace || leader.damage.unwrap_or(0.0) > 0.2
                || leader.fuel.unwrap_or(0.0) < p.tank(leader) * 0.25 {
                let id = match c.target { Target::Fleet(id) | Target::System(id) => id };
                self.avoided.insert(id, p.now + 1_800.0);
                self.campaign = None;
                return self.return_for_fuel(p, leader);
            }
            if let Target::System(id) = c.target {
                let site = p.military_sites.iter().find(|s| s.id == id)?;
                // Assemble enough troops at a shared berth before committing
                // any individual transport to a stronger ground garrison.
                let need = sim::ground::break_even_marines(site.intel.garrison_tier as f64, 0.0) * 1.5;
                let transports: Vec<_> = p.own().filter(|f| f.kind == H::Transport && f.docked.is_some()
                    && f.docked.as_deref() != Some("hub") && f.captain.is_none() && f.supplied
                    && self.ready(&format!("fleet:{}", f.id), p.now)).collect();
                for into in &transports {
                    let aboard = composition(into).get(&H::Transport).copied().unwrap_or(0) * sim::ship::marine_capacity(H::Transport);
                    if aboard as f64 >= need { continue; }
                    if let Some(from) = transports.iter().find(|f| f.id != into.id && f.docked == into.docked
                        && f.pos.distance(into.pos) <= sim::ship::COLONY_CLAIM_RADIUS) {
                        self.wait(format!("fleet:{}", into.id), p, into.pos);
                        self.wait(format!("fleet:{}", from.id), p, from.pos);
                        return Some(Command::MergeFleets { player_id: p.owner, into: into.id, from: from.id });
                    }
                }
                // A troop wave follows REPORTED orbital control. It does not
                // teleport a colony ship in or price an unseen siege victory.
                if site.blockade.is_some_and(|b| b.by == p.owner && b.siege_since.is_some()) {
                    for troop in p.own().filter(|f| composition(f).get(&H::Transport).copied().unwrap_or(0) > 0) {
                        let key = format!("fleet:{}", troop.id);
                        let marines: u32 = composition(troop).iter().map(|(k, n)| sim::ship::marine_capacity(*k) * n).sum();
                        let need = sim::ground::break_even_marines(site.intel.garrison_tier as f64, 0.0);
                        if self.ready(&key, p.now) && !p.fighting.contains(&troop.id) && troop.vel.length_sq() < 1.0
                            && troop.pos.distance(site.pos) > sim::ship::COLONY_CLAIM_RADIUS
                            && marines as f64 >= need * 1.5 && p.enough_fuel(troop, site.pos) {
                            self.wait(key, p, troop.pos);
                            return Some(Command::MoveShip { player_id: p.owner, ship_id: troop.id, dest: site.pos });
                        }
                    }
                }
            }
            return None; // no per-think reissue while an ordered campaign unfolds
        }
        if self.recovering(p) || p.fuel_critical() { return None; }
        let home_guard = self.home_guard(p);
        let fighters: Vec<_> = p.own().filter(|f| self.ready_warship(p, f)
            && Some(f.id) != home_guard && f.defend_system.is_none()).collect();
        for fleet in &fighters {
            let mission = sim::doctrine::MissionProfile {
                priority: sim::doctrine::TargetPriority::Balanced,
                screening: sim::doctrine::ScreeningRole::ProtectTransports,
                withdrawal: sim::doctrine::DamageWithdrawal::Hull50,
            };
            if fleet.mission_profile != Some(mission) {
                self.wait(format!("fleet:{}", fleet.id), p, fleet.pos);
                return Some(Command::SetFleetMission { player_id: p.owner, fleet_id: fleet.id, mission });
            }
            if composition(fleet).get(&H::Raider).copied().unwrap_or(0) == 0 { continue; }
            // Patrols first: a fresh visible contact, never a hidden NPC ledger.
            if let Some(target) = p.ghosts.iter().filter(|g| !g.own && !g.ally && g.pirate
                && !self.avoided.contains_key(&g.id) && g.age <= p.round_trip(g.pos) + 60.0
                && self.contact_strength(p, g.pos).is_some_and(|s| self.can_sortie(p, fleet, g.pos, s)))
                .min_by(|a, b| fleet.pos.distance(a.pos).total_cmp(&fleet.pos.distance(b.pos))) {
                return Some(self.begin_campaign(p, fleet, Target::Fleet(target.id), target.pos,
                    Command::AttackFleet { player_id: p.owner, fleet_id: fleet.id, target_id: target.id }));
            }
            if let Some(site) = p.military_sites.iter().filter(|s| !self.avoided.contains_key(&s.id)
                && self.site_strength(p, s).is_some_and(|enemy| self.can_sortie(p, fleet, s.pos, enemy))
                && (s.intel.enclave_tier > 0 || s.owner.is_some_and(|o| p.relations.get(&o) == Some(&RelationState::War))))
                .min_by(|a, b| fleet.pos.distance(a.pos).total_cmp(&fleet.pos.distance(b.pos))) {
                let command = if site.owner.is_some() { Command::BlockadeSystem { player_id: p.owner, fleet_id: fleet.id, system_id: site.id } }
                    else { Command::MoveShip { player_id: p.owner, ship_id: fleet.id, dest: site.pos } };
                return Some(self.begin_campaign(p, fleet, Target::System(site.id), site.pos, command));
            }
        }
        // Build a legal mixed formation at a real shared berth. A Captain's
        // command limit and the no-two-officers merge rule are honored locally.
        for into in &fighters {
            if composition(into).get(&H::Raider).copied().unwrap_or(0) == 0 || into.docked.is_none() { continue; }
            for from in &fighters {
                if from.id == into.id || from.docked != into.docked || from.docked.as_deref() == Some("hub")
                    || from.vel.length_sq() > 1.0 || into.vel.length_sq() > 1.0
                    || into.pos.distance(from.pos) > sim::ship::COLONY_CLAIM_RADIUS
                    || into.captain.is_some() && from.captain.is_some() { continue; }
                let load = sim::captain::command_load(&composition(into)) + sim::captain::command_load(&composition(from));
                if into.captain.as_ref().or(from.captain.as_ref()).is_some_and(|c| load > c.command_capacity) { continue; }
                self.wait(format!("fleet:{}", into.id), p, into.pos);
                self.wait(format!("fleet:{}", from.id), p, from.pos);
                return Some(Command::MergeFleets { player_id: p.owner, into: into.id, from: from.id });
            }
        }
        // Expansionist military corporations may escalate an actually scouted,
        // affordable target through the normal announced-war mechanism. Never
        // cancel a treaty opportunistically or attack during its notice period.
        if self.priorities().military >= 1.5 && p.sites.len() > 1 && self.ready("war", p.now) {
            if let Some(site) = p.military_sites.iter().find(|s| s.owner.is_some_and(|o|
                !o.is_sentinel() && p.relations.get(&o).copied().unwrap_or_default() == RelationState::Neutral)
                && fighters.iter().any(|f| self.site_strength(p, s).is_some_and(|enemy| self.can_sortie(p, f, s.pos, enemy * 1.5)))) {
                self.waits.insert("war".into(), p.now + 3_600.0);
                self.status = "announcing a territorial campaign through ordinary diplomacy".into();
                return Some(Command::DeclareWar { player_id: p.owner, target: site.owner.unwrap() });
            }
        }
        None
    }

    fn begin_campaign(&mut self, p: &Observation, fleet: &GhostView, target: Target, pos: Vec2, command: Command) -> Command {
        let travel = fleet.pos.distance(pos) / (fleet.kind.max_speed() * sim::transit::WARP_FACTOR).max(1.0);
        let siege = match target { Target::System(id) => p.military_sites.iter().find(|s| s.id == id)
            .map_or(0.0, |s| p.siege_secs * (1.0 + s.intel.garrison_tier as f64) * 2.0), _ => 0.0 };
        self.campaign = Some(Campaign { target, leader: fleet.id, since: p.now,
            deadline: p.now + travel * 2.0 + p.round_trip(pos) + siege.max(7_200.0) });
        self.wait(format!("fleet:{}", fleet.id), p, fleet.pos);
        self.status = "dispatching a strength-matched campaign while retaining defense and escorts".into();
        command
    }
}
