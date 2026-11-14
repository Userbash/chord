use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chord_core::rest::transport::HttpResponse;
use chord_client::search::{
    deterministic_uuid_v4, generate_uuid_v4, DiscordSearch, RemoteSearch, SearchError,
    SearchParams, SearchResponse, SearchSession, SearchTransport,
    DEFAULT_SEARCH_202_RETRY_AFTER_MS, MAX_INDEXING_RETRIES, MAX_SEARCH_202_RETRIES,
    MAX_SEARCH_OFFSET, MAX_SEARCH_PAGES,
};

struct FakeTransport {
    response: Mutex<Result<HttpResponse, SearchError>>,
    target: Mutex<Option<String>>,
}

impl FakeTransport {
    fn new(status: u16, body: &str) -> Self {
        Self {
            response: Mutex::new(Ok(HttpResponse {
                status,
                headers: Vec::new(),
                body: body.as_bytes().to_vec(),
            })),
            target: Mutex::new(None),
        }
    }

    fn network() -> Self {
        Self {
            response: Mutex::new(Err(SearchError::Network)),
            target: Mutex::new(None),
        }
    }
}

impl SearchTransport for FakeTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<HttpResponse, SearchError>> + Send + 'a>,
    > {
        *self.target.lock().unwrap() = Some(target.to_string());
        let response = self.response.lock().unwrap().clone();
        Box::pin(async move { response })
    }
}

fn params() -> SearchParams {
    SearchParams {
        content: Some("a phrase/тест".into()),
        channel_id: Some("123".into()),
        sort_by: Some("timestamp".into()),
        sort_order: Some("desc".into()),
        offset: Some(4),
        ..SearchParams::default()
    }
}

#[tokio::test]
async fn sends_confirmed_query_and_decodes_empty_result() {
    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search = DiscordSearch::with_transport(fake.clone());
    let result = search.search("guild 1", &params()).await.expect("result");
    assert!(result.messages.is_empty());
    assert_eq!(result.total_results, 0);
    assert!(!result.doing_deep_historical_index);
    let target = fake.target.lock().unwrap().clone().expect("target");
    assert_eq!(
        target,
        "/guilds/guild%201/messages/search?content=a%20phrase%2F%D1%82%D0%B5%D1%81%D1%82&channel_id=123&sort_by=timestamp&sort_order=desc&offset=4"
    );
}

#[tokio::test]
async fn maps_401_and_429_to_typed_errors() {
    for (status, expected) in [
        (401, SearchError::Unauthorized),
        (429, SearchError::RateLimited { retry_after_ms: 0 }),
    ] {
        let fake = Arc::new(FakeTransport::new(status, "{}"));
        let search = DiscordSearch::with_transport(fake);
        let error = search
            .search("1", &SearchParams::default())
            .await
            .expect_err("error");
        assert_eq!(error, expected);
    }
}

#[tokio::test]
async fn rejects_negative_offset_before_transport() {
    let fake = Arc::new(FakeTransport::new(200, "{}"));
    let search = DiscordSearch::with_transport(fake.clone());
    let error = search
        .search(
            "1",
            &SearchParams {
                offset: Some(-1),
                ..SearchParams::default()
            },
        )
        .await
        .expect_err("negative offset");
    assert_eq!(error, SearchError::InvalidArg);
    assert!(fake.target.lock().unwrap().is_none());
}

#[tokio::test]
async fn maps_network_error_without_exposing_details() {
    let search = DiscordSearch::with_transport(Arc::new(FakeTransport::network()));
    let error = search
        .search("1", &SearchParams::default())
        .await
        .expect_err("network");
    assert_eq!(error, SearchError::Network);
}

#[tokio::test]
async fn maps_malformed_json_to_typed_error() {
    let search = DiscordSearch::with_transport(Arc::new(FakeTransport::new(200, "[]")));
    let error = search
        .search("1", &SearchParams::default())
        .await
        .expect_err("invalid response");
    assert_eq!(error, SearchError::InvalidResponse);
}

#[test]
fn no_token_is_typed_before_network() {
    assert_eq!(
        DiscordSearch::new("").expect_err("empty token"),
        SearchError::MissingToken
    );
}

#[tokio::test]
async fn rejects_empty_guild_id_before_transport() {
    let fake = Arc::new(FakeTransport::new(200, "{}"));
    let search = DiscordSearch::with_transport(fake.clone());
    for guild_id in ["", "  "] {
        let error = search
            .search(guild_id, &SearchParams::default())
            .await
            .expect_err("empty guild id");
        assert_eq!(error, SearchError::InvalidArg);
    }
    assert!(fake.target.lock().unwrap().is_none());
}

#[tokio::test]
async fn maps_other_http_errors_to_typed_errors_without_body_leak() {
    // G1-01: старый оракул ждал `Network` для 403/404/5xx — это маскировало
    // «нет доступа»/«не найдено»/«ошибка сервера» под «сеть недоступна».
    // Единый классификатор различает статусы; тело ответа по-прежнему не утекает.
    for (status, expected) in [
        (403, SearchError::Forbidden),
        (404, SearchError::NotFound),
        (400, SearchError::Client { status: 400 }),
        (500, SearchError::Server { status: 500 }),
        (503, SearchError::Server { status: 503 }),
    ] {
        let fake = Arc::new(FakeTransport::new(status, r#"{"message":"raw body"}"#));
        let search = DiscordSearch::with_transport(fake);
        let error = search
            .search("1", &SearchParams::default())
            .await
            .expect_err("http failure");
        assert_eq!(error, expected, "статус {status}");
        assert!(!format!("{error}/{error:?}").contains("raw body"));
    }
}

#[tokio::test]
async fn decodes_hits_and_deep_index_flag() {
    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[{"id":"m1","content":"hi"}],"total_results":2,"doing_deep_historical_index":true}"#,
    ));
    let search = DiscordSearch::with_transport(fake);
    let result = search.search("1", &params()).await.expect("result");
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0]["id"], "m1");
    assert_eq!(result.total_results, 2);
    assert!(result.doing_deep_historical_index);
}

#[test]
fn debug_and_errors_do_not_expose_token() {
    let search = DiscordSearch::new("secret-token").expect("search config");
    assert!(!format!("{search:?}").contains("secret-token"));
    for error in [
        SearchError::InvalidArg,
        SearchError::MissingToken,
        SearchError::Unauthorized,
        SearchError::Forbidden,
        SearchError::NotFound,
        SearchError::Indexing {
            retry_after_ms: 5000,
            attempts: 5,
        },
        SearchError::RateLimited {
            retry_after_ms: 250,
        },
        SearchError::Client { status: 400 },
        SearchError::Server { status: 503 },
        SearchError::Network,
        SearchError::Storage,
        SearchError::InvalidResponse,
    ] {
        let rendered = format!(
            "{error}/{error:?}/{}",
            serde_json::to_string(&error).unwrap()
        );
        assert!(!rendered.contains("secret-token"), "{rendered}");
    }
}

/// SEC-02: отказ keyring/blocking — собственный аддитивный `kind`, чтобы UI не
/// отправлял пользователя «чинить сеть», и без текста причины из хранилища.
#[test]
fn storage_failure_is_typed_for_ui() {
    let value = serde_json::to_value(SearchError::Storage).expect("storage");
    assert_eq!(value["kind"], "storage", "{value}");
    let rendered = format!("{}/{:?}", SearchError::Storage, SearchError::Storage);
    assert!(!rendered.is_empty(), "Display непустой");
    assert!(
        !rendered.contains("Secret Service"),
        "причина keyring не попадает в доменную ошибку: {rendered}"
    );
}

#[test]
fn tauri_command_reads_token_from_backend_and_is_registered() {
    let source = include_str!("../src/main.rs");
    let start = source
        .find("async fn search_remote(")
        .expect("команда search_remote");
    let body = &source[start..source[start..].find("\n}\n").expect("конец команды") + start];
    assert!(
        body.contains("backend.token()"),
        "search_remote обязан брать токен из AuthBackend"
    );
    assert!(
        body.contains("SearchError::MissingToken"),
        "отсутствие токена — типизированная ошибка"
    );
    let start = source.find("generate_handler![").expect("handler") + "generate_handler![".len();
    let end = source[start..].find(']').expect("handler close");
    let commands = &source[start..start + end];
    assert!(commands
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|name| name == "search_remote"));
}

