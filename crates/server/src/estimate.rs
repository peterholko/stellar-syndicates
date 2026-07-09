//! The STALE-INTEL battle calculator (§FLEETS Part 3).
//!
//! When a player is about to commit an intercept/raid, they get a PROJECTED
//! engagement estimate — computed by running the SAME shared Lanchester
//! attrition ([`sim::project_engagement`]) forward, fed ONLY by that observer's
//! own view data:
//!
//!   * **Own fleet** — exact (you know your own ships).
//!   * **Target** — its ghost at the retarded state: exact composition when it's
//!     in your sensor coverage, otherwise a **typical warfleet of the bucket
//!     midpoint** ("assuming typical hulls") — provably NEVER the true count.
//!   * **Defenses** — a defense platform from the player's aging scout snapshot
//!     if one covers the target, else unknown.
//!
//! It MUST call the shared combat function (no reimplementation, no drift) and
//! MUST NOT touch authoritative state — it only reads the world (own fleet) and
//! the view filter (what the observer can see). The output is honest about the
//! age of every input.

use sim::{PlayerId, Vec2, World};

use crate::protocol::{CompCount, EngagementEstimate};
use crate::view::{NodeEffects, PositionHistory};

/// Ticks to project forward before giving up (a hard cap for a runaway
/// stalemate; real engagements resolve far sooner).
const MAX_PROJECTION_TICKS: u32 = 100_000;

fn comp_to_view(comp: &std::collections::BTreeMap<sim::ShipKind, u32>) -> Vec<CompCount> {
    comp.iter().filter(|(_, n)| **n > 0).map(|(k, n)| CompCount { kind: *k, count: *n }).collect()
}

