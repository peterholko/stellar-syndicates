//! §notifications (R8): opt-in alerts for a corporation that is away.
//!
//! The game loop buffers an alert when a timeline line of an alerting kind
//! reaches a player who is not connected (a colony under siege, a pirate raid,
//! research done, a warship launched, a Signal waiting, a fresh board, a
//! weekly medal). This task collects them every few seconds and, for each
//! account that turned that kind on, sends a web push to its subscribed
//! browsers. A daily email digest (the welcome-back report, as text) goes to
//! accounts that asked for one while they stay away.
//!
//! Delivery is off until the operator configures it: without `WEB_PUSH=on`
//! and a `VAPID_SUBJECT`, pushes are only logged; without `EMAIL_DIGEST_URL`,
//! digests are only logged. Preferences and subscriptions are kept either way.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64ct::{Base64UrlUnpadded, Encoding as _};
use serde::{Deserialize, Serialize};
use sim::PlayerId;
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;
use web_push_native::jwt_simple::algorithms::{ECDSAP256PublicKeyLike as _, ES256KeyPair};

use crate::auth::{AuthError, AuthStore};
use crate::session::GameHandle;

/// How often the task collects alerts from the game loop.
const POLL_EVERY: Duration = Duration::from_secs(5);
/// How often it looks for digests due.
const DIGEST_CHECK_EVERY: Duration = Duration::from_secs(600);
/// One push of a kind per account per this long; later ones fold in.
const PER_KIND_GAP: Duration = Duration::from_secs(15 * 60);
/// Pushes per account per day, all kinds.
const DAILY_CAP: u32 = 24;
/// Browsers one account may subscribe.
const DEVICES_MAX: i64 = 10;
/// A digest goes out at most this often per account.
const DIGEST_GAP_HOURS: i64 = 20;
/// A push stays deliverable for this long.
const PUSH_TTL: Duration = Duration::from_secs(12 * 3_600);

/// The kinds a player may turn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    Siege,
    Raid,
    Research,
    Warship,
    Signal,
    Board,
    Medal,
}

impl AlertKind {
    pub const ALL: [AlertKind; 7] = [Self::Siege, Self::Raid, Self::Research, Self::Warship, Self::Signal, Self::Board, Self::Medal];

    pub fn slug(self) -> &'static str {
        match self {
            Self::Siege => "siege",
            Self::Raid => "raid",
            Self::Research => "research",
            Self::Warship => "warship",
            Self::Signal => "signal",
            Self::Board => "board",
            Self::Medal => "medal",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.slug() == slug)
    }

    /// The notification's title.
    pub fn title(self) -> &'static str {
        match self {
            Self::Siege => "Colony under siege",
            Self::Raid => "Pirate raid",
            Self::Research => "Research done",
            Self::Warship => "Warship launched",
            Self::Signal => "Signal waiting",
            Self::Board => "Fresh contract board",
            Self::Medal => "Weekly medal",
        }
    }
}

/// The alert a timeline line for `recipient` raises, if any: a siege or raid
/// only for the side under attack.
pub fn alert_of(payload: &sim::EventPayload, recipient: PlayerId) -> Option<AlertKind> {
    use sim::EventPayload as E;
    Some(match payload {
        E::BlockadeContested { owner, .. } | E::BlockadeEstablished { owner, .. } if *owner == recipient => AlertKind::Siege,
        E::AssaultBegan { defender, .. } if *defender == recipient => AlertKind::Siege,
        E::SystemCaptured { old_owner, .. } if *old_owner == recipient => AlertKind::Siege,
        E::PirateRaidInbound { owner, .. } | E::FoundingRaided { owner, .. } | E::RaidSighted { owner, .. }
            if *owner == recipient => AlertKind::Raid,
        E::Vendetta { owner, beat: sim::founding::VendettaBeat::Arrival | sim::founding::VendettaBeat::MineStrike
            | sim::founding::VendettaBeat::BargeStrike | sim::founding::VendettaBeat::FreighterStrike
            | sim::founding::VendettaBeat::Fallen, .. } if *owner == recipient => AlertKind::Raid,
        E::ResearchCompleted { .. } => AlertKind::Research,
        E::ShipLaunched { kind, .. } if kind.is_combatant() => AlertKind::Warship,
        E::SignalDetected { .. } => AlertKind::Signal,
        E::ContractBoardPosted { .. } => AlertKind::Board,
        E::MedalWon { .. } | E::SyndicateMedalWon { .. } => AlertKind::Medal,
        _ => return None,
    })
}

