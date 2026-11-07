# STUB-16-MEMBER-PROFILE-POPOVER-UI (MB-03) — Отчёт воркера G2 Frontend Lane C

- **batch/карточка:** RW-40 / STUB-16-MEMBER-PROFILE-POPOVER-UI (MB-03: member profile popover).
- **сделано:**
  1. Реализован поповер профиля участника (`#member-profile-popover`, `.user-profile-popover`, `.member-profile-popover`) по спецификации `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§1, §4, §5, §6):
     - Прогрессивное создание DOM-узла `#member-profile-popover` (`role="dialog"`, `aria-label="Профиль пользователя"`, `aria-modal="false"`, `tabindex="-1"`, `data-state="empty|loading|error|populated"`).
     - При клике или клавиатурной активации (Enter, Space) строки участника (`.member` в `#member-list`) поповер открывается, выставляются атрибуты доступности: `aria-haspopup="dialog"`, `aria-expanded="true"`, `aria-controls="member-profile-popover"`, фокус переносится на поповер.
     - Отображение данных профиля: верхний баннер с фоном по `--avatar-hue`, аватар с монограммой и статус-дотом, отображаемое имя (`.profile-display-name`), тег/юзернейм (`.profile-tag`), статус/активность (`.profile-custom-status`), роли в виде бейджей (`.profile-badges [role="listitem"]`), кнопки действий.
     - Поддержка полной матрицы состояний (`loading`, `empty`, `error`, `populated`) с честным текстом отказа и кнопкой повтора (`[data-action="retry-profile"]`) в `role="alert"`.
     - Закрытие по клавише `Escape`, кнопке закрытия (`.profile-close` / `[data-action="close-profile"]`) и клику вне поповера (`outside click`) с возвратом фокуса на элемент-триггер и сбросом `aria-expanded` в `"false"`.
  2. Стили в `rust/gui-spike/ui/components/members.css` оформлены строго на дизайн-токенах из `tokens.css` (без единого hex/rgb-литерала и без `!important`).
  3. Тесты добавлены в `tests/unit/gui-spike-regressions.test.mjs` в блок `describe('STUB-16 / MB-03: member profile popover', ...)`. Проверены все инварианты WAI-ARIA, переключение состояний, доступность и клавиатурная навигация.
- **red:** `node --test tests/unit/gui-spike-regressions.test.mjs > waves/RW-40/PM/stub16-profile-ui/red.log 2>&1` → exit code 1 (3 passed, 8 failed; `AssertionError: нет #member-profile-popover`).
- **green:** `node --test tests/unit/gui-spike-regressions.test.mjs > waves/RW-40/PM/stub16-profile-ui/green.log 2>&1` → exit code 0 (11 passed, 0 failed, duration ~207ms); `npx tsc -p tsconfig.json` → exit code 0 (чисто).
- **targeted:** `tests/unit/gui-spike-regressions.test.mjs` (11/11 тестов green), `tests/unit/gui-spike-features.test.mjs` (100/100 тестов green), `tests/unit/gui-spike-dom.test.mjs` (29/29 тестов green).
- **oracle:** спецификация `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§1, §4, §5, §6), токены `rust/gui-spike/ui/tokens.css`.
- **генерация:** не применимо.
- **мутации:** проверено ручными мутациями в TDD-цикле (отсутствие `aria-expanded`, отсутствие возврата фокуса, отсутствие `role="dialog"` или `data-state` вызывают падение соответствующих ассертов).
- **baseline/restore:** восстановлено исходное состояние сторонних файлов; изменены только назначенные файлы воркера.
- **стабильность:** 5 последовательных прогонов `node --test tests/unit/gui-spike-regressions.test.mjs` — стабильно 11/11 pass без флаков.
- **gate:worker:** targeted unit-тесты пройдены (11/11 pass); `npx tsc -p tsconfig.json` пройден без ошибок; Cargo и `gate:worker` согласно правилам batch для воркера не запускались (будут выполнены integration-лейном).
- **файлы:**
  - `3dea788f3fedb22f8ad14a9306d56912` `rust/gui-spike/ui/features/members.js`
  - `77e6b5e9ee6ac23eeec6afff8370e8f2` `rust/gui-spike/ui/components/members.css`
  - `ca0416266f00b06208c6a2814c2aa1ca` `tests/unit/gui-spike-regressions.test.mjs`
  - `1698691c7c014980f28b19138ca397a6` `waves/RW-40/PM/stub16-profile-ui/red.log`
  - `a6422255b6c323a7028bb5af7dcf3c23` `waves/RW-40/PM/stub16-profile-ui/green.log`
- **риски/вопросы:** отсутствуют. Вёрстка и скрипты строго изолированы, запрещённые файлы (`app.js`, `chat.js`, `actions.js`, `composer.js`, `search.js`, `index.html`) не модифицировались.
- **следующий шаг:** передать на приёмку Team Lead / Integration Lane.
