#![forbid(unsafe_code)]

#[path = "../src/dispatch/mod.rs"]
mod dispatch;

use chord_codec::etf::EtfValue;
use chord_core::{DispatchEvent, DispatchKind, route_dispatch};
use dispatch::{
    AsyncTaskDispatcher, BackgroundEvent, DesktopNotificationManager, DispatchBridgeMetrics,
    ForwardOutcome, LinuxSystemTrayIconController, LocalRpcIpcServer, ParallelWorkerPool,
    RedactedVoiceToken, SystemTrayManager, extract_voice_server_token, forward_dispatch_event,
    forward_dispatch_event_with_metrics, map_dispatch_event, map_gateway_disconnect,
};

fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

fn str_val(s: &str) -> EtfValue {
    EtfValue::Str(s.to_string())
}

fn etf_map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(k, v)| (EtfValue::Atom(k.to_string()), v))
            .collect(),
    )
}

fn make_dispatch(name: &str, seq: Option<u64>, data: EtfValue) -> DispatchEvent {
    let mut top = vec![("op", EtfValue::Int(0)), ("t", atom(name))];
    if let Some(s) = seq {
        top.push(("s", EtfValue::Int(i64::try_from(s).unwrap_or(0))));
    }
    top.push(("d", data.clone()));
    let frame = etf_map(top);
    let mut ev = route_dispatch(&frame).unwrap_or(DispatchEvent {
        kind: DispatchKind::from_name(name),
        name: name.to_string(),
        seq,
        data,
    });
    ev.seq = seq;
    ev
}

#[test]
fn preserves_existing_five_background_event_variants_and_eq() {
    let events = vec![
        BackgroundEvent::GatewayMessage("ch-1".into(), "hello".into()),
        BackgroundEvent::PresenceUpdate("u-1".into(), "online".into()),
        BackgroundEvent::VoicePing(23),
        BackgroundEvent::ImageDecoded("img-1".into(), 1920, 1080),
        BackgroundEvent::TaskCompleted("task-42".into()),
        BackgroundEvent::GatewayDisconnected {
            close_code: Some(4000),
            resumable: true,
            reason: "zstd stream reset".into(),
        },
    ];
    let cloned = events.clone();
    assert_eq!(events, cloned);
}

#[test]
fn maps_ready_and_resumed_events() {
    let ready_data = etf_map(vec![
        ("session_id", str_val("sess_live_abc123")),
        (
            "resume_gateway_url",
            str_val("wss://gateway-us-east1-c.discord.gg"),
        ),
        (
            "user",
            etf_map(vec![
                ("id", EtfValue::Int(384_299_970_632_876_032)),
                ("username", str_val("pharos12510")),
            ]),
        ),
        (
            "guilds",
            EtfValue::List(vec![
                etf_map(vec![("id", EtfValue::Int(1_014_537_851_418_980_372))]),
                etf_map(vec![("id", str_val("1176785373989445703"))]),
            ]),
        ),
    ]);
    let ready_ev = make_dispatch("READY", Some(1), ready_data);
    assert_eq!(
        map_dispatch_event(&ready_ev),
        Some(BackgroundEvent::GatewayReady {
            session_id: "sess_live_abc123".into(),
            resume_gateway_url: Some("wss://gateway-us-east1-c.discord.gg".into()),
            user_id: "384299970632876032".into(),
            username: "pharos12510".into(),
            guild_count: 2,
        })
    );

    let resumed_ev = make_dispatch(
        "RESUMED",
        Some(332),
        etf_map(vec![("_trace", EtfValue::List(vec![str_val("gw-trace")]))]),
    );
    assert_eq!(
        map_dispatch_event(&resumed_ev),
        Some(BackgroundEvent::GatewayResumed { seq: 332 })
    );
}

