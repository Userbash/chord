# Независимое ревью волн RW-05 (P1-04, IDENTIFY/READY) и RW-06 (P1-05, RESUME/реконнект)

**Ревьюер:** независимый субагент (OpenCode). **Дата:** 2026-10-19.
**Объект:** `rust/crates/discord-core/{src/identify,src/ready,src/resume,src/ws,src/lib}.rs`,
`tests/{identify_ready,resume_reconnect}.rs`; отчёты `/tmp/opencode/rw-05/report.md`,
`/tmp/opencode/rw-06/report.md`; карточки §RW в `TASKS.md` («сдано; ждёт независимого ревью»).
**Режим:** read-only по проекту; единственные записи — временные мутации с восстановлением по бэкапу
и авторской проверкой md5; сеть не использовалась (стабы — только `127.0.0.1:0`), живой Discord не трогался;
`logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep` — только чтение.

## Вердикты

| Волна | Вердикт | Кратко |
|---|---|---|
| **RW-05** (IDENTIFY/READY) | **принято с нитами** | red-first честный (2 независимых red-артефакта + мой повтор стабами), md5 сходятся, оракулы lab сходятся, мутаций 10/10 поймано; ниты — только артефакты/числа в отчёте, не продукт |
| **RW-06** (RESUME/реконнект) | **принято с нитами** | независимая репродукция red стабами совпала 1:1 (1 passed / 11 failed), md5 сходятся, оракулы сходятся, 8/11 мутаций поймано; выжившие R4/R10/R11 — дыры покрытия тестов (не дефекты продукта) |

Продуктовых дефектов в обеих волнах не найдено. Все комментарии ниже — уровня тестов/артефактов/доков.

## 1. Файлы и контрольные суммы (фактические, сверены лично)

```
$ md5sum rust/crates/discord-core/src/{identify,ready,resume,ws,lib}.rs \
         rust/crates/discord-core/tests/{identify_ready,resume_reconnect}.rs
17f2f3102c3c1b9fcedcb3cc9e1f3cc6  src/identify.rs
c7c984890895eaca1214ac3705b34019  src/ready.rs
211448f7051afcf5af78a7411b9c94dc  src/resume.rs
b822d6a6790f11a306a442c6a58c975c  src/ws.rs
69e299f265a0459f4e104c437e1bf2a5  src/lib.rs
ea4c7f18e0d8d0f6472fcb9b025517e7  tests/identify_ready.rs
c0ce6ef149223e20a127179671d1ba34  tests/resume_reconnect.rs
```

- Совпадают с таблицами отчётов RW-05 (для identify/ready/теста) и RW-06 (все четыре: resume/ws/lib/тест).
- `ws.rs` `659ee47e…` и `lib.rs` `8986fa10…` из таблицы RW-05 — **промежуточное состояние** волны RW-05;
  RW-06 легитимно изменил оба файла (+resume), финальные значения — в таблице выше (и в отчёте RW-06).
- После всех моих мутаций и red-репродукций md5 вернулись к значениям выше (проверено скриптами:
  `restore: OK`, все `True`). `git status` до/после — без изменений: весь `rust/` untracked, коммитов нет.

## 2. Прогоны (выполнены лично, артефакты в `/tmp/opencode/review-rw-0506/`)

| Проверка | Команда | Результат |
|---|---|---|
| Workspace | `cd rust && cargo test --offline --workspace` | **100 passed, 0 failed**, `# EXIT=0` (`workspace-test.txt`) |
| discord-core | (в составе выше) | **58**: lib 2 + gateway_session 15 + identify_ready 16 + resume_reconnect 12 + ws_transport 13 |
| Стабильность RW-06 | `cargo test --offline -p discord-core --test resume_reconnect` ×5 подряд | каждый раз **12/12**, ~0.30 с, флаки не наблюдалось |
| Стабильность RW-05 | `identify_ready` | **16/16** |
| Clippy | `cargo clippy --offline --workspace --all-targets -- -D warnings` | чисто, `# CLIPPY_EXIT=0` (`clippy.txt`) |
| fmt | `cargo fmt --all -- --check` | чисто, `# FMT_EXIT=0` (`fmt.txt`) |
| Гейт | `.gate/worker-2026-10-19T{13-58-37-508,14-12-35-509,14-33-22-949}Z.json` | verdict `passed`, meta 22/22, unit 169/169, smoke 20/20 |

