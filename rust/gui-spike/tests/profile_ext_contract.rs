//! RW-40 / task-221 (BE-P1-19): контрактные тесты модуля `profile_ext` (`profile_ext_contract`).
//!
//! Проверяет:
//! 1. Константы `MAX_BIO_CHARS = 190`, `MAX_PRONOUNS_CHARS = 40`, `MAX_NOTE_CHARS = 256`.
//! 2. `UserProfileQuery` и `UpdateProfilePatch` — `Debug`, `Clone`, `PartialEq`, `Eq`, serde round-trip.
//! 3. `build_user_profile_route(q)`:
//!    - валидирует `user_id` (и `guild_id`, если `Some`) как канонические положительные `u64`
//!      Snowflake-строки (`1..=u64::MAX`, без ведущих нулей);
//!    - строит `/users/{user_id}/profile?with_mutual_guilds={true|false}&with_mutual_friends={true|false}[&guild_id={gid}]`.
//! 4. `validate_profile_patch(patch)`:
//!    - `bio: Some(b)` с `b.chars().count() > 190` -> `Err`; `None` и `0..=190` (включая Unicode) -> `Ok`;
//!    - `pronouns: Some(p)` с `p.chars().count() > 40` -> `Err`; `None` и `0..=40` (включая Unicode) -> `Ok`;
//!    - `accent_color: Some(c)` с `c > 0xFF_FFFF` -> `Err`; `None` и `0..=0xFF_FFFF` -> `Ok`.
//! 5. `validate_user_note(target_user_id, note)`:
//!    - валидирует `target_user_id` как канонический положительный `u64` Snowflake (`1..=u64::MAX`, без ведущих нулей);
//!    - проверяет `note.chars().count() <= 256` (Unicode scalar values);
//!    - возвращает `Ok("/users/@me/notes/{target_user_id}")`.

use discord_gui_spike::profile_ext::{
    build_user_profile_route, validate_profile_patch, validate_user_note, UpdateProfilePatch,
    UserProfileQuery, MAX_BIO_CHARS, MAX_NOTE_CHARS, MAX_PRONOUNS_CHARS,
};

const VALID_USER_ID: &str = "1014537851418980371";
const VALID_GUILD_ID: &str = "1014537851418980372";

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структур
// ---------------------------------------------------------------------------

#[test]
fn profile_ext_constants_match_discord_contract() {
    assert_eq!(MAX_BIO_CHARS, 190);
    assert_eq!(MAX_PRONOUNS_CHARS, 40);
    assert_eq!(MAX_NOTE_CHARS, 256);
}

#[test]
fn profile_ext_structs_serde_roundtrip() {
    let query = UserProfileQuery {
        user_id: VALID_USER_ID.to_string(),
        guild_id: Some(VALID_GUILD_ID.to_string()),
        with_mutual_guilds: true,
        with_mutual_friends: false,
    };
    let query_json = serde_json::to_string(&query).expect("serialize UserProfileQuery");
    let restored_query: UserProfileQuery =
        serde_json::from_str(&query_json).expect("deserialize UserProfileQuery");
    assert_eq!(restored_query, query);

    let patch = UpdateProfilePatch {
        bio: Some("Разработчик Rust и Tauri".to_string()),
        pronouns: Some("он/его".to_string()),
        accent_color: Some(0x58_65F2),
    };
    let patch_json = serde_json::to_string(&patch).expect("serialize UpdateProfilePatch");
    let restored_patch: UpdateProfilePatch =
        serde_json::from_str(&patch_json).expect("deserialize UpdateProfilePatch");
    assert_eq!(restored_patch, patch);
}

// ---------------------------------------------------------------------------
// 2. build_user_profile_route: построение пути и валидация Snowflake
// ---------------------------------------------------------------------------

#[test]
fn build_user_profile_route_formats_query_with_and_without_guild_id() {
    let without_guild = UserProfileQuery {
        user_id: VALID_USER_ID.to_string(),
        guild_id: None,
        with_mutual_guilds: true,
        with_mutual_friends: false,
    };
    assert_eq!(
        build_user_profile_route(&without_guild).expect("valid route without guild_id"),
        format!("/users/{VALID_USER_ID}/profile?with_mutual_guilds=true&with_mutual_friends=false")
    );

    let with_guild = UserProfileQuery {
        user_id: VALID_USER_ID.to_string(),
        guild_id: Some(VALID_GUILD_ID.to_string()),
        with_mutual_guilds: false,
        with_mutual_friends: true,
    };
    assert_eq!(
        build_user_profile_route(&with_guild).expect("valid route with guild_id"),
        format!(
            "/users/{VALID_USER_ID}/profile?with_mutual_guilds=false&with_mutual_friends=true&guild_id={VALID_GUILD_ID}"
        )
    );
}