/// Независимый оракул snowflake по timestamp (Discord epoch 2015-01-01T00:00:00Z, RE-DEEP-REA §2.3).
fn snowflake_from_timestamp_ms(timestamp_ms: u64) -> String {
    const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;
    ((timestamp_ms - DISCORD_EPOCH_MS) << 22).to_string()
}

#[tokio::test]
async fn sends_all_confirmed_re_filters_in_query() {
    // RE-DEEP-REA §2.3–2.4: author_id, mentions, has, pinned, author_type,
    // min_id, max_id, limit (=25) вместе с базовыми 5 полями.
    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search = DiscordSearch::with_transport(fake.clone());
    // 2026-10-20T00:00:00.000Z и 2026-10-20T23:59:59.999Z
    let min_id = snowflake_from_timestamp_ms(1_792_454_400_000);
    let max_id = snowflake_from_timestamp_ms(1_792_540_799_999);
    let full = SearchParams {
        content: Some("привет".into()),
        channel_id: Some("1163408591315812392".into()),
        author_id: Some("42".into()),
        mentions: Some("77".into()),
        has: Some("file".into()),
        pinned: Some(true),
        author_type: Some("bot".into()),
        min_id: Some(min_id.clone()),
        max_id: Some(max_id.clone()),
        sort_by: Some("timestamp".into()),
        sort_order: Some("desc".into()),
        limit: Some(25),
        offset: Some(0),
        ..SearchParams::default()
    };
    search
        .search("1014537851418980372", &full)
        .await
        .expect("full search query");
    let target = fake.target.lock().unwrap().clone().expect("target");
    assert_eq!(
        target,
        format!(
            "/guilds/1014537851418980372/messages/search?content=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82&channel_id=1163408591315812392&author_id=42&mentions=77&has=file&pinned=true&author_type=bot&min_id={min_id}&max_id={max_id}&sort_by=timestamp&sort_order=desc&limit=25&offset=0"
        )
    );
}

#[tokio::test]
async fn accepts_all_nine_confirmed_has_tokens_and_three_author_types() {
    // RE-DEEP-REA §2.3: 9 токенов `has`, 3 значения `author_type`, отрицание через `-` и `pinned=false`.
    for token in [
        "link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker", "-file",
    ] {
        let fake = Arc::new(FakeTransport::new(
            200,
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
        ));
        let search = DiscordSearch::with_transport(fake.clone());
        search
            .search(
                "1",
                &SearchParams {
                    has: Some(token.into()),
                    ..SearchParams::default()
                },
            )
            .await
            .unwrap_or_else(|err| panic!("has={token} должен приниматься: {err}"));
        let target = fake.target.lock().unwrap().clone().expect("target");
        assert_eq!(target, format!("/guilds/1/messages/search?has={token}"));
    }

    for author_type in ["user", "bot", "webhook", "-bot"] {
        let fake = Arc::new(FakeTransport::new(
            200,
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
        ));
        let search = DiscordSearch::with_transport(fake.clone());
        search
            .search(
                "1",
                &SearchParams {
                    author_type: Some(author_type.into()),
                    pinned: Some(false),
                    ..SearchParams::default()
                },
            )
            .await
            .unwrap_or_else(|err| panic!("author_type={author_type} должен приниматься: {err}"));
        let target = fake.target.lock().unwrap().clone().expect("target");
        assert_eq!(
            target,
            format!("/guilds/1/messages/search?pinned=false&author_type={author_type}")
        );
    }
}

#[tokio::test]
async fn deserializes_ipc_camel_case_snake_case_and_nested_query_payload() {
    // 1. Прямой camelCase JSON для IPC.
    let camel: SearchParams = serde_json::from_value(serde_json::json!({
        "content": "тест",
        "channelId": "10",
        "authorId": "42",
        "mentions": "77",
        "has": "image",
        "pinned": true,
        "authorType": "user",
        "minId": "100",
        "maxId": "200",
        "sortBy": "timestamp",
        "sortOrder": "asc",
        "limit": 25,
        "offset": 50
    }))
    .expect("camelCase SearchParams");
    assert_eq!(camel.author_id.as_deref(), Some("42"));
    assert_eq!(camel.author_type.as_deref(), Some("user"));
    assert_eq!(camel.min_id.as_deref(), Some("100"));
    assert_eq!(camel.max_id.as_deref(), Some("200"));
    assert_eq!(camel.limit, Some(25));

    // 2. Формат STUB-14 из ui/features/search-overlay.js (`params.query` со списками).
    let from_ui: SearchParams = serde_json::from_value(serde_json::json!({
        "content": "привет",
        "channelId": "1163408591315812392",
        "sortBy": "timestamp",
        "sortOrder": "desc",
        "offset": 0,
        "query": {
            "content": "привет",
            "author_id": ["42"],
            "mentions": ["77"],
            "has": ["image", "video"],
            "channel_id": ["1163408591315812392"],
            "pinned": true,
            "author_type": "bot",
            "min_id": "1559661262766080000",
            "max_id": "1560023650627579904",
            "limit": 25,
            "sort_by": "timestamp",
            "sort_order": "desc",
            "offset": 0
        }
    }))
    .expect("UI params with nested query");

    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search = DiscordSearch::with_transport(fake.clone());
    search
        .search("1014537851418980372", &from_ui)
        .await
        .expect("search with UI payload");
    let target = fake.target.lock().unwrap().clone().expect("target");
    assert_eq!(
        target,
        "/guilds/1014537851418980372/messages/search?content=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82&channel_id=1163408591315812392&author_id=42&mentions=77&has=image&has=video&pinned=true&author_type=bot&min_id=1559661262766080000&max_id=1560023650627579904&sort_by=timestamp&sort_order=desc&limit=25&offset=0"
    );
}

#[tokio::test]
async fn rejects_invalid_filter_values_before_transport() {
    let invalid_cases = [
        SearchParams {
            limit: Some(0),
            ..SearchParams::default()
        },
        SearchParams {
            limit: Some(-5),
            ..SearchParams::default()
        },
        SearchParams {
            limit: Some(26),
            ..SearchParams::default()
        },
        SearchParams {
            has: Some("exe".into()),
            ..SearchParams::default()
        },
        SearchParams {
            has: Some("".into()),
            ..SearchParams::default()
        },
        SearchParams {
            author_type: Some("admin".into()),
            ..SearchParams::default()
        },
        SearchParams {
            author_type: Some("".into()),
            ..SearchParams::default()
        },
        SearchParams {
            author_id: Some("   ".into()),
            ..SearchParams::default()
        },
        SearchParams {
            mentions: Some("".into()),
            ..SearchParams::default()
        },
        SearchParams {
            min_id: Some("not-a-snowflake".into()),
            ..SearchParams::default()
        },
        SearchParams {
            max_id: Some("".into()),
            ..SearchParams::default()
        },
        SearchParams {
            min_id: Some("200".into()),
            max_id: Some("100".into()),
            ..SearchParams::default()
        },
    ];

    for bad in invalid_cases {
        let fake = Arc::new(FakeTransport::new(200, "{}"));
        let search = DiscordSearch::with_transport(fake.clone());
        let error = search
            .search("1", &bad)
            .await
            .expect_err(&format!("ожидался отказ для {bad:?}"));
        assert_eq!(error, SearchError::InvalidArg, "для {bad:?}");
        assert!(
            fake.target.lock().unwrap().is_none(),
            "транспорт не должен вызываться для {bad:?}"
        );
    }
}

struct QueuedTransport {
    responses: Mutex<VecDeque<Result<HttpResponse, SearchError>>>,
    targets: Mutex<Vec<String>>,
    sleeps_ms: Mutex<Vec<u64>>,
}

impl QueuedTransport {
    fn new(responses: Vec<HttpResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into_iter().map(Ok).collect()),
            targets: Mutex::new(Vec::new()),
            sleeps_ms: Mutex::new(Vec::new()),
        }
    }

    fn resp(status: u16, headers: &[(&str, &str)], body: &str) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            body: body.as_bytes().to_vec(),
        }
    }
}