/// One alert for an offline player, as its timeline line reads.
#[derive(Debug, Clone, PartialEq)]
pub struct Alert {
    pub player: PlayerId,
    pub kind: AlertKind,
    pub text: String,
}

/// The digest email for a report on return.
pub fn digest_text(report: &crate::welcome::WelcomeBackView) -> String {
    let hours = (report.away / 3_600.0).floor();
    let mut out = format!("Stellar Syndicates · away {hours:.0}h\n");
    let section = |out: &mut String, title: &str, lines: &mut dyn Iterator<Item = &str>| {
        let lines: Vec<&str> = lines.collect();
        if !lines.is_empty() {
            out.push_str(&format!("\n{title}\n"));
            for line in lines {
                out.push_str(&format!("- {line}\n"));
            }
        }
    };
    section(&mut out, "While you were away", &mut report.happened.iter().map(|l| l.text.as_str()));
    section(&mut out, "Waiting", &mut report.waiting.iter().map(|l| l.text.as_str()));
    section(&mut out, "Next", &mut report.next.iter().map(|a| a.label.as_str()));
    out
}

struct EmailConfig {
    url: String,
    token: Option<String>,
    from: String,
}

struct Config {
    /// `WEB_PUSH=on` with a `VAPID_SUBJECT` (a mailto: or https: contact).
    push_subject: Option<String>,
    email: Option<EmailConfig>,
}

impl Config {
    fn from_env() -> Self {
        let env = |key: &str| std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let push_on = env("WEB_PUSH").is_some_and(|v| v.eq_ignore_ascii_case("on"));
        let push_subject = env("VAPID_SUBJECT").filter(|s| s.starts_with("mailto:") || s.starts_with("https://"));
        if push_on && push_subject.is_none() {
            tracing::warn!("WEB_PUSH=on needs VAPID_SUBJECT (mailto: or https:); pushes stay log-only");
        }
        let email = env("EMAIL_DIGEST_URL").filter(|u| u.starts_with("https://")).map(|url| EmailConfig {
            url,
            token: env("EMAIL_DIGEST_TOKEN"),
            from: env("EMAIL_DIGEST_FROM").unwrap_or_else(|| "Stellar Syndicates <noreply@localhost>".into()),
        });
        Config { push_subject: push_subject.filter(|_| push_on), email }
    }
}

struct Inner {
    auth: AuthStore,
    pool: PgPool,
    config: Config,
    vapid: ES256KeyPair,
    /// The VAPID public key, uncompressed and base64url: the browsers'
    /// `applicationServerKey`.
    public_key: String,
    http: reqwest::Client,
    /// When each (account, kind) last pushed, and pushes today per account.
    sent: Mutex<HashMap<(Uuid, AlertKind), Instant>>,
    daily: Mutex<HashMap<Uuid, (chrono::NaiveDate, u32)>>,
}

/// The notifier: settings routes plus the delivery task.
#[derive(Clone)]
pub struct Notify(Arc<Inner>);

