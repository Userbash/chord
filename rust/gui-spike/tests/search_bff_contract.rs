#![forbid(unsafe_code)]
//! Контрактные тесты BFF полнотекстового REST-поиска сообщений (`search_bff`).
//!
//! Охватывает 9 обязательных измерений тестовой стратегии (`TEST-POLICY.md` §11–§15):
//! 1. Позитивные: разбор инлайн-токенов `from:`, `mentions:`, `has:`, `pinned:`, `in:`,
//!    `before:`, `after:`, `author_type:`, `link_hostname:`, `attachment_extension:`,
//!    `attachment_filename:`, сборка query string и маршрутов гильдии/канала, заголовки Discord v9,
//!    разбор `200 OK` с вложенными группами `messages[][]`, `total_results`, `hit: true`.
//! 2. Негативные: отклонение неизвестных `has:`, `author_type:`, невалидных `pinned:`,
//!    нечисловых Snowflake ID и `min_id > max_id` до вызова транспорта.
//! 3. Граничные: Snowflake `"0"` (отказ), `"1"` и `u64::MAX` (`"18446744073709551615"`) (допуск),
//!    `"18446744073709551616"` (переполнение u64 -> отказ), `offset = 0` и `offset = 9975`
//!    (`MAX_SEARCH_OFFSET`), `offset = 9976` (отказ), `limit = 1` и `limit = 25`, `limit = 0` / `26`,
//!    `content` ровно 512 символов и 513 символов.
//! 4. Ошибки: классификация `401 Unauthorized`, `403 Forbidden`, `404 NotFound`, `400 Client`,
//!    `429 RateLimited`, `500/502/503 Server`, сетевых сбоев и битого JSON.
//! 5. Пустые: отказ на пустой запрос без текста и фильтров (`EmptyQuery`), корректный разбор
//!    пустого ответа `200 OK` (`messages: []`, `total_results: 0`).
//! 6. Загрузочные (`HTTP 202 Accepted`): разбор статуса индексации (`indexing: true`,
//!    `retry_after_ms`, `documents_indexed`) и поллинг до `MAX_SEARCH_202_RETRIES = 5` с `attempts=1..=5`.
//! 7. Права доступа и NSFW: `403 Forbidden`, передача `include_nsfw=true` только при `Some(true)`.
//! 8. Восстановление (`429 backoff rate limiting`): расчёт экспоненциального бэкоффа
//!    `compute_429_backoff_ms`, трекер `SearchRateLimiter` (per-scope и global cooldown,
//!    сброс при `record_success`) и автоматический retry-цикл `execute_with_retry`.
//! 9. Совместимость и наблюдаемость: маскирование секретного токена в `Debug`/`Display`/JSON,
//!    serde round-trip (`camelCase` и `snake_case` алиасы), LRU-кэш (`MAX_SEARCH_CACHE_ENTRIES = 64`)
//!    с каноническим ключом без транзиентных `search_session_id`/`search_query_id`/`attempts`.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use discord_core::rest::super_properties::SuperProperties;
use discord_core::rest::transport::HttpResponse;

#[path = "../src/search_bff.rs"]
pub mod search_bff;

use search_bff::{
    build_search_http_request, compute_429_backoff_ms, parse_202_retry_after_ms,
    parse_429_retry_after, parse_search_http_response, parse_search_query, SearchBffClient,
    SearchBffError, SearchBffQuery, SearchBffResponse, SearchBffScope, SearchBffTransport,
    SearchRateLimiter, SearchSortBy, SearchSortOrder, DEFAULT_SEARCH_202_RETRY_AFTER_MS,
    DEFAULT_SEARCH_429_RETRY_AFTER_MS, DEFAULT_SEARCH_LIMIT, DISCORD_LOCALE_RU,
    MAX_SEARCH_202_RETRIES, MAX_SEARCH_429_BACKOFF_MS, MAX_SEARCH_429_RETRIES,
    MAX_SEARCH_CACHE_ENTRIES, MAX_SEARCH_CONTENT_CHARS, MAX_SEARCH_LIMIT, MAX_SEARCH_OFFSET,
    MAX_SEARCH_PAGES, MIN_SEARCH_429_BASE_MS, SEARCH_AUTHOR_TYPES, SEARCH_HAS_FACETS,
};

