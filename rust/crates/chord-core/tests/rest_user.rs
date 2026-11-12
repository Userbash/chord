//! `GET /users/@me`: тип `Me` и строгий разбор тела ответа (RW-39 / NET-C1, P0-4).
//!
//! **Провенанс**: `docs/re/rest-d02.md` §2.1 (маршрут `GET /users/@me`, 3 запроса),
//! `docs/re/rest-d02.json` (значения `Authorization` санитизированы), схема user-объекта —
//! `docs/re/gateway-schema.md` (READY `user`) и `docs/re/rest-d02.md` §6.2.
//! Тело 401 — форма ошибки Discord (`{"message": "401: Unauthorized", "code": 0}`).
//!
//! Сэмпл санитизирован: идентификаторы и ники не из живого аккаунта.

use chord_core::rest::user::{Me, MeError, parse_me};

/// Санитизированный ответ `GET /users/@me` (200): все поля, которые нужны клиенту.
const ME_JSON: &str = concat!(
    r#"{"id":"123456789012345678","username":"discord_lab","discriminator":"0","#,
    r#""global_name":"Discord Lab","avatar":"a_0123456789abcdef0123456789abcdef","#,
    r#""bot":false,"system":false,"mfa_enabled":true,"locale":"ru","verified":true}"#
);

#[test]
fn parse_me_maps_required_fields() {
    let me = parse_me(ME_JSON.as_bytes()).expect("валидный /users/@me");

    assert_eq!(me.id, "123456789012345678");
    assert_eq!(me.username, "discord_lab");
    assert_eq!(me.discriminator, "0");
    assert_eq!(me.global_name.as_deref(), Some("Discord Lab"));
    assert_eq!(
        me.avatar.as_deref(),
        Some("a_0123456789abcdef0123456789abcdef")
    );
    assert!(!me.bot);
}

#[test]
fn parse_me_accepts_nullable_and_missing_optional_fields() {
    let json =
        r#"{"id":"42","username":"user","discriminator":"1234","global_name":null,"avatar":null}"#;
    let me = parse_me(json.as_bytes()).expect("nullable поля допустимы");

    assert_eq!(me.id, "42");
    assert_eq!(me.global_name, None);
    assert_eq!(me.avatar, None);
    assert!(!me.bot, "отсутствующий bot — не бот, а не ошибка");
}

#[test]
fn parse_me_reads_bot_flag() {
    let json = r#"{"id":"7","username":"webhook","discriminator":"0000","bot":true}"#;
    let me = parse_me(json.as_bytes()).expect("бот");
    assert!(me.bot);
}

#[test]
fn parse_me_rejects_broken_json() {
    let error = parse_me(br#"{"id":"1","username":"#).expect_err("битый JSON");
    assert!(
        matches!(error, MeError::Json(_)),
        "ожидался Json, получено {error:?}"
    );
}

#[test]
fn parse_me_rejects_401_body_as_unauthorized() {
    let body = br#"{"message": "401: Unauthorized", "code": 0}"#;
    let error = parse_me(body).expect_err("тело 401 — не пользователь");
    assert_eq!(error, MeError::Unauthorized);
}

#[test]
fn parse_me_keeps_other_api_errors_typed() {
    let body = br#"{"message": "You are being rate limited.", "code": 0, "retry_after": 1.5}"#;
    let error = parse_me(body).expect_err("тело ошибки API");
    assert!(
        matches!(&error, MeError::Discord { message, .. } if message.as_str() == "You are being rate limited."),
        "ожидался Discord, получено {error:?}"
    );
}

#[test]
fn parse_me_rejects_non_object_and_missing_fields() {
    assert_eq!(
        parse_me(b"[]").expect_err("массив"),
        MeError::NotObject,
        "корень — не объект"
    );
    assert_eq!(
        parse_me(br#"{"id":"1","username":"u"}"#).expect_err("нет discriminator"),
        MeError::MissingField("discriminator")
    );
}

#[test]
fn parse_me_rejects_wrong_field_types() {
    let json = r#"{"id":123,"username":"u","discriminator":"0"}"#;
    assert_eq!(
        parse_me(json.as_bytes()).expect_err("id обязан быть строкой"),
        MeError::InvalidField("id")
    );
}

#[test]
fn parse_me_rejects_non_utf8() {
    assert_eq!(
        parse_me(&[0xff, 0xfe]).expect_err("не UTF-8"),
        MeError::NotUtf8
    );
}

#[test]
fn me_debug_does_not_hide_public_fields() {
    let me = Me {
        id: "42".to_string(),
        username: "user".to_string(),
        discriminator: "0".to_string(),
        global_name: None,
        avatar: None,
        bot: false,
    };
    assert!(format!("{me:?}").contains("user"));
}
