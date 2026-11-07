//! RW-40 / task-249 (`BE-P1-28`): контрактные тесты BFF-модуля правил автомодерации гильдии (`automod`).
//!
//! Проверяет:
//! 1. Константы:
//!    - `AUTOMOD_EVENT_MESSAGE_SEND = 1`;
//!    - `AUTOMOD_TRIGGER_KEYWORD = 1`, `AUTOMOD_TRIGGER_SPAM = 3`,
//!      `AUTOMOD_TRIGGER_KEYWORD_PRESET = 4`, `AUTOMOD_TRIGGER_MENTION_SPAM = 5`;
//!    - `AUTOMOD_ACTION_BLOCK_MESSAGE = 1`, `AUTOMOD_ACTION_SEND_ALERT = 2`,
//!      `AUTOMOD_ACTION_TIMEOUT = 3`;
//!    - `MAX_AUTOMOD_NAME_CHARS = 100`, `MAX_KEYWORD_FILTER_ITEMS = 1000`,
//!      `MAX_MENTION_TOTAL_LIMIT = 50`, `MAX_TIMEOUT_DURATION_SECS = 2_419_200`.
//! 2. Структуру `UpsertAutoModRuleRequest` и её serde round-trip.
//! 3. `validate_automod_rule_request(req: &UpsertAutoModRuleRequest) -> Result<(), String>`:
//!    - `req.name.trim()` должен содержать от `1` до `MAX_AUTOMOD_NAME_CHARS` (`100`) символов Unicode;
//!    - `req.event_type` строго равен `AUTOMOD_EVENT_MESSAGE_SEND` (`1`);
//!    - при `trigger_type == 1` (`KEYWORD`): `keyword_filter` содержит `1..=1000` элементов,
//!      каждый после `trim()` непустой и длиной `<= 60` символов Unicode;
//!    - при `trigger_type == 5` (`MENTION_SPAM`): `mention_total_limit` равен `Some(1..=50)`;
//!    - при `trigger_type == 3` (`SPAM`) или `4` (`KEYWORD_PRESET`): допустимы;
//!    - любые другие значения `trigger_type` возвращают `Err`;
//!    - `action_type` равен `1`, `2` или `3`; при `action_type == 3` (`TIMEOUT`)
//!      `timeout_duration_secs` равен `Some(1..=2_419_200)`.
//! 4. `build_guild_automod_route(guild_id: &str, rule_id: Option<&str>) -> Result<String, String>`:
//!    - валидирует `guild_id` (и `rule_id`, если `Some`) как положительные числовые Snowflake
//!      (`1..=20` ASCII-цифр, без ведущего `'0'`);
//!    - возвращает `"/api/v9/guilds/{guild_id}/auto-moderation/rules"` или
//!      `"/api/v9/guilds/{guild_id}/auto-moderation/rules/{rule_id}"`.

use discord_gui_spike::automod::{
    build_guild_automod_route, validate_automod_rule_request, UpsertAutoModRuleRequest,
    AUTOMOD_ACTION_BLOCK_MESSAGE, AUTOMOD_ACTION_SEND_ALERT, AUTOMOD_ACTION_TIMEOUT,
    AUTOMOD_EVENT_MESSAGE_SEND, AUTOMOD_TRIGGER_KEYWORD, AUTOMOD_TRIGGER_KEYWORD_PRESET,
    AUTOMOD_TRIGGER_MENTION_SPAM, AUTOMOD_TRIGGER_SPAM, MAX_AUTOMOD_NAME_CHARS,
    MAX_KEYWORD_FILTER_ITEMS, MAX_MENTION_TOTAL_LIMIT, MAX_TIMEOUT_DURATION_SECS,
};
use serde_json::json;

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_RULE_ID: &str = "100000000000000002";

