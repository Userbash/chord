//! P2-08: settings-proto decoder (RW-21).
//!
//! Оракул — `waves/RW-21/oracle.py` и реальные READY/UPDATE-фикстуры из
//! `/home/sanya/re-deep` (read-only). Тесты не вызывают Python и не сверяются с
//! текущей реализацией: ожидания захардкожены из независимого разбора.

use discord_core::settings::{
    FRECENCY_AND_FAVORITES_SETTINGS_TYPE, MAX_FIELDS, MAX_PROTO_BYTES,
    PRELOADED_USER_SETTINGS_TYPE, ProtoMessage, ProtoValue, SettingsProto, SettingsProtoError,
    decode_settings,
};

const READY: &str = include_str!("data/settings-preloaded-ready-4400.b64");
const UPDATE_8269: &str = include_str!("data/settings-frecency-update-8269.b64");
const UPDATE_8608: &str = include_str!("data/settings-frecency-update-8608.b64");
const UPDATE_13072: &str = include_str!("data/settings-frecency-update-13072.b64");

fn ready() -> SettingsProto {
    decode_settings(PRELOADED_USER_SETTINGS_TYPE, READY.trim()).expect("READY settings")
}

fn preloaded() -> discord_core::settings::PreloadedUserSettings {
    let SettingsProto::Preloaded(settings) = ready() else {
        panic!("ожидался PreloadedUserSettings");
    };
    *settings
}

fn length(value: &ProtoValue) -> usize {
    match value {
        ProtoValue::LengthDelimited(bytes) => bytes.len(),
        other => panic!("ожидался length-delimited, получено {other:?}"),
    }
}

/// t01: READY — валидный base64 ровно 4400 символов, type 1, 18 top-level полей.
#[test]
fn p08t01_ready_identity() {
    assert_eq!(READY.trim().len(), 4400);
    let settings = ready();
    assert_eq!(settings.proto_type(), PRELOADED_USER_SETTINGS_TYPE);
    let SettingsProto::Preloaded(p) = settings else {
        panic!("ожидался PreloadedUserSettings");
    };
    assert_eq!(p.raw.fields.len(), 18);
}

/// t02: точный набор top-level полей READY и last-wins для повторного scalar.
#[test]
fn p08t02_ready_top_level_fields() {
    let p = preloaded();
    let numbers: Vec<u32> = p.raw.fields.iter().map(|field| field.number).collect();
    assert_eq!(
        numbers,
        vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 16, 19, 23, 25
        ]
    );

    let duplicate = ProtoMessage::decode(&[0x08, 0x01, 0x08, 0x02]).expect("duplicate scalar");
    assert_eq!(
        duplicate.scalar_u64(1).expect("scalar"),
        Some(2),
        "singular last-wins"
    );
}

/// t03: versions + inbox из READY.
#[test]
fn p08t03_versions_and_inbox() {
    let p = preloaded();
    let versions = p.versions.expect("versions");
    assert_eq!(versions.client_version, Some(21));
    assert_eq!(versions.server_version, None);
    assert_eq!(versions.data_version, Some(3027));

    let inbox = p.inbox.expect("inbox");
    assert_eq!(inbox.current_tab, Some(2));
    assert_eq!(inbox.viewed_tutorial, None);
}

/// t04: appearance — theme/density/timestamp/developer mode.
#[test]
fn p08t04_appearance() {
    let appearance = preloaded().appearance.expect("appearance");
    assert_eq!(appearance.theme, Some(3)); // DARKER
    assert_eq!(appearance.developer_mode, Some(true));
    assert_eq!(appearance.timestamp_hour_cycle, Some(2)); // H23
    assert_eq!(appearance.ui_density, Some(1)); // COMPACT
    assert_eq!(appearance.mobile_redesign_disabled, None);
    assert_eq!(appearance.dark_sidebar, None);
}

/// t05: voice_and_video — bool wrappers, Int32Value и пустой wrapper = false.
#[test]
fn p08t05_voice_and_video() {
    let voice = preloaded().voice_and_video.expect("voice_and_video");
    assert_eq!(voice.always_preview_video, Some(true));
    assert_eq!(voice.afk_timeout, Some(600));
    assert_eq!(voice.stream_notifications_enabled, Some(true));
    assert_eq!(voice.native_phone_integration_enabled, Some(true));
    assert_eq!(voice.disable_stream_previews, Some(false));
    assert_eq!(voice.preset_option, None);
}

