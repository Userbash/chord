use chord_json::{JsonValue, parse};
use chord_voice::udp::{DISCOVERY_LEN, DiscoveryPacket, UdpError, UdpMediaSocket};
use chord_voice::{
    Codec, Hello, Identify, OBSERVED_RTC_OPS, SelectProtocol, StreamSpec, VoiceOp, VoicePayload,
    VoiceServerUpdate, default_client_codecs, parse_frame, select_region,
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
                rtx_payload_type: None,
                encode: None,
                decode: None,
            },
            Codec {
                name: "AV1".into(),
                kind: "video".into(),
                priority: 1000,
                payload_type: 101,
                rtx_payload_type: None,
                encode: None,
                decode: None,
            },
        ],
        experiments: Vec::new(),
        rtc_connection_id: None,
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

#[test]
fn test_default_client_codecs_specification() {
    let codecs = default_client_codecs();
    assert_eq!(codecs.len(), 5);

    // Opus (audio, 120, priority 1000)
    assert_eq!(codecs[0].name, "opus");
    assert_eq!(codecs[0].kind, "audio");
    assert_eq!(codecs[0].priority, 1000);
    assert_eq!(codecs[0].payload_type, 120);
    assert_eq!(codecs[0].rtx_payload_type, None);
    assert_eq!(codecs[0].encode, None);
    assert_eq!(codecs[0].decode, None);

    // AV1 (video, 101, priority 1000, rtx 102)
    assert_eq!(codecs[1].name, "AV1");
    assert_eq!(codecs[1].kind, "video");
    assert_eq!(codecs[1].priority, 1000);
    assert_eq!(codecs[1].payload_type, 101);
    assert_eq!(codecs[1].rtx_payload_type, Some(102));
    assert_eq!(codecs[1].encode, Some(true));
    assert_eq!(codecs[1].decode, Some(true));

    // H265 (video, 103, priority 2000, rtx 104)
    assert_eq!(codecs[2].name, "H265");
    assert_eq!(codecs[2].kind, "video");
    assert_eq!(codecs[2].priority, 2000);
    assert_eq!(codecs[2].payload_type, 103);
    assert_eq!(codecs[2].rtx_payload_type, Some(104));
    assert_eq!(codecs[2].encode, Some(true));
    assert_eq!(codecs[2].decode, Some(true));

    // H264 (video, 105, priority 3000, rtx 106)
    assert_eq!(codecs[3].name, "H264");
    assert_eq!(codecs[3].kind, "video");
    assert_eq!(codecs[3].priority, 3000);
    assert_eq!(codecs[3].payload_type, 105);
    assert_eq!(codecs[3].rtx_payload_type, Some(106));
    assert_eq!(codecs[3].encode, Some(true));
    assert_eq!(codecs[3].decode, Some(true));

    // VP8 (video, 107, priority 4000, rtx 108)
    assert_eq!(codecs[4].name, "VP8");
    assert_eq!(codecs[4].kind, "video");
    assert_eq!(codecs[4].priority, 4000);
    assert_eq!(codecs[4].payload_type, 107);
    assert_eq!(codecs[4].rtx_payload_type, Some(108));
    assert_eq!(codecs[4].encode, Some(true));
    assert_eq!(codecs[4].decode, Some(true));
}

