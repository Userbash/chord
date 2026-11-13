//! Контрактные тесты шлюза и редьюсера Go Live / Stream (STUB-27, RW-40).
//!
//! Оракул: `waves/RW-40/VOICE/stream-opcodes-fixtures.json` (схема `rw40/stream-opcodes-fixtures-v1`),
//! `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` и декодированный бандл эталонного клиента
//! `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`.
//! Покрывает матрицу T-STREAM-01…T-STREAM-22.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chord_core::Op;
use chord_core::codec::etf::{self, EtfValue};
use chord_core::stream::{
    GatewaySessionState, LocalStreamAction, MAX_KEY_BYTES, StreamAction, StreamCommand,
    StreamCreated, StreamDeleteReason, StreamDeleted, StreamDiagnostics, StreamDispatch,
    StreamEffect, StreamError, StreamKey, StreamKeyError, StreamKind, StreamOp, StreamPhase,
    StreamSecret, StreamServerUpdate, StreamState, StreamType, StreamUpdate, apply, parse_dispatch,
    parse_dispatch_with_diagnostics,
};
use chord_json::{JsonValue, parse as parse_json};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("корень репозитория")
        .to_path_buf()
}

fn load_fixture_json() -> JsonValue {
    let path = repo_root().join("waves/RW-40/VOICE/stream-opcodes-fixtures.json");
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("не удалось прочитать фикстуру {}: {e}", path.display()));
    parse_json(&raw).expect("валидный JSON фикстуры stream-opcodes-fixtures.json")
}

fn load_contract_markdown() -> String {
    let path = repo_root().join("waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("не удалось прочитать контракт {}: {e}", path.display()))
}

fn json_to_etf(value: &JsonValue) -> EtfValue {
    match value {
        JsonValue::Null => EtfValue::Nil,
        JsonValue::Bool(b) => EtfValue::Atom(if *b { "true" } else { "false" }.to_string()),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                EtfValue::Int(i)
            } else if let Some(f) = n.as_f64() {
                EtfValue::Float(f)
            } else {
                EtfValue::Str(n.as_str().to_string())
            }
        }
        JsonValue::String(s) => EtfValue::Str(s.clone()),
        JsonValue::Array(items) => EtfValue::List(items.iter().map(json_to_etf).collect()),
        JsonValue::Object(map) => EtfValue::Map(
            map.iter()
                .map(|(k, v)| (EtfValue::Str(k.clone()), json_to_etf(v)))
                .collect(),
        ),
    }
}

fn canonicalize_etf(value: &EtfValue) -> EtfValue {
    match value {
        EtfValue::Map(pairs) => {
            let mut sorted: Vec<(String, EtfValue)> = pairs
                .iter()
                .map(|(k, v)| {
                    let key_name = match k {
                        EtfValue::Str(s) | EtfValue::Atom(s) => s.clone(),
                        other => format!("{other:?}"),
                    };
                    (key_name, canonicalize_etf(v))
                })
                .collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            EtfValue::Map(
                sorted
                    .into_iter()
                    .map(|(k, v)| (EtfValue::Str(k), v))
                    .collect(),
            )
        }
        EtfValue::List(items) => EtfValue::List(items.iter().map(canonicalize_etf).collect()),
        EtfValue::Tuple(items) => EtfValue::Tuple(items.iter().map(canonicalize_etf).collect()),
        other => other.clone(),
    }
}

fn effect_code(effect: &StreamEffect) -> &'static str {
    match effect {
        StreamEffect::OpenStreamRtc { .. } => "open_stream_rtc",
        StreamEffect::CloseStreamRtc { .. } => "close_stream_rtc",
        StreamEffect::ShowSafetyRateLimit { .. } => "show_safety_rate_limit",
        StreamEffect::NotifyViewerFull { .. } => "notify_viewer_full",
    }
}

/// Проверка якорей из фикстуры против STREAM-GATEWAY-CONTRACT.md и обязательного RE-бандла.
#[test]
fn t_stream_00_anchors_match_contract_and_oracle_bundle() {
    let fixture = load_fixture_json();
    let contract_md = load_contract_markdown();

    assert_eq!(
        fixture.get("schema").and_then(JsonValue::as_str),
        Some("rw40/stream-opcodes-fixtures-v1")
    );
    let anchors = fixture
        .get("anchors")
        .and_then(JsonValue::as_array)
        .expect("anchors array");
    assert_eq!(anchors.len(), 17);

    let oracle = fixture
        .get("provenance")
        .and_then(|p| p.get("oracle"))
        .unwrap();
    let lab_path = oracle.get("labPath").and_then(JsonValue::as_str).unwrap();
    let expected_bytes = oracle.get("bytes").and_then(JsonValue::as_u64).unwrap() as usize;
    assert_eq!(expected_bytes, 12_410_287);
    let bytes = if Path::new(lab_path).exists() {
        let b = fs::read(lab_path).unwrap_or_else(|e| {
            panic!(
                "внешний RE-оракул {lab_path} обязателен для побайтовой сверки T-STREAM-00 (TEST-POLICY): {e}"
            )
        });
        assert_eq!(
            b.len(),
            expected_bytes,
            "размер RE-бандла должен совпадать с оракулом (12_410_287 Б)"
        );
        Some(b)
    } else {
        eprintln!("SKIP(t_stream_00): external RE oracle {lab_path} not present in CI");
        None
    };

    for anchor in anchors {
        let id = anchor.get("id").and_then(JsonValue::as_str).unwrap();
        let offset = anchor.get("offset").and_then(JsonValue::as_u64).unwrap() as usize;
        let len = anchor.get("len").and_then(JsonValue::as_u64).unwrap() as usize;
        let quote = anchor.get("quote").and_then(JsonValue::as_str).unwrap();

        assert!(
            contract_md.contains(&format!("`{id}`")),
            "STREAM-GATEWAY-CONTRACT.md должен содержать якорь {id}"
        );
        assert_eq!(
            quote.len(),
            len,
            "длина цитаты якоря {id} в фикстуре должна равняться полю len"
        );

        if let Some(ref bytes) = bytes {
            let slice = std::str::from_utf8(&bytes[offset..offset + len])
                .expect("UTF-8 срез в RE-артефакте");
            assert_eq!(
                slice, quote,
                "байтовый срез артефакта для {id} @{offset}+{len} должен совпадать с quote"
            );
        }
    }
}