impl Notify {
    /// Load (or make, once) the VAPID key pair and start delivering.
    pub async fn start(auth: AuthStore, game: GameHandle) -> anyhow::Result<Self> {
        let pool = auth.pool();
        let vapid = vapid_key(&pool).await?;
        let public_key = Base64UrlUnpadded::encode_string(&vapid.public_key().public_key().to_bytes_uncompressed());
        let config = Config::from_env();
        tracing::info!(push = config.push_subject.is_some(), email = config.email.is_some(), "notifications ready");
        let notify = Notify(Arc::new(Inner {
            auth,
            pool,
            config,
            vapid,
            public_key,
            http: reqwest::Client::builder().timeout(Duration::from_secs(15)).build()?,
            sent: Mutex::new(HashMap::new()),
            daily: Mutex::new(HashMap::new()),
        }));
        let alerts = notify.clone();
        let poll_game = game.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(POLL_EVERY);
            loop {
                every.tick().await;
                for alert in poll_game.take_alerts().await {
                    if let Err(error) = alerts.deliver(&alert).await {
                        tracing::warn!(%error, kind = alert.kind.slug(), "alert not delivered");
                    }
                }
            }
        });
        let digests = notify.clone();
        tokio::spawn(async move {
            let mut every = tokio::time::interval(DIGEST_CHECK_EVERY);
            loop {
                every.tick().await;
                if let Err(error) = digests.send_digests(&game).await {
                    tracing::warn!(%error, "digest pass failed");
                }
            }
        });
        Ok(notify)
    }

    pub fn routes(self) -> Router {
        Router::new()
            .route("/api/notifications", get(get_prefs).post(set_prefs))
            .route("/api/notifications/push", post(add_push))
            .route("/api/notifications/push/remove", post(remove_push))
            .layer(DefaultBodyLimit::max(4096))
            .with_state(self)
    }

    /// Push `alert` to the account's browsers if it turned the kind on and the
    /// rate limits allow.
    async fn deliver(&self, alert: &Alert) -> anyhow::Result<()> {
        let row: Option<(Uuid, Vec<String>)> = sqlx::query_as(
            "SELECT a.id, p.kinds FROM accounts a JOIN notification_prefs p ON p.account_id = a.id \
             WHERE a.player_id = $1 AND NOT a.disabled")
            .bind(alert.player.0 as i64).fetch_optional(&self.0.pool).await?;
        let Some((account, kinds)) = row else { return Ok(()) };
        if !kinds.iter().any(|k| k == alert.kind.slug()) {
            return Ok(());
        }
        let now = Instant::now();
        {
            let mut sent = self.0.sent.lock().await;
            if sent.get(&(account, alert.kind)).is_some_and(|at| now.duration_since(*at) < PER_KIND_GAP) {
                return Ok(());
            }
            let today = chrono::Utc::now().date_naive();
            let mut daily = self.0.daily.lock().await;
            let count = daily.entry(account).or_insert((today, 0));
            if count.0 != today {
                *count = (today, 0);
            }
            if count.1 >= DAILY_CAP {
                return Ok(());
            }
            count.1 += 1;
            sent.insert((account, alert.kind), now);
        }
        let subscriptions: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT endpoint, p256dh, auth FROM push_subscriptions WHERE account_id = $1")
            .bind(account).fetch_all(&self.0.pool).await?;
        let payload = serde_json::json!({ "title": alert.kind.title(), "body": alert.text, "tag": alert.kind.slug() }).to_string();
        for (endpoint, p256dh, auth) in subscriptions {
            let Some(subject) = &self.0.config.push_subject else {
                tracing::info!(kind = alert.kind.slug(), "push (log-only): {}", alert.text);
                continue;
            };
            match self.push(&endpoint, &p256dh, &auth, subject, &payload).await {
                Ok(StatusCode::NOT_FOUND | StatusCode::GONE) => {
                    // The browser dropped the subscription.
                    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1").bind(&endpoint)
                        .execute(&self.0.pool).await?;
                }
                Ok(status) if !status.is_success() => tracing::warn!(%status, "push refused"),
                Ok(_) => {}
                Err(error) => tracing::warn!(%error, "push failed"),
            }
        }
        Ok(())
    }

    async fn push(&self, endpoint: &str, p256dh: &str, auth: &str, subject: &str, payload: &str) -> anyhow::Result<StatusCode> {
        let key = web_push_native::p256::PublicKey::from_sec1_bytes(&Base64UrlUnpadded::decode_vec(p256dh)?)?;
        let auth = web_push_native::Auth::clone_from_slice(&Base64UrlUnpadded::decode_vec(auth)?);
        let request = web_push_native::WebPushBuilder::new(endpoint.parse()?, key, auth)
            .with_valid_duration(PUSH_TTL)
            .with_vapid(&self.0.vapid, subject)
            .build(payload.as_bytes().to_vec())?;
        let (parts, body) = request.into_parts();
        let mut send = self.0.http.post(parts.uri.to_string()).body(body);
        for (name, value) in &parts.headers {
            send = send.header(name.as_str(), value.as_bytes());
        }
        let status = send.send().await?.status();
        Ok(StatusCode::from_u16(status.as_u16())?)
    }

    /// Email each account that asked for a digest and has stayed away.
    async fn send_digests(&self, game: &GameHandle) -> anyhow::Result<()> {
        let due: Vec<(Uuid, i64, String)> = sqlx::query_as(
            "SELECT a.id, a.player_id, p.email FROM accounts a JOIN notification_prefs p ON p.account_id = a.id \
             WHERE p.email_digest AND p.email IS NOT NULL AND NOT a.disabled \
             AND (p.digest_sent_at IS NULL OR p.digest_sent_at < now() - make_interval(hours => $1))")
            .bind(DIGEST_GAP_HOURS as i32).fetch_all(&self.0.pool).await?;
        for (account, player, email) in due {
            let Some(report) = game.digest(PlayerId(player as u64)).await else { continue };
            let text = digest_text(&report);
            match &self.0.config.email {
                Some(config) => {
                    let body = serde_json::json!({
                        "from": config.from, "to": email, "subject": "While you were away", "text": text,
                    });
                    let mut send = self.0.http.post(&config.url)
                        .header(header::CONTENT_TYPE.as_str(), "application/json").body(body.to_string());
                    if let Some(token) = &config.token {
                        send = send.bearer_auth(token);
                    }
                    let status = send.send().await?.status();
                    if !status.is_success() {
                        tracing::warn!(%status, "digest refused");
                        continue;
                    }
                }
                None => tracing::info!(lines = text.lines().count(), "email digest (log-only)"),
            }
            sqlx::query("UPDATE notification_prefs SET digest_sent_at = now() WHERE account_id = $1")
                .bind(account).execute(&self.0.pool).await?;
        }
        Ok(())
    }
}

