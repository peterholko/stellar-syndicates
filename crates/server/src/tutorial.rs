//! How far each player got in the founding charter (the tutorial), and where
//! new players stop. The game loop's founding programme is the authority; this
//! module names its steps as the in-game guide does, records the first time each
//! account reached each one, and turns the records into a funnel.
//!
//! Records live in the accounts database, keyed by galaxy, so they outlive a
//! reset. A poller writes them every few seconds whether or not anyone watches.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sim::founding::{FoundingProgram, FoundingStage as S};
use sim::{PlayerId, World};
use sqlx::PgPool;
use uuid::Uuid;

use crate::session::{AdminSnapshot, GameHandle};

/// How often the poller asks the game loop for progress.
const POLL_EVERY: Duration = Duration::from_secs(15);
/// Not seen for this long and not online: the player has gone quiet.
pub const QUIET_AFTER_HOURS: i64 = 24;

/// The charter in the order the guide teaches it, named as players read it.
/// Steps of one chapter can be done out of order; progress is the first step
/// not yet done, as the guide's next action is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    Founded,
    Mine,
    Shipment,
    Privateer,
    Shipyard,
    Research,
    Survey,
    Barge,
    BeltMining,
    Haul,
}

impl Step {
    pub const ALL: [Step; 10] = [
        Step::Founded,
        Step::Mine,
        Step::Shipment,
        Step::Privateer,
        Step::Shipyard,
        Step::Research,
        Step::Survey,
        Step::Barge,
        Step::BeltMining,
        Step::Haul,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Step::Founded => "founded",
            Step::Mine => "mine",
            Step::Shipment => "shipment",
            Step::Privateer => "privateer",
            Step::Shipyard => "shipyard",
            Step::Research => "research",
            Step::Survey => "survey",
            Step::Barge => "barge",
            Step::BeltMining => "belt_mining",
            Step::Haul => "haul",
        }
    }

    /// Records written under earlier charters (beacon, cache, ...) name no
    /// current step and are skipped.
    pub fn from_key(key: &str) -> Option<Step> {
        Step::ALL.into_iter().find(|step| step.key() == key)
    }

    /// The guide's own checklist wording (client `foundingPlan`).
    pub fn label(self) -> &'static str {
        match self {
            Step::Founded => "Found a colony",
            Step::Mine => "Bring the mine online",
            Step::Shipment => "Send your first shipment",
            Step::Privateer => "Defeat the Rogue Privateer",
            Step::Shipyard => "Build a Shipyard",
            Step::Research => "Start research",
            Step::Survey => "Chart a nearby system",
            Step::Barge => "Build a Mining Barge",
            Step::BeltMining => "Mine the belt",
            Step::Haul => "Bring the ore home",
        }
    }

    pub fn chapter(self) -> &'static str {
        match self {
            Step::Founded => "Founding",
            Step::Mine | Step::Shipment | Step::Privateer => "Landfall",
            Step::Shipyard | Step::Research | Step::Survey => "Industry",
            Step::Barge | Step::BeltMining | Step::Haul => "Reach",
        }
    }

    /// The play-order rank of the founding stage that cannot be reached
    /// without this step, so a later stamp proves it even when its own fact
    /// was never recorded.
    fn gate(self) -> u8 {
        match self {
            Step::Founded => 0,
            Step::Mine => 1,
            Step::Shipment => 2,
            Step::Privateer => 3,
            Step::Shipyard => 4,
            Step::Research => 5,
            Step::Survey => 6,
            Step::Barge => 7,
            Step::BeltMining => 8,
            Step::Haul => 9,
        }
    }
}

/// Where a founding stage sits in play order. Retired stages fold onto the
/// step that replaced them.
fn rank(stage: S) -> u8 {
    match stage {
        S::BuildMine => 0,
        S::PrepareExport | S::LocalRecovery | S::ExportProduction | S::BuildConvoy => 1,
        S::DefeatPrivateer => 2,
        S::BuildShipyard | S::CompleteExport | S::GrowBusiness | S::BuildSecondFreighter => 3,
        S::FirstResearch | S::BuildAcademy => 4,
        S::SurveyCandidates | S::BuildScout => 5,
        S::BuildBarge => 6,
        S::MineBelt => 7,
        S::HaulOre => 8,
        S::BuildColony | S::EstablishColony | S::Complete => 9,
    }
}

/// One corporation's charter as the game loop sees it now.
#[derive(Debug, Clone)]
pub struct TutorialProgress {
    /// Steps reached, with the game time when the game stamped one.
    pub reached: Vec<(Step, Option<f64>)>,
}

