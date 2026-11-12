//! RW-40 / task-233 (`BE-P1-23`): контрактные тесты BFF-модуля связей и заявок в друзья
//! (`relationships`).
//!
//! Проверяет:
//! 1. Константы:
//!    - `RELATIONSHIP_FRIEND: u8 = 1`
//!    - `RELATIONSHIP_BLOCKED: u8 = 2`
//!    - `RELATIONSHIP_PENDING_INCOMING: u8 = 3`
//!    - `RELATIONSHIP_PENDING_OUTGOING: u8 = 4`
//!    - `MAX_USERNAME_CHARS: usize = 32`
//! 2. Структуру `SendFriendRequest { pub username: String }` и её serde round-trip.
//! 3. `validate_friend_request_username(username: &str) -> Result<&str, String>`:
//!    - обрезка внешних пробелов (`trim()`);
//!    - длина `2..=MAX_USERNAME_CHARS` (`2..=32`) символов Unicode (`chars().count()`);
//!    - запрет подстрок `"@"`, `":"`, `"#"`;
//!    - возврат `Ok(trimmed)` при успехе или `Err` при нарушении.
//! 4. `build_send_friend_request_body(req: &SendFriendRequest) -> Result<serde_json::Value, String>`:
//!    - вызывает `validate_friend_request_username(&req.username)?`;
//!    - возвращает `Ok(json!({"username": trimmed, "discriminator": null}))`.
//! 5. `build_relationship_user_route(target_user_id: &str) -> Result<String, String>`:
//!    - валидирует `target_user_id` как канонический положительный `u64` Snowflake
//!      (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей);
//!    - возвращает `Ok(format!("/users/@me/relationships/{target_user_id}"))`
//!      (сверка с RE-оракулом `rest-static.json:34` `USER_RELATIONSHIP`).
//! 6. `build_block_user_body() -> serde_json::Value`:
//!    - возвращает `json!({"type": RELATIONSHIP_BLOCKED})` (`{"type": 2}`).