Стиль и надёжность: док-комментарии на русском, `unwrap()` в пяти ревьюируемых src-модулях — 0 вхождений (grep), ошибки не глотаются (варианты
`IdentifyError`/`ResumeError`/`TransportError`), тесты skip-aware (lab-кейсы пропускаются при отсутствии оракула).

## 3. Red-first: проверка честности

**RW-05.**
1. Исходный red воркера: `/tmp/opencode/rw-05/red.txt` — `# EXIT=101`, **1 passed; 12 failed**
   (13 тестов), md5 тест-файла `ca0ba9c2a3e542feb32c3bd354d626c8`; прошёл
   `ws_no_identify_without_option` — негативный контроль старого пути RW-04, новых сущностей не требует. Честно.
2. Red исправления контракта: `red-contract-fix.txt` — `# EXIT=101`, targeted-прогон
   `ws_identify_sent_once_after_hello` FAILED («payload не под `d`»), md5 теста `52479f7723e7a5c7f28eec2c96e893a3`.
3. Red добавленных после само-ревью «дыр»-тестов: `mut/red-gap-tests.txt` — `# EXIT=101`, 13 passed / 3 failed,
   **md5 теста `ea4c7f18…` = финальному** (значит после этого red правился только код, тест не менялся). Честно.
4. Моя независимая репродукция: заглушки тел `build_identify*`, READY-интеграции и
   `maybe_send_identify`/`last_ready` на **текущих 16 тестах** → **1 passed; 15 failed** (`# EXIT=101`,
   `red-repro-rw05b.txt`). Проваливаются все содержательные кейсы; зелёный — тот же негативный контроль.

**RW-06.**
1. Red воркера: `/tmp/opencode/rw-06/red.txt` — `# EXIT=101`, **1 passed; 11 failed**, md5 теста
   `60bf79f41d62dd3dff4556c5e2002b5a`; прошёл `resume_debug_redacts_token` (заглушки не ломали Debug). Честно отмечено.
2. **Нит:** md5 red-времени не совпадает с финальным `c0ce6ef1…` (файл менялся 19:32:48, red — 19:30:52) —
   это пост-red правки форматирования/clippy (explicit_counter_loop, 2× while_let_loop), совместимые с red;
   red-time-версия теста побайтово не сохранена (сохранён только `impl-backup/` — красная реализация). Проверяемо косвенно.
3. Моя независимая репродукция: заглушки тел `build_resume_frame`, `plan_reconnect`, resume-ветки
   `connect_and_run_resume`, `resume_state`, `last_invalid_session_resumable` на **финальных тестах** →
   **1 passed; 11 failed** (`red-repro-rw06.txt`), множество падений и счётчики совпадают с логом воркера.
   Оценка: **честно** (cosmetic-правки после red не могли «позеленить» тесты; независимая репродукция совпала).

## 4. Оракулы lab (перепроизводство независимо, Python по JSONL и JSON)

Разрывы/швы `gw-proto/msgs-run2-95974_{8269,8608,10029,13072}.jsonl`:

| файл | op0 (events) | s min..max | RESUMED | прочее |
|---|---|---|---|---|
| `…_8269` | 569 | 95..663 | s=96 | хвост предыдущей сессии |
| `…_8608` | 59 | 664..722 | s=668 | шов 663+1=664 |
| `…_10029` | 331 | 1..331 | — | op7 ×1 (reconnect) |
| `…_13072` | 40 | 332..371 | s=332 (первый op0) | шов 331+1=332 |

Числа отчёта RW-06 подтверждены; дополнительно совпало с `gw-proto/final-facts.log`
(швы 663→664 и 331→332, `sentFrames`: IDENTIFY 943 Б; RESUME 173 (.8269) / 176 (.8608, .13072) — байты маскированных кадров).

READY из `/home/sanya/re-deep/gateway/by-type/002-READY.json` (и `ready-stats.json`):
`top_keys=40`, `guilds=7`, `private_channels=17`, `users=146`, `read_state{157, partial=false, version=128949}`,
`relationships=60`, `user_guild_settings=58`; `user.id` `384…2032` (маскировано), `session_id` длиной 32 (маскировано),
`resume_gateway_url` — домен совпадает. Все числа ТЗ RW-05 сходятся 1:1.