/// Project the engagement `attacker` (one of `viewer`'s fleets) vs `target`,
/// from `viewer`'s command center at `cc`. Returns `None` if the attacker isn't
/// the viewer's or the target isn't currently observable at all.
#[allow(clippy::too_many_arguments)]
pub fn estimate_engagement(
    world: &World,
    history: &PositionHistory,
    viewer: PlayerId,
    cc: Vec2,
    c: f64,
    now: f64,
    arrays: &[(Vec2, f64)],
    attacker: sim::EntityId,
    target: sim::EntityId,
) -> Option<EngagementEstimate> {
    // Own fleet — EXACT (the player owns it, so it's fair to know it precisely),
    // carrying its current damage pools.
    let own_fleet = world.fleets.get(&attacker)?;
    if own_fleet.owner != viewer {
        return None; // can only estimate for your own attacker
    }
    let own = sim::Forces::from_fleet(&own_fleet.composition, &own_fleet.damage);

    // TARGET — only what the observer's view reveals. Find its ghost. §node: feed
    // the viewer's regional effects so a Deep-Scan target reads as exact (the
    // estimate honours the same tactical certainty the map shows).
    let veil = world.active_veil_regions();
    let deep = world.deep_scan_regions(viewer);
    let ghosts = history.view_for_with_arrays(
        viewer, cc, c, now, arrays, &std::collections::BTreeSet::new(),
        NodeEffects { veil: &veil, deep_scan: &deep },
    );
    let ghost = ghosts.into_iter().find(|g| g.id == target)?;
    let composition_age = ghost.age;

    // Exact composition when in coverage; otherwise the bucket-midpoint typical
    // warfleet — the fog-leak invariant: the projection is a function of the
    // BUCKET, never the target's true count.
    let (mut target_forces, target_known) = match &ghost.composition {
        Some(comp) => {
            let mut c = std::collections::BTreeMap::new();
            for cc in comp {
                c.insert(cc.kind, cc.count);
            }
            (sim::Forces::from_fleet(&c, &std::collections::BTreeMap::new()), true)
        }
        None => (sim::typical_forces(ghost.count_class), false),
    };

    // Fold a scouted defense platform in, if the player has a snapshot covering
    // the target's ghost position — aged, honest ("scouted N ago").
    let mut defenses_age = None;
    let mut platform_tiers = None;
    if let Some(corp) = world.players.get(&viewer) {
        let covering = corp
            .intel
            .values()
            .filter(|snap| snap.defense_tier > 0 && snap.pos.distance(ghost.pos) <= sim::build::DEFENSE_PLATFORM_RADIUS)
            .min_by(|a, b| a.pos.distance(ghost.pos).total_cmp(&b.pos.distance(ghost.pos)));
        if let Some(snap) = covering {
            target_forces = target_forces.with_platform(snap.defense_tier, 0.0);
            defenses_age = Some((now - snap.observed_at).max(0.0));
            platform_tiers = Some(snap.defense_tier);
        }
    }

    // Rate + retreat, mirroring the authoritative sim: a cargo raid (target
    // flagship convoy) is a skirmish; own doctrine's threshold governs when the
    // attacker would withdraw (the target's is unknown → assume it fights on).
    let raid = ghost.kind == sim::ShipKind::Convoy;
    // A raid uses the fixed quick RAID_RATE; a battle uses the config-scaled rate.
    let rate = if raid {
        sim::combat::RAID_RATE
    } else {
        sim::combat::dmg_rate(world.config.battle_target_secs)
    };
    let own_retreat = world
        .players
        .get(&viewer)
        .and_then(|c| c.doctrine.retreat.min_ratio())
        .map(|m| 1.0 - m);

    // Run the SHARED attrition forward — no reimplementation, no drift.
    let (own_after, target_after, own_losses, target_losses) =
        sim::project_engagement(&own, &target_forces, rate, own_retreat, None, MAX_PROJECTION_TICKS);

    Some(EngagementEstimate {
        attacker,
        target,
        own_losses: comp_to_view(&own_losses.per_kind),
        target_losses: comp_to_view(&target_losses.per_kind),
        own_survivors: comp_to_view(&own_after.comp),
        target_survivors: comp_to_view(&target_after.comp),
        target_known,
        target_count_class: ghost.count_class,
        composition_age,
        defenses_age,
        platform_tiers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{Command, Fleet, FleetOrder, ShipKind, SimConfig};

    /// Build a world with a viewer whose attacker fleet faces a rival target.
    fn setup(target_out_of_coverage: bool) -> (World, PositionHistory, PlayerId, Vec2, sim::EntityId, sim::EntityId) {
        let mut w = World::new(SimConfig::for_players(4242, 4));
        let (me, rival) = (PlayerId(1), PlayerId(2));
        w.step(&[
            Command::AddPlayer { id: me, name: "Me".into() },
            Command::AddPlayer { id: rival, name: "Rival".into() },
        ]);
        let cc = w.players[&me].command_center;
        // My attacker: a 6-raider fleet near my command center (in coverage).
        let aid = sim::EntityId(900_001);
        let mut atk = Fleet::single(aid, me, ShipKind::Raider, cc + Vec2::new(100.0, 0.0), FleetOrder::Idle, None);
        atk.composition.insert(ShipKind::Raider, 6);
        w.fleets.insert(aid, atk);
        // The rival target: a big BROADCASTING corvette fleet (so it's visible
        // galaxy-wide even out of sensor range — a dark fleet far away would just
        // be omitted). Placed just outside the sensor bubble, or well inside it.
        let far = 3_000.0; // beyond the 2200 su sensor bubble, still broadcasting
        let tpos = if target_out_of_coverage { cc + Vec2::new(far, 0.0) } else { cc + Vec2::new(150.0, 0.0) };
        let tid = sim::EntityId(900_002);
        let mut tgt = Fleet::single(tid, rival, ShipKind::Corvette, tpos, FleetOrder::Idle, None);
        tgt.composition.insert(ShipKind::Corvette, 25); // TRUE count = 25 (bucket 16–30)
        w.fleets.insert(tid, tgt);

        let mut hist = PositionHistory::for_world(&w);
        // Record enough ticks that the target's (broadcast) light has arrived
        // even at 3000 su (10 s delay ≈ 300 ticks).
        for _ in 0..420 {
            w.step(&[]);
            hist.record(&w);
        }
        (w, hist, me, cc, aid, tid)
    }

    #[test]
    fn in_coverage_uses_exact_composition() {
        let (w, hist, me, cc, aid, tid) = setup(false);
        let est = estimate_engagement(&w, &hist, me, cc, 300.0, w.time, &[], aid, tid).expect("estimate");
        assert!(est.target_known, "a target in sensor coverage is estimated from its EXACT composition");
    }

    #[test]
    fn out_of_coverage_uses_the_bucket_midpoint_never_the_true_count() {
        // LEAK CHECK: the true target is 25 raiders (bucket 16–30, midpoint 23).
        // Out of coverage, the estimate must be built from the MIDPOINT typical
        // warfleet — its projected target losses can never imply the true 25.
        let (w, hist, me, cc, aid, tid) = setup(true);
        let est = estimate_engagement(&w, &hist, me, cc, 300.0, w.time, &[], aid, tid).expect("estimate");
        assert!(!est.target_known, "an out-of-coverage target is a typical-hull estimate");
        assert_eq!(est.target_count_class, sim::CountClass::SixteenToThirty);
        // The typical fleet has midpoint(23) ships; total modelled ≤ 23, never 25.
        let modelled = est.target_losses.iter().map(|c| c.count).sum::<u32>()
            + est.target_survivors.iter().map(|c| c.count).sum::<u32>();
        assert!(modelled <= sim::CountClass::SixteenToThirty.midpoint(), "modelled {modelled} must not exceed the bucket midpoint (never the true 25)");
        assert!(modelled < 25, "the estimate provably does NOT use the true count of 25");
    }

    #[test]
    fn estimate_never_mutates_authoritative_state() {
        let (mut w, hist, me, cc, aid, tid) = setup(false);
        let before = serde_json::to_string(&w).unwrap();
        let _ = estimate_engagement(&w, &hist, me, cc, 300.0, w.time, &[], aid, tid);
        // Re-borrow mutably only to serialize again; the estimate took &World.
        let after = serde_json::to_string(&w).unwrap();
        assert_eq!(before, after, "the calculator must not touch authoritative state");
        let _ = &mut w;
    }
}
