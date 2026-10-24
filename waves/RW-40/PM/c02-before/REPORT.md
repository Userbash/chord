# C-02-BEFORE — older-history курсор (`before`/`position`) в BFF

- **Воркер:** G1 Backend, batch RW-40/PM, карточка **C-02-BEFORE** (P1B).
- **PM:** DeepSeek. **Дата:** 2026-10-24.
- **Офлайн:** live/окно/сеть/commit/full gate не запускались; cargo — последовательно,
  по одному процессу.
- **Источник:** `waves/RW-40/PM/c02-page/REPORT.md` §6 (вариант b),
  `waves/RW-40/PARITY/G6-RECHECK-01.md` (G1: `before`-курсор в `data.rs`,
  UI уже готов — `chat.js` не трогать), решение PM «older нужен».
- **Blocker:** нет. Backend хранит достаточно данных для older-курсора: строки канала
  уже дедуплицированы и отсортированы (`timestamp`, затем `id`), id строки однозначно
  задаёт позицию. Подделывать курсор/позицию не пришлось.

## 1. Контракт (определён и реализован)

### 1.1 IPC-команда

```
messages_page(channelId, cursor?, before?, position?, limit?)
```

| Аргумент | Смысл |
|---|---|
| `channelId` | реальный snowflake канала (прежняя проверка: trim, непустой, ≤256) |
| `cursor` | **forward**: id последней строки уже загруженной страницы; отдать строки строго **после** него (поведение не изменилось) |
| `before` | **older**: id первой строки текущего окна; отдать не более `limit` строк строго **до** него, в том же возрастающем порядке, что у forward-страниц |
| `position` | `"start"` (по умолчанию) — первая страница с начала истории; `"latest"` — стартовая страница с конца (последние `limit` строк) |
| `limit` | `1..=100` (прежний `MAX_PAGE_LIMIT`), по умолчанию 50 |

Правила: `cursor` и `before` **взаимоисключающие**; `position:"latest"` не сочетается
ни с одним курсором. Комбинация — `InvalidArg`, а не тихий приоритет. Пустой/пробельный
`cursor`/`before` = «якоря нет» (как раньше у `cursor`); неизвестный или длиннее 256
символов `before` — `InvalidArg`; неизвестное `position` — `InvalidArg` с усечённым
значением. Неизвестный канал — `NotFound`, тихий канал — пустая страница (не ошибка).

### 1.2 Ответ `MessagesPage` (camelCase)

```
{ channelId, items, nextCursor, hasMore, hasOlder, olderCursor, total, limit, loadedRowsLimit }
```

- `nextCursor`/`hasMore` — **forward без изменений**: id последней строки страницы и
  «есть строки новее»; пара согласована (пустая страница не обещает курсор);
- `hasOlder` — есть ли строки старше первого элемента страницы;
- `olderCursor` — id **первой** строки страницы (следующий `before`), `null` при
  `hasOlder=false`; курсор не выдумывается;
- `total` — все строки канала (до пагинации), `limit` — запрошенный размер,
  `loadedRowsLimit` — прежний инвариант окна UI (`UI_WINDOW_ROWS = 500`);
- других полей нет: `position`/`before` в ответ не эхо-тся, «направление» не добавляется.

### 1.3 Как это использует UI (G2, отдельная правка `chat.js`)

- открытие на последнем сообщении: `{channelId, position:"latest", limit:100}`
  → хвост в возрастающем порядке, `olderCursor` = якорь для скролла вверх;
- скролл вверх: `{channelId, before: olderCursor, limit:100}`; строки приходят
  возрастающими, существующий `mergeChannelItems` делает prepend + дедуп;
  `hasOlder=false` — истории выше нет;
- forward-ветка (`cursor: nextCursor`) не меняется.

## 2. Что сделано

`rust/gui-spike/src/data.rs`:

- `MessagesPage` + `hasOlder`/`olderCursor` (существующие поля не переименованы);
- `pub enum MessagesPosition { Start, Latest }` + `MessagesPosition::parse` —
  типизированный разбор IPC-значения (`start|latest`), ошибка `InvalidArg`;
