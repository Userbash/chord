# PM Acceptance — task-164 (`FE-P1-06`: Mount `createUserPopoutController` & `createGuildModalsController` in `app.js`)

- **Статус PM:** `Accepted for Technical Review` (`Ready for Technical Review`)
- **Проверенные файлы:**
  - `rust/gui-spike/ui/app.js`
  - `tests/unit/gui-spike-features.test.mjs`
- **Соответствие ТЗ и оракулу:**
  1. `createUserPopoutController` и `createGuildModalsController` смонтированы в `rust/gui-spike/ui/app.js`, привязаны к `#chat-list`, `#members-list` и `#guild-rail`/`#guild-header`, связаны с `notifications.openModal()`.
  2. Инварианты виртуализации 100k чата и 24k участников сохранены (`206/206` unit tests pass, `tsc 0`).
- **Следующий шаг:** Техническое ревью (`Review 78A`) с проверкой `5/5` мутаций.
