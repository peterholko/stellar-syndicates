//! §signals (`crate::signals`, docs/signals.md): the Command Center picks up
//! faint private contacts near home on Travian's slowing curve; a captained
//! fleet answers one, sweeps it and turns for home. Goods and crates ride home
//! aboard; everything else arrives with the report, at light speed. Signals
//! open with "Export your ore" (the step after the den), or at once for a
//! corporation outside the charter.
use super::*;
use crate::founding::FoundingStage;
use crate::signals::*;
use crate::sites::{ExpeditionTask, SITE_RANGE, SiteKind, SiteReport};

impl World {
    pub(super) fn tick_signals(&mut self, events: &mut Vec<Event>) {
        if self.tick.is_multiple_of(crate::config::TICK_HZ as u64) {
            let players: Vec<PlayerId> = self.players.keys().copied().collect();
            for player in players {
                self.detect_signals(player, events);
            }
        }
        self.sweep_signals();
        self.deliver_signal_reports(events);
    }

    /// Open signals for a corporation past the den, then detect the next on
    /// its curve when a slot is free.
    fn detect_signals(&mut self, player: PlayerId, events: &mut Vec<Event>) {
        let Some(corp) = self.players.get_mut(&player) else { return };
        let book = &mut corp.signals;
        if book.opened_at.is_none() {
            let founding = &corp.founding;
            if founding.enabled && founding.stage.step() < FoundingStage::PrepareExport.step() { return; }
            book.opened_at = Some(self.time);
            book.next_at = self.time;
        }
        if self.time + 1e-9 < book.next_at || book.open_count() >= SIGNAL_OPEN_MAX { return; }
        let opened = book.opened_at.unwrap_or(self.time);
        let opening = book.detected < SIGNAL_OPENING;
        self.spawn_signal(player, events);
        if let Some(book) = self.players.get_mut(&player).map(|c| &mut c.signals) {
            // The opening pair arrive together; the curve starts after them.
            book.next_at = if opening && book.detected < SIGNAL_OPENING { self.time }
                else { self.time + signal_interval_s(self.time - opened) };
        }
    }

    fn spawn_signal(&mut self, player: PlayerId, events: &mut Vec<Event>) {
        let Some(corp) = self.players.get(&player) else { return };
        let number = corp.signals.detected + 1;
        let home = corp.home;
        let mut rng = crate::rng::Rng::keyed_id(self.config.seed, "signals-v1",
            player.0.wrapping_mul(0x9E37_79B9) ^ u64::from(number));
        let (kind, hazardous, rewards) = if number as usize <= FIRST_TEN {
            first_ten(number)
        } else {
            draw_signal(&mut rng, self.time)
        };
        let (near, far) = if number as usize <= FIRST_TEN { SIGNAL_DOORSTEP_SU } else { SIGNAL_NEAR_SU };
        let others: Vec<Vec2> = corp.signals.signals.iter().filter(|s| s.open()).map(|s| s.pos).collect();
        let pos = (0..64).map(|_| home + Vec2::from_polar(rng.range(0.0, TAU), rng.range(near, far)))
            .find(|&pos| pos.distance(self.hub) > 5_000.0
                && self.systems.iter().all(|s| pos.distance(s.pos) > 3_000.0)
                && self.exploration.sites.values().all(|s| pos.distance(s.pos) > 2_500.0)
                && others.iter().all(|o| pos.distance(*o) > 2_500.0));
        let Some(pos) = pos else {
            // A crowded doorstep: look again in a minute.
            if let Some(corp) = self.players.get_mut(&player) { corp.signals.next_at = self.time + 60.0; }
            return;
        };
        self.signals_detected += 1;
        let id = EntityId(SIGNAL_ID_BASE + self.signals_detected);
        let corp = self.players.get_mut(&player).unwrap();
        corp.signals.detected = number;
        corp.signals.signals.push(Signal {
            id, number, kind, hazardous, pos, detected_at: self.time,
            xp: signal_xp(kind, hazardous), rewards, outcome: None, reported_at: None,
        });
        events.push(Event::new(self.time, EventPayload::SignalDetected { owner: player, signal: id, number, kind, hazardous })
            .at_origin(home));
    }