fn sample_keyword_rule() -> UpsertAutoModRuleRequest {
    UpsertAutoModRuleRequest {
        name: "Фильтр плохих слов".to_string(),
        event_type: AUTOMOD_EVENT_MESSAGE_SEND,
        trigger_type: AUTOMOD_TRIGGER_KEYWORD,
        keyword_filter: vec!["спам*".to_string(), "*фишинг*".to_string()],
        mention_total_limit: None,
        action_type: AUTOMOD_ACTION_BLOCK_MESSAGE,
        timeout_duration_secs: None,
        enabled: true,
    }
}

fn sample_mention_spam_timeout_rule() -> UpsertAutoModRuleRequest {
    UpsertAutoModRuleRequest {
        name: "Анти-рейд упоминаний".to_string(),
        event_type: AUTOMOD_EVENT_MESSAGE_SEND,
        trigger_type: AUTOMOD_TRIGGER_MENTION_SPAM,
        keyword_filter: vec![],
        mention_total_limit: Some(15),
        action_type: AUTOMOD_ACTION_TIMEOUT,
        timeout_duration_secs: Some(3600),
        enabled: true,
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры UpsertAutoModRuleRequest
// ---------------------------------------------------------------------------

#[test]
fn automod_constants_match_discord_contract() {
    assert_eq!(AUTOMOD_EVENT_MESSAGE_SEND, 1);

    assert_eq!(AUTOMOD_TRIGGER_KEYWORD, 1);
    assert_eq!(AUTOMOD_TRIGGER_SPAM, 3);
    assert_eq!(AUTOMOD_TRIGGER_KEYWORD_PRESET, 4);
    assert_eq!(AUTOMOD_TRIGGER_MENTION_SPAM, 5);

    assert_eq!(AUTOMOD_ACTION_BLOCK_MESSAGE, 1);
    assert_eq!(AUTOMOD_ACTION_SEND_ALERT, 2);
    assert_eq!(AUTOMOD_ACTION_TIMEOUT, 3);

    assert_eq!(MAX_AUTOMOD_NAME_CHARS, 100);
    assert_eq!(MAX_KEYWORD_FILTER_ITEMS, 1000);
    assert_eq!(MAX_MENTION_TOTAL_LIMIT, 50);
    assert_eq!(MAX_TIMEOUT_DURATION_SECS, 2_419_200);
}

#[test]
fn upsert_automod_rule_request_serde_roundtrip() {
    let req = sample_keyword_rule();
    let val = serde_json::to_value(&req).expect("serialize UpsertAutoModRuleRequest");
    assert_eq!(
        val,
        json!({
            "name": "Фильтр плохих слов",
            "event_type": 1,
            "trigger_type": 1,
            "keyword_filter": ["спам*", "*фишинг*"],
            "mention_total_limit": null,
            "action_type": 1,
            "timeout_duration_secs": null,
            "enabled": true
        })
    );
    let back: UpsertAutoModRuleRequest =
        serde_json::from_value(val).expect("deserialize UpsertAutoModRuleRequest");
    assert_eq!(back, req);

    let req2 = sample_mention_spam_timeout_rule();
    let val2 = serde_json::to_value(&req2).expect("serialize mention spam rule");
    let back2: UpsertAutoModRuleRequest =
        serde_json::from_value(val2).expect("deserialize mention spam rule");
    assert_eq!(back2, req2);
}

// ---------------------------------------------------------------------------
// 2. validate_automod_rule_request: имя (1..=100 Unicode после trim) и event_type == 1
// ---------------------------------------------------------------------------

#[test]
fn validate_automod_rule_request_enforces_name_bounds_and_event_type() {
    let mut req = sample_keyword_rule();
    assert!(validate_automod_rule_request(&req).is_ok());

    // Границы имени (1..=100 символов Unicode после trim)
    req.name = "Я".to_string();
    assert!(validate_automod_rule_request(&req).is_ok());

    req.name = format!("  {}  ", "Я".repeat(MAX_AUTOMOD_NAME_CHARS));
    assert!(validate_automod_rule_request(&req).is_ok());

    for bad_name in [
        "".to_string(),
        "   \t\n ".to_string(),
        "Я".repeat(MAX_AUTOMOD_NAME_CHARS + 1),
        format!("  {}  ", "a".repeat(MAX_AUTOMOD_NAME_CHARS + 1)),
    ] {
        req.name = bad_name;
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "invalid name should be rejected"
        );
    }
    req.name = "Валидное правило".to_string();

    // Только event_type == 1 (AUTOMOD_EVENT_MESSAGE_SEND) допустим
    for bad_event in [0u8, 2, 3, 255] {
        req.event_type = bad_event;
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "event_type={bad_event} must be rejected"
        );
    }
    req.event_type = AUTOMOD_EVENT_MESSAGE_SEND;
    assert!(validate_automod_rule_request(&req).is_ok());
}

