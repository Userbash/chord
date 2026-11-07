//! RW-40 / task-205: контрактные тесты BFF-модуля голосовой сцены и запуска демонстрации экрана (`voice_stage`).
//!
//! Проверяет:
//! 1. Константу `MAX_STAGE_VISIBLE_TILES == 25`.
//! 2. Перечисления `StreamResolution` (`P720`, `P1080`, `P1440`) и `StreamFps` (`Fps30`, `Fps60`)
//!    с хелперами `width()`, `height()`, `fps()`, `label()` и serde round-trip.
//! 3. Структуру `ScreenShareSpec` (`guild_id`, `channel_id`, `source_id`, `resolution`, `fps`, `share_audio`):
//!    - `validate(&self) -> Result<(), String>`: проверка ненулевых числовых Snowflake ID (`1..=u64::MAX`, без ведущих нулей)
//!      для `channel_id` и `guild_id` (если `Some`), а также непустого `source_id`;
//!    - `to_stream_key(&self, user_id: &str) -> String` и `to_stream_Key(&self, user_id: &str) -> String`:
//!      канонический формат `'guild:{gid}:{cid}:{uid}'` или `'call:{cid}:{uid}'` (сверка с RE-оракулом `A-KEY-BUILD`).
//! 4. Построители JSON-кадров Gateway opcodes 18 и 19 (`A-OPCODES`, `A-SENDERS`, `D-27-2`):
//!    - `build_gateway_stream_create_op18(spec: &ScreenShareSpec) -> Result<serde_json::Value, String>`
//!      (`{"op": 18, "d": {"type": "guild"|"call", "guild_id": "<gid>"|null, "channel_id": "<cid>", "preferred_region": null}}`);
//!    - `build_gateway_stream_delete_op19(stream_key: &str) -> Result<serde_json::Value, String>`
//!      (`{"op": 19, "d": {"stream_key": "guild:..."|"call:..."}}`) со строгой валидацией сегментов `stream_key` (`A-KEY-PARSE`).

use discord_gui_spike::voice_stage::{
    build_gateway_stream_create_op18, build_gateway_stream_delete_op19,
    build_gateway_stream_ping_op21, build_gateway_stream_set_paused_op22,
    build_gateway_stream_watch_op20, ScreenShareSpec, StreamFps, StreamResolution,
    MAX_STAGE_VISIBLE_TILES,
};
use serde_json::json;

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_CHANNEL_ID: &str = "100000000000000002";
const VALID_USER_ID: &str = "100000000000000003";

// ---------------------------------------------------------------------------
// 1. Константа MAX_STAGE_VISIBLE_TILES и хелперы StreamResolution / StreamFps
// ---------------------------------------------------------------------------

#[test]
fn max_stage_visible_tiles_and_resolution_fps_helpers_match_spec() {
    assert_eq!(MAX_STAGE_VISIBLE_TILES, 25);

    assert_eq!(StreamResolution::P720.width(), 1280);
    assert_eq!(StreamResolution::P720.height(), 720);
    assert_eq!(StreamResolution::P720.dimensions(), (1280, 720));
    assert_eq!(StreamResolution::P720.label(), "720p");

    assert_eq!(StreamResolution::P1080.width(), 1920);
    assert_eq!(StreamResolution::P1080.height(), 1080);
    assert_eq!(StreamResolution::P1080.dimensions(), (1920, 1080));
    assert_eq!(StreamResolution::P1080.label(), "1080p");

    assert_eq!(StreamResolution::P1440.width(), 2560);
    assert_eq!(StreamResolution::P1440.height(), 1440);
    assert_eq!(StreamResolution::P1440.dimensions(), (2560, 1440));
    assert_eq!(StreamResolution::P1440.label(), "1440p");

    assert_eq!(StreamFps::Fps30.fps(), 30);
    assert_eq!(StreamFps::Fps60.fps(), 60);
}

#[test]
fn resolution_and_fps_serde_roundtrip() {
    for res in [
        StreamResolution::P720,
        StreamResolution::P1080,
        StreamResolution::P1440,
    ] {
        let encoded = serde_json::to_value(res).expect("serialize StreamResolution");
        let decoded: StreamResolution =
            serde_json::from_value(encoded).expect("deserialize StreamResolution");
        assert_eq!(decoded, res);
    }

    for fps in [StreamFps::Fps30, StreamFps::Fps60] {
        let encoded = serde_json::to_value(fps).expect("serialize StreamFps");
        let decoded: StreamFps = serde_json::from_value(encoded).expect("deserialize StreamFps");
        assert_eq!(decoded, fps);
    }

    // Проверяем десериализацию FPS из чисел (30, 60) и строк ("30", "60", "Fps30", "Fps60")
    assert_eq!(
        serde_json::from_value::<StreamFps>(serde_json::json!(30)).unwrap(),
        StreamFps::Fps30
    );
    assert_eq!(
        serde_json::from_value::<StreamFps>(serde_json::json!(60)).unwrap(),
        StreamFps::Fps60
    );
    assert_eq!(
        serde_json::from_value::<StreamFps>(serde_json::json!("30")).unwrap(),
        StreamFps::Fps30
    );
    assert_eq!(
        serde_json::from_value::<StreamFps>(serde_json::json!("60")).unwrap(),
        StreamFps::Fps60
    );

    // Проверяем десериализацию ScreenShareSpec из camelCase JSON (фронтенд-контракт)
    let json_camel = serde_json::json!({
        "guildId": "111222333444555666",
        "channelId": "222333444555666777",
        "sourceId": "screen:0",
        "resolution": "1080p",
        "fps": 60,
        "shareAudio": true
    });
    let decoded_camel: ScreenShareSpec = serde_json::from_value(json_camel).unwrap();
    assert_eq!(decoded_camel.guild_id.as_deref(), Some("111222333444555666"));
    assert_eq!(decoded_camel.channel_id, "222333444555666777");
    assert_eq!(decoded_camel.source_id, "screen:0");
    assert_eq!(decoded_camel.resolution, StreamResolution::P1080);
    assert_eq!(decoded_camel.fps, StreamFps::Fps60);
    assert!(decoded_camel.share_audio);
}

