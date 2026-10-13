//! §belts: the founding charter's last lesson. Once the surveys are in, the
//! programme points at a belt they charted; the player builds a Mining Barge
//! and sends it there, and the lesson ends when command hears it cutting ore.
use super::*;

impl World {
    /// §charter v3: the one system the survey step charts: the home's local
    /// survey stop (18–33k su out), the one keeping a belt with ore first.
    pub(super) fn founding_lesson_stop(&self, home: Vec2) -> Option<EntityId> {
        self.systems
            .iter()
            .filter(|s| self.exploration_systems.contains(&s.id) && s.pos.distance(home) <= 34_000.0)
            .map(|s| {
                let ore_belt = s.bodies.iter().any(|b| b.is_belt() && !b.deposits.is_empty());
                (!ore_belt, s.pos.distance(home), s.id)
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)))
            .map(|(_, _, id)| id)
    }

    /// The charted belt the barge lesson points at: a surveyed prospect's
    /// belt (the mineral prospect, then the garden), else the nearest other
    /// charted belt within sensor range of home, else the home belt. Only
    /// unowned systems or the corporation's own, never a pirate base.
    pub(super) fn founding_belt_target(&self, owner: PlayerId) -> Option<(EntityId, u32)> {
        let corp = self.players.get(&owner)?;
        let home = corp.home_system;
        let system = |id: EntityId| self.systems.iter().find(|s| s.id == id);
        let usable = |s: &StarSystem| {
            Some(s.id) != home
                && (s.owner == Some(owner) || (s.owner.is_none() && corp.surveyed.contains(&s.id)))
                && !self.enclaves.values().any(|e| e.system == s.id)
        };
        let belt_of = |s: &StarSystem| {
            s.bodies
                .iter()
                .find(|b| b.is_belt() && !b.deposits.is_empty())
                .map(|b| (s.id, b.id))
        };
        // The prospects were dealt garden first, mineral second.
        for id in corp.founding.survey_candidates.iter().rev() {
            if let Some(s) = system(*id).filter(|s| usable(s))
                && let Some(target) = belt_of(s)
            {
                return Some(target);
            }
        }
        let origin = home.and_then(system).map_or(corp.home, |s| s.pos);
        let nearest = self
            .systems
            .iter()
            .filter(|s| usable(s) && s.pos.distance(origin) <= self.config.sensor_range)
            .filter_map(|s| belt_of(s).map(|target| (s.pos.distance(origin), target)))
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        if let Some((_, target)) = nearest {
            return Some(target);
        }
        home.and_then(system).and_then(belt_of)
    }

    /// Re-point the lesson when its belt stops being usable (a rival claimed
    /// the system, or the belt has nothing left to mine).
    pub(super) fn refresh_founding_belt_target(&mut self, owner: PlayerId) {
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        let home = corp.home_system;
        let valid = corp.founding.belt_target.is_some_and(|(system, body)| {
            self.systems.iter().find(|s| s.id == system).is_some_and(|s| {
                (s.owner.is_none() || s.owner == Some(owner) || Some(s.id) == home)
                    && s.bodies.iter().any(|b| {
                        b.id == body
                            && b.is_belt()
                            && b.deposits.iter().any(|d| d.reserves.is_none_or(|r| r > 1e-9))
                    })
            })
        });
        if !valid {
            let target = self.founding_belt_target(owner);
            if let Some(corp) = self.players.get_mut(&owner) {
                corp.founding.belt_target = target;
            }
        }
    }

    /// §charter v3: a barge's ore reached a stockpile or the Market warehouse
    /// at `at`; once the barge lesson is under way, that report sets out for
    /// the command center at light speed.
    pub(super) fn note_founding_haul(&mut self, owner: PlayerId, at: Vec2) {
        let c = self.config.c;
        let now = self.time;
        if let Some(corp) = self.players.get_mut(&owner)
            && corp.founding.enabled
            && matches!(
                corp.founding.stage,
                crate::founding::FoundingStage::MineBelt | crate::founding::FoundingStage::HaulOre
            )
            && corp.founding.haul_report_at.is_none()
        {
            corp.founding.haul_report_at = Some(now + crate::transit::delay(at, corp.command_center, c));
        }
    }

    /// Once one of the corporation's barges has cut ore at a belt away from
    /// home (at any belt, when the lesson can only point home), the report
    /// sets out for the command center at light speed.
    pub(super) fn note_founding_belt_mining(&mut self, owner: PlayerId) {
        let Some(corp) = self.players.get(&owner) else {
            return;
        };
        if corp.founding.belt_report_at.is_some() {
            return;
        }
        let home = corp.home_system;
        let anywhere = corp.founding.belt_target.is_some_and(|(system, _)| Some(system) == home);
        let command = corp.command_center;
        let heard_at = self
            .fleets
            .values()
            .filter(|f| f.owner == owner)
            .filter_map(|f| match &f.industry {
                Some(crate::industry::FleetIndustry::Mine { system, mined, .. })
                    if *mined > 0.0 && (anywhere || Some(*system) != home) =>
                {
                    Some(*system)
                }
                _ => None,
            })
            .filter_map(|id| self.systems.iter().find(|s| s.id == id))
            .map(|s| self.time + crate::transit::delay(s.pos, command, self.config.c))
            .reduce(f64::min);
        if let Some(at) = heard_at
            && let Some(corp) = self.players.get_mut(&owner)
        {
            corp.founding.belt_report_at = Some(at);
        }
    }
}
