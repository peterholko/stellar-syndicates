//! The same local tutorial as human founders, using only arrived observations.
use super::*;

impl Observation {
    pub(super) fn opening_local_work(&self) -> bool {
        self.local_opening
            && matches!(
                self.founding_stage,
                sim::FoundingStage::BuildMine | sim::FoundingStage::LocalRecovery
            )
    }
}

impl Bot {
    /// §belts: the charter's last lesson, as a founder plays it: send the
    /// Mining Barge to the charted belt the programme points at.
    pub(super) fn belt_lesson(&mut self, p: &Observation) -> Option<Command> {
        let (system_id, body_id, ore, pos) = p.belt_lesson?;
        let barge = p.own().find(|g| g.kind == H::MiningBarge)?;
        if !self.ready(&format!("fleet:{}", barge.id), p.now) || p.fighting.contains(&barge.id) {
            return None;
        }
        let command = match p.founding_stage {
            sim::FoundingStage::MineBelt => {
                let working = matches!(&barge.industry,
                    Some(sim::industry::FleetIndustry::Mine { system, .. }) if *system == system_id);
                if working || p.threatened_within(pos, super::belts::BELT_THREAT_SU) {
                    return None;
                }
                self.status = "founding charter: a Mining Barge to the charted belt".into();
                Command::MineBelt { player_id: p.owner, fleet_id: barge.id, system_id, body_id, ore, pocket: false }
            }
            // §charter v3: the last step brings a load home and unloads it.
            sim::FoundingStage::HaulOre => {
                let home = &p.home.system;
                let aboard: u32 = barge.cargo_manifest.iter().map(|c| c.units).sum();
                let full = matches!(&barge.industry,
                    Some(sim::industry::FleetIndustry::Mine { status, .. }) if status == "full");
                let at_home = barge.docked.as_deref() == Some(home.id.to_string().as_str());
                if at_home && aboard > 0 {
                    self.status = "founding charter: unloading the barge's ore at home".into();
                    Command::SystemUnload { player_id: p.owner, fleet_id: barge.id, system: home.id }
                } else if !at_home && barge.industry.is_some() && (aboard >= 30 || full) {
                    self.status = "founding charter: bringing the barge's ore home".into();
                    Command::MoveShip { player_id: p.owner, ship_id: barge.id, dest: home.pos }
                } else {
                    return None;
                }
            }
            _ => return None,
        };
        self.wait(format!("fleet:{}", barge.id), p, barge.pos);
        Some(command)
    }