#[test]
fn maps_message_create_up_to_2000_chars_and_message_update() {
    let content_2000 = "Ж".repeat(2000);
    assert_eq!(content_2000.chars().count(), 2000);

    let msg_create_data = etf_map(vec![
        ("id", EtfValue::Int(1_561_632_596_763_353_163)),
        ("channel_id", EtfValue::Int(1_176_785_373_989_445_703)),
        ("guild_id", EtfValue::Int(1_014_537_851_418_980_372)),
        (
            "author",
            etf_map(vec![
                ("id", str_val("384299970632876032")),
                ("username", str_val("ProBot")),
            ]),
        ),
        ("content", str_val(&content_2000)),
        ("timestamp", str_val("2026-10-19T06:18:08.463000+00:00")),
    ]);
    let msg_create_ev = make_dispatch("MESSAGE_CREATE", Some(48), msg_create_data);
    assert_eq!(
        map_dispatch_event(&msg_create_ev),
        Some(BackgroundEvent::GatewayMessageCreate {
            id: "1561632596763353163".into(),
            channel_id: "1176785373989445703".into(),
            guild_id: Some("1014537851418980372".into()),
            author_id: "384299970632876032".into(),
            author_name: "ProBot".into(),
            content: content_2000,
            timestamp: Some("2026-10-19T06:18:08.463000+00:00".into()),
        })
    );

    let msg_update_data = etf_map(vec![
        ("id", str_val("1561632596763353163")),
        ("channel_id", EtfValue::Int(1_176_785_373_989_445_703)),
        ("guild_id", atom("nil")),
        ("content", str_val("edited text")),
        (
            "edited_timestamp",
            str_val("2026-10-19T06:18:10.499000+00:00"),
        ),
    ]);
    let msg_update_ev = make_dispatch("MESSAGE_UPDATE", Some(50), msg_update_data);
    assert_eq!(
        map_dispatch_event(&msg_update_ev),
        Some(BackgroundEvent::GatewayMessageUpdate {
            id: "1561632596763353163".into(),
            channel_id: "1176785373989445703".into(),
            guild_id: None,
            content: Some("edited text".into()),
            edited_timestamp: Some("2026-10-19T06:18:10.499000+00:00".into()),
        })
    );
}