impl SearchTransport for QueuedTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<HttpResponse, SearchError>> + Send + 'a>,
    > {
        self.targets.lock().unwrap().push(target.to_string());
        let next = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(SearchError::Network));
        Box::pin(async move { next })
    }

    fn sleep<'a>(
        &'a self,
        delay: Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        self.sleeps_ms
            .lock()
            .unwrap()
            .push(u64::try_from(delay.as_millis()).unwrap_or(u64::MAX));
        Box::pin(async {})
    }
}

#[tokio::test]
async fn returns_immediate_indexing_envelope_on_http_202_without_sleeping_or_reading_body() {
    // SEARCH-202-NSFW-SPEC.md §3.1 (строки 436-458):
    // При получении HTTP 202 метод execute() / search() НЕ блокирует поток, игнорирует тело 202,
    // разбирает Retry-After через parse_retry_after_ms (fallback 5000 мс) и немедленно возвращает
    // IPC-конверт SearchResponse { indexing: true, retry_after_ms: Some(delay_ms), documents_indexed: 0, .. }.
    let transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(
            202,
            &[("Retry-After", "2")],
            r#"{"retry_after":99,"documents_indexed":1420,"code":110000}"#,
        ),
        QueuedTransport::resp(202, &[("retry-after", "0.25")], "not-json-body-ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "0")], r#"{"retry_after":1}"#),
        QueuedTransport::resp(202, &[("Retry-After", "-3")], "ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "abc")], "ignored"),
        QueuedTransport::resp(202, &[], "ignored"),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());

    let first = search
        .execute(
            "1014537851418980372",
            &SearchParams {
                content: Some("индекс".into()),
                ..SearchParams::default()
            },
        )
        .await
        .expect("202 envelope from execute");
    assert_eq!(
        first,
        SearchResponse {
            messages: Vec::new(),
            total_results: 0,
            doing_deep_historical_index: false,
            documents_indexed: 0,
            indexing: true,
            retry_after_ms: Some(2000),
        }
    );
    let first_json = serde_json::to_value(&first).expect("serialize 202 SearchResponse");
    assert_eq!(first_json["indexing"], true);
    assert_eq!(first_json["retry_after_ms"], 2000);
    assert_eq!(first_json["documents_indexed"], 0);
    assert_eq!(first_json["total_results"], 0);
    assert_eq!(first_json["doing_deep_historical_index"], false);

    // Все невалидные/отсутствующие/дробные значения Retry-After дают fallback 5000 мс без чтения тела.
    for _ in 0..5 {
        let env = search
            .search("1014537851418980372", &SearchParams::default())
            .await
            .expect("202 envelope with 5000ms fallback");
        assert!(env.indexing);
        assert_eq!(env.retry_after_ms, Some(DEFAULT_SEARCH_202_RETRY_AFTER_MS));
        assert_eq!(env.documents_indexed, 0);
        assert!(env.messages.is_empty());
    }

    assert!(
        transport.sleeps_ms.lock().unwrap().is_empty(),
        "execute() не должен вызывать sleep на 202"
    );
    assert_eq!(transport.targets.lock().unwrap().len(), 6);
}

#[tokio::test]
async fn retries_http_202_with_retry_after_header_ignoring_202_body_and_returns_200_results() {
    // M-13 (SEARCH-202-NSFW-RE §5.1–5.4, SEARCH-202-NSFW-SPEC §3.1–3.2):
    // execute_with_retry() выполняет до MAX_SEARCH_202_RETRIES = 5 повторов, пробрасывая &attempts=1..5 в URL:
    // - 1-й ответ: 202 с Retry-After: "2" и конфликтующим/битым телом (retry_after: 99, documents_indexed: 1420);
    //   тело 202 игнорируется, задержка берётся из заголовка Retry-After = 2000 мс.
    // - 2-й ответ (attempts=1): 202 с дробным Retry-After: "0.25" (fallback 5000 мс) и не-JSON телом.
    // - 3-й ответ (attempts=2): 202 с Retry-After: "0" -> fallback 5000 мс.
    // - 4-й ответ (attempts=3): 200 OK с результатами и documents_indexed: 590.
    assert_eq!(MAX_SEARCH_202_RETRIES, 5);
    assert_eq!(DEFAULT_SEARCH_202_RETRY_AFTER_MS, 5000);

    let transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(
            202,
            &[("Retry-After", "2")],
            r#"{"retry_after":99,"documents_indexed":1420,"code":110000}"#,
        ),
        QueuedTransport::resp(202, &[("retry-after", "0.25")], "not-json-body-ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "0")], r#"{"retry_after":1}"#),
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[[{"id":"m202","content":"indexed hit"}]],"total_results":1,"doing_deep_historical_index":true,"documents_indexed":590}"#,
        ),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());
    let result = search
        .execute_with_retry(
            "1014537851418980372",
            SearchParams {
                content: Some("индекс".into()),
                ..SearchParams::default()
            },
        )
        .await
        .expect("200 OK after 202 retries");

    assert_eq!(result.total_results, 1);
    assert!(result.doing_deep_historical_index);
    assert_eq!(result.documents_indexed, 590);
    assert!(!result.indexing);
    assert_eq!(result.retry_after_ms, None);
    assert_eq!(result.messages.len(), 1);
    assert_eq!(
        *transport.sleeps_ms.lock().unwrap(),
        vec![2000, 5000, 5000],
        "задержки должны браться из заголовка Retry-After (сек * 1000) с fallback 5000 мс"
    );
    let targets = transport.targets.lock().unwrap().clone();
    assert_eq!(
        targets,
        vec![
            "/guilds/1014537851418980372/messages/search?content=%D0%B8%D0%BD%D0%B4%D0%B5%D0%BA%D1%81",
            "/guilds/1014537851418980372/messages/search?content=%D0%B8%D0%BD%D0%B4%D0%B5%D0%BA%D1%81&attempts=1",
            "/guilds/1014537851418980372/messages/search?content=%D0%B8%D0%BD%D0%B4%D0%B5%D0%BA%D1%81&attempts=2",
            "/guilds/1014537851418980372/messages/search?content=%D0%B8%D0%BD%D0%B4%D0%B5%D0%BA%D1%81&attempts=3",
        ]
    );

    // Граничный случай: успех ровно на 5-м повторе (6-й запрос) обязан возвращать 200 OK.
    let boundary_transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(202, &[("Retry-After", "1")], "{}"),
        QueuedTransport::resp(202, &[("Retry-After", "1")], "{}"),
        QueuedTransport::resp(202, &[("Retry-After", "1")], "{}"),
        QueuedTransport::resp(202, &[("Retry-After", "1")], "{}"),
        QueuedTransport::resp(202, &[("Retry-After", "1")], "{}"),
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false,"documents_indexed":42}"#,
        ),
    ]));
    let boundary_search = DiscordSearch::with_transport(boundary_transport.clone());
    let boundary_ok = boundary_search
        .execute_with_retry("1", SearchParams::default())
        .await
        .expect("5th retry (6th request) returns 200 OK");
    assert_eq!(boundary_ok.documents_indexed, 42);
    assert!(!boundary_ok.indexing);
    assert_eq!(boundary_ok.retry_after_ms, None);
    assert_eq!(boundary_transport.sleeps_ms.lock().unwrap().len(), 5);
    assert_eq!(
        *boundary_transport.targets.lock().unwrap(),
        vec![
            "/guilds/1/messages/search",
            "/guilds/1/messages/search?attempts=1",
            "/guilds/1/messages/search?attempts=2",
            "/guilds/1/messages/search?attempts=3",
            "/guilds/1/messages/search?attempts=4",
            "/guilds/1/messages/search?attempts=5",
        ]
    );
}