    pub(super) fn local_opening(&mut self, p: &Observation) -> Option<Command> {
        if !p.opening_local_work() {
            return None;
        }
        let site = p.recovery_site.as_ref()?;
        let scout = p.own().find(|g| g.kind == H::Scout)?;
        let carrier = p.own().find(|g| g.kind.is_player_freighter())?;
        let home = &p.home.system;
        let at_home = |g: &GhostView| g.docked.as_deref() == Some(home.id.to_string().as_str());
        let ready = |bot: &Self, g: &GhostView| {
            bot.ready(&format!("fleet:{}", g.id), p.now) && !p.fighting.contains(&g.id)
        };
        if ready(self, scout) && !at_home(scout) && p.threatened(scout.pos) {
            return self.return_for_fuel(p, scout);
        }
        // A Scout never investigates a known threat. It can return and retry
        // after the escort's combat report rather than overriding safe arrival.
        let command = if site.details.is_none() {
            if !ready(self, scout) {
                return None;
            }
            if p.threatened(site.pos) || p.threatened(scout.pos) {
                return self.return_for_fuel(p, scout);
            }
            if scout.expedition.as_ref().is_some_and(|e| e.site == site.id) {
                return None;
            }
            Command::ExploreSite {
                player_id: p.owner,
                fleet_id: scout.id,
                site_id: site.id,
                task: sim::sites::ExpeditionTask::Investigate,
            }
        } else if !p.opening_recovered {
            if !ready(self, carrier) {
                return None;
            }
            if carrier
                .expedition
                .as_ref()
                .is_some_and(|e| e.site == site.id)
            {
                return None;
            }
            if carrier.guard_target.is_some() {
                return None;
            }
            if !p.own().any(|g| g.guard_target == Some(carrier.id)) {
                return None;
            }
            if cargo_used(carrier) > 0 {
                if at_home(carrier) {
                    Command::SystemUnload {
                        player_id: p.owner,
                        fleet_id: carrier.id,
                        system: home.id,
                    }
                } else {
                    Command::HaulToSystem {
                        player_id: p.owner,
                        fleet_id: carrier.id,
                        system: home.id,
                    }
                }
            } else {
                Command::ExploreSite {
                    player_id: p.owner,
                    fleet_id: carrier.id,
                    site_id: site.id,
                    task: sim::sites::ExpeditionTask::Recover,
                }
            }
        } else if !at_home(carrier) && ready(self, carrier) {
            if carrier.vel.length_sq() > 1.0
                && carrier
                    .path
                    .as_ref()
                    .and_then(|p| p.last())
                    .is_some_and(|v| v.pos.distance(home.pos) < 1.0)
            {
                return None;
            }
            Command::HaulToSystem {
                player_id: p.owner,
                fleet_id: carrier.id,
                system: home.id,
            }
        } else if at_home(carrier) && cargo_used(carrier) > 0 && ready(self, carrier) {
            Command::SystemUnload {
                player_id: p.owner,
                fleet_id: carrier.id,
                system: home.id,
            }
        } else if !at_home(scout) && ready(self, scout) {
            if scout.vel.length_sq() > 1.0
                && scout
                    .path
                    .as_ref()
                    .and_then(|p| p.last())
                    .is_some_and(|v| v.pos.distance(home.pos) < 1.0)
            {
                return None;
            }
            Command::MoveShip {
                player_id: p.owner,
                ship_id: scout.id,
                dest: home.pos,
            }
        } else if home.tier(K::ServiceBerth) == 0 && home.tier(K::Shipyard) == 0 {
            if !self.ready("build", p.now) || !p.home.builds.is_empty() {
                return None;
            }
            let body = home.bodies.iter().find(|b| b.tier(K::Warehouse) > 0)?;
            if !sim::build::SERVICE_BERTH_RECIPE
                .costs
                .iter()
                .all(|(c, n)| p.stock(*c) >= *n)
            {
                return None;
            }
            self.wait("build".into(), p, home.pos);
            return Some(Command::DevelopSystem {
                player_id: p.owner,
                system_id: home.id,
                body_id: Some(body.id),
                upgrade: K::ServiceBerth,
            });
        } else if at_home(scout)
            && ready(self, scout)
            && home
                .modules
                .get(&sim::ModuleKind::ReconSuite)
                .copied()
                .unwrap_or(0)
                > 0
        {
            let from = scout
                .loadouts
                .as_ref()
                .and_then(|v| v.iter().find(|s| s.kind == H::Scout))
                .map(|s| sim::Loadout::new(s.modules.clone()))
                .unwrap_or_default();
            if from.has_recon() {
                return None;
            }
            Command::RefitShips {
                player_id: p.owner,
                fleet_id: scout.id,
                ship: H::Scout,
                from,
                to: sim::Loadout::new(vec![sim::ModuleKind::ReconSuite]),
                n: 1,
            }
        } else {
            return None;
        };
        let fleet = match &command {
            Command::ExploreSite { fleet_id, .. }
            | Command::HaulToSystem { fleet_id, .. }
            | Command::SystemUnload { fleet_id, .. }
            | Command::RefitShips { fleet_id, .. } => *fleet_id,
            Command::MoveShip { ship_id, .. } => *ship_id,
            _ => return None,
        };
        let pos = p.own().find(|f| f.id == fleet)?.pos;
        self.wait(format!("fleet:{fleet}"), p, pos);
        self.status = "local recovery and Scout fitting before the long Hub voyage".into();
        Some(command)
    }
}
