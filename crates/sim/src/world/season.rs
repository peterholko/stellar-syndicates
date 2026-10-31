//! §season (docs/endgame-keystone.md): the Authority's census, its decrees
//! and the Galactic News Network.
use super::*;
use crate::cargo::Commodity;
use crate::season::{Bearing, Crackdown, CrackdownKind, Demand, Embargo, Ending, Grip, Keystone, Leak, LevyRise, NewsKind, NewsSource, Precursor, Relic, Requisition};

impl World {
    /// The census: every [`crate::season::CENSUS_EVERY_S`], measure the
    /// galaxy's maturity and decree each grip stage it has earned.
    pub(super) fn tick_season(&mut self, events: &mut Vec<Event>) {
        let every = (crate::season::CENSUS_EVERY_S * crate::config::TICK_HZ as f64) as u64;
        if !self.tick.is_multiple_of(every) {
            return;
        }
        let started = *self.season.started_at.get_or_insert(self.time);
        if self.season.census.is_empty() {
            // News starts with the season: older battles are history.
            self.season.battles_reported = self.tick;
        }
        let measured = crate::season::measure(self.corporation_growth());
        let floor = crate::season::floor(self.time - started);
        self.season.maturity = self.season.maturity.max(measured).max(floor).min(1.0);
        let earned = Grip::for_maturity(self.season.maturity);
        while self.season.grip < earned
            && let Some(next) = self.season.grip.next()
        {
            self.season.grip = next;
            self.decree(next, events);
        }
        self.season.record(self.time);
        // After the season's end only the census and the news go on.
        if self.season.ended.is_some() {
            return;
        }
        self.tick_squeeze(events);
        self.tick_resolve(events);
        self.tick_crackdowns(events);
        self.place_precursors();
        self.tick_relics(events);
    }

    /// Each active corporation's growth (`season::development`).
    fn corporation_growth(&self) -> Vec<f64> {
        let mut colonies: BTreeMap<PlayerId, usize> = BTreeMap::new();
        for sys in &self.systems {
            if let Some(owner) = sys.owner {
                *colonies.entry(owner).or_default() += 1;
            }
        }
        self.players.iter()
            .filter(|(id, corp)| self.time - corp.last_active <= crate::syndicate::ACTIVE_WINDOW_S || self.on_leave(**id))
            .map(|(id, corp)| {
                let held = colonies.get(id).copied().unwrap_or(0);
                let beyond_home = held.saturating_sub(usize::from(corp.home_system.is_some()));
                crate::season::development(beyond_home, corp.research.completed.len())
            })
            .collect()
    }

    /// Announce a grip stage: a GNN headline, and the decree every
    /// corporation hears from the Hub.
    fn decree(&mut self, grip: Grip, events: &mut Vec<Event>) {
        self.season.print(self.time, NewsSource::Gnn, NewsKind::Decree, grip.decree().to_string(), vec![], None, None);
        events.push(Event::new(self.time, EventPayload::Decree { grip, maturity: self.season.maturity, emergency: false }).at_origin(self.hub));
    }

    fn corp_label(&self, id: PlayerId) -> String {
        if id.is_pirate() {
            "pirates".into()
        } else if id.is_tca() {
            "the Authority".into()
        } else {
            self.players.get(&id).map_or_else(|| "an unknown corporation".into(), |c| c.name.clone())
        }
    }

    fn system_label(&self, id: EntityId) -> String {
        self.systems.iter().find(|s| s.id == id).map_or_else(|| "deep space".into(), |s| s.name.clone())
    }

    /// When the Hub hears of something that happened at `pos` at `time`.
    fn heard_at_hub(&self, time: f64, pos: Vec2) -> f64 {
        time + crate::transit::delay(pos, self.hub, self.config.c)
    }

    /// GNN: the galaxy's public stories from this tick's events and finished
    /// battles, each broadcast once the Hub has heard of it.
    pub(super) fn publish_news(&mut self, events: &[Event]) {
        if self.season.started_at.is_none() {
            return;
        }
        let mut joined: Vec<(PlayerId, String)> = Vec::new();
        let mut captures: Vec<(EntityId, PlayerId)> = Vec::new();
        let mut stories: Vec<(f64, NewsKind, String, Vec<PlayerId>, Option<EntityId>)> = Vec::new();
        for e in events {
            match &e.payload {
                EventPayload::PlayerJoined { id, name } => joined.push((*id, name.clone())),
                EventPayload::SystemCaptured { old_owner, new_owner, system, pos, .. } => {
                    captures.push((*system, *new_owner));
                    stories.push((self.heard_at_hub(e.time, *pos), NewsKind::Capture,
                        format!("{} takes {} from {}", self.corp_label(*new_owner), self.system_label(*system), self.corp_label(*old_owner)),
                        vec![*new_owner, *old_owner], Some(*system)));
                }
                EventPayload::DiplomacyUpdated { recipient, other, kind: crate::diplomacy::DiplomacyNoticeKind::Declaration, .. } => {
                    let at = self.players.get(other).map_or(e.time, |c| self.heard_at_hub(e.time, c.command_center));
                    stories.push((at, NewsKind::War,
                        format!("{} declares war on {}", self.corp_label(*other), self.corp_label(*recipient)),
                        vec![*other, *recipient], None));
                }
                EventPayload::MedalWon { owner, week, category, place: 1 } => {
                    stories.push((e.time, NewsKind::Medal,
                        format!("Week {week}: {} named {}", self.corp_label(*owner), category.title()),
                        vec![*owner], None));
                }
                EventPayload::Citation { culprit, offense, .. } => {
                    let due = self.season.wanted_at.get(culprit).is_none_or(|t| e.time - t >= crate::season::WANTED_EVERY_S);
                    if due {
                        self.season.wanted_at.insert(*culprit, e.time);
                        stories.push((e.time, NewsKind::Wanted,
                            format!("Wanted: {}, for {}", self.corp_label(*culprit), offense.title()),
                            vec![*culprit], None));
                    }
                }
                EventPayload::EnforcementDispatched { target, system, .. } => {
                    stories.push((e.time, NewsKind::Crackdown,
                        format!("Enforcement squadron sails for {}: {} proscribed", self.system_label(*system), self.corp_label(*target)),
                        vec![*target], Some(*system)));
                }
                _ => {}
            }
        }
        if !joined.is_empty() {
            let names: Vec<&str> = joined.iter().map(|(_, n)| n.as_str()).collect();
            let headline = match names.as_slice() {
                [one] => format!("New charter: {one}"),
                many => format!("{} new charters: {}", many.len(), many.join(", ")),
            };
            stories.push((self.time, NewsKind::Corporation, headline, joined.iter().map(|(id, _)| *id).collect(), None));
        }
        // Battles: a big one that ended since the last look.
        let since = self.season.battles_reported;
        let mut bounties: Vec<([PlayerId; 2], [u32; 2], Option<EntityId>)> = Vec::new();
        let mut latest = since;
        for record in self.battle_records.values() {
            let Some(ended) = record.ended_tick.filter(|t| *t > since) else { continue };
            latest = latest.max(ended);
            let Some(outcome) = &record.outcome else { continue };
            if record.sides.iter().any(|side| self.is_rebel(side.corp)) {
                bounties.push(([record.sides[0].corp, record.sides[1].corp],
                    [outcome.total_losses[0].values().sum(), outcome.total_losses[1].values().sum()], record.system));
            }
            let fielded: u32 = record.sides.iter().map(|s| s.initial.values().sum::<u32>()).sum();
            let lost: u32 = outcome.total_losses.iter().map(|l| l.values().sum::<u32>()).sum();
            if record.raid || (fielded < crate::season::BATTLE_NEWS_HULLS && lost < crate::season::BATTLE_NEWS_LOSSES) {
                continue;
            }
            let [a, b] = [record.sides[0].corp, record.sides[1].corp];
            let place = record.system.map_or_else(|| "in deep space".to_string(), |s| format!("at {}", self.system_label(s)));
            let time = ended as f64 * DT;
            stories.push((self.heard_at_hub(time, record.pos), NewsKind::Battle,
                format!("Battle {place}: {} and {} · {lost} hulls lost", self.corp_label(a), self.corp_label(b)),
                [a, b].into_iter().filter(|p| !p.is_sentinel()).collect(), record.system));
        }
        self.season.battles_reported = latest;
        for (corps, lost, system) in bounties {
            self.pay_bounties(corps, lost, system);
        }
        for (system, by) in captures {
            self.note_keystone_capture(system, by);
        }
        for (at, kind, headline, corps, system) in stories {
            self.season.print(at, NewsSource::Gnn, kind, headline, corps, system, None);
        }
    }
}


// ── The squeeze: levies, embargoes, requisitions (grip II on) ──
impl World {
    /// §season: the levy on `who`'s Hub trades: the grip's base and its own
    /// rises. Nothing under the plain charter, and nothing for a sentinel.
    pub fn levy_frac(&self, who: PlayerId) -> f64 {
        use crate::season::{EMERGENCY_LEVY, LOYAL_BELOW, LOYAL_LEVY_DISCOUNT};
        let base = crate::season::LEVY_BY_GRIP[self.season.grip as usize];
        if base <= 0.0 || !self.players.contains_key(&who) {
            return 0.0;
        }
        let sq = self.season.squeeze.get(&who);
        let emergency = if self.season.emergency { EMERGENCY_LEVY } else { 0.0 };
        // §season phase 3: loyalists pay less; sabotage doubles the Hub's fees.
        let loyal = if sq.map_or(0.0, |s| s.resolve) < LOYAL_BELOW { LOYAL_LEVY_DISCOUNT } else { 0.0 };
        let levy = (base + emergency - loyal).max(0.0) + sq.map_or(0.0, |s| s.levy_extra);
        levy * self.hub_fee_mult()
    }

    /// §season: what the Hub's fees are multiplied by now (sabotage doubles them).
    pub fn hub_fee_mult(&self) -> f64 {
        if self.season.in_force(CrackdownKind::Sabotage, self.time) { 2.0 } else { 1.0 }
    }

    /// §season phase 3: a corporation's Resolve, 0–1.
    pub fn resolve(&self, who: PlayerId) -> f64 {
        self.season.squeeze.get(&who).map_or(0.0, |s| s.resolve)
    }

    /// Below [`crate::season::LOYAL_BELOW`] Resolve, a loyalist.
    pub fn loyal(&self, who: PlayerId) -> bool {
        self.resolve(who) < crate::season::LOYAL_BELOW
    }

    /// Add Resolve (or take it, negative), clamped to 0–1; crossing 50% or
    /// 100% is the corporation's news and the Free Signal's.
    pub(super) fn add_resolve(&mut self, who: PlayerId, amount: f64, events: &mut Vec<Event>) {
        if !self.players.contains_key(&who) || amount == 0.0 {
            return;
        }
        let sq = self.season.squeeze.entry(who).or_default();
        let before = sq.resolve;
        sq.resolve = (before + amount).clamp(0.0, 1.0);
        let after = sq.resolve;
        for mark in [crate::season::RESOLVE_ATTUNE, 1.0] {
            if before + 1e-9 < mark && after + 1e-9 >= mark {
                events.push(Event::new(self.time, EventPayload::ResolveMilestone { owner: who, resolve: mark }));
                let headline = if mark >= 1.0 {
                    format!("{}'s colonists stand as one against the Authority", self.corp_label(who))
                } else {
                    format!("{} rallies its colonists", self.corp_label(who))
                };
                self.season.print(self.time, NewsSource::FreeSignal, NewsKind::Resolve, headline, vec![who], None, None);
            }
        }
    }

    /// §season: `who` may not trade or ship `commodity` through the Authority.
    pub fn embargoed(&self, who: PlayerId, commodity: Commodity) -> bool {
        self.season.squeeze.get(&who).is_some_and(|s| s.embargoes.contains_key(&commodity))
    }

