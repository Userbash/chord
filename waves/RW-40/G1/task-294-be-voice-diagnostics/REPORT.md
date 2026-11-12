# REPORT — TASK-294 (`RE-22-02`): BFF-модуль `voice_diagnostics.rs`

- **Ветка:** `feature/task-294-be-voice-diagnostics` (от `9cce7f4`, Merge PR-173)
- **Карточка:** `waves/RW-40/PM/BACKLOG-RE-22.md` (`RE-22-02`), `TASK.md` этого batch
- **Модель:** DeepSeek (`--agent opencode --model deepseek/deepseek-v4.1-flash`), уровень `L2`, приоритет `P1`
- **Общий target:** `CARGO_TARGET_DIR=/home/user/orca/project/rust/target` (`cargo --offline`)

## 1. Файлы (write-scope, ровно 4 + `lib.rs`)

| Файл | md5 |
| --- | --- |
| `rust/gui-spike/src/voice_diagnostics.rs` (новый) | `112e389cb5ad70889492aed02ac7c958` |
| `rust/gui-spike/src/lib.rs` (`pub mod voice_diagnostics;`) | `4b608f6ae2ff8597f3dff76fa2bf5606` |
| `rust/gui-spike/tests/voice_diagnostics_contract.rs` (новый) | `095d00ecfeba6f0be16da66d6c48541f` |
| `scripts/test-impact.mjs` (правило `voice_diagnostics` → `voice_diagnostics_contract`) | `b47ac41293906826c0f05f12feaf2411` |
| `tests/unit/test-impact.test.mjs` (строка в `RUST_BFF_MODULES`) | `9b29aa722cc8cb912b22a3a24af96a30` |

`rust/gui-spike/Cargo.toml` и `Cargo.lock` **не изменены**. Других правок в дереве нет
(до работы уже были `D node_modules` и `M package-lock.json` — не мои).

## 2. Решение по зависимости (согласовано с координатором)

`discord-gui-spike` не зависит от крейта `discord-voice`, а `Cargo.toml` вне write-scope.
Координатор выбрал вариант **B**: модуль полностью самодостаточен — в нём объявлены зеркала
`ConnectionQuality`, `PingSample`/`PingHistory`, `short_hostname`, `RtcConnectionStatsSnapshot`
с теми же порогами (`250`/`500` мс, `5`/`10`%), буфером `199`/`20`, таймаутом `4000` мс,
что в `rust/crates/discord-voice/src/stats.rs`. Отличие от крейта: зеркала дополнительно
`Serialize`/`Deserialize` (нужны для JSON-журнала) и `quality` пишется строчными именами.
`Cargo.lock` остаётся без дрейфа.

## 3. Контракт (что именно реализовано)

- Константы `MAX_DEBUG_SERIES_POINTS = 600`, `MAX_MEDIA_ENGINE_SNAPSHOTS = 30`,
  `PRIVACY_CODE_GROUP_COUNT = 6`, `PRIVACY_CODE_GROUP_LEN = 5`, `PRIVACY_CODE_TOTAL_DIGITS = 30`
  плюс зеркальные `PING_FINE_MAX_MS`, `PING_AVERAGE_MAX_MS`, `LOSS_FINE_MAX_PCT`,
  `LOSS_AVERAGE_MAX_PCT`, `MAX_PING_HISTORY_LEN`, `PING_AVERAGE_WINDOW`, `DEFAULT_PING_TIMEOUT_MS`.
- `VoiceDebugModalSection` — 8 секций `zWA`, `SCREAMING_SNAKE_CASE` в serde и `as_str()`
  (`TRANSPORT`…`STREAMS_INBOUND`).
- `format_voice_privacy_code(&str) -> Result<[String; 6], String>`: только ASCII-цифры;
  пробелы и дефисы — допустимые разделители; любой другой символ (включая юникод-цифры) —
  `Err`; длина ровно 30 цифр; результат — 6 групп по 5.
- `build_popout_dto(...)` — `state`, `quality` (`ConnectionQuality::evaluate`), `short_hostname`,
  `average_ping_ms` + `average_ping_formatted` (`toFixed(0)`), `last_ping_ms`/`last_ping_formatted`,
  `outbound_loss_rate_pct`/`outbound_loss_formatted` (`toFixed(1)`), `pings` (спарклайн из буфера),
  `is_end_to_end_encrypted` (`secure_frames_protocol_version > 0`), `privacy_code_chunks`
  (пусто без DAVE, при `None`/`Err` тоже пусто — попап открывается всегда).
- `VoiceDebugHistoryStore` — `BTreeMap<String, VecDeque<TimeSeriesPoint>>` (600 точек на ключ,
  FIFO-вытеснение), `VecDeque<RtcConnectionStatsSnapshot>` (30 снимков), `series`/`snapshots`/`clear`,
  `export_sanitized_json()`.
- Безопасность журнала: экспорт идёт через типизированную структуру `SanitizedExport`
  (только `series` + `snapshots`), а ряды с маркерами `secret_key` / `session_token` / `token`
  в имени отбрасываются целиком. Секретов нет на уровне типов, а не строковыми заменами.

