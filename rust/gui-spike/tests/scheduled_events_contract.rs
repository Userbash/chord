//! RW-40 / task-239 (`BE-P1-25`): контрактные тесты BFF-модуля запланированных событий гильдии (`scheduled_events`).
//!
//! Проверяет:
//! 1. Константы `EVENT_ENTITY_STAGE = 1`, `EVENT_ENTITY_VOICE = 2`, `EVENT_ENTITY_EXTERNAL = 3`,
//!    `EVENT_STATUS_SCHEDULED = 1`, `EVENT_STATUS_ACTIVE = 2`, `EVENT_STATUS_COMPLETED = 3`,
//!    `EVENT_STATUS_CANCELED = 4`, `MAX_EVENT_NAME_CHARS = 100`, `MAX_EVENT_DESC_CHARS = 1000`,
//!    `MAX_EVENT_LOCATION_CHARS = 100`.
//! 2. Структуру `CreateScheduledEventRequest` и её serde round-trip.
//! 3. `validate_scheduled_event_request(req: &CreateScheduledEventRequest) -> Result<(), String>`:
//!    - `name.trim()` от `1` до `100` символов Unicode (`MAX_EVENT_NAME_CHARS`);
//!    - `description`: если `Some`, не более `1000` символов Unicode (`MAX_EVENT_DESC_CHARS`);
//!    - `privacy_level` строго равен `2` (`GUILD_ONLY`);
//!    - `scheduled_start_time.trim()` непустой;
//!    - при `entity_type == 1` (`STAGE`) или `2` (`VOICE`): `channel_id` должен быть `Some` с валидной положительной числовой снежинкой (`1..=20` цифр, без ведущего `0`, `1..=u64::MAX`);
//!    - при `entity_type == 3` (`EXTERNAL`): `location` должен быть `Some` с непустой `trim()`-строкой `<= 100` символов Unicode И `scheduled_end_time` должен быть `Some` с непустой `trim()`-строкой;
//!    - любые другие значения `entity_type` (`0`, `4`, `255`) отклоняются (`Err`).
//! 4. `build_guild_events_route(guild_id: &str, with_user_count: bool) -> Result<String, String>`:
//!    - валидирует `guild_id` как канонический положительный Snowflake;
//!    - возвращает `"/api/v9/guilds/{guild_id}/scheduled-events?with_user_count=true"` (или `false`).
//! 5. `build_event_rsvp_route(guild_id: &str, event_id: &str) -> Result<String, String>`:
//!    - валидирует оба Snowflake (`guild_id`, `event_id`);
//!    - возвращает `"/api/v9/guilds/{guild_id}/scheduled-events/{event_id}/users/@me"`.

use chord_client::scheduled_events::{
    build_event_rsvp_route, build_guild_events_route, validate_scheduled_event_request,
    CreateScheduledEventRequest, EVENT_ENTITY_EXTERNAL, EVENT_ENTITY_STAGE, EVENT_ENTITY_VOICE,
    EVENT_STATUS_ACTIVE, EVENT_STATUS_CANCELED, EVENT_STATUS_COMPLETED, EVENT_STATUS_SCHEDULED,
    MAX_EVENT_DESC_CHARS, MAX_EVENT_LOCATION_CHARS, MAX_EVENT_NAME_CHARS,
};

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_CHANNEL_ID: &str = "100000000000000002";
const VALID_EVENT_ID: &str = "100000000000000003";

fn sample_voice_event() -> CreateScheduledEventRequest {
    CreateScheduledEventRequest {
        name: "Пятничный созвон".to_string(),
        description: Some("Обсуждаем релиз RW-40".to_string()),
        entity_type: EVENT_ENTITY_VOICE,
        channel_id: Some(VALID_CHANNEL_ID.to_string()),
        location: None,
        scheduled_start_time: "2026-11-01T18:00:00Z".to_string(),
        scheduled_end_time: None,
        privacy_level: 2,
    }
}

