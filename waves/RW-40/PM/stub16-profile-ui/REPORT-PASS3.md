# STUB-16-ATSPI-FIX (MB-03 Pass 3) — Отчёт воркера G2 Frontend Lane B

- **batch/карточка:** RW-40 / STUB-16-ATSPI-FIX (устранение регрессии AT-SPI `list item` на экране участников `#screen-members` при сохранении контракта триггера по `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` §4.1).
- **сделано:**
  1. **Восстановление `role="listitem"` на строках `.member` (`rust/gui-spike/ui/features/members.js`):**
     - В `renderMemberRow` для всех непустых строк `.member` (включая `#member-list` на `#screen-members` и `#member-panel-list`) возвращена роль `role="listitem"` (`const rowRole = options.role ?? "listitem"`), благодаря чему WebKitGTK AT-SPI снова экспортирует строки участников как `list item` (≥ 10 элементов на экране участников в `scripts/gui-window-probe.py:509`).
  2. **Кнопка-триггер внутри строки участника по `PINS-THREADS-PROFILE-SPEC.md` §4.1 (строка 199: `role="button"` (либо кнопка внутри строки)):**
     - Элемент имени `.member-label` внутри `.member` оформлен как интерактивный триггер с `role="button"`, `tabindex="0"`, `aria-haspopup="dialog"`, `aria-controls="user-profile-popover"`, `aria-expanded="false|true"` и обработчиками `click` / `keydown` (`Enter`, `Space`), при этом дочерняя структура строки `.member` (`avatar`, `member-name`, `member-role` — 3 дочерних элемента) сохранена без изменений.
     - Добавлена синхронизация `syncTriggerAria(trigger, expanded)` в `openMemberProfilePopover` и `closeMemberProfilePopover`, поддерживающая `aria-expanded="true|false"` согласованно как на строке `.member`, так и на вложенном триггере `[role="button"]`.
  3. **Тесты и мутационная проверка (`tests/unit/gui-spike-regressions.test.mjs`):**
     - В `describe("STUB-16 / MB-03: member profile popover")` закреплена проверка, что в `#member-list` рендерится `>= 10` строк `.member`, каждая строка имеет `role="listitem"` (инвариант AT-SPI), а внутри строки присутствует триггер `[role="button"]` с `aria-haspopup="dialog"`, `aria-controls="user-profile-popover"` и синхронным переключением `aria-expanded`.
- **red-прогон:** `waves/RW-40/PM/stub16-profile-ui/red-pass3.log` — зафиксировано падение до правки кода как в `tests/unit/gui-spike-regressions.test.mjs` (`actual: 'button', expected: 'listitem'`, `EXIT_UNIT=1`), так и в `tests/smoke/gui-window.smoke.test.mjs` (`gui-window-probe: FAIL: AT-SPI: участников list item 3 < 10`, `EXIT_SMOKE=1`).
- **green-прогон:** `waves/RW-40/PM/stub16-profile-ui/green-pass3.log` — `GREEN_EXIT_CODE=0`:
  - `node --test tests/smoke/gui-window.smoke.test.mjs`: 1/1 pass (~4.2s, AT-SPI видит `>= 10` `list item` на экране участников и в чате).
  - `node --test tests/unit/gui-spike-regressions.test.mjs tests/unit/gui-spike-features.test.mjs tests/unit/gui-spike-model.test.mjs`: 166/166 pass (0 failed, 0 skipped).
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json`: exit code 0 (чисто).
- **targeted:**
  - `tests/smoke/gui-window.smoke.test.mjs`: 1/1 pass
  - `tests/unit/gui-spike-regressions.test.mjs`: 13/13 pass
  - `tests/unit/gui-spike-features.test.mjs`: 124/124 pass
  - `tests/unit/gui-spike-model.test.mjs`: 29/29 pass
- **oracle:** `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` §4.1 (строка 199), `scripts/gui-window-probe.py:509` (AT-SPI `list item >= 10` на экране участников).
- **мутации:** `waves/RW-40/PM/stub16-profile-ui/mutations-pass3.log` — 5/5 KILLED, baseline MD5 `55356f6ee740d224c1339d70fc08ec1d` полностью восстановлен:
  1. `M1`: замена `rowRole` с `"listitem"` на `"button"` → KILLED (`EXIT_CODE=1`).
  2. `M2`: замена роли внутреннего триггера `.member-label` с `"button"` на `"none"` → KILLED (`EXIT_CODE=1`).
  3. `M3`: замена `popover.id` на `"wrong-popover-id"` → KILLED (`EXIT_CODE=1`).
  4. `M4`: удаление `body.append(skeletonName1, skeletonName2)` в `loading` → KILLED (`EXIT_CODE=1`).
  5. `M5`: отключение закрытия поповера при смене `body[data-active-screen]` → KILLED (`EXIT_CODE=1`).
- **gate:** targeted unit + live AT-SPI smoke (`gui-window.smoke.test.mjs`) + `ui-tsc` — 100% green.
- **файлы:**
  - `55356f6ee740d224c1339d70fc08ec1d` `rust/gui-spike/ui/features/members.js`
  - `2455ee3503f662730eb03bbbe992e6f5` `tests/unit/gui-spike-regressions.test.mjs`
  - `7672e63963b09f5fff2e4c70fb547b79` `waves/RW-40/PM/stub16-profile-ui/red-pass3.log`
  - `edbbcc1fb5ce7b7f1afc75753738856b` `waves/RW-40/PM/stub16-profile-ui/green-pass3.log`
  - `cff5c073ee2be5b65fb1298d37787e0f` `waves/RW-40/PM/stub16-profile-ui/mutations-pass3.log`
- **риски/вопросы:** отсутствуют. Затронуты только назначенные файлы карточки.
