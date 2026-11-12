# Отчёт воркера RW-02 (от 2026-10-19) — zstd-stream декодер (прототип), P0-03 / T-ENC-04

Сохранено координатором из финального ответа субагента-воркера (sessionID ses_eac1b4212ffeb1qWcppx2Yodts).

## Отчёт по PROCESS.md §4.2

**сделано**
- `tests/data/synth-zstd-frame.hexl` скопирован из `reference-oracle/gw-proto/`, md5 совпал с эталоном ТЗ (`0a7742ae…`).
- `tests/zstd_stream.rs` — 9 кейсов из ТЗ, только публичный API (`ZstdStream`, `ZstdError`, `etf`), оракулы независимые: hex-константы ТЗ (HELLO, терм payload ×4096), lab-файлы `{RE_LAB_ROOT:-reference-oracle/}/gw-proto/*.hexl` (skip-aware: нет файла → `eprintln!("skip: …")` + return; прочие ошибки чтения не глотаются).
- `src/zstd.rs` — реализация: один `FrameDecoder` на поток, `pending`, флаг магии, переиспользуемый буфер выдачи 64 КиБ; доккомментарии по-русски (семантика Err/терминальности, удержание окна 64 КиБ: 824 816 − 65 536 = 759 280, переподключение = новый `ZstdStream`).
- `src/lib.rs` — точечно: `pub mod zstd;`, `pub use zstd::{ZstdError, ZstdStream};`, строка статуса (строки 9–10) обновлена на «прототип (RW-02/P0-03; T-ENC-04)».
- Факты ТЗ перепроверены независимо (Node): 137 строк/104 066 Б сжатых, 44 терма/смещение 758 576/остаток 704, 8158 = 74 Б без магии; payload md5 `4dbd601d…` совпал; скан порчи индекса 12 подтверждён (`MalformedSectionHeader { expected_len: 7456, remaining_bytes: 40 }`).

**red** — `cd rust && cargo test --offline -p discord-codec` (заглушка + `lib.rs`):
`Running tests/zstd_stream.rs … test result: FAILED. 0 passed; 9 failed` (все 9 новых падают); прежние 24 (2 unit + 9 etf_properties + 13 etf_vectors) — ok. Лог: `/tmp/opencode/rw-02/red.txt`.

**green**
- `cargo test --offline -p discord-codec` → 33 passed / 0 failed (2 + 9 + 13 + 9 новых).
- `cargo test --offline` (весь воркспейс) → все сьюты ok, падений нет.
- `cargo clippy --offline -p discord-codec --all-targets -- -D warnings` → exit 0, чисто.
- `cargo fmt -p discord-codec` → затем `--check` → exit 0 (другие крейты не форматировались; mtime `etf.rs`/прочих не менялся).
Логи: `green.txt`, `green-crate.txt`, `clippy.txt`, `fmt.txt` в `/tmp/opencode/rw-02/`.

**gate:worker** — `npm run gate:worker` → **passed** (typecheck ✔; meta 22/22; unit 169/169; smoke 20/20; skip 0). Артефакт: `/home/user/orca/project/.gate/worker-2026-10-19T11-27-32-889Z.json`. Лог: `gate.txt`.

**файлы (только из ТЗ; Cargo.toml/Cargo.lock не трогались)**

| файл | md5 |
| --- | --- |
| `rust/crates/discord-codec/src/zstd.rs` | `0b4b9abb8c61f4e941ce842d5c3792ed` |
| `rust/crates/discord-codec/src/lib.rs` | `95e1267438115081fb4d19134946819d` |
| `rust/crates/discord-codec/tests/zstd_stream.rs` | `4dd800a4c5592481201fb8162d579de8` |
| `rust/crates/discord-codec/tests/data/synth-zstd-frame.hexl` | `0a7742ae83a58088f3f80a16be77ad83` |

**риски/расхождения (все — с фиксацией, оракулы не менялись)**
1. **Цикл `push`**: литеральное «`pending` пуст → стоп» с буфером 64 КиБ (минимум ТЗ) теряет payload > буфера — проверено отдельным пробником `/tmp/opencode/rw-02/probe64`: synth даёт 65 536 вместо 139 264 Б. Реализован стоп **по нулевому прогрессу**; архитектура ТЗ не изменена.
2. **`TrailingData` только после инициализации декодера** (`bytes_read_from_source() > 0`): у свежего `FrameDecoder` без состояния `is_finished()` == `true`, иначе куски < 18 Б получали ложный `TrailingData`. Публичный `is_finished()` остался прямым делегированием (контракт ТЗ).
3. `ZstdError` получил `#[derive(Debug, Clone, PartialEq, Eq)]` (в ТЗ derives не заданы).
4. Тест 2: «куски 1–2 не начинаются с магии» — как 2-й и 3-й куски (1-й — начало кадра).
5. md5 payload внутри Rust-теста не вычисляется (md5-крейта нет): оракул — byte-exact `терм × 4096`, md5 зафиксирован в шапке модуля.
6. `push` после `Err` не имеет явного «мёртвого» флага (поток терминальный по договорённости); death-state — кандидат P1-02.
7. lab-файлы на месте → skip-ов в прогонах нет; для `…_8158.hexl` сверил размер (74 Б) и отсутствие магии (md5 `c0ebadee3979230a5576254182c5d0da`).
