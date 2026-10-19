# Отчёт RW-03 (P1-03 / T-GW-01…03) — ядро сессии шлюза Discord на виртуальных часах

- **сделано**: в `discord-core` реализовано детерминированное ядро сессии шлюза (`gateway`):
  op10 HELLO (интервал из `d.heartbeat_interval`), первый удар — `jitter_seed % interval`,
  heartbeat по `on_timer` с `d = last_seq` (до первого диспатча `null`), учёт op11 ACK и RTT,
  ре-планирование от ACK (`ACK + interval`), детектор зомби (`now ≥ удар + interval` →
  `Reconnect{Zombie}`), `s`-последовательность из op0, op7/op9 → `Reconnect{ServerRequest/
  InvalidSession}`. Транспорта нет: часы виртуальные (`now_ms: u64` от вызывающего), `std::time`
  в модуле не используется. Паник на входных данных нет (арифметика saturating, нулевой/
  отрицательный интервал → `Malformed`, `unsafe` запрещён линтом крейта); ошибки не глотаются:
  `on_term` требует ровно один терм на кадр (хвост → `Malformed`), мусор → `Decode`.
  Контракт закреплён 12 новыми интеграционными тестами (`tests/gateway_session.rs`) на реальных
  оракулах. Проверено: `cargo test -p discord-core` (2 unit + 12 интеграционных), весь воркспейс,
  `clippy -D warnings`, `cargo fmt --check`, `npm run gate:worker` — всё зелёное.
  Переиспользована константа `HEARTBEAT_INTERVAL_MS = 41_250`; в `lib.rs` добавлены
  `pub mod gateway`, ре-экспорт кодека `pub use discord_codec as codec` и ключевые типы
  `gateway`, статус обновлён.

- **red**: `cd rust && cargo test --offline -p discord-core 2>&1 | tee /tmp/opencode/rw-03/red.txt`
  → **0 passed; 12 failed** (все 12 новых тестов; паника `not yet implemented` из заглушки
  `todo!()`, 11× `Session::new`, 1× `Op::from_code`), exit 101. Полный лог — `/tmp/opencode/rw-03/red.txt`.
  Провал зафиксирован до написания реализации. Первая (недействительная) фиксация от прерванной
  попытки содержала только ошибки компиляции теста — сохранена как `red-prev-attempt.txt` и
  перекрыта валидным red-прогоном.

- **green**: `cd rust && cargo test --offline -p discord-core`
  → unit 2 passed (в т.ч. `gateway::tests::op_codes_match_protocol_table`), интеграционные
  **12 passed**, 0 failed, exit 0 (`/tmp/opencode/rw-03/green-core.txt`);
  `cd rust && cargo test --offline` (весь воркспейс) → exit 0, 23 бинарника тестов, 56 тестов,
  0 failed/0 ignored (`/tmp/opencode/rw-03/green-workspace.txt`).
  `cargo clippy --offline -p discord-core --all-targets -- -D warnings` → exit 0, чисто
  (после `cargo clean -p discord-core` — полный re-check: clippy-driver на lib, lib-test и
  интеграционном тесте; `/tmp/opencode/rw-03/clippy-core.txt`).
  `cargo fmt -p discord-core` и `cargo fmt -p discord-core -- --check` → exit 0. После форматирования
  green-прогоны повторены.

- **gate:worker**: `npm run gate:worker` → **passed**; артефакт
  `.gate/worker-2026-10-19T12-12-01-547Z.json`: typecheck ok; meta 22/22; unit 169/169; smoke 20/20;
  warnings: [], skip: 0.

