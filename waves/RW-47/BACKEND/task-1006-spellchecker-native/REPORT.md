# Отчёт о выполнении TASK-1006 / TASK-1006A / TASK-1006B (Нативная проверка орфографии в поле ввода)

- **Batch / Карточка:** `RW-47` / `TASK-1006` (`TASK-1006A`, `TASK-1006B`) — Нативная проверка орфографии в поле ввода (`hunspell` / `libenchant`), дебаунс `150 мс`, интерактивное волнистое подчёркивание `#f23f43` (`red-wavy-underline`) и контекстное меню ПКМ.
- **Сделано:**
  1. Реализован нативный Rust-модуль `rust/discord-gpui/src/chat/spellchecker.rs` (`#![forbid(unsafe_code)]`, 0 `.unwrap()` / `.expect()`, функции `< 50` строк, 780 строк `< 800`): валидация и парсинг словарей Hunspell `.aff`/`.dic`, токенизация ввода с фильтрацией URL, e-mail, inline-кода, упоминаний и эмодзи, расчёт расстояния Дамерау — Левенштейна, генерация до 5 вариантов исправления (`MAX_SUGGESTIONS = 5`), контроллер дебаунса `150 мс`, интерактивные состояния `misspelled-word` (`red-wavy-underline` -> `pointer` -> `open suggestions`), контекстное меню ПКМ с заменой слова в 1 клик (`DISCORD_SPELLCHECK_REPLACE_MISSPELLING`) и пользовательским словарём (`DISCORD_SPELLCHECK_SET_LEARNED_WORDS`).
  2. Реализован автономный валидатор и CLI `scripts/verify-spellchecker.mjs` (сверка с оракулами `waves/kanban-board.json`, `docs/re/backend-ipc.json`, `fixtures/hunspell/test.{aff,dic}`, `/home/sanya/re-deep/{en,ru}-catalog.json`, проверка 9 измерений `TEST-POLICY.md`, маскирование секретов, коды выхода `0 / 1 / 2`).
  3. Реализован и пройден контрактный тест `tests/unit/spellchecker-native-1006.test.mjs` (`SUB-1006-0..SUB-1006-5`, включая мутационный гейт `5/5` для Rust и `5/5` для JS = `100%` kill rate).
- **Red-прогон:** `waves/RW-47/BACKEND/task-1006-spellchecker-native/red.log` (`6 failed` до создания продуктовых модулей).
- **Green-прогон:** `waves/RW-47/BACKEND/task-1006-spellchecker-native/green.log` (`6 passed, 0 failed`).
- **Targeted (`T0`):**
  - `node --test tests/unit/spellchecker-native-1006.test.mjs` — `PASS (6/6)`
  - `rustc --test --edition 2024 -D warnings rust/discord-gpui/src/chat/spellchecker.rs` — `PASS (4/4)`
  - `rustfmt --check --edition 2024 rust/discord-gpui/src/chat/spellchecker.rs` — `PASS`
  - `node scripts/verify-spellchecker.mjs` — `PASS (exit 0)`
- **Файлы и MD5:**
  - `rust/discord-gpui/src/chat/spellchecker.rs` — `44d18ecb2d471573065f7b91f1057a6a`
  - `scripts/verify-spellchecker.mjs` — `42ac36651ccf6287a35ac54f75d101eb`
  - `tests/unit/spellchecker-native-1006.test.mjs` — `cc45583461240ced642299c8a1abbaa9`
- **Риски:** Отсутствуют. Все инварианты, лимиты размера (`< 50` строк/fn, `< 800` строк/файл) и мутационный гейт `5/5` (`100%` kill rate) соблюдены.
