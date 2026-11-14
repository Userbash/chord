//! RW-40 / INT-01-RUST-WIRING: контрактный тест публичных IPC-обёрток и Tauri-диспетчеризации
//! в `discord-gui-spike` для:
//! - `attachments`: `request_upload_slots` (`POST /channels/{id}/attachments`)
//! - `invites`: `create_channel_invite` (`POST /channels/{id}/invites`)
//! - `pins_threads`:
//!   - `list_channel_pins` (`GET /channels/{id}/pins`)
//!   - `pin_channel_message` (`PUT /channels/{id}/pins/{message_id}`)
//!   - `unpin_channel_message` (`DELETE /channels/{id}/pins/{message_id}`)
//!   - `list_channel_threads` (`GET /channels/{id}/threads/active` и `/threads/archived/public`)

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use chord_core::rest::transport::HttpResponse;
use chord_client::attachments::{
    AttachmentError, AttachmentFileSpec, AttachmentTransport, AttachmentUploadSlot,
    DiscordAttachments, MAX_UPLOAD_FILE_BYTES,
};
use chord_client::invites::{
    CreateInviteParams, CreatedInvite, DiscordInvites, InviteError, InviteTransport,
    INVITE_FLAG_IS_GUEST,
};
use chord_client::messages::{send_message, MessageError, MessageReceipt, MessageSender};
use chord_client::notifications::{
    ChannelNotificationOverridePatch, DiscordNotifications, GuildNotificationSettingsPatch,
    NotificationError, NotificationTransport,
};
use chord_client::pins_threads::{
    DiscordPinsThreads, PinnedMessage, PinsThreadsError, PinsThreadsTransport, ThreadSummary,
};
use chord_client::{
    create_channel_invite, list_channel_pins, list_channel_threads, pin_channel_message,
    request_upload_slots, unpin_channel_message, update_channel_notification_override,
    update_guild_notification_settings, IPC_WIRING_COMMANDS,
};

const CHANNEL_ID: &str = "1014537851418980375";
const MESSAGE_ID: &str = "1293192721237938187";

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime")
        .block_on(future)
}

// ---------------------------------------------------------------------------
// Fakes для независимой проверки маршрутов и тел запросов
// ---------------------------------------------------------------------------

type PostLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

#[derive(Clone, Default)]
struct FakeAttachmentTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    posts: PostLog,
}

impl FakeAttachmentTransport {
    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            posts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    fn posts(&self) -> Vec<(String, Vec<u8>)> {
        self.posts.lock().unwrap().clone()
    }
}

impl AttachmentTransport for FakeAttachmentTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        self.posts
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Clone, Default)]
struct FakeInviteTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    gets: Arc<Mutex<Vec<String>>>,
    posts: PostLog,
}

impl FakeInviteTransport {
    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            gets: Arc::new(Mutex::new(Vec::new())),
            posts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn posts(&self) -> Vec<(String, Vec<u8>)> {
        self.posts.lock().unwrap().clone()
    }
}

impl InviteTransport for FakeInviteTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, InviteError>> + Send + 'a>> {
        self.gets.lock().unwrap().push(target.to_string());
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
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
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PinsCall {
    Get(String),
    Put(String, Vec<u8>),
    Delete(String),
}

#[derive(Clone, Default)]
struct FakePinsThreadsTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    calls: Arc<Mutex<Vec<PinsCall>>>,
}

impl FakePinsThreadsTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<PinsCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl PinsThreadsTransport for FakePinsThreadsTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(PinsCall::Get(target.to_string()));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(PinsCall::Put(target.to_string(), body.to_vec()));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, PinsThreadsError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push(PinsCall::Delete(target.to_string()));
        let response = HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[derive(Default)]
struct DummyMessageSender;

impl MessageSender for DummyMessageSender {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>> {
        let receipt = MessageReceipt {
            id: "1".to_string(),
            channel_id: channel_id.to_string(),
            content: content.to_string(),
            nonce: nonce.to_string(),
        };
        Box::pin(async move { Ok(receipt) })
    }
}

// ---------------------------------------------------------------------------
// 1. Тест публичной IPC-обёртки attachments (request_upload_slots)
// ---------------------------------------------------------------------------

#[test]
fn ipc_wrapper_request_upload_slots_delegates_and_enforces_contract() {
    let response_body = br#"{
        "attachments": [
            {
                "id": "loc-1",
                "upload_url": "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1",
                "upload_filename": "c1/loc-1/report.txt"
            }
        ]
    }"#;
    let transport = FakeAttachmentTransport::with_body(200, response_body);
    let client = DiscordAttachments::with_transport(Arc::new(transport.clone()));
    let files = vec![AttachmentFileSpec {
        filename: "report.txt".to_string(),
        file_size: 4096,
        id: "loc-1".to_string(),
        is_clip: false,
    }];

    let slots = run(request_upload_slots(&client, CHANNEL_ID, &files)).expect("upload slots");
    assert_eq!(
        slots,
        vec![AttachmentUploadSlot {
            id: "loc-1".to_string(),
            upload_url: "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1"
                .to_string(),
            upload_filename: "c1/loc-1/report.txt".to_string(),
        }]
    );

    let posts = transport.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, format!("/channels/{CHANNEL_ID}/attachments"));
    let wire_json: serde_json::Value = serde_json::from_slice(&posts[0].1).expect("JSON body");
    assert_eq!(
        wire_json,
        serde_json::json!({
            "files": [
                {
                    "filename": "report.txt",
                    "file_size": 4096,
                    "id": "loc-1",
                    "is_clip": false
                }
            ]
        })
    );

    // Предсетевая валидация невалидного channel_id, пустого списка и превышения размера:
    let guard_transport = FakeAttachmentTransport::with_body(200, response_body);
    assert_eq!(
        run(request_upload_slots(&guard_transport, "../bad", &files)),
        Err(AttachmentError::InvalidArg)
    );
    assert_eq!(
        run(request_upload_slots(&guard_transport, CHANNEL_ID, &[])),
        Err(AttachmentError::InvalidArg)
    );
    let oversized = vec![AttachmentFileSpec {
        filename: "huge.bin".to_string(),
        file_size: MAX_UPLOAD_FILE_BYTES + 1,
        id: "loc-2".to_string(),
        is_clip: false,
    }];
    assert_eq!(
        run(request_upload_slots(
            &guard_transport,
            CHANNEL_ID,
            &oversized
        )),
        Err(AttachmentError::EntityTooLarge)
    );
    assert!(
        guard_transport.posts().is_empty(),
        "предсетевая проверка обязана отсекать невалидные запросы до транспорта"
    );

    // 429 retry-after в мс:
    let rate_limited =
        FakeAttachmentTransport::with_body(429, b"{}").with_headers(&[("Retry-After", "2.5")]);
    assert_eq!(
        run(request_upload_slots(&rate_limited, CHANNEL_ID, &files)),
        Err(AttachmentError::RateLimited {
            retry_after_ms: 2500
        })
    );
}

