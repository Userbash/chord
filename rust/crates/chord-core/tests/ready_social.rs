//! B-часть контракта A1 (RW-12 / P2-01b): `users`, `private_channels`, `relationships`,
//! `sessions`, `merged_members` — кейсы B1–B12 дизайна §5.2 и ТЗ P2-01 (P-3, B12 из A8).
//!
//! Оракул capture-кейсов — числа jq по `fixtures/gateway/by-type/002-READY.json`
//! (`md5 a1a8fd5662be88108dd25ac619a377f2`, дизайн §1–§2); синтетика — контракт §2.3.
//! Парсеры `social` — `pub(crate)`, поэтому все проверки идут через публичный `Ready::parse`
//! (ТЗ P2-01 §1, P-2). Битые элементы коллекций пропускаются с `diagnostics.malformed`,
//! неверный тип необязательной коллекции — пусто + `diagnostics.ignored`; `nil` — пусто без
//! диагностики (§2.3).

mod support;

use chord_core::ready::{Ready, ReadyError, value};

/// Разобрать синтетический payload `d` через публичный `Ready::parse`.
fn parse_d(json: &str) -> Ready {
    let payload = support::ready_fixture::parse_json(json).expect("синтетика должна парситься");
    Ready::parse(&payload).expect("синтетика должна разбираться")
}

/// Реальный READY из курированной фикстуры (полный op0-кадр).
fn ready() -> Ready {
    let frame = support::ready_fixture::load_ready().expect("READY-фикстура читается");
    Ready::from_dispatch(&frame).expect("READY-фикстура разбирается")
}

fn malformed(ready: &Ready) -> Vec<&str> {
    ready
        .diagnostics
        .malformed
        .iter()
        .map(String::as_str)
        .collect()
}

fn duplicates(ready: &Ready) -> Vec<&str> {
    ready
        .diagnostics
        .duplicates
        .iter()
        .map(String::as_str)
        .collect()
}

fn ignored(ready: &Ready) -> Vec<&str> {
    ready
        .diagnostics
        .ignored
        .iter()
        .map(String::as_str)
        .collect()
}

/// B1: 146 users; 41 без `bot` → `bot=false`; 44 без `display_name` → `None`; 86 без `clan`
/// (не типизирован, остаётся в raw).
#[test]
fn b01_users_fixture_partial_fields() {
    let ready = ready();
    assert_eq!(ready.users.len(), 146);

    // jq: bot=true 4, bot=false 101, ключа нет 41 → оба последних вида дают false.
    assert_eq!(ready.users.iter().filter(|user| user.bot).count(), 4);
    assert_eq!(ready.users.iter().filter(|user| !user.bot).count(), 142);

    let raw_users = value::items_or_empty(ready.raw().get("users"));
    assert_eq!(raw_users.len(), 146);

    // jq: ключа `display_name` нет у 44, ещё у 5 — атом `"nil"`; оба вида → `None` (49).
    assert_eq!(
        raw_users
            .iter()
            .filter(|user| user.get("display_name").is_none())
            .count(),
        44
    );
    assert_eq!(
        raw_users
            .iter()
            .filter(|user| user.get("display_name").is_some_and(value::is_null))
            .count(),
        5
    );
    assert_eq!(
        ready
            .users
            .iter()
            .filter(|user| user.display_name.is_none())
            .count(),
        49
    );
    assert_eq!(
        raw_users
            .iter()
            .filter(|user| user.get("clan").is_some())
            .count(),
        60
    );
    assert_eq!(
        raw_users
            .iter()
            .filter(|user| user.get("clan").is_none())
            .count(),
        86
    );

    // Именно отсутствие `bot` (а не явный `"false"`) даёт типизированный false.
    let missing_bot_ids: Vec<u64> = raw_users
        .iter()
        .filter(|user| user.get("bot").is_none())
        .filter_map(|user| user.get("id").and_then(value::unsigned))
        .collect();
    assert_eq!(missing_bot_ids.len(), 41);
    for id in missing_bot_ids {
        let typed = ready
            .users
            .iter()
            .find(|user| user.id == id)
            .expect("user из users[] найден по id");
        assert!(!typed.bot, "отсутствие bot → bot=false (id {id})");
    }
}

