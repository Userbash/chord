# Отчёт RW-06 (P1-05) — RESUME/реконнект: op6, op7/op9, непрерывный `s`

## сделано

- `discord-core::resume` (новый модуль):
  - `ResumeParams {token, session_id, seq}` с **редактирующим Debug** (токен → `"REDACTED"`);
  - `build_resume_frame` — проводная форма `{op:6, d:{token, session_id, seq}}` (ключи-строки клиента,
    ведущий `0x83`), самопроверка разбором кодеком; `ResumeError::{EmptyToken, EmptySessionId, Encode}`;
  - `ResumeState {session_id, last_seq}`, `RESUME_MAX_DOWNTIME_MS = 30_000`;
  - `ReconnectAction::{Resume, Identify}` + `plan_reconnect(state, downtime_ms, invalid_session)`:
    Identify при отсутствии сессии, разрыве `>= 30 с` или `op9 d=false`; иначе Resume.
- `discord-core::ws`:
  - `connect_and_run_resume(url, pipe, opts, resume)` — после первого HELLO ровно один **op6**
    (IDENTIFY на этом соединении не шлётся); `connect_and_run_with` делегирует с `None`;
  - `GatewayPipe::resume_state()` — session_id из READY + последний `s`;
  - `GatewayPipe::last_invalid_session_resumable()` — `d` последнего op9 (атом true/false либо int 1/0);
  - `TransportError::Resume(ResumeError)`; отправка кадров вынесена в `maybe_send_handshake`
    (приоритет resume → identify → тишина).
- `lib.rs` — модуль и ре-экспорты (`connect_and_run_resume`).
- `tests/resume_reconnect.rs` — 12 кейсов (юнит/пайп/стаб/lab), T-GW-05…07.

Контракт `RunOptions` **не расширялся** — существующие литералы тестов RW-05 компилируются без правок
(отдельная функция `connect_and_run_resume`; отклонение от первой редакции ТЗ зафиксировано в §7 ТЗ).

## red

Red-first на финальной логике тестов (заглушки тел `build_resume_frame`, `plan_reconnect`,
`connect_and_run_resume`-resume-ветки, `resume_state`, `last_invalid_session_resumable`):

- `/tmp/opencode/rw-06/red.txt`: `1 passed; 11 failed; 0 ignored`, `# EXIT=101`,
  md5 `resume_reconnect.rs` `60bf79f41d62dd3dff4556c5e2002b5a`.
  Прошёл `resume_debug_redacts_token` — заглушки не ломали сам `impl Debug` (честно отмечено).
- Первый red (до правок харнесса по мере отладки): `0 passed; 12 failed`, `# EXIT=101`,
  md5 `4e55862b4f4b0aab2e82e4b7d8a4ea20` — история в `/tmp/opencode/rw-06/` (`red.txt` перезаписан
  финальной редакцией; промежуточные факты — в этом отчёте).

## green

- `cargo test --offline -p discord-core`: **58 passed** (2 lib + 15 gateway_session + 16
  identify_ready + 12 resume_reconnect + 13 ws_transport), 0 failed, `# EXIT=0` (`green-core.txt`).
- `resume_reconnect` ×5 подряд — 12/12 зелёные каждый раз (`green-repeat.txt`); тайминг-кейсы
  стабильны (0.30 с на прогон).
- `cargo test --offline --workspace`: **100 passed**, 0 failed, `# EXIT=0` (`green-workspace.txt`).
- `cargo clippy --offline --workspace --all-targets -- -D warnings` — чисто; `cargo fmt --check` —
  чисто (3 нита теста: explicit_counter_loop, 2× while_let_loop — закрыты без ослабления проверок).

## gate

`npm run gate:worker` из корня — **passed** (`.gate/worker-2026-10-19T14-33-22-949Z.json`):
typecheck / meta 22 / unit 169 / smoke 20 (`gate.txt`).

## файлы (md5)

| файл | md5 |
|---|---|
| `rust/crates/discord-core/src/resume.rs` | `211448f7051afcf5af78a7411b9c94dc` |
| `rust/crates/discord-core/src/ws.rs` | `b822d6a6790f11a306a442c6a58c975c` |
| `rust/crates/discord-core/src/lib.rs` | `69e299f265a0459f4e104c437e1bf2a5` |
| `rust/crates/discord-core/tests/resume_reconnect.rs` | `9852c2ca0a4960b9948d4b72aa3933b7` |

`src`-файлы при закрытии дыр ревью (ниже) не менялись — только тест-файл (13-й кейс R10 +
усиления R4/R11).

`Cargo.toml`/`Cargo.lock` не менялись (новых зависимостей нет). Живой Discord не трогался.

## Оракулы

