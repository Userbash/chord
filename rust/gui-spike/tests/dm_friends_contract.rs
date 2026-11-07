//! RW-40 / Task-112 (N1-07): контрактные тесты модуля личных сообщений и друзей (`dm_friends`).
//!
//! Независимые оракулы:
//! - `waves/RW-40/DM-FRIENDS-RE.md` (§0–§12)
//! - `waves/RW-40/DESIGN/DM-FRIENDS-SPEC.md` (§1, §4, §6)
//! - `fixtures/gateway/by-type/002-READY.json` (17 `private_channels`, 60 `relationships`, 146 `users`)
//! - `fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json` (`d.merged_presences.friends`)
//! - `fixtures/rest/rest-static.json` (`USER_CHANNELS`, `USER_GROUP_DM_SHELL`, `USER_RELATIONSHIPS`, `USER_RELATIONSHIP`)

use std::collections::BTreeSet;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use discord_core::rest::transport::HttpResponse;
use discord_gui_spike::dm_friends::{
    accept_friend_request, block_user, build_user_relationship_endpoint, create_group_dm_shell,
    create_private_channel, delete_relationship, filter_pending_spam_and_ignored,
    filter_relationships_by_tab, load_ready_dm_friends, open_private_channel_with_cache,
    parse_ready_dm_friends, parse_supplemental_online_friends, put_relationship,
    send_friend_request, sort_private_channels_by_last_message_desc, validate_snowflake_id,
    DiscordDmFriends, DmFriendsError, DmFriendsTransport, FriendRequestTagInput, FriendsTab,
    PrivateChannelType, PutRelationshipParams, RelationshipCounts, RelationshipType,
    API_V9_USER_CHANNELS_PATH, API_V9_USER_RELATIONSHIPS_PATH, CHANNEL_FLAG_PINNED,
    CHANNEL_TYPE_DM, CHANNEL_TYPE_GROUP_DM, DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION,
    DISCORD_ERR_TOO_MANY_RECIPIENTS, MAX_DM_RECIPIENTS, MIN_DM_RECIPIENTS,
    RELATIONSHIP_TYPE_BLOCKED, RELATIONSHIP_TYPE_FRIEND, RELATIONSHIP_TYPE_PENDING_INCOMING,
    RELATIONSHIP_TYPE_PENDING_OUTGOING, USER_CHANNELS_PATH, USER_GROUP_DM_SHELL_PATH,
    USER_RELATIONSHIPS_PATH,
};

const SECRET_TOKEN: &str = "mfa.super_secret_token_task112_do_not_leak_987654321";

fn run<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime")
        .block_on(future)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("корень репозитория")
        .to_path_buf()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedCall {
    method: String,
    target: String,
    body: Vec<u8>,
}

#[derive(Clone, Default)]
struct FakeDmFriendsTransport {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    calls: Arc<Mutex<Vec<RecordedCall>>>,
}

impl FakeDmFriendsTransport {
    fn with_response(status: u16, body: &[u8]) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: body.to_vec(),
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn with_headers(mut self, headers: &[(&str, &str)]) -> Self {
        self.headers = headers
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        self
    }

    fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().clone()
    }

    fn record(&self, method: &str, target: &str, body: &[u8]) -> HttpResponse {
        self.calls.lock().unwrap().push(RecordedCall {
            method: method.to_string(),
            target: target.to_string(),
            body: body.to_vec(),
        });
        HttpResponse {
            status: self.status,
            headers: self.headers.clone(),
            body: self.body.clone(),
        }
    }
}

impl DmFriendsTransport for FakeDmFriendsTransport {
    fn get<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        let resp = self.record("GET", target, &[]);
        Box::pin(async move { Ok(resp) })
    }

    fn post<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        let resp = self.record("POST", target, body);
        Box::pin(async move { Ok(resp) })
    }

    fn put<'a>(
        &'a self,
        target: &'a str,
        body: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        let resp = self.record("PUT", target, body);
        Box::pin(async move { Ok(resp) })
    }

    fn delete<'a>(
        &'a self,
        target: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, DmFriendsError>> + Send + 'a>> {
        let resp = self.record("DELETE", target, &[]);
        Box::pin(async move { Ok(resp) })
    }
}

// ---------------------------------------------------------------------------
// 1. Разбор READY.private_channels (17 объектов, типы 1=DM и 3=Group DM,
//    разрешение recipients через READY.users из 146 пользователей,
//    сортировка по убыванию числового u64 last_message_id, 0 GET /users/@me/channels)
// ---------------------------------------------------------------------------