/// T-STREAM-01: таблица опкодов 18…22 ↔ имена, `from_code(23)==None`, `Op::from_code(18)==Unknown(18)`.
#[test]
fn t_stream_01_opcode_table_and_gateway_isolation() {
    let fixture = load_fixture_json();
    let opcodes = fixture
        .get("opcodes")
        .and_then(JsonValue::as_array)
        .expect("opcodes array");

    let client_ops: Vec<&JsonValue> = opcodes
        .iter()
        .filter(|entry| {
            entry.get("direction").and_then(JsonValue::as_str) == Some("client_to_server")
        })
        .collect();
    assert_eq!(client_ops.len(), 5);

    let expected_variants = [
        (
            18,
            "STREAM_CREATE",
            StreamOp::Create,
            StreamOp::StreamCreate,
        ),
        (
            19,
            "STREAM_DELETE",
            StreamOp::Delete,
            StreamOp::StreamDelete,
        ),
        (20, "STREAM_WATCH", StreamOp::Watch, StreamOp::StreamWatch),
        (21, "STREAM_PING", StreamOp::Ping, StreamOp::StreamPing),
        (
            22,
            "STREAM_SET_PAUSED",
            StreamOp::SetPaused,
            StreamOp::StreamSetPaused,
        ),
    ];

    for ((code, name, variant, alias), fixture_op) in
        expected_variants.iter().zip(client_ops.iter())
    {
        assert_eq!(
            fixture_op.get("code").and_then(JsonValue::as_i64),
            Some(*code)
        );
        assert_eq!(
            fixture_op.get("name").and_then(JsonValue::as_str),
            Some(*name)
        );
        assert_eq!(variant.code(), *code);
        assert_eq!(alias.code(), *code);
        assert_eq!(*variant, *alias);
        assert_eq!(variant.name(), *name);
        assert_eq!(StreamOp::from_code(*code), Some(*variant));
        // В gateway.rs Op::from_code не расширяется (§0 п. 2).
        assert_eq!(Op::from_code(*code), Op::Unknown(*code));
    }

    assert_eq!(StreamOp::from_code(0), None);
    assert_eq!(StreamOp::from_code(17), None);
    assert_eq!(StreamOp::from_code(23), None);
    assert_eq!(StreamOp::from_code(24), None);
    assert_eq!(StreamType::Guild, StreamKind::Guild);
    assert_eq!(StreamType::Call, StreamKind::Call);
}

