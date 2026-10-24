# FIX-L02-CORE — устранение findings REVIEW-L02-CORE

Дата: 2026-10-24. Воркер: **G1 Core writer** (OpenCode DeepSeek V4.1 Flash). PM: DeepSeek.
Карточка: `TASK FIX-L02-CORE` (P1A). Сеть/live/окно/UI/чужие тесты не трогались; коммитов нет.

Проверено перед работой: `HEAD=0239543`; md5 `ws.rs`/`supervisor.rs`/`tests/gateway_supervisor.rs`/
`net.rs`/`net_contract.rs` совпали с REPORT §4/§6.4, кроме уже известного расхождения по
`tests/gateway_supervisor.rs` (`b9d806c6…` в §4 против фактического `84f5fcc1…`).

## 1. Что исправлено

### P1 — потеря уже разобранных op0 при ошибке в том же WS-фрагменте

`rust/crates/discord-core/src/ws.rs`, ветка `Message::Binary`:

```rust
let parsed = pipe.push_binary(&data, elapsed_ms(start));
flush_dispatches(pipe, &opts);
let effects = parsed?;
```

Дренаж теперь выполняется **до** распространения `Err(Zstd|Frame)`. Раньше `?` выходил из
цикла до `flush_dispatches`, супервайзер закрывал попытку, и op0, уже лежавший в
`GatewayPipe::events`, не доходил до `on_dispatch`. Поведение при `on_dispatch: None` не
меняется (`flush_dispatches` — no-op, буфер остаётся вызывающему). Порядок «flush до
`apply_effects`» сохранён, доставка по кадру не отложена.

Граница честно: `TransportError::Zstd` возникает в `ZstdStream::push` **до** цикла разбора
(`zstd.rs:148-157`: при ошибке декодера выдача `out` отбрасывается), поэтому в одном
фрагменте «сначала разобранный op0, потом Zstd-ошибка» невозможен по построению. Реальный
случай из ревью — `Frame` после валидного терма; он и покрыт тестом.

### P1 — md5 в REPORT.md

`REPORT.md` §4: слепок `rust/crates/discord-core/tests/gateway_supervisor.rs` исправлен с
`b9d806c6d243c625059eb1e924078d7e` на фактический `84f5fcc1404795faec939a24c15462d1`
(совпадает с §6.4 и с артефактом, который проверял ревьюер — до правок этой карточки).
Добавлена оговорка, что таблицы §4/§6.4 — состояние приёмки L-02, а актуальные после фикса
слепки — здесь, в §4 этого отчёта.

### P2 — oracle redaction: точный `token`

`rust/gui-spike/tests/net_contract.rs`:

- `secret_paths` (независимый оракул) теперь перечисляет точные ключи контракта
  `CONTRACTS/dispatch.md` §5: `session_id`, `resume_gateway_url`, `auth_session_id_hash`,
  `client_secret`, `password`, **`token`**, `access_token`, `refresh_token`, плюс суффикс
  `_token`; раньше точный `token` не считался секретом, и тест не мог поймать его утечку;
- новый тест `exact_token_and_nested_auth_tokens_are_redacted_and_counted`: `d` с вложенным
  `user.token` и верхнеуровневыми `token`/`access_token`/`refresh_token`; ожидание выписано
  руками (4 вырезанных ключа, `user.id`/`status` сохранены), значения токенов не попадают в
  сериализованный payload.

Реализация `net.rs::is_secret_key` уже соответствовала контракту — правок в ней нет
(md5 не изменился); закрыт именно пробел доказательства.

## 2. RED

| Прогон | Команда | Результат | Лог |
|---|---|---|---|
| P1: валидный op0 + повреждённый хвост | `cargo test -p discord-core --test gateway_supervisor dispatch_handler_gets_op0_parsed_before_frame_error_in_same_fragment` | **FAILED** `left: []`, `right: ["READY"]` — хук не увидел ни одного события | `fix-red-core-flush.log` |
| P2: oracle против мутанта `is_secret_key` без точного `token` | `cargo test --test net_contract exact_token_and_nested_auth_tokens_are_redacted_and_counted` | **FAILED** `left: 2`, `right: 4` (вложенный `token` остался) | `fix-red-oracle-token-mutant.log` |