// ---------------------------------------------------------------------------
// 2. Тест публичной IPC-обёртки invites (create_channel_invite)
// ---------------------------------------------------------------------------

#[test]
fn ipc_wrapper_create_channel_invite_delegates_and_enforces_re_tables() {
    let response_body = br#"{
        "code": "Inv2026",
        "max_age": 1800,
        "max_uses": 5,
        "temporary": true
    }"#;
    let transport = FakeInviteTransport::with_body(200, response_body);
    let client = DiscordInvites::with_transport(Arc::new(transport.clone()));
    let custom_params = CreateInviteParams {
        max_age: 1800,
        max_uses: 5,
        temporary: true,
        flags: Some(INVITE_FLAG_IS_GUEST),
    };

    let created =
        run(create_channel_invite(&client, CHANNEL_ID, &custom_params)).expect("created invite");
    assert_eq!(
        created,
        CreatedInvite {
            code: "Inv2026".to_string(),
            max_age: 1800,
            max_uses: 5,
            temporary: true,
        }
    );

    let posts = transport.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, format!("/channels/{CHANNEL_ID}/invites"));
    let wire_json: serde_json::Value = serde_json::from_slice(&posts[0].1).expect("JSON body");
    assert_eq!(
        wire_json,
        serde_json::json!({
            "max_age": 1800,
            "max_uses": 5,
            "temporary": true,
            "flags": 1
        })
    );

    // Невалидный channel_id и неразрешённые значения max_age/max_uses режутся до сети:
    let guard_transport = FakeInviteTransport::with_body(200, response_body);
    assert_eq!(
        run(create_channel_invite(
            &guard_transport,
            "../bad",
            &CreateInviteParams::default()
        )),
        Err(InviteError::InvalidRequest)
    );
    assert_eq!(
        run(create_channel_invite(
            &guard_transport,
            CHANNEL_ID,
            &CreateInviteParams {
                max_age: 999,
                ..CreateInviteParams::default()
            }
        )),
        Err(InviteError::InvalidRequest)
    );
    assert_eq!(
        run(create_channel_invite(
            &guard_transport,
            CHANNEL_ID,
            &CreateInviteParams {
                max_uses: 7,
                ..CreateInviteParams::default()
            }
        )),
        Err(InviteError::InvalidRequest)
    );
    assert!(
        guard_transport.posts().is_empty(),
        "невалидные параметры create_channel_invite не должны доходить до транспорта"
    );
}

// ---------------------------------------------------------------------------
// 3. Тест публичных IPC-обёрток pins_threads (list_channel_pins,
//    pin_channel_message, unpin_channel_message, list_channel_threads)
// ---------------------------------------------------------------------------

