# Техническое ревью 94A — PR-65 / task-180 (BE-P2-12)

- **PR:** PR-65 `feature/task-180-read-state-ack-bulk-flags -> unstable`
- **Карточка:** task-180 / BE-P2-12 — `POST /read-states/ack-bulk` и
  `AckMessageOptions` (`flags` / `manual` / `mention_count`).
- **Ревьюер:** Technical Reviewer (DeepSeek), Review 94A
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED — Ready for Merge** (`P0 = 0`, `P1 = 0`, мутации `5/5` убиты)

## 1. Write-scope

`git status` в worktree: изменены только

- `rust/gui-spike/src/messages.rs`
- `rust/gui-spike/tests/messages_contract.rs`

`package-lock.json` изменён до начала работы воркера (не относится к PR).
`rust/gui-spike/Cargo.lock` после прогонов откатан (`git checkout --`), в диффе отсутствует.
Общие файлы (`state/*`, `lib.rs`, `routes.rs`, доки, `waves/*`) не тронуты.

## 2. Целевые проверки

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test messages_contract --lib
```

- `lib`: `17 passed; 0 failed`
- `messages_contract`: `25 passed; 0 failed`
- `exit=0` (лог: `final-test.log`)

```
cargo clippy --manifest-path rust/gui-spike/Cargo.toml --test messages_contract --lib -- -D warnings
```

- `exit=0`, предупреждений `0` (лог: `final-clippy.log`).

## 3. Мутационное тестирование — 5/5 KILLED

Каждая мутация применялась к `rust/gui-spike/src/messages.rs`, прогонялся
`--test messages_contract`, затем файл восстанавливался из побайтового бэкапа с
контролем md5 до и после прогона (защита от гонки с параллельными воркерами).

| # | Мутация | Убийца (тест) | Итог |
|---|---|---|---|
| M1 | `MAX_ACK_BULK_ENTRIES` `100 -> 101` | `ack_bulk_constants_match_the_rest_static_oracle`, `ack_messages_bulk_rejects_empty_and_over_limit_before_transport` | KILLED (`rc=101`) |
| M2 | убрана проверка пустой пачки в `validate_bulk_entries` | `ack_messages_bulk_rejects_empty_and_over_limit_before_transport` | KILLED (`rc=101`) |
| M3 | убран `validate_ids` для элементов пачки | `ack_messages_bulk_rejects_any_invalid_entry_id_before_transport` | KILLED (`rc=101`) |
| M4 | `flags` не попадает в тело `ack_options_body` | `ack_message_with_options_sends_optionals_only_when_set_and_keeps_zero` | KILLED (`rc=101`) |
| M5 | `ACK_BULK_PATH` -> `/read-states/ack-bulk-mutated` | `ack_bulk_constants_match_the_rest_static_oracle`, `ack_messages_bulk_posts_oracle_body_to_bulk_path` | KILLED (`rc=101`) |

Логи: `mut-M1.log` … `mut-M5.log` в этом каталоге.

## 4. Восстановление артефактов

| Файл | md5 после ревью | md5 в REPORT.md |
|---|---|---|
| `rust/gui-spike/src/messages.rs` | `52c7f7d826859e6be924e3032912d3a3` | `52c7f7d826859e6be924e3032912d3a3` |
| `rust/gui-spike/tests/messages_contract.rs` | `8f44dd10bf03f139f55d968f3132c0e5` | `8f44dd10bf03f139f55d968f3132c0e5` |

Совпадение побайтовое (`diff` против бэкапа — пусто), дрейфа `Cargo.lock` нет.
`git commit` не выполнялся.

## 5. Проверка контракта по существу

- Тело `ack_message_with_options` содержит `token` и `last_viewed` всегда
  (`null` при `None`), `flags`/`manual`/`mention_count` — только при `Some(...)`;
  `mention_count: 0` сохраняется. Совпадает с оракулом
  `reference-oracle/rest-static.json` (`MESSAGE_ACK`, `bodyKeys`).
- `ack_messages_bulk` валидирует `1..=100` и оба id **до** транспорта, тело ровно
  `{"read_states":[...]}`, `read_state_type` опускается при `None`.
- `ack_message` / `ack_body` / `AckTransport` / `ReadStateAck` не менялись —
  обратная совместимость сохранена.
- Статусы `200/204 -> Ok`, `401/403/404/429/4xx/5xx -> типизированный MessageError`
  покрыты тестами.

## 6. Замечания (не блокирующие, P2)

1. `ACK_BULK_PATH` — константа, а не запись в `discord-core::rest::routes`.
   Причина: маршрута нет в каталоге, а его правка вне write-scope. Добавление
   маршрута — отдельная задача G1 core.
2. Живой клиент для mark-unread шлёт `{manual, mention_count}` без
   `token`/`last_viewed`; реализация по ТЗ всегда включает базовые ключи (`null`).
   Расхождение зафиксировано в REPORT.md, решение за PM.
3. `last_viewed: Option<u64>` не допускает отрицательных значений; поведение
   согласовано с типом счётчика.

## 7. Решение

`P0 = 0`, `P1 = 0`, мутации `5/5` убиты, тесты и clippy зелёные, write-scope соблюдён.
**APPROVED — Ready for Merge** в `unstable`.
