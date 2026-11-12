//! Контрактные тесты этапа S2 (`op:37 GUILD_SUBSCRIPTIONS_BULK`) и этапов V2–V4
//! в цикле Voice WS `v=8` (`rust/crates/discord-core/src/ws.rs`):
//! - S2: `RangedGuildSubscriptions` и `build_guild_subscriptions_bulk_op37` — двухшаговая
//!   подписка (`typing:true, threads:true, activities:true` -> непустые диапазоны `[[0, 99]]`),
//!   строгий запрет отправки пустого `channels: {}` или пустых диапазонов `[]` для уже
//!   подписанной гильдии с удержанием fallback `[[0, 99]]`.
//! - V2: отслеживание `seq_ack: Option<u16>` на каждом входящем текстовом (JSON `"seq"`)
//!   и бинарном фрейме (`u16::from_be_bytes([bin[0], bin[1]])` при `len >= 3`), передача
//!   `seq_ack` в `op:3 Heartbeat` и `op:7 Resume`.
//! - V3: в pure-Rust ветке (`dv_stdin.is_none()`) при получении `op:4` (`dave_protocol_version >= 1`)
//!   немедленная отправка бинарного `op:26 KeyPackage`, обработка бинарных фреймов DAVE
//!   (`op:25`, `op:27` -> бинарный `op:28`, `op:29/30` -> JSON `op:23` или `op:31` + свежий `op:26`).
//! - V4: счётчик `UdpFailures` (10 ошибок отправки UDP подряд -> переподключение, сброс при успехе).

use chord_codec::etf::{self, EtfValue};
use chord_core::ws::{
    DEFAULT_GUILD_SUBSCRIPTION_RANGE, PureRustVoiceV8State, RangedGuildSubscriptions, UdpFailures,
    VOICE_UDP_MAX_CONSECUTIVE_FAILURES, VoiceV8OutboundFrame, build_guild_subscriptions_bulk_op37,
    build_voice_v8_heartbeat_op3, build_voice_v8_resume_op7, drive_voice_v8_ws_step,
    update_voice_v8_seq_ack_from_binary, update_voice_v8_seq_ack_from_text,
};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::io::DuplexStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{WebSocketStream, accept_async, client_async};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

fn decode_op37_frame(raw: &[u8]) -> EtfValue {
    assert_eq!(
        raw.first(),
        Some(&0x83),
        "кадр ETF обязан начинаться с байта версии 0x83"
    );
    let (val, consumed) = etf::decode(raw).expect("валидный ETF-терм");
    assert_eq!(
        consumed,
        raw.len(),
        "ровно один ETF-терм без лишнего хвоста"
    );
    assert_eq!(
        val.get("op").and_then(EtfValue::as_int),
        Some(37),
        "ожидался op:37 GUILD_SUBSCRIPTIONS_BULK"
    );
    val
}

fn extract_channel_ranges(sub_entry: &EtfValue, channel_id: &str) -> Vec<[i64; 2]> {
    let channels = sub_entry
        .get("channels")
        .expect("поле channels обязано присутствовать")
        .as_map()
        .expect("channels должен быть ETF Map");
    assert!(
        !channels.is_empty(),
        "channels: {{}} запрещён для подписанной гильдии"
    );
    let ranges_val = sub_entry
        .get("channels")
        .and_then(|c| c.get(channel_id))
        .and_then(EtfValue::as_list)
        .expect("диапазоны канала должны быть списком");
    assert!(
        !ranges_val.is_empty(),
        "пустой список диапазонов [] запрещён"
    );
    ranges_val
        .iter()
        .map(|pair| {
            let items = pair.as_list().expect("диапазон [start, end]");
            assert_eq!(items.len(), 2);
            [
                items[0].as_int().expect("start int"),
                items[1].as_int().expect("end int"),
            ]
        })
        .collect()
}

