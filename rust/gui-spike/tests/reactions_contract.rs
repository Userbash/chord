use discord_gui_spike::reactions::{
    percent_encode_emoji, toggle_reaction, DiscordReactionApi, ReactionApi, ReactionError,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

type ReactionCall = (String, String, String, bool);

#[derive(Clone)]
struct FakeReactionApi {
    result: Result<(), ReactionError>,
    calls: Arc<Mutex<Vec<ReactionCall>>>,
}

impl ReactionApi for FakeReactionApi {
    fn toggle_reaction<'a>(
        &'a self,
        channel_id: &'a str,
        message_id: &'a str,
        emoji: &'a str,
        active: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(), ReactionError>> + Send + 'a>> {
        self.calls.lock().unwrap().push((
            channel_id.to_string(),
            message_id.to_string(),
            emoji.to_string(),
            active,
        ));
        let result = self.result.clone();
        Box::pin(async move { result })
    }
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn fake(result: Result<(), ReactionError>) -> FakeReactionApi {
    FakeReactionApi {
        result,
        calls: Arc::new(Mutex::new(Vec::new())),
    }
}

#[test]
fn active_selects_put_and_inactive_selects_delete() {
    let api = fake(Ok(()));
    assert_eq!(run(toggle_reaction(&api, "123", "456", "👍", true)), Ok(()));
    assert_eq!(
        run(toggle_reaction(&api, "123", "456", "👍", false)),
        Ok(())
    );
    assert_eq!(
        *api.calls.lock().unwrap(),
        vec![
            ("123".into(), "456".into(), "👍".into(), true),
            ("123".into(), "456".into(), "👍".into(), false),
        ]
    );
}

#[test]
fn invalid_ids_and_emoji_are_rejected_before_api() {
    let api = fake(Ok(()));
    for (channel, message, emoji) in [
        ("", "1", "👍"),
        ("abc", "1", "👍"),
        ("1", "", "👍"),
        ("1", "abc", "👍"),
        ("1", "2", ""),
        ("1", "2", "x".repeat(257).as_str()),
    ] {
        assert_eq!(
            run(toggle_reaction(&api, channel, message, emoji, true)),
            Err(ReactionError::InvalidArg)
        );
    }
    assert!(api.calls.lock().unwrap().is_empty());
}

#[test]
fn emoji_path_uses_percent_encoding_without_plain_unicode() {
    assert_eq!(percent_encode_emoji("👍"), "%F0%9F%91%8D");
    assert_eq!(percent_encode_emoji("party:123"), "party%3A123");
    assert_eq!(percent_encode_emoji("a/b"), "a%2Fb");
}

#[test]
fn typed_failures_are_preserved() {
    for error in [
        ReactionError::Unauthorized,
        ReactionError::Forbidden,
        ReactionError::NotFound,
        ReactionError::RateLimited {
            retry_after_ms: 250,
        },
        ReactionError::Client { status: 400 },
        ReactionError::Server { status: 500 },
        ReactionError::Network,
        ReactionError::Storage,
        ReactionError::InvalidResponse,
    ] {
        let api = fake(Err(error.clone()));
        assert_eq!(run(toggle_reaction(&api, "1", "2", "👍", true)), Err(error));
    }
    // SEC-02: отказ хранилища — собственный kind, а не «сеть».
    let value = serde_json::to_value(ReactionError::Storage).expect("storage");
    assert_eq!(value["kind"], "storage", "{value}");
    assert!(!ReactionError::Storage.to_string().is_empty());
}

#[test]
fn sender_debug_and_empty_token_do_not_expose_or_accept_token() {
    assert!(matches!(
        DiscordReactionApi::new(""),
        Err(ReactionError::InvalidArg)
    ));
    let sender = DiscordReactionApi::new("secret-token").expect("sender config");
    assert!(!format!("{sender:?}").contains("secret-token"));
}

#[test]
fn tauri_command_reads_token_from_backend_and_is_registered() {
    let source = include_str!("../src/main.rs");
    assert!(source.contains("backend.token()"));
    let start = source.find("generate_handler![").expect("handler") + "generate_handler![".len();
    let end = source[start..].find(']').expect("handler close");
    let commands = &source[start..start + end];
    assert!(commands
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .any(|name| name == "toggle_reaction"));
}