    /// The Exchange trade or Authority freight `cmd` would move of a good its
    /// issuer has embargoed, or by a rebel at war with the Authority:
    /// (owner, good, units, system, why).
    pub(super) fn embargo_blocks(&self, cmd: &Command) -> Option<(PlayerId, Commodity, u32, Option<EntityId>, TradeRejectReason)> {
        let (owner, commodity, units, system) = match cmd {
            Command::MarketBuy { player_id, commodity, units, .. }
            | Command::MarketSell { player_id, commodity, units, .. }
            | Command::PlaceLimitOrder { player_id, commodity, units, .. } => (*player_id, *commodity, *units, None),
            Command::BookFreightOut { player_id, system, commodity, units }
            | Command::BookFreightIn { player_id, system, commodity, units, .. }
            | Command::StockSystem { player_id, system_id: system, commodity, units } =>
                (*player_id, *commodity, *units, Some(*system)),
            _ => return None,
        };
        if self.is_rebel(owner) {
            return Some((owner, commodity, units, system, TradeRejectReason::CharterRevoked));
        }
        self.embargoed(owner, commodity).then_some((owner, commodity, units, system, TradeRejectReason::Embargoed))
    }

    /// An enforcement squadron is due: proscribed, or refusals answered for.
    pub(super) fn enforcement_due(&self, who: PlayerId) -> bool {
        self.is_rebel(who) || self.charter_of(who) == crate::tca::CharterStatus::Proscribed
            || self.season.squeeze.get(&who).is_some_and(|s| s.enforced_until > self.time)
    }

    /// Every Hub trade this tick pays its levy; a big one draws a rise.
    pub(super) fn assess_levies(&mut self, events: &mut Vec<Event>) {
        if self.season.grip == Grip::Charter {
            return;
        }
        let trades: Vec<(PlayerId, Commodity, f64)> = events.iter().filter_map(|e| match e.payload {
            EventPayload::Trade(TradeEvent::Bought { player, commodity, units, unit_price, .. }
                | TradeEvent::Sold { player, commodity, units, unit_price, .. }
                | TradeEvent::LimitFilled { player, commodity, units, unit_price, .. }) =>
                Some((player, commodity, units as f64 * unit_price)),
            _ => None,
        }).collect();
        for (player, commodity, gross) in trades {
            let levy = gross * self.levy_frac(player);
            if levy <= 0.0 {
                continue;
            }
            self.season.squeeze.entry(player).or_default().levy_paid += levy;
            self.add_resolve(player, levy / 1_000.0 * crate::season::RESOLVE_PER_LEVY_KCR, events);
            if gross >= crate::season::LEVY_RISE_TRADE {
                self.demand_rise(player, commodity, gross, events);
            }
        }
    }

    fn demand_rise(&mut self, owner: PlayerId, commodity: Commodity, trade: f64, events: &mut Vec<Event>) {
        use crate::season::{ANSWER_WINDOW_S, LEVY_EXTRA_MAX, LEVY_RISE_EVERY_S, LEVY_RISE_POINTS};
        let now = self.time;
        let Some(cc) = self.players.get(&owner).map(|c| c.command_center) else { return };
        let heard = now + crate::transit::delay(self.hub, cc, self.config.c);
        if self.is_rebel(owner) {
            return;
        }
        let sq = self.season.squeeze.entry(owner).or_default();
        if sq.rise.is_some() || sq.last_rise_at.is_some_and(|t| now - t < LEVY_RISE_EVERY_S) || sq.levy_extra + 1e-9 >= LEVY_EXTRA_MAX {
            return;
        }
        let points = LEVY_RISE_POINTS.min(LEVY_EXTRA_MAX - sq.levy_extra);
        let due = heard + ANSWER_WINDOW_S;
        sq.rise = Some(LevyRise { points, commodity, trade, at: now, due });
        sq.last_rise_at = Some(now);
        events.push(Event::new(now, EventPayload::LevyRiseDemanded { owner, points, commodity, trade, due }).at_origin(self.hub));
        // A computer-run corporation pays (loyal by default).
        if owner.is_bot() {
            self.answer_levy(owner, true, false, events);
        }
    }

    /// A rise answered at the Hub: accepted, the levy climbs; refused, the
    /// good is embargoed until its arrears are paid.
    pub(super) fn answer_levy(&mut self, owner: PlayerId, accept: bool, by_default: bool, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(sq) = self.season.squeeze.get_mut(&owner) else { return };
        let Some(rise) = sq.rise.take() else { return };
        let arrears = if accept {
            sq.levy_extra = (sq.levy_extra + rise.points).min(crate::season::LEVY_EXTRA_MAX);
            0.0
        } else {
            let arrears = (rise.trade * crate::season::ARREARS_FRAC).round();
            sq.embargoes.insert(rise.commodity, Embargo { arrears, since: now });
            arrears
        };
        let levy = self.levy_frac(owner);
        self.add_resolve(owner, if accept { crate::season::RESOLVE_RISE_ACCEPTED } else { crate::season::RESOLVE_RISE_REFUSED }, events);
        events.push(Event::new(now, EventPayload::LevyAnswered {
            owner, accepted: accept, by_default, levy, commodity: rise.commodity, arrears,
        }).at_origin(self.hub));
        if !accept {
            let headline = format!("{} defies the levy: {} embargoed at the Hub", self.corp_label(owner), rise.commodity.name());
            self.season.print(now, NewsSource::Gnn, NewsKind::Crackdown, headline, vec![owner], None, None);
        }
    }

    /// Arrears paid at the Hub (from the money that left with the order).
    pub(super) fn pay_arrears(&mut self, owner: PlayerId, commodity: Commodity, events: &mut Vec<Event>) {
        let Some(arrears) = self.season.squeeze.get(&owner).and_then(|s| s.embargoes.get(&commodity)).map(|e| e.arrears) else {
            return;
        };
        let Some(corp) = self.players.get_mut(&owner) else { return };
        if corp.credits + 1e-9 < arrears {
            self.reject_trade(events, owner, commodity, 0, None, TradeRejectReason::CantAfford { cost: arrears });
            return;
        }
        corp.credits -= arrears;
        if let Some(sq) = self.season.squeeze.get_mut(&owner) {
            sq.embargoes.remove(&commodity);
        }
        events.push(Event::new(self.time, EventPayload::EmbargoLifted { owner, commodity, arrears }).at_origin(self.hub));
    }

    /// A requisition paid (goods from the Hub warehouse, or the credits that
    /// came with the order) or refused.
    pub(super) fn answer_requisition(&mut self, owner: PlayerId, id: u64, pay: bool, by_default: bool, events: &mut Vec<Event>) {
        use crate::season::{ENFORCEMENT_ORDER_S, LEVY_EXTRA_MAX, REFUSAL_STANDING, REFUSALS_ENFORCED};
        let now = self.time;
        let Some(req) = self.season.squeeze.get(&owner).and_then(|s| s.requisitions.iter().find(|r| r.id == id)).cloned() else {
            return;
        };
        let Some(corp) = self.players.get_mut(&owner) else { return };
        if pay {
            match req.demand {
                Demand::Goods { commodity, units } => {
                    let have = corp.warehouse.get(&commodity).copied().unwrap_or(0);
                    if have < units {
                        self.reject_trade(events, owner, commodity, units, None, TradeRejectReason::InsufficientWarehouseStock { have });
                        return;
                    }
                    take_from(&mut corp.warehouse, commodity, units);
                }
                Demand::Credits { amount } => {
                    if corp.credits + 1e-9 < amount {
                        self.reject_trade(events, owner, Commodity::Provisions, 0, None, TradeRejectReason::CantAfford { cost: amount });
                        return;
                    }
                    corp.credits -= amount;
                }
            }
        } else {
            corp.tca_standing -= REFUSAL_STANDING;
        }
        let Some(sq) = self.season.squeeze.get_mut(&owner) else { return };
        sq.requisitions.retain(|r| r.id != id);
        let mut enforced = false;
        if !pay {
            sq.refusals += 1;
            sq.levy_extra = (sq.levy_extra + 0.01).min(LEVY_EXTRA_MAX);
            if sq.refusals >= REFUSALS_ENFORCED {
                sq.enforced_until = now + ENFORCEMENT_ORDER_S;
                enforced = true;
            }
        }
        events.push(Event::new(now, EventPayload::RequisitionSettled {
            owner, id, demand: req.demand, paid: pay, by_default, enforced,
        }).at_origin(self.hub));
        self.add_resolve(owner, if pay { crate::season::RESOLVE_REQUISITION_PAID } else { crate::season::RESOLVE_REQUISITION_REFUSED }, events);
        if !pay {
            let headline = format!("{} refuses an Authority requisition", self.corp_label(owner));
            self.season.print(now, NewsSource::Gnn, NewsKind::Wanted, headline, vec![owner], None, None);
        }
    }

    /// On the census: settle what lapsed and issue the requisitions due.
    pub(super) fn tick_squeeze(&mut self, events: &mut Vec<Event>) {
        if self.season.grip == Grip::Charter {
            return;
        }
        let now = self.time;
        let owners: Vec<PlayerId> = self.season.squeeze.keys().copied().collect();
        for owner in owners {
            if self.season.squeeze[&owner].rise.as_ref().is_some_and(|r| r.due <= now) {
                self.answer_levy(owner, true, true, events);
            }
            let lapsed: Vec<u64> = self.season.squeeze[&owner].requisitions.iter().filter(|r| r.due <= now).map(|r| r.id).collect();
            for id in lapsed {
                self.answer_requisition(owner, id, false, true, events);
            }
        }
        let due: Vec<PlayerId> = self.players.iter()
            .filter(|(id, corp)| corp.founding.stage == crate::founding::FoundingStage::Complete && !self.is_rebel(**id)
                && !self.on_leave(**id) && now - corp.last_active <= crate::syndicate::ACTIVE_WINDOW_S)
            .map(|(id, _)| *id)
            .collect();
        for owner in due {
            let sq = self.season.squeeze.entry(owner).or_default();
            // The first falls within a day of the levies, staggered by corporation.
            let next = *sq.next_requisition_at.get_or_insert_with(||
                now + crate::season::REQUISITION_EVERY_S * 0.5 * (owner.0 % 997) as f64 / 997.0);
            if now < next || !sq.requisitions.is_empty() {
                continue;
            }
            sq.next_requisition_at = Some(now + crate::season::REQUISITION_EVERY_S);
            self.issue_requisition(owner, events);
        }
    }

    fn issue_requisition(&mut self, owner: PlayerId, events: &mut Vec<Event>) {
        use crate::season::{ANSWER_WINDOW_S, REQUISITION_MIN, REQUISITION_VALUE_FRAC};
        let now = self.time;
        let Some(corp) = self.players.get(&owner) else { return };
        let scale = [1.0, 1.0, 1.5, 2.0][self.season.grip as usize];
        let value = (corp.valuation * REQUISITION_VALUE_FRAC).max(REQUISITION_MIN) * scale;
        self.season.next_requisition += 1;
        let id = self.season.next_requisition;
        // Every other one asks for goods: the corporation's largest holding.
        let goods = (id % 2 == 0 && !owner.is_bot()).then(|| {
            let mut held: BTreeMap<Commodity, f64> = BTreeMap::new();
            for (c, n) in &corp.warehouse {
                *held.entry(*c).or_default() += *n as f64;
            }
            for sys in self.systems.iter().filter(|s| s.owner == Some(owner)) {
                for (c, n) in &sys.stockpile {
                    *held.entry(*c).or_default() += *n;
                }
            }
            held.into_iter()
                .map(|(c, n)| (c, n * self.market.price(c)))
                .filter(|(c, v)| *v > 0.0 && self.market.price(*c) > 0.0)
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(c, _)| (c, ((value / self.market.price(c) / 10.0).ceil() * 10.0) as u32))
        }).flatten();
        let demand = goods.map_or(Demand::Credits { amount: value.round() }, |(commodity, units)| Demand::Goods { commodity, units });
        let heard = now + crate::transit::delay(self.hub, corp.command_center, self.config.c);
        let due = heard + ANSWER_WINDOW_S;
        self.season.squeeze.entry(owner).or_default().requisitions.push(Requisition { id, demand, at: now, due });
        events.push(Event::new(now, EventPayload::RequisitionIssued { owner, id, demand, due }).at_origin(self.hub));
        // A computer-run corporation pays if it can.
        if owner.is_bot() && self.players.get(&owner).is_some_and(|c| c.credits >= value) {
            self.answer_requisition(owner, id, true, false, events);
        }
    }
}


