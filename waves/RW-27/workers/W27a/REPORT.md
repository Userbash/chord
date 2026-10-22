# Отчёт W27a — P7-01b ListFormat (batch RW-27 / B09)

- **batch/карточка:** RW-27 / P7-01b (CLIENT-1, W27a).
- **сделано:**
  - `src/list_data.rs` скопирован байт-в-байт из `waves/RW-27/generated/list_data.rs`
    (`cmp` + md5 совпадают), вручную не редактировался.
  - Реализован `src/list.rs`: `ListStyle`, `ListType`, `format_list` по замороженному
    контракту (enabled/canonical локаль → паттерн, иначе en-US с тем же style/type,
    иначе `", "`; 0 → `""`, 1 → как есть, 2 → `pair`, 3+ → `start`/`middle`/`end`).
    Продуктовый код без `unwrap`/`expect`/`panic!`/`unsafe`.
  - В `src/lib.rs` добавлены ровно `pub mod list;` и приватный `mod list_data;`.
  - `tests/list_contract.rs`: 10 поведенческих тестов, включая все 1674 кейса
    оракула (парсинг фикстуры существующей зависимостью `discord-json`; новых
    зависимостей и правок `Cargo.toml` нет).
- **red:** `cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w27a cargo test --offline -p discord-i18n --test list_contract`
  → `error[E0432]: unresolved import discord_i18n::list` (compile-red, до реализации),
  **exit 101**. Лог: `waves/RW-27/workers/W27a/red.log`.
- **green:** та же команда после реализации → `10 passed; 0 failed`, **exit 0**.
  Лог: `waves/RW-27/workers/W27a/green.log`.
  Первый green-прогон содержал ошибку в тест-ожидании (`RU-ru`); тест исправлен
  без правок продукта, см. `TEST-CORRECTIONS.md`, лог `green-attempt1.log`.
- **targeted:** `list_contract` — 10 тестов, 1674/1674 кейсов оракула сверены
  (`unknown_and_disabled_locales_fall_back_to_en_us` дополнительно проверяет
  fallback для неизвестной/выключенной локали).
- **мутации:** 6/6 неэквивалентных пойманы, продукт восстановлен по md5
  (`38a3d1d5435ef29a9d061295ab74acb3`); 2 эквивалентных мутанта (`middle→start`,
  снятие en-US fallback) задокументированы (`mutations.md`, повторный прогон TL-2
  `waves/RW-27/mutations/w27a-final.log`).
- **дополнительно:** `cargo clippy --offline -p discord-i18n --lib --test list_contract -- -D warnings`
  → exit 0 (`clippy.log`).
- **gate:worker:** не запускался — batch-лейн TL-2 (по ТЗ). Полный workspace/gate,
  коммиты, доки и golden не трогались.
- **файлы + md5:**
  - `rust/crates/discord-i18n/src/list.rs` (new) — `38a3d1d5435ef29a9d061295ab74acb3`
  - `rust/crates/discord-i18n/src/list_data.rs` (new) — `4aad25e6b01d78afb13544132ceaf77f`
    (== `waves/RW-27/generated/list_data.rs`)
  - `rust/crates/discord-i18n/src/lib.rs` (2 строки) — `bf4ee71796fcf6bedc2e20ee27a4b786`
  - `rust/crates/discord-i18n/tests/list_contract.rs` (new) — `7389541dbefca9f4a2137d7aec8c19b4`
  - артефакты: `red.log`, `green.log`, `green-attempt1.log`, `clippy.log`,
    `fmt-check.log`, `mutations.md`, `TEST-CORRECTIONS.md`.
- **самоаудит кода:** P0=0, P1=0, P2=0 в изменённых файлах; замечания ниже —
  batch-level данные/гейт, не дефекты реализации.
- **риски/вопросы:**
  1. **Закрыто (TL-2):** `pt-BR` отсутствовал в первой редакции таблицы; данные и оракул
     перегенерированы — 31 включённая локаль, 279 строк, 1674 кейса. Fallback для
     `pt-BR` больше не применяется, строка есть (`list_data.rs`).
  2. **Закрыто (TL-2):** конфликт с rustfmt снят — генератор выдаёт данные в
     rustfmt-совместимой форме, `cargo fmt --all -- --check` чист.
  3. **P3:** у всех 279 строк `middle == start`, поэтому мутация `middle→start`
     эквивалентна; слот `middle` реализован по контракту, но на данных ненаблюдаем.
     Внутренний цикл проверяется мутацией M3b (`middle → pair`).
  4. **P2:** `canonical_locale` не разворачивает региональные алиасы (`RU-ru` → en-US).
     Это соответствует замороженному контракту; если нужен языковой fallback —
     требуется отдельное изменение контракта (`resolve_locale`).
  5. ТЗ говорит, что у `discord-i18n` нет `discord-json`, но зависимость уже есть
     (`src/lib.rs` её использует). Тест применяет её для разбора фикстуры; новых
     зависимостей нет. Если это нежелательно, можно заменить на ручной парсер —
     скажите.
- **следующий шаг:** передать TL-1/TL-2 на приёмку и batch gate; решение по
  `pt-BR` и rustfmt-конфликту `list_data.rs` — за TL. После приёмки —
  `waves/db.jsonl` + `STATE.md` (делает TL).