#[test]
fn ipc_wrapper_pins_and_threads_route_get_put_delete_and_archived_flag() {
    let pins_body = r#"[
        {
            "id": "9001",
            "channel_id": "1014537851418980375",
            "author": {"id": "u1", "username": "alice", "global_name": "Alice Wonder"},
            "content": "Релиз RW-40",
            "timestamp": "2026-10-30T12:00:00.000Z",
            "pinned": true
        }
    ]"#
    .as_bytes();
    let pins_transport = FakePinsThreadsTransport::with_body(200, pins_body);
    let pins_client = DiscordPinsThreads::with_transport(Arc::new(pins_transport.clone()));
    let pins = run(list_channel_pins(&pins_client, CHANNEL_ID)).expect("list_channel_pins");
    assert_eq!(
        pins,
        vec![PinnedMessage {
            id: "9001".to_string(),
            channel_id: CHANNEL_ID.to_string(),
            author: "Alice Wonder".to_string(),
            content: "Релиз RW-40".to_string(),
            timestamp: "2026-10-30T12:00:00.000Z".to_string(),
            pinned: true,
        }]
    );
    assert_eq!(
        pins_transport.calls(),
        vec![PinsCall::Get(format!("/channels/{CHANNEL_ID}/pins"))]
    );

    // pin_channel_message (PUT) и unpin_channel_message (DELETE):
    let mutating_transport = FakePinsThreadsTransport::answering(204);
    let mutating_client = DiscordPinsThreads::with_transport(Arc::new(mutating_transport.clone()));
    assert_eq!(
        run(pin_channel_message(
            &mutating_client,
            CHANNEL_ID,
            MESSAGE_ID
        )),
        Ok(())
    );
    assert_eq!(
        run(unpin_channel_message(
            &mutating_client,
            CHANNEL_ID,
            MESSAGE_ID
        )),
        Ok(())
    );
    assert_eq!(
        mutating_transport.calls(),
        vec![
            PinsCall::Put(
                format!("/channels/{CHANNEL_ID}/pins/{MESSAGE_ID}"),
                Vec::new()
            ),
            PinsCall::Delete(format!("/channels/{CHANNEL_ID}/pins/{MESSAGE_ID}")),
        ]
    );

    // list_channel_threads: archived == false -> /threads/active, archived == true -> /threads/archived/public
    let active_threads_body = r#"{
        "threads": [
            {
                "id": "th-active-1",
                "parent_id": "1014537851418980375",
                "name": "Активная ветка",
                "message_count": 12,
                "member_count": 4,
                "thread_metadata": {"archived": false, "locked": false},
                "last_message_id": "m-12"
            }
        ]
    }"#
    .as_bytes();
    let active_transport = FakePinsThreadsTransport::with_body(200, active_threads_body);
    let active_threads =
        run(list_channel_threads(&active_transport, CHANNEL_ID, false)).expect("active threads");
    assert_eq!(
        active_threads,
        vec![ThreadSummary {
            id: "th-active-1".to_string(),
            parent_id: CHANNEL_ID.to_string(),
            name: "Активная ветка".to_string(),
            message_count: 12,
            member_count: 4,
            archived: false,
            locked: false,
            last_message_id: Some("m-12".to_string()),
        }]
    );
    assert_eq!(
        active_transport.calls(),
        vec![PinsCall::Get(format!(
            "/channels/{CHANNEL_ID}/threads/active"
        ))]
    );

    let archived_threads_body = r#"{
        "threads": [
            {
                "id": "th-arch-1",
                "parent_id": "1014537851418980375",
                "name": "Архивная ветка",
                "message_count": 30,
                "member_count": 8,
                "thread_metadata": {"archived": true, "locked": true}
            }
        ]
    }"#
    .as_bytes();
    let archived_transport = FakePinsThreadsTransport::with_body(200, archived_threads_body);
    let archived_threads =
        run(list_channel_threads(&archived_transport, CHANNEL_ID, true)).expect("archived threads");
    assert_eq!(
        archived_threads,
        vec![ThreadSummary {
            id: "th-arch-1".to_string(),
            parent_id: CHANNEL_ID.to_string(),
            name: "Архивная ветка".to_string(),
            message_count: 30,
            member_count: 8,
            archived: true,
            locked: true,
            last_message_id: None,
        }]
    );
    assert_eq!(
        archived_transport.calls(),
        vec![PinsCall::Get(format!(
            "/channels/{CHANNEL_ID}/threads/archived/public"
        ))]
    );

    // Предсетевая валидация ID в паритете с messages::send_message:
    let guard_transport = FakePinsThreadsTransport::answering(204);
    let sender = DummyMessageSender;
    for sample in [CHANNEL_ID, "chan_1-A", "", "../..", "a/b", "a.b", "a b"] {
        let pin_err = run(pin_channel_message(&guard_transport, sample, MESSAGE_ID)).is_err();
        let msg_err = run(send_message(&sender, sample, "ok", "n1")).is_err();
        assert_eq!(
            pin_err, msg_err,
            "allowlist channel_id должен совпадать для {sample:?}"
        );
    }
    assert_eq!(
        run(pin_channel_message(&guard_transport, CHANNEL_ID, "../bad")),
        Err(PinsThreadsError::InvalidMessageId)
    );
    assert_eq!(
        run(unpin_channel_message(
            &guard_transport,
            CHANNEL_ID,
            "../bad"
        )),
        Err(PinsThreadsError::InvalidMessageId)
    );
}

// ---------------------------------------------------------------------------
// 4. Контракт подключения в `main.rs` (`#[tauri::command]` + `generate_handler!` + `token_or`)
// ---------------------------------------------------------------------------

fn command_body<'a>(source: &'a str, name: &str) -> &'a str {
    let marker = format!("async fn {name}(");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("main.rs должен объявлять команду {name}"));
    let rest = &source[start..];
    let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
    &rest[..end]
}

fn registered_commands(source: &str) -> Vec<&str> {
    let open = "generate_handler![";
    let start = source
        .find(open)
        .expect("main.rs объявляет invoke_handler!")
        + open.len();
    let block = &source[start..start + source[start..].find(']').expect("закрывающая ]")];
    block
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|name| !name.is_empty())
        .collect()
}

