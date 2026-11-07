//! RW-40 / task-223 (BE-P1-20): контрактные тесты BFF-модуля создания DM / Group DM каналов
//! и управления участниками Group DM (`dm_channels`).
//!
//! Проверяет:
//! 1. Константы `MAX_GROUP_DM_RECIPIENTS = 9`, `MAX_GROUP_DM_NAME_CHARS = 100`.
//! 2. Структуру `CreateDmRequest` и её serde round-trip.
//! 3. `validate_create_dm_request(req: &CreateDmRequest) -> Result<(), String>`:
//!    - отклонение пустого списка `recipient_ids` и `recipient_ids.len() > MAX_GROUP_DM_RECIPIENTS` (`9`);
//!    - проверка каждого `recipient_id` как канонической положительной `u64` Snowflake строки
//!      (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей);
//!    - отклонение дублирующихся `recipient_id`.
//! 4. `build_create_dm_body(req: &CreateDmRequest) -> Result<serde_json::Value, String>`:
//!    - вызывает `validate_create_dm_request(req)?`;
//!    - при `recipient_ids.len() == 1` возвращает `Ok(json!({"recipient_id": req.recipient_ids[0]}))`;
//!    - при `2..=9` возвращает `Ok(json!({"recipients": req.recipient_ids}))`.
//! 5. `build_group_dm_recipient_route(channel_id: &str, user_id: &str) -> Result<String, String>`:
//!    - валидация `channel_id` и `user_id` как канонических положительных `u64` Snowflake ID;
//!    - построение маршрута `"/channels/{channel_id}/recipients/{user_id}"`
//!      (сверка с RE-оракулом `rest-static.json:2594` `CHANNEL_RECIPIENT`).

use discord_gui_spike::dm_channels::{
    build_create_dm_body, build_group_dm_recipient_route, validate_create_dm_request,
    CreateDmRequest, MAX_GROUP_DM_NAME_CHARS, MAX_GROUP_DM_RECIPIENTS,
};
use serde_json::json;

const VALID_CHANNEL_ID: &str = "100000000000000001";
const VALID_USER_ID: &str = "100000000000000002";

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры CreateDmRequest
// ---------------------------------------------------------------------------

#[test]
fn dm_channels_constants_match_discord_contract() {
    assert_eq!(MAX_GROUP_DM_RECIPIENTS, 9);
    assert_eq!(MAX_GROUP_DM_NAME_CHARS, 100);
}

#[test]
fn create_dm_request_serde_roundtrip() {
    let req = CreateDmRequest {
        recipient_ids: vec![
            "100000000000000001".to_string(),
            "100000000000000002".to_string(),
        ],
    };
    let serialized = serde_json::to_value(&req).expect("serialize CreateDmRequest");
    assert_eq!(
        serialized,
        json!({
            "recipient_ids": ["100000000000000001", "100000000000000002"]
        })
    );
    let restored: CreateDmRequest =
        serde_json::from_value(serialized).expect("deserialize CreateDmRequest");
    assert_eq!(restored, req);
}

// ---------------------------------------------------------------------------
// 2. validate_create_dm_request: границы длины (1..=9), Snowflake (1..=u64::MAX), дубликаты
// ---------------------------------------------------------------------------

#[test]
fn validate_create_dm_request_enforces_recipient_count_bounds() {
    // Пустой список отклоняется
    let empty_req = CreateDmRequest {
        recipient_ids: vec![],
    };
    assert!(validate_create_dm_request(&empty_req).is_err());

    // 1 получатель (одиночный DM) -> Ok
    let single_req = CreateDmRequest {
        recipient_ids: vec![VALID_USER_ID.to_string()],
    };
    assert!(validate_create_dm_request(&single_req).is_ok());

    // 9 получателей (MAX_GROUP_DM_RECIPIENTS) -> Ok
    let max_req = CreateDmRequest {
        recipient_ids: (1..=MAX_GROUP_DM_RECIPIENTS)
            .map(|i| format!("10000000000000000{i}"))
            .collect(),
    };
    assert_eq!(max_req.recipient_ids.len(), 9);
    assert!(validate_create_dm_request(&max_req).is_ok());

    // 10 получателей (MAX_GROUP_DM_RECIPIENTS + 1) -> Err
    let too_many_req = CreateDmRequest {
        recipient_ids: (1..=MAX_GROUP_DM_RECIPIENTS + 1)
            .map(|i| format!("1000000000000000{i:02}"))
            .collect(),
    };
    assert_eq!(too_many_req.recipient_ids.len(), 10);
    assert!(validate_create_dm_request(&too_many_req).is_err());
}

