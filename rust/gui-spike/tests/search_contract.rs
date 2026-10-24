use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::search::{
    DiscordSearch, RemoteSearch, SearchError, SearchParams, SearchTransport,
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
