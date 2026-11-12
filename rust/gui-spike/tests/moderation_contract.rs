//! RW-40 / task-237 (BE-P1-24): контрактные тесты BFF-модуля модерации гильдии (`moderation`).
//!
//! Проверяет:
//! 1. Константы:
//!    - `MAX_AUDIT_REASON_CHARS = 512`;
//!    - `MAX_BAN_DELETE_MESSAGE_SECONDS = 604_800` (7 дней);
//!    - `MAX_TIMEOUT_SECONDS = 2_419_200` (28 дней);
//!    - `MAX_AUDIT_LOG_LIMIT = 100`.
//! 2. Структуру `BanMemberRequest` и её serde round-trip.
//! 3. `validate_audit_reason(reason: Option<&str>) -> Result<Option<String>, String>`:
//!    - `None` -> `Ok(None)`;
//!    - `Some(r)` с пустой или пробельной строкой после `trim()` -> `Ok(None)`;
//!    - `Some(r)` с длиной `r.chars().count() > MAX_AUDIT_REASON_CHARS` (512 Unicode-символов) -> `Err`;
//!    - иначе -> `Ok(Some(trimmed.to_string()))`.
//! 4. `build_ban_member_route_and_body(req: &BanMemberRequest) -> Result<(String, serde_json::Value, Option<String>), String>`:
//!    - валидация `guild_id` и `user_id` как канонических положительных `u64` Snowflake ID
//!      (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей);
//!    - проверка `req.delete_message_seconds <= MAX_BAN_DELETE_MESSAGE_SECONDS` (`0..=604_800`);
//!    - валидация `req.reason` через `validate_audit_reason(req.reason.as_deref())?`;
//!    - возврат `("/guilds/{guild_id}/bans/{user_id}", json!({"delete_message_seconds": ...}), clean_reason)`.
//! 5. `build_audit_log_query_route(guild_id: &str, action_type: Option<u16>, limit: u8) -> Result<String, String>`:
//!    - валидация `guild_id` как канонического положительного `u64` Snowflake ID;
//!    - проверка `1 <= limit && limit <= MAX_AUDIT_LOG_LIMIT` (`1..=100`);
//!    - формирование маршрута `"/guilds/{guild_id}/audit-logs?limit={limit}"`
//!      (и добавление `"&action_type={at}"`, если `action_type` задан).

use chord_client::moderation::{
    build_audit_log_query_route, build_ban_member_route_and_body, validate_audit_reason,
    BanMemberRequest, MAX_AUDIT_LOG_LIMIT, MAX_AUDIT_REASON_CHARS, MAX_BAN_DELETE_MESSAGE_SECONDS,
    MAX_TIMEOUT_SECONDS,
};
use serde_json::json;

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_USER_ID: &str = "100000000000000002";

