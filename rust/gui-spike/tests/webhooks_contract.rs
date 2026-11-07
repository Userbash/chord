//! RW-40 / task-245 (`BE-P1-27`): контрактные тесты BFF-модуля вебхуков и подписки на новостные
//! каналы (`webhooks`).
//!
//! Проверяет:
//! 1. Константы:
//!    - `WEBHOOK_TYPE_INCOMING: u8 = 1`
//!    - `WEBHOOK_TYPE_CHANNEL_FOLLOWER: u8 = 2`
//!    - `WEBHOOK_TYPE_APPLICATION: u8 = 3`
//!    - `MIN_WEBHOOK_NAME_CHARS: usize = 1`
//!    - `MAX_WEBHOOK_NAME_CHARS: usize = 80`
//! 2. Структуру `CreateWebhookRequest { pub name: String, pub avatar: Option<String> }` и её serde round-trip.
//! 3. `validate_webhook_name(name: &str) -> Result<String, String>`:
//!    - обрезка внешних пробелов (`trim()`);
//!    - длина `MIN_WEBHOOK_NAME_CHARS..=MAX_WEBHOOK_NAME_CHARS` (`1..=80`) символов Unicode (`chars().count()`);
//!    - запрет совпадения без учёта регистра с `"clyde"` и содержания подстроки `"discord"` без учёта регистра;
//!    - возврат `Ok(trimmed.to_string())`.
//! 4. `build_create_webhook_route_and_body(channel_id: &str, req: &CreateWebhookRequest) -> Result<(String, serde_json::Value), String>`:
//!    - валидация `channel_id` как положительного числового Snowflake (`1..=20` ASCII-цифр, без ведущего `'0'`);
//!    - валидация `req.name` через `validate_webhook_name`;
//!    - возврат `("/api/v9/channels/{channel_id}/webhooks", json!({ "name": clean_name, "avatar": req.avatar }))`.
//! 5. `build_follow_channel_route_and_body(news_channel_id: &str, target_channel_id: &str) -> Result<(String, serde_json::Value), String>`:
//!    - валидация обоих Snowflake (`news_channel_id`, `target_channel_id`);
//!    - отклонение совпадения `news_channel_id == target_channel_id`;
//!    - возврат `("/api/v9/channels/{news_channel_id}/followers", json!({ "webhook_channel_id": target_channel_id }))`.

use discord_gui_spike::webhooks::{
    build_create_webhook_route_and_body, build_follow_channel_route_and_body,
    validate_webhook_name, CreateWebhookRequest, MAX_WEBHOOK_NAME_CHARS, MIN_WEBHOOK_NAME_CHARS,
    WEBHOOK_TYPE_APPLICATION, WEBHOOK_TYPE_CHANNEL_FOLLOWER, WEBHOOK_TYPE_INCOMING,
};
use serde_json::json;

const VALID_CHANNEL_ID: &str = "100000000000000001";
const VALID_TARGET_CHANNEL_ID: &str = "100000000000000002";

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры CreateWebhookRequest
// ---------------------------------------------------------------------------

#[test]
fn webhooks_constants_match_discord_contract() {
    assert_eq!(WEBHOOK_TYPE_INCOMING, 1_u8);
    assert_eq!(WEBHOOK_TYPE_CHANNEL_FOLLOWER, 2_u8);
    assert_eq!(WEBHOOK_TYPE_APPLICATION, 3_u8);
    assert_eq!(MIN_WEBHOOK_NAME_CHARS, 1_usize);
    assert_eq!(MAX_WEBHOOK_NAME_CHARS, 80_usize);
}

#[test]
fn create_webhook_request_serde_roundtrip() {
    let with_avatar = CreateWebhookRequest {
        name: "GitHub Releases".to_string(),
        avatar: Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA".to_string()),
    };
    let json_with = serde_json::to_value(&with_avatar).expect("serialize CreateWebhookRequest");
    assert_eq!(
        json_with,
        json!({
            "name": "GitHub Releases",
            "avatar": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAUA"
        })
    );
    let back_with: CreateWebhookRequest =
        serde_json::from_value(json_with).expect("deserialize CreateWebhookRequest");
    assert_eq!(back_with, with_avatar);

    let without_avatar = CreateWebhookRequest {
        name: "CI Bot".to_string(),
        avatar: None,
    };
    let json_without = serde_json::to_value(&without_avatar)
        .expect("serialize CreateWebhookRequest without avatar");
    assert_eq!(
        json_without,
        json!({
            "name": "CI Bot",
            "avatar": null
        })
    );
    let back_without: CreateWebhookRequest = serde_json::from_value(json_without)
        .expect("deserialize CreateWebhookRequest without avatar");
    assert_eq!(back_without, without_avatar);
}

