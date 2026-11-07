//! RW-40 / task-209: контрактные тесты модуля `forum` (`forum_contract`).
//!
//! Проверяет:
//! 1. Константы `MAX_FORUM_APPLIED_TAGS = 5`, `MAX_FORUM_TITLE_CHARS = 100`,
//!    `ALLOWED_AUTO_ARCHIVE_MINUTES = [60, 1440, 4320, 10080]`.
//! 2. `ForumSortOrder` (`LatestActivity = 0`, `CreationDate = 1`) — дискриминанты и serde
//!    serialize/deserialize.
//! 3. `CreateForumPostRequest` и `validate_forum_post_request(req)`:
//!    - `channel_id`: непустой числовой Snowflake > 0 (`1..=u64::MAX`);
//!    - `name.trim()`: `1..=100` символов (Unicode scalar values `.chars().count()`);
//!    - `auto_archive_duration`: входит в `ALLOWED_AUTO_ARCHIVE_MINUTES` (`60`, `1440`, `4320`, `10080`);
//!    - `content.trim()`: непустой и `<= 2000` символов;
//!    - `applied_tags`: `len() <= 5`, каждый тег — непустой числовой Snowflake > 0.
//! 4. `build_forum_post_route_and_body(req)` -> `Ok(("/channels/{channel_id}/threads", json!({ ... })))`.

use discord_gui_spike::forum::{
    build_forum_post_route_and_body, validate_forum_post_request, CreateForumPostRequest,
    ForumSortOrder, ALLOWED_AUTO_ARCHIVE_MINUTES, MAX_FORUM_APPLIED_TAGS, MAX_FORUM_TITLE_CHARS,
};
use serde_json::json;

const VALID_CHANNEL_ID: &str = "1014537851418980375";
const VALID_TAG_1: &str = "1100000000000000001";
const VALID_TAG_2: &str = "1100000000000000002";