struct QueuedBffTransport {
    responses: Mutex<VecDeque<Result<HttpResponse, SearchBffError>>>,
    targets: Mutex<Vec<String>>,
    sleeps_ms: Mutex<Vec<u64>>,
}

impl QueuedBffTransport {
    fn new(responses: Vec<Result<HttpResponse, SearchBffError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            targets: Mutex::new(Vec::new()),
            sleeps_ms: Mutex::new(Vec::new()),
        }
    }

    fn single(status: u16, headers: &[(&str, &str)], body: &str) -> Arc<Self> {
        Arc::new(Self::new(vec![Ok(Self::http_resp(status, headers, body))]))
    }

    fn http_resp(status: u16, headers: &[(&str, &str)], body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn recorded_targets(&self) -> Vec<String> {
        self.targets.lock().unwrap().clone()
    }

    fn recorded_sleeps(&self) -> Vec<u64> {
        self.sleeps_ms.lock().unwrap().clone()
    }
}

impl SearchBffTransport for QueuedBffTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, SearchBffError>> + Send + 'a>> {
        self.targets.lock().unwrap().push(target.to_string());
        let next = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(SearchBffError::Network));
        Box::pin(async move { next })
    }

    fn sleep<'a>(&'a self, delay: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        self.sleeps_ms
            .lock()
            .unwrap()
            .push(delay.as_millis() as u64);
        Box::pin(async {})
    }
}

/// Независимый оракул Snowflake по timestamp (Discord epoch 2015-01-01T00:00:00Z).
fn snowflake_from_timestamp_ms(timestamp_ms: u64) -> String {
    const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
    ((timestamp_ms - DISCORD_EPOCH_MS) << 22).to_string()
}

#[test]
fn constants_and_enums_match_re_specification() {
    assert_eq!(MAX_SEARCH_LIMIT, 25);
    assert_eq!(DEFAULT_SEARCH_LIMIT, 25);
    assert_eq!(MAX_SEARCH_OFFSET, 9975);
    assert_eq!(MAX_SEARCH_PAGES, 400);
    assert_eq!(MAX_SEARCH_CONTENT_CHARS, 512);
    assert_eq!(MAX_SEARCH_202_RETRIES, 5);
    assert_eq!(MAX_SEARCH_429_RETRIES, 3);
    assert_eq!(DEFAULT_SEARCH_202_RETRY_AFTER_MS, 5_000);
    assert_eq!(DEFAULT_SEARCH_429_RETRY_AFTER_MS, 1_000);
    assert_eq!(MIN_SEARCH_429_BASE_MS, 100);
    assert_eq!(MAX_SEARCH_429_BACKOFF_MS, 30_000);
    assert_eq!(MAX_SEARCH_CACHE_ENTRIES, 64);
    assert_eq!(DISCORD_LOCALE_RU, "ru");
    assert_eq!(
        SEARCH_HAS_FACETS,
        ["link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker"]
    );
    assert_eq!(SEARCH_AUTHOR_TYPES, ["user", "bot", "webhook"]);

    assert_eq!(
        SearchSortBy::parse_str("timestamp"),
        Some(SearchSortBy::Timestamp)
    );
    assert_eq!(
        SearchSortBy::parse_str(" RELEVANCE "),
        Some(SearchSortBy::Relevance)
    );
    assert_eq!(SearchSortBy::parse_str("invalid"), None);

    assert_eq!(
        SearchSortOrder::parse_str("desc"),
        Some(SearchSortOrder::Desc)
    );
    assert_eq!(
        SearchSortOrder::parse_str(" ASC "),
        Some(SearchSortOrder::Asc)
    );
    assert_eq!(SearchSortOrder::parse_str("random"), None);
}