    /// Send a captained fleet to sweep a signal. Instant checks at command;
    /// the order itself travels to the fleet like any other.
    pub(super) fn answer_signal(&mut self, player: PlayerId, fleet_id: EntityId, signal_id: EntityId, events: &mut Vec<Event>) {
        let Some(pos) = self.players.get(&player)
            .and_then(|c| c.signals.signals.iter().find(|s| s.id == signal_id && s.outcome.is_none()))
            .map(|s| s.pos) else { return };
        if !self.fleets.get(&fleet_id)
            .is_some_and(|f| f.owner == player && !f.disposable && !f.contains(ShipKind::MiningBeltStation)) { return; }
        let Some(captain) = self.captain_assigned_to(player, fleet_id) else { return };
        if captain.hurt_until.is_some_and(|t| t > self.time) { return; }
        self.schedule_for_owner(player, fleet_id,
            FleetOrder::Expedition { site: signal_id, station: pos, task: ExpeditionTask::Signal, dwell_since: None },
            crate::event::OrderKind::Survey, events);
    }

    /// Fleets on station sweep for `SIGNAL_SWEEP_S`, then turn for home.
    fn sweep_signals(&mut self) {
        let jobs: Vec<_> = self.fleets.values().filter_map(|f| match f.order {
            FleetOrder::Expedition { site, station, task: ExpeditionTask::Signal, dwell_since } =>
                Some((f.id, f.owner, site, station, dwell_since)),
            _ => None,
        }).collect();
        for (id, player, signal_id, station, since) in jobs {
            let open = self.players.get(&player)
                .and_then(|c| c.signals.signals.iter().find(|s| s.id == signal_id))
                .is_some_and(|s| s.outcome.is_none());
            if !open {
                // Answered first by another fleet, or dismissed: nothing here.
                self.send_signal_fleet_home(id, player, false);
                continue;
            }
            let engaged = self.engagements.values().any(|e| e.attackers.contains(&id) || e.defenders.contains(&id));
            let Some(fleet) = self.fleets.get_mut(&id) else { continue };
            if engaged || fleet.defense.is_some() || fleet.pos.distance(station) > SITE_RANGE || fleet.vel.length_sq() > 1e-9 {
                if let FleetOrder::Expedition { dwell_since, .. } = &mut fleet.order { *dwell_since = None; }
                continue;
            }
            if let FleetOrder::Expedition { dwell_since, .. } = &mut fleet.order { dwell_since.get_or_insert(self.time); }
            if since.is_none_or(|t| self.time - t + 1e-9 < SIGNAL_SWEEP_S) { continue; }
            self.finish_signal_sweep(id, player, signal_id);
        }
    }