#[test]
fn tauri_main_registers_and_wires_all_six_ipc_commands_via_token_or() {
    assert_eq!(
        IPC_WIRING_COMMANDS,
        [
            "request_upload_slots",
            "create_channel_invite",
            "list_channel_pins",
            "pin_channel_message",
            "unpin_channel_message",
            "list_channel_threads",
        ]
    );

    let source = include_str!("../src/main.rs");
    let commands = registered_commands(source);

    for command in IPC_WIRING_COMMANDS {
        assert_eq!(
            commands.iter().filter(|name| **name == command).count(),
            1,
            "{command} должен быть зарегистрирован в generate_handler! ровно один раз"
        );
        let body = command_body(source, command);
        assert!(
            body.contains("token_or::<"),
            "{command}: токен обязан читаться через единый helper token_or: {body}"
        );
        assert_eq!(
            body.matches("backend.token()").count(),
            1,
            "{command}: ровно одно чтение backend.token(): {body}"
        );
        assert!(
            !body.contains("token: String") && !body.contains("bearer: String"),
            "{command}: токен никогда не передаётся из фронтенда: {body}"
        );
        assert!(
            !body.contains("println!") && !body.contains("eprintln!") && !body.contains("log::"),
            "{command}: токен и ошибки не логируются: {body}"
        );
    }

    let upload_body = command_body(source, "request_upload_slots");
    assert!(
        upload_body.contains("DiscordAttachments::new(&token)"),
        "request_upload_slots создаёт DiscordAttachments из токена: {upload_body}"
    );
    assert!(
        upload_body.contains("chord_client::request_upload_slots(&api, &channel_id, &files)"),
        "request_upload_slots делегирует в публичную обёртку крейта: {upload_body}"
    );

    let invite_body = command_body(source, "create_channel_invite");
    assert!(
        invite_body.contains("InviteError::Storage")
            && invite_body.contains("InviteError::MissingToken"),
        "create_channel_invite сохраняет доменные коды Storage/MissingToken: {invite_body}"
    );
    assert!(
        invite_body.contains("DiscordInvites::new(&token)"),
        "create_channel_invite создаёт DiscordInvites из токена: {invite_body}"
    );
    assert!(
        invite_body
            .contains("chord_client::create_channel_invite(&api, &channel_id, &params)"),
        "create_channel_invite делегирует в публичную обёртку крейта: {invite_body}"
    );

    for (cmd, expected_call) in [
        (
            "list_channel_pins",
            "chord_client::list_channel_pins(&api, &channel_id)",
        ),
        (
            "pin_channel_message",
            "chord_client::pin_channel_message(&api, &channel_id, &message_id)",
        ),
        (
            "unpin_channel_message",
            "chord_client::unpin_channel_message(&api, &channel_id, &message_id)",
        ),
        (
            "list_channel_threads",
            "chord_client::list_channel_threads(&api, &channel_id, archived)",
        ),
    ] {
        let body = command_body(source, cmd);
        assert!(
            body.contains("PinsThreadsError::Storage")
                && body.contains("PinsThreadsError::MissingToken"),
            "{cmd} сохраняет доменные коды Storage/MissingToken: {body}"
        );
        assert!(
            body.contains("DiscordPinsThreads::new(&token)"),
            "{cmd} создаёт DiscordPinsThreads из токена: {body}"
        );
        assert!(
            body.contains(expected_call),
            "{cmd} делегирует в {expected_call}: {body}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Сериализация тегов ошибок для фронтенда и защита секрета в Debug
// ---------------------------------------------------------------------------

#[test]
fn ipc_error_kinds_serialize_to_expected_frontend_tags_without_leaking_secrets() {
    let kind_of = |val: serde_json::Value| -> String {
        val.get("kind")
            .and_then(|k| k.as_str())
            .expect("поле kind")
            .to_string()
    };

    assert_eq!(
        kind_of(serde_json::to_value(AttachmentError::InvalidArg).unwrap()),
        "invalidArg"
    );
    assert_eq!(
        kind_of(serde_json::to_value(AttachmentError::EntityTooLarge).unwrap()),
        "entityTooLarge"
    );
    assert_eq!(
        kind_of(serde_json::to_value(AttachmentError::Unauthorized).unwrap()),
        "unauthorized"
    );
    assert_eq!(
        kind_of(serde_json::to_value(InviteError::InvalidRequest).unwrap()),
        "invalidRequest"
    );
    assert_eq!(
        kind_of(serde_json::to_value(InviteError::Storage).unwrap()),
        "storage"
    );
    assert_eq!(
        kind_of(serde_json::to_value(InviteError::MissingToken).unwrap()),
        "missingToken"
    );
    assert_eq!(
        kind_of(serde_json::to_value(PinsThreadsError::InvalidChannelId).unwrap()),
        "invalidChannelId"
    );
    assert_eq!(
        kind_of(serde_json::to_value(PinsThreadsError::InvalidMessageId).unwrap()),
        "invalidMessageId"
    );
    assert_eq!(
        kind_of(serde_json::to_value(PinsThreadsError::Storage).unwrap()),
        "storage"
    );
    assert_eq!(
        kind_of(serde_json::to_value(PinsThreadsError::MissingToken).unwrap()),
        "missingToken"
    );

    let secret = "super-secret-bearer-token-42";
    let attach_client = DiscordAttachments::new(secret).expect("attach client");
    let invite_client = DiscordInvites::new(secret).expect("invite client");
    let pins_client = DiscordPinsThreads::new(secret).expect("pins client");
    assert!(!format!("{attach_client:?}").contains(secret));
    assert!(!format!("{invite_client:?}").contains(secret));
    assert!(!format!("{pins_client:?}").contains(secret));
}

// ---------------------------------------------------------------------------
// 6. RW-40 / INT-05-IPC-ALIASES: выравнивание F-1 IPC-алиасов UI ↔ Rust main.rs
// ---------------------------------------------------------------------------

const F1_UI_IPC_ALIASES: [&str; 4] = [
    "attachments_upload_slots",
    "create_invite",
    "pins_list",
    "threads_list",
];

const F5_NOTIFICATIONS_IPC_COMMANDS: [&str; 2] = [
    "update_guild_notification_settings",
    "update_channel_notification_override",
];

const UI_IPC_COMMAND_CONSTANTS: [&str; 4] = [
    "AUTH_COMMANDS",
    "JOIN_COMMANDS",
    "NET_COMMANDS",
    "READY_COMMAND",
];

const ALL_47_UI_IPC_COMMANDS: [&str; 47] = [
    "accept_invite",
    "ack_message",
    "attachments_upload_put",
    "attachments_upload_slots",
    "auth_login_password",
    "auth_login_token",
    "auth_logout",
    "auth_mfa",
    "auth_restore",
    "auth_sms",
    "auth_status",
    "channels",
    "create_gift_code",
    "create_invite",
    "delete_message",
    "edit_message",
    "join_guild",
    "leave_guild",
    "list_nitro_gift_plans",
    "members_page",
    "messages_page",
    "net_connect",
    "net_status",
    "pins_list",
    "process_metrics",
    "read_state",
    "ready",
    "redeem_gift_code",
    "reply_message",
    "report_bench",
    "report_ui_snapshot",
    "resolve_gift_code",
    "resolve_invite",
    "search_remote",
    "send_message",
    "send_typing",
    "settings_load",
    "settings_save",
    "spike_config",
    "threads_list",
    "toggle_reaction",
    "update_channel_notification_override",
    "update_guild_notification_settings",
    "voice_desktop_sources",
    "voice_set_audio_device",
    "voice_stream_start",
    "voice_stream_stop",
];

fn is_valid_ipc_command_name(cmd: &str) -> bool {
    !cmd.is_empty()
        && cmd
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn extract_quoted_command_literals<'a>(segment: &'a str, out: &mut Vec<&'a str>) {
    for quote in ['"', '\''] {
        let mut rest = segment;
        while let Some(start) = rest.find(quote) {
            let after = &rest[start + 1..];
            if let Some(end) = after.find(quote) {
                let candidate = &after[..end];
                if is_valid_ipc_command_name(candidate) {
                    out.push(candidate);
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }
}

fn extract_ui_invoke_commands(js_source: &str) -> Vec<&str> {
    let mut out = Vec::new();
    for prefix in [
        "invoke(\"",
        "invoke('",
        "invokeFn(\"",
        "invokeFn('",
        "invokeReaction(\"",
        "invokeReaction('",
    ] {
        let quote = prefix.chars().last().expect("quote");
        let mut rest = js_source;
        while let Some(pos) = rest.find(prefix) {
            let after = &rest[pos + prefix.len()..];
            if let Some(end) = after.find(quote) {
                let cmd = &after[..end];
                if is_valid_ipc_command_name(cmd) {
                    out.push(cmd);
                }
                rest = &after[end + 1..];
            } else {
                break;
            }
        }
    }

    for const_name in UI_IPC_COMMAND_CONSTANTS {
        let decl_marker = format!("const {const_name}");
        let mut rest = js_source;
        while let Some(pos) = rest.find(&decl_marker) {
            let after_decl = &rest[pos + decl_marker.len()..];
            let Some(eq_pos) = after_decl.find('=') else {
                break;
            };
            let rhs = &after_decl[eq_pos + 1..];
            let stmt_end = rhs.find(';').unwrap_or(rhs.len());
            let stmt = &rhs[..stmt_end];
            if let Some(open_brace) = stmt.find('{') {
                if let Some(close_brace) = stmt[open_brace + 1..].find('}') {
                    let body = &stmt[open_brace + 1..open_brace + 1 + close_brace];
                    extract_quoted_command_literals(body, &mut out);
                }
            } else {
                extract_quoted_command_literals(stmt, &mut out);
            }
            rest = &rhs[stmt_end..];
        }
    }

    out
}

fn collect_ui_js_files(
    dir: &std::path::Path,
    root: &std::path::Path,
    out: &mut Vec<(String, String)>,
) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("не удалось прочитать каталог {}: {e}", dir.display()))
        .map(|entry| entry.expect("dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_ui_js_files(&path, root, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("js") {
            let rel = path
                .strip_prefix(root)
                .expect("относительный путь")
                .to_string_lossy()
                .replace('\\', "/");
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("не удалось прочитать {}: {e}", path.display()));
            out.push((rel, content));
        }
    }
}

#[test]
fn tauri_main_registers_and_wires_f1_ui_ipc_aliases_via_token_or() {
    let source = include_str!("../src/main.rs");
    let head = source
        .split("mod wiring_tests")
        .next()
        .expect("голова main.rs");
    let commands = registered_commands(head);

    for command in F1_UI_IPC_ALIASES {
        assert_eq!(
            commands.iter().filter(|name| **name == command).count(),
            1,
            "{command} должен быть зарегистрирован в generate_handler! ровно один раз"
        );
        let body = command_body(head, command);
        assert!(
            body.contains("token_or::<"),
            "{command}: токен обязан читаться через единый helper token_or: {body}"
        );
        assert_eq!(
            body.matches("backend.token()").count(),
            1,
            "{command}: ровно одно чтение backend.token(): {body}"
        );
        assert!(
            !body.contains("token: String") && !body.contains("bearer: String"),
            "{command}: токен никогда не передаётся из фронтенда: {body}"
        );
        assert!(
            !body.contains("println!") && !body.contains("eprintln!") && !body.contains("log::"),
            "{command}: токен и ошибки не логируются: {body}"
        );
    }

    // 1) attachments_upload_slots (composer.js:725 -> request_upload_slots + {attachments} envelope):
    let slots_alias_body = command_body(head, "attachments_upload_slots");
    assert!(
        slots_alias_body.contains("AttachmentError::Network")
            && slots_alias_body.contains("AttachmentError::Unauthorized"),
        "attachments_upload_slots сохраняет коды AttachmentError::Network/Unauthorized: {slots_alias_body}"
    );
    assert!(
        slots_alias_body.contains("DiscordAttachments::new(&token)"),
        "attachments_upload_slots создаёт DiscordAttachments из токена: {slots_alias_body}"
    );
    assert!(
        slots_alias_body.contains("chord_client::request_upload_slots(&api, &channel_id, &specs)"),
        "attachments_upload_slots делегирует в chord_client::request_upload_slots: {slots_alias_body}"
    );
    assert!(
        slots_alias_body.contains("UploadSlotsEnvelope { attachments }"),
        "attachments_upload_slots оборачивает ответ в конверт {{ attachments }} для composer.js:737: {slots_alias_body}"
    );

    // 2) create_invite (invite-modal.js:370 -> create_channel_invite, плоский и вложенный payload):
    let create_invite_body = command_body(head, "create_invite");
    assert!(
        create_invite_body.contains("InviteError::Storage")
            && create_invite_body.contains("InviteError::MissingToken"),
        "create_invite сохраняет доменные коды Storage/MissingToken: {create_invite_body}"
    );
    assert!(
        create_invite_body.contains("DiscordInvites::new(&token)"),
        "create_invite создаёт DiscordInvites из токена: {create_invite_body}"
    );
    assert!(
        create_invite_body
            .contains("CreateInviteParamsInput::resolve(params, max_age, max_uses, temporary, flags)"),
        "create_invite поддерживает плоский и вложенный payload из invite-modal.js: {create_invite_body}"
    );
    assert!(
        create_invite_body
            .contains("chord_client::create_channel_invite(&api, &channel_id, &resolved)"),
        "create_invite делегирует в chord_client::create_channel_invite: {create_invite_body}"
    );

    // 3) pins_list (app.js:1372 -> list_channel_pins):
    let pins_alias_body = command_body(head, "pins_list");
    assert!(
        pins_alias_body.contains("PinsThreadsError::Storage")
            && pins_alias_body.contains("PinsThreadsError::MissingToken"),
        "pins_list сохраняет доменные коды Storage/MissingToken: {pins_alias_body}"
    );
    assert!(
        pins_alias_body.contains("DiscordPinsThreads::new(&token)"),
        "pins_list создаёт DiscordPinsThreads из токена: {pins_alias_body}"
    );
    assert!(
        pins_alias_body.contains("chord_client::list_channel_pins(&api, &channel_id)"),
        "pins_list делегирует в chord_client::list_channel_pins: {pins_alias_body}"
    );

    // 4) threads_list (app.js:1378 -> list_channel_threads с archived.unwrap_or(false)):
    let threads_alias_body = command_body(head, "threads_list");
    assert!(
        threads_alias_body.contains("PinsThreadsError::Storage")
            && threads_alias_body.contains("PinsThreadsError::MissingToken"),
        "threads_list сохраняет доменные коды Storage/MissingToken: {threads_alias_body}"
    );
    assert!(
        threads_alias_body.contains("DiscordPinsThreads::new(&token)"),
        "threads_list создаёт DiscordPinsThreads из токена: {threads_alias_body}"
    );
    assert!(
        threads_alias_body.contains("archived: Option<bool>"),
        "threads_list принимает опциональный archived: Option<bool> для вызова app.js:1378 без archived: {threads_alias_body}"
    );
    assert!(
        threads_alias_body.contains(
            "chord_client::list_channel_threads(&api, &channel_id, archived.unwrap_or(false))"
        ),
        "threads_list делегирует в list_channel_threads с дефолтом archived=false: {threads_alias_body}"
    );
}

#[test]
fn every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main() {
    let main_source = include_str!("../src/main.rs");
    let head = main_source
        .split("mod wiring_tests")
        .next()
        .expect("голова main.rs");
    let registered = registered_commands(head);

    // Оракул разбора invokeFn("...") и констант AUTH_COMMANDS / JOIN_COMMANDS / NET_COMMANDS / READY_COMMAND:
    assert_eq!(
        extract_ui_invoke_commands(
            r#"export const AUTH_COMMANDS = Object.freeze({ status: "auth_status", login: "auth_login_token" });
               export const JOIN_COMMANDS = Object.freeze({ resolve: "resolve_invite" });
               const NET_COMMANDS = Object.freeze({ status: "net_status", connect: "net_connect" });
               const READY_COMMAND = "ready";
               await invokeFn("update_guild_notification_settings", { guildId });
               await invokeFn('update_channel_notification_override', { guildId, channelId });"#
        ),
        vec![
            "update_guild_notification_settings",
            "update_channel_notification_override",
            "auth_status",
            "auth_login_token",
            "resolve_invite",
            "net_status",
            "net_connect",
            "ready",
        ]
    );

    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ui_dir = crate_root.join("ui");
    let mut ui_sources = Vec::new();
    collect_ui_js_files(&ui_dir, crate_root, &mut ui_sources);
    assert!(
        ui_sources.len() >= 30,
        "ожидается полный обход rust/gui-spike/ui/**/*.js (найдено {} файлов)",
        ui_sources.len()
    );
    assert!(
        ui_sources
            .iter()
            .any(|(file, _)| file == "ui/features/notifications.js"),
        "обход ui/**/*.js обязан включать ui/features/notifications.js"
    );

    let mut seen_ui_commands = Vec::new();
    for (file, js) in &ui_sources {
        for cmd in extract_ui_invoke_commands(js) {
            assert!(
                registered.contains(&cmd),
                "UI-команда `{cmd}` из `{file}` не зарегистрирована в generate_handler! main.rs"
            );
            assert!(
                head.contains(&format!("fn {cmd}(")),
                "UI-команда `{cmd}` из `{file}` не объявлена как fn в main.rs"
            );
            if !seen_ui_commands.iter().any(|seen| *seen == cmd) {
                seen_ui_commands.push(cmd.to_string());
            }
        }
    }

    seen_ui_commands.sort();
    assert_eq!(
        seen_ui_commands.len(),
        ALL_47_UI_IPC_COMMANDS.len(),
        "множество всех найденных UI IPC-команд (включая AUTH_COMMANDS, JOIN_COMMANDS, NET_COMMANDS, READY_COMMAND) должно быть равно ровно {} командам: {seen_ui_commands:?}",
        ALL_47_UI_IPC_COMMANDS.len()
    );
    let seen_refs: Vec<&str> = seen_ui_commands.iter().map(String::as_str).collect();
    assert_eq!(
        seen_refs.as_slice(),
        &ALL_47_UI_IPC_COMMANDS,
        "множество найденных в ui/**/*.js IPC-команд должно точно совпадать с каноническим списком из {} команд",
        ALL_47_UI_IPC_COMMANDS.len()
    );

    for required in F1_UI_IPC_ALIASES
        .into_iter()
        .chain(F5_NOTIFICATIONS_IPC_COMMANDS)
    {
        assert!(
            seen_ui_commands.iter().any(|cmd| cmd == required),
            "оракул UI должен находить вызов `{required}` в исходниках JS"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. RW-40 / INT-06-NOTIFICATIONS-IPC-WIRING (F-5): подключение IPC-команд
//    уведомлений `update_guild_notification_settings` и
//    `update_channel_notification_override` в `main.rs` и делегирование в `lib.rs`
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
struct FakeNotificationTransport {
    status: u16,
    body: Vec<u8>,
    patches: PostLog,
}

impl FakeNotificationTransport {
    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            body: body.to_vec(),
            patches: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn patches(&self) -> Vec<(String, Vec<u8>)> {
        self.patches.lock().unwrap().clone()
    }
}

impl NotificationTransport for FakeNotificationTransport {
    fn patch<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, NotificationError>> + Send + 'a>> {
        self.patches
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let response = HttpResponse {
            status: self.status,
            headers: Vec::new(),
            body: self.body.clone(),
        };
        Box::pin(async move { Ok(response) })
    }
}

#[test]
fn notifications_ipc_commands_are_registered_and_delegate_to_lib() {
    let source = include_str!("../src/main.rs");
    let head = source
        .split("mod wiring_tests")
        .next()
        .expect("голова main.rs");
    let commands = registered_commands(head);

    // c) Оба имени зарегистрированы в tauri::generate_handler![...] ровно по одному разу:
    for command in F5_NOTIFICATIONS_IPC_COMMANDS {
        assert_eq!(
            commands.iter().filter(|name| **name == command).count(),
            1,
            "{command} должен быть зарегистрирован в generate_handler! ровно один раз"
        );
        let body = command_body(head, command);
        assert!(
            body.contains("backend: tauri::State<'_, Arc<AuthBackend>>"),
            "{command}: принимает состояние backend: tauri::State<'_, Arc<AuthBackend>>: {body}"
        );
        assert!(
            body.contains("guild_id: String"),
            "{command}: принимает guild_id: String: {body}"
        );
        assert!(
            body.contains("-> Result<serde_json::Value, NotificationError>"),
            "{command}: возвращает типизированный Result<serde_json::Value, NotificationError>: {body}"
        );
        // b) Оба обработчика читают токен исключительно через token_or:
        assert!(
            body.contains("token_or::<NotificationError, _>("),
            "{command}: токен обязан читаться через единый helper token_or::<NotificationError, _>: {body}"
        );
        assert_eq!(
            body.matches("backend.token()").count(),
            1,
            "{command}: ровно одно чтение backend.token(): {body}"
        );
        assert!(
            body.contains("NotificationError::Storage")
                && body.contains("NotificationError::MissingToken"),
            "{command}: сохраняет доменные коды NotificationError::Storage / MissingToken: {body}"
        );
        assert!(
            body.contains("DiscordNotifications::new(&token)?"),
            "{command}: создаёт DiscordNotifications::new(&token)?: {body}"
        );
        assert!(
            !body.contains("token: String") && !body.contains("bearer: String"),
            "{command}: токен никогда не передаётся из фронтенда: {body}"
        );
        assert!(
            !body.contains("println!") && !body.contains("eprintln!") && !body.contains("log::"),
            "{command}: токен и ошибки не логируются: {body}"
        );
    }

    // a) + b) Делегирование update_guild_notification_settings в chord_client::update_guild_notification_settings:
    let guild_body = command_body(head, "update_guild_notification_settings");
    assert!(
        guild_body.contains("patch: Option<GuildNotificationSettingsPatch>")
            && guild_body.contains("settings: Option<GuildNotificationSettingsPatch>")
            && guild_body.contains("payload: Option<GuildNotificationSettingsPatch>"),
        "update_guild_notification_settings принимает patch/settings/payload из notifications.js и IPC: {guild_body}"
    );
    let guild_compact: String = guild_body
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace(",)", ")");
    assert!(
        guild_compact.contains(
            "chord_client::update_guild_notification_settings(&api,&token,&guild_id,&resolved).await"
        ),
        "update_guild_notification_settings делегирует в chord_client::update_guild_notification_settings: {guild_body}"
    );

    // a) + b) Делегирование update_channel_notification_override в chord_client::update_channel_notification_override:
    let channel_body = command_body(head, "update_channel_notification_override");
    assert!(
        channel_body.contains("channel_id: String"),
        "update_channel_notification_override принимает channel_id: String: {channel_body}"
    );
    assert!(
        channel_body.contains("r#override: Option<ChannelNotificationOverridePatch>")
            && channel_body.contains("override_field: Option<ChannelNotificationOverridePatch>")
            && channel_body.contains("override_patch: Option<ChannelNotificationOverridePatch>")
            && channel_body.contains("patch: Option<ChannelOverrideInput>")
            && channel_body.contains("payload: Option<ChannelOverrideInput>"),
        "update_channel_notification_override принимает r#override / override_field / override_patch / patch / payload (P2-3): {channel_body}"
    );
    let channel_compact: String = channel_body
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace(",)", ")");
    assert!(
        channel_compact.contains(
            "ChannelOverrideInput::resolve(&channel_id,r#override,override_field,override_patch,patch,payload)?"
        ),
        "update_channel_notification_override передаёт r#override и override_field в ChannelOverrideInput::resolve с проверкой ? (P2-2, P2-3): {channel_body}"
    );
    assert!(
        channel_compact.contains(
            "chord_client::update_channel_notification_override(&api,&token,&guild_id,&channel_id,&resolved).await"
        ),
        "update_channel_notification_override делегирует в chord_client::update_channel_notification_override: {channel_body}"
    );

    // Поведенческая проверка делегирования в публичные функции lib.rs через FakeNotificationTransport:
    let guild_id = "1014537851418980372";
    let token = "notif-ipc-token-1";
    let transport = FakeNotificationTransport::with_body(
        200,
        br#"{"guild_id":"1014537851418980372","message_notifications":1,"suppress_everyone":true}"#,
    );
    let client =
        DiscordNotifications::with_transport(token, Arc::new(transport.clone())).expect("client");

    let guild_patch = GuildNotificationSettingsPatch::new()
        .with_message_notifications(1)
        .with_suppress_everyone(true);
    let guild_res = run(update_guild_notification_settings(
        &client,
        token,
        guild_id,
        &guild_patch,
    ))
    .expect("update_guild_notification_settings ok");
    assert_eq!(guild_res["guild_id"], guild_id);
    assert_eq!(guild_res["suppress_everyone"], true);

    let channel_patch = ChannelNotificationOverridePatch::new()
        .with_message_notifications(2)
        .with_muted(false);
    let channel_res = run(update_channel_notification_override(
        &client,
        token,
        guild_id,
        CHANNEL_ID,
        &channel_patch,
    ))
    .expect("update_channel_notification_override ok");
    assert_eq!(channel_res["guild_id"], guild_id);

    let patches = transport.patches();
    assert_eq!(patches.len(), 2);
    assert_eq!(
        patches[0].0,
        format!("/users/@me/guilds/{guild_id}/settings")
    );
    let guild_wire: serde_json::Value =
        serde_json::from_slice(&patches[0].1).expect("guild patch JSON");
    assert_eq!(
        guild_wire,
        serde_json::json!({
            "message_notifications": 1,
            "suppress_everyone": true
        })
    );
    assert_eq!(
        patches[1].0,
        format!("/users/@me/guilds/{guild_id}/settings")
    );
    let channel_wire: serde_json::Value =
        serde_json::from_slice(&patches[1].1).expect("channel override JSON");
    assert_eq!(
        channel_wire,
        serde_json::json!({
            "channel_overrides": {
                CHANNEL_ID: {
                    "message_notifications": 2,
                    "muted": false
                }
            }
        })
    );

    // Предсетевая валидация и отсутствие утечки токена в Debug:
    let guard_transport = FakeNotificationTransport::with_body(200, b"{}");
    assert_eq!(
        run(update_guild_notification_settings(
            &guard_transport,
            token,
            "../bad",
            &guild_patch
        )),
        Err(NotificationError::InvalidGuildId)
    );
    assert_eq!(
        run(update_channel_notification_override(
            &guard_transport,
            token,
            guild_id,
            "../bad",
            &channel_patch
        )),
        Err(NotificationError::InvalidChannelId)
    );
    assert!(guard_transport.patches().is_empty());
    assert!(!format!("{client:?}").contains(token));
}

#[test]
fn channel_override_input_into_patch_rejects_foreign_channel_and_supports_override_field() {
    let source = include_str!("../src/main.rs");
    let head = source
        .split("mod wiring_tests")
        .next()
        .expect("голова main.rs");

    let start = head
        .find("enum ChannelOverrideInput")
        .expect("main.rs должен объявлять enum ChannelOverrideInput");
    let end = head[start..]
        .find("async fn update_guild_notification_settings")
        .expect("конец блока ChannelOverrideInput");
    let block = &head[start..start + end];
    let compact: String = block
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .replace(",)", ")");

    // P2-2: into_patch НЕ должен молча брать чужой канал через `.into_values().next()`,
    // а обязан возвращать Result<ChannelNotificationOverridePatch, NotificationError>
    // с ошибкой NotificationError::InvalidChannelId при непустом конверте без запрошенного channel_id
    // и пустой патч только при пустом конверте:
    assert!(
        !block.contains("into_values().next()"),
        "ChannelOverrideInput::into_patch не должен молча брать первый попавшийся чужой канал (.into_values().next()): {block}"
    );
    assert!(
        compact.contains(
            "fninto_patch(self,channel_id:&str)->Result<ChannelNotificationOverridePatch,NotificationError>"
        ),
        "ChannelOverrideInput::into_patch обязан возвращать Result<ChannelNotificationOverridePatch, NotificationError>: {block}"
    );
    assert!(
        compact.contains("ifletSome(patch)=channel_overrides.remove(channel_id){Ok(patch)}elseifchannel_overrides.is_empty(){Ok(ChannelNotificationOverridePatch::default())}else{Err(NotificationError::InvalidChannelId)}"),
        "ChannelOverrideInput::into_patch обязан возвращать Ok(default) только при пустом конверте и Err(NotificationError::InvalidChannelId) при чужом channel_id: {block}"
    );

    // P2-3: поддержка #[serde(rename = "override")] / override_field без дублирования patch:
    assert!(
        block.contains("#[serde(")
            && block.contains("rename = \"override\"")
            && block.contains("override_field: ChannelNotificationOverridePatch"),
        "ChannelOverrideInput должен поддерживать #[serde(rename = \"override\")] override_field: {block}"
    );
    assert!(
        compact.contains(
            "Ok(r#override.or(override_field).or(override_patch).or(from_patch).or(from_payload).unwrap_or_default())"
        ),
        "ChannelOverrideInput::resolve должен резолвить r#override / override_field без дублирования patch: {block}"
    );
}