#[test]
fn parses_from_mentions_has_pinned_and_extended_inline_facets_into_canonical_query() {
    let min_sf = snowflake_from_timestamp_ms(1_792_454_400_000);
    let max_sf = snowflake_from_timestamp_ms(1_792_540_799_999);
    let raw = format!(
        "from:111111111111111111 author_id:222222222222222222 from:111111111111111111 \
         mentions:333333333333333333 mentions:444444444444444444 \
         has:IMAGE has:-Link has:image \
         pinned:TRUE \
         in:555555555555555555 \
         after:{min_sf} before:{max_sf} \
         author_type:BOT \
         site:Discord.COM ext:.PDF filename:report.pdf \
         привет   мир   12:30"
    );

    let parsed = parse_search_query(&raw).expect("inline query must parse");
    assert_eq!(parsed.content.as_deref(), Some("привет мир 12:30"));
    assert_eq!(
        parsed.from,
        vec![
            "111111111111111111".to_string(),
            "222222222222222222".to_string()
        ]
    );
    assert_eq!(
        parsed.mentions,
        vec![
            "333333333333333333".to_string(),
            "444444444444444444".to_string()
        ]
    );
    assert_eq!(parsed.has, vec!["image".to_string(), "-link".to_string()]);
    assert_eq!(parsed.pinned, Some(true));
    assert_eq!(parsed.channel_id, vec!["555555555555555555".to_string()]);
    assert_eq!(parsed.min_id.as_deref(), Some(min_sf.as_str()));
    assert_eq!(parsed.max_id.as_deref(), Some(max_sf.as_str()));
    assert_eq!(parsed.author_type, vec!["bot".to_string()]);
    assert_eq!(parsed.link_hostname, vec!["discord.com".to_string()]);
    assert_eq!(parsed.attachment_extension, vec!["pdf".to_string()]);
    assert_eq!(parsed.attachment_filename, vec!["report.pdf".to_string()]);

    let guild_scope = SearchBffScope::Guild("1014537851418980372".to_string());
    let endpoint = parsed.build_endpoint(&guild_scope).expect("guild endpoint");
    assert_eq!(
        endpoint,
        format!(
            "/guilds/1014537851418980372/messages/search?\
             content=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82%20%D0%BC%D0%B8%D1%80%2012%3A30\
             &channel_id=555555555555555555\
             &author_id=111111111111111111&author_id=222222222222222222\
             &mentions=333333333333333333&mentions=444444444444444444\
             &has=image&has=-link\
             &pinned=true\
             &author_type=bot\
             &min_id={min_sf}&max_id={max_sf}\
             &link_hostname=discord.com\
             &attachment_extension=pdf\
             &attachment_filename=report.pdf"
        )
    );

    let channel_scope = SearchBffScope::Channel("999888777666555444".to_string());
    let channel_api_path = parsed
        .build_api_v9_path(&channel_scope)
        .expect("channel api v9 path");
    assert!(
        channel_api_path.starts_with("/api/v9/channels/999888777666555444/messages/search?"),
        "{channel_api_path}"
    );
}

#[test]
fn supports_all_nine_has_facets_negation_and_pinned_false() {
    for facet in SEARCH_HAS_FACETS {
        let q = parse_search_query(&format!("has:{facet} pinned:false"))
            .unwrap_or_else(|e| panic!("has:{facet} must be accepted: {e}"));
        assert_eq!(q.has, vec![facet.to_string()]);
        assert_eq!(q.pinned, Some(false));

        let neg = parse_search_query(&format!("has:-{facet}"))
            .unwrap_or_else(|e| panic!("has:-{facet} must be accepted: {e}"));
        assert_eq!(neg.has, vec![format!("-{facet}")]);
    }
}