#[test]
fn s2_two_step_op37_guild_subscription_and_etf_wire_contract() {
    assert_eq!(DEFAULT_GUILD_SUBSCRIPTION_RANGE, [0, 99]);

    let mut subs = RangedGuildSubscriptions::new();
    let guild_id = "1014537851418980372";
    let channel_a = "1176785373989445703";
    let channel_b = "1275727867497680918";

    assert!(!subs.is_subscribed(guild_id));
    assert_eq!(subs.active_channel(guild_id), None);

    // 1. Первая подписка на гильдию — строго двухшаговая (2 бинарных ETF-кадра op:37):
    //    Шаг 1: typing:true, threads:true, activities:true (без каналов)
    //    Шаг 2: непустые диапазоны каналов [[0, 99]]
    let frames_first =
        build_guild_subscriptions_bulk_op37(&mut subs, guild_id, channel_a, &[[0, 99]]);
    assert_eq!(
        frames_first.len(),
        2,
        "первая подписка на гильдию обязана эмитить 2 кадра op:37 (шаг 1: флаги, шаг 2: каналы)"
    );
    assert!(subs.is_subscribed(guild_id));
    assert_eq!(subs.active_channel(guild_id), Some(channel_a));
    assert_eq!(subs.active_ranges(guild_id), Some(&[[0, 99]][..]));

    let step1 = decode_op37_frame(&frames_first[0]);
    let sub1 = step1
        .get("d")
        .and_then(|d| d.get("subscriptions"))
        .and_then(|s| s.get(guild_id))
        .expect("запись гильдии в шаге 1");
    assert_eq!(sub1.get("typing").and_then(EtfValue::as_atom), Some("true"));
    assert_eq!(
        sub1.get("threads").and_then(EtfValue::as_atom),
        Some("true")
    );
    assert_eq!(
        sub1.get("activities").and_then(EtfValue::as_atom),
        Some("true")
    );
    assert!(
        sub1.get("channels").is_none(),
        "шаг 1 не должен отправлять пустой channels:{{}}"
    );

    let step2 = decode_op37_frame(&frames_first[1]);
    let sub2 = step2
        .get("d")
        .and_then(|d| d.get("subscriptions"))
        .and_then(|s| s.get(guild_id))
        .expect("запись гильдии в шаге 2");
    assert_eq!(sub2.get("typing").and_then(EtfValue::as_atom), Some("true"));
    assert_eq!(
        sub2.get("threads").and_then(EtfValue::as_atom),
        Some("true")
    );
    assert_eq!(
        sub2.get("activities").and_then(EtfValue::as_atom),
        Some("true")
    );
    assert_eq!(extract_channel_ranges(sub2, channel_a), vec![[0, 99]]);

    // 2. Переключение канала в уже подписанной гильдии — 1 кадр op:37 с непустыми диапазонами:
    let frames_second =
        build_guild_subscriptions_bulk_op37(&mut subs, guild_id, channel_b, &[[0, 99], [100, 199]]);
    assert_eq!(frames_second.len(), 1);
    assert_eq!(subs.active_channel(guild_id), Some(channel_b));
    let decoded_second = decode_op37_frame(&frames_second[0]);
    let sub_second = decoded_second
        .get("d")
        .and_then(|d| d.get("subscriptions"))
        .and_then(|s| s.get(guild_id))
        .expect("запись гильдии во втором вызове");
    assert_eq!(
        extract_channel_ranges(sub_second, channel_b),
        vec![[0, 99], [100, 199]]
    );
}

