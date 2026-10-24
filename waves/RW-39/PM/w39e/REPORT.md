# REPORT W39e — backend BFF на `fixtures/` (Phase 3)

Статус: **green** (17/17 контрактных тестов, clippy/fmt exit 0). Сеть, GUI, probe,
gate:worker, commit/push не запускались. `Cargo.toml` не менялся, новых крейтов нет.

## Что сделано

Новый модуль `rust/gui-spike/src/data.rs` — BFF без сети: читает только локальные
`fixtures/gateway/by-type/*.json`. Чистые функции вынесены из Tauri-команд, поэтому
контракт проверяется обычными интеграционными тестами.

- Корень данных: `DL_FIXTURES` → иначе `<repo>/fixtures` (вычисляется из
  `CARGO_MANIFEST_DIR`, домашний каталог лаборатории не зашит). `resolve_root(Option<&str>)`
  — чистая функция, приоритет проверяется без мутации env.
- Типизированные ошибки: собственный `enum DataError { Io, Parse, NotFound, InvalidArg }`
  (без `thiserror`, без новых зависимостей). JSON:
  `{"kind":"Parse","message":"…","path":"…"}` / `{"kind":"NotFound","message":"…","resource":"…"}`.
  `Display`/`std::error::Error` реализованы; на входных данных нет `unwrap()` —
  отсутствие обязательного поля даёт `Parse` с путём файла.
- Tauri-команды (`src/main.rs`, зарегистрированы в `generate_handler!`):
  - `ready()`;
  - `messages_page(channelId, cursor?, limit?)`;
  - `members_page(guildId, cursor?, limit?)`;
  - `settings(section?)`.
- `src/lib.rs`: `pub mod data;` + описание модуля (старые функции не тронуты).

### Контракт ответов (camelCase; для W39d)

| Команда | Ответ |
|---|---|
| `ready` | `{user{id,username,globalName,avatar,email,bio,premium,mfaEnabled}, guilds[{id,name,icon,memberCount,channelCount}], guildCount, privateChannelCount, relationshipCount, readStateCount, session{client,os,version,status}\|null}` |
| `messages_page` | `{channelId, items[{id,channelId,guildId,author{id,username,globalName,avatar,bot},content,timestamp,edited,attachmentCount,embedCount,mentionEveryone}], nextCursor, hasMore, total, limit, loadedRowsLimit}` |
| `members_page` | `{guildId, guildName, memberCount, items[{id,username,displayName,avatar,status,bot,roles}], nextCursor, hasMore, total, limit, loadedRowsLimit}` |
| `settings` | `{sections[{id,title,values}], selected}`; секции: `profile`, `notifications`, `sessions`, `privacy`, `data` |

Семантика пагинации: `cursor` — id последней строки предыдущей страницы (непрозрачен
для UI), `null`/`""` — с начала; страница содержит строки **строго после** курсора.
`limit` по умолчанию 50, допустим `1..=100`, иначе `InvalidArg`. `hasMore=false` и
`nextCursor=null` — конец. Пустая страница (`items: []`, `hasMore:false`) — валидный
ответ (тихий канал / гильдия без снятых участников / хвост выборки).
`loadedRowsLimit = 500` — инвариант UI, сервер отдаёт не более `limit` строк.
`MESSAGE_CREATE`/`MESSAGE_UPDATE` одного id дедуплицируются (UPDATE помечает `edited`
и уточняет текст); порядок — `timestamp`, затем id (snowflake не парсим: в фикстурах
есть отрицательные id). Участники сортируются по presence (`online`→`offline`), затем
по `displayName`, затем id.

## Проверки: команда → вход → выход

| # | Команда | Вход | Выход |
|---|---|---|---|
| 1 | `cargo test --offline --manifest-path rust/gui-spike/Cargo.toml` | `fixtures/` (repo) | **exit 0**: data_contract 17 passed; ipc_contract 20 passed; launch 1 passed (skip без `GUI_SPIKE_E2E=1`); lib/main 0 |
| 2 | `cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings` | — | **exit 0**, без предупреждений |
| 3 | `cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check` | — | **exit 0** |
| 4 | `DL_FIXTURES=/nonexistent/fixtures cargo test … --test data_contract` | фикстур нет, `DL_STRICT` не задан | **exit 0**: 7 тестов без фикстур зелёные, 10 skip с причиной (`SKIP data_contract: нет фикстур в …`, видно с `--nocapture`) |
| 5 | `DL_FIXTURES=/nonexistent/fixtures DL_STRICT=1 cargo test … --test data_contract` | фикстур нет, CI-режим | **exit 101**: `DL_STRICT=1: фикстуры обязательны, но /nonexistent/fixtures не найден` — ожидаемо красный |
| 6 | red-прогон до реализации | `tests/data_contract.rs` | **exit 101**: `unresolved import discord_gui_spike::data` (модуля ещё нет) |

