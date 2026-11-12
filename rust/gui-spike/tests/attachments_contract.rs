//! B2a-ATTACH-BACKEND: контракт загрузки вложений.
//!
//! **Оракул — RE, не текущий код:**
//! - `waves/RW-40/RE-DEEP-REA.md` §1 — маршрут `POST /channels/{id}/attachments`,
//!   тело `{files:[{filename,file_size,id,is_clip}]}`, ответ
//!   `{attachments:[{upload_url,upload_filename}]}`, лимиты и коды `40005`/`429`;
//! - `waves/RW-40/ATTACH-RE.md` §2 — та же схема, `uploaded_filename` уходит в
//!   `POST /channels/{id}/messages`.
//!
//! Тесты не ходят в сеть: транспорт подменён фейком, который пишет target и тело.
//! Валидация `channel_id` проверяется до I/O и сверяется с публичным поведением
//! `messages::send_message` (тот же allowlist), чтобы два модуля не разъехались.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
use chord_client::attachments::{
    build_message_attachments, refresh_attachment_urls, request_upload_slots,
    request_upload_slots_with_tier, upload_attachment_bytes, upload_put_command, upload_put_input,
    AttachmentError, AttachmentFileSpec, AttachmentTransport, AttachmentUploadSlot,
    DiscordAttachments, GuildBoostTier, RefreshedAttachmentUrl, UploadBytes, UploadPutCommandArgs,
    UploadPutInput, UploadedAttachment, DEFAULT_UPLOAD_CONTENT_TYPE, MAX_ATTACHMENTS_PER_MESSAGE,
    MAX_REFRESH_URLS_PER_REQUEST, MAX_UPLOAD_FILE_BYTES, MAX_UPLOAD_FILE_BYTES_TIER2,
    MAX_UPLOAD_FILE_BYTES_TIER3, REFRESH_ATTACHMENT_URLS_PATH, UPLOAD_PUT_COMMAND_PARAM_KEYS,
};
use chord_client::messages::{send_message, MessageError, MessageReceipt, MessageSender};
use chord_client::{attachments_upload_put, attachments_upload_put_command};

const CHANNEL_ID: &str = "123456789012345678";

/// Дословный оракул §1.1: шаблон пути (без `/api/v9`).
const ORACLE_TEMPLATE: &str = "/channels/{id}/attachments";

/// Дословные ключи `files[]` (§1.3): ровно эти четыре поля обязательны.
const ORACLE_FILE_KEYS: [&str; 4] = ["file_size", "filename", "id", "is_clip"];

/// Дословные ключи элемента `attachments[]` ответа (§1.2).
const ORACLE_RESPONSE_KEYS: [&str; 3] = ["id", "upload_filename", "upload_url"];

/// Заголовок с задержкой повтора (§1.6, `parseInt(headers["retry-after"])`).
const ORACLE_RETRY_HEADER: &str = "retry-after";

type Calls = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
type PutCalls = Arc<Mutex<Vec<(String, String, Vec<u8>)>>>;

#[derive(Clone)]
struct FakeTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    error: Option<AttachmentError>,
    calls: Calls,
    puts: PutCalls,
}

impl FakeTransport {
    fn answering(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Vec::new(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
            puts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_body(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            error: None,
            calls: Arc::new(Mutex::new(Vec::new())),
            puts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        self
    }

    fn failing(error: AttachmentError) -> Self {
        Self {
            status: 0,
            headers: Vec::new(),
            body: Vec::new(),
            error: Some(error),
            calls: Arc::new(Mutex::new(Vec::new())),
            puts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn calls(&self) -> Vec<(String, Vec<u8>)> {
        self.calls.lock().unwrap().clone()
    }

    fn puts(&self) -> Vec<(String, String, Vec<u8>)> {
        self.puts.lock().unwrap().clone()
    }
}

impl AttachmentTransport for FakeTransport {
    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        self.calls
            .lock()
            .unwrap()
            .push((target.to_string(), body.to_vec()));
        let answer = match self.error.clone() {
            Some(error) => Err(error),
            None => Ok(HttpResponse {
                status: self.status,
                headers: self.headers.clone(),
                body: self.body.clone(),
            }),
        };
        Box::pin(async move { answer })
    }

    fn put<'a>(
        &'a self,
        upload_url: &'a str,
        content_type: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, AttachmentError>> + Send + 'a>> {
        self.puts.lock().unwrap().push((
            upload_url.to_string(),
            content_type.to_string(),
            body.to_vec(),
        ));
        let answer = match self.error.clone() {
            Some(error) => Err(error),
            None => Ok(HttpResponse {
                status: self.status,
                headers: self.headers.clone(),
                body: self.body.clone(),
            }),
        };
        Box::pin(async move { answer })
    }
}

fn api(transport: &FakeTransport) -> DiscordAttachments {
    DiscordAttachments::with_transport(Arc::new(transport.clone()))
}

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap()
        .block_on(future)
}

fn spec(id: &str, filename: &str, file_size: u64) -> AttachmentFileSpec {
    AttachmentFileSpec {
        filename: filename.to_string(),
        file_size,
        id: id.to_string(),
        is_clip: false,
    }
}

fn slots_json(body: &str) -> Vec<u8> {
    body.as_bytes().to_vec()
}

/// Фейковая отправка сообщений: нужна только как независимый оракул allowlist
/// `channel_id` и для доказательства, что до I/O дело не доходит.
#[derive(Default)]
struct FakeMessageSender {
    calls: Mutex<Vec<(String, String, String)>>,
}

impl FakeMessageSender {
    fn calls(&self) -> Vec<(String, String, String)> {
        self.calls.lock().unwrap().clone()
    }
}

impl MessageSender for FakeMessageSender {
    fn send_message<'a>(
        &'a self,
        channel_id: &'a str,
        content: &'a str,
        nonce: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<MessageReceipt, MessageError>> + Send + 'a>> {
        self.calls.lock().unwrap().push((
            channel_id.to_string(),
            content.to_string(),
            nonce.to_string(),
        ));
        let receipt = MessageReceipt {
            id: "1".to_string(),
            channel_id: channel_id.to_string(),
            content: content.to_string(),
            nonce: nonce.to_string(),
        };
        Box::pin(async move { Ok(receipt) })
    }
}

#[test]
fn oracle_constants_match_re() {
    // §1.5 (`RE-DEEP-REA.md:122-125`): база Math.max(0x1400000, tierLimit) = 20 МиБ (0x1400000 = 20_971_520),
    // максимум 10 вложений (n4=10).
    assert_eq!(MAX_UPLOAD_FILE_BYTES, 20 * 1024 * 1024);
    assert_eq!(MAX_ATTACHMENTS_PER_MESSAGE, 10);
    assert_eq!(ORACLE_TEMPLATE, "/channels/{id}/attachments");
    assert_eq!(ORACLE_FILE_KEYS, ["file_size", "filename", "id", "is_clip"]);
    assert_eq!(
        ORACLE_RESPONSE_KEYS,
        ["id", "upload_filename", "upload_url"]
    );
}

