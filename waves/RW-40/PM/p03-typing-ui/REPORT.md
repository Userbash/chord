# Отчёт: Воркер G2 Frontend Lane A (RW-40, карточка P-03-UI-TYPING-INDICATOR)

## 1. Карточка и задача
- **Карточка**: `P-03-UI-TYPING-INDICATOR` (RW-40, Lane A).
- **Роль**: Воркер G2 Frontend Lane A.
- **Статус**: **ACCEPTED / READY FOR REVIEW**.
- **Назначенные файлы**:
  - `rust/gui-spike/ui/features/typing-indicator.js`
  - `tests/unit/gui-spike-dom.test.mjs`

## 2. Что сделано
1. В `rust/gui-spike/ui/features/typing-indicator.js`:
   - Реализован и экспортирован контроллер `createTypingController(options)`.
   - Прогрессивное создание/поиск узла `#chat-typing-indicator` с атрибутами `role="status"`, `aria-live="polite"`, `data-state="idle|active|slowmode|error"` и идемпотентное переиспользование без дублирования элементов.
   - Экспортированы константы:
     - `TYPING_TTL_MS = 10_000` (10 с)
     - `TYPING_RESEND_MS = 8_000` (8 с)
     - `MAX_TYPING_CHANNELS = 5` (сохранена совместимость с `TYPING_MAX_CHANNELS = 5`).
   - Реализована функция форматирования `formatTypingText(names)`:
     - 1 чел. -> `X печатает…`
     - 2 чел. -> `X и Y печатают…`
     - 3 чел. -> `X, Y и Z печатают…`
     - >3 чел. -> `Несколько человек печатают…`
     - 0 чел. -> `""`
   - Обработка входящих событий `TYPING_START` (`onTypingStart({ channelId, userId, username, now })`):
     - Игнорирование собственного `currentUserId`.
     - Хранение активных печатающих по каналам с ограничением ≤ 5 каналов и LRU-вытеснением самого старого.
     - Автоматическое истечение по `TYPING_TTL_MS` (10 000 мс) и метод `sweep(at)`.
     - Мгновенное снятие набора по входящему сообщению `onMessageCreated({ channelId, authorId })`.
   - Исходящий `notifyInput(channelId, text, now)`:
     - Игнорирование пустого/пробельного текста (не вызывает `send_typing`).
     - Клиентский троттлинг `TYPING_RESEND_MS` (8 000 мс) на каждый канал независимо.
     - При ошибке 429 RateLimited переводит элемент в `data-state="slowmode"` и блокирует отправку на `retry_after_ms`.
     - При прочих ошибках выставляет `data-state="error"`.
   - Сохранена полная совместимость с `createTypingIndicator` и `createTypingSender` для остальных потребителей (`app.js`, `tests/unit/typing-ui.test.mjs`).

2. В `tests/unit/gui-spike-dom.test.mjs`:
   - Существующие 29 тестов оставлены без изменений и без ослаблений (строки 1–853 не менялись).
   - В конец файла добавлен новый describe-блок `"P-03-UI: контроллер индикатора набора и исходящего typing"` с 9 целевыми тестами:
     1. Экспорт констант и функции `createTypingController`.
     2. Прогрессивное создание узла `#chat-typing-indicator`, атрибуты и идемпотентность (отсутствие дублей).
     3. Форматирование русского текста для 1, 2, 3 и >3 человек.
     4. Игнорирование собственного `currentUserId`.
     5. Автоматическое истечение по TTL 10 с и мгновенное снятие по `onMessageCreated`.
     6. Ограничение ≤ 5 каналов и вытеснение самого старого канала.
     7. Исходящий `notifyInput`: пустой ввод и троттлинг 8000 мс на канал.
     8. Ошибка 429 RateLimited: выставление `data-state="slowmode"` и блокировка на `retry_after_ms`.
     9. Прочие ошибки: выставление `data-state="error"`.

## 3. Прогоны TDD (evidence)

### 3.1. RED-прогон
- **Команда**: `node --test tests/unit/gui-spike-dom.test.mjs`
- **Exit code**: `1`
- **Результат**: 29 pass (существующие тесты), 9 fail (`TypeError: createTypingController is not a function`).
- **Лог**: `waves/RW-40/PM/p03-typing-ui/red.log`

### 3.2. GREEN-прогон
- **Команда**: `node --test tests/unit/gui-spike-dom.test.mjs`
- **Exit code**: `0`
- **Результат**: 38 pass (29 существующих + 9 новых), 0 fail, duration ~495 мс.
- **Лог**: `waves/RW-40/PM/p03-typing-ui/green.log`

### 3.3. Регрессионный прогон смежных тестов typing и проверка типов TypeScript (`targeted.log`)
- **Команды**:
  - `node --test tests/unit/gui-spike-dom.test.mjs tests/unit/typing-ui.test.mjs` → `58 pass, 0 fail` (`exit_code=0`)
  - `npx tsc -p tsconfig.json` → `0 ошибок, 0 @ts-ignore` (`tsc_exit_code=0`; параметры `cmd`/`args` на строках 1014–1018 `tests/unit/gui-spike-dom.test.mjs` имеют явные JSDoc-аннотации `@param {string} cmd` и `@param {any} [args]`).
- **Лог**: `waves/RW-40/PM/p03-typing-ui/targeted.log` (`md5 = 0a7724826a55a13dae350fbf382a7c2f`).

## 4. Мутационное тестирование (5 мутаций)
- **Лог**: `waves/RW-40/PM/p03-typing-ui/mutations.log`
1. `M1`: Разделитель в тексте на 2 человек (`и` → `,`) → **CAUGHT** (`AssertionError: Expected 'Alice и Bob печатают…'`).
2. `M2`: Отключение проверки `currentUserId === userId` → **CAUGHT** (`AssertionError: Expected 'idle' but got 'active'`).
3. `M3`: Отключение LRU-вытеснения старого канала при превышении лимита 5 → **CAUGHT** (`AssertionError: c1 должен быть вытеснен как самый старый`).
4. `M4`: Отключение проверки пустого ввода в `notifyInput` → **CAUGHT** (`AssertionError: пустой ввод не должен вызывать send_typing: 1 !== 0`).
5. `M5`: Удаление `data-state="slowmode"` при ошибке 429 → **CAUGHT** (`AssertionError: 'idle' !== 'slowmode'`).

**Итог мутаций**: 5/5 поймано (100%).

## 5. Контрольные суммы файлов (MD5)
```
8c50cf9be15519b2fdc6906769d784ae  rust/gui-spike/ui/features/typing-indicator.js
e39c8a68936367fef0f51fa27a594b31  tests/unit/gui-spike-dom.test.mjs
d5143fd959af43d030eef9b1eb9b0fbd  waves/RW-40/PM/p03-typing-ui/green.log
8960228d6a633814b408fd0b60897873  waves/RW-40/PM/p03-typing-ui/mutations.log
3d1aa35e9acf5ba618cda230521b0169  waves/RW-40/PM/p03-typing-ui/red.log
0a7724826a55a13dae350fbf382a7c2f  waves/RW-40/PM/p03-typing-ui/targeted.log
```

## 6. Риски и ограничения
- Модули `app.js`, `chat.js`, `actions.js`, `composer.js`, `members.js`, `search.js`, `index.html` не модифицировались.
- Интеграция `createTypingController` в жизненный цикл живого `app.js` и маппинг событий через `dispatch-bridge.js` будут выполнены на этапе интеграционного лейнера в соответствии с TDD-планом.
