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
    Router::new().route("/ws", get(move |ws: WebSocketUpgrade| {
        let session = AuthSession { account: session.account.clone(), expires_at: session.expires_at, revoked: session.revoked.clone() };
        let handle = handle.clone();
        let revoke = revoke.clone();
        async move { ws.protocols([wire::SUBPROTOCOL]).on_upgrade(move |socket| async move {
            let _keep_auth_live = revoke;
            handle_socket(socket, handle, session).await;
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