// ---------------------------------------------------------------------------
// 2. validate_webhook_name: trim, 1..=80 Unicode chars, запрет "clyde" и "discord"
// ---------------------------------------------------------------------------

#[test]
fn validate_webhook_name_trims_and_enforces_1_to_80_unicode_char_bounds() {
    // Пустые и пробельные строки отклоняются
    for empty in ["", " ", "   ", "\t\n\r "] {
        assert!(
            validate_webhook_name(empty).is_err(),
            "empty/whitespace webhook name={empty:?} must be rejected"
        );
    }

    // Нижняя граница: 1 символ ASCII и 1 символ Unicode (кириллица), с пробелами по краям
    assert_eq!(
        validate_webhook_name("A").expect("1 ascii char"),
        "A".to_string()
    );
    assert_eq!(
        validate_webhook_name("  Я  ").expect("trimmed 1 cyrillic char"),
        "Я".to_string()
    );
    assert_eq!(
        validate_webhook_name("\tБот уведомлений\n").expect("trimmed russian webhook name"),
        "Бот уведомлений".to_string()
    );

    // Разрешено слово, содержащее "clyde" как часть большего имени (не равное "clyde")
    assert_eq!(
        validate_webhook_name("ClydeBot").expect("ClydeBot is allowed when not equal to clyde"),
        "ClydeBot".to_string()
    );

    // Верхняя граница: ровно 80 символов ASCII и 80 символов Unicode (160 байт UTF-8)
    let ascii_80 = "w".repeat(MAX_WEBHOOK_NAME_CHARS);
    assert_eq!(
        validate_webhook_name(&ascii_80).expect("80 ascii chars"),
        ascii_80
    );
    let padded_ascii_80 = format!("  {ascii_80}  ");
    assert_eq!(
        validate_webhook_name(&padded_ascii_80).expect("padded 80 ascii chars"),
        ascii_80
    );

    let cyrillic_80 = "ж".repeat(MAX_WEBHOOK_NAME_CHARS);
    assert_eq!(cyrillic_80.chars().count(), 80);
    assert_eq!(cyrillic_80.len(), 160);
    assert_eq!(
        validate_webhook_name(&cyrillic_80).expect("80 unicode chars"),
        cyrillic_80
    );

    // За верхней границей: 81 символ ASCII и 81 символ Unicode отклоняются
    let ascii_81 = "w".repeat(MAX_WEBHOOK_NAME_CHARS + 1);
    assert!(validate_webhook_name(&ascii_81).is_err());

    let cyrillic_81 = "ж".repeat(MAX_WEBHOOK_NAME_CHARS + 1);
    assert!(validate_webhook_name(&cyrillic_81).is_err());
}

