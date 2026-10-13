//! More places to survey without multiplying the habitable-world budget.
use super::*;
use crate::body::{Body, BodyKind, BodySize, Environment, Geology, PlanetaryProfile};
use crate::cargo::Commodity;
use crate::galaxy::{DEPOSIT_BASE_RICHNESS, Deposit};
use crate::rng::Rng;
use std::collections::BTreeSet;

const STAR_SPACING_SU: f64 = 12_000.0;
const EMPTY_PROSPECT_FRACTION: f64 = 0.20;
const STRONG_PROSPECT_FRACTION: f64 = 0.10;

impl World {
    /// NEW galaxies only, after baseline geography generation. Isolated RNGs,
    /// names and appended ids preserve the original systems byte-for-byte,
    /// including homes, starter opportunities, pirate bases and spectral bands.
    /// Habitability is a roster invariant, not a lower probability: none of
    /// these additional bodies can be Gaia/Terran. Do not call from save fixup.
    pub(super) fn seed_exploration_stars(&mut self) {
        let count = self.config.exploration_system_count as usize;
        if count == 0 {
            return;
        }
        let taken: BTreeSet<_> = self.systems.iter().map(|s| s.name.clone()).collect();
        let mut names_rng = Rng::keyed(self.config.seed, "exploration-star-names-v1");
        let names = crate::galaxy::shuffled_system_names(&mut names_rng, taken.len() + count);
        // Local survey stops reuse two extra-star slots per home. The original
        // garden/mineral colony prospects remain untouched and optional.
        let home_clearance: Vec<_> = self
            .home_slots
            .iter()
            .map(|home| {
                let radius = home
                    .founding_opportunities
                    .iter()
                    .filter_map(|id| {
                        self.systems
                            .iter()
                            .find(|s| s.id == *id)
                            .map(|s| s.pos.distance(home.pos))
                    })
                    .fold(STAR_SPACING_SU, f64::max);
                (home.pos, radius)
            })
            .collect();
        let mut rng = Rng::keyed(self.config.seed, "exploration-star-positions-v1");
        let mut added = Vec::with_capacity(count);
        let mut local_ice = Vec::new();
        for (i, name) in names
            .into_iter()
            .filter(|n| !taken.contains(n))
            .take(count)
            .enumerate()
        {
            let local = self.home_slots.get(i / 2).map(|h| h.pos);
            let safe = |pos: Vec2| pos.distance(self.hub) > STAR_SPACING_SU
                && pos.length() <= self.config.galaxy_radius
                && self.systems.iter().all(|s| pos.distance(s.pos) >= STAR_SPACING_SU)
                && self.exploration.sites.values().all(|s| pos.distance(s.pos) > 3_000.0);
            let nearby = local.and_then(|home| (0..512).find_map(|attempt| {
                let radius = 18_000.0 + (attempt / 128) as f64 * 5_000.0;
                let pos = home + Vec2::from_polar((attempt % 128) as f64 * TAU / 128.0 + i as f64, radius);
                safe(pos).then_some(pos)
            }));
            let pos = nearby.unwrap_or_else(|| (0..4096)
                .find_map(|_| {
                    let r = self.config.galaxy_radius * (0.12 + 0.84 * rng.next_f64().sqrt());
                    let pos = Vec2::from_polar(rng.range(0.0, TAU), r);
                    (pos.distance(self.hub) > STAR_SPACING_SU
                        && home_clearance
                            .iter()
                            .all(|(home, radius)| pos.distance(*home) > *radius)
                        && self
                            .systems
                            .iter()
                            .all(|s| pos.distance(s.pos) >= STAR_SPACING_SU)
                        && self
                            .exploration
                            .sites
                            .values()
                            .all(|s| pos.distance(s.pos) > 3_000.0))
                    .then_some(pos)
                })
                .expect("galaxy too crowded for exploration stars with safe spacing"));
            let id = self.alloc_entity_id();
            let mut body_rng =
                Rng::keyed_id(self.config.seed, "exploration-star-bodies-v1", i as u64);
            // §ore-ladder: the star's own frontier factor (the same 0.12R..0.96R
            // band the established generator uses), so the rim gate on the
            // scarce ores holds for every star in the chart.
            let frontier = ((pos.distance(self.hub) / self.config.galaxy_radius - 0.12) / 0.84)
                .clamp(0.0, 1.0);
            let bodies = if nearby.is_some() {
                let icy = i % 2 == 1;
                let mut b = sparse_body(0, format!("{name} I"), if icy { BodyKind::Ice } else { BodyKind::Rocky }, None);
                b.deposits.push(Deposit { resource: if icy { Commodity::Volatiles } else { Commodity::MetallicOre },
                    richness: DEPOSIT_BASE_RICHNESS * 0.65, reserves: None, accessibility: 0.75 });
                b.ensure_profile(&id.0.to_string());
                b.profile.environment = Environment::Uninhabitable;
                b.profile.size = BodySize::Small;
                if !icy { b.profile.special = Some(crate::body::BodySpecial::PrecursorRuins); }
                if icy { local_ice.push(pos); }
                vec![b]
            } else { sparse_bodies(id, &name, &mut body_rng, frontier) };
            let deposits: Vec<_> = bodies
                .iter()
                .flat_map(|b| b.deposits.iter().cloned())
                .collect();
            self.systems.push(crate::galaxy::unowned_system(
                id,
                pos,
                name,
                bodies,
                crate::galaxy::claim_cost_for(&deposits),
            ));
            if nearby.is_some() && i % 2 == 0 {
                // §belts: the rocky local stop keeps a belt with ore. The
                // founding charter's barge lesson mines it once the local
                // surveys have charted it: close enough that a barge with a
                // full hold comes home on one tank. Its own keyed stream.
                let mut belt_rng = Rng::keyed_id(self.config.seed, "local-stop-belt-v1", i as u64);
                let belt_frontier = crate::belts::frontier_at(pos, self.config.galaxy_radius);
                let belt = crate::belts::prospect_belt(&mut belt_rng, belt_frontier);
                crate::belts::add_belts(self.systems.last_mut().unwrap(), vec![belt]);
            }
            self.exploration_systems.insert(id);
            // Exotic-star rewards keep matching the public star artwork.
            if let Some(bonus) = crate::node::node_bonus_for(id) {
                self.nodes.insert(id, crate::node::Node::dormant(bonus));
            }
            added.push(pos);
        }
        self.seed_exploration_star_sites(&added);
        for (i, pos) in local_ice.into_iter().enumerate() {
            if self.exploration.sites.values().any(|s| s.details.kind == crate::sites::SiteKind::Station && s.pos.distance(pos) < 15_000.0) { continue; }
            if let Some(site) = self.clear_exploration_position(pos + Vec2::new(6_000.0, 0.0), 4_000.0, &mut rng) {
                self.create_exploration_site(site, crate::sites::SiteKind::Station, i as u64);
            }
        }
    }
}

