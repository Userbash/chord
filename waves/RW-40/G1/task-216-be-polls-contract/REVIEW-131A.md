# Ревью 131A — task-216 (`BE-P1-18`, BFF-контракт опросов)

**Вердикт: APPROVED**

- Ветка / worktree: `feature/task-216-be-polls-contract`
- Ревьюер: Technical Review (DeepSeek, окно выходного дня)
- Дата: 2026-10-31

## 1. Проверка артефактов

| Файл | md5 (факт) | md5 в REPORT.md | Итог |
|---|---|---|---|
| `rust/gui-spike/src/polls.rs` | `e344e2dd08aeaecaa14469932d347fdc` | `e344e2dd08aeaecaa14469932d347fdc` | совпадает |
| `rust/gui-spike/src/lib.rs` | `8a152fae017aa25135d333ece8fab1dc` | `8a152fae017aa25135d333ece8fab1dc` | совпадает |
| `rust/gui-spike/tests/polls_contract.rs` | `6f64245e6b6622008c64461935526058` | `6f64245e6b6622008c64461935526058` | совпадает |

`polls.rs` после всех мутаций восстановлен побайтно (`diff` пуст, md5 равен исходному).

## 2. Targeted-тест (baseline, до мутаций)

```
CARGO_TARGET_DIR=/home/user/orca/project/rust/target \
  cargo test --manifest-path rust/gui-spike/Cargo.toml --test polls_contract
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 3. Мутационное тестирование — 5/5 убито

Каждая мутация вносилась в `rust/gui-spike/src/polls.rs` точечной заменой (ровно одно вхождение),
затем прогонялся только `--test polls_contract`, после чего файл восстанавливался из бэкапа.

| # | Мутация | Ожидание | Факт прогона | Убита |
|---|---|---|---|---|
| M1 | `MAX_POLL_ANSWERS: usize = 10` → `= 11` | падение константного теста и границы 10/11 | `FAILED. 7 passed; 2 failed` | да |
| M2 | `value.starts_with('0')` → `false` (разрешены ведущие нули) | падение Snowflake-валидации | `FAILED. 7 passed; 2 failed` | да |
| M3 | `!allow_multiselect && len() > 1` → `len() > 2` | падение запрета 2 ответов в single-select | `FAILED. 8 passed; 1 failed` | да |
| M4 | убрано `.round()` в расчёте процентов (усечение) | падение округления 33/67 и 13/38 | `FAILED. 7 passed; 2 failed` | да |
| M5 | `is_winner = is_expired && max_votes > 0 && votes == max_votes` → без `is_expired` | падение «нет победителей у активного опроса» | `FAILED. 8 passed; 1 failed` | да |

Итог: **5/5 (100%)**. Ни один тест не «зеленится» при ослаблении продуктового поведения.

## 4. Содержательная проверка

- Контракт сверен с RE-оракулом: маршрут `PUT /channels/{channel_id}/polls/{message_id}/answers/@me`
  и `.../answers/{answer_id}` (`rest-static.json:10525/10552`, `mod-652215.pretty.js:97`); лимиты
  `10` ответов, `300`/`55` символов совпадают с `ipc_ext.rs`.
- `compute_poll_results` повторяет `poll-progress.js`: `total = sum(votes)`, процент `0` при нуле
  голосов, `Math.round`-семантика (`f64::round` — half away from zero, как в JS для положительных),
  победители только при `is_expired && max_votes > 0 && votes == max_votes` (ничья даёт нескольких).
- Валидация Snowflake строгая: пустая строка, `0`, ведущие нули, не-цифры, переполнение `u64`,
  пробелы — отклоняются; `1` и `u64::MAX` принимаются. Пустой `answer_ids` (снятие голоса) допустим.
- Тесты покрывают 9 измерений: позитив, негатив, границы (`u64::MAX`, `10`/`11`, `u32::MAX`),
  пустые, ошибки, активный/завершённый опрос, ничья, serde round-trip, константы.
- Write-scope соблюдён: `src/polls.rs`, `src/lib.rs` (`pub mod polls;`), `tests/polls_contract.rs`.
  Чужие модули, общие файлы и доки не тронуты; `git commit` не выполнялся.

## 5. Замечания (не блокирующие)

- `PollResultEntry.percentage: u8` теоретически может дать `100` при единственном варианте — корректно.
  Переполнение невозможно, т.к. доля `<= 100`.
- Дрейф `rust/gui-spike/Cargo.lock` при прогоне `cargo` (пакет `zstd` через `discord-codec`) —
  после ревью откатан до исходного состояния, как и в отчёте воркера.
- `package-lock.json` изменён в дереве до старта задачи — не относится к PR.

**P0 = 0, P1 = 0.** Готово к merge в `unstable` после одобрения PM.