Логи: `waves/RW-39/PM/w39e/red-test.log`, `green-targeted.log`, `green-all.log`,
`clippy.log`, `fmt.log`, `skip-mode.log`, `strict-mode.log`.

## Покрытие веток (data_contract.rs)

| Область | Ветки |
|---|---|
| `ready` | реальный READY против оракула `ready-stats.json` (7 гильдий / 17 ЛС / 60 связей); `properties.name` (=`ITSKIDON PC BOOST`), `member_count` (24 228), сессия `desktop/linux/invisible`, email остаётся `REDACTED` |
| `messages_page` | 3 уникальных сообщения из 9 кадров CREATE/UPDATE; страницы 2+1 по курсору; повтор с тем же курсором идемпотентен; хвост → пустая страница; известный тихий канал → пустая страница; `total`; `edited` |
| `members_page` | гильдия `-1467272931325252608`: 35 кадров, 98 уникальных участников, 4 страницы по 25; отсутствие дублей; `total ==` объединению страниц; presence-статусы непустые; гильдия без снятых участников → пустая страница |
| `settings` | 5 секций в стабильном порядке; профиль = READY.user; счётчики `data` = оракул `ready-stats`; фильтр по секции; неизвестная и пустая секция → `NotFound` |
| ошибки | `Parse` (битый READY, битый by-type), `NotFound` (нет READY, пустой каталог, неизвестные канал/гильдия/секция), `Io` (`gateway` — файл вместо каталога), `InvalidArg` (`limit` 0/101/`usize::MAX`, чужой курсор, пустой id), JSON-сериализация всех четырёх видов |
| root | `resolve_root(Some(..))` приоритет env; дефолт оканчивается на `fixtures` и существует |

Негативные кейсы из брифа закрыты: битый JSON, пустой каталог, отсутствующий id
(канал/гильдия/секция/READY), плюс `Io` и недопустимые аргументы.

## Красное (честно)

- Первый green-прогон: 16 passed / 1 failed — `unreadable_catalog_is_io_error` падал
  на **ошибке теста**, не продукта: `temp_root` уже создавал каталог `gateway`, и запись
  файла на его место давала `IsADirectory`. Исправлен setup теста (каталог удаляется
  перед записью файла), продовый код не менялся.
- `data_contract.rs` изначально давал warning `unused_assignments` (`total = None`
  перезаписывался в цикле) — убрано объявлением без инициализации; clippy чист.
- `cargo fmt --check` сначала был красным для `data.rs`/`data_contract.rs`;
  отформатированы только эти два файла (`rustfmt --edition 2021`). md5
  `ipc_contract.rs`/`launch.rs` до и после совпадают — чужие тесты не тронуты.

## Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` (новый, 1004 строки) | `eb832c379b56c39395937bec542371a3` |
| `rust/gui-spike/src/lib.rs` (+`pub mod data`) | `3de13bca66001aae70b0a924bcd16fea` |
| `rust/gui-spike/src/main.rs` (+4 команды) | `1dc3c8be0adfb1d99c4c616bc5f3a135` |
| `rust/gui-spike/tests/data_contract.rs` (новый, 547 строк) | `530a323235994feabc68a7714fe5aca7` |

`rust/gui-spike/Cargo.toml` — без изменений (зависимости `serde`, `serde_json`, `tauri`
уже были). `fixtures/**`, `rust/gui-spike/ui/**`, `tests/ipc_contract.rs`,
`tests/launch.rs` — read-only, не изменялись.

## Что осталось / риски

- Вызовы из фронтенда (`features/*.js`) и шим `window.__TAURI__` — зона W39d; имена
  аргументов Tauri v2 принимает в camelCase (`channelId`, `guildId`), команды —
  `ready`/`messages_page`/`members_page`/`settings`.
- Живой Discord и GUI smoke не запускались по брифу; `launch.rs` в прогоне самоскипнулся
  (нет `GUI_SPIKE_E2E=1`).
- `members_page` отдаёт только снятые `GUILD_MEMBER_LIST_UPDATE` (98/96/15 участников
  для трёх гильдий); для остальных гильдий READY — пустая страница при известном
  `memberCount`. Догрузка `merged_members`/`users` не делалась: в фикстурах по одному
  участнику на гильдию, это создало бы ложное впечатление полного списка.
- `user_settings_proto` — protobuf в base64; без нового крейта не декодируется, поэтому
  в `settings` не отдаётся (в payload попадают только поля READY, которые читаются как JSON).
- Кэша нет: каждый вызов перечитывает `by-type` (164 файла, ~1.5 МБ); для спайка
  приемлемо, при переходе на реальный backend — отдельная задача.
- `ready()`/`settings()` читают `002-READY.json` по фиксированному имени — это имя
  зафиксировано в `fixtures/MANIFEST.json` и `README.md`, при пересборке фикстур
  контракт нужно сверить повторно.
- Коммит/пуш не делались (уровень координатора).