    fn finish_signal_sweep(&mut self, fleet_id: EntityId, player: PlayerId, signal_id: EntityId) {
        let Some(captain) = self.captain_assigned_to(player, fleet_id).cloned() else {
            // No captain aboard any more: the fleet holds and the signal waits.
            if let Some(f) = self.fleets.get_mut(&fleet_id) { f.order = FleetOrder::Idle; }
            return;
        };
        let Some(cc) = self.players.get(&player).map(|c| c.command_center) else { return };
        let signal = self.players[&player].signals.signals.iter().find(|s| s.id == signal_id).cloned().unwrap();
        let hurt = signal.hazardous
            && crate::rng::Rng::keyed_id(self.config.seed, "signal-hurt", signal_id.0).next_f64()
                < hurt_chance(captain.attributes.fieldcraft);
        let fleet = self.fleets.get_mut(&fleet_id).unwrap();
        let mut aboard = Vec::new();
        let mut left = Vec::new();
        for reward in &signal.rewards {
            match reward {
                SignalReward::Cargo { goods } => {
                    let mut loaded = Vec::new();
                    for &(commodity, units) in goods {
                        let room = fleet.cargo_capacity().saturating_sub(fleet.cargo_units());
                        let taken = units.min(room);
                        if taken > 0 { fleet.add_cargo(commodity, taken); loaded.push((commodity, taken)); }
                        if units > taken { left.push((commodity, units - taken)); }
                    }
                    if !loaded.is_empty() { aboard.push(SignalFind::CargoAboard { goods: loaded }); }
                }
                // A crate rides in any hull.
                SignalReward::Module { module } => {
                    *fleet.modules.entry(*module).or_insert(0) += 1;
                    aboard.push(SignalFind::ModuleAboard { module: *module });
                }
                _ => {}
            }
        }
        let report_at = self.time + crate::transit::delay(signal.pos, cc, self.config.c);
        let outcome = SignalOutcome {
            fleet: fleet_id, captain: captain.id, captain_name: captain.name.clone(),
            swept_at: self.time, report_at, xp: signal.xp, hurt, aboard, left, finds: Vec::new(),
        };
        let carrying = !outcome.aboard.is_empty();
        if let Some(s) = self.players.get_mut(&player)
            .and_then(|c| c.signals.signals.iter_mut().find(|s| s.id == signal_id)) { s.outcome = Some(outcome); }
        self.send_signal_fleet_home(fleet_id, player, carrying);
    }

    /// The fleet turns for home; what it carries lands there with it.
    fn send_signal_fleet_home(&mut self, fleet_id: EntityId, player: PlayerId, carrying: bool) {
        let Some((home, home_system)) = self.players.get(&player).map(|c| (c.home, c.home_system)) else { return };
        let Some(fleet) = self.fleets.get_mut(&fleet_id) else { return };
        fleet.order = FleetOrder::MoveTo { dest: home };
        if carrying && let Some(system) = home_system {
            fleet.mission = Some(TradeMission::DeliverToSystem { system });
        }
    }

    /// The report reaches command: pay what the sweep found.
    fn deliver_signal_reports(&mut self, events: &mut Vec<Event>) {
        let due: Vec<(PlayerId, EntityId)> = self.players.iter().flat_map(|(&p, c)| c.signals.signals.iter()
            .filter(|s| s.reported_at.is_none() && s.outcome.as_ref().is_some_and(|o| o.report_at <= self.time + 1e-9))
            .map(move |s| (p, s.id))).collect();
        for (player, signal_id) in due {
            let signal = self.players[&player].signals.signals.iter().find(|s| s.id == signal_id).cloned().unwrap();
            let outcome = signal.outcome.clone().unwrap();
            let mut finds = outcome.aboard.clone();
            for reward in &signal.rewards {
                if let Some(find) = self.pay_signal_reward(player, &signal, reward) { finds.push(find); }
            }
            if !outcome.left.is_empty() {
                let site = self.leave_signal_cache(player, &signal, &outcome.left);
                finds.push(SignalFind::Cache { site, goods: outcome.left.clone() });
            }
            let now = self.time;
            let corp = self.players.get_mut(&player).unwrap();
            if let Some(captain) = corp.captains.get_mut(&outcome.captain).filter(|c| !c.missing()) {
                captain.grant_xp(outcome.xp);
                if outcome.hurt { captain.hurt_until = Some(outcome.swept_at + SIGNAL_HURT_S); }
            }
            if signal.number as usize <= FIRST_TEN { corp.signals.first_ten_reported.insert(signal.number); }
            if let Some(s) = corp.signals.signals.iter_mut().find(|s| s.id == signal_id) {
                s.reported_at = Some(now);
                if let Some(o) = s.outcome.as_mut() { o.finds = finds.clone(); }
            }
            // Keep the latest reports for their cards.
            let reported = corp.signals.signals.iter().filter(|s| !s.open()).count();
            for _ in SIGNAL_REPORTS_KEPT..reported {
                if let Some(i) = corp.signals.signals.iter().enumerate().filter(|(_, s)| !s.open())
                    .min_by(|a, b| a.1.reported_at.unwrap_or(0.0).total_cmp(&b.1.reported_at.unwrap_or(0.0))).map(|(i, _)| i) {
                    corp.signals.signals.remove(i);
                }
            }
            events.push(Event::new(now, EventPayload::SignalReported {
                owner: player, signal: signal_id, number: signal.number, kind: signal.kind,
                captain: outcome.captain_name.clone(), xp: outcome.xp, hurt: outcome.hurt, finds,
            }));
        }
    }