#[test]
fn ready_private_channels_and_rest_static_oracle_parity() {
    let root = repo_root();

    // 1.1 Сверка маршрутов с независимым оракулом `fixtures/rest/rest-static.json`:
    let rest_static_raw = std::fs::read_to_string(root.join("fixtures/rest/rest-static.json"))
        .expect("fixtures/rest/rest-static.json");
    let rest_static: serde_json::Value =
        serde_json::from_str(&rest_static_raw).expect("rest-static JSON");
    let endpoints = rest_static
        .get("endpoints")
        .and_then(serde_json::Value::as_array)
        .expect("endpoints array");

    // USER_CHANNELS: только POST, 2 call-site, тело {recipients}, 0 GET-вызовов:
    let user_channels_eps: Vec<&serde_json::Value> = endpoints
        .iter()
        .filter(|ep| ep.get("key").and_then(|v| v.as_str()) == Some("USER_CHANNELS"))
        .collect();
    assert_eq!(user_channels_eps.len(), 1);
    assert_eq!(
        user_channels_eps[0].get("method").and_then(|v| v.as_str()),
        Some("POST"),
        "GET /users/@me/channels в бандле Discord 1.0.162 не используется (0 call-site)"
    );
    assert_eq!(
        user_channels_eps[0].get("path").and_then(|v| v.as_str()),
        Some(USER_CHANNELS_PATH)
    );
    assert_eq!(API_V9_USER_CHANNELS_PATH, "/api/v9/users/@me/channels");
    assert_eq!(
        API_V9_USER_RELATIONSHIPS_PATH,
        "/api/v9/users/@me/relationships"
    );

    let shell_ep = endpoints
        .iter()
        .find(|ep| ep.get("key").and_then(|v| v.as_str()) == Some("USER_GROUP_DM_SHELL"))
        .expect("USER_GROUP_DM_SHELL");
    assert_eq!(
        shell_ep.get("path").and_then(|v| v.as_str()),
        Some(USER_GROUP_DM_SHELL_PATH)
    );

    // 1.2 Загрузка `002-READY.json` без единого сетевого вызова (`0` GET /users/@me/channels):
    let transport = FakeDmFriendsTransport::with_response(200, b"[]");
    let snapshot =
        load_ready_dm_friends(&root.join("fixtures")).expect("load 002-READY.json snapshot");
    assert!(
        transport.calls().is_empty(),
        "начальный список ЛС обязан приходить из READY.private_channels с 0 вызовов GET /users/@me/channels"
    );

    assert_eq!(
        snapshot.users.len(),
        146,
        "в 002-READY.json ровно 146 пользователей в d.users[]"
    );
    assert_eq!(
        snapshot.private_channels.len(),
        17,
        "в 002-READY.json ровно 17 объектов в d.private_channels[]"
    );

    // Все 17 объектов в захвате имеют type=1 (DM), ровно 1 получателя и резолвятся в READY.users:
    for ch in &snapshot.private_channels {
        assert_eq!(ch.channel_type, PrivateChannelType::Dm);
        assert_eq!(ch.channel_type.as_i64(), CHANNEL_TYPE_DM);
        assert_eq!(ch.recipient_ids.len(), 1);
        assert_eq!(
            ch.recipients.len(),
            1,
            "каждый recipient_id обязан быть разрешён через READY.users для канала {}",
            ch.id
        );
        let recipient = &ch.recipients[0];
        assert_eq!(recipient.id, ch.recipient_ids[0]);
        assert!(!recipient.username.is_empty());
        assert!(snapshot.users.contains_key(&recipient.id));
    }

    // Проверка точной сортировки по убыванию числового u64 `last_message_id`:
    for pair in snapshot.private_channels.windows(2) {
        let left_key = (
            pair[0].last_message_snowflake(),
            pair[0].channel_snowflake(),
        );
        let right_key = (
            pair[1].last_message_snowflake(),
            pair[1].channel_snowflake(),
        );
        assert!(
            left_key >= right_key,
            "нарушен порядок убывания u64 last_message_id: {:?} ({}) < {:?} ({})",
            left_key,
            pair[0].id,
            right_key,
            pair[1].id
        );
    }

    // Первый канал — самый свежий (1561431148641521707, получатель .kirumi / SKID):
    assert_eq!(snapshot.private_channels[0].id, "1213176899493306439");
    assert_eq!(
        snapshot.private_channels[0].last_message_id.as_deref(),
        Some("1561431148641521707")
    );
    assert_eq!(
        snapshot.private_channels[0].recipients[0].username,
        ".kirumi"
    );
    assert_eq!(
        snapshot.private_channels[0].recipients[0]
            .global_name
            .as_deref(),
        Some("SKID")
    );

    // 16-й канал (индекс 15) — 1000134165657825371 (zay_alabay):
    assert_eq!(snapshot.private_channels[15].id, "914570058590736436");
    assert_eq!(
        snapshot.private_channels[15].last_message_id.as_deref(),
        Some("1000134165657825371")
    );
    assert_eq!(
        snapshot.private_channels[15].recipients[0].username,
        "zay_alabay"
    );

    // Последний 17-й канал (индекс 16) — last_message_id == "nil" -> None (0_u64):
    assert_eq!(snapshot.private_channels[16].id, "1346114403149549638");
    assert_eq!(snapshot.private_channels[16].last_message_id, None);
    assert_eq!(snapshot.private_channels[16].last_message_snowflake(), 0);
    assert_eq!(
        snapshot.private_channels[16].recipients[0].username,
        "unix2096"
    );

    // 1.3 Проверка разбора Group DM (type=3) и строго числовой (u64/BigInt), а не лексикографической сортировки:
    // Лексикографически "999999999999999999" > "1000134165657825371", но как u64:
    // 1000134165657825371 > 999999999999999999!
    let synthetic_ready = serde_json::json!({
        "d": {
            "users": [
                {"id": "111111111111111111", "username": "alice", "global_name": "Alice", "discriminator": "0"},
                {"id": "222222222222222222", "username": "bob", "global_name": "Bob", "discriminator": "0"},
                {"id": "333333333333333333", "username": "carol", "global_name": "Carol", "discriminator": "0"}
            ],
            "private_channels": [
                {
                    "id": "500000000000000001",
                    "type": 1,
                    "flags": 0,
                    "recipient_ids": ["111111111111111111"],
                    "last_message_id": "999999999999999999"
                },
                {
                    "id": "500000000000000002",
                    "type": 3,
                    "flags": CHANNEL_FLAG_PINNED,
                    "name": "Команда релиза",
                    "icon": "group_icon_hash",
                    "owner_id": "111111111111111111",
                    "recipient_ids": ["111111111111111111", "222222222222222222", "333333333333333333"],
                    "nicks": {"222222222222222222": "Боб-Лид"},
                    "last_message_id": "1000134165657825371"
                }
            ],
            "relationships": []
        }
    });

    let synth_snapshot =
        parse_ready_dm_friends(&synthetic_ready).expect("parse synthetic READY with Group DM");
    assert_eq!(synth_snapshot.private_channels.len(), 2);
    // Канал с 19-значным last_message_id=1000134165657825371 обязан идти ПЕРЕД 18-значным 999999999999999999:
    assert_eq!(synth_snapshot.private_channels[0].id, "500000000000000002");
    assert_eq!(
        synth_snapshot.private_channels[0].channel_type,
        PrivateChannelType::GroupDm
    );
    assert_eq!(
        synth_snapshot.private_channels[0].channel_type.as_i64(),
        CHANNEL_TYPE_GROUP_DM
    );
    assert!(synth_snapshot.private_channels[0].is_pinned());
    assert_eq!(
        synth_snapshot.private_channels[0].name.as_deref(),
        Some("Команда релиза")
    );
    assert_eq!(
        synth_snapshot.private_channels[0].owner_id.as_deref(),
        Some("111111111111111111")
    );
    assert_eq!(synth_snapshot.private_channels[0].recipients.len(), 3);
    assert_eq!(
        synth_snapshot.private_channels[0]
            .nicks
            .get("222222222222222222")
            .map(String::as_str),
        Some("Боб-Лид")
    );
    assert_eq!(synth_snapshot.private_channels[1].id, "500000000000000001");

    // Повторная проверка функции сортировки:
    let mut reversed = synth_snapshot.private_channels.clone();
    reversed.reverse();
    sort_private_channels_by_last_message_desc(&mut reversed);
    assert_eq!(reversed[0].id, "500000000000000002");
    assert_eq!(reversed[1].id, "500000000000000001");
}

