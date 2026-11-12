//! REST v9: ack/read-state — target и тело `{token,last_viewed}` (T-REST-07, волна RW-11 /
//! P1-12, воркер W11c).
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - `docs/re/rest-d02.md` §4: `POST /channels/{id}/messages/{id}/ack` ×5, тело
//!   `{"token":null,"last_viewed":4310}` (requestId `ready|57992.7577`); ключи верхнего уровня
//!   `token`, `last_viewed`; `token` бывает `null`, `last_viewed` — номер сообщения (JSON-число).
//! - каталог `src/rest/routes.rs`: маршрут `POST /channels/{id}/messages/{id}/ack` с ключами
//!   тела `["token", "last_viewed"]` — путь и порядок ключей совпадают с оракулом.
//! - RFC 8259 §7: строка JSON экранирует `"` → `\"`, `\` → `\\`, управляющие U+0000…U+001F →
//!   `\u00XX` (шестнадцатеричные цифры в нижнем регистре); прочие символы UTF-8 — как есть.
//!
//! Кейсы 1–6 — `waves/RW-11/TZ.md` §3. Тесты падают до реализации (`todo!()`), green — после.

use chord_core::rest::ack::{ACK_PATH_TEMPLATE, AckError, build_ack_body, build_ack_target};

/// Точное тело из оракула D-02 §4 (`ready|57992.7577`) — порядок ключей `token`, `last_viewed`.
const ORACLE_BODY_4310: &str = r#"{"token":null,"last_viewed":4310}"#;

// ---------------------------------------------------------------------------
// Кейс 1: target
// ---------------------------------------------------------------------------

/// Кейс 1: путь ack — шаблон каталога D-02 с подставленными снежинками.
#[test]
fn case1_target_matches_oracle_path() {
    assert_eq!(
        ACK_PATH_TEMPLATE, "/channels/{id}/messages/{id}/ack",
        "шаблон пути — дословно из оракула D-02"
    );
    assert_eq!(
        build_ack_target("123", "456"),
        Ok("/channels/123/messages/456/ack".to_string()),
        "канал и сообщение подставляются в свои плейсхолдеры"
    );
    assert_eq!(
        build_ack_target("1234567890123456789", "987654321098765432"),
        Ok("/channels/1234567890123456789/messages/987654321098765432/ack".to_string()),
        "реальные снежинки (19 и 18 цифр) подставляются без изменений"
    );
}

// ---------------------------------------------------------------------------
// Кейс 2: пустые и нечисловые id
// ---------------------------------------------------------------------------

/// Кейс 2: пустой id канала или сообщения → `EmptyId` (проверка канала идёт первой).
#[test]
fn case2_empty_ids() {
    assert_eq!(
        build_ack_target("", "456"),
        Err(AckError::EmptyId),
        "пустой channel_id"
    );
    assert_eq!(
        build_ack_target("123", ""),
        Err(AckError::EmptyId),
        "пустой message_id"
    );
    assert_eq!(
        build_ack_target("", ""),
        Err(AckError::EmptyId),
        "оба id пустые"
    );
    assert_eq!(
        build_ack_target("", "12a"),
        Err(AckError::EmptyId),
        "EmptyId канала важнее NonNumericId сообщения (порядок проверок)"
    );
}

