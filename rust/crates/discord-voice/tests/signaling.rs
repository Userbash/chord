use discord_json::{JsonValue, parse};
use discord_voice::{
    Codec, Hello, Identify, OBSERVED_RTC_OPS, SelectProtocol, StreamSpec, VoiceOp, VoicePayload,
    VoiceServerUpdate, parse_frame, select_region,
};
use std::fmt::Debug;

const KEY_B64: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

fn captured_payloads(path: &str) -> Vec<String> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => {
            eprintln!("skip: capture unavailable at {path}: {error}");
            return Vec::new();
        }
    };
    contents
        .lines()
        .filter_map(|line| parse(line).ok())
        .filter_map(|wrapper| {
            wrapper
                .get("payload")
                .and_then(JsonValue::as_str)
                .map(str::to_owned)
        })
        .collect()
}

fn without_secrets<T: Debug>(value: &T) -> String {
    format!("{value:?}")
}

#[test]
fn all_observed_ops_round_trip_and_unknown_is_preserved() {
    for raw in OBSERVED_RTC_OPS {
        let op = VoiceOp::from_u8(raw);
        assert_eq!(op.as_u8(), raw);
    }
    assert_eq!(VoiceOp::from_u8(77), VoiceOp::Unknown(77));
    assert_eq!(VoiceOp::Unknown(77).as_u8(), 77);
}

