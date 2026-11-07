//! Контракт BFF фазы 3 (W39e): данные строго из `fixtures/`, без сети.
//!
//! Тесты сверяют ответы публичного API `discord_gui_spike::data` с независимым
//! оракулом — самими файлами фикстур (`ready-stats.json`, READY, by-type), а не
//! с текущей реализацией. Покрыты:
//! - `ready` — счётчики READY против `gateway/ready-stats.json`;
//! - `messages_page`/`members_page` — курсорная пагинация, дедупликация, пустая
//!   страница как валидный ответ, границы `limit`, неизвестный id;
//! - W39i: поля сообщений (`embeds`/`attachments`/`reactions`/`replyTo`/
//!   `editedAt`/`day`/`group`) и участников (`roles`/`group`/счётчики групп)
//!   сверяются с реальными кадрами `gateway/by-type`; отсутствующие в фикстурах
//!   сущности (вложения, реакции, ответы, правки) обязаны быть пустыми, а не
//!   выдуманными;
//! - `search_page` — все 7 фильтров R-10, пагинация, пустой результат, лимиты;
//! - `settings` — секции и счётчики, неизвестная секция, R5 (`section ≤ 256`);
//! - типизированные ошибки `Io`/`Parse`/`NotFound`/`InvalidArg` и их JSON.
//!
//! Skip-aware: без `fixtures/` тесты, которым нужны реальные файлы, печатают
//! причину и возвращаются; при `DL_STRICT=1` — падают (CI-режим).

use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use discord_gui_spike::data::{
    self, ChannelAccessPolicy, DataError, SearchFilters, DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT,
    PERM_ADD_REACTIONS, PERM_ADMINISTRATOR, PERM_ATTACH_FILES, PERM_CONNECT,
    PERM_CREATE_INSTANT_INVITE, PERM_READ_MESSAGE_HISTORY, PERM_SEND_MESSAGES, PERM_SPEAK,
    PERM_USE_VAD, PERM_VIEW_CHANNEL, UI_WINDOW_ROWS,
};
use serde_json::{json, Value};

fn live_test_lock() -> &'static RwLock<()> {
    static LOCK: OnceLock<RwLock<()>> = OnceLock::new();
    LOCK.get_or_init(|| RwLock::new(()))
}

thread_local! {
    static THREAD_READ_LOCKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct ScopedRoot {
    path: PathBuf,
    _guard: Option<RwLockReadGuard<'static, ()>>,
}

impl Drop for ScopedRoot {
    fn drop(&mut self) {
        self._guard.take();
        THREAD_READ_LOCKS.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

fn acquire_live_read_guard() -> Option<RwLockReadGuard<'static, ()>> {
    THREAD_READ_LOCKS.with(|c| {
        let prev = c.get();
        c.set(prev + 1);
        if prev == 0 {
            Some(
                live_test_lock()
                    .read()
                    .unwrap_or_else(|poison| poison.into_inner()),
            )
        } else {
            None
        }
    })
}

impl Deref for ScopedRoot {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for ScopedRoot {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

fn acquire_live_write_guard() -> RwLockWriteGuard<'static, ()> {
    live_test_lock()
        .write()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// Корень фикстур: `DL_FIXTURES` или `fixtures/` на два уровня выше crate.
fn repo_fixtures_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest
        .parent()
        .and_then(Path::parent)
        .expect("crate лежит в rust/gui-spike");
    repo.join("fixtures")
}

fn candidate_root() -> PathBuf {
    std::env::var("DL_FIXTURES")
        .ok()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(repo_fixtures_root)
}

fn fixtures_root() -> Option<ScopedRoot> {
    let root = candidate_root();
    if root.join("gateway/by-type/002-READY.json").is_file() {
        let guard = acquire_live_read_guard();
        Some(ScopedRoot {
            path: root,
            _guard: guard,
        })
    } else {
        None
    }
}

/// `None` — фикстур нет: печатаем причину и выходим; при `DL_STRICT=1` — паника.
macro_rules! fixtures_or_skip {
    () => {
        match fixtures_root() {
            Some(root) => root,
            None => {
                assert!(
                    std::env::var("DL_STRICT").ok().as_deref() != Some("1"),
                    "DL_STRICT=1: фикстуры обязательны, но {} не найден",
                    candidate_root().display()
                );
                eprintln!(
                    "SKIP data_contract: нет фикстур в {} (DL_STRICT≠1)",
                    candidate_root().display()
                );
                return;
            }
        }
    };
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("не читается {}: {err}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|err| panic!("не парсится {}: {err}", path.display()))
}

/// Временный корень фикстур под `target/tmp`-подобным каталогом ОС.
fn temp_root(name: &str) -> ScopedRoot {
    let guard = acquire_live_read_guard();
    let root = std::env::temp_dir().join(format!("dl-w39e-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог создаётся");
    ScopedRoot {
        path: root,
        _guard: guard,
    }
}

/// Убираем за собой; ошибки удаления не влияют на проверку — это best-effort.
fn cleanup(root: &Path) {
    let _ = fs::remove_dir_all(root);
}

fn kind(err: &DataError) -> &'static str {
    err.kind()
}

/// Минимальный READY с одним каналом/гильдией — каркас для негативных кейсов,
/// чтобы ошибка возникала именно в проверяемом месте, а не в READY.
fn write_minimal_ready(root: &Path, channel_id: &str, guild_id: &str) {
    let ready = serde_json::json!({
        "t": "READY",
        "d": {
            "user": {"id": "1", "username": "tester"},
            "guilds": [{
                "id": guild_id,
                "properties": {"name": "Тестовая гильдия"},
                "channels": [{"id": channel_id}],
            }],
            "private_channels": [],
        },
    });
    fs::write(
        root.join("gateway/by-type/002-READY.json"),
        ready.to_string(),
    )
    .expect("пишем минимальный READY");
}

// ---------------------------------------------------------------------------
// ready
// ---------------------------------------------------------------------------

#[test]
fn ready_matches_ready_stats_oracle() {
    let root = fixtures_or_skip!();
    let stats = read_json(&root.join("gateway/ready-stats.json"));
    let payload = data::ready(&root).expect("READY читается");

    let guilds = stats["guilds"].as_u64().expect("stats.guilds") as usize;
    let private = stats["privateChannels"]
        .as_u64()
        .expect("stats.privateChannels") as usize;
    let relationships = stats["relationships"]
        .as_u64()
        .expect("stats.relationships") as usize;

    assert_eq!(payload.guilds.len(), guilds, "гильдии: оракул ready-stats");
    assert_eq!(payload.guild_count, guilds);
    assert_eq!(payload.private_channel_count, private);
    assert_eq!(payload.relationship_count, relationships);

    // Сантизация: e-mail не должен утечь в BFF, даже если попадёт во фронтенд.
    assert_eq!(payload.user.email.as_deref(), Some("REDACTED"));
    assert!(!payload.user.id.is_empty());
    assert!(!payload.user.username.is_empty());

    let guild = payload
        .guilds
        .iter()
        .find(|guild| guild.id == "1014537851418980372")
        .expect("гильдия из READY присутствует");
    assert_eq!(guild.name, "ITSKIDON PC BOOST");
    assert_eq!(guild.member_count, 24_228);
    assert!(guild.channel_count > 0);

    let session = payload.session.as_ref().expect("сессия из READY");
    assert_eq!(session.client, "desktop");
    assert_eq!(session.status, "invisible");
}

#[test]
fn resolve_root_prefers_configured_path_and_defaults_to_repo_fixtures() {
    assert_eq!(
        data::resolve_root(Some("/tmp/dl-fixtures")),
        PathBuf::from("/tmp/dl-fixtures")
    );
    let default = data::resolve_root(None);
    assert!(
        default.ends_with("fixtures"),
        "по умолчанию — fixtures репозитория: {}",
        default.display()
    );
    assert!(default.is_dir());
}

// ---------------------------------------------------------------------------
// read_state (RW-40/M-07-BE2)
// ---------------------------------------------------------------------------

/// M-07-BE2: проекция `READY.read_state.entries[]` сверяется с самим READY.
/// Оракул собирается из файла, а не из вывода `data::read_state`: поле в поле,
/// порядок записей READY сохраняется. `last_viewed = 4310` (пример из
/// `046-MESSAGE_ACK`) присутствует в реальном READY; отсутствующие поля —
/// `null`, а не 0.
#[test]
fn read_state_matches_ready_oracle() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join("gateway/by-type/002-READY.json"));
    let oracle = ready["d"]["read_state"]["entries"]
        .as_array()
        .expect("READY.read_state.entries — массив");
    assert!(!oracle.is_empty(), "в фикстуре есть записи read_state");

    let entries = data::read_state(&root).expect("read_state читается из READY");
    assert_eq!(
        entries.len(),
        oracle.len(),
        "проекция не теряет и не добавляет записи READY"
    );

    let expected: Vec<Value> = oracle
        .iter()
        .map(|entry| {
            let nullable = |key: &str| match entry.get(key) {
                None | Some(Value::Null) => Value::Null,
                Some(Value::String(text)) if text == "nil" || text.is_empty() => Value::Null,
                Some(other) => other.clone(),
            };
            serde_json::json!({
                "channelId": entry["id"],
                "flags": nullable("flags"),
                "lastMessageId": nullable("last_message_id"),
                "lastPinTimestamp": nullable("last_pin_timestamp"),
                "lastViewed": nullable("last_viewed"),
                "mentionCount": nullable("mention_count"),
                // M-07-BE3: badge-форма READY — три поля как есть, имена — точный
                // camelCase; у обычной записи их нет → null.
                "badgeCount": entry
                    .get("badge_count")
                    .and_then(Value::as_i64)
                    .map_or(Value::Null, |count| serde_json::json!(count)),
                "lastAckedId": match entry.get("last_acked_id") {
                    Some(Value::String(text)) if text == "nil" || text.is_empty() => Value::Null,
                    Some(Value::String(text)) => Value::String(text.clone()),
                    _ => Value::Null,
                },
                "readStateType": entry
                    .get("read_state_type")
                    .and_then(Value::as_i64)
                    .map_or(Value::Null, |kind| serde_json::json!(kind)),
            })
        })
        .collect();
    assert_eq!(
        serde_json::to_value(&entries).expect("ответ сериализуется"),
        Value::Array(expected),
        "поля и порядок записей — как в READY"
    );

    // Независимая проверка знакового значения: 4310 — счётчик, не snowflake.
    let last_viewed: Vec<u64> = entries
        .iter()
        .filter_map(|entry| entry.last_viewed)
        .collect();
    assert!(
        last_viewed.contains(&4310),
        "last_viewed=4310 из MESSAGE_ACK присутствует в READY"
    );
    assert!(
        entries.iter().any(|entry| entry.last_viewed.is_none()),
        "в READY есть записи без last_viewed → null, не 0"
    );
    assert!(
        entries.iter().all(|entry| !entry.channel_id.is_empty()),
        "channelId не пустой"
    );

    // M-07-BE3: badge-форма — 2 записи из 157 (README `gateway-schema.md` §read_state).
    let badges: Vec<&data::ReadStateEntry> = entries
        .iter()
        .filter(|entry| entry.badge_count.is_some())
        .collect();
    assert_eq!(badges.len(), 2, "badge_count присутствует у 2/157 записей");
    assert_eq!(
        badges
            .iter()
            .map(|entry| entry.badge_count)
            .collect::<Vec<_>>(),
        vec![Some(16), Some(0)],
        "значения badge_count из READY: 16 и 0 (0 — валидное значение, не null)"
    );
    assert_eq!(
        badges
            .iter()
            .map(|entry| entry.read_state_type)
            .collect::<Vec<_>>(),
        vec![Some(2), Some(4)],
        "read_state_type 2/4 — badge-форма READY"
    );
    assert_eq!(
        badges
            .iter()
            .map(|entry| entry.last_acked_id.as_deref())
            .collect::<Vec<_>>(),
        vec![Some("1294268550826950656"), Some("1237379809395605509")],
        "last_acked_id — decimal-строка READY, без потери точности"
    );
    assert!(
        entries.iter().all(|entry| entry.badge_count.is_some()
            || (entry.badge_count.is_none()
                && entry.last_acked_id.is_none()
                && entry.read_state_type.is_none())),
        "badge-поля либо все три из READY, либо все три null"
    );

    // mentionCount не потерян при расширении проекции: 155/157 как в READY.
    assert_eq!(
        entries
            .iter()
            .filter(|entry| entry.mention_count.is_some())
            .count(),
        155,
        "mention_count есть у 155/157 записей READY"
    );
    assert_eq!(
        entries.iter().filter(|entry| entry.flags.is_some()).count(),
        155,
        "flags есть у 155/157 записей READY"
    );

    // Порядок записей — как в READY, без пересортировки.
    let ids: Vec<&str> = entries
        .iter()
        .map(|entry| entry.channel_id.as_str())
        .collect();
    let oracle_ids: Vec<&str> = oracle
        .iter()
        .map(|entry| entry["id"].as_str().expect("id записи READY — строка"))
        .collect();
    assert_eq!(ids, oracle_ids, "порядок channelId совпадает с READY");
}

// ---------------------------------------------------------------------------
// messages_page
// ---------------------------------------------------------------------------

#[test]
fn messages_page_is_stable_deduped_and_cursor_driven() {
    let root = fixtures_or_skip!();
    let channel = "1163408591315812392";

    let first = data::messages_page(&root, channel, None, Some(2)).expect("первая страница");
    assert_eq!(first.channel_id, channel);
    assert_eq!(first.limit, 2);
    assert_eq!(first.loaded_rows_limit, UI_WINDOW_ROWS);
    assert_eq!(first.items.len(), 2);
    assert!(first.has_more);
    assert_eq!(first.total, 3, "CREATE/UPDATE дедуплицируются по id");
    let cursor = first
        .next_cursor
        .clone()
        .expect("курсор есть, пока есть данные");

    let second =
        data::messages_page(&root, channel, Some(&cursor), Some(2)).expect("вторая страница");
    assert_eq!(second.items.len(), 1);
    assert!(!second.has_more, "страница последняя");
    assert_eq!(second.next_cursor, None);

    let ids: Vec<_> = first
        .items
        .iter()
        .chain(second.items.iter())
        .map(|row| row.id.clone())
        .collect();
    let unique: HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len(), "страницы не пересекаются");

    // Стабильность: тот же курсор — та же страница (без скрытого состояния).
    let repeated =
        data::messages_page(&root, channel, Some(&cursor), Some(2)).expect("повтор страницы");
    assert_eq!(repeated.items, second.items);

    // Пустая страница за концом — валидный ответ, не ошибка.
    let tail = data::messages_page(
        &root,
        channel,
        second.items.last().map(|row| row.id.as_str()),
        Some(2),
    )
    .expect("хвост данных");
    assert!(tail.items.is_empty());
    assert!(!tail.has_more);
    assert_eq!(tail.next_cursor, None);

    let row = first.items.first().expect("строка сообщения");
    assert!(!row.author.username.is_empty());
    assert!(row.content.starts_with('📰'));
    assert!(row.timestamp.is_some());
    assert_eq!(row.channel_id, channel);
}

#[test]
fn messages_page_unknown_channel_is_not_found() {
    let root = fixtures_or_skip!();
    let err = data::messages_page(&root, "999999999999999999", None, None)
        .expect_err("неизвестный канал");
    assert_eq!(kind(&err), "NotFound", "{err}");
}

#[test]
fn messages_page_empty_for_known_channel_without_messages() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join("gateway/by-type/002-READY.json"));

    // Оракул: каналы с сообщениями — из самих by-type-файлов.
    let mut with_messages = HashSet::new();
    for entry in fs::read_dir(root.join("gateway/by-type")).expect("каталог by-type") {
        let path = entry.expect("запись каталога").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path);
        if value["t"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("MESSAGE_"))
        {
            if let Some(id) = value["d"]["channel_id"].as_str() {
                with_messages.insert(id.to_string());
            }
        }
    }
    let silent = ready["d"]["guilds"]
        .as_array()
        .expect("guilds")
        .iter()
        .flat_map(|guild| guild["channels"].as_array().into_iter().flatten())
        .filter_map(|channel| channel["id"].as_str())
        .find(|id| !with_messages.contains(*id))
        .expect("в READY есть канал без сообщений");

    let page = data::messages_page(&root, silent, None, Some(DEFAULT_PAGE_LIMIT))
        .expect("известный тихий канал — пустая страница, не ошибка");
    assert!(page.items.is_empty());
    assert_eq!(page.total, 0);
    assert!(!page.has_more);
    assert_eq!(page.next_cursor, None);
}

