//! Operator audit trail for the admin dashboard: who connected from where, for
//! how long, and every command they sent. Gameplay never reads any of it.
//!
//! One background task owns every write, so a slow or absent database never
//! delays a socket or a tick: sockets hand events over a bounded queue and drop
//! them when it is full. Commands are batched every few seconds. A hard stop
//! leaves sessions open; the next start closes them at their last heartbeat.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::http::{HeaderMap, HeaderName, header};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, QueryBuilder};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{MissedTickBehavior, interval};
use uuid::Uuid;

use crate::protocol::ClientMsg;

const QUEUE: usize = 8192;
const FLUSH_EVERY: Duration = Duration::from_secs(5);
const HEARTBEAT_EVERY: Duration = Duration::from_secs(60);
/// Commands per INSERT; five binds each stays far below Postgres's limit.
const BATCH_ROWS: usize = 1000;
/// A recorded message larger than this keeps its kind but not its body.
const DETAIL_LIMIT: usize = 4096;
/// Active time counts the five-minute windows holding at least one command.
pub const ACTIVE_WINDOW_S: i64 = 300;

/// Who is on the other end of a request, as far as the server can tell.
#[derive(Clone, Debug, Serialize)]
pub struct ClientInfo {
    pub ip: IpAddr,
    pub user_agent: Option<String>,
    pub language: Option<String>,
    /// Random per-browser id from the device cookie (see `AuthConfig::device`).
    pub device_id: Option<String>,
}

impl ClientInfo {
    pub fn new(peer: IpAddr, headers: &HeaderMap, device_id: Option<String>) -> Self {
        Self {
            ip: client_ip(peer, headers),
            user_agent: header_text(headers, &header::USER_AGENT, 512),
            language: header_text(headers, &header::ACCEPT_LANGUAGE, 128),
            device_id,
        }
    }
}

/// The public site reaches this server through a Cloudflare tunnel whose
/// connector runs on this machine, so every visitor arrives from loopback and
/// Cloudflare names the visitor in `CF-Connecting-IP`, which its edge always
/// overwrites. Only a loopback peer may assert another address: a request that
/// reaches the port directly is recorded as itself. Rate limiting keeps using
/// the peer address; this is for the audit trail.
pub fn client_ip(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    if peer.is_loopback()
        && let Some(ip) = headers
            .get("cf-connecting-ip")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok())
    {
        return ip;
    }
    peer
}