    /// Everything a sweep holds beyond goods and crates, paid at command.
    fn pay_signal_reward(&mut self, player: PlayerId, signal: &Signal, reward: &SignalReward) -> Option<SignalFind> {
        let home_system = self.players.get(&player)?.home_system?;
        let home_pos = self.systems.iter().find(|s| s.id == home_system).map_or(Vec2::ZERO, |s| s.pos);
        let corp = self.players.get_mut(&player)?;
        Some(match reward {
            SignalReward::Cargo { .. } | SignalReward::Module { .. } => return None,
            SignalReward::Credits { credits } => {
                corp.credits += credits;
                SignalFind::Credits { credits: *credits }
            }
            SignalReward::Standing { credits, standing } => {
                corp.credits += credits;
                corp.tca_standing = (corp.tca_standing + standing).min(crate::tca::TCA_STANDING_MAX);
                SignalFind::Standing { credits: *credits, standing: *standing }
            }
            SignalReward::Dossier { fraction } => {
                let research = &corp.research;
                let fresh = |id: &str| !research.has(id) && !research.recovered_dossiers.contains(id)
                    && crate::research::programme(id).is_some_and(|p| !p.hidden);
                // The active programme first, then the queue, then the first
                // Tier I left in the catalogue.
                let programme = research.active.iter().chain(&research.queue).find(|id| fresh(id.as_str())).cloned()
                    .or_else(|| crate::research::visible_ids()
                        .filter(|id| crate::research::programme(id).is_some_and(|p| p.tier == 1))
                        .find(|id| fresh(id)).map(String::from));
                match programme {
                    Some(id) => {
                        corp.research.recover_dossier(&id, *fraction);
                        SignalFind::Dossier { programme: id }
                    }
                    None => {
                        corp.credits += 500.0;
                        SignalFind::Credits { credits: 500.0 }
                    }
                }
            }
            SignalReward::Blueprint { options } => {
                match options.iter().copied().find(|&m| !crate::research::has_module(&corp.research, m)) {
                    Some(module) => {
                        corp.research.blueprints.insert(module);
                        SignalFind::Blueprint { module }
                    }
                    // Every one already licensed: a finder's fee instead.
                    None => {
                        corp.credits += 1_500.0;
                        SignalFind::Credits { credits: 1_500.0 }
                    }
                }
            }
            SignalReward::Captain => {
                let mut captain = FIND_CAPTAINS.iter()
                    .map(|&p| crate::captain::Captain::founding(PlayerId(p), EntityId(0)))
                    .find(|c| !corp.captains.values().any(|o| o.name == c.name))
                    .unwrap_or_else(|| crate::captain::Captain::founding(PlayerId(FIND_CAPTAINS[0]), EntityId(0)));
                captain.id = corp.next_captain_id;
                corp.next_captain_id += 1;
                captain.assigned_fleet = None;
                captain.stationed_system = Some(home_system);
                captain.last_pos = home_pos;
                captain.grant_xp(crate::captain::xp_for_level(FIND_CAPTAIN_LEVEL).saturating_sub(captain.xp));
                let name = captain.name.clone();
                corp.captains.insert(captain.id, captain);
                SignalFind::Captain { name }
            }
            SignalReward::Specialist { specialist } => {
                // The survivors' own craft flies them home.
                let craft = self.spawn_trade_convoy(player, signal.pos, home_pos,
                    Cargo { commodity: crate::cargo::Commodity::Provisions, units: 0 },
                    TradeMission::DeliverToSystem { system: home_system });
                if let Some(f) = self.fleets.get_mut(&craft) {
                    f.take_cargo();
                    f.passengers.insert(*specialist, 1);
                }
                SignalFind::Specialist { specialist: *specialist }
            }
        })
    }