// ---------------------------------------------------------------------------
// 3. validate_automod_rule_request: триггеры KEYWORD (1), SPAM (3), KEYWORD_PRESET (4), MENTION_SPAM (5)
// ---------------------------------------------------------------------------

#[test]
fn validate_automod_rule_request_validates_keyword_trigger_bounds() {
    let mut req = sample_keyword_rule();

    // 1 элемент длиной 1 и 60 символов Unicode (после trim) -> Ok
    req.keyword_filter = vec!["я".to_string()];
    assert!(validate_automod_rule_request(&req).is_ok());

    req.keyword_filter = vec![format!("  {}  ", "Ы".repeat(60))];
    assert!(validate_automod_rule_request(&req).is_ok());

    // Ровно MAX_KEYWORD_FILTER_ITEMS (1000) элементов -> Ok
    req.keyword_filter = (0..MAX_KEYWORD_FILTER_ITEMS)
        .map(|i| format!("kw_{i}"))
        .collect();
    assert_eq!(req.keyword_filter.len(), 1000);
    assert!(validate_automod_rule_request(&req).is_ok());

    // Пустой список keyword_filter при trigger_type == 1 -> Err
    req.keyword_filter.clear();
    assert!(validate_automod_rule_request(&req).is_err());

    // 1001 элемент -> Err
    req.keyword_filter = (0..=MAX_KEYWORD_FILTER_ITEMS)
        .map(|i| format!("kw_{i}"))
        .collect();
    assert_eq!(req.keyword_filter.len(), 1001);
    assert!(validate_automod_rule_request(&req).is_err());

    // Элемент пустой после trim или длиннее 60 символов -> Err
    for bad_kw in [
        "".to_string(),
        "   \t ".to_string(),
        "Ы".repeat(61),
        format!("  {}  ", "a".repeat(61)),
    ] {
        req.keyword_filter = vec!["valid".to_string(), bad_kw.clone()];
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "keyword_filter item {bad_kw:?} must be rejected"
        );
    }
}

