//! PostgreSQL owns accounts and revocable sessions; World owns only gameplay.
//! No name-only login or in-memory authentication fallback exists. Account writes
//! commit immediately, independently of the asynchronous galaxy snapshot queue.
//!
//! Deployment: ACCOUNTS_DATABASE_URL (or DATABASE_URL), APP_ORIGIN=https://host.
//! Only localhost may use HTTP. With a separate ACCOUNTS_DATABASE_URL, leaving
//! DATABASE_URL unset keeps playtest galaxies ephemeral without losing accounts.
//! This single authoritative server serializes session changes with handshakes;
//! revocation reaches open sockets immediately and DB work never enters a tick.

mod password;
#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{ConnectInfo, DefaultBodyLimit, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sim::PlayerId;
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions, PgSslMode},
};
use tokio::sync::{Mutex as AsyncMutex, Semaphore, watch};
use uuid::Uuid;
use zeroize::Zeroizing;

const SESSION_SECONDS: i64 = 24 * 60 * 60;
const RATE_WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_RATE_KEYS: usize = 4096;
pub const AUTH_REQUIRED_CLOSE_CODE: u16 = 4003;

#[derive(Debug)]
pub enum AuthError {
    BadInput(&'static str),
    Unauthorized,
    Forbidden,
    Conflict,
    Throttled,
    Unavailable,
}

impl From<sqlx::Error> for AuthError {
    fn from(_: sqlx::Error) -> Self {
        // SQL error details can include login identifiers. Never log request
        // bodies, passwords, hashes, tokens, database URLs or constraint values.
        tracing::warn!("account database operation failed");
        Self::Unavailable
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadInput(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "Sign-in failed or your session expired.",
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "Open the game at its configured address.",
            ),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "Unable to create that account. Sign in or choose different details.",
            ),
            Self::Throttled => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many attempts. Please try again later.",
            ),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Account service unavailable. Please try again later.",
            ),
        };
        let mut response = (status, Json(serde_json::json!({"error": message}))).into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if status == StatusCode::TOO_MANY_REQUESTS {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("900"));
        }
        response
    }
}

#[derive(Clone)]
pub struct AuthConfig {
    origins: Vec<String>,
    secure: bool,
}

impl AuthConfig {
    pub fn from_env(port: u16) -> anyhow::Result<Self> {
        Self::for_origin(
            &std::env::var("APP_ORIGIN").unwrap_or_else(|_| format!("http://localhost:{port}")),
        )
    }

    fn for_origin(origin: &str) -> anyhow::Result<Self> {
        let url = url::Url::parse(origin)?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        anyhow::ensure!(
            url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/"
                && (url.scheme() == "https" || (url.scheme() == "http" && local)),
            "APP_ORIGIN must be an HTTPS origin (HTTP is allowed only on localhost)"
        );
        let mut origins = vec![url.origin().ascii_serialization()];
        if local && url.scheme() == "http" {
            // Vite proxies /api and /ws, so passwords never follow ?server= URLs.
            for port in [url.port_or_known_default().unwrap_or(8080), 5173] {
                for host in ["localhost", "127.0.0.1", "[::1]"] {
                    origins.push(format!("http://{host}:{port}"));
                }
            }
        }
        Ok(Self {
            origins,
            secure: url.scheme() == "https",
        })
    }

    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        let origin = headers.get(header::ORIGIN).and_then(|s| s.to_str().ok());
        if origin.is_some_and(|s| self.origins.iter().any(|allowed| allowed == s)) {
            Ok(())
        } else {
            Err(AuthError::Forbidden)
        }
    }

    fn cookie_name(&self) -> &'static str {
        if self.secure {
            "__Host-ss_session"
        } else {
            "ss_session"
        }
    }

    fn cookie(&self, token: &str, max_age: i64) -> HeaderValue {
        HeaderValue::from_str(&format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{}",
            self.cookie_name(),
            if self.secure { "; Secure" } else { "" }
        ))
        .expect("only generated hex tokens enter cookies")
    }

    fn token(&self, headers: &HeaderMap) -> Result<[u8; 32], AuthError> {
        let mut tokens = headers
            .get_all(header::COOKIE)
            .iter()
            .filter_map(|h| h.to_str().ok())
            .flat_map(|h| h.split(';'))
            .filter_map(|piece| piece.trim().split_once('='))
            .filter(|(name, _)| *name == self.cookie_name());
        let (_, token) = tokens.next().ok_or(AuthError::Unauthorized)?;
        if tokens.next().is_some()
            || token.len() != 64
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(AuthError::Unauthorized);
        }
        Ok(Sha256::digest(token.as_bytes()).into())
    }
}

