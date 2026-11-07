# PM Acceptance — task-160 (`FE-P1-05`: Invite Modal `#invite-modal` & Guild Context Menu `#guild-context-menu`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/gui-spike/ui/features/guild-modals.js`
  - `tests/unit/guild-modals.test.mjs`
- **Соответствие ТЗ и оракулу:**
  1. Реализован `createGuildModalsController` с контекстным меню сервера `#guild-context-menu` (`role="menu"`, клавиатурная навигация `ArrowDown/Up/Home/End/Escape`), модалкой приглашения `#invite-modal` (`create_invite`, копирование, `maxAge/maxUses/temporary`), модалкой выхода `#leave-guild-modal` (`leave_guild`) и модалкой входа по коду `#join-guild-modal` (`resolve_invite` + `accept_invite`/`join_guild`).
  2. 57/57 unit-тестов (`guild-modals.test.mjs` + `gui-spike-dom.test.mjs`), `tsc 0`, `INT-07 PASS`.
- **Следующий шаг:** Техническое ревью (`Review 73A`) с проверкой `5/5` мутаций.