Клиентский бандл `discord.com_assets_web.4d4b2f75fdea5470.js` и `rest-static.md` §4:
`_doIdentify` — 7 полей; проводная форма `send=(e,t,n)=>pack({op:e,d:t})`; capabilities `t?1767421:1734653`
(= `CAPABILITIES_ACTIVE/CHANNEL_OBFUSCATION`); `L7()` возвращает строку → `gateway_connect_reasons:"COLD_START"` строкой
(правка ТЗ, реализована верно); `_doResume` `{token,session_id,seq}`; INVALID_SESSION(false) → IDENTIFY;
QosToken `0A 02 08 00` (client_provided/is_active). Замечаний нет.

## 5. Мутационное тестирование (независимое; каждая: backup → правка src → targeted-тест → restore → md5-сверка)

21 валидная мутация: **18 поймано, 3 выжило**. Исходный I8 (просто `build_identify` без пути) — `COMPILE-FAIL` (нет импорта),
не зачтён, переделан как I8-v2. Логи: `mutations.txt`, `mutations-run.log`, `mutations-extra.txt`; восстановление после каждой — OK.

| # | Волна | Мутация | Итог | Поймал тест |
|---|---|---|---|---|
| R1 | RW-06 | op6 без конверта `{op,d}` | поймано (5) | resume_frame_has_evidence_fields, ws_invalid_session_false, ws_resume_suppresses_identify, ws_resume_sent_once_after_hello, ws_op7_then_resume_keeps_seq |
| R2 | RW-06 | `seq-1` в RESUME | поймано (5) | те же 5 |
| R3 | RW-06 | session_id подменён токеном | поймано (5) | те же 5 |
| R4 | RW-06 | порог `30_000` → `300_000` | **выжило** | — (дыра: тест сверяется с константой, не с литералом) |
| R5 | RW-06 | игнор `invalid_session=false` | поймано (2) | plan_reconnect_policy, ws_invalid_session_false |
| R6 | RW-06 | приоритет IDENTIFY на resume-соединении | поймано (1) | ws_resume_suppresses_identify |
| R7 | RW-06 | `resume_state()` всегда None | поймано (3) | pipe_resume_state_after_ready, lab_resume_seq_continuity, ws_op7_then_resume_keeps_seq |
| R8 | RW-06 | `last_invalid_session_resumable()` всегда None | поймано (2) | pipe_invalid_session_flag, ws_invalid_session_false |
| R9 | RW-06 | op9 `d=0` (int) не распознан | поймано (1) | pipe_invalid_session_flag |
| R10 | RW-06 | RESUME до HELLO (снят guard HELLO) | **выжило** | — (дыра: нет теста «тишина до HELLO» для op6) |
| R11 | RW-06 | перепутаны варианты EmptyToken/EmptySessionId | **выжило** | — (`resume_frame_rejects_empty_fields` проверяет только `is_err`, не вариант) |
| I1 | RW-05 | capabilities − 1 | поймано (2) | identify_has_evidence_fields, ws_identify_sent_once_after_hello |
| I2 | RW-05 | `compress` всегда true | поймано (1) | identify_has_evidence_fields |
| I3 | RW-05 | installation_id не попадает | поймано (1) | identify_properties_evidence_defaults |
| I4 | RW-05 | `user.id` только Int | поймано (1) | ready_accepts_string_user_id |
| I5 | RW-05 | partial int-1 не читается | поймано (1) | ready_accepts_int_partial |
| I6 | RW-05 | IDENTIFY до HELLO | поймано (1) | ws_identify_only_after_hello |
| I7 | RW-05 | IDENTIFY повторяется | поймано (1) | ws_identify_sent_once_after_hello |
| I8-v2 | RW-05 | payload без конверта (`crate::identify::build_identify`) | поймано (2) | ws_identify_sent_once_after_hello, ws_identify_only_after_hello |
| I9 | RW-05 | READY `top_keys + 1` | поймано (2) | ready_summary_from_synthetic_dispatch, lab_ready_from_run2_stream |
| I10 | RW-05 | `last_ready` не запоминается | поймано (2) | ws_identify_sent_once_after_hello, lab_ready_from_run2_stream |

Требование «≥8 мутаций, ≥3 по RW-06» выполнено (11 по RW-06, 10 по RW-05).