#[derive(Clone, Serialize, sqlx::FromRow)]
pub struct Account {
    pub id: Uuid,
    #[serde(skip)]
    player_id: i64,
    pub corporation_name: String,
}

impl Account {
    pub fn player_id(&self) -> PlayerId {
        PlayerId(self.player_id as u64)
    }
}

// Deliberately no Debug: the PHC hash is sensitive even though it is not plaintext.
#[derive(sqlx::FromRow)]
struct Credential {
    id: Uuid,
    password_hash: String,
    disabled: bool,
}

pub struct AuthSession {
    pub account: Account,
    pub expires_at: DateTime<Utc>,
    pub revoked: watch::Receiver<bool>,
}

#[cfg(test)]
pub(crate) fn transport_test_session() -> (AuthSession, watch::Sender<bool>) {
    let (revoke, revoked) = watch::channel(false);
    (
        AuthSession {
            account: Account {
                id: Uuid::new_v4(),
                player_id: 1234,
                corporation_name: "Socket test".into(),
            },
            expires_at: Utc::now() + chrono::Duration::hours(1),
            revoked,
        },
        revoke,
    )
}

struct LiveSession {
    token_hash: [u8; 32],
    revoke: watch::Sender<bool>,
}

#[derive(Default)]
struct RateLimits {
    keys: HashMap<String, (Instant, u32)>,
}

impl RateLimits {
    fn take(&mut self, key: String, limit: u32, now: Instant) -> Result<(), AuthError> {
        self.keys
            .retain(|_, (start, _)| now.duration_since(*start) < RATE_WINDOW);
        if !self.keys.contains_key(&key) && self.keys.len() >= MAX_RATE_KEYS {
            return Err(AuthError::Throttled);
        }
        let (_, count) = self.keys.entry(key).or_insert((now, 0));
        if *count >= limit {
            return Err(AuthError::Throttled);
        }
        *count += 1;
        Ok(())
    }
}

struct Inner {
    pool: PgPool,
    config: AuthConfig,
    dummy_hash: String,
    password_slots: Arc<Semaphore>,
    limits: Mutex<RateLimits>,
    // Short auth-only critical section: serializes DB session rotation and
    // socket registration so a concurrently authenticated old token cannot
    // subscribe AFTER its revocation. Hashing happens outside this lock.
    live: AsyncMutex<HashMap<Uuid, LiveSession>>,
}

#[derive(Clone)]
pub struct AuthStore(Arc<Inner>);

