//! Snapshot-only persistence: physics, delayed light and pending commands are
//! captured together. A hard crash intentionally loses progress since the last
//! successful save; there is no command journal or simulation replay on startup.

use super::*;
use crate::persistence::Storage;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct GalaxyCheckpoint {
    pub world: World,
    reporting: ReportingCheckpoint,
    pending: Vec<Command>,
}

impl GameLoop {
    fn durable_checkpoint(&self) -> GalaxyCheckpoint {
        // Only this consistent clone runs on the game task, once per 15 minutes.
        // Serialization, compression, checksumming and all disk I/O run off it.
        GalaxyCheckpoint {
            world: self.world.clone(),
            reporting: ReportingCheckpoint {
                galaxy_instance_id: Some(self.galaxy_instance_id.clone()),
                history: self.history.clone(),
                prices: self.prices.clone(),
                market_accounts: self.market_accounts.clone(),
                trade_reports: self.trade_reports.clone(),
                reports: self.reports.clone(),
                timeline: self.timeline.clone(),
                concluded_battles: self.concluded_battles.clone(),
                observed_order_plans: self
                    .observed_order_plans
                    .iter()
                    .map(|((player, id), plan)| (*player, *id, plan.clone()))
                    .collect(),
            },
            pending: self.pending.clone(),
        }
    }

    fn restore(
        saved: GalaxyCheckpoint,
        pacing_scale: f64,
        status: watch::Sender<ServerStatus>,
        estimates: mpsc::UnboundedSender<ConnId>,
    ) -> Self {
        // Sockets and their delivery bookkeeping aren't galaxy state. Start
        // with no sessions and use this process's configured pacing. Older
        // snapshots' session/pacing fields are ignored during deserialization.
        let mut game = Self::new(saved.world, pacing_scale, status, estimates);
        let r = saved.reporting;
        game.galaxy_instance_id = r
            .galaxy_instance_id
            .expect("validated checkpoint namespace");
        game.history = r.history;
        game.prices = r.prices;
        game.market_accounts = r.market_accounts;
        game.trade_reports = r.trade_reports;
        game.reports = r.reports;
        game.timeline = r.timeline;
        game.concluded_battles = r.concluded_battles;
        game.observed_order_plans = r
            .observed_order_plans
            .into_iter()
            .map(|(p, id, plan)| ((p, id), plan))
            .collect();
        game.pending = saved.pending;
        game
    }
}

impl GalaxyCheckpoint {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        let (status, _) = watch::channel(ServerStatus::default());
        let (estimates, _) = mpsc::unbounded_channel();
        GameLoop::new(
            World::new(sim::SimConfig::for_players(12345, 2)),
            1.0,
            status,
            estimates,
        )
        .durable_checkpoint()
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.reporting
                .galaxy_instance_id
                .as_ref()
                .is_some_and(|id| !id.is_empty()),
            "checkpoint lacks galaxy identity"
        );
        ensure!(self.world.time.is_finite(), "invalid checkpoint clock");
        Ok(())
    }
}

fn checkpoint_timer() -> tokio::time::Interval {
    let mut timer = tokio::time::interval_at(
        tokio::time::Instant::now() + CHECKPOINT_INTERVAL,
        CHECKPOINT_INTERVAL,
    );
    timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    timer
}

pub(crate) async fn run(
    initial_world: Option<World>,
    storage: Storage,
    checkpoint: Option<GalaxyCheckpoint>,
    pacing_scale: f64,
    status_tx: watch::Sender<ServerStatus>,
    mut rx: mpsc::UnboundedReceiver<GameInput>,
    mut shutdown: watch::Receiver<bool>,
    ready: oneshot::Sender<()>,
) -> Result<()> {
    let (estimate_done_tx, mut estimate_done_rx) = mpsc::unbounded_channel();
    let resuming = checkpoint.is_some();
    // Restore the saved instant without catch-up ticks or wall-time fast-forward.
    // Physics and the player's delayed picture roll back together after a crash.
    let mut game = tokio::task::spawn_blocking(move || -> Result<GameLoop> {
        if let Some(saved) = checkpoint {
            saved.validate()?;
            Ok(GameLoop::restore(
                saved,
                pacing_scale,
                status_tx,
                estimate_done_tx,
            ))
        } else {
            let world = initial_world.context("no galaxy or checkpoint supplied")?;
            // An unreadable legacy report history must not be replaced by
            // present-day truth when importing an old PostgreSQL world save.
            if let Some(saved) = &world.reporting_checkpoint {
                serde_json::from_str::<ReportingCheckpoint>(saved)
                    .context("legacy reporting checkpoint is unreadable")?;
            }
            Ok(GameLoop::new(
                world,
                pacing_scale,
                status_tx,
                estimate_done_tx,
            ))
        }
    })
    .await??;

    // Persist a new galaxy/import before accepting players. Loading an existing
    // save does not need another write and must not churn its backup generations.
    if !resuming {
        storage.checkpoint(game.durable_checkpoint()).await?;
    }
    game.publish_status();
    let _ = ready.send(());
    let mut ticker = interval(Duration::from_secs_f64(DT / pacing_scale));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut checkpoint_clock = checkpoint_timer();
    let mut saving: Option<tokio::task::JoinHandle<Result<()>>> = None;
    info!(
        pacing_scale,
        checkpoint_seconds = CHECKPOINT_INTERVAL.as_secs(),
        "authoritative game loop started"
    );

    loop {
        if *shutdown.borrow() {
            break;
        }
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            result = async { saving.as_mut().expect("save task").await }, if saving.is_some() => {
                saving = None;
                result??;
            }
            _ = checkpoint_clock.tick(), if saving.is_none() => {
                // The simulation continues in memory while this immutable copy
                // is compressed and published on a background thread.
                saving = Some(storage.start_checkpoint(game.durable_checkpoint()));
            }
            _ = ticker.tick() => game.tick(),
            input = rx.recv() => match input {
                Some(input) => game.handle_input(input),
                None => break,
            },
            Some(id) = estimate_done_rx.recv() => { game.estimate_inflight.remove(&id); }
        }
    }

    // Close intake and fold its finite backlog into the final snapshot. Pending
    // commands are saved without forcing another tick or delivering orders early.
    rx.close();
    while let Some(input) = rx.recv().await {
        game.handle_input(input);
    }
    if let Some(task) = saving {
        task.await??;
    }
    game.sessions = Sessions::new();
    storage.checkpoint(game.durable_checkpoint()).await?;
    info!(
        tick = game.world.tick,
        "galaxy saved; authoritative game loop stopped"
    );
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
