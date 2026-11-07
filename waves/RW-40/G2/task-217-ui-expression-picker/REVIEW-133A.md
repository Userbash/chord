# REVIEW-133A — PR-104 / task-217 (FE-P1-19) expression-picker.js

**Verdict: APPROVED**

- Карточка / ветка: `task-217`, `feature/task-217-ui-expression-picker`
- Ревьюер: Technical Reviewer (DeepSeek), PR-104
- Дата: 2026-10-31

## Прогоны

| Проверка | Команда | Результат |
|---|---|---|
| Targeted | `node --test tests/unit/expression-picker.test.mjs` | `tests 8, suites 2, pass 8, fail 0` |
| Post-mutation restore | тот же прогон после восстановления файла | `tests 8, pass 8, fail 0` |
| MD5 после restore | `md5sum rust/gui-spike/ui/features/expression-picker.js` | `d6bbcca8609833bced40b09cd8817e66` — совпадает с `REPORT.md` |
| MD5 теста | `md5sum tests/unit/expression-picker.test.mjs` | `5e4f7a2fef0224cf0135f5900bc0d885` — совпадает с `REPORT.md` |

## Мутации: 5/5 killed

Каждая мутация вносилась точечной заменой в `rust/gui-spike/ui/features/expression-picker.js` (assert на ровно одно вхождение), затем целевой прогон, затем восстановление из копии.

| # | Мутация | Ожидание | Факт | Статус |
|---|---|---|---|---|
| M1 | `MAX_RECENT_EMOJIS` 18 → 17 | падение константного теста и теста обрезки (len 18, `emoji-5` на позиции 17) | `fail 2` | KILLED |
| M2 | `recordRecentEmoji`: `[emojiItem, ...filtered]` → `[...filtered, emojiItem]` | падение проверок порядка (`rocket`, `smile` первыми) | `fail 1` | KILLED |
| M3 | `matchesSearchQuery`: убрать `.toLowerCase()` у запроса | падение регистронезависимого поиска (`ОГОНЬ`) | `fail 1` | KILLED |
| M4 | `renderGrid`: `applySkinTone(baseGlyph, skinTone)` → тон `0` | падение проверки `textContent === "👋\u{1F3FD}"` | `fail 1` | KILLED |
| M5 | GIF-обработчик: `if (!event?.shiftKey)` → `if (true)` | падение «Shift+клик оставляет пикер открытым» | `fail 1` | KILLED |

Итог: 5/5 мутаций убиты, эквивалентных выживших нет. После восстановления исходный файл побайтово совпадает с `REPORT.md` (md5 подтверждён).

## Что проверено по коду

- Контракт `EXPRESSION_TABS_RU` / `SKIN_TONE_MODIFIERS` (Fitzpatrick `1..5`, индекс `0` без модификатора) / `MAX_RECENT_EMOJIS = 18` заморожен через `Object.freeze`, значения совпадают с тестом.
- `applySkinTone` корректно отсекает `<= 0`, `>= length`, `NaN`, нечисловые и пустые входы; `recordRecentEmoji` не мутирует входной массив, дедуплицирует по `id || name`, обрезает до 18.
- Контроллер рендерит `[data-expression-picker][role=dialog]`, 3 вкладки с `aria-selected`, `[data-picker-search]` с русским плейсхолдером, `[data-skin-tone-select]` (6 опций) и `[data-picker-grid]`; переключение вкладок, поиск, тон кожи, `Shift+клик`, `Escape` и `onClose` покрыты тестами. Ошибка при отсутствии контейнера — явный `TypeError`, ошибки не глотаются.
- Red-фаза зафиксирована (`ERR_MODULE_NOT_FOUND`), green-лог и md5 в `REPORT.md` соответствуют фактическому состоянию.

## Замечания (не блокирующие)

1. `package-lock.json` в рабочем дереве изменён (`git status: M package-lock.json`). В scope PR-104 он не входит и в `REPORT.md` не заявлен — интегратору не включать его в merge и, если правка не нужна, откатить до `unstable`.
2. A11y-долг на будущее (вне DoD карточки): вкладки не поддерживают стрелки/`Home`/`End`, нет `aria-controls`/`tabpanel`, нет focus-trap в диалоге, нет `roving tabindex` при инициализации (только при `syncTabsState`).

## Блокеры

Нет. `P0 = 0`, `P1 = 0`.
