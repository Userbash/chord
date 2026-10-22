# Независимое ревью RW-13 / P2-02

- Дата: 2026-10-20.
- Предмет: `State`/`Event`/reducer/selectors, READY + события, эффекты-декларации.
- Ревьюер: отдельный read-only агент, не автор кода (сессия `ses_ea7235f2bffeTfEODXqFb1OTU5`).
- Контракт: `waves/RW-13/DESIGN-P2-02.md`, `waves/RW-13/TZ-P2-02.md`,
  `waves/RW-13/RESEARCH.md`, `waves/RW-13/TEST-CORRECTIONS.md`.
- Итог: **принято**, P0 = 0, P1 = 0, P2 = 0. P3 = 6 — неблокирующий backlog.

## 1. Первый раунд: findings

P1 = 1, P2 = 5. Все P1/P2 закрыты red-first; повторное ревью подтвердило закрытие.

### P1-1 — stale malformed dispatch откатывал `last_seq`

`state/reducer.rs`: в ветке ошибки разбора `last_seq = dispatch.seq.or(last_seq)`. Битый
кадр с меньшим `s` откатывал последовательность, после чего старые события применялись
повторно.

Закрыто: stale-проверка в `apply_dispatch` выполняется до разбора (`READY` — исключение),
в ветке ошибки используется `state.last_seq.max(dispatch.seq)`. Red: `r14`, runtime-red
`red-review-findings-runtime.log`; green — 19/19.

### P2-1 — malformed UPDATE без канала upsert-ил автора

`apply_message_patch` вставлял автора до проверки `channel_id`. Неизвестное сообщение без
канала отклонялось, но «фантомный» пользователь оставался.

Закрыто: upsert автора выполняется только для существующего сообщения либо после
подтверждения канала; для неизвестного без канала — только `malformed`. Red: `r15`.

### P2-2 — приватные каналы считались неизвестными

`insert_message` проверял только `channel_guild`/`thread_guild`, но не `private_channels`,
поэтому ЛС из READY давали ложную inconsistency.

Закрыто: проверка включает `private_channels`. Red: `r16` на реальном приватном канале
`696738350706458685`; green без inconsistency.

### P2-3 — невалидный raw-индекс supplemental классифицировался как `malformed`

`merge_supplemental` для `guild_ids[i] = None` с непустой группой писал `malformed`; по
контракту это ссылочное несоответствие (`inconsistencies`).

Закрыто: ветка пишет `inconsistencies`. Red: `r17`; green. Пустая группа с тем же дефектом
осталась P3-6 (диагностическая граница, данных нет).

### P2-4 — отсутствовали корневые re-exports

ТЗ обещало `discord_core::{State, Event, apply*}`, но `lib.rs` их не экспортировал.

Закрыто: `pub use state::{Event, State, apply, apply_dispatch, apply_frame};`. Red:
`r19` не компилировался (`red-review-findings.log`, exit 101); green.

### P2-5 — тест `r12` сверял точный формат строки

Контракт не фиксирует текст диагностики; тест был подогнан под реализацию.

Закрыто: проверяются свойства (одна запись, id `901`, `channel_id`). Отдельного red нет —
поведение не менялось; это отражено в `TEST-CORRECTIONS.md`.

## 2. Повторная проверка

Проверено независимо:

- `apply`, `apply_dispatch`, `apply_frame`; stale до/после разбора, `READY`, `seq=None`,
  malformed READY, non-op0;
- author upsert для create/update/unknown; приватные/неизвестные каналы;
- supplemental: missing guild, invalid raw index, длина групп;
- `last_seq`, детерминизм, порядок эффектов, `State::Debug` (redaction);
- panic-sweep: 1853 malformed-формы, 0 паник;
- READY-оракулы: 7/272/17/146/60/157/58/1;
- supplemental: 138 сырых, `[17,14,7,2,10,87,1]`, 137 после dedup;
- `MESSAGE_UPDATE`: content 208, embed 1; md5 фикстур совпадают с research.

Артефакты и команды:

| Проверка | Результат |
|---|---|
| state tests (`state_types`/`state_reducer`/`state_selectors`) | 42 passed / 0 failed (15+19+8) |
| `cargo test --offline --workspace` | 317 passed / 0 failed |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --offline --workspace --all-targets -- -D warnings` | exit 0 |
| red фиксов | `red-review-findings.log` (compile, 101), `red-review-findings-runtime.log` (4 failed, 101) |
| green фиксов | `green-review-findings.log`, 19/19 |

## 3. P3 (не блокирует)

1. Неполное покрытие: дубликаты/пустой `SESSIONS_REPLACE`, перенос канала между гильдиями,
   смена `channel_id` у сообщения, duplicate `MESSAGE_CREATE`, явный `seq=None`, unknown
   `MESSAGE_UPDATE` с каналом/автором.
2. Устаревшие комментарии тестов T2/T7/R3 — исправлены после ревью (поведение не менялось).
3. Синтетические thread-id — усечённые абсолютные значения реальной фикстуры; оракул слабее,
   на поведение не влияет.
4. Артефакты: `tests-red/` содержит только исходную спеку; r14–r19 — в `tests-final/`;
   `MD5SUMS.txt` и `TEST-CORRECTIONS.md` синхронизированы после ревью.
5. `r19` не вызывает `apply_dispatch`/`apply_frame`; их случайное удаление не поймает.
6. Пустая группа supplemental с невалидным raw-индексом не даёт inconsistency (см. P2-3).

## 4. Вердикт

**Принято.** Блокирующих P0/P1/P2 нет. P3-1…P3-6 — backlog; P3-4 закрыт синхронизацией
архива и corrections, P3-6 оставлен как осознанная диагностическая граница.
