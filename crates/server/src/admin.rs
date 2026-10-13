//! Operator dashboard API behind `/admin.html`: who registered and from where,
//! who is online, sign-in history, play time, every recorded command, and each
//! corporation's holdings. Read-only. Only a signed-in, registered account whose
//! login is listed in `ADMIN_LOGINS` may read it; with the list empty it is off.
//!
//! Holdings come from server truth, which no player view may carry; that is why
//! this lives outside the game protocol and behind an operator check.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sim::PlayerId;
use tokio::sync::watch;
use uuid::Uuid;

use crate::activity::{Activity, LivePlay};
use crate::auth::{AuthError, AuthStore};
use crate::session::{AdminSnapshot, CorporationStats, GameHandle, ServerStatus};
use crate::tutorial::{self, AccountTutorial, FunnelAccount, FunnelStage, PlayerTutorial, TutorialRecorder};

#[derive(Clone)]
pub struct AdminState {
    auth: AuthStore,
    activity: Activity,
    game: GameHandle,
    status: watch::Receiver<ServerStatus>,
    tutorial: TutorialRecorder,
    operators: Arc<HashSet<String>>,
    started_at: DateTime<Utc>,
}

impl AdminState {
    pub fn new(
        auth: AuthStore,
        activity: Activity,
        game: GameHandle,
        status: watch::Receiver<ServerStatus>,
        tutorial: TutorialRecorder,
        operators: HashSet<String>,
    ) -> Self {
        Self {
            auth,
            activity,
            game,
            status,
            tutorial,
            operators: Arc::new(operators),
            started_at: Utc::now(),
        }
    }
}

/// `ADMIN_LOGINS`: comma-separated logins, matched as sign-in normalizes them.
pub fn operators_from(list: &str) -> HashSet<String> {
    list.split(',')
        .map(|login| login.trim().to_lowercase())
        .filter(|login| !login.is_empty())
        .collect()
}

pub fn routes(state: AdminState) -> Router {
    Router::new()
        .route("/admin", get(|| async { Redirect::to("/admin.html") }))
        .route("/api/admin/overview", get(overview))
        .route("/api/admin/players/{id}", get(player))
        .with_state(state)
}

#[derive(Debug)]
pub enum AdminError {
    Off,
    SignIn,
    NotOperator,
    NoSuchAccount,
    Unavailable,
}

impl From<sqlx::Error> for AdminError {
    fn from(_: sqlx::Error) -> Self {
        tracing::warn!("admin dashboard query failed");
        Self::Unavailable
    }
}

impl From<AuthError> for AdminError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::Unavailable | AuthError::Throttled => Self::Unavailable,
            _ => Self::SignIn,
        }
    }
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Off => (
                StatusCode::NOT_FOUND,
                "off",
                "The admin dashboard is off on this server. Set ADMIN_LOGINS to enable it.",
            ),
            Self::SignIn => (
                StatusCode::UNAUTHORIZED,
                "sign_in",
                "Sign in to the game in this browser first.",
            ),
            Self::NotOperator => (
                StatusCode::FORBIDDEN,
                "not_operator",
                "This account is not an operator. Add its login to ADMIN_LOGINS.",
            ),
            Self::NoSuchAccount => (StatusCode::NOT_FOUND, "no_account", "No such account."),
            Self::Unavailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "The account database is unavailable. Try again shortly.",
            ),
        };
        let mut response = (
            status,
            Json(serde_json::json!({ "error": message, "code": code })),
        )
            .into_response();
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        response
    }
}

