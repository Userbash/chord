# C-02-BE — READY-проекция каналов (закрывает backend-часть G6-D-01)

- **Воркер:** G1 Backend, batch RW-40/PM, карточка **C-02-BE**.
- **Дата:** 2026-10-24. Офлайн: без live/сети/окна, без commit.
- **Источник:** `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.4; дефект
  `G6-D-01` (`channelId:"news"` → live 400/404) — backend-часть.
- **Новый REST endpoint не вводился:** только READY-проекция поверх существующей
  фикстуры `fixtures/gateway/by-type/002-READY.json`.

## 1. Что сделано

`rust/gui-spike/src/data.rs`:

- `pub struct ChannelRow { id, guildId, name, type, parentId, position }`
  (`#[serde(rename_all = "camelCase")]`, `type` — числом через
  `#[serde(rename = "type")]`; поля без выдуманных значений).
- `pub fn channels(root: &Path, guild_id: Option<&str>) -> Result<Vec<ChannelRow>, DataError>`:
  - источники — `READY.guilds[].channels[]` (272 записи, типы `0/2/4/5/15`) и
    `READY.private_channels[]` (17 ЛС, тип `1`); каналы, встреченные только в
    `MESSAGE_*`, в дерево **не** попадают;
  - `guildId` — id гильдии, у ЛС `null`; `parentId` — `parent_id` (`"nil"` → `null`);
    `position` — число из READY (у ЛС `null`); `name` — из READY (у ЛС имени нет → `null`);
  - стабильный порядок: гильдии в порядке READY (как рейл), внутри — `position` asc,
    затем `id` asc; ЛС — последними (нет гильдии и позиции) по `id`;
  - `guildId`-фильтр: trim, ≤ `MAX_INPUT_LEN` (иначе `InvalidArg` с `…` вместо
    значения целиком), пустой — `InvalidArg`, неизвестная гильдия — `NotFound`;
  - ошибки `DataError`: нет READY → `NotFound`, битый JSON / нет `d` /
    `guilds`/`private_channels` не массив / канал без `id` или числового `type` →
    `Parse`; пустой список при отсутствии READY не отдаётся;
  - `messages_page` не менялся; `known_channels`/`channel_index` не тронуты.
- Документация модуля: команда `channels(guildId?)` в списке IPC.

`rust/gui-spike/src/main.rs`:

- Tauri-команда `channels(guild_id: Option<String>) -> Result<Vec<ChannelRow>, DataError>`:
  `data::fixtures_root()` + `data::channels`; **без** `AuthBackend`/`token_or`/keyring,
  без `println!/eprintln!/log::` (локальная проекция, логировать нечего).
- Регистрация в `invoke_handler!` (рядом с `ready`).
- Wiring-тест `channels_command_is_local_and_carries_no_token` (исходник бинаря,
  окно не нужно) + `channels` в списке `invoke_handler_registers_all_bff_commands`.

`rust/gui-spike/src/lib.rs`: в шапке BFF перечислен `channels`.

`rust/gui-spike/tests/channels_contract.rs` (новый, 6 тестов): оракул — сам READY,
не реализация. `tests/data_contract.rs` не менялся.

## 2. Red

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test channels_contract
exit=101
error[E0425]: cannot find function `channels` in module `data` (×18)
```

Лог: `red.log` (111 строк, compile-red до реализации).

## 3. Green

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
test_exit=0, gui-spike = 238 passed / 0 failed
```

Было 231 (baseline до работы), добавлено 7: `channels_contract` 6 + wiring 1.
Существующие 231 не менялись и не ослаблялись. Лог: `green.log`.

```
cargo test --offline --manifest-path rust/Cargo.toml -p discord-core
exit=0, 574 passed / 0 failed (без изменений)
```

Лог: `core.log`.

Targeted (в составе green):

```
tests/channels_contract.rs ... 6 passed; 0 failed
```

Проверки контракта в `channels_contract.rs`:

1. `channels_projection_matches_ready_oracle` — 272 гильдейских + 17 ЛС; counts
   по типам `{0:137, 2:82, 4:43, 5:1, 15:9}`; каждое поле (id/guildId/name/type/
   parentId/position) сверено с READY; у ЛС `guildId/name/position/parentId = null`;
   канал вне READY (`-2684694197495146496`, есть только в `MESSAGE_*`) отсутствует;
   JSON-ключи `guildId/parentId/type` (не `kind`), `type` — число.
2. `channels_are_sorted_by_guild_position_and_id` — порядок гильдий как в READY,
   `position` asc, `id` asc при равной позиции (в фикстуре 33 группы дублей
   позиций), ЛС последними; повторный вызов идентичен.
3. `channels_filter_by_guild_returns_only_that_guild` — первая гильдия = 55
   каналов (оракул READY), чужих нет; trim; неизвестная → `NotFound`; пустой →
   `InvalidArg`; 10k → `InvalidArg` с `…` (значение не копируется); 256 символов →
   `NotFound` (граница включительно).
4. `channels_without_ready_or_broken_ready_are_typed_errors` — нет файла →
   `NotFound`; битый JSON / нет `d` / `guilds` не массив / `private_channels` не
   массив → `Parse`.
5. `channels_reject_records_without_id_or_type` — канал без `id`, без `type`,
   `type` не число → `Parse` с путём READY (не тихий пропуск).
6. `channels_keep_absent_fields_as_null_and_order_private_last` — `name/parent_id =
   "nil"` → `null`, порядок «гильдия → ЛС» на минимальном READY.

## 4. Clippy / fmt

```
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
exit=0 (0 warnings)
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
exit=0
```

Логи: `clippy.log`, `fmt.log`. `cargo fmt` применялся к своим правкам; после
форматирования тесты и clippy прогнаны заново (числа выше — финальные).

## 5. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/data.rs` | `5d9119baca7b655d0aa5a4351cf25aff` |
| `rust/gui-spike/src/main.rs` | `cbb5409a279a84c90b2486343eb7c7c3` |
| `rust/gui-spike/src/lib.rs` | `f4c9c451968726fe37945bed9b630d9c` |
| `rust/gui-spike/tests/channels_contract.rs` (new) | `58d375894162069e34aa3c1680d33e65` |
| `rust/gui-spike/tests/data_contract.rs` (не менялся) | `ffa9e6cb2a0d9bf82f4f868cd2bee78f` |

`ui/**`, `Cargo.toml`, `package.json`, `logs/`, `functions/`, `evidence/`,
`waves/db.jsonl`, `TASKS.md` — не трогались.

## 6. Остаток / риски

- **G6-D-01 закрыт только backend-частью.** UI по-прежнему шлёт `channelId:"news"` /
  `messageId:String(index)` (`ui/features/chat.js`, `ui/app.js`) — это C-02 UI (G2):
  дерево `#channel-tree` → `invoke("channels", { guildId })` → заголовок +
  `messages_page(realId)`; бенч-путь `CHAT_ROWS=100_000` не ослаблять.
- **Интерпретация ЛС:** в выдачу `channels()` включены 17 ЛС с `guildId/name/
  position = null` (требование 2 «null для ЛС»); 272 — счётчик гильдейских каналов.
  Если PM хочет только гильдейские — это `channels(guildId)` или фильтр в UI;
  менять контракт не потребуется.
- Порядок гильдий — READY-порядок (рейл), а не лексикографический `guildId`;
  внутри гильдии — `position`, `id`. Зафиксировано тестом.
- `type` обязателен: запись без числового `type` — `Parse` (иначе UI не решит,
  открывать ли чат); в фикстуре такие записи отсутствуют.
- Мутации и приёмка — за Team Lead; `waves/db.jsonl`/`STATE.md` не обновлялись.
- Live/окно не запускались; wire-паритет READY — по версии 1.0.162 (см. G6 §3.4).