impl AuthStore {
    pub async fn from_env(port: u16) -> anyhow::Result<Self> {
        let url = std::env::var("ACCOUNTS_DATABASE_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .map_err(|_| {
                anyhow::anyhow!(
                    "set ACCOUNTS_DATABASE_URL (or DATABASE_URL); accounts require PostgreSQL"
                )
            })?;
        let config = AuthConfig::from_env(port)?;
        let mut options: PgConnectOptions = url
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid account PostgreSQL configuration"))?;
        if !matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1")
            && !options.get_host().starts_with('/')
        {
            // A remote account database must authenticate its TLS certificate,
            // not merely try TLS then silently fall back to plaintext.
            options = options.ssl_mode(PgSslMode::VerifyFull);
        }
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(|_| {
                anyhow::anyhow!(
                    "cannot connect to account PostgreSQL; refusing unauthenticated startup"
                )
            })?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Self::new(pool, config).await
    }

    async fn new(pool: PgPool, config: AuthConfig) -> anyhow::Result<Self> {
        // Unknown logins pay the same verifier cost as known ones. This random
        // dummy credential is never a usable account or a default password.
        let dummy_hash =
            tokio::task::spawn_blocking(|| password::hash(Zeroizing::new(random_token())))
                .await?
                .map_err(|_| anyhow::anyhow!("cannot initialize password verifier"))?;
        Ok(Self(Arc::new(Inner {
            pool,
            config,
            dummy_hash,
            password_slots: Arc::new(Semaphore::new(2)),
            limits: Mutex::default(),
            live: AsyncMutex::default(),
        })))
    }

    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        self.0.config.check_origin(headers)
    }

    fn throttle(&self, ip: IpAddr, login: &str) -> Result<(), AuthError> {
        let mut limits = self.0.limits.lock().map_err(|_| AuthError::Unavailable)?;
        let now = Instant::now();
        // Never trust arbitrary X-Forwarded-For. Behind a proxy this is a
        // conservative shared IP bucket, not a spoofable client identity.
        limits.take(format!("ip:{ip}"), 60, now)?;
        limits.take(format!("login:{login}"), 12, now)?;
        limits.take("global".into(), 300, now)
    }

    async fn hash(&self, password: String) -> Result<String, AuthError> {
        let password = Zeroizing::new(password);
        let permit = self
            .0
            .password_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| AuthError::Throttled)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            password::hash(password)
        })
        .await
        .map_err(|_| AuthError::Unavailable)?
    }

    async fn verify(&self, password: String, hash: String) -> Result<bool, AuthError> {
        let password = Zeroizing::new(password);
        let permit = self
            .0
            .password_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| AuthError::Throttled)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            password::verify(password, &hash)
        })
        .await
        .map_err(|_| AuthError::Unavailable)
    }

    async fn register(&self, input: Register, ip: IpAddr) -> Result<(Account, String), AuthError> {
        let login = normalize_login(&input.login)?;
        self.throttle(ip, &login)?;
        password::validate(&input.password)?;
        let name = input.corporation_name.trim();
        if name.is_empty() || name.chars().count() > 32 || name.chars().any(char::is_control) {
            return Err(AuthError::BadInput(
                "Use a corporation name of 1–32 characters.",
            ));
        }
        let hash = self.hash(input.password).await?;
        let account = Account {
            id: Uuid::new_v4(),
            player_id: random_player_id(),
            corporation_name: name.into(),
        };
        let mut live = self.0.live.lock().await;
        let mut tx = self.0.pool.begin().await?;
        let inserted = sqlx::query("INSERT INTO accounts (id,player_id,login,corporation_name,corporation_key,password_hash) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING")
            .bind(account.id).bind(account.player_id).bind(login).bind(name).bind(name.to_lowercase()).bind(hash)
            .execute(&mut *tx).await?;
        if inserted.rows_affected() == 0 {
            return Err(AuthError::Conflict);
        }
        let token = random_token();
        write_session(&mut tx, account.id, &token).await?;
        tx.commit().await?;
        revoke(&mut live, account.id);
        Ok((account, token))
    }

    async fn login(&self, input: Login, ip: IpAddr) -> Result<(Account, String), AuthError> {
        let login = normalize_login(&input.login)?;
        self.throttle(ip, &login)?;
        // Reject oversized input before Argon2, but never normalize a password.
        if input.password.len() > 512 {
            return Err(AuthError::Unauthorized);
        }
        let credential: Option<Credential> =
            sqlx::query_as("SELECT id,password_hash,disabled FROM accounts WHERE login=$1")
                .bind(login)
                .fetch_optional(&self.0.pool)
                .await?;
        let hash = credential
            .as_ref()
            .map_or(&self.0.dummy_hash, |c| &c.password_hash)
            .clone();
        let valid = self.verify(input.password, hash.clone()).await?;
        let credential = credential
            .filter(|c| valid && !c.disabled)
            .ok_or(AuthError::Unauthorized)?;
        let mut live = self.0.live.lock().await;
        let mut tx = self.0.pool.begin().await?;
        // Recheck under a row lock: a password/disabled change while the costly
        // verifier was running must not issue a session using stale credentials.
        let account: Account = sqlx::query_as("SELECT id,player_id,corporation_name FROM accounts WHERE id=$1 AND password_hash=$2 AND NOT disabled FOR UPDATE")
            .bind(credential.id).bind(hash).fetch_optional(&mut *tx).await?.ok_or(AuthError::Unauthorized)?;
        let token = random_token();
        write_session(&mut tx, account.id, &token).await?;
        tx.commit().await?;
        revoke(&mut live, account.id);
        Ok((account, token))
    }

    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthSession, AuthError> {
        let token_hash = self.0.config.token(headers)?;
        let mut live = self.0.live.lock().await;
        let row: Option<(Uuid, i64, String, DateTime<Utc>)> = sqlx::query_as(
            "SELECT a.id,a.player_id,a.corporation_name,s.expires_at FROM accounts a JOIN account_sessions s ON s.account_id=a.id WHERE s.token_hash=$1 AND s.expires_at>now() AND NOT a.disabled")
            .bind(token_hash.as_slice()).fetch_optional(&self.0.pool).await?;
        let (id, player_id, corporation_name, expires_at) = row.ok_or(AuthError::Unauthorized)?;
        live.retain(|_, s| s.revoke.receiver_count() != 0);
        if live.get(&id).is_some_and(|s| s.token_hash != token_hash) {
            revoke(&mut live, id);
        }
        let entry = live.entry(id).or_insert_with(|| LiveSession {
            token_hash,
            revoke: watch::channel(false).0,
        });
        Ok(AuthSession {
            account: Account {
                id,
                player_id,
                corporation_name,
            },
            expires_at,
            revoked: entry.revoke.subscribe(),
        })
    }

    async fn logout(&self, headers: &HeaderMap) -> Result<(), AuthError> {
        let token_hash = match self.0.config.token(headers) {
            Ok(hash) => hash,
            Err(_) => return Ok(()),
        };
        let mut live = self.0.live.lock().await;
        let id: Option<(Uuid,)> =
            sqlx::query_as("DELETE FROM account_sessions WHERE token_hash=$1 RETURNING account_id")
                .bind(token_hash.as_slice())
                .fetch_optional(&self.0.pool)
                .await?;
        if let Some((id,)) = id {
            revoke(&mut live, id);
        }
        Ok(())
    }

    fn signed_in(&self, account: Account, token: String) -> Response {
        let mut response = Json(account).into_response();
        response.headers_mut().insert(
            header::SET_COOKIE,
            self.0.config.cookie(&token, SESSION_SECONDS),
        );
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

fn revoke(live: &mut HashMap<Uuid, LiveSession>, id: Uuid) {
    if let Some(session) = live.remove(&id) {
        session.revoke.send_replace(true);
    }
}

async fn write_session(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    token: &str,
) -> Result<(), AuthError> {
    sqlx::query("INSERT INTO account_sessions (account_id,token_hash,expires_at) VALUES ($1,$2,now()+($3 * interval '1 second')) ON CONFLICT (account_id) DO UPDATE SET token_hash=EXCLUDED.token_hash,created_at=now(),expires_at=EXCLUDED.expires_at")
        .bind(id).bind(Sha256::digest(token.as_bytes()).as_slice()).bind(SESSION_SECONDS as f64)
        .execute(&mut **tx).await?;
    Ok(())
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn random_player_id() -> i64 {
    loop {
        let id = OsRng.next_u64() & i64::MAX as u64;
        if id != 0 && !PlayerId(id).is_sentinel() {
            return id as i64;
        }
    }
}

fn normalize_login(login: &str) -> Result<String, AuthError> {
    let login = login.trim().to_lowercase();
    if !(3..=254).contains(&login.len())
        || login.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(AuthError::BadInput("Enter a valid login."));
    }
    Ok(login)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Register {
    login: String,
    password: String,
    corporation_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    login: String,
    password: String,
}

async fn register(
    State(auth): State<AuthStore>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<Response, AuthError> {
    auth.check_origin(&headers)?;
    let (account, token) = auth.register(input, peer.ip()).await?;
    Ok(auth.signed_in(account, token))
}
async fn login(
    State(auth): State<AuthStore>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response, AuthError> {
    auth.check_origin(&headers)?;
    let (account, token) = auth.login(input, peer.ip()).await?;
    Ok(auth.signed_in(account, token))
}
async fn session(State(auth): State<AuthStore>, headers: HeaderMap) -> Result<Response, AuthError> {
    // Fetch sends an explicit Origin-equivalent header for same-origin GETs,
    // where browsers commonly omit Origin. SOP prevents another site reading
    // it; cross-site requests cannot set this header without a denied preflight.
    if headers
        .get("x-stellar-client")
        .and_then(|h| h.to_str().ok())
        != Some("1")
    {
        return Err(AuthError::Forbidden);
    }
    let session = auth.authenticate(&headers).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(session.account)).into_response())
}
async fn logout(State(auth): State<AuthStore>, headers: HeaderMap) -> Result<Response, AuthError> {
    auth.check_origin(&headers)?;
    auth.logout(&headers).await?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, auth.0.config.cookie("", 0));
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

pub fn routes(auth: AuthStore) -> Router {
    Router::new()
        .route("/api/account/register", post(register))
        .route("/api/account/login", post(login))
        .route("/api/account/session", get(session))
        .route("/api/account/logout", post(logout))
        .layer(DefaultBodyLimit::max(4096))
        .with_state(auth)
}
