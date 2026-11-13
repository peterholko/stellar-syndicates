use super::*;
use axum::{Router, routing::get};
use serde_json::{Value, json};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as Frame, client::IntoClientRequest},
};

fn order(value: Value) -> Frame {
    let mut bytes = vec![b'S', b'S', 0, crate::protocol::PROTOCOL_VERSION as u8];
    bytes.extend(rmp_serde::to_vec_named(&value).unwrap());
    Frame::Binary(bytes.into())
}

// These codec/lifecycle tests start AFTER authentication. Actual HTTP/DB
// authorization is exercised separately in auth::tests, with no bypass route
// compiled into the production server.
fn transport_app(handle: GameHandle) -> Router {
    let (session, revoke) = crate::auth::transport_test_session();
    Router::new().route("/ws", get(move |ws: WebSocketUpgrade, headers: HeaderMap| {
        let session = AuthSession { account: session.account.clone(), expires_at: session.expires_at, revoked: session.revoked.clone() };
        let handle = handle.clone();
        let revoke = revoke.clone();
        async move { ws.protocols([accepted_protocol(&headers)]).on_upgrade(move |socket| async move {
            let _keep_auth_live = revoke;
            handle_socket(socket, handle, session, None).await;
        }) }
    }))
}

#[tokio::test]
async fn binary_socket_preserves_reliable_order_views_and_reconnect_lifecycle() {
    timeout(Duration::from_secs(10), async {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let app = transport_app(GameHandle::new(tx));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let request = || {
            let mut req = url.clone().into_client_request().unwrap();
            req.headers_mut().insert("Sec-WebSocket-Protocol", wire::SUBPROTOCOL.parse().unwrap());
            req
        };
        let (mut socket, response) = connect_async(request()).await.unwrap();
        assert_eq!(response.headers()["Sec-WebSocket-Protocol"], wire::SUBPROTOCOL);
        socket.send(order(json!({"type":"Join", "name":"Socket test", "view_hz":5}))).await.unwrap();
        let GameInput::Connect { conn_id, player_id, name, outbound, view_tx, replace_tx, view_divisor, .. } = rx.recv().await.unwrap() else { panic!("missing join") };
        assert_eq!(player_id, sim::PlayerId(1234));
        assert_eq!(name, "Socket test");
        assert_eq!(view_divisor, 2);

        for message in ["view old", "view newest"] {
            view_tx.send(Some(ServerMsg::Error { message: message.into() })).unwrap();
        }
        for message in ["reliable A", "reliable B"] {
            outbound.try_send(ServerMsg::Error { message: message.into() }).unwrap();
        }
        let mut reliable = Vec::new();
        let mut newest = 0;
        for _ in 0..3 {
            let Frame::Binary(bytes) = socket.next().await.unwrap().unwrap() else { panic!("nonbinary application frame") };
            assert_eq!(&bytes[..4], &[b'S', b'S', 0, crate::protocol::PROTOCOL_VERSION as u8]);
            let value: Value = rmp_serde::from_slice(&bytes[4..]).unwrap();
            let message = value["message"].as_str().unwrap();
            if message == "view newest" { newest += 1; } else { reliable.push(message.to_string()); }
        }
        assert_eq!(newest, 1);
        assert_eq!(reliable, ["reliable A", "reliable B"]);

        socket.send(order(json!({"type":"MoveShip","ship_id":"18446744073709551615","dest":{"x":123.75,"y":-50.125}}))).await.unwrap();
        let GameInput::Intent { conn_id: sender, msg: ClientMsg::MoveShip { ship_id, dest } } = rx.recv().await.unwrap() else { panic!("binary intent lost") };
        assert_eq!(sender, conn_id);
        assert_eq!(ship_id, sim::EntityId(u64::MAX));
        assert_eq!(dest, sim::Vec2::new(123.75, -50.125));

        replace_tx.send(true).unwrap();
        let Frame::Close(Some(close)) = socket.next().await.unwrap().unwrap() else { panic!("replacement must close") };
        assert_eq!(u16::from(close.code), SESSION_REPLACED_CLOSE_CODE);
        assert!(matches!(rx.recv().await.unwrap(), GameInput::Disconnect { conn_id: id } if id == conn_id));
        drop((outbound, view_tx, replace_tx, socket));

        // Fresh socket negotiates again and gets fresh connection/cursor state.
        let (mut socket, _) = connect_async(request()).await.unwrap();
        socket.send(order(json!({"type":"Join","name":"Socket test","view_hz":10}))).await.unwrap();
        let GameInput::Connect { conn_id: fresh, view_divisor, .. } = rx.recv().await.unwrap() else { panic!("rejoin lost") };
        assert_ne!(fresh, conn_id);
        assert_eq!(view_divisor, 1);
        let _ = socket.close(None).await;
        server.abort();
    }).await.expect("binary socket test timed out");
}