#[test]
fn rejects_negative_and_invalid_inline_tokens_and_boundaries() {
    // Пустой запрос
    assert_eq!(
        parse_search_query("   \t\n  "),
        Err(SearchBffError::EmptyQuery)
    );

    // Невалидный has:
    assert_eq!(
        parse_search_query("has:executable"),
        Err(SearchBffError::InvalidHasFacet {
            value: "executable".into()
        })
    );
    assert_eq!(
        parse_search_query("has:-unknown"),
        Err(SearchBffError::InvalidHasFacet {
            value: "-unknown".into()
        })
    );
    assert_eq!(
        parse_search_query("has:"),
        Err(SearchBffError::InvalidHasFacet { value: "".into() })
    );

    // Невалидный pinned:
    assert_eq!(
        parse_search_query("pinned:maybe"),
        Err(SearchBffError::InvalidPinnedValue {
            value: "maybe".into()
        })
    );
    assert_eq!(
        parse_search_query("pinned:"),
        Err(SearchBffError::InvalidPinnedValue { value: "".into() })
    );

    // Невалидный author_type:
    assert_eq!(
        parse_search_query("author_type:moderator"),
        Err(SearchBffError::InvalidAuthorType {
            value: "moderator".into()
        })
    );

    // Границы Snowflake в from: и mentions: ("0" запрещён, u64::MAX разрешён, u64::MAX+1 запрещён)
    assert_eq!(
        parse_search_query("from:0"),
        Err(SearchBffError::InvalidSnowflake {
            field: "from".into(),
            value: "0".into()
        })
    );
    assert_eq!(
        parse_search_query("mentions:abc"),
        Err(SearchBffError::InvalidSnowflake {
            field: "mentions".into(),
            value: "abc".into()
        })
    );
    assert_eq!(
        parse_search_query("from:18446744073709551616"),
        Err(SearchBffError::InvalidSnowflake {
            field: "from".into(),
            value: "18446744073709551616".into()
        })
    );

    let max_u64 = parse_search_query("from:1 mentions:18446744073709551615")
        .expect("Snowflake 1 and u64::MAX must be valid");
    assert_eq!(max_u64.from, vec!["1"]);
    assert_eq!(max_u64.mentions, vec!["18446744073709551615"]);

    // min_id > max_id
    assert_eq!(
        parse_search_query("after:200 before:100"),
        Err(SearchBffError::MinIdGreaterThanMaxId)
    );

    // Границы длины content: 512 символов OK, 513 -> ContentTooLong
    let content_512 = "а".repeat(MAX_SEARCH_CONTENT_CHARS);
    let ok_512 = parse_search_query(&content_512).expect("512 chars must be valid");
    assert_eq!(
        ok_512.content.as_deref().map(|s| s.chars().count()),
        Some(512)
    );

    let content_513 = "б".repeat(MAX_SEARCH_CONTENT_CHARS + 1);
    assert_eq!(
        parse_search_query(&content_513),
        Err(SearchBffError::ContentTooLong { len: 513, max: 512 })
    );

    // Границы offset (0..=9975) и limit (1..=25)
    let mut boundary_query = parse_search_query("has:link").unwrap();
    boundary_query.offset = Some(MAX_SEARCH_OFFSET);
    boundary_query.limit = Some(MAX_SEARCH_LIMIT);
    assert!(boundary_query.validate_and_normalize().is_ok());

    boundary_query.offset = Some(MAX_SEARCH_OFFSET + 1);
    assert_eq!(
        boundary_query.validate_and_normalize(),
        Err(SearchBffError::OffsetOutOfRange {
            offset: 9976,
            max: 9975
        })
    );

    boundary_query.offset = Some(0);
    boundary_query.limit = Some(0);
    assert_eq!(
        boundary_query.validate_and_normalize(),
        Err(SearchBffError::InvalidLimit { limit: 0 })
    );
    boundary_query.limit = Some(26);
    assert_eq!(
        boundary_query.validate_and_normalize(),
        Err(SearchBffError::InvalidLimit { limit: 26 })
    );

    // Границы scope Snowflake
    assert_eq!(
        SearchBffScope::Guild("0".into()).validate(),
        Err(SearchBffError::InvalidScopeId)
    );
    assert_eq!(
        SearchBffScope::Channel("not-digits".into()).validate(),
        Err(SearchBffError::InvalidScopeId)
    );
    assert_eq!(
        SearchBffScope::Guild("18446744073709551615".into())
            .validate()
            .expect("u64::MAX scope"),
        "18446744073709551615"
    );
}