#[test]
fn build_user_profile_route_validates_canonical_snowflake_bounds() {
    // Нижняя ("1") и верхняя (u64::MAX) границы
    let min_max = UserProfileQuery {
        user_id: "1".to_string(),
        guild_id: Some(u64::MAX.to_string()),
        with_mutual_guilds: true,
        with_mutual_friends: true,
    };
    let route = build_user_profile_route(&min_max).expect("1 and u64::MAX are valid snowflakes");
    assert_eq!(
        route,
        format!(
            "/users/1/profile?with_mutual_guilds=true&with_mutual_friends=true&guild_id={}",
            u64::MAX
        )
    );

    let invalid_snowflakes = [
        "",
        "0",
        "00",
        "01",
        "0123456789",
        "-1",
        "+1",
        "abc",
        "123a",
        " 123",
        "123 ",
        "18446744073709551616", // u64::MAX + 1
    ];

    for bad in invalid_snowflakes {
        let bad_user = UserProfileQuery {
            user_id: bad.to_string(),
            guild_id: None,
            with_mutual_guilds: true,
            with_mutual_friends: true,
        };
        assert!(
            build_user_profile_route(&bad_user).is_err(),
            "user_id={bad:?} must be rejected"
        );

        let bad_guild = UserProfileQuery {
            user_id: VALID_USER_ID.to_string(),
            guild_id: Some(bad.to_string()),
            with_mutual_guilds: true,
            with_mutual_friends: true,
        };
        assert!(
            build_user_profile_route(&bad_guild).is_err(),
            "guild_id={bad:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. validate_profile_patch: bio (<= 190), pronouns (<= 40), accent_color (<= 0xFF_FFFF)
// ---------------------------------------------------------------------------

#[test]
fn validate_profile_patch_enforces_bio_char_limit() {
    let empty_patch = UpdateProfilePatch {
        bio: None,
        pronouns: None,
        accent_color: None,
    };
    assert!(validate_profile_patch(&empty_patch).is_ok());

    let empty_bio = UpdateProfilePatch {
        bio: Some(String::new()),
        pronouns: None,
        accent_color: None,
    };
    assert!(validate_profile_patch(&empty_bio).is_ok());

    // Ровно 190 символов кириллицы (380 байт UTF-8, 190 chars) -> Ok
    let max_bio = UpdateProfilePatch {
        bio: Some("Я".repeat(MAX_BIO_CHARS)),
        pronouns: None,
        accent_color: None,
    };
    assert!(validate_profile_patch(&max_bio).is_ok());

    // 191 символ -> Err
    let too_long_bio = UpdateProfilePatch {
        bio: Some("Я".repeat(MAX_BIO_CHARS + 1)),
        pronouns: None,
        accent_color: None,
    };
    assert!(validate_profile_patch(&too_long_bio).is_err());
}

#[test]
fn validate_profile_patch_enforces_pronouns_char_limit() {
    let empty_pronouns = UpdateProfilePatch {
        bio: None,
        pronouns: Some(String::new()),
        accent_color: None,
    };
    assert!(validate_profile_patch(&empty_pronouns).is_ok());

    // Ровно 40 символов кириллицы (80 байт UTF-8, 40 chars) -> Ok
    let max_pronouns = UpdateProfilePatch {
        bio: None,
        pronouns: Some("Ы".repeat(MAX_PRONOUNS_CHARS)),
        accent_color: None,
    };
    assert!(validate_profile_patch(&max_pronouns).is_ok());

    // 41 символ -> Err
    let too_long_pronouns = UpdateProfilePatch {
        bio: None,
        pronouns: Some("Ы".repeat(MAX_PRONOUNS_CHARS + 1)),
        accent_color: None,
    };
    assert!(validate_profile_patch(&too_long_pronouns).is_err());
}

#[test]
fn validate_profile_patch_enforces_24bit_accent_color_bounds() {
    for valid_color in [0, 1, 0x58_65F2, 0xFF_FFFF] {
        let patch = UpdateProfilePatch {
            bio: Some("ok".to_string()),
            pronouns: Some("he/him".to_string()),
            accent_color: Some(valid_color),
        };
        assert!(
            validate_profile_patch(&patch).is_ok(),
            "accent_color=0x{valid_color:06X} must be accepted"
        );
    }

    for invalid_color in [0x0100_0000, 0x01FF_FFFF, u32::MAX] {
        let patch = UpdateProfilePatch {
            bio: None,
            pronouns: None,
            accent_color: Some(invalid_color),
        };
        assert!(
            validate_profile_patch(&patch).is_err(),
            "accent_color=0x{invalid_color:08X} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. validate_user_note: target_user_id Snowflake и note <= 256 chars
// ---------------------------------------------------------------------------

#[test]
fn validate_user_note_builds_route_and_enforces_snowflake_and_char_limits() {
    // Пустая заметка и валидный Snowflake -> Ok
    let route = validate_user_note(VALID_USER_ID, "").expect("empty note is valid");
    assert_eq!(route, format!("/users/@me/notes/{VALID_USER_ID}"));

    // Граничные Snowflake ("1" и u64::MAX) + ровно 256 символов кириллицы -> Ok
    let max_note = "Ж".repeat(MAX_NOTE_CHARS);
    assert_eq!(
        validate_user_note("1", &max_note).expect("min snowflake + 256 chars"),
        "/users/@me/notes/1"
    );
    let max_id = u64::MAX.to_string();
    assert_eq!(
        validate_user_note(&max_id, &max_note).expect("max snowflake + 256 chars"),
        format!("/users/@me/notes/{max_id}")
    );

    // 257 символов -> Err
    let too_long_note = "Ж".repeat(MAX_NOTE_CHARS + 1);
    assert!(validate_user_note(VALID_USER_ID, &too_long_note).is_err());

    // Некорректные Snowflake ID -> Err
    for bad_id in [
        "",
        "0",
        "00",
        "01",
        "0123456789",
        "-1",
        "abc",
        "18446744073709551616",
    ] {
        assert!(
            validate_user_note(bad_id, "заметка").is_err(),
            "target_user_id={bad_id:?} must be rejected"
        );
    }
}
