# REVIEW-154A — task-243 UI event banner (G2)

Карточка: `task-243-ui-event-banner`. Режим: TOKEN-LEAN (только `event-banner.js`, `event-banner.test.mjs`, `REPORT.md`; полный `gate:worker` и `git commit` не запускались по ТЗ).

## Verdict: APPROVED

## Проверено

**Contract vs tests (11/11 green).** `node --test tests/unit/event-banner.test.mjs` → exit 0, `tests 11 / pass 11 / fail 0`.

**md5 совпадает с REPORT.md:**
```
235ec0e4c33062b2c8928a99fc9defd3  rust/gui-spike/ui/features/event-banner.js
08e92860bfc7b01f99e61115125c3f08  tests/unit/event-banner.test.mjs
```

**Границы файлов.** Продуктовый код — чистые функции без DOM/IPC/сайд-эффектов, `// @ts-check`, оба словаря в `Object.freeze`. Тестовый файл не правился под код: red-фаза в `REPORT.md` зафиксирована падением `ERR_MODULE_NOT_FOUND`.

**Независимый оракул контракта.** Тесты проверяют не реализацию, а поведение: точные RU-строки, срез пробелов в `name`, зажим отрицательных/дробных/нечисловых `userCount` (`-5 → 0`, `"abc" → 0`, `12.9 → 12`), строгий `isActive` только для `status === 2`, инверсию интереса и зажим счётчика в 0. Граничные и негативные измерения покрыты.

## Мутации 5/5

Каждая мутация вносилась в `rust/gui-spike/ui/features/event-banner.js`, прогонялся целевой тест, файл восстанавливался из бэкапа. Лог: `mutations-154A.log`.

| # | Мутация | Результат |
|---|---------|-----------|
| M1 | fallback сущности `"Неизвестно"` → `"Неизвестный"` | KILLED (fail 2) |
| M2 | `isActive: status === 2` → `status !== 2` | KILLED (fail 4) |
| M3 | `Math.trunc` → `Math.round` в `safeCount` | KILLED (fail 1) |
| M4 | убран `.trim()` у `name` | KILLED (fail 1) |
| M5 | сдвиг RSVP `(was ? -1 : 1)` → `(was ? 1 : -1)` | KILLED (fail 4) |

Восстановление: `restored md5 = 235ec0e4c33062b2c8928a99fc9defd3`, `byte-identical = True`, baseline после восстановления — exit 0, 11/11.

## Замечания (не блокирующие)

- В рабочем дереве есть посторонний дрейф: `D node_modules`, `M package-lock.json`. К карточке task-243 он не относится, файлы не трогались — учесть при сборке PR.
- `gate:worker`, `cargo`, `git commit` не выполнялись по условиям TOKEN-LEAN; полный гейт остаётся за integration-лейном.
- `location` принят в typedef, но не выводится — соответствует ТЗ карточки, отдельного поля в контракте нет.