#[test]
fn messages_page_rejects_bad_limit_and_foreign_cursor() {
    let root = fixtures_or_skip!();
    let channel = "1163408591315812392";

    for limit in [0, MAX_PAGE_LIMIT + 1, usize::MAX] {
        let err = data::messages_page(&root, channel, None, Some(limit))
            .expect_err("недопустимый размер страницы");
        assert_eq!(kind(&err), "InvalidArg", "limit={limit}: {err}");
    }
    assert_eq!(
        data::messages_page(&root, channel, None, Some(MAX_PAGE_LIMIT))
            .expect("верхняя граница включительно")
            .limit,
        MAX_PAGE_LIMIT
    );
    let err =
        data::messages_page(&root, channel, Some("нет-такого-id"), None).expect_err("чужой курсор");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let err = data::messages_page(&root, "  ", None, None).expect_err("пустой channel id");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
}

// ---------------------------------------------------------------------------
// members_page
// ---------------------------------------------------------------------------

#[test]
fn members_page_paginates_with_stable_cursor() {
    let root = fixtures_or_skip!();
    let guild = "-1467272931325252608";
    let limit = 25;

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    let mut total;
    loop {
        let page = data::members_page(&root, guild, cursor.as_deref(), Some(limit))
            .expect("страница участников");
        assert_eq!(page.guild_id, guild);
        assert_eq!(page.loaded_rows_limit, UI_WINDOW_ROWS);
        assert!(page.items.len() <= limit);
        total = Some(page.total);
        pages += 1;
        assert!(pages <= 10, "пагинация не должна крутиться бесконечно");

        for row in &page.items {
            assert!(!row.id.is_empty());
            assert!(!row.username.is_empty());
            assert!(!row.display_name.is_empty());
            assert!(!row.status.is_empty(), "статус presence заполнен");
        }
        seen.extend(page.items.iter().map(|row| row.id.clone()));

        match page.next_cursor {
            Some(next) if page.has_more => cursor = Some(next),
            _ => {
                assert!(!page.has_more);
                assert_eq!(page.next_cursor, None);
                break;
            }
        }
    }

    let unique: HashSet<_> = seen.iter().collect();
    assert_eq!(
        unique.len(),
        seen.len(),
        "участники не дублируются между страницами"
    );
    assert_eq!(total, Some(seen.len()));
    assert!(pages > 1, "данных хватает на несколько страниц");
}

#[test]
fn members_page_unknown_guild_is_not_found() {
    let root = fixtures_or_skip!();
    let err = data::members_page(&root, "нет-такой-гильдии", None, None)
        .expect_err("неизвестная гильдия");
    assert_eq!(kind(&err), "NotFound", "{err}");
}