#[tokio::test]
async fn returns_typed_indexing_error_when_all_5_retries_return_202() {
    // M-13: 1 начальный запрос + 5 повторов (MAX_SEARCH_202_RETRIES = 5) = 6 ответов 202.
    // После 5-го повтора execute_with_retry возвращает SearchError::Indexing { retry_after_ms, attempts: 5 }.
    let transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(202, &[("Retry-After", "2")], r#"{"retry_after":99}"#),
        QueuedTransport::resp(202, &[], "ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "-3")], "ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "abc")], "ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "4")], "ignored"),
        QueuedTransport::resp(202, &[("Retry-After", "3")], r#"{"retry_after":77}"#),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());
    let error = search
        .execute_with_retry("1", SearchParams::default())
        .await
        .expect_err("all 5 retries returned 202");

    assert_eq!(
        error,
        SearchError::Indexing {
            retry_after_ms: 3000,
            attempts: MAX_SEARCH_202_RETRIES,
        }
    );
    assert_eq!(
        *transport.sleeps_ms.lock().unwrap(),
        vec![2000, 5000, 5000, 5000, 4000],
        "ровно 5 ожиданий перед 5 повторами; после финального 202 лишнего sleep нет"
    );
    assert_eq!(
        *transport.targets.lock().unwrap(),
        vec![
            "/guilds/1/messages/search",
            "/guilds/1/messages/search?attempts=1",
            "/guilds/1/messages/search?attempts=2",
            "/guilds/1/messages/search?attempts=3",
            "/guilds/1/messages/search?attempts=4",
            "/guilds/1/messages/search?attempts=5",
        ]
    );

    let json = serde_json::to_value(&error).expect("serialize Indexing error");
    assert_eq!(json["kind"], "indexing");
    assert_eq!(json["retry_after_ms"], 3000);
    assert_eq!(json["attempts"], 5);
}

#[tokio::test]
async fn decodes_documents_indexed_from_200_ok_and_defaults_to_zero_when_omitted() {
    // SEARCH-202-NSFW-SPEC.md:446-448 и RE §6:516-517:
    // documents_indexed имеет тип u64 с #[serde(default, rename = "documents_indexed")]:
    // при наличии 590 -> 590, при отсутствии в JSON 200 OK -> 0.
    let with_docs = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[{"id":"m1"}],"total_results":12,"doing_deep_historical_index":true,"documents_indexed":590}"#,
    ));
    let search = DiscordSearch::with_transport(with_docs);
    let result = search
        .search("1", &SearchParams::default())
        .await
        .expect("200 with documents_indexed");
    assert_eq!(result.documents_indexed, 590_u64);
    assert_eq!(result.total_results, 12);
    assert!(result.doing_deep_historical_index);
    assert!(!result.indexing);
    assert_eq!(result.retry_after_ms, None);
    let with_docs_json = serde_json::to_value(&result).expect("serialize 200 SearchResponse");
    assert_eq!(with_docs_json["documents_indexed"], 590);
    assert_eq!(with_docs_json["indexing"], false);
    assert!(
        with_docs_json.get("retry_after_ms").is_none(),
        "retry_after_ms == None должен опускаться при сериализации SearchResponse"
    );

    let without_docs = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search_zero = DiscordSearch::with_transport(without_docs);
    let result_zero = search_zero
        .search("1", &SearchParams::default())
        .await
        .expect("200 without documents_indexed");
    assert_eq!(result_zero.documents_indexed, 0_u64);
    assert!(!result_zero.indexing);
    assert_eq!(result_zero.retry_after_ms, None);
}

#[tokio::test]
async fn enforces_max_search_offset_9975_and_rejects_above_ceiling_before_transport() {
    assert_eq!(MAX_SEARCH_OFFSET, 9975);
    assert_eq!(MAX_SEARCH_PAGES, 400);

    // Граница 9975 (MAX_SEARCH_OFFSET) разрешена.
    let ok_fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":10000,"doing_deep_historical_index":false}"#,
    ));
    let ok_search = DiscordSearch::with_transport(ok_fake.clone());
    ok_search
        .search(
            "1",
            &SearchParams {
                offset: Some(MAX_SEARCH_OFFSET),
                ..SearchParams::default()
            },
        )
        .await
        .expect("offset == 9975 allowed");
    assert_eq!(
        ok_fake.target.lock().unwrap().as_deref(),
        Some("/guilds/1/messages/search?offset=9975")
    );

    // Любой offset > 9975 отвергается до сети (InvalidArg).
    for invalid_offset in [MAX_SEARCH_OFFSET + 1, 10_000, i64::MAX] {
        let bad_fake = Arc::new(FakeTransport::new(200, "{}"));
        let bad_search = DiscordSearch::with_transport(bad_fake.clone());
        let err = bad_search
            .search(
                "1",
                &SearchParams {
                    offset: Some(invalid_offset),
                    ..SearchParams::default()
                },
            )
            .await
            .expect_err("offset > 9975 must fail");
        assert_eq!(err, SearchError::InvalidArg, "offset={invalid_offset}");
        assert!(
            bad_fake.target.lock().unwrap().is_none(),
            "транспорт не должен вызываться при offset={invalid_offset}"
        );
    }
}

#[tokio::test]
async fn includes_nsfw_and_attempts_query_params_and_validates_attempts_range() {
    // 1. include_nsfw == Some(true) -> в query string добавляется include_nsfw=true.
    let fake_true = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search_true = DiscordSearch::with_transport(fake_true.clone());
    search_true
        .search(
            "1",
            &SearchParams {
                content: Some("nsfw".into()),
                include_nsfw: Some(true),
                ..SearchParams::default()
            },
        )
        .await
        .expect("include_nsfw=true");
    assert_eq!(
        fake_true.target.lock().unwrap().as_deref(),
        Some("/guilds/1/messages/search?content=nsfw&include_nsfw=true")
    );

    // 2. include_nsfw == Some(false) или None -> параметр include_nsfw опускается из URL.
    for omitted in [None, Some(false)] {
        let fake = Arc::new(FakeTransport::new(
            200,
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
        ));
        let search = DiscordSearch::with_transport(fake.clone());
        search
            .search(
                "1",
                &SearchParams {
                    content: Some("safe".into()),
                    include_nsfw: omitted,
                    ..SearchParams::default()
                },
            )
            .await
            .expect("search without include_nsfw");
        let target = fake.target.lock().unwrap().clone().expect("target");
        assert_eq!(target, "/guilds/1/messages/search?content=safe");
        assert!(!target.contains("include_nsfw"));
    }

    // 3. attempts: при None или Some(0) не добавляется в URL; при Some(1..=5) добавляется &attempts={n};
    //    при Some(n > 5) возвращается SearchError::InvalidArg до сети.
    for omitted_attempts in [None, Some(0)] {
        let fake = Arc::new(FakeTransport::new(
            200,
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
        ));
        let search = DiscordSearch::with_transport(fake.clone());
        search
            .search(
                "1",
                &SearchParams {
                    content: Some("q".into()),
                    attempts: omitted_attempts,
                    ..SearchParams::default()
                },
            )
            .await
            .expect("attempts None or Some(0)");
        assert_eq!(
            fake.target.lock().unwrap().as_deref(),
            Some("/guilds/1/messages/search?content=q")
        );
    }

    for n in 1..=MAX_SEARCH_202_RETRIES {
        let fake = Arc::new(FakeTransport::new(
            200,
            r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
        ));
        let search = DiscordSearch::with_transport(fake.clone());
        search
            .search(
                "1",
                &SearchParams {
                    content: Some("q".into()),
                    attempts: Some(n),
                    ..SearchParams::default()
                },
            )
            .await
            .expect("valid attempts 1..=5");
        assert_eq!(
            fake.target.lock().unwrap().as_deref(),
            Some(format!("/guilds/1/messages/search?content=q&attempts={n}").as_str())
        );
    }

    for invalid_attempts in [MAX_SEARCH_202_RETRIES + 1, 10, u32::MAX] {
        let bad_fake = Arc::new(FakeTransport::new(200, "{}"));
        let bad_search = DiscordSearch::with_transport(bad_fake.clone());
        let err = bad_search
            .search(
                "1",
                &SearchParams {
                    attempts: Some(invalid_attempts),
                    ..SearchParams::default()
                },
            )
            .await
            .expect_err("attempts > 5 must fail");
        assert_eq!(err, SearchError::InvalidArg, "attempts={invalid_attempts}");
        assert!(
            bad_fake.target.lock().unwrap().is_none(),
            "транспорт не должен вызываться при attempts={invalid_attempts}"
        );
    }

    // 4. Десериализация include_nsfw и attempts из верхнего уровня и вложенного query.
    let from_nested: SearchParams = serde_json::from_value(serde_json::json!({
        "content": "nsfw",
        "query": {
            "include_nsfw": true,
            "attempts": 3
        }
    }))
    .expect("nested include_nsfw and attempts");
    assert_eq!(from_nested.include_nsfw, Some(true));
    assert_eq!(from_nested.attempts, Some(3));

    let from_top: SearchParams = serde_json::from_value(serde_json::json!({
        "includeNsfw": false,
        "attempts": 2
    }))
    .expect("top-level includeNsfw and attempts");
    assert_eq!(from_top.include_nsfw, Some(false));
    assert_eq!(from_top.attempts, Some(2));
}