fn sample_valid_request() -> CreateForumPostRequest {
    CreateForumPostRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        name: "Обсуждение релиза RW-40".to_string(),
        auto_archive_duration: 1440,
        content: "Проверяем контракт создания поста в форум-канале.".to_string(),
        applied_tags: vec![VALID_TAG_1.to_string(), VALID_TAG_2.to_string()],
        rate_limit_per_user: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и ForumSortOrder (дискриминанты + serde)
// ---------------------------------------------------------------------------

#[test]
fn forum_constants_match_discord_contract() {
    assert_eq!(MAX_FORUM_APPLIED_TAGS, 5);
    assert_eq!(MAX_FORUM_TITLE_CHARS, 100);
    assert_eq!(ALLOWED_AUTO_ARCHIVE_MINUTES, [60, 1440, 4320, 10080]);
}

#[test]
fn forum_sort_order_discriminants_and_serde_roundtrip() {
    assert_eq!(ForumSortOrder::LatestActivity as u8, 0);
    assert_eq!(ForumSortOrder::CreationDate as u8, 1);

    for variant in [ForumSortOrder::LatestActivity, ForumSortOrder::CreationDate] {
        let serialized = serde_json::to_string(&variant).expect("serialize ForumSortOrder");
        let restored: ForumSortOrder =
            serde_json::from_str(&serialized).expect("deserialize ForumSortOrder");
        assert_eq!(restored, variant);
    }

    // Проверка десериализации из строковых и числовых представлений
    let from_zero_or_str: ForumSortOrder = serde_json::from_str("\"LatestActivity\"")
        .or_else(|_| serde_json::from_str("0"))
        .expect("deserialize LatestActivity");
    assert_eq!(from_zero_or_str, ForumSortOrder::LatestActivity);
}

// ---------------------------------------------------------------------------
// 2. Валидация channel_id (Snowflake)
// ---------------------------------------------------------------------------

#[test]
fn validate_forum_post_request_checks_channel_id_snowflake_bounds() {
    let mut min_req = sample_valid_request();
    min_req.channel_id = "1".to_string();
    assert!(validate_forum_post_request(&min_req).is_ok());

    let mut max_req = sample_valid_request();
    max_req.channel_id = u64::MAX.to_string();
    assert!(validate_forum_post_request(&max_req).is_ok());

    for invalid_id in [
        "",
        "0",
        "0000",
        "abc",
        "123a",
        "-1",
        " 123",
        "123 ",
        "18446744073709551616",
    ] {
        let mut bad = sample_valid_request();
        bad.channel_id = invalid_id.to_string();
        assert!(
            validate_forum_post_request(&bad).is_err(),
            "channel_id={invalid_id:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Валидация name.trim() (1..=100 символов)
// ---------------------------------------------------------------------------

#[test]
fn validate_forum_post_request_enforces_trimmed_title_1_to_100_chars() {
    // 1 символ после trim -> Ok
    let mut one_char = sample_valid_request();
    one_char.name = "  Я  ".to_string();
    assert!(validate_forum_post_request(&one_char).is_ok());

    // 100 символов кириллицы (200 байт UTF-8, но ровно 100 chars) -> Ok
    let mut hundred_chars = sample_valid_request();
    hundred_chars.name = "Ы".repeat(MAX_FORUM_TITLE_CHARS);
    assert!(validate_forum_post_request(&hundred_chars).is_ok());

    // 100 символов + пробелы по краям -> trim() даёт 100 символов -> Ok
    let mut hundred_trimmed = sample_valid_request();
    hundred_trimmed.name = format!("  {}  ", "A".repeat(MAX_FORUM_TITLE_CHARS));
    assert!(validate_forum_post_request(&hundred_trimmed).is_ok());

    // Пустая или пробельная строка -> Err
    for empty_name in ["", " ", "   \t\n  "] {
        let mut bad = sample_valid_request();
        bad.name = empty_name.to_string();
        assert!(
            validate_forum_post_request(&bad).is_err(),
            "empty/whitespace name {empty_name:?} must be rejected"
        );
    }

    // 101 символ -> Err
    let mut too_long = sample_valid_request();
    too_long.name = "Б".repeat(MAX_FORUM_TITLE_CHARS + 1);
    assert!(validate_forum_post_request(&too_long).is_err());
}

// ---------------------------------------------------------------------------
// 4. Валидация auto_archive_duration ([60, 1440, 4320, 10080])
// ---------------------------------------------------------------------------

#[test]
fn validate_forum_post_request_enforces_allowed_auto_archive_minutes() {
    for allowed in ALLOWED_AUTO_ARCHIVE_MINUTES {
        let mut req = sample_valid_request();
        req.auto_archive_duration = allowed;
        assert!(
            validate_forum_post_request(&req).is_ok(),
            "auto_archive_duration={allowed} must be accepted"
        );
    }

    for disallowed in [
        0,
        1,
        30,
        59,
        61,
        120,
        720,
        1439,
        1441,
        2880,
        10081,
        u32::MAX,
    ] {
        let mut bad = sample_valid_request();
        bad.auto_archive_duration = disallowed;
        assert!(
            validate_forum_post_request(&bad).is_err(),
            "auto_archive_duration={disallowed} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Валидация content.trim() (непустой и <= 2000 символов)
// ---------------------------------------------------------------------------

#[test]
fn validate_forum_post_request_enforces_trimmed_content_1_to_2000_chars() {
    // 1 символ -> Ok
    let mut one_char = sample_valid_request();
    one_char.content = "  !  ".to_string();
    assert!(validate_forum_post_request(&one_char).is_ok());

    // 2000 символов кириллицы -> Ok
    let mut max_content = sample_valid_request();
    max_content.content = "Ж".repeat(2000);
    assert!(validate_forum_post_request(&max_content).is_ok());

    // Пустой / только пробелы -> Err
    for empty_content in ["", " ", "\n\t  "] {
        let mut bad = sample_valid_request();
        bad.content = empty_content.to_string();
        assert!(
            validate_forum_post_request(&bad).is_err(),
            "empty/whitespace content {empty_content:?} must be rejected"
        );
    }

    // 2001 символ -> Err
    let mut too_long = sample_valid_request();
    too_long.content = "Z".repeat(2001);
    assert!(validate_forum_post_request(&too_long).is_err());
}

// ---------------------------------------------------------------------------
// 6. Валидация applied_tags (len() <= 5 и каждый тег — валидный Snowflake > 0)
// ---------------------------------------------------------------------------

#[test]
fn validate_forum_post_request_enforces_applied_tags_max_5_and_snowflakes() {
    // 0 тегов -> Ok
    let mut zero_tags = sample_valid_request();
    zero_tags.applied_tags = Vec::new();
    assert!(validate_forum_post_request(&zero_tags).is_ok());

    // 5 тегов (включая "1" и u64::MAX) -> Ok
    let mut five_tags = sample_valid_request();
    five_tags.applied_tags = vec![
        "1".to_string(),
        "2".to_string(),
        "3".to_string(),
        "4".to_string(),
        u64::MAX.to_string(),
    ];
    assert!(validate_forum_post_request(&five_tags).is_ok());

    // 6 тегов (> MAX_FORUM_APPLIED_TAGS) -> Err
    let mut six_tags = five_tags.clone();
    six_tags.applied_tags.push("6".to_string());
    assert!(validate_forum_post_request(&six_tags).is_err());

    // Некорректный Snowflake в applied_tags -> Err
    for bad_tag in ["", "0", "00", "tag_1", "-5", "18446744073709551616"] {
        let mut bad = sample_valid_request();
        bad.applied_tags = vec![VALID_TAG_1.to_string(), bad_tag.to_string()];
        assert!(
            validate_forum_post_request(&bad).is_err(),
            "applied_tag={bad_tag:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. build_forum_post_route_and_body: маршрут и JSON-тело
// ---------------------------------------------------------------------------

#[test]
fn build_forum_post_route_and_body_returns_threads_route_and_json_payload() {
    let req = sample_valid_request();
    let (route, body) =
        build_forum_post_route_and_body(&req).expect("valid request builds route and body");

    assert_eq!(route, format!("/channels/{VALID_CHANNEL_ID}/threads"));
    assert_eq!(
        body,
        json!({
            "name": "Обсуждение релиза RW-40",
            "auto_archive_duration": 1440,
            "applied_tags": [VALID_TAG_1, VALID_TAG_2],
            "message": {
                "content": "Проверяем контракт создания поста в форум-канале."
            }
        })
    );

    // С установленным rate_limit_per_user
    let mut req_with_rate_limit = sample_valid_request();
    req_with_rate_limit.rate_limit_per_user = Some(15);
    let (route_rl, body_rl) = build_forum_post_route_and_body(&req_with_rate_limit)
        .expect("valid request with rate_limit_per_user");
    assert_eq!(route_rl, format!("/channels/{VALID_CHANNEL_ID}/threads"));
    assert_eq!(body_rl["name"], json!("Обсуждение релиза RW-40"));
    assert_eq!(body_rl["auto_archive_duration"], json!(1440));
    assert_eq!(body_rl["applied_tags"], json!([VALID_TAG_1, VALID_TAG_2]));
    assert_eq!(
        body_rl["message"],
        json!({ "content": "Проверяем контракт создания поста в форум-канале." })
    );
    assert_eq!(body_rl["rate_limit_per_user"], json!(15));
}

#[test]
fn create_forum_post_request_serde_roundtrip() {
    let req = CreateForumPostRequest {
        channel_id: VALID_CHANNEL_ID.to_string(),
        name: "Тест сериализации".to_string(),
        auto_archive_duration: 4320,
        content: "Контент поста".to_string(),
        applied_tags: vec![VALID_TAG_1.to_string()],
        rate_limit_per_user: Some(30),
    };
    let val = serde_json::to_value(&req).expect("serialize CreateForumPostRequest");
    let decoded: CreateForumPostRequest =
        serde_json::from_value(val).expect("deserialize CreateForumPostRequest");
    assert_eq!(decoded, req);
}

#[test]
fn build_forum_post_route_and_body_propagates_validation_errors() {
    let mut bad = sample_valid_request();
    bad.auto_archive_duration = 999;
    assert!(build_forum_post_route_and_body(&bad).is_err());

    let mut bad_channel = sample_valid_request();
    bad_channel.channel_id = "0".to_string();
    assert!(build_forum_post_route_and_body(&bad_channel).is_err());
}