#[test]
fn capture_frames_match_signal_contract() {
    let first = captured_payloads("/home/sanya/re-deep/voice/rtc1-frames.jsonl");
    let second = captured_payloads("/home/sanya/re-deep/voice/rtc2-frames.jsonl");
    if first.is_empty() || second.is_empty() {
        eprintln!("skip: rtc1/rtc2 capture fixtures unavailable");
        return;
    }
    let all = first.into_iter().chain(second).collect::<Vec<_>>();
    let parsed = all
        .iter()
        .map(|payload| {
            let sanitized = payload
                .replace(
                    "\"secret_key\":\"REDACTED\"",
                    &format!("\"secret_key\":\"{KEY_B64}\""),
                )
                .replace(
                    "\"media_session_id\":\"REDACTED\"",
                    "\"media_session_id\":\"fixture-id\"",
                )
                .replace(
                    "\"session_id\":\"REDACTED\"",
                    "\"session_id\":\"fixture-session\"",
                )
                .replace("\"token\":\"REDACTED\"", "\"token\":\"fixture-token\"");
            parse_frame(&sanitized).expect("capture frame parses")
        })
        .collect::<Vec<_>>();

    assert!(parsed.iter().any(|frame| matches!(
        frame.payload,
        VoicePayload::Hello(Hello {
            version: 8,
            heartbeat_interval_ms: 13_750
        })
    )));
    let ready = parsed
        .iter()
        .find_map(|frame| match &frame.payload {
            VoicePayload::Ready(value) => Some(value),
            _ => None,
        })
        .expect("Ready capture");
    assert!(ready.ssrc > 0 && ready.port > 0 && !ready.ip.is_empty());
    assert!(ready.modes.contains(&"aead_aes256_gcm_rtpsize".to_owned()));
    let stream = ready.streams.first().expect("Ready stream");
    assert_eq!(stream.kind, "video");
    assert!(stream.ssrc.is_some(), "SSRC потока должен сохраняться");
    assert!(stream.rtx_ssrc.is_some(), "RTX SSRC должен сохраняться");
    assert_eq!(stream.rid.as_deref(), Some("100"));
    assert_eq!(stream.quality, Some(100));
    assert_eq!(stream.active, Some(false));
    let description = parsed
        .iter()
        .find_map(|frame| match &frame.payload {
            VoicePayload::SessionDescription(value) => Some(value),
            _ => None,
        })
        .expect("Session Description capture");
    assert_eq!(description.mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(description.secret_key.len(), 32);
    assert_eq!(description.dave_protocol_version, 1);
    assert_eq!(description.secure_frames_version, 1);
    assert!(
        parsed
            .iter()
            .any(|frame| frame.op == VoiceOp::Unknown(15) || frame.op.as_u8() == 15)
    );
    assert!(parsed.iter().any(|frame| frame.op.as_u8() == 16));
    assert!(
        parsed
            .iter()
            .any(|frame| frame.op.as_u8() == 3 || frame.op.as_u8() == 6)
    );

    let identify = parsed
        .iter()
        .find_map(|frame| match &frame.payload {
            VoicePayload::Identify(value) => Some(value),
            _ => None,
        })
        .expect("Identify capture");
    assert_eq!(identify.server_id, "1014537851418980372");
    assert_eq!(identify.channel_id, "1176785373989445703");
    assert_eq!(identify.user_id, "384299970632876032");
    assert_eq!(identify.max_dave_protocol_version, 1);
    assert!(identify.video);
    assert_eq!(identify.streams[0].rid.as_deref(), Some("100"));
    assert_eq!(identify.streams[0].quality, Some(100));
    let identify_debug = format!("{identify:?}");
    assert!(identify_debug.contains("token: \"<redacted>\""));
    assert!(!identify_debug.contains("fixture-token"));
    assert!(!identify_debug.contains("fixture-session"));
}

#[test]
fn select_protocol_codecs_match_capture_contract() {
    let frame = parse_frame(r#"{"op":1,"d":{"protocol":"udp","data":{"address":"127.0.0.1","port":19309,"mode":"aead_aes256_gcm_rtpsize"},"codecs":[{"name":"opus","type":"audio","priority":1000,"payload_type":120},{"name":"AV1","type":"video","priority":1000,"payload_type":101}]}}"#)
        .expect("select protocol parses");
    let value = match frame.payload {
        VoicePayload::SelectProtocol(value) => value,
        other => panic!("unexpected payload: {other:?}"),
    };
    assert_eq!(value.protocol, "udp");
    assert_eq!(value.address, "127.0.0.1");
    assert_eq!(value.port, 19309);
    assert_eq!(value.codecs.len(), 2);
    assert_eq!(
        (
            value.codecs[0].name.as_str(),
            value.codecs[0].payload_type,
            value.codecs[0].priority
        ),
        ("opus", 120, 1000)
    );
    assert_eq!(
        (value.codecs[1].name.as_str(), value.codecs[1].payload_type),
        ("AV1", 101)
    );
}

#[test]
fn secret_key_is_decoded_validated_and_redacted() {
    let frame = parse_frame(&format!(r#"{{"op":4,"d":{{"mode":"m","secret_key":"{KEY_B64}","media_session_id":"private-media-id","dave_protocol_version":1,"secure_frames_version":1}}}}"#))
        .expect("valid key parses");
    let value = match frame.payload {
        VoicePayload::SessionDescription(value) => value,
        other => panic!("unexpected payload: {other:?}"),
    };
    assert_eq!(value.secret_key, vec![0; 32]);
    let debug = without_secrets(&value);
    assert!(debug.contains("secret_key: \"<redacted>\""));
    assert!(!debug.contains(KEY_B64));
    assert!(!debug.contains("private-media-id"));
    let display = value.to_string();
    assert!(!display.contains(KEY_B64));
    assert!(!display.contains("private-media-id"));
    assert!(parse_frame(r#"{"op":4,"d":{"mode":"m","secret_key":"AA==","media_session_id":"x","dave_protocol_version":1,"secure_frames_version":1}}"#).is_err());
    assert!(parse_frame(r#"{"op":4,"d":{"mode":"m","secret_key":"!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!","media_session_id":"x","dave_protocol_version":1,"secure_frames_version":1}}"#).is_err());
}

#[test]
fn frame_parser_preserves_raw_other_and_rejects_missing_payload() {
    let frame = parse_frame(r#"{"op":15,"seq":9,"d":{"any":100}}"#).unwrap();
    assert_eq!(frame.op.as_u8(), 15);
    assert_eq!(frame.seq, Some(9));
    assert!(matches!(frame.payload, VoicePayload::Other { .. }));
    assert!(parse_frame(r#"{"op":0}"#).is_err());
    assert!(matches!(
        parse_frame(r#"{"op":99,"d":{}}"#).unwrap().op,
        VoiceOp::Unknown(99)
    ));
}

#[test]
fn voice_server_update_builds_valid_websocket_url_without_leaking_token() {
    let update = VoiceServerUpdate::parse(
        r#"{"token":"private-token","guild_id":"123","endpoint":"c-fra19.discord.media:2053"}"#,
    )
    .unwrap();
    assert_eq!(
        update.websocket_url().unwrap(),
        "wss://c-fra19.discord.media:2053/?v=9"
    );
    assert_eq!(
        VoiceServerUpdate::parse(
            r#"{"token":"t","guild_id":"g","endpoint":"wss://voice.example:2096"}"#
        )
        .unwrap()
        .websocket_url()
        .unwrap(),
        "wss://voice.example:2096/?v=9"
    );
    assert!(!format!("{update:?}").contains("private-token"));
    for endpoint in [
        "",
        " ",
        "http://voice.example",
        "ws://voice.example",
        "wss://voice.example/other",
        "wss://voice.example/?x=1",
        "voice.example/@x",
        "voice.example?x",
    ] {
        let input = format!(r#"{{"token":"t","guild_id":"g","endpoint":"{endpoint}"}}"#);
        assert!(
            VoiceServerUpdate::parse(&input)
                .and_then(|value| value.websocket_url())
                .is_err(),
            "accepted {endpoint:?}"
        );
    }
}

#[test]
fn region_selection_orders_latency_ties_and_manual_choice() {
    let samples = vec![
        ("paris".to_owned(), 20),
        ("frankfurt".to_owned(), 10),
        ("stockholm".to_owned(), 10),
    ];
    let geo = vec![
        "stockholm".to_owned(),
        "frankfurt".to_owned(),
        "paris".to_owned(),
    ];
    assert_eq!(select_region(&samples, &geo, None).unwrap(), "stockholm");
    assert_eq!(
        select_region(&samples, &geo, Some("paris")).unwrap(),
        "paris"
    );
    assert!(select_region(&samples, &geo, Some("milan")).is_err());
    assert_eq!(select_region(&[], &geo, None).unwrap(), "stockholm");
    assert!(select_region(&[], &[], None).is_err());
    assert_eq!(
        select_region(
            &[
                ("paris".into(), 4),
                ("paris".into(), 2),
                ("frankfurt".into(), 3)
            ],
            &geo,
            None
        )
        .unwrap(),
        "paris"
    );
    // Географический порядок важнее лексикографического при равной задержке.
    let tie = [("amsterdam".to_owned(), 10), ("zurich".to_owned(), 10)];
    assert_eq!(
        select_region(&tie, &["zurich".to_owned(), "amsterdam".to_owned()], None).unwrap(),
        "zurich"
    );
}

#[test]
fn outgoing_identify_and_select_protocol_are_wire_serializable() {
    let identify = Identify {
        server_id: "1014537851418980372".into(),
        channel_id: "1176785373989445703".into(),
        user_id: "384299970632876032".into(),
        session_id: "session-private".into(),
        token: "token-private".into(),
        max_dave_protocol_version: 1,
        video: true,
        streams: vec![StreamSpec {
            kind: "video".into(),
            ssrc: None,
            rtx_ssrc: None,
            rid: Some("100".into()),
            quality: Some(100),
            active: None,
        }],
    };
    let wire = identify.to_json();
    assert!(wire.contains("token-private"));
    let parsed = parse_frame(&wire).expect("identify wire");
    match parsed.payload {
        VoicePayload::Identify(value) => assert_eq!(value, identify),
        other => panic!("expected Identify, got {other:?}"),
    }
    assert!(!format!("{identify:?}").contains("token-private"));
    assert!(!format!("{identify:?}").contains("session-private"));

    let select = SelectProtocol {
        protocol: "udp".into(),
        address: "104.29.147.145".into(),
        port: 19_309,
        mode: "aead_aes256_gcm_rtpsize".into(),
        codecs: vec![
            Codec {
                name: "opus".into(),
                kind: "audio".into(),
                priority: 1000,
                payload_type: 120,
            },
            Codec {
                name: "AV1".into(),
                kind: "video".into(),
                priority: 1000,
                payload_type: 101,
            },
        ],
    };
    let wire = select.to_json();
    let wire_json = parse(&wire).expect("select protocol wire JSON");
    let data = wire_json
        .get("d")
        .and_then(|value| value.get("data"))
        .expect("nested protocol data");
    assert_eq!(
        data.get("address").and_then(JsonValue::as_str),
        Some("104.29.147.145")
    );
    assert_eq!(data.get("port").and_then(JsonValue::as_u64), Some(19_309));
    assert_eq!(
        wire_json
            .get("d")
            .and_then(|value| value.get("address"))
            .and_then(JsonValue::as_str),
        Some("104.29.147.145"),
        "capture содержит address и на верхнем уровне d"
    );
    assert_eq!(
        wire_json
            .get("d")
            .and_then(|value| value.get("mode"))
            .and_then(JsonValue::as_str),
        Some("aead_aes256_gcm_rtpsize")
    );
    let parsed = parse_frame(&wire).expect("select protocol wire");
    match parsed.payload {
        VoicePayload::SelectProtocol(value) => assert_eq!(value, select),
        other => panic!("expected SelectProtocol, got {other:?}"),
    }
}

#[test]
fn unknown_payload_debug_does_not_leak_secrets() {
    let frame = parse_frame(r#"{"op":99,"d":{"token":"super-secret","secret_key":"private"}}"#)
        .expect("unknown op");
    let debug = format!("{frame:?}");
    assert!(debug.contains("Other"));
    assert!(!debug.contains("super-secret"));
    assert!(!debug.contains("private"));
}

#[test]
fn endpoint_with_existing_version_query_is_not_duplicated() {
    let update = VoiceServerUpdate {
        token: "token".into(),
        guild_id: "guild".into(),
        endpoint: "wss://voice.example:2096/?v=9".into(),
    };
    assert_eq!(
        update.websocket_url().unwrap(),
        "wss://voice.example:2096/?v=9"
    );
}

#[test]
fn every_prefix_of_frame_is_panic_free() {
    let padding = "x".repeat(12_000);
    let frame = format!(
        r#"{{"op":4,"seq":1,"d":{{"mode":"aead_aes256_gcm_rtpsize","secret_key":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=","media_session_id":"id","dave_protocol_version":1,"secure_frames_version":1,"padding":"{padding}"}}}}"#
    );
    for end in 0..10_000 {
        assert!(
            parse_frame(&frame[..end]).is_err(),
            "prefix {end} unexpectedly parsed"
        );
    }
    for byte in 0..10_000usize {
        let malformed = format!("{{\"op\":{},\"d\":{{}}}}", byte % 256);
        let _ = parse_frame(&malformed);
    }
}