#[tokio::test]
async fn retries_http_202_against_local_mock_http_server_and_parses_200_ok() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind local mock HTTP server");
    let addr = listener.local_addr().expect("local addr");
    let requests = Arc::new(Mutex::new(Vec::<String>::new()));
    let requests_clone = requests.clone();

    let server = tokio::spawn(async move {
        let responses = [
            (
                "202 Accepted",
                "Retry-After: 1\r\n",
                r#"{"retry_after":99,"documents_indexed":9999}"#,
            ),
            (
                "200 OK",
                "",
                r#"{"messages":[[{"id":"m-live-202"}]],"total_results":1,"doing_deep_historical_index":false,"documents_indexed":590}"#,
            ),
        ];
        for (status, extra_headers, body) in responses {
            let Ok(Ok((mut stream, _))) =
                tokio::time::timeout(Duration::from_secs(3), listener.accept()).await
            else {
                break;
            };
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.expect("read request");
            let req_str = String::from_utf8_lossy(&buf[..n]).to_string();
            let first_line = req_str.lines().next().unwrap_or("").to_string();
            requests_clone.lock().unwrap().push(first_line);

            let wire = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(wire.as_bytes())
                .await
                .expect("write response");
        }
    });

    let search =
        DiscordSearch::with_base("secret-token", &format!("http://{addr}/api/v9")).expect("client");
    let result = search
        .execute_with_retry(
            "42",
            SearchParams {
                content: Some("hello".into()),
                include_nsfw: Some(true),
                offset: Some(9975),
                ..SearchParams::default()
            },
        )
        .await
        .expect("200 OK after 202 retry on local HTTP server");

    server.await.expect("mock server task");
    assert_eq!(result.total_results, 1);
    assert_eq!(result.documents_indexed, 590);
    assert!(!result.indexing);
    assert_eq!(result.retry_after_ms, None);
    let seen = requests.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(
        seen[0],
        "GET /api/v9/guilds/42/messages/search?content=hello&offset=9975&include_nsfw=true HTTP/1.1"
    );
    assert_eq!(
        seen[1],
        "GET /api/v9/guilds/42/messages/search?content=hello&offset=9975&include_nsfw=true&attempts=1 HTTP/1.1"
    );
}

#[tokio::test]
async fn serializes_search_session_id_and_search_query_id_in_query_string() {
    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{"messages":[],"total_results":0,"doing_deep_historical_index":false}"#,
    ));
    let search = DiscordSearch::with_transport(fake.clone());

    let session_id = "d6183157-1379-4323-9321-2c4743e835a5";
    let query_id = "cfb6f5e3-11e3-44be-91b2-8360807ed063";

    let params = SearchParams {
        content: Some("тест".into()),
        sort_by: Some("timestamp".into()),
        sort_order: Some("desc".into()),
        search_session_id: Some(session_id.into()),
        search_query_id: Some(query_id.into()),
        offset: Some(0),
        ..SearchParams::default()
    };

    search
        .search("1014537851418980372", &params)
        .await
        .expect("search with session and query ids");

    let target = fake.target.lock().unwrap().clone().expect("target");
    assert_eq!(
        target,
        format!(
            "/guilds/1014537851418980372/messages/search?content=%D1%82%D0%B5%D1%81%D1%82&sort_by=timestamp&sort_order=desc&search_session_id={session_id}&search_query_id={query_id}&offset=0"
        )
    );

    // Deserialization: camelCase and snake_case from top-level and nested query
    let from_camel: SearchParams = serde_json::from_value(serde_json::json!({
        "content": "word",
        "searchSessionId": session_id,
        "searchQueryId": query_id
    }))
    .expect("camelCase session IDs");
    assert_eq!(from_camel.search_session_id.as_deref(), Some(session_id));
    assert_eq!(from_camel.search_query_id.as_deref(), Some(query_id));

    let from_nested: SearchParams = serde_json::from_value(serde_json::json!({
        "query": {
            "content": "word",
            "search_session_id": session_id,
            "search_query_id": query_id
        }
    }))
    .expect("nested query session IDs");
    assert_eq!(from_nested.search_session_id.as_deref(), Some(session_id));
    assert_eq!(from_nested.search_query_id.as_deref(), Some(query_id));
}

#[tokio::test]
async fn reuses_search_session_id_and_query_id_on_pagination_and_refreshes_query_id_on_new_query() {
    let session_id = "045f2359-1bfe-4bd0-beb2-60be38c82478";
    let query_id = "cfb6f5e3-11e3-44be-91b2-8360807ed063";

    let mut session = SearchSession::with_ids(session_id, query_id);
    assert_eq!(session.session_id, session_id);
    assert_eq!(session.query_id, query_id);

    let mut initial_params = SearchParams {
        content: Some("поиск".into()),
        offset: Some(0),
        ..SearchParams::default()
    };
    session.apply_to(&mut initial_params);

    assert_eq!(
        initial_params.search_session_id.as_deref(),
        Some(session_id)
    );
    assert_eq!(initial_params.search_query_id.as_deref(), Some(query_id));

    // Pagination: page 2 (offset 25) preserves both session_id and query_id
    let page2 = initial_params.with_offset(25);
    assert_eq!(page2.offset, Some(25));
    assert_eq!(page2.search_session_id.as_deref(), Some(session_id));
    assert_eq!(page2.search_query_id.as_deref(), Some(query_id));

    // Pagination: next_page(25) step
    let page3 = page2.next_page(25);
    assert_eq!(page3.offset, Some(50));
    assert_eq!(page3.search_session_id.as_deref(), Some(session_id));
    assert_eq!(page3.search_query_id.as_deref(), Some(query_id));

    // New query in same session: session_id is preserved, query_id is refreshed, offset reset to 0
    let refreshed_query_id = session.refresh_query_id();
    assert_ne!(refreshed_query_id, query_id);
    assert_eq!(session.session_id, session_id);

    let new_params = initial_params.new_query_in_session(Some("новый запрос".into()));
    assert_eq!(new_params.content.as_deref(), Some("новый запрос"));
    assert_eq!(new_params.offset, Some(0));
    assert_eq!(new_params.search_session_id.as_deref(), Some(session_id));
    assert_ne!(new_params.search_query_id.as_deref(), Some(query_id));
    assert!(new_params.search_query_id.is_some());

    // Auto-generation of UUID v4
    let mut auto_params = SearchParams::default();
    auto_params.ensure_session_ids();
    let gen_session = auto_params.search_session_id.expect("generated session");
    let gen_query = auto_params.search_query_id.expect("generated query");
    assert_eq!(gen_session.len(), 36);
    assert_eq!(gen_query.len(), 36);
    assert_eq!(&gen_session[14..15], "4"); // UUID v4 version
    assert_eq!(&gen_query[14..15], "4");
    assert_ne!(gen_session, gen_query);
}