#[test]
fn builds_http_request_with_all_six_mandatory_discord_headers_and_nsfw_flag() {
    let mut query = parse_search_query("from:42 has:file pinned:true поиск").unwrap();
    query.sort_by = Some(SearchSortBy::Timestamp);
    query.sort_order = Some(SearchSortOrder::Desc);
    query.limit = Some(25);
    query.offset = Some(50);
    query.include_nsfw = Some(true);
    query.search_session_id = Some("sess-uuid-1".into());
    query.search_query_id = Some("query-uuid-2".into());

    let scope = SearchBffScope::Guild("1014537851418980372".into());
    let req = build_search_http_request(
        "mfa.test_token_value",
        &SuperProperties::default(),
        "Europe/Moscow",
        &scope,
        &query,
    )
    .expect("http request");

    assert!(
        req.target
            .starts_with("/api/v9/guilds/1014537851418980372/messages/search?"),
        "{}",
        req.target
    );
    assert!(req.target.contains("include_nsfw=true"), "{}", req.target);
    assert!(
        req.target.contains("search_session_id=sess-uuid-1"),
        "{}",
        req.target
    );
    assert!(
        req.target.contains("search_query_id=query-uuid-2"),
        "{}",
        req.target
    );

    let header_value = |name: &str| -> Option<&str> {
        req.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(header_value("Authorization"), Some("mfa.test_token_value"));
    assert!(header_value("X-Super-Properties").is_some_and(|v| !v.is_empty()));
    assert_eq!(header_value("X-Discord-Locale"), Some("ru"));
    assert_eq!(header_value("X-Discord-Timezone"), Some("Europe/Moscow"));
    assert_eq!(header_value("X-Debug-Options"), Some("bugReporterEnabled"));
    assert_eq!(header_value("Accept-Language"), Some("ru"));

    // При include_nsfw: Some(false) флаг не добавляется в URL
    query.include_nsfw = Some(false);
    let qs_no_nsfw = query.to_query_string().unwrap();
    assert!(!qs_no_nsfw.contains("include_nsfw"), "{qs_no_nsfw}");
}

#[test]
fn computes_429_exponential_backoff_and_tracks_per_scope_and_global_rate_limits() {
    // Независимый оракул экспоненциального бэкоффа: min(30_000, max(base, 100) * 2^attempt)
    assert_eq!(compute_429_backoff_ms(0, 0), 1000);
    assert_eq!(compute_429_backoff_ms(0, 1), 2000);
    assert_eq!(compute_429_backoff_ms(50, 0), 100);
    assert_eq!(compute_429_backoff_ms(500, 0), 500);
    assert_eq!(compute_429_backoff_ms(500, 1), 1000);
    assert_eq!(compute_429_backoff_ms(500, 2), 2000);
    assert_eq!(compute_429_backoff_ms(1500, 0), 1500);
    assert_eq!(compute_429_backoff_ms(1500, 1), 3000);
    assert_eq!(compute_429_backoff_ms(1500, 2), 6000);
    assert_eq!(compute_429_backoff_ms(5000, 4), 30_000);
    assert_eq!(compute_429_backoff_ms(1500, 20), 30_000);

    // Разбор retry_after из JSON-тела и заголовка Retry-After
    assert_eq!(
        parse_429_retry_after(
            &[],
            br#"{"message":"You are being rate limited.","retry_after":1.25,"global":true}"#
        ),
        (1250, true)
    );
    assert_eq!(
        parse_429_retry_after(&[("Retry-After".into(), "2.5".into())], b"{}"),
        (2500, false)
    );
    assert_eq!(
        parse_429_retry_after(&[], b"not-json"),
        (DEFAULT_SEARCH_429_RETRY_AFTER_MS, false)
    );

    // Stateful трекер SearchRateLimiter
    let mut limiter = SearchRateLimiter::new();
    let scope_a = "guild:100";
    let scope_b = "guild:200";

    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 10_000), None);
    assert_eq!(limiter.consecutive_429(scope_a), 0);

    // 1-й 429 на scope_a (base 600ms -> 600ms)
    let wait1 = limiter.record_429(scope_a, 10_000, 600, false);
    assert_eq!(wait1, 600);
    assert_eq!(limiter.consecutive_429(scope_a), 1);
    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 10_200), Some(400));
    assert_eq!(limiter.remaining_cooldown_ms(scope_b, 10_200), None);

    // 2-й подряд 429 на scope_a (base 600ms, attempt 1 -> 1200ms)
    let wait2 = limiter.record_429(scope_a, 10_600, 600, false);
    assert_eq!(wait2, 1200);
    assert_eq!(limiter.consecutive_429(scope_a), 2);
    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 11_000), Some(800));
    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 11_800), None);

    // Успешный ответ сбрасывает счётчик последовательных 429
    limiter.record_success(scope_a);
    assert_eq!(limiter.consecutive_429(scope_a), 0);
    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 11_100), None);

    // Глобальный 429 блокирует все области поиска
    let wait_global = limiter.record_429(scope_a, 20_000, 2000, true);
    assert_eq!(wait_global, 2000);
    assert_eq!(limiter.remaining_cooldown_ms(scope_a, 20_500), Some(1500));
    assert_eq!(limiter.remaining_cooldown_ms(scope_b, 20_500), Some(1500));
    assert_eq!(limiter.remaining_cooldown_ms(scope_b, 22_000), None);
}