/// Steps reached from the programme's own stamps and report arrivals.
pub fn reached_steps(founding: &FoundingProgram, now: f64) -> Vec<(Step, Option<f64>)> {
    let arrived = |at: Option<f64>| at.filter(|at| *at <= now);
    // The earliest stamp at or past a rank: the moment that rank was proved.
    let proved = |gate: u8| {
        founding
            .milestones
            .iter()
            .filter(|(stage, _)| rank(**stage) >= gate)
            .map(|(_, at)| *at)
            .reduce(f64::min)
    };
    let complete = founding.stage == S::Complete;
    Step::ALL
        .into_iter()
        .filter_map(|step| {
            let own = match step {
                Step::Founded => Some(founding.started_at),
                Step::Shipment => arrived(founding.export_dispatched_at),
                Step::Privateer => arrived(founding.privateer_report_at),
                Step::Haul => arrived(founding.haul_report_at),
                _ => None,
            };
            let gate = proved(step.gate());
            let at = match (own, gate) {
                (Some(own), Some(gate)) => Some(own.min(gate)),
                (own, gate) => own.or(gate),
            };
            (own.is_some() || gate.is_some() || complete).then_some((step, at))
        })
        .collect()
}

/// The programme of `owner` in `world`, or `None` when it has no tutorial.
pub fn progress(world: &World, owner: PlayerId) -> Option<TutorialProgress> {
    let founding = &world.players.get(&owner)?.founding;
    if !founding.enabled {
        return None;
    }
    Some(TutorialProgress {
        reached: reached_steps(founding, world.time),
    })
}

/// The wall-clock moment a game time corresponds to, counted back from now at
/// the server's pacing. Time the server was down is not in the game clock, so
/// a step stamped before a restart reads that much later than it happened.
fn wall_time(now: DateTime<Utc>, snapshot: &AdminSnapshot, at: Option<f64>) -> DateTime<Utc> {
    let Some(at) = at else { return now };
    let behind = ((snapshot.sim_time - at) / snapshot.pacing.max(1e-6)).max(0.0);
    now - chrono::Duration::milliseconds((behind * 1000.0) as i64)
}

struct Inner {
    pool: PgPool,
    /// (galaxy, player, step) already written by this process.
    written: Mutex<HashSet<(String, u64, Step)>>,
    galaxies: Mutex<HashSet<String>>,
}

/// Writes newly reached steps. Shared by the poller and the dashboard, so a
/// page load is never older than the game.
#[derive(Clone)]
pub struct TutorialRecorder(Arc<Inner>);

impl TutorialRecorder {
    pub fn new(pool: PgPool) -> Self {
        Self(Arc::new(Inner {
            pool,
            written: Mutex::default(),
            galaxies: Mutex::default(),
        }))
    }

    pub fn spawn_poller(&self, game: GameHandle) {
        let recorder = self.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(POLL_EVERY);
            every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                every.tick().await;
                if let Some(snapshot) = game.admin_snapshot().await {
                    recorder.record(&snapshot).await;
                }
            }
        });
    }

    /// Write every step first reached since the last call. Steps of bots and
    /// of corporations without an account are skipped by the account join.
    pub async fn record(&self, snapshot: &AdminSnapshot) {
        let now = Utc::now();
        let new_galaxy = self
            .0
            .galaxies
            .lock()
            .is_ok_and(|known| !known.contains(&snapshot.galaxy_id));
        if new_galaxy {
            let started = wall_time(now, snapshot, Some(0.0));
            if sqlx::query("INSERT INTO galaxies (id, started_at) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING")
                .bind(&snapshot.galaxy_id).bind(started).execute(&self.0.pool).await.is_ok()
                && let Ok(mut known) = self.0.galaxies.lock()
            {
                known.insert(snapshot.galaxy_id.clone());
            }
        }
        let fresh: Vec<(u64, Step, Option<f64>)> = {
            let Ok(written) = self.0.written.lock() else { return };
            snapshot
                .corporations
                .iter()
                .filter(|corp| !corp.bot)
                .filter_map(|corp| corp.tutorial.as_ref().map(|t| (corp.player_id, t)))
                .flat_map(|(player, tutorial)| {
                    tutorial.reached.iter().map(move |(step, at)| (player.0, *step, *at))
                })
                .filter(|(player, step, _)| {
                    !written.contains(&(snapshot.galaxy_id.clone(), *player, *step))
                })
                .collect()
        };
        if fresh.is_empty() {
            return;
        }
        let players: Vec<i64> = fresh.iter().map(|(player, _, _)| *player as i64).collect();
        let steps: Vec<&str> = fresh.iter().map(|(_, step, _)| step.key()).collect();
        let reached: Vec<DateTime<Utc>> = fresh.iter().map(|(_, _, at)| wall_time(now, snapshot, *at)).collect();
        let sim: Vec<Option<f64>> = fresh.iter().map(|(_, _, at)| *at).collect();
        let written = sqlx::query("INSERT INTO tutorial_progress (account_id, galaxy_id, step, reached_at, sim_time) SELECT a.id, $1, s.step, s.reached_at, s.sim_time FROM UNNEST($2::int8[], $3::text[], $4::timestamptz[], $5::float8[]) AS s(player_id, step, reached_at, sim_time) JOIN accounts a ON a.player_id = s.player_id ON CONFLICT (account_id, galaxy_id, step) DO NOTHING")
            .bind(&snapshot.galaxy_id).bind(&players).bind(&steps).bind(&reached).bind(&sim)
            .execute(&self.0.pool).await;
        match written {
            Ok(_) => {
                if let Ok(mut done) = self.0.written.lock() {
                    done.extend(fresh.into_iter().map(|(player, step, _)| (snapshot.galaxy_id.clone(), player, step)));
                }
            }
            Err(_) => tracing::warn!("could not record tutorial progress"),
        }
    }
}

