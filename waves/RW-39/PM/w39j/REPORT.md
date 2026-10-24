# W39j — отчёт: мост BFF → `discord-core` (пилот)

Владелец: W39j (Backend/Integration). PM: DeepSeek. Дата: 2026-10-23.
Ветка/HEAD: `0239543` (RW-35 принят; `rust/gui-spike/` в git ещё не отслеживается).

## Итог

Пилот сделан: страница сообщений собирается из ядра (`state::Message` +
`history::History`), а не только из JSON-строк BFF. Формат ответов BFF не менялся,
`tests/data_contract.rs` не тронут (29/29 зелёные).

- **Path-зависимость** `discord-core = { path = "../crates/discord-core" }` в
  `rust/gui-spike/Cargo.toml`. Внешних крейтов не добавляли: транзитивные
  зависимости `discord-core` (tokio/rustls/… ) уже были в кэше registry и
  собираются `--offline`. `rust/Cargo.lock` не менялся (у спайка свой workspace/lock).
- **`src/core_bridge.rs`** (новый):
  - `message_from_row(&MessageRow) -> Result<Message, DataError>` — id/channel/
    guild/author/content/timestamp/edited_timestamp/pinned/message_type,
    mention_user_ids, `embed_count`/`attachment_count`; `flags = 0`, `tts = false`,
    `mention_role_ids = []` (в строке BFF этих данных нет — не выдумываем).
  - `core_messages_page(root, channelId, cursor, limit) -> CoreMessagesPage` —
    строки берёт общий с BFF загрузчик `data::channel_rows`, конвертирует в
    `Message`, кладёт в `History` (окно = весь канал, `page_limit` = ёмкость),
    страницу выбирает селектором `History::messages` (возрастание id).
    Курсор проверяется **той же** функцией `data::cursor_start`, что у
    `messages_page`; `limit` — тем же `data::page_limit` (1..=100).
  - Нечисловой/отрицательный/переполненный id (сообщения, канала, автора,
    гильдии, упоминания) → `DataError::Parse` (`kind = "Parse"`), без паники и
    без тихого пропуска строки.
- **`src/data.rs`**: `SearchFilters` получил `#[derive(Deserialize)]` +
  `#[serde(rename_all = "camelCase", default)]`; `page_limit`/`cursor_start`
  стали `pub(crate)`; выделен `pub(crate) fn channel_rows` (общий загрузчик),
  `messages_page` использует его — поведение/ошибки прежние.
- **`src/main.rs`**: зеркало `SearchFiltersArg` + `From` удалено,
  `search_page` принимает `Option<SearchFilters>` напрямую; тесты в
  `wiring_tests` десериализуют канонический тип (проверка не ослаблена).
- **`tests/core_bridge.rs`** (новый, 6 тестов): первые страницы BFF и ядра
  совпадают по ids/порядку/курсору/`hasMore`/`total`; постраничный проход с
  `limit = 1` вперёд даёт ту же последовательность, повтор с тем же курсором
  стабилен, страницы не пересекаются; хвост за последней строкой и тихий канал
  — валидные пустые страницы; конверсия полей; нечисловой id → `Parse`;
  санитизированные отрицательные id → `Parse`.

## Совпадение ids (реальные фикстуры)

Канал `1163408591315812392` (READY, положительные snowflake, 3 сообщения после
дедупликации CREATE/UPDATE) — ids и порядок совпали на всех страницах:

```
1561627527296979046
1561627527833849898
1561627528685166655
```

`next_cursor`, `hasMore` и `total` совпали с `data::messages_page` на каждом шаге
(`limit = 1`, `2`, `3`, `MAX_PAGE_LIMIT`) и на повторе.

## Ограничения (честно)

- Санитизированные id дампа отрицательны (каналы `-1467273046216692736` и др.).
  Ядро хранит `u64`, поэтому такие строки конвертируются в `Parse` — это
  зафиксировано тестом. На положительных snowflake порядок ядра (числовой id)
  совпадает с порядком BFF (`timestamp`, затем `id`).
- `Message` ядра не несёт embeds/attachments/reactions/replyTo — мост отдаёт
  только счётчики; полные объекты остаются в BFF, поэтому полная замена
  `messages_page` на ядро пока невозможна без расширения модели ядра.
- Новых Tauri-команд не регистрировали (мост — library-level пилот): IPC-контракт
  и 20 тестов `ipc_contract` не менялись.
- **Осталось на следующую итерацию**: `settings` (включая R5-лимиты) и `voice`
  через ядро; расширение `state::Message` вложениями/embeds; решение, делать ли
  `messages_page` основным путём (тогда нужен маппинг полей ядра обратно в BFF).
- `cargo metadata --offline` падает на `android_system_properties v0.1.6`
  (нет в кэше; target-specific). Сборка/тесты/clippy/fmt `--offline` проходят;
  сеть не тянули, вендоринга нет. Если PM нужен `cargo metadata` — это отдельный
  вопрос к окружению, не к коду.

## Тесты

| Прогон | Было | Стало |
|---|---|---|
| `cargo test --offline` | 53 (3 bin + 29 data_contract + 20 ipc + 1 launch) | **59** (+6 `core_bridge`) |
| `data_contract` (не трогали) | 29 | 29 |
| `cargo clippy --all-targets -- -D warnings` | — | 0 предупреждений |
| `cargo fmt -- --check` | — | чисто |

Red-first (логи в `waves/RW-39/PM/w39j/`):
- `red-core_bridge.log` — `unresolved import discord_gui_spike::core_bridge`;
- `red-search-filters.log` — `SearchFilters: Deserialize` не реализован (E0277);
- `green-core_bridge.log`, `green-all.log` — зелёные;
- `clippy.log`, `fmt-before.log` (первый `--check` поймал форматирование),
  `fmt-check.log` — финальный.

## Файлы и md5

```
d71d063f85d335b6a376c852a54f3df0  rust/gui-spike/Cargo.toml
9bfa908061e1cace68665355cd8e491e  rust/gui-spike/Cargo.lock   (регенерирован cargo)
ef2ab3b3441365209c72fd11ccff1ac6  rust/gui-spike/src/core_bridge.rs   (новый)
6f674651419cfdecf92b143d606d2678  rust/gui-spike/src/data.rs
ca60b055de880e9b066053b4ffd80871  rust/gui-spike/src/lib.rs
82ae5f60629a9c067ef4272bddfc62a2  rust/gui-spike/src/main.rs
fe336446162cb4765d6728bfce95e9ef  rust/gui-spike/tests/core_bridge.rs  (новый)
```

`tests/data_contract.rs` не изменялся; `ui/**`, `fixtures/**`, остальные тесты —
read-only, не тронуты. Окно/probe/gate/сеть/commit не запускались.