#[test]
fn rejects_unsafe_channel_id_before_any_io() {
    let transport = FakeTransport::answering(200);
    let files = [spec("local-1", "a.txt", 1)];
    for unsafe_id in [
        "",
        "../..",
        "../../channels/1/attachments",
        "123/../456",
        "123 456",
        "123\n456",
        "123?x=1",
        "123#frag",
        "%2e%2e",
    ] {
        assert_eq!(
            run(request_upload_slots(&transport, unsafe_id, &files)),
            Err(AttachmentError::InvalidArg),
            "id={unsafe_id:?}"
        );
    }
    let too_long = "1".repeat(65);
    assert_eq!(
        run(request_upload_slots(&transport, &too_long, &files)),
        Err(AttachmentError::InvalidArg)
    );
    assert!(
        transport.calls().is_empty(),
        "валидация обязана отсекать id до I/O"
    );
}

#[test]
fn channel_id_allowlist_matches_messages_module() {
    let transport = FakeTransport::with_body(
        200,
        &slots_json(
            r#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":"1/a.txt"}]}"#,
        ),
    );
    let sender = FakeMessageSender::default();
    let files = [spec("local-1", "a.txt", 1)];

    for channel_id in [
        CHANNEL_ID, "news", "c1", "chan_1-A", "", "../..", "a.b", "a/b", "a b",
    ] {
        let attach_err = run(request_upload_slots(&transport, channel_id, &files)).is_err();
        let message_err = run(send_message(&sender, channel_id, "hi", "nonce")).is_err();
        assert_eq!(
            attach_err, message_err,
            "расхождение allowlist для id={channel_id:?}"
        );
        if message_err {
            assert_eq!(
                run(request_upload_slots(&transport, channel_id, &files)),
                Err(AttachmentError::InvalidArg),
                "id={channel_id:?}"
            );
        }
    }

    assert_eq!(
        transport.calls().len(),
        4,
        "только четыре валидных id дошли до транспорта: {:?}",
        transport.calls()
    );
    assert_eq!(
        sender.calls().len(),
        4,
        "только валидные id дошли до отправки: {:?}",
        sender.calls()
    );
}

#[test]
fn rejects_empty_or_broken_file_lists_before_io() {
    let transport = FakeTransport::answering(200);
    let cases: Vec<(&str, Vec<AttachmentFileSpec>)> = vec![
        ("пустой список files", Vec::new()),
        ("пустой filename", vec![spec("local-1", "", 1)]),
        ("filename из пробелов", vec![spec("local-1", "   ", 1)]),
        ("file_size == 0", vec![spec("local-1", "a.txt", 0)]),
        ("пустой id", vec![spec("", "a.txt", 1)]),
        (
            "дубликат id",
            vec![spec("local-1", "a.txt", 1), spec("local-1", "b.txt", 2)],
        ),
    ];
    for (label, files) in cases {
        assert_eq!(
            run(request_upload_slots(&transport, CHANNEL_ID, &files)),
            Err(AttachmentError::InvalidArg),
            "{label}"
        );
    }

    let too_many: Vec<AttachmentFileSpec> = (0..=MAX_ATTACHMENTS_PER_MESSAGE)
        .map(|index| spec(&format!("local-{index}"), "a.txt", 1))
        .collect();
    assert_eq!(
        run(request_upload_slots(&transport, CHANNEL_ID, &too_many)),
        Err(AttachmentError::InvalidArg),
        "§1.5: больше 10 вложений клиент не отправляет"
    );
    assert!(transport.calls().is_empty(), "до I/O дело не доходит");
}

#[test]
fn rejects_oversized_file_locally_with_entity_too_large() {
    let transport = FakeTransport::answering(200);
    let oversized = [spec("local-1", "big.bin", MAX_UPLOAD_FILE_BYTES + 1)];
    assert_eq!(
        run(request_upload_slots(&transport, CHANNEL_ID, &oversized)),
        Err(AttachmentError::EntityTooLarge)
    );
    assert!(
        transport.calls().is_empty(),
        "§1.6: превышение лимита режется до сети"
    );

    // Ровно на границе — валидно.
    let at_limit = FakeTransport::with_body(
        200,
        &slots_json(
            r#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":"1/big.bin"}]}"#,
        ),
    );
    let files = [spec("local-1", "big.bin", MAX_UPLOAD_FILE_BYTES)];
    assert!(run(request_upload_slots(&at_limit, CHANNEL_ID, &files)).is_ok());
    assert_eq!(at_limit.calls().len(), 1);
}

#[test]
fn posts_exact_upload_slot_request() {
    let transport = FakeTransport::with_body(
        200,
        &slots_json(
            r#"{"attachments":[
                {"id":"local-1","upload_url":"https://cdn.discordapp.com/upload/1","upload_filename":"1/report.pdf"},
                {"id":"local-2","upload_url":"https://cdn.discordapp.com/upload/2","upload_filename":"2/clip.mp4"}
            ]}"#,
        ),
    );
    let files = [
        spec("local-1", "report.pdf", 2048),
        AttachmentFileSpec {
            filename: "clip.mp4".to_string(),
            file_size: 4096,
            id: "local-2".to_string(),
            is_clip: true,
        },
    ];

    let slots = run(request_upload_slots(&transport, CHANNEL_ID, &files)).expect("слоты");
    assert_eq!(
        slots,
        vec![
            AttachmentUploadSlot {
                id: "local-1".to_string(),
                upload_url: "https://cdn.discordapp.com/upload/1".to_string(),
                upload_filename: "1/report.pdf".to_string(),
            },
            AttachmentUploadSlot {
                id: "local-2".to_string(),
                upload_url: "https://cdn.discordapp.com/upload/2".to_string(),
                upload_filename: "2/clip.mp4".to_string(),
            },
        ]
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 1, "ровно один POST за слотами");
    let (target, body) = &calls[0];
    assert_eq!(target, &format!("/channels/{CHANNEL_ID}/attachments"));
    assert!(
        !target.contains('?'),
        "query не подтверждён оракулом: {target}"
    );

    let payload: serde_json::Value = serde_json::from_slice(body).expect("JSON тела");
    let object = payload.as_object().expect("объект тела");
    assert_eq!(object.keys().collect::<Vec<_>>(), vec!["files"]);
    let wire = object["files"].as_array().expect("files[]");
    assert_eq!(wire.len(), 2);

    let mut keys: Vec<&str> = wire[0]
        .as_object()
        .expect("объект files[0]")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ORACLE_FILE_KEYS, "§1.3: обязательные поля files[]");

    assert_eq!(wire[0]["filename"], "report.pdf");
    assert_eq!(wire[0]["file_size"], 2048);
    assert_eq!(wire[0]["id"], "local-1");
    assert_eq!(wire[0]["is_clip"].as_bool(), Some(false));
    assert_eq!(wire[1]["is_clip"].as_bool(), Some(true));
}

