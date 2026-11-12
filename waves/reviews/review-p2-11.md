# Независимое ревью RW-20 / P2-11 — memory-only typing TTL

Дата: **2026-10-21**. Ревьюер: **REV-P2-11**, независимый (не автор карточки).
Режим: **read-only** по продукту/тестам/докам; запись только в
`waves/reviews/review-p2-11.md` и `waves/reviews/p2-11/` (логи прогонов). Cargo — только
последовательно под `flock /tmp/opencode/discord-cargo.lock`; полный workspace повторно не
перезапускался. Контракт: `waves/RW-20/DESIGN-P2-11.md`, `TZ-P2-11.md`,
`TEST-MATRIX-P2-11.md`, `TEST-CORRECTIONS.md`, research `RESEARCH-P2-11.md`.

## Вердикт — **принято**

**P0 = 0, P1 = 0, P2 = 0, P3 = 10.**

Блокирующих замечаний нет. Код соответствует DESIGN §3–§7, corrections C1–C3 являются
арифметическими уточнениями контракта, а не подгонкой под реализацию; targeted 15/15,
core 503, workspace 545, fmt/clippy 0 и 8/8 заявленных typing-мутаций (T1–T8)
подтверждаются логами и выборочными независимыми прогонами. P3 уходят в backlog и не
тормозят `accepted`.

## 1. Проверки

| # | Проверка | Команда / лог | Результат |
|---|---|---|---|
| 1 | md5 продуктовых файлов и тестов | `md5sum rust/crates/discord-core/src/state/typing/*.rs tests/typing_*.rs tests/data/typing-run2.jsonl` | совпадает с `w20-report.md` §5 и `INTEGRATION.md` §6 |
| 2 | md5 shared-файлов | `md5sum src/state/{mod,types,reducer}.rs src/lib.rs tests/state_{reducer,types}.rs` | совпадает с `INTEGRATION.md` §6 |
| 3 | fixture-копия | `md5sum waves/RW-20/tests-data/typing-run2.jsonl tests/data/typing-run2.jsonl` | `71526755…6ea55` в обоих; 28 строк |
| 4 | независимый оракул fixture | Python-подсчёт строк/пар/guild/timestamp | 28 строк; 27+1 пара; одна гильдия; ts 1 792 392 992…1 792 394 710; строки отсортированы |
| 5 | targeted повторно | `flock … cargo test --offline -p discord-core --test typing_types --test typing_reducer --test typing_selectors` → `waves/reviews/p2-11/targeted-rerun.log` | **exit 0**, 5+5+5 = 15/15 |
| 6 | fmt | `flock … cargo fmt --all -- --check` → `waves/reviews/p2-11/fmt-check.log` | **exit 0**, пустой вывод |
| 7 | clippy | `flock … cargo clippy --offline -p discord-core --all-targets -- -D warnings` → `waves/reviews/p2-11/clippy-core.log` | **exit 0** |
| 8 | targeted batch | `waves/B01/logs/targeted-final.log` | 6 test-binaries: typing 5+5+5, voice 5+4+6 = **30/30**, 0 FAILED |
| 9 | core | `waves/B01/logs/core-503.log` | сумма `test result: ok` = **503**, 0 FAILED; typing 3×5 присутствуют |
| 10 | workspace | `waves/B01/logs/workspace-545.log` | сумма = **545**, 0 FAILED; typing 3×5 присутствуют |
| 11 | gate | `waves/B01/gate-worker.json`, `logs/gate-worker.log` | `verdict: passed`; meta 25/25, unit 169/169, smoke 20/20. Это npm-гейт; Rust-фазы — отдельные логи (см. P3-8) |
| 12 | typing-мутации | `waves/B01/mutations/T1…T8.log`, `summary.txt`, `/tmp/opencode/b01-mutation-backup` | все 8 — падение targeted (exit 101); текущие md5 5 файлов совпадают с backup |
| 13 | TTL/часы | `grep` по `state/typing` | `TYPING_TTL_MS = 10_000`; `std::time`/`SystemTime`/`Instant` нет; expiry только из `received_at_ms` |
| 14 | паники/секреты | `grep` по typing-файлам | нет `unwrap/expect/panic!/todo!/unimplemented!`, нет сети/SQLite/UI/keyring/новых зависимостей |
| 15 | corrections C1–C3 | арифметика fixture + `TEST-CORRECTIONS.md` | C1: A истекает `1 792 393 002 000` < первого B `1 792 393 942 000`; C2: selector `None` на границе, `TypingEntry::remaining_ms` = 0; C3: store 2, active 1 — соответствует DESIGN §7 |
| 16 | тестовая матрица | `tests/typing_{types,reducer,selectors}.rs` | t01–t15 присутствуют, по 5 в каждом файле; t08/t11/t15 проверяют именно corrections |