/// T-STREAM-02, T-STREAM-03, T-STREAM-04: кодирование всех 9 исходящих векторов фикстуры,
/// D-27-2 (`guild_id: null` для `call`), D-27-3 (отправка только из `Established`).
#[test]
fn t_stream_02_03_04_outbound_commands_match_fixtures() {
    let fixture = load_fixture_json();
    let outbound = fixture
        .get("outbound")
        .and_then(JsonValue::as_array)
        .expect("outbound array");
    assert_eq!(outbound.len(), 9);

    for vector in outbound {
        let id = vector.get("id").and_then(JsonValue::as_str).unwrap();
        let expected_op = vector.get("op").and_then(JsonValue::as_i64).unwrap();
        let expected_op_name = vector.get("opName").and_then(JsonValue::as_str).unwrap();
        let payload_json = vector.get("payload").expect("payload");
        let expect_cmd = vector.get("expectCommand").expect("expectCommand");

        let spec_etf = json_to_etf(expect_cmd);
        let cmd = StreamCommand::from_raw_spec(&spec_etf, GatewaySessionState::Established)
            .unwrap_or_else(|e| panic!("вектор {id} должен строиться без ошибки: {e}"));

        assert_eq!(cmd.op().code(), expected_op, "op code для {id}");
        assert_eq!(cmd.op().name(), expected_op_name, "op name для {id}");

        let encoded_frame = cmd
            .encode_in_session(GatewaySessionState::Established)
            .unwrap_or_else(|e| panic!("encode для {id}: {e}"));
        let frame_pairs = encoded_frame.as_map().expect("кадр должен быть ETF map");
        assert_eq!(frame_pairs.len(), 2, "ровно поля op и d в кадре {id}");
        assert_eq!(encoded_frame.get("op"), Some(&EtfValue::Int(expected_op)));

        let d = encoded_frame.get("d").expect("поле d в кадре");
        let expected_d = json_to_etf(payload_json);
        assert_eq!(
            canonicalize_etf(d),
            canonicalize_etf(&expected_d),
            "полезная нагрузка d для {id} должна совпадать с фикстурой"
        );

        // Проверка D-27-2: у call-стрима поле guild_id присутствует и равно EtfValue::Nil.
        if id == "out-18-create-call" {
            let d_map = d.as_map().unwrap();
            let guild_pair = d_map
                .iter()
                .find(|(k, _)| k.as_str() == Some("guild_id"))
                .expect("поле guild_id обязано явно присутствовать для call (D-27-2)");
            assert_eq!(guild_pair.1, EtfValue::Nil);
        }

        // Проверка D-27-3: вне Established команда отклоняется с SessionNotEstablished.
        let err = cmd
            .encode_in_session(GatewaySessionState::Connecting)
            .expect_err("вне Established команда не должна кодироваться");
        assert_eq!(err, StreamError::SessionNotEstablished);
        assert_eq!(err.code(), "stream.session.not_established");
    }
}

/// T-STREAM-05: StreamKey round-trip `parse → to_wire` и `build` на канонических ключах фикстуры.
#[test]
fn t_stream_05_stream_key_roundtrip_and_canonical_form() {
    let fixture = load_fixture_json();
    let keys = fixture.get("keys").expect("keys section");
    let canonical = keys.get("canonical").expect("canonical keys");
    let guild_str = canonical.get("guild").and_then(JsonValue::as_str).unwrap();
    let call_str = canonical.get("call").and_then(JsonValue::as_str).unwrap();

    let guild_key = StreamKey::parse(guild_str).expect("разбор канонического guild ключа");
    assert_eq!(guild_key.kind, StreamKind::Guild);
    assert_eq!(guild_key.guild_id, Some(100_000_000_000_000_001));
    assert_eq!(guild_key.channel_id, 100_000_000_000_000_002);
    assert_eq!(guild_key.owner_id, 100_000_000_000_000_003);
    assert_eq!(guild_key.to_wire(), guild_str);
    assert_eq!(
        StreamKey::guild(
            100_000_000_000_000_001,
            100_000_000_000_000_002,
            100_000_000_000_000_003
        ),
        guild_key
    );
    assert_eq!(
        StreamKey::build(
            StreamKind::Guild,
            Some(100_000_000_000_000_001),
            100_000_000_000_000_002,
            100_000_000_000_000_003
        )
        .unwrap(),
        guild_key
    );

    let call_key = StreamKey::parse(call_str).expect("разбор канонического call ключа");
    assert_eq!(call_key.kind, StreamKind::Call);
    assert_eq!(call_key.guild_id, None);
    assert_eq!(call_key.channel_id, 100_000_000_000_000_002);
    assert_eq!(call_key.owner_id, 100_000_000_000_000_003);
    assert_eq!(call_key.to_wire(), call_str);
    assert_eq!(
        StreamKey::call(100_000_000_000_000_002, 100_000_000_000_000_003),
        call_key
    );
    assert_eq!(
        StreamKey::build(
            StreamKind::Call,
            None,
            100_000_000_000_000_002,
            100_000_000_000_000_003
        )
        .unwrap(),
        call_key
    );
    assert_eq!(MAX_KEY_BYTES, 128);
}

/// T-STREAM-06: все 13 negative-векторов ключа дают точный `StreamKeyError` и код `stream.key.invalid`.
#[test]
fn t_stream_06_negative_stream_key_vectors() {
    let fixture = load_fixture_json();
    let negatives = fixture
        .get("negative")
        .and_then(JsonValue::as_array)
        .expect("negative array");

    let key_negatives: Vec<&JsonValue> = negatives
        .iter()
        .filter(|v| v.get("kind").and_then(JsonValue::as_str) == Some("stream_key"))
        .collect();
    assert_eq!(key_negatives.len(), 13);

    for vector in key_negatives {
        let id = vector.get("id").and_then(JsonValue::as_str).unwrap();
        let raw = vector.get("value").and_then(JsonValue::as_str).unwrap();
        let expect_error = vector
            .get("expectError")
            .and_then(JsonValue::as_str)
            .unwrap();
        let expect_key_error = vector
            .get("expectKeyError")
            .and_then(JsonValue::as_str)
            .unwrap();

        let key_err = StreamKey::parse(raw).unwrap_err();
        let variant_name = match &key_err {
            StreamKeyError::Empty => "Empty",
            StreamKeyError::TooLong { .. } => "TooLong",
            StreamKeyError::UnknownKind { .. } => "UnknownKind",
            StreamKeyError::SegmentCount { .. } => "SegmentCount",
            StreamKeyError::EmptySegment { .. } => "EmptySegment",
            StreamKeyError::NotSnowflake { .. } => "NotSnowflake",
            StreamKeyError::Overflow { .. } => "Overflow",
        };
        assert_eq!(
            variant_name, expect_key_error,
            "несовпадение варианта StreamKeyError для {id} (вход: {raw:?})"
        );
        let stream_err = StreamError::InvalidKey { reason: key_err };
        assert_eq!(stream_err.code(), expect_error, "код ошибки для {id}");
    }
}