#[test]
fn maps_message_delete_single_and_bulk() {
    let single_del = make_dispatch(
        "MESSAGE_DELETE",
        Some(77),
        etf_map(vec![
            ("id", EtfValue::Int(1_561_632_596_763_353_163)),
            ("channel_id", EtfValue::Int(1_176_785_373_989_445_703)),
            ("guild_id", str_val("1014537851418980372")),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&single_del),
        Some(BackgroundEvent::GatewayMessageDelete {
            ids: vec!["1561632596763353163".into()],
            channel_id: "1176785373989445703".into(),
            guild_id: Some("1014537851418980372".into()),
        })
    );

    let bulk_del = make_dispatch(
        "MESSAGE_DELETE_BULK",
        Some(78),
        etf_map(vec![
            (
                "ids",
                EtfValue::List(vec![
                    EtfValue::Int(0),
                    str_val("1561624484056793151"),
                    atom("18446744073709551615"),
                ]),
            ),
            ("channel_id", str_val("1176785373989445703")),
            ("guild_id", EtfValue::Nil),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&bulk_del),
        Some(BackgroundEvent::GatewayMessageDelete {
            ids: vec![
                "0".into(),
                "1561624484056793151".into(),
                "18446744073709551615".into(),
            ],
            channel_id: "1176785373989445703".into(),
            guild_id: None,
        })
    );
}

#[test]
fn maps_channel_lifecycle_create_update_delete() {
    for (event_name, ch_type, ch_name) in [
        ("CHANNEL_CREATE", Some(0), Some("general-chat")),
        ("CHANNEL_UPDATE", Some(2), Some("Voice Lounge")),
        ("CHANNEL_DELETE", Some(1), None),
    ] {
        let data = etf_map(vec![
            ("id", EtfValue::Int(1_176_785_373_989_445_703)),
            ("guild_id", str_val("1014537851418980372")),
            (
                "type",
                ch_type.map_or(EtfValue::Nil, EtfValue::Int),
            ),
            (
                "name",
                ch_name.map_or(EtfValue::Atom("nil".into()), str_val),
            ),
        ]);
        let ev = make_dispatch(event_name, Some(105), data);
        assert_eq!(
            map_dispatch_event(&ev),
            Some(BackgroundEvent::GatewayChannelLifecycle {
                event_name: event_name.into(),
                id: "1176785373989445703".into(),
                guild_id: Some("1014537851418980372".into()),
                kind: ch_type,
                name: ch_name.map(str::to_string),
            })
        );
    }
}

#[test]
fn maps_presence_typing_and_voice_events_with_token_redaction() {
    let presence_ev = make_dispatch(
        "PRESENCE_UPDATE",
        Some(13),
        etf_map(vec![
            (
                "user",
                etf_map(vec![("id", EtfValue::Int(384_299_970_632_876_032))]),
            ),
            ("status", atom("dnd")),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&presence_ev),
        Some(BackgroundEvent::PresenceUpdate(
            "384299970632876032".into(),
            "dnd".into(),
        ))
    );

    let typing_ev = make_dispatch(
        "TYPING_START",
        Some(60),
        etf_map(vec![
            ("channel_id", str_val("1176785373989445703")),
            ("guild_id", EtfValue::Int(1_014_537_851_418_980_372)),
            ("user_id", str_val("18446744073709551615")),
            ("timestamp", EtfValue::Int(1_729_318_688)),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&typing_ev),
        Some(BackgroundEvent::GatewayTypingStart {
            channel_id: "1176785373989445703".into(),
            guild_id: Some("1014537851418980372".into()),
            user_id: "18446744073709551615".into(),
            timestamp: 1_729_318_688,
        })
    );

    let voice_state_ev = make_dispatch(
        "VOICE_STATE_UPDATE",
        Some(61),
        etf_map(vec![
            ("guild_id", str_val("1014537851418980372")),
            ("channel_id", EtfValue::Int(1_176_785_373_989_445_703)),
            ("user_id", EtfValue::Int(0)),
            ("session_id", str_val("voice_sess_999")),
            ("self_mute", atom("true")),
            ("self_deaf", atom("false")),
            ("self_video", EtfValue::Int(1)),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&voice_state_ev),
        Some(BackgroundEvent::GatewayVoiceStateUpdate {
            guild_id: Some("1014537851418980372".into()),
            channel_id: Some("1176785373989445703".into()),
            user_id: "0".into(),
            session_id: "voice_sess_999".into(),
            self_mute: true,
            self_deaf: false,
            self_video: true,
        })
    );

    let raw_secret = "super_secret_voice_token_do_not_leak_98765";
    let voice_server_ev = make_dispatch(
        "VOICE_SERVER_UPDATE",
        Some(62),
        etf_map(vec![
            ("guild_id", EtfValue::Int(1_014_537_851_418_980_372)),
            ("endpoint", str_val("frankfurt1234.discord.media:443")),
            ("token", str_val(raw_secret)),
        ]),
    );
    let mapped_vsu = map_dispatch_event(&voice_server_ev).expect("VOICE_SERVER_UPDATE mapped");
    assert_eq!(
        mapped_vsu,
        BackgroundEvent::GatewayVoiceServerUpdate {
            guild_id: Some("1014537851418980372".into()),
            endpoint: Some("frankfurt1234.discord.media:443".into()),
            token_present: true,
        }
    );
    let debug_str = format!("{mapped_vsu:?}");
    assert!(
        !debug_str.contains(raw_secret),
        "BackgroundEvent Debug must never leak raw voice token: {debug_str}"
    );

    let redacted: RedactedVoiceToken =
        extract_voice_server_token(&voice_server_ev).expect("voice token extracted");
    assert_eq!(redacted.expose(), raw_secret);
    assert_eq!(format!("{redacted:?}"), "[REDACTED]");
    assert!(!format!("{redacted:?}").contains(raw_secret));
}

#[test]
fn snowflake_boundaries_zero_and_u64_max_both_wire_encodings() {
    let max_str = u64::MAX.to_string(); // "18446744073709551615"

    // 0 as Int(0) and u64::MAX as Str/Atom
    let ev_int_zero = make_dispatch(
        "MESSAGE_DELETE",
        Some(1),
        etf_map(vec![
            ("id", EtfValue::Int(0)),
            ("channel_id", str_val(&max_str)),
            ("guild_id", atom(&max_str)),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&ev_int_zero),
        Some(BackgroundEvent::GatewayMessageDelete {
            ids: vec!["0".into()],
            channel_id: "18446744073709551615".into(),
            guild_id: Some("18446744073709551615".into()),
        })
    );

    // 0 as Str("0") and i64::MAX as Int
    let ev_str_zero = make_dispatch(
        "MESSAGE_DELETE",
        Some(2),
        etf_map(vec![
            ("id", str_val("0")),
            ("channel_id", EtfValue::Int(i64::MAX)),
            ("guild_id", EtfValue::Int(0)),
        ]),
    );
    assert_eq!(
        map_dispatch_event(&ev_str_zero),
        Some(BackgroundEvent::GatewayMessageDelete {
            ids: vec!["0".into()],
            channel_id: i64::MAX.to_string(),
            guild_id: Some("0".into()),
        })
    );
}

#[test]
fn malformed_payloads_and_unknown_dispatches_return_none_without_panic() {
    // Unknown future event
    let future_ev = make_dispatch(
        "SOME_FUTURE_EVENT",
        Some(999),
        etf_map(vec![("id", str_val("123"))]),
    );
    assert_eq!(map_dispatch_event(&future_ev), None);

    // Malformed / incomplete payloads for every mapped event type
    let malformed_cases = vec![
        make_dispatch("READY", Some(1), EtfValue::Nil),
        make_dispatch("READY", Some(1), etf_map(vec![("session_id", str_val("s"))])),
        make_dispatch("RESUMED", Some(2), EtfValue::Int(42)),
        make_dispatch(
            "MESSAGE_CREATE",
            Some(3),
            etf_map(vec![("id", str_val("not_a_number"))]),
        ),
        make_dispatch(
            "MESSAGE_CREATE",
            Some(4),
            etf_map(vec![
                ("id", EtfValue::Int(-1)),
                ("channel_id", str_val("123")),
            ]),
        ),
        make_dispatch(
            "MESSAGE_UPDATE",
            Some(5),
            etf_map(vec![
                ("id", str_val("18446744073709551616")), // u64::MAX + 1 overflow
                ("channel_id", str_val("123")),
            ]),
        ),
        make_dispatch(
            "MESSAGE_DELETE",
            Some(6),
            etf_map(vec![("id", EtfValue::Float(123.0))]),
        ),
        make_dispatch(
            "MESSAGE_DELETE_BULK",
            Some(7),
            etf_map(vec![
                ("ids", EtfValue::List(vec![])),
                ("channel_id", str_val("123")),
            ]),
        ),
        make_dispatch(
            "CHANNEL_CREATE",
            Some(8),
            etf_map(vec![("name", str_val("missing-id"))]),
        ),
        make_dispatch(
            "PRESENCE_UPDATE",
            Some(9),
            etf_map(vec![("status", str_val("online"))]),
        ),
        make_dispatch(
            "TYPING_START",
            Some(10),
            etf_map(vec![
                ("channel_id", str_val("123")),
                ("user_id", str_val("456")),
            ]),
        ),
        make_dispatch(
            "VOICE_STATE_UPDATE",
            Some(11),
            etf_map(vec![("user_id", str_val("123"))]), // missing session_id
        ),
        make_dispatch("VOICE_SERVER_UPDATE", Some(12), etf_map(vec![])),
    ];

    for case in &malformed_cases {
        assert_eq!(
            map_dispatch_event(case),
            None,
            "malformed case {} must return None",
            case.name
        );
    }
}

#[test]
fn bounded_channel_backpressure_increments_drop_metrics_without_blocking() {
    let dispatcher = AsyncTaskDispatcher::new(2);
    let tx = dispatcher.sender();
    let mut standalone_metrics = DispatchBridgeMetrics::default();

    let ev1 = make_dispatch("RESUMED", Some(10), etf_map(vec![]));
    let ev2 = make_dispatch("RESUMED", Some(11), etf_map(vec![]));
    let ev3 = make_dispatch("RESUMED", Some(12), etf_map(vec![]));
    let unknown = make_dispatch("SOME_FUTURE_EVENT", Some(13), etf_map(vec![]));

    assert_eq!(dispatcher.forward_dispatch_event(&ev1), ForwardOutcome::Sent);
    assert_eq!(dispatcher.forward_dispatch_event(&ev2), ForwardOutcome::Sent);
    assert_eq!(
        dispatcher.forward_dispatch_event(&ev3),
        ForwardOutcome::ChannelFull
    );
    assert_eq!(
        dispatcher.forward_dispatch_event(&unknown),
        ForwardOutcome::Ignored
    );

    let metrics = dispatcher.bridge_metrics();
    assert_eq!(metrics.sent, 2);
    assert_eq!(metrics.dropped, 1);
    assert_eq!(metrics.channel_full, 1);
    assert_eq!(metrics.channel_closed, 0);
    assert_eq!(metrics.ignored, 1);

    // Standalone forward while still full
    assert_eq!(
        forward_dispatch_event_with_metrics(&tx, &ev3, &mut standalone_metrics),
        ForwardOutcome::ChannelFull
    );
    assert_eq!(standalone_metrics.dropped, 1);
    assert_eq!(standalone_metrics.channel_full, 1);

    // Drain events and verify recovery
    let drained = dispatcher.drain_events();
    assert_eq!(
        drained,
        vec![
            BackgroundEvent::GatewayResumed { seq: 10 },
            BackgroundEvent::GatewayResumed { seq: 11 },
        ]
    );
    assert_eq!(forward_dispatch_event(&tx, &ev3), ForwardOutcome::Sent);
    assert_eq!(
        dispatcher.drain_events(),
        vec![BackgroundEvent::GatewayResumed { seq: 12 }]
    );

    // Close receiver and verify ChannelClosed outcome
    dispatcher.receiver().close();
    assert_eq!(
        dispatcher.forward_dispatch_event(&ev1),
        ForwardOutcome::ChannelClosed
    );
    let final_metrics = dispatcher.bridge_metrics();
    assert_eq!(final_metrics.dropped, 2);
    assert_eq!(final_metrics.channel_closed, 1);
    assert!(ForwardOutcome::Sent.is_sent());
    assert!(ForwardOutcome::Ignored.is_ignored());
    assert!(ForwardOutcome::ChannelFull.is_dropped());
    assert!(ForwardOutcome::ChannelClosed.is_dropped());

    dispatcher.reset_bridge_metrics();
    assert_eq!(dispatcher.bridge_metrics(), DispatchBridgeMetrics::default());
}

#[test]
fn dispatch_handler_callback_and_disconnect_classification_and_json_wire_payload() {
    let dispatcher = AsyncTaskDispatcher::new(4);
    let handler = dispatcher.dispatch_handler();

    let json_payload = r#"{"id":18446744073709551615,"channel_id":0,"guild_id":"1014537851418980372"}"#;
    let json_del_ev = DispatchEvent {
        kind: None,
        name: "MESSAGE_DELETE".into(),
        seq: Some(200),
        data: EtfValue::Str(json_payload.into()),
    };
    handler(&json_del_ev);

    let drained = dispatcher.drain_events();
    assert_eq!(
        drained,
        vec![BackgroundEvent::GatewayMessageDelete {
            ids: vec!["18446744073709551615".into()],
            channel_id: "0".into(),
            guild_id: Some("1014537851418980372".into()),
        }]
    );
    assert_eq!(dispatcher.bridge_metrics().sent, 1);

    // GatewayDisconnected classification
    assert_eq!(
        map_gateway_disconnect(Some(4000), "unknown error"),
        BackgroundEvent::GatewayDisconnected {
            close_code: Some(4000),
            resumable: true,
            reason: "unknown error".into(),
        }
    );
    assert_eq!(
        map_gateway_disconnect(Some(4004), "authentication failed"),
        BackgroundEvent::GatewayDisconnected {
            close_code: Some(4004),
            resumable: false,
            reason: "authentication failed".into(),
        }
    );
}

#[test]
fn existing_dispatch_utilities_and_workers_remain_intact() {
    let default_disp = AsyncTaskDispatcher::default();
    assert!(default_disp.drain_events().is_empty());

    let pool = ParallelWorkerPool::new(4);
    assert!((2..=32).contains(&pool.worker_count()));
    let mut squares = pool.execute_parallel_chunks(vec![2u32, 3, 4], |n| n * n);
    squares.sort_unstable();
    assert_eq!(squares, vec![4, 9, 16]);

    let notif = DesktopNotificationManager::build_message_notification("Alice", "Hi", "general");
    assert_eq!(notif.app_name, "Discord");

    let tray = LinuxSystemTrayIconController::default_linux();
    assert!(tray.has_valid_6_item_menu());
    assert_eq!(SystemTrayManager::format_badge(120).as_deref(), Some("99+"));

    let frame = LocalRpcIpcServer::create_handshake_frame(1, "123456").unwrap();
    let parsed = LocalRpcIpcServer::parse_ipc_frame(&frame).unwrap();
    assert!(parsed.payload.contains("123456"));
}


