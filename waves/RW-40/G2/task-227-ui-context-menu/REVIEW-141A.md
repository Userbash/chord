# REVIEW-141A — PR-112 / task-227 (`context-menu.js`, G2)

- **Ревьюер**: worker 141A (DeepSeek Technical Review, token-lean mode)
- **Ветка / HEAD**: `feature/task-227-ui-context-menu` @ `763dda5`
- **Дата**: 2026-10-31
- **Объём ревью**: `rust/gui-spike/ui/features/context-menu.js` (494 стр.), `tests/unit/context-menu.test.mjs` (616 стр.), `REPORT.md`

## Verdict: APPROVED

## Что проверено

1. Targeted-прогон: `node --test tests/unit/context-menu.test.mjs` → `tests 10, suites 3, pass 10, fail 0`.
2. Соответствие контракту карточки: экспортированы `buildContextMenuItems`, `clampMenuCoordinates`, `createContextMenuController`; наборы пунктов для `message` / `channel` / `user`, условные `pin`/`edit`/`delete`, `danger: true` только у `delete`, зажатие координат с отступом 8px, `role="menu"` / `role="menuitem"` / `data-context-menu` / `data-context-item` / `data-danger`.
3. A11y-навигация: `moveFocus` с циклическим заворотом, `ArrowDown` / `ArrowUp` / `Home` / `End` / `Enter` / `Escape`, `tabindex`/`data-focused`, `preventDefault` на обрабатываемых клавишах.
4. Тесты сверяют наблюдаемое поведение (payload `onSelectAction`, DOM-атрибуты, координаты), а не внутренние поля реализации; DOM-шим минимальный и не подменяет логику модуля.
5. МД5 файлов совпадают с `REPORT.md`: `0b0dac971c189f3cb1892f30fd2f23ce` (модуль), `b7b59792189270ef46d19b69746a9a38` (тест). После мутаций и восстановления md5 модуля совпал побайтно.

## Мутации (5/5 killed)

| # | Мутация в `context-menu.js` | Результат | Убито | Кем |
|---|---|---|---|---|
| M1 | `VIEWPORT_EDGE_PADDING_PX: 8 → 0` | `fail 2` | да | `clampMenuCoordinates` (граница), `createContextMenuController` (data-x/data-y) |
| M2 | `delete.danger: true → false` | `fail 3` | да | наборы items ×2, рендер `data-danger="true"` |
| M3 | `edit` выдаётся при `isOwnMessage \|\| canManageMessages` | `fail 1` | да | «pin и delete для модератора на чужом сообщении» |
| M4 | инверсия метки `mute-channel` (`isMuted ? "Заглушить…" : "Включить…"`) | `fail 2` | да | меню канала, навигация (текст пункта) |
| M5 | `End` → `applyFocusAt(0)` вместо `length - 1` | `fail 1` | да | циклическая навигация / End |

Итог: **5/5 (100%)**. После каждой мутации файл восстанавливался из копии; финальный `md5sum` = `0b0dac971c189f3cb1892f30fd2f23ce`, повторный прогон — `pass 10, fail 0`.

## Замечания (не блокирующие)

- `package-lock.json` числится изменённым в рабочем дереве (`M`), к диффу карточки не относится — перед merge убедиться, что изменение не попадает в PR либо осознанно.
- `createContextMenuController` пока не смонтирован в `app.js` / `chat.js`; это заявлено в «Рисках» отчёта и вынесено в отдельную интеграционную задачу — для данной карточки aceptable.
- `open()` полностью пересоздаёт пункты при каждом вызове; для повторных открытий это корректно, но при частых перерисовках возможен лишний DOM-шум. Оптимизация — вне scope.

## Условия merge

P0/P1 замечаний нет; `gate:worker` в token-lean режиме не запускался по указанию ТЗ. Разрешаю merge в `unstable` после зелёного CI и подтверждения PM.