#[test]
fn validate_automod_rule_request_validates_mention_spam_spam_preset_and_unknown_triggers() {
    // MENTION_SPAM (5): mention_total_limit must be Some(1..=50)
    let mut req = sample_mention_spam_timeout_rule();
    req.action_type = AUTOMOD_ACTION_BLOCK_MESSAGE;
    req.timeout_duration_secs = None;

    for valid_limit in [1u8, 25, MAX_MENTION_TOTAL_LIMIT] {
        req.mention_total_limit = Some(valid_limit);
        assert!(
            validate_automod_rule_request(&req).is_ok(),
            "mention_total_limit={valid_limit} must be accepted"
        );
    }

    req.mention_total_limit = None;
    assert!(validate_automod_rule_request(&req).is_err());

    for bad_limit in [0u8, MAX_MENTION_TOTAL_LIMIT + 1, 51, u8::MAX] {
        req.mention_total_limit = Some(bad_limit);
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "mention_total_limit={bad_limit} must be rejected"
        );
    }

    // SPAM (3) и KEYWORD_PRESET (4) допустимы без keyword_filter и mention_total_limit
    for trigger in [AUTOMOD_TRIGGER_SPAM, AUTOMOD_TRIGGER_KEYWORD_PRESET] {
        let mut preset_req = sample_keyword_rule();
        preset_req.trigger_type = trigger;
        preset_req.keyword_filter.clear();
        preset_req.mention_total_limit = None;
        assert!(
            validate_automod_rule_request(&preset_req).is_ok(),
            "trigger_type={trigger} must be accepted"
        );
    }

    // Неизвестные trigger_type (0, 2, 6, 255) отклоняются
    for bad_trigger in [0u8, 2, 6, 7, 255] {
        let mut bad_req = sample_keyword_rule();
        bad_req.trigger_type = bad_trigger;
        assert!(
            validate_automod_rule_request(&bad_req).is_err(),
            "trigger_type={bad_trigger} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. validate_automod_rule_request: action_type (1, 2, 3) и timeout_duration_secs (1..=2_419_200)
// ---------------------------------------------------------------------------

#[test]
fn validate_automod_rule_request_validates_action_types_and_timeout_duration() {
    let mut req = sample_keyword_rule();

    // action_type 1 (BLOCK_MESSAGE) и 2 (SEND_ALERT) допустимы без timeout_duration_secs
    for action in [AUTOMOD_ACTION_BLOCK_MESSAGE, AUTOMOD_ACTION_SEND_ALERT] {
        req.action_type = action;
        req.timeout_duration_secs = None;
        assert!(
            validate_automod_rule_request(&req).is_ok(),
            "action_type={action} must be accepted"
        );
    }

    // action_type 3 (TIMEOUT): timeout_duration_secs должен быть Some(1..=2_419_200)
    req.action_type = AUTOMOD_ACTION_TIMEOUT;
    for valid_secs in [1u32, 60, 86_400, MAX_TIMEOUT_DURATION_SECS] {
        req.timeout_duration_secs = Some(valid_secs);
        assert!(
            validate_automod_rule_request(&req).is_ok(),
            "timeout_duration_secs={valid_secs} must be accepted"
        );
    }

    req.timeout_duration_secs = None;
    assert!(validate_automod_rule_request(&req).is_err());

    for bad_secs in [0u32, MAX_TIMEOUT_DURATION_SECS + 1, 3_000_000, u32::MAX] {
        req.timeout_duration_secs = Some(bad_secs);
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "timeout_duration_secs={bad_secs} must be rejected"
        );
    }

    // Невалидные action_type (0, 4, 255) отклоняются
    for bad_action in [0u8, 4, 5, 255] {
        req.action_type = bad_action;
        req.timeout_duration_secs = Some(60);
        assert!(
            validate_automod_rule_request(&req).is_err(),
            "action_type={bad_action} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. build_guild_automod_route: маршрут списка и конкретного правила автомодерации
// ---------------------------------------------------------------------------

#[test]
fn build_guild_automod_route_formats_collection_and_item_routes_and_validates_snowflakes() {
    let collection_route =
        build_guild_automod_route(VALID_GUILD_ID, None).expect("valid collection route");
    assert_eq!(
        collection_route,
        "/api/v9/guilds/100000000000000001/auto-moderation/rules"
    );

    let item_route =
        build_guild_automod_route(VALID_GUILD_ID, Some(VALID_RULE_ID)).expect("valid item route");
    assert_eq!(
        item_route,
        "/api/v9/guilds/100000000000000001/auto-moderation/rules/100000000000000002"
    );

    let max_20 = "9".repeat(20);
    let boundary_route =
        build_guild_automod_route("1", Some(&max_20)).expect("boundary snowflakes route");
    assert_eq!(
        boundary_route,
        format!("/api/v9/guilds/1/auto-moderation/rules/{max_20}")
    );

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
            build_guild_automod_route(bad, None).is_err(),
            "guild_id={bad:?} (None rule_id) must fail"
        );
        assert!(
            build_guild_automod_route(bad, Some(VALID_RULE_ID)).is_err(),
            "guild_id={bad:?} (Some rule_id) must fail"
        );
        assert!(
            build_guild_automod_route(VALID_GUILD_ID, Some(bad)).is_err(),
            "rule_id={bad:?} must fail"
        );
    }
}