    /// Goods the fleet had no room for stay where the signal was: a private,
    /// identified cache for a Freighter.
    fn leave_signal_cache(&mut self, player: PlayerId, signal: &Signal, goods: &[(crate::cargo::Commodity, u32)]) -> EntityId {
        let ore = signal.kind == SignalKind::OreFragment;
        let site = self.create_exploration_site(signal.pos, if ore { SiteKind::Asteroids } else { SiteKind::Derelict }, 0);
        let now = self.time;
        let cache = self.exploration.sites.get_mut(&site).unwrap();
        cache.founding_owner = Some(player);
        cache.details.name = if ore { "Prospector's cache".into() } else { format!("{} cache", signal.kind.name()) };
        cache.details.cargo = goods.iter().copied().collect();
        cache.details.modules.clear();
        cache.details.research_fraction = 0.0;
        // Identified with the report: no dossier, no further study.
        cache.surveyed.insert(player);
        cache.study_paid.insert(player);
        let details = cache.details.clone();
        cache.known.insert(player, SiteReport { id: site, pos: signal.pos, reported_at: now, details: Some(details) });
        site
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ModuleKind;
    use crate::config::TICK_HZ;
    use crate::settlement::FoundingProfile;

    fn founder() -> (World, PlayerId) {
        let mut w = World::new(SimConfig::for_players(12648430, 2));
        w.enclaves.clear();
        let owner = PlayerId(971);
        w.step(&[Command::FoundColony { id: owner, name: "Signals test".into(), profile: FoundingProfile::CoastalPlains }]);
        (w, owner)
    }

    fn run(w: &mut World, seconds: f64) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..(seconds * TICK_HZ as f64) as usize { events.extend(w.step(&[])); }
        events
    }

    /// Past the den: "Export your ore" opens signals.
    fn opened(w: &mut World, owner: PlayerId) {
        w.players.get_mut(&owner).unwrap().founding.stage = FoundingStage::PrepareExport;
        run(w, 2.0);
    }

    /// An Interceptor at home under the founding captain.
    fn interceptor(w: &mut World, owner: PlayerId) -> EntityId {
        let home = w.players[&owner].home;
        let id = w.alloc_entity_id();
        w.fleets.insert(id, Fleet::single(id, owner, ShipKind::Raider, home, FleetOrder::Idle, None));
        let captain = w.players.get_mut(&owner).unwrap().captains.values_mut().next().unwrap();
        captain.assigned_fleet = Some(id);
        captain.stationed_system = None;
        id
    }

    /// Replace the book's signals with one scripted or crafted signal.
    fn only(w: &mut World, owner: PlayerId, mut signal: Signal) -> EntityId {
        let home = w.players[&owner].home;
        signal.pos = home + Vec2::new(10_000.0, 0.0);
        let id = signal.id;
        let book = &mut w.players.get_mut(&owner).unwrap().signals;
        book.signals = vec![signal];
        book.next_at = f64::INFINITY;
        id
    }

    fn scripted(number: u32) -> Signal {
        let (kind, hazardous, rewards) = first_ten(number);
        Signal { id: EntityId(SIGNAL_ID_BASE + 1_000 + number as u64), number, kind, hazardous, pos: Vec2::ZERO,
            detected_at: 0.0, xp: signal_xp(kind, hazardous), rewards, outcome: None, reported_at: None }
    }

    fn report(w: &mut World, owner: PlayerId, signal: EntityId) -> Vec<Event> {
        let mut events = Vec::new();
        for _ in 0..600 {
            events.extend(run(w, 1.0));
            if w.players[&owner].signals.signals.iter().any(|s| s.id == signal && s.reported_at.is_some()) { return events; }
        }
        panic!("no report within ten minutes");
    }

    fn reported(events: &[Event]) -> Vec<SignalFind> {
        events.iter().find_map(|e| match &e.payload { EventPayload::SignalReported { finds, .. } => Some(finds.clone()), _ => None })
            .expect("a report")
    }