/// One or two planets, sometimes an icy moon; never the full generator's
/// 3–8-planet filler roll. Keep at least one real body: an empty roster means
/// "legacy save needing migration" elsewhere, not an intentionally empty star.
fn sparse_bodies(id: EntityId, name: &str, rng: &mut Rng, frontier: f64) -> Vec<Body> {
    let icy = rng.next_f64() < 0.40;
    let gas_giant = icy && rng.next_f64() < 0.50;
    let mut bodies = vec![sparse_body(
        0,
        format!("{name} I"),
        if gas_giant {
            BodyKind::GasGiant
        } else if icy {
            BodyKind::Ice
        } else {
            BodyKind::Rocky
        },
        None,
    )];
    if rng.next_f64() < 0.35 {
        bodies.push(sparse_body(1, format!("{name} II"), BodyKind::Rocky, None));
    }
    let deposit_body = if gas_giant {
        let moon = bodies.len();
        bodies.push(sparse_body(
            moon as u32,
            format!("{name} I-a"),
            BodyKind::Ice,
            Some(0),
        ));
        moon
    } else {
        0
    };
    // Most prospects are modest and lack food production. Roughly one in ten
    // can justify a supplied specialist outpost, not a second all-purpose home.
    let roll = rng.next_f64();
    let strong = roll >= 1.0 - STRONG_PROSPECT_FRACTION;
    if roll >= EMPTY_PROSPECT_FRACTION {
        // §ore-ladder: a bare prospect draws from the same rarity table as the
        // established galaxy (ores only — no food or gas on rock) at its own
        // distance, and the scarce ores roll finite reserves here too.
        let resource = if icy {
            Commodity::Volatiles
        } else {
            crate::galaxy::roll_raw_deposit_where(rng, frontier, |c| c.is_ore())
        };
        let (lo, hi) = if strong { (0.90, 1.10) } else { (0.30, 0.60) };
        let vein = rng.range(lo, hi);
        let richness = DEPOSIT_BASE_RICHNESS * vein;
        let reserves = crate::galaxy::reserves_for(resource, (vein - lo) / (hi - lo));
        bodies[deposit_body].deposits.push(Deposit {
            resource,
            richness,
            reserves,
            accessibility: rng.range(0.3, 0.9),
        });
    }
    for body in &mut bodies {
        body.ensure_profile(&id.0.to_string());
        // This explicit boundary protects the fixed budget even if the normal
        // profile generator later permits a habitable rocky/icy body.
        body.profile.environment = match body.profile.environment {
            Environment::Hostile => Environment::Hostile,
            _ => Environment::Uninhabitable,
        };
        body.habitable = false;
        if body.kind != BodyKind::GasGiant {
            body.profile.size = if body.parent.is_some() {
                BodySize::Tiny
            } else {
                BodySize::Small
            };
        }
    }
    if strong {
        bodies[deposit_body].profile.geology = if rng.next_f64() < 0.5 {
            Geology::UltraRich
        } else {
            Geology::Rich
        };
    }
    bodies
}

