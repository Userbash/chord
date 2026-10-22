# Отчёт воркера — RW-29 / W29c (P4-12 reconnect/resume, headless)

## 1. Границы

- Продукт: `rust/crates/discord-voice/src/reconnect.rs` (новый);
  `rust/crates/discord-voice/src/lib.rs` — только строка `pub mod reconnect;` после
  `pub mod reception;` (строка `pub mod media;` и остальной файл не менялись).
- Тесты: `rust/crates/discord-voice/tests/reconnect_contract.rs` (10 тестов, r01–r10)
  плюс 2 unit-теста в `src/reconnect.rs` (насыщение `attempt`, потолок индекса бэкоффа).
- Доказательная база: `waves/RW-29/RE-CONNECT.md` (цитаты `docs/re/gateway-protocol.md`
  §4/§9, ограничения).
- Артефакты волны: `waves/RW-29/workers/W29c/{red.log,green.log}`,
  `waves/RW-29/run-mutations-w29c.py`, `waves/RW-29/mutations-w29c.log`, этот отчёт.
- Не менялись: `media/` и прочие чужие модули/тесты, `Cargo.toml`/`Cargo.lock`, другие
  крейты, доки (`STATE/STATUS/ROADMAP/TASKS/map`), golden (`docs/re/`, `logs/`,
  `functions/`, `evidence/` — только чтение), сеть, live Discord, `unsafe`,
  `git commit/push`.
- `cargo` запускался последовательно с `CARGO_TARGET_DIR=/tmp/opencode/target-w29c`;
  targeted-тесты только своего файла/модуля, workspace/clippy/fmt-гейт и npm не
  запускались (integration-лейн TL-2).

## 2. Файлы и md5

| Файл | md5 |
|---|---|
| `rust/crates/discord-voice/src/reconnect.rs` | `817abac1645a6743cfbe32c6d98993e0` |
| `rust/crates/discord-voice/tests/reconnect_contract.rs` | `3005a9eb2cf6a89c1eec8906aef7e7bb` |
| `rust/crates/discord-voice/src/lib.rs` | `7a1bbf9c84a9796788b60f17a41814eb` |
| `waves/RW-29/RE-CONNECT.md` | `c361221257128f54e133a2c28bc8968b` |
| `waves/RW-29/run-mutations-w29c.py` | `9d36a414265a137858445ff1b9fbe190` |
| `waves/RW-29/workers/W29c/red.log` | `6bf6e1fcefaef2ba8def57ec84e4b585` |
| `waves/RW-29/workers/W29c/green.log` | `1e353b8334fee135893e91d74776668f` |
| `waves/RW-29/mutations-w29c.log` | `0f10de244d167bdc810e6f511c6c2a24` |

`lib.rs` — общий файл волны: изменение ровно одна строка; `pub mod media;` (W29b) и
остальные строки остались без изменений.

## 3. Сделано

**`reconnect.rs`.** Константы `BACKOFF_MS = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000]`
и `INVALID_SESSION_DELAY_MS = 1_000`. `ReconnectAction::{Resume,Identify,GiveUp}`,
`ReconnectPlan { action, delay_ms, attempt }`, `ReconnectPolicy { attempt, max_attempts }`
с `new/attempts/on_disconnect/on_invalid_session/on_established`.

- `on_disconnect(resumable=true)` → `Resume`, задержка `BACKOFF_MS[min(attempt, 5)]`;
  `resumable=false` → `Identify` с тем же бэкоффом.
- `on_invalid_session()` (op9) → `Identify` с `INVALID_SESSION_DELAY_MS`; лимит попыток
  общий с `on_disconnect`.
- `attempt >= max_attempts` → `GiveUp`, `delay_ms = 0`, счётчик не наращивается
  (повторный вызов идемпотентен).
- `attempt += 1` через `saturating_add` (насыщение на `u32::MAX`), `new(0) → 1`.
- `on_established()` сбрасывает `attempt = 0`, в том числе после `GiveUp`.
- Нет случайности, времени, паник, `unwrap` и `unsafe`; индексация таблицы клампится,
  таймер применяется вне модуля.

**`lib.rs`.** Одна строка `pub mod reconnect;` после `pub mod reception;`, файл не
переформатирован.

**`RE-CONNECT.md`.** Зафиксированы цитаты §4 (op7 1×, `.10029`→`.13072` RESUME→RESUMED
s=332, три RESUMED s 95/664/332, паузы ~2.9 с, `op9` не наблюдался, heartbeat 41 250 мс,
исходящие кадры маскированы) и ограничения: точные клиентские задержки не измерены,
op9-ветка нормативная, wire-совместимость не заявляется, модуль безвременной.

## 4. Трактовки контракта (зафиксированы в тестах и доккомментариях)

1. **`ReconnectPlan.attempt` — номер попытки после события.** r01 ожидает `attempt = 1`
   при задержке 1000 мс, значит задержка считается по счётчику *до* инкремента, а в план
   попадает значение *после*.
2. **`GiveUp` не инкрементирует `attempt`** и всегда отдаёт `delay_ms = 0`; план несёт
   достигнутое значение (`max_attempts`), повторный вызов даёт тот же результат.