/// t06: text_and_images — значения из READY, включая enum/int32 wrappers.
#[test]
fn p08t06_text_and_images() {
    let text = preloaded().text_and_images.expect("text_and_images");
    assert_eq!(text.view_image_descriptions, Some(true));
    assert_eq!(text.inline_attachment_media, Some(true));
    assert_eq!(text.inline_embed_media, Some(true));
    assert_eq!(text.gif_auto_play, Some(false));
    assert_eq!(text.render_embeds, Some(true));
    assert_eq!(text.render_reactions, Some(true));
    assert_eq!(text.animate_emoji, Some(false));
    assert_eq!(text.animate_stickers, Some(2));
    assert_eq!(text.enable_tts_command, Some(false));
    assert_eq!(text.message_display_compact, Some(false));
    assert_eq!(text.explicit_content_filter, Some(2));
    assert_eq!(text.view_nsfw_guilds, Some(false));
    assert_eq!(text.convert_emoticons, Some(true));
    assert_eq!(text.expression_suggestions_enabled, Some(true));
    assert_eq!(text.view_nsfw_commands, Some(false));
    assert_eq!(text.dm_spam_filter_v2, Some(3)); // FRIENDS_AND_NON_FRIENDS
}

/// t07: notifications — bool wrapper и fixed64 id.
#[test]
fn p08t07_notifications() {
    let notifications = preloaded().notifications.expect("notifications");
    assert_eq!(notifications.notify_friends_on_go_live, Some(false));
    assert_eq!(
        notifications.notification_center_acked_before_id,
        Some(1_041_362_756_533_751_819)
    );
    assert_eq!(notifications.quiet_mode, None);
}

/// t08: status + localization, включая signed int64 timezone.
#[test]
fn p08t08_status_and_localization() {
    let p = preloaded();
    let status = p.status.expect("status");
    assert_eq!(status.status.as_deref(), Some("invisible"));
    assert_eq!(status.show_current_game, Some(false));
    assert_eq!(status.status_created_at_ms, Some(1_766_220_108_588));
    assert_eq!(status.status_expires_at_ms, None);

    let localization = p.localization.expect("localization");
    assert_eq!(localization.locale.as_deref(), Some("ru"));
    assert_eq!(localization.timezone_offset, Some(-300));
    assert_eq!(localization.timezone_name, None);
}

/// t09: неизвестные top-level поля сохраняются структурно, не теряются.
#[test]
fn p08t09_raw_preserves_unknown_fields() {
    let p = preloaded();
    for number in [3, 4, 8, 10, 14, 16, 19, 23, 25] {
        let field = p
            .raw
            .field(number)
            .unwrap_or_else(|| panic!("поле {number}"));
        assert!(length(&field.value) > 0, "поле {number} пустое");
    }
    // Debug-секция в этом снимке — валидное пустое сообщение, а не отсутствие поля.
    assert!(p.raw.field(9).is_some());
    assert!(p.raw.field(13).is_some(), "appearance");
}

/// t10: все три UPDATE — type 2, Frecency, одинаковый top-level, versions растут.
#[test]
fn p08t10_update_type2_frecency() {
    for (text, data_version) in [
        (UPDATE_8269, 7545),
        (UPDATE_8608, 7546),
        (UPDATE_13072, 7547),
    ] {
        assert_eq!(text.trim().len(), 17_436);
        let settings =
            decode_settings(FRECENCY_AND_FAVORITES_SETTINGS_TYPE, text.trim()).expect("type 2");
        assert_eq!(settings.proto_type(), FRECENCY_AND_FAVORITES_SETTINGS_TYPE);
        let SettingsProto::Frecency(frecency) = settings else {
            panic!("type 2 не должен быть Preloaded");
        };
        let numbers: Vec<u32> = frecency
            .raw
            .fields
            .iter()
            .map(|field| field.number)
            .collect();
        assert_eq!(numbers, vec![1, 2, 4, 6, 7, 9, 11, 12, 13]);
        let versions = frecency.versions.expect("versions");
        assert_eq!(versions.client_version, Some(10));
        assert_eq!(versions.server_version, None);
        assert_eq!(versions.data_version, Some(data_version));
    }
}

/// t11: неизвестный type не ошибка — generic wire сохраняется.
#[test]
fn p08t11_unsupported_type_preserves_wire() {
    let settings = SettingsProto::parse(3, &[0x08, 0x07]).expect("unknown type");
    assert_eq!(settings.proto_type(), 3);
    let SettingsProto::Unsupported {
        proto_type,
        message,
    } = settings
    else {
        panic!("ожидался Unsupported");
    };
    assert_eq!(proto_type, 3);
    assert_eq!(message.scalar_u64(1).expect("scalar"), Some(7));
}

/// t12: ошибки base64 не маскируются.
#[test]
fn p08t12_base64_errors() {
    for text in ["A", "!!!!", "AA=A"] {
        assert!(
            matches!(
                decode_settings(PRELOADED_USER_SETTINGS_TYPE, text),
                Err(SettingsProtoError::Base64(_))
            ),
            "ожидалась Base64-ошибка для {text:?}"
        );
    }
}