#[test]
fn test_select_protocol_wire_parity_with_rtc1_oracle() {
    let oracle_frame = r#"{"op":1,"d":{"protocol":"udp","data":{"address":"193.222.99.83","port":43689,"mode":"aead_aes256_gcm_rtpsize"},"address":"193.222.99.83","port":43689,"mode":"aead_aes256_gcm_rtpsize","codecs":[{"name":"opus","type":"audio","priority":1000,"payload_type":120},{"name":"AV1","type":"video","priority":1000,"payload_type":101,"rtx_payload_type":102,"encode":true,"decode":true},{"name":"H265","type":"video","priority":2000,"payload_type":103,"rtx_payload_type":104,"encode":true,"decode":true},{"name":"H264","type":"video","priority":3000,"payload_type":105,"rtx_payload_type":106,"encode":true,"decode":true},{"name":"VP8","type":"video","priority":4000,"payload_type":107,"rtx_payload_type":108,"encode":true,"decode":true}],"rtc_connection_id":"5363ba05-d6fa-4736-8c04-0d09144e7a8d","experiments":["fixed_keyframe_interval"]}}"#;

    let parsed = parse_frame(oracle_frame).expect("parse oracle op1 SelectProtocol frame");
    assert_eq!(parsed.op, VoiceOp::SelectProtocol);

    let select = match parsed.payload {
        VoicePayload::SelectProtocol(s) => s,
        other => panic!("expected VoicePayload::SelectProtocol, got {other:?}"),
    };

    assert_eq!(select.protocol, "udp");
    assert_eq!(select.address, "193.222.99.83");
    assert_eq!(select.port, 43689);
    assert_eq!(select.mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(select.codecs, default_client_codecs());
    assert_eq!(
        select.rtc_connection_id.as_deref(),
        Some("5363ba05-d6fa-4736-8c04-0d09144e7a8d")
    );
    assert_eq!(
        select.experiments,
        vec!["fixed_keyframe_interval".to_string()]
    );

    // Test serialization with to_frame_json()
    let wire_frame = select.to_frame_json();
    let wire_json = parse(&wire_frame).expect("valid wire frame json");
    let d = wire_json.get("d").expect("d in wire frame");

    assert_eq!(d.get("protocol").and_then(JsonValue::as_str), Some("udp"));
    assert_eq!(
        d.get("address").and_then(JsonValue::as_str),
        Some("193.222.99.83")
    );
    assert_eq!(d.get("port").and_then(JsonValue::as_u64), Some(43689));
    assert_eq!(
        d.get("mode").and_then(JsonValue::as_str),
        Some("aead_aes256_gcm_rtpsize")
    );
    assert_eq!(
        d.get("rtc_connection_id").and_then(JsonValue::as_str),
        Some("5363ba05-d6fa-4736-8c04-0d09144e7a8d")
    );

    let exps = d
        .get("experiments")
        .and_then(JsonValue::as_array)
        .expect("experiments array");
    assert_eq!(exps.len(), 1);
    assert_eq!(exps[0].as_str(), Some("fixed_keyframe_interval"));

    let codecs = d
        .get("codecs")
        .and_then(JsonValue::as_array)
        .expect("codecs array");
    assert_eq!(codecs.len(), 5);
    // Opus should not serialize rtx_payload_type/encode/decode
    assert!(codecs[0].get("rtx_payload_type").is_none());
    assert!(codecs[0].get("encode").is_none());
    assert!(codecs[0].get("decode").is_none());

    // AV1 should serialize rtx_payload_type: 102, encode: true, decode: true
    assert_eq!(
        codecs[1]
            .get("rtx_payload_type")
            .and_then(JsonValue::as_u64),
        Some(102)
    );
    assert_eq!(
        codecs[1].get("encode").and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        codecs[1].get("decode").and_then(JsonValue::as_bool),
        Some(true)
    );

    // VP8
    assert_eq!(
        codecs[4]
            .get("rtx_payload_type")
            .and_then(JsonValue::as_u64),
        Some(108)
    );
    assert_eq!(
        codecs[4].get("encode").and_then(JsonValue::as_bool),
        Some(true)
    );
    assert_eq!(
        codecs[4].get("decode").and_then(JsonValue::as_bool),
        Some(true)
    );

    // Roundtrip
    let roundtripped = parse_frame(&wire_frame).expect("roundtrip parse");
    match roundtripped.payload {
        VoicePayload::SelectProtocol(s) => assert_eq!(s, select),
        other => panic!("expected VoicePayload::SelectProtocol, got {other:?}"),
    }

    // to_json() produces equivalent result
    assert_eq!(select.to_json(), wire_frame);
}

#[test]
fn test_select_protocol_backward_compatibility_without_new_fields() {
    // Old frame without rtx_payload_type, encode, decode, rtc_connection_id, experiments
    let old_frame = r#"{"op":1,"d":{"protocol":"udp","data":{"address":"10.0.0.1","port":12345,"mode":"aead_aes256_gcm_rtpsize"},"codecs":[{"name":"opus","type":"audio","priority":1000,"payload_type":120},{"name":"AV1","type":"video","priority":1000,"payload_type":101}]}}"#;

    let parsed = parse_frame(old_frame).expect("old frame parses cleanly");
    let select = match parsed.payload {
        VoicePayload::SelectProtocol(s) => s,
        other => panic!("expected VoicePayload::SelectProtocol, got {other:?}"),
    };

    assert_eq!(select.protocol, "udp");
    assert_eq!(select.address, "10.0.0.1");
    assert_eq!(select.port, 12345);
    assert_eq!(select.mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(select.rtc_connection_id, None);
    assert!(select.experiments.is_empty());
    assert_eq!(select.codecs.len(), 2);
    assert_eq!(select.codecs[0].rtx_payload_type, None);
    assert_eq!(select.codecs[0].encode, None);
    assert_eq!(select.codecs[0].decode, None);
    assert_eq!(select.codecs[1].rtx_payload_type, None);
    assert_eq!(select.codecs[1].encode, None);
    assert_eq!(select.codecs[1].decode, None);

    let wire = select.to_frame_json();
    let parsed_back = parse_frame(&wire).expect("re-parsed wire");
    match parsed_back.payload {
        VoicePayload::SelectProtocol(s) => assert_eq!(s, select),
        other => panic!("expected VoicePayload::SelectProtocol, got {other:?}"),
    }
}

#[test]
fn test_voice_op_extended_variants_roundtrip_0_to_32_and_unknown() {
    let table: [(u8, VoiceOp, &'static str); 33] = [
        (0, VoiceOp::Identify, "IDENTIFY"),
        (1, VoiceOp::SelectProtocol, "SELECT_PROTOCOL"),
        (2, VoiceOp::Ready, "READY"),
        (3, VoiceOp::Heartbeat, "HEARTBEAT"),
        (4, VoiceOp::SessionDescription, "SESSION_DESCRIPTION"),
        (5, VoiceOp::Speaking, "SPEAKING"),
        (6, VoiceOp::HeartbeatAck, "HEARTBEAT_ACK"),
        (7, VoiceOp::Resume, "RESUME"),
        (8, VoiceOp::Hello, "HELLO"),
        (9, VoiceOp::Resumed, "RESUMED"),
        (10, VoiceOp::Unknown(10), "UNKNOWN"),
        (11, VoiceOp::ClientsConnect, "CLIENTS_CONNECT"),
        (12, VoiceOp::Video, "VIDEO"),
        (13, VoiceOp::ClientDisconnect, "CLIENT_DISCONNECT"),
        (14, VoiceOp::Unknown(14), "UNKNOWN"),
        (15, VoiceOp::MediaSinkWants, "MEDIA_SINK_WANTS"),
        (16, VoiceOp::VoiceBackendVersion, "VOICE_BACKEND_VERSION"),
        (17, VoiceOp::Unknown(17), "UNKNOWN"),
        (18, VoiceOp::Unknown(18), "UNKNOWN"),
        (19, VoiceOp::Unknown(19), "UNKNOWN"),
        (20, VoiceOp::Unknown(20), "UNKNOWN"),
        (
            21,
            VoiceOp::SecureFramesPrepareProtocolTransition,
            "SECURE_FRAMES_PREPARE_PROTOCOL_TRANSITION",
        ),
        (
            22,
            VoiceOp::SecureFramesExecuteTransition,
            "SECURE_FRAMES_EXECUTE_TRANSITION",
        ),
        (
            23,
            VoiceOp::SecureFramesReadyForTransition,
            "SECURE_FRAMES_READY_FOR_TRANSITION",
        ),
        (
            24,
            VoiceOp::SecureFramesPrepareEpoch,
            "SECURE_FRAMES_PREPARE_EPOCH",
        ),
        (
            25,
            VoiceOp::MlsExternalSenderPackage,
            "MLS_EXTERNAL_SENDER_PACKAGE",
        ),
        (26, VoiceOp::MlsKeyPackage, "MLS_KEY_PACKAGE"),
        (27, VoiceOp::MlsProposals, "MLS_PROPOSALS"),
        (28, VoiceOp::MlsCommitWelcome, "MLS_COMMIT_WELCOME"),
        (
            29,
            VoiceOp::MlsPrepareCommitTransition,
            "MLS_PREPARE_COMMIT_TRANSITION",
        ),
        (30, VoiceOp::MlsWelcome, "MLS_WELCOME"),
        (
            31,
            VoiceOp::MlsInvalidCommitWelcome,
            "MLS_INVALID_COMMIT_WELCOME",
        ),
        (32, VoiceOp::NoRoute, "NO_ROUTE"),
    ];

    for (code, expected_op, expected_name) in table {
        let op = VoiceOp::from_u8(code);
        assert_eq!(op, expected_op, "VoiceOp::from_u8({code}) mismatch");
        assert_eq!(op.as_u8(), code, "op.as_u8() mismatch for {code}");
        assert_eq!(op.name(), expected_name, "op.name() mismatch for {code}");
    }

    // Verify unmapped / unknown opcodes outside 0..=32 preserve raw byte
    for raw in [33u8, 77, 99, 128, 200, 255] {
        let op = VoiceOp::from_u8(raw);
        assert_eq!(op, VoiceOp::Unknown(raw));
        assert_eq!(op.as_u8(), raw);
        assert_eq!(op.name(), "UNKNOWN");
    }
}

#[test]
fn test_voice_op_is_dave_or_mls_predicate() {
    // 21..=31 are DAVE / MLS opcodes
    for code in 0..=255u8 {
        let op = VoiceOp::from_u8(code);
        let expected = (21..=31).contains(&code);
        assert_eq!(
            op.is_dave_or_mls(),
            expected,
            "is_dave_or_mls() failed for opcode {code} ({op:?})"
        );
    }

    // Also verify Unknown(21..=31) behaves consistently
    for code in 21..=31u8 {
        assert!(VoiceOp::Unknown(code).is_dave_or_mls());
    }
    assert!(!VoiceOp::Unknown(20).is_dave_or_mls());
    assert!(!VoiceOp::Unknown(32).is_dave_or_mls());
    assert!(!VoiceOp::Unknown(77).is_dave_or_mls());
}

#[test]
fn test_discovery_request_trailing_2_bytes_port_layout() {
    let ssrc = 9208u32; // 0x000023f8
    let port = 33801u16; // 0x8409

    // 1. encode_request_with_port
    let req_with_port = DiscoveryPacket::encode_request_with_port(ssrc, "", port)
        .expect("encode request with port");
    assert_eq!(req_with_port.len(), DISCOVERY_LEN);
    assert_eq!(&req_with_port[0..2], &[0x00, 0x01]); // type: 1 (Request)
    assert_eq!(&req_with_port[2..4], &[0x00, 0x46]); // length: 70
    assert_eq!(&req_with_port[4..8], &ssrc.to_be_bytes()); // SSRC: 9208
    assert_eq!(&req_with_port[8..72], &[0u8; 64]); // 64 bytes zeros
    assert_eq!(&req_with_port[72..74], &port.to_be_bytes()); // Trailing 2 bytes: port (0x8409)

    // Byte-for-byte match with strace-media-s3.jsonl line 2 hex oracle
    let oracle_hex = "00010046000023f8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008409";
    let expected_bytes: Vec<u8> = (0..oracle_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&oracle_hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(req_with_port.as_slice(), expected_bytes.as_slice());

    // 2. Backward compatibility: encode_request(ssrc) produces port 0 (strace line 1 oracle)
    let req_port_0 = DiscoveryPacket::encode_request(ssrc).expect("encode request without port");
    assert_eq!(&req_port_0[72..74], &[0x00, 0x00]);
    let oracle_line1_hex = "00010046000023f8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    let expected_line1: Vec<u8> = (0..oracle_line1_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&oracle_line1_hex[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(req_port_0.as_slice(), expected_line1.as_slice());

    // 3. Roundtrip parsing
    let parsed_with_port = DiscoveryPacket::parse(&req_with_port).expect("parse request with port");
    assert_eq!(parsed_with_port, DiscoveryPacket::Request { ssrc, port });

    let parsed_port_0 = DiscoveryPacket::parse(&req_port_0).expect("parse request port 0");
    assert_eq!(parsed_port_0, DiscoveryPacket::Request { ssrc, port: 0 });

    // 4. Overlong or invalid address is rejected
    assert_eq!(
        DiscoveryPacket::encode_request_with_port(ssrc, &"a".repeat(64), port),
        Err(UdpError::InvalidAddress)
    );
}

#[test]
fn test_udp_media_socket_send_discovery_with_local_port() {
    let server = match std::net::UdpSocket::bind("127.0.0.1:0") {
        Ok(socket) => socket,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            eprintln!("skip: loopback UDP unavailable: {err}");
            return;
        }
        Err(err) => panic!("server bind failed: {err}"),
    };
    let server_addr = server.local_addr().unwrap();

    let mut client = match UdpMediaSocket::bind("127.0.0.1:0") {
        Ok(client) => client,
        Err(err) => {
            eprintln!("skip: loopback UDP client bind: {err}");
            return;
        }
    };
    let client_addr = client.local_addr().unwrap();

    client
        .send_discovery_with_local_port(server_addr, 9208)
        .expect("send_discovery_with_local_port succeeds");

    let mut buf = [0u8; 128];
    let (size, peer) = server.recv_from(&mut buf).unwrap();
    assert_eq!(size, DISCOVERY_LEN);
    assert_eq!(peer, client_addr);

    let parsed = DiscoveryPacket::parse(&buf[..size]).expect("received valid discovery request");
    assert_eq!(
        parsed,
        DiscoveryPacket::Request {
            ssrc: 9208,
            port: client_addr.port()
        }
    );
    assert_eq!(
        u16::from_be_bytes([buf[72], buf[73]]),
        client_addr.port(),
        "trailing 2 bytes must match client local port"
    );
}
