use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::Duration;

use axum::{
    body::{Body, to_bytes},
    extract::ConnectInfo,
    http::Request,
};
use futures::SinkExt;
use serde_json::{Value, json};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::mpsc;
use tower::ServiceExt;

use super::*;
use crate::session::GameInput;
use crate::tutorial::{Step, TutorialProgress};
use crate::{AppState, auth, wire, ws};

#[test]
fn operator_logins_match_how_sign_in_normalizes_them() {
    let operators = operators_from(" Pilot@Example.com, ,ops@example.com ");
    assert!(operators.contains("pilot@example.com"));
    assert!(operators.contains("ops@example.com"));
    assert_eq!(operators.len(), 2);
    assert!(operators_from("").is_empty());
    assert!(operators_from(" , ").is_empty());
}

const ORIGIN: &str = "http://localhost:8080";
const VISITOR_IP: &str = "203.0.113.9";

/// One request through the router as if from the local tunnel connector.
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN)
        .header("x-stellar-client", "1")
        .header(header::USER_AGENT, "AdminTest/1.0")
        .header("cf-connecting-ip", VISITOR_IP)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    let mut request = request
        .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
        .unwrap();
    request.extensions_mut().insert(ConnectInfo(
        "127.0.0.1:40000".parse::<SocketAddr>().unwrap(),
    ));
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, headers, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// `name=value` pairs from every Set-Cookie, as a Cookie header.
fn cookies(headers: &HeaderMap) -> String {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap().split(';').next().unwrap().to_owned())
        .collect::<Vec<_>>()
        .join("; ")
}

fn device(cookie_header: &str) -> String {
    cookie_header
        .split("; ")
        .find_map(|pair| pair.strip_prefix("ss_device="))
        .expect("sign-in issues a device cookie")
        .to_owned()
}