#[test]
fn validate_create_dm_request_checks_canonical_positive_u64_snowflakes() {
    let max_u64 = u64::MAX.to_string();
    let boundary_req = CreateDmRequest {
        recipient_ids: vec!["1".to_string(), max_u64],
    };
    assert!(validate_create_dm_request(&boundary_req).is_ok());

    let invalid_snowflakes = [
        "",
        "0",
        "000",
        "0123456789",
        "01",
        "abc",
        "123a456",
        "-1",
        "+123",
        " 100000000000000001",
        "100000000000000001 ",
        "18446744073709551616", // u64::MAX + 1
    ];

    for bad in invalid_snowflakes {
        let req_single = CreateDmRequest {
            recipient_ids: vec![bad.to_string()],
        };
        assert!(
            validate_create_dm_request(&req_single).is_err(),
            "recipient_id={bad:?} must be rejected in 1-element list"
        );

        let req_in_group = CreateDmRequest {
            recipient_ids: vec![VALID_USER_ID.to_string(), bad.to_string()],
        };
        assert!(
            validate_create_dm_request(&req_in_group).is_err(),
            "recipient_id={bad:?} must be rejected in group DM list"
        );
    }
}

#[test]
fn validate_create_dm_request_rejects_duplicate_recipients() {
    let dup_pair = CreateDmRequest {
        recipient_ids: vec![VALID_USER_ID.to_string(), VALID_USER_ID.to_string()],
    };
    assert!(validate_create_dm_request(&dup_pair).is_err());

    let dup_non_adjacent = CreateDmRequest {
        recipient_ids: vec![
            "100000000000000001".to_string(),
            "100000000000000002".to_string(),
            "100000000000000003".to_string(),
            "100000000000000001".to_string(),
        ],
    };
    assert!(validate_create_dm_request(&dup_non_adjacent).is_err());
}

// ---------------------------------------------------------------------------
// 3. build_create_dm_body: одиночный DM {"recipient_id": ...} vs Group DM {"recipients": [...]}
// ---------------------------------------------------------------------------

#[test]
fn build_create_dm_body_returns_recipient_id_for_single_and_recipients_for_group() {
    // 1 получатель -> {"recipient_id": "..."}
    let single = CreateDmRequest {
        recipient_ids: vec!["100000000000000001".to_string()],
    };
    let single_body = build_create_dm_body(&single).expect("valid single DM body");
    assert_eq!(
        single_body,
        json!({
            "recipient_id": "100000000000000001"
        })
    );

    // 2 получателя -> {"recipients": ["...", "..."]}
    let two = CreateDmRequest {
        recipient_ids: vec![
            "100000000000000001".to_string(),
            "100000000000000002".to_string(),
        ],
    };
    let two_body = build_create_dm_body(&two).expect("valid 2-recipient group DM body");
    assert_eq!(
        two_body,
        json!({
            "recipients": ["100000000000000001", "100000000000000002"]
        })
    );

    // 9 получателей -> {"recipients": [...9 элементов...]}
    let nine_ids: Vec<String> = (1..=9).map(|i| format!("10000000000000000{i}")).collect();
    let nine = CreateDmRequest {
        recipient_ids: nine_ids.clone(),
    };
    let nine_body = build_create_dm_body(&nine).expect("valid 9-recipient group DM body");
    assert_eq!(
        nine_body,
        json!({
            "recipients": nine_ids
        })
    );
}

#[test]
fn build_create_dm_body_propagates_validation_errors() {
    // Пустой список
    assert!(build_create_dm_body(&CreateDmRequest {
        recipient_ids: vec![],
    })
    .is_err());

    // 10 получателей
    assert!(build_create_dm_body(&CreateDmRequest {
        recipient_ids: (1..=10).map(|i| i.to_string()).collect(),
    })
    .is_err());

    // Невалидный Snowflake
    assert!(build_create_dm_body(&CreateDmRequest {
        recipient_ids: vec!["0123".to_string()],
    })
    .is_err());

    // Дубликат
    assert!(build_create_dm_body(&CreateDmRequest {
        recipient_ids: vec!["42".to_string(), "42".to_string()],
    })
    .is_err());
}

// ---------------------------------------------------------------------------
// 4. build_group_dm_recipient_route: канонический маршрут и валидация Snowflake
// ---------------------------------------------------------------------------

#[test]
fn build_group_dm_recipient_route_returns_canonical_path_and_validates_inputs() {
    let route = build_group_dm_recipient_route(VALID_CHANNEL_ID, VALID_USER_ID)
        .expect("valid group DM recipient route");
    assert_eq!(
        route,
        "/channels/100000000000000001/recipients/100000000000000002"
    );

    let max_u64 = u64::MAX.to_string();
    let boundary_route =
        build_group_dm_recipient_route("1", &max_u64).expect("boundary group DM recipient route");
    assert_eq!(boundary_route, format!("/channels/1/recipients/{max_u64}"));

    for bad in [
        "",
        "0",
        "000",
        "0123",
        "abc",
        "-1",
        " 123",
        "123 ",
        "18446744073709551616",
    ] {
        assert!(
            build_group_dm_recipient_route(bad, VALID_USER_ID).is_err(),
            "channel_id={bad:?} must be rejected"
        );
        assert!(
            build_group_dm_recipient_route(VALID_CHANNEL_ID, bad).is_err(),
            "user_id={bad:?} must be rejected"
        );
    }
}