// ---------------------------------------------------------------------------
// 2. Валидация ScreenShareSpec (позитивные, граничные 1..=u64::MAX и негативные)
// ---------------------------------------------------------------------------

#[test]
fn screen_share_spec_validate_accepts_valid_guild_and_call_specs_including_boundaries() {
    let guild_spec = ScreenShareSpec {
        guild_id: Some(VALID_GUILD_ID.to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "screen:0:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    assert!(guild_spec.validate().is_ok());

    let call_spec = ScreenShareSpec {
        guild_id: None,
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "window:42:0".to_string(),
        resolution: StreamResolution::P720,
        fps: StreamFps::Fps30,
        share_audio: false,
    };
    assert!(call_spec.validate().is_ok());

    // Граничные значения Snowflake ID: "1" и u64::MAX ("18446744073709551615")
    let max_id = u64::MAX.to_string();
    let boundary_spec = ScreenShareSpec {
        guild_id: Some("1".to_string()),
        channel_id: max_id,
        source_id: "screen:1".to_string(),
        resolution: StreamResolution::P1440,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    assert!(boundary_spec.validate().is_ok());
}

#[test]
fn screen_share_spec_validate_rejects_invalid_snowflakes_and_empty_source_id() {
    let invalid_ids = [
        "",
        "0",
        "0000",
        "012345",
        "abc",
        "123a456",
        "-1",
        " 100000000000000001",
        "100000000000000001 ",
        "18446744073709551616", // u64::MAX + 1
    ];

    for bad_id in invalid_ids {
        let bad_channel = ScreenShareSpec {
            guild_id: None,
            channel_id: bad_id.to_string(),
            source_id: "screen:0:0".to_string(),
            resolution: StreamResolution::P1080,
            fps: StreamFps::Fps60,
            share_audio: false,
        };
        assert!(
            bad_channel.validate().is_err(),
            "channel_id={bad_id:?} must be rejected"
        );

        let bad_guild = ScreenShareSpec {
            guild_id: Some(bad_id.to_string()),
            channel_id: VALID_CHANNEL_ID.to_string(),
            source_id: "screen:0:0".to_string(),
            resolution: StreamResolution::P1080,
            fps: StreamFps::Fps60,
            share_audio: false,
        };
        assert!(
            bad_guild.validate().is_err(),
            "guild_id={bad_id:?} must be rejected"
        );
    }

    for bad_source in ["", "   ", "\t\n"] {
        let bad_src_spec = ScreenShareSpec {
            guild_id: Some(VALID_GUILD_ID.to_string()),
            channel_id: VALID_CHANNEL_ID.to_string(),
            source_id: bad_source.to_string(),
            resolution: StreamResolution::P1080,
            fps: StreamFps::Fps60,
            share_audio: true,
        };
        assert!(
            bad_src_spec.validate().is_err(),
            "source_id={bad_source:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Построение stream_key (`to_stream_Key` и `to_stream_key`)
// ---------------------------------------------------------------------------

#[test]
fn to_stream_key_builds_canonical_guild_and_call_keys() {
    let guild_spec = ScreenShareSpec {
        guild_id: Some(VALID_GUILD_ID.to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "screen:0:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    let expected_guild_key =
        "guild:100000000000000001:100000000000000002:100000000000000003".to_string();
    assert_eq!(guild_spec.to_stream_Key(VALID_USER_ID), expected_guild_key);
    assert_eq!(guild_spec.to_stream_key(VALID_USER_ID), expected_guild_key);

    let call_spec = ScreenShareSpec {
        guild_id: None,
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "window:1:0".to_string(),
        resolution: StreamResolution::P720,
        fps: StreamFps::Fps30,
        share_audio: false,
    };
    let expected_call_key = "call:100000000000000002:100000000000000003".to_string();
    assert_eq!(call_spec.to_stream_Key(VALID_USER_ID), expected_call_key);
    assert_eq!(call_spec.to_stream_key(VALID_USER_ID), expected_call_key);
}

// ---------------------------------------------------------------------------
// 4. Gateway Opcode 18 (STREAM_CREATE) и Opcode 19 (STREAM_DELETE)
// ---------------------------------------------------------------------------

#[test]
fn build_gateway_stream_create_op18_produces_exact_oracle_payloads_for_guild_and_call() {
    let guild_spec = ScreenShareSpec {
        guild_id: Some(VALID_GUILD_ID.to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "screen:0:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    let guild_frame =
        build_gateway_stream_create_op18(&guild_spec).expect("valid guild op18 payload");
    assert_eq!(
        guild_frame,
        json!({
            "op": 18,
            "d": {
                "type": "guild",
                "guild_id": VALID_GUILD_ID,
                "channel_id": VALID_CHANNEL_ID,
                "preferred_region": null
            }
        })
    );

    let call_spec = ScreenShareSpec {
        guild_id: None,
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "window:10:0".to_string(),
        resolution: StreamResolution::P720,
        fps: StreamFps::Fps30,
        share_audio: false,
    };
    let call_frame = build_gateway_stream_create_op18(&call_spec).expect("valid call op18 payload");
    assert_eq!(
        call_frame,
        json!({
            "op": 18,
            "d": {
                "type": "call",
                "guild_id": null,
                "channel_id": VALID_CHANNEL_ID,
                "preferred_region": null
            }
        })
    );
}

#[test]
fn build_gateway_stream_create_op18_rejects_invalid_spec() {
    let bad_spec = ScreenShareSpec {
        guild_id: Some("0".to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "screen:0:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: false,
    };
    assert!(build_gateway_stream_create_op18(&bad_spec).is_err());

    let empty_source = ScreenShareSpec {
        guild_id: None,
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: false,
    };
    assert!(build_gateway_stream_create_op18(&empty_source).is_err());
}

#[test]
fn build_gateway_stream_delete_op19_produces_exact_oracle_payloads_and_validates_stream_key() {
    let guild_key = "guild:100000000000000001:100000000000000002:100000000000000003";
    let guild_delete =
        build_gateway_stream_delete_op19(guild_key).expect("valid guild op19 payload");
    assert_eq!(
        guild_delete,
        json!({
            "op": 19,
            "d": {
                "stream_key": guild_key
            }
        })
    );

    let call_key = "call:100000000000000002:100000000000000003";
    let call_delete = build_gateway_stream_delete_op19(call_key).expect("valid call op19 payload");
    assert_eq!(
        call_delete,
        json!({
            "op": 19,
            "d": {
                "stream_key": call_key
            }
        })
    );

    for bad_key in [
        "",
        "   ",
        "guild:100:200",
        "guild:100:200:300:400",
        "call:100",
        "call:100:200:300",
        "dm:100:200",
        "guild:0:100000000000000002:100000000000000003",
        "guild:100000000000000001:0:100000000000000003",
        "guild:100000000000000001:100000000000000002:0",
        "call:0:100000000000000003",
        "call:100000000000000002:abc",
        "guild:0100:200:300",
        "guild:100:200:18446744073709551616",
    ] {
        assert!(
            build_gateway_stream_delete_op19(bad_key).is_err(),
            "stream_key={bad_key:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Построители Gateway opcodes 20, 21, 22 (STREAM_WATCH, STREAM_PING, STREAM_SET_PAUSED)
// ---------------------------------------------------------------------------

#[test]
fn build_gateway_stream_watch_op20_produces_exact_oracle_payload_and_validates_key() {
    let key = "guild:100000000000000001:100000000000000002:100000000000000003";
    let watch = build_gateway_stream_watch_op20(key).expect("valid op20 payload");
    assert_eq!(
        watch,
        json!({
            "op": 20,
            "d": {
                "stream_key": key
            }
        })
    );

    assert!(build_gateway_stream_watch_op20("invalid:key").is_err());
    assert!(build_gateway_stream_watch_op20("").is_err());
}

#[test]
fn build_gateway_stream_ping_op21_produces_exact_oracle_payload_and_validates_key() {
    let key = "call:100000000000000002:100000000000000003";
    let ping = build_gateway_stream_ping_op21(key).expect("valid op21 payload");
    assert_eq!(
        ping,
        json!({
            "op": 21,
            "d": {
                "stream_key": key
            }
        })
    );

    assert!(build_gateway_stream_ping_op21("bad_key").is_err());
}

#[test]
fn build_gateway_stream_set_paused_op22_produces_exact_oracle_payload_and_validates_key() {
    let key = "guild:100000000000000001:100000000000000002:100000000000000003";
    let paused = build_gateway_stream_set_paused_op22(key, true).expect("valid op22 payload");
    assert_eq!(
        paused,
        json!({
            "op": 22,
            "d": {
                "stream_key": key,
                "paused": true
            }
        })
    );

    let resumed = build_gateway_stream_set_paused_op22(key, false).expect("valid op22 payload");
    assert_eq!(
        resumed,
        json!({
            "op": 22,
            "d": {
                "stream_key": key,
                "paused": false
            }
        })
    );

    assert!(build_gateway_stream_set_paused_op22("", true).is_err());
}