// ── Resolve, the emergency decree, the war fleet and the crackdowns ──
impl World {
    /// On the census: Resolve fades, Assembly Halls rally it, the galaxy's is
    /// taken, and the emergency decree and the war fleet follow it.
    fn tick_resolve(&mut self, events: &mut Vec<Event>) {
        use crate::season::{RESOLVE_DECAY_PER_DAY, RESOLVE_HALLS_MAX_HOUR, RESOLVE_PER_HALL_HOUR};
        let minutes = crate::season::CENSUS_EVERY_S / 60.0;
        let mut halls: BTreeMap<PlayerId, u32> = BTreeMap::new();
        for sys in &self.systems {
            if let Some(owner) = sys.owner {
                *halls.entry(owner).or_default() += sys.tier_sum(crate::StructureKind::AssemblyHall);
            }
        }
        let owners: Vec<PlayerId> = self.players.keys().copied().collect();
        for owner in owners {
            let rally = (halls.get(&owner).copied().unwrap_or(0) as f64 * RESOLVE_PER_HALL_HOUR).min(RESOLVE_HALLS_MAX_HOUR);
            let delta = rally * minutes / 60.0 - RESOLVE_DECAY_PER_DAY * minutes / 1_440.0;
            if self.resolve(owner) > 0.0 || delta > 0.0 {
                self.add_resolve(owner, delta, events);
            }
        }
        let active: Vec<f64> = self.players.iter()
            .filter(|(id, corp)| self.time - corp.last_active <= crate::syndicate::ACTIVE_WINDOW_S || self.on_leave(**id))
            .map(|(id, _)| self.resolve(*id))
            .collect();
        let before = self.season.galaxy_resolve;
        self.season.galaxy_resolve = if active.is_empty() { 0.0 } else { active.iter().sum::<f64>() / active.len() as f64 };
        for mark in [0.25, 0.5, 0.75] {
            if before < mark && self.season.galaxy_resolve >= mark {
                let headline = format!("The free colonies grow: galaxy Resolve {:.0}%", mark * 100.0);
                self.season.print(self.time, NewsSource::FreeSignal, NewsKind::Resolve, headline, vec![], None, None);
            }
        }
        // The emergency decree: the grip jumps a stage early and levies rise.
        if !self.season.emergency && self.season.galaxy_resolve >= crate::season::EMERGENCY_RESOLVE && self.season.grip >= Grip::Levies {
            self.season.emergency = true;
            if let Some(next) = self.season.grip.next() {
                self.season.grip = next;
            }
            let grip = self.season.grip;
            self.season.print(self.time, NewsSource::Gnn, NewsKind::Decree,
                format!("Emergency decree: the Authority imposes {} and raises every levy", grip.title()), vec![], None, None);
            events.push(Event::new(self.time, EventPayload::Decree { grip, maturity: self.season.maturity, emergency: true }).at_origin(self.hub));
            self.season.record(self.time);
        }
        self.season.war_fleet = (crate::season::WAR_FLEET_BASE + crate::season::WAR_FLEET_PER_MATURITY * self.season.maturity
            + crate::season::WAR_FLEET_PER_RESOLVE * self.season.galaxy_resolve).round() as u32;
    }

    fn open_crackdown(&mut self, kind: CrackdownKind, pos: Vec2, system: Option<EntityId>, target: Option<PlayerId>,
        staged: bool, evidence: bool, events: &mut Vec<Event>) -> u64 {
        let now = self.time;
        self.season.next_crackdown += 1;
        let id = self.season.next_crackdown;
        let until = now + crate::season::CRACKDOWN_S;
        let evidence_until = if evidence { now + crate::season::EVIDENCE_S } else { 0.0 };
        self.season.crackdowns.push(Crackdown {
            id, kind, at: now, until, pos, system, target, staged, evidence_until, exposed: false, investigated: Vec::new(),
        });
        // Keep the season's last fifty.
        if self.season.crackdowns.len() > 50 {
            self.season.crackdowns.remove(0);
        }
        events.push(Event::new(now, EventPayload::CrackdownBegan { id, kind, system, target, until, evidence_until }).at_origin(self.hub));
        if let Some(t) = target {
            self.add_resolve(t, crate::season::RESOLVE_CRACKDOWN, events);
        }
        id
    }

    /// Whether a staged event is staged, decided once per crackdown id.
    fn staged_roll(&self, id: u64) -> bool {
        crate::rng::Rng::keyed_id(self.config.seed, "season-staged-v1", id).next_f64() < crate::season::STAGED_SHARE
    }

    /// The rebel the Authority blames for what happened at `pos`: the nearest
    /// corporation at Resolve 50% or more, else `fallback`.
    fn scapegoat(&self, pos: Vec2, fallback: Option<PlayerId>) -> Option<PlayerId> {
        self.players.iter()
            .filter(|(id, _)| self.resolve(**id) >= crate::season::RESOLVE_ATTUNE)
            .min_by(|a, b| a.1.command_center.distance(pos).total_cmp(&b.1.command_center.distance(pos)).then(a.0.cmp(b.0)))
            .map(|(id, _)| *id)
            .or(fallback)
    }

    /// Name a culprit for a massacre: an enforcement squadron and the wanted list.
    fn name_culprit(&mut self, culprit: PlayerId, pos: Vec2, staged: bool, events: &mut Vec<Event>) {
        let id = self.open_crackdown(CrackdownKind::Massacre, pos, None, Some(culprit), staged, true, events);
        let until = self.season.crackdowns.iter().find(|c| c.id == id).map_or(self.time, |c| c.until);
        let sq = self.season.squeeze.entry(culprit).or_default();
        sq.enforced_until = sq.enforced_until.max(until);
        let headline = format!("Convoy massacre: the Authority names {} and posts a bounty", self.corp_label(culprit));
        self.season.print(self.time, NewsSource::Gnn, NewsKind::Crackdown, headline, vec![culprit], None, None);
    }

    /// On the census: sabotage, staged massacres and quarantines, as the
    /// galaxy brings them on.
    fn tick_crackdowns(&mut self, events: &mut Vec<Event>) {
        use crate::season::CRACKDOWN_EVERY_S;
        let now = self.time;
        let due = |last: Option<f64>| last.is_none_or(|t| now - t >= CRACKDOWN_EVERY_S);
        if self.season.grip >= Grip::Inspections && self.season.galaxy_resolve >= 0.25 && due(self.season.last_sabotage_at) {
            self.season.last_sabotage_at = Some(now);
            let staged = self.staged_roll(self.season.next_crackdown + 1);
            self.open_crackdown(CrackdownKind::Sabotage, self.hub, None, None, staged, true, events);
            self.season.print(now, NewsSource::Gnn, NewsKind::Crackdown,
                "Sabotage at the Hub: insurgents blamed; inspections and doubled fees for 12h".into(), vec![], None, None);
        }
        // A staged massacre: an Authority convoy lost near a rebel, and blamed
        // on it. One roll a window, half of them staged.
        if self.season.grip >= Grip::Inspections && due(self.season.last_staged_at)
            && let Some(rebel) = self.scapegoat(self.hub, None)
        {
            self.season.last_staged_at = Some(now);
            let window = (now / CRACKDOWN_EVERY_S) as u64;
            if crate::rng::Rng::keyed_id(self.config.seed, "season-staged-massacre-v1", window).next_f64() < crate::season::STAGED_SHARE {
                let home = self.players[&rebel].command_center;
                let pos = home + (self.hub - home) * 0.3;
                self.name_culprit(rebel, pos, true, events);
            }
        }
        // Quarantine: the busiest freight destination.
        if self.season.grip >= Grip::Levies && due(self.season.last_quarantine_at) {
            let mut traffic: BTreeMap<EntityId, u32> = BTreeMap::new();
            for shipment in self.freight_queue.values() {
                *traffic.entry(shipment.system).or_default() += 1;
            }
            if let Some((system, _)) = traffic.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                && let Some(sys) = self.systems.iter().find(|s| s.id == system)
            {
                let (pos, owner, name) = (sys.pos, sys.owner, sys.name.clone());
                self.season.last_quarantine_at = Some(now);
                self.open_crackdown(CrackdownKind::Quarantine, pos, Some(system), owner, false, false, events);
                self.season.print(now, NewsSource::Gnn, NewsKind::Crackdown,
                    format!("Quarantine at {name}: Authority freight suspended for 12h"), owner.into_iter().collect(), Some(system), None);
            }
        }
    }

    /// The Authority answers this tick's incidents: a destroyed convoy is a
    /// massacre, blamed on the nearest rebel.
    pub(super) fn answer_incidents(&mut self, events: &mut Vec<Event>) {
        if self.season.grip < Grip::Levies {
            return;
        }
        let massacres: Vec<(PlayerId, Vec2)> = events.iter().filter_map(|e| match e.payload {
            EventPayload::Citation { culprit, offense: crate::tca::CitationOffense::FreightDestroyed, pos, .. } => Some((culprit, pos)),
            _ => None,
        }).collect();
        for (culprit, pos) in massacres {
            if let Some(named) = self.scapegoat(pos, Some(culprit)) {
                self.name_culprit(named, pos, false, events);
            }
        }
    }

    /// Scouts at a scene read its evidence (1 Hz). A staged crackdown,
    /// exposed, lifts at once.
    pub(super) fn investigate_evidence(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let open: Vec<(u64, Vec2)> = self.season.crackdowns.iter()
            .filter(|c| c.evidence_until > now && !c.exposed)
            .map(|c| (c.id, c.pos))
            .collect();
        if open.is_empty() {
            return;
        }
        let mut reads: Vec<(u64, PlayerId)> = Vec::new();
        for (id, pos) in open {
            let readers: std::collections::BTreeSet<PlayerId> = self.fleets.values()
                .filter(|f| f.contains(ShipKind::Scout) && self.players.contains_key(&f.owner)
                    && f.pos.distance(pos) <= crate::season::EVIDENCE_RADIUS)
                .map(|f| f.owner)
                .collect();
            for reader in readers {
                reads.push((id, reader));
            }
        }
        for (id, reader) in reads {
            let Some(c) = self.season.crackdowns.iter_mut().find(|c| c.id == id) else { continue };
            if c.exposed || c.investigated.contains(&reader) {
                continue;
            }
            c.investigated.push(reader);
            let (kind, staged, pos, target) = (c.kind, c.staged, c.pos, c.target);
            if staged {
                c.exposed = true;
                c.until = now;
                c.evidence_until = now;
                events.push(Event::new(now, EventPayload::CrackdownExposed { id, kind, by: reader }).at_origin(self.hub));
                if let Some(t) = target && let Some(sq) = self.season.squeeze.get_mut(&t) {
                    sq.enforced_until = now;
                }
                let owners: Vec<PlayerId> = self.players.keys().copied().collect();
                for owner in owners {
                    self.add_resolve(owner, crate::season::RESOLVE_EXPOSED, events);
                    if self.loyal(owner) && let Some(corp) = self.players.get_mut(&owner) {
                        corp.tca_standing -= crate::season::EXPOSED_LOYAL_STANDING;
                    }
                }
                self.season.print(now, NewsSource::Gnn, NewsKind::Retraction,
                    format!("GNN retracts its report on the {}", kind.title().to_lowercase()), vec![], None, None);
                self.season.print(now, NewsSource::FreeSignal, NewsKind::Retraction,
                    format!("False flag exposed by {}: the Authority staged the {}", self.corp_label(reader), kind.title().to_lowercase()),
                    vec![reader], None, None);
            } else {
                events.push(Event::new(now, EventPayload::EvidenceRead { owner: reader, id, kind, staged: false }).at_origin(pos));
            }
        }
    }
}


// ── The relic journey, the sealed worlds, the spy and the war ──
impl World {
    /// §season: `who` is at war with the Authority.
    pub fn is_rebel(&self, who: PlayerId) -> bool {
        self.season.rebels.contains_key(&who)
    }

    /// Who shares `who`'s finds: its syndicate, or itself.
    pub fn circle(&self, who: PlayerId) -> Vec<PlayerId> {
        self.players.get(&who).and_then(|c| c.syndicate).and_then(|id| self.syndicates.get(&id))
            .map_or_else(|| vec![who], |s| s.members.iter().copied().collect())
    }

