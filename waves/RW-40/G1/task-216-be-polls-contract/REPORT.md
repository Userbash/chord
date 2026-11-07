# Отчёт воркера — task-216: BFF-контракт голосования и результатов опросов (`polls`)

## 1. Batch / карточка

- **Волна**: `RW-40`, отдел `G1` (Backend).
- **Карточка**: `task-216` (`BE-P1-18` / `polls`) — контракт голосования в опросах и подсчёта результатов (`PollAnswerOption`, `PollVoteRequest`, `PollResultEntry`, `validate_poll_vote_request`, `build_poll_answer_put_route`, `compute_poll_results`).
- **Ветка / worktree**: `feature/task-216-be-polls-contract`, `CARGO_TARGET_DIR=/home/sanya/orca/project/rust/target`.
- **Write-Scope (единственные изменённые файлы)**:
  - `rust/gui-spike/src/polls.rs`
  - `rust/gui-spike/src/lib.rs`
  - `rust/gui-spike/tests/polls_contract.rs`
  - `/home/sanya/orca/project/waves/RW-40/G1/task-216-be-polls-contract/REPORT.md` (и `red.log`, `green.log`)

## 2. Оракул (read-only)

Сверено с `/home/sanya/re-deep/rest-static.json`, `/home/sanya/re-deep/pretty/mod-652215.pretty.js` и модулями `ipc_ext.rs`, `poll-progress.js`:

- `POLL_ANSWERS` (`rest-static.json:10525`): `PUT /channels/{channel_id}/polls/{message_id}/answers/@me`, тело `{"answer_ids": [...]}` (`<= 10` ответов).
- `POLL_ANSWER_VOTERS` (`rest-static.json:10552`, `mod-652215.pretty.js:97`): `/channels/{channel_id}/polls/{message_id}/answers/{answer_id}`.
- `poll-progress.js:23-68`: расчёт `totalVotes`, округление процентов `Math.round((votes * 100) / totalVotes)` (`0` при `totalVotes == 0`) и определение лидеров/победителей при `maxVotes > 0 && votes == maxVotes`.

## 3. Что сделано

1. **`rust/gui-spike/tests/polls_contract.rs` (Red-first, 9 тестов)**:
   - Проверка констант `MAX_POLL_ANSWERS = 10`, `MAX_POLL_QUESTION_CHARS = 300`, `MAX_POLL_ANSWER_CHARS = 55`.
   - Проверка serde round-trip для `PollAnswerOption`, `PollVoteRequest`, `PollResultEntry`.
   - Позитивные, граничные и негативные сценарии `validate_poll_vote_request`:
     - `channel_id` и `message_id`: `"1"` и `u64::MAX` (`Ok`), `""`, `"0"`, `"000"`, ведущие нули `"0123456789"`, не-цифры, `u64::MAX + 1` (`Err`);
     - `answer_ids`: пустой вектор (снятие голоса — `Ok`), `1` ответ при `allow_multiselect = false` (`Ok`), `>= 2` ответов при `allow_multiselect = false` (`Err`), `1..=10` ответов при `allow_multiselect = true` (`Ok`), `11` ответов (`Err`);
     - отклонение `answer_id == 0` и дублирующихся `answer_id` (`Err`).
   - Проверка `build_poll_answer_put_route(channel_id, message_id, answer_id)` — возврат маршрута `"/channels/{channel_id}/polls/{message_id}/answers/{answer_id}"` и отклонение невалидных Snowflake ID или `answer_id == 0`.
   - Проверка `compute_poll_results(options, is_expired)` — пустой срез и нулевые голоса (`total_votes = 0`, `percentage = 0`, `is_winner = false`), округление дробных процентов (`33%` / `67%`, `13%` / `38%`), выставление `is_winner = is_expired && max_votes > 0 && votes == max_votes` (включая ничью нескольких лидеров при `is_expired = true`).
2. **`rust/gui-spike/src/polls.rs`**:
   - Реализованы константы `MAX_POLL_ANSWERS`, `MAX_POLL_QUESTION_CHARS`, `MAX_POLL_ANSWER_CHARS`.
   - Реализованы структуры `PollAnswerOption`, `PollVoteRequest`, `PollResultEntry`.
   - Реализованы функции `validate_poll_vote_request`, `build_poll_answer_put_route`, `compute_poll_results`.
3. **`rust/gui-spike/src/lib.rs`**:
   - Экспортирован модуль `pub mod polls;`.

## 4. RED-прогон

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-216-be-polls-contract/red.log`:

```text
error[E0432]: unresolved import `discord_gui_spike::polls`
  --> tests/polls_contract.rs:21:24
   |
21 | use discord_gui_spike::polls::{
   |                        ^^^^^ could not find `polls` in `discord_gui_spike`

error: could not compile `discord-gui-spike` (test "polls_contract") due to 1 previous error
EXIT_CODE=101
```

## 5. GREEN-прогон и проверки (Targeted + Clippy + Rustfmt)

Сохранено в `/home/sanya/orca/project/waves/RW-40/G1/task-216-be-polls-contract/green.log`:

```text
running 9 tests
test compute_poll_results_calculates_rounded_percentages_and_winners_when_expired ... ok
test build_poll_answer_put_route_returns_canonical_path_and_validates_inputs ... ok
test compute_poll_results_handles_empty_and_zero_vote_polls ... ok
test poll_constants_match_discord_contract ... ok
test compute_poll_results_marks_all_tied_leaders_as_winners_when_expired ... ok
test poll_structs_serde_roundtrip ... ok
test validate_poll_vote_request_checks_channel_and_message_snowflake_bounds ... ok
test validate_poll_vote_request_rejects_zero_and_duplicate_answer_ids ... ok
test validate_poll_vote_request_enforces_multiselect_and_max_answers_bounds ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
EXIT_CODE=0
```

Дополнительно проверено:
- `cargo clippy --manifest-path rust/gui-spike/Cargo.toml -p discord-gui-spike --test polls_contract --lib -- -D warnings` — `0 warnings`, `exit=0`.
- `rustfmt --edition 2021 --check rust/gui-spike/src/polls.rs rust/gui-spike/tests/polls_contract.rs` — `exit=0`.
- `rustfmt --edition 2021 --check --config skip_children=true rust/gui-spike/src/lib.rs` — `exit=0`.

## 6. Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/polls.rs` | `e344e2dd08aeaecaa14469932d347fdc` |
| `rust/gui-spike/src/lib.rs` | `8a152fae017aa25135d333ece8fab1dc` |
| `rust/gui-spike/tests/polls_contract.rs` | `6f64245e6b6622008c64461935526058` |
| `waves/RW-40/G1/task-216-be-polls-contract/red.log` | `f1302318ea83469140c58fde535f5bff` |
| `waves/RW-40/G1/task-216-be-polls-contract/green.log` | `dd23f61b535de232c5dff7a27be8fa85` |

## 7. Риски и замечания

- **Дрейф `rust/gui-spike/Cargo.lock`**: при запуске `cargo` перегенерировался `Cargo.lock` из-за `discord-codec` (`zstd`); откатан через `git checkout -- rust/gui-spike/Cargo.lock`, чтобы строго соблюсти write-scope.
- `package-lock.json` был модифицирован в рабочем дереве до старта задачи — не трогался.
- `git commit` не выполнялся.