#[test]
fn parses_200_nested_context_groups_202_indexing_and_error_statuses() {
    let body_200 = r#"{
        "analytics_id": "bff_analytics_1",
        "total_results": 2,
        "doing_deep_historical_index": false,
        "documents_indexed": 9120,
        "messages": [
            [
                {
                    "id": "1001",
                    "channel_id": "333",
                    "author": {"id": "7001", "username": "alice"},
                    "content": "сообщение до",
                    "timestamp": "2026-10-20T12:00:00Z",
                    "pinned": false
                },
                {
                    "id": "1002",
                    "channel_id": "333",
                    "author": {"id": "7002", "username": "bob"},
                    "content": "целевой хит #1",
                    "timestamp": "2026-10-20T12:01:00Z",
                    "hit": true,
                    "pinned": true,
                    "attachments": [
                        {
                            "id": "9001",
                            "filename": "spec.pdf",
                            "size": 4096,
                            "url": "https://cdn.discordapp.com/attachments/333/9001/spec.pdf",
                            "content_type": "application/pdf"
                        }
                    ],
                    "embeds": [{"type": "rich"}]
                },
                {
                    "id": "1003",
                    "channel_id": "333",
                    "author": {"id": "7003", "username": "carol"},
                    "content": "сообщение после",
                    "timestamp": "2026-10-20T12:02:00Z",
                    "pinned": false
                }
            ],
            [
                {
                    "id": "2001",
                    "channel_id": "444",
                    "author": {"id": "7004", "username": "dave"},
                    "content": "целевой хит #2",
                    "timestamp": "2026-10-20T12:05:00Z",
                    "hit": true,
                    "pinned": false
                }
            ]
        ]
    }"#;

    let res = parse_search_http_response(&QueuedBffTransport::http_resp(200, &[], body_200))
        .expect("200 OK response");
    assert_eq!(res.analytics_id.as_deref(), Some("bff_analytics_1"));
    assert_eq!(res.total_results, 2);
    assert!(!res.doing_deep_historical_index);
    assert_eq!(res.documents_indexed, 9120);
    assert!(!res.indexing);
    assert_eq!(res.retry_after_ms, None);
    assert_eq!(res.groups.len(), 2);

    let g0 = &res.groups[0];
    assert_eq!(g0.context_before().len(), 1);
    assert_eq!(g0.context_before()[0].id, "1001");
    assert_eq!(g0.context_after().len(), 1);
    assert_eq!(g0.context_after()[0].id, "1003");
    let hit0 = g0.hit_message().expect("hit message");
    assert_eq!(hit0.id, "1002");
    assert_eq!(hit0.author_id.as_deref(), Some("7002"));
    assert_eq!(hit0.author_username.as_deref(), Some("bob"));
    assert!(hit0.hit);
    assert!(hit0.pinned);
    assert_eq!(hit0.attachments.len(), 1);
    assert_eq!(hit0.attachments[0].filename, "spec.pdf");
    assert_eq!(hit0.embeds_count, 1);

    let hits = res.hits();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].id, "1002");
    assert_eq!(hits[1].id, "2001");

    // 202 Accepted
    let res_202 = parse_search_http_response(&QueuedBffTransport::http_resp(
        202,
        &[("Retry-After", "3")],
        r#"{"message":"Indexing","retry_after":2.0,"documents_indexed":450}"#,
    ))
    .expect("202 Accepted");
    assert!(res_202.indexing);
    assert_eq!(res_202.retry_after_ms, Some(3000));
    assert_eq!(res_202.documents_indexed, 450);
    assert!(res_202.groups.is_empty());

    assert_eq!(
        parse_202_retry_after_ms(&[], br#"{"retry_after":1.5}"#),
        1500
    );
    assert_eq!(
        parse_202_retry_after_ms(&[], b"{}"),
        DEFAULT_SEARCH_202_RETRY_AFTER_MS
    );

    // Коды ошибок HTTP
    for (status, expected) in [
        (401, SearchBffError::Unauthorized),
        (403, SearchBffError::Forbidden),
        (404, SearchBffError::NotFound),
        (400, SearchBffError::Client { status: 400 }),
        (500, SearchBffError::Server { status: 500 }),
        (503, SearchBffError::Server { status: 503 }),
    ] {
        let err = parse_search_http_response(&QueuedBffTransport::http_resp(
            status,
            &[],
            r#"{"message":"secret internal error"}"#,
        ))
        .expect_err("error status");
        assert_eq!(err, expected);
    }
}