fn sample_ban_request() -> BanMemberRequest {
    BanMemberRequest {
        guild_id: VALID_GUILD_ID.to_string(),
        user_id: VALID_USER_ID.to_string(),
        delete_message_seconds: 86_400,
        reason: Some("  Спам в основном канале  ".to_string()),
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры BanMemberRequest
// ---------------------------------------------------------------------------

#[test]
fn moderation_constants_match_discord_contract() {
    assert_eq!(MAX_AUDIT_REASON_CHARS, 512);
    assert_eq!(MAX_BAN_DELETE_MESSAGE_SECONDS, 604_800);
    assert_eq!(MAX_TIMEOUT_SECONDS, 2_419_200);
    assert_eq!(MAX_AUDIT_LOG_LIMIT, 100);
}

#[test]
fn ban_member_request_serde_roundtrip() {
    let req = sample_ban_request();
    let serialized = serde_json::to_value(&req).expect("serialize BanMemberRequest");
    assert_eq!(
        serialized,
        json!({
            "guild_id": "100000000000000001",
            "user_id": "100000000000000002",
            "delete_message_seconds": 86_400u32,
            "reason": "  Спам в основном канале  "
        })
    );
    let restored: BanMemberRequest =
        serde_json::from_value(serialized).expect("deserialize BanMemberRequest");
    assert_eq!(restored, req);
}

// ---------------------------------------------------------------------------
// 2. validate_audit_reason: None, trim-empty, Unicode length bounds (..=512)
// ---------------------------------------------------------------------------

#[test]
fn validate_audit_reason_handles_none_blank_and_trims_valid_strings() {
    assert_eq!(validate_audit_reason(None), Ok(None));

    for blank in ["", " ", "   ", "\t\n\r "] {
        assert_eq!(
            validate_audit_reason(Some(blank)),
            Ok(None),
            "blank reason={blank:?} must normalize to Ok(None)"
        );
    }

    assert_eq!(
        validate_audit_reason(Some("Нарушение правил")),
        Ok(Some("Нарушение правил".to_string()))
    );
    assert_eq!(
        validate_audit_reason(Some("  Нарушение правил \n\t ")),
        Ok(Some("Нарушение правил".to_string()))
    );
}

#[test]
fn validate_audit_reason_enforces_512_unicode_char_limit() {
    // Ровно 512 символов (ASCII и кириллица) -> Ok
    for ch in ["a", "Я"] {
        let exact_512 = ch.repeat(MAX_AUDIT_REASON_CHARS);
        assert_eq!(exact_512.chars().count(), 512);
        assert_eq!(
            validate_audit_reason(Some(&exact_512)),
            Ok(Some(exact_512.clone()))
        );
    }

    // 513 символов (ASCII и кириллица) -> Err
    for ch in ["a", "Я"] {
        let too_long_513 = ch.repeat(MAX_AUDIT_REASON_CHARS + 1);
        assert_eq!(too_long_513.chars().count(), 513);
        assert!(
            validate_audit_reason(Some(&too_long_513)).is_err(),
            "513-char reason (ch={ch:?}) must be rejected"
        );
    }

    // Строка длиной 513 за счёт внешних пробелов (511 символов + 2 пробела) -> Err,
    // поскольку r.chars().count() > MAX_AUDIT_REASON_CHARS проверяется на исходном срезе r
    let padded_513 = format!(" {} ", "a".repeat(MAX_AUDIT_REASON_CHARS - 1));
    assert_eq!(padded_513.chars().count(), 513);
    assert!(validate_audit_reason(Some(&padded_513)).is_err());
}

// ---------------------------------------------------------------------------
// 3. build_ban_member_route_and_body: маршрут, JSON-тело, Snowflakes, лимиты
// ---------------------------------------------------------------------------

#[test]
fn build_ban_member_route_and_body_returns_canonical_route_json_and_clean_reason() {
    let req = sample_ban_request();
    let (route, body, clean_reason) =
        build_ban_member_route_and_body(&req).expect("valid BanMemberRequest");
    assert_eq!(route, "/guilds/100000000000000001/bans/100000000000000002");
    assert_eq!(body, json!({ "delete_message_seconds": 86_400 }));
    assert_eq!(clean_reason, Some("Спам в основном канале".to_string()));

    // Граничные значения: guild_id="1", user_id=u64::MAX, delete_message_seconds=0 и 604_800, reason=Some("   ")
    let max_u64 = u64::MAX.to_string();
    let boundary_req = BanMemberRequest {
        guild_id: "1".to_string(),
        user_id: max_u64.clone(),
        delete_message_seconds: MAX_BAN_DELETE_MESSAGE_SECONDS,
        reason: Some("   ".to_string()),
    };
    let (route2, body2, reason2) =
        build_ban_member_route_and_body(&boundary_req).expect("boundary BanMemberRequest");
    assert_eq!(route2, format!("/guilds/1/bans/{max_u64}"));
    assert_eq!(
        body2,
        json!({ "delete_message_seconds": MAX_BAN_DELETE_MESSAGE_SECONDS })
    );
    assert_eq!(reason2, None);

    let zero_del_req = BanMemberRequest {
        guild_id: VALID_GUILD_ID.to_string(),
        user_id: VALID_USER_ID.to_string(),
        delete_message_seconds: 0,
        reason: None,
    };
    let (_, body3, reason3) =
        build_ban_member_route_and_body(&zero_del_req).expect("0 delete_message_seconds");
    assert_eq!(body3, json!({ "delete_message_seconds": 0 }));
    assert_eq!(reason3, None);
}

#[test]
fn build_ban_member_route_and_body_rejects_invalid_snowflakes_seconds_and_reason() {
    let invalid_snowflakes = [
        "",
        "0",
        "00",
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
        let mut req_bad_guild = sample_ban_request();
        req_bad_guild.guild_id = bad.to_string();
        assert!(
            build_ban_member_route_and_body(&req_bad_guild).is_err(),
            "guild_id={bad:?} must be rejected"
        );

        let mut req_bad_user = sample_ban_request();
        req_bad_user.user_id = bad.to_string();
        assert!(
            build_ban_member_route_and_body(&req_bad_user).is_err(),
            "user_id={bad:?} must be rejected"
        );
    }

    for bad_seconds in [MAX_BAN_DELETE_MESSAGE_SECONDS + 1, 1_000_000, u32::MAX] {
        let mut req_bad_sec = sample_ban_request();
        req_bad_sec.delete_message_seconds = bad_seconds;
        assert!(
            build_ban_member_route_and_body(&req_bad_sec).is_err(),
            "delete_message_seconds={bad_seconds} must be rejected"
        );
    }

    let mut req_bad_reason = sample_ban_request();
    req_bad_reason.reason = Some("Я".repeat(MAX_AUDIT_REASON_CHARS + 1));
    assert!(
        build_ban_member_route_and_body(&req_bad_reason).is_err(),
        "reason exceeding MAX_AUDIT_REASON_CHARS must be rejected"
    );
}

// ---------------------------------------------------------------------------
// 4. build_audit_log_query_route: маршрут, action_type, limit (1..=100), Snowflake
// ---------------------------------------------------------------------------

#[test]
fn build_audit_log_query_route_formats_query_with_and_without_action_type() {
    let route_no_action =
        build_audit_log_query_route(VALID_GUILD_ID, None, 50).expect("valid audit log route");
    assert_eq!(
        route_no_action,
        "/guilds/100000000000000001/audit-logs?limit=50"
    );

    let route_with_action = build_audit_log_query_route(VALID_GUILD_ID, Some(22), 1)
        .expect("audit log route with action_type=22 (MEMBER_BAN_ADD) and limit=1");
    assert_eq!(
        route_with_action,
        "/guilds/100000000000000001/audit-logs?limit=1&action_type=22"
    );

    let max_u64 = u64::MAX.to_string();
    let route_max = build_audit_log_query_route(&max_u64, Some(0), MAX_AUDIT_LOG_LIMIT)
        .expect("boundary audit log route");
    assert_eq!(
        route_max,
        format!("/guilds/{max_u64}/audit-logs?limit=100&action_type=0")
    );
}

#[test]
fn build_audit_log_query_route_rejects_invalid_guild_id_and_out_of_range_limit() {
    for bad_limit in [0u8, MAX_AUDIT_LOG_LIMIT + 1, 101, u8::MAX] {
        assert!(
            build_audit_log_query_route(VALID_GUILD_ID, None, bad_limit).is_err(),
            "limit={bad_limit} must be rejected"
        );
        assert!(
            build_audit_log_query_route(VALID_GUILD_ID, Some(22), bad_limit).is_err(),
            "limit={bad_limit} with action_type must be rejected"
        );
    }

    for bad_guild in [
        "",
        "0",
        "00",
        "0123",
        "abc",
        "-1",
        " 123",
        "123 ",
        "18446744073709551616",
    ] {
        assert!(
            build_audit_log_query_route(bad_guild, None, 50).is_err(),
            "guild_id={bad_guild:?} must be rejected"
        );
    }
}
