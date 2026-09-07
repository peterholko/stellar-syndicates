use super::*;
use crate::{
    AppState,
    session::{GameHandle, GameInput, ServerStatus},
    wire, ws,
};
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use futures::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tower::ServiceExt;

#[test]
fn password_minimum_is_eight_characters() {
    assert!(password::validate("orbital").is_err());
    assert!(password::validate("orbital8").is_ok());
    // Count characters, not UTF-8 bytes, at the same seven/eight boundary.
    assert!(password::validate("orbita🌙").is_err());
    assert!(password::validate("orbital🌙").is_ok());
}

#[test]
fn passwords_are_salted_argon2id_not_plaintext_or_fast_hashes() {
    let secret = "A long passphrase with spaces 🪐";
    password::validate(secret).unwrap();
    let first = password::hash(Zeroizing::new(secret.into())).unwrap();
    let second = password::hash(Zeroizing::new(secret.into())).unwrap();
    assert!(first.starts_with("$argon2id$v=19$m=65536,t=3,p=1$"));
    assert_ne!(first, second, "each password gets a fresh random salt");
    assert!(!first.contains(secret));
    assert!(password::verify(Zeroizing::new(secret.into()), &first));
    assert!(!password::verify(
        Zeroizing::new("wrong passphrase".into()),
        &first
    ));
    assert!(!password::verify(
        Zeroizing::new(secret.into()),
        "invalid hash"
    ));
    assert!(password::validate("short").is_err());
    assert!(password::validate(&"a".repeat(129)).is_err());
    assert!(password::validate(&"ab".repeat(64)).is_ok());
    assert!(
        password::validate("                ").is_err(),
        "trivial repeated characters are not strong passwords"
    );
    assert!(password::validate("passwordpassword").is_err());
    assert!(password::validate("   spaces stay meaningful   ").is_ok());
}

#[test]
fn cookies_and_origins_protect_account_and_socket_requests() {
    let config = AuthConfig::for_origin("https://stellar.example").unwrap();
    let cookie = config
        .cookie(&random_token(), SESSION_SECONDS)
        .to_str()
        .unwrap()
        .to_string();
    for expected in [
        "__Host-ss_session=",
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=86400",
    ] {
        assert!(cookie.contains(expected));
    }
    assert!(!cookie.contains("Domain="));
    assert!(AuthConfig::for_origin("http://stellar.example").is_err());
    assert!(AuthConfig::for_origin("https://user:pass@stellar.example").is_err());
    assert!(AuthConfig::for_origin("https://stellar.example/other").is_err());
    let local = AuthConfig::for_origin("http://localhost:8080").unwrap();
    assert!(!local.cookie("", 0).to_str().unwrap().contains("Secure"));
    let mut headers = HeaderMap::new();
    assert!(config.check_origin(&headers).is_err());
    for origin in [
        "null",
        "https://evil.example",
        "https://stellar.example.evil",
        "http://stellar.example",
    ] {
        headers.insert(header::ORIGIN, origin.parse().unwrap());
        assert!(config.check_origin(&headers).is_err());
    }
    headers.insert(header::ORIGIN, "https://stellar.example".parse().unwrap());
    config.check_origin(&headers).unwrap();
    let token = random_token();
    headers.insert(
        header::COOKIE,
        format!("__Host-ss_session={token}").parse().unwrap(),
    );
    assert_eq!(
        config.token(&headers).unwrap().as_slice(),
        Sha256::digest(token.as_bytes()).as_slice()
    );
    headers.insert(
        header::COOKIE,
        format!("__Host-ss_session={token}; __Host-ss_session={token}")
            .parse()
            .unwrap(),
    );
    assert!(
        config.token(&headers).is_err(),
        "ambiguous cookies are not accepted"
    );
    assert_eq!(
        normalize_login("  Pilot@Example.com ").unwrap(),
        "pilot@example.com"
    );
    for _ in 0..100 {
        let id = random_player_id();
        assert!(id > 0 && !PlayerId(id as u64).is_sentinel());
    }
}

