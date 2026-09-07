use super::*;
use serde_json::{Value, json};

fn pack(value: &impl Serialize) -> Vec<u8> {
    let mut bytes = HEADER.to_vec();
    value
        .serialize(
            &mut rmp_serde::Serializer::new(&mut bytes)
                .with_struct_map()
                .with_human_readable(),
        )
        .unwrap();
    bytes
}

fn orders() -> Vec<Value> {
    vec![
        json!({"type":"Join", "name":"Étoile 星際 🚀", "view_hz":5}),
        json!({"type":"Join", "name":"Desktop"}),
        json!({"type":"MoveShip", "ship_id":"18446744073709551615", "dest":{"x":1234567.8901234567,"y":-0.00001234567890123}}),
        json!({"type":"JumpShip", "ship_id":"9007199254740993", "dest":{"x":-1234.5,"y":65536}}),
        json!({"type":"MarketBuy", "commodity":"alloys", "units":50, "max_unit_price":19.875}),
        json!({"type":"MarketSell", "commodity":"metallic_ore", "units":7}),
        json!({"type":"GuardFleet", "interceptor_id":"42", "target_id":"43"}),
        json!({"type":"SetAssignment", "system_id":"2", "structure":"extractor", "workers":12, "specialists":{}, "body_id":1}),
        json!({"type":"TransferModules", "from":"2", "to":"3", "manifest":{"torpedo_rack":2,"reflective_plating":1}}),
        json!({"type":"Withdraw", "fleet_id":"42"}),
    ]
}

#[test]
fn binary_commands_preserve_types_ids_precision_and_optional_fields() {
    assert_eq!(SUBPROTOCOL, format!("stellar.msgpack.v{PROTOCOL_VERSION}"));
    for value in orders() {
        let expected: ClientMsg = serde_json::from_value(value.clone()).unwrap();
        let actual = decode_client(&pack(&value)).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn binary_commands_reject_bad_versions_truncation_trailing_and_oversize() {
    let good = pack(&json!({"type":"Join", "name":"Example"}));
    for cut in 0..good.len() {
        assert!(decode_client(&good[..cut]).is_err());
    }
    let mut old = good.clone();
    old[3] -= 1;
    assert!(matches!(decode_client(&old), Err(DecodeError::Version)));
    let mut trailing = good.clone();
    trailing.push(0xc0);
    assert!(matches!(
        decode_client(&trailing),
        Err(DecodeError::Trailing)
    ));
    assert!(decode_client(br#"{"type":"Join","name":"old JSON"}"#).is_err());
    let mut huge = good;
    huge.resize(MAX_CLIENT_FRAME + 1, 0);
    assert!(matches!(decode_client(&huge), Err(DecodeError::TooLarge)));
    assert!(decode_client(&pack(&json!({"type":"NotAnOrder"}))).is_err());
    assert!(decode_client(&pack(&json!({"type":"MoveShip","ship_id":"1"}))).is_err());
}

#[test]
fn binary_orders_cannot_introduce_nan_infinity_or_unbounded_nesting() {
    #[derive(Serialize)]
    struct BadOrder {
        r#type: &'static str,
        ship_id: &'static str,
        dest: sim::Vec2,
    }
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let packet = pack(&BadOrder {
            r#type: "MoveShip",
            ship_id: "1",
            dest: sim::Vec2::new(bad, 0.0),
        });
        assert!(decode_client(&packet).is_err());
    }
    // A forged collection length must not trigger a huge allocation; validation
    // walks tokens first and errors when the claimed contents aren't present.
    let mut huge_array = HEADER.to_vec();
    huge_array.extend_from_slice(&[0xdd, 0xff, 0xff, 0xff, 0xff]);
    assert!(decode_client(&huge_array).is_err());
    let mut deep = json!(null);
    for _ in 0..128 {
        deep = json!([deep]);
    }
    let packet = pack(&json!({"type":"Join", "name":"Deep", "ignored":deep}));
    assert!(decode_client(&packet).is_err());
}

#[tokio::test]
async fn real_server_messages_preserve_the_served_picture_and_measure_bytes() {
    let messages = crate::game_loop::binary_protocol_fixtures().await;
    let mut fixtures = Vec::new();
    let mut totals = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
    for message in &messages {
        let expected = serde_json::to_value(message).unwrap();
        let binary = encode_server(message).unwrap();
        let actual: Value = rmp_serde::from_slice(&binary[HEADER.len()..]).unwrap();
        assert_eq!(
            actual, expected,
            "binary conversion changed the filtered DTO"
        );
        let json_size = serde_json::to_vec(message).unwrap().len();
        let counts = totals
            .entry(expected["type"].as_str().unwrap().to_owned())
            .or_default();
        counts.0 += 1;
        counts.1 += json_size;
        counts.2 += binary.len();
        fixtures.push(json!({"message":expected,"bytes":binary,"json_bytes":json_size}));
    }
    for (kind, (n, json, binary)) in &totals {
        eprintln!(
            "wire {kind}: {n} messages, JSON={json} bytes, binary={binary} bytes, saving={:.1}%",
            100.0 * (1.0 - *binary as f64 / *json as f64)
        );
    }
    assert!(
        totals.contains_key("Welcome")
            && totals.contains_key("View")
            && totals.contains_key("BattleRecords")
    );
    assert!(totals["View"].2 < totals["View"].1);
    assert!(totals["BattleRecords"].2 < totals["BattleRecords"].1);
    // The Node test consumes REAL Rust-encoded packets with the production JS
    // decoder, then sends its own encoded intents back through decode_client.
    if std::env::var_os("STELLAR_WIRE_FIXTURES").is_some() {
        println!(
            "WIRE_FIXTURES:{}",
            json!({"server":fixtures,"client":orders()})
        );
    }
}

#[test]
fn javascript_encoded_orders_decode_in_rust() {
    let Some(path) = std::env::var_os("STELLAR_WIRE_CLIENT_FIXTURES") else {
        return;
    };
    let cases: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(cases.len() >= 10);
    for case in cases {
        let bytes: Vec<u8> = serde_json::from_value(case["bytes"].clone()).unwrap();
        let actual = decode_client(&bytes).unwrap();
        let expected: ClientMsg = serde_json::from_value(case["message"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}