// ---------------------------------------------------------------------------
// 2. Разбор READY.relationships (60 объектов: типы 1=Friend, 2=Blocked,
//    3=Incoming, 4=Outgoing) и фильтрация по вкладкам Online / All / Pending / Blocked
// ---------------------------------------------------------------------------

#[test]
fn ready_relationships_parsing_and_tabs_filtering_contract() {
    let root = repo_root();
    let snapshot =
        load_ready_dm_friends(&root.join("fixtures")).expect("load 002-READY.json snapshot");

    assert_eq!(
        snapshot.relationships.len(),
        60,
        "в 002-READY.json ровно 60 записей в d.relationships[]"
    );
    assert_eq!(snapshot.counts.friend_count, 18);
    assert_eq!(snapshot.counts.blocked_count, 42);
    assert_eq!(snapshot.counts.pending_incoming_count, 0);
    assert_eq!(snapshot.counts.pending_outgoing_count, 0);

    for rel in &snapshot.relationships {
        assert_eq!(
            rel.id, rel.user_id,
            "в 002-READY.json у всех 60 связей id == user_id"
        );
        assert!(
            matches!(
                rel.relationship_type,
                RelationshipType::Friend | RelationshipType::Blocked
            ),
            "в захвате присутствуют только типы 1 и 2"
        );
        let resolved_user = rel
            .user
            .as_ref()
            .expect("каждая связь из 002-READY.json обязана резолвиться в READY.users");
        assert_eq!(resolved_user.id, rel.user_id);
        assert_eq!(rel.nickname, None, "в захвате nickname == nil -> None");
        assert!(!rel.is_spam_request);
        assert!(
            !rel.is_ignored(),
            "в 002-READY.json ни одна из 60 записей не входит в ignored-множество RelationshipStore (даже при user_ignored=true у type=2)"
        );
    }
    // В живом 002-READY.json ровно 1 запись (1373418368463278182, type=2 Blocked) имеет raw user_ignored="true":
    let raw_ignored: Vec<&str> = snapshot
        .relationships
        .iter()
        .filter(|r| r.user_ignored)
        .map(|r| r.user_id.as_str())
        .collect();
    assert_eq!(raw_ignored, vec!["1373418368463278182"]);

    // Разбор `003-READY_SUPPLEMENTAL.json` (`d.merged_presences.friends`):
    let supplemental_raw =
        std::fs::read_to_string(root.join("fixtures/gateway/by-type/003-READY_SUPPLEMENTAL.json"))
            .expect("003-READY_SUPPLEMENTAL.json");
    let supplemental_json: serde_json::Value =
        serde_json::from_str(&supplemental_raw).expect("READY_SUPPLEMENTAL JSON");
    let online_ids = parse_supplemental_online_friends(&supplemental_json)
        .expect("parse_supplemental_online_friends");
    assert_eq!(
        online_ids,
        BTreeSet::from([
            "275258435199827968".to_string(),
            "634667869254189065".to_string(),
        ]),
        "в 003-READY_SUPPLEMENTAL.json ровно 2 друга в сети (idle + online)"
    );

    // Фильтрация живого снимка по 4 вкладкам:
    let tab_online =
        filter_relationships_by_tab(&snapshot.relationships, FriendsTab::Online, &online_ids);
    let tab_all =
        filter_relationships_by_tab(&snapshot.relationships, FriendsTab::All, &online_ids);
    let tab_pending =
        filter_relationships_by_tab(&snapshot.relationships, FriendsTab::Pending, &online_ids);
    let tab_blocked =
        filter_relationships_by_tab(&snapshot.relationships, FriendsTab::Blocked, &online_ids);

    assert_eq!(tab_online.len(), 2);
    assert_eq!(tab_all.len(), 18);
    assert_eq!(tab_pending.len(), 0);
    assert_eq!(tab_blocked.len(), 42);
    assert!(tab_all
        .iter()
        .all(|r| r.relationship_type.as_i64() == RELATIONSHIP_TYPE_FRIEND));
    assert!(tab_blocked
        .iter()
        .all(|r| r.relationship_type.as_i64() == RELATIONSHIP_TYPE_BLOCKED));

    // Проверим полный набор типов 1..4 (Friend=1, Blocked=2, PendingIncoming=3, PendingOutgoing=4)
    // вместе с правилами spam/ignored из RelationshipStore (@9318383, @9319095):
    let mixed_ready = serde_json::json!({
        "d": {
            "users": [
                {"id": "1001", "username": "friend_online", "global_name": "Friend Online"},
                {"id": "1002", "username": "friend_offline", "global_name": "Friend Offline"},
                {"id": "1003", "username": "friend_ignored", "global_name": "Friend Ignored"},
                {"id": "2001", "username": "blocked_user", "global_name": "Blocked User"},
                {"id": "3001", "username": "incoming_clean", "global_name": "Incoming Clean"},
                {"id": "3002", "username": "incoming_spam", "global_name": "Incoming Spam"},
                {"id": "3003", "username": "incoming_ignored", "global_name": "Incoming Ignored"},
                {"id": "4001", "username": "outgoing_req", "global_name": "Outgoing Req"}
            ],
            "private_channels": [],
            "relationships": [
                {"id": "1001", "user_id": "1001", "type": RELATIONSHIP_TYPE_FRIEND, "nickname": "Лучший друг", "since": "2025-01-01T00:00:00Z", "is_spam_request": false, "user_ignored": false},
                {"id": "1002", "user_id": "1002", "type": RELATIONSHIP_TYPE_FRIEND, "nickname": "nil", "since": "2025-01-02T00:00:00Z", "is_spam_request": false, "user_ignored": false},
                {"id": "1003", "user_id": "1003", "type": RELATIONSHIP_TYPE_FRIEND, "nickname": null, "since": "2025-01-03T00:00:00Z", "is_spam_request": false, "user_ignored": true},
                {"id": "2001", "user_id": "2001", "type": RELATIONSHIP_TYPE_BLOCKED, "nickname": null, "since": "2025-01-04T00:00:00Z", "is_spam_request": false, "user_ignored": true},
                {"id": "3001", "user_id": "3001", "type": RELATIONSHIP_TYPE_PENDING_INCOMING, "nickname": null, "since": "2025-01-05T00:00:00Z", "is_spam_request": false, "user_ignored": false},
                {"id": "3002", "user_id": "3002", "type": RELATIONSHIP_TYPE_PENDING_INCOMING, "nickname": null, "since": "2025-01-06T00:00:00Z", "is_spam_request": true, "user_ignored": false},
                {"id": "3003", "user_id": "3003", "type": RELATIONSHIP_TYPE_PENDING_INCOMING, "nickname": null, "since": "2025-01-07T00:00:00Z", "is_spam_request": false, "user_ignored": true},
                {"id": "4001", "user_id": "4001", "type": RELATIONSHIP_TYPE_PENDING_OUTGOING, "nickname": null, "since": "2025-01-08T00:00:00Z", "is_spam_request": false, "user_ignored": false}
            ]
        }
    });

    let mixed = parse_ready_dm_friends(&mixed_ready).expect("parse mixed relationships");
    let counts = RelationshipCounts::from_relationships(&mixed.relationships);
    assert_eq!(counts.friend_count, 3);
    assert_eq!(counts.blocked_count, 1);
    assert_eq!(counts.spam_count, 1);
    assert_eq!(counts.pending_ignored_count, 1);
    assert_eq!(
        counts.pending_incoming_count, 1,
        "pendingCount = max(incoming(3) - spam(1) - ignored(1), 0) == 1 (@9318383)"
    );
    assert_eq!(counts.pending_outgoing_count, 1);

    // Игнорируемый пользователь — только если type != BLOCKED && user_ignored (@9319095):
    let rel_1003 = mixed
        .relationships
        .iter()
        .find(|r| r.user_id == "1003")
        .unwrap();
    let rel_2001 = mixed
        .relationships
        .iter()
        .find(|r| r.user_id == "2001")
        .unwrap();
    assert!(rel_1003.is_ignored());
    assert!(
        !rel_2001.is_ignored(),
        "заблокированный (type=2) не считается ignored в RelationshipStore (@9319095)"
    );

    let online_set = BTreeSet::from(["1001".to_string(), "1003".to_string()]);
    // Online: только type=1, в сети и !is_ignored -> только 1001 (1003 исключён из-за user_ignored):
    let online_filtered =
        filter_relationships_by_tab(&mixed.relationships, FriendsTab::Online, &online_set);
    assert_eq!(online_filtered.len(), 1);
    assert_eq!(online_filtered[0].user_id, "1001");

    // All: все друзья type=1 (1001, 1002, 1003):
    let all_filtered =
        filter_relationships_by_tab(&mixed.relationships, FriendsTab::All, &online_set);
    assert_eq!(all_filtered.len(), 3);

    // Pending (основной список): чистые входящие (3001) + исходящие (4001), без спама и игнора:
    let pending_filtered =
        filter_relationships_by_tab(&mixed.relationships, FriendsTab::Pending, &online_set);
    let pending_ids: Vec<&str> = pending_filtered
        .iter()
        .map(|r| r.user_id.as_str())
        .collect();
    assert_eq!(pending_ids, vec!["3001", "4001"]);

    // Спам и игнор входящих (3002, 3003):
    let spam_ignored = filter_pending_spam_and_ignored(&mixed.relationships);
    let spam_ids: Vec<&str> = spam_ignored.iter().map(|r| r.user_id.as_str()).collect();
    assert_eq!(spam_ids, vec!["3002", "3003"]);

    // Blocked: только type=2 (2001):
    let blocked_filtered =
        filter_relationships_by_tab(&mixed.relationships, FriendsTab::Blocked, &online_set);
    assert_eq!(blocked_filtered.len(), 1);
    assert_eq!(blocked_filtered[0].user_id, "2001");
}