/// B2: top-level user — 32 ключа в raw; точные поля; `email` не типизирован (только raw).
#[test]
fn b02_top_level_user_fixture() {
    let ready = ready();
    let user = &ready.user;
    assert_eq!(user.id, 384_299_970_632_876_032);
    assert_eq!(user.username.as_deref(), Some("pharos12510"));
    assert_eq!(user.global_name.as_deref(), Some("ShadowByte"));
    assert_eq!(user.discriminator.as_deref(), Some("0"));
    assert_eq!(
        user.avatar.as_deref(),
        Some("e234430751874dcfca60cdd3a53a2673")
    );
    assert!(!user.bot);
    assert_eq!(user.public_flags, None, "в top-level user ключа нет");
    assert_eq!(user.display_name, None, "в top-level user ключа нет");

    let raw_user = ready.raw().get("user").expect("raw user");
    assert_eq!(value::object(raw_user).map(<[_]>::len), Some(32));
    // `email` остаётся только в raw (в фикстуре — REDACTED), в модели поля нет.
    assert!(matches!(
        raw_user.get("email"),
        Some(chord_codec::etf::EtfValue::Str(text)) if text == "REDACTED"
    ));
}

/// B3: 17 ЛС; все `type=1`; `last_pin_timestamp` 2/17; `last_message_id="nil"` 1/17 → `None`;
/// `recipient_ids` непусты; точечная сверка первого канала с jq.
#[test]
fn b03_private_channels_fixture() {
    let ready = ready();
    let channels = &ready.private_channels;
    assert_eq!(channels.len(), 17);
    assert!(channels.iter().all(|channel| channel.channel_type == 1));
    assert_eq!(
        channels
            .iter()
            .filter(|channel| channel.last_pin_timestamp.is_some())
            .count(),
        2
    );
    assert_eq!(
        channels
            .iter()
            .filter(|channel| channel.last_message_id.is_none())
            .count(),
        1
    );
    assert!(
        channels
            .iter()
            .all(|channel| !channel.recipient_ids.is_empty())
    );

    let nil_message = channels
        .iter()
        .find(|channel| channel.id == 1_346_114_403_149_549_638)
        .expect("канал с last_message_id=\"nil\"");
    assert_eq!(nil_message.last_message_id, None);

    // jq `.d.private_channels[0]`.
    let first = &channels[0];
    assert_eq!(first.id, 696_738_350_706_458_685);
    assert_eq!(first.channel_type, 1);
    assert_eq!(first.flags, 0);
    assert_eq!(first.recipient_ids, vec![315_473_023_476_170_753]);
    assert_eq!(first.last_message_id, Some(1_558_137_543_688_720_415));
    assert_eq!(first.last_pin_timestamp, None);
    assert!(!first.is_message_request);
    assert_eq!(first.is_message_request_timestamp, None);
    assert!(!first.is_spam);
    assert_eq!(first.recipient_flags, 0);
}

/// B4: 60 связей; `type` 1:18, 2:42; первый id/user_id и `since`; `nickname=None` 60/60;
/// `is_spam_request=false` 60/60; `user_ignored` true 1 / false 59.
#[test]
fn b04_relationships_fixture() {
    let ready = ready();
    let relationships = &ready.relationships;
    assert_eq!(relationships.len(), 60);
    assert_eq!(
        relationships
            .iter()
            .filter(|relation| relation.relation_type == 1)
            .count(),
        18
    );
    assert_eq!(
        relationships
            .iter()
            .filter(|relation| relation.relation_type == 2)
            .count(),
        42
    );
    assert!(
        relationships
            .iter()
            .all(|relation| relation.nickname.is_none())
    );
    assert!(
        relationships
            .iter()
            .all(|relation| !relation.is_spam_request)
    );
    assert_eq!(
        relationships
            .iter()
            .filter(|relation| relation.user_ignored)
            .count(),
        1
    );

    let ignored_relation = relationships
        .iter()
        .find(|relation| relation.id == 1_373_418_368_463_278_182)
        .expect("связь с user_ignored=true");
    assert!(ignored_relation.user_ignored);
    assert!(!ignored_relation.is_spam_request);

    // jq `.d.relationships[0]`.
    let first = &relationships[0];
    assert_eq!(first.id, 194_374_444_821_643_266);
    assert_eq!(first.user_id, 194_374_444_821_643_266);
    assert_eq!(first.relation_type, 1);
    assert_eq!(
        first.since.as_deref(),
        Some("2025-06-21T06:48:57.474000+00:00")
    );
}

/// B5: 1 сессия; `client_info={desktop,linux,0}`; `status=invisible`;
/// `processed_at_timestamp=0`; `session_id` не печатается в `Debug`.
#[test]
fn b05_sessions_fixture_redacted_debug() {
    let ready = ready();
    assert_eq!(ready.sessions.len(), 1);
    let session = &ready.sessions[0];
    assert_eq!(session.session_id, "REDACTED");
    assert_eq!(session.status.as_deref(), Some("invisible"));
    assert_eq!(session.client_info.client.as_deref(), Some("desktop"));
    assert_eq!(session.client_info.os.as_deref(), Some("linux"));
    assert_eq!(session.client_info.version, 0);
    assert_eq!(session.processed_at_timestamp, 0);

    let debug = format!("{session:?}");
    assert!(
        !debug.contains("REDACTED"),
        "Debug сессии не печатает session_id: {debug}"
    );
    assert!(debug.contains("<redacted>"));
}