#[test]
fn validate_webhook_name_rejects_clyde_and_discord_case_insensitively() {
    for forbidden in [
        "clyde",
        "Clyde",
        "CLYDE",
        "cLyDe",
        "  clyde  ",
        "\tCLYDE\n",
        "discord",
        "Discord",
        "DISCORD",
        "dIsCoRd",
        "my-discord-webhook",
        "Discord Alerts",
        "Official DISCORD Bot",
        "  discord_hook  ",
    ] {
        assert!(
            validate_webhook_name(forbidden).is_err(),
            "forbidden webhook name={forbidden:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. build_create_webhook_route_and_body: валидация channel_id + имени и формирование JSON
// ---------------------------------------------------------------------------

#[test]
fn build_create_webhook_route_and_body_returns_canonical_route_and_payload() {
    let req = CreateWebhookRequest {
        name: "  Deploy Notifier  ".to_string(),
        avatar: Some("data:image/png;base64,AAAA".to_string()),
    };
    let (route, body) = build_create_webhook_route_and_body(VALID_CHANNEL_ID, &req)
        .expect("valid create webhook route and body");
    assert_eq!(route, "/api/v9/channels/100000000000000001/webhooks");
    assert_eq!(
        body,
        json!({
            "name": "Deploy Notifier",
            "avatar": "data:image/png;base64,AAAA"
        })
    );

    // С пустым аватаром (None -> null) и граничными снежинками (1 и 20 цифр)
    let req_no_avatar = CreateWebhookRequest {
        name: "Hook".to_string(),
        avatar: None,
    };
    let (route_min, body_min) =
        build_create_webhook_route_and_body("1", &req_no_avatar).expect("min 1-digit snowflake");
    assert_eq!(route_min, "/api/v9/channels/1/webhooks");
    assert_eq!(
        body_min,
        json!({
            "name": "Hook",
            "avatar": null
        })
    );

    let max_20 = "9".repeat(20);
    let (route_max, _) = build_create_webhook_route_and_body(&max_20, &req_no_avatar)
        .expect("max 20-digit snowflake");
    assert_eq!(route_max, format!("/api/v9/channels/{max_20}/webhooks"));
}

#[test]
fn build_create_webhook_route_and_body_rejects_invalid_channel_id_or_name() {
    let valid_req = CreateWebhookRequest {
        name: "Valid Hook".to_string(),
        avatar: None,
    };
    let twenty_one_digits = "1".repeat(21);
    for bad_cid in [
        "",
        "0",
        "00",
        "012345",
        "-1",
        "+1",
        "abc",
        "123a456",
        " 100000000000000001",
        "100000000000000001 ",
        &twenty_one_digits,
    ] {
        assert!(
            build_create_webhook_route_and_body(bad_cid, &valid_req).is_err(),
            "channel_id={bad_cid:?} must be rejected"
        );
    }

    for bad_name in ["", "   ", "Clyde", "My Discord Hook", &"x".repeat(81)] {
        let bad_req = CreateWebhookRequest {
            name: bad_name.to_string(),
            avatar: None,
        };
        assert!(
            build_create_webhook_route_and_body(VALID_CHANNEL_ID, &bad_req).is_err(),
            "webhook name={bad_name:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. build_follow_channel_route_and_body: валидация обоих Snowflake и запрет самоподписки
// ---------------------------------------------------------------------------

#[test]
fn build_follow_channel_route_and_body_returns_canonical_route_and_payload() {
    let (route, body) =
        build_follow_channel_route_and_body(VALID_CHANNEL_ID, VALID_TARGET_CHANNEL_ID)
            .expect("valid follow channel route and body");
    assert_eq!(route, "/api/v9/channels/100000000000000001/followers");
    assert_eq!(
        body,
        json!({
            "webhook_channel_id": "100000000000000002"
        })
    );

    // Граничные снежинки: "1" и 20 девяток
    let max_20 = "9".repeat(20);
    let (route_bound, body_bound) =
        build_follow_channel_route_and_body("1", &max_20).expect("boundary snowflakes");
    assert_eq!(route_bound, "/api/v9/channels/1/followers");
    assert_eq!(
        body_bound,
        json!({
            "webhook_channel_id": max_20
        })
    );
}

#[test]
fn build_follow_channel_route_and_body_rejects_same_channel_and_invalid_snowflakes() {
    // Совпадение news_channel_id == target_channel_id отклоняется
    assert!(
        build_follow_channel_route_and_body(VALID_CHANNEL_ID, VALID_CHANNEL_ID).is_err(),
        "following the same channel must be rejected"
    );

    let twenty_one_digits = "1".repeat(21);
    for bad_id in [
        "",
        "0",
        "00",
        "012345",
        "-1",
        "abc",
        "123a456",
        " 100000000000000001",
        "100000000000000001 ",
        &twenty_one_digits,
    ] {
        assert!(
            build_follow_channel_route_and_body(bad_id, VALID_TARGET_CHANNEL_ID).is_err(),
            "news_channel_id={bad_id:?} must be rejected"
        );
        assert!(
            build_follow_channel_route_and_body(VALID_CHANNEL_ID, bad_id).is_err(),
            "target_channel_id={bad_id:?} must be rejected"
        );
    }
}
