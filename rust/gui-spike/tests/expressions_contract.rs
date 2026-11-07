//! RW-40 / task-242 (`BE-P1-26`): контрактные тесты BFF-модуля кастомных эмодзи и стикеров гильдии (`expressions`).
//!
//! Проверяет:
//! 1. Константы:
//!    - `MIN_EMOJI_NAME_CHARS = 2`, `MAX_EMOJI_NAME_CHARS = 32`, `MAX_EMOJI_BYTES = 256 * 1024`;
//!    - `MIN_STICKER_NAME_CHARS = 2`, `MAX_STICKER_NAME_CHARS = 30`, `MAX_STICKER_DESC_CHARS = 100`;
//!    - `STICKER_FORMAT_PNG = 1`, `STICKER_FORMAT_APNG = 2`, `STICKER_FORMAT_LOTTIE = 3`, `STICKER_FORMAT_GIF = 4`.
//! 2. Структуру `CreateGuildEmojiRequest` (`name`, `image_data_uri`, `roles`) и её serde round-trip.
//! 3. `validate_emoji_name(name: &str) -> Result<(), String>`:
//!    - длина `name.trim()` должна быть в диапазоне `2..=32` (`MIN_EMOJI_NAME_CHARS..=MAX_EMOJI_NAME_CHARS`);
//!    - все символы после `trim()` должны быть ASCII-буквенно-цифровыми или `'_'` (`c.is_ascii_alphanumeric() || c == '_'`).
//! 4. `validate_create_guild_emoji(req: &CreateGuildEmojiRequest, byte_len: u64) -> Result<(), String>`:
//!    - вызывает `validate_emoji_name(&req.name)`;
//!    - `byte_len` в диапазоне `1..=MAX_EMOJI_BYTES` (`1..=262_144`);
//!    - `req.image_data_uri` начинается с `"data:image/png;base64,"`, `"data:image/gif;base64,"`,
//!      `"data:image/jpeg;base64,"` или `"data:image/webp;base64,"` и содержит непустой base64-пейлоад после запятой;
//!    - каждый элемент в `req.roles` является валидным положительным числовым Snowflake (`1..=20` ASCII-цифр, не начинается с `'0'`).
//! 5. `build_guild_emojis_route(guild_id: &str, emoji_id: Option<&str>) -> Result<String, String>`:
//!    - валидирует `guild_id` (и `emoji_id`, если `Some`) как положительные числовые Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
//!    - возвращает `"/api/v9/guilds/{guild_id}/emojis"` или `"/api/v9/guilds/{guild_id}/emojis/{emoji_id}"`.

use discord_gui_spike::expressions::{
    build_guild_emojis_route, validate_create_guild_emoji, validate_emoji_name,
    CreateGuildEmojiRequest, MAX_EMOJI_BYTES, MAX_EMOJI_NAME_CHARS, MAX_STICKER_DESC_CHARS,
    MAX_STICKER_NAME_CHARS, MIN_EMOJI_NAME_CHARS, MIN_STICKER_NAME_CHARS, STICKER_FORMAT_APNG,
    STICKER_FORMAT_GIF, STICKER_FORMAT_LOTTIE, STICKER_FORMAT_PNG,
};
use serde_json::json;

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_EMOJI_ID: &str = "100000000000000002";
const VALID_ROLE_ID_1: &str = "100000000000000003";
const VALID_ROLE_ID_2: &str = "100000000000000004";

