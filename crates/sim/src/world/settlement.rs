use super::*;
use crate::settlement::{FoundingCamp, FoundingProfile as F, SettlementProject as P};
use crate::{BuildKind, Commodity as C, StructureKind as K};

impl World {
    pub(super) fn establish_founding_camp(&mut self, owner: PlayerId, profile: F) {
        let Some(home) = self.players.get(&owner).and_then(|c| c.home_system) else {
            return;
        };
        let Some(sys) = self.systems.iter_mut().find(|s| s.id == home) else {
            return;
        };
        let Some(index) = sys.bodies.iter().position(|b| b.population > 0.0) else {
            return;
        };
        // The legacy system bootstrap may site storage/processing on a sibling
        // world. A landing camp visibly deploys all four starting modules here.
        for kind in [K::Habitat, K::Warehouse, K::Bioharvester, K::Agroplex] {
            let mut tier = 0;
            let mut assignment = None;
            for b in &mut sys.bodies {
                tier = tier.max(b.structures.remove(&kind).unwrap_or(0));
                if let Some(posting) = b.assignments.remove(&kind) {
                    assignment = Some(posting);
                }
            }
            sys.bodies[index].set_tier(kind, tier);
            if let Some(posting) = assignment {
                sys.bodies[index].assignments.insert(kind, posting);
            }
        }
        // Bring the starter Ferrite deposit onto the settlement world. No new
        // planet, habitable slot, stockpile grant, RNG draw or extra ore source.
        let mut ores = Vec::new();
        for b in &mut sys.bodies {
            b.deposits.retain(|d| {
                if d.resource == C::MetallicOre {
                    ores.push(d.clone());
                    false
                } else {
                    true
                }
            });
        }
        let body = &mut sys.bodies[index];
        body.deposits.extend(ores);
        body.founding_camp = Some(FoundingCamp::new(profile));
    }

    pub(super) fn build_settlement_project(
        &mut self,
        owner: PlayerId,
        system: EntityId,
        body: u32,
        project: P,
        events: &mut Vec<Event>,
    ) {
        let Some(sys) = self
            .systems
            .iter()
            .find(|s| s.id == system && s.owner == Some(owner))
        else {
            return;
        };
        let Some(b) = sys.bodies.iter().find(|b| b.id == body) else {
            return;
        };
        let Some(camp) = &b.founding_camp else {
            return;
        };
        let what = BuildKind::Founding { project };
        if camp.done(project)
            || self
                .build_queue
                .iter()
                .any(|j| j.system == system && j.body_id == body && j.what == what)
        {
            return;
        }
        if project == P::SurfaceWorkings
            && (!camp.done(P::Prospect) || b.tier(K::MiningComplex) == 0)
            || project == P::Storehouse && b.tier(K::Warehouse) != 1
            || project == P::Quarters && b.tier(K::Habitat) != 1
        {
            events.push(Event::new(
                self.time,
                EventPayload::IndustryRejected {
                    owner,
                    system,
                    reason: "Settlement project prerequisites are not met".into(),
                },
            ));
            return;
        }
        let recipe = project.recipe();
        if !recipe
            .costs
            .iter()
            .all(|(c, n)| sys.project_stock(*c, None) + 1e-9 >= *n)
        {
            events.push(Event::new(self.time, EventPayload::IndustryRejected { owner, system,
                reason: "Not enough unreserved goods in the local stockpile for this settlement project".into() }));
            return;
        }
        let sys = self.systems.iter_mut().find(|s| s.id == system).unwrap();
        for (c, n) in recipe.costs {
            *sys.stockpile.entry(*c).or_default() -= n;
        }
        let busy = self.build_queue.iter().any(|j| {
            j.system == system
                && j.body_id == body
                && matches!(
                    j.what,
                    BuildKind::Upgrade { .. } | BuildKind::Founding { .. }
                )
        });
        let work = crate::build::BuildWork {
            required: recipe.build_ticks as f64,
            completed: 0.0,
            at_tick: self.tick,
            rate: if busy { 0.0 } else { 1.0 },
        };
        let complete_tick = work.completion_tick().unwrap_or(u64::MAX);
        self.next_build_id += 1;
        self.build_queue.push(crate::build::BuildJob {
            id: self.next_build_id,
            owner,
            system,
            body_id: body,
            what,
            queued_tick: Some(self.tick),
            started_tick: (!busy).then_some(self.tick),
            complete_tick,
            structure_work: Some(work),
            ship_work: None,
            join: None,
            loadout: Default::default(),
        });
        events.push(Event::new(
            self.time,
            EventPayload::BuildStarted {
                id: self.next_build_id,
                owner,
                system,
                what,
                complete_tick,
            },
        ));
    }