#[tokio::test]
async fn legacy_text_client_is_closed_before_joining_the_game() {
    timeout(Duration::from_secs(5), async {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let app = transport_app(GameHandle::new(tx));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let (mut socket, _) = connect_async(url).await.unwrap();
        let Frame::Close(Some(close)) = socket.next().await.unwrap().unwrap() else {
            panic!("old client not rejected")
        };
        assert_eq!(u16::from(close.code), wire::PROTOCOL_CLOSE_CODE);
        assert!(
            rx.try_recv().is_err(),
            "legacy client must never register a corporation"
        );
        server.abort();
    })
    .await
    .expect("legacy rejection test timed out");
}

/// §client-updates (Nov 11 2026): a tab left open across a protocol bump
/// offers its old protocol. The handshake names it, so every browser opens the
/// socket, and the server closes it at once with the reload code.
#[tokio::test]
async fn an_old_tab_is_told_to_reload_instead_of_failing_its_handshake() {
    timeout(Duration::from_secs(5), async {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let app = transport_app(GameHandle::new(tx));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/ws", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let old = format!("stellar.msgpack.v{}", crate::protocol::PROTOCOL_VERSION - 1);
        let mut req = url.into_client_request().unwrap();
        req.headers_mut().insert("Sec-WebSocket-Protocol", old.parse().unwrap());
        let (mut socket, response) = connect_async(req).await.unwrap();
        assert_eq!(response.headers()["Sec-WebSocket-Protocol"], old.as_str(), "its own protocol, so the browser opens the socket");
        let Frame::Close(Some(close)) = socket.next().await.unwrap().unwrap() else { panic!("an old tab must be closed") };
        assert_eq!(u16::from(close.code), wire::PROTOCOL_CLOSE_CODE);
        assert!(close.reason.contains("reload"), "{}", close.reason);
        assert!(rx.try_recv().is_err(), "an old tab never joins the game");
        server.abort();
    }).await.expect("old tab test timed out");
}

#[test]
fn the_handshake_names_the_current_protocol_or_an_old_tab_s_own() {
    let offer = |value: &str| {
        let mut headers = HeaderMap::new();
        headers.insert("Sec-WebSocket-Protocol", value.parse().unwrap());
        accepted_protocol(&headers).into_owned()
    };
    assert_eq!(accepted_protocol(&HeaderMap::new()), wire::SUBPROTOCOL, "no offer: the current one, which no client named");
    assert_eq!(offer(wire::SUBPROTOCOL), wire::SUBPROTOCOL);
    assert_eq!(offer(&format!("stellar.msgpack.v12, {}", wire::SUBPROTOCOL)), wire::SUBPROTOCOL, "the current one wins");
    assert_eq!(offer("stellar.msgpack.v90"), "stellar.msgpack.v90");
    for foreign in ["chat", "stellar.msgpack.v", "stellar.msgpack.v9x", "stellar.msgpack.v123456", "stellar.json.v90"] {
        assert_eq!(offer(foreign), wire::SUBPROTOCOL, "{foreign}: never echoed");
    }
}