fn header_text(headers: &HeaderMap, name: &HeaderName, max: usize) -> Option<String> {
    let text: String = headers
        .get(name)?
        .to_str()
        .ok()?
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect();
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// The kind and body of a command worth recording, or `None` for traffic that
/// is not a player decision (readiness, keepalive, estimates, ledger reads).
pub fn action_record(msg: &ClientMsg) -> Option<(String, Option<serde_json::Value>)> {
    if matches!(
        msg,
        ClientMsg::Join { .. }
            | ClientMsg::Ping
            | ClientMsg::EstimateEngagement { .. }
            | ClientMsg::RequestTransactions { .. }
    ) {
        return None;
    }
    let mut value = serde_json::to_value(msg).ok()?;
    let kind = value
        .as_object_mut()
        .and_then(|fields| fields.remove("type"))
        .and_then(|kind| kind.as_str().map(str::to_owned))?;
    let detail = match &value {
        serde_json::Value::Object(fields) if fields.is_empty() => None,
        _ if value.to_string().len() > DETAIL_LIMIT => {
            Some(serde_json::json!({ "omitted": "message too large to record" }))
        }
        _ => Some(value),
    };
    Some((kind, detail))
}

/// A socket that is in the game right now. Kept in memory for "online now".
#[derive(Clone, Debug, Serialize)]
pub struct LivePlay {
    pub session_id: Uuid,
    pub account_id: Uuid,
    pub connected_at: DateTime<Utc>,
    pub client: ClientInfo,
    pub actions: u64,
    pub last_action_at: Option<DateTime<Utc>>,
}

struct ActionRow {
    session: Uuid,
    account: Uuid,
    at: DateTime<Utc>,
    kind: String,
    detail: Option<serde_json::Value>,
}

enum Event {
    Start(Box<LivePlay>),
    Action(ActionRow),
    End { session: Uuid, at: DateTime<Utc> },
    Shutdown(oneshot::Sender<()>),
}

struct Inner {
    tx: mpsc::Sender<Event>,
    live: Mutex<HashMap<Uuid, LivePlay>>,
}

#[derive(Clone)]
pub struct Activity(Arc<Inner>);

impl Activity {
    /// Close what a hard stop left open, then start the writer.
    pub async fn start(pool: PgPool) -> anyhow::Result<Self> {
        sqlx::query(
            "UPDATE play_sessions SET disconnected_at = last_seen_at WHERE disconnected_at IS NULL",
        )
        .execute(&pool)
        .await?;
        let (tx, rx) = mpsc::channel(QUEUE);
        tokio::spawn(Writer::new(pool).run(rx));
        Ok(Self(Arc::new(Inner {
            tx,
            live: Mutex::default(),
        })))
    }

    fn send(&self, event: Event) {
        if self.0.tx.try_send(event).is_err() {
            tracing::debug!("activity queue full; dropping an audit event");
        }
    }

    /// A socket joined the game. Dropping the returned [`Play`] ends it.
    pub fn begin(&self, account_id: Uuid, client: ClientInfo) -> Play {
        let play = LivePlay {
            session_id: Uuid::new_v4(),
            account_id,
            connected_at: Utc::now(),
            client,
            actions: 0,
            last_action_at: None,
        };
        if let Ok(mut live) = self.0.live.lock() {
            live.insert(play.session_id, play.clone());
        }
        let session = play.session_id;
        self.send(Event::Start(Box::new(play)));
        Play {
            activity: self.clone(),
            session,
            account: account_id,
        }
    }

    /// Every socket in the game right now.
    pub fn live(&self) -> Vec<LivePlay> {
        self.0
            .live
            .lock()
            .map(|live| live.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Write what is queued and close every open session. Bounded by the caller.
    pub async fn shutdown(&self) {
        let (done, wait) = oneshot::channel();
        if self.0.tx.send(Event::Shutdown(done)).await.is_ok() {
            let _ = wait.await;
        }
    }
}

/// One socket's stay in the game, from Join until it is dropped.
pub struct Play {
    activity: Activity,
    session: Uuid,
    account: Uuid,
}

impl Play {
    pub fn record(&self, msg: &ClientMsg) {
        let Some((kind, detail)) = action_record(msg) else {
            return;
        };
        let at = Utc::now();
        if let Ok(mut live) = self.activity.0.live.lock()
            && let Some(play) = live.get_mut(&self.session)
        {
            play.actions += 1;
            play.last_action_at = Some(at);
        }
        self.activity.send(Event::Action(ActionRow {
            session: self.session,
            account: self.account,
            at,
            kind,
            detail,
        }));
    }
}

impl Drop for Play {
    fn drop(&mut self) {
        if let Ok(mut live) = self.activity.0.live.lock() {
            live.remove(&self.session);
        }
        self.activity.send(Event::End {
            session: self.session,
            at: Utc::now(),
        });
    }
}

/// What one open session owes the database since the last flush.
#[derive(Default)]
struct Tally {
    actions: i32,
    active_seconds: i32,
    last_at: Option<DateTime<Utc>>,
    last_window: Option<i64>,
}

impl Tally {
    fn count(&mut self, at: DateTime<Utc>) {
        self.actions += 1;
        let window = at.timestamp().div_euclid(ACTIVE_WINDOW_S);
        if self.last_window != Some(window) {
            self.last_window = Some(window);
            self.active_seconds += ACTIVE_WINDOW_S as i32;
        }
        self.last_at = Some(self.last_at.map_or(at, |last| last.max(at)));
    }

    fn pending(&self) -> bool {
        self.actions > 0
    }

    fn written(&mut self) {
        self.actions = 0;
        self.active_seconds = 0;
        self.last_at = None;
    }
}

struct Writer {
    pool: PgPool,
    actions: Vec<ActionRow>,
    open: HashMap<Uuid, Tally>,
}

impl Writer {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            actions: Vec::new(),
            open: HashMap::new(),
        }
    }

    async fn run(mut self, mut rx: mpsc::Receiver<Event>) {
        let mut flush = interval(FLUSH_EVERY);
        let mut heartbeat = interval(HEARTBEAT_EVERY);
        flush.set_missed_tick_behavior(MissedTickBehavior::Delay);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                event = rx.recv() => match event {
                    Some(Event::Start(play)) => self.start(*play).await,
                    Some(Event::Action(row)) => {
                        // A session whose start was never written cannot hold
                        // commands; keep it from failing everyone's batch.
                        if let Some(tally) = self.open.get_mut(&row.session) {
                            tally.count(row.at);
                            self.actions.push(row);
                            if self.actions.len() >= BATCH_ROWS {
                                self.flush().await;
                            }
                        }
                    }
                    Some(Event::End { session, at }) => self.end(session, at).await,
                    Some(Event::Shutdown(done)) => {
                        self.close_all().await;
                        let _ = done.send(());
                    }
                    None => {
                        self.close_all().await;
                        break;
                    }
                },
                _ = flush.tick() => self.flush().await,
                _ = heartbeat.tick() => self.heartbeat().await,
            }
        }
    }

    async fn start(&mut self, play: LivePlay) {
        let written = sqlx::query("INSERT INTO play_sessions (id,account_id,connected_at,last_seen_at,ip,user_agent,device_id) VALUES ($1,$2,$3,$3,$4,$5,$6)")
            .bind(play.session_id).bind(play.account_id).bind(play.connected_at)
            .bind(play.client.ip.to_string()).bind(&play.client.user_agent).bind(&play.client.device_id)
            .execute(&self.pool).await;
        match written {
            Ok(_) => {
                self.open.insert(play.session_id, Tally::default());
            }
            Err(_) => tracing::warn!("could not record a play session start"),
        }
    }

    async fn flush(&mut self) {
        if self.actions.is_empty() && !self.open.values().any(Tally::pending) {
            return;
        }
        let rows = std::mem::take(&mut self.actions);
        let mut ids = Vec::new();
        let mut counts = Vec::new();
        let mut active = Vec::new();
        let mut seen = Vec::new();
        for (id, tally) in self.open.iter_mut().filter(|(_, tally)| tally.pending()) {
            ids.push(*id);
            counts.push(tally.actions);
            active.push(tally.active_seconds);
            seen.push(tally.last_at.unwrap_or_else(Utc::now));
            tally.written();
        }
        let written: Result<(), sqlx::Error> = async {
            let mut tx = self.pool.begin().await?;
            for chunk in rows.chunks(BATCH_ROWS) {
                let mut insert = QueryBuilder::new(
                    "INSERT INTO player_actions (session_id,account_id,at,kind,detail) ",
                );
                insert.push_values(chunk, |mut row, action| {
                    row.push_bind(action.session)
                        .push_bind(action.account)
                        .push_bind(action.at)
                        .push_bind(&action.kind)
                        .push_bind(&action.detail);
                });
                insert.build().execute(&mut *tx).await?;
            }
            sqlx::query("UPDATE play_sessions p SET actions = p.actions + c.n, active_seconds = p.active_seconds + c.active, last_seen_at = GREATEST(p.last_seen_at, c.seen) FROM UNNEST($1::uuid[], $2::int4[], $3::int4[], $4::timestamptz[]) AS c(id, n, active, seen) WHERE p.id = c.id")
                .bind(&ids).bind(&counts).bind(&active).bind(&seen)
                .execute(&mut *tx).await?;
            tx.commit().await
        }
        .await;
        if written.is_err() {
            tracing::warn!(dropped = rows.len(), "could not record player commands");
        }
    }

    async fn heartbeat(&mut self) {
        if self.open.is_empty() {
            return;
        }
        let ids: Vec<Uuid> = self.open.keys().copied().collect();
        if sqlx::query("UPDATE play_sessions SET last_seen_at = GREATEST(last_seen_at, now()) WHERE id = ANY($1)")
            .bind(&ids).execute(&self.pool).await.is_err()
        {
            tracing::warn!("could not refresh open play sessions");
        }
    }

    async fn end(&mut self, session: Uuid, at: DateTime<Utc>) {
        if !self.open.contains_key(&session) {
            return;
        }
        // Its commands land before its close, so totals never trail the end.
        self.flush().await;
        self.open.remove(&session);
        if sqlx::query("UPDATE play_sessions SET disconnected_at = $2, last_seen_at = GREATEST(last_seen_at, $2) WHERE id = $1")
            .bind(session).bind(at).execute(&self.pool).await.is_err()
        {
            tracing::warn!("could not record a play session end");
        }
    }

    async fn close_all(&mut self) {
        self.flush().await;
        let ids: Vec<Uuid> = self.open.drain().map(|(id, _)| id).collect();
        if !ids.is_empty()
            && sqlx::query("UPDATE play_sessions SET disconnected_at = GREATEST(last_seen_at, now()), last_seen_at = GREATEST(last_seen_at, now()) WHERE id = ANY($1)")
                .bind(&ids).execute(&self.pool).await.is_err()
        {
            tracing::warn!("could not close open play sessions");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn only_a_loopback_peer_may_name_the_visitor() {
        let mut headers = HeaderMap::new();
        headers.insert("cf-connecting-ip", HeaderValue::from_static("203.0.113.9"));
        let tunnel: IpAddr = "127.0.0.1".parse().unwrap();
        let direct: IpAddr = "198.51.100.4".parse().unwrap();
        assert_eq!(client_ip(tunnel, &headers), "203.0.113.9".parse::<IpAddr>().unwrap());
        assert_eq!(client_ip(direct, &headers), direct, "a direct caller cannot claim another IP");
        headers.insert("cf-connecting-ip", HeaderValue::from_static("not an address"));
        assert_eq!(client_ip(tunnel, &headers), tunnel);
        assert_eq!(client_ip(tunnel, &HeaderMap::new()), tunnel);
    }

    #[test]
    fn client_text_is_trimmed_bounded_and_printable() {
        let mut headers = HeaderMap::new();
        headers.insert(header::USER_AGENT, HeaderValue::from_str(&format!("  Mozilla/5.0 {}", "x".repeat(900))).unwrap());
        headers.insert(header::ACCEPT_LANGUAGE, HeaderValue::from_static("   "));
        let client = ClientInfo::new("10.0.0.2".parse().unwrap(), &headers, Some("ab".repeat(16)));
        let agent = client.user_agent.unwrap();
        assert!(agent.starts_with("Mozilla/5.0"));
        assert!(agent.chars().count() <= 512);
        assert_eq!(client.language, None);
        assert_eq!(client.ip, "10.0.0.2".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn commands_are_recorded_by_kind_and_reads_are_not() {
        let (kind, detail) = action_record(&ClientMsg::HoldFleet { ship_id: sim::EntityId(7) }).unwrap();
        assert_eq!(kind, "HoldFleet");
        let detail = detail.unwrap();
        assert!(detail.get("type").is_none(), "the kind is not stored twice");
        assert!(detail.get("ship_id").is_some());
        assert!(action_record(&ClientMsg::Ping).is_none());
        assert!(action_record(&ClientMsg::RequestTransactions { before: None, request_id: 1 }).is_none());
        assert!(action_record(&ClientMsg::EstimateEngagement { attacker: sim::EntityId(1), target: sim::EntityId(2) }).is_none());
    }

    #[test]
    fn active_time_counts_each_five_minute_window_once() {
        let mut tally = Tally::default();
        let start = DateTime::from_timestamp(ACTIVE_WINDOW_S * 1000, 0).unwrap();
        for offset in [0, 10, 299] {
            tally.count(start + chrono::Duration::seconds(offset));
        }
        tally.count(start + chrono::Duration::seconds(300));
        tally.count(start + chrono::Duration::seconds(1800));
        assert_eq!(tally.actions, 5);
        assert_eq!(tally.active_seconds, 3 * ACTIVE_WINDOW_S as i32);
        assert_eq!(tally.last_at, Some(start + chrono::Duration::seconds(1800)));
        tally.written();
        assert!(!tally.pending());
        tally.count(start + chrono::Duration::seconds(1810));
        assert_eq!(tally.active_seconds, 0, "the window already counted is not counted again");
    }
}
