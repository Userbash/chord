# G-01b-UI-SETTINGS-FIX — независимое ревью pass 2

**Вердикт: RETURN (P1; процессный P2).** Последовательный сценарий исправлен, но быстрые последовательные выборы серверов всё ещё могут сохранять устаревший id.

## Findings

1. **P1 — race при двух кликах.** `onChoose` запускает `void rememberGuild(id)` без ожидания/очереди (`rust/gui-spike/ui/app.js:1296-1308`). Каждый вызов отдельно делает `settings_load`, затем `settings_save` (`rust/gui-spike/ui/app.js:1282-1289`). При кликах A → B, если чтение/сохранение A завершится после B, последний `settings_save` снова запишет A. В тестах есть только один выбор и затем toggle (`tests/unit/guild-rail.test.mjs:411-480`), поэтому эта потеря последнего пользовательского выбора не обнаруживается.

2. **P2 — отсутствует mutation evidence.** `REPORT-FIX.md` и каталог карточки содержат red/green, но не mutation log/таблицу killed mutants. Это нарушение TEST-POLICY.md §3 и §9 для нового инварианта сохранения `lastGuildId`; одного green-прогона недостаточно.

## Evidence / TEST-POLICY

- RED: `waves/RW-40/PM/g01b-ui/red-fix.log` — exit 1, ожидаемое стирание `lastGuildId` после toggle.
- GREEN: `waves/RW-40/PM/g01b-ui/green-fix.log` — 112 passed / 0 failed, exit 0; отчёт также указывает `tsc` exit 0.
- Targeted тест не моделирует out-of-order `settings_load`/`settings_save`; требуется red-тест для A → B с задержкой и минимальная сериализация или проверка актуальности перед записью.
- Полный gate не требовался воркеру batch, но отсутствие mutation proof не может быть закрыто словами в отчёте.

Cargo в рамках этого ревью не запускался согласно заданию.