use chord_client::relationships::{
    build_block_user_body, build_relationship_user_route, build_send_friend_request_body,
    validate_friend_request_username, SendFriendRequest, MAX_USERNAME_CHARS, RELATIONSHIP_BLOCKED,
    RELATIONSHIP_FRIEND, RELATIONSHIP_PENDING_INCOMING, RELATIONSHIP_PENDING_OUTGOING,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры SendFriendRequest
// ---------------------------------------------------------------------------

#[test]
fn relationships_constants_match_discord_contract() {
    assert_eq!(RELATIONSHIP_FRIEND, 1_u8);
    assert_eq!(RELATIONSHIP_BLOCKED, 2_u8);
    assert_eq!(RELATIONSHIP_PENDING_INCOMING, 3_u8);
    assert_eq!(RELATIONSHIP_PENDING_OUTGOING, 4_u8);
    assert_eq!(MAX_USERNAME_CHARS, 32_usize);
}

#[test]
fn send_friend_request_serde_roundtrip() {
    let req = SendFriendRequest {
        username: "wumpus_friend".to_string(),
    };
    let serialized = serde_json::to_value(&req).expect("serialize SendFriendRequest");
    assert_eq!(
        serialized,
        json!({
            "username": "wumpus_friend"
        })
    );
    let restored: SendFriendRequest =
        serde_json::from_value(serialized).expect("deserialize SendFriendRequest");
    assert_eq!(restored, req);
}

// ---------------------------------------------------------------------------
// 2. validate_friend_request_username: trim, 2..=32 Unicode chars, запрет @ : #
// ---------------------------------------------------------------------------

#[test]
fn validate_friend_request_username_trims_and_enforces_2_to_32_unicode_char_bounds() {
    // Пустая, пробельная и 1-символьная строки отклоняются
    for too_short in ["", "   ", "\t\n", "a", " a ", "я", " я "] {
        assert!(
            validate_friend_request_username(too_short).is_err(),
            "username={too_short:?} must be rejected (< 2 chars after trim)"
        );
    }

    // Нижняя граница: ровно 2 символа ASCII и 2 символа Unicode (кириллица) + внешние пробелы
    assert_eq!(
        validate_friend_request_username("ab").expect("2 ascii chars"),
        "ab"
    );
    assert_eq!(
        validate_friend_request_username("  ab  ").expect("trimmed 2 ascii chars"),
        "ab"
    );
    assert_eq!(
        validate_friend_request_username(" \tДруг\n ").expect("trimmed unicode username"),
        "Друг"
    );
    assert_eq!(
        validate_friend_request_username("яя").expect("2 cyrillic chars"),
        "яя"
    );

    // Верхняя граница: ровно 32 символа ASCII и ровно 32 символа Unicode (64 байта UTF-8)
    let ascii_32 = "a".repeat(MAX_USERNAME_CHARS);
    assert_eq!(
        validate_friend_request_username(&ascii_32).expect("32 ascii chars"),
        ascii_32.as_str()
    );
    let padded_ascii_32 = format!("  {ascii_32}  ");
    assert_eq!(
        validate_friend_request_username(&padded_ascii_32).expect("padded 32 ascii chars"),
        ascii_32.as_str()
    );

    let cyrillic_32 = "ж".repeat(MAX_USERNAME_CHARS);
    assert_eq!(cyrillic_32.chars().count(), 32);
    assert_eq!(cyrillic_32.len(), 64);
    assert_eq!(
        validate_friend_request_username(&cyrillic_32).expect("32 unicode chars"),
        cyrillic_32.as_str()
    );

    // За верхней границей: 33 символа ASCII и 33 символа кириллицы отклоняются
    let ascii_33 = "a".repeat(MAX_USERNAME_CHARS + 1);
    assert!(validate_friend_request_username(&ascii_33).is_err());

    let cyrillic_33 = "ж".repeat(MAX_USERNAME_CHARS + 1);
    assert!(validate_friend_request_username(&cyrillic_33).is_err());
}

#[test]
fn validate_friend_request_username_rejects_forbidden_characters() {
    for bad in [
        "@wumpus",
        "wumpus@",
        "wum@pus",
        ":wumpus",
        "wumpus:",
        "wum:pus",
        "#wumpus",
        "wumpus#0",
        "wumpus#1234",
        "  wumpus#1234  ",
        "@@",
        "::",
        "##",
    ] {
        assert!(
            validate_friend_request_username(bad).is_err(),
            "username={bad:?} containing forbidden char (@, :, #) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. build_send_friend_request_body: валидация + {"username": trimmed, "discriminator": null}
// ---------------------------------------------------------------------------

#[test]
fn build_send_friend_request_body_returns_trimmed_username_and_null_discriminator() {
    let req = SendFriendRequest {
        username: "  pomelo_user  ".to_string(),
    };
    let body = build_send_friend_request_body(&req).expect("valid send friend request body");
    assert_eq!(
        body,
        json!({
            "username": "pomelo_user",
            "discriminator": null
        })
    );
    assert!(body.get("discriminator").is_some_and(|v| v.is_null()));

    // Ошибки валидации пробрасываются
    for bad_username in [
        "",
        "x",
        "user#1234",
        "user@domain",
        "user:tag",
        &"a".repeat(33),
    ] {
        let bad_req = SendFriendRequest {
            username: bad_username.to_string(),
        };
        assert!(
            build_send_friend_request_body(&bad_req).is_err(),
            "bad username={bad_username:?} must fail in build_send_friend_request_body"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. build_relationship_user_route: валидация Snowflake 1..=u64::MAX без ведущих нулей
// ---------------------------------------------------------------------------

#[test]
fn build_relationship_user_route_validates_canonical_snowflake_and_formats_path() {
    let route =
        build_relationship_user_route("194374444821643266").expect("valid relationship user route");
    assert_eq!(route, "/users/@me/relationships/194374444821643266");

    // Границы: 1 и u64::MAX
    assert_eq!(
        build_relationship_user_route("1").expect("min snowflake 1"),
        "/users/@me/relationships/1"
    );
    let max_u64 = u64::MAX.to_string();
    assert_eq!(
        build_relationship_user_route(&max_u64).expect("max u64 snowflake"),
        format!("/users/@me/relationships/{max_u64}")
    );

    // Негативные и граничные случаи Snowflake
    for bad_id in [
        "",
        "0",
        "00",
        "01",
        "0194374444821643266",
        "-1",
        "+1",
        "abc",
        "123a456",
        " 194374444821643266",
        "194374444821643266 ",
        "18446744073709551616", // u64::MAX + 1
    ] {
        assert!(
            build_relationship_user_route(bad_id).is_err(),
            "target_user_id={bad_id:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. build_block_user_body: {"type": 2}
// ---------------------------------------------------------------------------

#[test]
fn build_block_user_body_returns_blocked_relationship_type() {
    let body = build_block_user_body();
    assert_eq!(body, json!({ "type": RELATIONSHIP_BLOCKED }));
    assert_eq!(body, json!({ "type": 2 }));
}