### Что проверено по коду

- `TYPING_TTL_MS = 10_000`, `TypingKey = (u64,u64)`, `TypingStart`/`TypingEntry` — DESIGN §3;
- `TypingEntry::is_active` — `now_ms < expires_at_ms`, `remaining_ms` — `saturating_sub`;
- parser: `d` map/`Missing`/`InvalidId`, optional `guild_id`/`timestamp`, `member.user`,
  mismatch → `typing:<channel>:<user>:member-user:<id>`, style → `ignored`, clock не читается
  из провода;
- reducer: no-clock → `ignored "typing.received_at_ms"` без записи, upsert member только при
  совпадении id, repeat last-wins без `duplicates`, `remove_for_message`/`remove_entry`,
  `prune_expired` (`now >= expires`), saturating TTL;
- `MESSAGE_CREATE`: `channel_id`/`author_id` берутся до move, удаление после вставки;
- `READY` — пустая карта, `RESUMED` — без изменений; `State::Debug` — только длина `typing`;
- `Event::from_routed`/`from_frame` → `received_at_ms=None`; `from_frame_at` → `Some(now)`;
  `apply_frame_at` — stale-проверка **до** `Event::from_frame_at`; `apply_dispatch`/
  `apply_frame` typing не применяют;
- 9 селекторов: active-фильтр, сортировки `user_id`/`(channel,user)`, точный guild, counts
  только активных, `typing_users` только известные `State.users`, `None` для просроченных;
- root re-exports `TYPING_TTL_MS`, `TypingStart`, `TypingEntry`, `TypingKey`,
  `apply_frame_at` присутствуют.

## 2. Findings P0/P1/P2

**Не найдено.** Поведение соответствует замороженному контракту; расхождений, ломающих
приёмку, нет. Замечания ниже — только P3 (тестовые/процессные пробелы и мелкая точность
документации).

## 3. P3 backlog (не блокирует `accepted`)

### P3-1 — stale-before-parse в `apply_frame_at` не зафиксирован тестом

- **Где:** `rust/crates/discord-core/src/state/reducer.rs:156-164` (stale-проверка до
  `Event::from_frame_at`); тест `tests/typing_reducer.rs:231-241` (t10) использует stale-кадр
  с **валидным** `d`.
- **Почему это пробел:** DESIGN §6 требует «stale before parse». Текущий t10 не различает
  «stale до parse» и «parse, затем stale»: при parse-first валидный stale-кадр всё равно не
  продлит TTL и попадёт в `stale_sequences` через `apply`. Если кто-то переставит parse
  раньше stale, stale-битый кадр начнёт добавлять `malformed` и шуметь в диагностике.
- **Evidence:** код корректен по инспекции; тест, который бы упал на parse-first, отсутствует.
- **Закрытие red-first:** в t10 (или t10b) выставить `last_seq=Some(10)`, подать
  `TYPING_START` с `s=9` и `d = text("bad")`; ожидать `stale_sequences` содержит 9,
  `malformed` пуст, `last_seq=10`, `typing` пуст. На текущем коде — green; на мутанте
  «parse до stale» — red.