- **файлы** (только из ТЗ; `rust/` целиком ещё untracked — коммитов не делал):
  - `rust/crates/discord-core/src/gateway.rs` — новый модуль, реализация + доккомментарии
    по-русски (361 строка), md5 `f6fdb06b3b78bbf69634f150140782df`;
  - `rust/crates/discord-core/src/lib.rs` — точечная правка (44 строки), md5
    `21aea9a7eb18bc61fe26df4ec497458a`;
  - `rust/crates/discord-core/tests/gateway_session.rs` — новый тест-файл, 12 тестов (590 строк),
    md5 `ce29021a189fa5446fc612d03bcc82dc`.
  Вне этих файлов изменений нет (`find rust -newermt …` — только эти три; `git status` — без
  новых правок в вне-Rust-файлах; `Cargo.toml`/`Cargo.lock`, доки, golden не трогались).

- **оракулы (независимая проверка)**: md5 таймлайна `23f38750227ae79166748323199054d7` совпал с ТЗ;
  awk по `timeline-95974_1389.tsv`: событий 137, op10 = 1 (первое), op11 = 23, op0 = 113,
  `s` монотонен 1..113, first_ack_delta = 1214, last_ack_delta = 658 927, max ACK-дельта = 41 263,
  дельт в [41 050; 41 300] — 9; симуляция `/tmp/opencode/rw-03/sim.mjs` (семантика ТЗ):
  seed 1214 → 12 ударов, lastSeq 113, без зомби; все 41 250 сидов: зомби 0 раз, удары 11–12.
  HELLO-терм (138 Б) и пайплайн zstd (`zstd-frames-95974_1389.hexl`, md5
  `9199431aabbb31f0b85145373908826a`) — из RW-02, сверены тестом 10.

- **риски/вопросы**:
  1. Тест-файл унаследован оборванным черновиком прерванной попытки; до red исправлены 4 дефекта
     (отсутствующая строка `match`, `RangeInclusive::contains` по `&&u64`, парсинг `nil` как
     отсутствующего `s`, форматирование). Правки — только в тесте (контракт ТЗ), не «подгонка под код».
  2. Расхождение ТЗ и артефакта: в TSV значение `nil`, а не `-` («`-` = nil» из ТЗ); артефакт —
     источник истины (md5 сошёлся), парсер принимает оба токена (`parse_seq`), причина в комментарии.
  3. Сверх sketс-API ТЗ добавлены `Display` + `std::error::Error` для `GatewayError` (тесты
     форматируют `{e}`; неглотание ошибок) и счётчик `ignored_values()` (тест 8; ТЗ: «счётчик
     опционально»). Остальной API — ровно в границах ТЗ.
  4. Решения по неоднозначностям (задокументированы в модуле): повторный HELLO после первого
     удара игнорируется целиком; op0 без целого `s` → `Malformed`; `on_term` у мёртвой сессии
     всё равно декодирует кадр (`Decode`/`Malformed` не прячутся), `on_value` мёртвой сессии —
     `Ok` без эффектов; «лишние» ACK между ударами игнорируются; арифметика времени saturating.
  5. RW-04 (транспорт: tokio/tungstenite) — вне этой волны, нужны новые зависимости и отдельное
     разрешение владельца; в этой волне сеть не вызывалась.
  6. §8-строка не требуется (ТЗ; Rust-тесты вне meta-регистра); `TASKS.md`/доки не трогались.

---

## Ниты ревью: закрытие (после независимого ревью)

Ревью: «ПРИНЯТЬ С НИТАМИ», блокеров нет (`/tmp/opencode/rw-03/review.md`); закрыты ниты 1–3
(пробелы покрытия). Продовый код **не менялся** — только тесты.

- **сделано**: в `rust/crates/discord-core/tests/gateway_session.rs` добавлены 3 теста (только
  публичный API) и 2 хелпера (`hello_value`, `dispatch_value`):
  1. `zero_interval_hello_is_malformed` — интервал `0`/`-1` → `Malformed` (и через `on_value`, и
     через `on_term`), без паники (деление по модулю не выполняется); расписание не строится,
     `hello()` пуст, сессия жива, корректный HELLO после ошибки принимается;
  2. `second_hello_after_first_beat_is_ignored` — контроль: до первого удара повторный HELLO
     пере-планирует (интервал 8 000, дедлайн `seed % 8000`); после первого удара повторный HELLO
     игнорируется целиком — интервал 41 250, дедлайн и счётчик ударов не меняются, следующий ACK
     закрывает удар по исходному интервалу;
  3. `dead_session_is_silent_for_dispatches_and_acks` — после op7: op0 не двигает `last_seq`
     (Some(7) замер), op11 не меняет `rtt_ms` (Some(95) замер), повторный op7 не даёт эффектов,
     неизвестный опкод не считается в `ignored_values`, удары/дедлайны не появляются.