**Выжившие (дыры покрытия, не дефекты):**
- **R4** — `plan_reconnect_policy` использует `RESUME_MAX_DOWNTIME_MS` как порог (`tests/resume_reconnect.rs:327`), поэтому мутация
  константы не детектируется. Само значение в коде корректно (`30_000`). Рекомендация: один тест с литералом `30_000`
  (или assert на константу), чтобы случайная правка значения валила тест.
- **R10** — аналог честного теста RW-05 «IDENTIFY до HELLO» для resume-ветки отсутствует: снятый guard HELLO
  не ловится. Рекомендация: кейс «до HELLO на resume-соединении op6 не отправляется».
- **R11** — `resume_frame_rejects_empty_fields` проверяет только `is_err`; подмена варианта проходит. На провод не влияет,
  но матчинг вызывающей стороны может получить неверную диагностику. Рекомендация: assert конкретного варианта (или Display).

## 6. Ниты (по артефактам/отчётам)

**RW-05:**
- N1. `mut/results.txt` битый: пойманные мутации помечены `COMPILE-FAIL` (автоматика грепа ловит `error: test failed`),
  а `M4` помечена `SURVIVED`, хотя `M4.txt` показывает `identify_properties_evidence_defaults ... FAILED` и таблица
  отчёта помечает M4 «пойман». Логи поштучно корректны (M6/M7/M10 — реально зелёные), врёт только сводка. Исправить сводку.
- N2. Устаревшие числа в отчёте: «identify.rs 313 строк» — фактически 315; «identify_ready.rs 950 строк» — фактически 1107.
- N3. В таблице файлов RW-05 `ws.rs`/`lib.rs` — промежуточные md5 (см. §1). Полезно явно связать с правками RW-06,
  чтобы не читалось как расхождение.

**RW-06:**
- N4. Red-артефакт необратим побайтово: md5 теста red-времени `60bf79f4…` ≠ финальный `c0ce6ef1…` (пост-red fmt/clippy),
  сохранён только `impl-backup`. Для будущих волн: сохранять копию тест-файла red-времени (одна строка в скрипте red).
- N5. R4/R10/R11 — см. §5 (закрыть тестами).
- N6. Ограничение `resume_state()` (session_id только из READY; RESUMED-only процесс не перерезюмируется внутренне)
  задокументировано в отчёте и коде — претензий нет, но это контрактное ограничение стоит продублировать в доке модуля
  рядом с `connect_and_run_resume`.

**Общее:**
- N7. `rust/` целиком untracked — провенанс «до/после» доступен только через timestamp+md5+логи. Координатору при приёмке
  зафиксировать коммитом (L1), иначе следующая волна перезапишет единственные внешние доказательства.

## 7. Лично проверено

- md5 всех 7 файлов против таблиц обоих отчётов; финальные значения совпали (§1).
- `cargo test --offline --workspace` — 100/0, `# EXIT=0`; разбивка по бинарям; `resume_reconnect` ×5 — 12/12 стабильно;
  `identify_ready` — 16/16.
- `cargo clippy … -D warnings` и `cargo fmt --check` — чисто (`EXIT=0`).
- Red-first обеих волн: по логам (EXIT-маркеры, md5 тест-файлов, множества падений) **и** собственной репродукцией
  заглушками на текущих тестах (RW-06 — совпадение 1:1; RW-05 — 1/15 на 16 тестах, зелёный — только контрольный RW-04-кейс).
- Оракулы: 4 JSONL (op0/s-диапазоны/RESUMED/швы), `002-READY.json` (все числа), `final-facts.log`, клиентский бандл
  (структура IDENTIFY/RESUME, строковый `gateway_connect_reasons`, QoS-байты, размеры кадров), `rest-static.md` §4.
- Мутационное тестирование: 21 валидная мутация с полным циклом backup→правка→тест→restore→md5 (все восстановления OK).
- Отсутствие `unwrap()` в src-модулях; стабы только `127.0.0.1:0`; отсутствие сетевых вызовов.
- Неизменность рабочей копии после ревью: md5 совпадают с §1, `git status --short` тот же, коммитов нет.

**Ключевые артефакты ревью:** `/tmp/opencode/review-rw-0506/` — `workspace-test.txt`, `clippy.txt`, `fmt.txt`,
`mutations.txt`, `mutations-extra.txt`, `mutations-run.log`, `red-repro-rw06.txt`, `red-repro-rw05b.txt`,
скрипты `mutations.py`, `mutations-extra.py`, `red-repro.sh`, `red-repro-rw05b.sh`; бэкапы `mut-backup/`.