/// The server's VAPID key pair, made once and kept beside the accounts.
async fn vapid_key(pool: &PgPool) -> anyhow::Result<ES256KeyPair> {
    let read = || sqlx::query_scalar::<_, String>("SELECT value FROM server_keys WHERE name = 'vapid_private'").fetch_optional(pool);
    if read().await?.is_none() {
        let fresh = Base64UrlUnpadded::encode_string(&ES256KeyPair::generate().to_bytes());
        sqlx::query("INSERT INTO server_keys (name, value) VALUES ('vapid_private', $1) ON CONFLICT (name) DO NOTHING")
            .bind(fresh).execute(pool).await?;
    }
    let stored = read().await?.ok_or_else(|| anyhow::anyhow!("VAPID key missing"))?;
    Ok(ES256KeyPair::from_bytes(&Base64UrlUnpadded::decode_vec(&stored)?)?)
}

#[derive(Serialize)]
struct PrefsView {
    kinds: Vec<&'static str>,
    email: Option<String>,
    email_digest: bool,
    /// The browsers' `applicationServerKey`.
    public_key: String,
    /// Whether this server sends pushes and emails, or only logs them.
    push_live: bool,
    email_live: bool,
    devices: i64,
}

#[derive(Deserialize)]
struct PrefsBody {
    kinds: Vec<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    email_digest: bool,
}

#[derive(Deserialize)]
struct PushBody {
    endpoint: String,
    #[serde(default)]
    p256dh: String,
    #[serde(default)]
    auth: String,
}

/// Same-origin GETs: the client header browsers can't forge cross-site.
fn client_header(headers: &HeaderMap) -> Result<(), AuthError> {
    (headers.get("x-stellar-client").and_then(|h| h.to_str().ok()) == Some("1"))
        .then_some(()).ok_or(AuthError::Forbidden)
}

async fn get_prefs(State(n): State<Notify>, headers: HeaderMap) -> Result<Response, AuthError> {
    client_header(&headers)?;
    let account = n.0.auth.authenticate(&headers).await?.account.id;
    let row: Option<(Vec<String>, Option<String>, bool)> = sqlx::query_as(
        "SELECT kinds, email, email_digest FROM notification_prefs WHERE account_id = $1")
        .bind(account).fetch_optional(&n.0.pool).await?;
    let devices: i64 = sqlx::query_scalar("SELECT count(*) FROM push_subscriptions WHERE account_id = $1")
        .bind(account).fetch_one(&n.0.pool).await?;
    let (kinds, email, email_digest) = row.unwrap_or_default();
    let view = PrefsView {
        kinds: kinds.iter().filter_map(|k| AlertKind::from_slug(k)).map(AlertKind::slug).collect(),
        email,
        email_digest,
        public_key: n.0.public_key.clone(),
        push_live: n.0.config.push_subject.is_some(),
        email_live: n.0.config.email.is_some(),
        devices,
    };
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(view)).into_response())
}