fn no_store(body: impl Serialize) -> Response {
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn operator(state: &AdminState, headers: &HeaderMap) -> Result<(), AdminError> {
    if state.operators.is_empty() {
        return Err(AdminError::Off);
    }
    // The same same-origin marker the session check uses: another site cannot
    // send it without a preflight this server never grants.
    if headers.get("x-stellar-client").and_then(|h| h.to_str().ok()) != Some("1") {
        return Err(AdminError::SignIn);
    }
    let session = state.auth.authenticate(headers).await?;
    let login: Option<(Option<String>,)> = sqlx::query_as(
        "SELECT login FROM accounts WHERE id=$1 AND NOT disabled AND NOT is_guest",
    )
    .bind(session.account.id)
    .fetch_optional(&state.auth.pool())
    .await?;
    match login.and_then(|(login,)| login) {
        Some(login) if state.operators.contains(&login) => Ok(()),
        _ => Err(AdminError::NotOperator),
    }
}

/// The game loop's holdings and charter progress, with any newly reached
/// steps written first so the page is never behind the game.
async fn snapshot(state: &AdminState) -> Option<AdminSnapshot> {
    let snapshot = state.game.admin_snapshot().await?;
    state.tutorial.record(&snapshot).await;
    Some(snapshot)
}

/// The reported galaxy's start and every account's charter in it.
async fn tutorials(
    state: &AdminState,
    snapshot: Option<&AdminSnapshot>,
    account: Option<Uuid>,
) -> Result<Option<(String, DateTime<Utc>, HashMap<Uuid, AccountTutorial>)>, AdminError> {
    let pool = state.auth.pool();
    let Some((galaxy, started)) =
        tutorial::galaxy(&pool, snapshot.map(|s| s.galaxy_id.as_str())).await?
    else {
        return Ok(None);
    };
    let accounts = tutorial::load(&pool, &galaxy, started, account).await?;
    Ok(Some((galaxy, started, accounts)))
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: Uuid,
    player_id: i64,
    login: Option<String>,
    corporation_name: String,
    is_guest: bool,
    disabled: bool,
    created_at: DateTime<Utc>,
    last_login_at: Option<DateTime<Utc>>,
    logins: i64,
    last_ip: Option<String>,
    last_device: Option<String>,
    last_user_agent: Option<String>,
    sessions: i64,
    online_seconds: f64,
    active_seconds: i64,
    actions: i64,
    last_seen_at: Option<DateTime<Utc>>,
    devices: i64,
    shared_accounts: i64,
}

// Small tables aggregated whole on each request; the command log is not read.
// Loopback addresses are every local player's, so they never link accounts.
const ACCOUNTS_SQL: &str = r#"
WITH devices AS (
    SELECT account_id, device_id FROM account_logins WHERE device_id IS NOT NULL
    UNION
    SELECT account_id, device_id FROM play_sessions WHERE device_id IS NOT NULL
),
shared AS (
    SELECT mine.account_id,
           COUNT(DISTINCT mine.device_id) AS devices,
           COUNT(DISTINCT other.account_id) AS shared_accounts
    FROM devices mine
    LEFT JOIN devices other ON other.device_id = mine.device_id AND other.account_id <> mine.account_id
    GROUP BY mine.account_id
),
logins AS (
    SELECT account_id, COUNT(*) AS n, MAX(at) AS last_at FROM account_logins GROUP BY account_id
),
plays AS (
    SELECT account_id,
           COUNT(*) AS n,
           SUM(EXTRACT(EPOCH FROM (COALESCE(disconnected_at, now()) - connected_at)))::float8 AS online_seconds,
           SUM(active_seconds)::int8 AS active_seconds,
           SUM(actions)::int8 AS actions,
           MAX(COALESCE(disconnected_at, last_seen_at)) AS last_seen_at
    FROM play_sessions GROUP BY account_id
)
SELECT a.id, a.player_id, a.login, a.corporation_name, a.is_guest, a.disabled, a.created_at,
       logins.last_at AS last_login_at,
       COALESCE(logins.n, 0) AS logins,
       last.ip AS last_ip, last.device_id AS last_device, last.user_agent AS last_user_agent,
       COALESCE(plays.n, 0) AS sessions,
       COALESCE(plays.online_seconds, 0) AS online_seconds,
       COALESCE(plays.active_seconds, 0) AS active_seconds,
       COALESCE(plays.actions, 0) AS actions,
       plays.last_seen_at,
       COALESCE(shared.devices, 0) AS devices,
       COALESCE(shared.shared_accounts, 0) AS shared_accounts
FROM accounts a
LEFT JOIN logins ON logins.account_id = a.id
LEFT JOIN plays ON plays.account_id = a.id
LEFT JOIN shared ON shared.account_id = a.id
LEFT JOIN LATERAL (
    SELECT ip, device_id, user_agent FROM (
        SELECT at, ip, device_id, user_agent FROM account_logins WHERE account_id = a.id
        UNION ALL
        SELECT connected_at, ip, device_id, user_agent FROM play_sessions WHERE account_id = a.id
    ) seen ORDER BY at DESC LIMIT 1
) last ON true
WHERE ($1::uuid IS NULL OR a.id = $1)
ORDER BY a.created_at DESC
"#;

#[derive(Serialize)]
struct OnlineNow {
    since: DateTime<Utc>,
    ip: String,
    device_id: Option<String>,
    actions: u64,
    last_action_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
struct PlayerSummary {
    account_id: Uuid,
    login: Option<String>,
    corporation: String,
    guest: bool,
    disabled: bool,
    registered_at: DateTime<Utc>,
    last_login_at: Option<DateTime<Utc>>,
    logins: i64,
    last_ip: Option<String>,
    last_device: Option<String>,
    last_user_agent: Option<String>,
    sessions: i64,
    online_seconds: f64,
    active_seconds: i64,
    actions: i64,
    last_seen_at: Option<DateTime<Utc>>,
    online: Option<OnlineNow>,
    devices: i64,
    /// Other accounts that have used one of this account's browsers.
    shared_accounts: i64,
    /// `None` until the corporation is founded (or while the loop is busy).
    game: Option<CorporationStats>,
    /// Founding-charter progress in the reported galaxy.
    tutorial: Option<PlayerTutorial>,
    /// Listed in ADMIN_LOGINS; kept out of the funnel.
    operator: bool,
    /// Counted in the funnel: arrived during this galaxy and not an operator.
    in_funnel: bool,
    /// Online, or seen (or signed up) within the quiet window.
    active: bool,
}

fn summarize(
    row: AccountRow,
    live: &HashMap<Uuid, &LivePlay>,
    holdings: &mut HashMap<PlayerId, CorporationStats>,
    tutorial: Option<PlayerTutorial>,
) -> PlayerSummary {
    let online = live.get(&row.id).map(|play| OnlineNow {
        since: play.connected_at,
        ip: play.client.ip.to_string(),
        device_id: play.client.device_id.clone(),
        actions: play.actions,
        last_action_at: play.last_action_at,
    });
    PlayerSummary {
        account_id: row.id,
        login: row.login,
        corporation: row.corporation_name,
        guest: row.is_guest,
        disabled: row.disabled,
        registered_at: row.created_at,
        last_login_at: row.last_login_at,
        logins: row.logins,
        last_ip: row.last_ip,
        last_device: row.last_device,
        last_user_agent: row.last_user_agent,
        sessions: row.sessions,
        online_seconds: row.online_seconds,
        active_seconds: row.active_seconds,
        actions: row.actions,
        last_seen_at: if online.is_some() { Some(Utc::now()) } else { row.last_seen_at },
        online,
        devices: row.devices,
        shared_accounts: row.shared_accounts,
        game: holdings.remove(&PlayerId(row.player_id as u64)),
        tutorial,
        operator: false,
        in_funnel: false,
        active: false,
    }
}

/// The newest socket per account; one live corporation owns one socket.
fn live_by_account(live: &[LivePlay]) -> HashMap<Uuid, &LivePlay> {
    let mut by_account: HashMap<Uuid, &LivePlay> = HashMap::new();
    for play in live {
        let newer = by_account
            .get(&play.account_id)
            .is_none_or(|known| play.connected_at > known.connected_at);
        if newer {
            by_account.insert(play.account_id, play);
        }
    }
    by_account
}

#[derive(Serialize, Default)]
struct Totals {
    accounts: usize,
    registered: usize,
    guests: usize,
    founded: usize,
    online: usize,
    new_24h: usize,
    active_24h: usize,
    active_7d: usize,
}

#[derive(Serialize)]
struct ServerSummary {
    started_at: DateTime<Utc>,
    #[serde(flatten)]
    status: ServerStatus,
    /// False when the game loop did not answer in time; holdings are missing.
    holdings: bool,
}

/// Where new players stop: everyone who arrived during this galaxy, from
/// signing up through the founding charter. Operators are left out.
#[derive(Serialize)]
struct Funnel {
    galaxy_id: String,
    galaxy_started_at: DateTime<Utc>,
    players: usize,
    quiet_after_hours: i64,
    stages: Vec<FunnelStage>,
}

#[derive(Serialize)]
struct Overview {
    generated_at: DateTime<Utc>,
    server: ServerSummary,
    totals: Totals,
    /// `None` until the game loop has reported a galaxy.
    funnel: Option<Funnel>,
    players: Vec<PlayerSummary>,
    /// Corporations with no account: headless bots, or saves older than accounts.
    unlinked: Vec<CorporationStats>,
}

/// Still around: online, or active (or newly signed up) within the quiet window.
fn active(player: &PlayerSummary, now: DateTime<Utc>) -> bool {
    let last = player.last_seen_at.max(Some(player.registered_at));
    player.online.is_some()
        || last.is_some_and(|at| now - at <= chrono::Duration::hours(tutorial::QUIET_AFTER_HOURS))
}

fn totals(players: &[PlayerSummary], now: DateTime<Utc>) -> Totals {
    let seen_within = |player: &PlayerSummary, hours: i64| {
        player.online.is_some()
            || player
                .last_seen_at
                .is_some_and(|seen| now - seen <= chrono::Duration::hours(hours))
    };
    Totals {
        accounts: players.len(),
        registered: players.iter().filter(|p| !p.guest).count(),
        guests: players.iter().filter(|p| p.guest).count(),
        founded: players.iter().filter(|p| p.game.is_some()).count(),
        online: players.iter().filter(|p| p.online.is_some()).count(),
        new_24h: players
            .iter()
            .filter(|p| now - p.registered_at <= chrono::Duration::hours(24))
            .count(),
        active_24h: players.iter().filter(|p| seen_within(p, 24)).count(),
        active_7d: players.iter().filter(|p| seen_within(p, 24 * 7)).count(),
    }
}

async fn overview(
    State(state): State<AdminState>,
    headers: HeaderMap,
) -> Result<Response, AdminError> {
    operator(&state, &headers).await?;
    let snapshot = snapshot(&state).await;
    let rows: Vec<AccountRow> = sqlx::query_as(ACCOUNTS_SQL)
        .bind(None::<Uuid>)
        .fetch_all(&state.auth.pool())
        .await?;
    let charters = tutorials(&state, snapshot.as_ref(), None).await?;
    let available = snapshot.is_some();
    let mut holdings: HashMap<PlayerId, CorporationStats> = snapshot
        .map(|snapshot| snapshot.corporations)
        .unwrap_or_default()
        .into_iter()
        .map(|corp| (corp.player_id, corp))
        .collect();
    let live = state.activity.live();
    let live = live_by_account(&live);
    let now = Utc::now();
    let mut population: Vec<(Uuid, bool)> = Vec::new();
    let players: Vec<PlayerSummary> = rows
        .into_iter()
        .map(|row| {
            let operator = row.login.as_ref().is_some_and(|login| state.operators.contains(login));
            let charter = charters.as_ref().and_then(|(_, _, accounts)| accounts.get(&row.id));
            let since = charters.as_ref().map(|(_, started, _)| *started);
            // Everyone who arrived during this galaxy: signed up, played or
            // progressed since it began.
            let arrived = charter.is_some_and(|c| c.entered.is_some() || !c.steps.is_empty())
                || since.is_some_and(|since| row.created_at >= since);
            let id = row.id;
            let mut player = summarize(row, &live, &mut holdings, charter.map(|c| tutorial::summary(c, false)));
            player.operator = operator;
            player.active = active(&player, now);
            player.in_funnel = arrived && !operator;
            if player.in_funnel {
                population.push((id, player.active));
            }
            player
        })
        .collect();
    let funnel = charters.as_ref().map(|(galaxy, started, accounts)| {
        let empty = AccountTutorial::default();
        let members: Vec<FunnelAccount<'_>> = population
            .iter()
            .map(|(id, active)| FunnelAccount { tutorial: accounts.get(id).unwrap_or(&empty), active: *active })
            .collect();
        Funnel {
            galaxy_id: galaxy.clone(),
            galaxy_started_at: *started,
            players: members.len(),
            quiet_after_hours: tutorial::QUIET_AFTER_HOURS,
            stages: tutorial::funnel(&members),
        }
    });
    let mut unlinked: Vec<CorporationStats> = holdings.into_values().collect();
    unlinked.sort_by(|a, b| b.valuation.total_cmp(&a.valuation));
    Ok(no_store(Overview {
        generated_at: now,
        server: ServerSummary {
            started_at: state.started_at,
            status: state.status.borrow().clone(),
            holdings: available,
        },
        totals: totals(&players, now),
        funnel,
        players,
        unlinked,
    }))
}

#[derive(Serialize, sqlx::FromRow)]
struct LoginEntry {
    kind: String,
    at: DateTime<Utc>,
    ip: Option<String>,
    user_agent: Option<String>,
    language: Option<String>,
    device_id: Option<String>,
}

#[derive(Serialize, sqlx::FromRow)]
struct SessionEntry {
    connected_at: DateTime<Utc>,
    disconnected_at: Option<DateTime<Utc>>,
    last_seen_at: DateTime<Utc>,
    ip: Option<String>,
    user_agent: Option<String>,
    device_id: Option<String>,
    actions: i32,
    active_seconds: i32,
}

#[derive(Serialize, sqlx::FromRow)]
struct DayEntry {
    day: NaiveDate,
    sessions: i64,
    online_seconds: f64,
    active_seconds: i64,
    actions: i64,
}

#[derive(Serialize, sqlx::FromRow)]
struct CommandCount {
    kind: String,
    count: i64,
    last_at: DateTime<Utc>,
}

#[derive(Serialize, sqlx::FromRow)]
struct CommandEntry {
    at: DateTime<Utc>,
    kind: String,
    detail: Option<serde_json::Value>,
}

#[derive(Serialize, sqlx::FromRow)]
struct SharedEntry {
    account_id: Uuid,
    login: Option<String>,
    corporation: String,
    via: String,
    value: String,
}

#[derive(Serialize)]
struct PlayerDetail {
    generated_at: DateTime<Utc>,
    player: PlayerSummary,
    logins: Vec<LoginEntry>,
    sessions: Vec<SessionEntry>,
    /// The last 30 UTC days with any play, oldest first.
    days: Vec<DayEntry>,
    commands: Vec<CommandCount>,
    recent: Vec<CommandEntry>,
    shared: Vec<SharedEntry>,
}

const SHARED_SQL: &str = r#"
WITH seen AS (
    SELECT account_id, 'device' AS via, device_id AS value FROM account_logins WHERE device_id IS NOT NULL
    UNION SELECT account_id, 'device', device_id FROM play_sessions WHERE device_id IS NOT NULL
    UNION SELECT account_id, 'ip', ip FROM account_logins WHERE ip IS NOT NULL AND ip NOT IN ('127.0.0.1', '::1')
    UNION SELECT account_id, 'ip', ip FROM play_sessions WHERE ip IS NOT NULL AND ip NOT IN ('127.0.0.1', '::1')
)
SELECT DISTINCT other.account_id, a.login, a.corporation_name AS corporation, other.via, other.value
FROM seen mine
JOIN seen other ON other.via = mine.via AND other.value = mine.value AND other.account_id <> mine.account_id
JOIN accounts a ON a.id = other.account_id
WHERE mine.account_id = $1
ORDER BY other.via, a.corporation_name
LIMIT 100
"#;

async fn player(
    State(state): State<AdminState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, AdminError> {
    operator(&state, &headers).await?;
    let pool = state.auth.pool();
    let row: AccountRow = sqlx::query_as(ACCOUNTS_SQL)
        .bind(Some(id))
        .fetch_optional(&pool)
        .await?
        .ok_or(AdminError::NoSuchAccount)?;
    let logins = sqlx::query_as("SELECT kind, at, ip, user_agent, language, device_id FROM account_logins WHERE account_id=$1 ORDER BY at DESC LIMIT 100")
        .bind(id).fetch_all(&pool).await?;
    let sessions = sqlx::query_as("SELECT connected_at, disconnected_at, last_seen_at, ip, user_agent, device_id, actions, active_seconds FROM play_sessions WHERE account_id=$1 ORDER BY connected_at DESC LIMIT 100")
        .bind(id).fetch_all(&pool).await?;
    let days = sqlx::query_as("SELECT (connected_at AT TIME ZONE 'UTC')::date AS day, COUNT(*) AS sessions, SUM(EXTRACT(EPOCH FROM (COALESCE(disconnected_at, now()) - connected_at)))::float8 AS online_seconds, SUM(active_seconds)::int8 AS active_seconds, SUM(actions)::int8 AS actions FROM play_sessions WHERE account_id=$1 AND connected_at > now() - interval '30 days' GROUP BY 1 ORDER BY 1")
        .bind(id).fetch_all(&pool).await?;
    let commands = sqlx::query_as("SELECT kind, COUNT(*) AS count, MAX(at) AS last_at FROM player_actions WHERE account_id=$1 GROUP BY kind ORDER BY COUNT(*) DESC, kind")
        .bind(id).fetch_all(&pool).await?;
    let recent = sqlx::query_as("SELECT at, kind, detail FROM player_actions WHERE account_id=$1 ORDER BY at DESC, id DESC LIMIT 200")
        .bind(id).fetch_all(&pool).await?;
    let shared = sqlx::query_as(SHARED_SQL).bind(id).fetch_all(&pool).await?;
    let snapshot = snapshot(&state).await;
    let charter = tutorials(&state, snapshot.as_ref(), Some(id))
        .await?
        .and_then(|(_, _, accounts)| accounts.get(&id).map(|c| tutorial::summary(c, true)));
    let mut holdings: HashMap<PlayerId, CorporationStats> = snapshot
        .map(|snapshot| snapshot.corporations)
        .unwrap_or_default()
        .into_iter()
        .map(|corp| (corp.player_id, corp))
        .collect();
    let live = state.activity.live();
    let operator = row.login.as_ref().is_some_and(|login| state.operators.contains(login));
    let mut player = summarize(row, &live_by_account(&live), &mut holdings, charter);
    player.operator = operator;
    player.active = active(&player, Utc::now());
    Ok(no_store(PlayerDetail {
        generated_at: Utc::now(),
        player,
        logins,
        sessions,
        days,
        commands,
        recent,
        shared,
    }))
}

#[cfg(test)]
mod tests;