async fn register(app: &Router, login: &str, corp: &str, cookie: Option<&str>) -> String {
    let (status, headers, body) = call(
        app,
        "POST",
        "/api/account/register",
        cookie,
        Some(json!({"login": login, "password": "a long enough passphrase", "corporation_name": corp})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    cookies(&headers)
}

/// The real migration, auth routes, socket and admin API against PostgreSQL,
/// with a stand-in game loop. Owns a UUID-named schema and drops only that.
#[tokio::test]
#[ignore = "requires TEST_ACCOUNTS_DATABASE_URL; creates its own isolated test schema"]
async fn postgres_admin_dashboard_records_sign_ins_play_commands_and_the_charter() {
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message, client::IntoClientRequest},
    };
    let url = std::env::var("TEST_ACCOUNTS_DATABASE_URL").expect("set TEST_ACCOUNTS_DATABASE_URL");
    let admin_db = PgPool::connect(&url).await.unwrap();
    let schema = format!("admin_test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}")).execute(&admin_db).await.unwrap();
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

    let store = auth::AuthStore::new_for_test(pool.clone(), ORIGIN).await;
    let activity = Activity::start(pool.clone()).await.unwrap();
    let recorder = TutorialRecorder::new(pool.clone());
    // A stand-in loop: it answers the dashboard and ignores gameplay. Its
    // galaxy has run for an hour of game time at real-time pacing.
    let holdings: Arc<Mutex<Vec<CorporationStats>>> = Arc::default();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let answers = holdings.clone();
    tokio::spawn(async move {
        while let Some(input) = rx.recv().await {
            if let GameInput::AdminStats { reply } = input {
                let _ = reply.send(AdminSnapshot {
                    galaxy_id: "test-galaxy".into(),
                    sim_time: 3600.0,
                    pacing: 1.0,
                    corporations: answers.lock().unwrap().clone(),
                });
            }
        }
    });
    let game = GameHandle::new(tx);
    let (_status_tx, status_rx) = watch::channel(ServerStatus::default());
    let app_with = |operators: &str| {
        Router::new()
            .route("/ws", get(ws::ws_handler))
            .with_state(AppState {
                game: game.clone(),
                status: status_rx.clone(),
                auth: store.clone(),
                activity: activity.clone(),
            })
            .merge(auth::routes(store.clone()))
            .merge(routes(AdminState::new(
                store.clone(),
                activity.clone(),
                game.clone(),
                status_rx.clone(),
                recorder.clone(),
                operators_from(operators),
            )))
    };
    let app = app_with("Ops@Example.com");

    let ops = register(&app, "ops@example.com", "Operations", None).await;
    let pilot = register(&app, "pilot@example.com", "Pilot Corp", None).await;
    let pilot_device = device(&pilot);
    let (player_id,): (i64,) = sqlx::query_as("SELECT player_id FROM accounts WHERE login='pilot@example.com'")
        .fetch_one(&pool).await.unwrap();
    holdings.lock().unwrap().extend([
        CorporationStats {
            player_id: PlayerId(player_id as u64),
            name: "Pilot Corp".into(),
            bot: false,
            home: Some("Veles".into()),
            systems: 2,
            worlds: 3,
            population: 1.5,
            fleets: 4,
            ships: 11,
            credits: 2500.0,
            valuation: 9000.0,
            // Founded ten game-minutes ago; the mine and the first shipment
            // since; the privateer fight is next.
            tutorial: Some(TutorialProgress { reached: vec![
                (Step::Founded, Some(3000.0)),
                (Step::Mine, Some(3300.0)),
                (Step::Shipment, Some(3420.0)),
            ] }),
        },
        CorporationStats {
            player_id: PlayerId(77),
            name: "Headless".into(),
            bot: true,
            home: None,
            systems: 1,
            worlds: 1,
            population: 0.2,
            fleets: 1,
            ships: 1,
            credits: 10.0,
            valuation: 10.0,
            tutorial: Some(TutorialProgress { reached: vec![(Step::Founded, Some(0.0))] }),
        },
    ]);

    // Only a signed-in operator reads anything; with no operators it is off.
    assert_eq!(call(&app, "GET", "/api/admin/overview", None, None).await.0, StatusCode::UNAUTHORIZED);
    let refused = call(&app, "GET", "/api/admin/overview", Some(&pilot), None).await;
    assert_eq!((refused.0, refused.2["code"].as_str()), (StatusCode::FORBIDDEN, Some("not_operator")));
    let off = call(&app_with(""), "GET", "/api/admin/overview", Some(&ops), None).await;
    assert_eq!((off.0, off.2["code"].as_str()), (StatusCode::NOT_FOUND, Some("off")));

    // The pilot plays: Join, two commands, a keepalive, then leaves.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ws_url = format!("ws://{}/ws", listener.local_addr().unwrap());
    let serving = app.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, serving.into_make_service_with_connect_info::<SocketAddr>())
            .await
            .unwrap();
    });
    let mut request = ws_url.into_client_request().unwrap();
    for (name, value) in [
        ("Sec-WebSocket-Protocol", wire::SUBPROTOCOL),
        ("Origin", ORIGIN),
        ("Cookie", pilot.as_str()),
        ("cf-connecting-ip", VISITOR_IP),
        ("User-Agent", "AdminTest/1.0"),
    ] {
        request.headers_mut().insert(name, value.parse().unwrap());
    }
    let (mut socket, _) = connect_async(request).await.unwrap();
    let frame = |value: Value| {
        let mut bytes = vec![b'S', b'S', 0, crate::protocol::PROTOCOL_VERSION as u8];
        bytes.extend(rmp_serde::to_vec_named(&value).unwrap());
        Message::Binary(bytes.into())
    };
    socket.send(frame(json!({"type": "Join", "name": "Pilot Corp"}))).await.unwrap();
    for _ in 0..2 {
        socket.send(frame(json!({"type": "HoldFleet", "ship_id": "7"}))).await.unwrap();
    }
    socket.send(frame(json!({"type": "Ping"}))).await.unwrap();
    let mut online = false;
    for _ in 0..100 {
        let overview = call(&app, "GET", "/api/admin/overview", Some(&ops), None).await.2;
        let pilot_row = overview["players"].as_array().unwrap().iter()
            .find(|p| p["login"] == "pilot@example.com").unwrap().clone();
        if pilot_row["online"]["actions"] == 2 {
            assert_eq!(pilot_row["online"]["ip"], VISITOR_IP);
            assert_eq!(overview["totals"]["online"], 1);
            online = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(online, "the live socket and its commands show as online");
    socket.close(None).await.unwrap();
    drop(socket);
    let mut closed = false;
    for _ in 0..100 {
        let row: Option<(i32, bool)> = sqlx::query_as("SELECT actions, disconnected_at IS NOT NULL FROM play_sessions")
            .fetch_optional(&pool).await.unwrap();
        if row == Some((2, true)) {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(closed, "the session closes with both commands written");

    // An alt account registered in the pilot's browser shares its device.
    register(&app, "alt@example.com", "Alt Corp", Some(&format!("ss_device={pilot_device}"))).await;

    let (status, _, overview) = call(&app, "GET", "/api/admin/overview", Some(&ops), None).await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["totals"]["accounts"], 3);
    assert_eq!(overview["totals"]["online"], 0);
    assert_eq!(overview["totals"]["founded"], 1);
    assert_eq!(overview["server"]["holdings"], true);
    let row = overview["players"].as_array().unwrap().iter()
        .find(|p| p["login"] == "pilot@example.com").unwrap().clone();
    assert_eq!(row["corporation"], "Pilot Corp");
    assert_eq!(row["logins"], 1);
    assert_eq!(row["sessions"], 1);
    assert_eq!(row["actions"], 2);
    assert_eq!(row["active_seconds"], crate::activity::ACTIVE_WINDOW_S);
    assert_eq!(row["last_ip"], VISITOR_IP);
    assert_eq!(row["last_device"], pilot_device.as_str());
    assert_eq!(row["last_user_agent"], "AdminTest/1.0");
    assert_eq!(row["shared_accounts"], 1);
    assert!(row["online"].is_null());
    assert_eq!(row["game"]["ships"], 11);
    assert_eq!(row["game"]["worlds"], 3);
    assert_eq!(row["game"]["credits"], 2500.0);
    assert!(row["game"].get("player_id").is_none(), "player ids stay server-side");
    let unlinked = overview["unlinked"].as_array().unwrap();
    assert_eq!(unlinked.len(), 1);
    assert_eq!((unlinked[0]["name"].as_str(), unlinked[0]["bot"].as_bool()), (Some("Headless"), Some(true)));

    let account_id = row["account_id"].as_str().unwrap();
    let (status, _, detail) = call(&app, "GET", &format!("/api/admin/players/{account_id}"), Some(&ops), None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["logins"][0]["kind"], "register");
    assert_eq!(detail["logins"][0]["ip"], VISITOR_IP);
    assert_eq!(detail["logins"][0]["device_id"], pilot_device.as_str());
    assert_eq!(detail["sessions"][0]["actions"], 2);
    assert_eq!(detail["commands"], json!([{"kind": "HoldFleet", "count": 2, "last_at": detail["commands"][0]["last_at"]}]));
    let recent = detail["recent"].as_array().unwrap();
    assert_eq!(recent.len(), 2, "keepalives are not commands");
    assert_eq!(recent[0]["kind"], "HoldFleet");
    assert_eq!(recent[0]["detail"], json!({"ship_id": "7"}));
    assert_eq!(detail["days"].as_array().unwrap().len(), 1);
    assert_eq!(detail["days"][0]["actions"], 2);
    let shared = detail["shared"].as_array().unwrap();
    assert!(shared.iter().any(|s| s["login"] == "alt@example.com" && s["via"] == "device"));
    assert!(shared.iter().any(|s| s["login"] == "alt@example.com" && s["via"] == "ip"));
    assert_eq!(
        call(&app, "GET", &format!("/api/admin/players/{}", Uuid::new_v4()), Some(&ops), None).await.0,
        StatusCode::NOT_FOUND
    );

    // The charter: recorded once per step, in wall time counted back from the
    // game clock, and only for accounts (never the bot).
    let recorded: Vec<(String, DateTime<Utc>, Option<f64>)> = sqlx::query_as(
        "SELECT step, reached_at, sim_time FROM tutorial_progress WHERE galaxy_id='test-galaxy' ORDER BY sim_time")
        .fetch_all(&pool).await.unwrap();
    assert_eq!(recorded.iter().map(|(step, _, _)| step.as_str()).collect::<Vec<_>>(), ["founded", "mine", "shipment"]);
    let ten_minutes_ago = Utc::now() - chrono::Duration::minutes(10);
    assert!((recorded[0].1 - ten_minutes_ago).num_seconds().abs() < 30, "founded ten game-minutes back");
    assert_eq!(recorded[2].2, Some(3420.0));
    let (galaxy_started,): (DateTime<Utc>,) = sqlx::query_as("SELECT started_at FROM galaxies WHERE id='test-galaxy'")
        .fetch_one(&pool).await.unwrap();
    assert!((galaxy_started - (Utc::now() - chrono::Duration::hours(1))).num_seconds().abs() < 30);

    let funnel = &overview["funnel"];
    assert_eq!(funnel["galaxy_id"], "test-galaxy");
    assert_eq!(funnel["players"], 2, "the pilot and the alt; the operator is left out");
    let stage = |key: &str| funnel["stages"].as_array().unwrap().iter().find(|s| s["key"] == key).unwrap().clone();
    assert_eq!(stage("signed_up")["reached"], 2);
    assert_eq!(stage("entered")["reached"], 1);
    assert_eq!(stage("entered")["waiting_active"], 1, "the alt signed up but has not played");
    assert_eq!(stage("mine")["reached"], 1);
    assert_eq!(stage("privateer")["reached"], 0);
    assert_eq!(stage("privateer")["waiting_active"], 1, "the pilot's shipment is under way");
    assert_eq!(stage("privateer")["label"], "Defeat the Rogue Privateer");
    assert_eq!(row["tutorial"]["next"], "privateer");
    assert_eq!((row["in_funnel"].as_bool(), row["active"].as_bool(), row["operator"].as_bool()), (Some(true), Some(true), Some(false)));
    let ops_row = overview["players"].as_array().unwrap().iter().find(|p| p["login"] == "ops@example.com").unwrap();
    assert_eq!((ops_row["operator"].as_bool(), ops_row["in_funnel"].as_bool()), (Some(true), Some(false)));
    assert_eq!((row["tutorial"]["done"].as_u64(), row["tutorial"]["total"].as_u64()), (Some(3), Some(10)));
    assert!(row["tutorial"].get("steps").is_none(), "the overview stays compact");
    let charter = &detail["player"]["tutorial"];
    assert_eq!(charter["steps"].as_array().unwrap().len(), 12);
    assert_eq!(charter["steps"][2]["key"], "founded");
    assert!(charter["steps"][2]["at"].is_string());
    assert!(charter["steps"][5]["at"].is_null(), "no privateer report yet");

    // Asking again writes nothing new.
    call(&app, "GET", "/api/admin/overview", Some(&ops), None).await;
    let (rows,): (i64,) = sqlx::query_as("SELECT count(*) FROM tutorial_progress").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 3);

    // A restart closes nothing twice and loses nothing written.
    activity.shutdown().await;
    Activity::start(pool.clone()).await.unwrap();
    let (open,): (i64,) = sqlx::query_as("SELECT count(*) FROM play_sessions WHERE disconnected_at IS NULL")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(open, 0);

    server.abort();
    pool.close().await;
    // The identifier is generated above from a UUID, never user/database input.
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE")).execute(&admin_db).await.unwrap();
    admin_db.close().await;
}
