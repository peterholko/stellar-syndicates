//! §syndicate-projects (R6, `crate::syndicate`): each week a syndicate climbs
//! a ladder of shared deliveries. Members give goods from their own worlds;
//! a finished rung earns project points, and points buy lasting bonuses
//! (freight holds, research, migration) that reach a member as its tenure
//! allows. At the week's close the three syndicates that earned the most
//! points take medals, and every ladder starts again.
use super::*;
use crate::syndicate::*;

impl World {
    /// The medal week now running (1-based).
    pub(super) fn medal_week(&self) -> u32 {
        (self.tick / MEDAL_WEEK_TICKS) as u32 + 1
    }

    /// The multiplier `bonus` gives `player` now: its syndicate's level, cut to
    /// what the member's tenure has reached. 1 outside a syndicate.
    pub fn syndicate_mult(&self, player: PlayerId, bonus: SyndicateBonus) -> f64 {
        1.0 + bonus.per_level() * self.syndicate_level_for(player, bonus) as f64
    }

    /// The level of `bonus` that has reached `player`.
    pub fn syndicate_level_for(&self, player: PlayerId, bonus: SyndicateBonus) -> u8 {
        let Some(corp) = self.players.get(&player) else { return 0 };
        let Some(s) = corp.syndicate.and_then(|sid| self.syndicates.get(&sid)) else { return 0 };
        level_reaching(s.projects.level(bonus), self.time - corp.syndicate_since)
    }

    /// A member gives up to `units` of the rung's goods from its world
    /// `system`: never more than the rung still needs or the world holds free.
    pub(super) fn contribute_syndicate_project(&mut self, player: PlayerId, system: EntityId,
        commodity: crate::Commodity, units: u32, events: &mut Vec<Event>)
    {
        let Some(sid) = self.players.get(&player).and_then(|c| c.syndicate) else { return };
        let Some(projects) = self.syndicates.get(&sid).map(|s| &s.projects) else { return };
        let Some(goal) = projects.goal() else { return };
        if rung_goods(projects.week, projects.rung) != commodity {
            return;
        }
        let remaining = goal.saturating_sub(projects.progress);
        let Some(site) = self.systems.iter_mut().find(|s| s.id == system && s.owner == Some(player)) else { return };
        let take = units.min(site.free_stock(commodity).floor().max(0.0) as u32).min(remaining);
        if take == 0 {
            return;
        }
        let left = site.stockpile.get(&commodity).copied().unwrap_or(0.0) - take as f64;
        if left <= 1e-9 {
            site.stockpile.remove(&commodity);
        } else {
            site.stockpile.insert(commodity, left);
        }
        let projects = &mut self.syndicates.get_mut(&sid).unwrap().projects;
        projects.progress += take;
        *projects.given.entry(player).or_default() += take;
        if projects.progress >= goal {
            let (rung, points) = (projects.rung, LADDER_POINTS[projects.rung]);
            projects.points += points;
            projects.week_points += points;
            projects.rung += 1;
            projects.progress = 0;
            events.push(Event::new(self.time, EventPayload::SyndicateRungFinished {
                syndicate: sid, week: projects.week, rung: rung as u8 + 1, points,
            }));
        }
    }

    /// A manager spends points on the next level of `bonus`.
    pub(super) fn raise_syndicate_bonus(&mut self, player: PlayerId, bonus: SyndicateBonus, events: &mut Vec<Event>) {
        let Some(sid) = self.players.get(&player).and_then(|c| c.syndicate) else { return };
        let Some(s) = self.syndicates.get_mut(&sid) else { return };
        if !s.may_manage_operations(player) {
            return;
        }
        let Some(cost) = s.projects.next_cost(bonus) else { return };
        if s.projects.points < cost {
            return;
        }
        s.projects.points -= cost;
        let level = s.projects.levels.entry(bonus).or_insert(0);
        *level += 1;
        events.push(Event::new(self.time, EventPayload::SyndicateBonusRaised { syndicate: sid, bonus, level: *level }));
    }