## 4. Red → Green (strict TDD)

- **Red** (`RED.log`): тест написан первым, прогон упал компиляцией —
  `error[E0432]: unresolved import discord_gui_spike::voice_diagnostics`
  (`could not find voice_diagnostics in discord_gui_spike`), exit `101`.
- **Green** (`GREEN.log`): `voice_diagnostics_contract` — `21 passed; 0 failed`;
  `ipc_contract` — `20 passed; 0 failed`.
- Промежуточные падения были только в моих ожиданиях теста (округление среднего `11.5 → 12`
  и `quality = bad` при потерях `12.34% > 10%`) — ожидания исправлены по оракулу, реализация
  под тест не подгонялась.

## 5. Гейты

| Команда | Результат |
| --- | --- |
| `cargo test --test voice_diagnostics_contract` | ok, 21/21 (`GREEN.log`) |
| `cargo test --test ipc_contract` | ok, 20/20 (`GREEN.log`) |
| `cargo clippy --test voice_diagnostics_contract -- -D warnings` | чисто, exit 0 (`CLIPPY.log`) |
| `node --test tests/unit/test-impact.test.mjs` | 58/58 pass, exit 0 (`NODE-TEST-IMPACT.log`) |
| остальные таргеты `gui-spike` (`typing`, `voice_devices`, `voice_stage`, `webhooks`) | ok, 13/15/8/8 |
| `rustfmt --check` по двум новым файлам | чисто |
| `npm run gate:worker` | **failed**, причины ниже — все три вне моих файлов (`GATE-WORKER.log`) |

### 5.1. Почему `gate:worker` красный (не мои файлы)

1. `spike-rust` → `ipc_wiring_contract::every_ui_invoke_command_in_js_sources_is_registered_in_tauri_main`:
   `left: 37, right: 38`. Проверено на чистом `HEAD` (`9cce7f4`) в отдельном `git worktree`:
   тот же ассерт падает с теми же числами. Тест читает `ui/**/*.js` и `main.rs`, которых я не касался.
2. `workspace-rust` → `discord-core/tests/dispatch_types.rs` не компилируется: методов
   `is_stream`, `parse_stream`, `parse_stream_with_diagnostics`, `to_stream_action`, `stream_kind`
   нет в `rust/crates/discord-core/src` на `HEAD` (`git grep` по `HEAD` — пусто). Правок в
   `discord-core` в ветке нет.
3. `smoke` → `tests/smoke/gui-spike.smoke.test.mjs`, `gui-window.smoke.test.mjs` требуют
   собранный бинарник `rust/gui-spike/target/debug/discord-gui-spike`; при общем
   `CARGO_TARGET_DIR` его там нет (окружение, не код).

`unit` (2006) и `meta` (102) в гейте зелёные; `spike-rust` оборвался на `ipc_wiring_contract`,
поэтому `voice_diagnostics_contract` внутри гейта не запускался — он прогнан отдельно (21/21).

## 6. Покрытые измерения (9 из 9)

Позитивные (DTO/сериализация, группы кода), негативные (нецифровой символ, отсутствие кода),
граничные (`250/251/500/501` мс, `5.0/5.0001/10.0/10.0001`%, 600-я и 601-я точка, 30-й и 31-й
снимок, 199-й и 200-й пинг), ошибки (неверная длина кода, `-0.1`/`100.1`/`NaN` потерь → `unknown`),
пустые (`PingHistory` пуст, пустой журнал экспортируется), права/безопасность (нет
`secret_key`/`token` в журнале), восстановление (`clear` + повторная запись, попап без кода),
совместимость (serde round-trip DTO, `SCREAMING_SNAKE_CASE` секций), наблюдаемость
(структура JSON-журнала: `series`, `snapshots`, `quality`).

## 7. Риски и что осталось

- Зеркала `discord_voice::stats` дублируют контракт крейта: при изменении порогов там их надо
  править и здесь. Долгосрочно — вынести общий тип или добавить path-dep `discord-voice`
  в `rust/gui-spike/Cargo.toml` (вне write-scope этой задачи, отложено координатором).
- `format!("{:.1}")` в Rust округляет половину к чётному (`0.25` → `"0.2"`), а JS `toFixed`
  — от нуля (`"0.3"`). Спека задачи фиксирует именно `format!("{:.1}")`; расхождение
  зафиксировано здесь, чтобы фронтенд не считал его регрессией.
- `rustfmt --check` по `rust/gui-spike/src/lib.rs` ругается на порядок `pub mod` — это
  состояние `HEAD` (проверено копией файла из `HEAD`), форматирование чужих строк не трогал.
- `Cargo.lock` в рабочем дереве обновляется сам при первом `cargo test` (в `HEAD`-локе нет
  `zstd`/`jobserver` из dev-зависимостей `discord-core`); после прогонов файл возвращён в
  состояние `HEAD`, дрейфа в PR нет.
- Не сделано намеренно: `git merge`/`push` в `unstable` — по ТЗ и глобальным правилам это
  делает координатор после приёмки.