/// The galaxy the dashboard reports on: the running one, else the latest seen.
pub async fn galaxy(
    pool: &PgPool,
    running: Option<&str>,
) -> Result<Option<(String, DateTime<Utc>)>, sqlx::Error> {
    sqlx::query_as("SELECT id, started_at FROM galaxies WHERE ($1::text IS NULL OR id = $1) ORDER BY started_at DESC LIMIT 1")
        .bind(running)
        .fetch_optional(pool)
        .await
}

/// One account's charter in the reported galaxy.
#[derive(Debug, Clone, Default)]
pub struct AccountTutorial {
    pub signed_up: Option<DateTime<Utc>>,
    /// First game session of this galaxy's era.
    pub entered: Option<DateTime<Utc>>,
    pub steps: BTreeMap<Step, DateTime<Utc>>,
}

/// Entries and recorded steps for every account, scoped to one galaxy.
pub async fn load(
    pool: &PgPool,
    galaxy: &str,
    since: DateTime<Utc>,
    account: Option<Uuid>,
) -> Result<HashMap<Uuid, AccountTutorial>, sqlx::Error> {
    let mut accounts: HashMap<Uuid, AccountTutorial> = HashMap::new();
    let entries: Vec<(Uuid, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT a.id, a.created_at, (SELECT MIN(connected_at) FROM play_sessions p WHERE p.account_id = a.id AND p.connected_at >= $1) FROM accounts a WHERE ($2::uuid IS NULL OR a.id = $2)",
    )
    .bind(since)
    .bind(account)
    .fetch_all(pool)
    .await?;
    for (id, signed_up, entered) in entries {
        let entry = accounts.entry(id).or_default();
        entry.signed_up = Some(signed_up);
        entry.entered = entered;
    }
    let steps: Vec<(Uuid, String, DateTime<Utc>)> = sqlx::query_as(
        "SELECT account_id, step, reached_at FROM tutorial_progress WHERE galaxy_id = $1 AND ($2::uuid IS NULL OR account_id = $2)",
    )
    .bind(galaxy)
    .bind(account)
    .fetch_all(pool)
    .await?;
    for (id, key, at) in steps {
        if let Some(step) = Step::from_key(&key) {
            accounts.entry(id).or_default().steps.insert(step, at);
        }
    }
    Ok(accounts)
}

/// Funnel stages: signing up and entering the game, then the charter steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    SignedUp,
    Entered,
    Step(Step),
}

impl Stage {
    pub fn all() -> impl Iterator<Item = Stage> {
        [Stage::SignedUp, Stage::Entered]
            .into_iter()
            .chain(Step::ALL.into_iter().map(Stage::Step))
    }

    pub fn key(self) -> &'static str {
        match self {
            Stage::SignedUp => "signed_up",
            Stage::Entered => "entered",
            Stage::Step(step) => step.key(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Stage::SignedUp => "Sign up",
            Stage::Entered => "Enter the game",
            Stage::Step(step) => step.label(),
        }
    }

    pub fn chapter(self) -> &'static str {
        match self {
            Stage::SignedUp | Stage::Entered => "Arrival",
            Stage::Step(step) => step.chapter(),
        }
    }
}