/// B6: `merged_members` — 7 групп по 1; группа 5 прикреплена к ITSKIDON; поля участника.
#[test]
fn b06_merged_members_fixture() {
    let ready = ready();
    assert_eq!(ready.guilds.len(), 7);
    assert!(ready.guilds.iter().all(|guild| guild.members.len() == 1));

    let guild = &ready.guilds[5];
    assert_eq!(
        guild.id, 1_014_537_851_418_980_372,
        "merged_members[5] → guilds[5] (ITSKIDON)"
    );
    let member = &guild.members[0];
    assert_eq!(member.user_id, 384_299_970_632_876_032);
    assert_eq!(member.nick.as_deref(), Some("ShadowByte"));
    assert_eq!(member.avatar, None);
    assert_eq!(member.banner, None);
    assert_eq!(
        member.joined_at.as_deref(),
        Some("2023-12-25T17:19:01.733000+00:00")
    );
    assert_eq!(member.premium_since, None);
    assert_eq!(member.communication_disabled_until, None);
    assert!(!member.deaf);
    assert!(!member.mute);
    assert!(!member.pending);
    assert_eq!(member.flags, 2);
    assert_eq!(member.roles.len(), 8);
    assert_eq!(member.roles[0], 1_088_036_342_321_123_398);
}

/// B7: синтетика 3 гильдии / 2 группы — первые две прикреплены, третья пуста,
/// расхождение длин попадает в `malformed`, паники нет.
#[test]
fn b07_merged_members_length_mismatch_synthetic() {
    let ready = parse_d(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"1"},{"id":"2"},{"id":"3"}],
            "merged_members":[[{"user_id":"10"}],[{"user_id":"20"}]]}"#,
    );
    assert_eq!(ready.guilds.len(), 3);
    let ids = |index: usize| -> Vec<u64> {
        ready.guilds[index]
            .members
            .iter()
            .map(|member| member.user_id)
            .collect()
    };
    assert_eq!(ids(0), vec![10]);
    assert_eq!(ids(1), vec![20]);
    assert!(ready.guilds[2].members.is_empty());
    assert!(
        malformed(&ready)
            .iter()
            .any(|path| path.contains("merged_members")),
        "расхождение длин — malformed: {:?}",
        ready.diagnostics.malformed
    );
}

/// B8: дубликат id в `users` и `user_id` в группе `merged_members` — last wins,
/// позиция первого, запись в `duplicates`, число элементов уменьшается.
#[test]
fn b08_duplicates_last_wins_synthetic() {
    let ready = parse_d(
        r#"{"user":{"id":"1"},"session_id":"s",
            "guilds":[{"id":"1"}],
            "users":[{"id":"1","username":"a"},{"id":"1","username":"b"}],
            "merged_members":[[{"user_id":"5","nick":"a"},{"user_id":"5","nick":"b"}]]}"#,
    );
    assert_eq!(ready.users.len(), 1);
    assert_eq!(ready.users[0].username.as_deref(), Some("b"));

    assert_eq!(ready.guilds[0].members.len(), 1);
    assert_eq!(ready.guilds[0].members[0].nick.as_deref(), Some("b"));

    let duplicates = duplicates(&ready);
    assert!(
        duplicates.contains(&"users:1"),
        "duplicates: {duplicates:?}"
    );
    assert!(
        duplicates.contains(&"merged_members[0].members:5"),
        "duplicates: {duplicates:?}"
    );
}

