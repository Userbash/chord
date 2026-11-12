//! REST v9: rate-limit и ошибки (T-REST-05/06, волна RW-11 / P1-09).
//!
//! **Провенанс оракулов** (независимые источники, не подгонка под реализацию):
//! - `docs/re/rest-d02.md` §3: единственный 429 в захвате — `GET /users/@me/survey`
//!   (`run2|95974.8271`); коды ответов 200/204/304/404/429; тела и поля ответов в захвате
//!   отсутствуют (§5 — документированный пробел);
//! - RFC 6585 §4: 429 + `Retry-After`; семантика `retry-after` — секунды (целые/дробные);
//! - документированные Discord-поля `x-ratelimit-global`, `x-ratelimit-remaining`,
//!   `x-ratelimit-reset-after`, `x-ratelimit-bucket` (значения в тестах синтетические —
//!   в захвате полей ответов нет, это зафиксировано в ТЗ §1).

use std::time::Duration;

use chord_core::rest::rate_limit::{
    RestFailure, classify_failure, rate_limit_info, retry_delay, should_retry,
};
use chord_core::rest::routes::HttpMethod;

/// Поле ответа в форме провода.
fn header(name: &str, value: &str) -> (String, String) {
    (name.to_string(), value.to_string())
}

/// 429 с одним заголовком `retry-after`.
fn rate_limited(retry_after: &str) -> RestFailure {
    RestFailure::RateLimited(
        rate_limit_info(429, &[header("retry-after", retry_after)], b"").expect("429 → info"),
    )
}

// ---------------------------------------------------------------------------
// 1–5: разбор 429
// ---------------------------------------------------------------------------

#[test]
fn case1_retry_after_header() {
    let headers = [header("Retry-After", "1")];
    let info = rate_limit_info(429, &headers, b"").expect("429");
    assert_eq!(info.retry_after, Duration::from_secs(1));
    assert!(!info.global);
    assert_eq!(info.remaining, None);
    assert_eq!(info.reset_after, None);
    assert_eq!(info.bucket, None);
}

#[test]
fn case2_retry_after_body_and_global() {
    let body = br#"{"message":"You are being rate limited.","retry_after":0.25,"global":true}"#;
    let info = rate_limit_info(429, &[], body).expect("429");
    assert_eq!(info.retry_after, Duration::from_millis(250));
    assert!(info.global);
}

#[test]
fn case3_header_has_priority_over_body() {
    let body = br#"{"retry_after":0.5,"global":false}"#;
    let info = rate_limit_info(429, &[header("retry-after", "2")], body).expect("429");
    assert_eq!(info.retry_after, Duration::from_secs(2));
}

#[test]
fn case4_reset_after_fallback_and_metadata() {
    let headers = [
        header("x-ratelimit-reset-after", "1.5"),
        header("x-ratelimit-remaining", "4"),
        header("x-ratelimit-bucket", "abc"),
    ];
    let info = rate_limit_info(429, &headers, b"").expect("429");
    assert_eq!(info.retry_after, Duration::from_millis(1500));
    assert_eq!(info.reset_after, Some(Duration::from_millis(1500)));
    assert_eq!(info.remaining, Some(4));
    assert_eq!(info.bucket.as_deref(), Some("abc"));
}

#[test]
fn case5_malformed_values_do_not_panic() {
    for value in ["abc", "-1", "NaN", "inf", "1e999"] {
        let info = rate_limit_info(429, &[header("retry-after", value)], b"").expect("429");
        assert_eq!(info.retry_after, Duration::ZERO, "битое значение {value:?}");
    }

    // Ключ внутри строкового значения не должен считаться полем JSON.
    let body = br#"{"message":"retry_after\":99","retry_after":0.75,"global":false}"#;
    let info = rate_limit_info(429, &[], body).expect("429");
    assert_eq!(info.retry_after, Duration::from_millis(750));
    assert!(!info.global);
}