    pub(super) fn finish_settlement_project(
        &mut self,
        owner: PlayerId,
        system: EntityId,
        body: u32,
        project: P,
    ) -> bool {
        let Some(b) = self
            .systems
            .iter_mut()
            .find(|s| s.id == system && s.owner == Some(owner))
            .and_then(|s| s.bodies.iter_mut().find(|b| b.id == body))
        else {
            return false;
        };
        let Some(camp) = &mut b.founding_camp else {
            return false;
        };
        if !camp.completed.insert(project) {
            return false;
        }
        match project {
            P::Prospect => match camp.profile {
                F::MineralHighlands => {
                    if !b.deposits.iter().any(|d| d.resource == C::CupriteOre) {
                        b.deposits.push(crate::galaxy::Deposit {
                            resource: C::CupriteOre,
                            richness: 0.25,
                            reserves: Some(3_000.0),
                            accessibility: 0.2,
                        });
                    }
                }
                F::VerdantLowlands => {
                    if let Some(d) = b.deposits.iter_mut().find(|d| d.resource == C::Biomass) {
                        d.richness *= 1.25;
                    }
                }
                F::CoastalPlains => {} // body.infrastructure_slots reads the completed survey
            },
            P::SurfaceWorkings => {
                if let Some(d) = b.deposits.iter_mut().find(|d| d.resource == C::MetallicOre) {
                    d.richness *= 1.5;
                }
            }
            P::Storehouse | P::Quarters => {} // capacities derive from the completed physical project
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(profile: F) -> (World, PlayerId, EntityId, u32) {
        let mut w = World::new(SimConfig::for_players(12648430, 2));
        w.enclaves.clear();
        let owner = PlayerId(971);
        w.step(&[Command::FoundColony {
            id: owner,
            name: "Camp test".into(),
            profile,
        }]);
        let home = w.players[&owner].home_system.unwrap();
        let body = w
            .systems
            .iter()
            .find(|s| s.id == home)
            .unwrap()
            .bodies
            .iter()
            .find(|b| b.founding_camp.is_some())
            .unwrap()
            .id;
        (w, owner, home, body)
    }
    fn body(w: &World, home: EntityId, id: u32) -> &crate::Body {
        w.systems
            .iter()
            .find(|s| s.id == home)
            .unwrap()
            .bodies
            .iter()
            .find(|b| b.id == id)
            .unwrap()
    }
    fn queue(w: &mut World, owner: PlayerId, home: EntityId, body: u32, project: P) {
        w.build_settlement_project(owner, home, body, project, &mut Vec::new());
    }
    fn finish(w: &mut World) {
        w.tick = w.build_queue.iter().map(|j| j.complete_tick).min().unwrap();
        w.time = w.tick as f64 * crate::config::DT;
        w.resolve_builds(&mut Vec::new());
    }
    #[test]
    fn founding_profiles_preserve_population_world_budget_and_starter_economy() {
        let baseline = World::new(SimConfig::for_players(12648430, 2));
        let habitable = baseline
            .systems
            .iter()
            .flat_map(|s| &s.bodies)
            .filter(|b| b.habitable)
            .count();
        for profile in [F::MineralHighlands, F::VerdantLowlands, F::CoastalPlains] {
            let (w, owner, home, id) = start(profile);
            assert_eq!(w.systems.len(), baseline.systems.len());
            assert_eq!(
                w.systems
                    .iter()
                    .flat_map(|s| &s.bodies)
                    .filter(|b| b.habitable)
                    .count(),
                habitable
            );
            let sys = w.systems.iter().find(|s| s.id == home).unwrap();
            let b = body(&w, home, id);
            assert_eq!(b.population, 0.002);
            assert_eq!(b.population_capacity(), 0.003);
            assert_eq!(sys.storage_cap(), 250.0);
            assert_eq!(b.tier(K::MiningComplex), 0);
            assert_eq!(b.tier(K::Shipyard), 0);
            assert!(b.has_deposit_for(K::MiningComplex));
            assert!(b.has_deposit_for(K::Bioharvester));
            assert_eq!(w.players[&owner].credits, 2000.0);
            assert_eq!(w.fleets.values().filter(|f| f.owner == owner).count(), 3);
            assert_eq!(sys.stockpile[&C::Alloys], 30.0);
            assert_eq!(sys.stockpile[&C::Machinery], 15.0);
        }
    }
    #[test]
    fn founding_is_one_time_and_old_colonies_are_not_converted() {
        let (mut w, owner, home, id) = start(F::CoastalPlains);
        let began = w.players[&owner].founding.started_at;
        w.systems
            .iter_mut()
            .find(|s| s.id == home)
            .unwrap()
            .stockpile
            .insert(C::Alloys, 3.0);
        w.apply_now(
            &Command::FoundColony {
                id: owner,
                name: "Duplicate".into(),
                profile: F::MineralHighlands,
            },
            &mut Vec::new(),
        );
        assert_eq!(
            body(&w, home, id).founding_camp.as_ref().unwrap().profile,
            F::CoastalPlains
        );
        assert_eq!(w.players[&owner].founding.started_at, began);
        assert_eq!(
            w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&C::Alloys],
            3.0
        );
        let restored: World = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        assert_eq!(
            body(&restored, home, id).founding_camp,
            body(&w, home, id).founding_camp
        );
        let mut legacy = World::new(SimConfig::for_players(12648430, 2));
        legacy.step(&[Command::AddPlayer {
            id: owner,
            name: "Older corporation".into(),
        }]);
        legacy.step(&[Command::FoundColony {
            id: owner,
            name: "Replay".into(),
            profile: F::VerdantLowlands,
        }]);
        assert!(
            legacy
                .systems
                .iter()
                .flat_map(|s| &s.bodies)
                .all(|b| b.founding_camp.is_none())
        );
    }
    #[test]
    fn projects_are_fifo_paid_once_and_do_not_create_population() {
        let (mut w, owner, home, id) = start(F::CoastalPlains);
        queue(&mut w, owner, home, id, P::Prospect);
        queue(&mut w, owner, home, id, P::Quarters);
        queue(&mut w, owner, home, id, P::Quarters);
        assert_eq!(w.build_queue.len(), 2);
        assert!(w.build_queue[1].is_queued_structure());
        assert_eq!(
            w.build_queue[1].work().unwrap().completed_at(w.tick + 100),
            0.0
        );
        assert_eq!(
            w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&C::Alloys],
            25.0
        );
        let slots = body(&w, home, id).infrastructure_slots();
        finish(&mut w);
        assert_eq!(body(&w, home, id).infrastructure_slots(), slots + 1);
        assert_eq!(body(&w, home, id).population_capacity(), 0.003);
        w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        finish(&mut w);
        assert_eq!(body(&w, home, id).population_capacity(), 0.006);
        assert_eq!(
            body(&w, home, id).population,
            0.002,
            "housing is not instant people"
        );
        queue(&mut w, owner, home, id, P::Storehouse);
        finish(&mut w);
        assert_eq!(
            w.systems
                .iter()
                .find(|s| s.id == home)
                .unwrap()
                .storage_cap(),
            700.0
        );
        assert!(w.build_queue.is_empty());
    }
    #[test]
    fn projects_share_the_structure_queue_and_validate_site_materials_and_prerequisites() {
        let (mut w, owner, home, id) = start(F::MineralHighlands);
        queue(&mut w, PlayerId(972), home, id, P::Prospect);
        queue(&mut w, owner, home, id, P::SurfaceWorkings);
        assert!(w.build_queue.is_empty());
        queue(&mut w, owner, home, id, P::Prospect);
        w.apply_now(
            &Command::DevelopSystem {
                player_id: owner,
                system_id: home,
                body_id: Some(id),
                upgrade: K::MiningComplex,
            },
            &mut Vec::new(),
        );
        assert_eq!(w.build_queue.len(), 2);
        assert!(w.build_queue[1].is_queued_structure());
        queue(&mut w, owner, home, id, P::Storehouse);
        assert_eq!(w.build_queue.len(), 2, "mine left only 5 alloys");
        finish(&mut w);
        finish(&mut w);
        assert!(
            body(&w, home, id)
                .deposits
                .iter()
                .any(|d| d.resource == C::CupriteOre)
        );
        let before = body(&w, home, id)
            .deposits
            .iter()
            .find(|d| d.resource == C::MetallicOre)
            .unwrap()
            .richness;
        w.systems
            .iter_mut()
            .find(|s| s.id == home)
            .unwrap()
            .stockpile
            .insert(C::Alloys, 8.0);
        queue(&mut w, owner, home, id, P::SurfaceWorkings);
        finish(&mut w);
        assert_eq!(
            body(&w, home, id)
                .deposits
                .iter()
                .find(|d| d.resource == C::MetallicOre)
                .unwrap()
                .richness,
            before * 1.5
        );
        assert_eq!(
            body(&w, home, id).tier(K::MiningComplex),
            1,
            "no research tier bypass"
        );
        queue(&mut w, owner, home, id, P::SurfaceWorkings);
        assert!(w.build_queue.is_empty());
    }
    #[test]
    fn optional_projects_leave_the_opening_material_budget_viable() {
        // Starting supplies plus the finite, physically recovered cache can
        // cover every camp project AND the mandatory mine/Service Berth. No
        // extra grant or first-Hub import is needed to escape a bad click order.
        for (commodity, budget) in [(C::Alloys, 50.0), (C::Machinery, 19.0)] {
            let mut costs = 0.0;
            for kind in [K::MiningComplex, K::ServiceBerth] {
                costs += crate::build::recipe_for(BuildKind::Upgrade { upgrade: kind })
                    .costs
                    .iter()
                    .filter(|(c, _)| *c == commodity)
                    .map(|(_, n)| n)
                    .sum::<f64>();
            }
            for project in [P::Prospect, P::SurfaceWorkings, P::Storehouse, P::Quarters] {
                costs += project
                    .recipe()
                    .costs
                    .iter()
                    .filter(|(c, _)| *c == commodity)
                    .map(|(_, n)| n)
                    .sum::<f64>();
            }
            assert!(
                costs <= budget,
                "optional projects must not block the opening: {commodity:?}"
            );
        }
    }

