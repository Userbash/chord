# W20 / CORE-2 — отчёт P2-11 (typing TTL), batch B01 / RW-20

- **batch/карточка:** B01 / RW-20, P2-11 (memory-only typing TTL).
- **роль:** CORE-2 (W20), подволны a/b/c.
- **контракт:** `waves/RW-20/DESIGN-P2-11.md`, `TEST-MATRIX-P2-11.md`, `TZ-P2-11.md`
  (заморожены; реализация — по DESIGN §3–§7).
- **статус:** **integrated green 15/15 в рабочем дереве** (I0/I1 TL-2 + targeted QA).
  Batch gate и независимое review ещё впереди; коммитов нет.

## 0. QA integration addendum

- TL-2 применил I0/I1: `State.typing`, `EventPayload::TypingStart`, `from_frame_at`,
  `apply_frame_at`, hook `MESSAGE_CREATE`, re-exports.
- Targeted в рабочем дереве: **15/15, exit 0**; полный core/workspace 503/545, fmt/clippy 0.
- Все 8 typing-мутаций caught; TTL/removal-границы закрыты.
- `waves/RW-20/` восстановлен после внешнего удаления; fixture md5 совпал
  (`71526755701a7e1d33061ae8a5d6ea55`), код и тесты не пострадали.
- Актуальные evidence: `waves/B01/INTEGRATION.md`, `waves/B01/MUTATIONS.md`,
  `waves/B01/logs/targeted-final.log`.

## 1. Сделано

| Файл | Содержимое |
| --- | --- |
| `src/state/typing/types.rs` | `TYPING_TTL_MS = 10_000`, `TypingKey`, `TypingStart`, `TypingEntry`, `is_active` (`now < expires`), `remaining_ms` (saturating) |
| `src/state/typing/parser.rs` | `parse_typing_start(data, received_at_ms)`: map/`Missing`/`InvalidId`, optional `guild_id`/`timestamp`, `member.user` через `parse_user`, mismatch → inconsistency, `typing_indicator_style` → `ignored`, без паник |
| `src/state/typing/reducer.rs` | `apply_start` (no-clock → `ignored "typing.received_at_ms"`, upsert member только при совпадении id, last wins, saturating TTL), `remove_for_message`, `remove_entry`, `prune_expired` |
| `src/state/typing/selectors.rs` | все 9 методов DESIGN §7; активные записи, сортировки `user_id` / `(channel,user)`, counts по активным, `None` для просроченных |
| `src/state/typing/mod.rs` | `pub mod parser/reducer/selectors/types` + re-export типов/константы |
| `tests/typing_types.rs` | t01–t05 |
| `tests/typing_reducer.rs` | t06–t10 |
| `tests/typing_selectors.rs` | t11–t15 |
| `tests/data/typing-run2.jsonl` | копия fixture, 28 строк; md5 совпадает с `waves/RW-20/tests-data/typing-run2.jsonl` (`71526755…6ea55`) |

Оракулы: 28 реальных `TYPING_START` (2 пары 27+1, одна гильдия, timestamp 1 792 392 992…1 792 394 710),
TTL 10 000 мс из официального `TypingStore`; независимый JSON→`EtfValue` адаптер
`tests/support/ready_fixture`. Синтетика — только optional/mismatch/границы.

Проверено: в продуктовом коде нет `unwrap/expect/panic/todo/unimplemented`, нет `std::time`/
`SystemTime`/`Instant`; время только из `TypingStart.received_at_ms`.

## 2. Red

```bash
cd rust
flock /tmp/opencode/discord-cargo.lock cargo test --offline -p discord-core \
  --test typing_types --test typing_reducer --test typing_selectors
```

- до продуктового кода (только тесты): **exit 101**, 48 ошибок `cannot find typing in state` —
  `waves/RW-20/w20-compile-red.log` (лог перезаписан после реализации; причина та же —
  `state/mod.rs` не объявляет `pub mod typing;`; сейчас 68 ошибок компиляции тестов);
- поведенческий red parser/reducer/selectors отдельно не снимался: без I0 модуль не компилируется.
  Это соответствует TZ («compile-red»), но поведенческий red появится только после I0.

## 3. Green / targeted

Реальный green в рабочем дереве **невозможен до I0/I1 TL-2**: `state/mod.rs` не объявляет
`pub mod typing;`, у `State` нет поля `typing`, нет `EventPayload::TypingStart`,
`Event::from_frame_at`, `apply_frame_at` и hook `MESSAGE_CREATE`.

Для проверки кода и тестов собран **изолированный симулятор интеграции** в
`/tmp/opencode/w20-sim` (копия `rust/` + `fixtures/`, патч только в `/tmp`, не в репозитории):
I0-поля + I1-вариант `from_frame_at`/`apply_frame_at`/`MESSAGE_CREATE`.

```bash
cd /tmp/opencode/w20-sim/rust
CARGO_TARGET_DIR=/tmp/opencode/w20-target cargo test --offline -p discord-core \
  --test typing_types --test typing_reducer --test typing_selectors
# exit 0: typing_types 5/5, typing_reducer 5/5, typing_selectors 5/5 — 15/15
```