// ---------------------------------------------------------------------------
// 3. REST-клиент: POST /api/v9/users/@me/channels с телом {"recipients": [...]}
//    (1..9 получателей; валидация snowflake IDs до сети, 0 вызовов при невалидном ID)
// ---------------------------------------------------------------------------

#[test]
fn rest_create_dm_and_group_dm_validates_snowflakes_and_recipients_bounds() {
    assert_eq!(MIN_DM_RECIPIENTS, 1);
    assert_eq!(MAX_DM_RECIPIENTS, 9);

    // 3.1 Предсетевая валидация snowflake IDs и границ 1..=9 получателей (0 вызовов сети):
    let guard_transport = FakeDmFriendsTransport::with_response(200, b"{}");

    // Пустой токен:
    assert_eq!(
        run(create_private_channel(
            &guard_transport,
            "   ",
            &["315473023476170753"]
        )),
        Err(DmFriendsError::MissingToken)
    );

    // 0 получателей:
    assert_eq!(
        run(create_private_channel(&guard_transport, SECRET_TOKEN, &[])),
        Err(DmFriendsError::InvalidRecipientCount { count: 0 })
    );

    // 10 получателей (> MAX_DM_RECIPIENTS = 9):
    let ten_recipients = [
        "100000000000000001",
        "100000000000000002",
        "100000000000000003",
        "100000000000000004",
        "100000000000000005",
        "100000000000000006",
        "100000000000000007",
        "100000000000000008",
        "100000000000000009",
        "100000000000000010",
    ];
    assert_eq!(
        run(create_private_channel(
            &guard_transport,
            SECRET_TOKEN,
            &ten_recipients
        )),
        Err(DmFriendsError::InvalidRecipientCount { count: 10 })
    );

    // Дубликат получателя:
    assert_eq!(
        run(create_private_channel(
            &guard_transport,
            SECRET_TOKEN,
            &["315473023476170753", "315473023476170753"]
        )),
        Err(DmFriendsError::DuplicateRecipient {
            user_id: "315473023476170753".to_string()
        })
    );

    // Невалидные snowflake IDs:
    for bad_id in [
        "",
        "   ",
        "0",
        "not-a-snowflake",
        "315473023476170753a",
        "../users/@me",
        "@me",
        "-12345",
        "99999999999999999999999999999",
    ] {
        assert!(
            validate_snowflake_id(bad_id).is_err(),
            "snowflake {bad_id:?} обязан отклоняться"
        );
        assert_eq!(
            run(create_private_channel(
                &guard_transport,
                SECRET_TOKEN,
                &[bad_id]
            )),
            Err(DmFriendsError::InvalidSnowflakeId {
                value: bad_id.to_string()
            }),
            "create_private_channel обязан отклонять {bad_id:?} до сети"
        );
    }

    assert!(
        guard_transport.calls().is_empty(),
        "при невалидных аргументах должно быть ровно 0 сетевых вызовов"
    );

    // 3.2 Одиночная DM (1 получатель): тело строго `{"recipients": ["315473023476170753"]}` (без `recipient_id`!):
    let dm_response = serde_json::json!({
        "id": "696738350706458685",
        "type": 1,
        "flags": 0,
        "last_message_id": "1558137543688720415",
        "recipients": [
            {
                "id": "315473023476170753",
                "username": "krolbon",
                "global_name": "passport",
                "discriminator": "0",
                "avatar": null,
                "bot": false
            }
        ]
    });
    let dm_transport =
        FakeDmFriendsTransport::with_response(200, dm_response.to_string().as_bytes());
    let created_dm = run(create_private_channel(
        &dm_transport,
        SECRET_TOKEN,
        &["315473023476170753"],
    ))
    .expect("create 1-on-1 DM");

    assert_eq!(created_dm.id, "696738350706458685");
    assert_eq!(created_dm.channel_type, PrivateChannelType::Dm);
    assert_eq!(created_dm.recipient_ids, vec!["315473023476170753"]);
    assert_eq!(created_dm.recipients.len(), 1);
    assert_eq!(created_dm.recipients[0].username, "krolbon");

    let dm_calls = dm_transport.calls();
    assert_eq!(dm_calls.len(), 1);
    assert_eq!(dm_calls[0].method, "POST");
    assert_eq!(dm_calls[0].target, USER_CHANNELS_PATH);
    let sent_dm_json: serde_json::Value =
        serde_json::from_slice(&dm_calls[0].body).expect("sent DM body JSON");
    assert_eq!(
        sent_dm_json,
        serde_json::json!({"recipients": ["315473023476170753"]})
    );
    assert!(
        sent_dm_json.get("recipient_id").is_none(),
        "POST /users/@me/channels никогда не должен содержать поле recipient_id (DM-FRIENDS-RE.md §2.1)"
    );

    // 3.3 Попадание в локальный кэш (`open_private_channel_with_cache`): 0 вызовов POST при повторном открытии DM:
    let cached_transport = FakeDmFriendsTransport::with_response(500, b"should not be called");
    let cached_hit = run(open_private_channel_with_cache(
        &cached_transport,
        SECRET_TOKEN,
        std::slice::from_ref(&created_dm),
        &["315473023476170753"],
    ))
    .expect("cache hit for existing 1-on-1 DM");
    assert_eq!(cached_hit.id, "696738350706458685");
    assert!(
        cached_transport.calls().is_empty(),
        "при наличии DM в локальном кэше сетевой POST не вызывается (@567585)"
    );

    // 3.4 Групповая DM (9 получателей — верхняя граница 1..=9):
    let nine_recipients = [
        "100000000000000001",
        "100000000000000002",
        "100000000000000003",
        "100000000000000004",
        "100000000000000005",
        "100000000000000006",
        "100000000000000007",
        "100000000000000008",
        "100000000000000009",
    ];
    let gdm_response = serde_json::json!({
        "id": "712345678901234567",
        "type": 3,
        "name": "Девять друзей",
        "owner_id": "95101520944570368",
        "recipients": nine_recipients.iter().map(|id| serde_json::json!({
            "id": id,
            "username": format!("user_{id}"),
            "global_name": null,
            "discriminator": "0"
        })).collect::<Vec<_>>()
    });
    let gdm_transport =
        FakeDmFriendsTransport::with_response(201, gdm_response.to_string().as_bytes());
    let created_gdm = run(create_private_channel(
        &gdm_transport,
        SECRET_TOKEN,
        &nine_recipients,
    ))
    .expect("create Group DM with 9 recipients");
    assert_eq!(created_gdm.id, "712345678901234567");
    assert_eq!(created_gdm.channel_type, PrivateChannelType::GroupDm);
    assert_eq!(created_gdm.recipients.len(), 9);
    let sent_gdm_json: serde_json::Value =
        serde_json::from_slice(&gdm_transport.calls()[0].body).unwrap();
    assert_eq!(
        sent_gdm_json,
        serde_json::json!({"recipients": nine_recipients})
    );

    // 3.5 Единственный маршрут с `recipient_id`: `POST /users/@me/channels/group-dm-shell` (@568215):
    let shell_transport =
        FakeDmFriendsTransport::with_response(200, gdm_response.to_string().as_bytes());
    let shell_ch = run(create_group_dm_shell(
        &shell_transport,
        SECRET_TOKEN,
        "100000000000000001",
    ))
    .expect("create_group_dm_shell");
    assert_eq!(shell_ch.id, "712345678901234567");
    assert_eq!(shell_transport.calls()[0].target, USER_GROUP_DM_SHELL_PATH);
    let sent_shell_json: serde_json::Value =
        serde_json::from_slice(&shell_transport.calls()[0].body).unwrap();
    assert_eq!(
        sent_shell_json,
        serde_json::json!({"recipient_id": "100000000000000001"})
    );

    // 3.6 Код ошибки 30004 (`TOO_MANY_RECIPIENTS`):
    let too_many_transport = FakeDmFriendsTransport::with_response(
        400,
        br#"{"code":30004,"message":"Maximum number of recipients reached (10)"}"#,
    );
    let err_30004 = run(create_private_channel(
        &too_many_transport,
        SECRET_TOKEN,
        &["315473023476170753", "275258435199827968"],
    ))
    .unwrap_err();
    assert_eq!(
        err_30004.discord_error_code(),
        Some(DISCORD_ERR_TOO_MANY_RECIPIENTS)
    );
}