- `pub struct MessagesPageRequest<'a> { cursor, before, position, limit }` —
  один запрос вместо зоопарка аргументов; `Default` = forward с начала;
- `pub fn messages_page_request` — диспетчер: forward (`cursor`), older (`before`),
  хвост (`position=latest`); взаимоисключения и `limit` проверяются до чтения фикстур;
- `fn messages_window` — единственная сборка ответа: полуинтервал `start..end`
  дедуплицированного/отсортированного списка; `hasMore`/`nextCursor` и
  `hasOlder`/`olderCursor` считаются независимо;
- `pub(crate) fn cursor_end` — позиция `before` (строки строго до якоря), чужой id —
  `InvalidArg`; `cursor_start` не менялся;
- `messages_page(root, channelId, cursor, limit)` — **прежняя сигнатура** сохранена,
  делегирует в `messages_page_request` (старые вызовы/тесты не правились);
- шапка модуля: новый контракт пагинации.

`rust/gui-spike/src/main.rs`:

- Tauri-команда `messages_page` принимает `before`/`position`, разбирает позицию и
  вызывает `data::messages_page_request`; остальные команды не тронуты.

`rust/gui-spike/tests/history_before_contract.rs` (новый, 12 тестов) — оракул: сырые
кадры `fixtures/gateway/by-type/*.json` канала `-2684694197495146496` (8 уникальных
`MESSAGE_CREATE`), независимо дедуплицированные и отсортированные по `timestamp`, `id`.
`tests/data_contract.rs`, `tests/core_bridge.rs`, `src/core_bridge.rs`, `src/net.rs`,
`ui/**`, `tests/unit/channel-paging.test.mjs` — **не менялись**.

## 3. Red (red-first)

Тест написан до кода; первый прогон — против API, которого ещё нет
(`waves/RW-40/PM/c02-before/red.log`, 9 273 байта):

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test history_before_contract
EXIT=101
error[E0432]: unresolved import `discord_gui_spike::data::MessagesPageRequest`
error[E0432]: unresolved import `...::MessagesPosition`
error[E0425]: cannot find function `messages_page_request` in module `data`
error[E0609]: no field `has_older` / `older_cursor` on type `MessagesPage`
error: could not compile `discord-gui-spike` (test "history_before_contract")
due to 26 previous errors
```

Ни один assert после red не ослаблен. Тест на dedup/`MESSAGE_UPDATE` добавлен позже и
честно падал на неверном ожидании самого теста (`has_older` у страницы, начинающейся
с первой строки канала); исправлено ожидание, не реализация.

## 4. Green

```
cargo test ... --test history_before_contract
EXIT=0  12 passed / 0 failed        (targeted.log)

cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
EXIT=0  281 passed / 0 failed, 0 warnings   (green.log)

node --test tests/unit/channel-paging.test.mjs
EXIT=0  8 passed / 0 failed         (channel-paging.log)

cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
EXIT=0                              (clippy.log)