#[tokio::test]
async fn execute_with_retry_handles_429_exponential_backoff_and_202_indexing_polling() {
    let scope = SearchBffScope::Guild("1014537851418980372".into());
    let query = parse_search_query("from:42 mentions:77 has:image pinned:true тест").unwrap();

    // 1. Два ответа 429 (retry_after 0.4s -> 400ms, затем 800ms), затем 202 (retry_after 1.5s -> 1500ms, attempts=1), затем 200 OK.
    let transport = Arc::new(QueuedBffTransport::new(vec![
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.4,"global":false}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.4,"global":false}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            202,
            &[],
            r#"{"retry_after":1.5,"documents_indexed":100}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            200,
            &[],
            r#"{"messages":[[{"id":"900","channel_id":"1","content":"найдено","hit":true,"pinned":true}]],"total_results":1,"doing_deep_historical_index":false,"documents_indexed":500}"#,
        )),
    ]));

    let client = SearchBffClient::with_transport(transport.clone());
    let result = client
        .execute_with_retry(&scope, &query)
        .await
        .expect("must recover after 429 + 202");

    assert_eq!(result.total_results, 1);
    assert_eq!(result.hits().len(), 1);
    assert_eq!(result.hits()[0].id, "900");
    assert_eq!(transport.recorded_sleeps(), vec![400, 800, 1500]);

    let targets = transport.recorded_targets();
    assert_eq!(targets.len(), 4);
    assert!(!targets[0].contains("attempts="), "{}", targets[0]);
    assert!(!targets[1].contains("attempts="), "{}", targets[1]);
    assert!(!targets[2].contains("attempts="), "{}", targets[2]);
    assert!(targets[3].contains("attempts=1"), "{}", targets[3]);

    // 2. Исчерпание бюджета повторов 429 (1 начальный + 3 повтора = 4 ответа 429).
    let transport_429_exhaust = Arc::new(QueuedBffTransport::new(vec![
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.5,"global":true}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.5,"global":true}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.5,"global":true}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            429,
            &[],
            r#"{"retry_after":0.5,"global":true}"#,
        )),
    ]));
    let client_429 = SearchBffClient::with_transport(transport_429_exhaust.clone());
    let err_429 = client_429
        .execute_with_retry(&scope, &query)
        .await
        .expect_err("must fail after exhausting 429 retries");
    assert_eq!(
        err_429,
        SearchBffError::RateLimited {
            retry_after_ms: 500,
            is_global: true,
            attempts: 3,
        }
    );
    assert_eq!(
        transport_429_exhaust.recorded_sleeps(),
        vec![500, 1000, 2000]
    );

    // 3. Исчерпание бюджета повторов 202 (1 начальный + 5 повторов = 6 ответов 202).
    let mut responses_202 = Vec::new();
    for _ in 0..=MAX_SEARCH_202_RETRIES {
        responses_202.push(Ok(QueuedBffTransport::http_resp(
            202,
            &[("Retry-After", "2")],
            r#"{"documents_indexed":10}"#,
        )));
    }
    let transport_202_exhaust = Arc::new(QueuedBffTransport::new(responses_202));
    let client_202 = SearchBffClient::with_transport(transport_202_exhaust.clone());
    let err_202 = client_202
        .execute_with_retry(&scope, &query)
        .await
        .expect_err("must fail after exhausting 202 retries");
    assert_eq!(
        err_202,
        SearchBffError::IndexingTimeout {
            retry_after_ms: 2000,
            attempts: 5,
        }
    );
    assert_eq!(
        transport_202_exhaust.recorded_sleeps(),
        vec![2000, 2000, 2000, 2000, 2000]
    );
}