#[tokio::test]
async fn parses_retry_after_from_json_body_on_http_202_when_header_is_absent() {
    let transport = Arc::new(QueuedTransport::new(vec![
        // 1. Float seconds in body (2.0s -> 2000ms)
        QueuedTransport::resp(
            202,
            &[],
            r#"{"retry_after":2.0,"message":"indexing in progress"}"#,
        ),
        // 2. Fractional float in body (1.5s -> 1500ms)
        QueuedTransport::resp(
            202,
            &[],
            r#"{"retry_after":1.5,"message":"still indexing"}"#,
        ),
        // 3. Integer in body (3s -> 3000ms)
        QueuedTransport::resp(202, &[], r#"{"retry_after":3}"#),
    ]));
    let search = DiscordSearch::with_transport(transport);

    let res1 = search
        .execute("1", &SearchParams::default())
        .await
        .expect("202 envelope float");
    assert!(res1.indexing);
    assert_eq!(res1.retry_after_ms, Some(2000));

    let res2 = search
        .execute("1", &SearchParams::default())
        .await
        .expect("202 envelope fractional");
    assert!(res2.indexing);
    assert_eq!(res2.retry_after_ms, Some(1500));

    let res3 = search
        .execute("1", &SearchParams::default())
        .await
        .expect("202 envelope integer");
    assert!(res3.indexing);
    assert_eq!(res3.retry_after_ms, Some(3000));
}

#[tokio::test]
async fn retries_http_202_using_retry_after_from_json_body_and_preserves_session_ids() {
    assert_eq!(MAX_INDEXING_RETRIES, 5);

    let session_id = "d6183157-1379-4323-9321-2c4743e835a5";
    let query_id = "cfb6f5e3-11e3-44be-91b2-8360807ed063";

    let transport = Arc::new(QueuedTransport::new(vec![
        // Attempt 0: 202 with body retry_after: 1.0 (no header)
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.0,"message":"indexing"}"#),
        // Attempt 1: 202 with body retry_after: 2.0 (no header)
        QueuedTransport::resp(202, &[], r#"{"retry_after":2.0,"message":"indexing"}"#),
        // Attempt 2: 200 OK
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[[{"id":"m-retried"}]],"total_results":1,"doing_deep_historical_index":false,"documents_indexed":100}"#,
        ),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());

    let params = SearchParams {
        content: Some("test".into()),
        search_session_id: Some(session_id.into()),
        search_query_id: Some(query_id.into()),
        ..SearchParams::default()
    };

    let result = search
        .execute_with_retry("1", params)
        .await
        .expect("200 OK after body retry_after 202 responses");

    assert_eq!(result.total_results, 1);
    assert_eq!(result.documents_indexed, 100);
    assert!(!result.indexing);
    assert_eq!(result.retry_after_ms, None);

    assert_eq!(*transport.sleeps_ms.lock().unwrap(), vec![1000, 2000]);

    let targets = transport.targets.lock().unwrap().clone();
    assert_eq!(targets.len(), 3);
    assert_eq!(
        targets[0],
        format!("/guilds/1/messages/search?content=test&search_session_id={session_id}&search_query_id={query_id}")
    );
    assert_eq!(
        targets[1],
        format!("/guilds/1/messages/search?content=test&search_session_id={session_id}&search_query_id={query_id}&attempts=1")
    );
    assert_eq!(
        targets[2],
        format!("/guilds/1/messages/search?content=test&search_session_id={session_id}&search_query_id={query_id}&attempts=2")
    );
}

#[tokio::test]
async fn returns_typed_indexing_error_after_5_retries_with_json_body_retry_after() {
    let transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.5}"#),
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.5}"#),
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.5}"#),
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.5}"#),
        QueuedTransport::resp(202, &[], r#"{"retry_after":1.5}"#),
        QueuedTransport::resp(202, &[], r#"{"retry_after":2.5}"#),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());

    let error = search
        .execute_with_retry("1", SearchParams::default())
        .await
        .expect_err("5 retries exhausted");

    assert_eq!(
        error,
        SearchError::Indexing {
            retry_after_ms: 2500,
            attempts: MAX_INDEXING_RETRIES,
        }
    );
    assert_eq!(
        *transport.sleeps_ms.lock().unwrap(),
        vec![1500, 1500, 1500, 1500, 1500]
    );
}

#[tokio::test]
async fn rejects_invalid_session_and_query_ids_before_transport() {
    for bad in ["", "  ", "has\r\nnewline", "has{brace}"] {
        let fake_sess = Arc::new(FakeTransport::new(200, "{}"));
        let search_sess = DiscordSearch::with_transport(fake_sess.clone());
        let err1 = search_sess
            .search(
                "1",
                &SearchParams {
                    search_session_id: Some(bad.into()),
                    ..SearchParams::default()
                },
            )
            .await
            .expect_err("bad search_session_id");
        assert_eq!(err1, SearchError::InvalidArg);
        assert!(fake_sess.target.lock().unwrap().is_none());

        let fake_query = Arc::new(FakeTransport::new(200, "{}"));
        let search_query = DiscordSearch::with_transport(fake_query.clone());
        let err2 = search_query
            .search(
                "1",
                &SearchParams {
                    search_query_id: Some(bad.into()),
                    ..SearchParams::default()
                },
            )
            .await
            .expect_err("bad search_query_id");
        assert_eq!(err2, SearchError::InvalidArg);
        assert!(fake_query.target.lock().unwrap().is_none());
    }
}

#[test]
fn deterministic_uuid_v4_oracle_shape() {
    let id1 = deterministic_uuid_v4(42);
    let id2 = deterministic_uuid_v4(42);
    assert_eq!(id1, id2, "одинаковый seed -> одинаковый UUID");
    assert_eq!(id1.len(), 36);
    assert_eq!(&id1[14..15], "4"); // version 4
    let variant_char = id1.chars().nth(19).unwrap();
    assert!(matches!(variant_char, '8' | '9' | 'a' | 'b'));

    let id3 = deterministic_uuid_v4(43);
    assert_ne!(id1, id3, "разный seed -> разный UUID");

    let live = generate_uuid_v4();
    assert_eq!(live.len(), 36);
    assert_eq!(&live[14..15], "4");
}

// ============================================================================
// TASK-617: Полнотекстовый поиск по гильдии GET /guilds/{id}/messages/search (BFF + Core REST)
// ============================================================================

#[test]
fn task_617_core_rest_search_normalizes_inline_facets_and_builds_v9_request_with_6_auth_headers() {
    use chord_core::rest::routes::HttpMethod;
    use chord_core::rest::search::{
        build_channel_search_http_request, build_guild_search_http_request, SearchQueryRequest,
        SearchRestError, SearchSortBy, SearchSortOrder, ALLOWED_AUTHOR_TYPES, ALLOWED_HAS_FACETS,
    };
    use chord_core::rest::super_properties::SuperProperties;

    assert_eq!(
        ALLOWED_HAS_FACETS,
        &["link", "embed", "poll", "snapshot", "file", "video", "image", "sound", "sticker"]
    );
    assert_eq!(ALLOWED_AUTHOR_TYPES, &["user", "bot", "webhook"]);

    let raw = "  from:111111111111111111 mentions:222222222222222222 has:Image has:-video in:333333333333333333 before:555555555555555555 after:444444444444444444 pinned:true author_type:User link_hostname:GitHub.com attachment_extension:.PNG attachment_filename:report.pdf   привет   мир  ";
    let mut parsed = SearchQueryRequest::parse_and_normalize(raw).expect("parse inline facets");
    assert_eq!(parsed.content.as_deref(), Some("привет мир"));
    assert_eq!(parsed.author_id, vec!["111111111111111111"]);
    assert_eq!(parsed.mentions, vec!["222222222222222222"]);
    assert_eq!(parsed.has, vec!["image", "-video"]);
    assert_eq!(parsed.channel_id, vec!["333333333333333333"]);
    assert_eq!(parsed.min_id.as_deref(), Some("444444444444444444"));
    assert_eq!(parsed.max_id.as_deref(), Some("555555555555555555"));
    assert_eq!(parsed.pinned, Some(true));
    assert_eq!(parsed.author_type, vec!["user"]);
    assert_eq!(parsed.link_hostname, vec!["github.com"]);
    assert_eq!(parsed.attachment_extension, vec!["png"]);
    assert_eq!(parsed.attachment_filename, vec!["report.pdf"]);

    parsed.sort_by = Some(SearchSortBy::Relevance);
    parsed.sort_order = Some(SearchSortOrder::Desc);
    parsed.limit = Some(25);
    parsed.offset = Some(50);
    parsed.include_nsfw = Some(true);

    let sp = SuperProperties::default();
    let http_req = build_guild_search_http_request(
        "mfa.test_token_617",
        &sp,
        "Asia/Yekaterinburg",
        "1014537851418980372",
        &parsed,
    )
    .expect("build guild search HttpRequest");
    assert_eq!(http_req.method, HttpMethod::Get);
    assert!(
        http_req
            .target
            .starts_with("/api/v9/guilds/1014537851418980372/messages/search?"),
        "неожиданный путь: {}",
        http_req.target
    );
    assert!(http_req
        .target
        .contains("content=%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82%20%D0%BC%D0%B8%D1%80"));
    assert!(http_req.target.contains("channel_id=333333333333333333"));
    assert!(http_req.target.contains("author_id=111111111111111111"));
    assert!(http_req.target.contains("mentions=222222222222222222"));
    assert!(http_req.target.contains("has=image&has=-video"));
    assert!(http_req.target.contains("pinned=true"));
    assert!(http_req.target.contains("author_type=user"));
    assert!(http_req.target.contains("min_id=444444444444444444"));
    assert!(http_req.target.contains("max_id=555555555555555555"));
    assert!(http_req.target.contains("link_hostname=github.com"));
    assert!(http_req.target.contains("attachment_extension=png"));
    assert!(http_req.target.contains("attachment_filename=report.pdf"));
    assert!(http_req
        .target
        .contains("sort_by=relevance&sort_order=desc"));
    assert!(http_req.target.contains("limit=25&offset=50"));
    assert!(http_req.target.contains("include_nsfw=true"));

    // Проверяем 6 обязательных заголовков Discord REST v9.
    let header_names: Vec<&str> = http_req.headers.iter().map(|(k, _)| k.as_str()).collect();
    for expected in [
        "Authorization",
        "X-Super-Properties",
        "X-Discord-Locale",
        "X-Discord-Timezone",
        "X-Debug-Options",
        "Accept-Language",
    ] {
        assert!(
            header_names.contains(&expected),
            "отсутствует заголовок {expected}"
        );
    }

    let ch_req = build_channel_search_http_request(
        "mfa.test_token_617",
        &sp,
        "Asia/Yekaterinburg",
        "333333333333333333",
        &parsed,
    )
    .expect("build channel search HttpRequest");
    assert!(ch_req
        .target
        .starts_with("/api/v9/channels/333333333333333333/messages/search?"));

    // Валидация пустого запроса и невалидных границ.
    assert_eq!(
        SearchQueryRequest::default().normalized(),
        Err(SearchRestError::EmptyQuery)
    );
    assert_eq!(
        SearchQueryRequest {
            content: Some("ok".into()),
            offset: Some(9976),
            ..SearchQueryRequest::default()
        }
        .normalized(),
        Err(SearchRestError::OffsetOutOfRange {
            offset: 9976,
            max: 9975
        })
    );
    assert_eq!(
        SearchQueryRequest {
            content: Some("ok".into()),
            limit: Some(26),
            ..SearchQueryRequest::default()
        }
        .normalized(),
        Err(SearchRestError::InvalidLimit { limit: 26 })
    );
    assert_eq!(
        SearchQueryRequest {
            has: vec!["executable".into()],
            ..SearchQueryRequest::default()
        }
        .normalized(),
        Err(SearchRestError::InvalidHasFacet("executable".into()))
    );
}

#[test]
fn task_617_core_rest_search_parses_200_nested_context_groups_202_indexing_and_errors() {
    use chord_core::rest::search::{
        compute_429_exponential_backoff_ms, parse_search_http_response, SearchRestError,
    };

    let body_200 = r#"{
        "analytics_id": "a1b2c3d4e5f6",
        "total_results": 2,
        "doing_deep_historical_index": false,
        "documents_indexed": 4812,
        "messages": [
            [
                {
                    "id": "1001",
                    "channel_id": "333333333333333333",
                    "author": {"id": "7001", "username": "alice"},
                    "content": "контекст до",
                    "timestamp": "2026-10-20T12:00:00.000000+00:00",
                    "pinned": false,
                    "attachments": [],
                    "embeds": []
                },
                {
                    "id": "1002",
                    "channel_id": "333333333333333333",
                    "author": {"id": "7002", "username": "bob"},
                    "content": "найденное сообщение #1",
                    "timestamp": "2026-10-20T12:01:00.000000+00:00",
                    "hit": true,
                    "pinned": true,
                    "attachments": [
                        {
                            "id": "9001",
                            "filename": "report.pdf",
                            "size": 2048,
                            "url": "https://cdn.discordapp.com/attachments/333/9001/report.pdf",
                            "content_type": "application/pdf"
                        }
                    ],
                    "embeds": [{"type": "rich"}, {"type": "image"}]
                },
                {
                    "id": "1003",
                    "channel_id": "333333333333333333",
                    "author": {"id": "7003", "username": "carol"},
                    "content": "контекст после",
                    "timestamp": "2026-10-20T12:02:00.000000+00:00",
                    "pinned": false,
                    "attachments": [],
                    "embeds": []
                }
            ],
            [
                {
                    "id": "2001",
                    "channel_id": "444444444444444444",
                    "author": {"id": "7004", "username": "dave"},
                    "content": "найденное сообщение #2",
                    "timestamp": "2026-10-20T12:05:00.000000+00:00",
                    "hit": true,
                    "pinned": false
                }
            ]
        ]
    }"#;

    let res_200 = parse_search_http_response(&HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: body_200.as_bytes().to_vec(),
    })
    .expect("parse 200 OK nested messages[][]");

    assert_eq!(res_200.analytics_id.as_deref(), Some("a1b2c3d4e5f6"));
    assert_eq!(res_200.total_results, 2);
    assert!(!res_200.doing_deep_historical_index);
    assert_eq!(res_200.documents_indexed, 4812);
    assert!(!res_200.indexing);
    assert_eq!(res_200.retry_after_ms, None);
    assert_eq!(res_200.groups.len(), 2);

    let g0 = &res_200.groups[0];
    assert_eq!(g0.messages.len(), 3);
    assert_eq!(g0.context_before().len(), 1);
    assert_eq!(g0.context_before()[0].id, "1001");
    assert_eq!(g0.context_after().len(), 1);
    assert_eq!(g0.context_after()[0].id, "1003");
    let hit0 = g0.hit_message().expect("hit message in group 0");
    assert_eq!(hit0.id, "1002");
    assert_eq!(hit0.author_id.as_deref(), Some("7002"));
    assert_eq!(hit0.author_username.as_deref(), Some("bob"));
    assert!(hit0.hit);
    assert!(hit0.pinned);
    assert_eq!(hit0.attachments.len(), 1);
    assert_eq!(hit0.attachments[0].filename, "report.pdf");
    assert_eq!(hit0.attachments[0].size, 2048);
    assert_eq!(hit0.embeds_count, 2);

    let hits = res_200.hits();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].id, "1002");
    assert_eq!(hits[1].id, "2001");

    // 202 Accepted (indexing)
    let res_202 = parse_search_http_response(&HttpResponse {
        status: 202,
        headers: vec![("Retry-After".into(), "2".into())],
        body: br#"{"message":"Index not yet available","retry_after":2.0,"documents_indexed":310}"#
            .to_vec(),
    })
    .expect("parse 202 Accepted");
    assert!(res_202.indexing);
    assert_eq!(res_202.retry_after_ms, Some(2000));
    assert_eq!(res_202.documents_indexed, 310);
    assert!(res_202.groups.is_empty());

    // 429 Too Many Requests + экспоненциальный бэкофф
    let err_429 = parse_search_http_response(&HttpResponse {
        status: 429,
        headers: Vec::new(),
        body: br#"{"message":"You are being rate limited.","retry_after":1.5,"global":false}"#
            .to_vec(),
    })
    .expect_err("429 must be RateLimited");
    assert_eq!(
        err_429,
        SearchRestError::RateLimited {
            retry_after_ms: 1500,
            is_global: false,
        }
    );
    assert_eq!(compute_429_exponential_backoff_ms(1500, 0), 1500);
    assert_eq!(compute_429_exponential_backoff_ms(1500, 1), 3000);
    assert_eq!(compute_429_exponential_backoff_ms(1500, 2), 6000);
    assert_eq!(compute_429_exponential_backoff_ms(1500, 10), 30_000);
}