/// T-STREAM-07: `Debug`/`Display`/`snapshot` не содержат полной строки ключа, `guild_id`, `token`, `endpoint`.
#[test]
fn t_stream_07_redacted_debug_display_and_snapshot() {
    let wire_key = "guild:100000000000000001:100000000000000002:100000000000000003";
    let secret_raw = "super-secret-stream-rtc-token-999";
    let endpoint_raw = "wss://stream.example.invalid:443/?v=9";

    let key = StreamKey::parse(wire_key).unwrap();
    let key_debug = format!("{key:?}");
    let key_display = format!("{key}");
    assert!(
        !key_debug.contains(wire_key),
        "Debug ключа не должен содержать проводную строку"
    );
    assert!(
        !key_debug.contains("100000000000000001"),
        "Debug ключа не должен содержать guild_id (D-27-6)"
    );
    assert!(!key_display.contains(wire_key));
    assert!(!key_display.contains("100000000000000001"));

    let secret = StreamSecret::new(secret_raw.to_string());
    assert_eq!(secret.expose(), secret_raw);
    assert!(!format!("{secret:?}").contains(secret_raw));
    assert!(!format!("{secret}").contains(secret_raw));
    assert!(format!("{secret:?}").contains("<redacted>"));
    assert_eq!(format!("{secret}"), "<redacted>");

    let server_update = StreamServerUpdate {
        stream_key: key.clone(),
        endpoint: endpoint_raw.to_string(),
        token: secret.clone(),
    };
    let su_debug = format!("{server_update:?}");
    assert!(!su_debug.contains(secret_raw));
    assert!(!su_debug.contains(endpoint_raw));
    assert!(!su_debug.contains(wire_key));

    let mut state = StreamState::default();
    let _ = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(1),
            event: StreamDispatch::Created(StreamCreated {
                stream_key: key.clone(),
                region: Some("us-east".to_string()),
                viewer_ids: Some(vec![100_000_000_000_000_004]),
                rtc_server_id: Some("100000000000000010".to_string()),
                rtc_channel_id: Some(100_000_000_000_000_002),
                paused: Some(false),
            }),
        },
    );
    let effects = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(2),
            event: StreamDispatch::ServerUpdate(server_update),
        },
    );
    let effects_debug = format!("{effects:?}");
    assert!(!effects_debug.contains(secret_raw));
    assert!(!effects_debug.contains(endpoint_raw));
    assert!(!effects_debug.contains(wire_key));

    let snapshot = state.snapshot();
    let snap_debug = format!("{snapshot:?}");
    assert!(!snap_debug.contains(secret_raw));
    assert!(!snap_debug.contains(endpoint_raw));
    assert!(!snap_debug.contains(wire_key));
    assert!(!snap_debug.contains("100000000000000001"));
    assert_eq!(snapshot.streams.len(), 1);
    assert_eq!(snapshot.streams[0].phase, StreamPhase::Active);
    assert_eq!(snapshot.streams[0].phase_wire, "ACTIVE");
    assert_eq!(snapshot.streams[0].viewer_count, Some(1));
    assert!(snapshot.streams[0].transport_ready);
}

