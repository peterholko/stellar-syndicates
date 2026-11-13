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
        let Some(populated) = sys.bodies.iter().position(|b| b.population > 0.0) else {
            return;
        };
        // Land on the garden. The home generator makes the Biomass world the
        // roomy habitable body, but the bootstrap seeds people on the FIRST
        // habitable body, which can be a one-deposit rock. Settling there moved
        // the Bioharvester off its Biomass and let it fill the only Resource
        // slot, so the camp starved and could never build its mine. Every
        // generated home has this garden, so the first settlement is habitable.
        let index = sys
            .bodies
            .iter()
            .position(|b| b.habitable && b.has_deposit_for(K::Bioharvester))
            .unwrap_or(populated);
        if index != populated {
            let people = std::mem::take(&mut sys.bodies[populated].population);
            sys.bodies[index].population += people;
        }
        // The legacy system bootstrap may site storage/processing on a sibling
        // world. A landing camp visibly deploys all four starting modules here,
        // and its Landing Command stays with the people it administers.
        for kind in [K::Habitat, K::Warehouse, K::Bioharvester, K::Agroplex, K::CommandCenter] {
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
        // §home-ore: the garden holds its own basic Ferrite Ore (generated),
        // and the system's ore body keeps its deposit for the charter's
        // "Expand mining operations" step. Nothing moves.
        sys.bodies[index].founding_camp = Some(FoundingCamp::new(profile));
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
            || project == P::Storehouse && sys.landing_holds <= 0.0
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
            construction_yard: None,
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
            reactor: Default::default(),
            drive: Default::default(),
            charged: Vec::new(),
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
            P::Storehouse => {} // the holds derive from the completed physical project
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
    /// §charter-v4: the camp lands bare; the player builds these. Place them,
    /// staffed where they work.
    fn build_on_camp(w: &mut World, home: EntityId, id: u32, kinds: &[K]) {
        let b = w.systems.iter_mut().find(|s| s.id == home).unwrap().bodies.iter_mut().find(|b| b.id == id).unwrap();
        for kind in kinds {
            b.set_tier(*kind, 1);
            if kind.takes_workforce() {
                b.assignments.insert(*kind, crate::production::Assignment::crew(1));
            }
        }
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
            assert_eq!(b.population, crate::colony::HOME_FOUNDING_POP);
            assert_eq!(b.population_capacity(), 0.0, "§charter-v4: the settlers live aboard until the Habitat");
            assert_eq!(sys.storage_cap(), crate::build::LANDING_HOLDS, "the colony ship's holds");
            assert_eq!(b.tier(K::MiningComplex), 0);
            assert_eq!(b.tier(K::Shipyard), 0);
            assert_eq!(b.tier(K::CommandCenter), 1, "Landing Command deploys on the selected settlement planet");
            assert!(b.has_deposit_for(K::MiningComplex));
            assert!(b.has_deposit_for(K::Bioharvester));
            assert_eq!(w.players[&owner].credits, 2000.0);
            assert_eq!(w.fleets.values().filter(|f| f.owner == owner).count(), 0);
            // §charter-crates: the Landfall crate; the rest come chapter by
            // chapter. §smelt-from-landing: with the Smelter's 30 and 15.
            assert_eq!(sys.stockpile[&C::Alloys], 245.0);
            assert_eq!(sys.stockpile[&C::Machinery], 104.0);
        }
    }
    #[test]
    fn every_founding_camp_can_feed_itself_and_host_its_mine() {
        // Regression: 25% of pre-generated homes and 37% of overflow homes
        // used to land on a one-deposit habitable rock. The camp then had no
        // Biomass under its Bioharvester and no Resource slot for the mine.
        use crate::build::SlotPool;
        for seed in [12648430_u64, 1, 7, 42, 0xC0FFEE, 99_991, 4_354_685_564_932_589_508] {
            let mut w = World::new(SimConfig::for_players(seed, 4));
            w.enclaves.clear();
            for join in 0..6_u64 {
                let owner = PlayerId(20_000 + join);
                w.step(&[Command::FoundColony {
                    id: owner,
                    name: format!("Camp audit {join}"),
                    profile: F::MineralHighlands,
                }]);
                let home = w.players[&owner].home_system.unwrap();
                let sys = w.systems.iter().find(|s| s.id == home).unwrap();
                let camp = sys.bodies.iter().find(|b| b.founding_camp.is_some()).unwrap();
                let at = format!("seed {seed} join {join} {}", camp.name);
                assert!(camp.habitable, "{at}: the first settlement is habitable");
                assert!(camp.has_deposit_for(K::Bioharvester),
                    "{at}: the Bioharvester must work Biomass on the camp world");
                // §charter-v4: the camp lands bare; both its resource sites are
                // free for the Bioharvester and the mine (its own basic ore).
                assert!(camp.has_deposit_for(K::MiningComplex)
                    && camp.pool_slots(SlotPool::Resource) >= camp.pool_slots_built(SlotPool::Resource) + 2,
                    "{at}: the camp world must host the Bioharvester and the Mining Complex");
                // The Habitat, Agroplex and Academy all fit its Infrastructure.
                assert!(camp.pool_slots(SlotPool::Infrastructure) >= camp.pool_slots_built(SlotPool::Infrastructure) + 3,
                    "{at}: the camp world must have room for the Habitat, Agroplex and Academy");
                assert_eq!(camp.population, crate::colony::HOME_FOUNDING_POP, "{at}");
                assert!(sys.bodies.iter().filter(|b| b.id != camp.id).all(|b| b.population == 0.0
                    && b.tier(K::Habitat) == 0 && b.tier(K::CommandCenter) == 0), "{at}: people and administration move together");
            }
        }
    }

    #[test]
    fn a_previously_starving_home_slot_now_stays_fed() {
        // Seed 12648430's third home slot (Millstone) used to hit "OUT OF
        // PROVISIONS" about three minutes after landing.
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        w.enclaves.clear();
        for join in 0..3_u64 {
            w.step(&[Command::FoundColony { id: PlayerId(30_000 + join), name: format!("Slot {join}"), profile: F::VerdantLowlands }]);
        }
        let owner = PlayerId(30_002);
        let home = w.players[&owner].home_system.unwrap();
        for _ in 0..(300 * crate::config::TICK_HZ) {
            w.accrue_production(&mut Vec::new());
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.food_state, crate::colony::FoodState::WellSupplied);
        assert!(sys.stockpile.get(&C::Provisions).copied().unwrap_or(0.0) > 1.0);
    }

    #[test]
    fn a_camp_keeps_a_food_reserve_instead_of_filling_its_warehouse() {
        let (mut w, _owner, home, id) = start(F::VerdantLowlands);
        // §habitat-first (Nov 8 2026): the food chain runs on a Solar Array.
        build_on_camp(&mut w, home, id, &[K::PowerPlant, K::Bioharvester, K::Agroplex, K::Habitat]);
        let mut events = Vec::new();
        let food_at = |w: &World| { let s = w.systems.iter().find(|s| s.id == home).unwrap();
            s.stockpile.get(&C::Provisions).copied().unwrap_or(0.0) + s.stockpile.get(&C::Biomass).copied().unwrap_or(0.0) };
        let reserve_at = |w: &World| { let s = w.systems.iter().find(|s| s.id == home).unwrap();
            crate::settlement::camp_food_reserve(s.bodies.iter().map(|b| b.population * b.provisions_mult()).sum::<f64>()
                * crate::colony::PROVISIONS_PER_MILLION_PER_S) };
        let held = |w: &World| w.systems.iter().find(|s| s.id == home).unwrap().bodies.iter().any(|b| b.assignments
            .get(&K::Bioharvester).is_some_and(|a| a.suspended == Some(crate::production::SuspendReason::FoodReserve)));
        // §progression (Nov 11 2026): people eat at the output scale, but the
        // reserve keeps its units, so the landing crate's Provisions sit under
        // it and the first Bioharvester harvests from landing.
        let landed = food_at(&w);
        assert!(landed < reserve_at(&w), "landed {landed:.1} under the reserve {:.1}", reserve_at(&w));
        // Twenty sim-minutes of the default Bioharvester -> Agroplex chain.
        for _ in 0..(20 * 60 * crate::config::TICK_HZ) {
            w.accrue_production(&mut events);
            assert!(!held(&w), "under the reserve the harvest never holds");
        }
        // The Agroplex turns each tick's Biomass into Provisions at once, so
        // the harvest shows as food beyond what the camp ate, not as Biomass.
        assert!(food_at(&w) > landed, "the Bioharvester harvests from landing: {:.1} of {landed:.1}", food_at(&w));
        let reserve = reserve_at(&w);
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        let food = food_at(&w);
        assert!(food <= reserve + 5.0, "food {food:.1} must not grow past the reserve {reserve:.1}");
        let goods: f64 = sys.stockpile.iter().filter(|(c, _)| !matches!(c, C::Provisions | C::Biomass))
            .map(|(_, n)| n).sum();
        assert!(sys.storage_headroom() >= sys.storage_cap() - goods - reserve - 5.0,
            "food never crowds out the holds' room for ore: {:.1}", sys.storage_headroom());
        assert_eq!(sys.food_state, crate::colony::FoodState::WellSupplied);
        assert!(!events.iter().any(|e| matches!(e.payload,
            EventPayload::ProductionSuspended { reason: crate::production::SuspendReason::NoInputs, .. }
            | EventPayload::ProductionSuspended { reason: crate::production::SuspendReason::FoodReserve, .. })),
            "a reserve hold is quiet policy, not an input outage");
        // Filled to the reserve, the harvest holds; it resumes under 75%.
        let set_food = |w: &mut World, n: f64| { let s = w.systems.iter_mut().find(|s| s.id == home).unwrap();
            s.stockpile.insert(C::Biomass, 0.0);
            s.stockpile.insert(C::Provisions, n); };
        let mut later = Vec::new();
        set_food(&mut w, reserve + 1.0);
        w.accrue_production(&mut later);
        assert!(held(&w), "at the reserve the harvest holds");
        set_food(&mut w, reserve * 0.8);
        w.accrue_production(&mut later);
        assert!(held(&w), "above 75% it stays held");
        set_food(&mut w, reserve * 0.7);
        w.accrue_production(&mut later);
        assert!(!held(&w), "under 75% it resumes");
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
        build_on_camp(&mut w, home, id, &[K::Habitat]);
        queue(&mut w, owner, home, id, P::Prospect);
        queue(&mut w, owner, home, id, P::Storehouse);
        queue(&mut w, owner, home, id, P::Storehouse);
        assert_eq!(w.build_queue.len(), 2);
        assert!(w.build_queue[1].is_queued_structure());
        assert_eq!(
            w.build_queue[1].work().unwrap().completed_at(w.tick + 100),
            0.0
        );
        assert_eq!(
            w.systems.iter().find(|s| s.id == home).unwrap().stockpile[&C::Alloys],
            238.0
        );
        let slots = body(&w, home, id).infrastructure_slots();
        finish(&mut w);
        assert_eq!(body(&w, home, id).infrastructure_slots(), slots + 1);
        // §habitat-rebase: the camp's Habitat I is a plain Habitat I.
        let b = body(&w, home, id);
        assert_eq!(b.population_capacity(), crate::colony::POP_CAP_PER_HABITAT_TIER * b.habitat_capacity_mult());
        w = serde_json::from_str(&serde_json::to_string(&w).unwrap()).unwrap();
        finish(&mut w);
        // §charter-v4: the Storehouse enlarges the colony ship's holds.
        assert_eq!(
            w.systems
                .iter()
                .find(|s| s.id == home)
                .unwrap()
                .storage_cap(),
            crate::build::LANDING_HOLDS + crate::build::STOREHOUSE_HOLDS
        );
        assert_eq!(
            body(&w, home, id).population,
            crate::colony::HOME_FOUNDING_POP,
            "a project is not instant people"
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
        // §charter-gates: the mine's step.
        w.players.get_mut(&owner).unwrap().founding.set_stage(crate::founding::FoundingStage::BuildMine, w.time);
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
        // A short stockpile refuses a project: 5 Alloys cannot fund the
        // 7-Alloy Storehouse (the kit itself now covers every project).
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(C::Alloys, 5.0);
        queue(&mut w, owner, home, id, P::Storehouse);
        assert_eq!(w.build_queue.len(), 2, "5 alloys cannot fund the Storehouse");
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
            for project in [P::Prospect, P::SurfaceWorkings, P::Storehouse] {
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
    fn completed_settlement_profile_waits_for_report_light() {
        let (mut w, owner, home, id) = start(F::CoastalPlains);
        build_on_camp(&mut w, home, id, &[K::Habitat]);
        w.record_information();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        let cc = pos + Vec2::new(10.0 * crate::transit::signal_speed(w.config.c), 0.0);
        w.players.get_mut(&owner).unwrap().command_center = cc;
        w.time += 20.0;
        let slots = body(&w, home, id).infrastructure_slots();
        w.finish_settlement_project(owner, home, id, P::Prospect);
        w.record_information();
        let seen = |at: f64| {
            w.information
                .site(home, cc, w.config.c, at)
                .unwrap()
                .system
                .bodies
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .infrastructure_slots()
        };
        assert_eq!(seen(w.time + 9.0), slots);
        assert_eq!(seen(w.time + 10.0), slots + 1, "the Coastal Plains site, once its report arrives");
    }
}
