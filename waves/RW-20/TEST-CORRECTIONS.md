# TEST-CORRECTIONS — P2-11 / RW-20

Дата: 2026-10-21. Карточка: P2-11. Статус: уточнение матрицы/процесса после сверки
`DESIGN-P2-11.md` §7 с арифметикой fixture. **Это не подгонка кода**: продуктовый код и
тесты W20 уже реализуют DESIGN §3–§7; ниже зафиксированы расхождения формулировок
`TEST-MATRIX-P2-11.md`, которые иначе дали бы ложный красный.

## C1 — t08: active count

- Было: `typing_entry_count` = 2 на финальном `now = wire_timestamp * 1000`.
- Стало: store `map len 2`; **active count 1**; пара A истекла.
- Причина: DESIGN §7 считает только активные записи. Пара A
  (`942355562958573628/1301880740245405707`) истекает на `1 792 393 002 000`, пара B
  стартует только на `1 792 393 942 000`; одновременно активными они не бывают.

## C2 — t11: remaining на границе

- Было: `remaining 1/0` на `1 009 999/1 010 000`.
- Стало: `typing_remaining_ms` → `Some(1)` на `1_009_999` и `None` на `1_010_000`;
  `TypingEntry::remaining_ms` на границе → `Some(0)`.
- Причина: DESIGN §7 для просроченной записи возвращает `None`; saturating-ноль — метод
  самой записи (`TypingEntry::remaining_ms`).

## C3 — t15: store vs active selectors

- Было: «2 entries, 2 channels, 1 guild» на финальном `now`.
- Стало: store `2 entries, 2 channels, 1 guild`; active selectors на финальном `now`
  `1/1/1` (пара A скрыта).
- Причина: DESIGN §7 — селекторы фильтруют просроченные записи и не удаляют их; store
  хранит обе пары до явного `prune_expired`.

## C4 — бюджет тестов

- В задании W20 указано 14 кейсов `t01…t14`; в DESIGN §8, TZ и TEST-MATRIX — 15
  (`t01…t15`). Реализованы все 15; DoD — `15/15`.

## C5 — имена reducer API

- В задании: `remove_author`, `reset`, `expire`.
- В DESIGN §5: `remove_for_message`, `remove_entry`, `prune_expired`; READY-reset выполняется
  через `State::from_ready` (пустая карта), отдельной функции нет.
- Реализованы имена DESIGN; это единственный источник истины для integration-лейна.

## C6 — I0 vs I1

- I0 (`waves/B01/BATCH-TZ.md` §4) добавляет только `pub mod typing;`, поле `State.typing`,
  `Debug`/`from_ready`; `EventPayload`/routes не добавляет.
- Поэтому t05 (`Event::from_frame`/`from_frame_at`) и t08–t10 (`apply_frame_at`) остаются
  compile-red после I0 и становятся green только после I1. Это не дефект кода W20.

## Проверка

- Продуктовый код/тесты/shared-файлы этой коррекцией не менялись.
- Fixture: `tests-data/typing-run2.jsonl`, md5 `71526755701a7e1d33061ae8a5d6ea55`.