#[test]
fn s2_already_subscribed_guild_forbids_empty_channels_and_empty_ranges_with_fallback_0_99() {
    let mut subs = RangedGuildSubscriptions::new();
    let guild_id = "853389359330148352";
    let channel_id = "853389359330148355";

    // Граничный случай: неподписанная гильдия с пустым guild_id или пустым channel_id не шлёт кадров
    assert!(build_guild_subscriptions_bulk_op37(&mut subs, "", channel_id, &[]).is_empty());
    assert!(build_guild_subscriptions_bulk_op37(&mut subs, "nil", channel_id, &[]).is_empty());
    assert!(build_guild_subscriptions_bulk_op37(&mut subs, guild_id, "", &[]).is_empty());
    assert!(!subs.is_subscribed(guild_id));

    // Первичная подписка с пустым списком диапазонов `&[]` обязана подставить fallback `[[0, 99]]` на шаге 2
    let init_frames = build_guild_subscriptions_bulk_op37(&mut subs, guild_id, channel_id, &[]);
    assert_eq!(init_frames.len(), 2);
    let step2 = decode_op37_frame(&init_frames[1]);
    let sub2 = step2
        .get("d")
        .and_then(|d| d.get("subscriptions"))
        .and_then(|s| s.get(guild_id))
        .expect("запись гильдии");
    assert_eq!(extract_channel_ranges(sub2, channel_id), vec![[0, 99]]);

    // Попытка передать пустые диапазоны `&[]` для УЖЕ подписанной гильдии:
    // запрещено отправлять `[]`, удерживается fallback `[[0, 99]]`
    let empty_ranges_frames =
        build_guild_subscriptions_bulk_op37(&mut subs, guild_id, channel_id, &[]);
    assert_eq!(empty_ranges_frames.len(), 1);
    let decoded_empty_ranges =
        decode_op37_ranges_for_guild(&empty_ranges_frames[0], guild_id, channel_id);
    assert_eq!(decoded_empty_ranges, vec![[0, 99]]);

    // Попытка передать невалидные диапазоны (ширина != 99, не кратно 100) для УЖЕ подписанной гильдии:
    // отбраковываются и удерживается fallback `[[0, 99]]`
    let invalid_ranges_frames = build_guild_subscriptions_bulk_op37(
        &mut subs,
        guild_id,
        channel_id,
        &[[0, 50], [15, 114], [200, 100]],
    );
    assert_eq!(invalid_ranges_frames.len(), 1);
    assert_eq!(
        decode_op37_ranges_for_guild(&invalid_ranges_frames[0], guild_id, channel_id),
        vec![[0, 99]]
    );

    // Попытка передать пустой `channel_id` (`""` или `"nil"`) для УЖЕ подписанной гильдии:
    // запрещено отправлять `channels: {}`, удерживается последний активный канал и fallback `[[0, 99]]`
    for empty_cid in ["", "   ", "nil"] {
        let retained_frames =
            build_guild_subscriptions_bulk_op37(&mut subs, guild_id, empty_cid, &[]);
        assert_eq!(retained_frames.len(), 1);
        assert_eq!(
            decode_op37_ranges_for_guild(&retained_frames[0], guild_id, channel_id),
            vec![[0, 99]],
            "при пустом channel_id={empty_cid:?} для подписанной гильдии обязан сохраняться активный канал и [[0, 99]]"
        );
    }
}

fn decode_op37_ranges_for_guild(raw: &[u8], guild_id: &str, channel_id: &str) -> Vec<[i64; 2]> {
    let val = decode_op37_frame(raw);
    let sub = val
        .get("d")
        .and_then(|d| d.get("subscriptions"))
        .and_then(|s| s.get(guild_id))
        .expect("запись гильдии в op:37");
    extract_channel_ranges(sub, channel_id)
}