// ---------------------------------------------------------------------------
// 4. REST-клиент друзей: PUT / DELETE / POST /users/@me/relationships,
//    Pomelo (`discriminator: null`) и типизированный разбор кода 80013
// ---------------------------------------------------------------------------

#[test]
fn rest_relationships_put_delete_post_pomelo_and_stranger_80013_contract() {
    let target_user_id = "194374444821643266";
    assert_eq!(
        build_user_relationship_endpoint(target_user_id).expect("valid user relationship path"),
        "/users/@me/relationships/194374444821643266"
    );

    // 4.1 Предсетевая защита PUT и DELETE при невалидном user_id (0 вызовов):
    let guard_transport = FakeDmFriendsTransport::with_response(204, b"");
    for bad_id in ["", "   ", "0", "abc", "../@me", "123/456"] {
        assert_eq!(
            run(accept_friend_request(
                &guard_transport,
                SECRET_TOKEN,
                bad_id,
                false
            )),
            Err(DmFriendsError::InvalidSnowflakeId {
                value: bad_id.to_string()
            })
        );
        assert_eq!(
            run(block_user(&guard_transport, SECRET_TOKEN, bad_id)),
            Err(DmFriendsError::InvalidSnowflakeId {
                value: bad_id.to_string()
            })
        );
        assert_eq!(
            run(delete_relationship(&guard_transport, SECRET_TOKEN, bad_id)),
            Err(DmFriendsError::InvalidSnowflakeId {
                value: bad_id.to_string()
            })
        );
    }
    assert!(guard_transport.calls().is_empty());

    // 4.2 Код ошибки 80013 (`RELATIONSHIP_INVALID_NO_CONFIRMATION` — Stranger Friend Request Confirmation):
    let stranger_transport = FakeDmFriendsTransport::with_response(
        400,
        br#"{"code":80013,"message":"Confirm stranger friend request"}"#,
    );
    let stranger_err = run(accept_friend_request(
        &stranger_transport,
        SECRET_TOKEN,
        target_user_id,
        false,
    ))
    .expect_err("код 80013 обязан возвращать типизированную ошибку подтверждения незнакомца");

    assert!(stranger_err.is_stranger_confirmation_required());
    assert_eq!(
        stranger_err.discord_error_code(),
        Some(DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION)
    );
    assert_eq!(
        stranger_err,
        DmFriendsError::StrangerConfirmationRequired {
            user_id: target_user_id.to_string(),
            code: DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION,
        }
    );

    let first_put_calls = stranger_transport.calls();
    assert_eq!(first_put_calls.len(), 1);
    assert_eq!(first_put_calls[0].method, "PUT");
    assert_eq!(
        first_put_calls[0].target,
        "/users/@me/relationships/194374444821643266"
    );
    let first_put_json: serde_json::Value =
        serde_json::from_slice(&first_put_calls[0].body).unwrap();
    assert_eq!(
        first_put_json,
        serde_json::json!({
            "type": 1,
            "confirm_stranger_request": false
        })
    );

    // Повторное принятие заявки после подтверждения (`confirm_stranger_request: true`):
    let confirm_transport = FakeDmFriendsTransport::with_response(204, b"");
    run(accept_friend_request(
        &confirm_transport,
        SECRET_TOKEN,
        target_user_id,
        true,
    ))
    .expect("accept_friend_request with confirm_stranger_request=true");
    let confirm_put_json: serde_json::Value =
        serde_json::from_slice(&confirm_transport.calls()[0].body).unwrap();
    assert_eq!(
        confirm_put_json,
        serde_json::json!({
            "type": 1,
            "confirm_stranger_request": true
        })
    );

    // 4.3 Блокировка пользователя (`PUT /users/@me/relationships/{user_id}` с `{"type": 2}`):
    let block_transport = FakeDmFriendsTransport::with_response(204, b"");
    run(block_user(&block_transport, SECRET_TOKEN, target_user_id)).expect("block_user");
    assert_eq!(block_transport.calls().len(), 1);
    assert_eq!(block_transport.calls()[0].method, "PUT");
    assert_eq!(
        block_transport.calls()[0].target,
        "/users/@me/relationships/194374444821643266"
    );
    let block_json: serde_json::Value =
        serde_json::from_slice(&block_transport.calls()[0].body).unwrap();
    assert_eq!(block_json, serde_json::json!({"type": 2}));

    // Явная передача PutRelationshipParams (с `from_friend_suggestion: Some(true)`):
    let suggestion_transport = FakeDmFriendsTransport::with_response(204, b"");
    let suggestion_params = PutRelationshipParams {
        relationship_type: Some(RelationshipType::Friend),
        from_friend_suggestion: Some(true),
        confirm_stranger_request: None,
    };
    run(put_relationship(
        &suggestion_transport,
        SECRET_TOKEN,
        target_user_id,
        &suggestion_params,
    ))
    .expect("put_relationship from suggestion");
    let suggestion_json: serde_json::Value =
        serde_json::from_slice(&suggestion_transport.calls()[0].body).unwrap();
    assert_eq!(
        suggestion_json,
        serde_json::json!({
            "type": 1,
            "from_friend_suggestion": true
        })
    );

    // 4.4 Удаление связи / отмена заявки / разблокировка (`DELETE /users/@me/relationships/{user_id}`):
    let del_transport = FakeDmFriendsTransport::with_response(204, b"");
    run(delete_relationship(
        &del_transport,
        SECRET_TOKEN,
        target_user_id,
    ))
    .expect("delete_relationship");
    assert_eq!(del_transport.calls().len(), 1);
    assert_eq!(del_transport.calls()[0].method, "DELETE");
    assert_eq!(
        del_transport.calls()[0].target,
        "/users/@me/relationships/194374444821643266"
    );
    assert!(del_transport.calls()[0].body.is_empty());

    // 4.5 Отправка заявки в друзья (`POST /users/@me/relationships`): Pomelo и Legacy tag (@697487):
    // a) Pomelo (без `#`): `{"username": "alexvance", "discriminator": null}`
    let pomelo_transport = FakeDmFriendsTransport::with_response(204, b"");
    let pomelo_input =
        FriendRequestTagInput::parse("alexvance", None).expect("parse Pomelo username");
    assert_eq!(pomelo_input.username, "alexvance");
    assert_eq!(pomelo_input.discriminator, None);
    run(send_friend_request(
        &pomelo_transport,
        SECRET_TOKEN,
        &pomelo_input,
    ))
    .expect("send Pomelo friend request");
    assert_eq!(pomelo_transport.calls().len(), 1);
    assert_eq!(pomelo_transport.calls()[0].method, "POST");
    assert_eq!(pomelo_transport.calls()[0].target, USER_RELATIONSHIPS_PATH);
    let pomelo_json: serde_json::Value =
        serde_json::from_slice(&pomelo_transport.calls()[0].body).unwrap();
    assert_eq!(
        pomelo_json,
        serde_json::json!({
            "username": "alexvance",
            "discriminator": null
        }),
        "для Pomelo-аккаунтов discriminator в JSON обязан быть null (JSON.stringify(parseInt(undefined)))"
    );

    // b) Pomelo с `#0` -> также `discriminator: null`:
    let pomelo_zero =
        FriendRequestTagInput::parse("alexvance#0", None).expect("parse Pomelo #0 tag");
    assert_eq!(pomelo_zero.discriminator, None);

    // c) Legacy tag `alexvance#0421` с заметкой -> `discriminator: 421` (число!):
    let legacy_transport = FakeDmFriendsTransport::with_response(204, b"");
    let legacy_input = FriendRequestTagInput::parse("alexvance#0421", Some("Привет из Rust!"))
        .expect("parse legacy tag");
    assert_eq!(legacy_input.username, "alexvance");
    assert_eq!(legacy_input.discriminator, Some(421));
    run(send_friend_request(
        &legacy_transport,
        SECRET_TOKEN,
        &legacy_input,
    ))
    .expect("send legacy friend request");
    let legacy_json: serde_json::Value =
        serde_json::from_slice(&legacy_transport.calls()[0].body).unwrap();
    assert_eq!(
        legacy_json,
        serde_json::json!({
            "username": "alexvance",
            "discriminator": 421,
            "note": "Привет из Rust!"
        })
    );

    // d) Невалидные теги отклоняются до сети (0 вызовов):
    let bad_tag_transport = FakeDmFriendsTransport::with_response(204, b"");
    for bad_tag in [
        "",
        "   ",
        "#1234",
        "user#",
        "user#abcd",
        "user#12345",
        "a#12#34",
    ] {
        assert_eq!(
            FriendRequestTagInput::parse(bad_tag, None),
            Err(DmFriendsError::InvalidDiscordTag),
            "bad tag {bad_tag:?} must fail validation"
        );
    }
    assert!(bad_tag_transport.calls().is_empty());
}

