# RW-14 / P2-06 — read-only research

Дата: 2026-10-20. Источники: `docs/re/rest-d02.md`, `docs/re/rest-d02.json`,
`docs/rust-client/{PLAN,TEST-PLAN,STATUS,ROADMAP}.md`, `fixtures/gateway/by-type/*`,
`rust/crates/discord-core/src/{rest,state}`. Сеть, живой Discord, golden и `/home/sanya/re-deep`
не использовались.

## 1. Постановка

- P2-06 в `PLAN.md:74`: `GET /messages` лимиты, бесшовный скролл, кэш; T-DATA-08/T-REST-08;
  зависимости `P2-03`, `P1-07`.
- T-REST-08 (L2): «история: лимиты/after/before; 11 запросов кейс», оракул `rest-d02.json` §2.
- T-DATA-08 (L3): «пагинация истории вверх/вниз без дыр/дублей», стаб-сервер.
- `P2-03` (SQLite) заблокирован; ROADMAP явно допускает memory-вариант после `P2-02`/`I1`.
  **RW-14 = memory-only history core; SQLite, перезапуск и REST-JSON — вне волны.**

## 2. Подтверждено

| Факт | Доказательство |
|---|---|
| `GET /channels/{id}/messages` — 11 вызовов, все 200, Authorization, тела запроса нет | `docs/re/rest-d02.md:70`, `rest-d02.json:87–115` |
| Наблюдённые query: `?limit=12` ×8, `?before=<id>&limit=24` ×2, `?limit=30&around=<id>` ×1 | `rest-d02.md:180`, `:316` |
| `limit` — 16 вхождений, 11 из них messages | `rest-d02.md:153`, `rest-d02.stats.json:151` |
| `after` в захвате — 0 раз | проверка `rest-d02.json`; в прозе только обобщение `:180` |
| Тело ответа `/channels/*/messages` не захвачено; схема не извлекается | `rest-d02.md:287` |
| `fixtures/rest/rest-static.json` — каталог, `MESSAGES` `confirmed: false`, `bodyKeys: []` | `rest-static.json:2227–2241` |
| `has_more` в статике относится к `threads/search`, не к messages | `rest-static.json:1884–1895` |
| `RouteSpec` не хранит query; `render_path` только путь | `rest/routes.rs:34–43,171–186` |
| `ApiClient` принимает сырой target, JSON не парсит | `rest/client.rs:242,328–331`; `rest/transport.rs:56,96` |
| `state::Message` — id/channel/guild/author/content/timestamps/type/flags/pinned/tts/mentions/counts | `state/types.rs:240–275` |
| `MessagePatch` и `apply_patch_fields` приватны; live-update применяется reducer-ом | `state/reducer.rs:275–367` |
| `Effect::FetchMessages { channel_id, before, limit }` существует, `FetchMessages` не эмитится | `state/types.rs:559–570`; `reducer.rs:117` |
| `State.messages` — неограниченный `BTreeMap`; селектор O(N) | `state/types.rs:99`; `state/selectors.rs:67–80` |
| `MESSAGE_DELETE` нет среди 21 типа | `dispatch.rs:18–58`; `state/types.rs:841–899` |
| SQLite/serde/serde_json/rusqlite в `Cargo.lock`/`Cargo.toml` отсутствуют | `rust/Cargo.toml`, `discord-core/Cargo.toml`, `Cargo.lock` |

### Реальные сообщения

3 create + 6 update (seq 18–26), `fixtures/gateway/summary.json`:

| Файл | id | content | md5 |
|---|---|---|---|
| `023-MESSAGE_CREATE.json` | `1561627527296979046` | 208 | `d82d96cada5678de01bc18ab5e967430` |
| `024-MESSAGE_CREATE.json` | `1561627527833849898` | 168 | `df05409d5d7e939504a09c09b3a6ad19` |
| `025-MESSAGE_CREATE.json` | `1561627528685166655` | 173 | `038e0e1fa9621c08a95a666c008b3ea7` |
| `026-MESSAGE_UPDATE.json` | первый id | 208, embed 1 | `7bbf2592c7c41fbbc79940286cd01a85` |

Канал `1163408591315812392`, гильдия `1014537851418980372`, автор
`268478587651358721` (bot). Кластер `062–092` — санитайзенные отрицательные id, годится только
для malformed-тестов.

## 3. Неизвестно (прятать за интерфейсом)

- форма ответа, порядок, nullability, `has_more`, max/default limit, семантика `around`;
- `after` — только контракт, capture не подтверждает;
- ошибки 403/404/429 и тело ошибки для messages;
- `MESSAGE_DELETE`/tombstones;
- перезапуск/офлайн (SQLite);
- JSON-парсинг (нет serde).

`MessagePage` — единственный вход страницы в core; адаптер REST/JSON/сеть появится позже.
`has_older`/`has_newer` — tri-state (`None` = неизвестно, не false). Короткая страница не
означает конец истории.

## 4. Контракт-предложение (заморожен в DESIGN)

- `History` — in-memory, per-channel `BTreeMap<u64, Message>`, bounded window.
- `HistoryConfig { channel_window: 100, page_limit: 50 }` — локальные guard-числа, не факты API.
- `History::apply_live(full Message)`; live > page, page/page last-wins, dedup by id.
- `PageRequest`/`Cursor` (`Latest`/`Before`/`After`/`Around`), `MessagePage`, `PageToken`
  (generation), `PageError`; `from_effect` понимает только `FetchMessages` (`before`).
- `apply_page` атомарен при `ChannelMismatch`; stale token не меняет данные.
- Окно: Latest/After — новые N; Before — старые N; Around — ближайшие к anchor, tie → меньший id.
- Detached live (прокрутка вверх): не вытеснять старые, поднять `has_newer`, вернуть `Deferred`.
- Селекторы: `messages`, `recent`, `message`, `oldest/newest`, `range`, `window_around`,
  `has_older/newer`, `len`; только чтение.
- Без SQLite/UI/сети/новых зависимостей; отмена/таймауты — на будущем адаптере.

## 5. Риски

| Уровень | Риск | Что делаем |
|---|---|---|
| P1 | `State.messages` не ограничен, history cap его не ограничивает | не заявлять «кэш всего состояния»; отдельная задача на bound/TTL |
| P1 | REST response/`has_more`/max limit/`after` не подтверждены | `MessagePage` seam; request-формы только по capture, `after` помечен контрактом |
| P1 | Нет SQLite/перезапуска | memory-only; P2-03 отдельно |
| P2 | Устаревшая страница может перезаписать live | `PageToken` + Live > Page |
| P2 | Нет serde/JSON | адаптер вне core, позже |
| P2 | Нет MESSAGE_DELETE | tombstones — отдельная задача |
| P3 | Оракул истории слабый (3 create/6 update, 0 страниц) | реальные фикстуры + явно синтетические страницы |

## 6. Вывод

Research достаточен для red-first memory history/pagination core. Полный P2-06 (реальный
REST-ответ, SQLite, перезапуск, L3 на сервере) этой волной не закрывается и должен быть
отражён в приёмке как ограничение scope.