#[test]
fn v2_voice_v8_seq_ack_tracks_text_and_binary_frames_and_populates_op3_and_op7() {
    let mut seq_ack: Option<u16> = None;

    // 1. До получения seq в op:3 Heartbeat и op:7 Resume передаётся seq_ack: -1
    let hb0 = build_voice_v8_heartbeat_op3(1_700_000_000_001, seq_ack);
    let hb0_json = chord_json::parse(&hb0).expect("valid heartbeat json");
    assert_eq!(hb0_json.get("op").and_then(|v| v.as_i64()), Some(3));
    assert_eq!(
        hb0_json
            .get("d")
            .and_then(|d| d.get("t"))
            .and_then(|v| v.as_i64()),
        Some(1_700_000_000_001)
    );
    assert_eq!(
        hb0_json
            .get("d")
            .and_then(|d| d.get("seq_ack"))
            .and_then(|v| v.as_i64()),
        Some(-1)
    );

    let res0 = build_voice_v8_resume_op7(
        "1014537851418980372",
        "1176785373989445703",
        "voice-sess-01",
        "voice-tok-01",
        seq_ack,
    );
    let res0_json = chord_json::parse(&res0).expect("valid resume json");
    assert_eq!(res0_json.get("op").and_then(|v| v.as_i64()), Some(7));
    assert_eq!(
        res0_json
            .get("d")
            .and_then(|d| d.get("seq_ack"))
            .and_then(|v| v.as_i64()),
        Some(-1)
    );

    // 2. Текстовый фрейм без поля seq не меняет seq_ack
    assert_eq!(
        update_voice_v8_seq_ack_from_text(
            &mut seq_ack,
            r#"{"op":8,"d":{"v":8,"heartbeat_interval":13750}}"#,
        ),
        None
    );
    assert_eq!(seq_ack, None);

    // 3. Текстовый фрейм с JSON seq обновляет seq_ack: Option<u16>
    assert_eq!(
        update_voice_v8_seq_ack_from_text(&mut seq_ack, r#"{"op":15,"seq":42,"d":{"any":100}}"#),
        Some(42)
    );
    assert_eq!(seq_ack, Some(42));

    // 4. Бинарный фрейм (len >= 3) обновляет seq_ack = u16::from_be_bytes([bin[0], bin[1]])
    let bin_op25 = [0x01, 0x05, 25, 0xde, 0xad, 0xbe, 0xef];
    let parsed_bin =
        update_voice_v8_seq_ack_from_binary(&mut seq_ack, &bin_op25).expect("len >= 3");
    assert_eq!(parsed_bin.0, 0x0105);
    assert_eq!(parsed_bin.1, 25);
    assert_eq!(parsed_bin.2, &[0xde, 0xad, 0xbe, 0xef]);
    assert_eq!(seq_ack, Some(0x0105));

    // Граница u16::MAX (65535) и минимальная длина len == 3 (пустой payload)
    let bin_max = [0xff, 0xff, 27];
    let parsed_max = update_voice_v8_seq_ack_from_binary(&mut seq_ack, &bin_max).expect("len == 3");
    assert_eq!(parsed_max.0, u16::MAX);
    assert_eq!(parsed_max.1, 27);
    assert!(parsed_max.2.is_empty());
    assert_eq!(seq_ack, Some(u16::MAX));

    // Короткий бинарный фрейм (len < 3) игнорируется и не портит seq_ack
    assert_eq!(
        update_voice_v8_seq_ack_from_binary(&mut seq_ack, &[0x00, 0x01]),
        None
    );
    assert_eq!(seq_ack, Some(u16::MAX));

    // 5. Актуальный seq_ack передаётся в op:3 Heartbeat и op:7 Resume
    let hb1 = build_voice_v8_heartbeat_op3(1_700_000_000_002, seq_ack);
    let hb1_json = chord_json::parse(&hb1).expect("hb1 json");
    assert_eq!(
        hb1_json
            .get("d")
            .and_then(|d| d.get("seq_ack"))
            .and_then(|v| v.as_i64()),
        Some(i64::from(u16::MAX))
    );

    let res1 = build_voice_v8_resume_op7(
        "1014537851418980372",
        "1176785373989445703",
        "voice-sess-01",
        "voice-tok-01",
        seq_ack,
    );
    let res1_json = chord_json::parse(&res1).expect("res1 json");
    assert_eq!(res1_json.get("op").and_then(|v| v.as_i64()), Some(7));
    let d = res1_json.get("d").expect("d in op:7");
    assert_eq!(
        d.get("server_id").and_then(|v| v.as_str()),
        Some("1014537851418980372")
    );
    assert_eq!(
        d.get("channel_id").and_then(|v| v.as_str()),
        Some("1176785373989445703")
    );
    assert_eq!(
        d.get("session_id").and_then(|v| v.as_str()),
        Some("voice-sess-01")
    );
    assert_eq!(
        d.get("token").and_then(|v| v.as_str()),
        Some("voice-tok-01")
    );
    assert_eq!(
        d.get("seq_ack").and_then(|v| v.as_i64()),
        Some(i64::from(u16::MAX))
    );
}

async fn ws_pair() -> (WebSocketStream<DuplexStream>, WebSocketStream<DuplexStream>) {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let (client_res, server_res) = tokio::join!(
        client_async("ws://localhost/?v=8", client_io),
        accept_async(server_io)
    );
    (
        client_res.expect("client handshake").0,
        server_res.expect("server handshake"),
    )
}

#[tokio::test]
async fn v3_pure_rust_voice_v8_op4_sends_op26_and_handles_dave_binary_op25_op27_op29_op30() {
    let (mut client_ws, mut server_ws) = ws_pair().await;
    let mut state = PureRustVoiceV8State::new(
        "1014537851418980372",
        "1176785373989445703",
        "384299970632876032",
        "voice-sess-dave",
        "voice-tok-dave",
    );
    assert_eq!(state.seq_ack(), None);

    // 1. Получение op:4 (dave_protocol_version >= 1) в pure-Rust ветке немедленно отправляет бинарный op:26 KeyPackage
    let op4_json = r#"{"op":4,"seq":3,"d":{"mode":"aead_xchacha20_poly1305_rtpsize","dave_protocol_version":1,"secret_key":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32]}}"#;
    server_ws
        .send(Message::Text(op4_json.into()))
        .await
        .expect("server send op4");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step op4");
    assert_eq!(state.seq_ack(), Some(3));

    let msg_op26 = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for op26")
        .expect("stream open")
        .expect("ws msg");
    let initial_op26 = match msg_op26 {
        Message::Binary(b) => b.to_vec(),
        other => panic!("ожидался бинарный кадр op:26 KeyPackage, получено: {other:?}"),
    };
    assert_eq!(initial_op26.first().copied(), Some(26));
    assert!(
        initial_op26.len() > 16,
        "op:26 KeyPackage должен содержать непустой MLS-ключ"
    );

    // 2. Входящий бинарный op:25 ExternalSenderPackage обновляет seq_ack и сохраняет внешние ключи
    let op25_frame: Vec<u8> = vec![
        0x00, 0x0a, 25, 0xaa, 0xbb, 0xcc, 0xdd, 0x11, 0x22, 0x33, 0x44,
    ];
    server_ws
        .send(Message::Binary(op25_frame.into()))
        .await
        .expect("server send op25");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step op25");
    assert_eq!(state.seq_ack(), Some(10));

    // 3. Входящий бинарный op:27 Proposals обновляет seq_ack и отправляет бинарный op:28 CommitWelcome
    let mut op27_frame: Vec<u8> = vec![0x00, 0x0b, 27, 0x00];
    op27_frame.extend_from_slice(&555_666_777_888_999_000u64.to_be_bytes());
    op27_frame.extend_from_slice(&initial_op26[1..]);
    server_ws
        .send(Message::Binary(op27_frame.into()))
        .await
        .expect("server send op27");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step op27");
    assert_eq!(state.seq_ack(), Some(11));

    let msg_op28 = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for op28")
        .expect("stream open")
        .expect("ws msg");
    let op28_bytes = match msg_op28 {
        Message::Binary(b) => b.to_vec(),
        other => panic!("ожидался бинарный кадр op:28 CommitWelcome, получено: {other:?}"),
    };
    assert_eq!(op28_bytes.first().copied(), Some(28));
    let commit_body = &op28_bytes[1..];
    assert!(!commit_body.is_empty());

    // 4. Входящий валидный бинарный op:29 PrepareCommitTransition -> отправка JSON op:23 ReadyForTransition
    let mut op29_valid = vec![0x00, 0x0c, 29];
    op29_valid.extend_from_slice(&7u16.to_be_bytes()); // transition_id = 7
    op29_valid.extend_from_slice(commit_body);
    server_ws
        .send(Message::Binary(op29_valid.into()))
        .await
        .expect("server send op29");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step op29");
    assert_eq!(state.seq_ack(), Some(12));

    let msg_op23 = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for op23")
        .expect("stream open")
        .expect("ws msg");
    match msg_op23 {
        Message::Text(t) => {
            let parsed = chord_json::parse(&t).expect("valid op23 json");
            assert_eq!(parsed.get("op").and_then(|v| v.as_i64()), Some(23));
            assert_eq!(
                parsed
                    .get("d")
                    .and_then(|d| d.get("transition_id"))
                    .and_then(|v| v.as_i64()),
                Some(7)
            );
        }
        other => panic!("ожидался текстовый JSON op:23, получено: {other:?}"),
    }

    // 5. Входящий валидный бинарный op:30 Welcome -> отправка JSON op:23 ReadyForTransition
    let mut op30_valid = vec![0x00, 0x0d, 30];
    op30_valid.extend_from_slice(&8u16.to_be_bytes()); // transition_id = 8
    op30_valid.extend_from_slice(commit_body);
    server_ws
        .send(Message::Binary(op30_valid.into()))
        .await
        .expect("server send op30");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step op30");
    assert_eq!(state.seq_ack(), Some(13));

    let msg_op23_welcome = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for op23 on op30")
        .expect("stream open")
        .expect("ws msg");
    match msg_op23_welcome {
        Message::Text(t) => {
            let parsed = chord_json::parse(&t).expect("valid op23 json");
            assert_eq!(parsed.get("op").and_then(|v| v.as_i64()), Some(23));
            assert_eq!(
                parsed
                    .get("d")
                    .and_then(|d| d.get("transition_id"))
                    .and_then(|v| v.as_i64()),
                Some(8)
            );
        }
        other => panic!("ожидался текстовый JSON op:23 на op:30, получено: {other:?}"),
    }

    // 6. Входящий невалидный/битый op:29/30 -> отправка JSON op:31 InvalidCommitWelcome + свежий бинарный op:26 KeyPackage
    let corrupt_op30 = vec![0x00, 0x0e, 30, 0x00, 0x09, 0xff, 0x00, 0xfe, 0x01]; // seq=14, op=30, tid=9, corrupt body
    server_ws
        .send(Message::Binary(corrupt_op30.into()))
        .await
        .expect("server send corrupt op30");
    drive_voice_v8_ws_step(&mut client_ws, &mut state)
        .await
        .expect("client step corrupt op30");
    assert_eq!(state.seq_ack(), Some(14));

    let msg_op31 = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for op31")
        .expect("stream open")
        .expect("ws msg");
    match msg_op31 {
        Message::Text(t) => {
            let parsed = chord_json::parse(&t).expect("valid op31 json");
            assert_eq!(parsed.get("op").and_then(|v| v.as_i64()), Some(31));
            assert_eq!(
                parsed
                    .get("d")
                    .and_then(|d| d.get("transition_id"))
                    .and_then(|v| v.as_i64()),
                Some(9)
            );
        }
        other => panic!("ожидался текстовый JSON op:31, получено: {other:?}"),
    }

    let msg_fresh_op26 = timeout(TEST_TIMEOUT, server_ws.next())
        .await
        .expect("timeout waiting for fresh op26")
        .expect("stream open")
        .expect("ws msg");
    let fresh_op26 = match msg_fresh_op26 {
        Message::Binary(b) => b.to_vec(),
        other => panic!("ожидался свежий бинарный op:26 после op:31, получено: {other:?}"),
    };
    assert_eq!(fresh_op26.first().copied(), Some(26));
    assert_ne!(
        fresh_op26, initial_op26,
        "свежий op:26 KeyPackage после сброса обязан иметь новое поколение ключа"
    );

    // Также проверяем, что при dave_protocol_version == 0 op:26 не отправляется
    let mut legacy_state = PureRustVoiceV8State::new("1", "2", "3", "s", "t");
    let legacy_frames = legacy_state
        .on_text_frame(r#"{"op":4,"seq":1,"d":{"mode":"aead_xchacha20_poly1305_rtpsize","dave_protocol_version":0}}"#)
        .expect("legacy op4");
    assert!(
        legacy_frames
            .iter()
            .all(|f| !matches!(f, VoiceV8OutboundFrame::Binary(b) if b.first() == Some(&26))),
        "при dave_protocol_version == 0 бинарный op:26 отправляться не должен"
    );
}

#[tokio::test]
async fn v4_udp_failures_ten_consecutive_errors_trigger_reconnect_and_success_resets() {
    assert_eq!(VOICE_UDP_MAX_CONSECUTIVE_FAILURES, 10);

    let mut failures = UdpFailures::new();
    assert_eq!(failures.consecutive_failures(), 0);
    assert!(!failures.should_reconnect());

    // 1..=9 ошибок подряд не вызывают реконнект
    for expected in 1..=9 {
        assert!(
            failures
                .record_send_result::<usize, &str>(Err("would_block"))
                .is_ok()
        );
        assert_eq!(failures.consecutive_failures(), expected);
        assert!(!failures.should_reconnect());
    }

    // Успешная отправка сбрасывает счётчик ошибок в 0
    assert!(failures.record_send_result::<usize, &str>(Ok(74)).is_ok());
    assert_eq!(failures.consecutive_failures(), 0);
    assert!(!failures.should_reconnect());

    // 10 ошибок подряд: на 10-й возвращается ошибка переподключения
    for _ in 1..=9 {
        assert!(!failures.record_failure());
    }
    assert_eq!(failures.consecutive_failures(), 9);
    let tenth = failures.record_send_result::<usize, &str>(Err("network_unreachable"));
    assert!(
        tenth.is_err(),
        "10-я ошибка UDP подряд обязана инициировать переподключение"
    );
    assert_eq!(failures.consecutive_failures(), 10);
    assert!(failures.should_reconnect());

    // Проверка UdpFailures::send_packet на реальном неподключённом UDP-сокете (ошибка EDESTADDRREQ / NotConnected)
    let unconnected_sock = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind local udp");
    let mut sock_failures = UdpFailures::new();
    for attempt in 1..=9 {
        let res = sock_failures
            .send_packet(&unconnected_sock, &[0x13, 0x37, 0xca, 0xfe, 1, 0, 0, 0])
            .await;
        assert!(
            res.is_ok(),
            "ошибка #{attempt} (< 10) не должна рвать сессию"
        );
        assert_eq!(sock_failures.consecutive_failures(), attempt);
    }
    let tenth_sock_res = sock_failures
        .send_packet(&unconnected_sock, &[0x13, 0x37, 0xca, 0xfe, 10, 0, 0, 0])
        .await;
    assert!(
        tenth_sock_res.is_err(),
        "10-я подряд ошибка отправки в UdpSocket обязана вернуть Err для переподключения"
    );
    assert!(sock_failures.should_reconnect());
}