    #[test]
    fn signals_open_after_the_den_two_at_once_then_on_travians_curve() {
        let (mut w, owner) = founder();
        run(&mut w, 3.0);
        assert!(w.players[&owner].signals.opened_at.is_none(), "nothing during the charter's early steps");
        let events = { w.players.get_mut(&owner).unwrap().founding.stage = FoundingStage::PrepareExport; run(&mut w, 2.0) };
        let book = &w.players[&owner].signals;
        let detected: Vec<_> = events.iter().filter_map(|e| match e.payload {
            EventPayload::SignalDetected { owner: o, number, kind, .. } if o == owner => Some((number, kind)), _ => None }).collect();
        assert_eq!(detected, [(1, SignalKind::SilentProbe), (2, SignalKind::DistressBeacon)], "the first two together");
        let home = w.players[&owner].home;
        for s in &book.signals {
            let d = s.pos.distance(home);
            assert!((SIGNAL_DOORSTEP_SU.0..=SIGNAL_DOORSTEP_SU.1).contains(&d), "on the doorstep: {d}");
        }
        assert!((book.next_at - w.time - 8.0 * 3_600.0).abs() < 2.0, "then one every eight hours");
        for n in 3..=SIGNAL_OPEN_MAX as u32 + 2 {
            w.players.get_mut(&owner).unwrap().signals.next_at = w.time;
            run(&mut w, 1.0);
            let book = &w.players[&owner].signals;
            assert_eq!(book.detected, n.min(SIGNAL_OPEN_MAX as u32), "five open at most");
        }
    }

    #[test]
    fn a_sweep_pays_when_its_report_reaches_command_and_the_fleet_comes_home() {
        let (mut w, owner) = founder();
        opened(&mut w, owner);
        let fleet = interceptor(&mut w, owner);
        let signal = only(&mut w, owner, scripted(1));
        let credits = w.players[&owner].credits;
        let xp = w.players[&owner].captains.values().next().unwrap().xp;
        w.step(&[Command::AnswerSignal { player_id: owner, fleet_id: fleet, signal_id: signal }]);
        // Swept, but the report is still on its way.
        let mut swept = false;
        for _ in 0..(300 * TICK_HZ) {
            w.step(&[]);
            let s = &w.players[&owner].signals.signals[0];
            if s.outcome.is_some() {
                assert!(s.reported_at.is_none() && w.players[&owner].credits < credits + 1_000.0, "paid with the report, not the sweep");
                assert!(matches!(w.fleets[&fleet].order, FleetOrder::MoveTo { dest } if dest == w.players[&owner].home), "turns for home");
                swept = true;
                break;
            }
        }
        assert!(swept, "the sweep finishes within five minutes");
        let events = report(&mut w, owner, signal);
        assert_eq!(reported(&events), [SignalFind::Credits { credits: 1_500.0 }]);
        assert!(w.players[&owner].credits >= credits + 1_500.0 - 1.0);
        assert_eq!(w.players[&owner].captains.values().next().unwrap().xp, xp + SIGNAL_XP);
        assert!(w.players[&owner].signals.first_ten_reported.contains(&1));
        assert_eq!(w.players[&owner].signals.first_ten_next(), Some("a captain"));
    }

    #[test]
    fn a_crate_rides_home_in_any_hull() {
        let (mut w, owner) = founder();
        opened(&mut w, owner);
        let fleet = interceptor(&mut w, owner);
        let signal = only(&mut w, owner, scripted(6));
        w.step(&[Command::AnswerSignal { player_id: owner, fleet_id: fleet, signal_id: signal }]);
        let events = report(&mut w, owner, signal);
        assert_eq!(reported(&events), [SignalFind::ModuleAboard { module: ModuleKind::ExtendedTanks }]);
        let home = w.players[&owner].home_system.unwrap();
        for _ in 0..300 {
            run(&mut w, 1.0);
            if w.fleets[&fleet].modules.is_empty() { break; }
        }
        let sys = w.systems.iter().find(|s| s.id == home).unwrap();
        assert_eq!(sys.modules.get(&ModuleKind::ExtendedTanks), Some(&1), "landed at home with the fleet");
        assert!(w.fleets[&fleet].mission.is_none());
    }