#[tokio::test]
async fn task_617_bff_normalizes_inline_query_tokens_and_extracts_typed_hit_groups() {
    let raw_query = "from:42 mentions:77 has:link has:file in:1163408591315812392 before:2000 after:1000 pinned:false author_type:bot link_hostname:discord.com attachment_extension:.RS attachment_filename:search.rs   полнотекстовый   поиск ";
    let params = SearchParams::from_raw_query(raw_query);

    assert_eq!(params.content.as_deref(), Some("полнотекстовый поиск"));
    assert_eq!(params.author_id.as_deref(), Some("42"));
    assert_eq!(params.mentions.as_deref(), Some("77"));
    assert_eq!(params.has.as_deref(), Some("link,file"));
    assert_eq!(params.channel_id.as_deref(), Some("1163408591315812392"));
    assert_eq!(params.min_id.as_deref(), Some("1000"));
    assert_eq!(params.max_id.as_deref(), Some("2000"));
    assert_eq!(params.pinned, Some(false));
    assert_eq!(params.author_type.as_deref(), Some("bot"));
    assert_eq!(params.link_hostname, Some(vec!["discord.com".into()]));
    assert_eq!(params.attachment_extension, Some(vec!["rs".into()]));
    assert_eq!(params.attachment_filename, Some(vec!["search.rs".into()]));

    let fake = Arc::new(FakeTransport::new(
        200,
        r#"{
            "analytics_id": "bff_617",
            "total_results": 1,
            "doing_deep_historical_index": false,
            "documents_indexed": 120,
            "messages": [
                [
                    {"id": "500", "channel_id": "1163408591315812392", "content": "до", "author": {"id": "1", "username": "u1"}, "timestamp": "2026-10-20T00:00:00Z"},
                    {"id": "501", "channel_id": "1163408591315812392", "content": "хит", "hit": true, "pinned": true, "author": {"id": "42", "username": "bot42"}, "timestamp": "2026-10-20T00:01:00Z", "attachments": [{"id": "a1", "filename": "search.rs", "size": 100}], "embeds": [{"type": "link"}]}
                ]
            ]
        }"#,
    ));
    let search = DiscordSearch::with_transport(fake.clone());
    let response = search
        .search("1014537851418980372", &params)
        .await
        .expect("search with normalized facets");

    let target = fake.target.lock().unwrap().clone().expect("target");
    assert!(target.contains("link_hostname=discord.com"), "{target}");
    assert!(target.contains("attachment_extension=rs"), "{target}");
    assert!(target.contains("attachment_filename=search.rs"), "{target}");

    let groups = response.hit_groups();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].messages.len(), 2);
    let primary = response.primary_hits();
    assert_eq!(primary.len(), 1);
    assert_eq!(primary[0].id, "501");
    assert_eq!(primary[0].author_id.as_deref(), Some("42"));
    assert_eq!(primary[0].author_username.as_deref(), Some("bot42"));
    assert_eq!(primary[0].content, "хит");
    assert!(primary[0].hit);
    assert!(primary[0].pinned);
    assert_eq!(primary[0].attachments_count, 1);
    assert_eq!(primary[0].embeds_count, 1);
}