/// T-STREAM-08: разбор всех 21 inbound-векторов из фикстуры (`Some([]) != None`, D-27-5).
#[test]
fn t_stream_08_inbound_dispatches_match_fixtures() {
    let fixture = load_fixture_json();
    let inbound = fixture
        .get("inbound")
        .and_then(JsonValue::as_array)
        .expect("inbound array");
    assert_eq!(inbound.len(), 21);

    for vector in inbound {
        let id = vector.get("id").and_then(JsonValue::as_str).unwrap();
        let t = vector.get("t").and_then(JsonValue::as_str).unwrap();
        let payload = json_to_etf(vector.get("payload").unwrap());
        let expect = vector.get("expect").unwrap();
        let variant = expect.get("variant").and_then(JsonValue::as_str).unwrap();

        let parsed = parse_dispatch(t, &payload);
        if variant == "not_a_stream_dispatch" {
            assert!(
                parsed.is_none(),
                "событие {id} ({t}) не должно распознаваться как STREAM_*"
            );
            continue;
        }

        let dispatch = parsed
            .unwrap_or_else(|| panic!("событие {id} ({t}) должно распознаваться"))
            .unwrap_or_else(|e| panic!("событие {id} ({t}) должно разбираться без ошибки: {e}"));

        let expect_key = expect.get("streamKey").and_then(JsonValue::as_str).unwrap();
        match dispatch {
            StreamDispatch::Created(created) => {
                assert_eq!(variant, "created");
                assert_eq!(created.stream_key.to_wire(), expect_key);
                assert_eq!(
                    created.region.as_deref(),
                    expect.get("region").and_then(JsonValue::as_str)
                );
                let expected_viewers: Option<Vec<u64>> = expect
                    .get("viewerIds")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .map(|v| v.as_str().unwrap().parse::<u64>().unwrap())
                            .collect()
                    });
                assert_eq!(
                    created.viewer_ids, expected_viewers,
                    "D-27-5 viewer_ids для {id}"
                );
                assert_eq!(
                    created.rtc_server_id.as_deref(),
                    expect.get("rtcServerId").and_then(JsonValue::as_str)
                );
                let expected_rtc_channel = expect
                    .get("rtcChannelId")
                    .and_then(JsonValue::as_str)
                    .map(|s| s.parse::<u64>().unwrap());
                assert_eq!(created.rtc_channel_id, expected_rtc_channel);
                assert_eq!(
                    created.paused,
                    expect.get("paused").and_then(JsonValue::as_bool)
                );
            }
            StreamDispatch::ServerUpdate(su) => {
                assert_eq!(variant, "server_update");
                assert_eq!(su.stream_key.to_wire(), expect_key);
                assert_eq!(
                    su.endpoint,
                    expect.get("endpoint").and_then(JsonValue::as_str).unwrap()
                );
                assert_eq!(
                    su.token.expose(),
                    expect.get("token").and_then(JsonValue::as_str).unwrap()
                );
            }
            StreamDispatch::Update(upd) => {
                assert_eq!(variant, "update");
                assert_eq!(upd.stream_key.to_wire(), expect_key);
                assert_eq!(
                    upd.region.as_deref(),
                    expect.get("region").and_then(JsonValue::as_str)
                );
                let expected_viewers: Option<Vec<u64>> = expect
                    .get("viewerIds")
                    .and_then(JsonValue::as_array)
                    .map(|arr| {
                        arr.iter()
                            .map(|v| v.as_str().unwrap().parse::<u64>().unwrap())
                            .collect()
                    });
                assert_eq!(upd.viewer_ids, expected_viewers);
                assert_eq!(
                    upd.paused,
                    expect.get("paused").and_then(JsonValue::as_bool)
                );
            }
            StreamDispatch::Deleted(del) => {
                assert_eq!(variant, "deleted");
                assert_eq!(del.stream_key.to_wire(), expect_key);
                assert_eq!(
                    del.unavailable,
                    expect.get("unavailable").and_then(JsonValue::as_bool)
                );
                let expect_reason = expect.get("reason").and_then(JsonValue::as_str);
                let reason_kind = expect
                    .get("reasonKind")
                    .and_then(JsonValue::as_str)
                    .unwrap();
                match (reason_kind, &del.reason) {
                    ("absent", None) => {}
                    ("known", Some(reason)) => {
                        assert!(reason.is_known(), "ожидался известный reason в {id}");
                        assert_eq!(Some(reason.as_wire()), expect_reason);
                    }
                    ("unknown", Some(StreamDeleteReason::Unknown(raw))) => {
                        assert_eq!(Some(raw.as_str()), expect_reason);
                    }
                    other => panic!("неожиданный reason для {id}: {other:?}"),
                }
            }
        }
    }
}