impl AccountTutorial {
    /// When each funnel stage was reached, in funnel order.
    pub fn times(&self) -> Vec<Option<DateTime<Utc>>> {
        Stage::all()
            .map(|stage| match stage {
                Stage::SignedUp => self.signed_up,
                Stage::Entered => self.entered.or_else(|| {
                    // A recorded step proves entry even if its session predates
                    // the galaxy-era cut-off.
                    self.steps.values().min().copied()
                }),
                Stage::Step(step) => self.steps.get(&step).copied(),
            })
            .collect()
    }

    /// The first stage not yet reached, `None` when the charter is complete.
    pub fn next(&self) -> Option<Stage> {
        Stage::all().zip(self.times()).find(|(_, at)| at.is_none()).map(|(stage, _)| stage)
    }
}

/// The player-facing summary on an account's dashboard row.
#[derive(Debug, Clone, Serialize)]
pub struct PlayerTutorial {
    /// The step the player is on (`None` once the charter is fulfilled).
    pub next: Option<&'static str>,
    pub next_label: Option<&'static str>,
    /// Charter steps done, out of `total`.
    pub done: usize,
    pub total: usize,
    /// Since when the player has been on `next`.
    pub since: Option<DateTime<Utc>>,
    /// Every stage with its time; only the player's own page carries them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<StepTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StepTime {
    pub key: &'static str,
    pub label: &'static str,
    pub chapter: &'static str,
    pub at: Option<DateTime<Utc>>,
}

pub fn summary(account: &AccountTutorial, with_steps: bool) -> PlayerTutorial {
    let times = account.times();
    let next = account.next();
    // Waiting since the last progress of any kind, so a step done out of
    // order does not reset the clock on the one still missing.
    let since = next.and_then(|_| times.iter().flatten().max().copied());
    PlayerTutorial {
        next: next.map(Stage::key),
        next_label: next.map(Stage::label),
        done: account.steps.len(),
        total: Step::ALL.len(),
        since,
        steps: if with_steps {
            Stage::all()
                .zip(times)
                .map(|(stage, at)| StepTime {
                    key: stage.key(),
                    label: stage.label(),
                    chapter: stage.chapter(),
                    at,
                })
                .collect()
        } else {
            Vec::new()
        },
    }
}

/// One funnel row.
#[derive(Debug, Clone, Serialize)]
pub struct FunnelStage {
    pub key: &'static str,
    pub label: &'static str,
    pub chapter: &'static str,
    /// Accounts that reached this stage.
    pub reached: usize,
    /// Median wait from the previous stage, among accounts that reached both.
    pub median_seconds: Option<f64>,
    /// Accounts whose next stage is this one: still playing, or gone quiet.
    pub waiting_active: usize,
    pub waiting_quiet: usize,
}

/// One account in the funnel population, with whether it is still around.
pub struct FunnelAccount<'a> {
    pub tutorial: &'a AccountTutorial,
    pub active: bool,
}

pub fn funnel(accounts: &[FunnelAccount<'_>]) -> Vec<FunnelStage> {
    let rows: Vec<(Vec<Option<DateTime<Utc>>>, Option<Stage>, bool)> = accounts
        .iter()
        .map(|account| (account.tutorial.times(), account.tutorial.next(), account.active))
        .collect();
    Stage::all()
        .enumerate()
        .map(|(index, stage)| {
            let mut waits: Vec<f64> = rows
                .iter()
                .filter(|_| index > 0)
                .filter_map(|(times, _, _)| Some((times[index - 1]?, times[index]?)))
                .map(|(from, to)| (to - from).num_milliseconds() as f64 / 1000.0)
                .filter(|seconds| *seconds >= 0.0)
                .collect();
            waits.sort_by(f64::total_cmp);
            let median_seconds = (!waits.is_empty()).then(|| {
                let mid = waits.len() / 2;
                if waits.len() % 2 == 0 { (waits[mid - 1] + waits[mid]) / 2.0 } else { waits[mid] }
            });
            let waiting = |active: bool| {
                rows.iter()
                    .filter(|(_, next, is_active)| *next == Some(stage) && *is_active == active)
                    .count()
            };
            FunnelStage {
                key: stage.key(),
                label: stage.label(),
                chapter: stage.chapter(),
                reached: rows.iter().filter(|(times, _, _)| times[index].is_some()).count(),
                median_seconds,
                waiting_active: waiting(true),
                waiting_quiet: waiting(false),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
