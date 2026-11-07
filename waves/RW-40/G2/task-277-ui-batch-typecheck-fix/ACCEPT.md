# ACCEPT — PR-156 / task-277 (`FE-FIX-02`: Устранение 10 ошибок `tsc` в 7 файлах батча `Build-003`)

- **Статус PM:** `APPROVED` (`Ready for Technical Review`)
- **Ветка:** `bugfix/task-277-ui-batch-typecheck-fix`
- **Файлы и MD5:**
  - `rust/gui-spike/ui/features/thread-badge.js` — `8a8dbe830a80e378e3956ad614d29fc6`
  - `tests/unit/thread-badge.test.mjs` — `6d3d5ff74edd2275036d992c8e40c966`
  - `tests/unit/chat-helpers-bridge.test.mjs` — `5cd90592ac62d354d0c64c1d236dccfd`
  - `tests/unit/slowmode.test.mjs` — `47f98b38846d38d7bdd898873c1090cf`
  - `tests/unit/event-banner.test.mjs` — `df7199b63cd808f01081639e74063cf1`
  - `tests/unit/guild-boost.test.mjs` — `2b715cebeef80042911e35721c73e483`
  - `tests/unit/poll-card.test.mjs` — `ff5b0b0f4fd7f8395b28667c3e9d19ba`
- **Проверка DoD и TDD:**
  - Red-лог зафиксирован (10 ошибок TS), Green-прогон: `0` ошибок TS по этим 7 файлам и `68/68 PASS` по 6 наборам unit-тестов.
- **Следующий шаг:** Техническое ревью `TR-185A` (`REVIEW-185A.md`, DeepSeek, 5/5 мутаций на `thread-badge.js`).