### P3-2 — no-clock не проверяет отсутствие member upsert

- **Где:** `src/state/typing/reducer.rs:27-33` (ранний return до upsert);
  `tests/typing_reducer.rs:213-220` (t10) проверяет `typing`/`ignored`/`last_seq`, но не
  `users`.
- **Почему это пробел:** если upsert member перенести выше проверки часов, t10 останется
  зелёным (карта `typing` пуста), но `State.users`/`State.user` уже изменятся. Контракт
  «событие ignored, store не меняется» тогда будет нарушен незаметно.
- **Evidence:** инспекция кода; отдельного assert на `users` нет.
- **Закрытие red-first:** в no-clock кадр добавить `member.user`; снять копию `users`/
  `state.user` до кадра и сравнить после; на мутанте «upsert до проверки часов» — red.

### P3-3 — `MESSAGE_CREATE` без автора и с автором в другом канале не покрыты хуком

- **Где:** hook `src/state/reducer.rs:54-62`; t09 `tests/typing_reducer.rs:175-199`.
- **Почему это пробел:** t09 проверяет только сообщение автора в том же канале, где стоит
  запись; helper `remove_for_message` отдельно проверяет неверный канал, но сам hook — нет.
  Ошибка «передали не тот channel_id» или удаление при `author=None` не будут пойманы.
- **Evidence:** код корректен по инспекции (`channel_id` и `author_id` берутся до move);
  тест на authorless/wrong-channel отсутствует.
- **Закрытие red-first:** добавить три кейса: (а) `MESSAGE_CREATE` без `author` — запись
  остаётся; (б) тот же автор в другом канале — запись остаётся; (в) тот же автор в том же
  канале — запись удалена. На мутанте «удалять по author без channel» — red.

### P3-4 — saturating overflow/underflow и repeat при меньшем `now` не покрыты

- **Где:** `src/state/typing/types.rs:58-60` (`saturating_sub`),
  `src/state/typing/reducer.rs:49` (`saturating_add`); t07
  `tests/typing_reducer.rs:129-140` проверяет только repeat при большем `now`.
- **Почему это пробел:** контракт явно требует saturating `now + TTL` и
  `remaining_ms`; тест на `u64::MAX-1`/`u64::MAX` и на `remaining_ms(expires+1)` отсутствует.
  Семантика repeat при меньшем (out-of-order) `now` реализована как last-wins, но нигде не
  зафиксирована; caller-часы должны быть монотонными, поэтому это скорее документирование,
  чем баг.
- **Evidence:** `saturating_add`/`saturating_sub` в коде; тесты только на обычных значениях
  и на `now == expires` (0), без `now > expires`.
- **Закрытие red-first:** тест старта на `u64::MAX-1` и `u64::MAX`: expiry не переполняется,
  `is_active`/`remaining_ms` на границе; тест `TypingEntry::remaining_ms(expires+1) == 0`
  (на мутанте `saturating_sub` → `-` — red/panic); отдельный тест repeat с меньшим `now` с
  явным ожиданием last-wins (или явно запретить такой вход и диагностировать).

### P3-5 — интеграционные поверхности `RESUMED`/`Debug`/root re-exports/`apply_dispatch` не покрыты

- **Где:** `src/state/reducer.rs:44` (`Resumed` no-op), `src/state/types.rs:147-181`
  (`State::Debug` — длина), `src/lib.rs:44-52` (root re-exports),
  `src/state/reducer.rs:108-129` (`apply_dispatch`).
- **Почему это пробел:** `RESUMED` после typing не проверен (контракт — не очищать);
  `Debug` не проверен на отсутствие member payload; root re-exports компилируются, но ни
  один тест их не импортирует; `apply_dispatch` с `TYPING_START` без часов не проверен
  (проверен только `apply_frame`).