#[tokio::test]
async fn task_617_bff_retries_http_429_with_exponential_backoff_and_succeeds_or_fails_after_budget()
{
    use chord_client::search::MAX_SEARCH_429_RETRIES;
    assert_eq!(MAX_SEARCH_429_RETRIES, 3);

    // Сценарий A: два ответа 429 подряд (база 500 мс -> 500 мс, затем 1000 мс), затем 200 OK.
    let transport_ok = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(429, &[], r#"{"retry_after":0.5}"#),
        QueuedTransport::resp(429, &[], r#"{"retry_after":0.5}"#),
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[[{"id":"m429","content":"ok after 429","hit":true}]],"total_results":1,"doing_deep_historical_index":false}"#,
        ),
    ]));
    let search_ok = DiscordSearch::with_transport(transport_ok.clone());
    let res = search_ok
        .execute_with_retry(
            "1014537851418980372",
            SearchParams {
                content: Some("rate-limit".into()),
                ..SearchParams::default()
            },
        )
        .await
        .expect("200 OK after two 429 retries");
    assert_eq!(res.total_results, 1);
    assert_eq!(
        *transport_ok.sleeps_ms.lock().unwrap(),
        vec![500, 1000],
        "ожидался экспоненциальный бэкофф 500ms, 1000ms для повторных 429"
    );
    assert_eq!(transport_ok.targets.lock().unwrap().len(), 3);

    // Сценарий B: исчерпание бюджета повторов 429 (1 начальный + 3 повтора = 4 ответа 429).
    let transport_fail = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(429, &[], r#"{"retry_after":1.0}"#),
        QueuedTransport::resp(429, &[], r#"{"retry_after":1.0}"#),
        QueuedTransport::resp(429, &[], r#"{"retry_after":1.0}"#),
        QueuedTransport::resp(429, &[], r#"{"retry_after":1.0}"#),
    ]));
    let search_fail = DiscordSearch::with_transport(transport_fail.clone());
    let err = search_fail
        .execute_with_retry("1014537851418980372", SearchParams::default())
        .await
        .expect_err("must fail after exhausting 429 retry budget");
    assert_eq!(
        err,
        SearchError::RateLimited {
            retry_after_ms: 1000
        }
    );
    assert_eq!(
        *transport_fail.sleeps_ms.lock().unwrap(),
        vec![1000, 2000, 4000]
    );
}

#[tokio::test]
async fn task_617_bff_caches_recent_200_search_results_and_bypasses_cache_on_202_or_invalidate() {
    let transport = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[[{"id":"c1","content":"cached hit","hit":true}]],"total_results":1,"doing_deep_historical_index":false,"documents_indexed":500}"#,
        ),
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[[{"id":"c2","content":"after invalidate","hit":true}]],"total_results":2,"doing_deep_historical_index":false,"documents_indexed":501}"#,
        ),
    ]));
    let search = DiscordSearch::with_transport(transport.clone());

    let p1 = SearchParams::from_raw_query("from:42 has:image кэш");
    let r1 = search
        .search_cached("1014537851418980372", &p1)
        .await
        .expect("first search populates cache");
    assert_eq!(r1.total_results, 1);
    assert_eq!(search.cache_len(), 1);
    assert_eq!(transport.targets.lock().unwrap().len(), 1);

    // Повторный запрос с другим порядком инлайн-токенов и новым search_query_id попадает в кэш.
    let mut p2 = SearchParams::from_raw_query("has:image from:42   кэш  ");
    p2.search_query_id = Some(deterministic_uuid_v4(999));
    let r2 = search
        .search_cached("1014537851418980372", &p2)
        .await
        .expect("second search hits cache");
    assert_eq!(r2, r1);
    assert_eq!(
        transport.targets.lock().unwrap().len(),
        1,
        "попадание в кэш не должно вызывать транспорт повторно"
    );

    // Инвалидация кэша гильдии приводит к новому сетевому вызову.
    search.invalidate_guild_cache("1014537851418980372");
    assert_eq!(search.cache_len(), 0);
    let r3 = search
        .search_cached("1014537851418980372", &p1)
        .await
        .expect("third search after invalidation");
    assert_eq!(r3.total_results, 2);
    assert_eq!(transport.targets.lock().unwrap().len(), 2);

    // Ответ 202 (indexing) НЕ кэшируется.
    let transport_202 = Arc::new(QueuedTransport::new(vec![
        QueuedTransport::resp(202, &[("Retry-After", "2")], "{}"),
        QueuedTransport::resp(
            200,
            &[],
            r#"{"messages":[],"total_results":7,"doing_deep_historical_index":false}"#,
        ),
    ]));
    let search_202 = DiscordSearch::with_transport(transport_202.clone());
    let env_202 = search_202
        .search_cached("1", &SearchParams::from_raw_query("индекс"))
        .await
        .expect("202 response");
    assert!(env_202.indexing);
    assert_eq!(
        search_202.cache_len(),
        0,
        "202 Accepted не должен кэшироваться"
    );

    let after_202 = search_202
        .search_cached("1", &SearchParams::from_raw_query("индекс"))
        .await
        .expect("200 after uncached 202");
    assert_eq!(after_202.total_results, 7);
    assert_eq!(search_202.cache_len(), 1);
}