- `rest-static.md` §4: RESUME `{token,session_id,seq}`, «INVALID_SESSION(false) → IDENTIFY».
- `gw-proto/msgs-run2-95974_8269.jsonl` — 569 op0, s 95..663 (RESUMED s=96);
  `…_8608.jsonl` — 59 op0, s 664..722 (RESUMED s=668); `…_10029.jsonl` — 331 op0, s 1..331, op7 1×;
  `…_13072.jsonl` — первый op0 RESUMED s=332, 40 op0, s 332..371; швы 663+1=664, 331+1=332.
  Числа перепроверены мини-скриптом до фиксации в тесте; lab-кейс skip-aware.
- Размер RESUME: ориентир 173/176 Б (маскировано); тест — 120..400 Б с токеном 59 Б/session 32 Б.

## Независимое ревью и закрытие дыр (2026-10-19)

Отчёт ревьюера-субагента: `/tmp/opencode/review-rw-0506.md` — вердикт **«принято с нитами»**.
Проверено лично ревьюером: md5 (7 файлов), workspace 100/0, clippy/fmt, red-first (лог +
собственная репродукция заглушками: 1 passed/11 failed — совпало), оракулы (4 JSONL, READY-числа,
бандл, `rest-static.md` §4), 21 мутация (18 поймано, 3 выжило).

Выжившие мутации — дыры покрытия, закрыты координатором red-first:

| # | Мутация ревью | Закрыто |
|---|---|---|
| R4 | порог `30_000` → `300_000` | `plan_reconnect_policy`: `assert_eq!(RESUME_MAX_DOWNTIME_MS, 30_000)` + границы литералами 29 999/30 000 |
| R10 | RESUME до HELLO (снят guard) | новый кейс 13 `ws_resume_not_before_hello` (тишина 300 мс до HELLO, затем op6) |
| R11 | перепутаны `EmptyToken`/`EmptySessionId` | `resume_frame_rejects_empty_fields`: сравнение конкретных вариантов, не `is_err()` |

Red при живых мутациях: `red-gap-tests.txt` — `# EXIT=101`, **10 passed / 3 failed**, падают ровно
целевые кейсы (`plan_reconnect_policy`, `resume_frame_rejects_empty_fields`,
`ws_resume_not_before_hello`), md5 теста red-времени `9852c2ca…` (копия для обратимости:
`/tmp/opencode/rw-06/resume_reconnect-red-gap.rs` — нит N4 закрыт); восстановление — md5
`resume.rs`/`ws.rs` совпали с бэкапом (`impl-backup2/`).

**Инцидент залипшего rlib (найден при закрытии R11):** после мутаций ревьюера `cp`-restore вернул
файлам старый mtime, cargo счёл rlib свежим и тесты линковались с *мутированной* сборкой
(`EmptySessionId` для пустого токена при верном исходнике). Лечение: `cargo clean` (полный) и
повтор всех прогонов; процедура восстановления после мутаций теперь включает `touch` файлов (в
`gap-red.sh`). Урок: финальные прогоны волны — только после чистой/разогретой валидной сборки.

Финальные прогоны на **чистой сборке** (`cargo clean` → пересборка): `discord-core` — **59 passed**
(lib 2 + gateway_session 15 + identify_ready 16 + resume_reconnect **13** + ws_transport 13),
воркспейс — **101 passed, 0 failed**, `# EXIT=0`; `resume_reconnect` ×5 — 13/13 стабильно;
clippy `-D warnings` / fmt — чисто; `gate:worker` — **passed**
(`.gate/worker-2026-10-19T15-05-50-384Z.json`, `gate.txt`). Логи: `green-core.txt`,
`green-repeat.txt`, `green-workspace.txt`.

## риски

- `RESUME_MAX_DOWNTIME_MS = 30 000` — контракт плана (T-GW-07), не наблюдение захвата: в lab
  разрывы ~2.9 с. Живой сервер может переопределить решение через op9; при появлении нового
  захвата порог пересмотреть.
- Клиент сам не переподключается (супервайзер — вызывающий; так же в RW-04). Автоматический
  цикл с backoff — отдельная задача (кандидат в P1-05-продолжение).
- `resume_state()` берёт session_id только из READY: сессия, начатая RESUMED без READY в этом
  процессе, не резюмируется повторно — ожидаемо (session_id удерживает супервайзер).
- Lab-кейс синтезирует ETF-термы op0 из `s`/`t` артефактов (рамки zstd не реплеятся): проверяется
  именно непрерывность `s` и решения resume, а не декодер (он покрыт RW-02/RW-04).
- Независимое ревью RW-05/RW-06 — после снятия рейт-лимита субагентов; приёмка — после него.