/// T-STREAM-09, T-STREAM-10, T-STREAM-11, T-STREAM-12, T-STREAM-13, T-STREAM-14,
/// T-STREAM-15, T-STREAM-16, T-STREAM-21: полный реплей всех 14 сценариев `reducer` из фикстуры.
#[test]
fn t_stream_09_to_16_and_21_reducer_scenarios_replay() {
    let fixture = load_fixture_json();
    let inbound_list = fixture
        .get("inbound")
        .and_then(JsonValue::as_array)
        .expect("inbound array");
    let mut inbound_by_id: BTreeMap<String, &JsonValue> = BTreeMap::new();
    for item in inbound_list {
        let id = item.get("id").and_then(JsonValue::as_str).unwrap();
        inbound_by_id.insert(id.to_string(), item);
    }

    let scenarios = fixture
        .get("reducer")
        .and_then(JsonValue::as_array)
        .expect("reducer scenarios");
    assert_eq!(scenarios.len(), 14);

    for scenario in scenarios {
        let sc_id = scenario.get("id").and_then(JsonValue::as_str).unwrap();
        let key_str = scenario
            .get("streamKey")
            .and_then(JsonValue::as_str)
            .unwrap();
        let target_key = StreamKey::parse(key_str).unwrap();
        let steps = scenario.get("steps").and_then(JsonValue::as_array).unwrap();

        let mut state = StreamState::default();

        for (step_idx, step) in steps.iter().enumerate() {
            let kind = step.get("do").and_then(JsonValue::as_str).unwrap();
            let effects = match kind {
                "local" => {
                    let local_name = step.get("local").and_then(JsonValue::as_str).unwrap();
                    let action = match local_name {
                        "start_requested" => {
                            let st_kind = StreamKind::from_wire(
                                step.get("kind").and_then(JsonValue::as_str).unwrap(),
                            )
                            .unwrap();
                            let guild_id = step
                                .get("guildId")
                                .and_then(JsonValue::as_str)
                                .map(|s| s.parse::<u64>().unwrap());
                            let channel_id = step
                                .get("channelId")
                                .and_then(JsonValue::as_str)
                                .unwrap()
                                .parse::<u64>()
                                .unwrap();
                            StreamAction::Local(LocalStreamAction::StartRequested {
                                kind: st_kind,
                                guild_id,
                                channel_id,
                            })
                        }
                        "watch_requested" => {
                            StreamAction::Local(LocalStreamAction::WatchRequested {
                                key: target_key.clone(),
                            })
                        }
                        "stop_requested" => StreamAction::Local(LocalStreamAction::StopRequested {
                            key: target_key.clone(),
                        }),
                        "set_paused_requested" => {
                            let paused = step.get("paused").and_then(JsonValue::as_bool).unwrap();
                            StreamAction::Local(LocalStreamAction::SetPausedRequested {
                                key: target_key.clone(),
                                paused,
                            })
                        }
                        "close" => StreamAction::Local(LocalStreamAction::Close {
                            key: target_key.clone(),
                        }),
                        "ready" => StreamAction::Ready,
                        other => panic!("неизвестное локальное действие {other} в {sc_id}"),
                    };
                    apply(&mut state, action)
                }
                "dispatch" => {
                    let vec_id = step.get("vector").and_then(JsonValue::as_str).unwrap();
                    let vec_json = inbound_by_id
                        .get(vec_id)
                        .unwrap_or_else(|| panic!("не найден вектор {vec_id} для {sc_id}"));
                    let t = vec_json.get("t").and_then(JsonValue::as_str).unwrap();
                    let seq = vec_json.get("seq").and_then(JsonValue::as_u64);
                    let payload = json_to_etf(vec_json.get("payload").unwrap());
                    match parse_dispatch_with_diagnostics(t, &payload, &mut state.diagnostics) {
                        None => Vec::new(),
                        Some(Ok(event)) => apply(&mut state, StreamAction::Dispatch { seq, event }),
                        Some(Err(e)) => panic!("ошибка разбора {vec_id} в {sc_id}: {e}"),
                    }
                }
                other => panic!("неизвестный шаг {other} в {sc_id}"),
            };

            let expect = step.get("expect").unwrap();
            let current = state.get(&target_key);

            if let Some(expect_phase_val) = expect.get("phase") {
                match expect_phase_val.as_str() {
                    None => assert!(
                        current.is_none(),
                        "{sc_id} шаг {step_idx}: запись должна отсутствовать"
                    ),
                    Some(expected_phase) => {
                        let stream = current.unwrap_or_else(|| {
                            panic!("{sc_id} шаг {step_idx}: запись должна существовать")
                        });
                        assert_eq!(
                            stream.phase.as_lowercase(),
                            expected_phase,
                            "{sc_id} шаг {step_idx}: неверная фаза"
                        );
                    }
                }
            }

            if let Some(retained) = expect.get("retained").and_then(JsonValue::as_bool) {
                assert_eq!(
                    current.is_some(),
                    retained,
                    "{sc_id} шаг {step_idx}: несовпадение retained"
                );
            }

            if let Some(tr) = expect.get("transportReady").and_then(JsonValue::as_bool) {
                assert_eq!(
                    current.unwrap().transport_ready,
                    tr,
                    "{sc_id} шаг {step_idx}: несовпадение transport_ready"
                );
            }

            if let Some(region) = expect.get("region").and_then(JsonValue::as_str) {
                assert_eq!(
                    current.unwrap().region.as_deref(),
                    Some(region),
                    "{sc_id} шаг {step_idx}: несовпадение region"
                );
            }

            if let Some(viewers) = expect.get("viewerIds").and_then(JsonValue::as_array) {
                let expected_ids: Vec<u64> = viewers
                    .iter()
                    .map(|v| v.as_str().unwrap().parse::<u64>().unwrap())
                    .collect();
                assert_eq!(
                    current.unwrap().viewer_ids.as_ref(),
                    Some(&expected_ids),
                    "{sc_id} шаг {step_idx}: несовпадение viewer_ids"
                );
            }

            if let Some(del_reason) = expect.get("deleteReason").and_then(JsonValue::as_str) {
                assert_eq!(
                    current
                        .unwrap()
                        .delete_reason
                        .as_ref()
                        .map(StreamDeleteReason::as_wire),
                    Some(del_reason),
                    "{sc_id} шаг {step_idx}: несовпадение delete_reason"
                );
            }

            if let Some(stale) = expect.get("staleSequences").and_then(JsonValue::as_array) {
                let expected_stale: Vec<u64> = stale.iter().map(|v| v.as_u64().unwrap()).collect();
                assert_eq!(
                    state.diagnostics.stale_sequences, expected_stale,
                    "{sc_id} шаг {step_idx}: несовпадение stale_sequences"
                );
            }

            if let Some(unk) = expect.get("unknownStreams").and_then(JsonValue::as_u64) {
                assert_eq!(
                    state.diagnostics.unknown_streams, unk,
                    "{sc_id} шаг {step_idx}: несовпадение unknown_streams"
                );
            }

            let expected_effects: Vec<&str> = expect
                .get("effects")
                .and_then(JsonValue::as_array)
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            let actual_effects: Vec<&str> = effects.iter().map(effect_code).collect();
            assert_eq!(
                actual_effects, expected_effects,
                "{sc_id} шаг {step_idx}: несовпадение эффектов"
            );
        }
    }
}