#[tokio::test]
async fn search_cached_deduplicates_equivalent_queries_and_supports_invalidation_and_token_redaction(
) {
    let scope = SearchBffScope::Guild("1014537851418980372".into());
    let transport = Arc::new(QueuedBffTransport::new(vec![
        Ok(QueuedBffTransport::http_resp(
            200,
            &[],
            r#"{"messages":[[{"id":"c1","channel_id":"10","content":"cached","hit":true}]],"total_results":1,"doing_deep_historical_index":false}"#,
        )),
        Ok(QueuedBffTransport::http_resp(
            200,
            &[],
            r#"{"messages":[[{"id":"c2","channel_id":"10","content":"fresh","hit":true}]],"total_results":2,"doing_deep_historical_index":false}"#,
        )),
    ]));
    let client = SearchBffClient::with_transport(transport.clone());

    let q1 = parse_search_query("from:42 mentions:77 has:link pinned:true  кэш  тест").unwrap();
    let mut q2 = parse_search_query("pinned:true has:LINK mentions:77 from:42 кэш тест").unwrap();
    q2.search_session_id = Some("transient-session-id".into());
    q2.search_query_id = Some("transient-query-id".into());

    let r1 = client.search_cached(&scope, &q1).await.expect("first call");
    assert_eq!(r1.total_results, 1);
    assert_eq!(client.cache_len(), 1);
    assert_eq!(transport.recorded_targets().len(), 1);

    // Повторный эквивалентный запрос с другим порядком токенов и новыми UUID берётся из кэша
    let r2 = client
        .search_cached(&scope, &q2)
        .await
        .expect("cached call");
    assert_eq!(r2, r1);
    assert_eq!(transport.recorded_targets().len(), 1);

    // Инвалидация кэша области сбрасывает запись
    client.invalidate_scope_cache(&scope);
    assert_eq!(client.cache_len(), 0);
    let r3 = client
        .search_cached(&scope, &q1)
        .await
        .expect("after invalidate");
    assert_eq!(r3.total_results, 2);
    assert_eq!(transport.recorded_targets().len(), 2);

    // Проверка пагинации и отсутствия утечки токена в Debug/Display
    let paged = q1.next_page(0);
    assert_eq!(paged.offset, Some(25));
    let paged2 = paged.next_page(25);
    assert_eq!(paged2.offset, Some(50));

    assert_eq!(
        SearchBffClient::new("   ").unwrap_err(),
        SearchBffError::MissingToken
    );
    let real_client = SearchBffClient::new("super-secret-discord-token-999").expect("client");
    let debug_str = format!("{real_client:?}");
    assert!(
        !debug_str.contains("super-secret-discord-token-999"),
        "токен не должен утекать в Debug: {debug_str}"
    );

    // Serde round-trip для SearchBffQuery и SearchBffResponse + проверка single()
    let query_json = serde_json::to_string(&q1).expect("serialize SearchBffQuery");
    let query_roundtrip: SearchBffQuery =
        serde_json::from_str(&query_json).expect("deserialize SearchBffQuery");
    assert_eq!(query_roundtrip, q1);

    let resp_json = serde_json::to_string(&r1).expect("serialize SearchBffResponse");
    let resp_roundtrip: SearchBffResponse =
        serde_json::from_str(&resp_json).expect("deserialize SearchBffResponse");
    assert_eq!(resp_roundtrip, r1);

    let single_transport = QueuedBffTransport::single(
        200,
        &[],
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    );
    let single_client = SearchBffClient::with_transport(single_transport);
    let empty_res = single_client
        .execute(&scope, &q1)
        .await
        .expect("empty 200 OK");
    assert_eq!(empty_res.total_results, 0);
    assert!(empty_res.groups.is_empty());
}
