# REVIEW-130A — TASK-215 (PR-101), `settings-modal.js`

- **Карточка**: `task-215` / PR-101, отдел G2 Frontend
- **Ветка**: `feature/task-215-ui-settings-modal-controller` @ `fb7af06` (файлы не закоммичены: `?? rust/gui-spike/ui/features/settings-modal.js`, `?? tests/unit/settings-modal.test.mjs`)
- **Ревьюер**: техническое ревью (DeepSeek), режим token-lean
- **Дата**: 2026-10-31

## Verdict: APPROVED

## Прогон
- `node --test tests/unit/settings-modal.test.mjs` → `tests 10`, `suites 5`, `pass 10`, `fail 0`.
- Оракул тестов — независимый DOM-шим (`FakeElement`) с собственной семантикой атрибутов/событий/`querySelector`, не текущая реализация.
- MD5 после прогона мутаций совпадает с REPORT.md:
  - `dd3f783716ae3fc15f9ac2522d7564bf` `rust/gui-spike/ui/features/settings-modal.js`
  - `deb64673bd4fdf2ea8d2cb53b58affe5` `tests/unit/settings-modal.test.mjs`

## Мутации: 5/5 убиты

| # | Мутация | Результат | Убивший тест |
|---|---------|-----------|--------------|
| M1 | `setTab`: снят guard `isValidTabId` (`if (false && …)`) | exit 1, fail 1 | «игнорирует невалидные tabId…» |
| M2 | `updateSetting`: `onUpdateConfig(state)` вместо `{ ...state }` | exit 1, fail 1 | «вызывает onUpdateConfig({ ...state }) с копией» |
| M3 | `setTab`: `aria-selected` всегда `"true"` | exit 1, fail 3 | «переключает активную вкладку…», «игнорирует невалидные tabId…» |
| M4 | `close()`: убран `unmountModalNode()` | exit 1, fail 2 | «close() и кнопка 'Закрыть (ESC)'…», «Escape закрывает…» |
| M5 | `confirmLogout()`: подтверждение не вызывает `onLogout()` | exit 1, fail 1 | «confirmLogout() … вызывает onLogout() только после подтверждения» |

Исходный файл восстановлен после каждой мутации; финальный `md5sum` идентичен отчёту.

## Проверки контракта (по коду)
- `open(initialTab)`: `role="dialog"`, `aria-modal="true"`, `data-settings-open="true"`, `data-active-tab`; невалидный `initialTab` откатывается на `appearance`.
- `SETTINGS_TABS_RU`: 5 замороженных разделов, русские подписи, `aria-selected`/`tabindex` синхронизированы.
- `updateSetting`: пустой/нестроковый ключ → `false` без вызова колбэка; наружу уходит защитная копия; DOM-контрол активной панели синхронизируется.
- `confirmLogout`: `role="alertdialog"`, `data-logout-confirm="true"`, текст «Вы уверены, что хотите выйти?»; «Отмена» без `onLogout()`, «Выйти» → `onLogout()` один раз.
- `close`: идемпотентен (`false` на закрытом окне, `onClose()` не дублируется), Escape и кнопка закрытия идут тем же путём.
- Невалидный `containerEl` → `TypeError` (явная ошибка, не тихое падение).

## Non-blocking замечания
1. Нет отдельного теста на `normalizeInitialConfig` с `null`/массивом и на `open()` поверх уже открытого окна (повторный `open` вызывает `unmountModalNode()` без `onClose()` — поведение намеренное, но контракт не зафиксирован тестом).
2. Монтаж контроллера в shell (`app.js`/`screens.js`) вне write-scope карточки — остаётся интеграционной задачей (совпадает с разделом «Риски» отчёта).
3. Ветка не закоммичена и не запушена — ожидаемо для воркера; merge делает интегратор.

Блокирующих замечаний (`P0`, `P1`) нет.