// ---------------------------------------------------------------------------
// 6: классификация статусов
// ---------------------------------------------------------------------------

#[test]
fn case6_status_classification() {
    assert!(rate_limit_info(200, &[], b"").is_none());
    assert!(classify_failure(200, &[], b"").is_none());
    assert!(classify_failure(204, &[], b"").is_none());
    assert!(classify_failure(302, &[], b"").is_none());

    assert_eq!(
        classify_failure(401, &[], b""),
        Some(RestFailure::Unauthorized)
    );
    assert_eq!(
        classify_failure(404, &[], b""),
        Some(RestFailure::Client { status: 404 })
    );
    assert_eq!(
        classify_failure(500, &[], b""),
        Some(RestFailure::Server { status: 500 })
    );
    assert_eq!(
        classify_failure(503, &[], b""),
        Some(RestFailure::Server { status: 503 })
    );

    match classify_failure(429, &[header("retry-after", "3")], b"") {
        Some(RestFailure::RateLimited(info)) => {
            assert_eq!(info.retry_after, Duration::from_secs(3));
        }
        other => panic!("429 обязан стать RateLimited, получено {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 7: задержка повтора
// ---------------------------------------------------------------------------

#[test]
fn case7_retry_delay_429_capped() {
    let failure = rate_limited("3");
    assert_eq!(
        retry_delay(
            5,
            &failure,
            Duration::from_millis(100),
            Duration::from_secs(2)
        ),
        Some(Duration::from_secs(2))
    );
    assert_eq!(
        retry_delay(
            1,
            &rate_limited("0.25"),
            Duration::from_millis(100),
            Duration::from_secs(2)
        ),
        Some(Duration::from_millis(250))
    );
}

#[test]
fn case7_retry_delay_5xx_exponential() {
    let failure = RestFailure::Server { status: 503 };
    let base = Duration::from_millis(100);
    let max = Duration::from_secs(10);
    assert_eq!(
        retry_delay(1, &failure, base, max),
        Some(Duration::from_millis(100))
    );
    assert_eq!(
        retry_delay(2, &failure, base, max),
        Some(Duration::from_millis(200))
    );
    assert_eq!(
        retry_delay(3, &failure, base, max),
        Some(Duration::from_millis(400))
    );
    assert_eq!(
        retry_delay(u32::MAX, &failure, base, max),
        Some(max),
        "переполнение обязано насыщаться в max"
    );
    assert_eq!(
        retry_delay(1, &failure, base, Duration::ZERO),
        Some(Duration::ZERO)
    );
}

#[test]
fn case7_retry_delay_no_retry_for_auth_and_client() {
    let base = Duration::from_millis(100);
    let max = Duration::from_secs(1);
    assert_eq!(retry_delay(1, &RestFailure::Unauthorized, base, max), None);
    assert_eq!(
        retry_delay(1, &RestFailure::Client { status: 404 }, base, max),
        None
    );
}

// ---------------------------------------------------------------------------
// 8: политика повторов по методам
// ---------------------------------------------------------------------------

#[test]
fn case8_should_retry_by_method() {
    let limited = rate_limited("1");
    let server = RestFailure::Server { status: 500 };

    for method in [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
    ] {
        assert!(
            should_retry(method, &limited),
            "429 повторяем для {method:?}"
        );
    }

    for method in [HttpMethod::Get, HttpMethod::Put, HttpMethod::Delete] {
        assert!(
            should_retry(method, &server),
            "5xx повторяем для {method:?}"
        );
    }
    for method in [HttpMethod::Post, HttpMethod::Patch] {
        assert!(
            !should_retry(method, &server),
            "5xx не повторяем для {method:?}"
        );
    }

    assert!(!should_retry(HttpMethod::Get, &RestFailure::Unauthorized));
    assert!(!should_retry(
        HttpMethod::Get,
        &RestFailure::Client { status: 404 }
    ));
}
