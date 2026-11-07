//! RW-40 / task-229 (BE-P1-22): контрактные тесты BFF-модуля ролей и прав гильдии (`roles`).
//!
//! Проверяет:
//! 1. Константы `MAX_ROLE_NAME_CHARS = 100`, `MAX_RGB_COLOR = 0x00FF_FFFF`, `PERM_ADMINISTRATOR = 1 << 3`.
//! 2. Структуру `UpsertRoleRequest` и её serde round-trip.
//! 3. `validate_upsert_role_request(req: &UpsertRoleRequest) -> Result<(), String>`:
//!    - проверка `guild_id` как канонической положительной `u64` Snowflake строки
//!      (`1..=u64::MAX`, только ASCII-цифры, без ведущих нулей);
//!    - проверка `req.name.trim()` на длину `1..=MAX_ROLE_NAME_CHARS` (`100`) Unicode-символов;
//!    - проверка `req.color <= MAX_RGB_COLOR` (`0..=0x00FF_FFFF`);
//!    - проверка `req.permissions` как канонической десятичной `u64` строки
//!      (`0..=u64::MAX`, без ведущих нулей, кроме `"0"`).
//! 4. `compute_effective_permissions(everyone_perms: u64, role_perms: &[u64], is_owner: bool) -> u64`:
//!    - при `is_owner == true` возвращает `u64::MAX`;
//!    - побитово складывает (`|`) `everyone_perms` и все элементы `role_perms`;
//!    - если в аккумуляторе установлен бит `PERM_ADMINISTRATOR` (`1 << 3`), возвращает `u64::MAX`, иначе аккумулятор.
//! 5. `build_member_role_route(guild_id: &str, user_id: &str, role_id: &str) -> Result<String, String>`:
//!    - валидация `guild_id`, `user_id` и `role_id` как канонических положительных `u64` Snowflake ID;
//!    - построение маршрута `"/guilds/{guild_id}/members/{user_id}/roles/{role_id}"`.

use discord_gui_spike::roles::{
    build_member_role_route, compute_effective_permissions, validate_upsert_role_request,
    UpsertRoleRequest, MAX_RGB_COLOR, MAX_ROLE_NAME_CHARS, PERM_ADMINISTRATOR,
};
use serde_json::json;

const VALID_GUILD_ID: &str = "100000000000000001";
const VALID_USER_ID: &str = "100000000000000002";
const VALID_ROLE_ID: &str = "100000000000000003";