    #[test]
    fn remote_settlement_project_waits_for_command_light() {
        let (mut w, owner, home, id) = start(F::VerdantLowlands);
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        w.players.get_mut(&owner).unwrap().command_center =
            pos + Vec2::new(10.0 * crate::transit::signal_speed(w.config.c), 0.0);
        w.step(&[Command::DevelopSettlement {
            player_id: owner,
            system_id: home,
            body_id: id,
            project: P::Prospect,
        }]);
        assert!(w.build_queue.is_empty());
        for _ in 0..(11 * crate::config::TICK_HZ) {
            w.step(&[]);
        }
        assert_eq!(w.build_queue.len(), 1);
        assert!(
            !body(&w, home, id)
                .founding_camp
                .as_ref()
                .unwrap()
                .done(P::Prospect)
        );
    }

    #[test]
    fn completed_settlement_capacity_and_profile_wait_for_report_light() {
        let (mut w, owner, home, id) = start(F::CoastalPlains);
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let cc = pos + Vec2::new(10.0 * crate::transit::signal_speed(w.config.c), 0.0);
        w.players.get_mut(&owner).unwrap().command_center = cc;
        w.time += 20.0;
        w.finish_settlement_project(owner, home, id, P::Quarters);
        w.record_information();
        let before = w
            .information
            .site(home, cc, w.config.c, w.time + 9.0)
            .unwrap();
        assert_eq!(
            before
                .system
                .bodies
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .population_capacity(),
            0.003
        );
        let after = w
            .information
            .site(home, cc, w.config.c, w.time + 10.0)
            .unwrap();
        assert_eq!(
            after
                .system
                .bodies
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .population_capacity(),
            0.006
        );
        assert_eq!(
            after
                .system
                .bodies
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .population,
            0.002
        );
    }
}