async fn set_prefs(State(n): State<Notify>, headers: HeaderMap, Json(body): Json<PrefsBody>) -> Result<Response, AuthError> {
    n.0.auth.check_origin(&headers)?;
    let account = n.0.auth.authenticate(&headers).await?.account.id;
    let kinds: Vec<&str> = AlertKind::ALL.iter().filter(|k| body.kinds.iter().any(|s| s == k.slug())).map(|k| k.slug()).collect();
    let email = body.email.map(|e| e.trim().to_string()).filter(|e| !e.is_empty());
    if email.as_ref().is_some_and(|e| e.len() > 254 || !e.contains('@') || e.chars().any(char::is_whitespace)) {
        return Err(AuthError::BadInput("That email address doesn't look right."));
    }
    if body.email_digest && email.is_none() {
        return Err(AuthError::BadInput("Add an email address for the digest."));
    }
    sqlx::query("INSERT INTO notification_prefs (account_id, kinds, email, email_digest, updated_at) VALUES ($1, $2, $3, $4, now()) \
                 ON CONFLICT (account_id) DO UPDATE SET kinds = $2, email = $3, email_digest = $4, updated_at = now()")
        .bind(account).bind(&kinds).bind(&email).bind(body.email_digest).execute(&n.0.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn add_push(State(n): State<Notify>, headers: HeaderMap, Json(body): Json<PushBody>) -> Result<Response, AuthError> {
    n.0.auth.check_origin(&headers)?;
    let account = n.0.auth.authenticate(&headers).await?.account.id;
    let key_ok = Base64UrlUnpadded::decode_vec(&body.p256dh).ok()
        .is_some_and(|b| web_push_native::p256::PublicKey::from_sec1_bytes(&b).is_ok());
    let auth_ok = Base64UrlUnpadded::decode_vec(&body.auth).is_ok_and(|b| b.len() == 16);
    if !body.endpoint.starts_with("https://") || body.endpoint.len() > 1_000 || !key_ok || !auth_ok {
        return Err(AuthError::BadInput("That push subscription isn't valid."));
    }
    sqlx::query("INSERT INTO push_subscriptions (endpoint, account_id, p256dh, auth) VALUES ($1, $2, $3, $4) \
                 ON CONFLICT (endpoint) DO UPDATE SET account_id = $2, p256dh = $3, auth = $4")
        .bind(&body.endpoint).bind(account).bind(&body.p256dh).bind(&body.auth).execute(&n.0.pool).await?;
    // Keep the newest few browsers.
    sqlx::query("DELETE FROM push_subscriptions WHERE account_id = $1 AND endpoint NOT IN \
                 (SELECT endpoint FROM push_subscriptions WHERE account_id = $1 ORDER BY created_at DESC LIMIT $2)")
        .bind(account).bind(DEVICES_MAX).execute(&n.0.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn remove_push(State(n): State<Notify>, headers: HeaderMap, Json(body): Json<PushBody>) -> Result<Response, AuthError> {
    n.0.auth.check_origin(&headers)?;
    let account = n.0.auth.authenticate(&headers).await?.account.id;
    sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1 AND account_id = $2")
        .bind(&body.endpoint).bind(account).execute(&n.0.pool).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::welcome::{WelcomeAction, WelcomeBackView, WelcomeGo, WelcomeLine};

    #[test]
    fn a_siege_alerts_only_the_side_under_attack() {
        let (owner, by) = (PlayerId(1), PlayerId(2));
        let siege = sim::EventPayload::BlockadeEstablished { by, owner, system: sim::EntityId(5), pos: sim::Vec2::new(0.0, 0.0) };
        assert_eq!(alert_of(&siege, owner), Some(AlertKind::Siege));
        assert_eq!(alert_of(&siege, by), None);
        let launch = |kind| sim::EventPayload::ShipLaunched { owner, system: sim::EntityId(5), fleet: sim::EntityId(6), kind };
        assert_eq!(alert_of(&launch(sim::ShipKind::Destroyer), owner), Some(AlertKind::Warship));
        assert_eq!(alert_of(&launch(sim::ShipKind::TinyFreighter), owner), None, "freighters aren't warships");
        assert!(AlertKind::ALL.iter().all(|k| AlertKind::from_slug(k.slug()) == Some(*k)));
    }

    #[test]
    fn the_digest_reads_like_the_report() {
        let line = |text: &str| WelcomeLine { severity: crate::protocol::TimelineSeverity::Info, text: text.into(), go: None };
        let report = WelcomeBackView {
            away: 26.0 * 3_600.0,
            happened: vec![line("Sold 40 ferrite ore · +400 Cr")],
            waiting: vec![line("2 Signals waiting")],
            next: vec![WelcomeAction { label: "Answer a Signal".into(), go: WelcomeGo::Signals }],
        };
        assert_eq!(digest_text(&report), "Stellar Syndicates · away 26h\n\nWhile you were away\n- Sold 40 ferrite ore · +400 Cr\n\nWaiting\n- 2 Signals waiting\n\nNext\n- Answer a Signal\n");
    }
}