#[test]
fn members_page_empty_for_guild_without_member_capture() {
    let root = fixtures_or_skip!();
    let member_guilds: HashSet<String> = fs::read_dir(root.join("gateway/by-type"))
        .expect("by-type")
        .filter_map(|entry| {
            let path = entry.expect("запись").path();
            let value = read_json(&path);
            (value["t"].as_str() == Some("GUILD_MEMBER_LIST_UPDATE"))
                .then(|| value["d"]["guild_id"].as_str().map(str::to_string))
                .flatten()
        })
        .collect();
    let ready = read_json(&root.join("gateway/by-type/002-READY.json"));
    let empty_guild = ready["d"]["guilds"]
        .as_array()
        .expect("guilds")
        .iter()
        .filter_map(|guild| guild["id"].as_str())
        .find(|id| !member_guilds.contains(*id))
        .expect("в READY есть гильдия без снятых участников");

    let page = data::members_page(&root, empty_guild, None, None)
        .expect("известная гильдия без снимка участников — пустая страница");
    assert!(page.items.is_empty());
    assert_eq!(page.total, 0);
    assert!(!page.has_more);
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

#[test]
fn settings_exposes_sections_built_from_ready() {
    let root = fixtures_or_skip!();
    let ready = data::ready(&root).expect("READY читается");
    let payload = data::settings(&root, None).expect("настройки читаются");

    let ids: Vec<_> = payload
        .sections
        .iter()
        .map(|section| section.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["profile", "notifications", "sessions", "privacy", "data"]
    );
    assert_eq!(payload.selected, None);

    let profile = payload
        .sections
        .iter()
        .find(|section| section.id == "profile")
        .expect("секция profile");
    assert_eq!(profile.values["id"], ready.user.id);
    assert_eq!(profile.values["email"], "REDACTED");

    let stats = read_json(&root.join("gateway/ready-stats.json"));
    let data_section = payload
        .sections
        .iter()
        .find(|section| section.id == "data")
        .expect("секция data");
    assert_eq!(
        data_section.values["guilds"].as_u64().expect("guilds"),
        stats["guilds"].as_u64().expect("stats")
    );
    assert_eq!(
        data_section.values["privateChannels"]
            .as_u64()
            .expect("private"),
        stats["privateChannels"].as_u64().expect("stats")
    );
    assert_eq!(
        data_section.values["relationships"]
            .as_u64()
            .expect("relationships"),
        stats["relationships"].as_u64().expect("stats")
    );

    let sessions = payload
        .sections
        .iter()
        .find(|section| section.id == "sessions")
        .expect("секция sessions");
    assert!(sessions.values["count"].as_u64().expect("count") >= 1);
}

#[test]
fn settings_section_filter_and_not_found() {
    let root = fixtures_or_skip!();
    let single = data::settings(&root, Some("notifications")).expect("секция notifications");
    assert_eq!(single.sections.len(), 1);
    assert_eq!(single.sections[0].id, "notifications");
    assert_eq!(single.selected.as_deref(), Some("notifications"));
    assert!(single.sections[0].values["flags"].as_u64().is_some());

    let err = data::settings(&root, Some("nope")).expect_err("неизвестная секция");
    assert_eq!(kind(&err), "NotFound", "{err}");
    assert!(err.to_string().contains("nope"));

    let err = data::settings(&root, Some("")).expect_err("пустая секция");
    assert_eq!(kind(&err), "NotFound", "{err}");
}

// ---------------------------------------------------------------------------
// негативные кейсы: битый JSON, пустой каталог, отсутствующие файлы
// ---------------------------------------------------------------------------

#[test]
fn broken_ready_json_is_parse_error() {
    let root = temp_root("broken-ready");
    fs::write(root.join("gateway/by-type/x.json"), "{}").expect("пишем файл");
    fs::write(root.join("gateway/by-type/002-READY.json"), "{ это не json").expect("пишем READY");

    let err = data::ready(&root).expect_err("битый READY");
    assert_eq!(kind(&err), "Parse", "{err}");
    assert!(err.to_string().contains("002-READY.json"), "{err}");
    cleanup(&root);
}

#[test]
fn broken_by_type_json_is_parse_error() {
    let root = temp_root("broken-by-type");
    fs::write(
        root.join("gateway/by-type/002-READY.json"),
        r#"{"d":{"guilds":[{"id":"1","channels":[{"id":"42"}]}],"private_channels":[]}}"#,
    )
    .expect("пишем READY");
    fs::write(root.join("gateway/by-type/001-MESSAGE_CREATE.json"), "{").expect("пишем битый кадр");

    let err = data::messages_page(&root, "42", None, None).expect_err("битый by-type");
    assert_eq!(kind(&err), "Parse", "{err}");
    cleanup(&root);
}

#[test]
fn empty_catalog_is_not_a_crash() {
    let root = temp_root("empty-catalog");

    let err = data::ready(&root).expect_err("в пустом каталоге READY нет");
    assert_eq!(kind(&err), "NotFound", "{err}");

    let err = data::messages_page(&root, "42", None, None).expect_err("каналов нет");
    assert_eq!(kind(&err), "NotFound", "{err}");

    let err = data::members_page(&root, "1", None, None).expect_err("гильдий нет");
    assert_eq!(kind(&err), "NotFound", "{err}");
    cleanup(&root);
}

#[test]
fn missing_ready_file_is_not_found() {
    let root = temp_root("missing-ready");
    let err = data::ready(&root).expect_err("READY отсутствует");
    assert_eq!(kind(&err), "NotFound", "{err}");
    cleanup(&root);
}

#[test]
fn unreadable_catalog_is_io_error() {
    let root = temp_root("io-error");
    // `gateway` — файл, а не каталог: чтение by-type даёт NotADirectory (не NotFound).
    fs::remove_dir_all(root.join("gateway")).expect("убираем заготовленный каталог");
    fs::write(root.join("gateway"), "not a directory").expect("пишем файл");
    let err = data::messages_page(&root, "42", None, None).expect_err("каталог не читается");
    assert_eq!(kind(&err), "Io", "{err}");
    cleanup(&root);
}

// ---------------------------------------------------------------------------
// R1–R4 (возврат E4): лимиты размера/входа и строгая проверка кадров
// ---------------------------------------------------------------------------

/// Оракул спецификации: `MAX_FIXTURE_BYTES` = 64 MiB. Значение записано в тесте
/// независимо от реализации, чтобы проверять контракт, а не константу в коде.
const SPEC_FIXTURE_BYTES: u64 = 64 * 1024 * 1024;
/// Оракул спецификации: `MAX_INPUT_LEN` = 256 символов для id/курсора.
const SPEC_INPUT_LEN: usize = 256;

/// Контракт лимитов: публичные константы совпадают со спецификацией, с которой
/// сверяются тесты R1/R2 (ловит дрейф констант, не ослабляя сами проверки).
#[test]
fn public_limits_match_specification() {
    assert_eq!(data::MAX_FIXTURE_BYTES, SPEC_FIXTURE_BYTES);
    assert_eq!(data::MAX_INPUT_LEN, SPEC_INPUT_LEN);
}

/// R1: фикстура больше лимита отвергается **до** чтения в память — `Parse`
/// с путём и лимитом (разреженный файл: `metadata.len()` > лимита, байтов нет).
#[test]
fn oversized_fixture_is_rejected_before_reading() {
    let root = temp_root("oversized-fixture");
    write_minimal_ready(&root, "42", "1");
    let path = root.join("gateway/by-type/001-nil.json");
    let file = fs::File::create(&path).expect("создаём файл фикстуры");
    file.set_len(SPEC_FIXTURE_BYTES + 1)
        .expect("разреженный файл на 64 MiB + 1 байт");
    drop(file);

    let err = data::messages_page(&root, "42", None, None)
        .expect_err("фикстура больше лимита — ошибка, а не чтение 64 MiB");
    assert_eq!(kind(&err), "Parse", "{err}");
    let message = err.to_string();
    assert!(message.contains("001-nil.json"), "путь в ошибке: {message}");
    assert!(
        message.contains(&SPEC_FIXTURE_BYTES.to_string()),
        "лимит в ошибке: {message}"
    );
    cleanup(&root);
}

/// R2: `channelId`/`guildId`/`cursor` длиннее 256 символов — `InvalidArg`,
/// а в сообщении усечённое значение (`…`), не вся строка.
#[test]
fn oversized_ids_and_cursor_are_invalid_arg_with_truncated_message() {
    let root = fixtures_or_skip!();
    let channel = "1163408591315812392";
    let long_id = "7".repeat(10_000);
    let long_cursor = "x".repeat(10_000);

    let err = data::messages_page(&root, &long_id, None, None).expect_err("длинный channelId");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let message = err.to_string();
    assert!(
        !message.contains(&long_id),
        "вся строка не копируется в ошибку"
    );
    assert!(message.contains('…'), "видно усечение: {message}");
    assert!(
        message.chars().count() < SPEC_INPUT_LEN,
        "сообщение ограничено, символов: {}",
        message.chars().count()
    );

    let err =
        data::messages_page(&root, channel, Some(&long_cursor), None).expect_err("длинный курсор");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let message = err.to_string();
    assert!(
        !message.contains(&long_cursor),
        "вся строка не копируется в ошибку"
    );
    assert!(message.contains('…'), "видно усечение: {message}");
    assert!(
        message.chars().count() < SPEC_INPUT_LEN,
        "сообщение ограничено, символов: {}",
        message.chars().count()
    );

    let err = data::members_page(&root, &long_id, None, None).expect_err("длинный guildId");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let message = err.to_string();
    assert!(
        !message.contains(&long_id),
        "вся строка не копируется в ошибку"
    );
    assert!(message.contains('…'), "видно усечение: {message}");
    assert!(
        message.chars().count() < SPEC_INPUT_LEN,
        "сообщение ограничено, символов: {}",
        message.chars().count()
    );

    // Граница включительно: ровно 256 символов — валидная длина входа,
    // дальше это уже NotFound, а не InvalidArg.
    let allowed = "9".repeat(SPEC_INPUT_LEN);
    let err = data::messages_page(&root, &allowed, None, None).expect_err("такого канала нет");
    assert_eq!(kind(&err), "NotFound", "256 символов допустимы: {err}");
}

/// R3: кадр `by-type` обязан иметь непустой строковый `t` и присутствующий `d`.
/// `d: null` отвергается: ни в одном кадре дампа такого `d` нет (проверено по
/// `fixtures/gateway/by-type/*.json`, nil-кадры несут `"d": "nil"`).
#[test]
fn frame_without_type_or_data_is_parse_error() {
    let cases = [
        ("empty-object", "{}"),
        ("empty-type", r#"{"t":""}"#),
        ("no-data", r#"{"t":"MESSAGE_CREATE"}"#),
        ("non-string-type", r#"{"t":42,"d":{}}"#),
        ("null-data", r#"{"t":"MESSAGE_CREATE","d":null}"#),
    ];
    for (name, body) in cases {
        let root = temp_root(&format!("strict-frame-{name}"));
        write_minimal_ready(&root, "42", "1");
        fs::write(root.join("gateway/by-type/001-broken.json"), body).expect("пишем кадр");

        let err = match data::messages_page(&root, "42", None, None) {
            Ok(page) => panic!(
                "{name}: кадр без валидных t/d принят, строк: {}",
                page.total
            ),
            Err(error) => error,
        };
        assert_eq!(kind(&err), "Parse", "{name}: {err}");
        assert!(
            err.to_string().contains("001-broken.json"),
            "{name}: путь в ошибке: {err}"
        );
        cleanup(&root);
    }
}

/// R4: элемент `GUILD_MEMBER_LIST_UPDATE` без `member.user.id` — `Parse`
/// (индекс элемента в сообщении), а не тихий пропуск. Групповые элементы
/// (`{"group": ...}`) остаются валидными и в список участников не попадают.
#[test]
fn member_item_without_user_id_is_parse_error() {
    let root = temp_root("broken-member-item");
    write_minimal_ready(&root, "42", "1");
    let update = serde_json::json!({
        "t": "GUILD_MEMBER_LIST_UPDATE",
        "d": {
            "guild_id": "1",
            "ops": [{
                "op": "SYNC",
                "items": [
                    {"group": {"id": "online"}},
                    {"member": {"user": {}}},
                ],
            }],
        },
    });
    fs::write(
        root.join("gateway/by-type/001-member-list.json"),
        update.to_string(),
    )
    .expect("пишем битый снимок участников");

    let err = match data::members_page(&root, "1", None, None) {
        Ok(page) => panic!("битый item принят молча, участников: {}", page.total),
        Err(error) => error,
    };
    assert_eq!(kind(&err), "Parse", "{err}");
    let message = err.to_string();
    assert!(message.contains("items[1]"), "индекс элемента: {message}");
    assert!(message.contains("001-member-list.json"), "{message}");
    cleanup(&root);

    // Только группы — валидный (пустой по участникам) снимок, не ошибка.
    let group_root = temp_root("group-only-member-item");
    write_minimal_ready(&group_root, "42", "1");
    let groups_only = serde_json::json!({
        "t": "GUILD_MEMBER_LIST_UPDATE",
        "d": {
            "guild_id": "1",
            "ops": [{"op": "SYNC", "items": [{"group": {"id": "online"}}]}],
        },
    });
    fs::write(
        group_root.join("gateway/by-type/001-member-list.json"),
        groups_only.to_string(),
    )
    .expect("пишем снимок только с группами");
    let page = data::members_page(&group_root, "1", None, None)
        .expect("групповые элементы — валидный снимок");
    assert_eq!(page.total, 0);
    cleanup(&group_root);
}

// ---------------------------------------------------------------------------
// W39i: поля сообщений из фикстур (embeds/attachments/reactions/reply/day/group)
// ---------------------------------------------------------------------------

/// Канал `#новости` (ITSKIDON PC BOOST): три CREATE и UPDATE-кадры с тем же id.
const NEWS_CHANNEL: &str = "1163408591315812392";
/// Каналы, у которых в `by-type` есть сообщения (оракул: `grep -l MESSAGE_`).
const MESSAGE_CHANNELS: [&str; 4] = [
    "1163408591315812392",
    "-1467273046216692736",
    "-12225890619869898752",
    "-2684694197495146496",
];

#[test]
fn message_rows_expose_embeds_day_and_group_from_fixtures() {
    let root = fixtures_or_skip!();
    let page = data::messages_page(&root, NEWS_CHANNEL, None, Some(MAX_PAGE_LIMIT))
        .expect("страница канала");
    assert_eq!(page.total, 3);
    assert_eq!(page.items.len(), 3);

    // `day` — дата из `timestamp` (реальное поле кадра), `type`/`pinned` — из кадра.
    for row in &page.items {
        assert_eq!(
            row.day.as_deref(),
            Some("2026-10-19"),
            "day из timestamp: {}",
            row.id
        );
        assert_eq!(row.message_type, 0, "type из кадра: {}", row.id);
        assert!(!row.pinned, "pinned=false в фикстуре: {}", row.id);
        assert!(row.mentions.is_empty(), "mentions=null: {}", row.id);
    }

    // Один автор и один день → start + два continuation (без разрыва по времени).
    let groups: Vec<&str> = page.items.iter().map(|row| row.group.as_str()).collect();
    assert_eq!(groups, ["start", "continuation", "continuation"]);

    // `embeds` в CREATE = null, но в MESSAGE_UPDATE того же id есть объект статьи:
    // строка обязана получить его, а не потерять при дедупликации.
    for row in &page.items {
        assert_eq!(row.embeds.len(), 1, "embed из UPDATE: {}", row.id);
        let embed = &row.embeds[0];
        assert_eq!(embed.kind, "article");
        assert_eq!(embed.provider_name.as_deref(), Some("Хабр"));
        assert!(
            embed
                .url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://habr.com/")),
            "url embed: {:?}",
            embed.url
        );
        assert!(
            embed
                .title
                .as_deref()
                .is_some_and(|title| !title.is_empty()),
            "title embed"
        );
        assert!(embed.thumbnail_url.is_some(), "thumbnail.url embed");
    }

    // В фикстурах нет вложений/реакций/ответов и нет непустого edited_timestamp.
    for row in &page.items {
        assert!(row.attachments.is_empty(), "attachments=null: {}", row.id);
        assert!(
            row.reactions.is_empty(),
            "reactions отсутствует: {}",
            row.id
        );
        assert!(row.reply_to.is_none(), "reply отсутствует: {}", row.id);
        assert!(row.edited_at.is_none(), "edited_timestamp=nil: {}", row.id);
    }
}

#[test]
fn fixture_messages_have_no_attachments_reactions_replies_or_edit_timestamps() {
    let root = fixtures_or_skip!();
    let mut frames = 0;
    for entry in fs::read_dir(root.join("gateway/by-type")).expect("by-type") {
        let path = entry.expect("запись").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let value = read_json(&path);
        let Some(kind) = value["t"].as_str() else {
            continue;
        };
        if !kind.starts_with("MESSAGE_") || kind == "MESSAGE_ACK" {
            continue;
        }
        let data = &value["d"];
        assert!(
            data["attachments"].is_null(),
            "{}: attachments не null — тест и реализацию нужно расширить",
            path.display()
        );
        assert!(
            data.get("reactions").is_none(),
            "{}: появились reactions",
            path.display()
        );
        assert!(
            data.get("message_reference").is_none() && data.get("referenced_message").is_none(),
            "{}: появился reply",
            path.display()
        );
        assert_eq!(
            data["edited_timestamp"],
            "nil",
            "{}: реальный edited_timestamp",
            path.display()
        );
        frames += 1;
    }
    assert!(frames >= 13, "кадров сообщений: {frames}");

    // API честно отражает это на всех каналах с сообщениями.
    for channel in MESSAGE_CHANNELS {
        let mut cursor: Option<String> = None;
        let mut rows = 0;
        loop {
            let page = data::messages_page(&root, channel, cursor.as_deref(), Some(MAX_PAGE_LIMIT))
                .expect("страница канала с сообщениями");
            for row in &page.items {
                rows += 1;
                assert!(row.attachments.is_empty(), "{}", row.id);
                assert!(row.reactions.is_empty(), "{}", row.id);
                assert!(row.reply_to.is_none(), "{}", row.id);
                assert!(row.edited_at.is_none(), "{}", row.id);
            }
            match page.next_cursor {
                Some(next) if page.has_more => cursor = Some(next),
                _ => break,
            }
        }
        assert!(rows > 0, "канал {channel} без строк");
    }
}

// ---------------------------------------------------------------------------
// W39i: роли, группы и счётчики участников
// ---------------------------------------------------------------------------

#[test]
fn members_page_roles_and_groups_from_fixtures() {
    let root = fixtures_or_skip!();
    let guild = "294064825540280335";

    let page =
        data::members_page(&root, guild, None, Some(MAX_PAGE_LIMIT)).expect("страница участников");
    // Оракул: единственный SYNC этого снимка — 1 + 7 + 6 + 82 уникальных участника.
    assert_eq!(page.total, 96);

    let ids: Vec<&str> = page.groups.iter().map(|group| group.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "294152150374940673",
            "779231704552177664",
            "358921494128099328",
            "online",
            "offline"
        ],
        "роли по position desc, затем статусы"
    );
    assert_eq!(page.groups[0].kind, "role");
    assert_eq!(page.groups[0].title.as_deref(), Some("АДМИНИСТРАТОРЫ"));
    assert_eq!(page.groups[1].title.as_deref(), Some("Гуру вуду ПК"));
    assert_eq!(page.groups[2].title.as_deref(), Some("СВОИ"));
    assert_eq!(page.groups[3].kind, "status");

    // announced — из `groups[].count` последнего снимка (089: роль админов
    // появилась, online/offline изменились); loaded — сколько строк снято.
    let counters: Vec<(Option<u64>, usize)> = page
        .groups
        .iter()
        .map(|group| (group.announced_count, group.loaded_count))
        .collect();
    assert_eq!(
        counters,
        [
            (Some(1), 0),
            (Some(1), 1),
            (Some(7), 7),
            (Some(6), 6),
            (Some(97), 82)
        ]
    );

    // READY этой гильдии — оракул имён ролей.
    let ready = read_json(&root.join("gateway/by-type/002-READY.json"));
    let guild_roles: HashSet<String> = ready["d"]["guilds"]
        .as_array()
        .expect("guilds")
        .iter()
        .find(|entry| entry["id"] == guild)
        .expect("гильдия в READY")
        .get("roles")
        .and_then(Value::as_array)
        .expect("roles")
        .iter()
        .filter_map(|role| role["id"].as_str().map(str::to_string))
        .collect();

    let group_ids: HashSet<&str> = page.groups.iter().map(|group| group.id.as_str()).collect();
    for row in &page.items {
        assert!(!row.group.is_empty(), "group у {}", row.id);
        assert!(group_ids.contains(row.group.as_str()), "{}", row.group);
        for role in &row.roles {
            assert!(
                guild_roles.contains(role),
                "роль {role} не из READY гильдии"
            );
        }
    }

    let top = page
        .items
        .iter()
        .find(|row| row.group == "779231704552177664")
        .expect("участник группы «Гуру вуду ПК»");
    assert_eq!(
        top.roles,
        ["779231704552177664", "358921494128099328"],
        "роли по position desc"
    );
    assert_eq!(top.role_name.as_deref(), Some("Гуру вуду ПК"));
    assert_eq!(top.display_name, "ДраКс-Иван");

    // Стабильный порядок: группы идут блоками в порядке `page.groups`,
    // внутри группы — по display_name (без регистра), затем id.
    let index: std::collections::HashMap<&str, usize> = page
        .groups
        .iter()
        .enumerate()
        .map(|(position, group)| (group.id.as_str(), position))
        .collect();
    let mut previous = 0;
    for row in &page.items {
        let position = index[row.group.as_str()];
        assert!(position >= previous, "группы не перемешаны: {}", row.id);
        previous = position;
    }
    for pair in page.items.windows(2) {
        if pair[0].group == pair[1].group {
            assert!(
                pair[0].display_name.to_lowercase() <= pair[1].display_name.to_lowercase(),
                "порядок внутри группы: {} / {}",
                pair[0].display_name,
                pair[1].display_name
            );
        }
    }

    let repeated =
        data::members_page(&root, guild, None, Some(MAX_PAGE_LIMIT)).expect("повтор страницы");
    assert_eq!(repeated.items, page.items, "порядок детерминирован");
    assert_eq!(repeated.groups, page.groups);
}

// ---------------------------------------------------------------------------
// W39i: локальный поиск по фикстурам
// ---------------------------------------------------------------------------

#[test]
fn search_page_matches_fixture_fields_and_paginates() {
    let root = fixtures_or_skip!();

    let all = data::search_page(
        &root,
        "",
        &SearchFilters::default(),
        None,
        Some(MAX_PAGE_LIMIT),
    )
    .expect("полная выборка");
    assert_eq!(all.total, 13, "13 уникальных сообщений в by-type");
    assert_eq!(all.scanned, 13);
    assert_eq!(all.items.len(), 13);
    assert!(!all.has_more);
    assert_eq!(all.next_cursor, None);

    let by_content =
        data::search_page(&root, "call of duty", &SearchFilters::default(), None, None)
            .expect("поиск по тексту");
    assert_eq!(by_content.total, 1);
    assert_eq!(by_content.items[0].message.channel_id, NEWS_CHANNEL);
    assert_eq!(
        by_content.items[0].channel_name.as_deref(),
        Some("📃┃новости"),
        "имя канала из READY"
    );

    let by_author = data::search_page(
        &root,
        "",
        &SearchFilters {
            author_id: Some("268478587651358721".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр по автору");
    assert_eq!(by_author.total, 3);
    assert!(by_author
        .items
        .iter()
        .all(|hit| hit.message.author.id == "268478587651358721"));

    let by_channel = data::search_page(
        &root,
        "",
        &SearchFilters {
            channel_id: Some("-2684694197495146496".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр по каналу");
    assert_eq!(by_channel.total, 8);
    assert!(by_channel
        .items
        .iter()
        .all(|hit| hit.message.channel_id == "-2684694197495146496"));
    assert_eq!(
        by_channel.items[0].channel_name, None,
        "канал не из READY — имя честно отсутствует"
    );

    let by_contains = data::search_page(
        &root,
        "",
        &SearchFilters {
            content: Some("РАДЫ ВИДЕТЬ".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр «содержит»");
    assert_eq!(by_contains.total, 1, "регистр не важен");

    let by_mention = data::search_page(
        &root,
        "",
        &SearchFilters {
            mention_id: Some("-12103814603318778368".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр «упоминание»");
    assert_eq!(by_mention.total, 1);
    assert_eq!(by_mention.items[0].message.id, "-12394240098567211520");
    assert_eq!(
        by_mention.items[0].message.mentions,
        ["-12103814603318778368"]
    );

    let by_day = data::search_page(
        &root,
        "",
        &SearchFilters {
            day: Some("2026-10-19".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр по дате");
    assert_eq!(by_day.total, 13);

    let by_type = data::search_page(
        &root,
        "",
        &SearchFilters {
            message_type: Some(0),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр по типу");
    assert_eq!(by_type.total, 13);

    let pinned = data::search_page(
        &root,
        "",
        &SearchFilters {
            pinned: Some(true),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("фильтр «закреплено»");
    assert_eq!(pinned.total, 0, "закреплённых в фикстурах нет");
    assert!(pinned.items.is_empty());
    assert!(!pinned.has_more);

    // Пагинация: limit 5 → 3 страницы, без пересечений и без потерь.
    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let page = data::search_page(
            &root,
            "",
            &SearchFilters::default(),
            cursor.as_deref(),
            Some(5),
        )
        .expect("страница поиска");
        assert_eq!(page.total, 13);
        assert_eq!(page.limit, 5);
        pages += 1;
        assert!(pages <= 5, "пагинация не должна крутиться");
        seen.extend(page.items.iter().map(|hit| hit.message.id.clone()));
        match page.next_cursor {
            Some(next) if page.has_more => cursor = Some(next),
            _ => break,
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(seen.len(), 13);
    assert_eq!(seen.iter().collect::<HashSet<_>>().len(), 13);
}

#[test]
fn search_page_empty_result_and_filters_are_valid() {
    let root = fixtures_or_skip!();

    let none = data::search_page(
        &root,
        "zzz-такого-текста-нет-в-фикстурах",
        &SearchFilters::default(),
        None,
        None,
    )
    .expect("пустой результат — валидный ответ");
    assert!(none.items.is_empty());
    assert_eq!(none.total, 0);
    assert_eq!(none.scanned, 13);

    let none_type = data::search_page(
        &root,
        "",
        &SearchFilters {
            message_type: Some(1),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("тип 1 в фикстурах не встречается");
    assert!(none_type.items.is_empty());

    let unpinned = data::search_page(
        &root,
        "call of duty",
        &SearchFilters {
            pinned: Some(false),
            day: Some("2026-10-19".to_string()),
            channel_id: Some(NEWS_CHANNEL.to_string()),
            message_type: Some(0),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("комбинация фильтров");
    assert_eq!(unpinned.total, 1);
    assert_eq!(unpinned.items[0].message.day.as_deref(), Some("2026-10-19"));
    assert_eq!(unpinned.items[0].message.group, "start");

    // Пустые строки из полей ввода — «фильтр не задан», а не «искать пустое».
    let blank = SearchFilters {
        author_id: Some("  ".to_string()),
        channel_id: Some(String::new()),
        content: Some(String::new()),
        mention_id: Some(String::new()),
        day: Some(String::new()),
        message_type: None,
        pinned: None,
    };
    let all = data::search_page(&root, "", &blank, None, None).expect("пустые фильтры");
    assert_eq!(all.total, 13, "пустые фильтры ничего не отсекают");
}

#[test]
fn search_page_rejects_bad_limit_cursor_and_oversized_filters() {
    let root = fixtures_or_skip!();
    let long = "x".repeat(10_000);

    for limit in [0, MAX_PAGE_LIMIT + 1] {
        let err = data::search_page(&root, "", &SearchFilters::default(), None, Some(limit))
            .expect_err("недопустимый limit");
        assert_eq!(kind(&err), "InvalidArg", "limit={limit}: {err}");
    }

    let err = data::search_page(&root, &long, &SearchFilters::default(), None, None)
        .expect_err("длинный query");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    assert!(
        !err.to_string().contains(&long),
        "запрос не копируется целиком"
    );
    assert!(err.to_string().contains('…'), "видно усечение: {err}");

    let filters = [
        SearchFilters {
            author_id: Some(long.clone()),
            ..SearchFilters::default()
        },
        SearchFilters {
            channel_id: Some(long.clone()),
            ..SearchFilters::default()
        },
        SearchFilters {
            content: Some(long.clone()),
            ..SearchFilters::default()
        },
        SearchFilters {
            mention_id: Some(long.clone()),
            ..SearchFilters::default()
        },
        SearchFilters {
            day: Some(long.clone()),
            ..SearchFilters::default()
        },
    ];
    for filter in filters {
        let err = data::search_page(&root, "", &filter, None, None).expect_err("длинный фильтр");
        assert_eq!(kind(&err), "InvalidArg", "{filter:?}: {err}");
        assert!(
            err.to_string().chars().count() < 256,
            "сообщение ограничено: {err}"
        );
    }

    let err = data::search_page(
        &root,
        "",
        &SearchFilters {
            day: Some("19.10.2026".to_string()),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect_err("дата не в формате YYYY-MM-DD");
    assert_eq!(kind(&err), "InvalidArg", "{err}");

    let err = data::search_page(&root, "", &SearchFilters::default(), Some(&long), None)
        .expect_err("длинный курсор поиска");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let err = data::search_page(&root, "", &SearchFilters::default(), Some("чужой-id"), None)
        .expect_err("чужой курсор поиска");
    assert_eq!(kind(&err), "InvalidArg", "{err}");

    // Границы включительно: 100 строк и 256 символов фильтра — валидные значения.
    assert_eq!(
        data::search_page(
            &root,
            "",
            &SearchFilters::default(),
            None,
            Some(MAX_PAGE_LIMIT)
        )
        .expect("limit=100")
        .limit,
        MAX_PAGE_LIMIT
    );
    let allowed = data::search_page(
        &root,
        "",
        &SearchFilters {
            content: Some("я".repeat(256)),
            ..SearchFilters::default()
        },
        None,
        None,
    )
    .expect("256 символов допустимы");
    assert_eq!(allowed.total, 0);
}

// ---------------------------------------------------------------------------
// W39i R5: длина section в settings ограничена MAX_INPUT_LEN
// ---------------------------------------------------------------------------

#[test]
fn settings_rejects_oversized_section() {
    let root = fixtures_or_skip!();
    let long = "п".repeat(10_000);

    let err = data::settings(&root, Some(&long)).expect_err("длинная секция");
    assert_eq!(kind(&err), "InvalidArg", "{err}");
    let message = err.to_string();
    assert!(!message.contains(&long), "секция не копируется целиком");
    assert!(message.contains('…'), "видно усечение: {message}");
    assert!(
        message.chars().count() < SPEC_INPUT_LEN,
        "сообщение ограничено, символов: {}",
        message.chars().count()
    );

    // Ровно 256 символов — валидная длина: дальше NotFound, а не InvalidArg.
    let err =
        data::settings(&root, Some(&"x".repeat(SPEC_INPUT_LEN))).expect_err("такой секции нет");
    assert_eq!(kind(&err), "NotFound", "256 символов допустимы: {err}");
}

// ---------------------------------------------------------------------------
// сериализация ошибок
// ---------------------------------------------------------------------------

#[test]
fn errors_serialize_to_typed_json() {
    let cases = [
        (
            DataError::not_found("канал", "канал 42 не найден"),
            "NotFound",
        ),
        (DataError::invalid_arg("limit вне 1..=100"), "InvalidArg"),
        (
            DataError::parse(Path::new("a.json"), "ожидался json"),
            "Parse",
        ),
        (
            DataError::io(Path::new("b.json"), "отказано в доступе"),
            "Io",
        ),
    ];
    for (error, expected) in cases {
        let value = serde_json::to_value(&error).expect("ошибка сериализуется");
        assert_eq!(value["kind"], expected, "{value}");
        assert!(
            value["message"]
                .as_str()
                .is_some_and(|text| !text.is_empty()),
            "message непустой: {value}"
        );
        assert_eq!(error.kind(), expected);
        assert!(!error.to_string().is_empty());
    }
}

// ---------------------------------------------------------------------------
// task-575: права доступа Discord (роли, permission_overwrites, evaluate_channel_access)
// и синхронизация живой сессии (LiveSession ready/channels/read_state/record_sent_message)
// ---------------------------------------------------------------------------

#[test]
fn permission_constants_match_discord_api_v9_bitmasks() {
    assert_eq!(PERM_CREATE_INSTANT_INVITE, 1_u64 << 0);
    assert_eq!(PERM_ADMINISTRATOR, 1_u64 << 3);
    assert_eq!(PERM_ADD_REACTIONS, 1_u64 << 6);
    assert_eq!(PERM_VIEW_CHANNEL, 1_u64 << 10);
    assert_eq!(PERM_SEND_MESSAGES, 1_u64 << 11);
    assert_eq!(PERM_ATTACH_FILES, 1_u64 << 15);
    assert_eq!(PERM_READ_MESSAGE_HISTORY, 1_u64 << 16);
    assert_eq!(PERM_CONNECT, 1_u64 << 20);
    assert_eq!(PERM_SPEAK, 1_u64 << 21);
    assert_eq!(PERM_USE_VAD, 1_u64 << 25);
}

#[test]
fn compute_effective_permissions_owner_and_administrator_bypass_overwrites() {
    let guild_id = "100";
    let owner_id = "900";
    let admin_user = "901";
    let regular_user = "902";

    let guild_roles = vec![
        json!({"id": "100", "permissions": "0"}),
        json!({"id": "200", "permissions": PERM_ADMINISTRATOR.to_string()}),
    ];
    // Канал явно запрещает всё для @everyone и для конкретного участника.
    let restrictive_overwrites = vec![
        json!({
            "id": "100",
            "type": 0,
            "allow": "0",
            "deny": (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_CONNECT).to_string()
        }),
        json!({
            "id": owner_id,
            "type": 1,
            "allow": "0",
            "deny": (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES).to_string()
        }),
        json!({
            "id": admin_user,
            "type": 1,
            "allow": "0",
            "deny": (PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES).to_string()
        }),
    ];

    // a) Владелец сервера всегда получает u64::MAX.
    let owner_perms = data::compute_effective_permissions(
        owner_id,
        guild_id,
        Some(owner_id),
        &guild_roles,
        &[],
        &restrictive_overwrites,
    );
    assert_eq!(owner_perms, u64::MAX);
    let owner_access = data::evaluate_channel_access(owner_perms, 0);
    assert_eq!(
        owner_access,
        ChannelAccessPolicy {
            permissions: u64::MAX,
            can_view: true,
            can_send_messages: true,
            can_read_history: true,
            can_attach_files: true,
            can_add_reactions: true,
            can_connect_voice: false,
            can_speak_voice: false,
        }
    );

    // b) Участник с ролью ADMINISTRATOR (1 << 3) также получает u64::MAX и игнорирует deny-оверрайты.
    let admin_perms = data::compute_effective_permissions(
        admin_user,
        guild_id,
        Some(owner_id),
        &guild_roles,
        &["200".to_string()],
        &restrictive_overwrites,
    );
    assert_eq!(admin_perms, u64::MAX);

    // Обычный участник без ролей получает 0.
    let regular_perms = data::compute_effective_permissions(
        regular_user,
        guild_id,
        Some(owner_id),
        &guild_roles,
        &[],
        &restrictive_overwrites,
    );
    assert_eq!(regular_perms, 0);
}

#[test]
fn compute_effective_permissions_follows_everyone_role_and_member_overwrite_hierarchy() {
    let guild_id = "1000";
    let user_id = "5000";
    let base_bits =
        PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_READ_MESSAGE_HISTORY | PERM_ADD_REACTIONS;
    let guild_roles = vec![
        json!({"id": "1000", "permissions": base_bits.to_string()}),
        json!({"id": "2001", "permissions": PERM_ATTACH_FILES.to_string()}),
        json!({"id": "2002", "permissions": "0"}),
    ];

    // 1. @everyone overwrite запрещает SEND_MESSAGES и ATTACH_FILES.
    // 2. Роль 2001 разрешает SEND_MESSAGES и запрещает ADD_REACTIONS.
    // 3. Роль 2002 разрешает ATTACH_FILES и ADD_REACTIONS (в комбинированных ролях allow перекрывает deny другой роли).
    // 4. Member overwrite для user_id запрещает SEND_MESSAGES (перекрывает role allow) и разрешает CREATE_INSTANT_INVITE.
    let overwrites = vec![
        json!({
            "id": "1000",
            "type": 0,
            "allow": "0",
            "deny": (PERM_SEND_MESSAGES | PERM_ATTACH_FILES).to_string()
        }),
        json!({
            "id": "2001",
            "type": 0,
            "allow": PERM_SEND_MESSAGES.to_string(),
            "deny": PERM_ADD_REACTIONS.to_string()
        }),
        json!({
            "id": "2002",
            "type": 0,
            "allow": (PERM_ATTACH_FILES | PERM_ADD_REACTIONS).to_string(),
            "deny": "0"
        }),
        json!({
            "id": user_id,
            "type": 1,
            "allow": PERM_CREATE_INSTANT_INVITE.to_string(),
            "deny": PERM_SEND_MESSAGES.to_string()
        }),
    ];

    let member_roles = vec!["2001".to_string(), "2002".to_string()];

    // Проверка без member overwrite (для другого пользователя с теми же ролями):
    // SEND_MESSAGES разрешён ролью 2001, ATTACH_FILES и ADD_REACTIONS разрешены ролью 2002.
    let role_level_perms = data::compute_effective_permissions(
        "5001",
        guild_id,
        Some("9999"),
        &guild_roles,
        &member_roles,
        &overwrites,
    );
    assert_ne!(role_level_perms & PERM_VIEW_CHANNEL, 0);
    assert_ne!(role_level_perms & PERM_SEND_MESSAGES, 0);
    assert_ne!(role_level_perms & PERM_ATTACH_FILES, 0);
    assert_ne!(role_level_perms & PERM_ADD_REACTIONS, 0);

    // Проверка с member overwrite для user_id ("5000"):
    // SEND_MESSAGES сброшен member deny, а CREATE_INSTANT_INVITE выставлен member allow.
    let final_perms = data::compute_effective_permissions(
        user_id,
        guild_id,
        Some("9999"),
        &guild_roles,
        &member_roles,
        &overwrites,
    );
    let expected = PERM_VIEW_CHANNEL
        | PERM_READ_MESSAGE_HISTORY
        | PERM_ATTACH_FILES
        | PERM_ADD_REACTIONS
        | PERM_CREATE_INSTANT_INVITE;
    assert_eq!(final_perms, expected);

    let policy = data::evaluate_channel_access(final_perms, 0);
    assert!(policy.can_view);
    assert!(!policy.can_send_messages, "member deny запретил отправку");
    assert!(
        !policy.can_attach_files,
        "без права отправки нельзя прикреплять файлы в чат"
    );
    assert!(policy.can_read_history);
    assert!(policy.can_add_reactions);
}

#[test]
fn compute_effective_permissions_matches_real_ready_news_and_hidden_channels() {
    let root = fixtures_or_skip!();
    let ready = read_json(&root.join("gateway/by-type/002-READY.json"));
    let guilds = ready["d"]["guilds"].as_array().expect("guilds");

    // 1. Read-only новостной канал #📃┃новости (1163408591315812392) в гильдии ITSKIDON PC BOOST (1014537851418980372).
    let boost_guild = guilds
        .iter()
        .find(|g| g["id"] == "1014537851418980372")
        .expect("гильдия ITSKIDON PC BOOST");
    let boost_roles = boost_guild["roles"].as_array().expect("roles");
    let boost_owner = boost_guild["properties"]["owner_id"].as_str();
    let news_ch = boost_guild["channels"]
        .as_array()
        .expect("channels")
        .iter()
        .find(|c| c["id"] == NEWS_CHANNEL)
        .expect("канал новостей");
    let news_overwrites = news_ch["permission_overwrites"]
        .as_array()
        .expect("permission_overwrites");

    // Обычный участник с ролью «Пользователь» (1056564729235120158): может видеть и читать, но не писать.
    let member_roles = vec!["1056564729235120158".to_string()];
    let perms = data::compute_effective_permissions(
        "384299970632876032",
        "1014537851418980372",
        boost_owner,
        boost_roles,
        &member_roles,
        news_overwrites,
    );
    let access = data::evaluate_channel_access(perms, news_ch["type"].as_i64().unwrap_or(0));
    assert!(access.can_view, "новостной канал виден");
    assert!(access.can_read_history, "история новостей доступна");
    assert!(access.can_add_reactions, "реакции в новостях разрешены");
    assert!(
        !access.can_send_messages,
        "в новостном канале отправка запрещена оверрайтом @everyone"
    );
    assert!(!access.can_attach_files, "файлы в read-only канале запрещены");
    assert!(!access.can_connect_voice);
    assert!(!access.can_speak_voice);

    // 2. Скрытый канал ___hidden___ (751425447614480414) в гильдии MONOLIT (274093052673392641).
    let monolit_guild = guilds
        .iter()
        .find(|g| g["id"] == "274093052673392641")
        .expect("гильдия MONOLIT");
    let monolit_roles = monolit_guild["roles"].as_array().expect("roles");
    let monolit_owner = monolit_guild["properties"]["owner_id"].as_str();
    let hidden_ch = monolit_guild["channels"]
        .as_array()
        .expect("channels")
        .iter()
        .find(|c| c["id"] == "751425447614480414")
        .expect("скрытый канал ___hidden___");
    let hidden_overwrites = hidden_ch["permission_overwrites"]
        .as_array()
        .expect("permission_overwrites");

    let hidden_perms = data::compute_effective_permissions(
        "384299970632876032",
        "274093052673392641",
        monolit_owner,
        monolit_roles,
        &[],
        hidden_overwrites,
    );
    assert_eq!(
        hidden_perms & PERM_VIEW_CHANNEL,
        0,
        "VIEW_CHANNEL (1024) сброшен оверрайтом @everyone"
    );
    let hidden_access =
        data::evaluate_channel_access(hidden_perms, hidden_ch["type"].as_i64().unwrap_or(0));
    assert!(!hidden_access.can_view);
    assert!(!hidden_access.can_send_messages);
    assert!(!hidden_access.can_read_history);
    assert!(!hidden_access.can_attach_files);
    assert!(!hidden_access.can_add_reactions);
}

#[test]
fn evaluate_channel_access_handles_voice_connect_speak_and_dm_channels() {
    // Голосовой канал (type = 2): режим слушателя (CONNECT без SPEAK).
    let listener_perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_USE_VAD;
    let listener = data::evaluate_channel_access(listener_perms, 2);
    assert!(listener.can_view);
    assert!(listener.can_connect_voice);
    assert!(!listener.can_speak_voice);

    // Голосовой канал (type = 2): полный доступ (CONNECT + SPEAK).
    let speaker_perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
    let speaker = data::evaluate_channel_access(speaker_perms, 2);
    assert!(speaker.can_view);
    assert!(speaker.can_connect_voice);
    assert!(speaker.can_speak_voice);

    // Голосовой канал без CONNECT, но с битом SPEAK -> нельзя ни подключиться, ни говорить.
    let no_connect = data::evaluate_channel_access(PERM_VIEW_CHANNEL | PERM_SPEAK, 2);
    assert!(no_connect.can_view);
    assert!(!no_connect.can_connect_voice);
    assert!(!no_connect.can_speak_voice);

    // Голосовой канал без VIEW_CHANNEL -> всё закрыто.
    let hidden_voice = data::evaluate_channel_access(PERM_CONNECT | PERM_SPEAK, 2);
    assert!(!hidden_voice.can_view);
    assert!(!hidden_voice.can_connect_voice);
    assert!(!hidden_voice.can_speak_voice);

    // Личные сообщения (type = 1 / 3): все базовые действия разрешены.
    let dm_access = data::evaluate_channel_access(0, 1);
    assert!(dm_access.can_view);
    assert!(dm_access.can_send_messages);
    assert!(dm_access.can_read_history);
    assert!(dm_access.can_attach_files);
    assert!(dm_access.can_add_reactions);
    assert!(dm_access.can_connect_voice);
    assert!(dm_access.can_speak_voice);
}

#[test]
fn live_session_syncs_ready_channels_read_state_and_sent_messages() {
    let _write_guard = acquire_live_write_guard();
    let root = std::env::temp_dir().join(format!("dl-w47-live-sync-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог");
    write_minimal_ready(&root, "42", "1");

    let live_ready = json!({
        "user": {
            "id": "777000",
            "username": "live_pilot",
            "global_name": "Live Pilot",
            "avatar": "a_live_hash",
            "email": "REDACTED",
            "premium": true,
            "mfa_enabled": true
        },
        "guilds": [{
            "id": "888000",
            "properties": {
                "name": "Live Server Alpha",
                "icon": "icon_888",
                "owner_id": "777000"
            },
            "member_count": 42,
            "channels": [
                {"id": "888001", "type": 0, "name": "general", "position": 1},
                {"id": "888002", "type": 2, "name": "voice-lobby", "position": 2}
            ]
        }],
        "users": [{"id": "777001", "username": "friend_one"}],
        "relationships": [{"id": "777001", "type": 1}],
        "private_channels": [{"id": "999001", "type": 1, "recipient_ids": ["777001"]}],
        "read_state": {
            "entries": [{
                "id": "888001",
                "flags": 1,
                "last_message_id": "5001",
                "last_viewed": 4321,
                "mention_count": 3
            }]
        }
    });

    data::set_live_session(Some("live-test-token".to_string()), None);
    data::ingest_live_dispatch("READY", &live_ready);

    let ready_payload = data::ready(&root).expect("ready из живой сессии");
    assert_eq!(ready_payload.user.id, "777000");
    assert_eq!(ready_payload.user.username, "live_pilot");
    assert_eq!(ready_payload.guild_count, 1);
    assert_eq!(ready_payload.guilds[0].id, "888000");
    assert_eq!(ready_payload.guilds[0].name, "Live Server Alpha");
    assert_eq!(ready_payload.private_channel_count, 1);
    assert_eq!(ready_payload.relationship_count, 1);
    assert_eq!(ready_payload.read_state_count, 1);
    assert_eq!(ready_payload.users.len(), 1);

    let all_channels = data::channels(&root, None).expect("channels из живой сессии");
    assert_eq!(all_channels.len(), 3, "2 канала гильдии + 1 ЛС из live_d");
    let guild_channels =
        data::channels(&root, Some("888000")).expect("фильтр каналов живой гильдии");
    assert_eq!(guild_channels.len(), 2);
    assert_eq!(guild_channels[0].id, "888001");
    assert_eq!(guild_channels[1].id, "888002");

    let rs_entries = data::read_state(&root).expect("read_state из живой сессии");
    assert_eq!(rs_entries.len(), 1);
    assert_eq!(rs_entries[0].channel_id, "888001");
    assert_eq!(rs_entries[0].mention_count, Some(3));
    assert_eq!(rs_entries[0].last_viewed, Some(4321));

    // Отправленное в живой сессии сообщение сразу попадает в выдачу messages_page.
    data::record_sent_message(
        "888001",
        json!({
            "id": "5002",
            "channel_id": "888001",
            "content": "Привет из живой сессии!",
            "timestamp": "2026-10-31T15:00:00.000000+00:00",
            "author": {
                "id": "777000",
                "username": "live_pilot",
                "global_name": "Live Pilot"
            }
        }),
    );

    let msg_page =
        data::messages_page(&root, "888001", None, Some(50)).expect("сообщения живого канала");
    assert_eq!(msg_page.total, 1);
    assert_eq!(msg_page.items[0].id, "5002");
    assert_eq!(msg_page.items[0].content, "Привет из живой сессии!");

    // Сброс живой сессии возвращает чтение к фикстуре 002-READY.json.
    data::set_live_session(None, None);
    let fallback_ready = data::ready(&root).expect("fallback к фикстуре");
    assert_eq!(fallback_ready.guilds[0].id, "1");
    cleanup(&root);
}

#[test]
fn live_session_account_switch_to_zero_guilds_clears_previous_servers_and_fixtures() {
    let _write_guard = acquire_live_write_guard();
    let root =
        std::env::temp_dir().join(format!("dl-w47-live-zero-guilds-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог");
    write_minimal_ready(&root, "42", "1");

    // 1. Аккаунт 1 (с серверами, каналами, друзьями и сообщениями).
    let live_ready_acc1 = json!({
        "user": {
            "id": "777000",
            "username": "live_pilot",
            "global_name": "Live Pilot",
            "avatar": "a_live_hash"
        },
        "guilds": [{
            "id": "888000",
            "properties": {
                "name": "Live Server Alpha",
                "icon": "icon_888",
                "owner_id": "777000"
            },
            "member_count": 42,
            "channels": [
                {"id": "888001", "type": 0, "name": "general", "position": 1}
            ]
        }],
        "users": [{"id": "777001", "username": "friend_one"}],
        "relationships": [{"id": "777001", "type": 1}],
        "private_channels": [{"id": "999001", "type": 1, "recipient_ids": ["777001"]}],
        "read_state": {
            "entries": [{
                "id": "888001",
                "flags": 1,
                "last_message_id": "5001",
                "last_viewed": 4321,
                "mention_count": 3
            }]
        }
    });
    data::set_live_session(Some("live-test-token".to_string()), None);
    data::ingest_live_dispatch("READY", &live_ready_acc1);
    assert_eq!(data::ready(&root).expect("acc1 ready").guild_count, 1);

    // 2. Переключение на Аккаунт 2 (RU Rusty / @rusty021573), у которого 0 серверов.
    let acc2_user = discord_gui_spike::auth::AuthUser {
        id: "test-rusty-021573".to_string(),
        username: "rusty021573".to_string(),
        discriminator: "0".to_string(),
        global_name: Some("RU Rusty".to_string()),
        avatar: None,
        bot: false,
    };
    data::set_live_session(Some("live-test-token".to_string()), Some(&acc2_user));

    // Ещё до прихода READY от шлюза старые серверы Аккаунта 1 и фикстуры не должны утекать.
    let pre_ready = data::ready(&root).expect("ready до прихода WS READY");
    assert_eq!(pre_ready.user.id, "test-rusty-021573");
    assert_eq!(pre_ready.user.username, "rusty021573");
    assert_eq!(
        pre_ready.guild_count, 0,
        "до прихода READY чужие серверы не должны показываться"
    );
    assert!(pre_ready.guilds.is_empty());

    // 3. Приходит READY от шлюза для Аккаунта 2 с пустыми массивами guilds/private_channels/relationships/read_state.
    let live_ready_acc2 = json!({
        "user": {
            "id": "test-rusty-021573",
            "username": "rusty021573",
            "global_name": "RU Rusty",
            "avatar": null
        },
        "guilds": [],
        "users": [],
        "relationships": [],
        "private_channels": [],
        "read_state": {
            "entries": []
        }
    });
    data::ingest_live_dispatch("READY", &live_ready_acc2);

    let ready_acc2 = data::ready(&root).expect("ready acc2 без серверов");
    assert_eq!(ready_acc2.user.id, "test-rusty-021573");
    assert_eq!(ready_acc2.user.username, "rusty021573");
    assert_eq!(ready_acc2.user.global_name.as_deref(), Some("RU Rusty"));
    assert_eq!(ready_acc2.guild_count, 0, "у аккаунта 0 серверов");
    assert!(ready_acc2.guilds.is_empty(), "список гильдий обязан быть пустым");
    assert_eq!(ready_acc2.private_channel_count, 0);
    assert!(ready_acc2.private_channels.is_empty());
    assert_eq!(ready_acc2.relationship_count, 0);
    assert!(ready_acc2.relationships.is_empty());
    assert_eq!(ready_acc2.read_state_count, 0);
    assert!(ready_acc2.users.is_empty());

    let channels_acc2 = data::channels(&root, None).expect("channels acc2 без серверов");
    assert!(
        channels_acc2.is_empty(),
        "у аккаунта без серверов и ЛС список каналов обязан быть пустым: {channels_acc2:?}"
    );
    assert!(
        data::channels(&root, Some("888000")).is_err(),
        "гильдия предыдущего аккаунта не должна находиться"
    );
    assert!(
        data::channels(&root, Some("1")).is_err(),
        "гильдия из фикстуры не должна находиться"
    );

    let rs_acc2 = data::read_state(&root).expect("read_state acc2");
    assert!(
        rs_acc2.is_empty(),
        "пустой read_state живого аккаунта не должен откатываться к фикстуре"
    );

    data::set_live_session(None, None);
    cleanup(&root);
}

#[test]
fn live_session_syncs_relationship_private_channel_and_guild_dispatches() {
    let _write_guard = acquire_live_write_guard();
    let root =
        std::env::temp_dir().join(format!("dl-w47-live-rel-dm-sync-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("gateway/by-type")).expect("временный каталог");
    write_minimal_ready(&root, "42", "1");

    let live_ready = json!({
        "user": {
            "id": "777000",
            "username": "live_pilot",
            "global_name": "Live Pilot",
            "avatar": null
        },
        "guilds": [],
        "users": [],
        "relationships": [],
        "private_channels": [],
        "read_state": { "entries": [] }
    });
    data::set_live_session(Some("live-test-token".to_string()), None);
    data::ingest_live_dispatch("READY", &live_ready);

    // 1. Входящий запрос дружбы (RELATIONSHIP_ADD, type=3) с вложенным профилем user.
    data::ingest_live_dispatch(
        "RELATIONSHIP_ADD",
        &json!({
            "id": "777010",
            "type": 3,
            "user": {
                "id": "777010",
                "username": "incoming_friend",
                "global_name": "Incoming Friend",
                "avatar": "av_777010"
            }
        }),
    );
    let r1 = data::ready(&root).expect("ready after RELATIONSHIP_ADD");
    assert_eq!(r1.relationship_count, 1);
    assert_eq!(r1.relationships[0]["id"], "777010");
    assert_eq!(r1.relationships[0]["type"], 3);
    assert!(
        r1.users.iter().any(|u| u["id"] == "777010" && u["username"] == "incoming_friend"),
        "вложенный user из RELATIONSHIP_ADD обязан индексироваться в ready.users"
    );

    // 2. Подтверждение дружбы (RELATIONSHIP_UPDATE, type=1).
    data::ingest_live_dispatch(
        "RELATIONSHIP_UPDATE",
        &json!({
            "id": "777010",
            "type": 1
        }),
    );
    let r2 = data::ready(&root).expect("ready after RELATIONSHIP_UPDATE");
    assert_eq!(r2.relationships[0]["type"], 1);

    // 3. Создание группового чата (CHANNEL_CREATE, type=3) и добавление/удаление участника.
    data::ingest_live_dispatch(
        "CHANNEL_CREATE",
        &json!({
            "id": "999010",
            "type": 3,
            "name": "Rusty Squad",
            "recipients": [
                { "id": "777010", "username": "incoming_friend" }
            ]
        }),
    );
    let r3 = data::ready(&root).expect("ready after CHANNEL_CREATE");
    assert_eq!(r3.private_channel_count, 1);
    assert_eq!(r3.private_channels[0]["id"], "999010");
    assert_eq!(r3.private_channels[0]["name"], "Rusty Squad");

    data::ingest_live_dispatch(
        "CHANNEL_RECIPIENT_ADD",
        &json!({
            "channel_id": "999010",
            "user": { "id": "777011", "username": "third_member", "global_name": "Third Member" }
        }),
    );
    let r4 = data::ready(&root).expect("ready after CHANNEL_RECIPIENT_ADD");
    let recs = r4.private_channels[0]["recipients"].as_array().expect("recipients array");
    assert_eq!(recs.len(), 2);
    assert!(r4.users.iter().any(|u| u["id"] == "777011"));

    data::ingest_live_dispatch(
        "CHANNEL_RECIPIENT_REMOVE",
        &json!({
            "channel_id": "999010",
            "user": { "id": "777011" }
        }),
    );
    let r5 = data::ready(&root).expect("ready after CHANNEL_RECIPIENT_REMOVE");
    let recs_after = r5.private_channels[0]["recipients"].as_array().expect("recipients array");
    assert_eq!(recs_after.len(), 1);

    // 4. Присоединение к серверу по инвайту (GUILD_CREATE) и выход (GUILD_DELETE).
    data::ingest_live_dispatch(
        "GUILD_CREATE",
        &json!({
            "id": "888010",
            "name": "Joined Guild",
            "member_count": 12,
            "channels": [
                { "id": "888011", "type": 0, "name": "welcome", "position": 0 }
            ]
        }),
    );
    let r6 = data::ready(&root).expect("ready after GUILD_CREATE");
    assert_eq!(r6.guild_count, 1);
    assert_eq!(r6.guilds[0].id, "888010");
    assert_eq!(r6.guilds[0].name, "Joined Guild");

    // 5. Удаление друга, закрытие чата и удаление гильдии.
    data::ingest_live_dispatch("RELATIONSHIP_REMOVE", &json!({ "id": "777010" }));
    data::ingest_live_dispatch("CHANNEL_DELETE", &json!({ "id": "999010", "type": 3 }));
    data::ingest_live_dispatch("GUILD_DELETE", &json!({ "id": "888010" }));
    let r7 = data::ready(&root).expect("ready after removals");
    assert_eq!(r7.relationship_count, 0);
    assert_eq!(r7.private_channel_count, 0);
    assert_eq!(r7.guild_count, 0);

    data::set_live_session(None, None);
    cleanup(&root);
}


