# STUB-16-MEMBER-PROFILE-POPOVER-FIX (MB-03) — Отчёт воркера G2 Frontend Lane B

- **batch/карточка:** RW-40 / STUB-16-MEMBER-PROFILE-POPOVER-FIX (MB-03: member profile popover fix по `waves/RW-40/PM/stub16-profile-ui/REVIEW.md`).
- **сделано:**
  1. **Публичный id и `aria-controls` (`PINS-THREADS-PROFILE-SPEC.md:196-208`):**
     - Поповер создаётся с контрактным `id="user-profile-popover"` и классом `.user-profile-popover` (с поддержкой обратной совместимости для селекторов `#member-profile-popover`).
     - Интерактивная строка участника выставляет `aria-controls="user-profile-popover"`.
     - Функция `openMemberProfilePopover` и закрытие работают с `user-profile-popover`.
  2. **Интерактивная строка участника как trigger (`role="button"`):**
     - В `renderMemberRow` строка участника на экране участников (`#member-list`) получает `role="button"` (семантика кнопки-триггера WAI-ARIA для вызова диалога `aria-haspopup="dialog"`, `aria-expanded="false|true"`).
     - Для строк панели участников (`#member-panel-list`) сохранена роль `role="listitem"` (`options.role = "listitem"`), что предотвращает регрессии в проверках виртуального списка панели.
  3. **Состояние загрузки (`loading`) с avatar placeholder и name skeletons (`PINS-THREADS-PROFILE-SPEC.md:263-267`):**
     - В `renderMemberProfileState(popover, "loading")` добавлены:
       - `.profile-avatar-wrap` с элементом `.profile-avatar-skeleton` (`aria-hidden="true"`);
       - `.profile-body` с двумя скелетон-полосами имени: `.profile-skeleton-name` (14px) и `.profile-skeleton-name--sub` (12px);
       - `.profile-loading` со screen-reader текстом `aria-live="polite"`.
     - Стили в `rust/gui-spike/ui/components/members.css` оформлены строго на токенах (`var(--surface-3)`, `var(--surface-1)`, `var(--radius-round)`, `var(--radius-xs)`, анимация `profile-skeleton-pulse`) без hex/rgb и без `!important`.
  4. **Закрытие поповера при смене канала или экрана (`PINS-THREADS-PROFILE-SPEC.md:296-305`):**
     - Реализован сброс поповера (`closeMemberProfilePopover({ restoreFocus: false })`) при смене `body[data-active-screen]` (через `MutationObserver` для браузера и перехватчик `document.body.setAttribute` для DOM-шима Node.js).
     - Реализовано закрытие поповера при смене канала: клик по `.channel-item` в `#channel-tree`, клик по дереву каналов и смена `data-channel-id` в `#chat-header`.
     - При закрытии по смене экрана/канала `aria-expanded` на триггере корректно сбрасывается в `"false"`, предотвращается удержание поповера на старой строке.
  5. **Тесты и мутации:**
     - В `tests/unit/gui-spike-regressions.test.mjs` блок `describe("STUB-16 / MB-03: member profile popover", ...)` обновлён под новый контракт: 13 тестов проверяют WAI-ARIA атрибуты (`role="button"`, `user-profile-popover`), состояния (включая loading со скелетонами), закрытие по Esc, клику вне, кнопке, смене канала и смене экрана `data-active-screen`.
     - Проведено 5 мутаций с фиксацией падения тестов (exit code 1) и проверкой восстановления исходного MD5.
- **red-прогон:** `waves/RW-40/PM/stub16-profile-ui/red-fix.log` — exit code 1 (3 passed, 10 failed: падения на проверке `role=button`, отсутствии `#user-profile-popover`, отсутствии скелетонов и закрытия по каналу/экрану).
- **green-прогон:** `waves/RW-40/PM/stub16-profile-ui/green-fix.log` — exit code 0 (13 passed, 0 failed, duration ~248ms); `npx tsc -p tsconfig.json` — exit code 0 (чисто).
- **targeted:**
  - `tests/unit/gui-spike-regressions.test.mjs`: 13/13 pass (~248ms)
  - `tests/unit/gui-spike-features.test.mjs`: 100/100 pass (~1063ms)
  - `tests/unit/gui-spike-dom.test.mjs`: 38/38 pass (~483ms)
  - `tests/unit/dispatch_ui.test.mjs`: 43/43 pass (~458ms)
- **oracle:** спецификация `waves/RW-40/DESIGN/PINS-THREADS-PROFILE-SPEC.md` (§4, §5, §6), ревью `waves/RW-40/PM/stub16-profile-ui/REVIEW.md`, дизайн-токены `rust/gui-spike/ui/tokens.css`.
- **мутации:** 5 мутаций зафиксированы в `waves/RW-40/PM/stub16-profile-ui/mutations-fix.log` с проверкой восстановления baseline MD5:
  1. Мутация ID поповера (`popover.id = "wrong-popover-id"`) → killed (9 failing, exit 1), md5 restored.
  2. Мутация роли триггера (`rowRole = "listitem"`) → killed (1 failing, exit 1), md5 restored.
  3. Мутация скелетонов имени (удаление `body.append(...)` в loading) → killed (1 failing, exit 1), md5 restored.
  4. Мутация закрытия по экрану (отключение `data-active-screen`) → killed (1 failing, exit 1), md5 restored.
  5. Мутация закрытия по каналу (отключение `closeMemberProfilePopover` на `.channel-item`) → killed (2 failing, exit 1), md5 restored.
- **gate:** targeted unit-тесты green; typecheck чисто; Cargo/gate:worker в batch запускается integration-лейном.
- **файлы:**
  - `316bc1acb8ab609bd7814f4f2ffff1bd` `rust/gui-spike/ui/features/members.js`
  - `9ff3955f5c7b9941b0564b639e40e991` `rust/gui-spike/ui/components/members.css`
  - `5dbe993ecf515a9e7622ac2258903c03` `tests/unit/gui-spike-regressions.test.mjs`
  - `28ad7adcbbc78353f8d90d5c0866f5d0` `waves/RW-40/PM/stub16-profile-ui/red-fix.log`
  - `3a635c4a0bb4199ce0a0963044283d29` `waves/RW-40/PM/stub16-profile-ui/green-fix.log`
  - `110c83dd678b2a9a9ca5893ba7c7562e` `waves/RW-40/PM/stub16-profile-ui/mutations-fix.log`
- **риски/вопросы:** отсутствуют. Изменения строго изолированы в назначенных файлах воркера. Общие файлы вёрстки (`app.js`, `style.css`, `index.html`, `chat.js`) не затрагивались.