    /// At a medal week's close: the podium by points earned this week, then
    /// every ladder starts again at its first rung.
    pub(super) fn close_syndicate_week(&mut self, events: &mut Vec<Event>) {
        let week = (self.tick / MEDAL_WEEK_TICKS) as u32;
        let mut standings: Vec<(u32, SyndicateId)> = self.syndicates.values()
            .filter(|s| s.projects.week_points > 0)
            .map(|s| (s.projects.week_points, s.id))
            .collect();
        standings.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (i, (_, sid)) in standings.into_iter().take(crate::rankings::MEDAL_PLACES).enumerate() {
            let place = i as u8 + 1;
            if let Some(s) = self.syndicates.get_mut(&sid) {
                s.projects.medals.push((week, place));
            }
            events.push(Event::new(self.time, EventPayload::SyndicateMedalWon { syndicate: sid, week, place })
                .at_origin(self.hub));
        }
        for s in self.syndicates.values_mut() {
            s.projects.start_week(week + 1);
        }
    }

    /// The published syndicate table, beside the corporations' (same close).
    pub(super) fn syndicate_ranking_rows(&self) -> Vec<crate::rankings::SyndicateRankingRow> {
        let mut rows: Vec<_> = self.syndicates.values().map(|s| {
            let mut medals = [0u32; 3];
            for &(_, place) in &s.projects.medals {
                if (1..=3).contains(&place) {
                    medals[place as usize - 1] += 1;
                }
            }
            crate::rankings::SyndicateRankingRow {
                id: s.id,
                name: s.name.clone(),
                members: s.members.len() as u32,
                valuation: s.members.iter().filter_map(|m| self.players.get(m)).map(|c| c.valuation).sum(),
                week_points: s.projects.week_points,
                levels: s.projects.levels.values().map(|&l| l as u32).sum(),
                medals,
            }
        }).collect();
        rows.sort_by(|a, b| b.valuation.total_cmp(&a.valuation).then(a.id.cmp(&b.id)));
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_pact() -> (World, PlayerId, PlayerId, SyndicateId, EntityId) {
        let mut w = World::new(SimConfig::for_players(611, 2));
        let (a, b) = (PlayerId(1), PlayerId(2));
        w.step(&[Command::AddPlayer { id: a, name: "A".into() }, Command::AddPlayer { id: b, name: "B".into() }]);
        w.step(&[Command::CreateSyndicate { player_id: a, name: "Pact".into() }]);
        let sid = w.players[&a].syndicate.unwrap();
        w.syndicates.get_mut(&sid).unwrap().members.insert(b);
        w.set_membership(b, Some(sid));
        let home = w.players[&a].home_system.unwrap();
        (w, a, b, sid, home)
    }

    fn give(w: &mut World, player: PlayerId, system: EntityId, units: u32) {
        let week = w.syndicates.values().next().unwrap().projects.week;
        let rung = w.syndicates.values().next().unwrap().projects.rung;
        let goods = rung_goods(week, rung);
        w.systems.iter_mut().find(|s| s.id == system).unwrap().stockpile.insert(goods, units as f64);
        let mut events = Vec::new();
        w.contribute_syndicate_project(player, system, goods, units, &mut events);
    }

    /// Gifts climb the ladder, points buy a level, and a newcomer gets only
    /// the levels its tenure has reached.
    #[test]
    fn the_ladder_buys_bonuses_that_reach_newcomers_with_tenure() {
        let (mut w, a, b, sid, home) = world_with_pact();
        assert_eq!(w.syndicates[&sid].projects.week, 1);
        // Wrong goods change nothing; the rung takes only what it needs.
        let wrong = LADDER_GOODS.iter().copied().find(|&c| c != rung_goods(1, 0)).unwrap();
        w.systems.iter_mut().find(|s| s.id == home).unwrap().stockpile.insert(wrong, 500.0);
        w.contribute_syndicate_project(a, home, wrong, 500, &mut Vec::new());
        assert_eq!(w.syndicates[&sid].projects.progress, 0);
        give(&mut w, a, home, 1_000);
        let p = &w.syndicates[&sid].projects;
        assert_eq!((p.rung, p.progress, p.points, p.week_points), (1, 0, 1, 1));
        assert_eq!(p.given[&a], LADDER_GOALS[0], "only what the rung needed");
        give(&mut w, a, home, LADDER_GOALS[1]);
        assert_eq!(w.syndicates[&sid].projects.points, 3);
        // Only a manager raises a level, and only with the points for it.
        w.raise_syndicate_bonus(b, SyndicateBonus::Freight, &mut Vec::new());
        assert_eq!(w.syndicates[&sid].projects.level(SyndicateBonus::Freight), 0);
        w.raise_syndicate_bonus(a, SyndicateBonus::Freight, &mut Vec::new());
        let p = &w.syndicates[&sid].projects;
        assert_eq!((p.level(SyndicateBonus::Freight), p.points), (1, 0));
        assert!((w.syndicate_mult(a, SyndicateBonus::Freight) - 1.06).abs() < 1e-12);
        // Level II waits a day for a member who joined just now.
        w.syndicates.get_mut(&sid).unwrap().projects.levels.insert(SyndicateBonus::Research, 2);
        w.players.get_mut(&a).unwrap().syndicate_since = w.time - 30.0 * 3_600.0;
        w.players.get_mut(&b).unwrap().syndicate_since = w.time;
        assert_eq!(w.syndicate_level_for(a, SyndicateBonus::Research), 2);
        assert_eq!(w.syndicate_level_for(b, SyndicateBonus::Research), 1);
        assert_eq!(level_reaching(5, 96.0 * 3_600.0), 5);
        assert_eq!(level_reaching(5, 95.0 * 3_600.0), 4);
    }

    /// Freight widens only player freighter holds, for members its tenure
    /// has reached; leaving takes it away.
    #[test]
    fn freight_widens_member_freighter_holds() {
        let (mut w, a, _, sid, _) = world_with_pact();
        let home = w.players[&a].home;
        let id = EntityId(9_400_000_001);
        w.fleets.insert(id, crate::ship::Fleet::single(id, a, ShipKind::SmallFreighter, home, FleetOrder::Idle, None));
        w.fleets.get_mut(&id).unwrap().add(ShipKind::Raider, 1);
        let base = w.fleets[&id].cargo_capacity();
        assert_eq!(base, ShipKind::SmallFreighter.cargo_units() + ShipKind::Raider.cargo_units());
        w.syndicates.get_mut(&sid).unwrap().projects.levels.insert(SyndicateBonus::Freight, 2);
        w.players.get_mut(&a).unwrap().syndicate_since = w.time - 30.0 * 3_600.0;
        w.refresh_fleet_fuel_research();
        let widened = (ShipKind::SmallFreighter.cargo_units() as f64 * 1.12).floor() as u32 + ShipKind::Raider.cargo_units();
        assert_eq!(w.fleets[&id].cargo_capacity(), widened);
        w.set_membership(a, None);
        w.refresh_fleet_fuel_research();
        assert_eq!(w.fleets[&id].cargo_capacity(), base);
    }

    /// The week's close gives medals by points earned, then restarts every
    /// ladder; points and levels stay.
    #[test]
    fn the_week_closes_with_medals_and_a_fresh_ladder() {
        let (mut w, a, _, sid, home) = world_with_pact();
        give(&mut w, a, home, LADDER_GOALS[0]);
        w.syndicates.get_mut(&sid).unwrap().projects.levels.insert(SyndicateBonus::Migration, 1);
        w.tick = MEDAL_WEEK_TICKS;
        let mut events = Vec::new();
        w.close_syndicate_week(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SyndicateMedalWon { syndicate, week: 1, place: 1 } if syndicate == sid)));
        let p = &w.syndicates[&sid].projects;
        assert_eq!((p.week, p.rung, p.week_points, p.points), (2, 0, 0, 1));
        assert_eq!((p.medals.clone(), p.level(SyndicateBonus::Migration)), (vec![(1, 1)], 1));
        assert!(p.given.is_empty());
        let rows = w.syndicate_ranking_rows();
        assert_eq!((rows[0].name.as_str(), rows[0].members, rows[0].medals), ("Pact", 2, [1, 0, 0]));
    }
}