Мутант P2 откачен сразу после прогона: `net.rs` md5 вернулся к `e13d4e8cb4fbb22af2ce6d96702f0e8a`
(исходный), маркеров `MUTANT` в коде нет. Red P1 снят **до** правки `ws.rs` (на исходном
`176791cc…`).

Оракул нового core-теста — форма кадра стаба: первый терм фрагмента — валидный READY,
затем `0x83 0xff` (неизвестный тег ETF → `TransportError::Frame`), т.е. ошибка возникает
после того, как пайп уже положил READY в `events`. Стаб соединение не закрывает, поэтому
успешный `Closed`-исход исключён: попытка обязана упасть на разборе.

## 3. GREEN

| Проверка | Результат | Лог |
|---|---|---|
| `cargo test -p discord-core --test gateway_supervisor` | **14 passed, 0 failed** (было 13: +1 новый) | `fix-green-core-targeted.log` |
| `cargo test -p discord-core` | **577 passed, 0 failed** (было 576: +1) | `fix-green-core.log` |
| `cargo test --test net_contract` (gui-spike) | **23 passed, 0 failed** (было 22: +1 новый oracle-тест) | `fix-green-net-contract.log` |
| `cargo fmt --all -- --check` | exit 0, диффа нет | `fix-fmt.log` |
| `cargo clippy -p discord-core --all-targets -- -D warnings` | exit 0, чисто | `fix-clippy-core.log` |
| `cargo clippy` (gui-spike, `--all-targets -D warnings`) | exit 0, чисто | `fix-clippy-gui.log` |

Полный `gate:worker`/`test --workspace` не запускался — по ТЗ карточки не требуется.
UI/`ui/**`, чужие тесты, live/сеть, `git commit` не затрагивались.

## 4. Файлы + md5 (после правок)

| Файл | md5 | Изменение |
|---|---|---|
| `rust/crates/discord-core/src/ws.rs` | `8703c1d695cd0af26458393eb3c1e3f9` | P1: flush до `?` + доккомментарий |
| `rust/crates/discord-core/src/gateway/supervisor.rs` | `5180b63795b2eee3d017cb30324760e6` | не менялся |
| `rust/crates/discord-core/tests/gateway_supervisor.rs` | `7f23d85a4d3b0e54c58d765b8b879c58` | + `frame_bytes` в стабе, + P1-тест |
| `rust/gui-spike/src/net.rs` | `e13d4e8cb4fbb22af2ce6d96702f0e8a` | не менялся (мутант откачен) |
| `rust/gui-spike/tests/net_contract.rs` | `fe8aa92146e9dcf8b943335dfad652c8` | P2: oracle `token` + новый тест |
| `waves/RW-40/PM/l02-core/REPORT.md` | — | P1: исправлен слепок §4 + оговорка |

## 5. Остаток / риски

1. **Zstd-случай в одном фрагменте** не тестируется отдельно: по коду он невозможен
   (ошибка декодера возникает до разбора, выдача отбрасывается) — см. §1. Покрытие P1 —
   `Frame` после валидного op0, ровно сценарий ревью.
2. **Буфер при `on_dispatch: None`** по-прежнему растёт до конца соединения — осознанное
   поведение (`REPORT.md` §5 п.5), карточкой не менялось.
3. **UI (G2)** и live-проверка доставки до окна — вне среза, не делались.
4. `CONTRACTS/dispatch.md` не правился: §2/§6 описывают «flush после `push_binary`», что
   осталось верным; уточнение «до возврата ошибки» отражено здесь и в доккомментарии
   `flush_dispatches`. Если PM хочет видеть это в контракте — отдельная правка владельца
   контракта.
5. Запись `old → new → основание` в `DECISIONS.md` (REPORT §6.5 п.2) — по-прежнему зона PM.