/// Кейс 2: не-ASCII-цифры в любом id → `NonNumericId` (защита от подстановки в путь).
#[test]
fn case2_non_numeric_ids() {
    // `../x` — попытка выйти из сегмента пути; `+1`/` 1` — не снежинки; `\n` — control;
    // арабские и полноширинные цифры — Unicode-цифры, но не ASCII (контракт: ASCII-цифры).
    let bad_ids = [
        "12a",
        "../x",
        "+1",
        " 1",
        "1 ",
        "-1",
        "0x10",
        "1.0",
        "12\n",
        "١٢٣",
        "１２３",
    ];
    for bad in bad_ids {
        assert_eq!(
            build_ack_target(bad, "456"),
            Err(AckError::NonNumericId),
            "нечисловой channel_id {bad:?}"
        );
        assert_eq!(
            build_ack_target("123", bad),
            Err(AckError::NonNumericId),
            "нечисловой message_id {bad:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Кейс 3: тело без токена — точная строка оракула
// ---------------------------------------------------------------------------

/// Кейс 3: `token: null`, `last_viewed` — число без кавычек, ключи в порядке оракула.
#[test]
fn case3_body_without_token_is_oracle_exact() {
    assert_eq!(
        build_ack_body(None, 4310),
        ORACLE_BODY_4310,
        "тело — дословно `{{\"token\":null,\"last_viewed\":4310}}`"
    );
    assert_eq!(
        build_ack_body(None, 0),
        r#"{"token":null,"last_viewed":0}"#,
        "нулевой last_viewed остаётся числом (не строкой и не null)"
    );
}

// ---------------------------------------------------------------------------
// Кейс 4: тело с токеном
// ---------------------------------------------------------------------------

/// Кейс 4: строковый токен — в кавычках, `last_viewed` — число.
#[test]
fn case4_body_with_token() {
    assert_eq!(
        build_ack_body(Some("abc"), 1),
        r#"{"token":"abc","last_viewed":1}"#,
        "токен `abc` без изменений"
    );
    assert_eq!(
        build_ack_body(Some(""), 1),
        r#"{"token":"","last_viewed":1}"#,
        "пустой токен — валидная JSON-строка"
    );
}

// ---------------------------------------------------------------------------
// Кейс 5: экранирование
// ---------------------------------------------------------------------------

/// Кейс 5: `"` и `\` экранируются, `\n` — как `\u000a` (нижний регистр), не `\n`.
#[test]
fn case5_quote_backslash_and_newline() {
    assert_eq!(
        build_ack_body(Some("a\"b\\c\n"), 1),
        r#"{"token":"a\"b\\c\u000a","last_viewed":1}"#,
        "кавычка, обратный слэш и перевод строки"
    );
}

/// Кейс 5: все управляющие U+0000…U+001F → `\u00XX`, hex в нижнем регистре.
#[test]
fn case5_all_controls_are_lowercase_u00xx() {
    // Вход — ровно 32 control-символа по порядку кодов.
    let input: String = (0u8..=0x1f).map(char::from).collect();

    // Оракул ожидаемой строки: `\u0000`…`\u001f` (нижний регистр — требование ТЗ §3 п.5).
    let mut escaped = String::new();
    for code in 0u8..=0x1f {
        escaped.push_str(&format!("\\u{code:04x}"));
    }
    let expected = format!("{{\"token\":\"{escaped}\",\"last_viewed\":1}}");

    let actual = build_ack_body(Some(&input), 1);
    assert_eq!(actual, expected, "все 32 control-символа → \\u00XX");

    // Явная проверка регистра: `\u000A` (верхний) недопустим.
    assert!(
        !actual.contains("\\u000A"),
        "hex-цифры должны быть в нижнем регистре: {actual:?}"
    );
    // В теле нет ни одного сырого управляющего символа.
    assert!(
        actual.chars().all(|ch| !ch.is_control()),
        "control-символы обязаны быть экранированы: {actual:?}"
    );
}

/// Кейс 5: UTF-8 (кириллица, эмодзи) проходит без изменений и без `\u`-экранирования.
#[test]
fn case5_utf8_passes_unchanged() {
    assert_eq!(
        build_ack_body(Some("привет"), 1),
        r#"{"token":"привет","last_viewed":1}"#,
        "кириллица — как есть"
    );
    assert_eq!(
        build_ack_body(Some("😀"), 1),
        r#"{"token":"😀","last_viewed":1}"#,
        "не-BMP символ — как есть (UTF-8 в UTF-8)"
    );
}

// ---------------------------------------------------------------------------
// Кейс 6: u64::MAX
// ---------------------------------------------------------------------------

/// Кейс 6: `last_viewed = u64::MAX` печатается без потерь (десятичное, без кавычек).
#[test]
fn case6_u64_max_has_no_loss() {
    assert_eq!(
        build_ack_body(None, u64::MAX),
        r#"{"token":null,"last_viewed":18446744073709551615}"#,
        "u64::MAX = 18446744073709551615"
    );
    assert_eq!(
        build_ack_body(Some("t"), u64::MAX),
        r#"{"token":"t","last_viewed":18446744073709551615}"#,
        "u64::MAX с токеном"
    );
}
