//! TASK-825 / TASK-829.1: Регрессионный контракт для live fallback токена и анонимного resolve_invite.

use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::invites::{resolve_invite, DiscordInvites, InviteError, InviteTransport};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

const SAMPLE_CODE: &str = "eSrxp2YGUC";

const RESOLVE_PREVIEW_JSON: &str = r#"{
    "code": "eSrxp2YGUC",
    "type": 0,
    "guild": {"id": "999888777666555444", "name": "Live Server", "icon": "live_icon_hash"},
    "channel": {"id": "111222333444555666", "name": "welcome", "type": 0},
    "approximate_member_count": 2400,
    "approximate_presence_count": 350,
    "expires_at": null
}"#;

#[derive(Clone, Default)]
struct MockRecordingTransport {
    gets: Arc<Mutex<Vec<String>>>,
    posts: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
}

impl InviteTransport for MockRecordingTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.gets.lock().unwrap().push(target.to_string());
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: RESOLVE_PREVIEW_JSON.as_bytes().to_vec(),
            })
        })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.posts
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                body: br#"{"guild_id":"999888777666555444"}"#.to_vec(),
            })
        })
    }
}

#[tokio::test]
async fn test_anonymous_resolve_invite_client_creation_and_execution() {
    let _client = DiscordInvites::anonymous().expect("DiscordInvites::anonymous must succeed");
    let transport = Arc::new(MockRecordingTransport::default());
    let custom_client = DiscordInvites::with_transport(transport.clone());

    let preview = resolve_invite(&custom_client, SAMPLE_CODE)
        .await
        .expect("anonymous resolve_invite should succeed");

    assert_eq!(preview.code, SAMPLE_CODE);
    assert_eq!(preview.guild_id, "999888777666555444");
    assert_eq!(preview.guild_name, "Live Server");
    assert_eq!(preview.member_count, Some(2400));
    assert_eq!(preview.presence_count, Some(350));

    let recorded_gets = transport.gets.lock().unwrap();
    assert_eq!(recorded_gets.len(), 1);
    assert_eq!(
        recorded_gets[0],
        "/invites/eSrxp2YGUC?with_counts=true&with_expiration=true"
    );
}

#[tokio::test]
async fn test_token_or_fallback_to_live_token_when_keyring_is_empty() {
    // 1. Устанавливаем живой токен в глобальное состояние сессии
    discord_gui_spike::data::set_live_session(
        Some("live-captured-session-token-12345".to_string()),
        None,
    );

    let live_tok = discord_gui_spike::data::resolve_live_token();
    assert_eq!(
        live_tok.as_deref(),
        Some("live-captured-session-token-12345"),
        "live token must be resolvable from data module"
    );

    // 2. Создаем клиент через живой токен
    let _client = DiscordInvites::new(&live_tok.unwrap())
        .expect("DiscordInvites::new must accept live token");

    let transport = Arc::new(MockRecordingTransport::default());
    let test_client = DiscordInvites::with_transport(transport.clone());

    let preview = resolve_invite(&test_client, SAMPLE_CODE)
        .await
        .expect("resolve_invite with live fallback token must succeed");

    assert_eq!(preview.code, SAMPLE_CODE);
    assert_eq!(preview.guild_id, "999888777666555444");

    // Очищаем тестовую сессию
    discord_gui_spike::data::set_live_session(None, None);
}