- **green**: `cd rust && cargo test --offline -p discord-core` → 2 unit + **15 интеграционных**
  passed, 0 failed, exit 0 (`/tmp/opencode/rw-03/green-nits.txt`); весь воркспейс
  `cargo test --offline` → **59 passed**, 0 failed, exit 0
  (`/tmp/opencode/rw-03/green-workspace-nits.txt`); `cargo clippy --offline -p discord-core
  --all-targets -- -D warnings` → exit 0 (`/tmp/opencode/rw-03/clippy-nits.txt`; по пути поймана
  и исправлена `clippy::identity_op` на литерале `7 % 41_250` — заменено на переменную сида);
  `cargo fmt -p discord-core -- --check` → exit 0.
- **мутации (в копии `/tmp/opencode/rw-03/nits-mut/`, репозиторий не тронут)**: каждая — правка
  `gateway.rs` в копии → `cargo test --offline -p discord-core --test gateway_session` →
  восстановление из `gateway.rs.orig` (md5 `f6fdb06b3b78bbf69634f150140782df`); скрипт
  `nits-mut/run-mutations.py`, логи `mut-1.txt`…`mut-3.txt`. Во всех трёх прогонах упал **ровно
  целевой** новый тест, остальные 14 прошли:

  | # | Мутация | Вердикт | Упал ровно | Деталь паники |
  |---|---------|---------|------------|----------------|
  | 1 | снят фильтр `interval > 0` в `on_hello` | ПОЙМАНА (exit 101) | `zero_interval_hello_is_malformed` | `gateway.rs:311:56` — «attempt to calculate the remainder with a divisor of zero» |
  | 2 | снят ранний `return` при `beats_emitted > 0` | ПОЙМАНА (exit 101) | `second_hello_after_first_beat_is_ignored` | тест: `left: Some(8000)` vs `right: Some(41250)` (интервал подменён) |
  | 3 | снят guard `if self.dead` в `on_value` | ПОЙМАНА (exit 101) | `dead_session_is_silent_for_dispatches_and_acks` | тест: `left: Some(99)` vs `right: Some(7)` (`last_seq` сдвинут после смерти) |

  Дыры (f)/(g)/(i) из мутационной таблицы ревью (§6) закрыты: мутации больше не выживают.
- **gate:worker**: `npm run gate:worker` → **passed**;
  `.gate/worker-2026-10-19T12-29-11-179Z.json`: typecheck ok; meta 22/22; unit 169/169; smoke 20/20;
  warnings `[]`, skip 0.
- **файлы**: изменён только
  `rust/crates/discord-core/tests/gateway_session.rs` — md5 `c974da4d127000179222dc1405357780`
  (783 строки, 15 тестов; было 590/12). `gateway.rs` (`f6fdb06b3b78bbf69634f150140782df`) и
  `lib.rs` (`21aea9a7eb18bc61fe26df4ec497458a`) не менялись; вне тест-файла изменений нет
  (`find rust -newermt 17:20` — только он). Коммитов нет.
- **риски**: нит 3 покрыт полностью, с одной честной оговоркой: сценарий «ACK у мёртвой сессии
  меняет RTT» мутационно неотличим от корректного на текущем продовом коде — `kill()` уже
  очищает `pending`, поэтому «лишний» ACK не сработал бы даже без guard; assertion `rtt_ms`
  остаётся контрактным замком, а сама мутация (i) ловится по op0/повторному op7/счётчику
  проигнорированных. Остальные риски отчёта выше не изменились.