/// B9: partial user `{id:"1"}` в `users[]` — `Ok`; остальные поля дефолтные.
#[test]
fn b09_partial_user_synthetic() {
    let ready = parse_d(r#"{"user":{"id":"1"},"guilds":[],"session_id":"s","users":[{"id":"1"}]}"#);
    assert_eq!(ready.users.len(), 1);
    let user = &ready.users[0];
    assert_eq!(user.id, 1);
    assert_eq!(user.username, None);
    assert_eq!(user.global_name, None);
    assert_eq!(user.display_name, None);
    assert_eq!(user.discriminator, None);
    assert_eq!(user.avatar, None);
    assert!(!user.bot);
    assert_eq!(user.public_flags, None);
    assert_eq!(malformed(&ready), Vec::<&str>::new());
    assert_eq!(ignored(&ready), Vec::<&str>::new());
}

/// B10: битый id элемента `users[0].id="abc"` / отрицательный — пропуск + `malformed`;
/// top-level `user.id` битый/отрицательный — `Err InvalidId` (регрессия P-6, P-3).
#[test]
fn b10_broken_user_id_synthetic() {
    let ready =
        parse_d(r#"{"user":{"id":"1"},"guilds":[],"session_id":"s","users":[{"id":"abc"}]}"#);
    assert!(ready.users.is_empty());
    assert_eq!(malformed(&ready), vec!["users[0]"]);

    // Отрицательный big-int старого парсера — тоже битый элемент (не паника).
    let negative = parse_d(
        r#"{"user":{"id":"1"},"guilds":[],"session_id":"s",
            "users":[{"id":-1467272931325252608}]}"#,
    );
    assert!(negative.users.is_empty());
    assert_eq!(malformed(&negative), vec!["users[0]"]);

    // Верхнеуровневый якорь: нечисловой id.
    let payload =
        support::ready_fixture::parse_json(r#"{"user":{"id":"abc"},"guilds":[],"session_id":"s"}"#)
            .expect("синтетика парсится");
    let error = Ready::parse(&payload).expect_err("битый d.user.id обязан быть фатальным");
    assert!(
        matches!(&error, ReadyError::InvalidId { path, .. } if path == "d.user.id"),
        "ожидался InvalidId(d.user.id), получено {error:?}"
    );

    // Верхнеуровневый якорь: отрицательный id (Int).
    let payload = support::ready_fixture::parse_json(
        r#"{"user":{"id":-1467272931325252608},"guilds":[],"session_id":"s"}"#,
    )
    .expect("синтетика парсится");
    let error = Ready::parse(&payload).expect_err("отрицательный d.user.id невалиден");
    assert!(
        matches!(&error, ReadyError::InvalidId { path, .. } if path == "d.user.id"),
        "ожидался InvalidId(d.user.id), получено {error:?}"
    );
}

/// B11: `users`/`private_channels` (и прочие опциональные коллекции) отсутствуют или `nil` —
/// `Ok`, пустые `Vec`, пустая диагностика.
#[test]
fn b11_absent_or_nil_collections_synthetic() {
    let absent = parse_d(r#"{"user":{"id":"1"},"guilds":[],"session_id":"s"}"#);
    assert!(absent.users.is_empty());
    assert!(absent.private_channels.is_empty());
    assert!(absent.relationships.is_empty());
    assert!(absent.sessions.is_empty());
    assert_eq!(
        absent.diagnostics,
        chord_core::ready::ReadyDiagnostics::default()
    );

    let nil = parse_d(
        r#"{"user":{"id":"1"},"guilds":[],"session_id":"s",
            "users":"nil","private_channels":"nil","relationships":"nil",
            "sessions":"nil","merged_members":"nil"}"#,
    );
    assert!(nil.users.is_empty());
    assert!(nil.private_channels.is_empty());
    assert!(nil.relationships.is_empty());
    assert!(nil.sessions.is_empty());
    assert_eq!(
        nil.diagnostics,
        chord_core::ready::ReadyDiagnostics::default()
    );
}

/// B12 (из A8): неверный тип необязательной коллекции — пусто + `ignored`, без `Err`.
/// `relationships:"nil"` — атом `nil`, то есть «нет значения»: пусто без диагностики (B11).
#[test]
fn b12_wrong_type_collections_synthetic() {
    let ready = parse_d(
        r#"{"user":{"id":"1"},"session_id":"s","guilds":[],
            "users":5,"private_channels":5,"relationships":"nil","sessions":5,
            "merged_members":5}"#,
    );
    assert!(ready.users.is_empty());
    assert!(ready.private_channels.is_empty());
    assert!(ready.relationships.is_empty());
    assert!(ready.sessions.is_empty());

    let ignored = ignored(&ready);
    for path in [
        "d.users",
        "d.private_channels",
        "d.sessions",
        "d.merged_members",
    ] {
        assert!(ignored.contains(&path), "ignored {path}: {ignored:?}");
    }
    assert!(
        !ignored.contains(&"d.relationships"),
        "атом nil — пусто без диагностики: {ignored:?}"
    );
}

/// B12: строка (не `nil`-атом) в `relationships` — неверный тип → `ignored`.
#[test]
fn b12_wrong_type_relationships_synthetic() {
    let ready = parse_d(r#"{"user":{"id":"1"},"session_id":"s","guilds":[],"relationships":5}"#);
    assert!(ready.relationships.is_empty());
    assert_eq!(ignored(&ready), vec!["d.relationships"]);
}