rustfmt --edition 2021 --check <data.rs main.rs history_before_contract.rs>
EXIT=0
```

Было 269 тестов gui-spike (281 − 12 новых), стало 281: существующие не менялись и
не ослаблялись. `channel-paging` — 8/8, forward-контракт UI не сломан.

Покрытие (`history_before_contract.rs`, оракул — фикстуры, не реализация):

1. `before` перед 5-й строкой, limit 2 → ровно 2 строки строго старше якоря,
   возрастающий порядок, `hasOlder`, `olderCursor`, `nextCursor`/`hasMore`, `total`.
2. Обход вверх до начала: страницы не пересекаются, покрывают начало, `hasOlder=false`
   на первой строке; страница старше самой старой строки — пустая, без курсоров.
3. Строки older-страницы **побайтово равны** тем же строкам полного forward-обхода.
4. `position:"latest"` → хвост канала без знания id; затем `before` листает к началу.
5. Хвост при `limit > total` — одна страница, `hasOlder=false`.
6. Silent-канал + `latest` → пустая страница, `total=0`, курсоры `null` (не выдуманы).
7. Forward не изменился: старый `data::messages_page` даёт тот же ответ, что
   request-форма; новые поля не влияют на `nextCursor`/`hasMore`.
8. Взаимоисключения: `cursor+before`, `latest+cursor`, `latest+before` → `InvalidArg`.
9. `position` вне `start|latest` → `InvalidArg`; чужой/длинный `before` → `InvalidArg`;
   пустой `before` = без якоря.
10. `limit` 0 и 101 → `InvalidArg`; `limit=100`, `loadedRowsLimit=500`.
11. Dedup на older-странице: 4 кадра (CREATE×3 + UPDATE того же id) → `total=3`,
    одна строка на id, UPDATE уточняет текст/`edited`, страницы не пересекаются.
12. Сериализация: ровно 9 документированных ключей, `hasOlder`/`olderCursor` camelCase.

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` (изменён) | `a99647c19a4cfbdd7e1640506ea2e650` |
| `rust/gui-spike/src/main.rs` (изменён) | `336863b5b1ffca169b851a2095798f01` |
| `rust/gui-spike/tests/history_before_contract.rs` (new) | `796293ea6ae985dcb17dfd96a61bf1a1` |

Не тронуты (md5 до/после совпадает): `src/core_bridge.rs` `ef2ab3b3441365209c72fd11ccff1ac6`,
`tests/data_contract.rs` `a2694e42ebfde099265b70ebad469cee`,
`tests/core_bridge.rs` `fe336446162cb4765d6728bfce95e9ef`,
`src/net.rs` `e13d4e8cb4fbb22af2ce6d96702f0e8a`,
`ui/features/chat.js` `f4c8076386755a76efce9166dba71e50`,
`tests/unit/channel-paging.test.mjs` `54184e41ce632d3fdb47cec9672095e0`.
`logs/`, `functions/`, `evidence/`, `waves/db.jsonl`, `STATE.md`, `TASKS.md` — не трогались.

## 6. Остаток / риски (нужно решение PM)

1. **UI-ветка older не сделана** — карточка G1, `chat.js` по матрице принадлежит G2.
   Backend готов: `position:"latest"` + `before`/`hasOlder`/`olderCursor`; prepend
   и якорь в `mergeChannelItems` уже реализованы C-02-PAGE.
2. **`before`-страница сохраняет forward-метаданные.** `nextCursor` = id последней
   строки страницы, `hasMore` = есть строки новее. UI обязан хранить older-курсор
   отдельно (`olderCursor`), а не переиспользовать `nextCursor` как «вверх» —
   иначе направления перепутаются. Альтернатива (менять `nextCursor` на older) сломала
   бы текущий forward-контракт, поэтому не выбрана.
3. **Предел страницы 100, окно UI 500.** Требование «limit ≤ 500» выполняется
   инвариантом `loadedRowsLimit=500`; `MAX_PAGE_LIMIT` не повышался (это отдельное
   изменение IPC-контракта и тестов C-02-PAGE). Если PM нужен `limit` до 500 —
   отдельная карточка.
4. **RE-паритет live-истории не проверялся.** Реальный Discord REST history
   (`GET /channels/{id}/messages?before=&after=&limit=`) в `net.rs`/`messages.rs` не
   реализован и в этой карточке не открывался; BFF остаётся на фикстурах. При live-
   подключении `before`-снежинка и `hasMore` должны маппиться на ответ REST — это
   отдельный контракт G1 (сейчас нет live-оракула).
5. **`position` — только `start|latest`.** Числовая позиция/offset не вводилась
   (выдуманное поле без RE-оракула). Если нужен произвольный offset — новое решение PM.
6. **`core_bridge` (пилот) остаётся forward-only** и не отдаёт `hasOlder`: он не
   участвует в IPC/UI и имеет собственный `CoreMessagesPage`; менять его в этой
   карточке не требовалось, чтобы не расширять scope. При подключении моста к UI —
   отдельная правка с `data::cursor_end`.
7. Приёмка/мутации/`waves/db.jsonl`/`STATE.md`/доки — за Team Lead. **Blockers нет.**
