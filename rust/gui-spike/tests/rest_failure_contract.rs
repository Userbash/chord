//! Контракт единого классификатора REST-ошибок (G1-01).
//!
//! Оракул — HTTP-семантика: 401/403/404/429/5xx различаются, прочие 4xx —
//! «клиентская ошибка», транспорт — сеть. До G1-01 каждый модуль имел свою
//! копию маппера, и 403/404/5xx превращались в `Network` («сеть недоступна»).

use std::time::Duration;

use chord_core::rest::client::ApiError;
use chord_core::rest::transport::HttpError;
use chord_client::guilds::EnterGuildError;
use chord_client::messages::MessageError;
use chord_client::reactions::ReactionError;
use chord_client::rest_failure::RestFailure;
use chord_client::search::SearchError;

#[test]
fn http_statuses_are_classified_exactly() {
    let cases = [
        (401, RestFailure::Unauthorized),
        (403, RestFailure::Forbidden),
        (404, RestFailure::NotFound),
        (429, RestFailure::RateLimited { retry_after_ms: 0 }),
        (400, RestFailure::ClientOther { status: 400 }),
        (418, RestFailure::ClientOther { status: 418 }),
        (500, RestFailure::Server { status: 500 }),
        (503, RestFailure::Server { status: 503 }),
    ];
    for (status, expected) in cases {
        assert_eq!(
            RestFailure::from_status(status),
            Some(expected),
            "HTTP {status}"
        );
    }
    for status in [200u16, 201, 204, 299] {
        assert_eq!(
            RestFailure::from_status(status),
            None,
            "2xx — не отказ: {status}"
        );
    }
}

#[test]
fn api_error_keeps_status_and_retry_after() {
    let cases = [
        (ApiError::Unauthorized, RestFailure::Unauthorized),
        (
            ApiError::RateLimited {
                retry_after: Duration::from_millis(1500),
                attempts: 2,
            },
            RestFailure::RateLimited {
                retry_after_ms: 1500,
            },
        ),
        (
            ApiError::Server {
                status: 500,
                attempts: 2,
            },
            RestFailure::Server { status: 500 },
        ),
        (ApiError::Client { status: 403 }, RestFailure::Forbidden),
        (ApiError::Client { status: 404 }, RestFailure::NotFound),
        (
            ApiError::Client { status: 400 },
            RestFailure::ClientOther { status: 400 },
        ),
        // Локальные/транспортные отказы без HTTP-статуса остаются сетевыми:
        // контракт `Network` для них не менялся (G1-01 аддитивен).
        (ApiError::Timeout { attempts: 1 }, RestFailure::Network),
        (
            ApiError::Transport(HttpError::Connect("refused".into())),
            RestFailure::Network,
        ),
        (
            ApiError::InvalidRequest("bad target".into()),
            RestFailure::Network,
        ),
    ];
    for (error, expected) in cases {
        assert_eq!(
            RestFailure::from_api_error(&error),
            expected,
            "ApiError {error}"
        );
    }
}

#[test]
fn absurd_retry_after_saturates_instead_of_panicking() {
    let error = ApiError::RateLimited {
        retry_after: Duration::MAX,
        attempts: 1,
    };
    assert_eq!(
        RestFailure::from_api_error(&error),
        RestFailure::RateLimited {
            retry_after_ms: u64::MAX
        }
    );
}