/// t13: ошибки wire: обрыв, field 0, group, length overflow, oversize.
#[test]
fn p08t13_wire_errors() {
    assert!(matches!(
        ProtoMessage::decode(&[0x80]),
        Err(SettingsProtoError::UnexpectedEnd { .. })
    ));
    assert!(matches!(
        ProtoMessage::decode(&[
            0x08, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00,
        ]),
        Err(SettingsProtoError::VarintOverflow { .. })
    ));
    assert!(matches!(
        ProtoMessage::decode(&[0x00]),
        Err(SettingsProtoError::InvalidFieldNumber { number: 0, .. })
    ));
    assert!(matches!(
        ProtoMessage::decode(&[0x0b]),
        Err(SettingsProtoError::UnsupportedWireType { wire_type: 3, .. })
    ));
    assert!(matches!(
        ProtoMessage::decode(&[0x0a, 0x05, 0x00]),
        Err(SettingsProtoError::LengthOutOfBounds { field: 1, .. })
    ));
    assert!(matches!(
        ProtoMessage::decode(&vec![0_u8; MAX_PROTO_BYTES + 1]),
        Err(SettingsProtoError::TooLarge { .. })
    ));
}

/// t14: известное поле с чужим wire type — ошибка, не «угадывание».
#[test]
fn p08t14_known_field_wrong_wire_type() {
    let err = SettingsProto::parse(PRELOADED_USER_SETTINGS_TYPE, &[0x28, 0x01]).unwrap_err();
    assert!(
        matches!(
            err,
            SettingsProtoError::WrongWireType {
                field: 5,
                expected: 2,
                actual: 0
            }
        ),
        "получено {err:?}"
    );
}

/// t15: пустой wrapper — значение false, отсутствие поля — None.
#[test]
fn p08t15_empty_wrapper_vs_absent() {
    let settings = SettingsProto::parse(PRELOADED_USER_SETTINGS_TYPE, &[0x2a, 0x02, 0x52, 0x00])
        .expect("voice");
    let SettingsProto::Preloaded(p) = settings else {
        panic!("ожидался Preloaded");
    };
    let voice = p.voice_and_video.expect("voice section");
    assert_eq!(voice.disable_stream_previews, Some(false));
    assert_eq!(voice.always_preview_video, None);

    let empty = SettingsProto::parse(PRELOADED_USER_SETTINGS_TYPE, &[]).expect("empty");
    let SettingsProto::Preloaded(empty) = empty else {
        panic!("ожидался Preloaded");
    };
    assert!(empty.raw.fields.is_empty());
    assert!(empty.voice_and_video.is_none());
    assert!(empty.appearance.is_none());
}

/// t16: Debug не печатает строки состояния, locale и fixed64 id.
#[test]
fn p08t16_debug_redacts_payload() {
    let p = preloaded();
    let debug = format!("{p:?}");
    assert!(!debug.contains("invisible"), "{debug}");
    assert!(!debug.contains("1041362756533751819"), "{debug}");
    assert!(!debug.contains("\"ru\""), "{debug}");
    assert!(
        !debug.contains("status_created_at_ms: Some(1766220108588)"),
        "{debug}"
    );

    let status = p.status.as_ref().expect("status");
    let status_debug = format!("{status:?}");
    assert!(!status_debug.contains("invisible"), "{status_debug}");
    assert!(status_debug.contains("status_len"), "{status_debug}");
}

/// t17: Debug самих секций (не только outer) не печатает fixed64 id/locale/timezone.
#[test]
fn p08t17_section_debug_redacts_sensitive_values() {
    let p = preloaded();
    let notifications = p.notifications.as_ref().expect("notifications");
    let debug = format!("{notifications:?}");
    assert!(!debug.contains("1041362756533751819"), "{debug}");
    assert!(
        debug.contains("notification_center_acked_before_id_present"),
        "{debug}"
    );

    let localization = p.localization.as_ref().expect("localization");
    let debug = format!("{localization:?}");
    assert!(!debug.contains("\"ru\""), "{debug}");
    assert!(!debug.contains("Europe/"), "{debug}");
    assert!(debug.contains("locale_len"), "{debug}");
    assert!(debug.contains("timezone_name_len"), "{debug}");
}

/// t18: граница MAX_FIELDS: ровно лимит принимается, лимит+1 — TooManyFields.
#[test]
fn p08t18_max_fields_boundary() {
    // Каждое поле — tag 0x08 (field 1, varint) + значение 0, два байта.
    let at_limit = [0x08, 0x00].repeat(MAX_FIELDS);
    let message = ProtoMessage::decode(&at_limit).expect("ровно MAX_FIELDS допустимо");
    assert_eq!(message.fields.len(), MAX_FIELDS);

    let over = [0x08, 0x00].repeat(MAX_FIELDS + 1);
    assert!(matches!(
        ProtoMessage::decode(&over),
        Err(SettingsProtoError::TooManyFields { limit }) if limit == MAX_FIELDS
    ));
}
