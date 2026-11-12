# ТЗ RW-06 (P1-05) — Resume/реконнект: op6 RESUME, op7/op9, непрерывный `s`

## Цель волны
Транспорт и данные для реконнекта шлюза **без потери сессии**: клиент умеет отправить RESUME (op6)
вместо IDENTIFY после обрыва, продолжает `s`-последовательность и по op9 понимает, разрешён ли
resume. Плюс политика плана: малый разрыв (3 с) → resume; длинный (30 с) → новый IDENTIFY (T-GW-07).

## Свои файлы (только эти)
- `rust/crates/discord-core/src/resume.rs` — НОВЫЙ: `ResumeParams`, `ResumeState`,
  `build_resume_frame`, `plan_reconnect`/`ReconnectAction`, `RESUME_MAX_DOWNTIME_MS`, `ResumeError`.
- `rust/crates/discord-core/src/ws.rs` — точечные правки: `RunOptions.resume`, отправка op6 после
  HELLO, `GatewayPipe::resume_state()`, `GatewayPipe::last_invalid_session_resumable()`.
- `rust/crates/discord-core/src/lib.rs` — `pub mod resume;` + ре-экспорты.
- `rust/crates/discord-core/tests/resume_reconnect.rs` — НОВЫЙ тест-файл (12 кейсов, red-first).
- `Cargo.toml`/`Cargo.lock` — НЕ трогать (новых зависимостей нет; `tokio::time::Instant` уже
  используется в `ws.rs` — виртуальное время тестов через `tokio::time::pause()` работает).
- Существующие тесты (`gateway_session.rs` 15, `ws_transport.rs` 13, `identify_ready.rs` 16)
  обязаны остаться зелёными **без правок**.

## Оракулы (только чтение)
- `reference-oracle/rest-static.md` §4: RESUME `{token, session_id, seq}`; «при INVALID_SESSION
  (false) → IDENTIFY»; бандл веб-клиента — `_doResume`/`GatewaySocket._connect`.
- `reference-oracle/gw-proto/msgs-run2-95974_8269.jsonl` — resume #1: op10 HELLO, 569 op0,
  s 95..663, RESUMED при s=96.
- `…/msgs-run2-95974_8608.jsonl` — resume #2: 59 op0, s 664..722, RESUMED при s=668
  (шов: 663 + 1 = 664).
- `…/msgs-run2-95974_10029.jsonl` — s 1..331 (op7 1×); `…/msgs-run2-95974_13072.jsonl` — resume #3:
  первый op0 = RESUMED s=332, s 332..371 (шов: 331 + 1 = 332).
- Исходящие длины из захвата: RESUME — 173/176 Б (маскировано; размер — ориентир).
- Клиентская задержка между сокетами в захвате ≈ 2.9 с (переподключение без op7).

## Контракт
1. `ResumeParams { token: String, session_id: String, seq: u64 }`:
   - `Debug` печатает `token: "REDACTED"`, `session_id` целиком, `seq`;
   - `Clone`; поля публичные.
2. `ResumeError` (без паник): `EmptyToken`, `EmptySessionId`, `Encode(EtfError)` (+ `Display`/`Error`).
3. `build_resume_frame(&ResumeParams) -> Result<Vec<u8>, ResumeError>` — провод:
   `0x83` + `{op: 6, d: {token, session_id, seq}}` (ключи-строки wetf-формы, как в IDENTIFY);
   self-check разбором кодеком; пустые token/session_id — ошибки. Размер с реальным токеном 59 Б и
   session_id 32 Б — ориентир 120..400 Б (реальные кадры 173/176 Б).
4. `ResumeState { session_id: String, last_seq: u64 }`.
5. `RESUME_MAX_DOWNTIME_MS: u64 = 30_000` — контракт плана (T-GW-07: 3 с → resume, 30 с → IDENTIFY);
   порог «< 30 000 мс». Сервер может переопределить решение через op9.
6. `ReconnectAction { Resume, Identify }`;
   `plan_reconnect(state: Option<&ResumeState>, downtime_ms: u64, invalid_session: Option<bool>)
   -> ReconnectAction`:
   - `Identify`, если `state = None`, либо `downtime_ms >= RESUME_MAX_DOWNTIME_MS`, либо
     `invalid_session == Some(false)`;
   - иначе `Resume` (в т.ч. `invalid_session == Some(true)` — можно пробовать снова, и `None`).
