//! Continuing, report-driven goals. These are policy preferences, never unlocks
//! or sim balance changes. Wealth funds ambitions; it cannot waive a verb gate.
use super::*;
use sim::research::{Effect, Field, ModKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Role { Mining, Fuel, Agriculture, Industry, Science }

#[derive(Clone, Copy)]
pub(super) struct Priorities {
    pub trade: f64, pub industry: f64, pub science: f64,
    pub expansion: f64, pub military: f64,
}

impl Bot {
    pub(super) fn priorities(&self) -> Priorities {
        let [trade, industry, science, expansion, military] = match self.slot % 8 {
            0 => [1.8, 1.0, 0.8, 1.0, 0.8], // Atlas: carrier network
            1 => [0.9, 1.8, 1.0, 0.9, 1.2], // Cinder: heavy industry
            2 => [0.8, 0.9, 1.8, 1.4, 0.7], // Pioneer: science/exploration
            3 => [1.1, 1.7, 0.8, 1.2, 0.9], // Meridian: extraction
            4 => [1.9, 0.9, 1.0, 0.8, 1.0], // Orion: trade/cash reserves
            5 => [0.8, 1.5, 1.1, 0.9, 1.7], // Helix: naval industry
            6 => [1.0, 1.0, 1.2, 1.9, 0.8], // Far Horizon: settlement
            _ => [1.0, 1.2, 0.8, 1.4, 1.5], // Vesper: contested resources
        };
        Priorities { trade, industry, science, expansion, military }
    }

    pub(super) fn role(&self, p: &Observation) -> Role {
        let raw = |kind| p.home.system.bodies.iter().flat_map(|b| b.deposits.iter()
            .filter(move |d| sim::production::extraction_structure(d.resource) == Some(kind))
            .map(move |d| sim::explore::natural_extraction_rate(b, d, p.home.system.trait_)))
            .sum::<f64>();
        // The original home keeps a diversified industrial/science role once
        // other colonies can supply it. New sites specialize in their geology.
        if p.home.system.id == p.capital && p.sites.len() > 1 {
            return if self.priorities().science > 1.5 { Role::Science } else { Role::Industry };
        }
        let candidates = [(Role::Mining, raw(K::MiningComplex)),
            (Role::Fuel, raw(K::VolatileHarvester) * 1.4),
            (Role::Agriculture, raw(K::Bioharvester) * 1.2)];
        candidates.into_iter().max_by(|a, b| a.1.total_cmp(&b.1)).filter(|(_, v)| *v > 0.0)
            .map_or(Role::Industry, |(r, _)| r)
    }

    pub(super) fn preferred_freighter(&self, p: &Observation) -> H {
        [H::BulkFreighter, H::HeavyFreighter, H::LargeFreighter, H::Convoy,
            H::SmallFreighter, H::TinyFreighter].into_iter().find(|h|
            !sim::ship::requires_hull_unlock(*h) || sim::research::has_hull(&p.research, *h))
            .unwrap_or(H::TinyFreighter)
    }

    pub(super) fn preferred_warship(&self, p: &Observation) -> H {
        [H::Titan, H::Dreadnought, H::Battleship, H::Cruiser, H::Destroyer, H::Corvette, H::Raider]
            .into_iter().find(|h| !sim::ship::requires_hull_unlock(*h) || sim::research::has_hull(&p.research, *h))
            .unwrap_or(H::Raider)
    }

    pub(super) fn fleet_needs(&self, p: &Observation) -> Vec<(H, u32, f64)> {
        let profile = self.priorities();
        let ports = p.sites.len().max(1) as u32;
        let carriers = p.own().filter(|f| f.kind.is_player_freighter()).count() as u32;
        let capacity_now: u32 = p.own().filter(|f| f.kind.is_player_freighter()).map(capacity).sum();
        let surplus = p.sites.iter().map(|s| s.system.stockpile.iter()
            .filter(|(c, _)| c.is_ore()).map(|(_, n)| n).sum::<f64>()).sum::<f64>();
        let freight = self.preferred_freighter(p);
        // §fuel: once a bigger hold is researched, a fleet of smaller hulls gets
        // one: a Small Freighter brings three Tiny loads of Fuel home per run.
        let bigger = p.count(freight) == 0 && p.own().filter(|f| f.kind.is_player_freighter())
            .map(capacity).max().is_some_and(|hold| hold < freight.cargo_units());
        let freight_need = u32::from(carriers < ports * 2
            || surplus > capacity_now as f64 * (2.0 / profile.trade) || bigger);
        // Replacement demand is computed from arrived surviving hulls + paid
        // jobs. There is no 'already built one' latch after a casualty.
        let escorts = carriers.max(1) + ports;
        let dedicated = p.own().filter(|f| f.kind == H::Raider
            && warfare::composition(f).values().sum::<u32>() == 1 && !self.campaign_fleet(f.id)).count() as u32;
        let raiders = p.own().map(|f| warfare::composition(f).get(&H::Raider).copied().unwrap_or(0)).sum::<u32>();
        let pending = p.count(H::Raider).saturating_sub(raiders);
        // Twenty Interceptors in one campaign are not twenty freight escorts.
        // Keep a separate single-hull defense/escort pool when fleets merge.
        let guard_target = p.count(H::Raider) + (escorts + 1).saturating_sub(dedicated + pending);
        let warship = self.preferred_warship(p);
        let economic_scale = (self.spendable(p, false) / 20_000.0).sqrt().floor() as u32;
        let navy = ((ports + economic_scale) as f64 * profile.military).ceil() as u32;
        let mut needs = vec![
            (H::TinyFreighter, u32::from(carriers == 0), 180.0),
            (H::Scout, ((ports as f64 * profile.science / 2.0).ceil() as u32).max(1), 85.0 * profile.science),
            (H::Raider, guard_target, 95.0 * profile.military),
            (freight, p.count(freight) + freight_need, 100.0 * profile.trade),
            (warship, navy.max(1), 90.0 * profile.military),
        ];
        if warship != H::Raider && warship != H::Corvette {
            needs.push((H::Corvette, (navy / 2).max(1), 80.0 * profile.military));
        }
        if self.expansion.is_some() { needs.push((H::Colony, 1, 160.0 * profile.expansion)); }
        // §belts phase 6: barges for the belts; a station once one pays.
        let barges = p.barge_need();
        if barges > 0 { needs.push((H::MiningBarge, barges, 60.0 * profile.industry)); }
        if p.wants_station() { needs.push((H::MiningBeltStation, 1, 70.0 * profile.industry)); }
        if p.military_sites.iter().any(|s| s.owner.is_some_and(|o|
            p.relations.get(&o) == Some(&sim::diplomacy::RelationState::War))) {
            let marines = p.military_sites.iter().filter(|s| s.owner.is_some_and(|o|
                p.relations.get(&o) == Some(&sim::diplomacy::RelationState::War)))
                .map(|s| sim::ground::break_even_marines(s.intel.garrison_tier as f64, 0.0) * 1.5)
                .fold(40.0, f64::max);
            needs.push((H::Transport, (marines / sim::ship::marine_capacity(H::Transport) as f64).ceil() as u32, 100.0 * profile.military));
        }
        needs
    }

    pub(super) fn structure_value(&self, p: &Observation, kind: K) -> f64 {
        let profile = self.priorities();
        let role = self.role(p);
        // §fuel: Fuel made at home needs Volatiles at home. A refinery fed by
        // imported Volatiles saves no freight over importing the Fuel itself.
        let volatiles = p.home.system.bodies.iter().any(|b| b.deposits.iter().any(|d| d.resource == C::Volatiles));
        let base = match kind {
            K::MiningComplex => if role == Role::Mining { 125.0 } else { 45.0 },
            K::VolatileHarvester => if role == Role::Fuel || volatiles { 150.0 } else { 35.0 },
            K::FuelRefinery => if role == Role::Fuel || volatiles
                && (p.home.system.tier(K::VolatileHarvester) > 0 || p.stock(C::Volatiles) >= 1.0) { 150.0 } else { 20.0 },
            K::Bioharvester | K::Agroplex => if role == Role::Agriculture || p.stock(C::Provisions) < 10.0 { 140.0 } else { 35.0 },
            // §gaia: terraces grow only on Gaia worlds; a bot's home is never one.
            K::LivingTerraces => if p.home.system.bodies.iter().any(|b| b.profile.environment == sim::body::Environment::Gaia) {
                if role == Role::Agriculture || p.stock(C::Provisions) < 10.0 { 140.0 } else { 35.0 }
            } else { 0.0 },
            K::Academy => 75.0 * profile.science * if role == Role::Science { 1.5 } else { 1.0 },
            K::Smelter => 85.0 * profile.industry,
            K::ElectronicsFabricator | K::ChemicalWorks | K::MachineWorks | K::CompositeWorks
                | K::HullFabricator | K::PrecisionWorks | K::DriveWorks =>
                if role == Role::Industry { 100.0 * profile.industry } else { 20.0 },
            K::ArmamentsComplex => 65.0 * profile.military,
            K::Warehouse | K::OrbitalWarehouse => if p.home.system.storage_headroom() < p.home.system.storage_cap() * 0.3 { 180.0 } else { 15.0 },
            K::Habitat => if p.home.system.bodies.iter().any(|b| b.population > 0.0
                && b.population + b.inbound_migrants as f64 / 1_000_000.0 + 0.002
                    >= sim::colony::POP_CAP_PER_HABITAT_TIER * b.tier(K::Habitat) as f64
                        * b.habitat_capacity_mult() * sim::research::mod_of(&p.research, ModKey::HabitatCap))
                { 140.0 * profile.expansion } else { 15.0 },
            K::DefensePlatform | K::OrbitalDefense | K::Garrison => if p.threatened(p.home.system.pos) { 200.0 } else { 55.0 * profile.military },
            K::OrdnanceFoundry => if p.own().any(|f| f.damage.unwrap_or(0.0) > 0.15) { 200.0 } else { 40.0 * profile.military },
            K::SensorArray => 50.0 * profile.science,
            K::ServiceBerth => 10.0,
            K::CommandCenter => if p.home.system.bodies.iter().any(|b| b.tier(kind) > 0
                && b.pool_slots_built(sim::build::SlotPool::Infrastructure) >= b.infrastructure_slots()) {
                170.0 * profile.expansion
            } else { 45.0 * profile.industry },
            K::Shipyard | K::NavalDrydock | K::CapitalSlipway => 20.0,
            // §power: placed per planet by `power_project`, valued there.
            K::PowerPlant => 0.0,
            // §fuel-overhaul: the home's own slow-freight fuel, from its ore.
            K::UraniumMill => if p.home.system.tier(K::UraniumMill) == 0
                && p.home.system.bodies.iter().any(|b| b.deposits.iter().any(|d| d.resource == C::MetallicOre))
                { 170.0 } else { 25.0 },
            // The last tech's factory: antimatter for the jump bottles.
            K::AntimatterPlant => 40.0 * profile.military,
        };
        base / (1.0 + p.home.system.tier(kind) as f64 * 0.45)
    }

    fn yard_project(&self, p: &Observation, kind: K, tier: u32) -> Option<Project> {
        if let Some((parent, need)) = sim::build::yard_prereq(kind) {
            if p.home.system.tier(parent) < need { return self.yard_project(p, parent, need); }
        }
        p.structure(kind, tier)
    }

    pub(super) fn strategic_project(&self, p: &Observation) -> Option<Project> {
        // Preserve the short, understandable opening; beyond these necessities
        // select a benefit/cost goal, not a finite list ending at tier two.
        if p.home.system.id == p.capital {
            // §fuel-overhaul: the Uranium Mill right after the mine.
            for project in [p.structure(K::MiningComplex, 1), p.structure(K::UraniumMill, 1), p.structure(K::Shipyard, 1),
                (p.own().all(|f| !f.kind.is_player_freighter())).then(|| p.ship(H::TinyFreighter, 1)).flatten(),
                p.structure(K::Academy, 1),
                // §charter v3: the survey step's stop is the Survey Probe's job,
                // one probe per outstanding stop until its report lands.
                self.charter_probe_due(p).and_then(|_| p.ship(H::SurveyProbe, 1)),
                p.ship(H::Scout, 1),
                // §belts: the charter's barge lesson, once the surveys are in.
                p.belt_lesson.and_then(|_| p.ship(H::MiningBarge, 1))] {
                if project.is_some() { return project; }
            }
        }
        if let Some(project) = self.recovery_project(p) { return Some(project); }
        let mut choices = Vec::new();
        // §power: a planet short of power slows everything on it; its plant
        // outranks almost any other project.
        if let (Some(project), Some((_, short))) = (p.power_project(), p.power_shortfall()) {
            choices.push((160.0 + 10.0 * short, project));
        }
        for kind in K::ALL {
            if let Some(project) = p.structure(kind, p.home.system.tier(kind) + 1) {
                choices.push((self.structure_value(p, kind), project));
            }
        }
        // Each developed port may construct its own relief carriers/escorts.
        for (kind, desired, value) in self.fleet_needs(p) {
            if desired <= p.count(kind) { continue; }
            if let Some(project) = p.ship(kind, desired) {
                let supplies = self.desired_stock(p, None).values().sum::<f64>();
                if p.project_costs(project).values().sum::<f64>() + supplies > p.home.system.storage_cap() {
                    if let Some(storage) = p.structure(K::Warehouse, p.home.system.tier(K::Warehouse) + 1)
                        .or_else(|| p.structure(K::OrbitalWarehouse, p.home.system.tier(K::OrbitalWarehouse) + 1)) {
                        choices.push((value + 40.0, storage));
                    }
                } else { choices.push((value, project)); }
            } else if !sim::ship::requires_hull_unlock(kind) || sim::research::has_hull(&p.research, kind) {
                let (yard, tier) = sim::build::yard_for(kind);
                if let Some(project) = self.yard_project(p, yard, tier) { choices.push((value + 10.0, project)); }
            }
        }
        choices.into_iter().filter(|(_, project)| {
            let cost = p.project_costs(*project).iter().map(|(&c, n)| n * p.price(c)).sum::<f64>();
            // Commit only what can be financed, including goods already local.
            let shortfall = p.project_costs(*project).iter().map(|(&c, n)| (n - p.stock(c)).max(0.0) * p.price(c) * 1.3).sum::<f64>();
            cost.is_finite() && shortfall <= self.spendable(p, false)
        }).max_by(|(a, pa), (b, pb)| {
            let score = |value: f64, project: Project| {
                let cost = p.project_costs(project).iter().map(|(&c, n)| n * p.price(c)).sum::<f64>();
                value / (1.0 + cost / 10_000.0).sqrt()
            };
            score(*a, *pa).total_cmp(&score(*b, *pb))
        }).map(|(_, project)| project)
    }

    pub(super) fn research_value(&self, p: &Observation, programme: &sim::research::Programme) -> f64 {
        let profile = self.priorities();
        let field = match programme.field {
            Field::Propulsion => profile.trade, Field::Materials => profile.industry,
            Field::Computation => profile.science, Field::Weapons | Field::Hulls => profile.military,
            Field::Life => profile.expansion,
        };
        let mut value = 20.0 * field;
        for effect in programme.effects {
            value += match effect {
                Effect::UnlockHull(h) => if h.is_player_freighter() { 80.0 * profile.trade }
                    else if h.is_combatant() { 65.0 * profile.military } else { 40.0 * profile.expansion },
                Effect::UnlockStructureTier(k, _) => self.structure_value(p, *k),
                Effect::Mods(mods) => mods.iter().map(|(m, _)| match m {
                    ModKey::FuelCapacity | ModKey::FuelConsumption if p.fuel_critical() => 180.0,
                    _ => 12.0 * field,
                }).sum(),
                Effect::Flag(_) | Effect::UnlockModule(_) => 25.0 * field,
            };
        }
        // Buildings often use prerequisite IDs rather than an effect entry.
        for k in K::ALL {
            if k.research_prerequisite() == Some(programme.id) { value += self.structure_value(p, k); }
        }
        // An available predecessor can advance a desired locked ladder. Normal
        // availability (including activity gates) still decides what is legal.
        for goal in sim::research::CATALOG.iter().filter(|g| !g.hidden && !p.research.has(g.id)
            && g.field == programme.field && g.tier > programme.tier
            && (programme.school.is_none() || programme.school == g.school)) {
            if goal.effects.iter().any(|e| matches!(e, Effect::UnlockHull(_) | Effect::UnlockStructureTier(_, _))) {
                value += 8.0 * field / (goal.tier - programme.tier) as f64;
            }
        }
        value / (programme.tier as f64).sqrt()
    }

    pub(super) fn industry_buffers(&self, p: &Observation, project: Option<Project>, wanted: &mut BTreeMap<C, f64>) {
        let lead = p.supply_lead() * 2.0;
        let mut flow = BTreeMap::<C, f64>::new();
        for body in &p.home.system.bodies {
            for (&kind, &tier) in &body.structures {
                let Some(a) = body.assignments.get(&kind).filter(|a| a.workers > 0) else { continue; };
                let Some(recipe) = sim::production::assigned_converter(kind, Some(a)) else { continue; };
                // Food has its own Provisions target. An Agroplex runs on the
                // home's own Biomass; importing Biomass to feed it full-rate
                // only churns freight into Provisions the colony can't eat.
                if recipe.output == C::Provisions { continue; }
                let rate = recipe.rate * sim::production::tier_throughput(tier)
                    * a.workers.min(tier) as f64 / tier.max(1) as f64 * p.home.system.staffing_share();
                for &(c, n) in recipe.inputs { *flow.entry(c).or_default() += rate * n * lead; }
            }
        }
        // §power: each plant's burn, over the same delivery windows (a Solar
        // Array burns nothing).
        for body in &p.home.system.bodies {
            if let Some(fuel) = sim::power::plant_fuel(body.tier(K::PowerPlant)).filter(|_| body.grid.burn > 0.0) {
                *flow.entry(fuel).or_default() += body.grid.burn * lead;
            }
        }
        let total = flow.values().sum::<f64>();
        let scale = (p.home.system.storage_cap() * 0.35 / total.max(1.0)).min(1.0);
        for (c, n) in flow { *wanted.entry(c).or_default() += (n * scale).ceil(); }
        if project.is_some_and(|j| matches!(j.what, BuildKind::Upgrade { upgrade: K::OrdnanceFoundry }))
            || p.home.system.tier(K::OrdnanceFoundry) > 0 && p.own().any(|f| f.damage.unwrap_or(0.0) > 0.05) {
            *wanted.entry(C::Alloys).or_default() += 30.0;
            *wanted.entry(C::Machinery).or_default() += 10.0;
        }
        if self.specialist_need(p).is_some() {
            for &(c, n) in sim::specialist::ACADEMY_TRAIN_COSTS { *wanted.entry(c).or_default() += n; }
        }
    }

    pub(super) fn specialist_need(&self, p: &Observation) -> Option<sim::specialist::SpecialistKind> {
        if p.home.system.tier(K::Academy) == 0 || self.recovering(p) { return None; }
        sim::specialist::SpecialistKind::ALL.into_iter().filter_map(|kind| {
            let need: u32 = p.home.system.bodies.iter().flat_map(|b| b.assignments.iter()
                .filter(move |(k, a)| kind.affine(**k) && a.workers > 0)
                .map(move |(k, a)| b.tier(*k).min(a.workers + a.specialists.values().sum::<u32>()))) .sum();
            let have = p.home.system.specialists.get(&kind).copied().unwrap_or(0)
                + p.home.builds.iter().filter(|b| b.what == BuildKind::Train { specialist: kind }).count() as u32;
            (need > have).then_some((need - have, kind))
        }).max_by_key(|(short, _)| *short).map(|(_, kind)| kind)
    }
}

impl Observation {
    pub(super) fn at_site(&self, site: &sim::information::SiteReport) -> Self {
        let mut local = self.clone(); local.home = site.clone(); local
    }
}
