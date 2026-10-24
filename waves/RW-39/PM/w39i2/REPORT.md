# Отчёт W39i2 (Backend, микро-задача) — wiring `search_page` в Tauri

Дата: 2026-10-23. PM: DeepSeek. Роль: воркер (микро-задача к RW-39/W39i, D-15).

## 1. Сделано

| # | Пункт ТЗ | Статус |
|---|---|---|
| 1 | `#[tauri::command] fn search_page(query, filters, cursor, limit)`, корень `data::fixtures_root()?` | готово (см. §4 — фильтры через DTO-зеркало, обоснование) |
| 2 | Регистрация `search_page` в `invoke_handler!` | готово, `main.rs:172` |
| 3 | `data.rs` не тронут, новых зависимостей нет, `Cargo.toml` не тронут | готово (`data.rs` md5 = W39i-сдаточному) |

Файлы (только мандат): `rust/gui-spike/src/main.rs`; `lib.rs` **не** менялся (re-export не
потребовался — команда берёт типы из `discord_gui_spike::data`); логи — `waves/RW-39/PM/w39i2/`.

## 2. TDD: red → green

Red-1 (тест регистрации, `w39i2-red1.log`) — `search_page` отсутствует в хендлере:

```
test wiring_tests::invoke_handler_registers_all_bff_commands ... FAILED
panicked at src/main.rs:169:13: search_page не зарегистрирован в invoke_handler!
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured
```

Red-2 (контракт фильтров и сквозной вызов, `w39i2-red2.log`) — нет ни обёртки, ни DTO:

```
error[E0425]: cannot find type `SearchFiltersArg` in this scope
error[E0422]: cannot find struct, variant or union type `SearchFilters` in this scope
```

Green (`w39i2-green.log`):

```
Running unittests src/main.rs        test result: ok. 3 passed; 0 failed; 0 ignored
Running tests/data_contract.rs       test result: ok. 29 passed; 0 failed; 0 ignored
Running tests/ipc_contract.rs        test result: ok. 20 passed; 0 failed; 0 ignored
Running tests/launch.rs              test result: ok. 1 passed; 0 failed; 0 ignored
lib.rs unittests                     test result: ok. 0 passed; 0 failed; 0 ignored
```

Новые 3 теста (unit в `main.rs`, без окна):
- `invoke_handler_registers_all_bff_commands` — в теле `generate_handler![…]` есть `ready`,
  `messages_page`, `members_page`, `settings`, `search_page`; сравнение по точному токену, не
  подстроке. Оракул — сам исходник бинаря: Tauri не отдаёт список команд в рантайме, а окно
  для проверки регистрации не требуется.
- `filters_payload_is_camel_case_and_maps_to_data_type` — объект фильтров из IPC
  (`authorId/channelId/content/mentionId/day/messageType/pinned`) → `data::SearchFilters`
  один-в-один; `{}` → `SearchFilters::default()`.
- `search_page_command_passes_query_filters_and_cursor` — команда = слой данных при том же
  корне: `query` с пробелами обрезается данными (`total == 1` на `call of duty`), фильтр
  `pinned:false` доезжает (`limit 5` → 13 строк, 3 страницы), `next_cursor` второй страницы
  даёт тот же результат, что прямой вызов; страницы не пересекаются. Skip-aware: нет
  фикстур → skip с причиной, не падение.

## 3. Проверки (один cargo, без окна/probe/gate/сети)

```
$ cargo build --offline --manifest-path rust/gui-spike/Cargo.toml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s      (exit=0)

$ cargo test --offline --manifest-path rust/gui-spike/Cargo.toml
    3 + 29 + 20 + 1 passed; 0 failed (exit=0)

$ cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.22s      (exit=0)

$ cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check
    exit=0   (первый прогон дал diff в тестовом модуле — `w39i2-fmt-before-fix.log`,
              затем `cargo fmt`; `data.rs`/`tests/**` не затронуты)

$ rg -n "search_page" rust/gui-spike/src/main.rs
    130:fn search_page(
    138:    data::search_page(&root, &query, &filters, cursor.as_deref(), limit)
    172:            search_page
```

## 4. Отклонение от буквы ТЗ (обязательное) — DTO фильтров

ТЗ просило сигнатуру `filters: Option<SearchFilters>`. С такой сигнатурой крейт **не
собирается**: `data::SearchFilters` не реализует `Deserialize`, а аргумент `#[tauri::command]`
обязан десериализоваться. Доказательство — `w39i2-deserialize-constraint.log`:

```
error[E0277]: the trait bound `SearchFilters: serde::Deserialize<'de>` is not satisfied
   = note: for local types consider adding `#[derive(serde::Deserialize)]` to your `SearchFilters` type
```

`data.rs` — READ-ONLY по ТЗ, orphan rule запрещает `impl Deserialize for SearchFilters` в
`main.rs`, поэтому принято минимальное решение: приватное зеркало `SearchFiltersArg`
(`#[serde(rename_all = "camelCase", default)]`) + `From<SearchFiltersArg> for SearchFilters`
(перекладывание полей без логики). Имена — camelCase, как остальные аргументы Tauri v2 в
проекте (`channelId`, `guildId`). Вся валидация/обрезка/семантика осталась в `data.rs`.

Если PM предпочтёт каноническую сигнатуру — нужен один derive в `data.rs`
(`#[derive(..., Deserialize)]`) и удаление зеркала; это правка чужого файла, без разрешения
не делалась.

## 5. Файлы + md5

| файл | md5 | изменение |
|---|---|---|
| `rust/gui-spike/src/main.rs` | `6fafee88639db3c9d2da5709f083c7ce` | +DTO, +команда, +регистрация, +3 unit-теста |
| `rust/gui-spike/src/data.rs` | `bcd0bc7baa0c5a973a3d81808db13219` | **не тронут** (совпадает с W39i) |
| `rust/gui-spike/src/lib.rs` | `3de13bca66001aae70b0a924bcd16fea` | не тронут |
| `rust/gui-spike/tests/data_contract.rs` | `ffa9e6cb2a0d9bf82f4f868cd2bee78f` | не тронут (W39i-сдаточный) |
| `rust/gui-spike/tests/ipc_contract.rs` | `6e2fe83e366aa7ec3bfafd114f381085` | не тронут |
| `rust/gui-spike/tests/launch.rs` | `9cad6d87bfcfa4936367d24b91c36c94` | не тронут |

Логи: `w39i2-red1.log`, `w39i2-red2.log`, `w39i2-green.log`, `w39i2-build.log`,
`w39i2-clippy.log`, `w39i2-fmt.log`, `w39i2-fmt-before-fix.log`,
`w39i2-deserialize-constraint.log`, `w39i2-md5.log`, `rg-search_page.log`.

## 6. Риски / остаток

- Фронтенд получает фильтры в camelCase (`authorId`, `messageType`, `mentionId`); контракт
  зафиксирован тестом. Если будущий UI-воркер ждёт snake_case — правка одной строки
  `rename_all` в DTO, но это уже смена IPC-контракта, нужен явный ТЗ.
- Неизвестные ключи в объекте фильтров молча игнорируются (как у Tauri-аргументов в целом);
  строгая схема (`deny_unknown_fields`) не включалась, чтобы не ломать фронтенд.
- Регистрация проверена по исходнику, а не живым окном: окно/probe запрещены ТЗ. Закрытие
  E8c по-прежнему за PM после приёмки этого отчёта.