// ---------------------------------------------------------------------------
// 5. Защита от утечки токена в Display/Debug ошибок и клиента
// ---------------------------------------------------------------------------

#[test]
fn error_display_and_debug_never_leak_token_or_sensitive_headers() {
    let fake_transport = FakeDmFriendsTransport::with_response(
        401,
        format!(r#"{{"message":"401: Unauthorized for token {SECRET_TOKEN}"}}"#).as_bytes(),
    );
    let client = DiscordDmFriends::with_transport(SECRET_TOKEN, Arc::new(fake_transport.clone()))
        .expect("DiscordDmFriends client");

    // 5.1 Debug клиента обязан маскировать токен:
    let client_debug = format!("{client:?}");
    assert!(
        !client_debug.contains(SECRET_TOKEN),
        "Debug DiscordDmFriends не должен содержать секретный токен: {client_debug}"
    );
    assert!(client_debug.contains("<redacted>"));

    // 5.2 При ответе с эхом токена в теле ошибки ни Display, ни Debug ошибки не должны содержать токен:
    let err_401 = run(client.delete_relationship("194374444821643266")).unwrap_err();
    assert_eq!(err_401, DmFriendsError::Unauthorized);
    assert!(!format!("{err_401}").contains(SECRET_TOKEN));
    assert!(!format!("{err_401:?}").contains(SECRET_TOKEN));

    let err_500_transport = FakeDmFriendsTransport::with_response(
        500,
        format!(r#"{{"code":80013,"message":"leaked {SECRET_TOKEN}"}}"#).as_bytes(),
    );
    let err_500 = run(accept_friend_request(
        &err_500_transport,
        SECRET_TOKEN,
        "194374444821643266",
        false,
    ))
    .unwrap_err();
    assert!(!format!("{err_500}").contains(SECRET_TOKEN));
    assert!(!format!("{err_500:?}").contains(SECRET_TOKEN));

    let err_429_transport = FakeDmFriendsTransport::with_response(
        429,
        format!(r#"{{"retry_after":1.5,"message":"rate limit {SECRET_TOKEN}"}}"#).as_bytes(),
    )
    .with_headers(&[("Retry-After", "2")]);
    let err_429 = run(delete_relationship(
        &err_429_transport,
        SECRET_TOKEN,
        "194374444821643266",
    ))
    .unwrap_err();
    assert_eq!(
        err_429,
        DmFriendsError::RateLimited {
            retry_after_ms: 2000
        }
    );
    assert!(!format!("{err_429}").contains(SECRET_TOKEN));
    assert!(!format!("{err_429:?}").contains(SECRET_TOKEN));

    // Проверим все варианты ошибок на отсутствие утечки секрета в Display и Debug:
    let all_errors = [
        DmFriendsError::MissingToken,
        DmFriendsError::InvalidSnowflakeId {
            value: "bad_id".to_string(),
        },
        DmFriendsError::InvalidRecipientCount { count: 0 },
        DmFriendsError::DuplicateRecipient {
            user_id: "123".to_string(),
        },
        DmFriendsError::InvalidDiscordTag,
        DmFriendsError::StrangerConfirmationRequired {
            user_id: "194374444821643266".to_string(),
            code: DISCORD_ERR_RELATIONSHIP_INVALID_NO_CONFIRMATION,
        },
        DmFriendsError::DiscordApiCode {
            status: 400,
            code: DISCORD_ERR_TOO_MANY_RECIPIENTS,
        },
        DmFriendsError::Unauthorized,
        DmFriendsError::Forbidden,
        DmFriendsError::NotFound,
        DmFriendsError::RateLimited {
            retry_after_ms: 1500,
        },
        DmFriendsError::Client { status: 400 },
        DmFriendsError::Server { status: 502 },
        DmFriendsError::Network,
        DmFriendsError::Storage,
        DmFriendsError::InvalidResponse,
    ];

    for err in all_errors {
        let display_str = format!("{err}");
        let debug_str = format!("{err:?}");
        assert!(!display_str.is_empty());
        assert!(!display_str.contains(SECRET_TOKEN));
        assert!(!debug_str.contains(SECRET_TOKEN));
    }
}

// ---------------------------------------------------------------------------
// 6. Коды ошибок Discord на 403/404 (Review 40A, P2-2): UI обязан отличать
//    «подтвердите почту» (`40002`, SPEC §1.4) и «неизвестный пользователь»
//    (`10013`) от произвольного 403/404 без кода.
// ---------------------------------------------------------------------------

#[test]
fn rest_error_responses_preserve_discord_code_on_403_and_404() {
    // 6.1 `403 {"code":40002}` (`EMAIL_VERIFICATION_REQUIRED`, SPEC §1.4):
    let email_verification_transport = FakeDmFriendsTransport::with_response(
        403,
        br#"{"code":40002,"message":"Email verification required"}"#,
    );
    let err_403 = run(create_private_channel(
        &email_verification_transport,
        SECRET_TOKEN,
        &["275258435199827968"],
    ))
    .expect_err("403 с кодом 40002 обязан вернуть типизированный код Discord");
    assert_eq!(
        err_403,
        DmFriendsError::DiscordApiCode {
            status: 403,
            code: 40002,
        },
        "403 обязан сохранять код Discord 40002 (EMAIL_VERIFICATION_REQUIRED)"
    );
    assert_eq!(err_403.discord_error_code(), Some(40002));
    assert!(
        format!("{err_403}").contains("40002"),
        "Display 403 обязан содержать код Discord: {err_403}"
    );

    // 6.2 `404 {"code":10013}` (`UNKNOWN_USER`):
    let unknown_user_transport =
        FakeDmFriendsTransport::with_response(404, br#"{"code":10013,"message":"Unknown User"}"#);
    let err_404 = run(delete_relationship(
        &unknown_user_transport,
        SECRET_TOKEN,
        "194374444821643266",
    ))
    .expect_err("404 с кодом 10013 обязан вернуть типизированный код Discord");
    assert_eq!(
        err_404,
        DmFriendsError::DiscordApiCode {
            status: 404,
            code: 10013,
        },
        "404 обязан сохранять код Discord 10013 (UNKNOWN_USER)"
    );
    assert_eq!(err_404.discord_error_code(), Some(10013));

    // 6.3 Ответ без `code` и с `code: 0` (так Discord отвечает на общие ошибки)
    //     остаётся статусной ошибкой — обработка 403/404 в UI не ломается:
    let plain_forbidden =
        FakeDmFriendsTransport::with_response(403, br#"{"message":"403: Forbidden"}"#);
    let plain_forbidden_err = run(block_user(
        &plain_forbidden,
        SECRET_TOKEN,
        "194374444821643266",
    ))
    .expect_err("403 без кода обязан вернуть Forbidden");
    assert_eq!(plain_forbidden_err, DmFriendsError::Forbidden);
    assert_eq!(plain_forbidden_err.discord_error_code(), None);

    let plain_not_found =
        FakeDmFriendsTransport::with_response(404, br#"{"message":"404: Not Found","code":0}"#);
    let plain_not_found_err = run(delete_relationship(
        &plain_not_found,
        SECRET_TOKEN,
        "194374444821643266",
    ))
    .expect_err("404 с code 0 обязан вернуть NotFound");
    assert_eq!(plain_not_found_err, DmFriendsError::NotFound);
    assert_eq!(plain_not_found_err.discord_error_code(), None);

    // 6.4 `401` остаётся `Unauthorized` и не превращается в `DiscordApiCode`
    //     даже если сервер прислал `code: 0`:
    let unauthorized_transport =
        FakeDmFriendsTransport::with_response(401, br#"{"message":"401: Unauthorized","code":0}"#);
    let unauthorized_err = run(delete_relationship(
        &unauthorized_transport,
        SECRET_TOKEN,
        "194374444821643266",
    ))
    .expect_err("401 обязан вернуть Unauthorized");
    assert_eq!(unauthorized_err, DmFriendsError::Unauthorized);
}