- **Evidence:** код корректен по инспекции; покрытие — косвенное.
- **Закрытие red-first:** небольшие тесты: `RESUMED` сохраняет запись; `format!("{state:?}")`
  не содержит username/member; `use discord_core::{TYPING_TTL_MS, TypingStart, TypingEntry,
  TypingKey, apply_frame_at};`; `apply_dispatch` с `TYPING_START` даёт
  `ignored "typing.received_at_ms"` и пустой store.

### P3-6 — mutation evidence: не все ветви из DESIGN §9 и неточный label T3

- **Где:** `waves/B01/MUTATIONS.md:38-45`, `/tmp/opencode/b01-mutations.sh`
  (T1–T8).
- **Почему это пробел:** DESIGN §9 перечисляет M3 «no-clock создаёт запись», M4 «повтор не
  продлевает» и M6 «removal по user без channel»; они покрыты assert'ами t10/t07/t09, но
  мутации для них не запускались. T3 в `summary.txt` назван «prune removes active boundary
  entry», хотя мутация `now <= expires` **оставляет** истёкшую граничную запись, а не удаляет
  активную; T1 — TTL=0, а не 9_999/10_001. Это неточность evidence, не дефект кода.
- **Evidence:** T1–T8 падают (exit 101), текущие md5 совпадают с backup; T3-лог падает на
  t10; формулировка label не соответствует мутации.
- **Закрытие:** добавить T9 (removal игнорирует channel), T10 (repeat не меняет
  started/expires), T11 (no-clock создаёт запись); исправить label T3; опционально TTL
  9_999/10_001 вместо 0.

### P3-7 — same `(channel,user)` с другой гильдией молча перезаписывает guild

- **Где:** `src/state/typing/reducer.rs:41-51` (ключ только channel/user),
  `src/state/typing/selectors.rs:64-72` (точный guild).
- **Почему это пробел:** если в некорректном/повторном payload тот же `(channel,user)`
  придёт с другим `guild_id`, last-wins молча перенесёт запись в другую гильдию; диагностики
  `inconsistencies` нет. Для Discord channel id глобальны, поэтому риск низкий, но контракт
  этого случая не описывает.
- **Evidence:** инспекция; тест t13 использует разные channel id для разных гильдий.
- **Закрытие:** либо добавить тест/диагностику `typing:<channel>:<user>:guild-mismatch`, либо
  явно зафиксировать в DESIGN, что такой вход вне модели.

### P3-8 — процесс/трассируемость red и gate

- **Где:** `waves/RW-20/w20-report.md:50-53` (compile-red лог перезаписан после реализации;
  behavior-red parser/reducer/selectors отдельно не снимался); `waves/B01/logs/core-503.log`,
  `workspace-545.log`, `clippy.log` — mtime 00:49, а `fmt` — 00:52; `gate-worker.json` —
  npm-only (meta/unit/smoke).
- **Почему это пробел:** TZ §DoD требует сохранить red compile/behavior exit 101; сейчас
  есть compile-red и integration-red старых shared-тестов, но нет behavior-red новых typing
  тестов. Полные core/workspace/clippy прогоны сделаны до финального fmt; семантика fmt не
  меняет, но формально это pre-fmt срез. Gate-артефакт не аттестует Rust-фазы, они живут
  отдельными логами.
- **Evidence:** mtimes, `w20-report.md` §2, `INTEGRATION.md` §3, `gate-worker.json`.
- **Закрытие:** immutable red-логи с timestamp (не перезаписывать); behavior-red после I0
  на заглушках; после fmt — повторный core/workspace (targeted уже перезапущен) или явная
  запись «fmt-only, семантика не менялась»; в batch-сводку gate добавить cargo-счётчики/ссылки.
  Это процесс, не код.

### P3-9 — точность документации/провенанса

