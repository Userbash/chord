# B2b-ATTACH-UI + STUB-12-GIFT-UI — отчёт воркера

- **batch/карточка:** RW-40 / B2b-ATTACH-UI + STUB-12-GIFT-UI (Frontend Lane A).
- **сделано:**
  - Реализована очередь вложений (`#composer-attachments`), счётчик (`#composer-attach-count`), валидация лимитов RE (`MAX_UPLOAD_FILE_BYTES = 10 * 1024 * 1024 = 10485760`, `MAX_ATTACHMENTS_PER_MESSAGE = 10`), единая константа `ATTACH_LIMITS` и тексты `ATTACH_TEXT`/`GIFT_TEXT` в `rust/gui-spike/ui/features/composer.js`.
  - Реализованы 7 состояний чипов (`queued`, `uploading`, `ready`, `error-413`, `error-429`, `error-size`, `error-limit`), удаление чипа из очереди, вызов `invoke('attachments_upload_slots', ...)`, передача `uploaded_filename` при отправке сообщения в `rust/gui-spike/ui/features/composer.js`.
  - Реализована честная модалка подарка 🎁 (`#gift-modal`) при клике на `[data-action='composer-gift']`: открытие/закрытие по Escape/клику на backdrop/кнопке закрытия `[data-action='gift-close']`, атрибуты `aria-expanded`/`aria-controls`/`aria-haspopup`, возврат фокуса на кнопку открытия, отображение честного статуса Nitro-каталога без фейковой оплаты.
  - Реализованы стили вложений и модалки подарка в `rust/gui-spike/ui/components/composer.css` в строгом соответствии с AC-24 (без `!important`, без hex/rgb-литералов, все размеры через `var()` и hairline `1px`).
  - Реализовано progressive enhancement: идемпотентное достраивание недостающих узлов внутри `#composer` и хоста `#gift-modal` без модификации замороженных файлов (`index.html`, `app.js`, `chat.js`, `actions.js`).
  - Написаны 21 unit-тест в `tests/unit/composer.test.mjs`.
- **red:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/red.log 2>&1
  ```
  Exit code: 1 (падение из-за отсутствия экспортов и логики).
  Лог: `waves/RW-40/PM/b2b-attach-ui/red.log`.
- **green:**
  ```bash
  node --test tests/unit/composer.test.mjs > waves/RW-40/PM/b2b-attach-ui/green.log 2>&1
  npx tsc -p tsconfig.json
  ```
  Exit code: 0 (21 pass, 0 fail; duration ~47ms).
  TypeScript: 0 ошибок (`npx tsc -p tsconfig.json` exit code 0).
  Лог: `waves/RW-40/PM/b2b-attach-ui/green.log`.
- **targeted:**
  - `tests/unit/composer.test.mjs`: 21 тест (все pass).
  - Регрессионный контроль: `tests/unit/gui-spike-dom.test.mjs` + `tests/unit/gui-spike-features.test.mjs` — 129 тестов (все pass).
- **oracle:**
  - `waves/RW-40/DESIGN/ATTACHMENTS-SPEC.md` §1…§5.
  - `waves/RW-40/RE-DEEP-REA.md` §1 (STUB-13 вложения), §5 (STUB-12 подарок).
  - `rust/gui-spike/src/attachments.rs` (`MAX_UPLOAD_FILE_BYTES = 10485760`, `MAX_ATTACHMENTS_PER_MESSAGE = 10`, структура слотов `AttachmentUploadSlot`).
- **генерация:** не применимо.
- **мутации:**
  - Покрыты все 7 состояний чипов (`queued`, `uploading`, `ready`, `error-413`, `error-429`, `error-size`, `error-limit`).
  - Покрыта блокировка отправки при ошибочных/незавершённых вложениях и разблокировка при ready/none.
  - Покрыты границы 10 МиБ (`formatBytes`, отсечение на 10485761 байте без сети) и 10 файлов (отсечение 11-го файла с `error-limit`).
  - Покрыты закрытие модалки по Escape/backdrop/gift-close и запрет открытия при `aria-disabled="true"`.
- **baseline/restore:** продуктовый код и тесты проверены `npx tsc -p tsconfig.json`.
- **стабильность:** чипы очищают таймеры и `AbortController` при удалении и очистке очереди; утечек памяти нет.
- **gate:worker:** targeted-тесты зелёные, регрессионный прогон зелёный, `tsc` чистый. Полный `gate:worker` выполняется Integration/QA-лейном на batch.
- **файлы и md5:**
  - `rust/gui-spike/ui/features/composer.js`: `911b36398f7a4d2b68d35532ae2428e3`
  - `rust/gui-spike/ui/components/composer.css`: `f6830946937fe89191af445ff0b839d4`
  - `tests/unit/composer.test.mjs`: `03ae40546f4f1801c553c67656de5eea`
  - `waves/RW-40/PM/b2b-attach-ui/red.log`: `4a58eb33a174c4191fc58a46b65ebd37`
  - `waves/RW-40/PM/b2b-attach-ui/green.log`: `a67f774e6947be42f9f49976c81cb7a2`
- **риски/вопросы:**
  - Замороженные файлы (`index.html`, `app.js`, `chat.js`, `actions.js`, `quick-switcher.js`, `search.js`) не модифицировались.
  - Регистрация строки `tests/unit/composer.test.mjs` в `TDD-PLAN.md §8` выполняется Team Lead при интеграции batch (в соответствии с AGENTS.md §2).
- **следующий шаг:** передать на приёмку Team Lead / Coordinator.