7. `ws`: `RunOptions` **не расширяется** (литералы `RunOptions { identify: … }` в существующих тестах
   должны компилироваться без правок). Вместо этого новая функция
   `connect_and_run_resume(url, pipe, opts: RunOptions, resume: Option<ResumeParams>)`:
   - `resume = Some(p)` → после первого HELLO отправляется **op6 один раз**; IDENTIFY на этом
     соединении НЕ шлётся (opts.identify игнорируется);
   - `resume = None` → в точности поведение RW-05 (`connect_and_run_with` теперь делегирует:
     `connect_and_run_resume(url, pipe, opts, None)`).
   Ошибки сборки op6 → `TransportError::Resume(ResumeError)`.
8. `GatewayPipe::resume_state() -> Option<ResumeState>` (PartialEq для тестов):
   `Some` когда есть `last_ready.session_id` и `session.last_seq()`, иначе `None`.
9. `GatewayPipe::last_invalid_session_resumable() -> Option<bool>`: значение `d` последнего op9
   (`Atom("true"/"false")` либо int 1/0); не было op9 — `None`. `d` отсутствует/иной — `None`.
10. Клиент сам НЕ переподключается (супервайзер — вызывающий): исход `RunOutcome::Reconnect{reason}`
    как в RW-04; тесты играют роль супервайзера, используя `plan_reconnect` + новый пайп/сессию.

## Тест-кейсы (`tests/resume_reconnect.rs`, red-first, все в одном файле)
| № | Имя | Что проверяет |
|---|---|---|
| 1 | `resume_frame_has_evidence_fields` | op6, `d{token,session_id,seq}`, строки/числа, ведущий 0x83 |
| 2 | `resume_frame_size_plausible` | размер с токеном 59 Б и session_id 32 Б ∈ 120..400 (ориентир 173/176) |
| 3 | `resume_debug_redacts_token` | Debug не содержит токен, содержит REDACTED |
| 4 | `resume_frame_rejects_empty_fields` | пустые token/session_id → ошибки без паник |
| 5 | `plan_reconnect_policy` | (None,0,None)→Identify; (Some,3000,None)→Resume; (Some,30000,None)→Identify; (Some,3000,Some(false))→Identify; (Some,3000,Some(true))→Resume |
| 6 | `pipe_resume_state_after_ready` | синтетический READY(session_id,s=1) → Some(…,1); без READY/без session_id → None |
| 7 | `pipe_invalid_session_flag` | op9 d=false → Some(false); d=true → Some(true); нет op9 → None; d числом 0 → Some(false) |
| 8 | `ws_resume_sent_once_after_hello` | стаб: HELLO → ровно один op6 с полями, не op2; RESUMED s=2; `last_seq`=2; Closed |
| 9 | `ws_resume_suppresses_identify` | стаб: при `resume+identify` шлётся только op6; оп2 — ноль |
| 10 | `ws_op7_then_resume_keeps_seq` (T-GW-06) | конн1: READY s=1, dispatch s=2, op7 → Reconnect{ServerRequest}; конн2: RESUME seq=2 → RESUMED s=3, dispatch s=4; `last_seq`=4; ни одно событие не потеряно |
| 11 | `ws_invalid_session_false` | стаб: RESUME → op9 d=false → исход Reconnect{InvalidSession}, флаг Some(false) → `plan_reconnect` даёт Identify |
| 12 | `lab_resume_seq_continuity` (T-GW-05, skip-aware) | .8269 max=663, .8608 min=664, 59 op0 до 722 (RESUMED при 668); .10029 max=331, .13072 min=332, 40 op0 до 371, первый op0 RESUMED; реплей s-диапазонов в `Session` — без разрывов |

Стаб-кейсы: `127.0.0.1:0`, всё под `tokio::time::timeout(5 с)`; кейс 10 — два последовательных
accept в одной задаче стаба; кейс с задержкой (если появится) — `tokio::time::pause()`.

## Red-first
1. Сначала тест-файл + заглушки (`resume.rs` с `todo!()`, минимальные правки `ws.rs`/`lib.rs` для
   компиляции) → прогон `cargo test --offline -p discord-core` → зафиксировать падения: лог
   `/tmp/opencode/rw-06/red.txt` с `# EXIT=` и md5 тест-файла (политика волн).
2. Реализация → green: `-p discord-core`, затем воркспейс, `clippy -D warnings`, `fmt --check`.
3. `npm run gate:worker` из корня → отчёт §4.2 в `/tmp/opencode/rw-06/report.md`.

## Запреты
- Живой Discord/сеть за пределами `127.0.0.1` — нельзя; golden (`logs/`, `functions/`,
  `evidence/`, `reference-oracle/`) — только чтение; коммитов не делать.
- Не «зеленить» тесты правкой тестов под код: оракул — lab-файлы и `rest-static.md` §4.
- Токены/сессии в логи не писать (Debug-редакция — часть контракта).
