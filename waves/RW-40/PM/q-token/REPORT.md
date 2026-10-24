# Q-TOKEN-01 — отчёт воркера G1 Backend

Карточки: **G1-02** (6 копий чтения токена) + **Q-01** (`join_guild`/`enter_guild`).
Дата: 2026-10-24. Роль: G1 Backend (воркер), PM — DeepSeek.
Live/сеть/окно/commit: **не запускались**. `ui/**`, `logs/`, `functions/`, `evidence/` — не менялись.
G1-01 (error mapping 403/404) не трогался: `messages.rs`/`reactions.rs`/`search.rs`/`guilds.rs` без изменений.

## 1. Сделано

### G1-02 — один helper чтения токена

`rust/gui-spike/src/main.rs`: добавлен единственный generic helper

```rust
async fn token_or<E, F>(load: F, unavailable: fn() -> E, missing: fn() -> E) -> Result<String, E>
where
    F: FnOnce() -> Result<Option<String>, AuthError> + Send + 'static,
    E: Send + 'static,
```

- `spawn_blocking` (`blocking`) и оба отказа (join-error blocking-вызова и `AuthError::Keyring`)
  сведены в одну точку: раньше это были **6 копий** по 5 строк (`blocking + 2×map_err + ok_or`),
  теперь **1** (внутри helper'а: 2×`map_err(|_| unavailable())` + `ok_or_else(missing)`).
- Коды ошибок **не менялись** (контракт фронтенда): keyring/blocking-сбой → `Network` у всех
  доменов; «токена нет» → `Unauthorized` у `messages`/`reactions`, `MissingToken` у
  `search`/`guilds`. Публичные enum'ы не правились.
- Заменены все 7 потребителей: `search_remote`, `send_message`, `edit_message`,
  `delete_message`, `reply_message`, `toggle_reaction`, `join_guild`. Бывший `guild_token`
  удалён (его роль выполняет `token_or`).
- Токен и текст keyring-ошибки не логируются; в `Debug`/`Display` ошибок не попадают
  (проверяется тестами `every_network_command_reads_token_through_one_helper` и
  `token_or_preserves_domain_error_codes`).
- Почему коды передаются на месте вызова, а не берутся из trait: контрактные тесты
  `messages_contract.rs:249`, `search_contract.rs:208,212` (чужие файлы, не менялись) пинят
  буквальный `backend.token()` и `SearchError::MissingToken` в теле команды — то есть код
  домена обязан быть виден на границе команды. Это не дубль политики: сама политика
  (blocking + два маппинга) живёт в helper'е.

### Q-01 — одна Tauri-команда

- `enter_guild` удалён из `invoke_handler!` **и** как Tauri-команда в `main.rs`.
- Остался один command `join_guild` (UI-глагол), он вызывает `guilds::join_guild`.
- `guilds.rs` **не менялся**: модульные `guilds::enter_guild`/`guilds::join_guild` остались
  алиасами одной операции (закреплено тестом эквивалентности).
- `ui/**` не вызывает ни один из глаголов (read-only проверка `rg`: 0 совпадений) — второй
  IPC-имени никто не использовал.

## 2. Red-first

Тесты обновлены/добавлены до кода, прогон зафиксирован.

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml --test guilds_contract
EXIT=101   (лог: waves/RW-40/PM/q-token/red-guilds.log)

failures:
  tauri_registers_exactly_one_guild_command
    "алиас enter_guild не должен попадать в invoke_handler!"
  tauri_commands_take_no_token_and_read_backend
    "все команды должны читать токен через общий helper token_or"
11 passed; 2 failed
```

Изменения тестов (это фиксация нового контракта, не ослабление):
- `tauri_registers_join_and_enter_guild` → **`tauri_registers_exactly_one_guild_command`**:
  `join_guild` в `invoke_handler!` ровно один раз, `enter_guild` отсутствует, и в бинаре нет
  `async fn enter_guild(`. Старый тест пинил сам дубль (G1-09), поэтому замена — цель карточки.
- `tauri_commands_take_no_token_and_read_backend`: вместо `guild_token` проверяет общий
  `token_or` (объявлен, вызывается из `join_guild`), отсутствие `token: String` и логирования.
- **добавлен** `module_enter_and_join_are_equivalent`: `enter_guild` и `join_guild` дают равный
  `Result` и одинаковый target/тело на статусах 204/200/401/403/404/429/500 и на невалидных id
  (характеризация алиаса).
- **добавлены** unit-тесты в `main.rs`:
  `every_network_command_reads_token_through_one_helper` (helper объявлен один раз; у всех 7
  команд ровно один `backend.token()` через `token_or::<...>`, нет локального `map_err`, нет логов)
  и `token_or_preserves_domain_error_codes` (keyring → `Network`, нет токена → `Unauthorized`
  для messages/reactions и `MissingToken` для search/guild; успех возвращает токен).
- `ipc_contract.rs` менять не потребовалось (md5 не изменился).

Прозрачность по green-итерациям: две проверки в собственных новых тестах были уточнены при
первом прогоне (форма вызова `token_or::<` вместо `token_or(`; самосовпадение `include_str!`
при подсчёте объявления helper'а — теперь считаются строки). Ни один assert не удалён и не
ослаблен, проверяемое поведение то же.

## 3. Green

```
cargo test --offline --manifest-path rust/gui-spike/Cargo.toml          EXIT=0  164 passed / 0 failed
cargo clippy --offline --manifest-path rust/gui-spike/Cargo.toml \
       --all-targets -- -D warnings                                    EXIT=0
cargo fmt --manifest-path rust/gui-spike/Cargo.toml -- --check         EXIT=0
```

Логи: `green-test.log` (164 = 161 baseline + 3 новых; 14 test-бинарей, все ok),
`clippy.log`, `fmt.log`. Тесты, которые G1-01 будет менять
(`messages_contract` 39/ok… — в логе все suites зелёные), не правились.
`guilds_contract`: 13/13; `src/main.rs` unit: 5/5.

## 4. Файлы и md5 (после работы)

| Файл | md5 | Δ LOC |
|---|---|---:|
| `rust/gui-spike/src/main.rs` | `8b6eef551467500ad35466fd611a9cea` | 707 → 851 (prod 565 → 577, tests 142 → 274) |
| `rust/gui-spike/src/guilds.rs` | `db461cfbe3f3e1055995a88239b941bc` (не менялся) | 227 |
| `rust/gui-spike/tests/guilds_contract.rs` | `73adf0388eea5d6a86ea94f6dcf193c9` | 335 → 381 (+46, тесты) |
| `rust/gui-spike/tests/ipc_contract.rs` | `6e2fe83e366aa7ec3bfafd114f381085` (не менялся) | 274 |

`ui/**` — без изменений (`git status --short ui` пуст).

## 5. Что убрано / LOC

- Копий чтения токена: **6 → 1** (плюс удалён `guild_token`).
  `map_err(|_| DomainError::Network)` в токен-пути: **12 → 1** (`|_| unavailable()`);
  `ok_or(..)`: **6 → 1** (`ok_or_else(missing)`).
- Tauri-команд guild: **2 → 1** (`enter_guild` −1 command, −1 строка в `invoke_handler!`).
- Production `main.rs`: **565 → 577 (+12)**. Из этого G1-02 ≈ +24 (helper с доком + 6 мест
  вызова, где явно видны коды домена) и Q-01 ≈ −12/−13 (`enter_guild`, `guild_token`,
  регистрация). Честно: выигрыш карточки — **не в LOC**, а в числе копий политики (6 → 1):
  правка политики ошибок токена теперь одна, а не семь. Дальнейшее сокращение (+24 → 0)
  возможно только trait-вариантом с однострочными вызовами, но он требует правки
  `messages_contract.rs`/`search_contract.rs` (чужие файлы, красная зона G1-01) — вынесено
  PM как вопрос, если LOC важнее явности кодов на границе.

## 6. Риски / открытые вопросы

1. **Сигнатура helper'а** (`fn() -> E` для двух кодов) — сознательный компромисс под
   source-scrape тесты соседних доменов. Если PM разрешит править
   `messages_contract.rs`/`search_contract.rs`, можно перейти на trait `TokenFailure`
   (однострочные вызовы, −~20 LOC) без изменения кодов.
2. `EnterGuildError`/остальные enum'ы не расширялись: причина keyring по-прежнему уходит как
   `Network` (это и есть требование карточки — «код не менять»). Диагностика Secret Service в
   UI остаётся неотличима от сети — это долг G1-02 из аудита (аддитивные `Storage`) при
   отдельном решении PM (см. G1-AUDIT §9.1).
3. Тесты `guilds_contract` теперь пинят ровно одно IPC-имя: если UI когда-нибудь позовёт
   `enter_guild`, ошибка будет на фронтенде (`command not found`), не в бэкенде — намеренно
   (Q-01: один command; JS-алиас при необходимости делается в UI).

## 7. Evidence

```
waves/RW-40/PM/q-token/red-guilds.log    EXIT=101, 2 failed (контракт до кода)
waves/RW-40/PM/q-token/green-test.log    EXIT=0, 164 passed
waves/RW-40/PM/q-token/clippy.log        EXIT=0, -D warnings
waves/RW-40/PM/q-token/fmt.log           EXIT=0
waves/RW-40/PM/q-token/REPORT.md         этот отчёт
```