/// Дополнительная проверка T-STREAM-11, T-STREAM-12, T-STREAM-13 (H-PAUSE, orphan_server_updates, unknown_reasons).
#[test]
fn t_stream_11_12_13_hypotheses_and_diagnostics() {
    let key =
        StreamKey::parse("guild:100000000000000001:100000000000000002:100000000000000003").unwrap();
    let mut state = StreamState::default();

    // T-STREAM-13: SERVER_UPDATE без записи -> orphan_server_updates += 1, запись не создаётся, эффект OpenStreamRtc эмитится.
    let effects = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(1),
            event: StreamDispatch::ServerUpdate(StreamServerUpdate {
                stream_key: key.clone(),
                endpoint: "wss://stream.example.invalid/?v=9".to_string(),
                token: StreamSecret::new("REDACTED".to_string()),
            }),
        },
    );
    assert_eq!(state.diagnostics.orphan_server_updates, 1);
    assert!(state.get(&key).is_none());
    assert_eq!(effects.len(), 1);
    assert_eq!(effect_code(&effects[0]), "open_stream_rtc");

    // T-STREAM-12 (H-PAUSE): CREATE{paused:true} -> Paused; UPDATE без paused -> сохраняет Paused; UPDATE{paused:false} -> Active.
    let _ = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(2),
            event: StreamDispatch::Created(StreamCreated {
                stream_key: key.clone(),
                region: Some("us-east".to_string()),
                viewer_ids: Some(vec![]),
                rtc_server_id: None,
                rtc_channel_id: None,
                paused: Some(true),
            }),
        },
    );
    assert_eq!(state.get(&key).unwrap().phase, StreamPhase::Paused);
    assert_eq!(state.snapshot().streams[0].viewer_count, Some(0));

    // UPDATE без paused (paused: None) не должен сбрасывать паузу!
    let _ = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(3),
            event: StreamDispatch::Update(StreamUpdate {
                stream_key: key.clone(),
                region: Some("eu-west".to_string()),
                viewer_ids: None,
                paused: None,
            }),
        },
    );
    assert_eq!(state.get(&key).unwrap().phase, StreamPhase::Paused);
    assert!(state.get(&key).unwrap().paused);
    assert_eq!(state.get(&key).unwrap().region.as_deref(), Some("eu-west"));
    assert_eq!(state.get(&key).unwrap().viewer_ids, Some(vec![]));

    // UPDATE{paused: Some(false)} -> Active.
    let _ = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(4),
            event: StreamDispatch::Update(StreamUpdate {
                stream_key: key.clone(),
                region: None,
                viewer_ids: None,
                paused: Some(false),
            }),
        },
    );
    assert_eq!(state.get(&key).unwrap().phase, StreamPhase::Active);
    assert!(!state.get(&key).unwrap().paused);

    // T-STREAM-11 (D-27-7): неизвестный reason -> Unknown(raw), фаза Ended, unknown_reasons +1.
    let _ = apply(
        &mut state,
        StreamAction::Dispatch {
            seq: Some(5),
            event: StreamDispatch::Deleted(StreamDeleted {
                stream_key: key.clone(),
                unavailable: Some(false),
                reason: Some(StreamDeleteReason::Unknown(
                    "future_reason_2028".to_string(),
                )),
            }),
        },
    );
    assert_eq!(state.get(&key).unwrap().phase, StreamPhase::Ended);
    assert_eq!(
        state.diagnostics.unknown_reasons,
        vec!["future_reason_2028".to_string()]
    );
}