fn sparse_body(id: u32, name: String, kind: BodyKind, parent: Option<u32>) -> Body {
    Body {
        founding_camp: None,
        id,
        name,
        kind,
        parent,
        belt: None,
        habitable: false,
        profile: PlanetaryProfile::default(),
        deposits: Vec::new(),
        pockets: Vec::new(),
        prospected: Default::default(),
        structures: BTreeMap::new(),
        population: 0.0,
        migration_policy: Default::default(),
        inbound_migrants: 0,
        assignments: BTreeMap::new(),
        grid: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn habitable_count(world: &World) -> usize {
        world
            .systems
            .iter()
            .flat_map(|s| &s.bodies)
            .filter(|b| {
                assert_eq!(b.habitable, b.profile.environment.naturally_habitable());
                b.habitable
            })
            .count()
    }

    #[test]
    fn more_stars_preserve_the_exact_habitable_budget_and_starter_galaxy() {
        for players in [1, 4, 5, 10] {
            for seed in [0, 1, 42, 123, 992, 0xC0FFEE, u64::MAX] {
                let config = SimConfig::for_players(seed, players);
                let mut baseline = config.clone();
                baseline.exploration_system_count = 0;
                let old = World::new(baseline);
                let added = config.exploration_system_count as usize;
                let new = World::new(config);
                assert_eq!(new.systems.len(), old.systems.len() + added);
                assert_eq!(
                    &new.systems[..old.systems.len()],
                    old.systems.as_slice(),
                    "seed {seed}, players {players}: existing systems must not change"
                );
                assert_eq!(habitable_count(&new), habitable_count(&old));
                assert_eq!(
                    serde_json::to_value(&new.home_slots).unwrap(),
                    serde_json::to_value(&old.home_slots).unwrap()
                );
                assert_eq!((new.band_lo, new.band_hi), (old.band_lo, old.band_hi));
                for home in &new.home_slots {
                    let nearby: Vec<_> = new.systems.iter().filter(|s| new.exploration_systems.contains(&s.id)
                        && s.pos.distance(home.pos) <= 34_000.0).collect();
                    assert!(nearby.len() >= 2, "seed {seed}, players {players}: two local survey destinations");
                    assert!(nearby.iter().all(|s| s.bodies.iter().all(|b| !b.habitable)));
                }
                assert_eq!(
                    serde_json::to_value(new.rng).unwrap(),
                    serde_json::to_value(old.rng).unwrap(),
                    "extra exploration must not consume the simulation's random stream"
                );
                for (id, site) in &old.exploration.sites {
                    assert_eq!(
                        serde_json::to_value(&new.exploration.sites[id]).unwrap(),
                        serde_json::to_value(site).unwrap()
                    );
                }
                for (id, enclave) in &old.enclaves {
                    assert_eq!(
                        serde_json::to_value(&new.enclaves[id]).unwrap(),
                        serde_json::to_value(enclave).unwrap()
                    );
                }
                let names: BTreeSet<_> = new.systems.iter().map(|s| &s.name).collect();
                assert_eq!(names.len(), new.systems.len());
                for s in &new.systems[old.systems.len()..] {
                    // §belts: sparse stars may also carry bare belts. Only a
                    // home's local survey stop keeps a belt with ore, for the
                    // founding charter's barge lesson.
                    assert!((1..=3).contains(&s.bodies.iter().filter(|b| !b.is_belt()).count()));
                    let local = new.home_slots.iter().any(|h| h.pos.distance(s.pos) <= 34_000.0);
                    assert!(local || s.bodies.iter().filter(|b| b.is_belt()).all(|b| b.deposits.is_empty()));
                    assert!(s.bodies.iter().filter(|b| !b.is_belt()).flat_map(|b| &b.deposits).count() <= 1);
                    assert!(s.all_deposits().all(|d| d.resource != Commodity::Biomass));
                    assert!(s.pos.length() <= new.config.galaxy_radius);
                    assert!(
                        new.systems
                            .iter()
                            .filter(|other| other.id != s.id)
                            .all(|other| s.pos.distance(other.pos) >= STAR_SPACING_SU)
                    );
                    for body in &s.bodies {
                        assert!(!body.habitable);
                        assert!(!body.profile.environment.naturally_habitable());
                        assert!(body.parent.is_none_or(|id| {
                            s.bodies.iter().any(|p| p.id == id && p.parent.is_none())
                        }));
                    }
                }
                if players == 4 && seed == 0xC0FFEE {
                    eprintln!(
                        "default galaxy: {} → {} stars; {} → {} habitable bodies; {} → {} exploration sites",
                        old.systems.len(),
                        new.systems.len(),
                        habitable_count(&old),
                        habitable_count(&new),
                        old.exploration.sites.len(),
                        new.exploration.sites.len()
                    );
                }
            }
        }
    }

    #[test]
    fn sparse_prospects_are_mostly_modest_with_occasional_specialists() {
        let mut empty = 0;
        let mut strong = 0;
        let mut total_richness = 0.0;
        let mut kinds = BTreeSet::new();
        let mut resources = BTreeSet::new();
        for i in 0..1000 {
            let mut rng = Rng::keyed_id(0xC0FFEE, "exploration-star-bodies-v1", i);
            // Spread the prospects over the whole disk so the rim-gated ores show up.
            let frontier = i as f64 / 999.0;
            let bodies = sparse_bodies(EntityId(1000 + i), "Prospect", &mut rng, frontier);
            let deposits: Vec<_> = bodies.iter().flat_map(|b| &b.deposits).collect();
            empty += usize::from(deposits.is_empty());
            for body in &bodies {
                kinds.insert(body.kind.slug());
                assert!(!body.habitable);
                for d in &body.deposits {
                    resources.insert(d.resource.slug());
                    total_richness += d.richness;
                    assert!(d.richness <= DEPOSIT_BASE_RICHNESS * 1.10);
                    if d.richness > DEPOSIT_BASE_RICHNESS * 0.60 {
                        strong += 1;
                        assert!(matches!(
                            body.profile.geology,
                            Geology::Rich | Geology::UltraRich
                        ));
                    }
                }
            }
        }
        assert!(
            (150..=250).contains(&empty),
            "empty prospects: {empty}/1000"
        );
        assert!(
            (60..=140).contains(&strong),
            "strong prospects: {strong}/1000"
        );
        assert!(total_richness / 1000.0 < DEPOSIT_BASE_RICHNESS * 0.50);
        assert_eq!(kinds, BTreeSet::from(["rocky", "ice", "gas_giant"]));
        assert_eq!(resources, BTreeSet::from([
            "metallic_ore", "cuprite_ore", "titanium_ore", "crystalline_ore", "rare_metal_ore", "volatiles",
        ]), "all five ore families and volatile prospects occur");
    }

    #[test]
    fn exploration_stars_are_deterministic_and_survive_save_fixup() {
        let config = SimConfig::default();
        let a = World::new(config.clone());
        let b = World::new(config);
        assert_eq!(a.systems, b.systems);
        assert_eq!(
            serde_json::to_value(&a.exploration).unwrap(),
            serde_json::to_value(&b.exploration).unwrap()
        );
        let mut restored: World =
            serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
        assert_eq!(restored.exploration_systems, a.exploration_systems);
        restored.band_lo = 0.0;
        restored.band_hi = 0.0;
        restored.fixup_after_load();
        assert_eq!(restored.systems.len(), a.systems.len());
        assert_eq!(habitable_count(&restored), habitable_count(&a));
        assert!((restored.band_lo - a.band_lo).abs() < 1e-9);
        assert!((restored.band_hi - a.band_hi).abs() < 1e-9);
        for (original, loaded) in a.systems.iter().zip(&restored.systems) {
            assert_eq!(original.id, loaded.id);
            assert_eq!(original.bodies.len(), loaded.bodies.len());
        }
        // Absent field means an old galaxy, not permission to retrofit stars.
        let mut old_config = serde_json::to_value(SimConfig::default()).unwrap();
        old_config
            .as_object_mut()
            .unwrap()
            .remove("exploration_system_count");
        let config: SimConfig = serde_json::from_value(old_config).unwrap();
        assert_eq!(config.exploration_system_count, 0);
        let mut old = World::new(config);
        let before = old.systems.len();
        old.fixup_after_load();
        assert_eq!(old.systems.len(), before, "a reload never retrofits stars");
    }
}