#[test]
fn parses_slots_and_keeps_only_oracle_fields() {
    let transport = FakeTransport::with_body(
        200,
        &slots_json(
            r#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":"1/a.txt","extra":1}]}"#,
        ),
    );
    let files = [spec("local-1", "a.txt", 1)];
    let slots = run(request_upload_slots(&transport, CHANNEL_ID, &files)).expect("слоты");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].upload_url, "https://cdn/x");
    assert_eq!(slots[0].upload_filename, "1/a.txt");
}

#[test]
fn rejects_malformed_upload_slot_response() {
    let files = [spec("local-1", "a.txt", 1)];
    for (label, body) in [
        ("пустое тело", b"".as_slice()),
        ("не JSON", b"not-json".as_slice()),
        ("массив вместо объекта", b"[]".as_slice()),
        ("нет attachments", br#"{}"#.as_slice()),
        ("attachments не массив", br#"{"attachments":{}}"#.as_slice()),
        ("пустой attachments", br#"{"attachments":[]}"#.as_slice()),
        (
            "нет upload_url",
            br#"{"attachments":[{"id":"local-1","upload_filename":"1/a.txt"}]}"#.as_slice(),
        ),
        (
            "пустой upload_filename",
            br#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":""}]}"#.as_slice(),
        ),
        (
            "upload_url не строка",
            br#"{"attachments":[{"id":"local-1","upload_url":42,"upload_filename":"1/a.txt"}]}"#.as_slice(),
        ),
    ] {
        let transport = FakeTransport::with_body(200, body);
        assert_eq!(
            run(request_upload_slots(&transport, CHANNEL_ID, &files)),
            Err(AttachmentError::InvalidResponse),
            "{label}"
        );
    }
}

#[test]
fn maps_429_with_retry_after_header() {
    let files = [spec("local-1", "a.txt", 1)];
    for (header, value, expected_ms) in [
        (ORACLE_RETRY_HEADER, "3", 3000_u64),
        ("Retry-After", "1.5", 1500),
    ] {
        let transport = FakeTransport::answering(429).with_headers(&[(header, value)]);
        assert_eq!(
            run(request_upload_slots(&transport, CHANNEL_ID, &files)),
            Err(AttachmentError::RateLimited {
                retry_after_ms: expected_ms
            }),
            "header={header}"
        );
    }

    let no_header = FakeTransport::answering(429);
    assert_eq!(
        run(request_upload_slots(&no_header, CHANNEL_ID, &files)),
        Err(AttachmentError::RateLimited { retry_after_ms: 0 }),
        "без заголовка задержку не выдумываем"
    );
}

#[test]
fn maps_413_and_40005_to_entity_too_large() {
    let files = [spec("local-1", "a.txt", 1)];
    let status_413 = FakeTransport::answering(413);
    assert_eq!(
        run(request_upload_slots(&status_413, CHANNEL_ID, &files)),
        Err(AttachmentError::EntityTooLarge)
    );

    // §1.6: сервер отдаёт `40005` в теле, статус может быть 400.
    let too_large_body = FakeTransport::with_body(400, br#"{"code":40005,"message":"too big"}"#);
    assert_eq!(
        run(request_upload_slots(&too_large_body, CHANNEL_ID, &files)),
        Err(AttachmentError::EntityTooLarge)
    );
}

#[test]
fn maps_other_transport_and_http_failures() {
    let files = [spec("local-1", "a.txt", 1)];
    for (status, expected) in [
        (401, AttachmentError::Unauthorized),
        (403, AttachmentError::Forbidden),
        (404, AttachmentError::NotFound),
        (400, AttachmentError::Client { status: 400 }),
        (500, AttachmentError::Server { status: 500 }),
        (502, AttachmentError::Server { status: 502 }),
    ] {
        let transport = FakeTransport::answering(status);
        assert_eq!(
            run(request_upload_slots(&transport, CHANNEL_ID, &files)),
            Err(expected),
            "HTTP {status}"
        );
    }

    let offline = FakeTransport::failing(AttachmentError::Network);
    assert_eq!(
        run(request_upload_slots(&offline, CHANNEL_ID, &files)),
        Err(AttachmentError::Network)
    );
}

#[test]
fn builds_message_attachments_in_spec_order() {
    let slots = vec![
        AttachmentUploadSlot {
            id: "local-2".to_string(),
            upload_url: "https://cdn/2".to_string(),
            upload_filename: "2/b.png".to_string(),
        },
        AttachmentUploadSlot {
            id: "local-1".to_string(),
            upload_url: "https://cdn/1".to_string(),
            upload_filename: "1/a.txt".to_string(),
        },
    ];
    let specs = [spec("local-1", "a.txt", 1), spec("local-2", "b.png", 2)];

    assert_eq!(
        build_message_attachments(&slots, &specs),
        Ok(vec![
            UploadedAttachment {
                id: "local-1".to_string(),
                filename: "a.txt".to_string(),
                uploaded_filename: "1/a.txt".to_string(),
            },
            UploadedAttachment {
                id: "local-2".to_string(),
                filename: "b.png".to_string(),
                uploaded_filename: "2/b.png".to_string(),
            },
        ])
    );
}

#[test]
fn builder_rejects_mismatched_slots_and_specs() {
    let slots = vec![AttachmentUploadSlot {
        id: "local-1".to_string(),
        upload_url: "https://cdn/1".to_string(),
        upload_filename: "1/a.txt".to_string(),
    }];
    let specs = [spec("local-1", "a.txt", 1)];

    assert_eq!(
        build_message_attachments(&[], &specs),
        Err(AttachmentError::InvalidArg),
        "нет слотов под файл"
    );
    assert_eq!(
        build_message_attachments(&slots, &[]),
        Err(AttachmentError::InvalidArg),
        "нет файлов под слот"
    );

    let unknown = [spec("local-9", "z.txt", 1)];
    assert_eq!(
        build_message_attachments(&slots, &unknown),
        Err(AttachmentError::InvalidArg),
        "слот с чужим id"
    );

    let extra = [
        slots[0].clone(),
        AttachmentUploadSlot {
            id: "local-2".to_string(),
            upload_url: "https://cdn/2".to_string(),
            upload_filename: "2/b.png".to_string(),
        },
    ];
    assert_eq!(
        build_message_attachments(&extra, &specs),
        Err(AttachmentError::InvalidArg),
        "лишний слот"
    );

    let duplicate = [slots[0].clone(), slots[0].clone()];
    assert_eq!(
        build_message_attachments(&duplicate, &specs),
        Err(AttachmentError::InvalidArg),
        "дубликат слота"
    );

    let empty_url = AttachmentUploadSlot {
        id: "local-1".to_string(),
        upload_url: String::new(),
        upload_filename: "1/a.txt".to_string(),
    };
    assert_eq!(
        build_message_attachments(&[empty_url], &specs),
        Err(AttachmentError::InvalidArg),
        "пустой upload_url"
    );
}

#[test]
fn real_client_requires_token_and_sends_nothing() {
    assert!(matches!(
        DiscordAttachments::new("   "),
        Err(AttachmentError::Unauthorized)
    ));
}

#[test]
fn api_impl_matches_free_function() {
    let transport = FakeTransport::with_body(
        200,
        &slots_json(
            r#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":"1/a.txt"}]}"#,
        ),
    );
    let client = api(&transport);
    let files = [spec("local-1", "a.txt", 1)];
    let via_method = run(client.request_upload_slots(CHANNEL_ID, &files)).expect("слоты");
    let via_free = run(request_upload_slots(&client, CHANNEL_ID, &files)).expect("слоты");
    assert_eq!(via_method, via_free);
    assert_eq!(transport.calls().len(), 2);
}

#[test]
fn guild_boost_tier_limits_match_re_oracle() {
    // §1.5 (`waves/RW-40/RE-DEEP-REA.md:122-125`):
    // Формула лимита: `Math.max(0x1400000, tierLimit)`.
    // Базовый пол `0x1400000` = 20 MiB (20_971_520 байт):
    // NONE/TIER_1 (tierLimit 0xa00000 = 10 MiB) -> max(0x1400000, 0xa00000) = 0x1400000 (20 MiB),
    // TIER_2 -> 0x3200000 (50 MiB), TIER_3 -> 0x6400000 (100 MiB).
    assert_eq!(MAX_UPLOAD_FILE_BYTES, 0x140_0000);
    assert_eq!(MAX_UPLOAD_FILE_BYTES, 20 * 1024 * 1024);
    assert_eq!(MAX_UPLOAD_FILE_BYTES, 20_971_520);
    assert_eq!(MAX_UPLOAD_FILE_BYTES_TIER2, 0x320_0000);
    assert_eq!(MAX_UPLOAD_FILE_BYTES_TIER3, 0x640_0000);
    assert_eq!(MAX_UPLOAD_FILE_BYTES_TIER2, 50 * 1024 * 1024);
    assert_eq!(MAX_UPLOAD_FILE_BYTES_TIER3, 100 * 1024 * 1024);

    assert_eq!(GuildBoostTier::default(), GuildBoostTier::None);
    assert_eq!(GuildBoostTier::None.max_upload_bytes(), 0x140_0000);
    assert_eq!(GuildBoostTier::Tier1.max_upload_bytes(), 0x140_0000);
    assert_eq!(GuildBoostTier::Tier2.max_upload_bytes(), 0x320_0000);
    assert_eq!(GuildBoostTier::Tier3.max_upload_bytes(), 0x640_0000);
}

#[test]
fn enforces_exact_upload_size_boundaries_for_each_guild_boost_tier() {
    let ok_body = slots_json(
        r#"{"attachments":[{"id":"local-1","upload_url":"https://cdn/x","upload_filename":"1/big.bin"}]}"#,
    );

    // Диапазон (10 MiB + 1 ..= 20 MiB) обязан проходить для базового вызова, None и Tier1
    // по формуле Math.max(0x1400000, tierLimit).
    for valid_base_size in [0xa0_0000_u64 + 1, 15 * 1024 * 1024, 0x140_0000_u64] {
        let base_transport = FakeTransport::with_body(200, &ok_body);
        let file = [spec("local-1", "mid.bin", valid_base_size)];
        assert!(
            run(request_upload_slots(&base_transport, CHANNEL_ID, &file)).is_ok(),
            "базовый вызов обязан пропускать размер {valid_base_size} (<= 20 MiB)"
        );
        assert!(
            run(request_upload_slots_with_tier(
                &base_transport,
                CHANNEL_ID,
                GuildBoostTier::None,
                &file
            ))
            .is_ok(),
            "GuildBoostTier::None обязан пропускать размер {valid_base_size} (<= 20 MiB)"
        );
        assert!(
            run(request_upload_slots_with_tier(
                &base_transport,
                CHANNEL_ID,
                GuildBoostTier::Tier1,
                &file
            ))
            .is_ok(),
            "GuildBoostTier::Tier1 обязан пропускать размер {valid_base_size} (<= 20 MiB)"
        );
        assert_eq!(base_transport.calls().len(), 3);
    }

    for (tier, limit) in [
        (GuildBoostTier::None, 0x140_0000_u64),
        (GuildBoostTier::Tier1, 0x140_0000_u64),
        (GuildBoostTier::Tier2, 0x320_0000_u64),
        (GuildBoostTier::Tier3, 0x640_0000_u64),
    ] {
        // Ровно на границе проходит.
        let at_limit = FakeTransport::with_body(200, &ok_body);
        let exact = [spec("local-1", "big.bin", limit)];
        let slots = run(request_upload_slots_with_tier(
            &at_limit, CHANNEL_ID, tier, &exact,
        ))
        .unwrap_or_else(|err| panic!("tier={tier:?} на границе {limit} обязан проходить: {err:?}"));
        assert_eq!(slots.len(), 1);
        assert_eq!(at_limit.calls().len(), 1, "tier={tier:?}");

        // На +1 байт отклоняется локально без обращения к сети.
        let over_limit = FakeTransport::with_body(200, &ok_body);
        let oversized = [spec("local-1", "big.bin", limit + 1)];
        assert_eq!(
            run(request_upload_slots_with_tier(
                &over_limit,
                CHANNEL_ID,
                tier,
                &oversized
            )),
            Err(AttachmentError::EntityTooLarge),
            "tier={tier:?} limit+1 обязан возвращать EntityTooLarge"
        );
        assert!(
            over_limit.calls().is_empty(),
            "tier={tier:?}: превышение лимита обязано отсекаться до сети"
        );
    }

    // Проверка метода DiscordAttachments::request_upload_slots_with_tier на 20 MiB + 1 (Tier 2 / Tier 3).
    let transport = FakeTransport::with_body(200, &ok_body);
    let client = api(&transport);
    let over_base_file = [spec("local-1", "big.bin", 0x140_0000_u64 + 1)];
    assert_eq!(
        run(client.request_upload_slots(CHANNEL_ID, &over_base_file)),
        Err(AttachmentError::EntityTooLarge),
        "базовый вызов отклоняет 20 MiB + 1"
    );
    assert_eq!(
        run(client.request_upload_slots_with_tier(
            CHANNEL_ID,
            GuildBoostTier::Tier1,
            &over_base_file
        )),
        Err(AttachmentError::EntityTooLarge),
        "Tier1 отклоняет 20 MiB + 1"
    );
    assert!(
        transport.calls().is_empty(),
        "базовый/Tier1 отказ не ходит в сеть"
    );
    assert!(
        run(client.request_upload_slots_with_tier(
            CHANNEL_ID,
            GuildBoostTier::Tier2,
            &over_base_file
        ))
        .is_ok(),
        "Tier2 пропускает 20 MiB + 1"
    );
    assert_eq!(transport.calls().len(), 1);
}

// ---------------------------------------------------------------------------
// INT-04-ATTACH-PUT-BACKEND: PUT байтов вложения по upload_url (§1.4–§1.6)
// Оракул: `waves/RW-40/RE-DEEP-REA.md` §1.4 (`uploadFileToCloud`,
// `application/octet-stream`), §1.5 (лимиты), §1.6 (40005/413/429);
// `waves/RW-40/ATTACH-RE.md` §2 (`PUT {upload_url}`, `Content-Type` = mime или
// `application/octet-stream`).
// ---------------------------------------------------------------------------

#[test]
fn uploads_attachment_bytes_via_put_with_content_type_and_accepts_200_201_204() {
    let url = "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1?sig=abc123";
    let payload = b"\x89PNG\r\n\x1a\npayload-bytes";

    for status in [200_u16, 201, 204] {
        let transport = FakeTransport::answering(status);
        assert_eq!(
            run(upload_attachment_bytes(
                &transport,
                url,
                "image/png",
                payload
            )),
            Ok(()),
            "HTTP {status} обязан маппиться в Ok(()) для upload_attachment_bytes"
        );
        assert_eq!(
            transport.puts(),
            vec![(url.to_string(), "image/png".to_string(), payload.to_vec())],
            "HTTP {status}: транспорт должен получить точный upload_url, content_type и непустые байты"
        );
        assert!(
            transport.calls().is_empty(),
            "PUT не должен вызывать POST за слотами"
        );
    }

    // Публичная обёртка крейта `chord_client::attachments_upload_put` и метод
    // `DiscordAttachments::upload_attachment_bytes` делегируют в один и тот же контракт:
    let transport_204 = FakeTransport::answering(204);
    let client = api(&transport_204);
    assert_eq!(
        run(attachments_upload_put(
            &client,
            url,
            "application/pdf",
            b"%PDF-1.7"
        )),
        Ok(())
    );
    assert_eq!(
        run(client.upload_attachment_bytes(url, "", b"raw-binary")),
        Ok(()),
        "пустой content_type обязан подставлять DEFAULT_UPLOAD_CONTENT_TYPE"
    );
    assert_eq!(DEFAULT_UPLOAD_CONTENT_TYPE, "application/octet-stream");
    assert_eq!(
        transport_204.puts(),
        vec![
            (
                url.to_string(),
                "application/pdf".to_string(),
                b"%PDF-1.7".to_vec()
            ),
            (
                url.to_string(),
                "application/octet-stream".to_string(),
                b"raw-binary".to_vec()
            ),
        ]
    );
}

#[test]
fn rejects_invalid_or_insecure_upload_url_and_empty_bytes_before_io() {
    let transport = FakeTransport::answering(200);
    let valid_bytes = b"hello";

    for bad_url in [
        "",
        "   ",
        "http://cdn.discordapp.com/upload/1",
        "HTTP://cdn.discordapp.com/upload/1",
        "ftp://cdn.discordapp.com/upload/1",
        "/channels/123/attachments",
        "https://",
        "https:///upload/1",
        "https://cdn.discordapp.com/upload/1#frag",
        "https://cdn.discordapp.com/upload/1 space",
        "https://user@cdn.discordapp.com/upload/1",
    ] {
        assert_eq!(
            run(upload_attachment_bytes(
                &transport,
                bad_url,
                "text/plain",
                valid_bytes
            )),
            Err(AttachmentError::InvalidArg),
            "upload_url={bad_url:?} обязан отклоняться до сети"
        );
        assert_eq!(
            run(attachments_upload_put(
                &transport,
                bad_url,
                "text/plain",
                valid_bytes
            )),
            Err(AttachmentError::InvalidArg),
            "attachments_upload_put upload_url={bad_url:?} обязан отклоняться до сети"
        );
    }

    // Пустой массив байтов (0 байт -> ENTITY_EMPTY / InvalidArg) отклоняется до сети:
    assert_eq!(
        run(upload_attachment_bytes(
            &transport,
            "https://cdn.discordapp.com/upload/1",
            "text/plain",
            b""
        )),
        Err(AttachmentError::InvalidArg),
        "пустые байты не должны уходить в PUT"
    );

    // Заголовок Content-Type с CR/LF отклоняется до сети:
    assert_eq!(
        run(upload_attachment_bytes(
            &transport,
            "https://cdn.discordapp.com/upload/1",
            "text/plain\r\nX-Injected: 1",
            valid_bytes
        )),
        Err(AttachmentError::InvalidArg),
        "CR/LF в content_type обязан отклоняться до сети"
    );

    // Превышение базового лимита 20 MiB отклоняется локально с EntityTooLarge до сети:
    let oversized = vec![0_u8; (MAX_UPLOAD_FILE_BYTES as usize) + 1];
    assert_eq!(
        run(upload_attachment_bytes(
            &transport,
            "https://cdn.discordapp.com/upload/1",
            "application/octet-stream",
            &oversized
        )),
        Err(AttachmentError::EntityTooLarge),
        "превышение MAX_UPLOAD_FILE_BYTES обязано возвращать EntityTooLarge до сети"
    );

    assert!(
        transport.puts().is_empty() && transport.calls().is_empty(),
        "предсетевая валидация PUT не должна обращаться к транспорту"
    );
}

#[test]
fn maps_put_status_codes_413_429_5xx_and_network_errors() {
    let url = "https://cdn.discordapp.com/upload/slot-42";
    let body = b"attachment-content";

    // 413 и 400 + code 40005 -> EntityTooLarge
    let status_413 = FakeTransport::answering(413);
    assert_eq!(
        run(attachments_upload_put(
            &status_413,
            url,
            "application/octet-stream",
            body
        )),
        Err(AttachmentError::EntityTooLarge)
    );
    let body_40005 =
        FakeTransport::with_body(400, br#"{"code":40005,"message":"entity too large"}"#);
    assert_eq!(
        run(attachments_upload_put(
            &body_40005,
            url,
            "application/octet-stream",
            body
        )),
        Err(AttachmentError::EntityTooLarge)
    );

    // 429 с Retry-After и без него
    for (header, value, expected_ms) in [
        (ORACLE_RETRY_HEADER, "4", 4000_u64),
        ("Retry-After", "1.5", 1500),
    ] {
        let rl = FakeTransport::answering(429).with_headers(&[(header, value)]);
        assert_eq!(
            run(attachments_upload_put(&rl, url, "image/png", body)),
            Err(AttachmentError::RateLimited {
                retry_after_ms: expected_ms
            }),
            "PUT 429 header={header}"
        );
    }
    let rl_no_header = FakeTransport::answering(429);
    assert_eq!(
        run(attachments_upload_put(
            &rl_no_header,
            url,
            "image/png",
            body
        )),
        Err(AttachmentError::RateLimited { retry_after_ms: 0 })
    );

    // 401/403/404/400/5xx
    for (status, expected) in [
        (401, AttachmentError::Unauthorized),
        (403, AttachmentError::Forbidden),
        (404, AttachmentError::NotFound),
        (400, AttachmentError::Client { status: 400 }),
        (500, AttachmentError::Server { status: 500 }),
        (502, AttachmentError::Server { status: 502 }),
        (503, AttachmentError::Server { status: 503 }),
    ] {
        let transport = FakeTransport::answering(status);
        assert_eq!(
            run(attachments_upload_put(&transport, url, "image/png", body)),
            Err(expected),
            "PUT HTTP {status}"
        );
    }

    let offline = FakeTransport::failing(AttachmentError::Network);
    assert_eq!(
        run(attachments_upload_put(&offline, url, "image/png", body)),
        Err(AttachmentError::Network)
    );
}

#[test]
fn upload_put_input_deserializes_uint8array_and_vec_and_validates_ipc_payload() {
    // 1. Десериализация формы `composer.js` (`putPayload`), где `Uint8Array` в JSON представлен
    // объектом с индексными ключами `{"0": 10, "1": 20, "2": 30, "3": 40}`:
    let composer_json = serde_json::json!({
        "channelId": CHANNEL_ID,
        "id": "attach-1",
        "uploadUrl": "https://discord.upload/slot-1",
        "uploadFilename": "1/payload.bin",
        "filename": "payload.bin",
        "fileSize": 4,
        "contentType": "application/octet-stream",
        "file": {},
        "bytes": {"0": 10, "1": 20, "2": 30, "3": 40},
        "data": {"0": 10, "1": 20, "2": 30, "3": 40}
    });
    let input_from_uint8: UploadPutInput =
        serde_json::from_value(composer_json).expect("десериализация putPayload из composer.js");
    assert_eq!(input_from_uint8.payload_bytes(), &[10, 20, 30, 40]);

    let transport = FakeTransport::answering(200);
    assert_eq!(run(upload_put_input(&transport, &input_from_uint8)), Ok(()));
    assert_eq!(
        transport.puts(),
        vec![(
            "https://discord.upload/slot-1".to_string(),
            "application/octet-stream".to_string(),
            vec![10, 20, 30, 40],
        )]
    );

    // 2. Сборка через `UploadPutInput::from_ipc` из плоских аргументов Tauri-команды:
    let flat_input = UploadPutInput::from_ipc(
        None,
        Some("slot-2".to_string()),
        Some(CHANNEL_ID.to_string()),
        Some("https://discord.upload/slot-2".to_string()),
        Some("image/webp".to_string()),
        None,
        Some(UploadBytes::from(vec![1, 2, 3])),
    )
    .expect("from_ipc с плоскими аргументами");
    let transport_flat = FakeTransport::answering(204);
    assert_eq!(run(upload_put_input(&transport_flat, &flat_input)), Ok(()));
    assert_eq!(
        transport_flat.puts(),
        vec![(
            "https://discord.upload/slot-2".to_string(),
            "image/webp".to_string(),
            vec![1, 2, 3],
        )]
    );

    // 3. Небезопасный `channel_id` в `UploadPutInput` отсекается до обращения к транспорту:
    let bad_channel_input = UploadPutInput {
        channel_id: Some("../bad".to_string()),
        slot_id: Some("slot-1".to_string()),
        upload_url: "https://discord.upload/slot-1".to_string(),
        content_type: "text/plain".to_string(),
        data: vec![1, 2, 3],
        bytes: Vec::new(),
    };
    let guard_transport = FakeTransport::answering(200);
    assert_eq!(
        run(upload_put_input(&guard_transport, &bad_channel_input)),
        Err(AttachmentError::InvalidArg)
    );
    assert!(guard_transport.puts().is_empty());
}

#[test]
fn tauri_main_registers_and_wires_attachments_upload_put_command() {
    let source = include_str!("../src/main.rs");
    let open = "generate_handler![";
    let start = source.find(open).expect("generate_handler!") + open.len();
    let block = &source[start..start + source[start..].find(']').expect("]")];
    let registered: Vec<&str> = block
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|name| !name.is_empty())
        .collect();

    assert_eq!(
        registered
            .iter()
            .filter(|name| **name == "attachments_upload_put")
            .count(),
        1,
        "attachments_upload_put обязан быть зарегистрирован в generate_handler! ровно один раз"
    );

    let marker = "async fn attachments_upload_put(";
    let cmd_start = source
        .find(marker)
        .expect("main.rs должен объявлять async fn attachments_upload_put");
    let rest = &source[cmd_start..];
    let cmd_end = rest.find("#[tauri::command]").unwrap_or(rest.len());
    let body = &rest[..cmd_end];

    assert!(
        body.contains("token_or::<AttachmentError"),
        "attachments_upload_put обязан читать токен через token_or: {body}"
    );
    assert_eq!(
        body.matches("backend.token()").count(),
        1,
        "attachments_upload_put: ровно одно чтение backend.token(): {body}"
    );
    assert!(
        body.contains("DiscordAttachments::new(&token)"),
        "attachments_upload_put создаёт DiscordAttachments из токена: {body}"
    );
    assert!(
        body.contains("chord_client::attachments_upload_put("),
        "attachments_upload_put делегирует в публичную обёртку крейта: {body}"
    );
    assert!(
        !body.contains("println!") && !body.contains("eprintln!") && !body.contains("log::"),
        "attachments_upload_put не должен логировать токен или payload: {body}"
    );
}

/// Извлекает список `(snake_name, camel_key)` параметров `#[tauri::command] async fn attachments_upload_put`
/// из `src/main.rs` (кроме `backend: tauri::State`).
fn parse_attachments_upload_put_ipc_keys(main_source: &str) -> Vec<(String, String)> {
    let marker = "async fn attachments_upload_put(";
    let start = main_source
        .find(marker)
        .expect("main.rs должен объявлять async fn attachments_upload_put")
        + marker.len();
    let rest = &main_source[start..];
    let end = rest.find(')').expect("закрывающая ) сигнатуры команды");
    let args_block = &rest[..end];

    let mut out = Vec::new();
    for raw_arg in args_block.split(',') {
        let trimmed = raw_arg.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((name_part, type_part)) = trimmed.split_once(':') else {
            continue;
        };
        let name = name_part.trim();
        let ty = type_part.trim();
        if ty.contains("tauri::State") {
            continue;
        }
        let mut camel = String::with_capacity(name.len());
        let mut upper_next = false;
        for ch in name.chars() {
            if ch == '_' {
                upper_next = true;
            } else if upper_next {
                camel.push(ch.to_ascii_uppercase());
                upper_next = false;
            } else {
                camel.push(ch);
            }
        }
        out.push((name.to_string(), camel));
    }
    out
}

#[test]
fn tauri_command_extracts_flat_composer_put_payload_with_id_and_uint8array_into_http_put() {
    let main_source = include_str!("../src/main.rs");
    let declared_keys = parse_attachments_upload_put_ipc_keys(main_source);
    let expected_keys: Vec<(String, String)> = UPLOAD_PUT_COMMAND_PARAM_KEYS
        .iter()
        .map(|(snake, camel)| ((*snake).to_string(), (*camel).to_string()))
        .collect();
    assert_eq!(
        declared_keys, expected_keys,
        "#[tauri::command] async fn attachments_upload_put обязан принимать точный набор параметров Tauri command extraction (включая `id: Option<String>`)"
    );

    // Точный плоский JSON `putPayload` из `composer.js:744-755`:
    let upload_url =
        "https://discord-attachments-uploads-prd.storage.googleapis.com/upload-1?sig=abc123";
    let composer_put_payload = serde_json::json!({
        "channelId": CHANNEL_ID,
        "id": "slot-1",
        "uploadUrl": upload_url,
        "uploadFilename": "1/image.png",
        "fileSize": 2,
        "contentType": "image/png",
        "bytes": {"0": 137, "1": 80},
        "data": {"0": 137, "1": 80}
    });

    // Фильтруем JSON только по ключам, реально объявленным в сигнатуре `#[tauri::command]`:
    let raw_obj = composer_put_payload.as_object().expect("JSON object");
    let mut extracted_map = serde_json::Map::new();
    for (_snake, camel) in &declared_keys {
        if let Some(val) = raw_obj.get(camel) {
            extracted_map.insert(camel.clone(), val.clone());
        }
    }
    let filtered_invoke_json = serde_json::Value::Object(extracted_map);

    let extracted_args = UploadPutCommandArgs::from_tauri_ipc(&filtered_invoke_json)
        .expect("извлечение аргументов Tauri-команды из плоского putPayload");
    let serde_args: UploadPutCommandArgs = serde_json::from_value(filtered_invoke_json)
        .expect("десериализация UploadPutCommandArgs из плоского putPayload");
    assert_eq!(extracted_args, serde_args);
    assert_eq!(
        extracted_args.payload, None,
        "плоский putPayload из composer.js не содержит вложенного поля `payload`"
    );
    assert_eq!(
        extracted_args.slot_id, None,
        "плоский putPayload из composer.js передаёт `id`, а не `slotId`"
    );
    assert_eq!(extracted_args.id.as_deref(), Some("slot-1"));
    assert_eq!(extracted_args.channel_id.as_deref(), Some(CHANNEL_ID));
    assert_eq!(extracted_args.upload_url.as_deref(), Some(upload_url));
    assert_eq!(extracted_args.content_type.as_deref(), Some("image/png"));
    assert_eq!(extracted_args.data, Some(UploadBytes(vec![137, 80])));
    assert_eq!(extracted_args.bytes, Some(UploadBytes(vec![137, 80])));

    let resolved_input = extracted_args
        .clone()
        .into_input()
        .expect("into_input для плоского putPayload");
    assert_eq!(
        resolved_input.slot_id.as_deref(),
        Some("slot-1"),
        "slot_id.or(id) обязан сохранять id из плоского putPayload"
    );
    assert_eq!(resolved_input.payload_bytes(), &[137, 80]);

    let transport = FakeTransport::answering(200);
    assert_eq!(
        run(attachments_upload_put_command(
            &transport,
            extracted_args.clone()
        )),
        Ok(())
    );
    assert_eq!(run(upload_put_command(&transport, extracted_args)), Ok(()));
    assert_eq!(
        transport.puts(),
        vec![
            (
                upload_url.to_string(),
                "image/png".to_string(),
                vec![137, 80]
            ),
            (
                upload_url.to_string(),
                "image/png".to_string(),
                vec![137, 80]
            ),
        ],
        "байты [137, 80] и contentType обязаны доходить до HTTP PUT"
    );

    // Если в плоском putPayload передан пробельный `id` (без `slotId`), команда обязана
    // отклонить вызов с `InvalidArg` до обращения к сети:
    let bad_id_payload = serde_json::json!({
        "channelId": CHANNEL_ID,
        "id": "   ",
        "uploadUrl": upload_url,
        "uploadFilename": "1/image.png",
        "fileSize": 2,
        "contentType": "image/png",
        "bytes": {"0": 137, "1": 80},
        "data": {"0": 137, "1": 80}
    });
    let bad_id_args =
        UploadPutCommandArgs::from_tauri_ipc(&bad_id_payload).expect("десериализация bad_id");
    let guard_transport = FakeTransport::answering(200);
    assert_eq!(
        run(attachments_upload_put_command(
            &guard_transport,
            bad_id_args
        )),
        Err(AttachmentError::InvalidArg)
    );
    assert!(guard_transport.puts().is_empty());
}

#[test]
fn refresh_attachment_urls_constants_match_oracle_and_limits() {
    assert_eq!(REFRESH_ATTACHMENT_URLS_PATH, "/attachments/refresh-urls");
    assert_eq!(MAX_REFRESH_URLS_PER_REQUEST, 50);
}

#[test]
fn refreshed_attachment_url_implements_serde_and_traits() {
    let item = RefreshedAttachmentUrl {
        original: "https://cdn.discordapp.com/attachments/1/2/photo.png?ex=65e".to_string(),
        refreshed: "https://cdn.discordapp.com/attachments/1/2/photo.png?ex=66f".to_string(),
    };
    let cloned = item.clone();
    assert_eq!(item, cloned);
    assert_eq!(format!("{item:?}"), format!("{cloned:?}"));

    let json = serde_json::to_string(&item).expect("сериализация RefreshedAttachmentUrl");
    let deserialized: RefreshedAttachmentUrl =
        serde_json::from_str(&json).expect("десериализация RefreshedAttachmentUrl");
    assert_eq!(item, deserialized);
}

#[test]
fn refresh_attachment_urls_sends_post_and_parses_response() {
    let response_body = serde_json::json!({
        "refreshed_urls": [
            {
                "original": "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=65e",
                "refreshed": "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=66f"
            },
            {
                "original": "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=65e",
                "refreshed": "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=66f"
            }
        ]
    });
    let transport =
        FakeTransport::with_body(200, serde_json::to_vec(&response_body).unwrap().as_slice());

    let urls = vec![
        "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=65e".to_string(),
        "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=65e".to_string(),
    ];

    let result = run(refresh_attachment_urls(&transport, &urls)).expect("успешный refresh");
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0],
        RefreshedAttachmentUrl {
            original: "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=65e".to_string(),
            refreshed: "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=66f".to_string(),
        }
    );
    assert_eq!(
        result[1],
        RefreshedAttachmentUrl {
            original: "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=65e".to_string(),
            refreshed: "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=66f".to_string(),
        }
    );

    let calls = transport.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "/attachments/refresh-urls");
    let sent_body: serde_json::Value =
        serde_json::from_slice(&calls[0].1).expect("валидный JSON запроса");
    assert_eq!(
        sent_body,
        serde_json::json!({
            "attachment_urls": [
                "https://cdn.discordapp.com/attachments/1/2/pic1.png?ex=65e",
                "https://cdn.discordapp.com/attachments/1/2/pic2.png?ex=65e"
            ]
        })
    );

    // Проверяем также делегацию метода в DiscordAttachments:
    let client = api(&transport);
    let client_result = run(client.refresh_attachment_urls(&urls))
        .expect("успешный refresh через DiscordAttachments");
    assert_eq!(client_result, result);
}

#[test]
fn refresh_attachment_urls_rejects_empty_and_oversized_batches_before_network() {
    let transport = FakeTransport::answering(200);

    // Пустой список:
    let empty_urls: Vec<String> = Vec::new();
    assert_eq!(
        run(refresh_attachment_urls(&transport, &empty_urls)),
        Err(AttachmentError::InvalidArg)
    );
    assert!(transport.calls().is_empty());

    // 51 URL (лимит MAX_REFRESH_URLS_PER_REQUEST = 50):
    let mut oversized_urls = Vec::with_capacity(51);
    for i in 0..51 {
        oversized_urls.push(format!(
            "https://cdn.discordapp.com/attachments/1/{i}/file.png"
        ));
    }
    assert_eq!(
        run(refresh_attachment_urls(&transport, &oversized_urls)),
        Err(AttachmentError::InvalidArg)
    );
    assert!(transport.calls().is_empty());
}

#[test]
fn refresh_attachment_urls_rejects_invalid_urls_and_duplicates_before_network() {
    let transport = FakeTransport::answering(200);

    let invalid_cases = vec![
        vec!["".to_string()],
        vec!["   ".to_string()],
        vec!["http://cdn.discordapp.com/attachments/1/2/pic.png".to_string()],
        vec!["ftp://cdn.discordapp.com/attachments/1/2/pic.png".to_string()],
        vec!["https://cdn.discordapp.com/attachments/1/2/pic.png#section".to_string()],
        vec!["https://cdn.discordapp.com/attachments/1/2/pic with space.png".to_string()],
        vec!["https://cdn.discordapp.com/attachments/1/2/pic\n.png".to_string()],
        vec!["https://cdn.discordapp.com/attachments/1/2/pic\0.png".to_string()],
        vec![
            "https://cdn.discordapp.com/attachments/1/2/dup.png".to_string(),
            "https://cdn.discordapp.com/attachments/1/2/dup.png".to_string(),
        ],
    ];

    for case in invalid_cases {
        assert_eq!(
            run(refresh_attachment_urls(&transport, &case)),
            Err(AttachmentError::InvalidArg),
            "кейс должен отклоняться как InvalidArg: {:?}",
            case
        );
    }
    assert!(transport.calls().is_empty());
}

#[test]
fn refresh_attachment_urls_rejects_malformed_and_invalid_responses() {
    let urls = vec!["https://cdn.discordapp.com/attachments/1/2/pic.png".to_string()];

    // Некорректный JSON:
    let bad_json_transport = FakeTransport::with_body(200, b"not-a-json");
    assert_eq!(
        run(refresh_attachment_urls(&bad_json_transport, &urls)),
        Err(AttachmentError::InvalidResponse)
    );

    // Пустой JSON объект (нет ключа refreshed_urls):
    let empty_obj_transport = FakeTransport::with_body(200, b"{}");
    assert_eq!(
        run(refresh_attachment_urls(&empty_obj_transport, &urls)),
        Err(AttachmentError::InvalidResponse)
    );

    // Пустой original в ответе:
    let empty_orig_transport = FakeTransport::with_body(
        200,
        br#"{"refreshed_urls":[{"original":"","refreshed":"https://cdn.discordapp.com/attachments/1/2/pic.png"}]}"#,
    );
    assert_eq!(
        run(refresh_attachment_urls(&empty_orig_transport, &urls)),
        Err(AttachmentError::InvalidResponse)
    );

    // Пустой refreshed в ответе:
    let empty_ref_transport = FakeTransport::with_body(
        200,
        br#"{"refreshed_urls":[{"original":"https://cdn.discordapp.com/attachments/1/2/pic.png","refreshed":""}]}"#,
    );
    assert_eq!(
        run(refresh_attachment_urls(&empty_ref_transport, &urls)),
        Err(AttachmentError::InvalidResponse)
    );

    // Небезопасный (не-https) refreshed URL в ответе:
    let http_ref_transport = FakeTransport::with_body(
        200,
        br#"{"refreshed_urls":[{"original":"https://cdn.discordapp.com/attachments/1/2/pic.png","refreshed":"http://insecure.com/pic.png"}]}"#,
    );
    assert_eq!(
        run(refresh_attachment_urls(&http_ref_transport, &urls)),
        Err(AttachmentError::InvalidResponse)
    );
}

#[test]
fn refresh_attachment_urls_maps_rest_error_status_codes() {
    let urls = vec!["https://cdn.discordapp.com/attachments/1/2/pic.png".to_string()];

    // 429 с Retry-After:
    let transport_429 = FakeTransport::answering(429).with_headers(&[("retry-after", "3.5")]);
    assert_eq!(
        run(refresh_attachment_urls(&transport_429, &urls)),
        Err(AttachmentError::RateLimited {
            retry_after_ms: 3500
        })
    );

    // 401 Unauthorized:
    let transport_401 = FakeTransport::answering(401);
    assert_eq!(
        run(refresh_attachment_urls(&transport_401, &urls)),
        Err(AttachmentError::Unauthorized)
    );

    // 403 Forbidden:
    let transport_403 = FakeTransport::answering(403);
    assert_eq!(
        run(refresh_attachment_urls(&transport_403, &urls)),
        Err(AttachmentError::Forbidden)
    );

    // 404 NotFound:
    let transport_404 = FakeTransport::answering(404);
    assert_eq!(
        run(refresh_attachment_urls(&transport_404, &urls)),
        Err(AttachmentError::NotFound)
    );

    // 500 Server error:
    let transport_500 = FakeTransport::answering(500);
    assert_eq!(
        run(refresh_attachment_urls(&transport_500, &urls)),
        Err(AttachmentError::Server { status: 500 })
    );

    // 503 Service Unavailable:
    let transport_503 = FakeTransport::answering(503);
    assert_eq!(
        run(refresh_attachment_urls(&transport_503, &urls)),
        Err(AttachmentError::Server { status: 503 })
    );

    // 418 Client other error:
    let transport_418 = FakeTransport::answering(418);
    assert_eq!(
        run(refresh_attachment_urls(&transport_418, &urls)),
        Err(AttachmentError::Client { status: 418 })
    );

    // Network failure:
    let transport_net = FakeTransport::failing(AttachmentError::Network);
    assert_eq!(
        run(refresh_attachment_urls(&transport_net, &urls)),
        Err(AttachmentError::Network)
    );
}