#[test]
fn every_domain_preserves_the_classified_kind() {
    assert_eq!(
        MessageError::from(RestFailure::Forbidden),
        MessageError::Forbidden
    );
    assert_eq!(
        MessageError::from(RestFailure::NotFound),
        MessageError::NotFound
    );
    assert_eq!(
        MessageError::from(RestFailure::Server { status: 500 }),
        MessageError::Server { status: 500 }
    );
    assert_eq!(
        MessageError::from(RestFailure::ClientOther { status: 400 }),
        MessageError::Client { status: 400 }
    );

    assert_eq!(
        ReactionError::from(RestFailure::Forbidden),
        ReactionError::Forbidden
    );
    assert_eq!(
        ReactionError::from(RestFailure::NotFound),
        ReactionError::NotFound
    );
    assert_eq!(
        ReactionError::from(RestFailure::Server { status: 503 }),
        ReactionError::Server { status: 503 }
    );
    assert_eq!(
        ReactionError::from(RestFailure::ClientOther { status: 400 }),
        ReactionError::Client { status: 400 }
    );

    assert_eq!(
        SearchError::from(RestFailure::Forbidden),
        SearchError::Forbidden
    );
    assert_eq!(
        SearchError::from(RestFailure::NotFound),
        SearchError::NotFound
    );
    assert_eq!(
        SearchError::from(RestFailure::Server { status: 500 }),
        SearchError::Server { status: 500 }
    );
    assert_eq!(
        SearchError::from(RestFailure::ClientOther { status: 400 }),
        SearchError::Client { status: 400 }
    );

    assert_eq!(
        EnterGuildError::from(RestFailure::Forbidden),
        EnterGuildError::Forbidden
    );
    assert_eq!(
        EnterGuildError::from(RestFailure::NotFound),
        EnterGuildError::NotFound
    );
    assert_eq!(
        EnterGuildError::from(RestFailure::Server { status: 500 }),
        EnterGuildError::Server { status: 500 }
    );
    assert_eq!(
        EnterGuildError::from(RestFailure::ClientOther { status: 400 }),
        EnterGuildError::Client { status: 400 }
    );
}

#[test]
fn new_kinds_serialize_with_camel_case_and_have_display() {
    let cases = [
        ("forbidden", MessageError::Forbidden),
        ("notFound", MessageError::NotFound),
        ("client", MessageError::Client { status: 400 }),
        ("server", MessageError::Server { status: 503 }),
    ];
    for (kind, error) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }

    let cases = [
        ("forbidden", ReactionError::Forbidden),
        ("notFound", ReactionError::NotFound),
        ("client", ReactionError::Client { status: 400 }),
        ("server", ReactionError::Server { status: 500 }),
    ];
    for (kind, error) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }

    let cases = [
        ("forbidden", SearchError::Forbidden),
        ("notFound", SearchError::NotFound),
        ("client", SearchError::Client { status: 400 }),
        ("server", SearchError::Server { status: 503 }),
    ];
    for (kind, error) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }

    let cases = [
        ("forbidden", EnterGuildError::Forbidden),
        ("notFound", EnterGuildError::NotFound),
        ("client", EnterGuildError::Client { status: 400 }),
        ("server", EnterGuildError::Server { status: 500 }),
    ];
    for (kind, error) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], kind, "{value}");
        assert!(!error.to_string().is_empty(), "Display непустой: {error}");
    }

    // Статус не выбрасывается из сериализованной ошибки.
    let value = serde_json::to_value(MessageError::Server { status: 503 }).expect("json");
    assert_eq!(value["status"], 503, "{value}");
}

#[test]
fn legacy_kinds_are_unchanged() {
    // 401/429/Network/InvalidResponse — существующие kind'ы, их нельзя ломать.
    assert_eq!(
        MessageError::from(RestFailure::Unauthorized),
        MessageError::Unauthorized
    );
    assert_eq!(
        MessageError::from(RestFailure::RateLimited {
            retry_after_ms: 250
        }),
        MessageError::RateLimited {
            retry_after_ms: 250
        }
    );
    assert_eq!(
        SearchError::from(RestFailure::Network),
        SearchError::Network
    );
    assert_eq!(
        SearchError::from(RestFailure::InvalidResponse),
        SearchError::InvalidResponse
    );
    assert_eq!(
        ReactionError::from(RestFailure::RateLimited {
            retry_after_ms: 250
        }),
        ReactionError::RateLimited {
            retry_after_ms: 250
        }
    );
    assert_eq!(
        EnterGuildError::from(RestFailure::Network),
        EnterGuildError::Network
    );
}