3. **Насыщение `attempt` — `saturating_add`.** Публично достижимо только через
   `new(u32::MAX)` за неприемлемое число шагов, поэтому истинное насыщение проверено
   unit-тестом внутри модуля (структурный литерал `attempt: u32::MAX - 1`), а контрактный
   r07 проверяет насыщение индекса бэкоффа и отсутствие паники на 1000 шагах.
4. **Задержка op9 фиксированная** (не бэкофф): контракт §1 задаёт
   `INVALID_SESSION_DELAY_MS` независимо от `attempt`; r04 проверяет это на втором шаге.
5. **`BACKOFF_MS` индексируется по `min(attempt, 5)`**, т.е. первая попытка — 1000 мс,
   седьмая и далее — 30 000 мс (потолок), а не переполнение/паника.

## 5. Red (до реализации)

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29c \
  cargo test --offline -p discord-voice --test reconnect_contract
# EXIT=101
# error[E0432]: unresolved import `discord_voice::reconnect`
#   --> crates/discord-voice/tests/reconnect_contract.rs:10:20
# лог: waves/RW-29/workers/W29c/red.log (последняя строка: # EXIT=101)
```

`red.log` снят до появления `src/reconnect.rs` и `pub mod reconnect;`. После red в тестах
сделаны только две косметические правки без изменения утверждений: `let _ =` для
намеренно игнорируемых `#[must_use]`-результатов (иначе `unused_must_use` на гейте) и
`rustfmt` (edition 2024). Набор проверок r01–r10 не менялся.

## 6. Green

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29c \
  cargo test --offline -p discord-voice --test reconnect_contract
# EXIT=0
# test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
# r01..r10 — все ok, предупреждений компиляции 0

CARGO_TARGET_DIR=/tmp/opencode/target-w29c \
  cargo test --offline -p discord-voice --lib reconnect::tests
# EXIT_LIB=0
# 2 passed; 0 failed (attempt_saturates_at_u32_max_without_overflow,
#                     backoff_index_is_clamped_to_last_entry)
# лог: waves/RW-29/workers/W29c/green.log
```

`rustfmt --check --edition 2024` по двум моим файлам — чисто (весь `cargo fmt --all` не
запускался по ТЗ).

## 7. Мутации

`python3 waves/RW-29/run-mutations-w29c.py` — 6 точных строковых мутаций продукта
(`src/reconnect.rs`), каждая: mutate → targeted-тесты → restore из in-memory копии →
md5-сверка (+`touch`, чтобы cargo не переиспользовал артефакт мутанта). Лог:
`waves/RW-29/mutations-w29c.log`.

```
base_md5=reconnect:817abac1645a6743cfbe32c6d98993e0,test:3005a9eb2cf6a89c1eec8906aef7e7bb
M1: exit=101 caught restored=True [r01,r04,r05,r06,r07,r08,r10] — порядок бэкоффа перевёрнут (30с → 1с)
M2: exit=101 caught restored=True [r10,r09] — op9-задержка 1000 → 500 мс
M3: exit=101 caught restored=True [r05,r08] — on_established не сбрасывает attempt
M4: exit=101 caught restored=True [r03,r07,r08,r09] — лимит проверяется как attempt > max (off-by-one)
M5: exit=101 caught restored=True [r01..r08] — ветка resumable инвертирована (Resume ↔ Identify)
M6: exit=101 caught restored=True [r03,r07,r08,r09] — GiveUp возвращает бэкофф вместо 0
caught=6/6 restored_all=True base_md5=reconnect:817abac1645a6743cfbe32c6d98993e0,test:3005a9eb2cf6a89c1eec8906aef7e7bb
```

Все 6 пойманы, продукт восстановлен побайтно (`restored_all=True`, md5 совпадает с base).
Замечание: r04 использует импортированную константу `INVALID_SESSION_DELAY_MS` в
ожидании, поэтому значение op9-задержки пинят литералами r09 и r10 — M2 ловится ими.
Скрипт временно мутирует файл продукта и восстанавливает его по md5 (`restored_all=True`, восстановление проверяется); при SIGKILL восстановление не гарантировано — ограничение.

## 8. Риски и ограничения

- **Точные клиентские задержки Discord не измерены.** В захвате видны только паузы
  ~2.9 с между сокетами; jitter/рандомизация не наблюдались. Наш бэкофф
  детерминированный (TZ §1), а не воспроизведение клиентского.
- **`op9 INVALID_SESSION` в захвате отсутствует** (`docs/re/gateway-protocol.md` §4, §9.3):
  ветка `on_invalid_session` и задержка 1000 мс — нормативное решение, не измерение.
- **Wire-совместимость не заявляется.** Исходящие RESUME/IDENTIFY маскированы (§4, §9.2);
  модуль описывает только решение и задержку, поля кадра/коды закрытия не проверялись.
- **Модуль безвременной.** Применение таймера, закрытие сокета и переподключение — вне
  модуля; интеграция с транспортом P1-05/P4-01 в эту карточку не входила.
- **Лимит попыток — продуктовая политика.** В захвате исчерпания нет; значение задаёт
  вызывающая сторона, `new(0)` сводится к одной попытке.
- **Граница голос/шлюз.** Политика описана для сессии шлюза (op7/op9, `s`); голосовой
  RTC-сокет (op8 Hello, heartbeat 13 750 мс) не проверялся.