    /// Place the sealed worlds, once: between the Hub and the homes, spread
    /// apart, never a home, its neighbourhood or a relic.
    pub(super) fn place_precursors(&mut self) {
        if self.season.precursors_placed {
            return;
        }
        self.season.precursors_placed = true;
        let n = ((self.home_slots.len() as f64 / 20.0 * crate::season::PRECURSORS_PER_20).round() as usize).max(1);
        let ring = if self.home_slots.is_empty() { 125_000.0 } else {
            self.home_slots.iter().map(|h| h.pos.distance(self.hub)).sum::<f64>() / self.home_slots.len() as f64
        };
        let homes: std::collections::BTreeSet<EntityId> = self.home_slots.iter().filter_map(|h| h.system).collect();
        let mut candidates: Vec<(u64, EntityId, Vec2, u32)> = self.systems.iter()
            .filter(|sys| sys.owner.is_none() && !homes.contains(&sys.id) && !self.home_neighbours.contains(&sys.id)
                && !self.nodes.contains_key(&sys.id) && !self.enclaves.contains_key(&sys.id))
            .filter(|sys| (0.35..=0.8).contains(&(sys.pos.distance(self.hub) / ring)))
            .filter_map(|sys| {
                let world = sys.bodies.iter()
                    .filter(|b| !b.is_belt() && b.kind != crate::body::BodyKind::GasGiant)
                    .max_by_key(|b| (b.profile.size as u8, std::cmp::Reverse(b.id)))?;
                let key = crate::rng::Rng::keyed_id(self.config.seed, "season-precursors-v1", sys.id.0).next_u64();
                Some((key, sys.id, sys.pos, world.id))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut placed: Vec<Vec2> = Vec::new();
        for (_, id, pos, body) in candidates {
            if placed.len() >= n {
                break;
            }
            if placed.iter().any(|p| p.distance(pos) < ring * 0.5) {
                continue;
            }
            placed.push(pos);
            self.season.precursors.insert(id, Precursor { body, ..Default::default() });
        }
    }

    /// On the census: relics wake with the galaxy; held ones attune for a
    /// corporation set on freedom and, full, draw a bearing.
    fn tick_relics(&mut self, events: &mut Vec<Event>) {
        use crate::season::{RESOLVE_ATTUNE, RELIC_SEIZURE_STANDING};
        let now = self.time;
        if self.season.relics.is_empty() && !self.nodes.is_empty() {
            let mut order: Vec<(u64, EntityId)> = self.nodes.keys()
                .map(|id| (crate::rng::Rng::keyed_id(self.config.seed, "season-relics-v1", id.0).next_u64(), *id))
                .collect();
            order.sort();
            let n = order.len();
            for (rank, (_, id)) in order.into_iter().enumerate() {
                self.season.relics.insert(id, Relic { wake_at: crate::season::relic_wake(rank, n), ..Default::default() });
            }
        }
        let ids: Vec<EntityId> = self.season.relics.keys().copied().collect();
        let mut woke = 0;
        for id in &ids {
            let relic = &self.season.relics[id];
            if relic.awake_at.is_some() || self.season.maturity + 1e-9 < relic.wake_at {
                continue;
            }
            self.season.relics.get_mut(id).unwrap().awake_at = Some(now);
            woke += 1;
            let Some((pos, holder)) = self.systems.iter().find(|s| s.id == *id).map(|s| (s.pos, s.owner)) else { continue };
            events.push(Event::new(now, EventPayload::RelicWoke { system: *id, pos }));
            // The Authority wants the relics: a holder loses standing.
            if let Some(holder) = holder.filter(|h| self.players.contains_key(h)) {
                if let Some(corp) = self.players.get_mut(&holder) {
                    corp.tca_standing -= RELIC_SEIZURE_STANDING;
                }
                self.open_crackdown(CrackdownKind::RelicSeizure, pos, Some(*id), Some(holder), false, false, events);
            }
        }
        if woke > 0 {
            let awake = self.season.relics.values().filter(|r| r.awake_at.is_some()).count();
            let headline = if awake == woke { format!("The relics stir: {woke} resonators wake") } else { format!("{woke} more relics wake") };
            self.season.print(now, NewsSource::Gnn, NewsKind::Relic, headline, vec![], None, None);
        }
        let step = crate::season::CENSUS_EVERY_S / crate::season::attune_secs(self.season.maturity);
        for id in ids {
            let relic = &self.season.relics[&id];
            if relic.awake_at.is_none() {
                continue;
            }
            let Some((pos, holder)) = self.systems.iter().find(|s| s.id == id).and_then(|s| Some((s.pos, s.owner?))) else {
                continue;
            };
            if !self.players.contains_key(&holder) || self.resolve(holder) + 1e-9 < RESOLVE_ATTUNE || relic.bearings_to.contains(&holder) {
                continue;
            }
            let relic = self.season.relics.get_mut(&id).unwrap();
            if relic.attuner != Some(holder) {
                relic.attuner = Some(holder);
                relic.resonance = 0.0;
            }
            relic.resonance = (relic.resonance + step).min(1.0);
            if relic.resonance < 1.0 {
                continue;
            }
            relic.resonance = 0.0;
            relic.attuner = None;
            relic.bearings_to.push(holder);
            self.draw_bearing(holder, id, pos, events);
        }
    }

    /// A full relic points toward the nearest sealed world.
    fn draw_bearing(&mut self, holder: PlayerId, relic: EntityId, from: Vec2, events: &mut Vec<Event>) {
        let target = self.season.precursors.iter()
            .filter(|(_, p)| p.landed.is_none())
            .filter_map(|(id, _)| self.systems.iter().find(|s| s.id == *id).map(|s| (*id, s.pos)))
            .min_by(|a, b| a.1.distance(from).total_cmp(&b.1.distance(from)).then(a.0.cmp(&b.0)));
        let Some((target, to)) = target else { return };
        let d = to - from;
        let jitter = (crate::rng::Rng::keyed_id(self.config.seed, "season-bearing-v1", relic.0 ^ holder.0).next_f64() - 0.5)
            * crate::season::BEARING_SPREAD;
        let angle = crate::dmath::atan2(d.y, d.x) + jitter;
        self.season.bearings.push(Bearing { corp: holder, relic, from, angle, at: self.time, target });
        events.push(Event::new(self.time, EventPayload::BearingDrawn { owner: holder, relic, angle }).at_origin(from));
    }

    /// 1 Hz: a fleet arriving at a sealed world with one of its circle's
    /// bearings through it finds the world. Its guardians wake, and the spy
    /// starts the clock on the Authority's war.
    pub(super) fn discover_precursors(&mut self, events: &mut Vec<Event>) {
        if self.season.precursors.is_empty() {
            return;
        }
        let now = self.time;
        let mut finds: Vec<(EntityId, PlayerId, Vec2)> = Vec::new();
        for (sid, p) in &self.season.precursors {
            let Some(pos) = self.systems.iter().find(|s| s.id == *sid).map(|s| s.pos) else { continue };
            let finders: std::collections::BTreeSet<PlayerId> = self.fleets.values()
                .filter(|f| self.players.contains_key(&f.owner) && f.pos.distance(pos) <= crate::transit::HYPERLIMIT)
                .map(|f| f.owner)
                .collect();
            for finder in finders {
                // A syndicate learns it together, once.
                let circle = self.circle(finder);
                if p.found.iter().any(|(who, _)| circle.contains(who)) || finds.iter().any(|(id, f, _)| id == sid && circle.contains(f)) {
                    continue;
                }
                if self.season.bearings.iter().any(|b| circle.contains(&b.corp) && b.holds(pos)) {
                    finds.push((*sid, finder, pos));
                }
            }
        }
        for (sid, finder, pos) in finds {
            let first = self.season.first_found_at.is_none();
            self.season.first_found_at.get_or_insert(now);
            let body = self.season.precursors[&sid].body;
            self.season.precursors.get_mut(&sid).unwrap().found.push((finder, now));
            for member in self.circle(finder) {
                events.push(Event::new(now, EventPayload::PrecursorFound { owner: member, system: sid, body, finder }).at_origin(pos));
            }
            if self.season.precursors[&sid].guard.is_none() {
                self.wake_guardians(sid, pos);
            }
            if first {
                self.season.print(now, NewsSource::Gnn, NewsKind::Keystone,
                    "Registration closes: the Authority seals the charter rolls".into(), vec![], None, None);
            }
            // The spy: the Authority will know.
            let circle = self.circle(finder);
            if !circle.iter().all(|c| self.is_rebel(*c)) && !self.season.leaks.iter().any(|l| circle.contains(&l.corp)) {
                let at = now + crate::season::LEAK_S;
                self.season.leaks.push(Leak { corp: finder, world: sid, at });
                for member in circle {
                    events.push(Event::new(now, EventPayload::SpyLeak { owner: member, war_at: at }));
                }
            }
        }
    }

    /// The sealed world's guardians: a pirate warband on its world.
    fn wake_guardians(&mut self, system: EntityId, pos: Vec2) {
        let id = self.alloc_entity_id();
        let mut guard = Fleet::single(id, PlayerId::PIRATE, ShipKind::Destroyer, pos, FleetOrder::Idle, None);
        let corvettes = 2 + (self.season.maturity * 4.0).round() as u32;
        guard.reset_to(ShipKind::Destroyer, 2);
        guard.add(ShipKind::Corvette, corvettes);
        self.fleets.insert(id, guard);
        if let Some(p) = self.season.precursors.get_mut(&system) {
            p.guard = Some(id);
        }
    }

    /// The sealed world `system` is still guarded.
    pub fn precursor_guarded(&self, system: EntityId) -> bool {
        self.season.precursors.get(&system).and_then(|p| p.guard).is_some_and(|g| self.fleets.contains_key(&g))
    }

    /// 1 Hz: leaks whose time has come become war.
    pub(super) fn tick_leaks(&mut self, events: &mut Vec<Event>) {
        let now = self.time;
        let due: Vec<Leak> = self.season.leaks.iter().filter(|l| l.at <= now).cloned().collect();
        if due.is_empty() {
            return;
        }
        self.season.leaks.retain(|l| l.at > now);
        for leak in due {
            let circle = self.circle(leak.corp);
            let name = self.players.get(&leak.corp).and_then(|c| c.syndicate).and_then(|id| self.syndicates.get(&id))
                .map_or_else(|| self.corp_label(leak.corp), |s| s.name.clone());
            for member in &circle {
                self.declare_rebel(*member, events);
            }
            self.season.print(now, NewsSource::Gnn, NewsKind::War,
                format!("The Authority declares war on {name}: insurgents seek to close the gates"), circle, None, None);
        }
    }

    /// Permanent war with the Authority: proscribed, standing frozen, the
    /// enforcement squadrons and the war fleet, bounties on every hull.
    pub(super) fn declare_rebel(&mut self, who: PlayerId, events: &mut Vec<Event>) {
        if self.is_rebel(who) || !self.players.contains_key(&who) {
            return;
        }
        self.season.rebels.insert(who, self.time);
        if let Some(sq) = self.season.squeeze.get_mut(&who) {
            sq.rise = None;
            sq.requisitions.clear();
        }
        // Resting orders are grandfathered (as for a revoked charter): no new trades.
        events.push(Event::new(self.time, EventPayload::AuthorityWar { owner: who }).at_origin(self.hub));
    }

    /// A colony landed on a sealed world: public from now, and its holder at war.
    pub(super) fn precursor_landed(&mut self, system: EntityId, owner: PlayerId, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(p) = self.season.precursors.get_mut(&system) else { return };
        if p.landed.is_none() {
            p.landed = Some((owner, now));
            let headline = format!("{} lands on the sealed world at {}: the Authority declares war", self.corp_label(owner), self.system_label(system));
            self.season.print(now, NewsSource::Gnn, NewsKind::War, headline, vec![owner], Some(system), None);
        }
        self.declare_rebel(owner, events);
    }

    /// Bounties: the Authority pays whoever sinks a rebel's hulls in battle
    /// (`corps` the two sides, `lost` the hulls each lost).
    pub(super) fn pay_bounties(&mut self, corps: [PlayerId; 2], lost: [u32; 2], system: Option<EntityId>) {
        for (side, other) in [(0, 1), (1, 0)] {
            let (rebel, hunter) = (corps[side], corps[other]);
            if !self.is_rebel(rebel) || self.is_rebel(hunter) || !self.players.contains_key(&hunter) || lost[side] == 0 {
                continue;
            }
            let hulls = lost[side];
            self.wire_home(hunter, hulls as f64 * crate::season::BOUNTY_PER_HULL, self.hub);
            *self.season.stoppers.entry(hunter).or_default() += hulls as f64 * crate::season::STOP_HULL;
            let headline = format!("Bounty paid: {} sinks {hulls} insurgent hulls", self.corp_label(hunter));
            self.season.print(self.time, NewsSource::Gnn, NewsKind::Wanted, headline, vec![hunter, rebel], system, None);
        }
    }
}


// ── The Keystone, its powers, the Seal and the endings ──
impl World {
    /// The completed Keystone whose powers `who` commands (its circle holds a
    /// sealed world at level 10), if any.
    pub fn keystone_of(&self, who: PlayerId) -> Option<EntityId> {
        if self.season.ended.is_some() || !self.players.contains_key(&who) {
            return None;
        }
        let circle = self.circle(who);
        self.season.precursors.iter()
            .filter(|(_, p)| p.keystone.as_ref().is_some_and(|k| k.level >= crate::season::KEYSTONE_LEVELS))
            .find(|(id, _)| self.systems.iter().any(|s| s.id == **id && s.owner.is_some_and(|o| circle.contains(&o))))
            .map(|(id, _)| *id)
    }

    /// `who` commands a completed Keystone's four powers.
    pub fn keystone_powers(&self, who: PlayerId) -> bool {
        self.keystone_of(who).is_some()
    }

    /// Relic systems `who`'s circle holds, awake.
    fn relics_held(&self, who: PlayerId) -> Vec<EntityId> {
        let circle = self.circle(who);
        self.season.relics.iter()
            .filter(|(id, r)| r.awake_at.is_some()
                && self.systems.iter().any(|s| s.id == **id && s.owner.is_some_and(|o| circle.contains(&o))))
            .map(|(id, _)| *id)
            .collect()
    }

    fn keystone_scale(&self) -> f64 {
        (self.home_slots.len() as f64 / 20.0).max(0.25)
    }

    /// Feed a Keystone from its world's stockpile. Feeding one is war.
    pub(super) fn feed_keystone(&mut self, who: PlayerId, system: EntityId, commodity: Commodity, units: u32, events: &mut Vec<Event>) {
        let now = self.time;
        if self.season.ended.is_some() || units == 0 {
            return;
        }
        let circle = self.circle(who);
        let Some(p) = self.season.precursors.get(&system) else { return };
        let found = p.found.iter().any(|(f, _)| circle.contains(f)) || p.landed.is_some();
        let Some(sys) = self.systems.iter().find(|s| s.id == system) else { return };
        if !found || !sys.owner.is_some_and(|o| circle.contains(&o)) {
            events.push(Event::new(now, EventPayload::IndustryRejected {
                owner: who, system, reason: "A Keystone is fed on a sealed world your syndicate holds".into(),
            }));
            return;
        }
        let level = p.keystone.as_ref().map_or(0, |k| k.level);
        if level >= crate::season::KEYSTONE_LEVELS {
            return;
        }
        let basket = crate::season::keystone_basket(level + 1, self.keystone_scale());
        let Some(need) = basket.iter().find(|(c, _)| *c == commodity).map(|(_, n)| *n) else {
            events.push(Event::new(now, EventPayload::IndustryRejected {
                owner: who, system, reason: format!("Level {} takes no {}", level + 1, commodity.name()),
            }));
            return;
        };
        let paid = p.keystone.as_ref().and_then(|k| k.paid.get(&commodity)).copied().unwrap_or(0.0);
        let have = sys.stockpile.get(&commodity).copied().unwrap_or(0.0);
        let take = (units as f64).min(need - paid).min(have.floor()).max(0.0);
        if take <= 0.0 {
            return;
        }
        if let Some(sys) = self.systems.iter_mut().find(|s| s.id == system) {
            *sys.stockpile.entry(commodity).or_insert(0.0) -= take;
        }
        let keystone = self.season.precursors.get_mut(&system).unwrap().keystone.get_or_insert_with(Keystone::default);
        *keystone.paid.entry(commodity).or_insert(0.0) += take;
        events.push(Event::new(now, EventPayload::KeystoneFed { owner: who, system, commodity, units: take as u32 }));
        // Anyone who feeds a Keystone is at war with the Authority at once.
        self.declare_rebel(who, events);
        self.advance_keystone(system, events);
    }

    /// A Keystone whose basket is paid (and whose builders hold the relics it
    /// asks) rises a level.
    fn advance_keystone(&mut self, system: EntityId, events: &mut Vec<Event>) {
        let now = self.time;
        let Some(holder) = self.systems.iter().find(|s| s.id == system).and_then(|s| s.owner) else { return };
        loop {
            let Some(k) = self.season.precursors.get(&system).and_then(|p| p.keystone.as_ref()) else { return };
            let next = k.level + 1;
            if next > crate::season::KEYSTONE_LEVELS {
                return;
            }
            let basket = crate::season::keystone_basket(next, self.keystone_scale());
            let paid = basket.iter().all(|(c, n)| k.paid.get(c).copied().unwrap_or(0.0) + 1e-6 >= *n);
            if !paid || self.relics_held(holder).len() < crate::season::keystone_relics(next) {
                return;
            }
            let k = self.season.precursors.get_mut(&system).unwrap().keystone.as_mut().unwrap();
            k.level = next;
            k.paid.clear();
            let pos = self.systems.iter().find(|s| s.id == system).map_or(self.hub, |s| s.pos);
            events.push(Event::new(now, EventPayload::KeystoneLevel { holder, system, level: next, fell: false }).at_origin(pos));
            self.season.print(self.heard_at_hub(now, pos), NewsSource::Gnn, NewsKind::Keystone,
                format!("Keystone at {}: level {next} ({})", self.system_label(system), self.corp_label(holder)), vec![holder], Some(system), None);
            if crate::season::KEYSTONE_CRACKDOWNS.contains(&next) {
                self.open_crackdown(CrackdownKind::Keystone, pos, Some(system), Some(holder), false, false, events);
                let until = now + crate::season::CRACKDOWN_S;
                for member in self.circle(holder) {
                    let sq = self.season.squeeze.entry(member).or_default();
                    sq.enforced_until = sq.enforced_until.max(until);
                }
            }
            if next == crate::season::KEYSTONE_LEVELS {
                self.complete_keystone(system, holder, pos);
            }
        }
    }

    /// Level 10: the four powers wake and the Seal begins its public charge.
    fn complete_keystone(&mut self, system: EntityId, holder: PlayerId, pos: Vec2) {
        let now = self.time;
        let raise_fleet = {
            let k = self.season.precursors.get_mut(&system).unwrap().keystone.as_mut().unwrap();
            k.charge_since = Some(now);
            k.fleet.is_none()
        };
        // The Precursor Fleet: the network's own warships. Losses are permanent.
        if raise_fleet {
            let id = self.alloc_entity_id();
            let mut fleet = Fleet::single(id, holder, ShipKind::Titan, pos, FleetOrder::Idle, None);
            fleet.add(ShipKind::Dreadnought, 2);
            self.fleets.insert(id, fleet);
            self.season.precursors.get_mut(&system).unwrap().keystone.as_mut().unwrap().fleet = Some(id);
        }
        self.season.print(self.heard_at_hub(now, pos), NewsSource::Gnn, NewsKind::Keystone,
            format!("The Keystone at {} is complete: the Seal charges, 48h", self.system_label(system)), vec![holder], Some(system), None);
        self.season.print(self.heard_at_hub(now, pos), NewsSource::FreeSignal, NewsKind::Keystone,
            format!("{} holds the network: hold for 48h and every gate closes", self.corp_label(holder)), vec![holder], Some(system), None);
        // The Authority throws its war fleet at a completed Keystone.
        for member in self.circle(holder) {
            let sq = self.season.squeeze.entry(member).or_default();
            sq.enforced_until = sq.enforced_until.max(now + crate::season::CHARGE_S);
        }
    }

    /// Gate Step: a fleet at the Keystone world or a held relic moves at once
    /// to another of them.
    pub(super) fn gate_step(&mut self, who: PlayerId, fleet: EntityId, to: EntityId, events: &mut Vec<Event>) {
        let Some(keystone) = self.keystone_of(who) else { return };
        let mut gates = self.relics_held(who);
        gates.push(keystone);
        let pos_of = |id: EntityId| self.systems.iter().find(|s| s.id == id).map(|s| s.pos);
        let Some(dest) = gates.contains(&to).then(|| pos_of(to)).flatten() else { return };
        let Some(f) = self.fleets.get(&fleet).filter(|f| f.owner == who) else { return };
        let at_gate = gates.iter().filter_map(|g| pos_of(*g)).any(|p| f.pos.distance(p) <= crate::transit::HYPERLIMIT);
        if !at_gate {
            return;
        }
        let f = self.fleets.get_mut(&fleet).unwrap();
        f.pos = dest;
        f.vel = Vec2::ZERO;
        f.order = FleetOrder::Idle;
        events.push(Event::new(self.time, EventPayload::GateStepped { owner: who, fleet, system: to }).at_origin(dest));
    }

    /// 1 Hz: sieges knock Keystones down; a held charge fires the Seal; the
    /// race and the backstop end the season.
    pub(super) fn tick_keystones(&mut self, events: &mut Vec<Event>) {
        if self.season.ended.is_some() {
            return;
        }
        let now = self.time;
        let systems: Vec<EntityId> = self.season.precursors.iter()
            .filter(|(_, p)| p.keystone.as_ref().is_some_and(|k| k.level > 0))
            .map(|(id, _)| *id)
            .collect();
        for system in systems {
            let Some(sys) = self.systems.iter().find(|s| s.id == system) else { continue };
            let (pos, holder) = (sys.pos, sys.owner);
            let circle = holder.map(|h| self.circle(h)).unwrap_or_default();
            let siege = sys.blockade.as_ref().filter(|b| b.held_since.is_some() && !circle.contains(&b.by)).map(|b| b.by);
            let k = self.season.precursors.get_mut(&system).unwrap().keystone.as_mut().unwrap();
            match siege {
                Some(by) => {
                    let since = *k.siege_tick_at.get_or_insert(now);
                    if now - since >= crate::season::KEYSTONE_SIEGE_TICK_S {
                        k.siege_tick_at = Some(now);
                        k.level -= 1;
                        k.paid.clear();
                        k.charge_since = None;
                        let level = k.level;
                        *self.season.stoppers.entry(by).or_default() += crate::season::STOP_LEVEL;
                        if let Some(h) = holder {
                            events.push(Event::new(now, EventPayload::KeystoneLevel { holder: h, system, level, fell: true }).at_origin(pos));
                        }
                        self.season.print(self.heard_at_hub(now, pos), NewsSource::Gnn, NewsKind::Keystone,
                            format!("Keystone at {} knocked down to level {level} by {}", self.system_label(system), self.corp_label(by)),
                            [Some(by), holder].into_iter().flatten().collect(), Some(system), None);
                    }
                }
                None => k.siege_tick_at = None,
            }
            // The Seal: 48 hours held at level 10.
            let k = self.season.precursors[&system].keystone.as_ref().unwrap();
            if k.level >= crate::season::KEYSTONE_LEVELS && k.charge_since.is_some_and(|t| now - t >= crate::season::CHARGE_S)
                && let Some(h) = holder
            {
                self.end_season(Ending::Sovereign { holder: h, system, at: now }, events);
                return;
            }
        }
        if let Some(found) = self.season.first_found_at {
            if now - found >= crate::season::RACE_S {
                let winners = self.charter_winners();
                self.end_season(Ending::CharterHolds { at: now, winners }, events);
            }
        } else if self.season.started_at.is_some_and(|t| now - t >= crate::season::BACKSTOP_S) {
            let winners = self.charter_winners();
            self.end_season(Ending::CharterHolds { at: now, winners }, events);
        }
    }

    /// The Charter Holds: the loyalists, and the three who did most to stop
    /// the Keystones.
    fn charter_winners(&self) -> Vec<PlayerId> {
        let mut stoppers: Vec<(PlayerId, f64)> = self.season.stoppers.iter()
            .filter(|(id, score)| **score > 0.0 && !self.is_rebel(**id)).map(|(id, s)| (*id, *s)).collect();
        stoppers.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut winners: Vec<PlayerId> = stoppers.into_iter().take(3).map(|(id, _)| id).collect();
        for (id, corp) in &self.players {
            if !winners.contains(id) && !self.is_rebel(*id) && self.loyal(*id)
                && self.time - corp.last_active <= crate::syndicate::ACTIVE_WINDOW_S
            {
                winners.push(*id);
            }
        }
        winners
    }

    fn end_season(&mut self, ending: Ending, events: &mut Vec<Event>) {
        let now = self.time;
        let headline = match &ending {
            Ending::Sovereign { holder, system, .. } => format!("The Seal fires at {}: every gate closes. {} rules the galaxy",
                self.system_label(*system), self.corp_label(*holder)),
            Ending::CharterHolds { .. } => if self.season.first_found_at.is_some() {
                "The race is over: no Seal fired. The Charter holds".to_string()
            } else {
                "The Authority reseals the Precursor worlds. The Charter holds".to_string()
            },
        };
        let free = matches!(ending, Ending::Sovereign { .. });
        self.season.print(now, if free { NewsSource::FreeSignal } else { NewsSource::Gnn }, NewsKind::Keystone, headline, vec![], None, None);
        self.season.ended = Some(ending.clone());
        events.push(Event::new(now, EventPayload::SeasonEnded { ending }).at_origin(self.hub));
    }

    /// Credit a Keystone world's capture to its taker.
    pub(super) fn note_keystone_capture(&mut self, system: EntityId, by: PlayerId) {
        if self.season.precursors.get(&system).and_then(|p| p.keystone.as_ref()).is_some_and(|k| k.level > 0) {
            *self.season.stoppers.entry(by).or_default() += crate::season::STOP_CAPTURE;
            if let Some(k) = self.season.precursors.get_mut(&system).and_then(|p| p.keystone.as_mut()) {
                k.charge_since = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::season::CENSUS_EVERY_S;
    use crate::settlement::FoundingProfile;

    fn galaxy() -> World {
        let mut w = World::new(SimConfig::for_players(12648430, 4));
        w.enclaves.clear();
        w
    }

    fn found(w: &mut World, ids: &[(u64, &str)]) -> Vec<Event> {
        let cmds: Vec<Command> = ids.iter().map(|(id, name)| Command::FoundColony {
            id: PlayerId(*id), name: (*name).into(), profile: FoundingProfile::CoastalPlains,
        }).collect();
        w.step(&cmds)
    }

    /// Run the tick that takes the next census.
    fn census(w: &mut World) -> Vec<Event> {
        let every = (CENSUS_EVERY_S * crate::config::TICK_HZ as f64) as u64;
        w.tick = (w.tick / every + 1) * every - 1;
        w.step(&[])
    }

    #[test]
    fn a_grown_galaxy_hears_each_decree_once_in_order() {
        let mut w = galaxy();
        let a = PlayerId(1);
        found(&mut w, &[(1, "Alpha")]);
        census(&mut w);
        assert_eq!(w.season.grip, Grip::Charter, "a new galaxy starts under the plain charter");
        // Alpha grows: eight colonies past home and fifty programmes.
        let home = w.players[&a].home_system;
        let free: Vec<EntityId> = w.systems.iter().filter(|s| s.owner.is_none() && Some(s.id) != home).take(8).map(|s| s.id).collect();
        for id in free {
            w.systems.iter_mut().find(|s| s.id == id).unwrap().owner = Some(a);
        }
        let programmes: Vec<String> = crate::research::visible_ids().take(50).map(String::from).collect();
        w.players.get_mut(&a).unwrap().research.completed.extend(programmes);
        let events = census(&mut w);
        let decrees: Vec<Grip> = events.iter().filter_map(|e| match e.payload {
            EventPayload::Decree { grip, .. } => Some(grip),
            _ => None,
        }).collect();
        assert_eq!(decrees, [Grip::Levies, Grip::Inspections, Grip::Martial]);
        assert_eq!(w.season.grip, Grip::Martial);
        assert_eq!(w.season.news.iter().filter(|n| n.kind == NewsKind::Decree).count(), 3);
        // Maturity never falls, even when the colonies go.
        for s in w.systems.iter_mut().filter(|s| s.owner == Some(a) && Some(s.id) != home) {
            s.owner = None;
        }
        census(&mut w);
        assert!((w.season.maturity - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_time_floor_moves_a_quiet_galaxy() {
        let mut w = galaxy();
        found(&mut w, &[(1, "Alpha")]);
        census(&mut w);
        let started = w.season.started_at.unwrap();
        w.time = started + crate::season::MATURITY_FLOOR_S * 0.3;
        census(&mut w);
        assert_eq!(w.season.grip, Grip::Levies, "30% of the floor's ten weeks decrees the levies");
    }

    #[test]
    fn a_far_command_center_reads_an_older_census() {
        let mut w = galaxy();
        let a = PlayerId(1);
        found(&mut w, &[(1, "Alpha")]);
        census(&mut w);
        let cc = w.players[&a].command_center;
        let delay = crate::transit::delay(w.hub, cc, w.config.c);
        assert!(delay > 1.0);
        let first = w.season.census[0];
        assert!(w.season.census_for(cc, w.hub, w.config.c, first.at + delay - 0.5).is_none());
        assert_eq!(w.season.census_for(cc, w.hub, w.config.c, first.at + delay).map(|c| c.at), Some(first.at));
    }

    #[test]
    fn new_charters_and_captures_are_news() {
        let mut w = galaxy();
        let a = PlayerId(1);
        found(&mut w, &[(1, "Alpha")]);
        census(&mut w);
        found(&mut w, &[(2, "Beta"), (3, "Gamma")]);
        let latest = w.season.news.back().unwrap();
        assert_eq!(latest.kind, NewsKind::Corporation);
        assert!(latest.headline.contains("Beta") && latest.headline.contains("Gamma"), "{}", latest.headline);
        let sys = w.systems[0].id;
        let pos = w.systems[0].pos;
        w.publish_news(&[Event::new(w.time, EventPayload::SystemCaptured {
            old_owner: PlayerId(2), new_owner: a, system: sys, pos, plunder: Default::default(),
        })]);
        let capture = w.season.news.iter().find(|n| n.kind == NewsKind::Capture).unwrap();
        assert!(capture.headline.starts_with("Alpha takes "), "{}", capture.headline);
        assert!((capture.at - (w.time + crate::transit::delay(pos, w.hub, w.config.c))).abs() < 1e-6,
            "the Hub broadcasts it once it has heard");
    }

    /// A founder past its charter under the levies.
    fn levied() -> (World, PlayerId) {
        let mut w = galaxy();
        let a = PlayerId(1);
        found(&mut w, &[(1, "Alpha")]);
        census(&mut w);
        w.players.get_mut(&a).unwrap().founding.stage = crate::founding::FoundingStage::Complete;
        w.season.grip = Grip::Levies;
        // Neither loyal nor a rebel: the plain levy.
        w.season.squeeze.entry(a).or_default().resolve = 0.3;
        (w, a)
    }

    /// A sale settling at the Hub, then the levy pass that follows it in a tick.
    fn sell(w: &mut World, a: PlayerId, commodity: Commodity, units: u32) -> Vec<Event> {
        w.players.get_mut(&a).unwrap().warehouse.insert(commodity, units);
        let mut events = Vec::new();
        w.apply_now(&Command::MarketSell { player_id: a, commodity, units, min_unit_price: None }, &mut events);
        w.assess_levies(&mut events);
        events
    }

    #[test]
    fn the_levy_takes_its_share_of_every_hub_trade_from_grip_two() {
        let (mut w, a) = levied();
        assert!((w.levy_frac(a) - 0.05).abs() < 1e-9);
        let events = sell(&mut w, a, Commodity::Alloys, 100);
        let (gross, penalty) = events.iter().find_map(|e| match e.payload {
            EventPayload::Trade(TradeEvent::Sold { units, unit_price, penalty, .. }) => Some((units as f64 * unit_price, penalty)),
            _ => None,
        }).expect("sold");
        assert!((penalty - gross * 0.05).abs() < 1e-6, "5% levy in good standing: {penalty} of {gross}");
        assert!((w.season.squeeze[&a].levy_paid - gross * 0.05).abs() < 1e-6);
        w.season.grip = Grip::Charter;
        assert_eq!(w.levy_frac(a), 0.0, "no levy under the plain charter");
    }

    #[test]
    fn a_refused_rise_embargoes_the_good_until_the_arrears_are_paid() {
        let (mut w, a) = levied();
        let price = w.market.price(Commodity::Alloys);
        let units = (crate::season::LEVY_RISE_TRADE / price * 2.0).ceil() as u32;
        let events = sell(&mut w, a, Commodity::Alloys, units);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::LevyRiseDemanded { owner, .. } if owner == a)), "a big trade draws a rise: {:?}",
            events.iter().map(|e| format!("{:?}", e.payload)).collect::<Vec<_>>());
        let rise = w.season.squeeze[&a].rise.clone().unwrap();
        w.receive_admin_for_test(&[Command::AnswerLevy { player_id: a, accept: false }]);
        assert!(w.embargoed(a, Commodity::Alloys));
        assert!((w.levy_frac(a) - 0.05).abs() < 1e-9, "refusing keeps the levy where it was");
        let refused = sell(&mut w, a, Commodity::Alloys, 10);
        assert!(refused.iter().any(|e| matches!(e.payload,
            EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::Embargoed, .. }))), "an embargoed good stays off the Exchange");
        let arrears = (rise.trade * crate::season::ARREARS_FRAC).round();
        w.players.get_mut(&a).unwrap().credits = arrears + 10.0;
        w.receive_admin_for_test(&[Command::PayArrears { player_id: a, commodity: Commodity::Alloys }]);
        assert!(!w.embargoed(a, Commodity::Alloys), "paid, it is lifted");
        assert!((w.players[&a].credits - 10.0).abs() < 1e-6);
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Crackdown && n.headline.contains("defies the levy")));
    }

    #[test]
    fn an_unanswered_rise_is_accepted_and_one_a_day() {
        let (mut w, a) = levied();
        let price = w.market.price(Commodity::Alloys);
        let units = (crate::season::LEVY_RISE_TRADE / price * 2.0).ceil() as u32;
        sell(&mut w, a, Commodity::Alloys, units);
        let due = w.season.squeeze[&a].rise.as_ref().unwrap().due;
        sell(&mut w, a, Commodity::Alloys, units);
        w.time = due + 1.0;
        census(&mut w);
        assert!(w.season.squeeze[&a].rise.is_none());
        assert!((w.levy_frac(a) - 0.07).abs() < 1e-9, "the default is to accept: 5% + 2%");
    }

    #[test]
    fn requisitions_lapse_into_refusals_and_three_bring_a_squadron() {
        let (mut w, a) = levied();
        let mut issued = 0;
        for round in 0..3 {
            // The next requisition is due; it lapses unanswered.
            let sq = w.season.squeeze.entry(a).or_default();
            sq.next_requisition_at = Some(w.time);
            w.players.get_mut(&a).unwrap().last_active = w.time + 1.0;
            let events = census(&mut w);
            issued += events.iter().filter(|e| matches!(e.payload, EventPayload::RequisitionIssued { .. })).count();
            let due = w.season.squeeze[&a].requisitions[0].due;
            w.time = due + 1.0;
            w.players.get_mut(&a).unwrap().last_active = w.time;
            let before = w.players[&a].tca_standing;
            let events = census(&mut w);
            let settled = events.iter().find_map(|e| match e.payload {
                EventPayload::RequisitionSettled { paid, by_default, enforced, .. } => Some((paid, by_default, enforced)),
                _ => None,
            }).expect("settled");
            assert_eq!(settled, (false, true, round == 2), "lapsed, refused by default; the third brings enforcement");
            assert!(w.players[&a].tca_standing < before, "standing falls");
        }
        assert_eq!(issued, 3);
        assert_eq!(w.season.squeeze[&a].refusals, 3);
        assert!((w.levy_frac(a) - 0.08).abs() < 1e-9, "each refusal adds a point");
        assert!(w.enforcement_due(a), "a squadron is due");
        let mut events = Vec::new();
        w.enforcement_ai(&mut events);
        assert!(w.expeditions.values().any(|e| e.target_corp == a), "it sails");
    }

    #[test]
    fn a_requisition_paid_in_goods_comes_from_the_hub_warehouse() {
        let (mut w, a) = levied();
        w.season.next_requisition = 1; // the next one asks for goods
        w.players.get_mut(&a).unwrap().warehouse.insert(Commodity::Alloys, 5_000);
        let sq = w.season.squeeze.entry(a).or_default();
        sq.next_requisition_at = Some(w.time);
        census(&mut w);
        let req = w.season.squeeze[&a].requisitions[0].clone();
        let Demand::Goods { commodity, units } = req.demand else { panic!("goods: {:?}", req.demand) };
        let held = w.players[&a].warehouse.get(&commodity).copied().unwrap_or(0);
        w.players.get_mut(&a).unwrap().warehouse.insert(commodity, held.max(units));
        let before = w.players[&a].warehouse[&commodity];
        w.receive_admin_for_test(&[Command::AnswerRequisition { player_id: a, id: req.id, pay: true }]);
        assert!(w.season.squeeze[&a].requisitions.is_empty());
        assert_eq!(w.players[&a].warehouse.get(&commodity).copied().unwrap_or(0), before - units);
        assert_eq!(w.season.squeeze[&a].refusals, 0);
    }


    fn rebel(w: &mut World, a: PlayerId, resolve: f64) {
        w.season.squeeze.entry(a).or_default().resolve = resolve;
    }

    #[test]
    fn resolve_builds_from_the_squeeze_fades_and_rallies_in_halls() {
        let (mut w, a) = levied();
        let mut events = Vec::new();
        rebel(&mut w, a, 0.0);
        w.add_resolve(a, 0.40, &mut events);
        assert!(!w.loyal(a) && (w.resolve(a) - 0.40).abs() < 1e-9);
        // A day without grievance fades it three points.
        for _ in 0..(1_440.0 / (crate::season::CENSUS_EVERY_S / 60.0)) as usize {
            w.tick_resolve(&mut events);
        }
        assert!((w.resolve(a) - 0.37).abs() < 1e-6, "{}", w.resolve(a));
        // A level-III Assembly Hall rallies 3% an hour.
        let home = w.players[&a].home_system.unwrap();
        let sys = w.systems.iter_mut().find(|s| s.id == home).unwrap();
        let cc = sys.bodies.iter().find(|b| b.tier(crate::StructureKind::CommandCenter) > 0).unwrap().id;
        sys.bodies.iter_mut().find(|b| b.id == cc).unwrap().set_tier(crate::StructureKind::AssemblyHall, 3);
        let before = w.resolve(a);
        for _ in 0..60 {
            w.tick_resolve(&mut events);
        }
        assert!((w.resolve(a) - before - (0.03 - 0.03 / 24.0)).abs() < 1e-6, "{}", w.resolve(a) - before);
        // Crossing 50% is news on the Free Signal, and production rises.
        w.add_resolve(a, 0.2, &mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::ResolveMilestone { resolve, .. } if resolve == 0.5)));
        assert!(w.season.news.iter().any(|n| n.source == NewsSource::FreeSignal && n.headline.contains("rallies its colonists")));
        assert!((w.production_mods(a).extraction / w.research_mod(a, crate::research::ModKey::ExtractionRate) - 1.05).abs() < 1e-9);
    }

    #[test]
    fn loyalists_pay_a_lighter_levy() {
        let (mut w, a) = levied();
        rebel(&mut w, a, 0.0);
        assert!(w.loyal(a));
        assert!((w.levy_frac(a) - 0.03).abs() < 1e-9, "5% less the loyal 2%: {}", w.levy_frac(a));
        rebel(&mut w, a, 0.3);
        assert!((w.levy_frac(a) - 0.05).abs() < 1e-9);
    }

    #[test]
    fn the_galaxys_resolve_brings_the_emergency_decree() {
        let (mut w, a) = levied();
        rebel(&mut w, a, 0.6);
        w.players.get_mut(&a).unwrap().last_active = w.time;
        let mut events = Vec::new();
        w.tick_resolve(&mut events);
        assert!(w.season.emergency);
        assert_eq!(w.season.grip, Grip::Inspections, "the grip jumps a stage early");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::Decree { emergency: true, .. })));
        assert!((w.levy_frac(a) - (0.07 + 0.02)).abs() < 1e-9, "and every levy rises");
        assert!(w.season.war_fleet > crate::season::WAR_FLEET_BASE as u32, "the war fleet grows with the galaxy's Resolve");
    }

    #[test]
    fn sabotage_doubles_hub_fees_and_its_evidence_can_expose_it() {
        let (mut w, a) = levied();
        w.season.grip = Grip::Inspections;
        w.season.galaxy_resolve = 0.3;
        let mut events = Vec::new();
        // Find a seed offset where the sabotage is staged.
        while !w.staged_roll(w.season.next_crackdown + 1) {
            w.season.next_crackdown += 1;
        }
        w.tick_crackdowns(&mut events);
        let c = w.season.crackdowns.iter().find(|c| c.kind == CrackdownKind::Sabotage).cloned().expect("sabotage");
        assert!(c.staged);
        assert_eq!(w.hub_fee_mult(), 2.0);
        // A Scout at the Hub reads the evidence: exposed.
        let scout = w.alloc_entity_id();
        w.fleets.insert(scout, Fleet::single(scout, a, ShipKind::Scout, w.hub, FleetOrder::Idle, None));
        let before = w.resolve(a);
        let mut events = Vec::new();
        w.investigate_evidence(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::CrackdownExposed { by, .. } if by == a)));
        assert_eq!(w.hub_fee_mult(), 1.0, "the crackdown lifts at once");
        assert!((w.resolve(a) - before - crate::season::RESOLVE_EXPOSED).abs() < 1e-9, "every corporation's Resolve rises");
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Retraction && n.source == NewsSource::Gnn));
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Retraction && n.source == NewsSource::FreeSignal));
    }

    #[test]
    fn a_destroyed_convoy_is_blamed_on_the_nearest_rebel() {
        let (mut w, a) = levied();
        found(&mut w, &[(2, "Beta")]);
        let b = PlayerId(2);
        rebel(&mut w, a, 0.6);
        let pos = w.players[&a].command_center;
        let mut events = vec![Event::new(w.time, EventPayload::Citation {
            culprit: b, offense: crate::tca::CitationOffense::FreightDestroyed, pos, occurred_at: w.time,
        })];
        w.answer_incidents(&mut events);
        let c = w.season.crackdowns.iter().find(|c| c.kind == CrackdownKind::Massacre).expect("massacre");
        assert_eq!(c.target, Some(a), "the rebel is named, not the one who did it");
        assert!(!c.staged);
        assert!(w.enforcement_due(a));
        // Its evidence holds: a Scout learns it was real.
        let scout = w.alloc_entity_id();
        w.fleets.insert(scout, Fleet::single(scout, b, ShipKind::Scout, pos, FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.investigate_evidence(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::EvidenceRead { owner, staged: false, .. } if owner == b)));
        assert!(w.enforcement_due(a), "a real massacre's crackdown runs its course");
    }

    #[test]
    fn a_quarantined_system_gets_no_authority_freight() {
        let (mut w, a) = levied();
        let home = w.players[&a].home_system.unwrap();
        w.players.get_mut(&a).unwrap().warehouse.insert(Commodity::Alloys, 100);
        let mut events = Vec::new();
        let pos = w.systems.iter().find(|s| s.id == home).unwrap().pos;
        w.open_crackdown(CrackdownKind::Quarantine, pos, Some(home), Some(a), false, false, &mut events);
        assert!(w.season.quarantined(home, w.time));
        let events = w.receive_admin_for_test(&[Command::BookFreightOut { player_id: a, system: home, commodity: Commodity::Alloys, units: 10 }]);
        assert!(events.iter().any(|e| matches!(e.payload,
            EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::DestinationBlockaded, .. }))));
    }

    #[test]
    fn no_assembly_hall_before_the_levies() {
        let mut w = galaxy();
        let a = PlayerId(1);
        found(&mut w, &[(1, "Alpha")]);
        w.players.get_mut(&a).unwrap().founding.stage = crate::founding::FoundingStage::Complete;
        let home = w.players[&a].home_system.unwrap();
        let events = w.receive_admin_for_test(&[Command::DevelopSystem {
            player_id: a, system_id: home, upgrade: crate::StructureKind::AssemblyHall, body_id: None,
        }]);
        assert!(events.iter().any(|e| matches!(&e.payload, EventPayload::IndustryRejected { reason, .. } if reason.contains("no grievance"))));
    }


    /// A galaxy with its relics and sealed worlds placed, one founder in it.
    fn journey() -> (World, PlayerId) {
        let (mut w, a) = levied();
        w.place_precursors();
        assert!(!w.season.precursors.is_empty(), "sealed worlds placed");
        let mut events = Vec::new();
        w.tick_relics(&mut events);
        assert!(!w.season.relics.is_empty(), "every node is a relic");
        (w, a)
    }

    fn give(w: &mut World, sys: EntityId, owner: PlayerId) {
        w.systems.iter_mut().find(|s| s.id == sys).unwrap().owner = Some(owner);
    }

    #[test]
    fn relics_wake_in_waves_and_the_authority_wants_them() {
        let (mut w, a) = journey();
        assert!(w.season.relics.values().all(|r| r.awake_at.is_none()), "asleep in a young galaxy");
        let n = w.season.relics.len();
        let first = w.season.relics.values().filter(|r| r.wake_at <= crate::season::RELIC_FIRST_WAVE + 1e-9).count();
        assert_eq!(first, ((n as f64) / 3.0).ceil() as usize);
        assert!(w.season.relics.values().all(|r| r.wake_at <= crate::season::RELIC_LAST_WAVE + 1e-9));
        // Alpha holds a first-wave relic.
        let held = *w.season.relics.iter().find(|(_, r)| r.wake_at <= 0.3).unwrap().0;
        give(&mut w, held, a);
        let standing = w.players[&a].tca_standing;
        w.season.maturity = 0.31;
        let mut events = Vec::new();
        w.tick_relics(&mut events);
        let woke = events.iter().filter(|e| matches!(e.payload, EventPayload::RelicWoke { .. })).count();
        assert_eq!(woke, first, "the first wave wakes at 30%");
        assert!(w.players[&a].tca_standing < standing, "the holder loses standing");
        assert!(w.season.crackdowns.iter().any(|c| c.kind == CrackdownKind::RelicSeizure && c.target == Some(a)));
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Relic && n.headline.starts_with("The relics stir")));
    }

    /// Alpha holds an awake relic and is set on freedom; returns the relic.
    fn attuned(w: &mut World, a: PlayerId) -> EntityId {
        let relic = *w.season.relics.iter().find(|(_, r)| r.wake_at <= 0.3).unwrap().0;
        give(w, relic, a);
        rebel(w, a, 0.6);
        w.season.maturity = 1.0;
        let mut events = Vec::new();
        w.tick_relics(&mut events);
        let steps = (crate::season::ATTUNE_MATURE_S / crate::season::CENSUS_EVERY_S).ceil() as usize;
        for _ in 0..steps {
            w.tick_relics(&mut events);
        }
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::BearingDrawn { owner, .. } if owner == a)), "a full relic gives a bearing");
        relic
    }

    #[test]
    fn a_full_relic_points_at_the_nearest_sealed_world() {
        let (mut w, a) = journey();
        // Too little Resolve: nothing attunes.
        let relic = *w.season.relics.iter().find(|(_, r)| r.wake_at <= 0.3).unwrap().0;
        give(&mut w, relic, a);
        rebel(&mut w, a, 0.3);
        w.season.maturity = 1.0;
        let mut events = Vec::new();
        w.tick_relics(&mut events);
        w.tick_relics(&mut events);
        assert_eq!(w.season.relics[&relic].resonance, 0.0, "only those set on freedom attune");
        attuned(&mut w, a);
        let bearing = w.season.bearings.iter().find(|b| b.corp == a).unwrap();
        let target = w.systems.iter().find(|s| s.id == bearing.target).unwrap().pos;
        assert!(bearing.holds(target), "the wedge holds its world");
        assert!(w.season.precursors.contains_key(&bearing.target));
    }

    #[test]
    fn arriving_with_a_bearing_finds_the_world_and_the_spy_brings_war() {
        let (mut w, a) = journey();
        attuned(&mut w, a);
        let target = w.season.bearings[0].target;
        let pos = w.systems.iter().find(|s| s.id == target).unwrap().pos;
        // A fleet at another sealed world without a bearing through it finds nothing.
        let scout = w.alloc_entity_id();
        w.fleets.insert(scout, Fleet::single(scout, a, ShipKind::Scout, pos, FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.discover_precursors(&mut events);
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::PrecursorFound { owner, system, .. } if owner == a && system == target)));
        assert!(w.precursor_guarded(target), "its guardians wake");
        assert!(w.season.first_found_at.is_some(), "the race begins");
        let leak = w.season.leaks[0].clone();
        assert!((leak.at - w.time - crate::season::LEAK_S).abs() < 1e-6, "the Authority will know in two hours");
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SpyLeak { owner, .. } if owner == a)));
        // Before the leak lands, the Hub still trades.
        w.time = leak.at + 1.0;
        let mut events = Vec::new();
        w.tick_leaks(&mut events);
        assert!(w.is_rebel(a));
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::AuthorityWar { owner } if owner == a)));
        assert!(w.enforcement_due(a), "squadrons sail at rebels");
        let refused = sell(&mut w, a, Commodity::Alloys, 10);
        assert!(refused.iter().any(|e| matches!(e.payload,
            EventPayload::Trade(TradeEvent::Rejected { reason: TradeRejectReason::CharterRevoked, .. }))), "no Hub market at war");
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::War && n.headline.contains("declares war")));
    }

    #[test]
    fn guardians_hold_the_world_until_cleared_and_landing_is_war() {
        let (mut w, a) = journey();
        attuned(&mut w, a);
        let target = w.season.bearings[0].target;
        let pos = w.systems.iter().find(|s| s.id == target).unwrap().pos;
        let scout = w.alloc_entity_id();
        w.fleets.insert(scout, Fleet::single(scout, a, ShipKind::Scout, pos, FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.discover_precursors(&mut events);
        w.fleets.remove(&scout);
        let colony = w.alloc_entity_id();
        w.fleets.insert(colony, Fleet::single(colony, a, ShipKind::Colony, pos, FleetOrder::Idle, None));
        let mut events = Vec::new();
        w.resolve_colony_arrivals(&mut events);
        assert_ne!(w.systems.iter().find(|s| s.id == target).unwrap().owner, Some(a), "the guardians hold it");
        let guard = w.season.precursors[&target].guard.unwrap();
        w.fleets.remove(&guard);
        w.resolve_colony_arrivals(&mut events);
        assert_eq!(w.systems.iter().find(|s| s.id == target).unwrap().owner, Some(a), "cleared, the colony lands");
        assert!(w.season.precursors[&target].landed.is_some_and(|(who, _)| who == a));
        assert!(w.is_rebel(a), "landing on a sealed world is war at once");
    }

    #[test]
    fn the_authority_pays_bounties_on_rebel_hulls() {
        let (mut w, a) = levied();
        found(&mut w, &[(2, "Beta")]);
        let b = PlayerId(2);
        let mut events = Vec::new();
        w.declare_rebel(a, &mut events);
        let owed = |w: &World| w.players[&b].credits + w.players[&b].incoming();
        let before = owed(&w);
        w.pay_bounties([a, b], [3, 1], None);
        assert!((owed(&w) - before - 3.0 * crate::season::BOUNTY_PER_HULL).abs() < 1e-6, "three rebel hulls, three bounties");
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Wanted && n.headline.contains("3 insurgent hulls")));
        let rebel_before = w.players[&a].credits + w.players[&a].incoming();
        w.pay_bounties([b, a], [2, 0], None);
        assert_eq!(w.players[&a].credits + w.players[&a].incoming(), rebel_before, "no bounty for a rebel");
    }



    /// Alpha holds a found sealed world (its Keystone at `level`); returns it.
    fn keystone_world(w: &mut World, a: PlayerId, level: u32) -> EntityId {
        w.place_precursors();
        let (system, _) = w.season.precursors.iter().next().map(|(id, p)| (*id, p.clone())).unwrap();
        give(w, system, a);
        let p = w.season.precursors.get_mut(&system).unwrap();
        p.found.push((a, w.time));
        p.landed = Some((a, w.time));
        if level > 0 {
            p.keystone = Some(Keystone { level, ..Default::default() });
        }
        system
    }

    fn stock(w: &mut World, system: EntityId, commodity: Commodity, units: f64) {
        let sys = w.systems.iter_mut().find(|s| s.id == system).unwrap();
        sys.stockpile.insert(commodity, units);
    }

    #[test]
    fn feeding_a_keystone_raises_it_and_is_war() {
        let (mut w, a) = levied();
        let system = keystone_world(&mut w, a, 0);
        let basket = crate::season::keystone_basket(1, w.keystone_scale());
        let mut events = Vec::new();
        for (c, n) in &basket {
            stock(&mut w, system, *c, *n + 5.0);
            w.feed_keystone(a, system, *c, *n as u32 + 100, &mut events);
        }
        assert!(w.is_rebel(a), "feeding a Keystone is war at once");
        let k = w.season.precursors[&system].keystone.clone().unwrap();
        assert_eq!(k.level, 1);
        assert!(k.paid.is_empty(), "the next level starts afresh");
        for (c, _) in &basket {
            assert!((w.systems.iter().find(|s| s.id == system).unwrap().stockpile[c] - 5.0).abs() < 1e-6, "only what it needed");
        }
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::KeystoneLevel { level: 1, fell: false, .. })));
        assert!(w.season.news.iter().any(|n| n.kind == NewsKind::Keystone && n.headline.contains("level 1")));
    }

    #[test]
    fn upper_levels_need_relics_and_level_three_brings_a_crackdown() {
        let (mut w, a) = levied();
        let system = keystone_world(&mut w, a, 5);
        let basket = crate::season::keystone_basket(6, w.keystone_scale());
        let mut events = Vec::new();
        for (c, n) in &basket {
            stock(&mut w, system, *c, *n);
            w.feed_keystone(a, system, *c, *n as u32, &mut events);
        }
        assert_eq!(w.season.precursors[&system].keystone.as_ref().unwrap().level, 5, "level 6 waits for a relic");
        let mut ev = Vec::new();
        w.tick_relics(&mut ev);
        let relic = *w.season.relics.keys().next().unwrap();
        w.season.relics.get_mut(&relic).unwrap().awake_at = Some(w.time);
        give(&mut w, relic, a);
        w.advance_keystone(system, &mut events);
        assert_eq!(w.season.precursors[&system].keystone.as_ref().unwrap().level, 6, "a relic held, it rises");
        assert!(w.season.crackdowns.iter().any(|c| c.kind == CrackdownKind::Keystone && c.target == Some(a)), "level 6 brings a crackdown");
        assert!(w.enforcement_due(a));
    }

    #[test]
    fn a_completed_keystone_wakes_its_powers_and_the_seal_fires_after_48h() {
        let (mut w, a) = levied();
        let system = keystone_world(&mut w, a, 9);
        let mut events = Vec::new();
        w.tick_relics(&mut events);
        for relic in w.season.relics.keys().copied().take(5).collect::<Vec<_>>() {
            w.season.relics.get_mut(&relic).unwrap().awake_at = Some(w.time);
            give(&mut w, relic, a);
        }
        let basket = crate::season::keystone_basket(10, w.keystone_scale());
        for (c, n) in &basket {
            stock(&mut w, system, *c, *n);
            w.feed_keystone(a, system, *c, *n as u32, &mut events);
        }
        let k = w.season.precursors[&system].keystone.clone().unwrap();
        assert_eq!(k.level, 10);
        assert!(w.keystone_powers(a));
        assert_eq!(w.relay_factor(a, Vec2::new(1.0e6, 0.0)), 0.0, "Instant Command");
        let fleet = w.fleets.get(&k.fleet.unwrap()).expect("the Precursor Fleet");
        assert!(fleet.contains(ShipKind::Titan) && fleet.owner == a);
        // Gate Step: from the Keystone world to a held relic, at once.
        let relic = w.relics_held(a)[0];
        let to = w.systems.iter().find(|s| s.id == relic).unwrap().pos;
        let fid = k.fleet.unwrap();
        w.gate_step(a, fid, relic, &mut events);
        assert!(w.fleets[&fid].pos.distance(to) < 1.0, "stepped");
        // Held 48 hours: the Seal fires.
        w.time += crate::season::CHARGE_S + 1.0;
        let mut events = Vec::new();
        w.tick_keystones(&mut events);
        assert!(matches!(w.season.ended, Some(Ending::Sovereign { holder, .. }) if holder == a));
        assert!(events.iter().any(|e| matches!(e.payload, EventPayload::SeasonEnded { .. })));
        assert!(!w.keystone_powers(a), "the season is over");
    }

    #[test]
    fn a_held_siege_knocks_the_keystone_down_and_scores_its_stopper() {
        let (mut w, a) = levied();
        found(&mut w, &[(2, "Beta")]);
        let b = PlayerId(2);
        let system = keystone_world(&mut w, a, 10);
        w.season.precursors.get_mut(&system).unwrap().keystone.as_mut().unwrap().charge_since = Some(w.time);
        let sys = w.systems.iter_mut().find(|s| s.id == system).unwrap();
        sys.blockade = Some(crate::galaxy::Blockade { by: b, since: 0.0, held_since: Some(0.0), siege_since: None, anchored: false, catch: 0.0, catch_stabilized: 0.0 });
        let mut events = Vec::new();
        w.tick_keystones(&mut events);
        w.time += crate::season::KEYSTONE_SIEGE_TICK_S + 1.0;
        w.tick_keystones(&mut events);
        let k = w.season.precursors[&system].keystone.clone().unwrap();
        assert_eq!(k.level, 9, "a siege tick knocks a level off");
        assert!(k.charge_since.is_none(), "the charge stops");
        assert!(!w.keystone_powers(a), "and the powers with it");
        assert_eq!(w.season.stoppers[&b], crate::season::STOP_LEVEL);
    }

    #[test]
    fn the_charter_holds_when_the_race_runs_out() {
        let (mut w, a) = levied();
        found(&mut w, &[(2, "Beta")]);
        let b = PlayerId(2);
        rebel(&mut w, b, 0.0);
        w.players.get_mut(&b).unwrap().last_active = w.time;
        w.season.stoppers.insert(a, 20.0);
        w.season.first_found_at = Some(w.time);
        w.time += crate::season::RACE_S + 1.0;
        w.players.get_mut(&b).unwrap().last_active = w.time;
        let mut events = Vec::new();
        w.tick_keystones(&mut events);
        let Some(Ending::CharterHolds { winners, .. }) = w.season.ended.clone() else { panic!("{:?}", w.season.ended) };
        assert_eq!(winners[0], a, "the top stopper first");
        assert!(winners.contains(&b), "and every active loyalist");
        // Registration closed with the first find.
        found(&mut w, &[(9, "Late")]);
        assert!(!w.players.contains_key(&PlayerId(9)));
    }

    #[test]
    fn the_backstop_reseals_the_worlds_after_twelve_weeks() {
        let (mut w, _) = levied();
        let start = w.season.started_at.unwrap();
        w.time = start + crate::season::BACKSTOP_S + 1.0;
        let mut events = Vec::new();
        w.tick_keystones(&mut events);
        assert!(matches!(w.season.ended, Some(Ending::CharterHolds { .. })));
        assert!(w.season.news.iter().any(|n| n.headline.contains("reseals")));
    }

}