- **Где:** `waves/RW-20/TEST-MATRIX-P2-11.md:39` («final at max expiry» — при буквальном
  `now = max expires` активных было бы 0, тест использует последний wire timestamp);
  `waves/RW-20/w20-report.md:101-102` («shared-файлы… не менялись, git status — только
  перечисленные новые файлы» — после I0/I1 shared-файлы изменены TL-2, addendum §0 это
  уточняет); `tests/typing_types.rs:61-96` (t01 называет member-поля capture-точными, но
  сам payload строится вручную, `username="user"`; fixture member не содержит).
- **Почему это пробел:** формулировки могут дать ложное ожидание при следующем ревью;
  провенанс member-полей t01 нельзя независимо сверить по fixture (raw capture —
  read-only вне репозитория).
- **Evidence:** текст матрицы/отчёта; fixture 28 строк без `member`; t01 строит payload
  вручную.
- **Закрытие:** заменить «max expiry» на «последний wire timestamp / последний START»;
  переформулировать §5 отчёта («W20 не менял shared; изменения I0/I1 — TL-2, см.
  INTEGRATION.md»); пометить member-поля t01 как синтетические или добавить одну
  минимизированную строку с `member`.

### P3-10 — `typing_entries_by_guild` не проверяет порядок каналов внутри одной гильдии

- **Где:** `src/state/typing/selectors.rs:64-71` (сортировка `(channel_id, user_id)`);
  t13 `tests/typing_selectors.rs:155-186` использует только один канал на гильдию.
- **Почему это пробел:** t13 проверяет точный guild и сортировку пользователей внутри
  канала, но не первичный ключ `channel_id`: сортировка только по `user_id` прошла бы t13.
- **Evidence:** инспекция t13; в guild 100 только channel 10.
- **Закрытие red-first:** добавить в guild 100 второй канал с user_id меньше, чем у channel
  10; ожидать порядок по `channel_id`, затем `user_id`; на мутанте «сортировать только по
  user» — red.

## 4. Ограничения ревью

- Raw capture `reference-oracle/...` не читался (запрещённый путь); 28/28, пары 27+1,
  guild/timestamp проверены по fixture, research и независимому Python-подсчёту, но не
  повторной выборкой из raw JSONL.
- Полный `cargo test --workspace`/`gate:worker` не перезапускался по указанию «gate уже
  есть»; проверены суммы/0 FAILED в `core-503.log` и `workspace-545.log`, а targeted/fmt/clippy
  перезапущены выборочно.
- Мутации не воспроизводились разрушительно (read-only); проверены логи T1–T8 и совпадение
  текущих md5 с `/tmp/opencode/b01-mutation-backup`.
- Часть P3 (stale-before-parse, no-clock member, authorless message, overflow, RESUMED/Debug)
  подтверждена инспекцией кода, а не отдельными красными тестами; это и есть суть замечаний.
- Live Discord, сеть, SQLite/UI/keyring, `reference-oracle/` не использовались; коммитов нет.
- Incident с внешним удалением `waves/RW-20/` отмечен в отчёте; fixture md5 после
  восстановления совпал, код/тесты не пострадали. Это ограничение провенанса, не дефект P2-11.

## 5. Итог

P2-11 реализована по замороженному контракту: TTL 10 000 мс, время только из
`received_at_ms`/`now_ms`, parser/reducer/selectors и shared-интеграция соответствуют
DESIGN §3–§7; corrections C1–C3 арифметически обоснованы и не являются подгонкой.
Независимо подтверждены targeted 15/15, fmt/clippy 0, суммы core 503/workspace 545 и
8/8 заявленных T1–T8; md5 restore совпадает с backup. Блокирующих P0/P1/P2 нет; 10 P3 —
backlog.

Файл ревью: `waves/reviews/review-p2-11.md`.
Логи повторных прогонов: `waves/reviews/p2-11/targeted-rerun.log`,
`waves/reviews/p2-11/fmt-check.log`, `waves/reviews/p2-11/clippy-core.log`.