fn sample_create_emoji_request() -> CreateGuildEmojiRequest {
    CreateGuildEmojiRequest {
        name: "party_parrot_2".to_string(),
        image_data_uri: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string(),
        roles: vec![VALID_ROLE_ID_1.to_string(), VALID_ROLE_ID_2.to_string()],
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры CreateGuildEmojiRequest
// ---------------------------------------------------------------------------

#[test]
fn expressions_constants_match_discord_contract() {
    assert_eq!(MIN_EMOJI_NAME_CHARS, 2);
    assert_eq!(MAX_EMOJI_NAME_CHARS, 32);
    assert_eq!(MAX_EMOJI_BYTES, 256 * 1024);
    assert_eq!(MAX_EMOJI_BYTES, 262_144);

    assert_eq!(MIN_STICKER_NAME_CHARS, 2);
    assert_eq!(MAX_STICKER_NAME_CHARS, 30);
    assert_eq!(MAX_STICKER_DESC_CHARS, 100);

    assert_eq!(STICKER_FORMAT_PNG, 1);
    assert_eq!(STICKER_FORMAT_APNG, 2);
    assert_eq!(STICKER_FORMAT_LOTTIE, 3);
    assert_eq!(STICKER_FORMAT_GIF, 4);
}

#[test]
fn create_guild_emoji_request_serde_roundtrip() {
    let req = sample_create_emoji_request();
    let val = serde_json::to_value(&req).expect("serialize CreateGuildEmojiRequest");
    assert_eq!(
        val,
        json!({
            "name": "party_parrot_2",
            "image_data_uri": req.image_data_uri,
            "roles": [VALID_ROLE_ID_1, VALID_ROLE_ID_2]
        })
    );
    let back: CreateGuildEmojiRequest =
        serde_json::from_value(val).expect("deserialize CreateGuildEmojiRequest");
    assert_eq!(back, req);
}

// ---------------------------------------------------------------------------
// 2. validate_emoji_name: границы длины (2..=32) и набор символов [A-Za-z0-9_]
// ---------------------------------------------------------------------------

#[test]
fn validate_emoji_name_accepts_valid_trimmed_ascii_alphanumeric_and_underscore_names() {
    let exact_32 = "a_B_0_9_".repeat(4);
    assert_eq!(exact_32.len(), MAX_EMOJI_NAME_CHARS);

    for valid in [
        "ab",
        "__a",
        "__",
        "A1",
        "party_parrot_2026",
        &exact_32,
        &format!("  {exact_32}  "),
        "  ok  ",
    ] {
        assert!(
            validate_emoji_name(valid).is_ok(),
            "emoji name {valid:?} should be valid"
        );
    }
}

#[test]
fn validate_emoji_name_rejects_short_long_non_ascii_and_invalid_punctuation() {
    let too_long_33 = "a".repeat(MAX_EMOJI_NAME_CHARS + 1);
    assert_eq!(too_long_33.len(), 33);

    for invalid in [
        "",
        "   ",
        "a",
        "  a  ",
        "_",
        &too_long_33,
        &format!("  {too_long_33}  "),
        "party-parrot",
        "party parrot",
        "party.parrot",
        "party:parrot",
        "смайл",
        "emoji🎉",
    ] {
        assert!(
            validate_emoji_name(invalid).is_err(),
            "emoji name {invalid:?} should be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. validate_create_guild_emoji: byte_len (1..=256 KiB), data URI, roles, name
// ---------------------------------------------------------------------------

#[test]
fn validate_create_guild_emoji_enforces_byte_len_and_supported_image_data_uris() {
    let mut req = sample_create_emoji_request();

    // Границы byte_len: 1 и MAX_EMOJI_BYTES (256 * 1024) -> Ok
    assert!(validate_create_guild_emoji(&req, 1).is_ok());
    assert!(validate_create_guild_emoji(&req, MAX_EMOJI_BYTES).is_ok());

    // Невалидные byte_len: 0, MAX_EMOJI_BYTES + 1, u64::MAX -> Err
    for bad_bytes in [0u64, MAX_EMOJI_BYTES + 1, u64::MAX] {
        assert!(
            validate_create_guild_emoji(&req, bad_bytes).is_err(),
            "byte_len={bad_bytes} must be rejected"
        );
    }

    // Все 4 поддерживаемых префикса MIME-типов с непустым base64-пейлоадом -> Ok
    for valid_uri in [
        "data:image/png;base64,QUJD",
        "data:image/gif;base64,R0lGODlhAQABAAAAACw=",
        "data:image/jpeg;base64,/9j/4AAQSkZJRg==",
        "data:image/webp;base64,UklGRh4AAABXRUJQVlA4TBEAAAAvAAAAAAfQ//73v/+BiOh/AAA=",
    ] {
        req.image_data_uri = valid_uri.to_string();
        assert!(
            validate_create_guild_emoji(&req, 1024).is_ok(),
            "image_data_uri={valid_uri:?} must be accepted"
        );
    }

    // Невалидные префиксы или пустой пейлоад после запятой -> Err
    for bad_uri in [
        "",
        "data:image/png;base64,",
        "data:image/gif;base64,",
        "data:image/jpeg;base64,",
        "data:image/webp;base64,",
        "data:image/png;base64,   ",
        "data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=",
        "data:image/bmp;base64,QUJD",
        "data:text/plain;base64,QUJD",
        "https://cdn.discordapp.com/emojis/123.png",
        "iVBORw0KGgoAAAANSUhEUg==",
    ] {
        req.image_data_uri = bad_uri.to_string();
        assert!(
            validate_create_guild_emoji(&req, 1024).is_err(),
            "image_data_uri={bad_uri:?} must be rejected"
        );
    }
}

#[test]
fn validate_create_guild_emoji_validates_name_and_role_snowflakes() {
    let mut req = sample_create_emoji_request();

    // Пустой список ролей допустим
    req.roles.clear();
    assert!(validate_create_guild_emoji(&req, 1024).is_ok());

    // Граничные положительные снежинки ролей (1..=20 цифр, без ведущего '0') -> Ok
    let twenty_nines = "9".repeat(20);
    req.roles = vec!["1".to_string(), VALID_ROLE_ID_1.to_string(), twenty_nines];
    assert!(validate_create_guild_emoji(&req, 1024).is_ok());

    // Невалидное имя эмодзи отклоняет весь запрос
    req.name = "a".to_string();
    assert!(validate_create_guild_emoji(&req, 1024).is_err());
    req.name = "valid_emoji".to_string();

    // Любая невалидная снежинка в roles отклоняет запрос
    let twenty_one_digits = "1".repeat(21);
    for bad_role in [
        "",
        "0",
        "00",
        "0123456789",
        "abc",
        "123a456",
        "-1",
        " 12345",
        "12345 ",
        &twenty_one_digits,
    ] {
        req.roles = vec![VALID_ROLE_ID_1.to_string(), bad_role.to_string()];
        assert!(
            validate_create_guild_emoji(&req, 1024).is_err(),
            "role={bad_role:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. build_guild_emojis_route: маршрут списка и конкретного эмодзи
// ---------------------------------------------------------------------------

#[test]
fn build_guild_emojis_route_formats_collection_and_item_routes_and_validates_snowflakes() {
    let collection_route =
        build_guild_emojis_route(VALID_GUILD_ID, None).expect("valid collection route");
    assert_eq!(collection_route, "/api/v9/guilds/100000000000000001/emojis");

    let item_route =
        build_guild_emojis_route(VALID_GUILD_ID, Some(VALID_EMOJI_ID)).expect("valid item route");
    assert_eq!(
        item_route,
        "/api/v9/guilds/100000000000000001/emojis/100000000000000002"
    );

    let max_20 = "9".repeat(20);
    let boundary_route =
        build_guild_emojis_route("1", Some(&max_20)).expect("boundary snowflakes route");
    assert_eq!(boundary_route, format!("/api/v9/guilds/1/emojis/{max_20}"));

    let twenty_one_digits = "1".repeat(21);
    for bad in [
        "",
        "0",
        "00",
        "012345",
        "abc",
        "123a456",
        "-1",
        " 123",
        "123 ",
        &twenty_one_digits,
    ] {
        assert!(
            build_guild_emojis_route(bad, None).is_err(),
            "guild_id={bad:?} (None emoji_id) must fail"
        );
        assert!(
            build_guild_emojis_route(bad, Some(VALID_EMOJI_ID)).is_err(),
            "guild_id={bad:?} (Some emoji_id) must fail"
        );
        assert!(
            build_guild_emojis_route(VALID_GUILD_ID, Some(bad)).is_err(),
            "emoji_id={bad:?} must fail"
        );
    }
}