/// T-STREAM-17, T-STREAM-18: все негативные векторы команд (9) и входящих диспатчей (5) + неизвестный `t`.
#[test]
fn t_stream_17_18_negative_command_and_inbound_vectors() {
    let fixture = load_fixture_json();
    let negatives = fixture
        .get("negative")
        .and_then(JsonValue::as_array)
        .expect("negative array");

    let cmd_negatives: Vec<&JsonValue> = negatives
        .iter()
        .filter(|v| v.get("kind").and_then(JsonValue::as_str) == Some("command"))
        .collect();
    assert_eq!(cmd_negatives.len(), 9);

    for vector in cmd_negatives {
        let id = vector.get("id").and_then(JsonValue::as_str).unwrap();
        let expect_error = vector
            .get("expectError")
            .and_then(JsonValue::as_str)
            .unwrap();
        let session = match vector.get("session").and_then(JsonValue::as_str) {
            Some("connecting") => GatewaySessionState::Connecting,
            _ => GatewaySessionState::Established,
        };
        let cmd_spec = json_to_etf(vector.get("command").unwrap());
        let err = StreamCommand::from_raw_spec(&cmd_spec, session)
            .expect_err("команда из negative должна отклоняться");
        assert_eq!(err.code(), expect_error, "неверный код ошибки для {id}");
        assert!(
            !err.to_string().is_empty(),
            "Display ошибки не должен быть пустым"
        );
    }

    let in_negatives: Vec<&JsonValue> = negatives
        .iter()
        .filter(|v| v.get("kind").and_then(JsonValue::as_str) == Some("inbound"))
        .collect();
    assert_eq!(in_negatives.len(), 5);

    for vector in in_negatives {
        let id = vector.get("id").and_then(JsonValue::as_str).unwrap();
        let t = vector.get("t").and_then(JsonValue::as_str).unwrap();
        let payload = json_to_etf(vector.get("payload").unwrap());
        let mut diag = StreamDiagnostics::default();

        let res = parse_dispatch_with_diagnostics(t, &payload, &mut diag)
            .expect("событие STREAM_* должно возвращать Some(...)");

        if let Some(expect_err) = vector.get("expectError").and_then(JsonValue::as_str) {
            let err = res.expect_err("входящий negative-вектор должен давать Err");
            assert_eq!(err.code(), expect_err, "код ошибки для {id}");
            assert_eq!(
                diag.invalid_dispatches, 1,
                "счётчик invalid_dispatches для {id}"
            );
        } else {
            let expect_parsed = vector
                .get("expectParsed")
                .and_then(JsonValue::as_bool)
                .unwrap();
            assert!(expect_parsed);
            let _dispatch =
                res.unwrap_or_else(|e| panic!("мягкий отказ {id} должен разбирать событие: {e}"));
            let expect_diag = vector
                .get("expectDiagnostic")
                .and_then(JsonValue::as_str)
                .unwrap();
            assert_eq!(expect_diag, "malformed_fields");
            assert!(
                !diag.malformed_fields.is_empty(),
                "malformed_fields не должен быть пустым для {id}"
            );
        }
    }

    // T-STREAM-18: неизвестные имена событий возвращают None.
    let empty_map = EtfValue::Map(vec![]);
    assert!(parse_dispatch("STREAM_FOO", &empty_map).is_none());
    assert!(parse_dispatch("stream_create", &empty_map).is_none());
    assert!(parse_dispatch("VOICE_STATE_UPDATE", &empty_map).is_none());
}

/// T-STREAM-19, T-STREAM-20, T-STREAM-22: префиксы ETF-кадров без паники, идемпотентность encode, G5↔G1 стык.
#[test]
fn t_stream_19_20_22_etf_prefixes_idempotency_and_layer_boundary() {
    let fixture = load_fixture_json();
    let outbound = fixture
        .get("outbound")
        .and_then(JsonValue::as_array)
        .unwrap();
    let inbound = fixture
        .get("inbound")
        .and_then(JsonValue::as_array)
        .unwrap();

    // T-STREAM-19 + T-STREAM-20 на исходящих кадрах:
    for vector in outbound {
        let spec_etf = json_to_etf(vector.get("expectCommand").unwrap());
        let cmd =
            StreamCommand::from_raw_spec(&spec_etf, GatewaySessionState::Established).unwrap();
        let first = cmd.encode().unwrap();
        let second = cmd.encode().unwrap();
        assert_eq!(
            first, second,
            "T-STREAM-20: encode обязан быть идемпотентным"
        );

        let bytes = etf::encode(&first);
        for end in 0..bytes.len() {
            assert!(
                etf::decode(&bytes[..end]).is_err(),
                "префикс {end}/{} не должен декодироваться как полный ETF-терм",
                bytes.len()
            );
        }
        let (decoded, consumed) = etf::decode(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        let reparsed = StreamCommand::from_etf_frame(&decoded).unwrap();
        assert_eq!(reparsed, cmd);
    }

    // T-STREAM-19 на входящих кадрах:
    for vector in inbound {
        let t = vector.get("t").and_then(JsonValue::as_str).unwrap();
        let payload = json_to_etf(vector.get("payload").unwrap());
        let bytes = etf::encode(&payload);
        for end in 0..bytes.len() {
            if let Ok((partial, _)) = etf::decode(&bytes[..end]) {
                let _ = parse_dispatch(t, &partial);
            }
        }
    }

    // T-STREAM-22: ключ строится только из {kind, guild_id, channel_id, owner_id},
    // без участия source_id/кадров захвата; некорректные комбинации kind/guild_id отвергаются.
    assert_eq!(
        StreamKey::build(StreamKind::Guild, None, 10, 20),
        Err(StreamError::MissingGuildId)
    );
    assert_eq!(
        StreamKey::build(StreamKind::Call, Some(1), 10, 20),
        Err(StreamError::UnexpectedGuildId)
    );
    assert_eq!(
        StreamKey::build(StreamKind::Call, None, 0, 20),
        Err(StreamError::InvalidId {
            field: "channel_id"
        })
    );
    assert_eq!(
        StreamKey::build(StreamKind::Call, None, 10, 0),
        Err(StreamError::InvalidId { field: "owner_id" })
    );
}