    #[test]
    fn goods_with_no_hold_room_wait_in_a_cache_for_a_freighter() {
        let (mut w, owner) = founder();
        opened(&mut w, owner);
        let fleet = interceptor(&mut w, owner);
        let signal = only(&mut w, owner, scripted(4));
        w.step(&[Command::AnswerSignal { player_id: owner, fleet_id: fleet, signal_id: signal }]);
        let events = report(&mut w, owner, signal);
        let finds = reported(&events);
        let [SignalFind::Cache { site, goods }] = finds.as_slice() else { panic!("{finds:?}") };
        assert_eq!(goods, &FIND_CACHE.to_vec());
        let cache = &w.exploration.sites[site];
        assert_eq!(cache.founding_owner, Some(owner), "only its finder sees and recovers it");
        assert_eq!(cache.details.name, "Prospector's cache");
        assert!(w.exploration.reports_for(owner).iter().any(|r| r.id == *site && r.details.is_some()), "identified with the report");
    }

    #[test]
    fn a_hazardous_signal_can_hurt_the_captain_who_then_sits_signals_out() {
        let (mut w, owner) = founder();
        opened(&mut w, owner);
        let fleet = interceptor(&mut w, owner);
        w.players.get_mut(&owner).unwrap().captains.values_mut().next().unwrap().attributes.fieldcraft = 1;
        let seed = w.config.seed;
        // A signal whose roll falls inside Fieldcraft 1's 30%.
        let id = (1..).map(|n| EntityId(SIGNAL_ID_BASE + 5_000 + n))
            .find(|id| crate::rng::Rng::keyed_id(seed, "signal-hurt", id.0).next_f64() < hurt_chance(1)).unwrap();
        let signal = only(&mut w, owner, Signal { id, ..scripted(10) });
        w.step(&[Command::AnswerSignal { player_id: owner, fleet_id: fleet, signal_id: signal }]);
        let events = report(&mut w, owner, signal);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SignalReported { hurt: true, xp, .. } if xp == 2 * SIGNAL_XP)),
            "double experience, and hurt");
        let captain = w.players[&owner].captains.values().next().unwrap().clone();
        let swept = w.players[&owner].signals.signals[0].outcome.as_ref().unwrap().swept_at;
        assert_eq!(captain.hurt_until, Some(swept + SIGNAL_HURT_S));
        // Until then the captain can't answer another.
        let next = Signal { id: EntityId(SIGNAL_ID_BASE + 9_999), ..scripted(5) };
        let home = w.players[&owner].home;
        w.players.get_mut(&owner).unwrap().signals.signals.push(Signal { pos: home + Vec2::new(0.0, 10_000.0), ..next.clone() });
        w.step(&[Command::AnswerSignal { player_id: owner, fleet_id: fleet, signal_id: next.id }]);
        run(&mut w, 2.0);
        assert!(!matches!(w.fleets[&fleet].order, FleetOrder::Expedition { site, .. } if site == next.id), "refused while hurt");
    }

    #[test]
    fn random_signals_are_fixed_when_they_appear() {
        let draw = || {
            let (mut w, owner) = founder();
            opened(&mut w, owner);
            w.players.get_mut(&owner).unwrap().signals.detected = FIRST_TEN as u32;
            w.players.get_mut(&owner).unwrap().signals.next_at = w.time;
            run(&mut w, 1.0);
            w.players[&owner].signals.signals.last().cloned().unwrap()
        };
        let (a, b) = (draw(), draw());
        assert_eq!(a.number, FIRST_TEN as u32 + 1);
        assert_eq!((a.kind, a.hazardous, &a.rewards, a.pos), (b.kind, b.hazardous, &b.rewards, b.pos));
    }
}