fn sample_external_event() -> CreateScheduledEventRequest {
    CreateScheduledEventRequest {
        name: "Конференция Rust".to_string(),
        description: None,
        entity_type: EVENT_ENTITY_EXTERNAL,
        channel_id: None,
        location: Some("Зал №1 / Онлайн-стрим".to_string()),
        scheduled_start_time: "2026-11-01T18:00:00Z".to_string(),
        scheduled_end_time: Some("2026-11-01T20:00:00Z".to_string()),
        privacy_level: 2,
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn scheduled_events_constants_match_discord_contract() {
    assert_eq!(EVENT_ENTITY_STAGE, 1);
    assert_eq!(EVENT_ENTITY_VOICE, 2);
    assert_eq!(EVENT_ENTITY_EXTERNAL, 3);

    assert_eq!(EVENT_STATUS_SCHEDULED, 1);
    assert_eq!(EVENT_STATUS_ACTIVE, 2);
    assert_eq!(EVENT_STATUS_COMPLETED, 3);
    assert_eq!(EVENT_STATUS_CANCELED, 4);

    assert_eq!(MAX_EVENT_NAME_CHARS, 100);
    assert_eq!(MAX_EVENT_DESC_CHARS, 1000);
    assert_eq!(MAX_EVENT_LOCATION_CHARS, 100);
}

#[test]
fn create_scheduled_event_request_serde_roundtrip() {
    for req in [sample_voice_event(), sample_external_event()] {
        let json = serde_json::to_value(&req).expect("serialize CreateScheduledEventRequest");
        let back: CreateScheduledEventRequest =
            serde_json::from_value(json).expect("deserialize CreateScheduledEventRequest");
        assert_eq!(back, req);
    }
}

// ---------------------------------------------------------------------------
// 2. Валидация CreateScheduledEventRequest: name, description, privacy_level, start_time
// ---------------------------------------------------------------------------

#[test]
fn validate_scheduled_event_request_enforces_name_and_description_bounds() {
    let mut req = sample_voice_event();
    assert!(validate_scheduled_event_request(&req).is_ok());

    // Границы имени (1..=100 символов Unicode после trim)
    req.name = "Я".to_string();
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.name = format!("  {}  ", "Я".repeat(MAX_EVENT_NAME_CHARS));
    assert!(validate_scheduled_event_request(&req).is_ok());

    for bad_name in [
        "".to_string(),
        "   \t\n ".to_string(),
        "Я".repeat(MAX_EVENT_NAME_CHARS + 1),
    ] {
        req.name = bad_name;
        assert!(
            validate_scheduled_event_request(&req).is_err(),
            "invalid name should be rejected"
        );
    }

    // Границы описания (None, Some(""), Some(1000 символов) -> Ok; Some(1001 символ) -> Err)
    req.name = "Валидное событие".to_string();
    req.description = None;
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.description = Some(String::new());
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.description = Some("Д".repeat(MAX_EVENT_DESC_CHARS));
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.description = Some("Д".repeat(MAX_EVENT_DESC_CHARS + 1));
    assert!(validate_scheduled_event_request(&req).is_err());
}

#[test]
fn validate_scheduled_event_request_enforces_privacy_level_and_start_time() {
    let mut req = sample_voice_event();

    // Только privacy_level == 2 (GUILD_ONLY) допустим
    for bad_privacy in [0u8, 1, 3, 255] {
        req.privacy_level = bad_privacy;
        assert!(
            validate_scheduled_event_request(&req).is_err(),
            "privacy_level={bad_privacy} must be rejected"
        );
    }
    req.privacy_level = 2;
    assert!(validate_scheduled_event_request(&req).is_ok());

    // scheduled_start_time после trim не может быть пустым
    for bad_start in ["", "   ", "\t\n"] {
        req.scheduled_start_time = bad_start.to_string();
        assert!(
            validate_scheduled_event_request(&req).is_err(),
            "scheduled_start_time={bad_start:?} must be rejected"
        );
    }
    req.scheduled_start_time = "  2026-11-01T18:00:00Z  ".to_string();
    assert!(validate_scheduled_event_request(&req).is_ok());
}

// ---------------------------------------------------------------------------
// 3. Валидация CreateScheduledEventRequest: entity_type (STAGE/VOICE vs EXTERNAL vs invalid)
// ---------------------------------------------------------------------------

#[test]
fn validate_scheduled_event_request_validates_stage_and_voice_channel_snowflake() {
    let max_20_digits = "9".repeat(20);
    for entity_type in [EVENT_ENTITY_STAGE, EVENT_ENTITY_VOICE] {
        let mut req = sample_voice_event();
        req.entity_type = entity_type;

        for valid_cid in ["1", VALID_CHANNEL_ID, &u64::MAX.to_string(), &max_20_digits] {
            req.channel_id = Some(valid_cid.to_string());
            assert!(
                validate_scheduled_event_request(&req).is_ok(),
                "entity_type={entity_type} channel_id={valid_cid} must be valid"
            );
        }

        // Отсутствующий channel_id отклоняется для STAGE и VOICE
        req.channel_id = None;
        assert!(validate_scheduled_event_request(&req).is_err());

        // Невалидные снежинки (пустая, ведущий 0, не-цифры, >20 цифр)
        let twenty_one_digits = "1".repeat(21);
        for bad_cid in [
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
            req.channel_id = Some(bad_cid.to_string());
            assert!(
                validate_scheduled_event_request(&req).is_err(),
                "entity_type={entity_type} channel_id={bad_cid:?} must be rejected"
            );
        }
    }
}

#[test]
fn validate_scheduled_event_request_validates_external_location_and_end_time_and_rejects_unknown_entity(
) {
    let mut req = sample_external_event();
    assert!(validate_scheduled_event_request(&req).is_ok());

    // Границы location для EXTERNAL: 1..=100 символов Unicode после trim
    req.location = Some("А".to_string());
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.location = Some(format!("  {}  ", "А".repeat(MAX_EVENT_LOCATION_CHARS)));
    assert!(validate_scheduled_event_request(&req).is_ok());

    req.location = None;
    assert!(validate_scheduled_event_request(&req).is_err());

    for bad_loc in [
        "".to_string(),
        "   ".to_string(),
        "А".repeat(MAX_EVENT_LOCATION_CHARS + 1),
    ] {
        req.location = Some(bad_loc);
        assert!(validate_scheduled_event_request(&req).is_err());
    }
    req.location = Some("Главная сцена".to_string());

    // scheduled_end_time обязателен и непустой после trim для EXTERNAL
    req.scheduled_end_time = None;
    assert!(validate_scheduled_event_request(&req).is_err());

    for bad_end in ["", "   ", "\t\n"] {
        req.scheduled_end_time = Some(bad_end.to_string());
        assert!(validate_scheduled_event_request(&req).is_err());
    }
    req.scheduled_end_time = Some("2026-11-01T21:00:00Z".to_string());
    assert!(validate_scheduled_event_request(&req).is_ok());

    // Неизвестные entity_type отклоняются
    for bad_type in [0u8, 4, 5, 255] {
        let mut bad_req = sample_voice_event();
        bad_req.entity_type = bad_type;
        assert!(
            validate_scheduled_event_request(&bad_req).is_err(),
            "entity_type={bad_type} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Маршруты build_guild_events_route и build_event_rsvp_route
// ---------------------------------------------------------------------------

#[test]
fn build_guild_events_route_validates_guild_id_and_formats_query() {
    let route_true =
        build_guild_events_route(VALID_GUILD_ID, true).expect("valid route with_user_count=true");
    assert_eq!(
        route_true,
        "/api/v9/guilds/100000000000000001/scheduled-events?with_user_count=true"
    );

    let route_false =
        build_guild_events_route("1", false).expect("valid route with_user_count=false");
    assert_eq!(
        route_false,
        "/api/v9/guilds/1/scheduled-events?with_user_count=false"
    );

    let twenty_one_digits = "1".repeat(21);
    for bad_gid in ["", "0", "0123", "abc", "-1", " 123", &twenty_one_digits] {
        assert!(
            build_guild_events_route(bad_gid, true).is_err(),
            "guild_id={bad_gid:?} must fail"
        );
    }
}

#[test]
fn build_event_rsvp_route_validates_both_snowflakes_and_formats_path() {
    let route = build_event_rsvp_route(VALID_GUILD_ID, VALID_EVENT_ID).expect("valid rsvp route");
    assert_eq!(
        route,
        "/api/v9/guilds/100000000000000001/scheduled-events/100000000000000003/users/@me"
    );

    let boundary = build_event_rsvp_route("1", &"9".repeat(20)).expect("boundary rsvp route");
    assert_eq!(
        boundary,
        format!(
            "/api/v9/guilds/1/scheduled-events/{}/users/@me",
            "9".repeat(20)
        )
    );

    let twenty_one_digits = "1".repeat(21);
    for bad in ["", "0", "0123", "abc", "-1", " 123", &twenty_one_digits] {
        assert!(
            build_event_rsvp_route(bad, VALID_EVENT_ID).is_err(),
            "guild_id={bad:?} must fail"
        );
        assert!(
            build_event_rsvp_route(VALID_GUILD_ID, bad).is_err(),
            "event_id={bad:?} must fail"
        );
    }
}