Логи: `w20-sim-green-targeted.log`, `w20-sim-clippy.log` (clippy `-D warnings` — exit 0).
Это **не** заменяет targeted после I0/I1: TL-2 должен прогнать те же три target в рабочем дереве.

## 4. Targeted / gate

- targeted: **15/15, EXIT=0** в рабочем дереве после I0/I1
  (`waves/B01/logs/targeted-final.log`).
- `cargo fmt --check`, `cargo clippy -D warnings`, core 503, workspace 545 — зелёные в batch
  integration lane (`waves/B01/INTEGRATION.md`).
- `gate:worker` как batch-гейт — следующий шаг QA; воркер его не запускал.

## 5. Файлы + md5

```
47f6714bde6a90db968cd93eb7f04b72  rust/crates/discord-core/src/state/typing/mod.rs
e78c66f3bc451247550a28d22fc3a337  rust/crates/discord-core/src/state/typing/types.rs
f0c31339a80070723028b73c0ae26c62  rust/crates/discord-core/src/state/typing/parser.rs
0f6d580c9b85007e145772d4fbe3b839  rust/crates/discord-core/src/state/typing/reducer.rs
7815aa3ee56e3060d4c63cca99e20fbf  rust/crates/discord-core/src/state/typing/selectors.rs
1cb8621d0f20798206717d4d23982333  rust/crates/discord-core/tests/typing_types.rs
6d38c4565f02768d2425799848f273ea  rust/crates/discord-core/tests/typing_reducer.rs
650d67216dd31353db0fa852b4ca29a6  rust/crates/discord-core/tests/typing_selectors.rs
71526755701a7e1d33061ae8a5d6ea55  rust/crates/discord-core/tests/data/typing-run2.jsonl
f0ae699206727fa14ab69f5025603aba  waves/RW-20/w20-compile-red.log
29b4d54c0eb8effebdb686ac9c43f612  waves/RW-20/w20-sim-green-targeted.log
5f5eecce86ed0969be77e9d277fc1f0f  waves/RW-20/w20-sim-clippy.log
```

Shared-файлы (`state/mod.rs`, `state/types.rs`, `state/reducer.rs`, `src/lib.rs`), Cargo,
доки, golden и чужие тесты не менялись (`git status` — только перечисленные новые файлы).

## 6. Контрактные расхождения (не догадки — нужна TEST-CORRECTIONS)

1. **C1 / t08:** матрица ждёт `typing_entry_count == 2` при `now=wire_timestamp*1000`. По
   DESIGN §7 счётчик учитывает только активные, а fixture-арифметика даёт 1: пара A
   (`942355562958573628/1301880740245405707`) истекает на `1 792 393 002 000`, пара B стартует
   только на `1 792 393 942 000`. Тест проверяет `typing.len()==2` (store) и active count `1`.
2. **C2 / t11:** матрица ждёт `remaining 1/0` на `1 009 999/1 010 000`. DESIGN §7:
   `typing_remaining_ms` → `None` для просроченной записи. Тест проверяет selector `Some(1)`/`None`
   и отдельно `TypingEntry::remaining_ms` → `Some(0)` на границе.
3. **C3 / t15:** матрица ждёт «2 entries, 2 channels, 1 guild» на финальном `now`. По DESIGN §7
   активна только B (1/1/1); store содержит 2 записи/2 канала/1 гильдию. Тест проверяет оба
   уровня явно.
4. **C4 / бюджет:** в задании указано 14 кейсов `t01…t14`, в DESIGN/TZ/TEST-MATRIX — 15
   (`t01…t15`). Реализованы все 15; DoD `15/15`.
5. **C5 / имена reducer API:** в задании `remove_author/reset/expire`, в DESIGN §5 —
   `remove_for_message/remove_entry/prune_expired`. Реализованы имена DESIGN; READY-reset —
   `State::from_ready` (пустая карта), отдельной функции нет.
6. **C6 / I0 vs I1:** I0 не добавляет `EventPayload`/routes (`waves/B01/BATCH-TZ.md` §4).
   Поэтому t05 (`from_frame`/`from_frame_at`) и t08–t10 (`apply_frame_at`) останутся
   compile-red после I0 и станут green только после I1. Это не дефект кода W20.

## 7. Риски

- **P1 (закрыт):** `waves/RW-20/` исчез из рабочего дерева во время работы (внешнее изменение,
  не W20). Каталог восстановлен из контекста W20: DESIGN/TZ/MATRIX/RESEARCH, `TEST-CORRECTIONS`
  и fixture на месте; md5 fixture совпал с копией в `tests/data`. Код/тесты не пострадали.
- **P2:** t08/t15 при буквальной проверке по матрице дадут ложный красный (см. C1/C3);
  нужен фикс матрицы до приёмки.
- **P3:** `State.typing` без cap — по DESIGN §7/B8; защита только TTL + `prune_expired`.
- **P4:** `member.user` может отсутствовать в `State.users` (fixture) — `typing_users` честно
  возвращает пусто, запись доступна через `typing_entries`.

## 8. Следующий шаг

1. Batch `gate:worker` на замороженном срезе B01.
2. Два независимых review (P2-10/P2-11); P0/P1/P2 — red-first.
3. Приёмка Team Lead; после accepted — доки/журнал один раз.