fn sample_upsert_request() -> UpsertRoleRequest {
    UpsertRoleRequest {
        guild_id: VALID_GUILD_ID.to_string(),
        name: "Модераторы".to_string(),
        color: 0x0034_98DB,
        hoist: true,
        mentionable: false,
        permissions: "1071698660929".to_string(),
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и serde round-trip структуры UpsertRoleRequest
// ---------------------------------------------------------------------------

#[test]
fn roles_constants_match_discord_contract() {
    assert_eq!(MAX_ROLE_NAME_CHARS, 100);
    assert_eq!(MAX_RGB_COLOR, 0x00FF_FFFF);
    assert_eq!(PERM_ADMINISTRATOR, 1 << 3);
    assert_eq!(PERM_ADMINISTRATOR, 8);
}

#[test]
fn upsert_role_request_serde_roundtrip() {
    let req = sample_upsert_request();
    let serialized = serde_json::to_value(&req).expect("serialize UpsertRoleRequest");
    assert_eq!(
        serialized,
        json!({
            "guild_id": "100000000000000001",
            "name": "Модераторы",
            "color": 0x0034_98DBu32,
            "hoist": true,
            "mentionable": false,
            "permissions": "1071698660929"
        })
    );
    let restored: UpsertRoleRequest =
        serde_json::from_value(serialized).expect("deserialize UpsertRoleRequest");
    assert_eq!(restored, req);
}

// ---------------------------------------------------------------------------
// 2. validate_upsert_role_request: guild_id Snowflake (1..=u64::MAX)
// ---------------------------------------------------------------------------

#[test]
fn validate_upsert_role_request_checks_canonical_guild_id_snowflake() {
    let mut min_req = sample_upsert_request();
    min_req.guild_id = "1".to_string();
    assert!(validate_upsert_role_request(&min_req).is_ok());

    let mut max_req = sample_upsert_request();
    max_req.guild_id = u64::MAX.to_string();
    assert!(validate_upsert_role_request(&max_req).is_ok());

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
        let mut req = sample_upsert_request();
        req.guild_id = bad.to_string();
        assert!(
            validate_upsert_role_request(&req).is_err(),
            "guild_id={bad:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. validate_upsert_role_request: name.trim() Unicode length (1..=100)
// ---------------------------------------------------------------------------

#[test]
fn validate_upsert_role_request_enforces_trimmed_unicode_name_bounds() {
    // Пустое и пробельное имя отклоняется
    for blank in ["", "   ", "\t\n\r "] {
        let mut req = sample_upsert_request();
        req.name = blank.to_string();
        assert!(
            validate_upsert_role_request(&req).is_err(),
            "blank name={blank:?} must be rejected"
        );
    }

    // 1 символ (ASCII и кириллица) -> Ok
    for one_char in ["A", "Я", "  Я  "] {
        let mut req = sample_upsert_request();
        req.name = one_char.to_string();
        assert!(
            validate_upsert_role_request(&req).is_ok(),
            "1-char trimmed name={one_char:?} must be accepted"
        );
    }

    // 100 символов Unicode (кириллица, 200 байт в UTF-8, но ровно 100 chars) -> Ok
    let mut exact_100_unicode = sample_upsert_request();
    exact_100_unicode.name = "Я".repeat(MAX_ROLE_NAME_CHARS);
    assert_eq!(exact_100_unicode.name.chars().count(), 100);
    assert!(validate_upsert_role_request(&exact_100_unicode).is_ok());

    // 100 символов Unicode с внешними пробелами (после trim() ровно 100 chars) -> Ok
    let mut padded_100 = sample_upsert_request();
    padded_100.name = format!("  {}  ", "Ж".repeat(MAX_ROLE_NAME_CHARS));
    assert!(validate_upsert_role_request(&padded_100).is_ok());

    // 101 символ (ASCII и кириллица) -> Err
    for unit in ["a", "Я"] {
        let mut too_long = sample_upsert_request();
        too_long.name = unit.repeat(MAX_ROLE_NAME_CHARS + 1);
        assert_eq!(too_long.name.trim().chars().count(), 101);
        assert!(
            validate_upsert_role_request(&too_long).is_err(),
            "101-char name (unit={unit:?}) must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. validate_upsert_role_request: color (0..=0x00FF_FFFF) и permissions (0..=u64::MAX)
// ---------------------------------------------------------------------------

#[test]
fn validate_upsert_role_request_enforces_rgb_color_bounds() {
    for valid_color in [0, 1, 0x0058_65F2, MAX_RGB_COLOR] {
        let mut req = sample_upsert_request();
        req.color = valid_color;
        assert!(
            validate_upsert_role_request(&req).is_ok(),
            "color=0x{valid_color:08X} must be accepted"
        );
    }

    for invalid_color in [MAX_RGB_COLOR + 1, 0x0100_0000, 0xFF00_0000, u32::MAX] {
        let mut req = sample_upsert_request();
        req.color = invalid_color;
        assert!(
            validate_upsert_role_request(&req).is_err(),
            "color=0x{invalid_color:08X} must be rejected"
        );
    }
}

#[test]
fn validate_upsert_role_request_enforces_canonical_u64_permissions() {
    let max_u64 = u64::MAX.to_string();
    for valid_perms in ["0", "1", "8", "1071698660929", max_u64.as_str()] {
        let mut req = sample_upsert_request();
        req.permissions = valid_perms.to_string();
        assert!(
            validate_upsert_role_request(&req).is_ok(),
            "permissions={valid_perms:?} must be accepted"
        );
    }

    let invalid_perms = [
        "",
        "00",
        "01",
        "08",
        "0123",
        "-1",
        "+8",
        "0x8",
        "abc",
        " 0",
        "0 ",
        "18446744073709551616", // u64::MAX + 1
    ];

    for bad in invalid_perms {
        let mut req = sample_upsert_request();
        req.permissions = bad.to_string();
        assert!(
            validate_upsert_role_request(&req).is_err(),
            "permissions={bad:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. compute_effective_permissions: owner, bitwise OR, PERM_ADMINISTRATOR
// ---------------------------------------------------------------------------

#[test]
fn compute_effective_permissions_handles_owner_admin_and_bitwise_or() {
    // Владелец гильдии всегда получает u64::MAX даже при нулевых правах
    assert_eq!(compute_effective_permissions(0, &[], true), u64::MAX);
    assert_eq!(compute_effective_permissions(1, &[2, 4], true), u64::MAX);

    // Обычный участник без ролей: возвращается everyone_perms
    assert_eq!(compute_effective_permissions(0, &[], false), 0);
    assert_eq!(compute_effective_permissions(0b10100, &[], false), 0b10100);

    // Обычный участник с набором ролей без PERM_ADMINISTRATOR: побитовое ИЛИ
    let everyone = 1 << 0; // CREATE_INSTANT_INVITE
    let roles = [1 << 1, 1 << 6, 1 << 10];
    assert_eq!(
        compute_effective_permissions(everyone, &roles, false),
        (1 << 0) | (1 << 1) | (1 << 6) | (1 << 10)
    );

    // Если PERM_ADMINISTRATOR (1 << 3) присутствует в everyone_perms -> u64::MAX
    assert_eq!(
        compute_effective_permissions(PERM_ADMINISTRATOR, &[], false),
        u64::MAX
    );

    // Если PERM_ADMINISTRATOR присутствует хотя бы в одной роли -> u64::MAX
    assert_eq!(
        compute_effective_permissions(1 << 0, &[1 << 1, PERM_ADMINISTRATOR, 1 << 2], false),
        u64::MAX
    );
}

// ---------------------------------------------------------------------------
// 6. build_member_role_route: канонический маршрут и валидация трёх Snowflake ID
// ---------------------------------------------------------------------------

#[test]
fn build_member_role_route_returns_canonical_path_and_validates_all_three_snowflakes() {
    let route = build_member_role_route(VALID_GUILD_ID, VALID_USER_ID, VALID_ROLE_ID)
        .expect("valid member role route");
    assert_eq!(
        route,
        "/guilds/100000000000000001/members/100000000000000002/roles/100000000000000003"
    );

    let max_u64 = u64::MAX.to_string();
    let boundary_route = build_member_role_route("1", &max_u64, "42")
        .expect("boundary snowflakes member role route");
    assert_eq!(
        boundary_route,
        format!("/guilds/1/members/{max_u64}/roles/42")
    );

    for bad in [
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
            build_member_role_route(bad, VALID_USER_ID, VALID_ROLE_ID).is_err(),
            "guild_id={bad:?} must be rejected"
        );
        assert!(
            build_member_role_route(VALID_GUILD_ID, bad, VALID_ROLE_ID).is_err(),
            "user_id={bad:?} must be rejected"
        );
        assert!(
            build_member_role_route(VALID_GUILD_ID, VALID_USER_ID, bad).is_err(),
            "role_id={bad:?} must be rejected"
        );
    }
}