#[test]
fn brute_force_budget_expires_and_cannot_grow_without_bound() {
    let mut limits = RateLimits::default();
    let now = Instant::now();
    for _ in 0..12 {
        limits.take("pilot".into(), 12, now).unwrap();
    }
    assert!(matches!(
        limits.take("pilot".into(), 12, now),
        Err(AuthError::Throttled)
    ));
    limits.take("pilot".into(), 12, now + RATE_WINDOW).unwrap();
    for n in 1..MAX_RATE_KEYS {
        limits.take(n.to_string(), 1, now + RATE_WINDOW).unwrap();
    }
    assert!(
        limits
            .take("overflow".into(), 1, now + RATE_WINDOW)
            .is_err()
    );
    assert_eq!(limits.keys.len(), MAX_RATE_KEYS);
}

async fn api(
    app: &Router,
    path: &str,
    method: &str,
    origin: Option<&str>,
    cookie: Option<&str>,
    body: Value,
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("/api/account/{path}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-stellar-client", "1");
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let mut request = request
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:12345".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 16_384).await.unwrap();
    (
        status,
        headers,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn cookie(headers: &HeaderMap) -> String {
    headers[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// Actual PostgreSQL + actual auth endpoints and authenticated binary sockets.
/// Uses only a freshly generated, isolated schema; never truncates a galaxy or
/// shared accounts. Run explicitly with TEST_ACCOUNTS_DATABASE_URL set.
#[tokio::test]
#[ignore = "requires TEST_ACCOUNTS_DATABASE_URL; creates its own isolated test schema"]
async fn postgres_accounts_survive_restart_and_authenticate_every_socket() {
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message, client::IntoClientRequest},
    };
    let url = std::env::var("TEST_ACCOUNTS_DATABASE_URL").expect("set TEST_ACCOUNTS_DATABASE_URL");
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!("account_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let statement = search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let config = AuthConfig::for_origin("http://localhost:8080").unwrap();
    let store = AuthStore::new(pool.clone(), config.clone()).await.unwrap();
    let app = routes(store.clone());
    let origin = Some("http://localhost:8080");
    let credentials = json!({"login":"Pilot@Example.com", "password":"my spacious stellar passphrase", "corporation_name":"Secure Corp"});

    assert_eq!(
        api(&app, "register", "POST", None, None, credentials.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        api(
            &app,
            "register",
            "POST",
            Some("https://evil.example"),
            None,
            credentials.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, headers, account) =
        api(&app, "register", "POST", origin, None, credentials.clone()).await;
    assert_eq!(status, StatusCode::OK, "{account}");
    let first_cookie = cookie(&headers);
    assert_eq!(headers[header::CACHE_CONTROL], "no-store");
    assert_eq!(account["corporation_name"], "Secure Corp");
    assert_eq!(
        account.as_object().unwrap().len(),
        3,
        "no password/token/login leaked in response"
    );
    let (stored_hash, player_id): (String, i64) =
        sqlx::query_as("SELECT password_hash,player_id FROM accounts")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(stored_hash.starts_with("$argon2id$"));
    assert_ne!(
        PlayerId(player_id as u64),
        crate::protocol::player_id_from_name("Secure Corp")
    );
    let (token_hash,): (Vec<u8>,) = sqlx::query_as("SELECT token_hash FROM account_sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(token_hash.len(), 32);
    assert_ne!(
        token_hash,
        first_cookie.split('=').nth(1).unwrap().as_bytes()
    );
    let duplicate = json!({"login":"pilot@example.com", "password":"a different long passphrase", "corporation_name":"New Corp"});
    assert_eq!(
        api(&app, "register", "POST", origin, None, duplicate)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let wrong = json!({"login":"pilot@example.com", "password":"not the actual password"});
    let unknown = json!({"login":"absent@example.com", "password":"not the actual password"});
    let wrong = api(&app, "login", "POST", origin, None, wrong).await;
    let unknown = api(&app, "login", "POST", origin, None, unknown).await;
    assert_eq!(
        (wrong.0, wrong.2),
        (unknown.0, unknown.2),
        "no account enumeration by response"
    );
    assert_eq!(
        api(&app, "session", "GET", None, None, Value::Null).await.0,
        StatusCode::UNAUTHORIZED
    );

    // A fresh store (the restart boundary) resumes the durable account/session.
    let restarted = AuthStore::new(pool.clone(), config).await.unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let (_status_tx, status_rx) = watch::channel(ServerStatus::default());
    let full_app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .with_state(AppState {
            game: GameHandle::new(tx),
            status: status_rx,
            auth: restarted.clone(),
        })
        .merge(routes(restarted.clone()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_url = format!("ws://{}/ws", listener.local_addr().unwrap());
    let serving = full_app.clone();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            serving.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let socket_request = |session_cookie: Option<&str>, origin: &str| {
        let mut req = ws_url.clone().into_client_request().unwrap();
        req.headers_mut()
            .insert("Sec-WebSocket-Protocol", wire::SUBPROTOCOL.parse().unwrap());
        req.headers_mut()
            .insert(header::ORIGIN, origin.parse().unwrap());
        if let Some(cookie) = session_cookie {
            req.headers_mut()
                .insert(header::COOKIE, cookie.parse().unwrap());
        }
        req
    };
    for request in [
        socket_request(None, origin.unwrap()),
        socket_request(Some(&first_cookie), "https://evil.example"),
    ] {
        assert!(
            connect_async(request).await.is_err(),
            "anonymous/cross-site WS must not upgrade"
        );
    }
    assert!(rx.try_recv().is_err());
    let (mut socket, _) = connect_async(socket_request(Some(&first_cookie), origin.unwrap()))
        .await
        .unwrap();
    let mut join = vec![b'S', b'S', 0, crate::protocol::PROTOCOL_VERSION as u8];
    join.extend(
        rmp_serde::to_vec_named(
            &json!({"type":"Join", "name":"Somebody else's corporation", "view_hz":10}),
        )
        .unwrap(),
    );
    socket.send(Message::Binary(join.into())).await.unwrap();
    let input = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let GameInput::Connect {
        player_id: authenticated_id,
        name,
        outbound,
        view_tx,
        replace_tx,
        ..
    } = input
    else {
        panic!("missing authenticated connect")
    };
    assert_eq!(authenticated_id, PlayerId(player_id as u64));
    assert_eq!(
        name, "Secure Corp",
        "Join.name cannot steal or rename a corporation"
    );

    let login = json!({"login":"PILOT@example.com", "password":"my spacious stellar passphrase"});
    let signed_in = api(&full_app, "login", "POST", origin, None, login.clone()).await;
    assert_eq!(signed_in.0, StatusCode::OK);
    assert_eq!(signed_in.2, account);
    let second_cookie = cookie(&signed_in.1);
    assert_ne!(second_cookie, first_cookie, "login rotates the token");
    let close = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(close, Message::Close(Some(c)) if u16::from(c.code)==AUTH_REQUIRED_CLOSE_CODE),
        "new login immediately revokes the old socket"
    );
    drop((outbound, view_tx, replace_tx));
    assert_eq!(
        api(
            &full_app,
            "session",
            "GET",
            None,
            Some(&first_cookie),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        api(
            &full_app,
            "session",
            "GET",
            None,
            Some(&second_cookie),
            Value::Null
        )
        .await
        .2,
        account
    );
    assert_eq!(
        api(
            &full_app,
            "logout",
            "POST",
            origin,
            Some(&first_cookie),
            json!({})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        api(
            &full_app,
            "session",
            "GET",
            None,
            Some(&second_cookie),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK,
        "stale logout cannot revoke a newer login"
    );
    let mut headers = HeaderMap::new();
    headers.insert(header::COOKIE, second_cookie.parse().unwrap());
    let lease = restarted.authenticate(&headers).await.unwrap();
    assert_eq!(
        api(
            &full_app,
            "logout",
            "POST",
            origin,
            Some(&second_cookie),
            json!({})
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(
        *lease.revoked.borrow(),
        "logout revokes existing leases, not just future handshakes"
    );
    assert_eq!(
        api(
            &full_app,
            "session",
            "GET",
            None,
            Some(&second_cookie),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );

    let signed_in = api(&full_app, "login", "POST", origin, None, login.clone()).await;
    assert_eq!(signed_in.0, StatusCode::OK);
    let expired_cookie = cookie(&signed_in.1);
    sqlx::query("UPDATE account_sessions SET expires_at=now()+interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    let (mut expiring_socket, _) =
        connect_async(socket_request(Some(&expired_cookie), origin.unwrap()))
            .await
            .unwrap();
    let close = tokio::time::timeout(Duration::from_secs(5), expiring_socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        matches!(close, Message::Close(Some(c)) if u16::from(c.code)==AUTH_REQUIRED_CLOSE_CODE),
        "session lifetime is enforced even on an already-open, idle socket"
    );
    assert_eq!(
        api(
            &full_app,
            "session",
            "GET",
            None,
            Some(&expired_cookie),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("UPDATE accounts SET disabled=true")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        api(&full_app, "login", "POST", origin, None, login).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (accounts_count,): (i64,) = sqlx::query_as("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        accounts_count, 1,
        "session expiry/logout never delete accounts"
    );
    server.abort();
    pool.close().await;
    // The identifier is generated above from a UUID, never user/database input.
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

/// No shared accounts are mutated: this test owns a UUID-named schema and drops
/// only that schema. Exercises the real migration, routes and cookie rotation.
#[tokio::test]
#[ignore = "requires TEST_ACCOUNTS_DATABASE_URL; creates its own isolated test schema"]
async fn postgres_guests_register_in_place_without_claiming_other_players() {
    let url = std::env::var("TEST_ACCOUNTS_DATABASE_URL").expect("set TEST_ACCOUNTS_DATABASE_URL");
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!("guest_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _| {
            let statement = search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(conn).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let config = AuthConfig::for_origin("http://localhost:8080").unwrap();
    let store = AuthStore::new(pool.clone(), config.clone()).await.unwrap();
    let app = routes(store.clone());
    let origin = Some("http://localhost:8080");
    for bad_origin in [None, Some("https://evil.example")] {
        assert_eq!(
            api(&app, "guest", "POST", bad_origin, None, json!({}))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let (status, headers, guest) = api(&app, "guest", "POST", origin, None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(guest["is_guest"], true);
    assert!(
        guest["corporation_name"]
            .as_str()
            .unwrap()
            .starts_with("Venture ")
    );
    assert_eq!(
        guest.as_object().unwrap().len(),
        3,
        "no credential or player ID exposed"
    );
    assert!(
        headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=2592000")
    );
    let guest_cookie = cookie(&headers);
    let guest_id: Uuid = guest["id"].as_str().unwrap().parse().unwrap();
    let (player_id, login, hash): (i64, Option<String>, Option<String>) =
        sqlx::query_as("SELECT player_id,login,password_hash FROM accounts WHERE id=$1")
            .bind(guest_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        login.is_none() && hash.is_none(),
        "no generated or shared password can claim a guest"
    );
    let (ttl,): (f64,) = sqlx::query_as("SELECT extract(epoch FROM (expires_at-created_at))::float8 FROM account_sessions WHERE account_id=$1")
        .bind(guest_id).fetch_one(&pool).await.unwrap();
    assert!((ttl - GUEST_SESSION_SECONDS as f64).abs() < 1.0);
    let resumed = api(
        &app,
        "guest",
        "POST",
        origin,
        Some(&guest_cookie),
        json!({}),
    )
    .await;
    assert_eq!(
        resumed.2, guest,
        "retry resumes instead of orphaning a corporation"
    );
    assert!(!resumed.1.contains_key(header::SET_COOKIE));
    let other = api(&app, "guest", "POST", origin, None, json!({})).await;
    let other_cookie = cookie(&other.1);
    assert_ne!(other.2["id"], guest["id"]);
    let claim = json!({"login":"MyPilot", "password":"orbital8"});
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            None,
            claim.clone()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            Some("https://evil.example"),
            Some(&guest_cookie),
            claim.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            Some(&guest_cookie),
            json!({"login":"MyPilot", "password":"orbital"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            Some(&guest_cookie),
            json!({"login":"MyPilot", "password":"orbital8", "id":other.2["id"]})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    // Reserving a used login refuses the claim atomically: guest/session intact.
    let registered = api(
        &app,
        "register",
        "POST",
        origin,
        None,
        json!({"login":"taken", "password":"another8", "corporation_name":"Existing Corporation"}),
    )
    .await;
    assert_eq!(registered.0, StatusCode::OK);
    assert_eq!(registered.2["is_guest"], false);
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            Some(&guest_cookie),
            json!({"login":"taken", "password":"orbital8"})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        api(
            &app,
            "session",
            "GET",
            None,
            Some(&guest_cookie),
            Value::Null
        )
        .await
        .2,
        guest
    );
    let mut auth_headers = HeaderMap::new();
    auth_headers.insert(header::COOKIE, guest_cookie.parse().unwrap());
    let lease = store.authenticate(&auth_headers).await.unwrap();
    let upgraded = api(
        &app,
        "complete-registration",
        "POST",
        origin,
        Some(&guest_cookie),
        claim.clone(),
    )
    .await;
    assert_eq!(upgraded.0, StatusCode::OK, "{}", upgraded.2);
    assert_eq!(upgraded.2["id"], guest["id"]);
    assert_eq!(upgraded.2["corporation_name"], guest["corporation_name"]);
    assert_eq!(upgraded.2["is_guest"], false);
    let registered_cookie = cookie(&upgraded.1);
    assert_ne!(registered_cookie, guest_cookie);
    assert!(
        *lease.revoked.borrow(),
        "claim immediately revokes old socket leases"
    );
    assert_eq!(
        api(
            &app,
            "session",
            "GET",
            None,
            Some(&guest_cookie),
            Value::Null
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            Some(&guest_cookie),
            claim.clone()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        api(
            &app,
            "complete-registration",
            "POST",
            origin,
            Some(&registered_cookie),
            claim.clone()
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (same_player, hash): (i64, String) =
        sqlx::query_as("SELECT player_id,password_hash FROM accounts WHERE id=$1")
            .bind(guest_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        same_player, player_id,
        "World ownership/progress uses this unchanged player ID"
    );
    assert!(hash.starts_with("$argon2id$"));
    assert!(password::verify(Zeroizing::new("orbital8".into()), &hash));
    assert_eq!(
        api(
            &app,
            "session",
            "GET",
            None,
            Some(&other_cookie),
            Value::Null
        )
        .await
        .2,
        other.2
    );

    // A new store represents server restart; registration and identity persist.
    let restarted = AuthStore::new(pool.clone(), config.clone()).await.unwrap();
    let restarted_app = routes(restarted.clone());
    let signed_in = api(&restarted_app, "login", "POST", origin, None, claim.clone()).await;
    assert_eq!(signed_in.0, StatusCode::OK);
    assert_eq!(signed_in.2, upgraded.2);
    let signed_in_cookie = cookie(&signed_in.1);
    auth_headers.insert(header::COOKIE, signed_in_cookie.parse().unwrap());
    assert_eq!(
        restarted
            .authenticate(&auth_headers)
            .await
            .unwrap()
            .account
            .player_id(),
        PlayerId(player_id as u64)
    );
    assert_eq!(
        api(
            &restarted_app,
            "guest",
            "POST",
            origin,
            Some(&signed_in_cookie),
            json!({})
        )
        .await
        .2,
        upgraded.2,
        "guest entry never replaces a registered session"
    );
    assert_eq!(
        api(
            &restarted_app,
            "session",
            "GET",
            None,
            Some(&other_cookie),
            Value::Null
        )
        .await
        .2,
        other.2,
        "unregistered guests survive restart too"
    );
    let other_id: Uuid = other.2["id"].as_str().unwrap().parse().unwrap();
    sqlx::query(
        "UPDATE account_sessions SET expires_at=now()-interval '1 second' WHERE account_id=$1",
    )
    .bind(other_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        api(
            &restarted_app,
            "complete-registration",
            "POST",
            origin,
            Some(&other_cookie),
            json!({"login":"stale-claim", "password":"orbital8"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (still_guest,): (bool,) = sqlx::query_as("SELECT is_guest FROM accounts WHERE id=$1")
        .bind(other_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(still_guest);
    let disabled = api(&restarted_app, "guest", "POST", origin, None, json!({})).await;
    let disabled_id: Uuid = disabled.2["id"].as_str().unwrap().parse().unwrap();
    sqlx::query("UPDATE accounts SET disabled=true WHERE id=$1")
        .bind(disabled_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        api(
            &restarted_app,
            "complete-registration",
            "POST",
            origin,
            Some(&cookie(&disabled.1)),
            json!({"login":"disabled-claim", "password":"orbital8"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (count,): (i64,) = sqlx::query_as("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 4, "claims and resumes never create another player");

    let limited = routes(AuthStore::new(pool.clone(), config).await.unwrap());
    for _ in 0..12 {
        assert_eq!(
            api(&limited, "guest", "POST", origin, None, json!({}))
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        api(&limited, "guest", "POST", origin, None, json!({}))
            .await
            .0,
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(
        api(
            &limited,
            "guest",
            "POST",
            origin,
            Some(&signed_in_cookie),
            json!({})
        )
        .await
        .0,
        StatusCode::OK,
        "creation limits do not block resuming an existing account"
    );
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
