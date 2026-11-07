# PM Acceptance — task-159 (`FE-P1-04`: User Popout `#user-popout` & Profile Modal `#user-profile-modal`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/gui-spike/ui/features/user-popout.js`
  - `tests/unit/user-popout.test.mjs`
- **Соответствие ТЗ и оракулу:**
  1. Реализован `createUserPopoutController` с поповером `#user-popout` (`role="dialog"`, `aria-modal="false"`, роли, заметка, быстрое ЛС по Enter) и модалкой полного профиля `#user-profile-modal` (`role="dialog"`, `aria-modal="true"`, 3 вкладки, возврат фокуса).
  2. Делегированный перехват кликов `attachToChatAndMembers({ chatList, membersList })` и `destroy()`.
  3. 56/56 unit-тестов (`user-popout.test.mjs` + `gui-spike-dom.test.mjs`), `tsc 0`, `INT-07 PASS`.
- **Следующий шаг:** Техническое ревью (`Review 72A`) с проверкой `5/5` мутаций.
